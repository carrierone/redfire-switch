use anyhow::{anyhow, Result};
use regex::Regex;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::Arc;

use crate::lcr::types::*;

use crate::lcr::cache::LcrCache;
use crate::lcr::jurisdiction::JurisdictionCalculator;
use crate::lcr::timers::TimerManager;
use crate::lcr::trunk_manager::TrunkManager;
pub use crate::lcr::types::RouteResponse;

pub struct RoutingEngine {
    cache: Arc<LcrCache>,
    trunk_manager: Arc<TrunkManager>,
    timer_manager: Arc<TimerManager>,
    static_route_patterns: HashMap<i32, Regex>, // Cache compiled regexes
}

impl RoutingEngine {
    pub fn new(
        cache: Arc<LcrCache>,
        trunk_manager: Arc<TrunkManager>,
        timer_manager: Arc<TimerManager>,
    ) -> Self {
        Self {
            cache,
            trunk_manager,
            timer_manager,
            static_route_patterns: HashMap::new(),
        }
    }

    pub async fn find_routes(&self, request: &RouteRequest) -> Result<RouteResponse> {
        // Get ingress trunk info
        let ingress_trunk = self
            .cache
            .get_ingress_trunk(request.ingress_trunk_id)
            .ok_or_else(|| anyhow!("Ingress trunk {} not found", request.ingress_trunk_id))?;

        // Check static routes BEFORE dynamic routing
        if let Some(static_route) = self.check_static_routes(
            &request.dnis,
            Some(request.ingress_trunk_id),
            RoutePosition::Before,
        ) {
            return self
                .build_static_route_response(static_route, request)
                .await;
        }

        // Determine jurisdiction and get LRN if needed
        let use_lrn = request.route_type == RouteType::NANPA;
        let (jurisdiction, lrn) = JurisdictionCalculator::get_jurisdiction_with_lrn(
            &self.cache,
            &request.ani,
            &request.dnis,
            use_lrn,
        )
        .await;

        // Get the number to use for rating
        let rating_number = if let Some(ref lrn_number) = lrn {
            lrn_number
        } else {
            &request.dnis
        };

        // Extract code for rating (1NPANXX for NANPA)
        let rating_code = if request.route_type == RouteType::NANPA {
            JurisdictionCalculator::normalize_nanpa_number(rating_number)
        } else {
            rating_number.to_string()
        };

        // Get client rate - either specified or from ingress trunk association
        let client_rate = if let Some(deck_id) = request.client_deck_id {
            self.cache.get_client_rate(deck_id, &rating_code)
        } else {
            // Try to find client rate deck associated with ingress trunk
            let client_deck_ids = self
                .cache
                .get_client_decks_for_trunk(request.ingress_trunk_id);
            client_deck_ids
                .iter()
                .find_map(|&deck_id| self.cache.get_client_rate(deck_id, &rating_code))
        };

        // Build list of potential routes
        let mut routes = Vec::new();

        // Get all active egress trunks
        for egress_trunk in self.cache.get_all_egress_trunks() {
            if !egress_trunk.active {
                continue;
            }

            // Check trunk capacity
            if !self
                .trunk_manager
                .can_accept_call(egress_trunk.id, TrunkType::Egress)
                .await
            {
                continue;
            }

            // Get vendor rate decks associated with this trunk
            let vendor_deck_ids = self.cache.get_vendor_decks_for_trunk(egress_trunk.id);

            for deck_id in vendor_deck_ids {
                if let Some(vendor_rate) = self.cache.get_vendor_rate(deck_id, &rating_code) {
                    // Calculate costs based on jurisdiction
                    let cost_per_minute = match jurisdiction {
                        CallJurisdiction::Interstate => vendor_rate.inter_rate,
                        CallJurisdiction::Intrastate => vendor_rate.intra_rate,
                        CallJurisdiction::IndeterminateJurisdiction => vendor_rate.ij_rate,
                        CallJurisdiction::Local => {
                            vendor_rate.local_rate.unwrap_or(vendor_rate.intra_rate)
                        }
                    };

                    let selling_per_minute = if let Some(ref client_rate) = client_rate {
                        match jurisdiction {
                            CallJurisdiction::Interstate => client_rate.inter_rate,
                            CallJurisdiction::Intrastate => client_rate.intra_rate,
                            CallJurisdiction::IndeterminateJurisdiction => client_rate.ij_rate,
                            CallJurisdiction::Local => {
                                client_rate.local_rate.unwrap_or(client_rate.intra_rate)
                            }
                        }
                    } else {
                        // Default markup if no client rate specified
                        cost_per_minute * dec!(1.2)
                    };

                    let profit_margin = selling_per_minute - cost_per_minute;

                    // Check profit protection
                    if ingress_trunk.profit_protection || request.require_profit_protection {
                        let min_margin = request
                            .min_profit_margin
                            .unwrap_or(ingress_trunk.min_profit_margin);

                        if profit_margin < min_margin {
                            continue; // Skip this route due to insufficient profit
                        }
                    }

                    routes.push(CallRoute {
                        egress_trunk: egress_trunk.clone(),
                        vendor_rate: Some(vendor_rate.clone()),
                        cost_per_minute,
                        selling_per_minute,
                        profit_margin,
                        priority: egress_trunk.priority,
                        setup_fee: vendor_rate.setup_fee.unwrap_or(Decimal::ZERO),
                        min_increment: vendor_rate.min_increment,
                        interval: vendor_rate.interval,
                    });
                }
            }
        }

        // LCR Sort: Cost first (including setup fees), then by trunk priority, then by vendor reliability
        routes.sort_by(|a, b| {
            // For typical 60-second call, calculate total cost including setup
            let a_total_cost = a.setup_fee + a.cost_per_minute;
            let b_total_cost = b.setup_fee + b.cost_per_minute;

            a_total_cost
                .cmp(&b_total_cost)
                .then(a.priority.cmp(&b.priority))
                .then(a.egress_trunk.vendor_id.cmp(&b.egress_trunk.vendor_id)) // Vendor consistency
        });

        // Check static routes AFTER dynamic routing
        if routes.is_empty() {
            if let Some(static_route) = self.check_static_routes(
                &request.dnis,
                Some(request.ingress_trunk_id),
                RoutePosition::After,
            ) {
                return self
                    .build_static_route_response(static_route, request)
                    .await;
            }
        }

        Ok(RouteResponse {
            total_routes: routes.len(),
            routes,
            jurisdiction,
            lrn,
        })
    }

