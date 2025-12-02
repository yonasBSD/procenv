//! File format edge case tests (TOML, JSON, YAML).
//!
//! Tests for file loading, parsing errors, and format-specific edge cases.
//! Each test uses a unique file path to avoid race conditions.

#![allow(clippy::pedantic)]
#![cfg(feature = "file-all")]

use procenv::EnvConfig;
use serde::Deserialize;
use std::fs;

const BASE_DIR: &str = "/tmp/procenv_fmt_tests";

fn ensure_dir() {
    let _ = fs::create_dir_all(BASE_DIR);
}

fn write_file(name: &str, content: &str) {
    ensure_dir();
    let path = format!("{BASE_DIR}/{name}");
    fs::write(&path, content).expect("Failed to write test file");
}

fn cleanup_file(name: &str) {
    let path = format!("{BASE_DIR}/{name}");
    let _ = fs::remove_file(&path);
}

fn cleanup_env(vars: &[&str]) {
    unsafe {
        for k in vars {
            std::env::remove_var(*k);
        }
    }
}

fn with_env<F, R>(vars: &[(&str, &str)], f: F) -> R
where
    F: FnOnce() -> R,
{
    unsafe {
        for (k, v) in vars {
            std::env::set_var(*k, *v);
        }
    }

    let result = f();

    unsafe {
        for (k, _) in vars {
            std::env::remove_var(*k);
        }
    }

    result
}

// ============================================================================
// TOML Format Tests
// ============================================================================

#[test]
fn test_toml_basic_loading() {
    cleanup_env(&["TOML_NAME", "TOML_PORT", "TOML_DEBUG"]);
    cleanup_file("toml_basic.toml");

    let content = r#"
name = "test-app"
port = 3000
debug = true
"#;
    write_file("toml_basic.toml", content);

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "TOML_",
        file_optional = "/tmp/procenv_fmt_tests/toml_basic.toml"
    )]
    struct TomlLoadConfig {
        #[env(var = "NAME")]
        name: String,

        #[env(var = "PORT", default = "8080")]
        port: u16,

        #[env(var = "DEBUG", default = "false")]
        debug: bool,
    }

    let config = TomlLoadConfig::from_config().expect("should load TOML");

    assert_eq!(config.name, "test-app");
    assert_eq!(config.port, 3000);
    assert!(config.debug);

    cleanup_file("toml_basic.toml");
}

// ============================================================================
// JSON Format Tests
// ============================================================================

#[test]
fn test_json_basic_loading() {
    cleanup_env(&["JSON_NAME", "JSON_PORT"]);
    cleanup_file("json_basic.json");

    let content = r#"{"name": "json-app", "port": 4000}"#;
    write_file("json_basic.json", content);

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "JSON_",
        file_optional = "/tmp/procenv_fmt_tests/json_basic.json"
    )]
    struct JsonConfig {
        #[env(var = "NAME")]
        name: String,

        #[env(var = "PORT", default = "8080")]
        port: u16,
    }

    let config = JsonConfig::from_config().expect("should load JSON");

    assert_eq!(config.name, "json-app");
    assert_eq!(config.port, 4000);

    cleanup_file("json_basic.json");
}

#[test]
fn test_json_special_characters() {
    cleanup_env(&["JSONSP_VALUE"]);
    cleanup_file("json_special.json");

    let content = r#"{"value": "line1\nline2\ttab\"quoted\""}"#;
    write_file("json_special.json", content);

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "JSONSP_",
        file_optional = "/tmp/procenv_fmt_tests/json_special.json"
    )]
    struct SpecialJsonConfig {
        #[env(var = "VALUE")]
        value: String,
    }

    let config = SpecialJsonConfig::from_config().expect("should handle special chars");

    assert!(config.value.contains('\n'));
    assert!(config.value.contains('\t'));
    assert!(config.value.contains('"'));

    cleanup_file("json_special.json");
}

// ============================================================================
// YAML Format Tests
// ============================================================================

#[test]
fn test_yaml_basic_loading() {
    cleanup_env(&["YAML_NAME", "YAML_PORT"]);
    cleanup_file("yaml_basic.yaml");

    let content = r"
name: yaml-app
port: 5000
";
    write_file("yaml_basic.yaml", content);

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "YAML_",
        file_optional = "/tmp/procenv_fmt_tests/yaml_basic.yaml"
    )]
    struct YamlConfig {
        #[env(var = "NAME")]
        name: String,

        #[env(var = "PORT", default = "8080")]
        port: u16,
    }

    let config = YamlConfig::from_config().expect("should load YAML");

    assert_eq!(config.name, "yaml-app");
    assert_eq!(config.port, 5000);

    cleanup_file("yaml_basic.yaml");
}

