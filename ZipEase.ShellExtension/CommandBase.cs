using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;

namespace ZipEase.ShellExtension;

/// <summary>
/// Abstract base class for IExplorerCommand implementations.
/// All methods are wrapped in try-catch to ensure explorer.exe never crashes.
/// Exceptions are logged to %TEMP%\ZipEase_shell.log.
/// </summary>
public abstract class CommandBase : IExplorerCommand
{
    private const int S_OK = 0;
    private const int E_FAIL = unchecked((int)0x80004005);

    // SIGDN_FILESYSPATH = 0x80058000
    private const uint SIGDN_FILESYSPATH = 0x80058000;

    private static readonly string LogPath = Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "Temp",
        "ZipEase_shell.log");

    public abstract int GetTitle(IShellItemArray? psiItemArray, out string ppszName);
    public abstract int GetIcon(IShellItemArray? psiItemArray, out string ppszIcon);
    public abstract int GetState(IShellItemArray? psiItemArray, bool fOkToBeSlow, out uint pCmdState);
    public abstract int Invoke(IShellItemArray? psiItemArray, IntPtr pbc);

    public virtual int GetToolTip(IShellItemArray? psiItemArray, out string ppszInfotip)
    {
        ppszInfotip = string.Empty;
        return S_OK;
    }

    public virtual int GetCanonicalName(out Guid pguidCommandName)
    {
        pguidCommandName = Guid.Empty;
        return S_OK;
    }

    public virtual int GetFlags(out uint pFlags)
    {
        pFlags = 0;
        return S_OK;
    }

    public virtual int EnumSubCommands(out IntPtr ppEnum)
    {
        ppEnum = IntPtr.Zero;
        return S_OK;
    }

    /// <summary>
    /// Enumerates the shell item array and returns file system paths for all selected items.
    /// </summary>
    protected string[] GetSelectedPaths(IShellItemArray? psiItemArray)
    {
        if (psiItemArray == null)
            return Array.Empty<string>();

        try
        {
            psiItemArray.GetCount(out uint count);
            var paths = new List<string>((int)count);

            for (uint i = 0; i < count; i++)
            {
                psiItemArray.GetItemAt(i, out IShellItem item);
                if (item != null)
                {
                    int hr = item.GetDisplayName(SIGDN_FILESYSPATH, out IntPtr pszName);
                    if (hr == S_OK && pszName != IntPtr.Zero)
                    {
                        string? path = Marshal.PtrToStringUni(pszName);
                        Marshal.FreeCoTaskMem(pszName);
                        if (!string.IsNullOrEmpty(path))
                        {
                            paths.Add(path);
                        }
                    }
                }
            }

            return paths.ToArray();
        }
        catch (Exception ex)
        {
            LogError(nameof(GetSelectedPaths), ex);
            return Array.Empty<string>();
        }
    }

    /// <summary>
    /// Finds ZipEase.exe relative to the shell extension DLL location.
    /// </summary>
    protected string GetZipEaseExePath()
    {
        try
        {
            string dllDir = Path.GetDirectoryName(typeof(CommandBase).Assembly.Location)
                ?? string.Empty;
            return Path.Combine(dllDir, "ZipEase.exe");
        }
        catch (Exception ex)
        {
            LogError(nameof(GetZipEaseExePath), ex);
            return string.Empty;
        }
    }

    /// <summary>
    /// Launches ZipEase.exe with the specified arguments.
    /// </summary>
    protected void LaunchZipEase(string arguments)
    {
        try
        {
            string exePath = GetZipEaseExePath();
            if (string.IsNullOrEmpty(exePath) || !File.Exists(exePath))
            {
                LogError(nameof(LaunchZipEase), $"ZipEase.exe not found at: {exePath}");
                return;
            }

            Process.Start(new ProcessStartInfo
            {
                FileName = exePath,
                Arguments = arguments,
                UseShellExecute = false
            });
        }
        catch (Exception ex)
        {
            LogError(nameof(LaunchZipEase), ex);
        }
    }

    /// <summary>
    /// Builds a command line argument string by quoting each path and joining with spaces.
    /// </summary>
    public static string BuildArguments(string[] paths)
    {
        if (paths == null || paths.Length == 0)
            return string.Empty;

        var quoted = new string[paths.Length];
        for (int i = 0; i < paths.Length; i++)
        {
            quoted[i] = $"\"{paths[i]}\"";
        }

        return string.Join(" ", quoted);
    }

    /// <summary>
    /// Returns the icon path if the .ico file exists, or empty string for graceful degradation.
    /// When the icon file is missing, the context menu item still appears but without an icon.
    /// </summary>
    protected string GetIconPathOrEmpty(string iconFileName)
    {
        try
        {
            string dllDir = Path.GetDirectoryName(typeof(CommandBase).Assembly.Location)
                ?? string.Empty;
            string iconPath = Path.Combine(dllDir, iconFileName);
            return File.Exists(iconPath) ? iconPath : string.Empty;
        }
        catch (Exception ex)
        {
            LogError(nameof(GetIconPathOrEmpty), ex);
            return string.Empty;
        }
    }

    /// <summary>
    /// Logs an error message to the ZipEase shell extension log file.
    /// </summary>
    protected static void LogError(string method, Exception ex)
    {
        LogError(method, $"{ex.GetType().Name}: {ex.Message}");
    }

    /// <summary>
    /// Logs an error message to the ZipEase shell extension log file.
    /// </summary>
    protected static void LogError(string method, string message)
    {
        try
        {
            string logEntry = $"[{DateTime.UtcNow:yyyy-MM-dd HH:mm:ss}] [{method}] {message}{Environment.NewLine}";
            File.AppendAllText(LogPath, logEntry);
        }
        catch
        {
            // Swallow logging failures — never crash explorer.exe
        }
    }
}
