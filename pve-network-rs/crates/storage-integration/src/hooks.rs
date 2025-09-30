//! Storage hooks reacting to system-wide events.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use pve_event_bus::{EventBus, EventBusResult, EventListener};
use pve_shared_types::SystemEvent;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::{
    future_integration::FutureStorageIntegration, StorageIntegrationError, StorageResult,
    StorageVlanManager,
};

#[async_trait]
pub trait StorageHook: Send + Sync {
    fn name(&self) -> &str;
    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub struct StorageHooks {
    hooks: Arc<RwLock<HashMap<String, Arc<dyn StorageHook>>>>,
    future_integration: Arc<dyn FutureStorageIntegration + Send + Sync>,
}

impl StorageHooks {
    pub fn new(future_integration: Arc<dyn FutureStorageIntegration + Send + Sync>) -> Self {
        Self {
            hooks: Arc::new(RwLock::new(HashMap::new())),
            future_integration,
        }
    }

    pub async fn register_hook<H>(&self, hook: H) -> StorageResult<()>
    where
        H: StorageHook + 'static,
    {
        let mut guard = self.hooks.write().await;
        let name = hook.name().to_string();
        if guard.contains_key(&name) {
            return Err(StorageIntegrationError::Configuration(format!(
                "Storage hook '{}' already registered",
                name
            )));
        }
        guard.insert(name, Arc::new(hook));
        Ok(())
    }

    pub async fn unregister_hook(&self, name: &str) -> StorageResult<()> {
        let mut guard = self.hooks.write().await;
        if guard.remove(name).is_some() {
            Ok(())
        } else {
            Err(StorageIntegrationError::Configuration(format!(
                "Storage hook '{}' not found",
                name
            )))
        }
    }

    pub async fn bind_event_bus(&self, bus: Arc<EventBus>) -> EventBusResult<()> {
        bus.register_listener(
            "storage-hooks",
            StorageEventListener {
                hooks: self.clone(),
            },
        )
        .await
    }

    pub fn future_integration(&self) -> Arc<dyn FutureStorageIntegration + Send + Sync> {
        Arc::clone(&self.future_integration)
    }

    pub async fn notify(&self, event: &SystemEvent) -> StorageResult<()> {
        let guard = self.hooks.read().await;
        for hook in guard.values() {
            hook.on_event(event).await.map_err(|err| {
                StorageIntegrationError::Configuration(format!(
                    "Storage hook '{}' failed: {}",
                    hook.name(),
                    err
                ))
            })?;
        }
        Ok(())
    }
}

struct StorageEventListener {
    hooks: StorageHooks,
}

#[async_trait]
impl EventListener for StorageEventListener {
    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()> {
        match event {
            SystemEvent::NetworkApplied { .. } => {
                info!("Storage hooks reacting to applied network changes");
            }
            SystemEvent::StorageVlanCreated { id } => {
                info!("Storage hooks received VLAN creation event for {}", id);
            }
            _ => {}
        }

        self.hooks
            .notify(event)
            .await
            .map_err(anyhow::Error::from)?;

        Ok(())
    }
}

pub struct StorageEventLogger;

#[async_trait]
impl StorageHook for StorageEventLogger {
    fn name(&self) -> &str {
        "storage-event-logger"
    }

    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()> {
        info!("Storage event received: {:?}", event);
        Ok(())
    }
}

pub struct StorageStatusRefresher {
    integration: Arc<dyn FutureStorageIntegration + Send + Sync>,
}

impl StorageStatusRefresher {
    pub fn new(integration: Arc<dyn FutureStorageIntegration + Send + Sync>) -> Self {
        Self { integration }
    }
}

#[async_trait]
impl StorageHook for StorageStatusRefresher {
    fn name(&self) -> &str {
        "storage-status-refresher"
    }

    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()> {
        match event {
            SystemEvent::NetworkApplied { .. } => {
                let backends = self.integration.list_storage_backends().await?;
                for backend in backends {
                    if let Err(err) = self
                        .integration
                        .get_storage_network_status(&backend.storage_id)
                        .await
                    {
                        warn!(
                            "Failed to refresh storage status for {}: {}",
                            backend.storage_id, err
                        );
                    }
                }
            }
            SystemEvent::StorageVlanCreated { id } => {
                if let Err(err) = self.integration.get_storage_network_status(id).await {
                    warn!("Failed to refresh storage VLAN {}: {}", id, err);
                }
            }
            _ => {}
        }

        Ok(())
    }
}

pub struct StorageVlanReconciler {
    vlan_manager: Arc<RwLock<StorageVlanManager>>,
}

impl StorageVlanReconciler {
    pub fn new(vlan_manager: Arc<RwLock<StorageVlanManager>>) -> Self {
        Self { vlan_manager }
    }
}

#[async_trait]
impl StorageHook for StorageVlanReconciler {
    fn name(&self) -> &str {
        "storage-vlan-reconciler"
    }

    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()> {
        if let SystemEvent::NetworkApplied { .. } = event {
            let configs = {
                let manager = self.vlan_manager.read().await;
                manager.configured_vlans()
            };

            if configs.is_empty() {
                debug!("No storage VLAN definitions registered, skipping reconciliation");
                return Ok(());
            }

            for (storage_id, config) in configs {
                let mut manager = self.vlan_manager.write().await;
                if let Err(err) = manager.create_storage_vlan(&storage_id, &config).await {
                    warn!(
                        "Failed to reconcile storage VLAN for {}: {}",
                        storage_id, err
                    );
                }
            }
        }

        Ok(())
    }
}
