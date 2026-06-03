using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Threading.Tasks;

namespace ZipEase.UI.Core
{
    // ─── DTOs ────────────────────────────────────────────────────────────

    public class DamageReport
    {
        public string Format { get; set; } = "";
        public int TotalEntries { get; set; }
        public int ValidEntries { get; set; }
        public int CorruptedEntries { get; set; }
        public int UnrecoverableEntries { get; set; }
        public List<DamageEntry> Damages { get; set; } = new();
        public bool Repairable { get; set; }
    }

    public class DamageEntry
    {
        public string DamageType { get; set; } = "";
        public long Offset { get; set; }
        public string? EntryName { get; set; }
        public string Description { get; set; } = "";
    }

    public class RepairResult
    {
        public bool Success { get; set; }
        public List<string> RecoveredEntries { get; set; } = new();
        public List<string> FailedEntries { get; set; } = new();
        public string? RepairedPath { get; set; }
    }

    public class RepairProgress
    {
        public int CurrentStep { get; set; }
        public int TotalSteps { get; set; }
        public string CurrentEntryName { get; set; } = "";
    }

    // ─── Service ─────────────────────────────────────────────────────────

    /// <summary>
    /// Thin P/Invoke bridge for the Rust repair engine.
    /// Zero business logic — marshalling and memory management only.
    /// </summary>
    public class RepairService
    {
        private static readonly JsonSerializerOptions _jsonOptions = new()
        {
            PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower
        };

        /// <summary>
        /// Diagnoses a damaged archive and returns a structured damage report.
        /// Returns null if the file cannot be read or is not a recognized archive.
        /// Memory contract: Rust allocates the JSON string; freed via FreeDiagnosis in finally.
        /// </summary>
        public async Task<DamageReport?> DiagnoseAsync(string archivePath)
        {
            return await Task.Run(() =>
            {
                IntPtr jsonPtr = IntPtr.Zero;
                try
                {
                    jsonPtr = NativeMethods.DiagnoseArchive(archivePath);
                    if (jsonPtr == IntPtr.Zero) return null;

                    string json = Marshal.PtrToStringUni(jsonPtr) ?? "";
                    return JsonSerializer.Deserialize<DamageReport>(json, _jsonOptions);
                }
                finally
                {
                    if (jsonPtr != IntPtr.Zero)
                        NativeMethods.FreeDiagnosis(jsonPtr);
                }
            });
        }

        /// <summary>
        /// Repairs a damaged archive and writes the result to outputPath (or auto-generates one).
        /// Progress is reported back via IProgress on the caller's context.
        /// Memory contract: callback delegate is pinned via GCHandle for the duration of the native call.
        /// </summary>
        public async Task<RepairResult?> RepairAsync(
            string archivePath,
            string? outputPath,
            IProgress<RepairProgress>? progress)
        {
            return await Task.Run(() =>
            {
                NativeMethods.RepairProgressCallback? callback = null;
                GCHandle callbackHandle = default;

                try
                {
                    if (progress != null)
                    {
                        callback = (current, total, namePtr) =>
                        {
                            string name = namePtr != IntPtr.Zero
                                ? Marshal.PtrToStringUni(namePtr) ?? ""
                                : "";
                            progress.Report(new RepairProgress
                            {
                                CurrentStep = current,
                                TotalSteps = total,
                                CurrentEntryName = name
                            });
                        };
                        callbackHandle = GCHandle.Alloc(callback);
                    }

                    int result = NativeMethods.RepairArchive(archivePath, outputPath, callback);

                    // 0 = full success
                    // 0x2007 = partial success (some entries unrecoverable)
                    // 0x2006 = not repairable
                    // -1 = IO error or panic
                    return new RepairResult
                    {
                        Success = result == 0,
                        RepairedPath = (result == 0 || result == 0x2007) ? outputPath : null
                    };
                }
                finally
                {
                    if (callbackHandle.IsAllocated)
                        callbackHandle.Free();
                }
            });
        }
    }
}
