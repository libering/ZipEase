using System.Collections;
using System.Runtime.InteropServices;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// Uses Windows StrCmpLogicalW for natural sort order (1, 2, 10 instead of 1, 10, 2).
    /// </summary>
    public sealed class NaturalFileNameComparer : IComparer
    {
        public static readonly NaturalFileNameComparer Instance = new();

        [DllImport("shlwapi.dll", CharSet = CharSet.Unicode)]
        private static extern int StrCmpLogicalW(string x, string y);

        public int Compare(object? x, object? y)
        {
            var a = (x as ArchiveEntryViewModel)?.FileName ?? string.Empty;
            var b = (y as ArchiveEntryViewModel)?.FileName ?? string.Empty;
            return StrCmpLogicalW(a, b);
        }
    }
}
