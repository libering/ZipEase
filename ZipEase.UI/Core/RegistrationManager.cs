using System;
using System.IO;
using System.Threading.Tasks;
using Microsoft.Win32;

namespace ZipEase.UI.Core;

/// <summary>
/// Manages Windows Shell Extension registration for ZipEase context menu integration.
/// Supports two strategies: Sparse MSIX (Windows 11+) and Registry fallback (Windows 10).
/// All operations are per-user (HKCU) and do not require admin privileges.
/// </summary>
public class RegistrationManager
{
    public enum Strategy { SparseMsix, Registry }

    // Registry key paths (HKCU)
    private const string ExtractKeyPath = @"Software\Classes\*\shell\ZipEaseExtract";
    private const string CompressKeyPath = @"Software\Classes\*\shell\ZipEaseCompress";
    private const string DirectoryCompressKeyPath = @"Software\Classes\Directory\shell\ZipEaseCompress";

    // Sparse MSIX package identity
    private const string PackageFamilyName = "ZipEase.App_8wekyb3d8bbwe";
    private const string PackageName = "ZipEase.App";

    /// <summary>
    /// All archive extensions supported by ZipEase for the AppliesTo registry value.
    /// </summary>
    private static readonly string[] SupportedExtensions =
    [
        ".zip", ".7z", ".rar", ".tar", ".gz", ".bz2",
        ".cab", ".iso", ".apk", ".tgz",
        ".001", ".z01", ".z02", ".z03", ".z04", ".z05", ".z06", ".z07", ".z08", ".z09"
    ];

    /// <summary>
    /// Detects the appropriate registration strategy based on OS version.
    /// Windows 11 (Build 22000+) uses Sparse MSIX for modern context menu.
    /// Older versions fall back to traditional registry-based shell extension.
    /// </summary>
    public Strategy DetectStrategy()
    {
        var build = Environment.OSVersion.Version.Build;
        return DetectStrategyForBuild(build);
    }

    /// <summary>
    /// Determines the registration strategy for a given Windows build number.
    /// Build >= 22000 (Windows 11) → SparseMsix; otherwise → Registry.
    /// This method is exposed for testability.
    /// </summary>
    /// <param name="buildNumber">The Windows OS build number.</param>
    /// <returns>The appropriate registration strategy for the given build.</returns>
    public static Strategy DetectStrategyForBuild(int buildNumber)
    {
        return buildNumber >= 22000 ? Strategy.SparseMsix : Strategy.Registry;
    }

    /// <summary>
    /// Registers the shell extension using the best available strategy.
    /// On Windows 11, tries Sparse MSIX first; falls back to Registry on failure.
    /// On Windows 10, uses Registry directly.
    /// </summary>
    public async Task<RegistrationResult> RegisterAsync()
    {
        var strategy = DetectStrategy();

        if (strategy == Strategy.SparseMsix)
        {
            try
            {
                var msixSuccess = await RegisterSparseMsixAsync();
                if (msixSuccess)
                    return new RegistrationResult(true, Strategy.SparseMsix);
            }
            catch (Exception ex)
            {
                // Log warning and fall back to Registry
                System.Diagnostics.Debug.WriteLine(
                    $"[RegistrationManager] Sparse MSIX registration failed, falling back to Registry: {ex.Message}");
            }
        }

        // Registry fallback (or primary strategy for Win10)
        try
        {
            var registrySuccess = RegisterRegistry();
            return registrySuccess
                ? new RegistrationResult(true, Strategy.Registry)
                : new RegistrationResult(false, Strategy.Registry, "Failed to write registry keys.");
        }
        catch (Exception ex)
        {
            return new RegistrationResult(false, Strategy.Registry, ex.Message);
        }
    }

    /// <summary>
    /// Removes all shell extension registrations (both Sparse MSIX and Registry).
    /// Best-effort: attempts both removal methods regardless of which was used to register.
    /// </summary>
    public async Task<RegistrationResult> UnregisterAsync()
    {
        var errors = new System.Collections.Generic.List<string>();

        // Try removing Sparse MSIX registration
        try
        {
            await UnregisterSparseMsixAsync();
        }
        catch (Exception ex)
        {
            errors.Add($"MSIX removal: {ex.Message}");
        }

        // Try removing Registry registration
        try
        {
            UnregisterRegistry();
        }
        catch (Exception ex)
        {
            errors.Add($"Registry removal: {ex.Message}");
        }

        if (errors.Count > 0)
        {
            return new RegistrationResult(false, Strategy.Registry,
                string.Join("; ", errors));
        }

        return new RegistrationResult(true, Strategy.Registry);
    }

