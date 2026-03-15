using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;

namespace ZipEase.UI.Core
{
    public enum ListResult { Success, PasswordRequired, Error }

    public class ArchivePreviewService
    {
        private static readonly HashSet<string> SupportedExtensions =
            new(StringComparer.OrdinalIgnoreCase) { ".zip", ".rar", ".7z", ".tar", ".gz" };

        public bool IsSupportedArchive(string filePath)
        {
            if (string.IsNullOrEmpty(filePath)) return false;
            return SupportedExtensions.Contains(Path.GetExtension(filePath));
        }

        public List<ArchiveEntry> ListArchiveContents(string archivePath)
        {
            if (string.IsNullOrEmpty(archivePath))
                throw new ArgumentException("Archive path cannot be null or empty", nameof(archivePath));
            if (!File.Exists(archivePath))
                throw new FileNotFoundException("Archive file not found", archivePath);

            IntPtr entriesPtr = IntPtr.Zero;
            int count = 0;

            try
            {
                int result = NativeMethods.ListArchiveContents(archivePath, out entriesPtr, out count);

                if (result < 0)
                {
                    IntPtr errPtr = NativeMethods.GetLastError();
                    string error = errPtr != IntPtr.Zero
                        ? Marshal.PtrToStringUni(errPtr) ?? "Unknown error"
                        : "Unknown error";
                    if (errPtr != IntPtr.Zero) NativeMethods.FreeErrorString(errPtr);
                    if (result == unchecked((int)0x2003))
                        throw new ExtractionException($"⚠️ {error}", result);
                    throw new ExtractionException(error, result);
                }

                return ParseEntries(entriesPtr, count);
            }
            finally
            {
                if (entriesPtr != IntPtr.Zero)
                    NativeMethods.FreeArchiveEntries(entriesPtr, count);
            }
        }

        public (ListResult result, List<ArchiveEntry> entries, string? errorMessage) ListArchiveContentsWithPassword(string archivePath, string? password)
        {
            if (string.IsNullOrEmpty(archivePath))
                throw new ArgumentException("Archive path cannot be null or empty", nameof(archivePath));
            if (!File.Exists(archivePath))
                throw new FileNotFoundException("Archive file not found", archivePath);

            IntPtr entriesPtr = IntPtr.Zero;
            int count = 0;

            try
            {
                int result = NativeMethods.ListArchiveContentsWithPassword(archivePath, password, out entriesPtr, out count);

                if (result == unchecked((int)0x2004))
                    return (ListResult.PasswordRequired, new List<ArchiveEntry>(), "Password required or incorrect");

                if (result < 0)
                {
                    IntPtr errPtr = NativeMethods.GetLastError();
                    string error = errPtr != IntPtr.Zero ? Marshal.PtrToStringUni(errPtr) ?? "Unknown error" : "Unknown error";
                    if (errPtr != IntPtr.Zero) NativeMethods.FreeErrorString(errPtr);
                    if (result == unchecked((int)0x2003))
                        throw new ExtractionException($"⚠️ {error}", result);
                    throw new ExtractionException(error, result);
                }

                var entries = ParseEntries(entriesPtr, count);
                return (ListResult.Success, entries, null);
            }
            finally
            {
                if (entriesPtr != IntPtr.Zero)
                    NativeMethods.FreeArchiveEntries(entriesPtr, count);
            }
        }

        private List<ArchiveEntry> ParseEntries(IntPtr entriesPtr, int count)
        {
            var entries = new List<ArchiveEntry>(count);
            int structSize = Marshal.SizeOf<NativeMethods.ArchiveEntryFFI>();
            for (int i = 0; i < count; i++)
            {
                IntPtr structPtr = IntPtr.Add(entriesPtr, i * structSize);
                var ffi = Marshal.PtrToStructure<NativeMethods.ArchiveEntryFFI>(structPtr);
                string fileName = ffi.FileNamePtr != IntPtr.Zero
                    ? Marshal.PtrToStringUni(ffi.FileNamePtr) ?? string.Empty
                    : string.Empty;
                bool isDir = ffi.IsDirectory != 0;
                entries.Add(new ArchiveEntry
                {
                    FileName = fileName,
                    Size = ffi.FileSize,
                    IsDirectory = isDir,
                    FileType = isDir ? "Folder" : GetFileType(fileName),
                    FormattedSize = isDir ? "—" : FormatFileSize(ffi.FileSize)
                });
            }
            return entries;
        }

        private static string GetFileType(string fileName)
        {
            var ext = Path.GetExtension(fileName);
            return string.IsNullOrEmpty(ext) ? "File" : ext.TrimStart('.').ToUpperInvariant();
        }

        private static string FormatFileSize(long bytes)
        {
            if (bytes < 0) return "—";
            string[] sizes = { "B", "KB", "MB", "GB", "TB" };
            double len = bytes;
            int order = 0;
            while (len >= 1024 && order < sizes.Length - 1) { order++; len /= 1024; }
            return $"{len:0.##} {sizes[order]}";
        }
    }
}
