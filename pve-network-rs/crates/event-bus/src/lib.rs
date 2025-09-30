//! Simple asynchronous event bus used to broadcast high-level system events
//! between network, storage, migration and container services.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use log::warn;
use pve_shared_types::SystemEvent;
use thiserror::Error;
use tokio::sync::RwLock;

/// Result alias for event bus operations
pub type EventBusResult<T> = Result<T, EventBusError>;

/// Contract implemented by listeners interested in [`SystemEvent`] notifications.
#[async_trait]
pub trait EventListener: Send + Sync {
    async fn on_event(&self, event: &SystemEvent) -> anyhow::Result<()>;
}

/// Shared event bus that multiplexes published [`SystemEvent`] values to
/// registered listeners.
#[derive(Clone, Default)]
pub struct EventBus {
    listeners: Arc<RwLock<HashMap<String, Arc<dyn EventListener>>>>,
}

impl EventBus {
    /// Create a new bus instance without any registered listeners.
    pub fn new() -> Self {
        Self {
            listeners: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a listener under the given name. Listener names must be unique
    /// so that they can be replaced or removed later on.
    pub async fn register_listener<L>(
        &self,
        name: impl Into<String>,
        listener: L,
    ) -> EventBusResult<()>
    where
        L: EventListener + 'static,
    {
        let name = name.into();
        let mut guard = self.listeners.write().await;
        if guard.contains_key(&name) {
            return Err(EventBusError::ListenerExists(name));
        }

        guard.insert(name, Arc::new(listener));
        Ok(())
    }

    /// Remove a listener by name.
    pub async fn unregister_listener(&self, name: &str) -> EventBusResult<()> {
        let mut guard = self.listeners.write().await;
        guard
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| EventBusError::ListenerNotFound(name.to_string()))
    }

    /// Publish a new [`SystemEvent`] to all registered listeners. Listener
    /// failures are collected and surfaced as a single [`EventBusError`] so that
    /// publishers receive feedback while the other listeners still get a chance
    /// to react.
    pub async fn publish(&self, event: SystemEvent) -> EventBusResult<()> {
        let listeners: Vec<(String, Arc<dyn EventListener>)> = {
            let guard = self.listeners.read().await;
            guard
                .iter()
                .map(|(name, listener)| (name.clone(), Arc::clone(listener)))
                .collect()
        };

        let mut failures = Vec::new();
        for (name, listener) in listeners {
            if let Err(err) = listener.on_event(&event).await {
                warn!("event listener '{}' failed: {}", name, err);
                failures.push(ListenerFailure {
                    listener: name,
                    error: err.to_string(),
                });
            }
        }

        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.into())
        }
    }
}

/// Error type returned by event bus operations.
#[derive(Debug, Error)]
pub enum EventBusError {
    #[error("listener '{0}' already registered")]
    ListenerExists(String),
    #[error("listener '{0}' not found")]
    ListenerNotFound(String),
    #[error("one or more listeners failed: {0}")]
    ListenerFailures(ListenerFailureReport),
}

/// Aggregate information about listener failures.
#[derive(Debug, Clone)]
pub struct ListenerFailure {
    pub listener: String,
    pub error: String,
}

impl std::fmt::Display for ListenerFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.listener, self.error)
    }
}

/// Wrapper used to present multiple failures as a single error payload.
#[derive(Debug, Clone)]
pub struct ListenerFailureReport(pub Vec<ListenerFailure>);

impl std::fmt::Display for ListenerFailureReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for failure in &self.0 {
            if !first {
                write!(f, "; ")?;
            }
            first = false;
            write!(f, "{}", failure)?;
        }
        Ok(())
    }
}

impl std::convert::From<Vec<ListenerFailure>> for ListenerFailureReport {
    fn from(value: Vec<ListenerFailure>) -> Self {
        ListenerFailureReport(value)
    }
}

impl std::convert::From<Vec<ListenerFailure>> for EventBusError {
    fn from(value: Vec<ListenerFailure>) -> Self {
        EventBusError::ListenerFailures(value.into())
    }
}

impl EventBusError {
    pub fn listener_failures(&self) -> Option<&[ListenerFailure]> {
        match self {
            EventBusError::ListenerFailures(report) => Some(&report.0),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingListener {
        counter: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl EventListener for CountingListener {
        async fn on_event(&self, _event: &SystemEvent) -> anyhow::Result<()> {
            self.counter.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_listener_registration_and_publish() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        bus.register_listener(
            "counter",
            CountingListener {
                counter: Arc::clone(&counter),
            },
        )
        .await
        .unwrap();

        bus.publish(SystemEvent::StorageVlanCreated {
            id: "storage1".into(),
        })
        .await
        .unwrap();

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
