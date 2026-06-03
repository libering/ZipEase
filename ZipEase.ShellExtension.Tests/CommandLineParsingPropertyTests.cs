using FsCheck;
using FsCheck.Xunit;
using Xunit;
using ZipEase.UI.Core;
using static ZipEase.UI.Core.CommandLineParser;

namespace ZipEase.ShellExtension.Tests;

// Feature: context-menu, Property 3: Command line parsing round-trip
/// <summary>
/// Property-based tests verifying that CommandLineParser.Parse correctly identifies
/// the mode and returns all paths that exist on disk, filtering out non-existent paths
/// without error.
/// Validates: Requirements 7.1, 7.2, 7.5, 7.6
/// </summary>
public class CommandLineParsingPropertyTests
{
    // ─── Generators ───────────────────────────────────────────────────────────

    /// <summary>
    /// Generator for paths that definitely do NOT exist on disk.
    /// Uses random GUIDs in the path to ensure non-existence.
    /// </summary>
    private static Gen<string> GenNonExistentPath()
    {
        return from guid in Gen.Elements(
                   Guid.NewGuid().ToString("N"),
                   Guid.NewGuid().ToString("N"),
                   Guid.NewGuid().ToString("N"),
                   Guid.NewGuid().ToString("N"),
                   Guid.NewGuid().ToString("N"))
               from prefix in Gen.Elements(
                   @"C:\NonExistent_",
                   @"Z:\FakeDrive_",
                   @"X:\NoSuchFolder\SubDir_",
                   @"C:\Users\NoUser_",
                   @"D:\Missing_")
               from ext in Gen.Elements(".zip", ".7z", ".rar", ".txt", ".dat", "")
               select prefix + guid + ext;
    }

    /// <summary>
    /// Generator for arrays of non-existent paths (1 to 8 paths).
    /// </summary>
    private static Gen<string[]> GenNonExistentPathArray()
    {
        return from count in Gen.Choose(1, 8)
               from paths in Gen.ArrayOf(count, GenNonExistentPath())
               select paths;
    }

    /// <summary>
    /// Generator for paths that DO exist on disk (uses temp files).
    /// Returns the path to a temp file that exists.
    /// </summary>
    private static string CreateTempFile()
    {
        string path = Path.GetTempFileName();
        return path;
    }

    /// <summary>
    /// Generator for the --register-shell flag with optional extra args.
    /// </summary>
    private static Gen<string[]> GenRegisterShellArgs()
    {
        return from extraBefore in Gen.Choose(0, 2)
               from extraAfter in Gen.Choose(0, 2)
               from beforePaths in Gen.ArrayOf(extraBefore, GenNonExistentPath())
               from afterPaths in Gen.ArrayOf(extraAfter, GenNonExistentPath())
               select beforePaths.Append("--register-shell").Concat(afterPaths).ToArray();
    }

    /// <summary>
    /// Generator for the --unregister-shell flag with optional extra args.
    /// </summary>
    private static Gen<string[]> GenUnregisterShellArgs()
    {
        return from extraBefore in Gen.Choose(0, 2)
               from extraAfter in Gen.Choose(0, 2)
               from beforePaths in Gen.ArrayOf(extraBefore, GenNonExistentPath())
               from afterPaths in Gen.ArrayOf(extraAfter, GenNonExistentPath())
               select beforePaths.Append("--unregister-shell").Concat(afterPaths).ToArray();
    }

    /// <summary>
    /// Generator for case variants of the --register-shell flag.
    /// </summary>
    private static Gen<string> GenRegisterShellFlag()
    {
        return Gen.Elements(
            "--register-shell",
            "--Register-Shell",
            "--REGISTER-SHELL",
            "--Register-shell"
        );
    }

    /// <summary>
    /// Generator for case variants of the --unregister-shell flag.
    /// </summary>
    private static Gen<string> GenUnregisterShellFlag()
    {
        return Gen.Elements(
            "--unregister-shell",
            "--Unregister-Shell",
            "--UNREGISTER-SHELL",
            "--Unregister-shell"
        );
    }