#[test]
fn test_yaml_multiline_strings() {
    cleanup_env(&["YAMLML_DESCRIPTION"]);
    cleanup_file("yaml_multiline.yaml");

    let content = r"
description: |
  This is a multiline
  description that spans
  multiple lines.
";
    write_file("yaml_multiline.yaml", content);

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "YAMLML_",
        file_optional = "/tmp/procenv_fmt_tests/yaml_multiline.yaml"
    )]
    struct MultilineConfig {
        #[env(var = "DESCRIPTION")]
        description: String,
    }

    let config = MultilineConfig::from_config().expect("should handle multiline");

    assert!(config.description.contains("multiline"));
    assert!(config.description.contains('\n'));

    cleanup_file("yaml_multiline.yaml");
}

// ============================================================================
// File Not Found Handling
// ============================================================================

#[test]
fn test_optional_file_missing_uses_defaults() {
    cleanup_env(&["OPTF_NAME", "OPTF_PORT"]);
    cleanup_file("missing_optional.toml");

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "OPTF_",
        file_optional = "/tmp/procenv_fmt_tests/missing_optional.toml"
    )]
    struct OptionalFileConfig {
        #[env(var = "NAME", default = "default-name")]
        name: String,

        #[env(var = "PORT", default = "8080")]
        port: u16,
    }

    let config = OptionalFileConfig::from_config().expect("should use defaults when file missing");

    assert_eq!(config.name, "default-name");
    assert_eq!(config.port, 8080);
}

// ============================================================================
// Environment Override Tests
// ============================================================================

#[test]
fn test_env_overrides_file() {
    cleanup_env(&["OVERRIDE_NAME", "OVERRIDE_PORT"]);
    cleanup_file("override_test.toml");

    let content = r#"
name = "from-file"
port = 3000
"#;
    write_file("override_test.toml", content);

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "OVERRIDE_",
        file_optional = "/tmp/procenv_fmt_tests/override_test.toml"
    )]
    struct OverrideConfig {
        #[env(var = "NAME")]
        name: String,

        #[env(var = "PORT", default = "8080")]
        port: u16,
    }

    with_env(
        &[("OVERRIDE_NAME", "from-env"), ("OVERRIDE_PORT", "9000")],
        || {
            let config = OverrideConfig::from_config().expect("should load with override");

            // Env should override file
            assert_eq!(config.name, "from-env");
            assert_eq!(config.port, 9000);
        },
    );

    cleanup_file("override_test.toml");
}

#[test]
fn test_partial_env_override() {
    cleanup_env(&["PARTIAL_NAME", "PARTIAL_PORT"]);
    cleanup_file("partial_override.toml");

    let content = r#"
name = "from-file"
port = 3000
"#;
    write_file("partial_override.toml", content);

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "PARTIAL_",
        file_optional = "/tmp/procenv_fmt_tests/partial_override.toml"
    )]
    struct PartialConfig {
        #[env(var = "NAME")]
        name: String,

        #[env(var = "PORT", default = "8080")]
        port: u16,
    }

    // Only override name, not port
    with_env(&[("PARTIAL_NAME", "from-env")], || {
        let config = PartialConfig::from_config().expect("should load with partial override");

        assert_eq!(config.name, "from-env"); // from env
        assert_eq!(config.port, 3000); // from file
    });

    cleanup_file("partial_override.toml");
}

// ============================================================================
// Format Detection Tests
// ============================================================================

#[test]
fn test_detects_toml_by_extension() {
    cleanup_env(&["DETECT_NAME"]);
    cleanup_file("detect_fmt.toml");

    let content = r#"name = "toml-format""#;
    write_file("detect_fmt.toml", content);

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "DETECT_",
        file_optional = "/tmp/procenv_fmt_tests/detect_fmt.toml"
    )]
    struct DetectTomlConfig {
        #[env(var = "NAME")]
        name: String,
    }

    let config = DetectTomlConfig::from_config().expect("should detect TOML format");
    assert_eq!(config.name, "toml-format");

    cleanup_file("detect_fmt.toml");
}

#[test]
fn test_detects_json_by_extension() {
    cleanup_env(&["DETECTJ_NAME"]);
    cleanup_file("detect_fmt.json");

    let content = r#"{"name": "json-format"}"#;
    write_file("detect_fmt.json", content);

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "DETECTJ_",
        file_optional = "/tmp/procenv_fmt_tests/detect_fmt.json"
    )]
    struct DetectJsonConfig {
        #[env(var = "NAME")]
        name: String,
    }

    let config = DetectJsonConfig::from_config().expect("should detect JSON format");
    assert_eq!(config.name, "json-format");

    cleanup_file("detect_fmt.json");
}

#[test]
fn test_detects_yaml_by_extension() {
    cleanup_env(&["DETECTY_NAME"]);
    cleanup_file("detect_fmt.yaml");

    let content = "name: yaml-format";
    write_file("detect_fmt.yaml", content);

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "DETECTY_",
        file_optional = "/tmp/procenv_fmt_tests/detect_fmt.yaml"
    )]
    struct DetectYamlConfig {
        #[env(var = "NAME")]
        name: String,
    }

    let config = DetectYamlConfig::from_config().expect("should detect YAML format");
    assert_eq!(config.name, "yaml-format");

    cleanup_file("detect_fmt.yaml");
}

