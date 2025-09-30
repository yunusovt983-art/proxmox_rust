//! Migration hooks that listen for system events via the shared event bus.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use log::info;
use pve_event_bus::{EventBus, EventBusResult, EventListener};
use pve_shared_types::{MigrationPhase, SystemEvent};
use tokio::sync::RwLock;

use crate::{MigrationError, Result as MigrationResult};

#[async_trait]
pub trait MigrationHook: Send + Sync {
    fn name(&self) -> &str;
    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub struct MigrationHooks {
    hooks: Arc<RwLock<HashMap<String, Arc<dyn MigrationHook>>>>,
    current_phase: Arc<RwLock<MigrationPhase>>,
    network_applied_events: Arc<RwLock<u64>>,
}

impl MigrationHooks {
    pub fn new(initial_phase: MigrationPhase) -> Self {
        Self {
            hooks: Arc::new(RwLock::new(HashMap::new())),
            current_phase: Arc::new(RwLock::new(initial_phase)),
            network_applied_events: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn register_hook<H>(&self, hook: H) -> MigrationResult<()>
    where
        H: MigrationHook + 'static,
    {
        let mut guard = self.hooks.write().await;
        let name = hook.name().to_string();
        if guard.contains_key(&name) {
            return Err(MigrationError::Configuration(format!(
                "Migration hook '{}' already registered",
                name
            )));
        }
        guard.insert(name, Arc::new(hook));
        Ok(())
    }

    pub async fn bind_event_bus(&self, bus: Arc<EventBus>) -> EventBusResult<()> {
        bus.register_listener(
            "migration-hooks",
            MigrationEventListener {
                hooks: self.clone(),
            },
        )
        .await
    }

    pub async fn notify(&self, event: &SystemEvent) -> MigrationResult<()> {
        let guard = self.hooks.read().await;
        for hook in guard.values() {
            hook.on_event(event).await.map_err(|err| {
                MigrationError::Configuration(format!(
                    "Migration hook '{}' failed: {}",
                    hook.name(),
                    err
                ))
            })?;
        }
        Ok(())
    }

    pub async fn current_phase(&self) -> MigrationPhase {
        *self.current_phase.read().await
    }

    pub async fn network_applied_count(&self) -> u64 {
        *self.network_applied_events.read().await
    }
}

struct MigrationEventListener {
    hooks: MigrationHooks,
}

#[async_trait]
impl EventListener for MigrationEventListener {
    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()> {
        match event {
            SystemEvent::MigrationPhaseChanged { phase } => {
                info!("Migration hooks observing phase change: {:?}", phase);
                *self.hooks.current_phase.write().await = *phase;
            }
            SystemEvent::NetworkApplied { .. } => {
                let mut counter = self.hooks.network_applied_events.write().await;
                *counter += 1;
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

pub struct MigrationEventLogger;

#[async_trait]
impl MigrationHook for MigrationEventLogger {
    fn name(&self) -> &str {
        "migration-event-logger"
    }

    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()> {
        info!("Migration event received: {:?}", event);
        Ok(())
    }
}
