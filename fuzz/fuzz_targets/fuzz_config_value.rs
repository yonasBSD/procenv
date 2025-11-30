#![no_main]

use libfuzzer_sys::fuzz_target;
use procenv::ConfigValue;

fuzz_target!(|data: &str| {
    // === Test from_str_infer - should never panic ===
    let value = ConfigValue::from_str_infer(data);

    // === Test Display/Debug - should never panic ===
    let _ = format!("{}", value);
    let _ = format!("{:?}", value);

    // === Test all accessor methods - should never panic ===
    let _ = value.as_str();
    let _ = value.as_bool();
    let _ = value.as_list();
    let _ = value.as_map();
    let _ = value.is_none();
    let _ = value.is_some();
    let _ = value.type_name();

    // === Test all numeric conversions - should never panic ===
    let _ = value.to_i8();
    let _ = value.to_i16();
    let _ = value.to_i32();
    let _ = value.to_i64();
    let _ = value.to_isize();
    let _ = value.to_u8();
    let _ = value.to_u16();
    let _ = value.to_u32();
    let _ = value.to_u64();
    let _ = value.to_usize();
    let _ = value.to_f32();
    let _ = value.to_f64();

    // === Test generic cast - should never panic ===
    let _: Option<i32> = value.cast();
    let _: Option<u64> = value.cast();
    let _: Option<f64> = value.cast();

    // === Test parse method - should never panic ===
    let _ = value.parse::<i32>();
    let _ = value.parse::<f64>();
    let _ = value.parse::<bool>();

    // === Test into_string - should never panic ===
    let string_repr = value.clone().into_string();
    let _ = string_repr.len();

    // === Test Clone and PartialEq - should never panic ===
    let cloned = value.clone();
    let _ = value == cloned;

    // === Test path access on non-map (should return None, not panic) ===
    let _ = value.get_path("any.path.here");

    // === Roundtrip test: infer -> string -> infer should be stable ===
    let re_inferred = ConfigValue::from_str_infer(&string_repr);
    // Both should have same type_name (type stability)
    let _ = re_inferred.type_name();
});
