#![no_main]

use libfuzzer_sys::fuzz_target;
use procenv::MaybeRedacted;

fuzz_target!(|data: (String, bool)| {
    let (value, is_secret) = data;

    // Construction should never panic
    let redacted = MaybeRedacted::new(&value, is_secret);

    // CRITICAL: Verify structural invariants for secret values
    if is_secret {
        // The secret value must NEVER be stored or accessible
        assert!(redacted.is_redacted(), "Secret not marked as redacted!");
        assert!(redacted.as_str().is_none(), "Secret value accessible via as_str()!");

        // Verify Display/Debug don't panic
        let _ = format!("{:?}", redacted);
        let _ = format!("{}", redacted);
    } else {
        // Non-secret values must be preserved exactly
        assert!(!redacted.is_redacted());
        assert_eq!(redacted.as_str(), Some(value.as_str()));
    }
});
