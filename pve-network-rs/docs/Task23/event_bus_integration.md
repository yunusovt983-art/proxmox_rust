# Task 23 – Введение шины событий и интеграция хуков

Цель этого шага — наладить общий канал коммуникации между основными подсистемами (сеть, контейнеры, сторидж, миграция). Ниже подробно описано, какие файлы затронуты, какой код появился и как теперь использовать новые возможности.

---

## 1. Общие типы событий (`crates/pve-shared-types/src/events.rs`)
Мы обновили раздел с системными событиями, чтобы все сервисы оперировали единой моделью данных.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SystemEvent {
    NetworkApplied { changes: Vec<ConfigChange> },
    ContainerStarted { id: ContainerId },
    StorageVlanCreated { id: String },
    MigrationPhaseChanged { phase: MigrationPhase },
    Custom { name: String, data: HashMap<String, serde_json::Value> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType { Create, Update, Delete, Modify }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigChange {
    pub change_type: ChangeType,
    pub target: String,
    pub old_config: Option<serde_json::Value>,
    pub new_config: Option<serde_json::Value>,
    pub description: String,
}
```

Все три типа реэкспортируются через `pve_shared_types::SystemEvent`, `ChangeType` и `ConfigChange`, что упрощает их повторное использование.

---

## 2. Новый крейт `pve-event-bus` (`crates/event-bus`)
Появился отдельный крейт, реализующий асинхронную шину событий на `tokio`:

```rust
#[derive(Clone, Default)]
pub struct EventBus {
    listeners: Arc<RwLock<HashMap<String, Arc<dyn EventListener>>>>,
}

#[async_trait]
pub trait EventListener: Send + Sync {
    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()>;
}
```

- `register_listener` не позволяет зарегистрировать слушателя с уже существующим именем.
- `publish` последовательно вызывает `on_event` у всех слушателей, собирая ошибки в `EventBusError::ListenerFailures`, но не прерывая рассылку.
- Любые индивидуальные ошибки логируются через `warn!`, чтобы владельцы хука получили сигнал о сбое.

Крейт добавлен в workspace, а зависимость `pve-event-bus = { path = "../event-bus" }` подключена в `container-integration`, `storage-integration` и `net-migration`.

---

## 3. Контейнерные хуки (`crates/container-integration/src/hooks.rs`)
`ContainerNetworkHooks` теперь умеет подключаться к шине и передавать события хукам:

```rust
#[derive(Clone)]
pub struct ContainerNetworkHooks {
    hooks: Arc<RwLock<HashMap<String, Box<dyn ContainerNetworkHook + Send + Sync>>>>,
    execution_history: Arc<RwLock<Vec<HookExecution>>>,
}

impl ContainerNetworkHooks {
    pub async fn bind_event_bus(&self, bus: Arc<EventBus>) -> EventBusResult<()> {
        bus.register_listener(
            "container-network-hooks",
            ContainerEventListener { hooks: self.clone() },
        ).await
    }

    pub async fn notify_system_event(&self, event: &SystemEvent) -> Result<()> {
        for hook in self.hooks.read().await.values() {
            hook.on_system_event(event).await?;
        }
        Ok(())
    }
}
```

Каждый `ContainerNetworkHook` получил новый метод по умолчанию:

```rust
#[async_trait]
pub trait ContainerNetworkHook {
    async fn on_system_event(&self, _event: &SystemEvent) -> Result<()> { Ok(()) }
    // ... остальные методы без изменений
}
```

`ContainerEventListener` реагирует на `SystemEvent::ContainerStarted` и инициирует выполнение жизненных хуков:

```rust
#[async_trait]
impl EventListener for ContainerEventListener {
    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()> {
        if let SystemEvent::ContainerStarted { id } = event {
            let config = ContainerNetworkConfig::new(*id);
            self.hooks.execute_lifecycle_hooks(
                *id,
                ContainerNetworkEventType::ContainerStarted,
                &config,
            ).await?;
        }

        self.hooks.notify_system_event(event).await?;
        Ok(())
    }
}
```

Кроме того, в `ContainerIntegration` добавлен простейший фасад:

```rust
impl ContainerIntegration {
    pub async fn bind_event_bus(&self, bus: Arc<EventBus>) -> EventBusResult<()> {
        self.hooks.bind_event_bus(bus).await
    }
}
```

---

## 4. Хуки сториджа (`crates/storage-integration/src/hooks.rs`)
Создан отдельный менеджер с интерфейсом, аналогичным контейнерному:

```rust
#[derive(Clone)]
pub struct StorageHooks {
    hooks: Arc<RwLock<HashMap<String, Arc<dyn StorageHook>>>>,
    future_integration: Arc<dyn FutureStorageIntegration + Send + Sync>,
}

impl StorageHooks {
    pub fn new(future_integration: Arc<dyn FutureStorageIntegration + Send + Sync>) -> Self { /* ... */ }
    pub async fn notify(&self, event: &SystemEvent) -> StorageResult<()> { /* ... */ }
}

pub struct StorageStatusRefresher {
    integration: Arc<dyn FutureStorageIntegration + Send + Sync>,
}

#[async_trait]
impl StorageHook for StorageStatusRefresher {
    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()> {
        match event {
            SystemEvent::NetworkApplied { .. } => { /* list_storage_backends + get_storage_network_status */ }
            SystemEvent::StorageVlanCreated { id } => { /* get_storage_network_status(id) */ }
            _ => {}
        }
        Ok(())
    }
}
```

Помимо логирующего `StorageEventLogger`, теперь есть «боевой» `StorageStatusRefresher`, который после применения сетевой конфигурации обновляет статус всех стораджей, а при создании VLAN — актуализирует конкретный бэкенд. Всё это использует реальные методы `FutureStorageIntegration`.

Интерфейс экспортирован через `pub use hooks::{StorageEventLogger, StorageHook, StorageHooks, StorageStatusRefresher};`.

---

## 5. Хуки миграции (`crates/net-migration/src/hooks.rs`)
Точно такой же паттерн реализован для миграционного слоя:

```rust
#[derive(Clone)]
pub struct MigrationHooks {
    hooks: Arc<RwLock<HashMap<String, Arc<dyn MigrationHook>>>>,
    current_phase: Arc<RwLock<MigrationPhase>>,
    network_applied_events: Arc<RwLock<u64>>,
}

impl MigrationHooks {
    pub fn new(initial_phase: MigrationPhase) -> Self { /* ... */ }
    pub async fn current_phase(&self) -> MigrationPhase { /* ... */ }
    pub async fn network_applied_count(&self) -> u64 { /* ... */ }
}

#[async_trait]
impl EventListener for MigrationEventListener {
    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()> {
        match event {
            SystemEvent::MigrationPhaseChanged { phase } => { /* обновляем phase */ }
            SystemEvent::NetworkApplied { .. } => { /* увеличиваем счётчик */ }
            _ => {}
        }
        self.hooks.notify(event).await?;
        Ok(())
    }
}
```

Таким образом, миграционный слой теперь хранит актуальную фазу и количество применённых сетевых транзакций, что открывает путь к более информативным метрикам/решениям на основе этих данных. `MigrationEventLogger` оставлен как пример дополнительно регистрируемого хука.

Дополнительно `StorageVlanManager::create_storage_vlan` публикует `SystemEvent::StorageVlanCreated`, так что хуки сториджа получают сигнал сразу после успешного создания VLAN.

---

## 6. Использование общих `ConfigChange` (`crates/net-apply`)
`pve-network-apply` теперь использует единые типы изменений из `pve-shared-types`, поэтому события `NetworkApplied` содержат ровно те же структуры, что формирует транзакционный апплайер:

```rust
use pve_shared_types::{ChangeType, ConfigChange};
```

Это исключает расхождения при сериализации и уведомлении подписчиков.

---

## 7. Как подключать шину событий
Ниже пример инициализации (псевдо-код) на уровне API или сервиса:

```rust
let bus = Arc::new(EventBus::new());

let container = Arc::new(ContainerIntegration::new());
container.bind_event_bus(bus.clone()).await?;

let future_integration: Arc<dyn FutureStorageIntegration + Send + Sync> =
    Arc::new(DefaultFutureStorageIntegration::new());
future_integration.initialize().await?;
let storage_hooks = StorageHooks::new(Arc::clone(&future_integration));
storage_hooks.register_hook(StorageEventLogger).await?;
storage_hooks
    .register_hook(StorageStatusRefresher::new(Arc::clone(&future_integration)))
    .await?;
storage_hooks.bind_event_bus(bus.clone()).await?;

let migration_hooks = MigrationHooks::new(MigrationPhase::PerlOnly);
migration_hooks.register_hook(MigrationEventLogger).await?;
migration_hooks.bind_event_bus(bus.clone()).await?;

// Пример публикации события после применённой конфигурации:
let changes: Vec<ConfigChange> = gather_changes();
bus.publish(SystemEvent::NetworkApplied { changes }).await?;
```

Любой сервис, подписанный на шину, получит событие и выполнит собственные хуки/обновление состояния.

### 7.1 Реализация в API-сервере
Файл `crates/net-api/src/bin/api-server.rs` теперь создает общую инфраструктуру при запуске:

```rust
let event_bus = Arc::new(EventBus::new());

let container_integration = Arc::new(ContainerIntegration::new());
container_integration
    .bind_event_bus(event_bus.clone())
    .await?;

let future_integration: Arc<dyn FutureStorageIntegration + Send + Sync> =
    Arc::new(DefaultFutureStorageIntegration::new());
future_integration.initialize().await?;

let storage_hooks = Arc::new(StorageHooks::new(Arc::clone(&future_integration)));
storage_hooks.register_hook(StorageEventLogger).await?;
storage_hooks
    .register_hook(StorageStatusRefresher::new(Arc::clone(&future_integration)))
    .await?;
storage_hooks.bind_event_bus(event_bus.clone()).await?;

let migration_hooks = Arc::new(MigrationHooks::new(MigrationPhase::PerlOnly));
migration_hooks.register_hook(MigrationEventLogger).await?;
migration_hooks.bind_event_bus(event_bus.clone()).await?;

let _context_guard = (
    Arc::clone(&event_bus),
    container_integration,
    future_integration,
    storage_hooks,
    migration_hooks,
);
```

Таким образом, единая шина и все базовые слушатели поднимаются до старта HTTP-сервера и живут на протяжении всего его жизненного цикла.

---

## 8. Проверка
- `cargo fmt --all`
- `cargo check --workspace`
- точечно: `cargo check -p pve-network-apply`, `cargo check -p storage-integration`, `cargo check -p net-migration`

---

## 9. Что дальше
1. Выбрать место, где будет создан единый `EventBus` (скорее всего, bootstrap REST API).
2. Заменить логирующие хуки на реальные действия (пересоздание VLAN, обновление контейнеров, реакция миграции).
3. При необходимости — добавить журналирование/хранение истории событий.
4. Подумать о гарантированной доставке/повторных попытках, если это требуется бизнес-логикой.

---

Таким образом, шаг 23 создал фундаментальную шину событий и связал с ней существующие менеджеры хуков. Теперь добавление новых реакций сводится к написанию небольшого хука и его регистрации.
