use zipease_shared::LockError;

/// All possible errors in the image preview subsystem.
///
/// Each variant carries internal diagnostic data for logging,
/// but `user_message()` returns only user-friendly text (Chinese)
/// with no internal details exposed.
#[derive(Debug, Clone)]
pub enum PreviewError {
    /// Archive extraction failed (corrupt archive, I/O error, etc.)
    ExtractionFailed(String),
    /// Image decoding failed (unsupported sub-format, corrupt image data)
    DecodeFailed(String),
    /// File header magic bytes do not match the claimed extension
    MagicByteMismatch { expected: String, actual: String },
    /// Compressed file size exceeds the allowed limit
    FileTooLarge { size_mb: u64, limit_mb: u64 },
    /// Image resolution exceeds the allowed limit
    ResolutionTooLarge { width: u32, height: u32 },
    /// Decode operation timed out
    DecodeTimeout { elapsed_secs: u64 },
    /// Memory usage exceeded the allowed limit during decoding
    MemoryLimitExceeded { used_mb: u64, limit_mb: u64 },
    /// Path traversal attempt detected
    PathTraversal(String),
    /// A Rust panic was caught by catch_unwind
    InternalPanic,
}

impl PreviewError {
    /// Returns a negative i32 error code for FFI return values.
    /// Each variant maps to a unique negative value.
    pub fn to_error_code(&self) -> i32 {
        match self {
            PreviewError::ExtractionFailed(_) => -1001,
            PreviewError::DecodeFailed(_) => -1002,
            PreviewError::MagicByteMismatch { .. } => -1003,
            PreviewError::FileTooLarge { .. } => -1004,
            PreviewError::ResolutionTooLarge { .. } => -1005,
            PreviewError::DecodeTimeout { .. } => -1006,
            PreviewError::MemoryLimitExceeded { .. } => -1007,
            PreviewError::PathTraversal(_) => -1008,
            PreviewError::InternalPanic => -1009,
        }
    }

    /// Returns a user-friendly error message in Chinese.
    /// No internal details (error codes, stack traces, paths) are exposed.
    pub fn user_message(&self) -> String {
        match self {
            PreviewError::ExtractionFailed(_) => {
                "無法從壓縮檔中提取此檔案，檔案可能已損毀。".to_string()
            }
            PreviewError::DecodeFailed(_) => {
                "此檔案格式無法預覽。".to_string()
            }
            PreviewError::MagicByteMismatch { .. } => {
                "此檔案的實際格式與副檔名不符，無法預覽。".to_string()
            }
            PreviewError::FileTooLarge { .. } => {
                "此檔案太大，無法預覽（超過 100 MB）。".to_string()
            }
            PreviewError::ResolutionTooLarge { .. } => {
                "此圖片解析度過高，無法預覽。".to_string()
            }
            PreviewError::DecodeTimeout { .. } => {
                "圖片載入逾時，請稍後再試。".to_string()
            }
            PreviewError::MemoryLimitExceeded { .. } => {
                "圖片解碼所需記憶體超出限制，無法預覽。".to_string()
            }
            PreviewError::PathTraversal(_) => {
                "檔案路徑不合法，操作已被拒絕。".to_string()
            }
            PreviewError::InternalPanic => {
                "發生內部錯誤，請稍後再試。".to_string()
            }
        }
    }

    /// Converts this `PreviewError` into a `LockError` and stores it in the
    /// shared thread-local via `zipease_shared::set_last_error`.
    ///
    /// This allows C# to retrieve the user-facing message through the existing
    /// `zip_ease_get_last_error` FFI function.
    pub fn set_last_error(&self) {
        let msg = self.user_message();
        let lock_error = match self {
            PreviewError::ExtractionFailed(_) => LockError::ExtractionFailed(msg),
            _ => LockError::Unknown(msg),
        };
        zipease_shared::set_last_error(lock_error);
    }
}

impl std::fmt::Display for PreviewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Display uses user_message for safe external representation
        write!(f, "{}", self.user_message())
    }
}

