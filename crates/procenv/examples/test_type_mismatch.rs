//! Test example for type mismatch error spans.
//!
//! This example verifies that type mismatch errors in config files
//! show proper source spans pointing to the exact location of the error.
//!
//! Run with:
//!   cargo run --example test_type_mismatch --features file-all

use procenv::ConfigBuilder;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SimpleConfig {
    name: String,
    port: u16,
    debug: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DatabaseConfig {
    host: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct NestedConfig {
    name: String,
    port: u16,
    database: DatabaseConfig,
}

const FIXTURES_DIR: &str = "crates/procenv/tests/fixtures";

fn main() {
    println!("=== Type Mismatch Error Span Tests ===\n");

    // Test 1: Valid config (should succeed)
    println!("--- Test 1: Valid TOML config ---");
    let result: Result<SimpleConfig, _> = ConfigBuilder::new()
        .file(format!("{}/valid.toml", FIXTURES_DIR))
        .build();
    match result {
        Ok(config) => println!("Success: {:?}\n", config),
        Err(e) => println!("Error: {:?}\n", miette::Report::from(e)),
    }

    // Test 2: Type mismatch in TOML (port is string, expected u16)
    println!("--- Test 2: TOML type mismatch (port = \"not_a_number\") ---");
    let result: Result<SimpleConfig, _> = ConfigBuilder::new()
        .file(format!("{}/type_error.toml", FIXTURES_DIR))
        .build();
    match result {
        Ok(config) => println!("Unexpected success: {:?}\n", config),
        Err(e) => {
            println!("Expected error with source span:");
            println!("{:?}\n", miette::Report::from(e));
        }
    }

    // Test 3: Nested type mismatch in TOML
    println!("--- Test 3: Nested TOML type mismatch (database.port = \"invalid_port\") ---");
    let result: Result<NestedConfig, _> = ConfigBuilder::new()
        .file(format!("{}/nested_error.toml", FIXTURES_DIR))
        .build();
    match result {
        Ok(config) => println!("Unexpected success: {:?}\n", config),
        Err(e) => {
            println!("Expected error with source span:");
            println!("{:?}\n", miette::Report::from(e));
        }
    }

    // Test 4: Type mismatch in JSON
    println!("--- Test 4: JSON type mismatch (port = \"still_not_a_number\") ---");
    let result: Result<SimpleConfig, _> = ConfigBuilder::new()
        .file(format!("{}/type_error.json", FIXTURES_DIR))
        .build();
    match result {
        Ok(config) => println!("Unexpected success: {:?}\n", config),
        Err(e) => {
            println!("Expected error with source span:");
            println!("{:?}\n", miette::Report::from(e));
        }
    }

    // Test 5: Missing field
    println!("--- Test 5: Missing required field ---");
    let result: Result<SimpleConfig, _> = ConfigBuilder::new()
        .file(format!("{}/missing_field.json", FIXTURES_DIR))
        .build();
    match result {
        Ok(config) => println!("Unexpected success: {:?}\n", config),
        Err(e) => {
            println!("Expected error:");
            println!("{:?}\n", miette::Report::from(e));
        }
    }

    println!("=== Tests Complete ===");
}
