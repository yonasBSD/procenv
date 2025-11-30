//! Source attribution code generation.

use proc_macro2::TokenStream as QuoteStream;
use quote::quote;
use syn::Ident;

use crate::field::FieldGenerator;
use crate::parse::EnvConfigAttr;

use super::args::generate_from_args_impl;
use super::env::{generate_dotenv_load, generate_field_loader, generate_profile_setup};

/// Generate the from_env_with_sources() implementation.
pub fn generate_from_env_with_sources_impl(
    struct_name: &Ident,
    generators: &[Box<dyn FieldGenerator>],
    env_config: &EnvConfigAttr,
) -> QuoteStream {
    // Collect all env var names for pre-dotenv check
    let env_var_names: Vec<_> = generators.iter().filter_map(|g| g.env_var_name()).collect();

    // Generate pre-dotenv var collection
    let pre_dotenv_collection = quote! {
        let __pre_dotenv_vars: std::collections::HashSet<&str> = [
            #(#env_var_names),*
        ]
        .iter()
        .filter(|var| std::env::var(var).is_ok())
        .copied()
        .collect();
    };

    // Dotenv loading
    let dotenv_load = generate_dotenv_load(&env_config.dotenv);

    // Track if dotenv was loaded
    let dotenv_loaded_flag = if env_config.dotenv.is_some() {
        quote! { let __dotenv_loaded = true; }
    } else {
        quote! { let __dotenv_loaded = false; }
    };

    // Generate profile setup code
    let profile_setup = generate_profile_setup(env_config);

    // Generate loaders
    let loaders: Vec<QuoteStream> = generators
        .iter()
        .map(|g| generate_field_loader(g.as_ref(), env_config))
        .collect();

    // Generate source tracking
    let source_tracking: Vec<QuoteStream> = generators
        .iter()
        .map(|g| g.generate_source_tracking())
        .collect();

    // Generate assignments
    let assignments: Vec<QuoteStream> = generators.iter().map(|g| g.generate_assignment()).collect();

    // Check if any fields have CLI config
    let has_cli_fields = generators.iter().any(|g| g.cli_config().is_some());

    // Generate from_args() if any CLI fields exist
    let from_args_impl = if has_cli_fields {
        generate_from_args_impl(struct_name, generators, env_config)
    } else {
        quote! {}
    };

    quote! {
        #from_args_impl

        impl #struct_name {
            /// Load configuration from environment with source attribution.
            ///
            /// Returns both the config and information about where each value came from.
            pub fn from_env_with_sources() -> std::result::Result<(Self, ::procenv::ConfigSources), ::procenv::Error> {
                #pre_dotenv_collection

                #dotenv_load

                #dotenv_loaded_flag

                // Define external prefix as None for regular from_env calls
                let __external_prefix: std::option::Option<&str> = std::option::Option::None;

                let mut __errors: std::vec::Vec<::procenv::Error> = std::vec::Vec::new();
                let mut __sources = ::procenv::ConfigSources::new();

                // Read and validate profile (if configured)
                #profile_setup

                #(#loaders)*

                #(#source_tracking)*

                if !__errors.is_empty() {
                    return std::result::Result::Err(if __errors.len() == 1 {
                        __errors.pop().unwrap()
                    } else {
                        ::procenv::Error::Multiple { errors: __errors }
                    });
                }

                std::result::Result::Ok((
                    Self {
                        #(#assignments),*
                    },
                    __sources
                ))
            }

            /// Load configuration and return only the source attribution.
            pub fn sources() -> std::result::Result<::procenv::ConfigSources, ::procenv::Error> {
                let (_, sources) = Self::from_env_with_sources()?;
                std::result::Result::Ok(sources)
            }
        }
    }
}
