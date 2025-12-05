//! Builder for configuring hot reload behavior.
//!
//! The [`WatchBuilder`] provides a fluent API for setting up file watching
//! with customizable debouncing, callbacks, and error handling.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use super::handle::ConfigHandle;
use super::types::{ConfigChange, WatchError};
use super::watcher::{ConfigWatcher, WatcherConfig};
use crate::{ConfigSources, Error};

/// Callback type for configuration changes.
pub type ChangeCallback<T> = Box<dyn Fn(ConfigChange<T>) + Send + Sync + 'static>;

/// Callback type for reload errors.
pub type ErrorCallback = Box<dyn Fn(WatchError) + Send + Sync + 'static>;

/// Builder for configuring hot reload behavior.
///
/// `WatchBuilder` provides a fluent API for setting up file watching with
/// customizable options including:
/// - Files to watch
/// - Debounce duration
/// - Change callbacks
/// - Error callbacks
///
/// # Example
///
/// ```ignore
/// let handle = WatchBuilder::new()
///     .watch_file("config.toml")
///     .watch_file("config.local.toml")
///     .debounce(Duration::from_millis(200))
///     .on_change(|change| {
///         println!("Config reloaded: {} fields changed", change.changed_fields.len());
///     })
///     .on_error(|err| {
///         eprintln!("Reload failed: {}", err);
///     })
///     .build_sync(|| Config::from_config_with_sources())?;
/// ```
pub struct WatchBuilder<T: Clone + Send + Sync + 'static> {
    /// Files to watch.
    files: Vec<PathBuf>,

    /// Debounce duration (default: 100ms).
    debounce: Duration,

    /// Callback for configuration changes.
    on_change: Option<ChangeCallback<T>>,

    /// Callback for errors.
    on_error: Option<ErrorCallback>,
}

