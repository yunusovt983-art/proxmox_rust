//! ifupdown2 integration for safe network configuration application

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

use pve_network_core::error::SystemError;
use pve_network_core::{NetworkConfiguration, NetworkError, Result};

/// ifupdown2 integration for network configuration
pub struct IfUpDownIntegration {
    /// Path to ifup command
    ifup_path: String,
    /// Path to ifdown command
    ifdown_path: String,
    /// Path to ifquery command
    ifquery_path: String,
    /// Timeout for ifupdown operations
    operation_timeout: Duration,
    /// Whether to use verbose output
    verbose: bool,
}

/// Result of ifupdown2 operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IfUpDownResult {
    /// Whether operation was successful
    pub success: bool,
    /// Exit code of the command
    pub exit_code: Option<i32>,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Duration of the operation
    pub duration_ms: u64,
}

/// Interface state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterfaceState {
    /// Interface name
    pub name: String,
    /// Whether interface is up
    pub is_up: bool,
    /// Current IP addresses
    pub addresses: Vec<String>,
    /// Interface flags
    pub flags: Vec<String>,
    /// MTU size
    pub mtu: Option<u16>,
}

impl IfUpDownIntegration {
    /// Create new ifupdown2 integration
    pub fn new() -> Self {
        Self {
            ifup_path: "/sbin/ifup".to_string(),
            ifdown_path: "/sbin/ifdown".to_string(),
            ifquery_path: "/sbin/ifquery".to_string(),
            operation_timeout: Duration::from_secs(60),
            verbose: false,
        }
    }

    /// Create with custom paths and settings
    pub fn with_config(
        ifup_path: String,
        ifdown_path: String,
        ifquery_path: String,
        operation_timeout: Duration,
        verbose: bool,
    ) -> Self {
        Self {
            ifup_path,
            ifdown_path,
            ifquery_path,
            operation_timeout,
            verbose,
        }
    }

    /// Perform dry-run validation of network configuration
    pub async fn dry_run(&self, config: &NetworkConfiguration) -> Result<IfUpDownResult> {
        info!("Performing ifupdown2 dry-run validation");

        // Write temporary configuration file
        let temp_config = self.write_temp_config(config).await?;

        let result = self.run_ifquery_dry_run(&temp_config).await;

        // Clean up temporary file
        if let Err(e) = tokio::fs::remove_file(&temp_config).await {
            warn!("Failed to remove temporary config file: {}", e);
        }

        match result {
            Ok(result) => {
                if result.success {
                    info!("Dry-run validation successful");
                } else {
                    error!("Dry-run validation failed: {}", result.stderr);
                }
                Ok(result)
            }
            Err(e) => {
                error!("Dry-run validation error: {}", e);
                Err(e)
            }
        }
    }

    /// Reload network configuration
    pub async fn reload_configuration(&self) -> Result<IfUpDownResult> {
        info!("Reloading network configuration");

        // Use ifup --all to reload all interfaces
        let mut cmd = Command::new(&self.ifup_path);
        cmd.arg("--all")
            .arg("--force")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if self.verbose {
            cmd.arg("--verbose");
        }

        self.execute_command(cmd, "reload configuration").await
    }

    /// Bring up a specific interface
    pub async fn bring_up_interface(&self, interface_name: &str) -> Result<IfUpDownResult> {
        info!("Bringing up interface {}", interface_name);

        let mut cmd = Command::new(&self.ifup_path);
        cmd.arg(interface_name)
            .arg("--force")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if self.verbose {
            cmd.arg("--verbose");
        }

        self.execute_command(cmd, &format!("bring up interface {}", interface_name))
            .await
    }

    /// Bring down a specific interface
    pub async fn bring_down_interface(&self, interface_name: &str) -> Result<IfUpDownResult> {
        info!("Bringing down interface {}", interface_name);

        let mut cmd = Command::new(&self.ifdown_path);
        cmd.arg(interface_name)
            .arg("--force")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if self.verbose {
            cmd.arg("--verbose");
        }

        self.execute_command(cmd, &format!("bring down interface {}", interface_name))
            .await
    }

    /// Query interface configuration
    pub async fn query_interface(&self, interface_name: &str) -> Result<InterfaceState> {
        debug!("Querying interface {}", interface_name);

        let mut cmd = Command::new(&self.ifquery_path);
        cmd.arg(interface_name)
            .arg("--running")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let result = self
            .execute_command(cmd, &format!("query interface {}", interface_name))
            .await?;

        if !result.success {
            return Err(NetworkError::System(SystemError::InterfaceOperation {
                interface: interface_name.to_string(),
            }));
        }

        // Parse ifquery output to extract interface state
        self.parse_interface_state(interface_name, &result.stdout)
    }

    /// List all configured interfaces
    pub async fn list_interfaces(&self) -> Result<Vec<String>> {
        debug!("Listing all configured interfaces");

        let mut cmd = Command::new(&self.ifquery_path);
        cmd.arg("--list")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let result = self.execute_command(cmd, "list interfaces").await?;

        if !result.success {
            return Err(NetworkError::System(SystemError::CommandFailed {
                command: "ifquery --list".to_string(),
            }));
        }

        Ok(result
            .stdout
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect())
    }

    /// Check if ifupdown2 is available
    pub async fn check_availability(&self) -> Result<bool> {
        debug!("Checking ifupdown2 availability");

        // Check if ifup command exists and is executable
        let mut cmd = Command::new(&self.ifup_path);
        cmd.arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match timeout(Duration::from_secs(5), cmd.output()).await {
            Ok(Ok(output)) => {
                let version_output = String::from_utf8_lossy(&output.stdout);
                // Check if this is ifupdown2 (not the original ifupdown)
                Ok(version_output.contains("ifupdown2") || version_output.contains("2."))
            }
            _ => Ok(false),
        }
    }

