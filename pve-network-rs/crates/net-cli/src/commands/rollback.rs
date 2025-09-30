//! Rollback command

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use pve_network_api::context::AppContext;
use std::fs;
use std::path::Path;
use std::sync::Arc;

/// Rollback command implementation
pub struct RollbackCommand {
    #[allow(dead_code)]
    _context: Arc<AppContext>,
}

impl RollbackCommand {
    /// Create new rollback command
    pub fn new(context: Arc<AppContext>) -> Self {
        Self { _context: context }
    }

    /// Execute rollback command
    pub async fn execute(&self, version: Option<&str>) -> Result<()> {
        match version {
            Some(v) => {
                println!("Rolling back network configuration to version: {}", v);
                self.rollback_to_version(v).await
            }
            None => {
                println!("Rolling back network configuration to previous version");
                self.rollback_to_previous().await
            }
        }
    }

    /// Rollback to specific version
    async fn rollback_to_version(&self, version: &str) -> Result<()> {
        // Check if version exists
        let backup_path = format!("/etc/pve/network-backups/{}", version);
        if !Path::new(&backup_path).exists() {
            anyhow::bail!("Backup version '{}' not found", version);
        }

        println!("Found backup version: {}", version);

        // Perform rollback (simulated for now)
        println!(
            "✓ Successfully rolled back to version: {} (simulated)",
            version
        );

        Ok(())
    }

    /// Rollback to previous version
    async fn rollback_to_previous(&self) -> Result<()> {
        // Find the most recent backup
        let backup_dir = "/etc/pve/network-backups";
        if !Path::new(backup_dir).exists() {
            anyhow::bail!("No backup directory found. Cannot rollback.");
        }

        let entries =
            fs::read_dir(backup_dir).with_context(|| "Failed to read backup directory")?;

        let mut backups: Vec<String> = entries
            .filter_map(|entry| {
                entry
                    .ok()
                    .and_then(|e| e.file_name().to_str().map(|s| s.to_string()))
            })
            .collect();

        if backups.is_empty() {
            anyhow::bail!("No backups found. Cannot rollback.");
        }

        // Sort backups by timestamp (assuming timestamp format)
        backups.sort();
        let latest_backup = backups.last().unwrap();

        println!("Found latest backup: {}", latest_backup);
        self.rollback_to_version(latest_backup).await
    }

    /// List available backup versions
    pub async fn list_versions(&self) -> Result<()> {
        let backup_dir = "/etc/pve/network-backups";

        if !Path::new(backup_dir).exists() {
            println!("No backup directory found.");
            return Ok(());
        }

        let entries =
            fs::read_dir(backup_dir).with_context(|| "Failed to read backup directory")?;

        let mut backups: Vec<(String, u64)> = Vec::new();

        for entry in entries {
            let entry = entry?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            let metadata = entry.metadata()?;
            let modified = metadata.modified()?;
            let timestamp = modified.duration_since(std::time::UNIX_EPOCH)?.as_secs();
            backups.push((file_name, timestamp));
        }

        if backups.is_empty() {
            println!("No backups available.");
            return Ok(());
        }

        // Sort by timestamp (newest first)
        backups.sort_by(|a, b| b.1.cmp(&a.1));

        println!("Available backup versions:");
        println!("{:<20} {:<20} {}", "Version", "Date", "Time");
        println!("{}", "-".repeat(60));

        for (version, timestamp) in backups {
            let datetime =
                DateTime::<Utc>::from_timestamp(timestamp as i64, 0).unwrap_or_else(|| Utc::now());
            println!(
                "{:<20} {:<20} {}",
                version,
                datetime.format("%Y-%m-%d").to_string(),
                datetime.format("%H:%M:%S").to_string()
            );
        }

        Ok(())
    }

    /// Show rollback status
    pub async fn show_status(&self) -> Result<()> {
        println!("Network configuration rollback status:");

        // Check if rollback is in progress
        let rollback_lock = "/var/lock/pve-network-rollback.lock";
        if Path::new(rollback_lock).exists() {
            println!("Status: Rollback in progress");
            return Ok(());
        }

        // Check last rollback information
        let rollback_log = "/var/log/pve-network-rollback.log";
        if Path::new(rollback_log).exists() {
            let content =
                fs::read_to_string(rollback_log).with_context(|| "Failed to read rollback log")?;

            let lines: Vec<&str> = content.lines().collect();
            if let Some(last_line) = lines.last() {
                println!("Last rollback: {}", last_line);
            }
        } else {
            println!("Status: No rollback history found");
        }

        // Show current configuration timestamp
        let config_path = "/etc/network/interfaces";
        if Path::new(config_path).exists() {
            let metadata = fs::metadata(config_path)?;
            let modified = metadata.modified()?;
            let datetime = DateTime::<Utc>::from(modified);
            println!(
                "Current config modified: {}",
                datetime.format("%Y-%m-%d %H:%M:%S UTC")
            );
        }

        Ok(())
    }
}
