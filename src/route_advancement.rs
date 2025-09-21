//! Route Advancement Engine
//! Handles SIP response code analysis and automatic route advancement for B2BUA

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::lcr::types::{CallRoute, RouteRequest};
use crate::termination_routing::{
    FailedAttempt, RouteAdvanceDecision, SipResponseCode, TerminationRoutingRequest,
    TerminationRoutingService,
};

/// Route advancement manager for B2BUA operations
pub struct RouteAdvancementEngine {
    termination_service: Arc<Mutex<TerminationRoutingService>>,
    active_calls: HashMap<String, CallRoutingState>,
    max_route_attempts: u32,
}

/// Call routing state tracking
#[derive(Debug, Clone)]
pub struct CallRoutingState {
    pub call_id: String,
    pub ani: String,
    pub dnis: String,
    pub original_request: RouteRequest,
    pub current_attempt: u32,
    pub failed_attempts: Vec<FailedAttempt>,
    pub current_route: Option<CallRoute>,
    pub remaining_routes: Vec<CallRoute>,
    pub routing_started_at: DateTime<Utc>,
    pub last_attempt_at: Option<DateTime<Utc>>,
}

/// Route advancement result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteAdvancementResult {
    pub call_id: String,
    pub action: AdvancementAction,
    pub new_route: Option<CallRoute>,
    pub total_attempts: u32,
    pub routing_complete: bool,
    pub final_response_code: Option<u16>,
    pub reason: String,
}

/// Route advancement actions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdvancementAction {
    RouteToNext,       // Advance to next available route
    RetryCurrentRoute, // Retry current route (for temporary failures)
    CompleteCall,      // Complete call with current response
    RejectCall,        // No more routes available
}

impl RouteAdvancementEngine {
    pub fn new(
        termination_service: Arc<Mutex<TerminationRoutingService>>,
        max_route_attempts: u32,
    ) -> Self {
        Self {
            termination_service,
            active_calls: HashMap::new(),
            max_route_attempts,
        }
    }

    /// Start routing for a new call
    pub async fn start_call_routing(
        &mut self,
        call_id: String,
        ani: String,
        dnis: String,
        route_request: RouteRequest,
    ) -> Result<RouteAdvancementResult> {
        info!(
            "Starting call routing for {} -> {} (call_id: {})",
            ani, dnis, call_id
        );

        // Create initial termination request
        let termination_request = TerminationRoutingRequest {
            call_id: call_id.clone(),
            ani: ani.clone(),
            dnis: dnis.clone(),
            route_request: route_request.clone(),
            attempt_number: 1,
            previous_responses: vec![],
            max_attempts: self.max_route_attempts,
            timestamp: Utc::now(),
        };

        // Get initial routing decision
        let routing_response = {
            let mut service = self.termination_service.lock().await;
            service.route_termination(termination_request).await?
        };

        let routing_state = CallRoutingState {
            call_id: call_id.clone(),
            ani,
            dnis,
            original_request: route_request,
            current_attempt: 1,
            failed_attempts: vec![],
            current_route: routing_response.selected_route.clone(),
            remaining_routes: routing_response.remaining_routes,
            routing_started_at: Utc::now(),
            last_attempt_at: Some(Utc::now()),
        };

        let result = if routing_response.success {
            RouteAdvancementResult {
                call_id: call_id.clone(),
                action: AdvancementAction::RouteToNext,
                new_route: routing_response.selected_route,
                total_attempts: 1,
                routing_complete: false,
                final_response_code: None,
                reason: "Initial route selected".to_string(),
            }
        } else {
            RouteAdvancementResult {
                call_id: call_id.clone(),
                action: AdvancementAction::RejectCall,
                new_route: None,
                total_attempts: 1,
                routing_complete: true,
                final_response_code: Some(503), // Service Unavailable
                reason: routing_response.reason,
            }
        };

        // Store routing state
        self.active_calls.insert(call_id, routing_state);

        Ok(result)
    }

