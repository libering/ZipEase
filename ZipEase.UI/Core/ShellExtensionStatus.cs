namespace ZipEase.UI.Core;

/// <summary>
/// Represents the current state of the Windows Shell Extension registration.
/// </summary>
public enum ShellExtensionStatus
{
    /// <summary>Shell extension is registered and active (via Sparse MSIX or Registry).</summary>
    Enabled,

    /// <summary>Shell extension is not registered (user disabled or never registered).</summary>
    Disabled,

    /// <summary>Registration was attempted but failed (permission issue or other error).</summary>
    Failed
}

/// <summary>
/// Result of a shell extension registration or unregistration operation.
/// </summary>
/// <param name="Success">Whether the operation completed successfully.</param>
/// <param name="UsedStrategy">Which registration strategy was used or attempted.</param>
/// <param name="ErrorMessage">Error details if the operation failed; null on success.</param>
public record RegistrationResult(
    bool Success,
    RegistrationManager.Strategy UsedStrategy,
    string? ErrorMessage = null
);
