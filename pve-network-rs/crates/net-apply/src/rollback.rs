//! Rollback mechanisms for network configuration changes

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use tokio::fs;

use pve_network_config::NetworkConfigManager;
use pve_network_core::error::SystemError;
use pve_network_core::{NetworkConfiguration, NetworkError, Result};

/// Rollback point containing configuration snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackPoint {
    /// Unique rollback point ID
    pub id: String,
    /// Transaction ID this rollback point belongs to
    pub transaction_id: String,
    /// Timestamp when rollback point was created
    pub timestamp: u64,
    /// Configuration snapshot
    pub configuration: NetworkConfiguration,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
    /// Files that were backed up
    pub backed_up_files: Vec<BackupFile>,
}

/// Information about a backed up file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFile {
    /// Original file path
    pub original_path: PathBuf,
    /// Backup file path
    pub backup_path: PathBuf,
    /// File checksum for integrity verification
    pub checksum: String,
    /// File size in bytes
    pub size: u64,
}

/// Rollback manager for network configurations
pub struct RollbackManager {
    /// Configuration manager for reading/writing configs
    config_manager: Option<std::sync::Arc<NetworkConfigManager>>,
    /// Directory for storing rollback points
    rollback_dir: PathBuf,
    /// Maximum number of rollback points to keep
    max_rollback_points: usize,
    /// Maximum age of rollback points in seconds
    max_age_seconds: u64,
}

impl RollbackManager {
    /// Create new rollback manager
    pub async fn new(
        config_manager: Option<std::sync::Arc<NetworkConfigManager>>,
        rollback_dir: Option<PathBuf>,
    ) -> Result<Self> {
        let rollback_dir =
            rollback_dir.unwrap_or_else(|| PathBuf::from("/var/lib/pve-network/rollback"));

        // Ensure rollback directory exists
        if !rollback_dir.exists() {
            fs::create_dir_all(&rollback_dir).await?;
        }

        let manager = Self {
            config_manager,
            rollback_dir,
            max_rollback_points: 50,        // Keep last 50 rollback points
            max_age_seconds: 7 * 24 * 3600, // Keep rollback points for 7 days
        };

        // Clean up old rollback points on startup
        if let Err(e) = manager.cleanup_old_rollback_points().await {
            warn!("Failed to cleanup old rollback points: {}", e);
        }

        Ok(manager)
    }

    /// Create a rollback point for the given configuration
    pub async fn create_rollback_point(
        &self,
        transaction_id: &str,
        configuration: &NetworkConfiguration,
    ) -> Result<RollbackPoint> {
        let rollback_id = self.generate_rollback_id(transaction_id);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        info!(
            "Creating rollback point {} for transaction {}",
            rollback_id, transaction_id
        );

        // Backup critical configuration files
        let backed_up_files = self.backup_configuration_files(&rollback_id).await?;

        let rollback_point = RollbackPoint {
            id: rollback_id.clone(),
            transaction_id: transaction_id.to_string(),
            timestamp,
            configuration: configuration.clone(),
            metadata: HashMap::new(),
            backed_up_files,
        };

        // Save rollback point metadata
        let rollback_file = self.rollback_dir.join(format!("{}.json", rollback_id));
        let rollback_data = serde_json::to_string_pretty(&rollback_point)?;
        fs::write(&rollback_file, rollback_data).await?;

        info!("Created rollback point {}", rollback_id);
        Ok(rollback_point)
    }

    /// Restore configuration from a rollback point
    pub async fn restore_rollback_point(&self, transaction_id: &str) -> Result<()> {
        let rollback_point = self
            .find_rollback_point_by_transaction(transaction_id)
            .await?;

        info!(
            "Restoring rollback point {} for transaction {}",
            rollback_point.id, transaction_id
        );

        // Restore backed up files
        for backup_file in &rollback_point.backed_up_files {
            self.restore_backup_file(backup_file).await?;
        }

        // Restore configuration through config manager if available
        if let Some(config_manager) = &self.config_manager {
            config_manager
                .write_config(&rollback_point.configuration)
                .await?;
        }

        info!("Restored rollback point {}", rollback_point.id);
        Ok(())
    }

