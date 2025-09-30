pub use pve_shared_types::SdnConfiguration;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SubnetConfig, SubnetType, VNetConfig, ZoneConfig, ZoneType};
    use ipnet::IpNet;

    #[test]
    fn test_sdn_configuration_validation() {
        let mut config = SdnConfiguration::new();

        let zone_config = ZoneConfig::new(ZoneType::Simple, "zone1".to_string());
        config.add_zone(zone_config).unwrap();

        let vnet_config = VNetConfig::new("vnet1".to_string(), "zone1".to_string());
        config.add_vnet(vnet_config).unwrap();

        let cidr: IpNet = "192.168.1.0/24".parse().unwrap();
        let subnet_config = SubnetConfig::new("subnet1".to_string(), "vnet1".to_string(), cidr);
        config.add_subnet(subnet_config).unwrap();

        config.validate().unwrap();
    }

    #[test]
    fn test_dependency_validation() {
        let mut config = SdnConfiguration::new();

        let vnet_config = VNetConfig::new("vnet1".to_string(), "nonexistent".to_string());
        assert!(config.add_vnet(vnet_config).is_err());

        let zone_config = ZoneConfig::new(ZoneType::Simple, "zone1".to_string());
        config.add_zone(zone_config).unwrap();

        let vnet_config = VNetConfig::new("vnet1".to_string(), "zone1".to_string());
        config.add_vnet(vnet_config).unwrap();

        assert!(config.remove_zone("zone1").is_err());

        config.remove_vnet("vnet1").unwrap();
        config.remove_zone("zone1").unwrap();
    }
}
