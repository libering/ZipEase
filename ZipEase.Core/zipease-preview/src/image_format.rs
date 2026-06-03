// Image format detection — extension classification
//
// Determines whether an archive entry is a previewable image based on its
// file extension. Comparison is case-insensitive and uses the substring
// after the last '.' in the filename.

use zipease_extract::ArchiveEntryInfo;

use crate::natural_sort::natural_cmp;

/// Supported image file extensions (lowercase, without leading dot).
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "webp", "tiff", "tif", "ico",
];

/// Returns `true` if the given archive entry name refers to a previewable image.
///
/// Rules:
/// - Directories are never previewable (`is_directory == true` → `false`).
/// - Names without a `.` character are not previewable.
/// - The extension is the substring after the **last** `.` in the name.
/// - Comparison against [`SUPPORTED_EXTENSIONS`] is case-insensitive.
pub fn is_previewable(entry_name: &str, is_directory: bool) -> bool {
    if is_directory {
        return false;
    }

    // Find the last '.' — if absent, not previewable.
    let dot_pos = match entry_name.rfind('.') {
        Some(pos) => pos,
        None => return false,
    };

    let extension = &entry_name[dot_pos + 1..];

    // Empty extension (name ends with '.') is not supported.
    if extension.is_empty() {
        return false;
    }

    // Case-insensitive comparison against supported extensions.
    let ext_lower = extension.to_ascii_lowercase();
    SUPPORTED_EXTENSIONS.contains(&ext_lower.as_str())
}

/// Returns the parent directory of an entry name within an archive.
///
/// Archive entries use `/` as the path separator. The parent directory is
/// everything up to and including the last `/` before the filename.
/// If there is no `/`, the entry is at the root level and the parent is `""`.
///
/// Examples:
/// - `"images/photo.png"` → `"images/"`
/// - `"a/b/c.jpg"` → `"a/b/"`
/// - `"root.png"` → `""`
fn parent_directory(entry_name: &str) -> &str {
    // Normalize: treat both '/' and '\' as separators, find last one
    let last_sep = entry_name.rfind(|c| c == '/' || c == '\\');
    match last_sep {
        Some(pos) => &entry_name[..=pos],
        None => "",
    }
}

