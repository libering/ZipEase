// Feature: image-preview-plugin, Property 13: Decode produces correct RGBA buffer size
//
// **Validates: Requirements 6.1, 6.2, 6.3**
//
// For any valid image file within size and resolution limits, `decode_image`
// returns a pixel buffer of exactly `width × height × 4` bytes, where width
// and height match the image's actual dimensions (or first frame dimensions
// for animated formats).

use proptest::prelude::*;
use std::path::Path;
use tempfile::TempDir;
use zipease_preview::decoder::{decode_image, DecodeOptions};

/// Strategy: generate random image dimensions (width 1..200, height 1..200).
/// Kept small for test speed.
fn dimension_strategy() -> impl Strategy<Value = (u32, u32)> {
    (1u32..=200, 1u32..=200)
}

/// Creates a PNG image file with the given dimensions and returns the path.
fn create_png_image(dir: &Path, width: u32, height: u32) -> std::path::PathBuf {
    let path = dir.join(format!("test_{}x{}.png", width, height));
    let img = image::RgbaImage::from_pixel(width, height, image::Rgba([128, 64, 32, 255]));
    img.save(&path).expect("Failed to save test PNG");
    path
}

/// Creates a BMP image file with the given dimensions and returns the path.
fn create_bmp_image(dir: &Path, width: u32, height: u32) -> std::path::PathBuf {
    let path = dir.join(format!("test_{}x{}.bmp", width, height));
    let img = image::RgbaImage::from_pixel(width, height, image::Rgba([64, 128, 255, 200]));
    img.save(&path).expect("Failed to save test BMP");
    path
}

proptest! {
    /// Property 13a: For any valid PNG image with random dimensions (1..200 × 1..200),
    /// decode_image returns a pixel buffer of exactly width × height × 4 bytes,
    /// and the reported width/height match the generated dimensions.
    #[test]
    fn decode_png_produces_correct_rgba_buffer_size(
        (width, height) in dimension_strategy(),
    ) {
        let tmp = TempDir::new().unwrap();
        let path = create_png_image(tmp.path(), width, height);
        let options = DecodeOptions::default();

        let result = decode_image(&path, &options).unwrap();

        // Verify width and height match the generated dimensions
        prop_assert_eq!(
            result.width, width,
            "Decoded width {} does not match generated width {}",
            result.width, width
        );
        prop_assert_eq!(
            result.height, height,
            "Decoded height {} does not match generated height {}",
            result.height, height
        );

        // Verify pixel buffer is exactly width × height × 4 bytes (RGBA)
        let expected_len = (width as usize) * (height as usize) * 4;
        prop_assert_eq!(
            result.pixels.len(), expected_len,
            "Buffer size {} does not match expected {} ({}×{}×4)",
            result.pixels.len(), expected_len, width, height
        );
    }

    /// Property 13b: For any valid BMP image with random dimensions (1..200 × 1..200),
    /// decode_image returns a pixel buffer of exactly width × height × 4 bytes,
    /// and the reported width/height match the generated dimensions.
    #[test]
    fn decode_bmp_produces_correct_rgba_buffer_size(
        (width, height) in dimension_strategy(),
    ) {
        let tmp = TempDir::new().unwrap();
        let path = create_bmp_image(tmp.path(), width, height);
        let options = DecodeOptions::default();

        let result = decode_image(&path, &options).unwrap();

        // Verify width and height match the generated dimensions
        prop_assert_eq!(
            result.width, width,
            "Decoded width {} does not match generated width {}",
            result.width, width
        );
        prop_assert_eq!(
            result.height, height,
            "Decoded height {} does not match generated height {}",
            result.height, height
        );

        // Verify pixel buffer is exactly width × height × 4 bytes (RGBA)
        let expected_len = (width as usize) * (height as usize) * 4;
        prop_assert_eq!(
            result.pixels.len(), expected_len,
            "Buffer size {} does not match expected {} ({}×{}×4)",
            result.pixels.len(), expected_len, width, height
        );
    }
}
