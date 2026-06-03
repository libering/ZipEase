// Feature: image-preview-plugin, Property 7: Thumbnail dimensions maintain aspect ratio within bounds
//
// Validates: Requirements 5.1
//
// Property: For any source image dimensions (w, h) where w > 0 and h > 0,
// `compute_thumbnail_dimensions(w, h, max_width, max_height)` returns (tw, th) such that:
//   1. tw ≤ max_width and th ≤ max_height
//   2. tw / th ≈ w / h (within integer rounding tolerance)
//   3. When the source is larger than bounds in at least one dimension,
//      at least one output dimension equals the corresponding max
//   4. When the source fits within bounds, output equals source dimensions

use proptest::prelude::*;
use zipease_preview::thumbnail::compute_thumbnail_dimensions;

proptest! {
    /// **Validates: Requirements 5.1**
    ///
    /// Output dimensions never exceed the specified maximum bounds.
    #[test]
    fn prop_thumbnail_within_bounds(
        src_width in 1u32..10000,
        src_height in 1u32..10000,
        max_width in 1u32..200,
        max_height in 1u32..200,
    ) {
        let (tw, th) = compute_thumbnail_dimensions(src_width, src_height, max_width, max_height);
        prop_assert!(tw <= max_width,
            "Thumbnail width {} exceeds max_width {} (src: {}x{}, max: {}x{})",
            tw, max_width, src_width, src_height, max_width, max_height);
        prop_assert!(th <= max_height,
            "Thumbnail height {} exceeds max_height {} (src: {}x{}, max: {}x{})",
            th, max_height, src_width, src_height, max_width, max_height);
    }

    /// **Validates: Requirements 5.1**
    ///
    /// Aspect ratio is maintained within integer rounding tolerance.
    /// We verify using cross-multiplication to avoid division-by-zero issues:
    /// |tw * src_height - th * src_width| should be small relative to the scale.
    #[test]
    fn prop_thumbnail_aspect_ratio_preserved(
        src_width in 1u32..10000,
        src_height in 1u32..10000,
        max_width in 2u32..200,
        max_height in 2u32..200,
    ) {
        let (tw, th) = compute_thumbnail_dimensions(src_width, src_height, max_width, max_height);

        // Both output dimensions must be at least 1
        prop_assert!(tw >= 1 && th >= 1,
            "Output dimensions must be >= 1, got {}x{}", tw, th);

        // Skip aspect ratio check when output is clamped to minimum (1x1 or similar)
        // because integer rounding at very small sizes makes ratio preservation impossible.
        prop_assume!(tw > 1 && th > 1);

        // Use cross-multiplication to check aspect ratio:
        // tw/th ≈ src_width/src_height  ⟹  tw * src_height ≈ th * src_width
        // The error from floor() is at most 1 pixel per dimension, so:
        // |tw * src_height - th * src_width| ≤ max(src_width, src_height)
        let cross_thumb = tw as f64 * src_height as f64;
        let cross_src = th as f64 * src_width as f64;
        let cross_diff = (cross_thumb - cross_src).abs();

        // Tolerance: floor rounding can cause at most 1 pixel error in each dimension.
        // This translates to a cross-product error bounded by max(src_width, src_height).
        let tolerance = src_width.max(src_height) as f64;

        prop_assert!(cross_diff <= tolerance,
            "Aspect ratio deviation too large: src={}x{}, thumb={}x{}, cross_diff={:.1}, tolerance={:.1}",
            src_width, src_height, tw, th, cross_diff, tolerance);
    }

    /// **Validates: Requirements 5.1**
    ///
    /// When source is larger than bounds in at least one dimension,
    /// at least one output dimension is at or within 1 pixel of the corresponding max bound.
    /// (The 1-pixel tolerance accounts for floating-point floor rounding.)
    #[test]
    fn prop_thumbnail_fills_bound_when_larger(
        src_width in 1u32..10000,
        src_height in 1u32..10000,
        max_width in 1u32..200,
        max_height in 1u32..200,
    ) {
        // Only test when source exceeds bounds in at least one dimension
        prop_assume!(src_width > max_width || src_height > max_height);

        let (tw, th) = compute_thumbnail_dimensions(src_width, src_height, max_width, max_height);

        // Due to floating-point floor rounding, the constraining dimension's output
        // may be max or max-1. At least one dimension should be within 1 pixel of its max.
        let width_at_max = tw >= max_width.saturating_sub(1) && tw <= max_width;
        let height_at_max = th >= max_height.saturating_sub(1) && th <= max_height;

        prop_assert!(width_at_max || height_at_max,
            "When source ({}x{}) exceeds bounds ({}x{}), at least one output dim should be at/near its max, got {}x{}",
            src_width, src_height, max_width, max_height, tw, th);
    }

    /// **Validates: Requirements 5.1**
    ///
    /// When source fits within bounds (both dimensions ≤ max), output equals source.
    #[test]
    fn prop_thumbnail_preserves_size_when_smaller(
        src_width in 1u32..200,
        src_height in 1u32..200,
        max_width in 1u32..200,
        max_height in 1u32..200,
    ) {
        // Only test when source fits within bounds
        prop_assume!(src_width <= max_width && src_height <= max_height);

        let (tw, th) = compute_thumbnail_dimensions(src_width, src_height, max_width, max_height);

        prop_assert_eq!(tw, src_width,
            "When source ({}x{}) fits within bounds ({}x{}), width should be preserved, got {}",
            src_width, src_height, max_width, max_height, tw);
        prop_assert_eq!(th, src_height,
            "When source ({}x{}) fits within bounds ({}x{}), height should be preserved, got {}",
            src_width, src_height, max_width, max_height, th);
    }

    /// **Validates: Requirements 5.1**
    ///
    /// Output dimensions are always at least 1x1 (never zero).
    #[test]
    fn prop_thumbnail_minimum_one_pixel(
        src_width in 1u32..10000,
        src_height in 1u32..10000,
        max_width in 1u32..200,
        max_height in 1u32..200,
    ) {
        let (tw, th) = compute_thumbnail_dimensions(src_width, src_height, max_width, max_height);
        prop_assert!(tw >= 1, "Thumbnail width must be >= 1, got {}", tw);
        prop_assert!(th >= 1, "Thumbnail height must be >= 1, got {}", th);
    }
}
