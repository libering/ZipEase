using FsCheck;
using FsCheck.Xunit;
using Xunit;
using ZipEase.UI.Core;

namespace ZipEase.ShellExtension.Tests;

// Feature: context-menu, Property 5: AppliesTo value encodes all supported extensions
/// <summary>
/// Property-based tests verifying that RegistrationManager.GenerateAppliesToValue produces
/// a valid Windows Shell AppliesTo string using "System.FileExtension:=.ext" syntax joined by " OR ".
/// Validates: Requirements 4.4
/// </summary>
public class AppliesToPropertyTests
{
    /// <summary>
    /// The full set of supported archive extensions that GenerateAppliesToValue should encode.
    /// </summary>
    private static readonly string[] AllSupportedExtensions =
    [
        ".zip", ".7z", ".rar", ".tar", ".gz", ".bz2",
        ".cab", ".iso", ".apk", ".tgz",
        ".001", ".z01", ".z02", ".z03", ".z04", ".z05", ".z06", ".z07", ".z08", ".z09"
    ];

    /// <summary>
    /// Generator for a random non-empty subset of supported extensions.
    /// </summary>
    private static Gen<string[]> GenExtensionSubset()
    {
        return Gen.ArrayOf(AllSupportedExtensions.Length, Gen.Elements(AllSupportedExtensions))
            .Select(arr => arr.Distinct().ToArray())
            .Where(arr => arr.Length > 0);
    }

    // ─── Property Tests ───────────────────────────────────────────────────────

    [Property(MaxTest = 100)]
    public Property OutputContainsAllSupportedExtensions()
    {
        // **Validates: Requirements 4.4**
        // The full GenerateAppliesToValue output must contain every supported extension
        // with the correct System.FileExtension:= prefix
        return Prop.ForAll(Gen.Constant(AllSupportedExtensions).ToArbitrary(), extensions =>
        {
            var result = RegistrationManager.GenerateAppliesToValue();

            foreach (var ext in extensions)
            {
                var expected = $"System.FileExtension:={ext}";
                if (!result.Contains(expected))
                    return false;
            }

            return true;
        });
    }

    [Property(MaxTest = 100)]
    public Property OutputUsesCorrectSyntaxForEachExtension()
    {
        // **Validates: Requirements 4.4**
        // For any supported extension, the output must contain "System.FileExtension:=<ext>"
        var gen = Gen.Elements(AllSupportedExtensions).ToArbitrary();

        return Prop.ForAll(gen, ext =>
        {
            var result = RegistrationManager.GenerateAppliesToValue();
            var expectedEntry = $"System.FileExtension:={ext}";
            return result.Contains(expectedEntry);
        });
    }

    [Property(MaxTest = 100)]
    public Property OutputJoinedByOrSeparator()
    {
        // **Validates: Requirements 4.4**
        // The output must be entries joined by " OR " — splitting by " OR " should yield
        // the same number of entries as supported extensions
        return Prop.ForAll(Gen.Constant(0).ToArbitrary(), _ =>
        {
            var result = RegistrationManager.GenerateAppliesToValue();
            var parts = result.Split(" OR ");

            // Number of parts must equal number of supported extensions
            return parts.Length == AllSupportedExtensions.Length;
        });
    }

    [Property(MaxTest = 100)]
    public Property EachPartHasCorrectPrefix()
    {
        // **Validates: Requirements 4.4**
        // Every part (split by " OR ") must start with "System.FileExtension:="
        return Prop.ForAll(Gen.Constant(0).ToArbitrary(), _ =>
        {
            var result = RegistrationManager.GenerateAppliesToValue();
            var parts = result.Split(" OR ");

            return parts.All(part => part.StartsWith("System.FileExtension:="));
        });
    }

    [Property(MaxTest = 100)]
    public Property NoDuplicateEntries()
    {
        // **Validates: Requirements 4.4**
        // The output must not contain duplicate extension entries
        return Prop.ForAll(Gen.Constant(0).ToArbitrary(), _ =>
        {
            var result = RegistrationManager.GenerateAppliesToValue();
            var parts = result.Split(" OR ");

            return parts.Length == parts.Distinct().Count();
        });
    }

    [Property(MaxTest = 100)]
    public Property OutputIsNonEmpty()
    {
        // **Validates: Requirements 4.4**
        // The output must always be non-empty since there are supported extensions
        return Prop.ForAll(Gen.Constant(0).ToArbitrary(), _ =>
        {
            var result = RegistrationManager.GenerateAppliesToValue();
            return !string.IsNullOrWhiteSpace(result);
        });
    }

    [Property(MaxTest = 100)]
    public Property RandomSubsetAppearsInOutput()
    {
        // **Validates: Requirements 4.4**
        // For any random non-empty subset of supported extensions, every extension
        // in that subset must appear in the output with correct prefix
        var gen = GenExtensionSubset().ToArbitrary();

        return Prop.ForAll(gen, subset =>
        {
            var result = RegistrationManager.GenerateAppliesToValue();

            foreach (var ext in subset)
            {
                var expectedEntry = $"System.FileExtension:={ext}";
                if (!result.Contains(expectedEntry))
                    return false;
            }

            return true;
        });
    }

    [Property(MaxTest = 100)]
    public Property ExtensionsExtractedFromOutputMatchSupportedSet()
    {
        // **Validates: Requirements 4.4**
        // Parsing the output back should yield exactly the supported extensions set
        return Prop.ForAll(Gen.Constant(0).ToArbitrary(), _ =>
        {
            var result = RegistrationManager.GenerateAppliesToValue();
            var parts = result.Split(" OR ");

            var extractedExtensions = parts
                .Select(p => p.Replace("System.FileExtension:=", ""))
                .OrderBy(e => e)
                .ToArray();

            var expectedExtensions = AllSupportedExtensions
                .OrderBy(e => e)
                .ToArray();

            return extractedExtensions.SequenceEqual(expectedExtensions);
        });
    }
}
