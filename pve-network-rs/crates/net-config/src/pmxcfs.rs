//! pmxcfs integration for cluster configuration management

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::sync::{Mutex, RwLock};

use crate::sdn_config::SdnConfiguration;

/// pmxcfs configuration paths
pub const PMXCFS_BASE_PATH: &str = "/etc/pve";
pub const SDN_CONFIG_PATH: &str = "/etc/pve/sdn";
pub const NETWORK_CONFIG_PATH: &str = "/etc/pve/nodes";
pub const CLUSTER_LOCK_PATH: &str = "/etc/pve/.locks";

/// Lock timeout in seconds
pub const LOCK_TIMEOUT: Duration = Duration::from_secs(30);

/// Lock information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    pub node: String,
    pub pid: u32,
    pub timestamp: u64,
    pub operation: String,
}

/// Cluster lock manager for preventing concurrent modifications
#[derive(Debug)]
pub struct ClusterLock {
    path: PathBuf,
    lock_info: LockInfo,
    _lock_file: Option<tokio::fs::File>,
}

impl ClusterLock {
    /// Create a new cluster lock
    pub async fn acquire(lock_name: &str, node: &str, operation: &str) -> Result<Self> {
        Self::acquire_with_base_path(lock_name, node, operation, CLUSTER_LOCK_PATH).await
    }