    /// Clean up a rollback point after successful commit
    pub async fn cleanup_rollback_point(&self, transaction_id: &str) -> Result<()> {
        if let Ok(rollback_point) = self
            .find_rollback_point_by_transaction(transaction_id)
            .await
        {
            info!(
                "Cleaning up rollback point {} for transaction {}",
                rollback_point.id, transaction_id
            );

            // Remove backed up files
            for backup_file in &rollback_point.backed_up_files {
                if backup_file.backup_path.exists() {
                    if let Err(e) = fs::remove_file(&backup_file.backup_path).await {
                        warn!(
                            "Failed to remove backup file {:?}: {}",
                            backup_file.backup_path, e
                        );
                    }
                }
            }

            // Remove rollback point metadata
            let rollback_file = self
                .rollback_dir
                .join(format!("{}.json", rollback_point.id));
            if rollback_file.exists() {
                fs::remove_file(&rollback_file).await?;
            }

            info!("Cleaned up rollback point {}", rollback_point.id);
        }

        Ok(())
    }

    /// List all available rollback points
    pub async fn list_rollback_points(&self) -> Result<Vec<RollbackPoint>> {
        let mut rollback_points = Vec::new();
        let mut entries = fs::read_dir(&self.rollback_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_rollback_point(&path).await {
                    Ok(rollback_point) => rollback_points.push(rollback_point),
                    Err(e) => warn!("Failed to load rollback point from {:?}: {}", path, e),
                }
            }
        }

