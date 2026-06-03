using FsCheck;
using FsCheck.Xunit;
using Xunit;
using ZipEase.ShellExtension;

namespace ZipEase.ShellExtension.Tests;

// Feature: context-menu, Property 1: Archive extension classification
/// <summary>
/// Property-based tests verifying that ArchiveExtensions.IsArchiveFile correctly classifies
/// file extensions as archive or non-archive, case-insensitively, for all supported formats.
/// Validates: Requirements 1.1, 1.2, 1.3, 3.4
/// </summary>
public class ArchiveExtensionPropertyTests
{
    /// <summary>
    /// The complete set of supported single-segment archive extensions.
    /// </summary>
    private static readonly string[] SupportedSingleExtensions =
    {
        ".zip", ".7z", ".rar", ".tar", ".gz", ".bz2",
        ".cab", ".iso", ".apk", ".tgz",
        ".001", ".z01", ".z02", ".z03", ".z04", ".z05", ".z06", ".z07", ".z08", ".z09"
    };

    /// <summary>
    /// Compound extensions that require checking the last two segments.
    /// </summary>
    private static readonly string[] SupportedCompoundExtensions =
    {
        ".tar.gz", ".tar.bz2"
    };

    /// <summary>
    /// Extensions that are NOT supported and should return false.
    /// </summary>
    private static readonly string[] UnsupportedExtensions =
    {
        ".txt", ".exe", ".dll", ".pdf", ".doc", ".docx", ".xls", ".xlsx",
        ".png", ".jpg", ".gif", ".bmp", ".mp3", ".mp4", ".avi", ".mkv",
        ".html", ".css", ".js", ".ts", ".cs", ".rs", ".py", ".java",
        ".xml", ".json", ".yaml", ".toml", ".ini", ".cfg", ".log",
        ".bat", ".ps1", ".sh", ".cmd", ".msi", ".sys", ".dat"
    };

    /// <summary>
    /// Generator for random base filenames (without extension).
    /// Includes directory paths to test path handling.
    /// </summary>
    private static Gen<string> GenBaseName()
    {
        var simpleNames = Gen.Elements(
            "file", "archive", "backup", "data", "test", "document",
            "my file", "日本語ファイル", "中文檔案", "données"
        );

        var directories = Gen.Elements(
            @"C:\Users\test\Downloads\",
            @"D:\Archives\",
            @"\\server\share\folder\",
            @"C:\Program Files\App\",
            @"C:\Users\用戶\文件\",
            ""
        );

        return from dir in directories
               from name in simpleNames
               select dir + name;
    }

    /// <summary>
    /// Generator for supported single extensions with random case variants.
    /// </summary>
    private static Gen<string> GenSupportedSingleExtension()
    {
        return from ext in Gen.Elements(SupportedSingleExtensions)
               from variant in GenCaseVariant(ext)
               select variant;
    }

    /// <summary>
    /// Generator for supported compound extensions with random case variants.
    /// </summary>
    private static Gen<string> GenSupportedCompoundExtension()
    {
        return from ext in Gen.Elements(SupportedCompoundExtensions)
               from variant in GenCaseVariant(ext)
               select variant;
    }

    /// <summary>
    /// Generator for unsupported extensions with random case variants.
    /// </summary>
    private static Gen<string> GenUnsupportedExtension()
    {
        return from ext in Gen.Elements(UnsupportedExtensions)
               from variant in GenCaseVariant(ext)
               select variant;
    }

    /// <summary>
    /// Generates a random case variant of the given extension string.
    /// Produces: original, UPPER, lower, or random mixed case.
    /// </summary>
    private static Gen<string> GenCaseVariant(string ext)
    {
        return Gen.Choose(0, 3).Select(variant => variant switch
        {
            0 => ext,                    // original (lowercase)
            1 => ext.ToUpperInvariant(), // ALL CAPS
            2 => ext.ToLowerInvariant(), // all lower
            _ => MixCase(ext)            // mixed case
        });
    }

    /// <summary>
    /// Creates a mixed-case variant by alternating character casing.
    /// </summary>
    private static string MixCase(string s)
    {
        var chars = s.ToCharArray();
        for (int i = 0; i < chars.Length; i++)
        {
            chars[i] = i % 2 == 0
                ? char.ToUpperInvariant(chars[i])
                : char.ToLowerInvariant(chars[i]);
        }
        return new string(chars);
    }

