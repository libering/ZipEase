using FsCheck;
using FsCheck.Xunit;
using Xunit;
using ZipEase.ShellExtension;

namespace ZipEase.ShellExtension.Tests;

// Feature: context-menu, Property 2: Command line construction preserves all paths
/// <summary>
/// Property-based tests verifying that CommandBase.BuildArguments correctly constructs
/// a quoted command line string that preserves all input paths, including paths with
/// spaces, unicode characters, and special characters.
/// Validates: Requirements 1.4, 1.5, 2.2, 2.3
/// </summary>
public class CommandLineConstructionPropertyTests
{
    // ─── Generators ───────────────────────────────────────────────────────────

    /// <summary>
    /// Generator for paths with spaces.
    /// </summary>
    private static Gen<string> GenPathWithSpaces()
    {
        return Gen.Elements(
            @"C:\My Documents\file.zip",
            @"C:\Program Files\ZipEase\archive.7z",
            @"D:\User Data\My Files\backup 2024.rar",
            @"C:\Users\John Doe\Downloads\test file.tar.gz",
            @"E:\path with   multiple spaces\data.cab",
            @"C:\New Folder (2)\archive copy.zip"
        );
    }

    /// <summary>
    /// Generator for paths with unicode characters.
    /// </summary>
    private static Gen<string> GenPathWithUnicode()
    {
        return Gen.Elements(
            @"C:\用戶\文件\archive.7z",
            @"C:\Users\日本語\ファイル.zip",
            @"D:\données\sauvegarde.rar",
            @"C:\Пользователи\Документы\архив.tar",
            @"C:\사용자\문서\압축파일.zip",
            @"C:\المستخدمين\المستندات\ملف.7z",
            @"C:\Ñoño\café\résumé.gz"
        );
    }

    /// <summary>
    /// Generator for paths with special characters (ampersands, parentheses, etc.).
    /// Note: We exclude embedded double-quotes since BuildArguments uses simple quoting.
    /// </summary>
    private static Gen<string> GenPathWithSpecialChars()
    {
        return Gen.Elements(
            @"C:\Tom & Jerry\archive.zip",
            @"C:\folder (1)\backup [final].7z",
            @"D:\100% complete\data.rar",
            @"C:\path;with;semicolons\file.tar",
            @"C:\exclaim!\file.gz",
            @"C:\hash#tag\file.bz2",
            @"C:\dollar$sign\file.cab",
            @"C:\at@sign\file.iso",
            @"C:\caret^mark\file.apk",
            @"C:\equals=sign\file.tgz"
        );
    }

    /// <summary>
    /// Generator for simple paths without special characters.
    /// </summary>
    private static Gen<string> GenSimplePath()
    {
        return Gen.Elements(
            @"C:\archive.zip",
            @"D:\backup.7z",
            @"E:\data.rar",
            @"C:\Users\test\file.tar",
            @"D:\Downloads\package.gz"
        );
    }

    /// <summary>
    /// Generator for any valid path (mix of all types).
    /// Excludes paths containing embedded double-quote characters since
    /// BuildArguments uses simple quoting without escape sequences.
    /// </summary>
    private static Gen<string> GenAnyPath()
    {
        return Gen.OneOf(
            GenPathWithSpaces(),
            GenPathWithUnicode(),
            GenPathWithSpecialChars(),
            GenSimplePath()
        );
    }

    /// <summary>
    /// Generator for non-empty arrays of paths (1 to 10 paths).
    /// </summary>
    private static Gen<string[]> GenPathArray()
    {
        return from count in Gen.Choose(1, 10)
               from paths in Gen.ArrayOf(count, GenAnyPath())
               select paths;
    }

    // ─── Round-trip Parser (Oracle) ───────────────────────────────────────────

    /// <summary>
    /// Parses a Windows-style quoted command line string back into individual arguments.
    /// This follows the standard Windows command-line parsing convention:
    /// - Arguments are separated by whitespace
    /// - Arguments enclosed in double quotes preserve internal whitespace
    /// - The quotes themselves are stripped from the result
    /// </summary>
    private static string[] ParseCommandLine(string commandLine)
    {
        if (string.IsNullOrEmpty(commandLine))
            return Array.Empty<string>();

        var args = new List<string>();
        int i = 0;

        while (i < commandLine.Length)
        {
            // Skip whitespace between arguments
            while (i < commandLine.Length && char.IsWhiteSpace(commandLine[i]))
                i++;

            if (i >= commandLine.Length)
                break;

            if (commandLine[i] == '"')
            {
                // Quoted argument - find closing quote
                i++; // skip opening quote
                int start = i;
                while (i < commandLine.Length && commandLine[i] != '"')
                    i++;

                args.Add(commandLine[start..i]);

                if (i < commandLine.Length)
                    i++; // skip closing quote
            }
            else
            {
                // Unquoted argument - read until whitespace
                int start = i;
                while (i < commandLine.Length && !char.IsWhiteSpace(commandLine[i]))
                    i++;

                args.Add(commandLine[start..i]);
            }
        }

        return args.ToArray();
    }

    // ─── Property Tests ───────────────────────────────────────────────────────