    fn check_static_routes(
        &self,
        dnis: &str,
        ingress_trunk_id: Option<i32>,
        position: RoutePosition,
    ) -> Option<StaticRoute> {
        let static_routes = self.cache.get_static_routes_for_ingress(ingress_trunk_id);

        for route in static_routes {
            if !route.active || route.position != position {
                continue;
            }

            // Check if pattern matches
            if let Ok(regex) = Regex::new(&route.pattern) {
                if regex.is_match(dnis) {
                    return Some(route);
                }
            }
        }

        None
    }

    async fn build_static_route_response(
        &self,
        static_route: StaticRoute,
        _request: &RouteRequest,
    ) -> Result<RouteResponse> {
        let egress_trunk = self
            .cache
            .get_egress_trunk(static_route.egress_trunk_id)
            .ok_or_else(|| anyhow!("Egress trunk {} not found", static_route.egress_trunk_id))?;

        // For static routes, we don't calculate actual costs
        // Use placeholder values or fetch from configuration
        let route = CallRoute {
            egress_trunk,
            vendor_rate: None,
            cost_per_minute: Decimal::ZERO,
            selling_per_minute: Decimal::ZERO,
            profit_margin: Decimal::ZERO,
            priority: static_route.priority,
            setup_fee: Decimal::ZERO,
            min_increment: 6,
            interval: 6,
        };

        Ok(RouteResponse {
            routes: vec![route],
            jurisdiction: CallJurisdiction::IndeterminateJurisdiction,
            lrn: None,
            total_routes: 1,
        })
    }

