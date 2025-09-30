//! IPAM drivers

pub mod factory;
pub mod netbox;
pub mod phpipam;
pub mod pve;

#[cfg(test)]
mod tests;

pub use factory::IpamPluginFactory;
pub use netbox::NetBoxIpam;
pub use phpipam::PhpIpam;
pub use pve::PveIpam;
