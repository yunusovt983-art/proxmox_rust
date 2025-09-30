//! Proxmox VE Network CLI (pvenet)

use anyhow::Result;
use clap::{Parser, Subcommand};
use pve_network_api::context::AppContext;
use pvenet::commands::{
    ApplyCommand, CompatCommand, RollbackCommand, StatusCommand, ValidateCommand,
};

#[derive(Parser)]
#[command(name = "pvenet")]
#[command(about = "Proxmox VE Network Management CLI")]
#[command(version)]
#[command(long_about = "
Proxmox VE Network Management CLI

This tool provides command-line interface for managing Proxmox VE network 
configurations, including validation, application, rollback, and status 
operations. It maintains compatibility with existing Proxmox network tools.

Examples:
  pvenet validate                          # Validate default config
  pvenet validate -c /path/to/interfaces   # Validate specific config
  pvenet validate -i eth0                  # Validate specific interface
  pvenet apply --dry-run                   # Test configuration changes
  pvenet apply                             # Apply configuration
  pvenet apply -i eth0                     # Apply specific interface
  pvenet rollback                          # Rollback to previous version
  pvenet rollback -v 20231201-120000       # Rollback to specific version
  pvenet rollback --list                   # List available versions
  pvenet status                            # Show basic status
  pvenet status -v                         # Show detailed status
  pvenet status --stats                    # Show interface statistics
")]
struct Cli {
    /// Enable verbose output
    #[arg(short = 'V', long, global = true)]
    verbose: bool,

    /// Enable debug output
    #[arg(short, long, global = true)]
    debug: bool,

    /// Suppress all output except errors
    #[arg(short, long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate network configuration
    Validate {
        /// Configuration file to validate
        #[arg(short, long, default_value = "/etc/network/interfaces")]
        config: String,

        /// Validate specific interface only
        #[arg(short, long)]
        interface: Option<String>,

        /// Check syntax only (skip semantic validation)
        #[arg(long)]
        syntax_only: bool,

        /// Skip ifupdown2 dry-run validation
        #[arg(long)]
        skip_ifupdown: bool,
    },

    /// Apply network configuration
    Apply {
        /// Perform dry-run only (test without applying)
        #[arg(short, long)]
        dry_run: bool,

        /// Apply specific interface only
        #[arg(short, long)]
        interface: Option<String>,

        /// Force apply even if validation warnings exist
        #[arg(short, long)]
        force: bool,

        /// Skip backup creation before apply
        #[arg(long)]
        no_backup: bool,

        /// Configuration file to apply
        #[arg(short, long, default_value = "/etc/network/interfaces")]
        config: String,
    },

    /// Rollback network configuration
    Rollback {
        /// Rollback to specific version
        #[arg(short, long)]
        version: Option<String>,

        /// List available backup versions
        #[arg(short, long)]
        list: bool,

        /// Show rollback status
        #[arg(long)]
        status: bool,

        /// Force rollback without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Show network status
    Status {
        /// Show detailed status
        #[arg(short, long)]
        verbose: bool,

        /// Show interface statistics
        #[arg(long)]
        stats: bool,

        /// Show status for specific interface
        #[arg(short, long)]
        interface: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Show only interfaces matching pattern
        #[arg(long)]
        filter: Option<String>,
    },

    /// List network interfaces (pvesh compatible)
    List {
        /// Node name
        #[arg(short, long, default_value = "localhost")]
        node: String,

        /// Output format (text, json, yaml)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Show network configuration
    Show {
        /// Node name
        #[arg(short, long, default_value = "localhost")]
        node: String,

        /// Show specific interface configuration
        #[arg(short, long)]
        interface: Option<String>,
    },

    /// Reload network configuration
    Reload {
        /// Node name
        #[arg(short, long, default_value = "localhost")]
        node: String,

        /// Force reload without confirmation
        #[arg(short, long)]
        force: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging based on verbosity
    let log_level = if cli.debug {
        "debug"
    } else if cli.verbose {
        "info"
    } else if cli.quiet {
        "error"
    } else {
        "warn"
    };

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(log_level)).init();

    let context = AppContext::bootstrap().await?;

    // Execute commands
    let result = match cli.command {
        Commands::Validate {
            config,
            interface,
            syntax_only: _,
            skip_ifupdown: _,
        } => {
            let cmd = ValidateCommand::new(context.clone());
            match interface {
                Some(iface) => cmd.validate_interface(&config, &iface).await,
                None => cmd.execute(&config).await,
            }
        }

        Commands::Apply {
            dry_run,
            interface,
            force: _,
            no_backup: _,
            config: _,
        } => {
            let cmd = ApplyCommand::new(context.clone());
            match interface {
                Some(iface) => cmd.apply_interface(&iface, dry_run).await,
                None => cmd.execute(dry_run).await,
            }
        }

        Commands::Rollback {
            version,
            list,
            status,
            force: _,
        } => {
            let cmd = RollbackCommand::new(context.clone());
            if list {
                cmd.list_versions().await
            } else if status {
                cmd.show_status().await
            } else {
                cmd.execute(version.as_deref()).await
            }
        }

        Commands::Status {
            verbose,
            stats,
            interface,
            format: _,
            filter: _,
        } => {
            let cmd = StatusCommand::new(context.clone());
            if stats {
                cmd.show_statistics(interface.as_deref()).await
            } else {
                cmd.execute(verbose).await
            }
        }

        Commands::List { node, format } => {
            let cmd = CompatCommand::new(context.clone());
            cmd.list_nodes_network(&node, &format).await
        }

        Commands::Show { node, interface } => {
            let cmd = CompatCommand::new(context.clone());
            cmd.show_config(&node, interface.as_deref()).await
        }

        Commands::Reload { node, force: _ } => {
            let cmd = CompatCommand::new(context.clone());
            cmd.reload_network(&node).await
        }
    };

    // Handle errors with appropriate exit codes
    match result {
        Ok(()) => {
            if !cli.quiet {
                log::info!("Command completed successfully");
            }
            std::process::exit(0);
        }
        Err(e) => {
            if !cli.quiet {
                eprintln!("Error: {}", e);

                // Print error chain if in verbose mode
                if cli.verbose || cli.debug {
                    let mut source = e.source();
                    while let Some(err) = source {
                        eprintln!("  Caused by: {}", err);
                        source = err.source();
                    }
                }
            }
            std::process::exit(1);
        }
    }
}