    /// <summary>
    /// Checks if an extension (case-insensitive) is in the supported set.
    /// This is the oracle function for the property test.
    /// </summary>
    private static bool IsInSupportedSet(string filePath)
    {
        if (string.IsNullOrEmpty(filePath))
            return false;

        string fileName = Path.GetFileName(filePath);

        // Check compound extensions
        foreach (var compound in SupportedCompoundExtensions)
        {
            if (fileName.EndsWith(compound, StringComparison.OrdinalIgnoreCase))
                return true;
        }

        // Check single extensions
        string ext = Path.GetExtension(filePath);
        if (string.IsNullOrEmpty(ext))
            return false;

        return SupportedSingleExtensions.Any(
            supported => string.Equals(ext, supported, StringComparison.OrdinalIgnoreCase));
    }

    // ─── Property Tests ───────────────────────────────────────────────────────

    [Property(MaxTest = 100)]
    public Property SupportedSingleExtension_AlwaysReturnsTrue()
    {
        // **Validates: Requirements 1.1, 1.2, 1.3, 3.4**
        var gen = (from baseName in GenBaseName()
                   from ext in GenSupportedSingleExtension()
                   select baseName + ext)
                  .ToArbitrary();

        return Prop.ForAll(gen, filePath =>
            ArchiveExtensions.IsArchiveFile(filePath));
    }

    [Property(MaxTest = 100)]
    public Property SupportedCompoundExtension_AlwaysReturnsTrue()
    {
        // **Validates: Requirements 1.1, 1.2, 1.3, 3.4**
        var gen = (from baseName in GenBaseName()
                   from ext in GenSupportedCompoundExtension()
                   select baseName + ext)
                  .ToArbitrary();

        return Prop.ForAll(gen, filePath =>
            ArchiveExtensions.IsArchiveFile(filePath));
    }

    [Property(MaxTest = 100)]
    public Property UnsupportedExtension_AlwaysReturnsFalse()
    {
        // **Validates: Requirements 1.1, 1.2, 1.3, 3.4**
        var gen = (from baseName in GenBaseName()
                   from ext in GenUnsupportedExtension()
                   select baseName + ext)
                  .ToArbitrary();

        return Prop.ForAll(gen, filePath =>
            !ArchiveExtensions.IsArchiveFile(filePath));
    }

    [Property(MaxTest = 100)]
    public Property CaseVariants_OfSupportedExtensions_AlwaysReturnTrue()
    {
        // **Validates: Requirements 1.1, 1.2, 1.3, 3.4**
        // Specifically tests case-insensitive matching (.ZIP, .Rar, .7Z, etc.)
        var gen = Gen.Elements(SupportedSingleExtensions)
            .Select(ext => ext.ToUpperInvariant())
            .Select(ext => "testfile" + ext)
            .ToArbitrary();

        return Prop.ForAll(gen, filePath =>
            ArchiveExtensions.IsArchiveFile(filePath));
    }

    [Property(MaxTest = 100)]
    public Property IsArchiveFile_MatchesOracleFunction()
    {
        // **Validates: Requirements 1.1, 1.2, 1.3, 3.4**
        // The main property: IsArchiveFile returns true iff extension is in supported set
        var allExtensions = SupportedSingleExtensions
            .Concat(UnsupportedExtensions)
            .ToArray();

        var gen = (from baseName in GenBaseName()
                   from ext in Gen.Elements(allExtensions).SelectMany(e => GenCaseVariant(e))
                   select baseName + ext)
                  .ToArbitrary();

        return Prop.ForAll(gen, filePath =>
            ArchiveExtensions.IsArchiveFile(filePath) == IsInSupportedSet(filePath));
    }

    [Property(MaxTest = 100)]
    public Property PathsWithDirectories_DoNotAffectClassification()
    {
        // **Validates: Requirements 1.1, 1.2, 1.3, 3.4**
        // Verifies that directory components don't interfere with extension detection
        var directories = Gen.Elements(
            @"C:\Users\test\Downloads\",
            @"D:\My Archives\backup\",
            @"\\network\share\folder.zip\subfolder\",
            @"C:\folder.tar.gz\nested\"
        );

        var gen = (from dir in directories
                   from ext in GenSupportedSingleExtension()
                   select dir + "archive" + ext)
                  .ToArbitrary();

        return Prop.ForAll(gen, filePath =>
            ArchiveExtensions.IsArchiveFile(filePath));
    }
}
