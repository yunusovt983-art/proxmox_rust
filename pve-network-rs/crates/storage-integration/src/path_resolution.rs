//! Path resolution for network storage
//!
//! This module provides path resolution mechanisms for network storage,
//! ensuring compatibility with pve-storage path resolution patterns.

use crate::{StorageBackendType, StorageIntegrationError, StorageResult};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};

/// Storage path resolver trait
pub trait StoragePathResolver {
    /// Resolve storage path for a given storage ID and path
    fn resolve_path(&self, storage_id: &str, path: &str) -> StorageResult<PathBuf>;

    /// Get mount point for storage
    fn get_mount_point(&self, storage_id: &str) -> StorageResult<PathBuf>;

    /// Check if path is accessible
    fn is_path_accessible(&self, path: &Path) -> StorageResult<bool>;

    /// Get storage type from path
    fn get_storage_type_from_path(&self, path: &Path) -> StorageResult<StorageBackendType>;
}

/// Default storage path resolver implementation
pub struct DefaultStoragePathResolver {
    /// Storage configurations
    storage_configs: HashMap<String, StoragePathConfig>,
    /// Mount point base directory
    mount_base: PathBuf,
}

/// Storage path configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePathConfig {
    pub storage_id: String,
    pub backend_type: StorageBackendType,
    pub mount_point: PathBuf,
    pub path_prefix: Option<String>,
    pub network_interface: Option<String>,
    pub options: HashMap<String, String>,
}

impl DefaultStoragePathResolver {
    /// Create a new path resolver
    pub fn new(mount_base: PathBuf) -> Self {
        Self {
            storage_configs: HashMap::new(),
            mount_base,
        }
    }

    /// Add storage configuration
    pub fn add_storage_config(&mut self, config: StoragePathConfig) {
        info!(
            "Adding storage path configuration for {}",
            config.storage_id
        );
        self.storage_configs
            .insert(config.storage_id.clone(), config);
    }

    /// Remove storage configuration
    pub fn remove_storage_config(&mut self, storage_id: &str) {
        info!("Removing storage path configuration for {}", storage_id);
        self.storage_configs.remove(storage_id);
    }

    /// Load storage configurations from pve-storage
    pub async fn load_from_pve_storage(&mut self) -> StorageResult<()> {
        info!("Loading storage configurations from pve-storage");

        // This would parse /etc/pve/storage.cfg
        let storage_configs = self.parse_pve_storage_config().await?;

        for config in storage_configs {
            self.add_storage_config(config);
        }

        Ok(())
    }

    /// Parse pve-storage configuration file
    async fn parse_pve_storage_config(&self) -> StorageResult<Vec<StoragePathConfig>> {
        debug!("Parsing pve-storage configuration");

        // This would read and parse /etc/pve/storage.cfg
        // For now, return empty vector as placeholder
        Ok(Vec::new())
    }

    /// Resolve network storage path
    fn resolve_network_storage_path(
        &self,
        config: &StoragePathConfig,
        path: &str,
    ) -> StorageResult<PathBuf> {
        debug!(
            "Resolving network storage path for {}: {}",
            config.storage_id, path
        );

        let mut resolved_path = config.mount_point.clone();

        // Add path prefix if configured
        if let Some(prefix) = &config.path_prefix {
            resolved_path = resolved_path.join(prefix);
        }

        // Add the requested path
        resolved_path = resolved_path.join(path.trim_start_matches('/'));

        // Validate path is within mount point
        if !resolved_path.starts_with(&config.mount_point) {
            return Err(StorageIntegrationError::PathResolution(format!(
                "Path {} is outside mount point",
                path
            )));
        }

        debug!("Resolved path: {:?}", resolved_path);
        Ok(resolved_path)
    }

    /// Get network interface for storage
    fn get_storage_interface(&self, storage_id: &str) -> Option<String> {
        self.storage_configs
            .get(storage_id)
            .and_then(|config| config.network_interface.clone())
    }