    /// Get ifupdown2 version information
    pub async fn get_version(&self) -> Result<String> {
        let mut cmd = Command::new(&self.ifup_path);
        cmd.arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let result = self.execute_command(cmd, "get version").await?;

        if result.success {
            Ok(result.stdout.trim().to_string())
        } else {
            Err(NetworkError::System(SystemError::CommandFailed {
                command: "ifup --version".to_string(),
            }))
        }
    }

    /// Run ifquery dry-run on temporary configuration
    async fn run_ifquery_dry_run(&self, config_file: &Path) -> Result<IfUpDownResult> {
        let mut cmd = Command::new(&self.ifquery_path);
        cmd.arg("--interfaces")
            .arg(config_file)
            .arg("--dry-run")
            .arg("--all")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if self.verbose {
            cmd.arg("--verbose");
        }

        self.execute_command(cmd, "dry-run validation").await
    }

    /// Write temporary configuration file for validation
    async fn write_temp_config(&self, config: &NetworkConfiguration) -> Result<std::path::PathBuf> {
        use pve_network_config::InterfacesParser;

        let parser = InterfacesParser::new();
        let config_content = parser.generate(config)?;

        let temp_file =
            std::env::temp_dir().join(format!("pve-network-{}.tmp", std::process::id()));

        tokio::fs::write(&temp_file, config_content).await?;
        Ok(temp_file)
    }

    /// Execute a command with timeout and logging
    async fn execute_command(&self, mut cmd: Command, operation: &str) -> Result<IfUpDownResult> {
        let start_time = std::time::Instant::now();

        debug!("Executing {}: {:?}", operation, cmd);

        let result = timeout(self.operation_timeout, cmd.output()).await;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let success = output.status.success();
                let exit_code = output.status.code();

                if success {
                    debug!("{} completed successfully in {}ms", operation, duration_ms);
                } else {
                    warn!(
                        "{} failed with exit code {:?}: {}",
                        operation, exit_code, stderr
                    );
                }

                Ok(IfUpDownResult {
                    success,
                    exit_code,
                    stdout,
                    stderr,
                    duration_ms,
                })
            }
            Ok(Err(e)) => {
                error!("{} failed to execute: {}", operation, e);
                Err(NetworkError::System(SystemError::CommandFailed {
                    command: operation.to_string(),
                }))
            }
            Err(_) => {
                error!("{} timed out after {:?}", operation, self.operation_timeout);
                Err(NetworkError::System(SystemError::CommandFailed {
                    command: format!("{} (timeout)", operation),
                }))
            }
        }
    }

    /// Parse interface state from ifquery output
    fn parse_interface_state(&self, interface_name: &str, output: &str) -> Result<InterfaceState> {
        let mut addresses = Vec::new();
        let mut flags = Vec::new();
        let mut mtu = None;
        let mut is_up = false;

        for line in output.lines() {
            let line = line.trim();

            if line.starts_with("address ") {
                if let Some(addr) = line.strip_prefix("address ") {
                    addresses.push(addr.to_string());
                }
            } else if line.starts_with("mtu ") {
                if let Some(mtu_str) = line.strip_prefix("mtu ") {
                    if let Ok(mtu_val) = mtu_str.parse::<u16>() {
                        mtu = Some(mtu_val);
                    }
                }
            } else if line.contains("UP") {
                is_up = true;
                flags.push("UP".to_string());
            } else if line.contains("RUNNING") {
                flags.push("RUNNING".to_string());
            }
        }

        Ok(InterfaceState {
            name: interface_name.to_string(),
            is_up,
            addresses,
            flags,
            mtu,
        })
    }

    /// Restart networking service (fallback method)
    pub async fn restart_networking_service(&self) -> Result<IfUpDownResult> {
        warn!("Using fallback method: restarting networking service");

        let mut cmd = Command::new("systemctl");
        cmd.arg("restart")
            .arg("networking")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.execute_command(cmd, "restart networking service")
            .await
    }

    /// Apply configuration changes to specific interfaces only
    pub async fn apply_interface_changes(
        &self,
        interface_changes: &HashMap<String, InterfaceChangeType>,
    ) -> Result<Vec<IfUpDownResult>> {
        let mut results = Vec::new();

        // Process changes in order: down, up, reload
        let mut interfaces_to_down = Vec::new();
        let mut interfaces_to_up = Vec::new();
        let mut interfaces_to_reload = Vec::new();

        for (interface, change_type) in interface_changes {
            match change_type {
                InterfaceChangeType::Delete => interfaces_to_down.push(interface),
                InterfaceChangeType::Create => interfaces_to_up.push(interface),
                InterfaceChangeType::Update => interfaces_to_reload.push(interface),
            }
        }

        // Bring down deleted interfaces
        for interface in interfaces_to_down {
            let result = self.bring_down_interface(interface).await?;
            results.push(result);
        }

        // Reload updated interfaces
        for interface in interfaces_to_reload {
            let result = self.bring_down_interface(interface).await?;
            results.push(result);
            let result = self.bring_up_interface(interface).await?;
            results.push(result);
        }

        // Bring up new interfaces
        for interface in interfaces_to_up {
            let result = self.bring_up_interface(interface).await?;
            results.push(result);
        }

        Ok(results)
    }
}

/// Type of change to an interface
#[derive(Debug, Clone, PartialEq)]
pub enum InterfaceChangeType {
    /// Interface was created
    Create,
    /// Interface was updated
    Update,
    /// Interface was deleted
    Delete,
}

impl Default for IfUpDownIntegration {
    fn default() -> Self {
        Self::new()
    }
}
