//! Integration tests for hot reload functionality.
//!
//! These tests verify the file watching and automatic reload behavior.

#![cfg(feature = "watch")]

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use procenv::{ConfigSources, EnvConfig, WatchBuilder, WatchError};
use tempfile::tempdir;

// ============================================================================
// Test Configuration Types
// ============================================================================

#[derive(EnvConfig, Clone, PartialEq)]
struct SimpleConfig {
    #[env(var = "PORT", default = "8080")]
    port: u16,

    #[env(var = "HOST", default = "localhost")]
    host: String,
}

#[derive(EnvConfig, Clone)]
struct ConfigWithSecret {
    #[env(var = "API_KEY", secret)]
    api_key: String,

    #[env(var = "DEBUG", default = "false")]
    debug: bool,
}

// ============================================================================
// Basic Functionality Tests
// ============================================================================

#[test]
fn test_watch_builder_basic() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");

    // Write initial config
    fs::write(&config_path, "port = 8080\nhost = \"localhost\"").unwrap();

    // Build watcher
    let handle = WatchBuilder::<SimpleConfig>::new()
        .watch_file(&config_path)
        .debounce(Duration::from_millis(50))
        .build_sync(|| {
            Ok((
                SimpleConfig {
                    port: 8080,
                    host: "localhost".to_string(),
                },
                ConfigSources::default(),
            ))
        })
        .expect("should build watcher");

    // Verify initial config
    let config = handle.get();
    assert_eq!(config.port, 8080);
    assert_eq!(config.host, "localhost");

    // Stop watcher
    handle.stop();
    assert!(!handle.is_running());
}

#[test]
fn test_watch_builder_no_files_fails() {
    let result = WatchBuilder::<SimpleConfig>::new()
        .debounce(Duration::from_millis(50))
        .build_sync(|| Ok((SimpleConfig::default(), ConfigSources::default())));

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("no files specified"));
}

#[test]
fn test_watched_config_epoch() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    fs::write(&config_path, "port = 8080").unwrap();

    let handle = WatchBuilder::<SimpleConfig>::new()
        .watch_file(&config_path)
        .build_sync(|| Ok((SimpleConfig::default(), ConfigSources::default())))
        .unwrap();

    let initial_epoch = handle.epoch();
    assert_eq!(initial_epoch, 0);

    // Epoch should not change without file modification
    thread::sleep(Duration::from_millis(50));
    assert_eq!(handle.epoch(), initial_epoch);

    handle.stop();
}

#[test]
fn test_has_changed_since() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    fs::write(&config_path, "port = 8080").unwrap();

    let handle = WatchBuilder::<SimpleConfig>::new()
        .watch_file(&config_path)
        .build_sync(|| Ok((SimpleConfig::default(), ConfigSources::default())))
        .unwrap();

    let epoch = handle.epoch();
    assert!(!handle.has_changed_since(epoch));

    handle.stop();
}

// ============================================================================
// File Change Detection Tests
// ============================================================================

#[test]
fn test_file_modification_triggers_reload() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    fs::write(&config_path, "port = 8080").unwrap();

    let reload_count = Arc::new(AtomicU32::new(0));
    let reload_count_clone = reload_count.clone();

    let handle = WatchBuilder::<SimpleConfig>::new()
        .watch_file(&config_path)
        .debounce(Duration::from_millis(50))
        .build_sync(move || {
            reload_count_clone.fetch_add(1, Ordering::SeqCst);
            Ok((SimpleConfig::default(), ConfigSources::default()))
        })
        .unwrap();

    // Initial load counts as 1
    assert_eq!(reload_count.load(Ordering::SeqCst), 1);

    // Modify file
    thread::sleep(Duration::from_millis(100));
    fs::write(&config_path, "port = 9090").unwrap();

    // Wait for debounce + processing
    thread::sleep(Duration::from_millis(300));

    // Should have reloaded
    assert!(reload_count.load(Ordering::SeqCst) >= 2);

    handle.stop();
}

#[test]
fn test_callback_on_change() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    fs::write(&config_path, "port = 8080").unwrap();

    let callback_called = Arc::new(AtomicBool::new(false));
    let callback_clone = callback_called.clone();

    let handle = WatchBuilder::<SimpleConfig>::new()
        .watch_file(&config_path)
        .debounce(Duration::from_millis(50))
        .on_change(move |_change| {
            callback_clone.store(true, Ordering::SeqCst);
        })
        .build_sync(|| Ok((SimpleConfig::default(), ConfigSources::default())))
        .unwrap();

    // Modify file
    thread::sleep(Duration::from_millis(100));
    fs::write(&config_path, "port = 9090").unwrap();

    // Wait for callback
    thread::sleep(Duration::from_millis(300));

    assert!(callback_called.load(Ordering::SeqCst));

    handle.stop();
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_error_keeps_old_config() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    fs::write(&config_path, "port = 8080").unwrap();

    let reload_count = Arc::new(AtomicU32::new(0));
    let reload_count_clone = reload_count.clone();

    let handle = WatchBuilder::<SimpleConfig>::new()
        .watch_file(&config_path)
        .debounce(Duration::from_millis(50))
        .build_sync(move || {
            let count = reload_count_clone.fetch_add(1, Ordering::SeqCst);
            if count == 0 {
                // First load succeeds
                Ok((
                    SimpleConfig {
                        port: 8080,
                        host: "localhost".to_string(),
                    },
                    ConfigSources::default(),
                ))
            } else {
                // Subsequent loads fail
                Err(procenv::Error::Missing {
                    var: "TEST".to_string(),
                    help: "Set the TEST environment variable".to_string(),
                })
            }
        })
        .unwrap();

    // Verify initial config
    let initial = handle.get();
    assert_eq!(initial.port, 8080);

    // Trigger reload (should fail)
    thread::sleep(Duration::from_millis(100));
    fs::write(&config_path, "port = 9090").unwrap();
    thread::sleep(Duration::from_millis(300));

    // Config should be unchanged (error keeps old config)
    let current = handle.get();
    assert_eq!(current.port, 8080);

    handle.stop();
}

