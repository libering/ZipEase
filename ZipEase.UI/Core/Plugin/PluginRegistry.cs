using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;

namespace ZipEase.UI.Core.Plugin
{
    /// <summary>
    /// Scans %AppData%\ZipEase\plugins\ for installed plugins and exposes them.
    /// </summary>
    public static class PluginRegistry
    {
        public static readonly string PluginsDir = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "ZipEase", "plugins");

        private static List<LoadedPlugin>? _plugins;

        public static IReadOnlyList<LoadedPlugin> Plugins
        {
            get
            {
                if (_plugins == null) Reload();
                return _plugins!;
            }
        }

        /// <summary>Re-scan the plugins directory.</summary>
        public static void Reload()
        {
            _plugins = new List<LoadedPlugin>();
            if (!Directory.Exists(PluginsDir)) return;

            foreach (var dir in Directory.GetDirectories(PluginsDir))
            {
                var manifestPath = Path.Combine(dir, "plugin.json");
                if (!File.Exists(manifestPath)) continue;

                try
                {
                    var json = File.ReadAllText(manifestPath);
                    var manifest = JsonSerializer.Deserialize<PluginManifest>(json);
                    if (manifest == null) continue;

                    var exePath = Path.Combine(dir, manifest.Executable);
                    if (!File.Exists(exePath)) continue;

                    if (manifest.Requires7zaDll)
                    {
                        var pluginDll = Path.Combine(dir, "7za.dll");
                        var appDll = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "7za.dll");
                        if (!File.Exists(pluginDll) && !File.Exists(appDll))
                            continue; // Skip loading if 7za.dll is missing
                    }

                    _plugins.Add(new LoadedPlugin(manifest, exePath));
                }
                catch { /* skip malformed plugins */ }
            }
        }

        /// <summary>Find a plugin that handles the given file extension.</summary>
        public static LoadedPlugin? FindForExtension(string ext)
        {
            var lower = ext.ToLowerInvariant();
            foreach (var p in Plugins)
                foreach (var e in p.Manifest.Extensions)
                    if (e.Equals(lower, StringComparison.OrdinalIgnoreCase))
                        return p;
            return null;
        }

        /// <summary>Find a fallback plugin that handles the given file extension if the primary plugin fails.</summary>
        public static LoadedPlugin? FindFallbackPlugin(string ext)
        {
            var lower = ext.ToLowerInvariant();
            var primary = FindForExtension(lower);
            if (primary?.Manifest.FallbackExtensions != null &&
                primary.Manifest.FallbackExtensions.TryGetValue(lower, out var fallbackName) &&
                !string.IsNullOrEmpty(fallbackName))
            {
                foreach (var p in Plugins)
                {
                    if (p.Manifest.Name.Equals(fallbackName, StringComparison.OrdinalIgnoreCase))
                        return p;
                }
            }
            return null;
        }
    }


    public class LoadedPlugin
    {
        public PluginManifest Manifest { get; }
        public string ExecutablePath  { get; }

        public string DisplayExtensions => string.Join(", ", Manifest.Extensions);
        public string DisplayCapabilities => Manifest.CanCompress ? "解壓、壓縮" : "解壓";

        public LoadedPlugin(PluginManifest manifest, string exePath)
        {
            Manifest = manifest;
            ExecutablePath = exePath;
        }
    }
}
