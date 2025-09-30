//! Core network types and data structures

use crate::error::{ConfigError, NetworkError};
use pve_shared_types::SharedTypeError;

pub use pve_shared_types::{
    AddressMethod, BondMode, Interface, InterfaceType, IpAddress, MacAddr, NetworkConfiguration,
};

impl From<SharedTypeError> for NetworkError {
    fn from(err: SharedTypeError) -> Self {
        match err {
            SharedTypeError::InvalidValue { field, value } => {
                NetworkError::Configuration(ConfigError::InvalidValue {
                    field: field.to_string(),
                    value,
                })
            }
            SharedTypeError::ParseError(message) => {
                NetworkError::Configuration(ConfigError::Parse { line: 0, message })
            }
            SharedTypeError::Unsupported(value) => {
                NetworkError::Configuration(ConfigError::InvalidValue {
                    field: "unsupported".to_string(),
                    value,
                })
            }
        }
    }
}

// JSON Schema definitions for API validation (will be implemented with proxmox-schema)

/// Regex for valid interface names
pub const INTERFACE_NAME_REGEX: &str = r"^[a-zA-Z][a-zA-Z0-9_-]*$";

/// Regex for IP addresses
pub const IP_ADDRESS_REGEX: &str = r"^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)(?:/(?:[0-9]|[1-2][0-9]|3[0-2]))?$|^(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}(?:/(?:[0-9]|[1-9][0-9]|1[0-1][0-9]|12[0-8]))?$";

/// Regex for MAC addresses
pub const MAC_ADDRESS_REGEX: &str =
    r"^[0-9a-fA-F]{2}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}$";

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn test_ip_address_parsing() {
        let addr1: IpAddress = "192.168.1.1/24".parse().unwrap();
        assert_eq!(addr1.addr, "192.168.1.1".parse::<IpAddr>().unwrap());
        assert_eq!(addr1.prefix_len, Some(24));

        let addr2: IpAddress = "192.168.1.1".parse().unwrap();
        assert_eq!(addr2.addr, "192.168.1.1".parse::<IpAddr>().unwrap());
        assert_eq!(addr2.prefix_len, None);
    }

    #[test]
    fn test_bond_mode_parsing() {
        assert_eq!(
            "active-backup".parse::<BondMode>().unwrap(),
            BondMode::ActiveBackup
        );
        assert_eq!("1".parse::<BondMode>().unwrap(), BondMode::ActiveBackup);
        assert_eq!("802.3ad".parse::<BondMode>().unwrap(), BondMode::Ieee8023ad);
    }

    #[test]
    fn test_mac_address_parsing() {
        let mac: MacAddr = "00:11:22:33:44:55".parse().unwrap();
        assert_eq!(mac.to_string(), "00:11:22:33:44:55");
    }

    #[test]
    fn test_same_network() {
        let addr1: IpAddress = "192.168.1.10/24".parse().unwrap();
        let addr2: IpAddress = "192.168.1.20/24".parse().unwrap();
        let addr3: IpAddress = "192.168.2.10/24".parse().unwrap();

        assert!(addr1.same_network(&addr2));
        assert!(!addr1.same_network(&addr3));
    }
}
