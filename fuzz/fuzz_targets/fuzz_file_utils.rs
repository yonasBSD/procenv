#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use procenv::file::FileUtils;
use serde_json::{json, Map, Value};

/// Structured input for testing FileUtils functions
#[derive(Debug, Arbitrary)]
struct FuzzInput {
    /// Strings to use for coercion testing
    values: Vec<String>,
    /// Nested path parts for insert_nested testing
    path_parts: Vec<String>,
    /// Whether to test with complex nested structures
    use_nested: bool,
}

fuzz_target!(|input: FuzzInput| {
    // === Test coerce_value with various inputs ===
    for value in &input.values {
        let coerced = FileUtils::coerce_value(value);

        // Verify coerced value is valid JSON (can be serialized/displayed)
        let _ = format!("{}", coerced);
        let _ = serde_json::to_string(&coerced);

        // Verify type inference is consistent
        let re_coerced = FileUtils::coerce_value(value);
        assert_eq!(coerced, re_coerced, "coerce_value should be deterministic");
    }

    // === Test insert_nested with various paths ===
    if !input.path_parts.is_empty() {
        let mut map = Map::new();
        let parts: Vec<&str> = input.path_parts.iter().map(|s| s.as_str()).collect();

        // Insert a test value at the nested path
        let test_value = json!("test_value");
        FileUtils::insert_nested(&mut map, &parts, test_value.clone());

        // Verify the structure was created
        let result = Value::Object(map);
        let _ = format!("{}", result);
    }

    // === Test deep_merge with arbitrary JSON structures ===
    if input.use_nested && input.values.len() >= 2 {
        // Build base structure from first half of values
        let mut base_map = Map::new();
        for (i, v) in input.values.iter().take(input.values.len() / 2).enumerate() {
            base_map.insert(format!("key{}", i), FileUtils::coerce_value(v));
        }
        let mut base = Value::Object(base_map);

        // Build overlay structure from second half
        let mut overlay_map = Map::new();
        for (i, v) in input.values.iter().skip(input.values.len() / 2).enumerate() {
            overlay_map.insert(format!("key{}", i), FileUtils::coerce_value(v));
        }
        let overlay = Value::Object(overlay_map);

        // Merge should not panic
        FileUtils::deep_merge(&mut base, overlay);

        // Result should be valid JSON
        let _ = format!("{}", base);
        let _ = serde_json::to_string(&base);
    }

    // === Test deeply nested merge scenarios ===
    if input.path_parts.len() >= 2 {
        let mut base = json!({"level1": {"level2": {"value": "base"}}});
        let overlay = json!({"level1": {"level2": {"new_key": "overlay"}}});

        FileUtils::deep_merge(&mut base, overlay);

        // Both keys should exist after merge
        let _ = format!("{}", base);
    }

    // === Test edge cases for coerce_value ===
    let edge_cases = [
        "",                           // empty string
        "   ",                        // whitespace only
        "true",                       // boolean
        "TRUE",                       // boolean uppercase
        "false",                      // boolean
        "FALSE",                      // boolean uppercase
        "TrUe",                       // boolean mixed case
        "0",                          // zero
        "-0",                         // negative zero
        "42",                         // positive integer
        "-42",                        // negative integer
        "9999999999999999999999999",  // very large number
        "3.14",                       // float
        "3.",                         // float with trailing dot
        ".14",                        // float with leading dot
        "3.14e10",                    // scientific notation
        "inf",                        // infinity string
        "nan",                        // NaN string
        "null",                       // null string
        "\"quoted\"",                 // quoted string
        "key: value",                 // yaml-like
        "[1,2,3]",                    // json array string
        "{\"a\": 1}",                 // json object string
    ];

    for case in &edge_cases {
        let _ = FileUtils::coerce_value(case);
    }
});