    pub async fn handle_route_advance(
        &self,
        response_code: SipResponseCode,
        ingress_trunk_id: i32,
        current_route_index: usize,
        available_routes: &[CallRoute],
    ) -> Option<usize> {
        // Get route advance configuration
        let config = self
            .cache
            .get_route_advance_config(ConfigScope::IngressTrunk, Some(ingress_trunk_id));

        // Check if we should stop or advance
        if response_code.should_stop(&config) {
            return None; // Stop routing
        }

        if response_code.should_advance(&config) {
            // Find next available route with capacity
            for next_index in (current_route_index + 1)..available_routes.len() {
                let next_trunk_id = available_routes[next_index].egress_trunk.id;
                if self
                    .trunk_manager
                    .can_accept_call(next_trunk_id, TrunkType::Egress)
                    .await
                {
                    return Some(next_index);
                }
            }
        }

        None // No more routes available
    }

    pub async fn simulate_call(
        &self,
        ani: &str,
        dnis: &str,
        ingress_trunk_name: Option<&str>,
    ) -> Result<CallSimulation> {
        // Find ingress trunk by name or use first available
        let ingress_trunk = if let Some(name) = ingress_trunk_name {
            self.cache
                .get_all_ingress_trunks()
                .into_iter()
                .find(|t| t.name == name)
                .ok_or_else(|| anyhow!("Ingress trunk '{}' not found", name))?
        } else {
            self.cache
                .get_all_ingress_trunks()
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("No ingress trunks configured"))?
        };

        // Create route request
        let request = RouteRequest {
            ani: ani.to_string(),
            dnis: dnis.to_string(),
            ingress_trunk_id: ingress_trunk.id,
            client_deck_id: None, // TODO: Get from trunk associations
            route_type: RouteType::NANPA,
            require_profit_protection: ingress_trunk.profit_protection,
            min_profit_margin: Some(ingress_trunk.min_profit_margin),
        };

        // Find routes
        let response = self.find_routes(&request).await?;

        // Build simulation result
        Ok(CallSimulation {
            ani: ani.to_string(),
            dnis: dnis.to_string(),
            lrn: response.lrn,
            jurisdiction: response.jurisdiction,
            ingress_trunk: ingress_trunk.name,
            total_routes: response.total_routes,
            routes: response
                .routes
                .into_iter()
                .map(|r| SimulatedRoute {
                    egress_trunk: r.egress_trunk.name,
                    vendor: format!("Vendor {}", r.egress_trunk.vendor_id),
                    cost_per_minute: r.cost_per_minute,
                    selling_per_minute: r.selling_per_minute,
                    profit_margin: r.profit_margin,
                    priority: r.priority,
                    setup_fee: r.setup_fee,
                    min_increment: r.min_increment,
                    interval: r.interval,
                })
                .collect(),
            routing_decision: if response.total_routes > 0 {
                "ROUTE_FOUND".to_string()
            } else {
                "NO_ROUTE_AVAILABLE".to_string()
            },
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSimulation {
    pub ani: String,
    pub dnis: String,
    pub lrn: Option<String>,
    pub jurisdiction: CallJurisdiction,
    pub ingress_trunk: String,
    pub total_routes: usize,
    pub routes: Vec<SimulatedRoute>,
    pub routing_decision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatedRoute {
    pub egress_trunk: String,
    pub vendor: String,
    pub cost_per_minute: Decimal,
    pub selling_per_minute: Decimal,
    pub profit_margin: Decimal,
    pub priority: i32,
    pub setup_fee: Decimal,
    pub min_increment: i32,
    pub interval: i32,
}

use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

impl RoutingEngine {
    /// Calculate total call cost including setup fees and billing increments
    fn calculate_call_cost(
        &self,
        rate_per_minute: Decimal,
        setup_fee: Decimal,
        min_increment: i32,
        interval: i32,
        call_duration_seconds: i32,
    ) -> Decimal {
        // Calculate billed duration based on billing increments
        let billed_duration = if call_duration_seconds <= min_increment {
            min_increment
        } else {
            let excess = call_duration_seconds - min_increment;
            let additional_intervals = (excess + interval - 1) / interval; // Ceiling division
            min_increment + (additional_intervals * interval)
        };

        // Calculate cost: setup fee + (rate per minute * billed minutes)
        let billed_minutes = Decimal::from(billed_duration) / Decimal::from(60);
        setup_fee + (rate_per_minute * billed_minutes)
    }
}
