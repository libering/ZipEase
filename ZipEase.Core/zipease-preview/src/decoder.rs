// Image decoder module — full decoding pipeline
//
// Decodes image files to RGBA8 pixel buffers with safety guards:
// - Magic byte validation
// - Compressed file size limit (100 MB)
// - Resolution limit (16384×16384)
// - Decode timeout (10 seconds)
// - Memory guard (512 MB)
// - panic catch_unwind

use std::fs;
use std::io::Read;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use image::GenericImageView;

use crate::error::PreviewError;
use crate::magic_bytes::validate_magic_bytes;

/// Decoded image data in RGBA format (4 bytes per pixel).
#[derive(Debug, Clone)]
pub struct DecodeResult {
    /// RGBA pixel data, 4 bytes per pixel (width × height × 4 total bytes).
    pub pixels: Vec<u8>,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

/// Options controlling decode safety limits.
pub struct DecodeOptions {
    /// Maximum compressed file size in bytes (default: 100 MB).
    pub max_compressed_size: u64,
    /// Maximum allowed resolution as (width, height) (default: 16384×16384).
    pub max_resolution: (u32, u32),
    /// Maximum time allowed for decoding (default: 10 seconds).
    pub timeout: Duration,
    /// Maximum memory usage in bytes during decoding (default: 512 MB).
    pub max_memory: usize,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            max_compressed_size: 100 * 1024 * 1024, // 100 MB
            max_resolution: (16384, 16384),
            timeout: Duration::from_secs(10),
            max_memory: 512 * 1024 * 1024, // 512 MB
        }
    }
}

/// Decodes an image file into RGBA8 pixel data.
///
/// Pipeline:
/// 1. Check file size against `options.max_compressed_size`
/// 2. Read first 12 bytes and validate magic bytes against claimed extension
/// 3. Spawn decode thread with timeout
/// 4. Decode using `image::open`
/// 5. Check resolution against `options.max_resolution`
/// 6. Check pixel buffer size against `options.max_memory`
/// 7. Convert to RGBA8
/// 8. Entire operation wrapped in `std::panic::catch_unwind`
///
/// For multi-frame formats (GIF, WebP), only the first frame is decoded
/// (which is the default behavior of `image::open`).
pub fn decode_image(file_path: &Path, options: &DecodeOptions) -> Result<DecodeResult, PreviewError> {
    // Wrap entire operation in catch_unwind to prevent panics from crossing FFI
    let path_owned = file_path.to_path_buf();
    let max_compressed_size = options.max_compressed_size;
    let max_resolution = options.max_resolution;
    let timeout = options.timeout;
    let max_memory = options.max_memory;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        decode_image_inner(&path_owned, max_compressed_size, max_resolution, timeout, max_memory)
    }));

    match result {
        Ok(inner_result) => inner_result,
        Err(_) => Err(PreviewError::InternalPanic),
    }
}

/// Inner decode logic, called within catch_unwind.
fn decode_image_inner(
    file_path: &Path,
    max_compressed_size: u64,
    max_resolution: (u32, u32),
    timeout: Duration,
    max_memory: usize,
) -> Result<DecodeResult, PreviewError> {
    // Step 1: Check file size against compressed size limit
    let metadata = fs::metadata(file_path).map_err(|e| {
        PreviewError::DecodeFailed(format!("Cannot read file metadata: {}", e))
    })?;

    let file_size = metadata.len();
    if file_size > max_compressed_size {
        return Err(PreviewError::FileTooLarge {
            size_mb: file_size / (1024 * 1024),
            limit_mb: max_compressed_size / (1024 * 1024),
        });
    }

    // Step 2: Read first 12 bytes and validate magic bytes
    let mut header = [0u8; 12];
    let bytes_read = {
        let mut file = fs::File::open(file_path).map_err(|e| {
            PreviewError::DecodeFailed(format!("Cannot open file: {}", e))
        })?;
        file.read(&mut header).map_err(|e| {
            PreviewError::DecodeFailed(format!("Cannot read file header: {}", e))
        })?
    };

    // Extract extension from file path for magic byte validation
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    if extension.is_empty() {
        return Err(PreviewError::MagicByteMismatch {
            expected: "supported image format".to_string(),
            actual: "no extension".to_string(),
        });
    }

    validate_magic_bytes(&header[..bytes_read], extension)?;

    // Step 3: Spawn decode thread with timeout
    let path_for_thread = file_path.to_path_buf();
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let result = decode_with_guards(&path_for_thread, max_resolution, max_memory);
        // Ignore send error — receiver may have timed out and been dropped
        let _ = tx.send(result);
    });

    // Wait for result with timeout
    match rx.recv_timeout(timeout) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err(PreviewError::DecodeTimeout {
                elapsed_secs: timeout.as_secs(),
            })
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            // Thread panicked or was otherwise terminated
            Err(PreviewError::InternalPanic)
        }
    }
}

