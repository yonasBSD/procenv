//! Example: Error accumulation and source span diagnostics
//!
//! This demonstrates procenv's unique error handling features:
//! 1. Error accumulation - shows ALL errors at once
//! 2. Source spans - points to exact location in config files
//! 3. Secret redaction - never exposes secret values in errors
//!
//! Run with:
//!   `cargo run --example error_demo --features file-all`

#![allow(dead_code, clippy::result_large_err)]

use procenv::{ConfigBuilder, EnvConfig};

// ============================================================================
// Demo 1: Environment Variable Error Accumulation
// ============================================================================

#[derive(EnvConfig)]
#[env_config(prefix = "DEMO_")]
struct EnvOnlyConfig {
    /// Database connection URL (required)
    #[env(var = "DATABASE_URL")]
    database_url: String,

    /// Server port
    #[env(var = "PORT")]
    port: u16,

    /// API secret key (will be redacted in errors)
    #[env(var = "SECRET_KEY", secret)]
    secret_key: String,

    /// Request timeout in seconds
    #[env(var = "TIMEOUT")]
    timeout: u32,

    /// Max connections
    #[env(var = "MAX_CONN")]
    max_connections: u16,
}

// ============================================================================
// Demo 2: File Config with Source Spans (using ConfigBuilder for serde errors)
// ============================================================================

#[derive(Debug, procenv::serde::Deserialize)]
#[serde(crate = "procenv::serde")]
struct FileConfig {
    name: String,
    port: u16,
    debug: bool,
    #[serde(default)]
    database: DatabaseSection,
}

#[derive(Debug, Default, procenv::serde::Deserialize)]
#[serde(crate = "procenv::serde")]
struct DatabaseSection {
    host: String,
    port: u16,
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║          PROCENV ERROR ACCUMULATION DEMO                         ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    // ========================================================================
    // Demo 1: Multiple missing/invalid environment variables
    // ========================================================================
    println!("┌──────────────────────────────────────────────────────────────────┐");
    println!("│ DEMO 1: Environment Variable Error Accumulation                  │");
    println!("├──────────────────────────────────────────────────────────────────┤");
    println!("│ Setting invalid values to trigger multiple parse errors...       │");
    println!("└──────────────────────────────────────────────────────────────────┘\n");

    // Set some invalid values, leave others missing
    unsafe {
        std::env::set_var("DEMO_PORT", "not_a_port");
        std::env::set_var("DEMO_SECRET_KEY", "invalid_secret");
        std::env::set_var("DEMO_TIMEOUT", "abc");
        // DEMO_DATABASE_URL - missing (required)
        // DEMO_MAX_CONN - missing (required)
    }

    match EnvOnlyConfig::from_env() {
        Ok(_) => println!("Unexpected success!"),
        Err(e) => {
            println!("{:?}\n", miette::Report::from(e));
        }
    }

    // Cleanup
    unsafe {
        std::env::remove_var("DEMO_PORT");
        std::env::remove_var("DEMO_SECRET_KEY");
        std::env::remove_var("DEMO_TIMEOUT");
    }

    // ========================================================================
    // Demo 2: File config errors with source spans
    // ========================================================================
    println!("┌──────────────────────────────────────────────────────────────────┐");
    println!("│ DEMO 2: File Config Errors with Source Spans                     │");
    println!("├──────────────────────────────────────────────────────────────────┤");
    println!("│ Loading config file with type errors - watch the source spans!   │");
    println!("└──────────────────────────────────────────────────────────────────┘\n");

    // Load file with type errors
    let result: Result<FileConfig, _> = ConfigBuilder::new()
        .file("crates/procenv/data/error_demo.toml")
        .build();

    match result {
        Ok(_) => println!("Unexpected success!"),
        Err(e) => {
            println!("{:?}\n", miette::Report::from(e));
        }
    }

    // ========================================================================
    // Demo 3: JSON file errors
    // ========================================================================
    println!("┌──────────────────────────────────────────────────────────────────┐");
    println!("│ DEMO 3: JSON File Error with Source Span                         │");
    println!("└──────────────────────────────────────────────────────────────────┘\n");

    let result: Result<FileConfig, _> = ConfigBuilder::new()
        .file("crates/procenv/tests/fixtures/type_error.json")
        .build();

    match result {
        Ok(_) => println!("Unexpected success!"),
        Err(e) => {
            println!("{:?}\n", miette::Report::from(e));
        }
    }

    // ========================================================================
    // Summary
    // ========================================================================
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║                         KEY FEATURES                             ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    println!("║ ✓ ALL errors shown at once - no fix-one-run-again cycles         ║");
    println!("║ ✓ Source spans point to exact line/column in config files        ║");
    println!("║ ✓ Secret values are NEVER exposed in error messages              ║");
    println!("║ ✓ Clear error codes and help text via miette diagnostics         ║");
    println!("╚══════════════════════════════════════════════════════════════════╝");
}
