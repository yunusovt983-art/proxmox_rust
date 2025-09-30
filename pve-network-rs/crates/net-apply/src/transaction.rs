//! Transactional network configuration application

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::Mutex;

use pve_event_bus::EventBus;
use pve_network_config::{NetworkConfigManager, PmxcfsConfig};
use pve_network_core::{NetworkConfiguration, Result};
use pve_network_validate::NetworkValidator;
use pve_shared_types::{ChangeType, ConfigChange, SystemEvent};

use crate::ifupdown::IfUpDownIntegration;
use crate::rollback::RollbackManager;

/// Transaction state for network configuration changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Unique transaction ID
    pub id: String,
    /// Timestamp when transaction was created
    pub timestamp: u64,
    /// Original configuration before changes
    pub original_config: NetworkConfiguration,
    /// New configuration to apply
    pub new_config: NetworkConfiguration,
    /// Current transaction state
    pub state: TransactionState,
    /// Changes made in this transaction
    pub changes: Vec<ConfigChange>,
    /// Transaction metadata
    pub metadata: HashMap<String, String>,
}

/// Transaction states
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionState {
    /// Transaction created but not started
    Created,
    /// Transaction is being validated
    Validating,
    /// Transaction validation completed successfully
    Validated,
    /// Transaction is being applied
    Applying,
    /// Transaction applied successfully
    Applied,
    /// Transaction is being committed
    Committing,
    /// Transaction committed successfully
    Committed,
    /// Transaction is being rolled back
    RollingBack,
    /// Transaction rolled back successfully
    RolledBack,
    /// Transaction failed
    Failed,
}

/// Result of applying a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    /// Transaction ID
    pub transaction_id: String,
    /// Whether application was successful
    pub success: bool,
    /// Changes that were applied
    pub applied_changes: Vec<ConfigChange>,
    /// Any warnings generated during application
    pub warnings: Vec<String>,
    /// Error message if application failed
    pub error: Option<String>,
    /// Time taken to apply changes (in milliseconds)
    pub duration_ms: u64,
}

/// Network applier with transaction support
pub struct NetworkApplier {
    /// Configuration manager
    config_manager: Arc<NetworkConfigManager>,
    /// Validator for configuration changes
    validator: Arc<NetworkValidator>,
    /// ifupdown2 integration
    ifupdown: Arc<IfUpDownIntegration>,
    /// Rollback manager
    rollback_manager: Arc<RollbackManager>,
    /// pmxcfs integration for cluster synchronization
    pmxcfs: Arc<PmxcfsConfig>,
    /// Active transactions
    active_transactions: Arc<Mutex<HashMap<String, Transaction>>>,
    /// Transaction log directory
    transaction_log_dir: PathBuf,
    /// Optional event bus for broadcasting applied changes
    event_bus: Option<Arc<EventBus>>,
}

impl NetworkApplier {
    /// Create new network applier
    pub async fn new(
        config_manager: Arc<NetworkConfigManager>,
        validator: Arc<NetworkValidator>,
        ifupdown: Arc<IfUpDownIntegration>,
        rollback_manager: Arc<RollbackManager>,
        pmxcfs: Arc<PmxcfsConfig>,
    ) -> Result<Self> {
        let transaction_log_dir = PathBuf::from("/var/log/pve-network/transactions");
        Self::new_with_log_dir(
            config_manager,
            validator,
            ifupdown,
            rollback_manager,
            pmxcfs,
            transaction_log_dir,
        )
        .await
    }

    /// Create new network applier with custom transaction log directory
    pub async fn new_with_log_dir(
        config_manager: Arc<NetworkConfigManager>,
        validator: Arc<NetworkValidator>,
        ifupdown: Arc<IfUpDownIntegration>,
        rollback_manager: Arc<RollbackManager>,
        pmxcfs: Arc<PmxcfsConfig>,
        transaction_log_dir: PathBuf,
    ) -> Result<Self> {
        // Ensure transaction log directory exists
        if !transaction_log_dir.exists() {
            fs::create_dir_all(&transaction_log_dir).await?;
        }

        Ok(Self {
            config_manager,
            validator,
            ifupdown,
            rollback_manager,
            pmxcfs,
            active_transactions: Arc::new(Mutex::new(HashMap::new())),
            transaction_log_dir,
            event_bus: None,
        })
    }

