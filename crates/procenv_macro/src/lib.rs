//! # procenv_macro
//!
//! Procedural macro implementation for the `procenv` crate.
//!
//! This crate provides the `#[derive(EnvConfig)]` procedural macro that
//! generates code for loading configuration from environment variables.
//! It is a proc-macro crate and can only export procedural macros.
//!
//! **Note:** Users should depend on the `procenv` crate, not this one directly.
//! The `procenv` crate re-exports this macro along with runtime types.
//!
//! # Module Structure
//!
//! - `parse` - Attribute parsing for `#[env(...)]` and `#[env_config(...)]`
//! - `field` - Field type processing and code generation strategies
//! - `expand` - Macro expansion orchestration and code generation
//!
//! # Generated Code
//!
//! The macro generates the following methods on the annotated struct:
//!
//! | Method | Description |
//! |--------|-------------|
//! | `from_env()` | Load from environment variables |
//! | `from_env_with_sources()` | Load with source attribution |
//! | `from_config()` | Load from files + env (requires `file` feature) |
//! | `from_args()` | Load from CLI + env (requires CLI attributes) |
//! | `env_example()` | Generate `.env.example` template |
//!
//! It also generates a custom `Debug` implementation that masks secret fields.

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

// Internal modules - not exposed publicly
mod expand;
mod field;
mod parse;

/// Derive macro for loading configuration from environment variables.
///
/// This macro generates methods that read environment variables and construct
/// the struct, with full error accumulation support. All configuration errors
/// are collected and reported together, rather than failing on the first error.
///
/// # Field Attributes
///
/// Each field must have an `#[env(...)]` attribute:
///
/// | Attribute | Description |
/// |-----------|-------------|
/// | `var = "NAME"` | Environment variable name (required) |
/// | `default = "value"` | Default value if env var is missing |
/// | `optional` | Field is `Option<T>`, becomes `None` if missing |
/// | `secret` | Masks value in Debug output and error messages |
/// | `no_prefix` | Skip struct-level prefix for this field |
/// | `flatten` | Embed a nested config struct |
/// | `format = "json"` | Parse value as JSON/TOML/YAML |
/// | `arg = "name"` | CLI argument name (enables `from_args()`) |
/// | `short = 'n'` | CLI short flag (requires `arg`) |
///
/// # Struct Attributes
///
/// Optional `#[env_config(...)]` attribute on the struct:
///
/// | Attribute | Description |
/// |-----------|-------------|
/// | `prefix = "APP_"` | Prefix all env var names |
/// | `dotenv` | Load `.env` file automatically |
/// | `dotenv = ".env.local"` | Load specific dotenv file |
/// | `file = "config.toml"` | Load required config file |
/// | `file_optional = "..."` | Load optional config file |
/// | `profile_env = "APP_ENV"` | Env var for profile selection |
/// | `profiles = ["dev", "prod"]` | Valid profile names |
///
/// # Profile Attributes
///
/// Per-field profile defaults with `#[profile(...)]`:
///
/// ```ignore
/// #[env(var = "DATABASE_URL")]
/// #[profile(dev = "postgres://localhost/dev", prod = "postgres://prod/app")]
/// database_url: String,
/// ```
///
/// # Basic Example
///
/// ```ignore
/// use procenv::EnvConfig;
///
/// #[derive(EnvConfig)]
/// struct Config {
///     #[env(var = "DATABASE_URL")]
///     db_url: String,
///
///     #[env(var = "PORT", default = "8080")]
///     port: u16,
///
///     #[env(var = "API_KEY", secret)]
///     api_key: String,
///
///     #[env(var = "DEBUG_MODE", optional)]
///     debug: Option<bool>,
/// }
///
/// fn main() -> Result<(), procenv::Error> {
///     let config = Config::from_env()?;
///     println!("Server running on port {}", config.port);
///     Ok(())
/// }
/// ```
///
/// # Advanced Example
///
/// ```ignore
/// use procenv::EnvConfig;
/// use serde::Deserialize;
///
/// #[derive(EnvConfig, Deserialize)]
/// #[env_config(
///     prefix = "APP_",
///     dotenv,
///     file_optional = "config.toml",
///     profile_env = "APP_ENV",
///     profiles = ["dev", "staging", "prod"]
/// )]
/// struct Config {
///     #[env(var = "DATABASE_URL")]
///     #[profile(dev = "postgres://localhost/dev")]
///     db_url: String,
///
///     #[env(var = "PORT", default = "8080", arg = "port", short = 'p')]
///     port: u16,
///
///     #[env(flatten)]
///     database: DatabaseConfig,
/// }
///
/// // Load with source attribution
/// let (config, sources) = Config::from_env_with_sources()?;
/// println!("{}", sources);
///
/// // Or load from files + env with layering
/// let config = Config::from_config()?;
/// ```
///
/// # Generated Methods
///
/// The macro generates:
///
/// - `from_env()` - Load from environment variables
/// - `from_env_with_sources()` - Load with source attribution
/// - `from_config()` - Load from files + env (when files configured)
/// - `from_config_with_sources()` - Layered loading with sources
/// - `from_args()` - Load from CLI + env (when `arg` attributes present)
/// - `env_example()` - Generate `.env.example` template
/// - Custom `Debug` impl with secret masking
#[proc_macro_derive(EnvConfig, attributes(env, env_config, profile))]
pub fn derive_env_config(input: TokenStream) -> TokenStream {
    // Parse the input TokenStream into syn's DeriveInput AST
    // This gives us structured access to the struct definition
    let input = parse_macro_input!(input as DeriveInput);

    // Delegate to the Expander which orchestrates code generation
    // On error, convert to a compile_error!() invocation for better error messages
    expand::Expander::expand(input).unwrap_or_else(|err| err.to_compile_error().into())
}
