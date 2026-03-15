use proptest::prelude::*;
use zipease_core::error::LockError;

proptest! {
    /// Property: Any error can be converted to an error code
    #[test]
    fn property_any_error_has_code(msg in ".*") {
        let err = LockError::Unknown(msg);
        prop_assert!(err.to_error_code() != 0);
    }

    /// Property: Error message contains the provided context
    #[test]
    fn property_error_message_contains_context(context in "[a-zA-Z0-9]+") {
        let err = LockError::InvalidPath(context.clone());
        prop_assert!(err.message().contains(&context));
    }
}
