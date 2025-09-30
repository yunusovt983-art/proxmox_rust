//! phpIPAM driver

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;

use pve_sdn_core::{
    IpAllocation, IpAllocationRequest, IpamConfig, IpamError, IpamPlugin, IpamType, Subnet,
};

/// phpIPAM API response wrapper
#[derive(Debug, Deserialize)]
struct PhpIpamResponse<T> {
    success: bool,
    data: Option<T>,
    message: Option<String>,
}

/// phpIPAM subnet data
#[derive(Debug, Serialize, Deserialize)]
struct PhpIpamSubnet {
    id: String,
    subnet: String,
    mask: String,
    description: Option<String>,
    #[serde(rename = "sectionId")]
    section_id: String,
}

/// phpIPAM address data
#[derive(Debug, Serialize, Deserialize)]
struct PhpIpamAddress {
    id: Option<String>,
    #[serde(rename = "subnetId")]
    subnet_id: String,
    ip: String,
    hostname: Option<String>,
    description: Option<String>,
    mac: Option<String>,
    owner: Option<String>,
    tag: Option<String>,
    #[serde(rename = "excludePing")]
    exclude_ping: Option<String>,
}

/// phpIPAM implementation
///
/// Integrates with phpIPAM REST API for IP address management
pub struct PhpIpam {
    name: String,
    config: IpamConfig,
    client: Client,
    base_url: String,
    token: Option<String>,
    section_id: Option<String>,
}

impl PhpIpam {
    /// Create new phpIPAM client
    pub fn new(name: String, config: IpamConfig) -> Result<Self> {
        let base_url = config
            .url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("phpIPAM URL is required"))?
            .trim_end_matches('/')
            .to_string();

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            name,
            token: config.token.clone(),
            section_id: config.section.clone(),
            config,
            client,
            base_url,
        })
    }

    /// Authenticate with phpIPAM and get token
    async fn authenticate(&mut self) -> Result<()> {
        if self.token.is_some() {
            return Ok(()); // Already have token
        }

        let username = self
            .config
            .username
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("phpIPAM username is required"))?;
        let password = self
            .config
            .password
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("phpIPAM password is required"))?;

        let auth_url = format!("{}/api/app/user/", self.base_url);
        let response = self
            .client
            .post(&auth_url)
            .basic_auth(username, Some(password))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(IpamError::Api {
                message: format!("Authentication failed: {}", response.status()),
            }
            .into());
        }

        let auth_response: PhpIpamResponse<HashMap<String, String>> = response.json().await?;

        if !auth_response.success {
            return Err(IpamError::Api {
                message: auth_response
                    .message
                    .unwrap_or_else(|| "Authentication failed".to_string()),
            }
            .into());
        }

        if let Some(data) = auth_response.data {
            if let Some(token) = data.get("token") {
                self.token = Some(token.clone());
                log::info!("Successfully authenticated with phpIPAM");
                return Ok(());
            }
        }

        Err(IpamError::Api {
            message: "No token received from phpIPAM".to_string(),
        }
        .into())
    }

    /// Make authenticated API request
    async fn api_request<T>(
        &mut self,
        method: reqwest::Method,
        path: &str,
        body: Option<&impl Serialize>,
    ) -> Result<PhpIpamResponse<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        self.authenticate().await?;

        let token = self
            .token
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No authentication token available"))?;

        let url = format!("{}/api/app{}", self.base_url, path);
        let mut request = self.client.request(method, &url).header("token", token);

        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(IpamError::Api {
                message: format!("API request failed: {}", response.status()),
            }
            .into());
        }

        Ok(response.json().await?)
    }

    /// Find subnet by CIDR
    async fn find_subnet(&mut self, _cidr: &str) -> Result<Option<PhpIpamSubnet>> {
        let response: PhpIpamResponse<Vec<PhpIpamSubnet>> = self
            .api_request(reqwest::Method::GET, "/subnets/cidr/{cidr}/", None::<&()>)
            .await?;

        if let Some(subnets) = response.data {
            Ok(subnets.into_iter().next())
        } else {
            Ok(None)
        }
    }

    /// Get subnet by ID
    async fn get_subnet(&mut self, subnet_id: &str) -> Result<Option<PhpIpamSubnet>> {
        let path = format!("/subnets/{}/", subnet_id);
        let response: PhpIpamResponse<PhpIpamSubnet> = self
            .api_request(reqwest::Method::GET, &path, None::<&()>)
            .await?;

        Ok(response.data)
    }

    /// Create subnet in phpIPAM
    async fn create_subnet(&mut self, subnet: &Subnet) -> Result<String> {
        let section_id = self
            .section_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("phpIPAM section ID is required"))?;

        let subnet_data = serde_json::json!({
            "subnet": subnet.config.cidr.network().to_string(),
            "mask": subnet.config.cidr.prefix_len(),
            "description": format!("PVE subnet {}", subnet.config.subnet),
            "sectionId": section_id
        });

        let response: PhpIpamResponse<HashMap<String, String>> = self
            .api_request(reqwest::Method::POST, "/subnets/", Some(&subnet_data))
            .await?;

        if !response.success {
            return Err(IpamError::Api {
                message: response
                    .message
                    .unwrap_or_else(|| "Failed to create subnet".to_string()),
            }
            .into());
        }

        response
            .data
            .and_then(|data| data.get("id").cloned())
            .ok_or_else(|| {
                IpamError::Api {
                    message: "No subnet ID returned".to_string(),
                }
                .into()
            })
    }

    /// Convert phpIPAM address to IpAllocation
    fn to_ip_allocation(&self, addr: &PhpIpamAddress, subnet_name: &str) -> Result<IpAllocation> {
        let ip: IpAddr = addr
            .ip
            .parse()
            .map_err(|_| anyhow::anyhow!("Invalid IP address: {}", addr.ip))?;

        // Parse VMID from description or owner field
        let vmid = addr
            .description
            .as_ref()
            .or(addr.owner.as_ref())
            .and_then(|s| s.parse::<u32>().ok());

        Ok(IpAllocation {
            ip,
            subnet: subnet_name.to_string(),
            vmid,
            hostname: addr.hostname.clone(),
            mac: addr.mac.clone(),
            description: addr.description.clone(),
            allocated_at: chrono::Utc::now(), // phpIPAM doesn't provide creation time
        })
    }
}

