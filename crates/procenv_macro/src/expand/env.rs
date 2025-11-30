//! Environment variable loading code generation.

use proc_macro2::TokenStream as QuoteStream;
use quote::{format_ident, quote};
use syn::{Generics, Ident};

use crate::field::FieldGenerator;
use crate::parse::{DotenvConfig, EnvConfigAttr};

/// Generate the `from_env()` method implementation.
///
/// This generates code that:
/// 1. Optionally loads .env file(s) if configured
/// 2. Reads and validates profile if configured
/// 3. Creates an error accumulator vector
/// 4. Loads each field (calling each FieldGenerator's `generate_loader()`)
/// 5. If any errors occurred, returns the (single or Multiple variant)
/// 6. Otherwise constructs and returns the struct
pub fn generate_from_env_impl(
    struct_name: &Ident,
    generics: &Generics,
    fields: &[Box<dyn FieldGenerator>],
    env_config_attr: &EnvConfigAttr,
) -> QuoteStream {
    // Split generics for the impl block
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    // Generate loader code for each field
    let loaders: Vec<QuoteStream> = fields
        .iter()
        .map(|f| generate_field_loader(f.as_ref(), env_config_attr))
        .collect();

    // Generate assignment code for each field
    let assignments: Vec<QuoteStream> = fields.iter().map(|f| f.generate_assignment()).collect();

    // Generate dotenv loading code (if configured)
    let dotenv_load = generate_dotenv_load(&env_config_attr.dotenv);

    // Generate profile setup code (if configured)
    let profile_setup = generate_profile_setup(env_config_attr);

    quote! {
        impl #impl_generics #struct_name #type_generics #where_clause {
            /// Load configuration from environment variables.
            ///
            /// This method reads all configured environment variables and
            /// attempts to parse them into their respective types.
            ///
            /// # Errors
            /// Returns an error if any required variables are missing or
            /// if any values fail to parse. All errors are accumulated
            /// and returned together.
            pub fn from_env() -> std::result::Result<Self, ::procenv::Error> {
                // Load .env file(s) if configured (errors are silently ignored)
                #dotenv_load

                // Define external prefix as None for regular from_env calls
                let __external_prefix: std::option::Option<&str> = std::option::Option::None;

                // Accumulator for all errors encountered during loading
                let mut __errors: std::vec::Vec<::procenv::Error> = std::vec::Vec::new();

                // Read and validate profile (if configured)
                #profile_setup

                // Load each field - errors are pushed to __errors
                #(#loaders)*

                // If any errors occurred, return them
                if !__errors.is_empty() {
                    return std::result::Result::Err(if __errors.len() == 1 {
                        __errors.pop().unwrap()
                    } else {
                        ::procenv::Error::Multiple { errors: __errors }
                    });
                }

                // All fields loaded successfully - construct the struct
                std::result::Result::Ok(Self {
                    #(#assignments),*
                })
            }
        }
    }
}

