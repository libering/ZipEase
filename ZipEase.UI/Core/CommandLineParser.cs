using System.IO;

namespace ZipEase.UI.Core;

/// <summary>
/// Parses command-line arguments passed to ZipEase.exe from the Shell Extension
/// or manual invocation. Determines the startup mode and filters out non-existent paths.
/// </summary>
public sealed class CommandLineParser
{
    public enum Mode { Normal, Extract, Compress, RegisterShell, UnregisterShell }

    public record ParseResult(Mode Mode, string[] ValidPaths);

    /// <summary>
    /// Parses the given command-line arguments.
    /// </summary>
    /// <param name="args">
    /// Raw args from Environment.GetCommandLineArgs() — note: the first element
    /// is typically the executable path and should be excluded before calling this method.
    /// </param>
    /// <returns>A <see cref="ParseResult"/> indicating the mode and valid file paths.</returns>
    public static ParseResult Parse(string[] args)
    {
        if (args is null || args.Length == 0)
            return new ParseResult(Mode.Normal, []);

        // --register-shell → RegisterShell mode (no paths needed)
        if (args.Any(a => a.Equals("--register-shell", StringComparison.OrdinalIgnoreCase)))
            return new ParseResult(Mode.RegisterShell, []);

        // --unregister-shell → UnregisterShell mode (no paths needed)
        if (args.Any(a => a.Equals("--unregister-shell", StringComparison.OrdinalIgnoreCase)))
            return new ParseResult(Mode.UnregisterShell, []);

        // --compress path1 path2 ... → Compress mode + filter existing paths
        if (args.Length > 0 && args[0].Equals("--compress", StringComparison.OrdinalIgnoreCase))
        {
            var paths = FilterExistingPaths(args.Skip(1));
            // If all paths are invalid → Normal mode
            return paths.Length > 0
                ? new ParseResult(Mode.Compress, paths)
                : new ParseResult(Mode.Normal, []);
        }

        // path1 path2 ... (no flags) → Extract mode + filter existing paths
        {
            var paths = FilterExistingPaths(args);
            // If all paths are invalid → Normal mode
            return paths.Length > 0
                ? new ParseResult(Mode.Extract, paths)
                : new ParseResult(Mode.Normal, []);
        }
    }

    /// <summary>
    /// Filters the given paths, returning only those that exist on disk
    /// (as either a file or a directory).
    /// </summary>
    private static string[] FilterExistingPaths(IEnumerable<string> paths)
    {
        return paths
            .Where(p => !string.IsNullOrWhiteSpace(p))
            .Where(p => File.Exists(p) || Directory.Exists(p))
            .ToArray();
    }
}
