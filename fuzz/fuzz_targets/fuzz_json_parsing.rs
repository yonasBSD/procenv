#![no_main]

use libfuzzer_sys::fuzz_target;
use serde::Deserialize;
use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Deserialize)]
struct FuzzConfig {
    #[serde(default)]
    string_field: Option<String>,
    #[serde(default)]
    int_field: Option<i64>,
    #[serde(default)]
    bool_field: Option<bool>,
    #[serde(default)]
    nested: Option<HashMap<String, String>>,
}

fuzz_target!(|data: &[u8]| {
    // Parsing should return Result, never panic
    let _ = serde_json::from_slice::<FuzzConfig>(data);

    // Also test as generic Value
    let _ = serde_json::from_slice::<serde_json::Value>(data);
});