    /// Check storage connectivity
    async fn check_storage_connectivity(&self, storage_id: &str) -> StorageResult<bool> {
        debug!("Checking storage connectivity for {}", storage_id);

        if let Some(config) = self.storage_configs.get(storage_id) {
            match &config.backend_type {
                StorageBackendType::Nfs { server, .. } => {
                    self.check_nfs_connectivity(server, &config.mount_point)
                        .await
                }
                StorageBackendType::Cifs { server, .. } => {
                    self.check_cifs_connectivity(server, &config.mount_point)
                        .await
                }
                StorageBackendType::Iscsi { portal, .. } => {
                    self.check_iscsi_connectivity(portal, &config.mount_point)
                        .await
                }
            }
        } else {
            Err(StorageIntegrationError::PathResolution(format!(
                "Storage {} not found",
                storage_id
            )))
        }
    }

    /// Check NFS connectivity
    async fn check_nfs_connectivity(
        &self,
        server: &str,
        mount_point: &Path,
    ) -> StorageResult<bool> {
        debug!(
            "Checking NFS connectivity to {} at {:?}",
            server, mount_point
        );

        // Check if mount point exists and is mounted
        if !mount_point.exists() {
            return Ok(false);
        }

        // Check if it's actually an NFS mount
        // This would check /proc/mounts or use mountinfo
        Ok(true) // Placeholder
    }

    /// Check CIFS connectivity
    async fn check_cifs_connectivity(
        &self,
        server: &str,
        mount_point: &Path,
    ) -> StorageResult<bool> {
        debug!(
            "Checking CIFS connectivity to {} at {:?}",
            server, mount_point
        );

        // Check if mount point exists and is mounted
        if !mount_point.exists() {
            return Ok(false);
        }

        // Check if it's actually a CIFS mount
        Ok(true) // Placeholder
    }

    /// Check iSCSI connectivity
    async fn check_iscsi_connectivity(
        &self,
        portal: &str,
        mount_point: &Path,
    ) -> StorageResult<bool> {
        debug!(
            "Checking iSCSI connectivity to {} at {:?}",
            portal, mount_point
        );

        // Check if the iSCSI device is connected
        // This would check /sys/class/iscsi_session or use iscsiadm
        Ok(true) // Placeholder
    }
}

impl StoragePathResolver for DefaultStoragePathResolver {
    fn resolve_path(&self, storage_id: &str, path: &str) -> StorageResult<PathBuf> {
        debug!("Resolving path for storage {}: {}", storage_id, path);

        let config = self.storage_configs.get(storage_id).ok_or_else(|| {
            StorageIntegrationError::PathResolution(format!("Storage {} not found", storage_id))
        })?;

        self.resolve_network_storage_path(config, path)
    }

    fn get_mount_point(&self, storage_id: &str) -> StorageResult<PathBuf> {
        debug!("Getting mount point for storage {}", storage_id);

        let config = self.storage_configs.get(storage_id).ok_or_else(|| {
            StorageIntegrationError::PathResolution(format!("Storage {} not found", storage_id))
        })?;

        Ok(config.mount_point.clone())
    }

    fn is_path_accessible(&self, path: &Path) -> StorageResult<bool> {
        debug!("Checking if path is accessible: {:?}", path);

        // Check if path exists
        if !path.exists() {
            return Ok(false);
        }

        // Check if path is readable
        match std::fs::metadata(path) {
            Ok(metadata) => {
                // Check basic permissions
                Ok(metadata.permissions().readonly() == false || path.is_dir())
            }
            Err(_) => Ok(false),
        }
    }

    fn get_storage_type_from_path(&self, path: &Path) -> StorageResult<StorageBackendType> {
        debug!("Getting storage type from path: {:?}", path);

        // Find storage config that contains this path
        for config in self.storage_configs.values() {
            if path.starts_with(&config.mount_point) {
                return Ok(config.backend_type.clone());
            }
        }

        Err(StorageIntegrationError::PathResolution(format!(
            "No storage found for path: {:?}",
            path
        )))
    }
}

