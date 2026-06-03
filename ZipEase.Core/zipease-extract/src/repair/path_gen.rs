use std::path::{Path, PathBuf};

/// Generate the repair output path: `{stem}_repaired.{ext}`.
/// If that path exists, increment: `_repaired_2`, `_repaired_3`, etc.
/// The generated path will never equal the original path.
pub fn generate_repair_path(original: &Path) -> PathBuf {
    let stem = original
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let ext = original
        .extension()
        .map(|e| e.to_string_lossy().to_string());
    let parent = original.parent().unwrap_or_else(|| Path::new(""));

    let make_path = |suffix: &str| -> PathBuf {
        let filename = match &ext {
            Some(e) => format!("{stem}{suffix}.{e}"),
            None => format!("{stem}{suffix}"),
        };
        parent.join(filename)
    };

    let first = make_path("_repaired");
    // Safety: generated path must never equal the original path
    if first != original && !first.exists() {
        return first;
    }

    let mut counter = 2u32;
    loop {
        let candidate = make_path(&format!("_repaired_{counter}"));
        if candidate != original && !candidate.exists() {
            return candidate;
        }
        counter += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_basic_zip_file() {
        let dir = TempDir::new().unwrap();
        let original = dir.path().join("archive.zip");
        fs::write(&original, b"fake").unwrap();

        let result = generate_repair_path(&original);
        assert_eq!(result, dir.path().join("archive_repaired.zip"));
    }

    #[test]
    fn test_no_extension() {
        let dir = TempDir::new().unwrap();
        let original = dir.path().join("archive");
        fs::write(&original, b"fake").unwrap();

        let result = generate_repair_path(&original);
        assert_eq!(result, dir.path().join("archive_repaired"));
    }

    #[test]
    fn test_multiple_dots() {
        let dir = TempDir::new().unwrap();
        let original = dir.path().join("my.archive.zip");
        fs::write(&original, b"fake").unwrap();

        let result = generate_repair_path(&original);
        assert_eq!(result, dir.path().join("my.archive_repaired.zip"));
    }

    #[test]
    fn test_unicode_filename() {
        let dir = TempDir::new().unwrap();
        let original = dir.path().join("壓縮檔.zip");
        fs::write(&original, b"fake").unwrap();

        let result = generate_repair_path(&original);
        assert_eq!(result, dir.path().join("壓縮檔_repaired.zip"));
    }

    #[test]
    fn test_increment_when_repaired_exists() {
        let dir = TempDir::new().unwrap();
        let original = dir.path().join("archive.zip");
        fs::write(&original, b"fake").unwrap();
        // Create the first _repaired file so it must increment
        fs::write(dir.path().join("archive_repaired.zip"), b"exists").unwrap();

        let result = generate_repair_path(&original);
        assert_eq!(result, dir.path().join("archive_repaired_2.zip"));
    }

    #[test]
    fn test_increment_multiple_existing() {
        let dir = TempDir::new().unwrap();
        let original = dir.path().join("archive.zip");
        fs::write(&original, b"fake").unwrap();
        fs::write(dir.path().join("archive_repaired.zip"), b"exists").unwrap();
        fs::write(dir.path().join("archive_repaired_2.zip"), b"exists").unwrap();
        fs::write(dir.path().join("archive_repaired_3.zip"), b"exists").unwrap();

        let result = generate_repair_path(&original);
        assert_eq!(result, dir.path().join("archive_repaired_4.zip"));
    }

    #[test]
    fn test_generated_path_never_equals_original() {
        let dir = TempDir::new().unwrap();
        let original = dir.path().join("test.zip");
        fs::write(&original, b"fake").unwrap();

        let result = generate_repair_path(&original);
        assert_ne!(result, original);
    }

    #[test]
    fn test_no_extension_increment() {
        let dir = TempDir::new().unwrap();
        let original = dir.path().join("archive");
        fs::write(&original, b"fake").unwrap();
        fs::write(dir.path().join("archive_repaired"), b"exists").unwrap();

        let result = generate_repair_path(&original);
        assert_eq!(result, dir.path().join("archive_repaired_2"));
    }
}