impl<T: Clone + Send + Sync + 'static> WatchBuilder<T> {
    /// Create a new watch builder with default settings.
    ///
    /// Default settings:
    /// - No files watched
    /// - 100ms debounce
    /// - No callbacks
    #[must_use]
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            debounce: Duration::from_millis(100),
            on_change: None,
            on_error: None,
        }
    }

    /// Add a file to watch.
    ///
    /// The file will be monitored for changes. When modified, the configuration
    /// will be reloaded after the debounce period.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file to watch
    ///
    /// # Example
    ///
    /// ```ignore
    /// WatchBuilder::new()
    ///     .watch_file("config.toml")
    ///     .watch_file("secrets.toml")
    /// ```
    #[must_use]
    pub fn watch_file(mut self, path: impl AsRef<Path>) -> Self {
        self.files.push(path.as_ref().to_path_buf());
        self
    }

    /// Add multiple files to watch.
    ///
    /// # Example
    ///
    /// ```ignore
    /// WatchBuilder::new()
    ///     .watch_files(["config.toml", "local.toml", "secrets.toml"])
    /// ```
    #[must_use]
    pub fn watch_files<P: AsRef<Path>>(mut self, paths: impl IntoIterator<Item = P>) -> Self {
        self.files
            .extend(paths.into_iter().map(|p| p.as_ref().to_path_buf()));
        self
    }

    /// Set the debounce duration.
    ///
    /// File system events are often emitted multiple times for a single save
    /// operation. The debounce duration controls how long to wait after the
    /// last event before triggering a reload.
    ///
    /// # Arguments
    ///
    /// * `duration` - How long to wait after file events (default: 100ms)
    ///
    /// # Example
    ///
    /// ```ignore
    /// WatchBuilder::new()
    ///     .debounce(Duration::from_millis(200))
    /// ```
    #[must_use]
    pub const fn debounce(mut self, duration: Duration) -> Self {
        self.debounce = duration;
        self
    }

    /// Register a callback for configuration changes.
    ///
    /// The callback is invoked after each successful reload with a
    /// [`ConfigChange`] containing the old and new configurations.
    ///
    /// # Arguments
    ///
    /// * `callback` - Function to call on configuration changes
    ///
    /// # Example
    ///
    /// ```ignore
    /// WatchBuilder::new()
    ///     .on_change(|change| {
    ///         if change.field_changed("port") {
    ///             println!("Port changed, restarting server...");
    ///         }
    ///     })
    /// ```
    #[must_use]
    pub fn on_change<F>(mut self, callback: F) -> Self
    where
        F: Fn(ConfigChange<T>) + Send + Sync + 'static,
    {
        self.on_change = Some(Box::new(callback));
        self
    }

    /// Register a callback for reload errors.
    ///
    /// When a reload fails (e.g., due to invalid configuration), the error
    /// callback is invoked. The previous valid configuration is retained.
    ///
    /// # Arguments
    ///
    /// * `callback` - Function to call on reload errors
    ///
    /// # Example
    ///
    /// ```ignore
    /// WatchBuilder::new()
    ///     .on_error(|err| {
    ///         eprintln!("Config reload failed: {}", err);
    ///         // Log to monitoring system, etc.
    ///     })
    /// ```
    #[must_use]
    pub fn on_error<F>(mut self, callback: F) -> Self
    where
        F: Fn(WatchError) + Send + Sync + 'static,
    {
        self.on_error = Some(Box::new(callback));
        self
    }

    /// Build and start the file watcher (synchronous).
    ///
    /// This spawns a background thread to watch for file changes. The returned
    /// [`ConfigHandle`] can be used to access the current configuration and
    /// control the watcher.
    ///
    /// # Arguments
    ///
    /// * `reload_fn` - Function that loads the configuration from all sources
    ///
    /// # Returns
    ///
    /// A [`ConfigHandle`] for accessing and controlling the watched configuration.
    ///
    /// # Errors
    ///
    /// Returns [`WatchError`] if:
    /// - No files were specified to watch
    /// - Failed to initialize the file watcher
    /// - Initial configuration load failed
    ///
    /// # Example
    ///
    /// ```ignore
    /// let handle = WatchBuilder::new()
    ///     .watch_file("config.toml")
    ///     .build_sync(|| Config::from_config_with_sources())?;
    ///
    /// // Access current config
    /// let config = handle.get();
    /// println!("Port: {}", config.port);
    /// ```
    pub fn build_sync<F>(self, reload_fn: F) -> Result<ConfigHandle<T>, WatchError>
    where
        F: Fn() -> Result<(T, ConfigSources), Error> + Send + Sync + 'static,
    {
        if self.files.is_empty() {
            return Err(WatchError::init_failed("no files specified to watch", None));
        }

        // Perform initial load
        let (initial_config, initial_sources) = reload_fn()
            .map_err(|e| WatchError::reload_failed("initial configuration load failed", vec![e]))?;

        let watcher_config = WatcherConfig {
            debounce: self.debounce,
            paths: self.files,
        };

        let watcher =
            ConfigWatcher::start(initial_config, initial_sources, &watcher_config, reload_fn)?;

        Ok(ConfigHandle::new(watcher, self.on_change, self.on_error))
    }
}

impl<T: Clone + Send + Sync + 'static> Default for WatchBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Default)]
    struct TestConfig {
        port: u16,
    }

    #[test]
    fn test_builder_defaults() {
        let builder: WatchBuilder<TestConfig> = WatchBuilder::new();
        assert!(builder.files.is_empty());
        assert_eq!(builder.debounce, Duration::from_millis(100));
        assert!(builder.on_change.is_none());
        assert!(builder.on_error.is_none());
    }

    #[test]
    fn test_builder_fluent_api() {
        let builder: WatchBuilder<TestConfig> = WatchBuilder::new()
            .watch_file("config.toml")
            .watch_file("local.toml")
            .debounce(Duration::from_millis(200));

        assert_eq!(builder.files.len(), 2);
        assert_eq!(builder.debounce, Duration::from_millis(200));
    }

    #[test]
    fn test_watch_files() {
        let builder: WatchBuilder<TestConfig> =
            WatchBuilder::new().watch_files(["a.toml", "b.toml", "c.toml"]);

        assert_eq!(builder.files.len(), 3);
    }

    #[test]
    fn test_build_without_files_fails() {
        let result: Result<ConfigHandle<TestConfig>, _> = WatchBuilder::new()
            .build_sync(|| Ok((TestConfig::default(), ConfigSources::default())));

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("no files specified"));
    }
}
