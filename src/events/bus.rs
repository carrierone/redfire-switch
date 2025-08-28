//! Event bus implementation for distributed event handling

use super::{EventFilter, EventHandler, EventStats, EventType, HandlerRegistration, TelecomEvent};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, warn};

/// Configuration for the event bus
#[derive(Debug, Clone)]
pub struct EventBusConfig {
    /// Maximum number of events to buffer per subscriber
    pub buffer_size: usize,
    /// Timeout for event handler processing
    pub handler_timeout: Duration,
    /// Enable event persistence to disk
    pub enable_persistence: bool,
    /// Maximum events to persist
    pub max_persisted_events: usize,
    /// Health check interval for handlers
    pub health_check_interval: Duration,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            buffer_size: 1000,
            handler_timeout: Duration::from_secs(30),
            enable_persistence: false,
            max_persisted_events: 10000,
            health_check_interval: Duration::from_secs(60),
        }
    }
}

/// High-performance event bus with async handlers and filtering
#[derive(Debug)]
pub struct EventBus {
    /// Broadcast channel for real-time event distribution
    sender: broadcast::Sender<Arc<TelecomEvent>>,
    
    /// Registered event handlers
    handlers: RwLock<HashMap<String, HandlerRegistration>>,
    
    /// Event processing statistics
    stats: RwLock<EventStats>,
    
    /// Configuration
    config: EventBusConfig,
    
    /// Event persistence storage (if enabled)
    persisted_events: RwLock<Vec<Arc<TelecomEvent>>>,
}

impl EventBus {
    /// Create a new event bus with default configuration
    pub fn new() -> Self {
        Self::with_config(EventBusConfig::default())
    }

    /// Create a new event bus with custom configuration
    pub fn with_config(config: EventBusConfig) -> Self {
        let (sender, _receiver) = broadcast::channel(config.buffer_size);
        
        Self {
            sender,
            handlers: RwLock::new(HashMap::new()),
            stats: RwLock::new(EventStats::default()),
            config,
            persisted_events: RwLock::new(Vec::new()),
        }
    }

    /// Register an event handler
    pub async fn register_handler(&self, handler: Arc<dyn EventHandler>) -> Result<()> {
        let handler_name = handler.name().to_string();
        let event_types = handler.interested_events();
        
        debug!("Registering event handler: {} for events: {:?}", handler_name, event_types);

        // Perform initial health check
        if let Err(e) = handler.health_check().await {
            warn!("Handler {} failed initial health check: {}", handler_name, e);
        }

        let registration = HandlerRegistration {
            handler,
            event_types,
            created_at: Utc::now(),
            last_health_check: Some(Utc::now()),
            error_count: 0,
        };

        let mut handlers = self.handlers.write().await;
        handlers.insert(handler_name.clone(), registration);

        info!("Successfully registered event handler: {}", handler_name);
        Ok(())
    }

    /// Unregister an event handler
    pub async fn unregister_handler(&self, handler_name: &str) -> Result<()> {
        let mut handlers = self.handlers.write().await;
        
        if handlers.remove(handler_name).is_some() {
            info!("Unregistered event handler: {}", handler_name);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Handler {} not found", handler_name))
        }
    }

    /// Publish an event to all registered handlers
    pub async fn publish(&self, event: TelecomEvent) -> Result<()> {
        let event_arc = Arc::new(event);
        let event_type = EventType::from(event_arc.as_ref());
        
        debug!("Publishing event: {:?} with ID: {}", event_type, event_arc.event_id());

        // Update statistics
        {
            let mut stats = self.stats.write().await;
            stats.total_events_published += 1;
            let type_name = format!("{:?}", event_type);
            *stats.events_by_type.entry(type_name).or_insert(0) += 1;
        }

        // Persist event if enabled
        if self.config.enable_persistence {
            self.persist_event(event_arc.clone()).await?;
        }

        // Send to broadcast channel (for real-time subscribers)
        if let Err(e) = self.sender.send(event_arc.clone()) {
            warn!("Failed to broadcast event: {}", e);
        }

        // Process with registered handlers
        self.process_with_handlers(&event_arc).await?;

        Ok(())
    }

