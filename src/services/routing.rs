//! Routing Service - Handles LCR and call routing decisions
//! 
//! This service encapsulates least cost routing logic and integrates
//! with the event-driven architecture for real-time routing decisions.

use crate::events::{EventBus, EventType, RouteInfo, TelecomEvent};
use crate::lcr::LCREngine;
use crate::origination_routing::OriginationRoutes;
use crate::termination_routing::{TerminationRoutes, RoutingDecision};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Configuration for the Routing Service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    /// Enable real-time route optimization
    pub enable_optimization: bool,
    /// Maximum routes to consider per call
    pub max_routes_per_call: usize,
    /// Route selection timeout in milliseconds
    pub route_timeout_ms: u64,
    /// Enable route caching
    pub enable_caching: bool,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    /// Enable fraud checking during routing
    pub enable_fraud_check: bool,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            enable_optimization: true,
            max_routes_per_call: 10,
            route_timeout_ms: 5000,
            enable_caching: true,
            cache_ttl_seconds: 300,
            enable_fraud_check: true,
        }
    }
}

/// Route request information
#[derive(Debug, Clone)]
pub struct RouteRequest {
    pub call_id: String,
    pub session_id: String,
    pub calling_number: String,
    pub called_number: String,
    pub source_ip: IpAddr,
    pub customer_id: Option<i32>,
    pub trunk_id: Option<i32>,
    pub user_agent: Option<String>,
}

/// Route response with selected routes
#[derive(Debug, Clone)]
pub struct RouteResponse {
    pub primary_route: Option<RouteInfo>,
    pub backup_routes: Vec<RouteInfo>,
    pub routing_time_ms: u64,
    pub cache_hit: bool,
    pub fraud_check_passed: bool,
    pub routing_decision: String,
}

/// Route cache entry
#[derive(Debug, Clone)]
struct CachedRoute {
    response: RouteResponse,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// Microservice for handling routing decisions
pub struct RoutingService {
    /// Service configuration
    config: RoutingConfig,
    /// LCR engine for cost optimization
    lcr_engine: Arc<LCREngine>,
    /// Origination routing table
    origination_routes: Arc<RwLock<OriginationRoutes>>,
    /// Termination routing table
    termination_routes: Arc<RwLock<TerminationRoutes>>,
    /// Event bus for publishing routing events
    event_bus: Arc<EventBus>,
    /// Route cache for performance
    route_cache: Arc<RwLock<HashMap<String, CachedRoute>>>,
    /// Request processing channel
    request_sender: mpsc::UnboundedSender<(RouteRequest, tokio::sync::oneshot::Sender<Result<RouteResponse>>)>,
}

impl RoutingService {
    /// Create a new routing service
    pub fn new(
        config: RoutingConfig,
        lcr_engine: Arc<LCREngine>,
        origination_routes: Arc<RwLock<OriginationRoutes>>,
        termination_routes: Arc<RwLock<TerminationRoutes>>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        let route_cache = Arc::new(RwLock::new(HashMap::new()));
        let (request_sender, request_receiver) = mpsc::unbounded_channel();

        let service = Self {
            config: config.clone(),
            lcr_engine,
            origination_routes,
            termination_routes,
            event_bus: event_bus.clone(),
            route_cache: route_cache.clone(),
            request_sender,
        };

        // Start background request processor
        let processor = RoutingProcessor {
            config,
            lcr_engine: service.lcr_engine.clone(),
            origination_routes: service.origination_routes.clone(),
            termination_routes: service.termination_routes.clone(),
            event_bus,
            route_cache,
            request_receiver,
        };

        tokio::spawn(async move {
            processor.run().await;
        });

        service
    }

    /// Request routing for a call
    pub async fn route_call(&self, request: RouteRequest) -> Result<RouteResponse> {
        let (response_sender, response_receiver) = tokio::sync::oneshot::channel();

        self.request_sender
            .send((request.clone(), response_sender))
            .map_err(|_| anyhow::anyhow!("Failed to send routing request"))?;

        let response = response_receiver
            .await
            .map_err(|_| anyhow::anyhow!("Failed to receive routing response"))??;

        debug!("Routing completed for call {} in {}ms", 
               request.call_id, response.routing_time_ms);

        Ok(response)
    }

    /// Get routing statistics
    pub async fn get_stats(&self) -> Result<RoutingStats> {
        let cache = self.route_cache.read().await;
        let cache_size = cache.len();
        let cache_hit_rate = 0.0; // TODO: Track actual hit rate

        Ok(RoutingStats {
            total_routes_requested: 0, // TODO: Track this
            cache_size,
            cache_hit_rate,
            average_routing_time_ms: 0.0, // TODO: Track this
            active_routes: 0, // TODO: Track this
        })
    }

