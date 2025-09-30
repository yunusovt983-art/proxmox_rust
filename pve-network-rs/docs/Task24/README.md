# Task 24 – Интеграция AppContext и унификация API

## Что сделано

### Eдиный контекст приложения
- `crates/net-api/src/context.rs`: реализован `AppContext::bootstrap`, который собирает `EventBus`, `ContainerIntegration`, `StorageHooks`, `MigrationHooks`, `NetworkApplier`, общий `SdnApiState` и прокидывает `Arc<AppContext>` во все сервисы.
- В этот же контекст добавлены: `StorageVlanManager` с привязкой к шине событий, общий `StorageNetworkManager`, `StoragePathResolver`, `ContainerNetworkAPI`, `StorageNetworkAPI`.

```rust
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
    .await?
    .with_event_bus(event_bus.clone());
    …
}
```
*Инициализация собирает все зависимости (pmxcfs, валидация, ifupdown, rollback) и связывает `NetworkApplier` с общей шиной событий.*

### Хуки и сторидж-интеграция
- `crates/storage-integration/src/storage_network.rs`: добавлен `impl NetworkConfigTrait for NetworkConfigManager`, чтобы использовать реальные сетевые конфиги.
- `crates/storage-integration/src/hooks.rs`: новый `StorageVlanReconciler` — при событии `SystemEvent::NetworkApplied` пересоздает VLAN на основе сохраненных определений.
- `crates/storage-integration/src/vlan_isolation.rs`: метод `configured_vlans()` возвращает снимок конфигов для reconciler’а.

```rust
#[async_trait::async_trait]
impl NetworkConfigTrait for NetworkConfigManager {
    async fn get_configuration(&self) -> anyhow::Result<NetworkConfiguration> {
        self.get_current_config()
            .await
            .map_err(|err| anyhow::anyhow!(err))
    }

    async fn set_configuration(
        &self,
        config: &NetworkConfiguration,
    ) -> anyhow::Result<()> {
        self.write_config(config)
            .await
            .map_err(|err| anyhow::anyhow!(err))
    }
}
```
*Storage-слой теперь напрямую читает/пишет сетевые конфиги через общий менеджер.*

```rust
#[async_trait]
impl StorageHook for StorageVlanReconciler {
    fn name(&self) -> &str { "storage-vlan-reconciler" }

    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()> {
        if let SystemEvent::NetworkApplied { .. } = event {
            let configs = {
                let manager = self.vlan_manager.read().await;
                manager.configured_vlans()
            };

            for (storage_id, config) in configs {
                let mut manager = self.vlan_manager.write().await;
                if let Err(err) = manager.create_storage_vlan(&storage_id, &config).await {
                    warn!("Failed to reconcile storage VLAN for {}: {}", storage_id, err);
                }
            }
        }
        Ok(())
    }
}
```
*После применения сети автоматом пересоздаются VLAN для стораджей.*

### REST-API
- `crates/net-api/src/storage.rs`: построен роутер на `AppContext`, введены обертки `StorageResponse/StorageErrorResponse`, каждая ручка дергает `context.storage_api` и правильно мапит `StorageIntegrationError`.
- `crates/net-api/src/container.rs`: роутер контейнеров переводит ответы интеграции в JSON, статусы hotplug сериализуются в текст, добавлены типы `ContainerResponse`, `ContainerErrorResponse`.
- `crates/net-api/src/sdn.rs`: все обработчики используют `context.sdn_state`, добавлен роутер, сохраняющий совместимость с axum-состоянием.
- `crates/net-api/src/network.rs`: роутер теперь принимает готовый `Arc<AppContext>`.

```rust
fn map_storage_error(err: StorageIntegrationError)
    -> (StatusCode, Json<StorageErrorResponse>) {
    let status = match err {
        Configuration(_) | UnsupportedBackend(_) | NetworkInterface(_)
        | VlanConfiguration(_) | PathResolution(_) => StatusCode::BAD_REQUEST,
        StoragePlugin(_) => StatusCode::BAD_GATEWAY,
        System(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(StorageErrorResponse { error: err.to_string() }))
}

async fn list_storage_networks(
    State(context): State<Arc<AppContext>>,
    Path(node): Path<String>,
) -> Result<Json<StorageResponse<Vec<StorageNetworkInfo>>>, (StatusCode, Json<StorageErrorResponse>)>
{
    context
        .storage_api
        .list_storage_networks(node)
        .await
        .map(|data| Json(StorageResponse { data }))
        .map_err(map_storage_error)
}
```
*Каждый endpoint использует общие зависимости и унифицированные ответы.*