impl std::error::Error for PreviewError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_error_codes_are_negative() {
        let errors: Vec<PreviewError> = vec![
            PreviewError::ExtractionFailed("io error".into()),
            PreviewError::DecodeFailed("bad format".into()),
            PreviewError::MagicByteMismatch {
                expected: "png".into(),
                actual: "jpeg".into(),
            },
            PreviewError::FileTooLarge {
                size_mb: 200,
                limit_mb: 100,
            },
            PreviewError::ResolutionTooLarge {
                width: 20000,
                height: 20000,
            },
            PreviewError::DecodeTimeout { elapsed_secs: 11 },
            PreviewError::MemoryLimitExceeded {
                used_mb: 600,
                limit_mb: 512,
            },
            PreviewError::PathTraversal("../etc/passwd".into()),
            PreviewError::InternalPanic,
        ];

        for error in &errors {
            assert!(
                error.to_error_code() < 0,
                "Error code for {:?} should be negative, got {}",
                error,
                error.to_error_code()
            );
        }
    }

    #[test]
    fn all_error_codes_are_unique() {
        let errors: Vec<PreviewError> = vec![
            PreviewError::ExtractionFailed("".into()),
            PreviewError::DecodeFailed("".into()),
            PreviewError::MagicByteMismatch {
                expected: "".into(),
                actual: "".into(),
            },
            PreviewError::FileTooLarge {
                size_mb: 0,
                limit_mb: 0,
            },
            PreviewError::ResolutionTooLarge {
                width: 0,
                height: 0,
            },
            PreviewError::DecodeTimeout { elapsed_secs: 0 },
            PreviewError::MemoryLimitExceeded {
                used_mb: 0,
                limit_mb: 0,
            },
            PreviewError::PathTraversal("".into()),
            PreviewError::InternalPanic,
        ];

        let codes: Vec<i32> = errors.iter().map(|e| e.to_error_code()).collect();
        let mut unique_codes = codes.clone();
        unique_codes.sort();
        unique_codes.dedup();
        assert_eq!(
            codes.len(),
            unique_codes.len(),
            "Error codes must be unique"
        );
    }

    #[test]
    fn user_messages_contain_no_internal_details() {
        let error = PreviewError::ExtractionFailed("HRESULT 0x80070005".into());
        let msg = error.user_message();
        assert!(!msg.contains("HRESULT"));
        assert!(!msg.contains("0x80070005"));

        let error = PreviewError::DecodeFailed("thread 'main' panicked at...".into());
        let msg = error.user_message();
        assert!(!msg.contains("panicked"));
        assert!(!msg.contains("thread"));

        let error = PreviewError::PathTraversal("../../../etc/passwd".into());
        let msg = error.user_message();
        assert!(!msg.contains("../"));
        assert!(!msg.contains("passwd"));
    }

    #[test]
    fn user_messages_are_non_empty() {
        let errors: Vec<PreviewError> = vec![
            PreviewError::ExtractionFailed("".into()),
            PreviewError::DecodeFailed("".into()),
            PreviewError::MagicByteMismatch {
                expected: "".into(),
                actual: "".into(),
            },
            PreviewError::FileTooLarge {
                size_mb: 0,
                limit_mb: 0,
            },
            PreviewError::ResolutionTooLarge {
                width: 0,
                height: 0,
            },
            PreviewError::DecodeTimeout { elapsed_secs: 0 },
            PreviewError::MemoryLimitExceeded {
                used_mb: 0,
                limit_mb: 0,
            },
            PreviewError::PathTraversal("".into()),
            PreviewError::InternalPanic,
        ];

        for error in &errors {
            assert!(
                !error.user_message().is_empty(),
                "User message for {:?} should not be empty",
                error
            );
        }
    }

    #[test]
    fn set_last_error_stores_message() {
        zipease_shared::clear_last_error();

        let error = PreviewError::FileTooLarge {
            size_mb: 200,
            limit_mb: 100,
        };
        error.set_last_error();

        let last = zipease_shared::get_last_error().expect("should have last error");
        let msg = last.message();
        // The stored message should be the user-friendly Chinese message
        assert!(msg.contains("此檔案太大"));
    }

    #[test]
    fn display_impl_uses_user_message() {
        let error = PreviewError::DecodeTimeout { elapsed_secs: 15 };
        let display = format!("{}", error);
        assert_eq!(display, error.user_message());
    }
}
