using System;
using System.IO;
using System.Runtime.InteropServices;

namespace ZipEase.ShellExtension;

/// <summary>
/// IExplorerCommand implementation for the "Extract with ZipEase" context menu item.
/// Shown only when all selected files are supported archive formats.
/// </summary>
[ComVisible(true)]
[Guid("B5A3D1E7-8F2C-4A6B-9D0E-1C3F5A7B9D2E")]
public sealed class ExtractCommand : CommandBase
{
    private const int S_OK = 0;
    private const int E_FAIL = unchecked((int)0x80004005);

    /// <summary>
    /// Returns the localized menu title based on the current system locale.
    /// zh-* → "用 ZipEase 解壓縮", otherwise → "Extract with ZipEase"
    /// </summary>
    public override int GetTitle(IShellItemArray? psiItemArray, out string ppszName)
    {
        try
        {
            string locale = LocalizedStrings.GetCurrentLocale();
            ppszName = LocalizedStrings.GetExtractTitle(locale);
            return S_OK;
        }
        catch (Exception ex)
        {
            LogError(nameof(GetTitle), ex);
            ppszName = "Extract with ZipEase";
            return E_FAIL;
        }
    }

    /// <summary>
    /// Returns the path to extract.ico relative to the DLL location.
    /// Returns empty string if the icon file doesn't exist (graceful degradation).
    /// </summary>
    public override int GetIcon(IShellItemArray? psiItemArray, out string ppszIcon)
    {
        try
        {
            ppszIcon = GetIconPathOrEmpty("extract.ico");
            return S_OK;
        }
        catch (Exception ex)
        {
            LogError(nameof(GetIcon), ex);
            ppszIcon = string.Empty;
            return E_FAIL;
        }
    }

    /// <summary>
    /// Returns ECS_ENABLED if ALL selected files are archive files, ECS_HIDDEN otherwise.
    /// </summary>
    public override int GetState(IShellItemArray? psiItemArray, bool fOkToBeSlow, out uint pCmdState)
    {
        try
        {
            string[] paths = GetSelectedPaths(psiItemArray);

            if (paths.Length == 0)
            {
                pCmdState = (uint)EXPCMDSTATE.ECS_HIDDEN;
                return S_OK;
            }

            // All selected files must be archive files
            foreach (string path in paths)
            {
                if (!ArchiveExtensions.IsArchiveFile(path))
                {
                    pCmdState = (uint)EXPCMDSTATE.ECS_HIDDEN;
                    return S_OK;
                }
            }

            pCmdState = (uint)EXPCMDSTATE.ECS_ENABLED;
            return S_OK;
        }
        catch (Exception ex)
        {
            LogError(nameof(GetState), ex);
            pCmdState = (uint)EXPCMDSTATE.ECS_HIDDEN;
            return E_FAIL;
        }
    }

    /// <summary>
    /// Launches ZipEase.exe with the selected archive file paths.
    /// </summary>
    public override int Invoke(IShellItemArray? psiItemArray, IntPtr pbc)
    {
        try
        {
            string[] paths = GetSelectedPaths(psiItemArray);
            LaunchZipEase(BuildArguments(paths));
            return S_OK;
        }
        catch (Exception ex)
        {
            LogError(nameof(Invoke), ex);
            return E_FAIL;
        }
    }
}