```rust
async fn get_hotplug_operation(
    State(context): State<Arc<AppContext>>,
    Path((node, vmid, operation_id)): Path<(String, String, String)>,
) -> Result<
    Json<ContainerResponse<Option<HotplugOperationResponse>>>,
    (StatusCode, Json<ContainerErrorResponse>),
> {
    let container_id = parse_container_id(&vmid)?;
    context
        .container_api
        .get_hotplug_operation(node, container_id, operation_id)
        .await
        .map(|data| {
            let response = data.map(|op| HotplugOperationResponse {
                operation_id: op.id,
                container_id: op.container_id,
                status: match op.status {
                    HotplugStatus::InProgress => "in-progress".to_string(),
                    HotplugStatus::Completed => "completed".to_string(),
                    HotplugStatus::Failed => "failed".to_string(),
                },
            });
            Json(ContainerResponse { data: response })
        })
        .map_err(container_error)
}
```
*Контейнерные ручки отдают согласованный JSON и конвертируют внутренние состояния hotplug.*

### HTTP сервер и миграция
- `crates/net-api/src/bin/api-server.rs`: сервер стартует через `AppContext::bootstrap`, объединяет роутеры Network/Storage/Container/SDN, добавляет `/metrics/migration`, расширяет `/health` (фаза миграции + кол-во `NetworkApplied`).
- `crates/net-api/src/migration.rs`: `NetApiRustHandler` хранит `Arc<AppContext>` и делегирует вызовы через реальный API (network/container/storage).

```rust
let app = Router::new()
    .merge(NetworkAPI::router())
    .merge(StorageNetworkAPI::router())
    .merge(ContainerNetworkAPI::router())
    .merge(SDNAPI::router())
    .route("/", get(root))
    .route("/health", get(health_check))
    .route("/metrics/migration", get(migration_metrics))
    …
    .with_state(context.clone());

async fn migration_metrics(
    State(context): State<Arc<AppContext>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let phase = context.migration_hooks.current_phase().await;
    let applied = context.migration_hooks.network_applied_count().await;

    Ok(Json(json!({
        "phase": phase,
        "network_applied_events": applied
    })))
}
```
*Роутер строится один раз, все сервисы получают доступ к метрикам миграции.*

### CLI
- `crates/net-cli/src/commands/*.rs`: команды получают `Arc<AppContext>`; `ApplyCommand` использует реальный `NetworkApplier`, `StatusCommand` выводит `migration_phase` и `network_applied_count`, `CompatCommand`/`ValidateCommand` и др. приведены к единому интерфейсу.
- `crates/net-cli/src/main.rs`: один раз вызывает `AppContext::bootstrap`, раздает клон в команды.

```rust
let context = AppContext::bootstrap().await?;

let result = match cli.command {
    Commands::Apply { … } => {
        let cmd = ApplyCommand::new(context.clone());
        …
    }
    Commands::Status { … } => {
        let cmd = StatusCommand::new(context.clone());
        …
    }
    …
};
```
*CLI использует тот же контекст, что и HTTP-сервер.*

```rust
async fn show_migration_metrics(&self) -> Result<()> {
    let phase = self.context.migration_hooks.current_phase().await;
    let applied = self.context.migration_hooks.network_applied_count().await;

    println!("\nMigration status:");
    println!("  Current phase: {:?}", phase);
    println!("  Network apply events observed: {}", applied);
    Ok(())
}
```
*`StatusCommand` выводит актуальные миграционные показатели.*

### Документация
- Создан файл `docs/Task24/README.md` (текущий документ) с описанием изменений.

## Запущенные команды
```
ls
sed/grep/apply_patch (серия правок в коде)
cargo fmt --all
cargo check --workspace
```
