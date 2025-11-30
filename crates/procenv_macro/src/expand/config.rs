//! File-based configuration code generation.

use proc_macro2::TokenStream as QuoteStream;
use quote::quote;
use syn::{Generics, Ident};

use crate::field::FieldGenerator;
use crate::parse::EnvConfigAttr;

use super::env::generate_dotenv_load;

/// Generate the `from_config()` method for file-based configuration loading.
pub fn generate_from_config_impl(
    struct_name: &Ident,
    generics: &Generics,
    generators: &[Box<dyn FieldGenerator>],
    env_config_attr: &EnvConfigAttr,
) -> QuoteStream {
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    // Generate file loading code
    let file_loads: Vec<QuoteStream> = env_config_attr
        .files
        .iter()
        .map(|f| {
            let path = &f.path;
            if f.required {
                quote! {
                    builder = builder.file(#path);
                }
            } else {
                quote! {
                    builder = builder.file_optional(#path);
                }
            }
        })
        .collect();

    // Generate env prefix setup
    let env_prefix = if let Some(prefix) = &env_config_attr.prefix {
        quote! { builder = builder.env_prefix(#prefix); }
    } else {
        quote! {}
    };

    // Generate direct env var mappings for fields with custom var names
    let env_mapping_calls: Vec<QuoteStream> = generators
        .iter()
        .filter_map(|g| {
            if g.is_flatten() {
                return None;
            }

            let env_var = g.env_var_name()?;
            let field_name = g.name().to_string();

            Some(quote! {
                builder = builder.env_mapping(#field_name, #env_var);
            })
        })
        .collect();

    let env_mappings = quote! {
        #(#env_mapping_calls)*
    };

    // Generate dotenv loading
    let dotenv_load = generate_dotenv_load(&env_config_attr.dotenv);

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

    // Track if dotenv was loaded
    let dotenv_loaded_flag = if env_config_attr.dotenv.is_some() {
        quote! { let __dotenv_loaded = true; }
    } else {
        quote! { let __dotenv_loaded = false; }
    };

    // Generate default values for fields that have them
    let default_entries: Vec<QuoteStream> = generators
        .iter()
        .filter_map(|g| {
            if g.is_flatten() {
                return None;
            }

            let field_name = g.name().to_string();
            let json_key = field_name.clone();

            g.default_value().map(|default| {
                quote! {
                    __defaults.insert(
                        #json_key.to_string(),
                        ::procenv::FileUtils::coerce_value(#default)
                    );
                }
            })
        })
        .collect();

    // Generate nested defaults collection for flatten fields
    let flatten_default_entries: Vec<QuoteStream> = generators
        .iter()
        .filter_map(|g| {
            if !g.is_flatten() {
                return None;
            }

            let field_name = g.name().to_string();
            let ty = g.field_type()?;

            Some(quote! {
                if let ::serde_json::Value::Object(nested_map) = <#ty>::__config_defaults() {
                    __defaults.insert(
                        #field_name.to_string(),
                        ::serde_json::Value::Object(nested_map)
                    );
                }
            })
        })
        .collect();

    // Determine if we need defaults setup
    let has_flatten = generators.iter().any(|g| g.is_flatten());
    let defaults_setup = if default_entries.is_empty() && !has_flatten {
        quote! {}
    } else {
        quote! {
            let mut __defaults = ::serde_json::Map::new();
            #(#default_entries)*
            #(#flatten_default_entries)*
            builder = builder.defaults_value(::serde_json::Value::Object(__defaults));
        }
    };

    // Generate source tracking entries
    let source_entries: Vec<QuoteStream> = generators
        .iter()
        .map(|g| {
            let field_name = g.name().to_string();
            let has_default = g.default_value().is_some();

            if g.is_flatten() {
                quote! {
                    {
                        let prefix = concat!(#field_name, ".");
                        for tracked_path in __origins.tracked_fields() {
                            if tracked_path.starts_with(prefix) || tracked_path == #field_name {
                                let nested_name = if tracked_path == #field_name {
                                    tracked_path.to_string()
                                } else {
                                    tracked_path.to_string()
                                };

                                let source = if let Some(file_path) = __origins.get_file_source(tracked_path) {
                                    ::procenv::Source::ConfigFile(Some(file_path))
                                } else {
                                    ::procenv::Source::NotSet
                                };

                                __sources.add(
                                    nested_name,
                                    ::procenv::ValueSource::new(tracked_path, source)
                                );
                            }
                        }
                    }
                }
            } else {
                let env_var = g.env_var_name().unwrap_or("");

                quote! {
                    {
                        let source = if std::env::var(#env_var).is_ok() {
                            if __dotenv_loaded && !__pre_dotenv_vars.contains(#env_var) {
                                ::procenv::Source::DotenvFile(None)
                            } else {
                                ::procenv::Source::Environment
                            }
                        } else if let Some(file_path) = __origins.get_file_source(#field_name) {
                            ::procenv::Source::ConfigFile(Some(file_path))
                        } else if #has_default {
                            ::procenv::Source::Default
                        } else {
                            ::procenv::Source::NotSet
                        };
                        __sources.add(
                            #field_name,
                            ::procenv::ValueSource::new(#env_var, source)
                        );
                    }
                }
            }
        })
        .collect();

    quote! {
        impl #impl_generics #struct_name #type_generics #where_clause
        where
            Self: ::serde::de::DeserializeOwned,
        {
            /// Load configuration from files and environment variables.
            pub fn from_config() -> std::result::Result<Self, ::procenv::Error> {
                #dotenv_load

                let mut builder = ::procenv::ConfigBuilder::new();

                #defaults_setup

                #(#file_loads)*

                #env_prefix

                #env_mappings

                builder.build()
            }

            /// Load configuration from files and environment variables with source attribution.
            pub fn from_config_with_sources() -> std::result::Result<(Self, ::procenv::ConfigSources), ::procenv::Error> {
                #pre_dotenv_collection

                #dotenv_load

                #dotenv_loaded_flag

                let mut builder = ::procenv::ConfigBuilder::new();

                #defaults_setup

                #(#file_loads)*

                #env_prefix

                #env_mappings

                let (__config, __origins) = builder.build_with_origins()?;

                let mut __sources = ::procenv::ConfigSources::new();
                #(#source_entries)*

                std::result::Result::Ok((__config, __sources))
            }
        }
    }
}

/// Generate the `__config_defaults()` method for nested struct defaults.
pub fn generate_config_defaults_impl(
    struct_name: &Ident,
    generics: &Generics,
    generators: &[Box<dyn FieldGenerator>],
) -> QuoteStream {
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();

    // Generate default entries for regular fields
    let default_entries: Vec<QuoteStream> = generators
        .iter()
        .filter_map(|g| {
            if g.is_flatten() {
                return None;
            }

            let field_name = g.name().to_string();
            g.default_value().map(|default| {
                quote! {
                    __map.insert(
                        #field_name.to_string(),
                        ::procenv::FileUtils::coerce_value(#default)
                    );
                }
            })
        })
        .collect();

    // Generate nested defaults for flatten fields
    let flatten_entries: Vec<QuoteStream> = generators
        .iter()
        .filter_map(|g| {
            if !g.is_flatten() {
                return None;
            }

            let field_name = g.name().to_string();
            let ty = g.field_type()?;

            Some(quote! {
                if let ::serde_json::Value::Object(nested) = <#ty>::__config_defaults() {
                    __map.insert(
                        #field_name.to_string(),
                        ::serde_json::Value::Object(nested)
                    );
                }
            })
        })
        .collect();

    quote! {
        impl #impl_generics #struct_name #type_generics #where_clause {
            /// Returns default values for this config as a JSON object.
            #[doc(hidden)]
            pub fn __config_defaults() -> ::serde_json::Value {
                let mut __map = ::serde_json::Map::new();
                #(#default_entries)*
                #(#flatten_entries)*
                ::serde_json::Value::Object(__map)
            }
        }
    }
}
