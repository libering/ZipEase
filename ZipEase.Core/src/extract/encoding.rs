use chardetng::EncodingDetector;

/// Decode raw ZIP filename bytes to a correct UTF-8 String.
///
/// `is_utf8_flag_set` is `true` when bit 11 of `general_purpose_bit_flag` is set.
/// No panics, no unwrap — this function is total over all byte inputs.
pub fn decode_zip_filename(raw: &[u8], is_utf8_flag_set: bool) -> String {
    // Step 1: UTF-8 flag fast path
    if is_utf8_flag_set {
        if let Ok(s) = std::str::from_utf8(raw) {
            return s.to_string();
        }
        // Flag was set but bytes aren't valid UTF-8 — fall through to detection
    }

    // Step 2: Strict UTF-8 parse (covers ASCII and already-valid UTF-8)
    if let Ok(s) = std::str::from_utf8(raw) {
        return s.to_string();
    }

    // Step 3: chardetng detection
    let mut detector = EncodingDetector::new();
    detector.feed(raw, true);
    let detected = detector.guess(None, true);

    // Step 4: If chardetng returned UTF-8 (its "undetermined" signal for non-UTF-8 bytes),
    // substitute WINDOWS_1252 as the explicit fallback
    let encoding = if detected == encoding_rs::UTF_8 {
        encoding_rs::WINDOWS_1252
    } else {
        detected
    };

    // Step 5: Decode with encoding_rs (never panics; uses replacement chars for truly
    // undecodable bytes as last resort)
    let (cow, _, _) = encoding.decode(raw);
    cow.into_owned()
}