/// Filters archive entries to those in the given directory that are previewable,
/// returning their names sorted by natural sort order.
///
/// - Only entries whose parent directory **exactly** matches `directory` are included.
/// - Only entries that pass [`is_previewable`] are included.
/// - Results are sorted using [`natural_cmp`].
///
/// The `directory` parameter should match the format used in archive entry names
/// (e.g., `"images/"` for entries like `"images/photo.png"`, or `""` for root-level entries).
pub fn previewable_entries_in_directory(
    entries: &[ArchiveEntryInfo],
    directory: &str,
) -> Vec<String> {
    let mut result: Vec<String> = entries
        .iter()
        .filter(|entry| {
            parent_directory(&entry.name) == directory
                && is_previewable(&entry.name, entry.is_directory)
        })
        .map(|entry| entry.name.clone())
        .collect();

    result.sort_by(|a, b| natural_cmp(a, b));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_extensions_recognized() {
        for ext in SUPPORTED_EXTENSIONS {
            let name = format!("image.{}", ext);
            assert!(
                is_previewable(&name, false),
                "Expected '{}' to be previewable",
                name
            );
        }
    }

    #[test]
    fn test_case_insensitive_matching() {
        assert!(is_previewable("photo.PNG", false));
        assert!(is_previewable("photo.Jpg", false));
        assert!(is_previewable("photo.JPEG", false));
        assert!(is_previewable("photo.GiF", false));
        assert!(is_previewable("photo.BMP", false));
        assert!(is_previewable("photo.WebP", false));
        assert!(is_previewable("photo.TIFF", false));
        assert!(is_previewable("photo.TIF", false));
        assert!(is_previewable("photo.ICO", false));
    }

    #[test]
    fn test_directories_not_previewable() {
        assert!(!is_previewable("images/", true));
        assert!(!is_previewable("photo.png", true));
        assert!(!is_previewable("folder.jpg/", true));
    }

    #[test]
    fn test_no_dot_not_previewable() {
        assert!(!is_previewable("readme", false));
        assert!(!is_previewable("Makefile", false));
        assert!(!is_previewable("LICENSE", false));
    }

    #[test]
    fn test_unsupported_extensions() {
        assert!(!is_previewable("document.pdf", false));
        assert!(!is_previewable("archive.zip", false));
        assert!(!is_previewable("code.rs", false));
        assert!(!is_previewable("data.json", false));
        assert!(!is_previewable("video.mp4", false));
    }

    #[test]
    fn test_last_dot_used_for_extension() {
        // "archive.tar.gz" → extension is "gz", not previewable
        assert!(!is_previewable("archive.tar.gz", false));
        // "photo.backup.png" → extension is "png", previewable
        assert!(is_previewable("photo.backup.png", false));
        // "my.file.name.jpg" → extension is "jpg", previewable
        assert!(is_previewable("my.file.name.jpg", false));
    }

    #[test]
    fn test_trailing_dot_not_previewable() {
        assert!(!is_previewable("file.", false));
        assert!(!is_previewable("photo.png.", false));
    }

    #[test]
    fn test_path_separators_in_name() {
        // Entry names in archives can contain path separators
        assert!(is_previewable("images/photo.png", false));
        assert!(is_previewable("deep/nested/dir/image.JPEG", false));
        assert!(!is_previewable("images/readme.txt", false));
    }

    #[test]
    fn test_empty_name() {
        assert!(!is_previewable("", false));
    }

    #[test]
    fn test_dot_only_name() {
        assert!(!is_previewable(".", false));
        assert!(!is_previewable("..", false));
        assert!(!is_previewable(".hidden", false)); // extension is "hidden", not supported
    }

    #[test]
    fn test_hidden_file_with_supported_extension() {
        // ".png" → dot at position 0, extension is "png"
        assert!(is_previewable(".png", false));
        // ".hidden.jpg" → last dot at position 7, extension is "jpg"
        assert!(is_previewable(".hidden.jpg", false));
    }

    // ── Tests for parent_directory ──────────────────────────────────────────

    #[test]
    fn test_parent_directory_with_slash() {
        assert_eq!(parent_directory("images/photo.png"), "images/");
        assert_eq!(parent_directory("a/b/c.jpg"), "a/b/");
    }

    #[test]
    fn test_parent_directory_root_level() {
        assert_eq!(parent_directory("photo.png"), "");
        assert_eq!(parent_directory("readme.txt"), "");
    }

    #[test]
    fn test_parent_directory_backslash() {
        assert_eq!(parent_directory("images\\photo.png"), "images\\");
    }

    // ── Tests for previewable_entries_in_directory ───────────────────────────

    #[test]
    fn test_previewable_entries_filters_by_directory() {
        let entries = vec![
            ArchiveEntryInfo { name: "images/a.png".to_string(), is_directory: false, size: 100 },
            ArchiveEntryInfo { name: "images/b.jpg".to_string(), is_directory: false, size: 200 },
            ArchiveEntryInfo { name: "other/c.png".to_string(), is_directory: false, size: 300 },
            ArchiveEntryInfo { name: "root.png".to_string(), is_directory: false, size: 50 },
        ];

        let result = previewable_entries_in_directory(&entries, "images/");
        assert_eq!(result, vec!["images/a.png", "images/b.jpg"]);
    }

    #[test]
    fn test_previewable_entries_root_directory() {
        let entries = vec![
            ArchiveEntryInfo { name: "photo.png".to_string(), is_directory: false, size: 100 },
            ArchiveEntryInfo { name: "readme.txt".to_string(), is_directory: false, size: 50 },
            ArchiveEntryInfo { name: "sub/image.jpg".to_string(), is_directory: false, size: 200 },
        ];

        let result = previewable_entries_in_directory(&entries, "");
        assert_eq!(result, vec!["photo.png"]);
    }

    #[test]
    fn test_previewable_entries_excludes_directories() {
        let entries = vec![
            ArchiveEntryInfo { name: "images/photo.png".to_string(), is_directory: false, size: 100 },
            ArchiveEntryInfo { name: "images/subdir".to_string(), is_directory: true, size: 0 },
            ArchiveEntryInfo { name: "images/icon.ico".to_string(), is_directory: false, size: 50 },
        ];

        let result = previewable_entries_in_directory(&entries, "images/");
        assert_eq!(result, vec!["images/icon.ico", "images/photo.png"]);
    }

    #[test]
    fn test_previewable_entries_excludes_non_image_files() {
        let entries = vec![
            ArchiveEntryInfo { name: "docs/readme.txt".to_string(), is_directory: false, size: 100 },
            ArchiveEntryInfo { name: "docs/diagram.png".to_string(), is_directory: false, size: 200 },
            ArchiveEntryInfo { name: "docs/notes.md".to_string(), is_directory: false, size: 50 },
        ];

        let result = previewable_entries_in_directory(&entries, "docs/");
        assert_eq!(result, vec!["docs/diagram.png"]);
    }

    #[test]
    fn test_previewable_entries_natural_sort_order() {
        let entries = vec![
            ArchiveEntryInfo { name: "img/photo10.png".to_string(), is_directory: false, size: 100 },
            ArchiveEntryInfo { name: "img/photo2.png".to_string(), is_directory: false, size: 100 },
            ArchiveEntryInfo { name: "img/photo1.png".to_string(), is_directory: false, size: 100 },
            ArchiveEntryInfo { name: "img/photo20.png".to_string(), is_directory: false, size: 100 },
        ];

        let result = previewable_entries_in_directory(&entries, "img/");
        assert_eq!(
            result,
            vec!["img/photo1.png", "img/photo2.png", "img/photo10.png", "img/photo20.png"]
        );
    }

    #[test]
    fn test_previewable_entries_empty_input() {
        let entries: Vec<ArchiveEntryInfo> = vec![];
        let result = previewable_entries_in_directory(&entries, "any/");
        assert!(result.is_empty());
    }

    #[test]
    fn test_previewable_entries_no_matches() {
        let entries = vec![
            ArchiveEntryInfo { name: "other/photo.png".to_string(), is_directory: false, size: 100 },
        ];

        let result = previewable_entries_in_directory(&entries, "images/");
        assert!(result.is_empty());
    }
}