    /// Create a new cluster lock with custom base path
    pub async fn acquire_with_base_path(
        lock_name: &str,
        node: &str,
        operation: &str,
        lock_base_path: &str,
    ) -> Result<Self> {
        let lock_path = PathBuf::from(lock_base_path).join(format!("{}.lock", lock_name));

        // Ensure lock directory exists
        if let Some(parent) = lock_path.parent() {
            fs::create_dir_all(parent)
                .await
                .context("Failed to create lock directory")?;
        }

        let lock_info = LockInfo {
            node: node.to_string(),
            pid: std::process::id(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            operation: operation.to_string(),
        };

        // Try to acquire lock with timeout
        let start_time = SystemTime::now();
        loop {
            match Self::try_acquire_lock(&lock_path, &lock_info).await {
                Ok(lock_file) => {
                    return Ok(ClusterLock {
                        path: lock_path,
                        lock_info,
                        _lock_file: Some(lock_file),
                    });
                }
                Err(_) => {
                    if start_time.elapsed().unwrap() > LOCK_TIMEOUT {
                        bail!("Failed to acquire lock '{}' within timeout", lock_name);
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    async fn try_acquire_lock(lock_path: &Path, lock_info: &LockInfo) -> Result<tokio::fs::File> {
        // Check if lock file exists and is still valid
        if lock_path.exists() {
            if let Ok(existing_content) = fs::read_to_string(lock_path).await {
                if let Ok(existing_lock) = serde_json::from_str::<LockInfo>(&existing_content) {
                    let current_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();

                    // Check if lock is still valid (not expired)
                    if current_time - existing_lock.timestamp < LOCK_TIMEOUT.as_secs() {
                        // Check if the process is still running
                        if Self::is_process_running(&existing_lock.node, existing_lock.pid).await {
                            bail!(
                                "Lock is held by {}:{} for operation '{}'",
                                existing_lock.node,
                                existing_lock.pid,
                                existing_lock.operation
                            );
                        }
                    }
                }
            }
            // Remove stale lock
            let _ = fs::remove_file(lock_path).await;
        }

        // Create new lock file
        let lock_content = serde_json::to_string_pretty(lock_info)?;
        fs::write(lock_path, lock_content)
            .await
            .context("Failed to write lock file")?;

        // Open file handle to keep lock
        let lock_file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(lock_path)
            .await
            .context("Failed to open lock file")?;

        Ok(lock_file)
    }

    async fn is_process_running(_node: &str, pid: u32) -> bool {
        // In a real implementation, this would check if the process is running
        // on the specified node. For now, we'll assume local node only.
        let proc_path = format!("/proc/{}", pid);
        Path::new(&proc_path).exists()
    }

    /// Get lock information
    pub fn lock_info(&self) -> &LockInfo {
        &self.lock_info
    }
}

impl Drop for ClusterLock {
    fn drop(&mut self) {
        // Remove lock file when dropped
        let path = self.path.clone();
        tokio::spawn(async move {
            let _ = fs::remove_file(path).await;
        });
    }
}

/// pmxcfs configuration manager with cluster synchronization
#[derive(Clone)]
pub struct PmxcfsConfig {
    base_path: PathBuf,
    node_name: String,
    active_locks: Arc<Mutex<HashMap<String, Arc<ClusterLock>>>>,
    config_cache: Arc<RwLock<HashMap<String, (String, SystemTime)>>>,
}

impl PmxcfsConfig {
    /// Create new pmxcfs config manager
    pub fn new() -> Result<Self> {
        let node_name = Self::get_node_name()?;

        Ok(Self {
            base_path: PathBuf::from(PMXCFS_BASE_PATH),
            node_name,
            active_locks: Arc::new(Mutex::new(HashMap::new())),
            config_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create pmxcfs config manager with custom base path (for testing)
    pub fn with_base_path<P: AsRef<Path>>(base_path: P) -> Result<Self> {
        let node_name = Self::get_node_name()?;

        Ok(Self {
            base_path: base_path.as_ref().to_path_buf(),
            node_name,
            active_locks: Arc::new(Mutex::new(HashMap::new())),
            config_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create mock pmxcfs config manager for testing
    pub fn mock() -> Self {
        Self {
            base_path: PathBuf::from("/tmp/pve-network-test"),
            node_name: "test-node".to_string(),
            active_locks: Arc::new(Mutex::new(HashMap::new())),
            config_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn get_node_name() -> Result<String> {
        // In a real implementation, this would read from /etc/hostname or pmxcfs
        // For now, use hostname command
        std::process::Command::new("hostname")
            .output()
            .context("Failed to get hostname")
            .and_then(|output| {
                String::from_utf8(output.stdout)
                    .context("Invalid hostname encoding")
                    .map(|s| s.trim().to_string())
            })
    }

    /// Acquire a cluster lock for configuration modification
    pub async fn acquire_lock(&self, lock_name: &str, operation: &str) -> Result<Arc<ClusterLock>> {
        let lock_base_path = self.base_path.join(".locks");
        let lock_base_path_str = lock_base_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid lock path"))?;

        let lock = Arc::new(
            ClusterLock::acquire_with_base_path(
                lock_name,
                &self.node_name,
                operation,
                lock_base_path_str,
            )
            .await?,
        );

        let mut active_locks = self.active_locks.lock().await;
        active_locks.insert(lock_name.to_string(), lock.clone());

        Ok(lock)
    }

    /// Execute a function with a cluster lock
    pub async fn with_lock<F, R>(&self, lock_name: &str, operation: &str, f: F) -> Result<R>
    where
        F: FnOnce() -> Result<R> + Send + 'static,
        R: Send + 'static,
    {
        let _lock = self.acquire_lock(lock_name, operation).await?;

        // Execute the function
        let result = tokio::task::spawn_blocking(f)
            .await
            .context("Task execution failed")?;

        // Lock is automatically released when _lock is dropped
        result
    }

    /// Read SDN configuration from pmxcfs
    pub async fn read_sdn_config(&self) -> Result<SdnConfiguration> {
        let config_path = self.base_path.join("sdn");

        // Check cache first
        let cache_key = "sdn_config".to_string();
        {
            let cache = self.config_cache.read().await;
            if let Some((cached_content, cached_time)) = cache.get(&cache_key) {
                // Check if cache is still valid (1 second)
                if cached_time.elapsed().unwrap_or(Duration::from_secs(2)) < Duration::from_secs(1)
                {
                    return serde_json::from_str(cached_content)
                        .context("Failed to deserialize cached SDN configuration");
                }
            }
        }

        let mut config = SdnConfiguration::default();

        // Read zones
        let zones_path = config_path.join("zones");
        if zones_path.exists() {
            let mut zones_dir = fs::read_dir(&zones_path)
                .await
                .context("Failed to read zones directory")?;

            while let Some(entry) = zones_dir.next_entry().await? {
                if entry.file_type().await?.is_file() {
                    let content = fs::read_to_string(entry.path())
                        .await
                        .context("Failed to read zone file")?;

                    // Parse zone configuration (simplified)
                    if let Some(zone_name) = entry.file_name().to_str() {
                        if let Ok(zone_config) = serde_json::from_str(&content) {
                            config.zones.insert(zone_name.to_string(), zone_config);
                        }
                    }
                }
            }
        }

        // Read vnets
        let vnets_path = config_path.join("vnets");
        if vnets_path.exists() {
            let mut vnets_dir = fs::read_dir(&vnets_path)
                .await
                .context("Failed to read vnets directory")?;

            while let Some(entry) = vnets_dir.next_entry().await? {
                if entry.file_type().await?.is_file() {
                    let content = fs::read_to_string(entry.path())
                        .await
                        .context("Failed to read vnet file")?;

                    if let Some(vnet_name) = entry.file_name().to_str() {
                        if let Ok(vnet_config) = serde_json::from_str(&content) {
                            config.vnets.insert(vnet_name.to_string(), vnet_config);
                        }
                    }
                }
            }
        }

        // Cache the result
        let config_json = serde_json::to_string(&config)?;
        {
            let mut cache = self.config_cache.write().await;
            cache.insert(cache_key, (config_json, SystemTime::now()));
        }

        Ok(config)
    }

    /// Write SDN configuration to pmxcfs
    pub async fn write_sdn_config(&self, config: &SdnConfiguration) -> Result<()> {
        let config_path = self.base_path.join("sdn");

        // Ensure directories exist
        fs::create_dir_all(&config_path)
            .await
            .context("Failed to create SDN config directory")?;

        let zones_path = config_path.join("zones");
        let vnets_path = config_path.join("vnets");
        let subnets_path = config_path.join("subnets");

        fs::create_dir_all(&zones_path)
            .await
            .context("Failed to create zones directory")?;
        fs::create_dir_all(&vnets_path)
            .await
            .context("Failed to create vnets directory")?;
        fs::create_dir_all(&subnets_path)
            .await
            .context("Failed to create subnets directory")?;

        // Write zones
        for (zone_name, zone_config) in &config.zones {
            let zone_file = zones_path.join(zone_name);
            let zone_content = serde_json::to_string_pretty(zone_config)?;
            fs::write(zone_file, zone_content)
                .await
                .context("Failed to write zone configuration")?;
        }

        // Write vnets
        for (vnet_name, vnet_config) in &config.vnets {
            let vnet_file = vnets_path.join(vnet_name);
            let vnet_content = serde_json::to_string_pretty(vnet_config)?;
            fs::write(vnet_file, vnet_content)
                .await
                .context("Failed to write vnet configuration")?;
        }

        // Write subnets
        for (subnet_name, subnet_config) in &config.subnets {
            let subnet_file = subnets_path.join(subnet_name);
            let subnet_content = serde_json::to_string_pretty(subnet_config)?;
            fs::write(subnet_file, subnet_content)
                .await
                .context("Failed to write subnet configuration")?;
        }

        // Invalidate cache
        {
            let mut cache = self.config_cache.write().await;
            cache.remove("sdn_config");
        }

        // Trigger cluster synchronization
        self.trigger_cluster_sync("sdn").await?;

        Ok(())
    }

    /// Read network configuration for a specific node
    pub async fn read_node_network_config(&self, node: &str) -> Result<String> {
        let config_path = self.base_path.join("nodes").join(node).join("network");

        fs::read_to_string(config_path)
            .await
            .context("Failed to read node network configuration")
    }

    /// Write network configuration for a specific node
    pub async fn write_node_network_config(&self, node: &str, config: &str) -> Result<()> {
        let config_dir = self.base_path.join("nodes").join(node);
        let config_path = config_dir.join("network");

        // Ensure directory exists
        fs::create_dir_all(&config_dir)
            .await
            .context("Failed to create node config directory")?;

        fs::write(config_path, config)
            .await
            .context("Failed to write node network configuration")?;

        // Trigger cluster synchronization
        self.trigger_cluster_sync(&format!("nodes/{}/network", node))
            .await?;

        Ok(())
    }

    /// Trigger cluster synchronization for a configuration path
    async fn trigger_cluster_sync(&self, config_path: &str) -> Result<()> {
        // In a real implementation, this would trigger pmxcfs synchronization
        // For now, we'll just log the sync request
        log::info!("Triggering cluster sync for path: {}", config_path);

        // Simulate sync delay
        tokio::time::sleep(Duration::from_millis(10)).await;

        Ok(())
    }

    /// Get cluster nodes
    pub async fn get_cluster_nodes(&self) -> Result<Vec<String>> {
        let nodes_path = self.base_path.join("nodes");
        let mut nodes = Vec::new();

        if nodes_path.exists() {
            let mut nodes_dir = fs::read_dir(&nodes_path)
                .await
                .context("Failed to read nodes directory")?;

            while let Some(entry) = nodes_dir.next_entry().await? {
                if entry.file_type().await?.is_dir() {
                    if let Some(node_name) = entry.file_name().to_str() {
                        nodes.push(node_name.to_string());
                    }
                }
            }
        }

        Ok(nodes)
    }

    /// Check if configuration is synchronized across cluster
    pub async fn verify_cluster_sync(&self, config_path: &str) -> Result<bool> {
        let nodes = self.get_cluster_nodes().await?;

        if nodes.len() <= 1 {
            return Ok(true); // Single node cluster is always synchronized
        }

        // In a real implementation, this would check configuration consistency
        // across all cluster nodes. For now, we'll simulate the check.
        log::info!(
            "Verifying cluster sync for {} across {} nodes",
            config_path,
            nodes.len()
        );

        // Simulate verification delay
        tokio::time::sleep(Duration::from_millis(50)).await;

        Ok(true)
    }

    /// Get current node name
    pub fn node_name(&self) -> &str {
        &self.node_name
    }

    /// Synchronize configuration with cluster
    pub async fn sync_configuration(&self) -> Result<()> {
        log::info!("Synchronizing configuration with cluster");

        // In a real implementation, this would trigger pmxcfs synchronization
        // For now, we'll simulate the sync process
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(())
    }
}

impl Default for PmxcfsConfig {
    fn default() -> Self {
        Self::new().expect("Failed to create default PmxcfsConfig")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cluster_lock_acquire_release() {
        let temp_dir = TempDir::new().unwrap();
        let lock_path = temp_dir.path().join(".locks");
        std::fs::create_dir_all(&lock_path).unwrap();

        let lock = ClusterLock::acquire_with_base_path(
            "test_lock",
            "test_node",
            "test_operation",
            lock_path.to_str().unwrap(),
        )
        .await;
        assert!(lock.is_ok());

        let lock = lock.unwrap();
        assert_eq!(lock.lock_info().node, "test_node");
        assert_eq!(lock.lock_info().operation, "test_operation");

        // Lock should be released when dropped
        drop(lock);
    }

    #[tokio::test]
    async fn test_pmxcfs_config_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = PmxcfsConfig::with_base_path(temp_dir.path()).unwrap();

        assert!(!config.node_name().is_empty());
    }

    #[tokio::test]
    async fn test_sdn_config_read_write() {
        let temp_dir = TempDir::new().unwrap();
        let config = PmxcfsConfig::with_base_path(temp_dir.path()).unwrap();

        let sdn_config = SdnConfiguration::default();
        // Add test data would go here

        let result = config.write_sdn_config(&sdn_config).await;
        assert!(result.is_ok());

        let read_config = config.read_sdn_config().await;
        assert!(read_config.is_ok());
    }

    #[tokio::test]
    async fn test_with_lock() {
        let temp_dir = TempDir::new().unwrap();
        let config = PmxcfsConfig::with_base_path(temp_dir.path()).unwrap();

        let result = config
            .with_lock("test_lock", "test_operation", || {
                Ok("test_result".to_string())
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test_result");
    }
}