    /// <summary>
    /// Checks the current status of shell extension registration.
    /// Returns Enabled if either MSIX package is registered or registry keys exist.
    /// </summary>
    public ShellExtensionStatus CheckStatus()
    {
        try
        {
            // Check registry keys first (works for both strategies)
            using var extractKey = Registry.CurrentUser.OpenSubKey(ExtractKeyPath);
            if (extractKey != null)
                return ShellExtensionStatus.Enabled;

            // Check if MSIX package is registered
            if (IsSparseMsixRegistered())
                return ShellExtensionStatus.Enabled;

            return ShellExtensionStatus.Disabled;
        }
        catch
        {
            return ShellExtensionStatus.Failed;
        }
    }

    /// <summary>
    /// Registers shell extension via HKCU registry keys.
    /// Creates keys for Extract (files), Compress (files), and Compress (directories).
    /// </summary>
    private bool RegisterRegistry()
    {
        var exePath = GetZipEaseExePath();
        var installDir = Path.GetDirectoryName(exePath) ?? ".";
        var extractIcoPath = Path.Combine(installDir, "extract.ico");
        var compressIcoPath = Path.Combine(installDir, "compress.ico");
        var appliesToValue = GenerateAppliesToValue();

        // Extract command: *\shell\ZipEaseExtract
        using (var key = Registry.CurrentUser.CreateSubKey(ExtractKeyPath))
        {
            key.SetValue("", "用 ZipEase 解壓縮");
            key.SetValue("Icon", extractIcoPath);
            key.SetValue("AppliesTo", appliesToValue);

            using var cmdKey = key.CreateSubKey("command");
            cmdKey.SetValue("", $"\"{exePath}\" \"%1\"");
        }

        // Compress command for files: *\shell\ZipEaseCompress
        using (var key = Registry.CurrentUser.CreateSubKey(CompressKeyPath))
        {
            key.SetValue("", "用 ZipEase 壓縮");
            key.SetValue("Icon", compressIcoPath);

            using var cmdKey = key.CreateSubKey("command");
            cmdKey.SetValue("", $"\"{exePath}\" --compress \"%1\"");
        }

        // Compress command for directories: Directory\shell\ZipEaseCompress
        using (var key = Registry.CurrentUser.CreateSubKey(DirectoryCompressKeyPath))
        {
            key.SetValue("", "用 ZipEase 壓縮");
            key.SetValue("Icon", compressIcoPath);

            using var cmdKey = key.CreateSubKey("command");
            cmdKey.SetValue("", $"\"{exePath}\" --compress \"%1\"");
        }

        return true;
    }

    /// <summary>
    /// Removes all ZipEase registry keys from HKCU.
    /// </summary>
    private void UnregisterRegistry()
    {
        TryDeleteSubKeyTree(ExtractKeyPath);
        TryDeleteSubKeyTree(CompressKeyPath);
        TryDeleteSubKeyTree(DirectoryCompressKeyPath);
    }