    [Property(MaxTest = 100)]
    public Property BuildArguments_ContainsAllPaths()
    {
        // **Validates: Requirements 1.4, 1.5, 2.2, 2.3**
        // Every input path must appear in the constructed command line string.
        var gen = GenPathArray().ToArbitrary();

        return Prop.ForAll(gen, paths =>
        {
            string result = CommandBase.BuildArguments(paths);

            foreach (string path in paths)
            {
                if (!result.Contains(path))
                    return false;
            }
            return true;
        });
    }

    [Property(MaxTest = 100)]
    public Property BuildArguments_RoundTrip_PreservesAllPaths()
    {
        // **Validates: Requirements 1.4, 1.5, 2.2, 2.3**
        // Parsing the constructed command line back must yield the original paths.
        var gen = GenPathArray().ToArbitrary();

        return Prop.ForAll(gen, paths =>
        {
            string commandLine = CommandBase.BuildArguments(paths);
            string[] parsed = ParseCommandLine(commandLine);

            if (parsed.Length != paths.Length)
                return false;

            for (int i = 0; i < paths.Length; i++)
            {
                if (parsed[i] != paths[i])
                    return false;
            }
            return true;
        });
    }

    [Property(MaxTest = 100)]
    public Property BuildArguments_PathsWithSpaces_AreCorrectlyQuoted()
    {
        // **Validates: Requirements 1.4, 1.5, 2.2, 2.3**
        // Paths with spaces must be wrapped in quotes to preserve them.
        var gen = (from count in Gen.Choose(1, 5)
                   from paths in Gen.ArrayOf(count, GenPathWithSpaces())
                   select paths).ToArbitrary();

        return Prop.ForAll(gen, paths =>
        {
            string commandLine = CommandBase.BuildArguments(paths);
            string[] parsed = ParseCommandLine(commandLine);

            if (parsed.Length != paths.Length)
                return false;

            for (int i = 0; i < paths.Length; i++)
            {
                if (parsed[i] != paths[i])
                    return false;
            }
            return true;
        });
    }

    [Property(MaxTest = 100)]
    public Property BuildArguments_PathsWithUnicode_ArePreserved()
    {
        // **Validates: Requirements 1.4, 1.5, 2.2, 2.3**
        // Unicode characters in paths must be preserved through the round-trip.
        var gen = (from count in Gen.Choose(1, 5)
                   from paths in Gen.ArrayOf(count, GenPathWithUnicode())
                   select paths).ToArbitrary();

        return Prop.ForAll(gen, paths =>
        {
            string commandLine = CommandBase.BuildArguments(paths);
            string[] parsed = ParseCommandLine(commandLine);

            if (parsed.Length != paths.Length)
                return false;

            for (int i = 0; i < paths.Length; i++)
            {
                if (parsed[i] != paths[i])
                    return false;
            }
            return true;
        });
    }

    [Property(MaxTest = 100)]
    public Property BuildArguments_PathsWithSpecialChars_ArePreserved()
    {
        // **Validates: Requirements 1.4, 1.5, 2.2, 2.3**
        // Special characters (ampersands, parentheses, etc.) must survive the round-trip.
        var gen = (from count in Gen.Choose(1, 5)
                   from paths in Gen.ArrayOf(count, GenPathWithSpecialChars())
                   select paths).ToArbitrary();

        return Prop.ForAll(gen, paths =>
        {
            string commandLine = CommandBase.BuildArguments(paths);
            string[] parsed = ParseCommandLine(commandLine);

            if (parsed.Length != paths.Length)
                return false;

            for (int i = 0; i < paths.Length; i++)
            {
                if (parsed[i] != paths[i])
                    return false;
            }
            return true;
        });
    }

    [Property(MaxTest = 100)]
    public Property BuildArguments_EachPathQuotedExactlyOnce()
    {
        // **Validates: Requirements 1.4, 1.5, 2.2, 2.3**
        // Each path should appear exactly once in the output (no duplication).
        var gen = GenPathArray().ToArbitrary();

        return Prop.ForAll(gen, paths =>
        {
            string commandLine = CommandBase.BuildArguments(paths);
            string[] parsed = ParseCommandLine(commandLine);

            // The number of parsed arguments must equal the number of input paths
            return parsed.Length == paths.Length;
        });
    }

    [Property(MaxTest = 100)]
    public Property BuildArguments_OutputFormat_IsQuotedAndSpaceSeparated()
    {
        // **Validates: Requirements 1.4, 1.5, 2.2, 2.3**
        // The output format must be: "path1" "path2" "path3"
        var gen = GenPathArray().ToArbitrary();

        return Prop.ForAll(gen, paths =>
        {
            string commandLine = CommandBase.BuildArguments(paths);

            // Each path should be wrapped in quotes
            foreach (string path in paths)
            {
                string quoted = $"\"{path}\"";
                if (!commandLine.Contains(quoted))
                    return false;
            }
            return true;
        });
    }

    [Property(MaxTest = 100)]
    public Property BuildArguments_EmptyArray_ReturnsEmptyString()
    {
        // **Validates: Requirements 1.4, 1.5, 2.2, 2.3**
        // Edge case: empty input should produce empty output.
        return Prop.ForAll(Arb.From(Gen.Constant(Array.Empty<string>())), paths =>
        {
            string result = CommandBase.BuildArguments(paths);
            return result == string.Empty;
        });
    }
}
