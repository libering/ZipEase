using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading.Tasks;

namespace ZipEase.UI.Core
{
    public enum ListResult { Success, PasswordRequired, ZipBomb, Error }

    public class ArchivePreviewService
    {
        private static readonly HashSet<string> SupportedExtensions =
            new(StringComparer.OrdinalIgnoreCase)
            {
                ".zip", ".rar", ".7z", ".tar", ".gz", ".bz2", ".xz", ".zst",
                ".cab", ".iso",
                ".apk", ".ipa", ".jar", ".war", ".ear",  // ZIP-based formats
                // Split archives
                ".001", ".z01", ".z02", ".z03", ".z04", ".z05",
                ".z06", ".z07", ".z08", ".z09"
            };

        public bool IsSupportedArchive(string filePath)
        {
            if (string.IsNullOrEmpty(filePath)) return false;
            var ext = System.IO.Path.GetExtension(filePath);
            if (SupportedExtensions.Contains(ext)) return true;
            if (filePath.EndsWith(".rar", StringComparison.OrdinalIgnoreCase)) return true;
            // Check installed plugins
            return Plugin.PluginRegistry.FindForExtension(ext) != null;
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
                        ? Marshal.PtrToStringUTF8(errPtr) ?? "Unknown error"
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

            // Try plugin first if extension is not natively supported
            var ext = Path.GetExtension(archivePath);
            if (!IsNativeExtension(ext))
            {
                var plugin = Plugin.PluginRegistry.FindForExtension(ext);
                if (plugin != null)
                {
                    try
                    {
                        return ListViaPlugin(plugin, archivePath);
                    }
                    catch (Exception ex) when (ex is Plugin.PluginException || ex.InnerException is Plugin.PluginException)
                    {
                        var fallback = Plugin.PluginRegistry.FindFallbackPlugin(ext);
                        if (fallback != null)
                        {
                            try
                            {
                                return ListViaPlugin(fallback, archivePath);
                            }
                            catch { /* ignore and throw original */ }
                        }
                        throw;
                    }
                }
            }


            IntPtr entriesPtr = IntPtr.Zero;
            int count = 0;

            try
            {
                int result = NativeMethods.ListArchiveContentsWithPassword(archivePath, password, out entriesPtr, out count);

                if (result == unchecked((int)0x2004))
                    return (ListResult.PasswordRequired, new List<ArchiveEntry>(), "Password required or incorrect");

                if (result == unchecked((int)0x2005))
                {
                    IntPtr errPtr = NativeMethods.GetLastError();
                    string msg = errPtr != IntPtr.Zero
                        ? Marshal.PtrToStringUTF8(errPtr) ?? "壓縮炸彈偵測"
                        : "壓縮炸彈偵測";
                    if (errPtr != IntPtr.Zero) NativeMethods.FreeErrorString(errPtr);
                    return (ListResult.ZipBomb, new List<ArchiveEntry>(), msg);
                }

                if (result < 0)
                {
                    IntPtr errPtr = NativeMethods.GetLastError();
                    string error = errPtr != IntPtr.Zero ? Marshal.PtrToStringUTF8(errPtr) ?? "Unknown error" : "Unknown error";
                    if (errPtr != IntPtr.Zero) NativeMethods.FreeErrorString(errPtr);
                    return (ListResult.Error, new List<ArchiveEntry>(), error);
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

        /// <summary>
        /// Lists archive contents and returns the native entries pointer for use with search FFI.
        /// The caller is responsible for freeing the native pointer via NativeMethods.FreeArchiveEntries.
        /// </summary>
        public (ListResult result, List<ArchiveEntry> entries, string? errorMessage, IntPtr nativePtr, int nativeCount)
            ListArchiveContentsKeepNative(string archivePath, string? password)
        {
            if (string.IsNullOrEmpty(archivePath))
                throw new ArgumentException("Archive path cannot be null or empty", nameof(archivePath));
            if (!File.Exists(archivePath))
                throw new FileNotFoundException("Archive file not found", archivePath);

            // Try plugin first if extension is not natively supported
            var ext = Path.GetExtension(archivePath);
            if (!IsNativeExtension(ext))
            {
                var plugin = Plugin.PluginRegistry.FindForExtension(ext);
                if (plugin != null)
                {
                    try
                    {
                        var (r, e, m) = ListViaPlugin(plugin, archivePath);
                        return (r, e, m, IntPtr.Zero, 0);
                    }
                    catch (Exception ex) when (ex is Plugin.PluginException || ex.InnerException is Plugin.PluginException)
                    {
                        var fallback = Plugin.PluginRegistry.FindFallbackPlugin(ext);
                        if (fallback != null)
                        {
                            try
                            {
                                var (r, e, m) = ListViaPlugin(fallback, archivePath);
                                return (r, e, m, IntPtr.Zero, 0);
                            }
                            catch { /* ignore and throw original */ }
                        }
                        throw;
                    }
                }
            }


            IntPtr entriesPtr = IntPtr.Zero;
            int count = 0;

            int result = NativeMethods.ListArchiveContentsWithPassword(archivePath, password, out entriesPtr, out count);

            if (result == unchecked((int)0x2004))
                return (ListResult.PasswordRequired, new List<ArchiveEntry>(), "Password required or incorrect", IntPtr.Zero, 0);

            if (result == unchecked((int)0x2005))
            {
                IntPtr errPtr = NativeMethods.GetLastError();
                string msg = errPtr != IntPtr.Zero
                    ? Marshal.PtrToStringUTF8(errPtr) ?? "壓縮炸彈偵測"
                    : "壓縮炸彈偵測";
                if (errPtr != IntPtr.Zero) NativeMethods.FreeErrorString(errPtr);
                if (entriesPtr != IntPtr.Zero) NativeMethods.FreeArchiveEntries(entriesPtr, count);
                return (ListResult.ZipBomb, new List<ArchiveEntry>(), msg, IntPtr.Zero, 0);
            }

            if (result < 0)
            {
                IntPtr errPtr = NativeMethods.GetLastError();
                string error = errPtr != IntPtr.Zero ? Marshal.PtrToStringUTF8(errPtr) ?? "Unknown error" : "Unknown error";
                if (errPtr != IntPtr.Zero) NativeMethods.FreeErrorString(errPtr);
                if (entriesPtr != IntPtr.Zero) NativeMethods.FreeArchiveEntries(entriesPtr, count);
                return (ListResult.Error, new List<ArchiveEntry>(), error, IntPtr.Zero, 0);
            }

            var entries = ParseEntries(entriesPtr, count);
            // Return the native pointer — caller owns it and must free it
            return (ListResult.Success, entries, null, entriesPtr, count);
        }

        // ==========================================
        // 新增的方法：安全預覽單一檔案 (解決 Bug 3 & 4)
        // ==========================================
        public async Task PreviewEntryAsync(string archivePath, string entryName, string? password = null)
        {
            await Task.Run(() =>
            {
                // 1. 建立具有唯一 GUID 的暫存資料夾
                string tempDir = Path.Combine(Path.GetTempPath(), $"ZipEase_preview_{Guid.NewGuid()}");
                Directory.CreateDirectory(tempDir);

                IntPtr outNamePtr = IntPtr.Zero;

                try
                {
                    // 2. 呼叫 FFI 進行單一檔案提取
                    // 注意：傳入 out outNamePtr 接收 Rust 回傳的實際檔案路徑
                    int result = NativeMethods.ExtractEntryByName(archivePath, entryName, tempDir, out outNamePtr);

                    if (result != 0)
                    {
                        throw new InvalidOperationException($"FFI Extraction failed with code: {result}");
                    }

                    // 3. 【架構修正】直接讀取 Rust 回傳的準確相對路徑
                    string extractedRelPath = entryName;
                    if (outNamePtr != IntPtr.Zero)
                    {
                        // 讀取 UTF-16 指標轉為 C# 字串
                        extractedRelPath = Marshal.PtrToStringUni(outNamePtr) ?? entryName;
                    }

                    string exactExtractedPath = Path.Combine(tempDir, extractedRelPath);
                    exactExtractedPath = exactExtractedPath.Replace("/", "\\");

                    if (!File.Exists(exactExtractedPath))
                    {
                        throw new FileNotFoundException($"Extraction succeeded but file not found at: {exactExtractedPath}");
                    }

                    // 4. 使用 Windows 預設程式開啟
                    var processStartInfo = new ProcessStartInfo
                    {
                        FileName = exactExtractedPath,
                        UseShellExecute = true
                    };
                    Process.Start(processStartInfo);
                }
                catch (Exception ex)
                {
                    Debug.WriteLine($"[ArchivePreviewService] Preview Error: {ex.Message}");
                    throw;
                }
                finally
                {
                    // 5. 【記憶體安全防線】釋放 Rust 分配的字串指標
                    if (outNamePtr != IntPtr.Zero)
                    {
                        NativeMethods.FreeString(outNamePtr);
                    }
                }
            });
        }
        // ==========================================

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

        internal static bool IsNativeExtension(string ext) =>
            SupportedExtensions.Contains(ext) ||
            ext.Equals(".rar", StringComparison.OrdinalIgnoreCase);


        private static (ListResult, List<ArchiveEntry>, string?) ListViaPlugin(
            Plugin.LoadedPlugin plugin, string archivePath)
        {
            try
            {
                var pluginEntries = Plugin.PluginBackend.ListAsync(plugin, archivePath)
                    .GetAwaiter().GetResult();

                var entries = new List<ArchiveEntry>(pluginEntries.Count);
                foreach (var e in pluginEntries)
                {
                    entries.Add(new ArchiveEntry
                    {
                        FileName = e.Name,
                        Size = e.Size,
                        IsDirectory = e.IsDir,
                        FileType = e.IsDir ? "Folder" : GetFileType(e.Name),
                        FormattedSize = e.IsDir ? "—" : FormatFileSize(e.Size)
                    });
                }
                return (ListResult.Success, entries, null);
            }
            catch (Plugin.PluginException ex)
            {
                throw new ExtractionException(ex.Message, -1);
            }
        }
    }
}