    /// Attach an event bus so applied changes can be broadcast to subscribers.
    pub fn with_event_bus(mut self, event_bus: Arc<EventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    /// Begin a new transaction for configuration changes
    pub async fn begin_transaction(&self, new_config: NetworkConfiguration) -> Result<Transaction> {
        let transaction_id = self.generate_transaction_id();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Get current configuration
        let original_config = self.config_manager.get_current_config().await?;

        // Calculate changes
        let changes = self.calculate_changes(&original_config, &new_config)?;

        let transaction = Transaction {
            id: transaction_id.clone(),
            timestamp,
            original_config,
            new_config,
            state: TransactionState::Created,
            changes,
            metadata: HashMap::new(),
        };

        // Store transaction
        {
            let mut active = self.active_transactions.lock().await;
            active.insert(transaction_id.clone(), transaction.clone());
        }

        // Log transaction creation
        self.log_transaction(&transaction, "Transaction created")
            .await?;

        info!("Created transaction {}", transaction_id);
        Ok(transaction)
    }

    /// Apply configuration changes transactionally
    pub async fn apply_configuration(&self, config: &NetworkConfiguration) -> Result<ApplyResult> {
        let start_time = SystemTime::now();
        let mut transaction = self.begin_transaction(config.clone()).await?;

        let result = self.apply_transaction_internal(&mut transaction).await;

        let duration_ms = start_time.elapsed().unwrap_or_default().as_millis() as u64;

        match result {
            Ok(applied_changes) => {
                // Commit transaction
                if let Err(e) = self.commit_transaction(&mut transaction).await {
                    error!("Failed to commit transaction {}: {}", transaction.id, e);
                    // Try to rollback
                    if let Err(rollback_err) = self.rollback_transaction(&mut transaction).await {
                        error!(
                            "Failed to rollback transaction {}: {}",
                            transaction.id, rollback_err
                        );
                    }
                    return Ok(ApplyResult {
                        transaction_id: transaction.id,
                        success: false,
                        applied_changes: vec![],
                        warnings: vec![],
                        error: Some(format!("Commit failed: {}", e)),
                        duration_ms,
                    });
                }

                Ok(ApplyResult {
                    transaction_id: transaction.id,
                    success: true,
                    applied_changes,
                    warnings: vec![],
                    error: None,
                    duration_ms,
                })
            }
            Err(e) => {
                error!("Transaction {} failed: {}", transaction.id, e);

                // Automatic rollback on failure
                if let Err(rollback_err) = self.rollback_transaction(&mut transaction).await {
                    error!(
                        "Failed to rollback transaction {}: {}",
                        transaction.id, rollback_err
                    );
                }

                Ok(ApplyResult {
                    transaction_id: transaction.id,
                    success: false,
                    applied_changes: vec![],
                    warnings: vec![],
                    error: Some(e.to_string()),
                    duration_ms,
                })
            }
        }
    }

    /// Apply transaction with staged approach
    async fn apply_transaction_internal(
        &self,
        transaction: &mut Transaction,
    ) -> Result<Vec<ConfigChange>> {
        info!("Applying transaction {}", transaction.id);

        // Stage 1: Validation
        transaction.state = TransactionState::Validating;
        self.update_transaction(transaction).await?;

        self.validator.validate(&transaction.new_config).await?;

        transaction.state = TransactionState::Validated;
        self.update_transaction(transaction).await?;

        info!("Transaction {} validation completed", transaction.id);

        // Stage 2: Dry-run with ifupdown2
        self.ifupdown.dry_run(&transaction.new_config).await?;
        info!("Transaction {} dry-run completed", transaction.id);

        // Stage 3: Create rollback point
        self.rollback_manager
            .create_rollback_point(&transaction.id, &transaction.original_config)
            .await?;

        info!("Created rollback point for transaction {}", transaction.id);

        // Stage 4: Apply changes
        transaction.state = TransactionState::Applying;
        self.update_transaction(transaction).await?;

        let applied_changes = self.apply_changes_staged(transaction).await?;

        transaction.state = TransactionState::Applied;
        self.update_transaction(transaction).await?;

        info!("Transaction {} applied successfully", transaction.id);

        if let Some(bus) = &self.event_bus {
            if let Err(err) = bus
                .publish(SystemEvent::NetworkApplied {
                    changes: applied_changes.clone(),
                })
                .await
            {
                warn!("Failed to publish NetworkApplied event: {}", err);
            }
        }

        Ok(applied_changes)
    }

    /// Apply changes in staged manner
    async fn apply_changes_staged(&self, transaction: &Transaction) -> Result<Vec<ConfigChange>> {
        let mut applied_changes = Vec::new();

        // Apply changes in order: deletes, updates, creates
        let mut deletes = Vec::new();
        let mut updates = Vec::new();
        let mut creates = Vec::new();

        for change in &transaction.changes {
            match change.change_type {
                ChangeType::Delete => deletes.push(change),
                ChangeType::Update | ChangeType::Modify => updates.push(change),
                ChangeType::Create => creates.push(change),
            }
        }

        // Stage 1: Apply deletions
        for change in deletes {
            self.apply_single_change(change).await?;
            applied_changes.push(change.clone());
            debug!("Applied delete change for {}", change.target);
        }

        // Stage 2: Apply updates
        for change in updates {
            self.apply_single_change(change).await?;
            applied_changes.push(change.clone());
            debug!("Applied update change for {}", change.target);
        }

        // Stage 3: Apply creates
        for change in creates {
            self.apply_single_change(change).await?;
            applied_changes.push(change.clone());
            debug!("Applied create change for {}", change.target);
        }

        // Stage 4: Write new configuration
        self.config_manager
            .write_config(&transaction.new_config)
            .await?;

        // Stage 5: Reload network configuration
        self.ifupdown.reload_configuration().await?;

        Ok(applied_changes)
    }

    /// Apply a single configuration change
    async fn apply_single_change(&self, change: &ConfigChange) -> Result<()> {
        match change.change_type {
            ChangeType::Create => {
                info!("Creating {}: {}", change.target, change.description);
                // Interface creation is handled by configuration write
            }
            ChangeType::Update | ChangeType::Modify => {
                info!("Updating {}: {}", change.target, change.description);
                // Interface updates are handled by configuration write
            }
            ChangeType::Delete => {
                info!("Deleting {}: {}", change.target, change.description);
                // Bring down interface before deletion
                if let Err(e) = self.ifupdown.bring_down_interface(&change.target).await {
                    warn!("Failed to bring down interface {}: {}", change.target, e);
                }
            }
        }
        Ok(())
    }

    /// Commit a transaction
    async fn commit_transaction(&self, transaction: &mut Transaction) -> Result<()> {
        transaction.state = TransactionState::Committing;
        self.update_transaction(transaction).await?;

        // Synchronize with cluster if needed
        if let Err(e) = self.pmxcfs.sync_configuration().await {
            warn!("Failed to sync configuration with cluster: {}", e);
        }

        // Clean up rollback point
        self.rollback_manager
            .cleanup_rollback_point(&transaction.id)
            .await?;

        transaction.state = TransactionState::Committed;
        self.update_transaction(transaction).await?;

        // Remove from active transactions
        {
            let mut active = self.active_transactions.lock().await;
            active.remove(&transaction.id);
        }

        self.log_transaction(transaction, "Transaction committed")
            .await?;
        info!("Committed transaction {}", transaction.id);
        Ok(())
    }

    /// Rollback a transaction
    async fn rollback_transaction(&self, transaction: &mut Transaction) -> Result<()> {
        transaction.state = TransactionState::RollingBack;
        self.update_transaction(transaction).await?;

        info!("Rolling back transaction {}", transaction.id);

        // Restore original configuration
        self.rollback_manager
            .restore_rollback_point(&transaction.id)
            .await?;

        // Reload network configuration
        self.ifupdown.reload_configuration().await?;

        transaction.state = TransactionState::RolledBack;
        self.update_transaction(transaction).await?;

        // Remove from active transactions
        {
            let mut active = self.active_transactions.lock().await;
            active.remove(&transaction.id);
        }

        self.log_transaction(transaction, "Transaction rolled back")
            .await?;
        info!("Rolled back transaction {}", transaction.id);
        Ok(())
    }

    /// Calculate changes between configurations
    fn calculate_changes(
        &self,
        old_config: &NetworkConfiguration,
        new_config: &NetworkConfiguration,
    ) -> Result<Vec<ConfigChange>> {
        let mut changes = Vec::new();

        // Find deleted interfaces
        for (name, old_iface) in &old_config.interfaces {
            if !new_config.interfaces.contains_key(name) {
                changes.push(ConfigChange {
                    change_type: ChangeType::Delete,
                    target: name.clone(),
                    old_config: Some(serde_json::to_value(old_iface)?),
                    new_config: None,
                    description: format!("Delete interface {}", name),
                });
            }
        }

        // Find created and updated interfaces
        for (name, new_iface) in &new_config.interfaces {
            if let Some(old_iface) = old_config.interfaces.get(name) {
                // Check if interface was modified
                if serde_json::to_value(old_iface)? != serde_json::to_value(new_iface)? {
                    changes.push(ConfigChange {
                        change_type: ChangeType::Update,
                        target: name.clone(),
                        old_config: Some(serde_json::to_value(old_iface)?),
                        new_config: Some(serde_json::to_value(new_iface)?),
                        description: format!("Update interface {}", name),
                    });
                }
            } else {
                // New interface
                changes.push(ConfigChange {
                    change_type: ChangeType::Create,
                    target: name.clone(),
                    old_config: None,
                    new_config: Some(serde_json::to_value(new_iface)?),
                    description: format!("Create interface {}", name),
                });
            }
        }

        Ok(changes)
    }

    /// Update transaction state
    async fn update_transaction(&self, transaction: &Transaction) -> Result<()> {
        {
            let mut active = self.active_transactions.lock().await;
            active.insert(transaction.id.clone(), transaction.clone());
        }

        self.log_transaction(
            transaction,
            &format!("State changed to {:?}", transaction.state),
        )
        .await?;

        Ok(())
    }

    /// Log transaction event
    async fn log_transaction(&self, transaction: &Transaction, message: &str) -> Result<()> {
        let log_entry = serde_json::json!({
            "timestamp": SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            "transaction_id": transaction.id,
            "state": transaction.state,
            "message": message,
            "changes_count": transaction.changes.len(),
        });

        let log_file = self
            .transaction_log_dir
            .join(format!("{}.log", transaction.id));
        let log_line = format!("{}\n", log_entry);

        fs::write(&log_file, log_line).await?;

        Ok(())
    }

    /// Generate unique transaction ID
    fn generate_transaction_id(&self) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        format!("txn_{}", timestamp)
    }

