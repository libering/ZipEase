using System;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// Debug-only helper that allocates a Win32 console window and redirects
    /// both .NET stderr AND the Win32 STD_ERROR_HANDLE (used by Rust's eprintln!)
    /// to a log file at %TEMP%\ZipEase_debug.log.
    ///
    /// Only compiled and used in DEBUG_CONSOLE builds (Debug configuration).
    /// Release builds have no console window and no log file overhead.
    /// </summary>
    internal static class DebugConsole
    {
        [DllImport("kernel32.dll")] private static extern bool AllocConsole();
        [DllImport("kernel32.dll")] private static extern bool AttachConsole(int dwProcessId);
        [DllImport("kernel32.dll")] private static extern bool SetStdHandle(uint nStdHandle, IntPtr hHandle);
        [DllImport("kernel32.dll")] private static extern IntPtr CreateFileW(
            [MarshalAs(UnmanagedType.LPWStr)] string lpFileName,
            uint dwDesiredAccess, uint dwShareMode, IntPtr lpSecurityAttributes,
            uint dwCreationDisposition, uint dwFlagsAndAttributes, IntPtr hTemplateFile);

        private const uint STD_ERROR_HANDLE  = 0xFFFFFFF4;
        private const uint GENERIC_WRITE     = 0x40000000;
        private const uint FILE_SHARE_READ   = 0x00000001;
        private const uint CREATE_ALWAYS     = 2;
        private const uint FILE_ATTRIBUTE_NORMAL = 0x80;

        public static string LogPath { get; private set; } = string.Empty;

        /// <summary>
        /// Allocates a console, redirects Win32 STD_ERROR_HANDLE to a log file,
        /// and also tees .NET Console.Error to the same file.
        /// Call once at application startup before any FFI calls.
        /// </summary>
        public static void Attach()
        {
            // Attach to parent console (PowerShell/cmd) or allocate a new one
            if (!AttachConsole(-1))
                AllocConsole();

            // Use a timestamped filename so each run gets a fresh log — no file-lock conflicts
            string timestamp = DateTime.Now.ToString("yyyyMMdd_HHmmss");
            LogPath = Path.Combine(Path.GetTempPath(), $"ZipEase_debug_{timestamp}.log");

            // Clean up old logs (keep last 5)
            try
            {
                var oldLogs = Directory.GetFiles(Path.GetTempPath(), "ZipEase_debug_*.log")
                    .OrderByDescending(f => f)
                    .Skip(5);
                foreach (var old in oldLogs)
                    try { File.Delete(old); } catch { }
            }
            catch { }

            try
            {
                // Open via Win32 CreateFile to get a real HANDLE
                IntPtr hFile = CreateFileW(
                    LogPath,
                    GENERIC_WRITE,
                    FILE_SHARE_READ,
                    IntPtr.Zero,
                    CREATE_ALWAYS,
                    FILE_ATTRIBUTE_NORMAL,
                    IntPtr.Zero);

                if (hFile != new IntPtr(-1))
                {
                    // Redirect Win32 STD_ERROR_HANDLE → log file
                    // This captures Rust's eprintln! which writes to the Win32 stderr handle directly
                    SetStdHandle(STD_ERROR_HANDLE, hFile);

                    // Wrap the same Win32 handle in a .NET FileStream so Console.Error also goes here
                    var fs = new FileStream(
                        new Microsoft.Win32.SafeHandles.SafeFileHandle(hFile, ownsHandle: false),
                        FileAccess.Write, bufferSize: 1, isAsync: false);
                    var logWriter = new StreamWriter(fs, System.Text.Encoding.UTF8) { AutoFlush = true };
                    Console.SetError(logWriter);
                }
                else
                {
                    // Fallback: plain StreamWriter (won't capture Rust output but captures C# errors)
                    var logWriter = new StreamWriter(LogPath, append: false, System.Text.Encoding.UTF8)
                        { AutoFlush = true };
                    Console.SetError(logWriter);
                }

                Console.Error.WriteLine($"[ZipEase] Debug log started: {LogPath}");
                Console.Error.WriteLine($"[ZipEase] Build: {DateTime.Now:yyyy-MM-dd HH:mm:ss}");
                Console.Error.WriteLine($"[ZipEase] ----------------------------------------");
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"[ZipEase] Warning: could not set up log file: {ex.Message}");
            }
        }

        /// <summary>
        /// Write a timestamped line to the debug log (and console stdout).
        /// Use this for C#-side diagnostic messages.
        /// </summary>
        public static void Log(string message)
        {
            string line = $"[{DateTime.Now:HH:mm:ss.fff}] {message}";
            Console.WriteLine(line);          // stdout → visible in cmd window
            Console.Error.WriteLine(line);    // stderr → goes to log file
        }
    }
}
