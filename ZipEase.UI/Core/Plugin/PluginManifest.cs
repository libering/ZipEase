using System.Collections.Generic;
using System.Text.Json.Serialization;

namespace ZipEase.UI.Core.Plugin
{
    /// <summary>
    /// Deserialised from plugin.json in each plugin directory.
    /// </summary>
    public class PluginManifest
    {
        [JsonPropertyName("name")]        public string Name        { get; set; } = string.Empty;
        [JsonPropertyName("version")]     public string Version     { get; set; } = "0.0.0";
        [JsonPropertyName("author")]      public string Author      { get; set; } = string.Empty;
        [JsonPropertyName("description")] public string Description { get; set; } = string.Empty;
        /// <summary>File extensions this plugin handles, e.g. [".lzma", ".lz4"]</summary>
        [JsonPropertyName("extensions")]  public List<string> Extensions { get; set; } = new();
        /// <summary>Relative path to the executable inside the plugin directory.</summary>
        [JsonPropertyName("executable")]  public string Executable  { get; set; } = string.Empty;
        /// <summary>Whether the plugin supports compression (optional).</summary>
        [JsonPropertyName("can_compress")] public bool CanCompress  { get; set; } = false;
        /// <summary>Whether the plugin requires 7za.dll to function (optional).</summary>
        [JsonPropertyName("requires_7za_dll")] public bool Requires7zaDll { get; set; } = false;

        /// <summary>Fallback plugins mapping if this plugin fails to run (optional).</summary>
        [JsonPropertyName("fallback_extensions")] public Dictionary<string, string?>? FallbackExtensions { get; set; }
    }
}

