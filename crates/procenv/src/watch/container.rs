//! Thread-safe configuration container.
//!
//! This module provides [`WatchedConfig`], a thread-safe container for
//! configuration that supports atomic updates for hot reload scenarios.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;

use crate::ConfigSources;

/// Thread-safe container for watched configuration.
///
/// `WatchedConfig` provides efficient concurrent read access with atomic
/// updates for hot reload. It uses [`parking_lot::RwLock`] for better
/// performance compared to `std::sync::RwLock`.
///
/// # Thread Safety
///
/// - Multiple threads can read the configuration concurrently
/// - Updates are atomic - readers never see partial updates
/// - The epoch counter allows efficient change detection
///
/// # Example
///
/// ```ignore
/// let config = WatchedConfig::new(MyConfig::default(), ConfigSources::default());
///
/// // Read current config (multiple threads can do this concurrently)
/// let current = config.get();
///
/// // Read via closure (avoids Arc clone for read-only access)
/// config.read(|cfg| println!("Port: {}", cfg.port));
///
/// // Check if changed since last read
/// let epoch = config.epoch();
/// // ... later ...
/// if config.epoch() != epoch {
///     println!("Config changed!");
/// }
/// ```
pub struct WatchedConfig<T> {
    /// Current configuration wrapped in Arc for cheap cloning.
    inner: RwLock<Arc<T>>,

    /// Source attribution for current configuration.
    sources: RwLock<ConfigSources>,

    /// Epoch counter - incremented on each update.
    /// Used for efficient change detection without comparing configs.
    epoch: AtomicU64,
}

impl<T> WatchedConfig<T> {
    /// Create a new watched configuration container.
    ///
    /// # Arguments
    ///
    /// * `config` - The initial configuration
    /// * `sources` - Source attribution for the initial configuration
    pub fn new(config: T, sources: ConfigSources) -> Self {
        Self {
            inner: RwLock::new(Arc::new(config)),
            sources: RwLock::new(sources),
            epoch: AtomicU64::new(0),
        }
    }

    /// Get a clone of the current configuration.
    ///
    /// This returns an `Arc<T>`, which is cheap to clone. Multiple calls
    /// to `get()` return the same `Arc` until the configuration is updated.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let config = watched.get();
    /// println!("Port: {}", config.port);
    /// ```
    pub fn get(&self) -> Arc<T> {
        self.inner.read().clone()
    }

    /// Read the current configuration via a closure.
    ///
    /// This is more efficient than `get()` when you don't need to keep
    /// a reference to the configuration, as it avoids the `Arc` clone.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let port = watched.read(|cfg| cfg.port);
    /// ```
    pub fn read<R, F>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        let guard = self.inner.read();
        f(&**guard)
    }

    /// Get the current epoch.
    ///
    /// The epoch is incremented each time the configuration is updated.
    /// This provides an efficient way to detect changes without comparing
    /// the actual configuration values.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let epoch = watched.epoch();
    /// // ... do some work ...
    /// if watched.epoch() != epoch {
    ///     println!("Config changed while we were working!");
    /// }
    /// ```
    pub fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    /// Get a clone of the current source attribution.
    pub fn sources(&self) -> ConfigSources {
        self.sources.read().clone()
    }

    /// Atomically swap in a new configuration.
    ///
    /// This method:
    /// 1. Acquires the write lock
    /// 2. Swaps in the new configuration
    /// 3. Increments the epoch counter
    /// 4. Returns the old configuration and sources
    ///
    /// Readers are never blocked for long - they either see the old config
    /// or the new config, never a partial state.
    ///
    /// # Arguments
    ///
    /// * `new_config` - The new configuration to install
    /// * `new_sources` - Source attribution for the new configuration
    ///
    /// # Returns
    ///
    /// A tuple of (old_config, old_sources)
    pub(crate) fn swap(
        &self,
        new_config: Arc<T>,
        new_sources: ConfigSources,
    ) -> (Arc<T>, ConfigSources) {
        // Swap config
        let old_config = {
            let mut guard = self.inner.write();
            std::mem::replace(&mut *guard, new_config)
        };

        // Swap sources
        let old_sources = {
            let mut guard = self.sources.write();
            std::mem::replace(&mut *guard, new_sources)
        };

        // Increment epoch after both swaps complete
        self.epoch.fetch_add(1, Ordering::Release);

        (old_config, old_sources)
    }

    /// Update the configuration using a function.
    ///
    /// This is useful when you need to compute the new configuration
    /// based on the current one.
    ///
    /// # Arguments
    ///
    /// * `f` - Function that takes the current config and returns new (config, sources)
    ///
    /// # Returns
    ///
    /// A tuple of (old_config, old_sources)
    pub(crate) fn update<F>(&self, f: F) -> (Arc<T>, ConfigSources)
    where
        F: FnOnce(&T, &ConfigSources) -> (T, ConfigSources),
    {
        let (new_config, new_sources) = {
            let config_guard = self.inner.read();
            let sources_guard = self.sources.read();
            f(&**config_guard, &sources_guard)
        };
        self.swap(Arc::new(new_config), new_sources)
    }
}

