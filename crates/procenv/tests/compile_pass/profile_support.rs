//! Test that profile attributes are parsed and work correctly.

use procenv::{EnvConfig, Source};

#[derive(EnvConfig)]
#[env_config(
    profile_env = "APP_ENV",
    profiles = ["dev", "staging", "prod"]
)]
struct ProfileConfig {
    /// The database URL
    #[env(var = "DATABASE_URL")]
    #[profile(
        dev = "postgres://localhost/myapp_dev",
        staging = "postgres://staging-db/myapp",
        prod = "postgres://prod-db/myapp"
    )]
    database_url: String,

    /// Log level
    #[env(var = "LOG_LEVEL", default = "info")]
    #[profile(dev = "debug", staging = "info")]
    log_level: String,

    /// Port (no profile - same across all environments)
    #[env(var = "PORT", default = "8080")]
    port: u16,
}

fn main() {
    // Test 1: Profile defaults work when no env vars set
    // SAFETY: This is a test environment with no concurrent access
    unsafe {
        std::env::set_var("APP_ENV", "dev");
    }

    let config = ProfileConfig::from_env().unwrap();
    assert_eq!(config.database_url, "postgres://localhost/myapp_dev");
    assert_eq!(config.log_level, "debug");
    assert_eq!(config.port, 8080);

    // Test 2: Env vars override profile defaults
    unsafe {
        std::env::set_var("APP_ENV", "dev");
        std::env::set_var("DATABASE_URL", "postgres://custom/db");
    }

    let config = ProfileConfig::from_env().unwrap();
    assert_eq!(config.database_url, "postgres://custom/db");

    // Clean up
    unsafe {
        std::env::remove_var("APP_ENV");
        std::env::remove_var("DATABASE_URL");
    }

    // Test 3: Production profile
    unsafe {
        std::env::set_var("APP_ENV", "prod");
    }

    let config = ProfileConfig::from_env().unwrap();
    assert_eq!(config.database_url, "postgres://prod-db/myapp");
    // log_level uses default "info" because prod has no profile override
    assert_eq!(config.log_level, "info");

    // Clean up
    unsafe {
        std::env::remove_var("APP_ENV");
    }

    // =========================================================================
    // Test 4: Source attribution for profile values
    // =========================================================================
    unsafe {
        std::env::set_var("APP_ENV", "staging");
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("LOG_LEVEL");
        std::env::remove_var("PORT");
    }

    let (_config, sources) = ProfileConfig::from_env_with_sources().unwrap();

    // database_url should come from profile (staging)
    let db_source = sources.get("database_url").expect("database_url source");
    assert!(
        matches!(db_source.source, Source::Profile(ref p) if p == "staging"),
        "database_url should be Source::Profile(staging), got {:?}",
        db_source.source
    );

    // log_level should come from profile (staging has profile value)
    let log_source = sources.get("log_level").expect("log_level source");
    assert!(
        matches!(log_source.source, Source::Profile(ref p) if p == "staging"),
        "log_level should be Source::Profile(staging), got {:?}",
        log_source.source
    );

    // port should come from default (no profile for port)
    let port_source = sources.get("port").expect("port source");
    assert!(
        matches!(port_source.source, Source::Default),
        "port should be Source::Default, got {:?}",
        port_source.source
    );

    // Clean up
    unsafe {
        std::env::remove_var("APP_ENV");
    }

    // =========================================================================
    // Test 5: Environment overrides profile - source should be Environment
    // =========================================================================
    unsafe {
        std::env::set_var("APP_ENV", "dev");
        std::env::set_var("DATABASE_URL", "postgres://override/db");
    }

    let (_config, sources) = ProfileConfig::from_env_with_sources().unwrap();

    // database_url should come from environment (override)
    let db_source = sources.get("database_url").expect("database_url source");
    assert!(
        matches!(db_source.source, Source::Environment),
        "database_url should be Source::Environment when overridden, got {:?}",
        db_source.source
    );

    // Clean up
    unsafe {
        std::env::remove_var("APP_ENV");
        std::env::remove_var("DATABASE_URL");
    }

    // =========================================================================
    // Test 6: Default used when profile has no value for field
    // =========================================================================
    unsafe {
        std::env::set_var("APP_ENV", "prod");
        std::env::remove_var("LOG_LEVEL");
    }

    let (_config, sources) = ProfileConfig::from_env_with_sources().unwrap();

    // log_level should come from default (prod profile has no log_level)
    let log_source = sources.get("log_level").expect("log_level source");
    assert!(
        matches!(log_source.source, Source::Default),
        "log_level should be Source::Default when prod profile has no value, got {:?}",
        log_source.source
    );

    // Clean up
    unsafe {
        std::env::remove_var("APP_ENV");
    }
}
