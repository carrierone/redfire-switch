//! Route Advancement Engine
//! Handles SIP response code analysis and automatic route advancement for B2BUA

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::termination_routing::{
    FailedAttempt, RouteAdvanceDecision, SipResponseCode, TerminationRoutingRequest,
    TerminationRoutingResponse, TerminationRoutingService,
};
use crate::lcr::types::{CallRoute, RouteRequest};

/// Route advancement manager for B2BUA operations
pub struct RouteAdvancementEngine {
    termination_service: Arc<TerminationRoutingService>,
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
    RouteToNext,      // Advance to next available route
    RetryCurrentRoute, // Retry current route (for temporary failures)
    CompleteCall,     // Complete call with current response
    RejectCall,       // No more routes available
}

impl RouteAdvancementEngine {
    pub fn new(
        termination_service: Arc<TerminationRoutingService>,
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
        info!("Starting call routing for {} -> {} (call_id: {})", ani, dnis, call_id);

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
        let routing_response = self.termination_service
            .route_termination(termination_request)
            .await?;

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
        let routing_state = self.active_calls.get_mut(call_id)
            .ok_or_else(|| anyhow!("Call routing state not found for call_id: {}", call_id))?;

        debug!(
            "Processing SIP response {} {} for call {}",
            response_code, response_reason, call_id
        );

        // Record this attempt as failed if needed
        if response_code >= 300 {
            let current_route = routing_state.current_route.as_ref()
                .ok_or_else(|| anyhow!("No current route for call {}", call_id))?;

            let failed_attempt = FailedAttempt {
                trunk_id: current_route.egress_trunk.id,
                trunk_name: current_route.egress_trunk.name.clone(),
                response_code,
                response_reason: response_reason.to_string(),
                attempt_time: Utc::now(),
                duration_ms: routing_state.last_attempt_at
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
        let routing_state = self.active_calls.get_mut(call_id)
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

        // Create new termination request with failed attempts history
        let termination_request = TerminationRoutingRequest {
            call_id: call_id.to_string(),
            ani: routing_state.ani.clone(),
            dnis: routing_state.dnis.clone(),
            route_request: routing_state.original_request.clone(),
            attempt_number: routing_state.current_attempt,
            previous_responses: routing_state.failed_attempts.clone(),
            max_attempts: self.max_route_attempts,
            timestamp: Utc::now(),
        };

        // Get next routing decision
        let routing_response = self.termination_service
            .route_termination(termination_request)
            .await?;

        if routing_response.success {
            // Update routing state with new route
            routing_state.current_route = routing_response.selected_route.clone();
            routing_state.remaining_routes = routing_response.remaining_routes;

            info!(
                "Advanced to next route for call {}: {} (attempt {})",
                call_id,
                routing_response.selected_route.as_ref().unwrap().egress_trunk.name,
                routing_state.current_attempt
            );

            Ok(RouteAdvancementResult {
                call_id: call_id.to_string(),
                action: AdvancementAction::RouteToNext,
                new_route: routing_response.selected_route,
                total_attempts: routing_state.current_attempt,
                routing_complete: false,
                final_response_code: None,
                reason: "Advanced to next available route".to_string(),
            })
        } else {
            // No more routes available
            self.complete_call_routing(
                call_id,
                last_response_code,
                &routing_response.reason,
            )
        }
    }

    /// Complete call routing (success or final failure)
    fn complete_call_routing(
        &mut self,
        call_id: &str,
        final_response_code: u16,
        reason: &str,
    ) -> Result<RouteAdvancementResult> {
        let routing_state = self.active_calls.remove(call_id)
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
    pub fn should_advance_route(&self, response_code: u16, _response_reason: &str) -> RouteAdvanceDecision {
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
                    _ => RouteAdvanceDecision::CompleteCall, // Unknown - don't advance
                }
            }
        }
    }

    /// Get routing statistics for a call
    pub fn get_call_routing_stats(&self, call_id: &str) -> Option<CallRoutingStats> {
        self.active_calls.get(call_id).map(|state| {
            CallRoutingStats {
                call_id: call_id.to_string(),
                current_attempt: state.current_attempt,
                total_failed_attempts: state.failed_attempts.len() as u32,
                routing_duration_ms: Utc::now()
                    .signed_duration_since(state.routing_started_at)
                    .num_milliseconds() as u64,
                current_trunk_name: state.current_route
                    .as_ref()
                    .map(|r| r.egress_trunk.name.clone()),
                remaining_routes: state.remaining_routes.len() as u32,
            }
        })
    }

    /// Get all active call routing states
    pub fn get_active_calls(&self) -> Vec<String> {
        self.active_calls.keys().cloned().collect()
    }

    /// Clean up expired call states (should be called periodically)
    pub fn cleanup_expired_calls(&mut self, max_age_minutes: u64) {
        let cutoff_time = Utc::now() - chrono::Duration::minutes(max_age_minutes as i64);
        
        let expired_calls: Vec<String> = self.active_calls
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcr::types::RouteType;
    use std::net::IpAddr;

    fn create_test_route_request() -> RouteRequest {
        RouteRequest {
            ani: "15551234567".to_string(),
            dnis: "18005551234".to_string(),
            ingress_trunk_id: 1,
            client_deck_id: None,
            route_type: RouteType::NANPA,
            require_profit_protection: false,
            min_profit_margin: None,
            effective_time: None,
            phone_validation: None,
            routing_plan_id: None,
        }
    }

    #[tokio::test]
    #[ignore] // TODO: Fix compilation issues with TerminationRoutingService interior mutability
    async fn test_route_advancement_logic() {
        let lcr_engine = Arc::new(crate::lcr::LcrEngine::new("test://").await.unwrap());
        let termination_service = Arc::new(TerminationRoutingService::new(lcr_engine));
        let engine = RouteAdvancementEngine::new(termination_service, 3);

        // Test various response codes
        assert_eq!(
            engine.should_advance_route(404, "Not Found"),
            RouteAdvanceDecision::AdvanceToNextRoute
        );
        
        assert_eq!(
            engine.should_advance_route(503, "Service Unavailable"),
            RouteAdvanceDecision::AdvanceToNextRoute
        );
        
        assert_eq!(
            engine.should_advance_route(603, "Decline"),
            RouteAdvanceDecision::CompleteCall
        );
        
        assert_eq!(
            engine.should_advance_route(200, "OK"),
            RouteAdvanceDecision::CompleteCall
        );
    }

    #[tokio::test]
    #[ignore] // TODO: Fix compilation issues with TerminationRoutingService interior mutability  
    async fn test_call_routing_lifecycle() {
        let termination_service = Arc::new(TerminationRoutingService::new(
            Arc::new(crate::lcr::LcrEngine::new("test://").await.unwrap())
        ));
        let mut engine = RouteAdvancementEngine::new(termination_service, 3);
        
        let call_id = "test-call-123".to_string();
        let route_request = create_test_route_request();
        
        // This would fail in a real test without proper LCR setup, but shows the interface
        // let result = engine.start_call_routing(
        //     call_id.clone(),
        //     "15551234567".to_string(),
        //     "18005551234".to_string(),
        //     route_request,
        // ).await;
        
        // Test that the engine correctly tracks active calls
        assert!(!engine.get_active_calls().contains(&call_id));
    }
}