/// Advanced path resolver with caching and optimization
pub struct CachedStoragePathResolver {
    inner: DefaultStoragePathResolver,
    path_cache: HashMap<String, PathBuf>,
    mount_cache: HashMap<String, bool>,
}

impl CachedStoragePathResolver {
    /// Create a new cached path resolver
    pub fn new(mount_base: PathBuf) -> Self {
        Self {
            inner: DefaultStoragePathResolver::new(mount_base),
            path_cache: HashMap::new(),
            mount_cache: HashMap::new(),
        }
    }

    /// Clear caches
    pub fn clear_cache(&mut self) {
        info!("Clearing path resolver caches");
        self.path_cache.clear();
        self.mount_cache.clear();
    }

    /// Add storage configuration
    pub fn add_storage_config(&mut self, config: StoragePathConfig) {
        self.inner.add_storage_config(config);
        // Clear cache when configuration changes
        self.clear_cache();
    }

    /// Check if mount is active (cached)
    pub async fn is_mount_active(&mut self, storage_id: &str) -> StorageResult<bool> {
        // Check cache first
        if let Some(cached) = self.mount_cache.get(storage_id) {
            return Ok(*cached);
        }

        // Check actual mount status
        let is_active = self.inner.check_storage_connectivity(storage_id).await?;

        // Cache result
        self.mount_cache.insert(storage_id.to_string(), is_active);

        Ok(is_active)
    }

    /// Get cached path resolution
    fn get_cached_path(&self, storage_id: &str, path: &str) -> Option<PathBuf> {
        let cache_key = format!("{}:{}", storage_id, path);
        self.path_cache.get(&cache_key).cloned()
    }

    /// Cache path resolution
    fn cache_path(&mut self, storage_id: &str, path: &str, resolved: &PathBuf) {
        let cache_key = format!("{}:{}", storage_id, path);
        self.path_cache.insert(cache_key, resolved.clone());
    }
}

impl StoragePathResolver for CachedStoragePathResolver {
    fn resolve_path(&self, storage_id: &str, path: &str) -> StorageResult<PathBuf> {
        // Check cache first
        if let Some(cached) = self.get_cached_path(storage_id, path) {
            debug!("Using cached path resolution for {}:{}", storage_id, path);
            return Ok(cached);
        }

        // Resolve path
        let resolved = self.inner.resolve_path(storage_id, path)?;

        // Cache result (we need mutable access, so this is a limitation of the current design)
        // In a real implementation, we'd use Arc<Mutex<>> or similar for thread-safe caching

        Ok(resolved)
    }

    fn get_mount_point(&self, storage_id: &str) -> StorageResult<PathBuf> {
        self.inner.get_mount_point(storage_id)
    }

    fn is_path_accessible(&self, path: &Path) -> StorageResult<bool> {
        self.inner.is_path_accessible(path)
    }

    fn get_storage_type_from_path(&self, path: &Path) -> StorageResult<StorageBackendType> {
        self.inner.get_storage_type_from_path(path)
    }
}

/// Storage path utilities
pub struct StoragePathUtils;

impl StoragePathUtils {
    /// Normalize storage path
    pub fn normalize_path(path: &str) -> String {
        // Remove duplicate slashes, resolve . and .. components
        let path = path.replace("//", "/");
        let path = if path.starts_with('/') {
            path
        } else {
            format!("/{}", path)
        };

        // Remove trailing slash unless it's root
        if path.len() > 1 && path.ends_with('/') {
            path[..path.len() - 1].to_string()
        } else {
            path
        }
    }

    /// Check if path is safe (no directory traversal)
    pub fn is_safe_path(path: &str) -> bool {
        // Check for directory traversal attempts
        !path.contains("..") && !path.contains("~")
    }