    /// <summary>
    /// Generator for case variants of the --compress flag.
    /// </summary>
    private static Gen<string> GenCompressFlag()
    {
        return Gen.Elements(
            "--compress",
            "--Compress",
            "--COMPRESS",
            "--Compress"
        );
    }

    // ─── Property Tests ───────────────────────────────────────────────────────

    [Property(MaxTest = 100)]
    public Property EmptyArgs_ReturnsNormalMode()
    {
        // **Validates: Requirements 7.1, 7.2, 7.5, 7.6**
        // No arguments → Normal mode with empty paths.
        return Prop.ForAll(Arb.From(Gen.Constant(Array.Empty<string>())), args =>
        {
            var result = CommandLineParser.Parse(args);
            return result.Mode == Mode.Normal && result.ValidPaths.Length == 0;
        });
    }

    [Property(MaxTest = 100)]
    public Property NullArgs_ReturnsNormalMode()
    {
        // **Validates: Requirements 7.1, 7.2, 7.5, 7.6**
        // Null arguments → Normal mode with empty paths.
        return Prop.ForAll(Arb.From(Gen.Constant((string[]?)null)), args =>
        {
            var result = CommandLineParser.Parse(args!);
            return result.Mode == Mode.Normal && result.ValidPaths.Length == 0;
        });
    }

    [Property(MaxTest = 100)]
    public Property RegisterShellFlag_ReturnsRegisterShellMode()
    {
        // **Validates: Requirements 7.1, 7.2, 7.5, 7.6**
        // --register-shell (case-insensitive) anywhere in args → RegisterShell mode.
        var gen = (from flag in GenRegisterShellFlag()
                   from extraBefore in Gen.Choose(0, 3)
                   from extraAfter in Gen.Choose(0, 3)
                   from before in Gen.ArrayOf(extraBefore, GenNonExistentPath())
                   from after in Gen.ArrayOf(extraAfter, GenNonExistentPath())
                   select before.Append(flag).Concat(after).ToArray())
                  .ToArbitrary();

        return Prop.ForAll(gen, args =>
        {
            var result = CommandLineParser.Parse(args);
            return result.Mode == Mode.RegisterShell && result.ValidPaths.Length == 0;
        });
    }

    [Property(MaxTest = 100)]
    public Property UnregisterShellFlag_ReturnsUnregisterShellMode()
    {
        // **Validates: Requirements 7.1, 7.2, 7.5, 7.6**
        // --unregister-shell (case-insensitive) anywhere in args → UnregisterShell mode.
        var gen = (from flag in GenUnregisterShellFlag()
                   from extraBefore in Gen.Choose(0, 3)
                   from extraAfter in Gen.Choose(0, 3)
                   from before in Gen.ArrayOf(extraBefore, GenNonExistentPath())
                   from after in Gen.ArrayOf(extraAfter, GenNonExistentPath())
                   select before.Append(flag).Concat(after).ToArray())
                  .ToArbitrary();

        return Prop.ForAll(gen, args =>
        {
            var result = CommandLineParser.Parse(args);
            return result.Mode == Mode.UnregisterShell && result.ValidPaths.Length == 0;
        });
    }

    [Property(MaxTest = 100)]
    public Property CompressFlag_WithNonExistentPaths_ReturnsNormalMode()
    {
        // **Validates: Requirements 7.1, 7.2, 7.5, 7.6**
        // --compress + all non-existent paths → Normal mode (all paths filtered out).
        var gen = (from flag in GenCompressFlag()
                   from paths in GenNonExistentPathArray()
                   select new[] { flag }.Concat(paths).ToArray())
                  .ToArbitrary();

        return Prop.ForAll(gen, args =>
        {
            var result = CommandLineParser.Parse(args);
            return result.Mode == Mode.Normal && result.ValidPaths.Length == 0;
        });
    }

