using System;
using System.Runtime.InteropServices;

namespace ZipEase.ShellExtension;

/// <summary>
/// Explorer command state flags returned by IExplorerCommand.GetState.
/// </summary>
public enum EXPCMDSTATE : uint
{
    ECS_ENABLED = 0,
    ECS_HIDDEN = 2
}

/// <summary>
/// IExplorerCommand COM interface for Windows 11 modern context menu integration.
/// </summary>
[ComImport]
[Guid("a08ce4d0-fa25-44ab-b57c-c7b1c323e0b9")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IExplorerCommand
{
    int GetTitle(IShellItemArray? psiItemArray, out string ppszName);
    int GetIcon(IShellItemArray? psiItemArray, out string ppszIcon);
    int GetToolTip(IShellItemArray? psiItemArray, out string ppszInfotip);
    int GetCanonicalName(out Guid pguidCommandName);
    int GetState(IShellItemArray? psiItemArray, bool fOkToBeSlow, out uint pCmdState);
    int Invoke(IShellItemArray? psiItemArray, IntPtr pbc);
    int GetFlags(out uint pFlags);
    int EnumSubCommands(out IntPtr ppEnum);
}

/// <summary>
/// IShellItemArray COM interface for accessing selected items in Explorer.
/// </summary>
[ComImport]
[Guid("b63ea76d-1f85-456f-a19c-48159efa858b")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IShellItemArray
{
    int BindToHandler(IntPtr pbc, ref Guid bhid, ref Guid riid, out IntPtr ppvOut);
    int GetPropertyStore(int flags, ref Guid riid, out IntPtr ppv);
    int GetPropertyDescriptionList(IntPtr keyType, ref Guid riid, out IntPtr ppv);
    int GetAttributes(int AttribFlags, uint sfgaoMask, out uint psfgaoAttribs);
    int GetCount(out uint pdwNumItems);
    int GetItemAt(uint dwIndex, out IShellItem ppsi);
    int EnumItems(out IEnumShellItems ppenumShellItems);
}

/// <summary>
/// IShellItem COM interface for accessing individual shell items.
/// </summary>
[ComImport]
[Guid("43826d1e-e718-42ee-bc55-a1e261c37bfe")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IShellItem
{
    int BindToHandler(IntPtr pbc, ref Guid bhid, ref Guid riid, out IntPtr ppv);
    int GetParent(out IShellItem ppsi);
    int GetDisplayName(uint sigdnName, out IntPtr ppszName);
    int GetAttributes(uint sfgaoMask, out uint psfgaoAttribs);
    int Compare(IShellItem psi, uint hint, out int piOrder);
}

/// <summary>
/// IEnumShellItems COM interface for enumerating shell items.
/// </summary>
[ComImport]
[Guid("70629033-e363-4a28-a567-0db78006e6d7")]
[InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
public interface IEnumShellItems
{
    int Next(uint celt, out IShellItem rgelt, out uint pceltFetched);
    int Skip(uint celt);
    int Reset();
    int Clone(out IEnumShellItems ppenum);
}