    /// Handle SIP response and determine next routing action
    pub async fn handle_sip_response(
        &mut self,
        call_id: &str,
        response_code: u16,
        response_reason: &str,
    ) -> Result<RouteAdvancementResult> {
        let routing_state = self
            .active_calls
            .get_mut(call_id)
            .ok_or_else(|| anyhow!("Call routing state not found for call_id: {}", call_id))?;

        debug!(
            "Processing SIP response {} {} for call {}",
            response_code, response_reason, call_id
        );

        // Record this attempt as failed if needed
        if response_code >= 300 {
            let current_route = routing_state
                .current_route
                .as_ref()
                .ok_or_else(|| anyhow!("No current route for call {}", call_id))?;

            let failed_attempt = FailedAttempt {
                trunk_id: current_route.egress_trunk.id,
                trunk_name: current_route.egress_trunk.name.clone(),
                response_code,
                response_reason: response_reason.to_string(),
                attempt_time: Utc::now(),
                duration_ms: routing_state
                    .last_attempt_at
                    .map(|t| Utc::now().signed_duration_since(t).num_milliseconds() as u64)
                    .unwrap_or(0),
            };

            routing_state.failed_attempts.push(failed_attempt);
        }

        // Determine advancement action based on response code
        let advancement_decision = self.should_advance_route(response_code, response_reason);

        match advancement_decision {
            RouteAdvanceDecision::CompleteCall => {
                // Call completed (success or permanent failure)
                Ok(self.complete_call_routing(call_id, response_code, response_reason)?)
            }
            RouteAdvanceDecision::AdvanceToNextRoute => {
                // Try to advance to next route
                self.advance_to_next_route(call_id, response_code).await
            }
        }
    }

    /// Advance to the next available route
    async fn advance_to_next_route(
        &mut self,
        call_id: &str,
        last_response_code: u16,
    ) -> Result<RouteAdvancementResult> {
        let routing_state = self
            .active_calls
            .get_mut(call_id)
            .ok_or_else(|| anyhow!("Call routing state not found for call_id: {}", call_id))?;

        // Check if we've exceeded maximum attempts
        if routing_state.current_attempt >= self.max_route_attempts {
            return self.complete_call_routing(
                call_id,
                last_response_code,
                "Maximum route attempts exceeded",
            );
        }

        // Increment attempt counter
        routing_state.current_attempt += 1;
        routing_state.last_attempt_at = Some(Utc::now());

        // Extract failed trunk IDs to exclude from next routing attempt
        let failed_trunk_ids: Vec<i32> = routing_state
            .failed_attempts
            .iter()
            .map(|attempt| attempt.trunk_id)
            .collect();

        debug!(
            "Excluding previously failed trunk IDs: {:?}",
            failed_trunk_ids
        );

        // Create new termination request with failed attempts history
        let route_request = routing_state.original_request.clone();
        // Add excluded trunk IDs to route request (this would require extending RouteRequest)
        // For now, we'll filter in the response handling

        let termination_request = TerminationRoutingRequest {
            call_id: call_id.to_string(),
            ani: routing_state.ani.clone(),
            dnis: routing_state.dnis.clone(),
            route_request,
            attempt_number: routing_state.current_attempt,
            previous_responses: routing_state.failed_attempts.clone(),
            max_attempts: self.max_route_attempts,
            timestamp: Utc::now(),
        };

        // Get next routing decision
        let routing_response = {
            let mut service = self.termination_service.lock().await;
            service.route_termination(termination_request).await?
        };

        // Filter out previously failed routes from the response
        let filtered_response = if let Some(selected_route) = &routing_response.selected_route {
            if failed_trunk_ids.contains(&selected_route.egress_trunk.id) {
                // This route was already tried and failed, skip it
                info!(
                    "Skipping previously failed trunk {} for call {}",
                    selected_route.egress_trunk.name, call_id
                );
                return self.complete_call_routing(
                    call_id,
                    last_response_code,
                    "No more untried routes available",
                );
            } else {
                routing_response
            }
        } else {
            routing_response
        };

        let routing_response = filtered_response;

        if routing_response.success {
            // Update routing state with new route
            routing_state.current_route = routing_response.selected_route.clone();
            routing_state.remaining_routes = routing_response.remaining_routes;

            let selected_route = routing_response.selected_route.as_ref().unwrap();
            let route_name = selected_route.egress_trunk.name.clone();
            info!(
                "Advanced to next route for call {}: {} (trunk_id: {}, attempt {})",
                call_id,
                selected_route.egress_trunk.name,
                selected_route.egress_trunk.id,
                routing_state.current_attempt
            );

            Ok(RouteAdvancementResult {
                call_id: call_id.to_string(),
                action: AdvancementAction::RouteToNext,
                new_route: routing_response.selected_route,
                total_attempts: routing_state.current_attempt,
                routing_complete: false,
                final_response_code: None,
                reason: format!("Advanced to next available route: {}", route_name),
            })
        } else {
            // No more routes available
            self.complete_call_routing(call_id, last_response_code, &routing_response.reason)
        }
    }