    /// Clear route cache
    pub async fn clear_cache(&self) -> Result<()> {
        let mut cache = self.route_cache.write().await;
        cache.clear();
        info!("Routing cache cleared");
        Ok(())
    }

    /// Shutdown the routing service
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down routing service");
        Ok(())
    }
}

/// Background processor for routing requests
struct RoutingProcessor {
    config: RoutingConfig,
    lcr_engine: Arc<LCREngine>,
    origination_routes: Arc<RwLock<OriginationRoutes>>,
    termination_routes: Arc<RwLock<TerminationRoutes>>,
    event_bus: Arc<EventBus>,
    route_cache: Arc<RwLock<HashMap<String, CachedRoute>>>,
    request_receiver: mpsc::UnboundedReceiver<(RouteRequest, tokio::sync::oneshot::Sender<Result<RouteResponse>>)>,
}

impl RoutingProcessor {
    async fn run(mut self) {
        while let Some((request, response_sender)) = self.request_receiver.recv().await {
            let response = self.process_routing_request(request).await;
            let _ = response_sender.send(response);
        }
    }

    async fn process_routing_request(&self, request: RouteRequest) -> Result<RouteResponse> {
        let start_time = std::time::Instant::now();

        // Check cache first if enabled
        if self.config.enable_caching {
            if let Some(cached_response) = self.check_cache(&request).await? {
                return Ok(cached_response);
            }
        }

        // Perform fraud check if enabled
        let fraud_check_passed = if self.config.enable_fraud_check {
            self.perform_fraud_check(&request).await?
        } else {
            true
        };

        if !fraud_check_passed {
            return Ok(RouteResponse {
                primary_route: None,
                backup_routes: Vec::new(),
                routing_time_ms: start_time.elapsed().as_millis() as u64,
                cache_hit: false,
                fraud_check_passed: false,
                routing_decision: "FRAUD_DETECTED".to_string(),
            });
        }

        // Get routes from origination/termination tables
        let routes = self.get_available_routes(&request).await?;

        // Apply LCR optimization if enabled
        let optimized_routes = if self.config.enable_optimization {
            self.optimize_routes(routes, &request).await?
        } else {
            routes
        };

        // Select primary and backup routes
        let (primary_route, backup_routes) = self.select_routes(optimized_routes);

        let routing_time_ms = start_time.elapsed().as_millis() as u64;

        let response = RouteResponse {
            primary_route: primary_route.clone(),
            backup_routes: backup_routes.clone(),
            routing_time_ms,
            cache_hit: false,
            fraud_check_passed,
            routing_decision: if primary_route.is_some() { 
                "ROUTE_FOUND".to_string() 
            } else { 
                "NO_ROUTE_AVAILABLE".to_string() 
            },
        };

        // Cache the response if enabled
        if self.config.enable_caching {
            self.cache_response(&request, &response).await?;
        }

        // Publish routing event
        self.publish_routing_event(&request, &response).await?;

        Ok(response)
    }

    async fn check_cache(&self, request: &RouteRequest) -> Result<Option<RouteResponse>> {
        let cache_key = self.generate_cache_key(request);
        let cache = self.route_cache.read().await;
        
        if let Some(cached) = cache.get(&cache_key) {
            let age = (Utc::now() - cached.created_at).num_seconds() as u64;
            if age < self.config.cache_ttl_seconds {
                let mut response = cached.response.clone();
                response.cache_hit = true;
                return Ok(Some(response));
            }
        }

        Ok(None)
    }

    async fn perform_fraud_check(&self, request: &RouteRequest) -> Result<bool> {
        // TODO: Implement actual fraud checking logic
        // For now, just check basic patterns
        
        // Check for suspicious calling patterns
        if request.calling_number.len() < 3 || request.called_number.len() < 3 {
            return Ok(false);
        }

        // Check for international premium rate numbers
        if request.called_number.starts_with("900") || 
           request.called_number.starts_with("976") {
            warn!("Potential premium rate fraud detected for call {}", request.call_id);
            return Ok(false);
        }

        Ok(true)
    }