    /// <summary>
    /// Registers the Sparse MSIX package using Windows.Management.Deployment APIs.
    /// Returns false on failure (caller should fall back to Registry).
    /// </summary>
    private async Task<bool> RegisterSparseMsixAsync()
    {
        try
        {
            var installDir = Path.GetDirectoryName(GetZipEaseExePath()) ?? ".";
            var manifestPath = Path.Combine(installDir, "packaging", "AppxManifest.xml");

            if (!File.Exists(manifestPath))
            {
                System.Diagnostics.Debug.WriteLine(
                    "[RegistrationManager] AppxManifest.xml not found, cannot register Sparse MSIX.");
                return false;
            }

            // Use dynamic loading to avoid hard dependency on WinRT APIs
            // which may not be available on all target platforms
            var packageManagerType = Type.GetType(
                "Windows.Management.Deployment.PackageManager, Windows.Management.Deployment");

            if (packageManagerType == null)
            {
                // WinRT APIs not available — fall back
                System.Diagnostics.Debug.WriteLine(
                    "[RegistrationManager] PackageManager API not available.");
                return false;
            }

            dynamic packageManager = Activator.CreateInstance(packageManagerType)!;
            var manifestUri = new Uri(manifestPath);

            // AddPackageByUriAsync with AllowUnsigned for dev builds
            var deploymentOptions = 0x00000040; // DeploymentOptions.DevelopmentMode (AllowUnsigned)
            var externalLocationUri = new Uri(installDir);

            // Use AddPackageByUriAsync with external location
            var addOptions = Activator.CreateInstance(
                Type.GetType("Windows.Management.Deployment.AddPackageOptions, Windows.Management.Deployment")!);

            // Set AllowUnsigned and ExternalLocationUri via reflection
            var addOptionsType = addOptions!.GetType();
            addOptionsType.GetProperty("AllowUnsigned")?.SetValue(addOptions, true);
            addOptionsType.GetProperty("ExternalLocationUri")?.SetValue(addOptions, externalLocationUri);

            var task = packageManager.AddPackageByUriAsync(manifestUri, addOptions);
            await task.AsTask();

            return true;
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine(
                $"[RegistrationManager] Sparse MSIX registration failed: {ex.Message}");
            return false;
        }
    }

    /// <summary>
    /// Removes the Sparse MSIX package registration.
    /// Best-effort: logs warning on failure but does not throw.
    /// </summary>
    private async Task UnregisterSparseMsixAsync()
    {
        try
        {
            var packageManagerType = Type.GetType(
                "Windows.Management.Deployment.PackageManager, Windows.Management.Deployment");

            if (packageManagerType == null)
                return;

            dynamic packageManager = Activator.CreateInstance(packageManagerType)!;

            // Find the package by family name
            var packages = packageManager.FindPackagesForUser(string.Empty, PackageFamilyName);

            foreach (dynamic package in packages)
            {
                string fullName = package.Id.FullName;
                var task = packageManager.RemovePackageAsync(fullName);
                await task.AsTask();
            }
        }
        catch (Exception ex)
        {
            System.Diagnostics.Debug.WriteLine(
                $"[RegistrationManager] Sparse MSIX unregistration warning: {ex.Message}");
        }
    }

    /// <summary>
    /// Checks if the Sparse MSIX package is currently registered.
    /// </summary>
    private bool IsSparseMsixRegistered()
    {
        try
        {
            var packageManagerType = Type.GetType(
                "Windows.Management.Deployment.PackageManager, Windows.Management.Deployment");

            if (packageManagerType == null)
                return false;

            dynamic packageManager = Activator.CreateInstance(packageManagerType)!;
            var packages = packageManager.FindPackagesForUser(string.Empty, PackageFamilyName);

            foreach (dynamic _ in packages)
                return true;

            return false;
        }
        catch
        {
            return false;
        }
    }

    /// <summary>
    /// Generates the AppliesTo registry value string that filters context menu
    /// visibility to supported archive file extensions only.
    /// Uses Windows Shell AppliesTo syntax: "System.FileExtension:=.zip OR System.FileExtension:=.7z OR ..."
    /// </summary>
    internal static string GenerateAppliesToValue()
    {
        return string.Join(" OR ",
            SupportedExtensions.Select(ext => $"System.FileExtension:={ext}"));
    }

    /// <summary>
    /// Gets the full path to ZipEase.exe (the main application executable).
    /// </summary>
    private static string GetZipEaseExePath()
    {
        return Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "ZipEase.exe");
    }

    /// <summary>
    /// Safely attempts to delete a registry sub-key tree. Does not throw if key doesn't exist.
    /// </summary>
    private static void TryDeleteSubKeyTree(string subKeyPath)
    {
        try
        {
            Registry.CurrentUser.DeleteSubKeyTree(subKeyPath, throwOnMissingSubKey: false);
        }
        catch
        {
            // Best-effort removal — ignore errors for non-existent keys
        }
    }
}