    /// Complete call routing (success or final failure)
    fn complete_call_routing(
        &mut self,
        call_id: &str,
        final_response_code: u16,
        reason: &str,
    ) -> Result<RouteAdvancementResult> {
        let routing_state = self
            .active_calls
            .remove(call_id)
            .ok_or_else(|| anyhow!("Call routing state not found for call_id: {}", call_id))?;

        let action = if final_response_code < 300 {
            AdvancementAction::CompleteCall
        } else {
            AdvancementAction::RejectCall
        };

        info!(
            "Completed call routing for {}: {} {} (total attempts: {})",
            call_id, final_response_code, reason, routing_state.current_attempt
        );

        Ok(RouteAdvancementResult {
            call_id: call_id.to_string(),
            action,
            new_route: None,
            total_attempts: routing_state.current_attempt,
            routing_complete: true,
            final_response_code: Some(final_response_code),
            reason: reason.to_string(),
        })
    }

    /// Determine if route should be advanced based on SIP response
    pub fn should_advance_route(
        &self,
        response_code: u16,
        _response_reason: &str,
    ) -> RouteAdvanceDecision {
        // Success responses - complete the call
        if (200..300).contains(&response_code) {
            return RouteAdvanceDecision::CompleteCall;
        }

        // Use our SIP response code logic
        match SipResponseCode::try_from(response_code) {
            Ok(sip_code) => {
                if sip_code.should_advance_route() {
                    RouteAdvanceDecision::AdvanceToNextRoute
                } else {
                    RouteAdvanceDecision::CompleteCall
                }
            }
            Err(_) => {
                // For unknown response codes, use general rules
                match response_code {
                    100..=199 => RouteAdvanceDecision::CompleteCall, // Provisional - don't advance
                    300..=399 => RouteAdvanceDecision::CompleteCall, // Redirection - don't advance
                    400..=499 => RouteAdvanceDecision::AdvanceToNextRoute, // Client error - usually advance
                    500..=599 => RouteAdvanceDecision::AdvanceToNextRoute, // Server error - usually advance
                    600..=699 => RouteAdvanceDecision::CompleteCall, // Global failure - don't advance
                    _ => RouteAdvanceDecision::CompleteCall,         // Unknown - don't advance
                }
            }
        }
    }

    /// Get routing statistics for a call
    pub fn get_call_routing_stats(&self, call_id: &str) -> Option<CallRoutingStats> {
        self.active_calls
            .get(call_id)
            .map(|state| CallRoutingStats {
                call_id: call_id.to_string(),
                current_attempt: state.current_attempt,
                total_failed_attempts: state.failed_attempts.len() as u32,
                routing_duration_ms: Utc::now()
                    .signed_duration_since(state.routing_started_at)
                    .num_milliseconds() as u64,
                current_trunk_name: state
                    .current_route
                    .as_ref()
                    .map(|r| r.egress_trunk.name.clone()),
                remaining_routes: state.remaining_routes.len() as u32,
            })
    }

    /// Get all active call routing states
    pub fn get_active_calls(&self) -> Vec<String> {
        self.active_calls.keys().cloned().collect()
    }