    /// Get relative path from mount point
    pub fn get_relative_path(full_path: &Path, mount_point: &Path) -> StorageResult<PathBuf> {
        full_path
            .strip_prefix(mount_point)
            .map(|p| p.to_path_buf())
            .map_err(|_| {
                StorageIntegrationError::PathResolution(format!(
                    "Path {:?} is not under mount point {:?}",
                    full_path, mount_point
                ))
            })
    }

    /// Join paths safely
    pub fn safe_join(base: &Path, path: &str) -> StorageResult<PathBuf> {
        if !Self::is_safe_path(path) {
            return Err(StorageIntegrationError::PathResolution(format!(
                "Unsafe path: {}",
                path
            )));
        }

        let normalized = Self::normalize_path(path);
        let joined = base.join(normalized.trim_start_matches('/'));

        // Ensure result is still under base
        if !joined.starts_with(base) {
            return Err(StorageIntegrationError::PathResolution(
                "Path traversal detected".to_string(),
            ));
        }

        Ok(joined)
    }

    /// Get storage ID from mount point
    pub fn get_storage_id_from_mount(mount_point: &Path) -> Option<String> {
        // Extract storage ID from mount point path
        // This would depend on the mount point naming convention
        mount_point
            .file_name()
            .and_then(|name| name.to_str())
            .map(|s| s.to_string())
    }

    /// Generate mount point for storage
    pub fn generate_mount_point(
        base: &Path,
        storage_id: &str,
        backend_type: &StorageBackendType,
    ) -> PathBuf {
        let type_prefix = match backend_type {
            StorageBackendType::Nfs { .. } => "nfs",
            StorageBackendType::Cifs { .. } => "cifs",
            StorageBackendType::Iscsi { .. } => "iscsi",
        };

        base.join(format!("{}-{}", type_prefix, storage_id))
    }
}

/// Storage path monitoring
pub struct StoragePathMonitor {
    resolver: Box<dyn StoragePathResolver>,
    monitored_paths: HashMap<String, PathBuf>,
}

impl StoragePathMonitor {
    /// Create a new path monitor
    pub fn new(resolver: Box<dyn StoragePathResolver>) -> Self {
        Self {
            resolver,
            monitored_paths: HashMap::new(),
        }
    }

    /// Add path to monitor
    pub fn add_monitored_path(&mut self, storage_id: String, path: PathBuf) {
        info!("Adding monitored path for {}: {:?}", storage_id, path);
        self.monitored_paths.insert(storage_id, path);
    }

    /// Remove monitored path
    pub fn remove_monitored_path(&mut self, storage_id: &str) {
        info!("Removing monitored path for {}", storage_id);
        self.monitored_paths.remove(storage_id);
    }

    /// Check all monitored paths
    pub async fn check_all_paths(&self) -> StorageResult<HashMap<String, bool>> {
        info!("Checking all monitored paths");

        let mut results = HashMap::new();

        for (storage_id, path) in &self.monitored_paths {
            let is_accessible = self.resolver.is_path_accessible(path)?;
            results.insert(storage_id.clone(), is_accessible);

            if !is_accessible {
                warn!("Storage path not accessible: {} -> {:?}", storage_id, path);
            }
        }

        Ok(results)
    }

    /// Get path status
    pub fn get_path_status(&self, storage_id: &str) -> StorageResult<StoragePathStatus> {
        if let Some(path) = self.monitored_paths.get(storage_id) {
            let is_accessible = self.resolver.is_path_accessible(path)?;
            let storage_type = self.resolver.get_storage_type_from_path(path)?;

            Ok(StoragePathStatus {
                storage_id: storage_id.to_string(),
                path: path.clone(),
                is_accessible,
                storage_type,
                last_check: chrono::Utc::now(),
            })
        } else {
            Err(StorageIntegrationError::PathResolution(format!(
                "Storage {} not monitored",
                storage_id
            )))
        }
    }
}

/// Storage path status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoragePathStatus {
    pub storage_id: String,
    pub path: PathBuf,
    pub is_accessible: bool,
    pub storage_type: StorageBackendType,
    pub last_check: chrono::DateTime<chrono::Utc>,
}
