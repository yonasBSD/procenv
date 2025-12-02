//! Compile-time tests for `EnvConfig` derive macro.
//!
//! These tests verify that:
//! - Valid derive usage compiles successfully
//! - Invalid derive usage produces helpful error messages
//!
//! Run with: cargo nextest run --package procenv trybuild

/// Core `compile_pass` tests that work with default features (dotenv only)
#[test]
fn compile_pass() {
    let t = trybuild::TestCases::new();
    // Only run tests that don't require optional features
    t.pass("tests/compile_pass/basic_required.rs");
    t.pass("tests/compile_pass/with_default.rs");
    t.pass("tests/compile_pass/optional_field.rs");
    t.pass("tests/compile_pass/secret_field.rs");
    t.pass("tests/compile_pass/all_features.rs");
    t.pass("tests/compile_pass/source_attribution.rs");
    t.pass("tests/compile_pass/source_attribution_nested.rs");
    t.pass("tests/compile_pass/prefix_support.rs");
    t.pass("tests/compile_pass/env_example_gen.rs");
    t.pass("tests/compile_pass/various_types.rs");
}

/// Tests requiring clap feature
#[test]
#[cfg(feature = "clap")]
fn compile_pass_clap() {
    let t = trybuild::TestCases::new();
    t.pass("tests/compile_pass/cli_args.rs");
}

/// Tests requiring serde/file features
#[test]
#[cfg(feature = "serde")]
fn compile_pass_serde() {
    let t = trybuild::TestCases::new();
    t.pass("tests/compile_pass/serde_format.rs");
    t.pass("tests/compile_pass/serde_format_semantics.rs");
}

/// Tests requiring file features for profiles
#[test]
#[cfg(feature = "file")]
fn compile_pass_profiles() {
    let t = trybuild::TestCases::new();
    t.pass("tests/compile_pass/profile_support.rs");
}

#[test]
fn compile_fail() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
