//! NetBox IPAM driver

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;

use pve_sdn_core::{
    IpAllocation, IpAllocationRequest, IpamConfig, IpamError, IpamPlugin, IpamType, Subnet,
};

/// NetBox API response wrapper
#[derive(Debug, Deserialize)]
struct NetBoxResponse<T> {
    count: Option<u32>,
    results: Option<Vec<T>>,
}

/// NetBox prefix (subnet) data
#[derive(Debug, Serialize, Deserialize)]
struct NetBoxPrefix {
    id: Option<u32>,
    prefix: String,
    description: Option<String>,
    tenant: Option<NetBoxTenant>,
    status: Option<NetBoxStatus>,
    role: Option<NetBoxRole>,
}

/// NetBox IP address data
#[derive(Debug, Serialize, Deserialize)]
struct NetBoxIpAddress {
    id: Option<u32>,
    address: String,
    description: Option<String>,
    dns_name: Option<String>,
    tenant: Option<NetBoxTenant>,
    status: Option<NetBoxStatus>,
    role: Option<NetBoxRole>,
    assigned_object: Option<serde_json::Value>,
    custom_fields: Option<HashMap<String, serde_json::Value>>,
}

/// NetBox tenant
#[derive(Debug, Serialize, Deserialize)]
struct NetBoxTenant {
    id: u32,
    name: String,
    slug: String,
}

/// NetBox status
#[derive(Debug, Serialize, Deserialize)]
struct NetBoxStatus {
    value: String,
    label: String,
}

/// NetBox role
#[derive(Debug, Serialize, Deserialize)]
struct NetBoxRole {
    id: u32,
    name: String,
    slug: String,
}

/// NetBox IPAM implementation
///
/// Integrates with NetBox REST API for IP address management
pub struct NetBoxIpam {
    name: String,
    config: IpamConfig,
    client: Client,
    base_url: String,
    token: String,
    tenant_id: Option<u32>,
}

impl NetBoxIpam {
    /// Create new NetBox IPAM client
    pub fn new(name: String, config: IpamConfig) -> Result<Self> {
        let base_url = config
            .url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NetBox URL is required"))?
            .trim_end_matches('/')
            .to_string();

        let token = config
            .token
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NetBox token is required"))?
            .clone();

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        // Parse tenant ID if provided
        let tenant_id = config.tenant.as_ref().and_then(|t| t.parse::<u32>().ok());

        Ok(Self {
            name,
            config,
            client,
            base_url,
            token,
            tenant_id,
        })
    }