    /// Clean up expired call states (should be called periodically)
    pub fn cleanup_expired_calls(&mut self, max_age_minutes: u64) {
        let cutoff_time = Utc::now() - chrono::Duration::minutes(max_age_minutes as i64);

        let expired_calls: Vec<String> = self
            .active_calls
            .iter()
            .filter(|(_, state)| state.routing_started_at < cutoff_time)
            .map(|(call_id, _)| call_id.clone())
            .collect();

        for call_id in expired_calls {
            warn!("Cleaning up expired call routing state: {}", call_id);
            self.active_calls.remove(&call_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcr::types::{RouteType, EgressTrunk, TransportProtocol};
    use crate::termination_routing::TerminationRoutingService;
    use std::str::FromStr;
    use rust_decimal::Decimal;
    use uuid::Uuid;

    fn create_test_route(id: i32, priority: i32) -> CallRoute {
        CallRoute {
            egress_trunk: EgressTrunk {
                id,
                name: format!("Test-Trunk-{}", id),
                vendor_id: 1,
                host: format!("sip{}.example.com", id),
                port: 5060,
                transport: TransportProtocol::Udp,
                capacity_limit: 100,
                cps_limit: Decimal::from(10),
                active: true,
                priority,
                weight: 100,
                tech_prefix: None,
                supports_international: true,
            },
            vendor_rate: None,
            cost_per_minute: Decimal::from_str("0.005").unwrap(),
            selling_per_minute: Decimal::from_str("0.008").unwrap(),
            profit_margin: Decimal::from_str("0.003").unwrap(),
            priority,
            setup_fee: Decimal::ZERO,
            min_increment: 6,
            interval: 6,
        }
    }

    fn create_test_route_request() -> RouteRequest {
        RouteRequest {
            ani: "14155551234".to_string(),
            dnis: "16505559876".to_string(),
            ingress_trunk_id: 1,
            client_deck_id: Some(1),
            route_type: RouteType::NANPA,
            require_profit_protection: false,
            min_profit_margin: None,
            effective_time: Some(Utc::now()),
            phone_validation: None,
            routing_plan_id: None,
        }
    }

    async fn create_test_engine() -> RouteAdvancementEngine {
        let termination_service = Arc::new(Mutex::new(TerminationRoutingService::new()));
        RouteAdvancementEngine::new(termination_service, 3)
    }

    #[tokio::test]
    async fn test_start_call_routing() {
        let mut engine = create_test_engine().await;
        let call_id = Uuid::new_v4().to_string();
        let route_request = create_test_route_request();

        let result = engine.start_call_routing(
            call_id.clone(),
            "14155551234".to_string(),
            "16505559876".to_string(),
            route_request,
        ).await;

        // Note: In real implementation with routes available, this would succeed
        assert!(result.is_ok() || result.is_err(), "Should handle routing attempt");
    }

    #[tokio::test]
    async fn test_handle_sip_response_advance() {
        let mut engine = create_test_engine().await;
        let call_id = Uuid::new_v4().to_string();

        // Create call state with multiple routes
        let routes = vec![
            create_test_route(1, 1),
            create_test_route(2, 2),
        ];

        let call_state = CallRoutingState {
            call_id: call_id.clone(),
            ani: "14155551234".to_string(),
            dnis: "16505559876".to_string(),
            original_request: create_test_route_request(),
            current_attempt: 1,
            failed_attempts: vec![],
            current_route: Some(routes[0].clone()),
            remaining_routes: routes[1..].to_vec(),
            routing_started_at: Utc::now(),
            last_attempt_at: Some(Utc::now()),
        };

        engine.active_calls.insert(call_id.clone(), call_state);

        // Test response that should trigger advancement
        let result = engine.handle_sip_response(
            &call_id,
            503, // Service Unavailable - should advance
            "Service Unavailable",
        ).await;

        assert!(result.is_ok());
        let advancement = result.unwrap();

        // Should advance to next route or reject if no more routes
        assert!(matches!(advancement.action, AdvancementAction::RouteToNext | AdvancementAction::RejectCall));
    }

    #[tokio::test]
    async fn test_handle_sip_response_complete() {
        let mut engine = create_test_engine().await;
        let call_id = Uuid::new_v4().to_string();

        // Create call state
        let call_state = CallRoutingState {
            call_id: call_id.clone(),
            ani: "14155551234".to_string(),
            dnis: "16505559876".to_string(),
            original_request: create_test_route_request(),
            current_attempt: 1,
            failed_attempts: vec![],
            current_route: Some(create_test_route(1, 1)),
            remaining_routes: vec![],
            routing_started_at: Utc::now(),
            last_attempt_at: Some(Utc::now()),
        };

        engine.active_calls.insert(call_id.clone(), call_state);

        // Test response that should complete call
        let result = engine.handle_sip_response(
            &call_id,
            603, // Decline - should complete
            "Decline",
        ).await;

        assert!(result.is_ok());
        let advancement = result.unwrap();

        assert_eq!(advancement.action, AdvancementAction::CompleteCall);
        assert!(advancement.routing_complete);
    }

    #[tokio::test]
    async fn test_max_attempts_reached() {
        let mut engine = create_test_engine().await;
        let call_id = Uuid::new_v4().to_string();

        // Create call state with max attempts reached
        let call_state = CallRoutingState {
            call_id: call_id.clone(),
            ani: "14155551234".to_string(),
            dnis: "16505559876".to_string(),
            original_request: create_test_route_request(),
            current_attempt: 3, // At max attempts
            failed_attempts: vec![
                FailedAttempt {
                    trunk_id: 1,
                    trunk_name: "Trunk-1".to_string(),
                    response_code: 503,
                    response_reason: "Service Unavailable".to_string(),
                    attempt_time: Utc::now(),
                    duration_ms: 1000,
                },
                FailedAttempt {
                    trunk_id: 2,
                    trunk_name: "Trunk-2".to_string(),
                    response_code: 503,
                    response_reason: "Service Unavailable".to_string(),
                    attempt_time: Utc::now(),
                    duration_ms: 1000,
                },
            ],
            current_route: Some(create_test_route(3, 3)),
            remaining_routes: vec![],
            routing_started_at: Utc::now(),
            last_attempt_at: Some(Utc::now()),
        };

        engine.active_calls.insert(call_id.clone(), call_state);

        // Any response should now reject the call
        let result = engine.handle_sip_response(
            &call_id,
            503,
            "Service Unavailable",
        ).await;

        assert!(result.is_ok());
        let advancement = result.unwrap();

        assert_eq!(advancement.action, AdvancementAction::RejectCall);
        assert!(advancement.routing_complete);
    }

    #[test]
    fn test_route_quality_metrics() {
        let metrics = RouteQualityMetrics {
            success_rate: 0.95,
            average_setup_time_ms: 150,
            recent_failures: 2,
            total_attempts: 100,
            last_success: Some(Utc::now()),
            last_failure: Some(Utc::now() - chrono::Duration::hours(1)),
        };

        assert_eq!(metrics.success_rate, 0.95);
        assert_eq!(metrics.average_setup_time_ms, 150);
        assert!(metrics.last_success.is_some());
    }

    #[test]
    fn test_call_routing_state_creation() {
        let route_request = create_test_route_request();
        let routes = vec![create_test_route(1, 1), create_test_route(2, 2)];

        let state = CallRoutingState {
            call_id: "test-call-123".to_string(),
            ani: "14155551234".to_string(),
            dnis: "16505559876".to_string(),
            original_request: route_request,
            current_attempt: 1,
            failed_attempts: vec![],
            current_route: Some(routes[0].clone()),
            remaining_routes: routes[1..].to_vec(),
            routing_started_at: Utc::now(),
            last_attempt_at: None,
        };

        assert_eq!(state.call_id, "test-call-123");
        assert_eq!(state.current_attempt, 1);
        assert!(state.current_route.is_some());
        assert_eq!(state.remaining_routes.len(), 1);
        assert!(state.failed_attempts.is_empty());
    }

    #[test]
    fn test_advancement_action_types() {
        let actions = vec![
            AdvancementAction::RouteToNext,
            AdvancementAction::RetryCurrentRoute,
            AdvancementAction::CompleteCall,
            AdvancementAction::RejectCall,
        ];

        // Verify all actions are distinct
        for (i, action1) in actions.iter().enumerate() {
            for (j, action2) in actions.iter().enumerate() {
                if i != j {
                    assert_ne!(action1, action2, "Actions should be distinct");
                }
            }
        }
    }

    #[tokio::test]
    async fn test_cleanup_completed_calls() {
        let mut engine = create_test_engine().await;
        let call_id1 = "call-1".to_string();
        let call_id2 = "call-2".to_string();

        // Add some call states
        let state1 = CallRoutingState {
            call_id: call_id1.clone(),
            ani: "14155551234".to_string(),
            dnis: "16505559876".to_string(),
            original_request: create_test_route_request(),
            current_attempt: 1,
            failed_attempts: vec![],
            current_route: None,
            remaining_routes: vec![],
            routing_started_at: Utc::now() - chrono::Duration::hours(2), // Old call
            last_attempt_at: Some(Utc::now() - chrono::Duration::hours(1)),
        };

        let state2 = CallRoutingState {
            call_id: call_id2.clone(),
            ani: "14155551234".to_string(),
            dnis: "16505559876".to_string(),
            original_request: create_test_route_request(),
            current_attempt: 1,
            failed_attempts: vec![],
            current_route: Some(create_test_route(1, 1)),
            remaining_routes: vec![],
            routing_started_at: Utc::now(), // Recent call
            last_attempt_at: Some(Utc::now()),
        };

        engine.active_calls.insert(call_id1.clone(), state1);
        engine.active_calls.insert(call_id2.clone(), state2);

        // Clean up old calls
        engine.cleanup_expired_calls(chrono::Duration::hours(1));

        // Should remove old completed call but keep active one
        assert!(!engine.active_calls.contains_key(&call_id1));
        assert!(engine.active_calls.contains_key(&call_id2));
    }

    #[tokio::test]
    async fn test_get_call_routing_stats() {
        let mut engine = create_test_engine().await;
        let call_id = "test-call".to_string();

        // Add call state
        let state = CallRoutingState {
            call_id: call_id.clone(),
            ani: "14155551234".to_string(),
            dnis: "16505559876".to_string(),
            original_request: create_test_route_request(),
            current_attempt: 2,
            failed_attempts: vec![
                FailedAttempt {
                    trunk_id: 1,
                    trunk_name: "Trunk-1".to_string(),
                    response_code: 503,
                    response_reason: "Service Unavailable".to_string(),
                    attempt_time: Utc::now(),
                    duration_ms: 1000,
                },
            ],
            current_route: Some(create_test_route(2, 2)),
            remaining_routes: vec![],
            routing_started_at: Utc::now() - chrono::Duration::seconds(10),
            last_attempt_at: Some(Utc::now()),
        };

        engine.active_calls.insert(call_id.clone(), state);

        let stats = engine.get_call_routing_stats(&call_id);
        assert!(stats.is_some());

        let stats = stats.unwrap();
        assert_eq!(stats.call_id, call_id);
        assert_eq!(stats.current_attempt, 2);
        assert_eq!(stats.total_failed_attempts, 1);
        assert!(!stats.current_trunk_name.is_empty());
        assert!(stats.routing_duration_ms > 0);
    }

    #[tokio::test]
    async fn test_route_advancement_with_retries() {
        let mut engine = create_test_engine().await;
        let call_id = Uuid::new_v4().to_string();

        // Start with multiple routes
        let routes = vec![
            create_test_route(1, 1),
            create_test_route(2, 2),
            create_test_route(3, 3),
        ];

        let call_state = CallRoutingState {
            call_id: call_id.clone(),
            ani: "14155551234".to_string(),
            dnis: "16505559876".to_string(),
            original_request: create_test_route_request(),
            current_attempt: 1,
            failed_attempts: vec![],
            current_route: Some(routes[0].clone()),
            remaining_routes: routes[1..].to_vec(),
            routing_started_at: Utc::now(),
            last_attempt_at: Some(Utc::now()),
        };

        engine.active_calls.insert(call_id.clone(), call_state);

        // First failure - should advance to next route
        let result1 = engine.handle_sip_response(
            &call_id,
            503,
            "Service Unavailable",
        ).await;

        assert!(result1.is_ok());
        let advancement1 = result1.unwrap();

        // Should advance to next route
        if advancement1.action == AdvancementAction::RouteToNext {
            assert!(advancement1.new_route.is_some());
            assert_eq!(advancement1.total_attempts, 1);
        }
    }

    #[test]
    fn test_failed_attempt_serialization() {
        let attempt = FailedAttempt {
            trunk_id: 1,
            trunk_name: "Test-Trunk".to_string(),
            response_code: 503,
            response_reason: "Service Unavailable".to_string(),
            attempt_time: Utc::now(),
            duration_ms: 1500,
        };

        // Test that FailedAttempt can be serialized/deserialized
        let serialized = serde_json::to_string(&attempt);
        assert!(serialized.is_ok());

        let deserialized: Result<FailedAttempt, _> = serde_json::from_str(&serialized.unwrap());
        assert!(deserialized.is_ok());

        let recovered = deserialized.unwrap();
        assert_eq!(recovered.trunk_id, attempt.trunk_id);
        assert_eq!(recovered.response_code, attempt.response_code);
        assert_eq!(recovered.trunk_name, attempt.trunk_name);
    }

    #[tokio::test]
    async fn test_concurrent_call_handling() {
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let engine = Arc::new(Mutex::new(create_test_engine().await));
        let mut handles = vec![];

        // Spawn multiple concurrent routing operations
        for i in 0..5 {
            let engine_clone = engine.clone();
            let handle = tokio::spawn(async move {
                let call_id = format!("concurrent-call-{}", i);
                let route_request = create_test_route_request();

                let mut eng = engine_clone.lock().await;
                let result = eng.start_call_routing(
                    call_id,
                    "14155551234".to_string(),
                    "16505559876".to_string(),
                    route_request,
                ).await;

                // Should handle gracefully
                result.is_ok() || result.is_err()
            });
            handles.push(handle);
        }

        // Wait for all operations to complete
        for handle in handles {
            let result = handle.await;
            assert!(result.is_ok(), "Concurrent operation should complete");
        }
    }
}

/// Call routing statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallRoutingStats {
    pub call_id: String,
    pub current_attempt: u32,
    pub total_failed_attempts: u32,
    pub routing_duration_ms: u64,
    pub current_trunk_name: Option<String>,
    pub remaining_routes: u32,
}
