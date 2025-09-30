//! Container network hooks for future Rust pve-container integration

use async_trait::async_trait;
use log::{error, info, warn};
use pve_event_bus::{EventBus, EventBusResult, EventListener};
use pve_shared_types::SystemEvent;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{ContainerError, Result};
use crate::types::{
    ContainerId, ContainerNetworkConfig, ContainerNetworkConfigExt, ContainerNetworkEventType,
};

/// Container network hooks manager
#[derive(Clone)]
pub struct ContainerNetworkHooks {
    /// Registered hooks
    hooks: Arc<RwLock<HashMap<String, Box<dyn ContainerNetworkHook + Send + Sync>>>>,
    /// Hook execution history
    execution_history: Arc<RwLock<Vec<HookExecution>>>,
}

/// Hook execution record
#[derive(Debug, Clone)]
pub struct HookExecution {
    /// Hook name
    pub hook_name: String,
    /// Container ID
    pub container_id: ContainerId,
    /// Event type
    pub event_type: ContainerNetworkEventType,
    /// Execution timestamp
    pub executed_at: chrono::DateTime<chrono::Utc>,
    /// Execution result
    pub result: HookExecutionResult,
    /// Execution duration
    pub duration: chrono::Duration,
}

/// Hook execution result
#[derive(Debug, Clone)]
pub enum HookExecutionResult {
    /// Hook executed successfully
    Success,
    /// Hook execution failed
    Failed(String),
    /// Hook was skipped
    Skipped(String),
}

