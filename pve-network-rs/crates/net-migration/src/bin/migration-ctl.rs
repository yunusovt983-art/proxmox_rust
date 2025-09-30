//! Migration control CLI tool
//!
//! Command-line utility for managing the migration from Perl to Rust

use clap::{Parser, Subcommand};
use net_migration::{MigrationConfig, MigrationPhase};
// use serde_json::json;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "migration-ctl")]
#[command(about = "PVE Network Migration Control Tool")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Configuration file path
    #[arg(short, long, default_value = "/etc/pve/network-migration.conf")]
    config: PathBuf,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Show current migration status
    Status,

    /// Set migration phase
    SetPhase {
        /// Migration phase to set
        #[arg(value_enum)]
        phase: MigrationPhaseArg,
    },

    /// Enable or disable fallback
    Fallback {
        /// Enable or disable fallback
        enabled: bool,
    },

    /// Show configuration
    Config,

    /// Validate configuration file
    Validate,

    /// Generate example configuration
    GenerateConfig {
        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Test Perl API connectivity
    TestPerl,

    /// Show migration metrics (if available)
    Metrics,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum MigrationPhaseArg {
    PerlOnly,
    RustReadOnly,
    RustBasicWrite,
    RustAdvanced,
    RustSdn,
    RustFull,
}

impl From<MigrationPhaseArg> for MigrationPhase {
    fn from(arg: MigrationPhaseArg) -> Self {
        match arg {
            MigrationPhaseArg::PerlOnly => MigrationPhase::PerlOnly,
            MigrationPhaseArg::RustReadOnly => MigrationPhase::RustReadOnly,
            MigrationPhaseArg::RustBasicWrite => MigrationPhase::RustBasicWrite,
            MigrationPhaseArg::RustAdvanced => MigrationPhase::RustAdvanced,
            MigrationPhaseArg::RustSdn => MigrationPhase::RustSdn,
            MigrationPhaseArg::RustFull => MigrationPhase::RustFull,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    if cli.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }

    match cli.command {
        Commands::Status => {
            let config = load_config(&cli.config)?;
            print_status(&config);
        }

        Commands::SetPhase { phase } => {
            let mut config = load_config(&cli.config)?;
            config.phase = phase.into();
            config.update_endpoints_for_phase();
            save_config(&cli.config, &config)?;
            println!("Migration phase updated to: {:?}", config.phase);
        }

        Commands::Fallback { enabled } => {
            let mut config = load_config(&cli.config)?;
            config.fallback_enabled = enabled;
            save_config(&cli.config, &config)?;
            println!("Fallback {}", if enabled { "enabled" } else { "disabled" });
        }

        Commands::Config => {
            let config = load_config(&cli.config)?;
            let config_json = serde_json::to_string_pretty(&config)?;
            println!("{}", config_json);
        }

        Commands::Validate => match load_config(&cli.config) {
            Ok(_) => println!("Configuration is valid"),
            Err(e) => {
                eprintln!("Configuration validation failed: {}", e);
                std::process::exit(1);
            }
        },

        Commands::GenerateConfig { output } => {
            let config = MigrationConfig::default();
            let config_toml = toml::to_string_pretty(&config)?;

            if let Some(output_path) = output {
                std::fs::write(&output_path, config_toml)?;
                println!(
                    "Example configuration written to: {}",
                    output_path.display()
                );
            } else {
                println!("{}", config_toml);
            }
        }

        Commands::TestPerl => {
            let config = load_config(&cli.config)?;
            test_perl_connectivity(&config).await?;
        }

        Commands::Metrics => {
            println!("Metrics collection requires a running migration server.");
            println!("Use the migration server's health endpoint to view metrics.");
        }
    }

    Ok(())
}

fn load_config(path: &PathBuf) -> Result<MigrationConfig, Box<dyn std::error::Error>> {
    if path.exists() {
        MigrationConfig::load_from_file(path).map_err(Into::into)
    } else {
        println!("Configuration file not found, using defaults");
        Ok(MigrationConfig::default())
    }
}

fn save_config(path: &PathBuf, config: &MigrationConfig) -> Result<(), Box<dyn std::error::Error>> {
    let config_toml = toml::to_string_pretty(config)?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::write(path, config_toml)?;
    Ok(())
}

fn print_status(config: &MigrationConfig) {
    println!("PVE Network Migration Status");
    println!("============================");
    println!("Phase: {:?}", config.phase);
    println!("Fallback enabled: {}", config.fallback_enabled);
    println!("Perl API URL: {}", config.perl_api_base_url);
    println!("Logging enabled: {}", config.log_migration_decisions);
    println!("Metrics enabled: {}", config.metrics_enabled);

    println!("\nEndpoint Configuration:");
    println!("-----------------------");
    for (endpoint, endpoint_config) in &config.endpoints {
        println!("  {}", endpoint);
        println!("    Use Rust: {}", endpoint_config.use_rust);
        println!("    Fallback: {}", endpoint_config.fallback_on_error);
        println!(
            "    Timeout: {}s",
            endpoint_config.rust_timeout.unwrap_or(30)
        );
        if !endpoint_config.rust_methods.is_empty() {
            println!("    Methods: {:?}", endpoint_config.rust_methods);
        }
        println!();
    }

    if !config.features.is_empty() {
        println!("Feature Flags:");
        println!("--------------");
        for (feature, enabled) in &config.features {
            println!("  {}: {}", feature, enabled);
        }
    }
}

async fn test_perl_connectivity(
    config: &MigrationConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    use net_migration::perl_client::{HttpPerlApiClient, PerlApiClient};
    use std::time::Duration;

    println!("Testing Perl API connectivity...");
    println!("URL: {}", config.perl_api_base_url);

    let client = HttpPerlApiClient::new(
        config.perl_api_base_url.clone(),
        Duration::from_secs(config.perl_api_timeout),
    );

    match client.health_check().await {
        Ok(true) => {
            println!("✓ Perl API is healthy and reachable");
        }
        Ok(false) => {
            println!("✗ Perl API is reachable but not healthy");
        }
        Err(e) => {
            println!("✗ Failed to connect to Perl API: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
