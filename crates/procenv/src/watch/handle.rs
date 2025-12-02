//! User-facing handle for watched configuration.
//!
//! The [`ConfigHandle`] provides the main interface for accessing watched
//! configuration and controlling the file watcher.

use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crossbeam_channel::Receiver;

use super::WatchedConfig;
use super::builder::{ChangeCallback, ErrorCallback};
use super::types::{ConfigChange, WatchError};
use super::watcher::{ConfigWatcher, WatchCommand};
use crate::ConfigSources;

/// Handle for accessing and controlling watched configuration.
///
/// `ConfigHandle` is the main user-facing type for hot reload functionality.
/// It provides:
/// - Access to the current configuration
/// - Manual reload triggering
/// - Graceful shutdown
///
/// The handle is cheaply cloneable - all clones share the same underlying
/// watched configuration.
///
/// # Example
///
/// ```ignore
/// // Get current configuration
/// let config = handle.get();
/// println!("Port: {}", config.port);
///
/// // Check for changes
/// let epoch = handle.epoch();
/// // ... later ...
/// if handle.has_changed_since(epoch) {
///     println!("Config was reloaded!");
/// }
///
/// // Manual reload
/// handle.reload()?;
///
/// // Stop watching
/// handle.stop();
/// ```
pub struct ConfigHandle<T: Clone + Send + Sync + 'static> {
    /// The watcher managing file events.
    watcher: Arc<ConfigWatcher<T>>,

    /// Handle to the callback processor thread.
    callback_thread: Option<Arc<JoinHandle<()>>>,
}

impl<T: Clone + Send + Sync + 'static> ConfigHandle<T> {
    /// Create a new config handle.
    pub(crate) fn new(
        watcher: ConfigWatcher<T>,
        on_change: Option<ChangeCallback<T>>,
        on_error: Option<ErrorCallback>,
    ) -> Self {
        let watcher = Arc::new(watcher);

        // Spawn callback processor thread if callbacks are registered
        let callback_thread = if on_change.is_some() || on_error.is_some() {
            let change_rx = watcher.change_receiver().clone();
            let error_rx = watcher.error_receiver().clone();
            let watcher_clone = watcher.clone();

            let handle = thread::Builder::new()
                .name("procenv-callbacks".to_string())
                .spawn(move || {
                    callback_loop(
                        &change_rx,
                        &error_rx,
                        on_change.as_ref(),
                        on_error.as_ref(),
                        &watcher_clone,
                    );
                })
                .ok();

            handle.map(Arc::new)
        } else {
            None
        };

        Self {
            watcher,
            callback_thread,
        }
    }

    /// Get the current configuration.
    ///
    /// Returns an `Arc<T>` to the current configuration. This is cheap to
    /// clone and can be held across reloads (old configurations remain valid).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = handle.get();
    /// println!("Server running on port {}", config.port);
    /// ```
    #[must_use]
    pub fn get(&self) -> Arc<T> {
        self.watcher.config().get()
    }

    /// Read the current configuration via a closure.
    ///
    /// This is more efficient than `get()` when you don't need to keep
    /// a reference to the configuration.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let port = handle.read(|cfg| cfg.port);
    /// ```
    pub fn read<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        self.watcher.config().read(f)
    }

    /// Get the current source attribution.
    ///
    /// Returns information about where each configuration value came from.
    #[must_use]
    pub fn sources(&self) -> ConfigSources {
        self.watcher.config().sources()
    }

    /// Get the current epoch.
    ///
    /// The epoch increments each time the configuration is reloaded.
    /// Use this for efficient change detection.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let epoch = handle.epoch();
    /// // ... do some work ...
    /// if handle.epoch() != epoch {
    ///     println!("Config changed while working!");
    /// }
    /// ```
    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.watcher.config().epoch()
    }

    /// Check if the configuration has changed since a given epoch.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let epoch = handle.epoch();
    /// // ... later ...
    /// if handle.has_changed_since(epoch) {
    ///     let new_config = handle.get();
    ///     // Handle the change
    /// }
    /// ```
    #[must_use]
    pub fn has_changed_since(&self, epoch: u64) -> bool {
        self.epoch() != epoch
    }

    /// Manually trigger a configuration reload.
    ///
    /// This forces an immediate reload of the configuration from all sources,
    /// bypassing the file watcher.
    ///
    /// # Errors
    ///
    /// Returns [`WatchError::Stopped`] if the watcher has been stopped,
    /// or [`WatchError::ChannelError`] if communication with the watcher failed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Force reload after external changes
    /// handle.reload()?;
    /// ```
    pub fn reload(&self) -> Result<(), WatchError> {
        self.watcher.request_reload()
    }

    /// Stop the file watcher.
    ///
    /// This gracefully shuts down the watcher thread. After calling `stop()`,
    /// the handle can still be used to access the last known configuration,
    /// but no further reloads will occur.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Graceful shutdown
    /// handle.stop();
    /// // Config is still accessible
    /// let final_config = handle.get();
    /// ```
    pub fn stop(&self) {
        self.watcher.stop();
    }

    /// Check if the watcher is still running.
    ///
    /// Returns `false` after `stop()` is called or if the watcher thread
    /// terminated unexpectedly.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.watcher.is_running()
    }

    /// Get a clone of the command sender for advanced use cases.
    ///
    /// This allows sending commands to the watcher from other contexts.
    #[must_use]
    pub fn command_sender(&self) -> crossbeam_channel::Sender<WatchCommand> {
        self.watcher.command_sender()
    }
}

impl<T: Clone + Send + Sync + 'static> Clone for ConfigHandle<T> {
    fn clone(&self) -> Self {
        Self {
            watcher: self.watcher.clone(),
            callback_thread: self.callback_thread.clone(),
        }
    }
}

// Manual Debug impl to avoid T: Debug bound
impl<T: Clone + Send + Sync + 'static> std::fmt::Debug for ConfigHandle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigHandle")
            .field("epoch", &self.epoch())
            .field("running", &self.is_running())
            .finish()
    }
}

/// Callback processing loop.
fn callback_loop<T: Clone + Send + Sync + 'static>(
    change_rx: &Receiver<ConfigChange<T>>,
    error_rx: &Receiver<WatchError>,
    on_change: Option<&ChangeCallback<T>>,
    on_error: Option<&ErrorCallback>,
    watcher: &Arc<ConfigWatcher<T>>,
) {
    use crossbeam_channel::select;

    while watcher.is_running() {
        select! {
            recv(change_rx) -> change => {
                if let Ok(change) = change
                    && let Some(cb) = on_change
                {
                    cb(change);
                }
            }
            recv(error_rx) -> error => {
                if let Ok(error) = error
                    && let Some(cb) = on_error
                {
                    cb(error);
                }
            }
            default(std::time::Duration::from_millis(100)) => {
                // Check if still running
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Clone, Default, Debug)]
    struct TestConfig {
        port: u16,
    }

    #[test]
    fn test_handle_debug() {
        // Just ensure Debug impl compiles
        let _: fn(&ConfigHandle<TestConfig>) -> String = |h| format!("{h:?}");
    }
}