impl ContainerNetworkHooks {
    /// Create new hooks manager
    pub fn new() -> Self {
        Self {
            hooks: Arc::new(RwLock::new(HashMap::new())),
            execution_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a network hook
    pub async fn register_hook<H>(&self, name: String, hook: H) -> Result<()>
    where
        H: ContainerNetworkHook + Send + Sync + 'static,
    {
        info!("Registering container network hook: {}", name);

        let mut hooks = self.hooks.write().await;
        hooks.insert(name.clone(), Box::new(hook));

        info!("Hook '{}' registered successfully", name);
        Ok(())
    }

    /// Attach the hook manager to the shared event bus.
    pub async fn bind_event_bus(&self, bus: Arc<EventBus>) -> EventBusResult<()> {
        bus.register_listener(
            "container-network-hooks",
            ContainerEventListener {
                hooks: self.clone(),
            },
        )
        .await
    }

    /// Unregister a network hook
    pub async fn unregister_hook(&self, name: &str) -> Result<()> {
        info!("Unregistering container network hook: {}", name);

        let mut hooks = self.hooks.write().await;
        if hooks.remove(name).is_some() {
            info!("Hook '{}' unregistered successfully", name);
            Ok(())
        } else {
            warn!("Hook '{}' not found", name);
            Err(ContainerError::NetworkOperation {
                message: format!("Hook '{}' not found", name),
            })
        }
    }

    /// Execute hooks for container lifecycle events
    pub async fn execute_lifecycle_hooks(
        &self,
        container_id: ContainerId,
        event_type: ContainerNetworkEventType,
        config: &ContainerNetworkConfig,
    ) -> Result<()> {
        info!(
            "Executing lifecycle hooks for container {} event {:?}",
            container_id, event_type
        );

        let hooks = self.hooks.read().await;
        let mut executions = Vec::new();

        for (name, hook) in hooks.iter() {
            let start_time = chrono::Utc::now();

            let result = match hook
                .on_lifecycle_event(container_id, &event_type, config)
                .await
            {
                Ok(()) => {
                    info!(
                        "Hook '{}' executed successfully for container {}",
                        name, container_id
                    );
                    HookExecutionResult::Success
                }
                Err(e) => {
                    error!(
                        "Hook '{}' failed for container {}: {}",
                        name, container_id, e
                    );
                    HookExecutionResult::Failed(e.to_string())
                }
            };

            let duration = chrono::Utc::now() - start_time;

            executions.push(HookExecution {
                hook_name: name.clone(),
                container_id,
                event_type: event_type.clone(),
                executed_at: start_time,
                result,
                duration,
            });
        }

        // Record executions
        {
            let mut history = self.execution_history.write().await;
            history.extend(executions);
        }

        Ok(())
    }

    /// Execute hooks for network configuration changes
    pub async fn execute_config_hooks(
        &self,
        container_id: ContainerId,
        old_config: Option<&ContainerNetworkConfig>,
        new_config: &ContainerNetworkConfig,
    ) -> Result<()> {
        info!(
            "Executing configuration hooks for container {}",
            container_id
        );

        let hooks = self.hooks.read().await;
        let mut executions = Vec::new();

        for (name, hook) in hooks.iter() {
            let start_time = chrono::Utc::now();

            let result = match hook
                .on_config_change(container_id, old_config, new_config)
                .await
            {
                Ok(()) => {
                    info!(
                        "Config hook '{}' executed successfully for container {}",
                        name, container_id
                    );
                    HookExecutionResult::Success
                }
                Err(e) => {
                    error!(
                        "Config hook '{}' failed for container {}: {}",
                        name, container_id, e
                    );
                    HookExecutionResult::Failed(e.to_string())
                }
            };

            let duration = chrono::Utc::now() - start_time;

            executions.push(HookExecution {
                hook_name: name.clone(),
                container_id,
                event_type: ContainerNetworkEventType::InterfaceUpdated,
                executed_at: start_time,
                result,
                duration,
            });
        }

        // Record executions
        {
            let mut history = self.execution_history.write().await;
            history.extend(executions);
        }

        Ok(())
    }

    /// Notify hooks about a broadcast system event so they can react to
    /// cross-service changes.
    pub async fn notify_system_event(&self, event: &SystemEvent) -> Result<()> {
        let hooks = self.hooks.read().await;
        for hook in hooks.values() {
            hook.on_system_event(event).await?;
        }
        Ok(())
    }

    /// Get hook execution history
    pub async fn get_execution_history(
        &self,
        container_id: Option<ContainerId>,
    ) -> Result<Vec<HookExecution>> {
        let history = self.execution_history.read().await;

        if let Some(id) = container_id {
            Ok(history
                .iter()
                .filter(|exec| exec.container_id == id)
                .cloned()
                .collect())
        } else {
            Ok(history.clone())
        }
    }

    /// Clear execution history
    pub async fn clear_execution_history(&self, container_id: Option<ContainerId>) -> Result<()> {
        let mut history = self.execution_history.write().await;

        if let Some(id) = container_id {
            history.retain(|exec| exec.container_id != id);
            info!("Cleared execution history for container {}", id);
        } else {
            history.clear();
            info!("Cleared all execution history");
        }

        Ok(())
    }

    /// List registered hooks
    pub async fn list_hooks(&self) -> Result<Vec<String>> {
        let hooks = self.hooks.read().await;
        Ok(hooks.keys().cloned().collect())
    }

    /// Get hook statistics
    pub async fn get_hook_statistics(&self) -> Result<HashMap<String, HookStatistics>> {
        let history = self.execution_history.read().await;
        let mut stats = HashMap::new();

        for execution in history.iter() {
            let hook_stats = stats
                .entry(execution.hook_name.clone())
                .or_insert_with(|| HookStatistics::new());

            hook_stats.total_executions += 1;

            match &execution.result {
                HookExecutionResult::Success => hook_stats.successful_executions += 1,
                HookExecutionResult::Failed(_) => hook_stats.failed_executions += 1,
                HookExecutionResult::Skipped(_) => hook_stats.skipped_executions += 1,
            }

            hook_stats.total_duration = hook_stats.total_duration + execution.duration;

            if hook_stats.max_duration < execution.duration {
                hook_stats.max_duration = execution.duration;
            }

            if hook_stats.min_duration > execution.duration {
                hook_stats.min_duration = execution.duration;
            }
        }

        // Calculate average durations
        for stat in stats.values_mut() {
            if stat.total_executions > 0 {
                stat.average_duration = stat.total_duration / stat.total_executions as i32;
            }
        }

        Ok(stats)
    }
}

impl Default for ContainerNetworkHooks {
    fn default() -> Self {
        Self::new()
    }
}

/// Hook statistics
#[derive(Debug, Clone)]
pub struct HookStatistics {
    /// Total number of executions
    pub total_executions: u64,
    /// Number of successful executions
    pub successful_executions: u64,
    /// Number of failed executions
    pub failed_executions: u64,
    /// Number of skipped executions
    pub skipped_executions: u64,
    /// Total execution duration
    pub total_duration: chrono::Duration,
    /// Average execution duration
    pub average_duration: chrono::Duration,
    /// Maximum execution duration
    pub max_duration: chrono::Duration,
    /// Minimum execution duration
    pub min_duration: chrono::Duration,
}

impl HookStatistics {
    fn new() -> Self {
        Self {
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            skipped_executions: 0,
            total_duration: chrono::Duration::zero(),
            average_duration: chrono::Duration::zero(),
            max_duration: chrono::Duration::zero(),
            min_duration: chrono::Duration::max_value(),
        }
    }
}

/// Container network hook trait
#[async_trait]
pub trait ContainerNetworkHook {
    /// Called when container lifecycle events occur
    async fn on_lifecycle_event(
        &self,
        container_id: ContainerId,
        event_type: &ContainerNetworkEventType,
        config: &ContainerNetworkConfig,
    ) -> Result<()>;

    /// Called when container network configuration changes
    async fn on_config_change(
        &self,
        container_id: ContainerId,
        old_config: Option<&ContainerNetworkConfig>,
        new_config: &ContainerNetworkConfig,
    ) -> Result<()>;

    /// Called when the shared system event bus delivers a message.
    async fn on_system_event(&self, _event: &SystemEvent) -> Result<()> {
        Ok(())
    }

    /// Hook name for identification
    fn name(&self) -> &str;

    /// Hook description
    fn description(&self) -> &str {
        "Container network hook"
    }
}

struct ContainerEventListener {
    hooks: ContainerNetworkHooks,
}

#[async_trait]
impl EventListener for ContainerEventListener {
    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()> {
        match event {
            SystemEvent::ContainerStarted { id } => {
                let config = ContainerNetworkConfig::new(*id);
                self.hooks
                    .execute_lifecycle_hooks(
                        *id,
                        ContainerNetworkEventType::ContainerStarted,
                        &config,
                    )
                    .await
                    .map_err(anyhow::Error::from)?;
            }
            SystemEvent::NetworkApplied { .. } => {
                info!("Container hooks reacting to applied network changes");
            }
            _ => {}
        }

        self.hooks
            .notify_system_event(event)
            .await
            .map_err(anyhow::Error::from)?;

        Ok(())
    }
}

/// Built-in hook for logging network events
pub struct NetworkEventLogger {
    name: String,
}

impl NetworkEventLogger {
    pub fn new() -> Self {
        Self {
            name: "network-event-logger".to_string(),
        }
    }
}

#[async_trait]
impl ContainerNetworkHook for NetworkEventLogger {
    async fn on_lifecycle_event(
        &self,
        container_id: ContainerId,
        event_type: &ContainerNetworkEventType,
        config: &ContainerNetworkConfig,
    ) -> Result<()> {
        info!(
            "Container {} lifecycle event {:?} - {} interfaces configured",
            container_id,
            event_type,
            config.interfaces.len()
        );
        Ok(())
    }

    async fn on_config_change(
        &self,
        container_id: ContainerId,
        old_config: Option<&ContainerNetworkConfig>,
        new_config: &ContainerNetworkConfig,
    ) -> Result<()> {
        let old_count = old_config.map(|c| c.interfaces.len()).unwrap_or(0);
        let new_count = new_config.interfaces.len();

        info!(
            "Container {} network config changed: {} -> {} interfaces",
            container_id, old_count, new_count
        );
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Logs container network events and configuration changes"
    }
}

/// Built-in hook for VNet validation
pub struct VNetValidationHook {
    name: String,
}

impl VNetValidationHook {
    pub fn new() -> Self {
        Self {
            name: "vnet-validation".to_string(),
        }
    }
}

#[async_trait]
impl ContainerNetworkHook for VNetValidationHook {
    async fn on_lifecycle_event(
        &self,
        container_id: ContainerId,
        event_type: &ContainerNetworkEventType,
        config: &ContainerNetworkConfig,
    ) -> Result<()> {
        // Validate VNet configurations
        for interface in config.vnet_interfaces() {
            if let Some(vnet) = &interface.vnet {
                // TODO: Validate VNet exists and is accessible
                info!("Validating VNet '{}' for container {}", vnet, container_id);
            }
        }
        Ok(())
    }

    async fn on_config_change(
        &self,
        container_id: ContainerId,
        _old_config: Option<&ContainerNetworkConfig>,
        new_config: &ContainerNetworkConfig,
    ) -> Result<()> {
        // Validate new VNet configurations
        for interface in new_config.vnet_interfaces() {
            if let Some(vnet) = &interface.vnet {
                info!(
                    "Validating new VNet '{}' configuration for container {}",
                    vnet, container_id
                );
                // TODO: Perform actual VNet validation
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Validates VNet configurations for containers"
    }
}

/// Future Rust pve-container integration hook
pub struct RustContainerIntegrationHook {
    name: String,
}

impl RustContainerIntegrationHook {
    pub fn new() -> Self {
        Self {
            name: "rust-container-integration".to_string(),
        }
    }
}

#[async_trait]
impl ContainerNetworkHook for RustContainerIntegrationHook {
    async fn on_lifecycle_event(
        &self,
        container_id: ContainerId,
        event_type: &ContainerNetworkEventType,
        config: &ContainerNetworkConfig,
    ) -> Result<()> {
        info!(
            "Rust container integration hook: container {} event {:?}",
            container_id, event_type
        );

        // TODO: When pve-container is migrated to Rust, this hook will:
        // 1. Coordinate network setup with container lifecycle
        // 2. Share data structures between network and container management
        // 3. Provide efficient Rust-to-Rust communication
        // 4. Handle container migration network coordination

        // For now, just log the event
        match event_type {
            ContainerNetworkEventType::ContainerStarted => {
                info!("Container {} started - network ready", container_id);
            }
            ContainerNetworkEventType::ContainerStopped => {
                info!("Container {} stopped - cleaning up network", container_id);
            }
            ContainerNetworkEventType::ContainerMigrated => {
                info!(
                    "Container {} migrated - updating network configuration",
                    container_id
                );
            }
            _ => {}
        }

        Ok(())
    }

    async fn on_config_change(
        &self,
        container_id: ContainerId,
        old_config: Option<&ContainerNetworkConfig>,
        new_config: &ContainerNetworkConfig,
    ) -> Result<()> {
        info!(
            "Rust container integration: config change for container {}",
            container_id
        );

        // TODO: Coordinate configuration changes with Rust pve-container
        // This would include:
        // 1. Sharing configuration data structures
        // 2. Coordinating network and container state changes
        // 3. Providing efficient update mechanisms

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "Integration hook for future Rust pve-container implementation"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hook_registration() {
        let hooks = ContainerNetworkHooks::new();
        let logger = NetworkEventLogger::new();

        assert!(hooks
            .register_hook("logger".to_string(), logger)
            .await
            .is_ok());

        let hook_list = hooks.list_hooks().await.unwrap();
        assert!(hook_list.contains(&"logger".to_string()));

        assert!(hooks.unregister_hook("logger").await.is_ok());

        let hook_list_after = hooks.list_hooks().await.unwrap();
        assert!(!hook_list_after.contains(&"logger".to_string()));
    }

    #[tokio::test]
    async fn test_lifecycle_hooks() {
        let hooks = ContainerNetworkHooks::new();
        let logger = NetworkEventLogger::new();

        hooks
            .register_hook("logger".to_string(), logger)
            .await
            .unwrap();

        let config = ContainerNetworkConfig::new(100);

        assert!(hooks
            .execute_lifecycle_hooks(100, ContainerNetworkEventType::ContainerStarted, &config)
            .await
            .is_ok());

        let history = hooks.get_execution_history(Some(100)).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].container_id, 100);
    }

    #[tokio::test]
    async fn test_hook_statistics() {
        let hooks = ContainerNetworkHooks::new();
        let logger = NetworkEventLogger::new();

        hooks
            .register_hook("logger".to_string(), logger)
            .await
            .unwrap();

        let config = ContainerNetworkConfig::new(100);

        // Execute multiple hooks
        for _ in 0..3 {
            hooks
                .execute_lifecycle_hooks(100, ContainerNetworkEventType::ContainerStarted, &config)
                .await
                .unwrap();
        }

        let stats = hooks.get_hook_statistics().await.unwrap();
        let logger_stats = stats.get("logger").unwrap();

        assert_eq!(logger_stats.total_executions, 3);
        assert_eq!(logger_stats.successful_executions, 3);
        assert_eq!(logger_stats.failed_executions, 0);
    }
}