    /// Process event with all registered handlers
    async fn process_with_handlers(&self, event: &Arc<TelecomEvent>) -> Result<()> {
        let handlers = self.handlers.read().await;
        let event_type = EventType::from(event.as_ref());

        let start_time = Instant::now();
        let mut handled_count = 0;
        let mut error_count = 0;

        for (handler_name, registration) in handlers.iter() {
            // Check if handler is interested in this event type
            if !registration.event_types.contains(&EventType::All) 
                && !registration.event_types.contains(&event_type) {
                continue;
            }

            handled_count += 1;

            // Process event with timeout
            let handler = registration.handler.clone();
            let event_clone = event.clone();
            let handler_name_clone = handler_name.clone();

            tokio::spawn(async move {
                let result = timeout(
                    Duration::from_secs(30), // TODO: Use config timeout
                    handler.handle_event(event_clone.as_ref())
                ).await;

                match result {
                    Ok(Ok(_)) => {
                        debug!("Handler {} processed event successfully", handler_name_clone);
                    }
                    Ok(Err(e)) => {
                        error!("Handler {} failed to process event: {}", handler_name_clone, e);
                    }
                    Err(_) => {
                        error!("Handler {} timed out processing event", handler_name_clone);
                    }
                }
            });
        }

        // Update processing statistics
        let processing_time = start_time.elapsed().as_millis() as f64;
        {
            let mut stats = self.stats.write().await;
            stats.total_events_handled += handled_count;
            stats.handler_error_count += error_count;
            
            // Update rolling average processing time
            if stats.total_events_handled > 0 {
                stats.average_processing_time_ms = 
                    (stats.average_processing_time_ms * (stats.total_events_handled - handled_count) as f64 
                     + processing_time) / stats.total_events_handled as f64;
            } else {
                stats.average_processing_time_ms = processing_time;
            }
        }

        debug!("Processed event with {} handlers in {:.2}ms", handled_count, processing_time);
        Ok(())
    }

    /// Subscribe to events with optional filtering
    pub fn subscribe(&self, filter: Option<EventFilter>) -> broadcast::Receiver<Arc<TelecomEvent>> {
        let mut receiver = self.sender.subscribe();
        
        // If no filter, return receiver as-is
        if filter.is_none() {
            return receiver;
        }

        // For filtered subscriptions, we'd need a wrapper that filters events
        // This is a simplified implementation - in production you'd want
        // a more sophisticated filtering mechanism
        receiver
    }

    /// Get current event processing statistics
    pub async fn get_stats(&self) -> EventStats {
        self.stats.read().await.clone()
    }

    /// Get list of registered handlers
    pub async fn get_handlers(&self) -> Vec<String> {
        let handlers = self.handlers.read().await;
        handlers.keys().cloned().collect()
    }

    /// Perform health check on all registered handlers
    pub async fn health_check_handlers(&self) -> Result<HashMap<String, Result<()>>> {
        let handlers = self.handlers.read().await;
        let mut results = HashMap::new();

        for (name, registration) in handlers.iter() {
            let result = registration.handler.health_check().await;
            results.insert(name.clone(), result);
        }

        Ok(results)
    }

    /// Persist event to storage (if enabled)
    async fn persist_event(&self, event: Arc<TelecomEvent>) -> Result<()> {
        if !self.config.enable_persistence {
            return Ok(());
        }

        let mut persisted = self.persisted_events.write().await;
        
        // Add new event
        persisted.push(event);
        
        // Trim old events if over limit
        if persisted.len() > self.config.max_persisted_events {
            let excess = persisted.len() - self.config.max_persisted_events;
            persisted.drain(0..excess);
        }

        Ok(())
    }

    /// Get persisted events (if persistence is enabled)
    pub async fn get_persisted_events(&self, filter: Option<EventFilter>) -> Result<Vec<Arc<TelecomEvent>>> {
        if !self.config.enable_persistence {
            return Err(anyhow::anyhow!("Event persistence is not enabled"));
        }

        let persisted = self.persisted_events.read().await;
        
        let filtered_events = if let Some(filter) = filter {
            persisted
                .iter()
                .filter(|event| filter.matches(event))
                .cloned()
                .collect()
        } else {
            persisted.clone()
        };

        Ok(filtered_events)
    }

    /// Clear persisted events
    pub async fn clear_persisted_events(&self) -> Result<()> {
        if !self.config.enable_persistence {
            return Err(anyhow::anyhow!("Event persistence is not enabled"));
        }

        let mut persisted = self.persisted_events.write().await;
        persisted.clear();
        
        info!("Cleared all persisted events");
        Ok(())
    }