    /// Make authenticated API request
    async fn api_request<T>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let url = format!("{}/api{}", self.base_url, path);
        let mut request = self
            .client
            .request(method, &url)
            .header("Authorization", format!("Token {}", self.token))
            .header("Content-Type", "application/json");

        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(IpamError::Api {
                message: format!("NetBox API request failed: {} - {}", status, text),
            }
            .into());
        }

        Ok(response.json().await?)
    }

    /// Find prefix by CIDR
    async fn find_prefix(&self, cidr: &str) -> Result<Option<NetBoxPrefix>> {
        let path = format!("/ipam/prefixes/?prefix={}", urlencoding::encode(cidr));
        let response: NetBoxResponse<NetBoxPrefix> = self
            .api_request(reqwest::Method::GET, &path, None::<&()>)
            .await?;

        Ok(response.results.and_then(|mut prefixes| prefixes.pop()))
    }

    /// Create prefix in NetBox
    async fn create_prefix(&self, subnet: &Subnet) -> Result<NetBoxPrefix> {
        let mut prefix_data = serde_json::json!({
            "prefix": subnet.config.cidr.to_string(),
            "description": format!("PVE subnet {}", subnet.config.subnet),
            "status": "active"
        });

        if let Some(tenant_id) = self.tenant_id {
            prefix_data["tenant"] = serde_json::json!(tenant_id);
        }

        let response: NetBoxPrefix = self
            .api_request(reqwest::Method::POST, "/ipam/prefixes/", Some(&prefix_data))
            .await?;

        Ok(response)
    }

    /// Convert NetBox IP address to IpAllocation
    fn to_ip_allocation(&self, addr: &NetBoxIpAddress, subnet_name: &str) -> Result<IpAllocation> {
        // Parse IP address (remove prefix length if present)
        let ip_str = addr.address.split('/').next().unwrap_or(&addr.address);
        let ip: IpAddr = ip_str
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid IP address: {}", ip_str))?;

        // Extract VMID from custom fields or description
        let vmid = addr
            .custom_fields
            .as_ref()
            .and_then(|cf| cf.get("vmid"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .or_else(|| {
                addr.description
                    .as_ref()
                    .and_then(|d| d.parse::<u32>().ok())
            });

        Ok(IpAllocation {
            ip,
            subnet: subnet_name.to_string(),
            vmid,
            hostname: addr.dns_name.clone(),
            mac: None, // NetBox doesn't store MAC in IP address records
            description: addr.description.clone(),
            allocated_at: chrono::Utc::now(), // NetBox doesn't provide creation time in basic API
        })
    }

    /// Get available IP from prefix
    async fn get_available_ip(&self, prefix_id: u32) -> Result<Option<IpAddr>> {
        let path = format!("/ipam/prefixes/{}/available-ips/", prefix_id);
        let response: Vec<HashMap<String, String>> = self
            .api_request(reqwest::Method::GET, &path, None::<&()>)
            .await?;

        if let Some(first_available) = response.first() {
            if let Some(ip_str) = first_available.get("address") {
                let ip_only = ip_str.split('/').next().unwrap_or(ip_str);
                return Ok(Some(ip_only.parse()?));
            }
        }

        Ok(None)
    }
}

#[async_trait]
impl IpamPlugin for NetBoxIpam {
    fn plugin_type(&self) -> IpamType {
        IpamType::NetBox
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn validate_config(&self, config: &IpamConfig) -> Result<()> {
        if config.ipam_type != IpamType::NetBox {
            anyhow::bail!("Invalid IPAM type for NetBox plugin");
        }

        if config.url.is_none() {
            anyhow::bail!("NetBox URL is required");
        }

        if config.token.is_none() {
            anyhow::bail!("NetBox token is required");
        }

        Ok(())
    }

    async fn allocate_ip(&self, request: &IpAllocationRequest) -> Result<IpAllocation> {
        // Find prefix
        let prefix =
            self.find_prefix(&request.subnet)
                .await?
                .ok_or_else(|| IpamError::SubnetNotFound {
                    subnet: request.subnet.clone(),
                })?;

        let prefix_id = prefix.id.ok_or_else(|| IpamError::Api {
            message: "Prefix has no ID".to_string(),
        })?;

        let ip = if let Some(requested_ip) = request.requested_ip {
            requested_ip
        } else {
            self.get_available_ip(prefix_id)
                .await?
                .ok_or_else(|| IpamError::NoFreeIps {
                    subnet: request.subnet.clone(),
                })?
        };

        // Create IP address record
        let mut ip_data = serde_json::json!({
            "address": format!("{}/{}", ip, prefix.prefix.split('/').nth(1).unwrap_or("24")),
            "status": "active"
        });

        if let Some(hostname) = &request.hostname {
            ip_data["dns_name"] = serde_json::json!(hostname);
        }

        if let Some(description) = &request.description {
            ip_data["description"] = serde_json::json!(description);
        }

        if let Some(vmid) = request.vmid {
            ip_data["custom_fields"] = serde_json::json!({
                "vmid": vmid
            });
        }

        if let Some(tenant_id) = self.tenant_id {
            ip_data["tenant"] = serde_json::json!(tenant_id);
        }

        let _: NetBoxIpAddress = self
            .api_request(reqwest::Method::POST, "/ipam/ip-addresses/", Some(&ip_data))
            .await?;

        Ok(IpAllocation {
            ip,
            subnet: request.subnet.clone(),
            vmid: request.vmid,
            hostname: request.hostname.clone(),
            mac: request.mac.clone(),
            description: request.description.clone(),
            allocated_at: chrono::Utc::now(),
        })
    }

    async fn release_ip(&self, subnet: &str, ip: &IpAddr) -> Result<()> {
        // Find IP address
        let path = format!("/ipam/ip-addresses/?address={}", ip);
        let response: NetBoxResponse<NetBoxIpAddress> = self
            .api_request(reqwest::Method::GET, &path, None::<&()>)
            .await?;

        if let Some(addresses) = response.results {
            for addr in addresses {
                if let Some(addr_id) = addr.id {
                    let delete_path = format!("/ipam/ip-addresses/{}/", addr_id);
                    let _: serde_json::Value = self
                        .api_request(reqwest::Method::DELETE, &delete_path, None::<&()>)
                        .await?;
                    return Ok(());
                }
            }
        }

        Err(IpamError::IpNotFound {
            ip: *ip,
            subnet: subnet.to_string(),
        }
        .into())
    }

    async fn update_ip(&self, subnet: &str, ip: &IpAddr, allocation: &IpAllocation) -> Result<()> {
        // Find IP address
        let path = format!("/ipam/ip-addresses/?address={}", ip);
        let response: NetBoxResponse<NetBoxIpAddress> = self
            .api_request(reqwest::Method::GET, &path, None::<&()>)
            .await?;

        if let Some(addresses) = response.results {
            for addr in addresses {
                if let Some(addr_id) = addr.id {
                    let mut update_data = serde_json::json!({});

                    if let Some(hostname) = &allocation.hostname {
                        update_data["dns_name"] = serde_json::json!(hostname);
                    }

                    if let Some(description) = &allocation.description {
                        update_data["description"] = serde_json::json!(description);
                    }

                    if let Some(vmid) = allocation.vmid {
                        update_data["custom_fields"] = serde_json::json!({
                            "vmid": vmid
                        });
                    }

                    let update_path = format!("/ipam/ip-addresses/{}/", addr_id);
                    let _: NetBoxIpAddress = self
                        .api_request(reqwest::Method::PATCH, &update_path, Some(&update_data))
                        .await?;
                    return Ok(());
                }
            }
        }

        Err(IpamError::IpNotFound {
            ip: *ip,
            subnet: subnet.to_string(),
        }
        .into())
    }

    async fn get_ip(&self, subnet: &str, ip: &IpAddr) -> Result<Option<IpAllocation>> {
        let path = format!("/ipam/ip-addresses/?address={}", ip);
        let response: NetBoxResponse<NetBoxIpAddress> = self
            .api_request(reqwest::Method::GET, &path, None::<&()>)
            .await?;

        if let Some(addresses) = response.results {
            for addr in addresses {
                return Ok(Some(self.to_ip_allocation(&addr, subnet)?));
            }
        }

        Ok(None)
    }

    async fn list_subnet_ips(&self, subnet: &str) -> Result<Vec<IpAllocation>> {
        // Find prefix first
        let prefix = self
            .find_prefix(subnet)
            .await?
            .ok_or_else(|| IpamError::SubnetNotFound {
                subnet: subnet.to_string(),
            })?;

        let path = format!(
            "/ipam/ip-addresses/?parent={}",
            urlencoding::encode(&prefix.prefix)
        );
        let response: NetBoxResponse<NetBoxIpAddress> = self
            .api_request(reqwest::Method::GET, &path, None::<&()>)
            .await?;

        let mut allocations = Vec::new();
        if let Some(addresses) = response.results {
            for addr in addresses {
                allocations.push(self.to_ip_allocation(&addr, subnet)?);
            }
        }

        allocations.sort_by_key(|a| a.ip);
        Ok(allocations)
    }

    async fn validate_subnet(&self, subnet: &Subnet) -> Result<()> {
        subnet.config.validate()
    }

    async fn add_subnet(&self, subnet: &Subnet) -> Result<()> {
        self.create_prefix(subnet).await?;
        Ok(())
    }

    async fn remove_subnet(&self, subnet_name: &str) -> Result<()> {
        let prefix =
            self.find_prefix(subnet_name)
                .await?
                .ok_or_else(|| IpamError::SubnetNotFound {
                    subnet: subnet_name.to_string(),
                })?;

        if let Some(prefix_id) = prefix.id {
            let path = format!("/ipam/prefixes/{}/", prefix_id);
            let _: serde_json::Value = self
                .api_request(reqwest::Method::DELETE, &path, None::<&()>)
                .await?;
        }

        Ok(())
    }

    async fn get_next_free_ip(&self, subnet: &str) -> Result<Option<IpAddr>> {
        let prefix = self
            .find_prefix(subnet)
            .await?
            .ok_or_else(|| IpamError::SubnetNotFound {
                subnet: subnet.to_string(),
            })?;

        if let Some(prefix_id) = prefix.id {
            return self.get_available_ip(prefix_id).await;
        }

        Ok(None)
    }

    async fn is_ip_available(&self, subnet: &str, ip: &IpAddr) -> Result<bool> {
        Ok(self.get_ip(subnet, ip).await?.is_none())
    }
}