/// Generate code to setup profile from env var and validate it.
pub fn generate_profile_setup(env_config_attr: &EnvConfigAttr) -> QuoteStream {
    let Some(profile_env) = &env_config_attr.profile_env else {
        // No profile configured - just define __profile as None
        return quote! {
            let __profile: std::option::Option<std::string::String> = std::option::Option::None;
        };
    };

    // Generate profile validation if profiles list is provided
    let validation = if let Some(profiles) = &env_config_attr.profiles {
        let profile_strs: Vec<&str> = profiles.iter().map(|s| s.as_str()).collect();
        quote! {
            // Validate profile against allowed list
            if let std::option::Option::Some(ref p) = __profile {
                let valid_profiles: &[&str] = &[#(#profile_strs),*];
                if !valid_profiles.contains(&p.as_str()) {
                    __errors.push(::procenv::Error::invalid_profile(
                        p.clone(),
                        #profile_env,
                        valid_profiles.to_vec(),
                    ));
                }
            }
        }
    } else {
        quote! {}
    };

    quote! {
        // Read profile from environment variable
        let __profile: std::option::Option<std::string::String> = std::env::var(#profile_env).ok();
        #validation
    }
}

/// Generate field loader with profile and format support.
pub fn generate_field_loader(
    field: &dyn FieldGenerator,
    _env_config_attr: &EnvConfigAttr,
) -> QuoteStream {
    // Determine which loader to use based on format
    let base_loader = if let Some(format) = field.format_config() {
        field.generate_format_loader(format)
    } else {
        field.generate_loader()
    };

    // Check if this field has profile-specific values
    let Some(profile_config) = field.profile_config() else {
        return base_loader;
    };

    // Field has profile values - generate profile-aware loader
    let name = field.name();
    let profile_used_ident = format_ident!("__{}_from_profile", name);

    // Generate match arms for each profile
    let match_arms: Vec<QuoteStream> = profile_config
        .values
        .iter()
        .map(|(profile_name, value)| {
            quote! {
                std::option::Option::Some(#profile_name) => std::option::Option::Some(#value),
            }
        })
        .collect();

    // Get env var name and type info for parsing
    let env_var = field.env_var_name().unwrap_or("");
    let ty = field.type_name();
    let secret = field.is_secret();
    let default_value = field.default_value();
    let used_default_ident = format_ident!("__{}_used_default", name);

    // Generate fallback code for when no env var and no profile match
    let no_value_handling = if let Some(default) = default_value {
        quote! {
            #used_default_ident = true;
            (std::option::Option::Some(#default.to_string()), false)
        }
    } else {
        quote! {
            (std::option::Option::None, false)
        }
    };

    // Generate error or None handling for missing value
    let missing_value_handling = if default_value.is_some() {
        quote! { std::option::Option::None }
    } else {
        quote! {
            __errors.push(::procenv::Error::missing(#env_var));
            std::option::Option::None
        }
    };

    quote! {
        // Track if we used the compile-time default
        let mut #used_default_ident = false;

        // Determine profile default value (if profile matches)
        let __profile_default: std::option::Option<&str> = match __profile.as_deref() {
            #(#match_arms)*
            _ => std::option::Option::None,
        };

        // Get value to parse: env var > profile > default
        let (__value_to_parse, #profile_used_ident): (std::option::Option<std::string::String>, bool) =
            match std::env::var(#env_var) {
                std::result::Result::Ok(val) => {
                    (std::option::Option::Some(val), false)
                }
                std::result::Result::Err(std::env::VarError::NotPresent) => {
                    match __profile_default {
                        std::option::Option::Some(profile_val) => {
                            (std::option::Option::Some(profile_val.to_string()), true)
                        }
                        std::option::Option::None => {
                            #no_value_handling
                        }
                    }
                }
                std::result::Result::Err(std::env::VarError::NotUnicode(_)) => {
                    __errors.push(::procenv::Error::InvalidUtf8 { var: #env_var.to_string() });
                    (std::option::Option::None, false)
                }
            };

        // Parse the value
        let #name = match __value_to_parse {
            std::option::Option::Some(val) => {
                match val.parse() {
                    std::result::Result::Ok(v) => std::option::Option::Some(v),
                    std::result::Result::Err(e) => {
                        __errors.push(::procenv::Error::parse(
                            #env_var,
                            if #secret { "[REDACTED]".to_string() } else { val },
                            #secret,
                            #ty,
                            std::boxed::Box::new(e),
                        ));
                        std::option::Option::None
                    }
                }
            }
            std::option::Option::None => {
                #missing_value_handling
            }
        };
    }
}

/// Generate code to load .env file(s) based on configuration.
pub fn generate_dotenv_load(dotenv_config: &Option<DotenvConfig>) -> QuoteStream {
    match dotenv_config {
        None => quote! {},

        Some(DotenvConfig::Default) => {
            quote! {
                let _ = ::dotenvy::dotenv();
            }
        }

        Some(DotenvConfig::Custom(path)) => {
            quote! {
                let _ = ::dotenvy::from_filename(#path);
            }
        }

        Some(DotenvConfig::Multiple(paths)) => {
            let load_calls: Vec<QuoteStream> = paths
                .iter()
                .map(|p| {
                    quote! {
                        let _ = ::dotenvy::from_filename(#p);
                    }
                })
                .collect();

            quote! {
                #(#load_calls)*
            }
        }
    }
}

/// Generate the `__from_env_with_external_prefix` method.
pub fn generate_from_env_with_external_prefix_impl(
    struct_name: &Ident,
    generics: &Generics,
    generators: &[Box<dyn FieldGenerator>],
    env_config: &EnvConfigAttr,
) -> QuoteStream {
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    // Collect all env var names for pre-dotenv check
    let env_var_names: Vec<_> = generators.iter().filter_map(|g| g.env_var_name()).collect();

    // Generate loaders using the prefixed version
    let loaders: Vec<QuoteStream> = generators
        .iter()
        .map(|g| {
            if let Some(format) = g.format_config() {
                g.generate_format_loader(format)
            } else {
                g.generate_loader_with_external_prefix()
            }
        })
        .collect();

    // Generate simplified source tracking
    let source_tracking: Vec<QuoteStream> = generators
        .iter()
        .map(|g| generate_simple_source_tracking(g.as_ref()))
        .collect();

    // Generate assignments
    let assignments: Vec<QuoteStream> = generators.iter().map(|g| g.generate_assignment()).collect();

    // Dotenv loading
    let dotenv_load = generate_dotenv_load(&env_config.dotenv);

    let dotenv_loaded_flag = if env_config.dotenv.is_some() {
        quote! { let __dotenv_loaded = true; }
    } else {
        quote! { let __dotenv_loaded = false; }
    };

    // Profile setup
    let profile_setup = generate_profile_setup(env_config);

    quote! {
        impl #impl_generics #struct_name #type_generics #where_clause {
            /// Load configuration with an external prefix prepended to env var names.
            #[doc(hidden)]
            pub fn __from_env_with_external_prefix(
                __external_prefix: std::option::Option<&str>
            ) -> std::result::Result<(Self, ::procenv::ConfigSources), ::procenv::Error> {
                // Track pre-dotenv env vars
                let __pre_dotenv_vars: std::collections::HashSet<&str> = [
                    #(#env_var_names),*
                ]
                .iter()
                .filter(|var| std::env::var(var).is_ok())
                .copied()
                .collect();

                // Load dotenv
                #dotenv_load
                #dotenv_loaded_flag

                // Error accumulator
                let mut __errors: std::vec::Vec<::procenv::Error> = std::vec::Vec::new();
                let mut __sources = ::procenv::ConfigSources::new();

                // Read and validate profile (if configured)
                #profile_setup

                // Load each field with prefixed env var names
                #(#loaders)*

                // Track sources
                #(#source_tracking)*

                // Check for errors
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
        }
    }
}

/// Generate simplified source tracking for __from_env_with_external_prefix.
fn generate_simple_source_tracking(field: &dyn FieldGenerator) -> QuoteStream {
    let name = field.name();
    let name_str = name.to_string();

    // For flatten fields, use the nested sources
    if field.is_flatten() {
        let nested_sources_ident = format_ident!("__{}_nested_sources", name);
        return quote! {
            __sources.extend_nested(#name_str, #nested_sources_ident);
        };
    }

    // For regular fields
    if let Some(env_var) = field.env_var_name() {
        let source_ident = format_ident!("__{}_source", name);
        let has_default = field.default_value().is_some();

        quote! {
            let #source_ident = if std::env::var(#env_var).is_ok() {
                if __dotenv_loaded && !__pre_dotenv_vars.contains(#env_var) {
                    ::procenv::ValueSource::new(#env_var, ::procenv::Source::DotenvFile(None))
                } else {
                    ::procenv::ValueSource::new(#env_var, ::procenv::Source::Environment)
                }
            } else if #has_default {
                ::procenv::ValueSource::new(#env_var, ::procenv::Source::Default)
            } else {
                ::procenv::ValueSource::new(#env_var, ::procenv::Source::NotSet)
            };
            __sources.add(#name_str, #source_ident);
        }
    } else {
        quote! {}
    }
}
