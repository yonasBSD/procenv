//! Hot reload support for configuration files.
//!
//! This module provides file watching and automatic configuration reloading.
//! When enabled via the `watch` feature, you can monitor configuration files
//! for changes and automatically reload them.
//!
//! # Features
//!
//! - **File watching** - Monitor config files for changes using the `notify` crate
//! - **Debouncing** - Configurable delay to handle rapid file saves
//! - **Error resilience** - Keep last valid config on reload errors
//! - **Callbacks** - Register handlers for changes and errors
//! - **Thread-safe** - Concurrent access to configuration
//!
//! # Quick Start
//!
//! ```ignore
//! use procenv::{EnvConfig, watch::WatchBuilder};
//! use std::time::Duration;
//!
//! #[derive(EnvConfig, Clone)]
//! #[env_config(file_optional = "config.toml")]
//! struct Config {
//!     #[env(var = "PORT", default = "8080")]
//!     port: u16,
//!
//!     #[env(var = "HOST", default = "localhost")]
//!     host: String,
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Build a file watcher
//!     let handle = WatchBuilder::new()
//!         .watch_file("config.toml")
//!         .debounce(Duration::from_millis(200))
//!         .on_change(|change| {
//!             println!("Config reloaded!");
//!             if change.field_changed("port") {
//!                 println!("Port changed - restart server");
//!             }
//!         })
//!         .on_error(|err| {
//!             eprintln!("Reload failed: {}", err);
//!         })
//!         .build_sync(|| Config::from_config_with_sources())?;
//!
//!     // Access current config
//!     let config = handle.get();
//!     println!("Server starting on {}:{}", config.host, config.port);
//!
//!     // Config updates automatically in the background
//!     // ...
//!
//!     // Graceful shutdown
//!     handle.stop();
//!     Ok(())
//! }
//! ```
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌───────────────┐     ┌─────────────────┐
//! │   notify    │────▶│ ConfigWatcher │────▶│  WatchedConfig  │
//! │  (events)   │     │  (debounce)   │     │ (Arc<RwLock<T>>)│
//! └─────────────┘     └───────────────┘     └─────────────────┘
//!                            │                       │
//!                            ▼                       ▼
//!                     ┌─────────────┐        ┌─────────────┐
//!                     │  Callbacks  │        │ ConfigHandle│
//!                     │ (on_change) │        │  (user API) │
//!                     └─────────────┘        └─────────────┘
//! ```
//!
//! # Error Handling
//!
//! When a reload fails (e.g., due to invalid TOML), the previous valid
//! configuration is retained. The error is passed to the `on_error` callback
//! if one is registered.
//!
//! ```ignore
//! let handle = WatchBuilder::new()
//!     .watch_file("config.toml")
//!     .on_error(|err| {
//!         // Log error, alert monitoring, etc.
//!         eprintln!("Config reload failed: {}", err);
//!         // Previous config is still in use
//!     })
//!     .build_sync(|| Config::from_config_with_sources())?;
//! ```
//!
//! # Change Detection
//!
//! Use the epoch counter for efficient change detection without callbacks:
//!
//! ```ignore
//! let epoch = handle.epoch();
//!
//! // Do some work...
//!
//! if handle.has_changed_since(epoch) {
//!     // Config was reloaded, update state
//!     let new_config = handle.get();
//! }
//! ```

mod builder;
mod container;
mod handle;
mod types;
mod watcher;

// Public API
pub use builder::WatchBuilder;
pub use container::WatchedConfig;
pub use handle::ConfigHandle;
pub use types::{ChangeTrigger, ChangedField, ConfigChange, WatchError};

// Re-export for convenience
pub use watcher::WatchCommand;

#[cfg(test)]
mod tests {
    use super::*;

    // Basic smoke test to ensure module compiles
    #[test]
    fn test_module_exports() {
        // Just verify types exist and are accessible
        let _: fn() -> WatchBuilder<()> = WatchBuilder::new;
    }
}
