using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Threading.Tasks;

namespace ZipEase.UI.Core.Plugin
{
    /// <summary>
    /// Communicates with a CLI plugin via stdin/stdout using JSON Lines protocol.
    ///
    /// Protocol (newline-delimited JSON):
    ///   ZipEase → plugin:
    ///     {"action":"list","path":"C:\\archive.xyz"}
    ///     {"action":"extract","path":"C:\\archive.xyz","output":"C:\\out","password":null}
    ///
    ///   Plugin → ZipEase:
    ///     {"status":"ok","entries":[{"name":"foo.txt","is_dir":false,"size":1234},...]}
    ///     {"status":"progress","pct":42,"file":"foo.txt"}
    ///     {"status":"error","message":"..."}
    ///     {"status":"done","count":5}
    /// </summary>
    public static class PluginBackend
    {
        private static readonly JsonSerializerOptions Opts = new() { PropertyNameCaseInsensitive = true };

        // ── List entries ──────────────────────────────────────────────────────

        public static async Task<List<PluginEntry>> ListAsync(LoadedPlugin plugin, string archivePath)
        {
            var request = JsonSerializer.Serialize(new { action = "list", path = archivePath });
            var lines = await RunAsync(plugin, request);

            foreach (var line in lines)
            {
                var msg = TryParse<PluginMessage>(line);
                if (msg?.Status == "ok" && msg.Entries != null)
                    return msg.Entries;
                if (msg?.Status == "error")
                    throw new PluginException(msg.Message ?? "Plugin error");
            }
            throw new PluginException("Plugin returned no entries");
        }

        // ── Extract ───────────────────────────────────────────────────────────

        public static async Task<int> ExtractAsync(
            LoadedPlugin plugin,
            string archivePath,
            string outputDir,
            string? password,
            ProgressCallback? progress)
        {
            var request = JsonSerializer.Serialize(new
            {
                action = "extract",
                path = archivePath,
                output = outputDir,
                password
            });

            int count = 0;
            await RunStreamingAsync(plugin, request, line =>
            {
                var msg = TryParse<PluginMessage>(line);
                if (msg == null) return;
                switch (msg.Status)
                {
                    case "progress":
                        progress?.Invoke(msg.Pct, msg.File ?? string.Empty);
                        break;
                    case "done":
                        count = msg.Count;
                        break;
                    case "error":
                        throw new PluginException(msg.Message ?? "Plugin extraction error");
                }
            });
            return count;
        }

        // ── Internal helpers ──────────────────────────────────────────────────

        private static async Task<List<string>> RunAsync(LoadedPlugin plugin, string request)
        {
            var lines = new List<string>();
            await RunStreamingAsync(plugin, request, l => lines.Add(l));
            return lines;
        }

        private static async Task RunStreamingAsync(LoadedPlugin plugin, string request, Action<string> onLine)
        {
            string fileName = plugin.ExecutablePath;
            string arguments = string.Empty;

            if (plugin.ExecutablePath.EndsWith(".py", StringComparison.OrdinalIgnoreCase))
            {
                fileName = "python";
                arguments = $"\"{plugin.ExecutablePath}\"";
            }

            var psi = new ProcessStartInfo(fileName, arguments)
            {
                RedirectStandardInput  = true,
                RedirectStandardOutput = true,
                RedirectStandardError  = true,
                UseShellExecute        = false,
                CreateNoWindow         = true,
                StandardInputEncoding  = Encoding.UTF8,
                StandardOutputEncoding = Encoding.UTF8,
                WorkingDirectory       = Path.GetDirectoryName(plugin.ExecutablePath) ?? string.Empty
            };

            using var proc = new Process { StartInfo = psi };
            try
            {
                proc.Start();
            }
            catch (Exception ex)
            {
                throw new PluginException($"Failed to start plugin process: {ex.Message}. Make sure the plugin executable or python interpreter is available.");
            }


            using var cts = new System.Threading.CancellationTokenSource(TimeSpan.FromMinutes(5));

            try
            {
                await proc.StandardInput.WriteLineAsync(request);
                proc.StandardInput.Close();

                string? line;
                while ((line = await proc.StandardOutput.ReadLineAsync(cts.Token)) != null)
                {
                    if (!string.IsNullOrWhiteSpace(line))
                        onLine(line);
                }

                await proc.WaitForExitAsync(cts.Token);
            }
            catch (OperationCanceledException)
            {
                try
                {
                    proc.Kill(entireProcessTree: true);
                }
                catch { /* best effort */ }
                throw new PluginException("Plugin execution timed out (5 minutes limit exceeded).");
            }

            if (proc.ExitCode != 0)
            {
                var err = await proc.StandardError.ReadToEndAsync(cts.Token);
                throw new PluginException($"Plugin exited with code {proc.ExitCode}: {err}");
            }
        }

        private static T? TryParse<T>(string json)
        {
            try { return JsonSerializer.Deserialize<T>(json, Opts); }
            catch { return default; }
        }
    }

    // ── DTOs ──────────────────────────────────────────────────────────────────

    public class PluginMessage
    {
        [JsonPropertyName("status")]  public string?       Status  { get; set; }
        [JsonPropertyName("entries")] public List<PluginEntry>? Entries { get; set; }
        [JsonPropertyName("message")] public string?       Message { get; set; }
        [JsonPropertyName("pct")]     public int           Pct     { get; set; }
        [JsonPropertyName("file")]    public string?       File    { get; set; }
        [JsonPropertyName("count")]   public int           Count   { get; set; }
    }

    public class PluginEntry
    {
        [JsonPropertyName("name")]   public string Name   { get; set; } = string.Empty;
        [JsonPropertyName("is_dir")] public bool   IsDir  { get; set; }
        [JsonPropertyName("size")]   public long   Size   { get; set; }
    }

    public class PluginException : Exception
    {
        public PluginException(string message) : base(message) { }
    }
}