#[test]
fn test_error_callback() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    fs::write(&config_path, "port = 8080").unwrap();

    let error_received = Arc::new(AtomicBool::new(false));
    let error_clone = error_received.clone();

    let reload_count = Arc::new(AtomicU32::new(0));
    let reload_count_clone = reload_count.clone();

    let handle = WatchBuilder::<SimpleConfig>::new()
        .watch_file(&config_path)
        .debounce(Duration::from_millis(50))
        .on_error(move |_err| {
            error_clone.store(true, Ordering::SeqCst);
        })
        .build_sync(move || {
            let count = reload_count_clone.fetch_add(1, Ordering::SeqCst);
            if count == 0 {
                Ok((SimpleConfig::default(), ConfigSources::default()))
            } else {
                Err(procenv::Error::Missing {
                    var: "TEST".to_string(),
                    help: "Set the TEST environment variable".to_string(),
                })
            }
        })
        .unwrap();

    // Trigger reload
    thread::sleep(Duration::from_millis(100));
    fs::write(&config_path, "port = 9090").unwrap();
    thread::sleep(Duration::from_millis(300));

    // Error callback should have been called
    assert!(error_received.load(Ordering::SeqCst));

    handle.stop();
}

// ============================================================================
// Manual Reload Tests
// ============================================================================

#[test]
fn test_manual_reload() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    fs::write(&config_path, "port = 8080").unwrap();

    let reload_count = Arc::new(AtomicU32::new(0));
    let reload_count_clone = reload_count.clone();

    let handle = WatchBuilder::<SimpleConfig>::new()
        .watch_file(&config_path)
        .build_sync(move || {
            reload_count_clone.fetch_add(1, Ordering::SeqCst);
            Ok((SimpleConfig::default(), ConfigSources::default()))
        })
        .unwrap();

    let initial_count = reload_count.load(Ordering::SeqCst);

    // Manual reload
    handle.reload().unwrap();
    thread::sleep(Duration::from_millis(100));

    assert!(reload_count.load(Ordering::SeqCst) > initial_count);

    handle.stop();
}

#[test]
fn test_reload_after_stop_fails() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    fs::write(&config_path, "port = 8080").unwrap();

    let handle = WatchBuilder::<SimpleConfig>::new()
        .watch_file(&config_path)
        .build_sync(|| Ok((SimpleConfig::default(), ConfigSources::default())))
        .unwrap();

    handle.stop();
    thread::sleep(Duration::from_millis(50));

    let result = handle.reload();
    assert!(result.is_err());
}

// ============================================================================
// Handle Clone Tests
// ============================================================================

#[test]
fn test_handle_clone_shares_state() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    fs::write(&config_path, "port = 8080").unwrap();

    let handle1 = WatchBuilder::<SimpleConfig>::new()
        .watch_file(&config_path)
        .build_sync(|| Ok((SimpleConfig::default(), ConfigSources::default())))
        .unwrap();

    let handle2 = handle1.clone();

    // Both handles should see same epoch
    assert_eq!(handle1.epoch(), handle2.epoch());

    // Stopping one stops both
    handle1.stop();
    assert!(!handle2.is_running());
}

// ============================================================================
// Multiple Files Tests
// ============================================================================

#[test]
fn test_watch_multiple_files() {
    let dir = tempdir().unwrap();
    let config1 = dir.path().join("config.toml");
    let config2 = dir.path().join("local.toml");

    fs::write(&config1, "port = 8080").unwrap();
    fs::write(&config2, "host = \"localhost\"").unwrap();

    let reload_count = Arc::new(AtomicU32::new(0));
    let reload_count_clone = reload_count.clone();

    let handle = WatchBuilder::<SimpleConfig>::new()
        .watch_files([&config1, &config2])
        .debounce(Duration::from_millis(50))
        .build_sync(move || {
            reload_count_clone.fetch_add(1, Ordering::SeqCst);
            Ok((SimpleConfig::default(), ConfigSources::default()))
        })
        .unwrap();

    // Initial load
    assert_eq!(reload_count.load(Ordering::SeqCst), 1);

    // Modify second file
    thread::sleep(Duration::from_millis(100));
    fs::write(&config2, "host = \"0.0.0.0\"").unwrap();
    thread::sleep(Duration::from_millis(300));

    // Should have reloaded
    assert!(reload_count.load(Ordering::SeqCst) >= 2);

    handle.stop();
}

// ============================================================================
// WatchError Tests
// ============================================================================

#[test]
fn test_watch_error_display() {
    let err = WatchError::init_failed("test error", None);
    let msg = err.to_string();
    assert!(msg.contains("test error"));

    let err = WatchError::reload_failed("config invalid", vec![]);
    let msg = err.to_string();
    assert!(msg.contains("config invalid"));

    let err = WatchError::Stopped;
    let msg = err.to_string();
    assert!(msg.contains("stopped"));
}

// ============================================================================
// Default Trait Implementation
// ============================================================================

impl Default for SimpleConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            host: "localhost".to_string(),
        }
    }
}

impl Default for ConfigWithSecret {
    fn default() -> Self {
        Self {
            api_key: "secret".to_string(),
            debug: false,
        }
    }
}
