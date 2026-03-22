/// Property-based tests for toast notification XML builders and FFI layer.
///
/// Feature: toast-notifications
use proptest::prelude::*;
use zipease_extract::ffi::notify::{zip_ease_notify_failure, zip_ease_notify_success};
use zipease_extract::notify::toast::{build_failure_xml, build_success_xml, ptr_to_string};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the raw XML string from an XmlDocument.
fn xml_to_string(doc: &windows::Data::Xml::Dom::XmlDocument) -> String {
    doc.GetXml()
        .expect("GetXml() should not fail on a valid document")
        .to_string()
}

// ---------------------------------------------------------------------------
// Property 1 — Success toast XML structure
// Validates: Requirements 1.1, 1.2, 2.1
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 1.1, 1.2, 2.1**
    ///
    /// Feature: toast-notifications, Property 1: Success toast XML structure
    #[test]
    fn success_xml_contains_required_fields(
        name  in "[a-zA-Z0-9_\\-\\.]{1,40}",
        folder in "C:\\\\[a-zA-Z0-9]{1,20}",
        count in 0i32..10_000i32,
    ) {
        let doc = build_success_xml(&name, &folder, count)
            .expect("build_success_xml should succeed for valid inputs");
        let xml = xml_to_string(&doc);

        prop_assert!(xml.contains(&name),   "XML must contain archive name");
        prop_assert!(xml.contains(&count.to_string()), "XML must contain file count");
        prop_assert!(xml.contains("Open Folder"), "XML must contain Open Folder action");
        prop_assert!(xml.contains(&folder), "XML must contain output folder path");
    }
}

// ---------------------------------------------------------------------------
// Property 2 — Failure toast XML structure
// Validates: Requirements 3.1, 3.2, 3.3
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 3.1, 3.2, 3.3**
    ///
    /// Feature: toast-notifications, Property 2: Failure toast XML structure
    #[test]
    fn failure_xml_contains_required_fields(
        name in "[a-zA-Z0-9_\\-\\.]{1,40}",
        msg  in "\\PC{0,100}",
    ) {
        let doc = build_failure_xml(&name, &msg)
            .expect("build_failure_xml should succeed for valid inputs");
        let xml = xml_to_string(&doc);

        prop_assert!(xml.contains(&name), "XML must contain archive name");

        let expected_msg = if msg.trim().is_empty() {
            "Something went wrong \u{2014} please try again"
        } else {
            msg.as_str()
        };
        // WinRT XmlDocument serialises text-node content with only &amp; &lt; &gt; escaped.
        // Quotes and apostrophes are left as literals in text nodes.
        let escaped_msg = expected_msg
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        prop_assert!(xml.contains(escaped_msg.as_str()), "XML must contain error message or fallback");
        prop_assert!(!xml.contains("Open Folder"), "Failure XML must not contain Open Folder action");
    }
}

// ---------------------------------------------------------------------------
// Property 3 — No-crash under adversarial inputs
// Validates: Requirements 5.4, 5.5, 7.1
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 5.4, 5.5, 7.1**
    ///
    /// Feature: toast-notifications, Property 3: No-crash under adversarial inputs
    #[test]
    fn ffi_does_not_crash_on_null_inputs(_seed in 0u8..=255u8) {
        // null archive_name, null output_folder — must not panic
        zip_ease_notify_success(std::ptr::null(), std::ptr::null(), 0);
        // null archive_name, null error_msg — must not panic
        zip_ease_notify_failure(std::ptr::null(), std::ptr::null());
        // Reaching here means no panic crossed the FFI boundary
    }
}

// ---------------------------------------------------------------------------
// Property 4 — UTF-16 round-trip fidelity
// Validates: Requirements 7.2, 7.3
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// **Validates: Requirements 7.2, 7.3**
    ///
    /// Feature: toast-notifications, Property 4: UTF-16 round-trip fidelity
    #[test]
    fn utf16_roundtrip(s in "\\PC*") {
        let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
        let recovered = ptr_to_string(wide.as_ptr());
        prop_assert_eq!(s, recovered, "UTF-16 round-trip must be byte-equivalent");
    }
}

// ---------------------------------------------------------------------------
// Edge-case unit tests
// ---------------------------------------------------------------------------

#[test]
fn null_ptr_to_notify_success_does_not_panic() {
    zip_ease_notify_success(std::ptr::null(), std::ptr::null(), 0);
}

#[test]
fn null_ptr_to_notify_failure_does_not_panic() {
    zip_ease_notify_failure(std::ptr::null(), std::ptr::null());
}

#[test]
fn empty_error_message_produces_fallback_text() {
    let doc = build_failure_xml("archive.zip", "").expect("should build XML");
    let xml = xml_to_string(&doc);
    assert!(
        xml.contains("Something went wrong"),
        "Empty error_msg must produce fallback text"
    );
}

#[test]
fn whitespace_only_error_message_produces_fallback_text() {
    let doc = build_failure_xml("archive.zip", "   ").expect("should build XML");
    let xml = xml_to_string(&doc);
    assert!(
        xml.contains("Something went wrong"),
        "Whitespace-only error_msg must produce fallback text"
    );
}

#[test]
fn failure_xml_has_no_action_element() {
    let doc = build_failure_xml("archive.zip", "file is corrupted").expect("should build XML");
    let xml = xml_to_string(&doc);
    assert!(
        !xml.contains("<action"),
        "Failure XML must not contain any <action> element"
    );
}

#[test]
fn success_xml_has_exactly_one_action_element() {
    let doc = build_success_xml("archive.zip", "C:\\Output", 42).expect("should build XML");
    let xml = xml_to_string(&doc);
    let count = xml.matches("<action ").count();
    assert_eq!(count, 1, "Success XML must contain exactly one <action> element");
}
