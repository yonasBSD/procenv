//! Validation error types for the `validator` crate integration.
//!
//! This module provides error types for validation failures, converting
//! from the `validator` crate's error types to procenv's error model.
//!
//! # Overview
//!
//! When `#[env_config(validate)]` is set on a struct that also derives
//! `validator::Validate`, the generated `from_env_validated()` method will:
//!
//! 1. Load all configuration values using `from_env()`
//! 2. Run validator crate validation rules
//! 3. Run any custom field validators specified with `#[env(validate = "fn_name")]`
//! 4. Accumulate all validation errors into [`ValidationFieldError`] instances
//!
//! # Error Structure
//!
//! Each validation failure is represented as a [`ValidationFieldError`] containing:
//! - `field` - The field name that failed validation
//! - `code` - The validation rule code (e.g., "email", "range", "url")
//! - `message` - Human-readable error message
//! - `params` - Optional parameters from the validation rule
//!
//! # Example
//!
//! ```rust,ignore
//! use procenv::EnvConfig;
//! use validator::Validate;
//!
//! #[derive(EnvConfig, Validate)]
//! #[env_config(validate)]
//! struct Config {
//!     #[env(var = "PORT", default = "8080")]
//!     #[validate(range(min = 1, max = 65535))]
//!     port: u16,
//!
//!     #[env(var = "EMAIL")]
//!     #[validate(email)]
//!     admin_email: String,
//! }
//!
//! // Validation runs automatically after loading
//! let config = Config::from_env_validated()?;
//! ```

use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};
use std::string::ToString;

use miette::Diagnostic;

/// A validation error for a specific field.
///
/// This struct represents a single validation failure, including the field
/// name, validation rule code, and human-readable message.
#[derive(Debug, Diagnostic)]
#[diagnostic(code(procenv::field_validation_error))]
pub struct ValidationFieldError {
    /// The field name that failed validation.
    pub field: String,

    /// The validation rule that failed (e.g., "email", "range", "url").
    pub code: String,

    /// Human-readable error message.
    #[help]
    pub message: String,

    /// Additional parameters from the validation rule (e.g., min/max values).
    pub params: Option<String>,
}

impl ValidationFieldError {
    /// Create a new validation field error.
    pub fn new(
        field: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            field: field.into(),
            code: code.into(),
            message: message.into(),
            params: None,
        }
    }

    /// Add parameters to the error (e.g., "min: 1, max: 100").
    #[must_use]
    pub fn with_params(mut self, params: impl Into<String>) -> Self {
        self.params = Some(params.into());

        self
    }

    /// Extract the human-readable message from a validation error.
    ///
    /// Returns the custom message if set, otherwise generates a default
    /// message using the validation code.
    fn extract_message(error: &::validator::ValidationError) -> String {
        error.message.as_ref().map_or_else(
            || format!("validation failed: {}", error.code),
            ToString::to_string,
        )
    }

    /// Extract validation parameters as a formatted string.
    ///
    /// Filters out the "value" parameter (which contains the actual value)
    /// and formats remaining parameters as "key: value" pairs.
    fn extract_params(error: &::validator::ValidationError) -> Option<String> {
        if error.params.is_empty() {
            return None;
        }

        let param_strs: Vec<String> = error
            .params
            .iter()
            .filter(|(k, _)| *k != "value")
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();

        if param_strs.is_empty() {
            None
        } else {
            Some(param_strs.join(", "))
        }
    }

    /// Create a `ValidationFieldError` from a validator error.
    fn from_validator_error(field: &str, error: &::validator::ValidationError) -> Self {
        let message = Self::extract_message(error);
        let params = Self::extract_params(error);

        let mut err = Self::new(field.to_string(), error.code.to_string(), message);

        if let Some(p) = params {
            err = err.with_params(p);
        }

        err
    }

    /// Convert validator crate errors to our error type.
    #[must_use]
    pub fn validation_errors_to_procenv(errors: &::validator::ValidationErrors) -> Vec<Self> {
        // Collect flat field errors
        let flat_errors = errors
            .field_errors()
            .into_iter()
            .flat_map(|(field, field_errors)| {
                field_errors
                    .iter()
                    .map(move |error| Self::from_validator_error(&field, error))
            });

        // Collect nested struct errors with prefixed field paths
        let nested_errors = errors.errors().iter().filter_map(|(field, nested)| {
            if let ::validator::ValidationErrorsKind::Struct(nested) = nested {
                let nested_field_errors = Self::validation_errors_to_procenv(&nested.clone());

                Some(nested_field_errors.into_iter().map(move |mut err| {
                    err.field = format!("{}.{}", field, err.field);
                    err
                }))
            } else {
                None
            }
        });

        flat_errors.chain(nested_errors.flatten()).collect()
    }
}

impl Display for ValidationFieldError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "field `{}` failed validation: {}",
            self.field, self.message
        )
    }
}

impl StdError for ValidationFieldError {}

/// Standalone function to convert validator crate errors to procenv errors.
///
/// This is a convenience wrapper around [`ValidationFieldError::validation_errors_to_procenv`].
/// It's exported at the crate root for use in generated code.
#[must_use]
pub fn validation_errors_to_procenv(
    errors: &::validator::ValidationErrors,
) -> Vec<ValidationFieldError> {
    ValidationFieldError::validation_errors_to_procenv(errors)
}
