//! Proxmox VE Network CLI
//!
//! CLI utilities for managing Proxmox VE network configurations.
//!
//! This crate provides command-line tools for validating, applying,
//! rolling back, and monitoring network configurations in Proxmox VE
//! environments. It maintains compatibility with existing Perl-based
//! tools while providing enhanced functionality and performance.

pub mod commands;

#[cfg(test)]
mod tests;