    [Property(MaxTest = 100)]
    public Property CompressFlag_WithExistingPaths_ReturnsCompressMode()
    {
        // **Validates: Requirements 7.1, 7.2, 7.5, 7.6**
        // --compress + existing paths → Compress mode with those paths.
        var gen = (from flag in GenCompressFlag()
                   from pathCount in Gen.Choose(1, 4)
                   select (flag, pathCount))
                  .ToArbitrary();

        return Prop.ForAll(gen, input =>
        {
            var tempFiles = new List<string>();
            try
            {
                for (int i = 0; i < input.pathCount; i++)
                    tempFiles.Add(CreateTempFile());

                var args = new[] { input.flag }.Concat(tempFiles).ToArray();
                var result = CommandLineParser.Parse(args);

                return result.Mode == Mode.Compress
                    && result.ValidPaths.Length == tempFiles.Count
                    && tempFiles.All(f => result.ValidPaths.Contains(f));
            }
            finally
            {
                foreach (var f in tempFiles)
                    if (File.Exists(f)) File.Delete(f);
            }
        });
    }

    [Property(MaxTest = 100)]
    public Property BarePaths_NonExistent_ReturnsNormalMode()
    {
        // **Validates: Requirements 7.1, 7.2, 7.5, 7.6**
        // Bare paths (no flags) that don't exist → Normal mode.
        var gen = GenNonExistentPathArray().ToArbitrary();

        return Prop.ForAll(gen, args =>
        {
            var result = CommandLineParser.Parse(args);
            return result.Mode == Mode.Normal && result.ValidPaths.Length == 0;
        });
    }

    [Property(MaxTest = 100)]
    public Property BarePaths_Existing_ReturnsExtractMode()
    {
        // **Validates: Requirements 7.1, 7.2, 7.5, 7.6**
        // Bare paths (no flags) that exist → Extract mode with those paths.
        var gen = Gen.Choose(1, 4).ToArbitrary();

        return Prop.ForAll(gen, pathCount =>
        {
            var tempFiles = new List<string>();
            try
            {
                for (int i = 0; i < pathCount; i++)
                    tempFiles.Add(CreateTempFile());

                var args = tempFiles.ToArray();
                var result = CommandLineParser.Parse(args);

                return result.Mode == Mode.Extract
                    && result.ValidPaths.Length == tempFiles.Count
                    && tempFiles.All(f => result.ValidPaths.Contains(f));
            }
            finally
            {
                foreach (var f in tempFiles)
                    if (File.Exists(f)) File.Delete(f);
            }
        });
    }

    [Property(MaxTest = 100)]
    public Property MixedPaths_OnlyExistingPathsKept()
    {
        // **Validates: Requirements 7.1, 7.2, 7.5, 7.6**
        // Mix of existing and non-existing paths → only existing paths are returned.
        var gen = (from existingCount in Gen.Choose(1, 3)
                   from nonExistingCount in Gen.Choose(1, 3)
                   from nonExisting in Gen.ArrayOf(nonExistingCount, GenNonExistentPath())
                   select (existingCount, nonExisting))
                  .ToArbitrary();

        return Prop.ForAll(gen, input =>
        {
            var tempFiles = new List<string>();
            try
            {
                for (int i = 0; i < input.existingCount; i++)
                    tempFiles.Add(CreateTempFile());

                // Interleave existing and non-existing paths
                var allPaths = new List<string>();
                int existIdx = 0, nonExistIdx = 0;
                while (existIdx < tempFiles.Count || nonExistIdx < input.nonExisting.Length)
                {
                    if (existIdx < tempFiles.Count)
                        allPaths.Add(tempFiles[existIdx++]);
                    if (nonExistIdx < input.nonExisting.Length)
                        allPaths.Add(input.nonExisting[nonExistIdx++]);
                }

                var result = CommandLineParser.Parse(allPaths.ToArray());

                // Should be Extract mode (bare paths with at least one valid)
                return result.Mode == Mode.Extract
                    && result.ValidPaths.Length == tempFiles.Count
                    && tempFiles.All(f => result.ValidPaths.Contains(f))
                    && input.nonExisting.All(f => !result.ValidPaths.Contains(f));
            }
            finally
            {
                foreach (var f in tempFiles)
                    if (File.Exists(f)) File.Delete(f);
            }
        });
    }