    async fn get_available_routes(&self, request: &RouteRequest) -> Result<Vec<RouteInfo>> {
        let mut routes = Vec::new();

        // Get routes from termination routing table
        let termination_routes = self.termination_routes.read().await;
        if let Ok(termination_result) = termination_routes.route_call(
            &request.calling_number,
            &request.called_number,
            request.customer_id,
        ) {
            // Convert termination routes to RouteInfo
            // This is a simplified conversion - in practice you'd have more detailed mapping
            for (i, route) in termination_result.selected_routes.iter().enumerate() {
                routes.push(RouteInfo {
                    route_id: format!("term_{}_{}", request.call_id, i),
                    trunk_id: route.trunk_id.unwrap_or(0),
                    trunk_name: route.trunk_name.clone(),
                    gateway_ip: "127.0.0.1".parse().map_err(|_| anyhow::anyhow!("Invalid default gateway IP"))?,
                    gateway_port: 5060,
                    priority: route.priority,
                    cost: route.rate,
                });
            }
        }

        // Limit routes if configured
        if routes.len() > self.config.max_routes_per_call {
            routes.truncate(self.config.max_routes_per_call);
        }

        Ok(routes)
    }

    async fn optimize_routes(&self, mut routes: Vec<RouteInfo>, _request: &RouteRequest) -> Result<Vec<RouteInfo>> {
        // Sort by cost (LCR principle)
        routes.sort_by(|a, b| a.cost.partial_cmp(&b.cost).unwrap_or(std::cmp::Ordering::Equal));
        
        // TODO: Apply more sophisticated optimization algorithms
        // - Quality scoring
        // - ASR (Answer Seizure Ratio) considerations
        // - Route performance metrics

        Ok(routes)
    }

    fn select_routes(&self, mut routes: Vec<RouteInfo>) -> (Option<RouteInfo>, Vec<RouteInfo>) {
        if routes.is_empty() {
            return (None, Vec::new());
        }

        let primary = routes.remove(0);
        (Some(primary), routes)
    }

    async fn cache_response(&self, request: &RouteRequest, response: &RouteResponse) -> Result<()> {
        let cache_key = self.generate_cache_key(request);
        let cached_route = CachedRoute {
            response: response.clone(),
            created_at: Utc::now(),
        };

        let mut cache = self.route_cache.write().await;
        cache.insert(cache_key, cached_route);

        Ok(())
    }

    async fn publish_routing_event(&self, request: &RouteRequest, response: &RouteResponse) -> Result<()> {
        let event = TelecomEvent::CallRouted(crate::events::CallRoutedEvent {
            call_id: request.call_id.clone(),
            session_id: request.session_id.clone(),
            selected_route: response.primary_route.clone(),
            attempted_routes: {
                let mut routes = response.backup_routes.clone();
                if let Some(primary) = &response.primary_route {
                    routes.insert(0, primary.clone());
                }
                routes
            },
            routing_time_ms: response.routing_time_ms,
            routing_decision: response.routing_decision.clone(),
            timestamp: Utc::now(),
        });

        self.event_bus.publish(event).await
            .context("Failed to publish routing event")?;

        Ok(())
    }

    fn generate_cache_key(&self, request: &RouteRequest) -> String {
        format!("{}:{}:{}", 
                request.calling_number, 
                request.called_number, 
                request.customer_id.unwrap_or(0))
    }
}

/// Routing service statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingStats {
    pub total_routes_requested: u64,
    pub cache_size: usize,
    pub cache_hit_rate: f64,
    pub average_routing_time_ms: f64,
    pub active_routes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_routing_service_creation() {
        let config = RoutingConfig::default();
        let lcr_engine = Arc::new(LCREngine::new());
        let origination_routes = Arc::new(RwLock::new(OriginationRoutes::new()));
        let termination_routes = Arc::new(RwLock::new(TerminationRoutes::new()));
        let event_bus = Arc::new(EventBus::new());

        let _service = RoutingService::new(
            config,
            lcr_engine,
            origination_routes,
            termination_routes,
            event_bus,
        );
    }

    #[tokio::test]
    async fn test_routing_request() {
        let config = RoutingConfig::default();
        let lcr_engine = Arc::new(LCREngine::new());
        let origination_routes = Arc::new(RwLock::new(OriginationRoutes::new()));
        let termination_routes = Arc::new(RwLock::new(TerminationRoutes::new()));
        let event_bus = Arc::new(EventBus::new());

        let service = RoutingService::new(
            config,
            lcr_engine,
            origination_routes,
            termination_routes,
            event_bus,
        );

        let request = RouteRequest {
            call_id: "test-call-123".to_string(),
            session_id: "test-session-456".to_string(),
            calling_number: "1234567890".to_string(),
            called_number: "0987654321".to_string(),
            source_ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            customer_id: Some(42),
            trunk_id: None,
            user_agent: Some("TestUA/1.0".to_string()),
        };

        let response = service.route_call(request).await;
        assert!(response.is_ok());
    }
}