    /// Get active transactions
    pub async fn get_active_transactions(&self) -> Vec<Transaction> {
        let active = self.active_transactions.lock().await;
        active.values().cloned().collect()
    }

    /// Get transaction by ID
    pub async fn get_transaction(&self, transaction_id: &str) -> Option<Transaction> {
        let active = self.active_transactions.lock().await;
        active.get(transaction_id).cloned()
    }
    /// Create a placeholder NetworkApplier for CLI testing
    /// This should not be used in production - use new() instead
    pub fn placeholder() -> Self {
        use std::collections::HashMap;
        use std::path::PathBuf;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        // Create minimal placeholder instances using existing constructors
        Self {
            config_manager: Arc::new(pve_network_config::NetworkConfigManager::new()),
            validator: Arc::new(pve_network_validate::NetworkValidator::new()),
            ifupdown: Arc::new(crate::IfUpDownIntegration::new()),
            rollback_manager: Arc::new(crate::RollbackManager::placeholder()),
            pmxcfs: Arc::new(pve_network_config::PmxcfsConfig::mock()),
            active_transactions: Arc::new(Mutex::new(HashMap::new())),
            transaction_log_dir: PathBuf::from("/tmp/pve-network-transactions"),
            event_bus: None,
        }
    }
}

impl Default for NetworkApplier {
    fn default() -> Self {
        // This is a placeholder - in practice, NetworkApplier should be created with new()
        panic!("NetworkApplier must be created with new() method")
    }
}