// ============================================================================
// Source Attribution with Files
// ============================================================================

#[test]
fn test_source_attribution_from_file() {
    cleanup_env(&["SRCF_NAME", "SRCF_PORT"]);
    cleanup_file("source_attr.toml");

    let content = r#"name = "from-file""#;
    write_file("source_attr.toml", content);

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "SRCF_",
        file_optional = "/tmp/procenv_fmt_tests/source_attr.toml"
    )]
    struct SourceFileConfig {
        #[env(var = "NAME")]
        name: String,

        #[env(var = "PORT", default = "8080")]
        port: u16,
    }

    let (config, sources) =
        SourceFileConfig::from_config_with_sources().expect("should load with sources");

    assert_eq!(config.name, "from-file");
    assert_eq!(config.port, 8080);

    // Name should be from file
    let name_source = sources.get("name").expect("should have name source");
    assert!(
        matches!(name_source.source, procenv::Source::ConfigFile(_)),
        "name should come from ConfigFile"
    );

    // Port should be from default
    let port_source = sources.get("port").expect("should have port source");
    assert!(
        matches!(port_source.source, procenv::Source::Default),
        "port should come from Default"
    );

    cleanup_file("source_attr.toml");
}

// ============================================================================
// Unicode in File Content
// ============================================================================

#[test]
fn test_unicode_in_toml() {
    cleanup_env(&["UNI_NAME"]);
    cleanup_file("unicode_test.toml");

    let content = r#"name = "日本語アプリ 🚀""#;
    write_file("unicode_test.toml", content);

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "UNI_",
        file_optional = "/tmp/procenv_fmt_tests/unicode_test.toml"
    )]
    struct UnicodeConfig {
        #[env(var = "NAME")]
        name: String,
    }

    let config = UnicodeConfig::from_config().expect("should handle unicode");
    assert_eq!(config.name, "日本語アプリ 🚀");

    cleanup_file("unicode_test.toml");
}

// ============================================================================
// Empty File Handling
// ============================================================================

#[test]
fn test_empty_file_uses_defaults() {
    cleanup_env(&["EMPTY_NAME", "EMPTY_PORT"]);
    cleanup_file("empty_test.toml");

    // Empty TOML file
    write_file("empty_test.toml", "");

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "EMPTY_",
        file_optional = "/tmp/procenv_fmt_tests/empty_test.toml"
    )]
    struct EmptyFileConfig {
        #[env(var = "NAME", default = "default")]
        name: String,

        #[env(var = "PORT", default = "8080")]
        port: u16,
    }

    let config = EmptyFileConfig::from_config().expect("should handle empty file");
    assert_eq!(config.name, "default");
    assert_eq!(config.port, 8080);

    cleanup_file("empty_test.toml");
}

// ============================================================================
// Boolean Values in Files
// ============================================================================

#[test]
fn test_boolean_values_in_toml() {
    cleanup_env(&["BOOL_DEBUG", "BOOL_VERBOSE"]);
    cleanup_file("bool_test.toml");

    let content = r"
debug = true
verbose = false
";
    write_file("bool_test.toml", content);

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "BOOL_",
        file_optional = "/tmp/procenv_fmt_tests/bool_test.toml"
    )]
    struct BoolConfig {
        #[env(var = "DEBUG", default = "false")]
        debug: bool,

        #[env(var = "VERBOSE", default = "true")]
        verbose: bool,
    }

    let config = BoolConfig::from_config().expect("should load booleans");
    assert!(config.debug);
    assert!(!config.verbose);

    cleanup_file("bool_test.toml");
}

// ============================================================================
// Numeric Values in Files
// ============================================================================

#[test]
fn test_numeric_values_in_json() {
    cleanup_env(&["NUM_PORT", "NUM_TIMEOUT", "NUM_RATIO"]);
    cleanup_file("numbers_test.json");

    let content = r#"{"port": 8080, "timeout": 30, "ratio": 1.5}"#;
    write_file("numbers_test.json", content);

    #[derive(EnvConfig, Deserialize)]
    #[env_config(
        prefix = "NUM_",
        file_optional = "/tmp/procenv_fmt_tests/numbers_test.json"
    )]
    struct NumericConfig {
        #[env(var = "PORT")]
        port: u16,

        #[env(var = "TIMEOUT")]
        timeout: u32,

        #[env(var = "RATIO")]
        ratio: f64,
    }

    let config = NumericConfig::from_config().expect("should load numbers");
    assert_eq!(config.port, 8080);
    assert_eq!(config.timeout, 30);
    assert!((config.ratio - 1.5).abs() < 0.001);

    cleanup_file("numbers_test.json");
}