impl<T: Clone> WatchedConfig<T> {
    /// Get a clone of the current configuration value.
    ///
    /// Unlike `get()` which returns `Arc<T>`, this returns `T` directly.
    /// Use this when you need an owned copy of the configuration.
    pub fn clone_inner(&self) -> T {
        (*self.inner.read()).as_ref().clone()
    }
}

impl<T: Default> Default for WatchedConfig<T> {
    fn default() -> Self {
        Self::new(T::default(), ConfigSources::default())
    }
}

// Manual Debug impl to avoid requiring T: Debug
impl<T> std::fmt::Debug for WatchedConfig<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WatchedConfig")
            .field("epoch", &self.epoch())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Default, PartialEq, Debug)]
    struct TestConfig {
        port: u16,
        host: String,
    }

    #[test]
    fn test_basic_operations() {
        let config = TestConfig {
            port: 8080,
            host: "localhost".to_string(),
        };
        let watched = WatchedConfig::new(config.clone(), ConfigSources::default());

        // Get should return the config
        let got = watched.get();
        assert_eq!(got.port, 8080);
        assert_eq!(got.host, "localhost");

        // Read should work
        let port = watched.read(|c| c.port);
        assert_eq!(port, 8080);

        // Initial epoch should be 0
        assert_eq!(watched.epoch(), 0);
    }

    #[test]
    fn test_swap() {
        let watched = WatchedConfig::new(
            TestConfig {
                port: 8080,
                host: "localhost".to_string(),
            },
            ConfigSources::default(),
        );

        let initial_epoch = watched.epoch();

        // Swap in new config
        let new_config = Arc::new(TestConfig {
            port: 9090,
            host: "0.0.0.0".to_string(),
        });
        let (old, _) = watched.swap(new_config, ConfigSources::default());

        // Old config should be returned
        assert_eq!(old.port, 8080);

        // New config should be in place
        assert_eq!(watched.get().port, 9090);

        // Epoch should have incremented
        assert_eq!(watched.epoch(), initial_epoch + 1);
    }

    #[test]
    fn test_concurrent_reads() {
        use std::thread;

        let watched = Arc::new(WatchedConfig::new(
            TestConfig {
                port: 8080,
                host: "localhost".to_string(),
            },
            ConfigSources::default(),
        ));

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let watched = watched.clone();
                thread::spawn(move || {
                    for _ in 0..1000 {
                        let _ = watched.get();
                        let _ = watched.read(|c| c.port);
                        let _ = watched.epoch();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }
    }

    #[test]
    fn test_clone_inner() {
        let config = TestConfig {
            port: 8080,
            host: "localhost".to_string(),
        };
        let watched = WatchedConfig::new(config.clone(), ConfigSources::default());

        let cloned = watched.clone_inner();
        assert_eq!(cloned, config);
    }
}