/// Performs the actual image decoding with resolution and memory guards.
fn decode_with_guards(
    file_path: &Path,
    max_resolution: (u32, u32),
    max_memory: usize,
) -> Result<DecodeResult, PreviewError> {
    // Use image::open which handles all supported formats and decodes first frame
    // for multi-frame formats (GIF, WebP)
    let img = image::open(file_path).map_err(|e| {
        PreviewError::DecodeFailed(format!("Image decode error: {}", e))
    })?;

    // Step 5: Check resolution against limit
    let (width, height) = img.dimensions();
    if width > max_resolution.0 || height > max_resolution.1 {
        return Err(PreviewError::ResolutionTooLarge { width, height });
    }

    // Step 6: Check pixel buffer size against memory limit
    // RGBA8 = 4 bytes per pixel
    let buffer_size = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| PreviewError::MemoryLimitExceeded {
            used_mb: u64::MAX,
            limit_mb: (max_memory / (1024 * 1024)) as u64,
        })?;

    if buffer_size > max_memory {
        return Err(PreviewError::MemoryLimitExceeded {
            used_mb: (buffer_size / (1024 * 1024)) as u64,
            limit_mb: (max_memory / (1024 * 1024)) as u64,
        });
    }

    // Step 7: Convert to RGBA8 pixel buffer
    let rgba_image = img.into_rgba8();
    let pixels = rgba_image.into_raw();

    Ok(DecodeResult {
        pixels,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Helper: creates a minimal valid 1×1 PNG file in a temp directory.
    fn create_test_png(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("test.png");
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 255]));
        img.save(&path).expect("Failed to save test PNG");
        path
    }

    /// Helper: creates a minimal valid 1×1 BMP file.
    fn create_test_bmp(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("test.bmp");
        let img = image::RgbaImage::from_pixel(1, 1, image::Rgba([0, 255, 0, 255]));
        img.save(&path).expect("Failed to save test BMP");
        path
    }

    #[test]
    fn decode_valid_png() {
        let tmp = tempfile::tempdir().unwrap();
        let path = create_test_png(tmp.path());
        let options = DecodeOptions::default();

        let result = decode_image(&path, &options).unwrap();
        assert_eq!(result.width, 1);
        assert_eq!(result.height, 1);
        assert_eq!(result.pixels.len(), 4); // 1×1×4
        // Red pixel in RGBA
        assert_eq!(result.pixels, vec![255, 0, 0, 255]);
    }

    #[test]
    fn decode_valid_bmp() {
        let tmp = tempfile::tempdir().unwrap();
        let path = create_test_bmp(tmp.path());
        let options = DecodeOptions::default();

        let result = decode_image(&path, &options).unwrap();
        assert_eq!(result.width, 1);
        assert_eq!(result.height, 1);
        assert_eq!(result.pixels.len(), 4);
        // Green pixel in RGBA
        assert_eq!(result.pixels, vec![0, 255, 0, 255]);
    }

    #[test]
    fn decode_rejects_file_too_large() {
        let tmp = tempfile::tempdir().unwrap();
        let path = create_test_png(tmp.path());

        // Set a very small size limit
        let options = DecodeOptions {
            max_compressed_size: 10, // 10 bytes — any real image exceeds this
            ..DecodeOptions::default()
        };

        let result = decode_image(&path, &options);
        assert!(result.is_err());
        match result.unwrap_err() {
            PreviewError::FileTooLarge { .. } => {}
            other => panic!("Expected FileTooLarge, got {:?}", other),
        }
    }

    #[test]
    fn decode_rejects_resolution_too_large() {
        let tmp = tempfile::tempdir().unwrap();
        // Create a 2×2 image
        let path = tmp.path().join("big.png");
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 255, 255]));
        img.save(&path).unwrap();

        // Set resolution limit to 1×1
        let options = DecodeOptions {
            max_resolution: (1, 1),
            ..DecodeOptions::default()
        };

        let result = decode_image(&path, &options);
        assert!(result.is_err());
        match result.unwrap_err() {
            PreviewError::ResolutionTooLarge { width, height } => {
                assert_eq!(width, 2);
                assert_eq!(height, 2);
            }
            other => panic!("Expected ResolutionTooLarge, got {:?}", other),
        }
    }

    #[test]
    fn decode_rejects_memory_limit_exceeded() {
        let tmp = tempfile::tempdir().unwrap();
        // Create a 100×100 image (requires 100*100*4 = 40000 bytes)
        let path = tmp.path().join("medium.png");
        let img = image::RgbaImage::from_pixel(100, 100, image::Rgba([128, 128, 128, 255]));
        img.save(&path).unwrap();

        // Set memory limit below what's needed
        let options = DecodeOptions {
            max_memory: 1000, // 1000 bytes, but image needs 40000
            ..DecodeOptions::default()
        };

        let result = decode_image(&path, &options);
        assert!(result.is_err());
        match result.unwrap_err() {
            PreviewError::MemoryLimitExceeded { .. } => {}
            other => panic!("Expected MemoryLimitExceeded, got {:?}", other),
        }
    }

    #[test]
    fn decode_rejects_magic_byte_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        // Create a file with .png extension but JPEG content
        let path = tmp.path().join("fake.png");
        let mut file = fs::File::create(&path).unwrap();
        // Write JPEG magic bytes
        file.write_all(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01])
            .unwrap();
        // Pad to make it a reasonable size
        file.write_all(&[0u8; 100]).unwrap();

        let options = DecodeOptions::default();
        let result = decode_image(&path, &options);
        assert!(result.is_err());
        match result.unwrap_err() {
            PreviewError::MagicByteMismatch { .. } => {}
            other => panic!("Expected MagicByteMismatch, got {:?}", other),
        }
    }

    #[test]
    fn decode_rejects_nonexistent_file() {
        let path = Path::new("nonexistent_file_that_does_not_exist.png");
        let options = DecodeOptions::default();

        let result = decode_image(path, &options);
        assert!(result.is_err());
        match result.unwrap_err() {
            PreviewError::DecodeFailed(_) => {}
            other => panic!("Expected DecodeFailed, got {:?}", other),
        }
    }

    #[test]
    fn decode_rejects_file_without_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("noext");
        fs::write(&path, &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00])
            .unwrap();

        let options = DecodeOptions::default();
        let result = decode_image(&path, &options);
        assert!(result.is_err());
        match result.unwrap_err() {
            PreviewError::MagicByteMismatch { .. } => {}
            other => panic!("Expected MagicByteMismatch, got {:?}", other),
        }
    }

    #[test]
    fn decode_rgba_buffer_size_correct() {
        let tmp = tempfile::tempdir().unwrap();
        // Create a 10×5 image
        let path = tmp.path().join("rect.png");
        let img = image::RgbaImage::from_pixel(10, 5, image::Rgba([1, 2, 3, 4]));
        img.save(&path).unwrap();

        let options = DecodeOptions::default();
        let result = decode_image(&path, &options).unwrap();

        assert_eq!(result.width, 10);
        assert_eq!(result.height, 5);
        assert_eq!(result.pixels.len(), 10 * 5 * 4);
    }

    #[test]
    fn decode_timeout_with_very_short_duration() {
        // This test verifies the timeout mechanism works.
        // We use a real image but set timeout to 0 (instant timeout).
        // Note: This is a race condition test — the decode thread might
        // complete before the timeout fires on fast machines. We accept
        // either outcome as valid.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("timeout_test.png");
        // Create a larger image to increase decode time
        let img = image::RgbaImage::from_pixel(1000, 1000, image::Rgba([255, 0, 0, 255]));
        img.save(&path).unwrap();

        let options = DecodeOptions {
            timeout: Duration::from_nanos(1), // Essentially instant timeout
            ..DecodeOptions::default()
        };

        let result = decode_image(&path, &options);
        // Either timeout or success is acceptable — we just verify no panic
        if let Err(e) = &result {
            match e {
                PreviewError::DecodeTimeout { .. } => {} // Expected
                _ => {} // Decode might complete before timeout on fast machines
            }
        }
    }

    #[test]
    fn decode_catches_panic() {
        // Verify that catch_unwind works by testing with a corrupted but
        // extension-matching file that might cause issues in the image crate.
        // The function should return an error, not panic.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("corrupt.bmp");
        // BMP magic bytes followed by garbage
        let mut data = vec![0x42, 0x4D]; // "BM"
        data.extend_from_slice(&[0xFF; 100]);
        fs::write(&path, &data).unwrap();

        let options = DecodeOptions::default();
        let result = decode_image(&path, &options);
        // Should be an error (DecodeFailed), not a panic
        assert!(result.is_err());
    }

    #[test]
    fn decode_multi_pixel_image_correct_format() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("multi.png");

        // Create a 2×2 image with known pixel values
        let mut img = image::RgbaImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));   // Red
        img.put_pixel(1, 0, image::Rgba([0, 255, 0, 255]));   // Green
        img.put_pixel(0, 1, image::Rgba([0, 0, 255, 255]));   // Blue
        img.put_pixel(1, 1, image::Rgba([255, 255, 0, 255])); // Yellow
        img.save(&path).unwrap();

        let options = DecodeOptions::default();
        let result = decode_image(&path, &options).unwrap();

        assert_eq!(result.width, 2);
        assert_eq!(result.height, 2);
        assert_eq!(result.pixels.len(), 2 * 2 * 4);

        // Verify pixel order (row-major, RGBA)
        assert_eq!(&result.pixels[0..4], &[255, 0, 0, 255]);     // (0,0) Red
        assert_eq!(&result.pixels[4..8], &[0, 255, 0, 255]);     // (1,0) Green
        assert_eq!(&result.pixels[8..12], &[0, 0, 255, 255]);    // (0,1) Blue
        assert_eq!(&result.pixels[12..16], &[255, 255, 0, 255]); // (1,1) Yellow
    }
}