    /// Start background health check task
    pub fn start_health_check_task(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let bus = self.clone();
        let interval = self.config.health_check_interval;
        
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(interval);
            
            loop {
                timer.tick().await;
                
                match bus.health_check_handlers().await {
                    Ok(results) => {
                        let mut unhealthy_count = 0;
                        
                        for (handler_name, result) in results {
                            if let Err(e) = result {
                                warn!("Handler {} health check failed: {}", handler_name, e);
                                unhealthy_count += 1;
                            }
                        }
                        
                        if unhealthy_count > 0 {
                            warn!("{} handlers failed health check", unhealthy_count);
                        } else {
                            debug!("All event handlers passed health check");
                        }
                    }
                    Err(e) => {
                        error!("Failed to perform handler health checks: {}", e);
                    }
                }
            }
        })
    }

    /// Get configuration
    pub fn config(&self) -> &EventBusConfig {
        &self.config
    }

    /// Shutdown the event bus gracefully
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down event bus");
        
        // Clear handlers to prevent new event processing
        let mut handlers = self.handlers.write().await;
        handlers.clear();
        
        // Clear persisted events if enabled
        if self.config.enable_persistence {
            let mut persisted = self.persisted_events.write().await;
            persisted.clear();
        }
        
        info!("Event bus shutdown complete");
        Ok(())
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{EventHandler, EventType, TelecomEvent};
    use async_trait::async_trait;
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::atomic::{AtomicU32, Ordering};
    use tokio::time::sleep;

    struct TestHandler {
        name: String,
        event_count: AtomicU32,
        interested_types: Vec<EventType>,
    }

    impl TestHandler {
        fn new(name: String, interested_types: Vec<EventType>) -> Self {
            Self {
                name,
                event_count: AtomicU32::new(0),
                interested_types,
            }
        }

        fn get_event_count(&self) -> u32 {
            self.event_count.load(Ordering::Relaxed)
        }
    }

    #[async_trait]
    impl EventHandler for TestHandler {
        async fn handle_event(&self, _event: &TelecomEvent) -> Result<()> {
            self.event_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn interested_events(&self) -> Vec<EventType> {
            self.interested_types.clone()
        }
    }

    #[tokio::test]
    async fn test_event_bus_handler_registration() {
        let bus = EventBus::new();
        let handler = Arc::new(TestHandler::new(
            "test-handler".to_string(),
            vec![EventType::CallInitiated],
        ));

        bus.register_handler(handler).await.expect("Failed to register handler");

        let handlers = bus.get_handlers().await;
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0], "test-handler");
    }

    #[tokio::test]
    async fn test_event_publishing_and_handling() {
        let bus = EventBus::new();
        let handler = Arc::new(TestHandler::new(
            "test-handler".to_string(),
            vec![EventType::CallInitiated],
        ));

        bus.register_handler(handler.clone()).await.expect("Failed to register handler");

        let event = TelecomEvent::call_initiated(
            "test-call".to_string(),
            "test-session".to_string(),
            "1234567890".to_string(),
            "0987654321".to_string(),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        );

        bus.publish(event).await.expect("Failed to publish event");

        // Give handler time to process
        sleep(Duration::from_millis(100)).await;

        assert_eq!(handler.get_event_count(), 1);

        let stats = bus.get_stats().await;
        assert_eq!(stats.total_events_published, 1);
    }

    #[tokio::test]
    async fn test_event_filtering() {
        let bus = EventBus::new();
        let call_handler = Arc::new(TestHandler::new(
            "call-handler".to_string(),
            vec![EventType::CallInitiated],
        ));
        let health_handler = Arc::new(TestHandler::new(
            "health-handler".to_string(),
            vec![EventType::HealthStatus],
        ));

        bus.register_handler(call_handler.clone()).await.expect("Failed to register call handler");
        bus.register_handler(health_handler.clone()).await.expect("Failed to register health handler");

        // Publish call event
        let call_event = TelecomEvent::call_initiated(
            "test-call".to_string(),
            "test-session".to_string(),
            "1234567890".to_string(),
            "0987654321".to_string(),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        );
        bus.publish(call_event).await.expect("Failed to publish call event");

        // Publish health event
        let health_event = TelecomEvent::health_status(
            "test-service".to_string(),
            "instance-1".to_string(),
            crate::events::HealthStatus::Healthy,
            HashMap::new(),
        );
        bus.publish(health_event).await.expect("Failed to publish health event");

        // Give handlers time to process
        sleep(Duration::from_millis(100)).await;

        // Each handler should have processed only one event (their interested type)
        assert_eq!(call_handler.get_event_count(), 1);
        assert_eq!(health_handler.get_event_count(), 1);
    }

    #[tokio::test]
    async fn test_event_persistence() {
        let config = EventBusConfig {
            enable_persistence: true,
            max_persisted_events: 100,
            ..Default::default()
        };
        
        let bus = EventBus::with_config(config);

        let event = TelecomEvent::call_initiated(
            "test-call".to_string(),
            "test-session".to_string(),
            "1234567890".to_string(),
            "0987654321".to_string(),
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        );

        bus.publish(event).await.expect("Failed to publish event");

        let persisted = bus.get_persisted_events(None).await.expect("Failed to get persisted events");
        assert_eq!(persisted.len(), 1);
    }
}