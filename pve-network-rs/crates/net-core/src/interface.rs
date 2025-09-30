//! Network interface management

use async_trait::async_trait;
use std::collections::HashMap;

use crate::error::NetworkError;
use crate::types::{AddressMethod, Interface, InterfaceType, IpAddress};
use crate::Result;

/// Interface configuration for creation/updates
#[derive(Debug, Clone)]
pub struct InterfaceConfig {
    pub name: String,
    pub iface_type: InterfaceType,
    pub method: AddressMethod,
    pub addresses: Vec<IpAddress>,
    pub gateway: Option<IpAddress>,
    pub mtu: Option<u16>,
    pub options: HashMap<String, String>,
    pub enabled: bool,
}

impl InterfaceConfig {
    /// Create new interface configuration
    pub fn new(name: String, iface_type: InterfaceType) -> Self {
        Self {
            name,
            iface_type,
            method: AddressMethod::Manual,
            addresses: Vec::new(),
            gateway: None,
            mtu: None,
            options: HashMap::new(),
            enabled: true,
        }
    }

    /// Set address method
    pub fn with_method(mut self, method: AddressMethod) -> Self {
        self.method = method;
        self
    }

    /// Add IP address
    pub fn with_address(mut self, address: IpAddress) -> Self {
        self.addresses.push(address);
        self
    }

    /// Set gateway
    pub fn with_gateway(mut self, gateway: IpAddress) -> Self {
        self.gateway = Some(gateway);
        self
    }

    /// Set MTU
    pub fn with_mtu(mut self, mtu: u16) -> Self {
        self.mtu = Some(mtu);
        self
    }

    /// Add option
    pub fn with_option(mut self, key: String, value: String) -> Self {
        self.options.insert(key, value);
        self
    }

    /// Set enabled state
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Convert to Interface
    pub fn to_interface(self) -> Interface {
        Interface {
            name: self.name,
            iface_type: self.iface_type,
            method: self.method,
            addresses: self.addresses,
            gateway: self.gateway,
            mtu: self.mtu,
            options: self.options,
            enabled: self.enabled,
            comments: Vec::new(),
        }
    }
}

/// Network manager trait for interface operations
#[async_trait]
pub trait NetworkManager {
    /// Get all network interfaces
    async fn get_interfaces(&self) -> Result<Vec<Interface>>;

    /// Get specific interface by name
    async fn get_interface(&self, name: &str) -> Result<Option<Interface>>;

    /// Create new network interface
    async fn create_interface(&self, config: InterfaceConfig) -> Result<Interface>;

    /// Update existing network interface
    async fn update_interface(&self, name: &str, config: InterfaceConfig) -> Result<Interface>;

    /// Delete network interface
    async fn delete_interface(&self, name: &str) -> Result<()>;

    /// Check if interface exists
    async fn interface_exists(&self, name: &str) -> Result<bool>;

    /// Validate interface configuration
    async fn validate_interface(&self, config: &InterfaceConfig) -> Result<()>;
}

/// Interface validation functions
pub struct InterfaceValidator;

