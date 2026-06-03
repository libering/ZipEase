using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

namespace ZipEase.UI.Core
{
    /// <summary>
    /// Manages directory-scoped image navigation state.
    /// Maintains a sorted list of previewable entries in the current archive directory
    /// and tracks the current position. Pure state management — no FFI calls, no UI dependencies.
    /// </summary>
    public sealed class NavigationService
    {
        private List<string> _entries = new();
        private int _currentIndex = -1;
        private bool _isLoading;

        // ─── Events ───────────────────────────────────────────────────────

        /// <summary>
        /// Fired when <see cref="GoNext"/> or <see cref="GoPrevious"/> is called successfully.
        /// The event argument is the entry name to navigate to.
        /// </summary>
        public event EventHandler<string>? NavigationRequested;

        // ─── Properties ───────────────────────────────────────────────────

        /// <summary>Current position in the navigation list (-1 if not initialized).</summary>
        public int CurrentIndex => _currentIndex;

        /// <summary>Current entry name, or empty string if not initialized.</summary>
        public string CurrentEntry =>
            _currentIndex >= 0 && _currentIndex < _entries.Count
                ? _entries[_currentIndex]
                : string.Empty;

        /// <summary>True if not at the last entry and not currently loading.</summary>
        public bool CanGoNext =>
            !_isLoading && _currentIndex >= 0 && _currentIndex < _entries.Count - 1;

        /// <summary>True if not at the first entry and not currently loading.</summary>
        public bool CanGoPrevious =>
            !_isLoading && _currentIndex > 0;

        /// <summary>
        /// Set externally to disable navigation during image load.
        /// When true, <see cref="CanGoNext"/> and <see cref="CanGoPrevious"/> return false.
        /// </summary>
        public bool IsLoading
        {
            get => _isLoading;
            set => _isLoading = value;
        }

        /// <summary>Total number of previewable entries in the current directory.</summary>
        public int EntryCount => _entries.Count;

        // ─── Methods ──────────────────────────────────────────────────────

        /// <summary>
        /// Initializes the navigation list with pre-sorted entry names and sets the
        /// current position to the specified entry.
        /// </summary>
        /// <param name="entries">
        /// Sorted list of previewable entry names in the current archive directory.
        /// The list is expected to be pre-sorted by natural sort order from the Rust backend.
        /// </param>
        /// <param name="currentEntry">The entry name to set as the current position.</param>
        public void Initialize(List<string> entries, string currentEntry)
        {
            _entries = entries ?? new List<string>();
            _currentIndex = _entries.IndexOf(currentEntry);
            _isLoading = false;
        }

        /// <summary>
        /// Advances to the next entry in the navigation list.
        /// Fires <see cref="NavigationRequested"/> with the new entry name.
        /// </summary>
        /// <returns>The next entry name, or null if already at the end or loading.</returns>
        public string? GoNext()
        {
            if (!CanGoNext)
                return null;

            _currentIndex++;
            string entry = _entries[_currentIndex];
            NavigationRequested?.Invoke(this, entry);
            return entry;
        }

        /// <summary>
        /// Goes to the previous entry in the navigation list.
        /// Fires <see cref="NavigationRequested"/> with the new entry name.
        /// </summary>
        /// <returns>The previous entry name, or null if already at the start or loading.</returns>
        public string? GoPrevious()
        {
            if (!CanGoPrevious)
                return null;

            _currentIndex--;
            string entry = _entries[_currentIndex];
            NavigationRequested?.Invoke(this, entry);
            return entry;
        }

        /// <summary>
        /// Clears the navigation state, resetting to uninitialized.
        /// </summary>
        public void Reset()
        {
            _entries = new List<string>();
            _currentIndex = -1;
            _isLoading = false;
        }

        // ─── Natural Sort (Windows StrCmpLogicalW) ────────────────────────

        [DllImport("shlwapi.dll", CharSet = CharSet.Unicode)]
        private static extern int StrCmpLogicalW(string x, string y);

        /// <summary>
        /// Compares two file names using Windows natural sort order.
        /// Numeric segments are compared by value (e.g., "img2" &lt; "img10").
        /// </summary>
        public static int NaturalCompare(string a, string b) => StrCmpLogicalW(a, b);
    }
}
