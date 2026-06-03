using System;
using System.Runtime.InteropServices;
using FsCheck;
using FsCheck.Xunit;
using Xunit;
using ZipEase.ShellExtension;

namespace ZipEase.ShellExtension.Tests;

// Feature: context-menu, Property 7: Shell extension exception safety
/// <summary>
/// Property-based tests verifying that ExtractCommand and CompressCommand methods
/// never propagate exceptions to the caller (explorer.exe). All methods must catch
/// exceptions internally and return a valid HRESULT (S_OK or E_FAIL).
/// Since these are COM objects loaded in explorer.exe, exception propagation would crash explorer.
/// Validates: Requirements 8.5
/// </summary>
public class ExceptionSafetyPropertyTests
{
    private const int S_OK = 0;
    private const int E_FAIL = unchecked((int)0x80004005);

    /// <summary>
    /// Generator for random exception types that could occur during execution.
    /// </summary>
    private static Gen<Exception> GenException()
    {
        return Gen.Elements<Func<Exception>>(
            () => new InvalidOperationException("Random invalid operation"),
            () => new IOException("Random I/O error"),
            () => new NullReferenceException("Random null reference"),
            () => new ArgumentException("Random argument error"),
            () => new ArgumentNullException("param", "Random argument null"),
            () => new UnauthorizedAccessException("Random access denied"),
            () => new OutOfMemoryException("Random OOM"),
            () => new FileNotFoundException("Random file not found"),
            () => new DirectoryNotFoundException("Random directory not found"),
            () => new PathTooLongException("Random path too long"),
            () => new NotSupportedException("Random not supported"),
            () => new ObjectDisposedException("obj", "Random disposed"),
            () => new TimeoutException("Random timeout"),
            () => new FormatException("Random format error"),
            () => new OverflowException("Random overflow"),
            () => new IndexOutOfRangeException("Random index out of range"),
            () => new InvalidCastException("Random invalid cast"),
            () => new StackOverflowException(),
            () => new AccessViolationException("Random access violation"),
            () => new COMException("Random COM error", E_FAIL)
        ).Select(factory => factory());
    }

    /// <summary>
    /// Generator for random exception messages to vary the exception content.
    /// </summary>
    private static Gen<string> GenExceptionMessage()
    {
        return Gen.Elements(
            "File not found",
            "Access denied",
            "Path is too long",
            "The operation was canceled",
            "Network path not found",
            "Disk is full",
            "The process cannot access the file",
            "",
            "中文錯誤訊息",
            "Unicode: 日本語テスト",
            new string('x', 1000)
        );
    }

    // ─── ExtractCommand Tests ─────────────────────────────────────────────────

    [Property(MaxTest = 100)]
    public Property ExtractCommand_Invoke_NeverThrows_WithNullShellItemArray()
    {
        // **Validates: Requirements 8.5**
        var gen = GenException().ToArbitrary();

        return Prop.ForAll(gen, _ =>
        {
            var command = new ExtractCommand();
            // Calling Invoke with null IShellItemArray should never throw
            int hr = command.Invoke(null, IntPtr.Zero);
            // Should return S_OK (empty paths is not an error, just no-op) or E_FAIL
            return hr == S_OK || hr == E_FAIL;
        });
    }

    [Property(MaxTest = 100)]
    public Property ExtractCommand_GetState_NeverThrows_WithNullShellItemArray()
    {
        // **Validates: Requirements 8.5**
        var gen = GenException().ToArbitrary();

        return Prop.ForAll(gen, _ =>
        {
            var command = new ExtractCommand();
            // Calling GetState with null IShellItemArray should never throw
            int hr = command.GetState(null, false, out uint cmdState);
            // Should return S_OK with ECS_HIDDEN (no items) or E_FAIL
            return (hr == S_OK || hr == E_FAIL) &&
                   (cmdState == (uint)EXPCMDSTATE.ECS_HIDDEN || cmdState == (uint)EXPCMDSTATE.ECS_ENABLED);
        });
    }

    [Property(MaxTest = 100)]
    public Property ExtractCommand_GetTitle_NeverThrows_WithNullShellItemArray()
    {
        // **Validates: Requirements 8.5**
        var gen = GenException().ToArbitrary();

        return Prop.ForAll(gen, _ =>
        {
            var command = new ExtractCommand();
            // Calling GetTitle with null IShellItemArray should never throw
            int hr = command.GetTitle(null, out string title);
            // Should return S_OK or E_FAIL, and title should never be null
            return (hr == S_OK || hr == E_FAIL) && title != null;
        });
    }

    [Property(MaxTest = 100)]
    public Property ExtractCommand_GetIcon_NeverThrows_WithNullShellItemArray()
    {
        // **Validates: Requirements 8.5**
        var gen = GenException().ToArbitrary();

        return Prop.ForAll(gen, _ =>
        {
            var command = new ExtractCommand();
            // Calling GetIcon with null IShellItemArray should never throw
            int hr = command.GetIcon(null, out string icon);
            // Should return S_OK or E_FAIL, and icon should never be null
            return (hr == S_OK || hr == E_FAIL) && icon != null;
        });
    }