impl InterfaceValidator {
    /// Validate interface name
    pub fn validate_name(name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidInterfaceName {
                    name: name.to_string(),
                },
            ));
        }

        if name.len() > 15 {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidInterfaceName {
                    name: name.to_string(),
                },
            ));
        }

        // Check for valid characters (allow dots for VLAN interfaces)
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidInterfaceName {
                    name: name.to_string(),
                },
            ));
        }

        // Must start with letter
        if !name.chars().next().unwrap().is_ascii_alphabetic() {
            return Err(NetworkError::Configuration(
                crate::error::ConfigError::InvalidInterfaceName {
                    name: name.to_string(),
                },
            ));
        }

        Ok(())
    }

    /// Validate interface configuration
    pub fn validate_config(config: &InterfaceConfig) -> Result<()> {
        // Validate name
        Self::validate_name(&config.name)?;

        // Validate MTU
        if let Some(mtu) = config.mtu {
            if mtu < 68 {
                return Err(NetworkError::Configuration(
                    crate::error::ConfigError::InvalidValue {
                        field: "mtu".to_string(),
                        value: mtu.to_string(),
                    },
                ));
            }
        }

        // Validate interface type specific requirements
        match &config.iface_type {
            InterfaceType::Bridge { ports, .. } => {
                for port in ports {
                    Self::validate_name(port)?;
                }
            }
            InterfaceType::Bond { slaves, .. } => {
                if slaves.is_empty() {
                    return Err(NetworkError::Configuration(
                        crate::error::ConfigError::MissingField {
                            field: "bond_slaves".to_string(),
                        },
                    ));
                }
                for slave in slaves {
                    Self::validate_name(slave)?;
                }
            }
            InterfaceType::Vlan { parent, tag } => {
                Self::validate_name(parent)?;
                if *tag == 0 || *tag > 4094 {
                    return Err(NetworkError::Configuration(
                        crate::error::ConfigError::InvalidValue {
                            field: "vlan_tag".to_string(),
                            value: tag.to_string(),
                        },
                    ));
                }
            }
            InterfaceType::Vxlan { id, .. } => {
                if *id > 16777215 {
                    return Err(NetworkError::Configuration(
                        crate::error::ConfigError::InvalidValue {
                            field: "vxlan_id".to_string(),
                            value: id.to_string(),
                        },
                    ));
                }
            }
            _ => {}
        }

        // Validate address method consistency
        match config.method {
            AddressMethod::Static => {
                if config.addresses.is_empty() {
                    return Err(NetworkError::Configuration(
                        crate::error::ConfigError::MissingField {
                            field: "address".to_string(),
                        },
                    ));
                }
            }
            AddressMethod::Dhcp => {
                if !config.addresses.is_empty() {
                    return Err(NetworkError::Configuration(
                        crate::error::ConfigError::InvalidValue {
                            field: "method".to_string(),
                            value: "dhcp with static addresses".to_string(),
                        },
                    ));
                }
            }
            _ => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AddressMethod, InterfaceType};

    #[test]
    fn test_interface_name_validation() {
        assert!(InterfaceValidator::validate_name("eth0").is_ok());
        assert!(InterfaceValidator::validate_name("br-test").is_ok());
        assert!(InterfaceValidator::validate_name("bond_0").is_ok());

        assert!(InterfaceValidator::validate_name("").is_err());
        assert!(InterfaceValidator::validate_name("0eth").is_err());
        assert!(InterfaceValidator::validate_name("eth@0").is_err());
        assert!(InterfaceValidator::validate_name("very-long-interface-name").is_err());
    }

    #[test]
    fn test_interface_config_validation() {
        let config = InterfaceConfig::new("eth0".to_string(), InterfaceType::Physical)
            .with_method(AddressMethod::Static)
            .with_address("192.168.1.10/24".parse().unwrap());

        assert!(InterfaceValidator::validate_config(&config).is_ok());

        let invalid_config = InterfaceConfig::new("eth0".to_string(), InterfaceType::Physical)
            .with_method(AddressMethod::Static);

        assert!(InterfaceValidator::validate_config(&invalid_config).is_err());
    }

    #[test]
    fn test_vlan_validation() {
        let config = InterfaceConfig::new(
            "eth0.100".to_string(),
            InterfaceType::Vlan {
                parent: "eth0".to_string(),
                tag: 100,
            },
        );

        assert!(InterfaceValidator::validate_config(&config).is_ok());

        let invalid_config = InterfaceConfig::new(
            "eth0.5000".to_string(),
            InterfaceType::Vlan {
                parent: "eth0".to_string(),
                tag: 5000,
            },
        );

        assert!(InterfaceValidator::validate_config(&invalid_config).is_err());
    }
}
