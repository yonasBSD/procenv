//! Debug implementation code generation.
//!
//! This module generates a custom `Debug` implementation that masks secret
//! fields to prevent accidental exposure of sensitive values in logs.
//!
//! # Generated Implementation
//!
//! - [`generate_debug_impl`] - Generates `impl Debug for Struct`
//!
//! # Secret Masking
//!
//! Fields marked with `secret` or using secrecy types show `[REDACTED]`:
//!
//! ```rust,ignore
//! #[derive(EnvConfig)]
//! struct Config {
//!     #[env(var = "PORT")]
//!     port: u16,
//!
//!     #[env(var = "API_KEY", secret)]
//!     api_key: String,
//! }
//!
//! // Debug output:
//! // Config { port: 8080, api_key: "[REDACTED]" }
//! ```
//!
//! # Secrecy Types
//!
//! Fields using `SecretString` or `SecretBox<T>` delegate to the type's
//! own `Debug` implementation, which also redacts values:
//!
//! ```rust,ignore
//! #[env(var = "API_KEY")]
//! api_key: SecretString,
//!
//! // Uses SecretString's Debug impl which shows: Secret([REDACTED])
//! ```
//!
//! # Regular Fields
//!
//! Non-secret fields are shown with their actual values using the standard
//! debug formatting.

use proc_macro2::TokenStream as QuoteStream;
use quote::quote;
use syn::{Generics, Ident};

use crate::field::FieldGenerator;

/// Generate a custom `Debug` implementation with secret masking.
pub fn generate_debug_impl(
    struct_name: &Ident,
    generics: &Generics,
    fields: &[Box<dyn FieldGenerator>],
) -> QuoteStream {
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    // Get struct name as string for debug_struct()
    let struct_name_str = struct_name.to_string();

    // Generate .field() calls for each field
    let field_entries: Vec<QuoteStream> = fields
        .iter()
        .map(|f| {
            let name = f.name();
            let name_str = name.to_string();

            if f.is_secrecy_type() {
                // Secrecy types handle their own Debug - just reference the field
                quote! { .field(#name_str, &self.#name) }
            } else if f.is_secret() {
                // Manual secret field - show placeholder
                quote! { .field(#name_str, &"[REDACTED]") }
            } else {
                // Normal field - show actual value
                quote! { .field(#name_str, &self.#name) }
            }
        })
        .collect();

    quote! {
        impl #impl_generics std::fmt::Debug for #struct_name #type_generics #where_clause {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct(#struct_name_str)
                    #(#field_entries)*
                    .finish()
            }
        }
    }
}
