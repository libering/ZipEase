using FsCheck;
using FsCheck.Xunit;
using Xunit;
using ZipEase.UI.Core;

namespace ZipEase.ShellExtension.Tests;

// Feature: context-menu, Property 4: OS version determines registration strategy
/// <summary>
/// Property-based tests verifying that RegistrationManager.DetectStrategyForBuild correctly
/// determines the registration strategy based on Windows build number.
/// Build >= 22000 (Windows 11) → SparseMsix; Build &lt; 22000 → Registry.
/// Validates: Requirements 3.1, 4.1
/// </summary>
public class StrategyDetectionPropertyTests
{
    /// <summary>
    /// The Windows 11 threshold build number.
    /// </summary>
    private const int Windows11Threshold = 22000;

    /// <summary>
    /// Generator for random build numbers in the range 10000-30000.
    /// This covers both Windows 10 (below 22000) and Windows 11 (22000+) territory.
    /// </summary>
    private static Gen<int> GenBuildNumber()
    {
        return Gen.Choose(10000, 30000);
    }

    /// <summary>
    /// Generator for build numbers strictly below the Windows 11 threshold (10000-21999).
    /// These should always map to the Registry strategy.
    /// </summary>
    private static Gen<int> GenWindows10BuildNumber()
    {
        return Gen.Choose(10000, Windows11Threshold - 1);
    }

    /// <summary>
    /// Generator for build numbers at or above the Windows 11 threshold (22000-30000).
    /// These should always map to the SparseMsix strategy.
    /// </summary>
    private static Gen<int> GenWindows11BuildNumber()
    {
        return Gen.Choose(Windows11Threshold, 30000);
    }

    // ─── Property Tests ───────────────────────────────────────────────────────

    [Property(MaxTest = 100)]
    public Property BuildBelow22000_AlwaysReturnsRegistry()
    {
        // **Validates: Requirements 3.1, 4.1**
        // For any build number below 22000, strategy must be Registry
        var gen = GenWindows10BuildNumber().ToArbitrary();

        return Prop.ForAll(gen, buildNumber =>
            RegistrationManager.DetectStrategyForBuild(buildNumber) == RegistrationManager.Strategy.Registry);
    }

    [Property(MaxTest = 100)]
    public Property BuildAtOrAbove22000_AlwaysReturnsSparseMsix()
    {
        // **Validates: Requirements 3.1, 4.1**
        // For any build number >= 22000, strategy must be SparseMsix
        var gen = GenWindows11BuildNumber().ToArbitrary();

        return Prop.ForAll(gen, buildNumber =>
            RegistrationManager.DetectStrategyForBuild(buildNumber) == RegistrationManager.Strategy.SparseMsix);
    }

    [Property(MaxTest = 100)]
    public Property AnyBuildNumber_StrategyDeterminedByThreshold()
    {
        // **Validates: Requirements 3.1, 4.1**
        // The main property: for any build number in range, the strategy is SparseMsix iff build >= 22000
        var gen = GenBuildNumber().ToArbitrary();

        return Prop.ForAll(gen, buildNumber =>
        {
            var expected = buildNumber >= Windows11Threshold
                ? RegistrationManager.Strategy.SparseMsix
                : RegistrationManager.Strategy.Registry;

            return RegistrationManager.DetectStrategyForBuild(buildNumber) == expected;
        });
    }

    [Property(MaxTest = 100)]
    public Property BoundaryBuild22000_AlwaysReturnsSparseMsix()
    {
        // **Validates: Requirements 3.1, 4.1**
        // The exact boundary value 22000 must always return SparseMsix
        // Using a property that generates the boundary value to confirm determinism
        var gen = Gen.Constant(Windows11Threshold).ToArbitrary();

        return Prop.ForAll(gen, buildNumber =>
            RegistrationManager.DetectStrategyForBuild(buildNumber) == RegistrationManager.Strategy.SparseMsix);
    }

    [Property(MaxTest = 100)]
    public Property BoundaryBuild21999_AlwaysReturnsRegistry()
    {
        // **Validates: Requirements 3.1, 4.1**
        // The value just below the boundary (21999) must always return Registry
        var gen = Gen.Constant(Windows11Threshold - 1).ToArbitrary();

        return Prop.ForAll(gen, buildNumber =>
            RegistrationManager.DetectStrategyForBuild(buildNumber) == RegistrationManager.Strategy.Registry);
    }
}
