use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use container_integration::ContainerIntegration;
use net_migration::hooks::{MigrationEventLogger, MigrationHooks};
use pve_event_bus::EventBus;
use pve_network_apply::{ifupdown::IfUpDownIntegration, rollback::RollbackManager, NetworkApplier};
use pve_network_config::{NetworkConfigManager, PmxcfsConfig};
use pve_network_validate::NetworkValidator;
use pve_shared_types::MigrationPhase;
use storage_integration::{
    future_integration::{DefaultFutureStorageIntegration, FutureStorageIntegration},
    hooks::{StorageEventLogger, StorageHooks, StorageStatusRefresher, StorageVlanReconciler},
    path_resolution::{DefaultStoragePathResolver, StoragePathResolver},
    storage_network::{DefaultStorageNetworkManager, NetworkConfigTrait},
    StorageNetworkManager, StorageVlanManager,
};
use crate::{container::ContainerNetworkAPI, sdn::SdnApiState, NetworkAPI, StorageNetworkAPI};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct AppContext {
    pub event_bus: Arc<EventBus>,
    pub network_api: Arc<NetworkAPI>,
    pub container_integration: Arc<ContainerIntegration>,
    pub container_api: Arc<ContainerNetworkAPI>,
    pub storage_hooks: Arc<StorageHooks>,
    pub storage_api: Arc<StorageNetworkAPI>,
    pub storage_network_manager: Arc<dyn StorageNetworkManager + Send + Sync>,
    pub storage_vlan_manager: Arc<RwLock<StorageVlanManager>>,
    pub storage_path_resolver: Arc<dyn StoragePathResolver + Send + Sync>,
    pub migration_hooks: Arc<MigrationHooks>,
    pub future_storage_integration: Arc<dyn FutureStorageIntegration + Send + Sync>,
    pub network_applier: Arc<NetworkApplier>,
    pub sdn_state: Arc<SdnApiState>,
}

impl AppContext {
    pub async fn bootstrap() -> Result<Arc<Self>> {
        let event_bus = Arc::new(EventBus::new());

        let pmxcfs = Arc::new(PmxcfsConfig::new().unwrap_or_else(|_| PmxcfsConfig::mock()));
        let config_manager = Arc::new(NetworkConfigManager::with_pmxcfs((*pmxcfs).clone()));

        let validator = Arc::new(NetworkValidator::new());
        let ifupdown = Arc::new(IfUpDownIntegration::new());
        let rollback_manager = Arc::new(
            RollbackManager::new(Some(config_manager.clone()), None)
                .await
                .map_err(|err| anyhow::anyhow!(err))?,
        );

        let network_applier = NetworkApplier::new(
            config_manager.clone(),
            validator.clone(),
            ifupdown.clone(),
            rollback_manager.clone(),
            pmxcfs.clone(),
        )
        .await
        .map_err(|err| anyhow::anyhow!(err))?
        .with_event_bus(event_bus.clone());
        let network_applier = Arc::new(network_applier);

        let network_api = Arc::new(NetworkAPI::with_shared_config_manager(
            config_manager.clone(),
        ));

        let container_integration = Arc::new(ContainerIntegration::new());
        container_integration
            .bind_event_bus(event_bus.clone())
            .await
            .map_err(|err| anyhow::anyhow!(err))?;
        let container_api = Arc::new(ContainerNetworkAPI::new(container_integration.clone()));

        let future_integration: Arc<dyn FutureStorageIntegration + Send + Sync> =
            Arc::new(DefaultFutureStorageIntegration::new());
        future_integration.initialize().await?;

        let storage_hooks = Arc::new(StorageHooks::new(future_integration.clone()));

        let storage_vlan_manager = Arc::new(RwLock::new(StorageVlanManager::with_event_bus(
            event_bus.clone(),
        )));

        let network_config_trait: Arc<dyn NetworkConfigTrait> = config_manager.clone();
        let storage_network_manager: Arc<dyn StorageNetworkManager + Send + Sync> = Arc::new(
            DefaultStorageNetworkManager::new(network_config_trait.clone()),
        );

        let mut resolver = DefaultStoragePathResolver::new(PathBuf::from("/mnt/pve"));
        resolver
            .load_from_pve_storage()
            .await
            .map_err(|err| anyhow::anyhow!(err))?;
        let storage_path_resolver: Arc<dyn StoragePathResolver + Send + Sync> = Arc::new(resolver);

        let storage_api = Arc::new(StorageNetworkAPI::new(
            storage_network_manager.clone(),
            storage_vlan_manager.clone(),
            storage_path_resolver.clone(),
            future_integration.clone(),
        ));

        storage_hooks
            .register_hook(StorageEventLogger)
            .await
            .map_err(|err| anyhow::anyhow!(err))?;
        storage_hooks
            .register_hook(StorageStatusRefresher::new(future_integration.clone()))
            .await
            .map_err(|err| anyhow::anyhow!(err))?;
        storage_hooks
            .register_hook(StorageVlanReconciler::new(storage_vlan_manager.clone()))
            .await
            .map_err(|err| anyhow::anyhow!(err))?;
        storage_hooks
            .bind_event_bus(event_bus.clone())
            .await
            .map_err(|err| anyhow::anyhow!(err))?;

        let migration_hooks = Arc::new(MigrationHooks::new(MigrationPhase::PerlOnly));
        migration_hooks
            .register_hook(MigrationEventLogger)
            .await
            .map_err(|err| anyhow::anyhow!(err))?;
        migration_hooks
            .bind_event_bus(event_bus.clone())
            .await
            .map_err(|err| anyhow::anyhow!(err))?;

        let sdn_state = Arc::new(SdnApiState::new());

        Ok(Arc::new(Self {
            event_bus,
            network_api,
            container_integration,
            container_api,
            storage_hooks,
            storage_api,
            storage_network_manager,
            storage_vlan_manager,
            storage_path_resolver,
            migration_hooks,
            future_storage_integration: future_integration,
            network_applier,
            sdn_state,
        }))
    }
}
