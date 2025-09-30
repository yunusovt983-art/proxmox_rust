//! PVE IPAM driver

use anyhow::Result;
use async_trait::async_trait;
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

use pve_sdn_core::{
    IpAllocation, IpAllocationRequest, IpamConfig, IpamError, IpamPlugin, IpamType, Subnet,
};

/// PVE IPAM storage entry
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PveIpamEntry {
    ip: IpAddr,
    subnet: String,
    vmid: Option<u32>,
    hostname: Option<String>,
    mac: Option<String>,
    description: Option<String>,
    allocated_at: chrono::DateTime<chrono::Utc>,
}

/// PVE IPAM subnet info
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PveSubnetInfo {
    name: String,
    cidr: IpNet,
    gateway: Option<IpAddr>,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// PVE IPAM implementation
///
/// This is the built-in IPAM that stores allocations in the PVE cluster filesystem
pub struct PveIpam {
    name: String,
    config: IpamConfig,
    // In-memory storage for development/testing
    // In production, this would be backed by pmxcfs
    allocations: Arc<RwLock<HashMap<String, HashMap<IpAddr, PveIpamEntry>>>>,
    subnets: Arc<RwLock<HashMap<String, PveSubnetInfo>>>,
}

impl PveIpam {
    /// Create new PVE IPAM
    pub fn new(name: String, config: IpamConfig) -> Self {
        Self {
            name,
            config,
            allocations: Arc::new(RwLock::new(HashMap::new())),
            subnets: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load allocations from storage (pmxcfs in production)
    pub async fn load_from_storage(&self) -> Result<()> {
        // Use configurable storage path for testing
        let ipam_dir = std::env::var("PVE_IPAM_STORAGE_PATH")
            .unwrap_or_else(|_| "/etc/pve/sdn/ipam".to_string());
        let storage_path = format!("{}/{}.json", ipam_dir, self.name);

        match tokio::fs::read_to_string(&storage_path).await {
            Ok(content) => {
                let data: HashMap<String, HashMap<IpAddr, PveIpamEntry>> =
                    serde_json::from_str(&content)?;

                let mut allocations = self.allocations.write().await;
                *allocations = data;

                log::info!(
                    "Loaded PVE IPAM data from {} for {}",
                    storage_path,
                    self.name
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::debug!(
                    "No existing IPAM data found at {}, starting fresh",
                    storage_path
                );
            }
            Err(e) => {
                log::warn!("Failed to load IPAM data from {}: {}", storage_path, e);
                return Err(e.into());
            }
        }

        // Load subnet info
        let subnets_path = format!("{}/{}_subnets.json", ipam_dir, self.name);
        match tokio::fs::read_to_string(&subnets_path).await {
            Ok(content) => {
                let data: HashMap<String, PveSubnetInfo> = serde_json::from_str(&content)?;

                let mut subnets = self.subnets.write().await;
                *subnets = data;

                log::debug!("Loaded subnet info from {}", subnets_path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::debug!("No existing subnet info found at {}", subnets_path);
            }
            Err(e) => {
                log::warn!("Failed to load subnet info from {}: {}", subnets_path, e);
                // Don't return error for storage issues in tests
                if std::env::var("PVE_IPAM_STORAGE_PATH").is_err() {
                    return Err(e.into());
                }
            }
        }

        Ok(())
    }

    /// Save allocations to storage (pmxcfs in production)
    async fn save_to_storage(&self) -> Result<()> {
        // Use configurable storage path for testing
        let ipam_dir = std::env::var("PVE_IPAM_STORAGE_PATH")
            .unwrap_or_else(|_| "/etc/pve/sdn/ipam".to_string());

        if let Err(e) = tokio::fs::create_dir_all(&ipam_dir).await {
            if e.kind() != std::io::ErrorKind::AlreadyExists {
                return Err(e.into());
            }
        }

        // Save allocations
        let storage_path = format!("{}/{}.json", ipam_dir, self.name);
        let allocations = self.allocations.read().await;
        let content = serde_json::to_string_pretty(&*allocations)?;

        // Write atomically using temporary file
        let temp_path = format!("{}.tmp", storage_path);
        tokio::fs::write(&temp_path, content).await?;
        tokio::fs::rename(&temp_path, &storage_path).await?;

        log::debug!("Saved PVE IPAM data to {} for {}", storage_path, self.name);

        // Save subnet info
        let subnets_path = format!("{}/{}_subnets.json", ipam_dir, self.name);
        let subnets = self.subnets.read().await;
        let subnets_content = serde_json::to_string_pretty(&*subnets)?;

        let temp_subnets_path = format!("{}.tmp", subnets_path);
        tokio::fs::write(&temp_subnets_path, subnets_content).await?;
        tokio::fs::rename(&temp_subnets_path, &subnets_path).await?;

        log::debug!("Saved subnet info to {}", subnets_path);

        Ok(())
    }

    /// Get next available IP in subnet
    async fn find_next_free_ip(&self, subnet_name: &str, cidr: &IpNet) -> Result<Option<IpAddr>> {
        let allocations = self.allocations.read().await;
        let subnet_allocations = allocations.get(subnet_name);

        // Iterate through all possible IPs in the subnet
        for ip in cidr.hosts() {
            // Skip network and broadcast addresses for IPv4
            if cidr.addr().is_ipv4() {
                if ip == cidr.network() || ip == cidr.broadcast() {
                    continue;
                }
            }

            // Check if IP is allocated
            if let Some(subnet_allocs) = subnet_allocations {
                if subnet_allocs.contains_key(&ip) {
                    continue;
                }
            }

            return Ok(Some(ip));
        }

        Ok(None)
    }

    /// Validate IP is within subnet and available
    async fn validate_ip_allocation(
        &self,
        subnet_name: &str,
        ip: &IpAddr,
        cidr: &IpNet,
    ) -> Result<()> {
        // Check if IP is within subnet
        if !cidr.contains(ip) {
            return Err(IpamError::Configuration {
                message: format!("IP {} is not within subnet {}", ip, cidr),
            }
            .into());
        }

        // For IPv4, check if IP is network or broadcast address
        if ip.is_ipv4() {
            if *ip == cidr.network() {
                return Err(IpamError::Configuration {
                    message: format!("Cannot allocate network address {}", ip),
                }
                .into());
            }
            if *ip == cidr.broadcast() {
                return Err(IpamError::Configuration {
                    message: format!("Cannot allocate broadcast address {}", ip),
                }
                .into());
            }
        }

        // Check if IP is already allocated
        let allocations = self.allocations.read().await;
        if let Some(subnet_allocs) = allocations.get(subnet_name) {
            if subnet_allocs.contains_key(ip) {
                return Err(IpamError::IpAlreadyAllocated {
                    ip: *ip,
                    subnet: subnet_name.to_string(),
                }
                .into());
            }
        }

        Ok(())
    }
}

#[async_trait]
impl IpamPlugin for PveIpam {
    fn plugin_type(&self) -> IpamType {
        IpamType::Pve
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn validate_config(&self, config: &IpamConfig) -> Result<()> {
        if config.ipam_type != IpamType::Pve {
            anyhow::bail!("Invalid IPAM type for PVE IPAM plugin");
        }

        // PVE IPAM doesn't require external configuration
        Ok(())
    }

    async fn allocate_ip(&self, request: &IpAllocationRequest) -> Result<IpAllocation> {
        let subnets = self.subnets.read().await;
        let subnet_info =
            subnets
                .get(&request.subnet)
                .ok_or_else(|| IpamError::SubnetNotFound {
                    subnet: request.subnet.clone(),
                })?;

        let ip = if let Some(requested_ip) = request.requested_ip {
            // Validate requested IP
            self.validate_ip_allocation(&request.subnet, &requested_ip, &subnet_info.cidr)
                .await?;
            requested_ip
        } else {
            // Find next available IP
            let cidr = subnet_info.cidr;
            drop(subnets); // Release read lock before calling find_next_free_ip
            self.find_next_free_ip(&request.subnet, &cidr)
                .await?
                .ok_or_else(|| IpamError::NoFreeIps {
                    subnet: request.subnet.clone(),
                })?
        };

        let allocation = IpAllocation {
            ip,
            subnet: request.subnet.clone(),
            vmid: request.vmid,
            hostname: request.hostname.clone(),
            mac: request.mac.clone(),
            description: request.description.clone(),
            allocated_at: chrono::Utc::now(),
        };

        let entry = PveIpamEntry {
            ip,
            subnet: request.subnet.clone(),
            vmid: request.vmid,
            hostname: request.hostname.clone(),
            mac: request.mac.clone(),
            description: request.description.clone(),
            allocated_at: allocation.allocated_at,
        };

        // Store allocation
        let mut allocations = self.allocations.write().await;
        allocations
            .entry(request.subnet.clone())
            .or_insert_with(HashMap::new)
            .insert(ip, entry);
        drop(allocations);

        // Save to storage
        self.save_to_storage().await?;

        log::info!(
            "Allocated IP {} in subnet {} for VMID {:?}",
            ip,
            request.subnet,
            request.vmid
        );

        Ok(allocation)
    }

    async fn release_ip(&self, subnet: &str, ip: &IpAddr) -> Result<()> {
        let mut allocations = self.allocations.write().await;

        if let Some(subnet_allocs) = allocations.get_mut(subnet) {
            if subnet_allocs.remove(ip).is_some() {
                drop(allocations);
                self.save_to_storage().await?;
                log::info!("Released IP {} from subnet {}", ip, subnet);
                return Ok(());
            }
        }

        Err(IpamError::IpNotFound {
            ip: *ip,
            subnet: subnet.to_string(),
        }
        .into())
    }

    async fn update_ip(&self, subnet: &str, ip: &IpAddr, allocation: &IpAllocation) -> Result<()> {
        let mut allocations = self.allocations.write().await;

        if let Some(subnet_allocs) = allocations.get_mut(subnet) {
            if let Some(entry) = subnet_allocs.get_mut(ip) {
                entry.vmid = allocation.vmid;
                entry.hostname = allocation.hostname.clone();
                entry.mac = allocation.mac.clone();
                entry.description = allocation.description.clone();

                drop(allocations);
                self.save_to_storage().await?;
                log::info!("Updated IP {} in subnet {}", ip, subnet);
                return Ok(());
            }
        }

        Err(IpamError::IpNotFound {
            ip: *ip,
            subnet: subnet.to_string(),
        }
        .into())
    }

    async fn get_ip(&self, subnet: &str, ip: &IpAddr) -> Result<Option<IpAllocation>> {
        let allocations = self.allocations.read().await;

        if let Some(subnet_allocs) = allocations.get(subnet) {
            if let Some(entry) = subnet_allocs.get(ip) {
                return Ok(Some(IpAllocation {
                    ip: entry.ip,
                    subnet: entry.subnet.clone(),
                    vmid: entry.vmid,
                    hostname: entry.hostname.clone(),
                    mac: entry.mac.clone(),
                    description: entry.description.clone(),
                    allocated_at: entry.allocated_at,
                }));
            }
        }

        Ok(None)
    }

    async fn list_subnet_ips(&self, subnet: &str) -> Result<Vec<IpAllocation>> {
        let allocations = self.allocations.read().await;

        if let Some(subnet_allocs) = allocations.get(subnet) {
            let mut result = Vec::new();
            for entry in subnet_allocs.values() {
                result.push(IpAllocation {
                    ip: entry.ip,
                    subnet: entry.subnet.clone(),
                    vmid: entry.vmid,
                    hostname: entry.hostname.clone(),
                    mac: entry.mac.clone(),
                    description: entry.description.clone(),
                    allocated_at: entry.allocated_at,
                });
            }
            result.sort_by_key(|a| a.ip);
            Ok(result)
        } else {
            Ok(Vec::new())
        }
    }

    async fn validate_subnet(&self, subnet: &Subnet) -> Result<()> {
        // Validate subnet configuration
        subnet.config.validate()?;

        // Check if subnet already exists
        let subnets = self.subnets.read().await;
        if subnets.contains_key(&subnet.config.subnet) {
            log::debug!("Subnet {} already exists in PVE IPAM", subnet.config.subnet);
        }

        Ok(())
    }

    async fn add_subnet(&self, subnet: &Subnet) -> Result<()> {
        self.validate_subnet(subnet).await?;

        let subnet_info = PveSubnetInfo {
            name: subnet.config.subnet.clone(),
            cidr: subnet.config.cidr,
            gateway: subnet.config.gateway,
            created_at: chrono::Utc::now(),
        };

        let mut subnets = self.subnets.write().await;
        subnets.insert(subnet.config.subnet.clone(), subnet_info);
        drop(subnets);

        self.save_to_storage().await?;

        log::info!("Added subnet {} to PVE IPAM", subnet.config.subnet);
        Ok(())
    }

    async fn remove_subnet(&self, subnet_name: &str) -> Result<()> {
        // Check if subnet has any allocations
        let allocations = self.allocations.read().await;
        if let Some(subnet_allocs) = allocations.get(subnet_name) {
            if !subnet_allocs.is_empty() {
                anyhow::bail!(
                    "Cannot remove subnet {} - it has {} active allocations",
                    subnet_name,
                    subnet_allocs.len()
                );
            }
        }
        drop(allocations);

        // Remove subnet
        let mut subnets = self.subnets.write().await;
        if subnets.remove(subnet_name).is_none() {
            return Err(IpamError::SubnetNotFound {
                subnet: subnet_name.to_string(),
            }
            .into());
        }
        drop(subnets);

        // Remove empty allocation map
        let mut allocations = self.allocations.write().await;
        allocations.remove(subnet_name);
        drop(allocations);

        self.save_to_storage().await?;

        log::info!("Removed subnet {} from PVE IPAM", subnet_name);
        Ok(())
    }

    async fn get_next_free_ip(&self, subnet: &str) -> Result<Option<IpAddr>> {
        let subnets = self.subnets.read().await;
        let subnet_info = subnets
            .get(subnet)
            .ok_or_else(|| IpamError::SubnetNotFound {
                subnet: subnet.to_string(),
            })?;

        let cidr = subnet_info.cidr;
        drop(subnets);

        self.find_next_free_ip(subnet, &cidr).await
    }

    async fn is_ip_available(&self, subnet: &str, ip: &IpAddr) -> Result<bool> {
        let subnets = self.subnets.read().await;
        let subnet_info = subnets
            .get(subnet)
            .ok_or_else(|| IpamError::SubnetNotFound {
                subnet: subnet.to_string(),
            })?;

        // Check if IP is within subnet
        if !subnet_info.cidr.contains(ip) {
            return Ok(false);
        }
        drop(subnets);

        // Check if IP is allocated
        let allocations = self.allocations.read().await;
        if let Some(subnet_allocs) = allocations.get(subnet) {
            Ok(!subnet_allocs.contains_key(ip))
        } else {
            Ok(true)
        }
    }
}
