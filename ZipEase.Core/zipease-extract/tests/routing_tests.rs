//! Unit tests for smart backend routing.
//!
//! Feature: sevenzip-backend, task 4.3
//! Validates: Requirements 1.3, 1.4

// These tests verify that detect_backend routes to the correct backend by extension.
// Since detect_backend is private, we test it indirectly via smart_list_entries
// with non-existent paths — the error type tells us which backend was attempted.

use zipease_extract::extract::smart::smart_list_entries;
use zipease_shared::LockError;
use std::path::Path;

fn get_error_for_path(path: &str) -> LockError {
    smart_list_entries(Path::new(path)).unwrap_err()
}

#[test]
fn test_unsupported_format_returns_error() {
    // .exe is not a supported format
    let err = get_error_for_path("test.exe");
    assert!(
        matches!(err, LockError::UnsupportedFormat(_)),
        "expected UnsupportedFormat, got {:?}", err
    );
}

#[test]
fn test_no_extension_returns_error() {
    let err = get_error_for_path("noextension");
    assert!(
        matches!(err, LockError::UnsupportedFormat(_)),
        "expected UnsupportedFormat for no extension, got {:?}", err
    );
}

#[test]
fn test_zip_routes_to_zip_backend() {
    // A non-existent .zip file should fail with PathNotFound (not UnsupportedFormat)
    // because the ZIP backend was selected and tried to open the file
    let err = get_error_for_path("nonexistent_file_xyz.zip");
    assert!(
        !matches!(err, LockError::UnsupportedFormat(_)),
        ".zip should route to ZipBackend, not return UnsupportedFormat"
    );
}

#[test]
fn test_7z_routes_to_sevenz_backend() {
    let err = get_error_for_path("nonexistent_file_xyz.7z");
    assert!(
        !matches!(err, LockError::UnsupportedFormat(_)),
        ".7z should route to SevenZBackend, not return UnsupportedFormat"
    );
}

#[test]
fn test_tar_routes_to_tar_backend() {
    let err = get_error_for_path("nonexistent_file_xyz.tar");
    assert!(
        !matches!(err, LockError::UnsupportedFormat(_)),
        ".tar should route to TarBackend, not return UnsupportedFormat"
    );
}

#[test]
fn test_gz_routes_to_tar_backend() {
    let err = get_error_for_path("nonexistent_file_xyz.gz");
    assert!(
        !matches!(err, LockError::UnsupportedFormat(_)),
        ".gz should route to TarBackend, not return UnsupportedFormat"
    );
}

#[test]
fn test_rar_routes_to_sevenzadll_backend() {
    // .rar routes to SevenZaDllBackend — if 7za.dll is missing, returns PluginRequired
    // If 7za.dll is present, returns PathNotFound for the non-existent file
    let err = get_error_for_path("nonexistent_file_xyz.rar");
    assert!(
        !matches!(err, LockError::UnsupportedFormat(_)),
        ".rar should route to SevenZaDllBackend, not return UnsupportedFormat"
    );
}