        // Sort by timestamp (newest first)
        rollback_points.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(rollback_points)
    }

    /// Get rollback point by ID
    pub async fn get_rollback_point(&self, rollback_id: &str) -> Result<RollbackPoint> {
        let rollback_file = self.rollback_dir.join(format!("{}.json", rollback_id));
        self.load_rollback_point(&rollback_file).await
    }

    /// Find rollback point by transaction ID
    async fn find_rollback_point_by_transaction(
        &self,
        transaction_id: &str,
    ) -> Result<RollbackPoint> {
        let rollback_points = self.list_rollback_points().await?;

        rollback_points
            .into_iter()
            .find(|rp| rp.transaction_id == transaction_id)
            .ok_or_else(|| {
                NetworkError::System(SystemError::FileOperation {
                    path: format!("rollback point for transaction {}", transaction_id),
                })
            })
    }

    /// Load rollback point from file
    async fn load_rollback_point(&self, path: &Path) -> Result<RollbackPoint> {
        let content = fs::read_to_string(path).await?;
        let rollback_point: RollbackPoint = serde_json::from_str(&content)?;
        Ok(rollback_point)
    }

    /// Backup critical configuration files
    async fn backup_configuration_files(&self, rollback_id: &str) -> Result<Vec<BackupFile>> {
        let mut backed_up_files = Vec::new();

        // Files to backup
        let files_to_backup = vec![
            PathBuf::from("/etc/network/interfaces"),
            PathBuf::from("/etc/pve/sdn/zones.cfg"),
            PathBuf::from("/etc/pve/sdn/vnets.cfg"),
            PathBuf::from("/etc/pve/sdn/subnets.cfg"),
        ];

        for file_path in files_to_backup {
            if file_path.exists() {
                match self.backup_single_file(&file_path, rollback_id).await {
                    Ok(backup_file) => backed_up_files.push(backup_file),
                    Err(e) => warn!("Failed to backup file {:?}: {}", file_path, e),
                }
            }
        }

        Ok(backed_up_files)
    }

    /// Backup a single file
    async fn backup_single_file(&self, file_path: &Path, rollback_id: &str) -> Result<BackupFile> {
        let file_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        let backup_filename = format!("{}_{}", rollback_id, file_name);
        let backup_path = self.rollback_dir.join(backup_filename);

        // Copy file
        fs::copy(file_path, &backup_path).await?;

        // Calculate checksum
        let content = fs::read(&backup_path).await?;
        let checksum = format!("{:x}", md5::compute(&content));
        let size = content.len() as u64;

        Ok(BackupFile {
            original_path: file_path.to_path_buf(),
            backup_path,
            checksum,
            size,
        })
    }

    /// Restore a backed up file
    async fn restore_backup_file(&self, backup_file: &BackupFile) -> Result<()> {
        if !backup_file.backup_path.exists() {
            return Err(NetworkError::System(SystemError::FileOperation {
                path: format!("backup file {:?}", backup_file.backup_path),
            }));
        }

        // Verify backup file integrity
        let content = fs::read(&backup_file.backup_path).await?;
        let checksum = format!("{:x}", md5::compute(&content));

        if checksum != backup_file.checksum {
            return Err(NetworkError::System(SystemError::FileOperation {
                path: format!(
                    "backup file {:?} checksum mismatch",
                    backup_file.backup_path
                ),
            }));
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = backup_file.original_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await?;
            }
        }

        // Restore file
        fs::copy(&backup_file.backup_path, &backup_file.original_path).await?;

        debug!("Restored file {:?} from backup", backup_file.original_path);
        Ok(())
    }

    /// Clean up old rollback points
    async fn cleanup_old_rollback_points(&self) -> Result<()> {
        let rollback_points = self.list_rollback_points().await?;
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut cleaned_count = 0;

        // Remove rollback points that are too old or exceed the maximum count
        for (index, rollback_point) in rollback_points.iter().enumerate() {
            let should_remove =
                // Too old
                (current_time - rollback_point.timestamp) > self.max_age_seconds ||
                // Exceeds maximum count (keep newest ones)
                index >= self.max_rollback_points;

            if should_remove {
                if let Err(e) = self
                    .cleanup_rollback_point(&rollback_point.transaction_id)
                    .await
                {
                    warn!(
                        "Failed to cleanup old rollback point {}: {}",
                        rollback_point.id, e
                    );
                } else {
                    cleaned_count += 1;
                }
            }
        }

        if cleaned_count > 0 {
            info!("Cleaned up {} old rollback points", cleaned_count);
        }

        Ok(())
    }

    /// Generate unique rollback ID
    fn generate_rollback_id(&self, transaction_id: &str) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        format!("rb_{}_{}", transaction_id, timestamp)
    }

    /// Get rollback statistics
    pub async fn get_rollback_stats(&self) -> Result<RollbackStats> {
        let rollback_points = self.list_rollback_points().await?;
        let total_size = self.calculate_total_backup_size(&rollback_points).await?;

        Ok(RollbackStats {
            total_rollback_points: rollback_points.len(),
            total_backup_size_bytes: total_size,
            oldest_rollback_timestamp: rollback_points.last().map(|rp| rp.timestamp),
            newest_rollback_timestamp: rollback_points.first().map(|rp| rp.timestamp),
        })
    }

    /// Calculate total size of all backup files
    async fn calculate_total_backup_size(&self, rollback_points: &[RollbackPoint]) -> Result<u64> {
        let mut total_size = 0u64;

        for rollback_point in rollback_points {
            for backup_file in &rollback_point.backed_up_files {
                total_size += backup_file.size;
            }
        }

        Ok(total_size)
    }

    /// Create a placeholder RollbackManager for CLI testing
    pub fn placeholder() -> Self {
        use std::path::PathBuf;

        Self {
            config_manager: None,
            rollback_dir: PathBuf::from("/tmp/pve-network-rollback"),
            max_rollback_points: 10,
            max_age_seconds: 86400 * 30, // 30 days
        }
    }
}

/// Statistics about rollback points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackStats {
    /// Total number of rollback points
    pub total_rollback_points: usize,
    /// Total size of all backup files in bytes
    pub total_backup_size_bytes: u64,
    /// Timestamp of oldest rollback point
    pub oldest_rollback_timestamp: Option<u64>,
    /// Timestamp of newest rollback point
    pub newest_rollback_timestamp: Option<u64>,
}

impl Default for RollbackManager {
    fn default() -> Self {
        // This is a placeholder - in practice, RollbackManager should be created with new()
        panic!("RollbackManager must be created with new() method")
    }
}
