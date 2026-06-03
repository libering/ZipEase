using System;
using System.IO;
using System.Runtime.InteropServices;

namespace ZipEase.ShellExtension;

/// <summary>
/// IExplorerCommand implementation for the "Compress with ZipEase" context menu item.
/// Shown when any items are selected; hidden when selection is empty.
/// </summary>
[ComVisible(true)]
[Guid("C6B4E2F8-9A3D-5B7C-AE1F-2D406B8C0E3F")]
public sealed class CompressCommand : CommandBase
{
    private const int S_OK = 0;
    private const int E_FAIL = unchecked((int)0x80004005);

    /// <summary>
    /// Returns the localized menu title based on the current system locale.
    /// zh-* → "用 ZipEase 壓縮", otherwise → "Compress with ZipEase"
    /// </summary>
    public override int GetTitle(IShellItemArray? psiItemArray, out string ppszName)
    {
        try
        {
            string locale = LocalizedStrings.GetCurrentLocale();
            ppszName = LocalizedStrings.GetCompressTitle(locale);
            return S_OK;
        }
        catch (Exception ex)
        {
            LogError(nameof(GetTitle), ex);
            ppszName = "Compress with ZipEase";
            return E_FAIL;
        }
    }

    /// <summary>
    /// Returns the path to compress.ico relative to the DLL location.
    /// Returns empty string if the icon file doesn't exist (graceful degradation).
    /// </summary>
    public override int GetIcon(IShellItemArray? psiItemArray, out string ppszIcon)
    {
        try
        {
            ppszIcon = GetIconPathOrEmpty("compress.ico");
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
    /// Returns ECS_ENABLED if items are selected, ECS_HIDDEN if selection is empty.
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
    /// Launches ZipEase.exe with --compress flag and the selected file/folder paths.
    /// </summary>
    public override int Invoke(IShellItemArray? psiItemArray, IntPtr pbc)
    {
        try
        {
            string[] paths = GetSelectedPaths(psiItemArray);
            LaunchZipEase("--compress " + BuildArguments(paths));
            return S_OK;
        }
        catch (Exception ex)
        {
            LogError(nameof(Invoke), ex);
            return E_FAIL;
        }
    }
}
