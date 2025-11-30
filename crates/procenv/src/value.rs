//! Type-erased configuration values for runtime access.
//!
//! The [`ConfigValue`] enum provides a way to work with configuration values
//! without knowing their types at compile time. This enables:
//!
//! - Dynamic key-based access to configuration
//! - Partial loading without instantiating full config structs
//! - Runtime introspection of configuration values
//!
//! # Example
//!
//! ```rust,ignore
//! use procenv::ConfigValue;
//!
//! let value = ConfigValue::Integer(8080);
//! let port: i64 = value.to_i64().unwrap();
//! let port: u16 = value.cast().unwrap();
//! ```

use std::collections::HashMap;

// ============================================================================
// Macros for reducing boilerplate
// ============================================================================

/// Generates `From<T>` implementations for ConfigValue
macro_rules! impl_from_integer {
    ($($t:ty => $variant:ident),+ $(,)?) => {
        $(
            impl From<$t> for ConfigValue {
                fn from(n: $t) -> Self {
                    ConfigValue::$variant(n as _)
                }
            }
        )+
    };
}

/// Generates `to_*` methods that use ToPrimitive
macro_rules! impl_to_primitive {
    ($($method:ident -> $t:ty),+ $(,)?) => {
        $(
            #[doc = concat!("Converts to `", stringify!($t), "` if possible.")]
            pub fn $method(&self) -> Option<$t> {
                match self {
                    ConfigValue::Integer(n) => n.$method(),

                    ConfigValue::UnsignedInteger(n) => n.$method(),

                    ConfigValue::Float(f) => f.$method(),

                    ConfigValue::String(s) => s.parse().ok(),

                    ConfigValue::Boolean(b) => Some(
                        if *b {
                            1 as $t
                        } else {
                            0 as $t
                        }
                    )

                    _ => None,
                }
            }
        )+
    };
}

// ============================================================================
// ConfigValue Enum
// ============================================================================

/// A type-erased configuration value.
///
/// Supports common configuration value types with automatic conversions
/// via the `num-traits` crate.
///
/// # Supported Types
///
/// | Variant | Rust Types |
/// |---------|------------|
/// | `String` | `String`, `&str` |
/// | `Integer` | `i8` - `i64`, `isize` |
/// | `UnsignedInteger` | `u8` - `u64`, `usize` |
/// | `Float` | `f32`, `f64` |
/// | `Boolean` | `bool` |
/// | `List` | `Vec<ConfigValue>` |
/// | `Map` | `HashMap<String, ConfigValue>` |
#[derive(Clone, Debug, PartialEq)]
pub enum ConfigValue {
    /// A string value.
    String(String),

    /// A signed integer (stored as i64).
    Integer(i64),

    /// An unsigned integer (stored as u64).
    UnsignedInteger(u64),

    /// A floating-point value (stored as f64).
    Float(f64),

    /// A boolean value.
    Boolean(bool),

    /// A list of values.
    List(Vec<ConfigValue>),

    /// A map of string keys to values.
    Map(HashMap<String, ConfigValue>),

    /// No value (missing optional).
    None,
}

// ============================================================================
// Constructors
// ============================================================================

impl ConfigValue {
    /// Creates from a string with automatic type inference.
    ///
    /// Inference order: bool -> unsigned int -> signed int -> float -> string
    pub fn from_str_infer(s: &str) -> Self {
        // Boolean
        match s.to_ascii_lowercase().as_str() {
            "true" => return ConfigValue::Boolean(true),

            "false" => return ConfigValue::Boolean(false),

            _ => {}
        }

        // Unsigned integer
        if let Ok(n) = s.parse::<u64>() {
            return ConfigValue::UnsignedInteger(n);
        }

        // Signed integer (negative numbers)
        if let Ok(n) = s.parse::<i64>() {
            return ConfigValue::Integer(n);
        }

        // Float (contains decimal or exponent)
        if (s.contains('.') || s.contains('e') || s.contains('E'))
            && let Ok(f) = s.parse::<f64>()
        {
            return ConfigValue::Float(f);
        }

        // Default: string
        ConfigValue::String(s.to_string())
    }
}