    [Property(MaxTest = 100)]
    public Property CompressFlag_MixedPaths_OnlyExistingPathsKept()
    {
        // **Validates: Requirements 7.1, 7.2, 7.5, 7.6**
        // --compress + mix of existing/non-existing → Compress mode with only existing paths.
        var gen = (from flag in GenCompressFlag()
                   from existingCount in Gen.Choose(1, 3)
                   from nonExistingCount in Gen.Choose(1, 3)
                   from nonExisting in Gen.ArrayOf(nonExistingCount, GenNonExistentPath())
                   select (flag, existingCount, nonExisting))
                  .ToArbitrary();

        return Prop.ForAll(gen, input =>
        {
            var tempFiles = new List<string>();
            try
            {
                for (int i = 0; i < input.existingCount; i++)
                    tempFiles.Add(CreateTempFile());

                // Build args: --compress + interleaved paths
                var pathArgs = new List<string>();
                int existIdx = 0, nonExistIdx = 0;
                while (existIdx < tempFiles.Count || nonExistIdx < input.nonExisting.Length)
                {
                    if (existIdx < tempFiles.Count)
                        pathArgs.Add(tempFiles[existIdx++]);
                    if (nonExistIdx < input.nonExisting.Length)
                        pathArgs.Add(input.nonExisting[nonExistIdx++]);
                }

                var args = new[] { input.flag }.Concat(pathArgs).ToArray();
                var result = CommandLineParser.Parse(args);

                return result.Mode == Mode.Compress
                    && result.ValidPaths.Length == tempFiles.Count
                    && tempFiles.All(f => result.ValidPaths.Contains(f))
                    && input.nonExisting.All(f => !result.ValidPaths.Contains(f));
            }
            finally
            {
                foreach (var f in tempFiles)
                    if (File.Exists(f)) File.Delete(f);
            }
        });
    }

    [Property(MaxTest = 100)]
    public Property NonExistentPaths_NeverAppearInValidPaths()
    {
        // **Validates: Requirements 7.1, 7.2, 7.5, 7.6**
        // Non-existent paths are always filtered out regardless of mode.
        var gen = GenNonExistentPathArray().ToArbitrary();

        return Prop.ForAll(gen, nonExistentPaths =>
        {
            // Test with bare paths
            var result1 = CommandLineParser.Parse(nonExistentPaths);

            // Test with --compress prefix
            var compressArgs = new[] { "--compress" }.Concat(nonExistentPaths).ToArray();
            var result2 = CommandLineParser.Parse(compressArgs);

            return result1.ValidPaths.Length == 0
                && result2.ValidPaths.Length == 0;
        });
    }

    [Property(MaxTest = 100)]
    public Property RegisterShellFlag_CaseInsensitive_AlwaysDetected()
    {
        // **Validates: Requirements 7.1, 7.2, 7.5, 7.6**
        // --register-shell is detected regardless of case.
        var gen = GenRegisterShellFlag().ToArbitrary();

        return Prop.ForAll(gen, flag =>
        {
            var result = CommandLineParser.Parse(new[] { flag });
            return result.Mode == Mode.RegisterShell;
        });
    }

    [Property(MaxTest = 100)]
    public Property UnregisterShellFlag_CaseInsensitive_AlwaysDetected()
    {
        // **Validates: Requirements 7.1, 7.2, 7.5, 7.6**
        // --unregister-shell is detected regardless of case.
        var gen = GenUnregisterShellFlag().ToArbitrary();

        return Prop.ForAll(gen, flag =>
        {
            var result = CommandLineParser.Parse(new[] { flag });
            return result.Mode == Mode.UnregisterShell;
        });
    }
}
