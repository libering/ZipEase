// Toast notification logic

use windows::Win32::System::Registry::{
    RegCreateKeyExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_CREATE_KEY_DISPOSITION,
    REG_OPTION_NON_VOLATILE,
};
use windows::core::PCWSTR;

const AUMID_REG_PATH: &str = "Software\\Classes\\AppUserModelId\\ZipEase.App";

/// Ensures "ZipEase.App" is registered under
/// HKCU\Software\Classes\AppUserModelId\ZipEase.App.
/// Silently ignores all registry errors (Requirement 4.2).
/// Skips the write if the key already exists (Requirement 4.3).
pub(crate) fn ensure_aumid_registered() {
    let path_wide: Vec<u16> = AUMID_REG_PATH
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut hkey = HKEY::default();
    let mut disposition = REG_CREATE_KEY_DISPOSITION::default();

    let _ = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(path_wide.as_ptr()),
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_READ | KEY_WRITE,
            None,
            &mut hkey,
            Some(&mut disposition),
        )
    };
    // hkey is closed automatically when dropped (windows-rs handles this)
    // disposition tells us if it was created or already existed — we don't need to act on it
}

/// Converts a null-terminated UTF-16 pointer to a Rust `String`.
/// Null pointer → returns an empty string.
/// Non-null → walks the null-terminated sequence and decodes via `from_utf16_lossy`.
pub fn ptr_to_string(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe {
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len)).to_owned()
    }
}

/// Builds the WinRT XML document for a success toast.
/// Body text: "Extracted {file_count} files from {archive_name}"
/// Includes one "Open Folder" action button (Requirement 2.1).
pub fn build_success_xml(
    archive_name: &str,
    output_folder: &str,
    file_count: i32,
) -> windows::core::Result<windows::Data::Xml::Dom::XmlDocument> {
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::core::HSTRING;

    let xml_str = format!(
        r#"<toast><visual><binding template="ToastGeneric"><text>Extraction complete</text><text>Extracted {} files from {}</text></binding></visual><actions><action content="Open Folder" activationType="protocol" arguments="explorer.exe {}"/></actions></toast>"#,
        file_count,
        escape_xml(archive_name),
        escape_xml(output_folder)
    );

    let doc = XmlDocument::new()?;
    doc.LoadXml(&HSTRING::from(xml_str.as_str()))?;
    Ok(doc)
}

/// Builds the WinRT XML document for a failure toast.
/// Substitutes empty/whitespace-only error_msg with a fallback (Requirement 3.3).
/// No action button (Requirement 3.2).
pub fn build_failure_xml(
    archive_name: &str,
    error_msg: &str,
) -> windows::core::Result<windows::Data::Xml::Dom::XmlDocument> {
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::core::HSTRING;

    let effective_msg = if error_msg.trim().is_empty() {
        "Something went wrong \u{2014} please try again"
    } else {
        error_msg
    };

    let xml_str = format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>Extraction failed</text><text>Couldn\u{2019}t extract {} \u{2014} {}</text></binding></visual></toast>",
        escape_xml(archive_name),
        escape_xml(effective_msg)
    );

    let doc = XmlDocument::new()?;
    doc.LoadXml(&HSTRING::from(xml_str.as_str()))?;
    Ok(doc)
}

/// Registers AUMID, builds success XML, and dispatches the toast.
/// All WinRT errors are discarded silently (Requirement 4.1).
pub fn notify_success(archive_name: &str, output_folder: &str, file_count: i32) {
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    use windows::core::HSTRING;

    ensure_aumid_registered();

    let Ok(xml) = build_success_xml(archive_name, output_folder, file_count) else {
        return;
    };
    let Ok(notifier) =
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from("ZipEase.App"))
    else {
        return;
    };
    if let Ok(notification) = ToastNotification::CreateToastNotification(&xml) {
        let _ = notifier.Show(&notification);
    }
}

/// Registers AUMID, builds failure XML, and dispatches the toast.
/// All WinRT errors are discarded silently (Requirement 4.1).
pub fn notify_failure(archive_name: &str, error_msg: &str) {
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    use windows::core::HSTRING;

    ensure_aumid_registered();

    let Ok(xml) = build_failure_xml(archive_name, error_msg) else {
        return;
    };
    let Ok(notifier) =
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from("ZipEase.App"))
    else {
        return;
    };
    if let Ok(notification) = ToastNotification::CreateToastNotification(&xml) {
        let _ = notifier.Show(&notification);
    }
}

/// Escapes special XML characters in a string.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&apos;")
}