#[async_trait]
impl IpamPlugin for PhpIpam {
    fn plugin_type(&self) -> IpamType {
        IpamType::PhpIpam
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn validate_config(&self, config: &IpamConfig) -> Result<()> {
        if config.ipam_type != IpamType::PhpIpam {
            anyhow::bail!("Invalid IPAM type for phpIPAM plugin");
        }

        if config.url.is_none() {
            anyhow::bail!("phpIPAM URL is required");
        }

        if config.token.is_none() && (config.username.is_none() || config.password.is_none()) {
            anyhow::bail!("phpIPAM requires either token or username/password");
        }

        Ok(())
    }

    async fn allocate_ip(&self, request: &IpAllocationRequest) -> Result<IpAllocation> {
        let mut client = PhpIpam::new(self.name.clone(), self.config.clone())?;

        // Find subnet in phpIPAM
        let subnet = client.find_subnet(&request.subnet).await?.ok_or_else(|| {
            IpamError::SubnetNotFound {
                subnet: request.subnet.clone(),
            }
        })?;

        let addr_data = PhpIpamAddress {
            id: None,
            subnet_id: subnet.id.clone(),
            ip: request
                .requested_ip
                .map(|ip| ip.to_string())
                .unwrap_or_default(),
            hostname: request.hostname.clone(),
            description: request
                .vmid
                .map(|id| id.to_string())
                .or(request.description.clone()),
            mac: request.mac.clone(),
            owner: request.vmid.map(|id| id.to_string()),
            tag: Some("2".to_string()), // Used tag
            exclude_ping: Some("0".to_string()),
        };

        let path = format!("/addresses/{}/", subnet.id);
        let response: PhpIpamResponse<HashMap<String, String>> = client
            .api_request(reqwest::Method::POST, &path, Some(&addr_data))
            .await?;

        if !response.success {
            return Err(IpamError::Api {
                message: response
                    .message
                    .unwrap_or_else(|| "Failed to allocate IP".to_string()),
            }
            .into());
        }

        let allocated_ip = if let Some(requested_ip) = request.requested_ip {
            requested_ip
        } else {
            // Get first available IP from subnet
            let first_free_path = format!("/subnets/{}/first_free/", subnet.id);
            let free_response: PhpIpamResponse<HashMap<String, String>> = client
                .api_request(reqwest::Method::GET, &first_free_path, None::<&()>)
                .await?;

            if let Some(data) = free_response.data {
                if let Some(ip_str) = data.get("data") {
                    ip_str.parse().map_err(|_| IpamError::Api {
                        message: format!("Invalid IP address returned: {}", ip_str),
                    })?
                } else {
                    return Err(IpamError::NoFreeIps {
                        subnet: request.subnet.clone(),
                    }
                    .into());
                }
            } else {
                return Err(IpamError::NoFreeIps {
                    subnet: request.subnet.clone(),
                }
                .into());
            }
        };

        Ok(IpAllocation {
            ip: allocated_ip,
            subnet: request.subnet.clone(),
            vmid: request.vmid,
            hostname: request.hostname.clone(),
            mac: request.mac.clone(),
            description: request.description.clone(),
            allocated_at: chrono::Utc::now(),
        })
    }

    async fn release_ip(&self, subnet: &str, ip: &IpAddr) -> Result<()> {
        let mut client = PhpIpam::new(self.name.clone(), self.config.clone())?;

        // Find subnet
        let subnet_info =
            client
                .find_subnet(subnet)
                .await?
                .ok_or_else(|| IpamError::SubnetNotFound {
                    subnet: subnet.to_string(),
                })?;

        // Find address
        let path = format!("/addresses/search/{}/", ip);
        let response: PhpIpamResponse<Vec<PhpIpamAddress>> = client
            .api_request(reqwest::Method::GET, &path, None::<&()>)
            .await?;

        if let Some(addresses) = response.data {
            for addr in addresses {
                if addr.subnet_id == subnet_info.id {
                    if let Some(addr_id) = addr.id {
                        let delete_path = format!("/addresses/{}/", addr_id);
                        let _: PhpIpamResponse<()> = client
                            .api_request(reqwest::Method::DELETE, &delete_path, None::<&()>)
                            .await?;
                        return Ok(());
                    }
                }
            }
        }

        Err(IpamError::IpNotFound {
            ip: *ip,
            subnet: subnet.to_string(),
        }
        .into())
    }

    async fn update_ip(
        &self,
        _subnet: &str,
        _ip: &IpAddr,
        _allocation: &IpAllocation,
    ) -> Result<()> {
        // phpIPAM update implementation would be similar to release + allocate
        // For brevity, implementing as a placeholder
        log::warn!("phpIPAM update_ip not fully implemented");
        Ok(())
    }

    async fn get_ip(&self, subnet: &str, ip: &IpAddr) -> Result<Option<IpAllocation>> {
        let mut client = PhpIpam::new(self.name.clone(), self.config.clone())?;

        let path = format!("/addresses/search/{}/", ip);
        let response: PhpIpamResponse<Vec<PhpIpamAddress>> = client
            .api_request(reqwest::Method::GET, &path, None::<&()>)
            .await?;

        if let Some(addresses) = response.data {
            for addr in addresses {
                if addr.ip == ip.to_string() {
                    return Ok(Some(client.to_ip_allocation(&addr, subnet)?));
                }
            }
        }

        Ok(None)
    }

    async fn list_subnet_ips(&self, subnet: &str) -> Result<Vec<IpAllocation>> {
        let mut client = PhpIpam::new(self.name.clone(), self.config.clone())?;

        let subnet_info =
            client
                .find_subnet(subnet)
                .await?
                .ok_or_else(|| IpamError::SubnetNotFound {
                    subnet: subnet.to_string(),
                })?;

        let path = format!("/subnets/{}/addresses/", subnet_info.id);
        let response: PhpIpamResponse<Vec<PhpIpamAddress>> = client
            .api_request(reqwest::Method::GET, &path, None::<&()>)
            .await?;

        let mut allocations = Vec::new();
        if let Some(addresses) = response.data {
            for addr in addresses {
                allocations.push(client.to_ip_allocation(&addr, subnet)?);
            }
        }

        allocations.sort_by_key(|a| a.ip);
        Ok(allocations)
    }

    async fn validate_subnet(&self, subnet: &Subnet) -> Result<()> {
        subnet.config.validate()
    }

    async fn add_subnet(&self, subnet: &Subnet) -> Result<()> {
        let mut client = PhpIpam::new(self.name.clone(), self.config.clone())?;
        client.create_subnet(subnet).await?;
        Ok(())
    }

    async fn remove_subnet(&self, subnet_name: &str) -> Result<()> {
        let mut client = PhpIpam::new(self.name.clone(), self.config.clone())?;

        let subnet_info =
            client
                .find_subnet(subnet_name)
                .await?
                .ok_or_else(|| IpamError::SubnetNotFound {
                    subnet: subnet_name.to_string(),
                })?;

        let path = format!("/subnets/{}/", subnet_info.id);
        let _: PhpIpamResponse<()> = client
            .api_request(reqwest::Method::DELETE, &path, None::<&()>)
            .await?;

        Ok(())
    }

    async fn get_next_free_ip(&self, subnet: &str) -> Result<Option<IpAddr>> {
        let mut client = PhpIpam::new(self.name.clone(), self.config.clone())?;

        let subnet_info =
            client
                .find_subnet(subnet)
                .await?
                .ok_or_else(|| IpamError::SubnetNotFound {
                    subnet: subnet.to_string(),
                })?;

        let path = format!("/subnets/{}/first_free/", subnet_info.id);
        let response: PhpIpamResponse<HashMap<String, String>> = client
            .api_request(reqwest::Method::GET, &path, None::<&()>)
            .await?;

        if let Some(data) = response.data {
            if let Some(ip_str) = data.get("data") {
                return Ok(Some(ip_str.parse()?));
            }
        }

        Ok(None)
    }

    async fn is_ip_available(&self, subnet: &str, ip: &IpAddr) -> Result<bool> {
        Ok(self.get_ip(subnet, ip).await?.is_none())
    }
}
