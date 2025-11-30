#![no_main]

use libfuzzer_sys::fuzz_target;
use procenv::file::{FileFormat, FileUtils};

fuzz_target!(|data: &[u8]| {
    // Convert bytes to string (lossy to handle invalid UTF-8)
    let content = String::from_utf8_lossy(data);

    // === Test parse_str with TOML format - should never panic ===
    // It's OK to return errors, but should never crash
    let _ = FileUtils::parse_str(&content, FileFormat::Toml);

    // === Test coerce_value with arbitrary strings - should never panic ===
    let _ = FileUtils::coerce_value(&content);

    // === Test with valid UTF-8 substrings ===
    if let Ok(valid_str) = std::str::from_utf8(data) {
        let _ = FileUtils::parse_str(valid_str, FileFormat::Toml);
    }

    // === Test deep_merge with parsed values - should never panic ===
    // Create two simple values and merge them
    if let Ok(base) = FileUtils::parse_str("{}", FileFormat::Json) {
        let mut base = base;
        if let Ok(overlay) = FileUtils::parse_str(&content, FileFormat::Toml) {
            FileUtils::deep_merge(&mut base, overlay);
        }
    }
});
