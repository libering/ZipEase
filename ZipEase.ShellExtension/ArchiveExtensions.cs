using System;
using System.Collections.Generic;
using System.IO;

namespace ZipEase.ShellExtension;

/// <summary>
/// Provides archive file extension detection for the shell extension context menu.
/// Used by ExtractCommand to determine whether to show or hide the "Extract" menu item.
/// </summary>
public static class ArchiveExtensions
{
    /// <summary>
    /// Single-segment supported archive extensions (case-insensitive).
    /// </summary>
    private static readonly HashSet<string> SupportedExtensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".zip", ".7z", ".rar", ".tar", ".gz", ".bz2",
        ".cab", ".iso", ".apk", ".tgz",
        ".001", ".z01", ".z02", ".z03", ".z04", ".z05", ".z06", ".z07", ".z08", ".z09"
    };

    /// <summary>
    /// Compound extensions that require checking the last two segments of the filename.
    /// </summary>
    private static readonly HashSet<string> CompoundExtensions = new(StringComparer.OrdinalIgnoreCase)
    {
        ".tar.gz", ".tar.bz2"
    };

    /// <summary>
    /// Determines whether the given file path has a supported archive extension.
    /// Supports both single extensions (.zip, .7z) and compound extensions (.tar.gz, .tar.bz2).
    /// Matching is case-insensitive.
    /// </summary>
    /// <param name="filePath">The file path to check.</param>
    /// <returns>true if the file has a supported archive extension; otherwise, false.</returns>
    public static bool IsArchiveFile(string filePath)
    {
        if (string.IsNullOrEmpty(filePath))
            return false;

        // Check compound extensions first (e.g., .tar.gz, .tar.bz2)
        string fileName = Path.GetFileName(filePath);
        foreach (string compound in CompoundExtensions)
        {
            if (fileName.EndsWith(compound, StringComparison.OrdinalIgnoreCase))
                return true;
        }

        // Check single-segment extension
        string extension = Path.GetExtension(filePath);
        if (string.IsNullOrEmpty(extension))
            return false;

        return SupportedExtensions.Contains(extension);
    }
}