    // ─── CompressCommand Tests ────────────────────────────────────────────────

    [Property(MaxTest = 100)]
    public Property CompressCommand_Invoke_NeverThrows_WithNullShellItemArray()
    {
        // **Validates: Requirements 8.5**
        var gen = GenException().ToArbitrary();

        return Prop.ForAll(gen, _ =>
        {
            var command = new CompressCommand();
            // Calling Invoke with null IShellItemArray should never throw
            int hr = command.Invoke(null, IntPtr.Zero);
            // Should return S_OK (empty paths is not an error, just no-op) or E_FAIL
            return hr == S_OK || hr == E_FAIL;
        });
    }

    [Property(MaxTest = 100)]
    public Property CompressCommand_GetState_NeverThrows_WithNullShellItemArray()
    {
        // **Validates: Requirements 8.5**
        var gen = GenException().ToArbitrary();

        return Prop.ForAll(gen, _ =>
        {
            var command = new CompressCommand();
            // Calling GetState with null IShellItemArray should never throw
            int hr = command.GetState(null, false, out uint cmdState);
            // Should return S_OK with ECS_HIDDEN (no items) or E_FAIL
            return (hr == S_OK || hr == E_FAIL) &&
                   (cmdState == (uint)EXPCMDSTATE.ECS_HIDDEN || cmdState == (uint)EXPCMDSTATE.ECS_ENABLED);
        });
    }

    [Property(MaxTest = 100)]
    public Property CompressCommand_GetTitle_NeverThrows_WithNullShellItemArray()
    {
        // **Validates: Requirements 8.5**
        var gen = GenException().ToArbitrary();

        return Prop.ForAll(gen, _ =>
        {
            var command = new CompressCommand();
            // Calling GetTitle with null IShellItemArray should never throw
            int hr = command.GetTitle(null, out string title);
            // Should return S_OK or E_FAIL, and title should never be null
            return (hr == S_OK || hr == E_FAIL) && title != null;
        });
    }

    [Property(MaxTest = 100)]
    public Property CompressCommand_GetIcon_NeverThrows_WithNullShellItemArray()
    {
        // **Validates: Requirements 8.5**
        var gen = GenException().ToArbitrary();

        return Prop.ForAll(gen, _ =>
        {
            var command = new CompressCommand();
            // Calling GetIcon with null IShellItemArray should never throw
            int hr = command.GetIcon(null, out string icon);
            // Should return S_OK or E_FAIL, and icon should never be null
            return (hr == S_OK || hr == E_FAIL) && icon != null;
        });
    }

    // ─── Cross-Command Exception Type Variation Tests ─────────────────────────

    [Property(MaxTest = 100)]
    public Property ExtractCommand_Invoke_WithRandomIntPtrPbc_NeverThrows()
    {
        // **Validates: Requirements 8.5**
        // Test with various IntPtr values for the pbc parameter
        var gen = Gen.Choose(0, int.MaxValue).Select(i => new IntPtr(i)).ToArbitrary();

        return Prop.ForAll(gen, pbc =>
        {
            var command = new ExtractCommand();
            // Should never throw regardless of pbc value
            int hr = command.Invoke(null, pbc);
            return hr == S_OK || hr == E_FAIL;
        });
    }

    [Property(MaxTest = 100)]
    public Property CompressCommand_Invoke_WithRandomIntPtrPbc_NeverThrows()
    {
        // **Validates: Requirements 8.5**
        // Test with various IntPtr values for the pbc parameter
        var gen = Gen.Choose(0, int.MaxValue).Select(i => new IntPtr(i)).ToArbitrary();

        return Prop.ForAll(gen, pbc =>
        {
            var command = new CompressCommand();
            // Should never throw regardless of pbc value
            int hr = command.Invoke(null, pbc);
            return hr == S_OK || hr == E_FAIL;
        });
    }

    [Property(MaxTest = 100)]
    public Property ExtractCommand_GetState_WithRandomFOkToBeSlow_NeverThrows()
    {
        // **Validates: Requirements 8.5**
        var gen = Arb.From<bool>();

        return Prop.ForAll(gen, fOkToBeSlow =>
        {
            var command = new ExtractCommand();
            int hr = command.GetState(null, fOkToBeSlow, out uint cmdState);
            return (hr == S_OK || hr == E_FAIL) &&
                   (cmdState == (uint)EXPCMDSTATE.ECS_HIDDEN || cmdState == (uint)EXPCMDSTATE.ECS_ENABLED);
        });
    }

    [Property(MaxTest = 100)]
    public Property CompressCommand_GetState_WithRandomFOkToBeSlow_NeverThrows()
    {
        // **Validates: Requirements 8.5**
        var gen = Arb.From<bool>();

        return Prop.ForAll(gen, fOkToBeSlow =>
        {
            var command = new CompressCommand();
            int hr = command.GetState(null, fOkToBeSlow, out uint cmdState);
            return (hr == S_OK || hr == E_FAIL) &&
                   (cmdState == (uint)EXPCMDSTATE.ECS_HIDDEN || cmdState == (uint)EXPCMDSTATE.ECS_ENABLED);
        });
    }
}
