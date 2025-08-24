use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use regex::Regex;
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

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
    pool: PgPool,
    static_route_patterns: HashMap<i32, Regex>,
}

impl RoutingEngine {
    pub fn new(
        cache: Arc<LcrCache>,
        trunk_manager: Arc<TrunkManager>,
        timer_manager: Arc<TimerManager>,
        pool: PgPool,
    ) -> Self {
        Self {
            cache,
            trunk_manager,
            timer_manager,
            pool,
            static_route_patterns: HashMap::new(),
        }
    }

    /// Find routes with effective date consideration
    pub async fn find_routes(&self, request: &RouteRequest) -> Result<RouteResponse> {
        // Determine the effective timestamp for this routing request
        let effective_time = request.effective_time.unwrap_or_else(Utc::now);
        
        info!(
            "Finding routes for {} -> {} at effective time {}",
            request.ani, request.dnis, effective_time
        );

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

        // Get client rate using effective date
        let client_rate = if let Some(deck_id) = request.client_deck_id {
            self.get_client_rate_at_time(deck_id, &rating_code, effective_time).await?
        } else {
            // Try to find client rate deck associated with ingress trunk at effective time
            self.get_client_rate_for_trunk_at_time(
                request.ingress_trunk_id,
                &rating_code,
                effective_time,
            ).await?
        };

        // Build list of potential routes with time-aware rates
        let mut routes: Vec<CallRoute> = Vec::new();

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

            // Get vendor rate using effective date
            let vendor_rate = self
                .get_vendor_rate_for_trunk_at_time(
                    egress_trunk.id,
                    &rating_code,
                    effective_time,
                )
                .await?;

            if let Some(rate) = vendor_rate {
                // Calculate costs based on jurisdiction
                let cost = self.calculate_cost(&rate, jurisdiction);
                let sell = client_rate
                    .as_ref()
                    .map(|cr| self.calculate_cost(cr, jurisdiction))
                    .unwrap_or(cost);
                let profit = sell - cost;

                // Apply profit protection if required
                if request.require_profit_protection || ingress_trunk.profit_protection {
                    let min_margin = request
                        .min_profit_margin
                        .unwrap_or(ingress_trunk.min_profit_margin);
                    if profit < min_margin {
                        continue;
                    }
                }

                routes.push(CallRoute {
                    egress_trunk: egress_trunk.clone(),
                    vendor_rate: Some(rate.clone()),
                    cost_per_minute: cost,
                    selling_per_minute: sell,
                    profit_margin: profit,
                    priority: egress_trunk.priority,
                    setup_fee: rate.setup_fee.unwrap_or(Decimal::ZERO),
                    min_increment: rate.min_increment,
                    interval: rate.interval,
                });
            }
        }

        // Sort routes by priority, then by cost
        routes.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then(a.cost_per_minute.cmp(&b.cost_per_minute))
        });

        // Check static routes AFTER dynamic routing if no routes found
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

        let total_routes = routes.len();
        Ok(RouteResponse {
            routes,
            jurisdiction,
            lrn,
            total_routes,
        })
    }

    /// Get client rate at specific time
    async fn get_client_rate_at_time(
        &self,
        deck_id: i32,
        code: &str,
        effective_time: DateTime<Utc>,
    ) -> Result<Option<NanpaRate>> {
        // First check if deck is active at the given time
        let deck_active = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM client_rate_decks
                WHERE id = $1
                  AND effective_date <= $2
                  AND (end_date IS NULL OR end_date > $2)
                  AND active = true
            )
            "#
        )
        .bind(deck_id)
        .bind(effective_time)
        .fetch_one(&self.pool)
        .await?;

        if !deck_active {
            // Find the correct version for this time
            let correct_deck_id = sqlx::query_scalar::<_, Option<i32>>(
                r#"
                SELECT id FROM client_rate_decks
                WHERE name = (SELECT name FROM client_rate_decks WHERE id = $1)
                  AND client_id = (SELECT client_id FROM client_rate_decks WHERE id = $1)
                  AND effective_date <= $2
                  AND (end_date IS NULL OR end_date > $2)
                  AND active = true
                ORDER BY deck_version DESC
                LIMIT 1
                "#
            )
            .bind(deck_id)
            .bind(effective_time)
            .fetch_optional(&self.pool)
            .await?;

            if let Some(Some(active_deck_id)) = correct_deck_id {
                // Try cache first, then database
                if let Some(rate) = self.cache.get_client_rate(active_deck_id, code) {
                    return Ok(Some(rate));
                }
                
                // Load from database if not in cache
                return self.load_client_rate_from_db(active_deck_id, code).await;
            }
        } else {
            // Deck is active, check cache first
            if let Some(rate) = self.cache.get_client_rate(deck_id, code) {
                return Ok(Some(rate));
            }
            
            // Load from database if not in cache
            return self.load_client_rate_from_db(deck_id, code).await;
        }

        Ok(None)
    }

    /// Get vendor rate at specific time
    async fn get_vendor_rate_for_trunk_at_time(
        &self,
        trunk_id: i32,
        code: &str,
        effective_time: DateTime<Utc>,
    ) -> Result<Option<NanpaRate>> {
        // Get the vendor deck associated with this trunk at the given time
        let deck_id = sqlx::query_scalar::<_, Option<i32>>(
            r#"
            SELECT vrd.id
            FROM lcr_route_trunks lrt
            JOIN vendor_rate_decks vrd ON vrd.id = lrt.vendor_deck_id
            WHERE lrt.egress_trunk_id = $1
              AND vrd.effective_date <= $2
              AND (vrd.end_date IS NULL OR vrd.end_date > $2)
              AND vrd.active = true
            ORDER BY vrd.deck_version DESC
            LIMIT 1
            "#
        )
        .bind(trunk_id)
        .bind(effective_time)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(Some(deck_id)) = deck_id {
            // Check cache first
            if let Some(rate) = self.cache.get_vendor_rate(deck_id, code) {
                return Ok(Some(rate));
            }
            
            // Load from database if not in cache
            return self.load_vendor_rate_from_db(deck_id, code).await;
        }

        Ok(None)
    }

    /// Get client rate for trunk at specific time
    async fn get_client_rate_for_trunk_at_time(
        &self,
        trunk_id: i32,
        code: &str,
        effective_time: DateTime<Utc>,
    ) -> Result<Option<NanpaRate>> {
        let deck_id = sqlx::query_scalar::<_, Option<i32>>(
            r#"
            SELECT crd.id
            FROM trunk_rate_associations tra
            JOIN client_rate_decks crd ON crd.id = tra.client_deck_id
            WHERE tra.ingress_trunk_id = $1
              AND crd.effective_date <= $2
              AND (crd.end_date IS NULL OR crd.end_date > $2)
              AND crd.active = true
            ORDER BY crd.deck_version DESC
            LIMIT 1
            "#
        )
        .bind(trunk_id)
        .bind(effective_time)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(Some(deck_id)) = deck_id {
            if let Some(rate) = self.cache.get_client_rate(deck_id, code) {
                return Ok(Some(rate));
            }
            
            return self.load_client_rate_from_db(deck_id, code).await;
        }

        Ok(None)
    }

    /// Load vendor rate from database
    async fn load_vendor_rate_from_db(&self, deck_id: i32, code: &str) -> Result<Option<NanpaRate>> {
        // Try exact match first
        let row = sqlx::query(
            r#"
            SELECT id, deck_id, code, inter_rate, intra_rate, ij_rate, 
                   local_rate, min_increment, interval, setup_fee
            FROM vendor_nanpa_rates
            WHERE deck_id = $1 AND code = $2
            "#
        )
        .bind(deck_id)
        .bind(code)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            return Ok(Some(NanpaRate {
                id: row.get("id"),
                deck_id: row.get("deck_id"),
                code: row.get("code"),
                inter_rate: Decimal::try_from(row.get::<f64, _>("inter_rate")).unwrap_or_default(),
                intra_rate: Decimal::try_from(row.get::<f64, _>("intra_rate")).unwrap_or_default(),
                ij_rate: Decimal::try_from(row.get::<f64, _>("ij_rate")).unwrap_or_default(),
                local_rate: row.get::<Option<f64>, _>("local_rate").map(|f| Decimal::try_from(f).unwrap_or_default()),
                min_increment: row.get("min_increment"),
                interval: row.get("interval"),
                setup_fee: row.get::<Option<f64>, _>("setup_fee").map(|f| Decimal::try_from(f).unwrap_or_default()),
            }));
        }

        // Try prefix matching for less specific rates
        for i in (3..=code.len()).rev() {
            let prefix = &code[..i];
            let row = sqlx::query(
                r#"
                SELECT id, deck_id, code, inter_rate, intra_rate, ij_rate, 
                       local_rate, min_increment, interval, setup_fee
                FROM vendor_nanpa_rates
                WHERE deck_id = $1 AND code = $2
                "#
            )
            .bind(deck_id)
            .bind(prefix)
            .fetch_optional(&self.pool)
            .await?;

            if let Some(row) = row {
                return Ok(Some(NanpaRate {
                    id: row.get("id"),
                    deck_id: row.get("deck_id"),
                    code: row.get("code"),
                    inter_rate: Decimal::try_from(row.get::<f64, _>("inter_rate")).unwrap_or_default(),
                    intra_rate: Decimal::try_from(row.get::<f64, _>("intra_rate")).unwrap_or_default(),
                    ij_rate: Decimal::try_from(row.get::<f64, _>("ij_rate")).unwrap_or_default(),
                    local_rate: row.get::<Option<f64>, _>("local_rate").map(|f| Decimal::try_from(f).unwrap_or_default()),
                    min_increment: row.get("min_increment"),
                    interval: row.get("interval"),
                    setup_fee: row.get::<Option<f64>, _>("setup_fee").map(|f| Decimal::try_from(f).unwrap_or_default()),
                }));
            }
        }

        Ok(None)
    }

    /// Load client rate from database
    async fn load_client_rate_from_db(&self, deck_id: i32, code: &str) -> Result<Option<NanpaRate>> {
        // Try exact match first
        let row = sqlx::query(
            r#"
            SELECT id, deck_id, code, inter_rate, intra_rate, ij_rate, 
                   local_rate, min_increment, interval, setup_fee
            FROM client_nanpa_rates
            WHERE deck_id = $1 AND code = $2
            "#
        )
        .bind(deck_id)
        .bind(code)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            return Ok(Some(NanpaRate {
                id: row.get("id"),
                deck_id: row.get("deck_id"),
                code: row.get("code"),
                inter_rate: Decimal::try_from(row.get::<f64, _>("inter_rate")).unwrap_or_default(),
                intra_rate: Decimal::try_from(row.get::<f64, _>("intra_rate")).unwrap_or_default(),
                ij_rate: Decimal::try_from(row.get::<f64, _>("ij_rate")).unwrap_or_default(),
                local_rate: row.get::<Option<f64>, _>("local_rate").map(|f| Decimal::try_from(f).unwrap_or_default()),
                min_increment: row.get("min_increment"),
                interval: row.get("interval"),
                setup_fee: row.get::<Option<f64>, _>("setup_fee").map(|f| Decimal::try_from(f).unwrap_or_default()),
            }));
        }

        // Try prefix matching for less specific rates
        for i in (3..=code.len()).rev() {
            let prefix = &code[..i];
            let row = sqlx::query(
                r#"
                SELECT id, deck_id, code, inter_rate, intra_rate, ij_rate, 
                       local_rate, min_increment, interval, setup_fee
                FROM client_nanpa_rates
                WHERE deck_id = $1 AND code = $2
                "#
            )
            .bind(deck_id)
            .bind(prefix)
            .fetch_optional(&self.pool)
            .await?;

            if let Some(row) = row {
                return Ok(Some(NanpaRate {
                    id: row.get("id"),
                    deck_id: row.get("deck_id"),
                    code: row.get("code"),
                    inter_rate: Decimal::try_from(row.get::<f64, _>("inter_rate")).unwrap_or_default(),
                    intra_rate: Decimal::try_from(row.get::<f64, _>("intra_rate")).unwrap_or_default(),
                    ij_rate: Decimal::try_from(row.get::<f64, _>("ij_rate")).unwrap_or_default(),
                    local_rate: row.get::<Option<f64>, _>("local_rate").map(|f| Decimal::try_from(f).unwrap_or_default()),
                    min_increment: row.get("min_increment"),
                    interval: row.get("interval"),
                    setup_fee: row.get::<Option<f64>, _>("setup_fee").map(|f| Decimal::try_from(f).unwrap_or_default()),
                }));
            }
        }

        Ok(None)
    }

    /// Calculate cost based on jurisdiction
    fn calculate_cost(&self, rate: &NanpaRate, jurisdiction: CallJurisdiction) -> Decimal {
        match jurisdiction {
            CallJurisdiction::Interstate => rate.inter_rate,
            CallJurisdiction::Intrastate => rate.intra_rate,
            CallJurisdiction::IndeterminateJurisdiction => rate.ij_rate,
            CallJurisdiction::Local => rate.local_rate.unwrap_or(rate.intra_rate),
        }
    }

    /// Check static routes
    fn check_static_routes(
        &self,
        dnis: &str,
        ingress_trunk_id: Option<i32>,
        position: RoutePosition,
    ) -> Option<StaticRoute> {
        self.cache
            .get_static_routes()
            .iter()
            .filter(|r| r.position == position)
            .filter(|r| {
                r.ingress_trunk_id.is_none() || r.ingress_trunk_id == ingress_trunk_id
            })
            .find(|r| {
                if let Some(regex) = self.static_route_patterns.get(&r.id) {
                    regex.is_match(dnis)
                } else if let Ok(regex) = Regex::new(&r.pattern) {
                    regex.is_match(dnis)
                } else {
                    false
                }
            })
            .cloned()
    }

    /// Build response for static route
    async fn build_static_route_response(
        &self,
        static_route: StaticRoute,
        _request: &RouteRequest,
    ) -> Result<RouteResponse> {
        let egress_trunk = self
            .cache
            .get_egress_trunk(static_route.egress_trunk_id)
            .ok_or_else(|| {
                anyhow!(
                    "Egress trunk {} not found for static route",
                    static_route.egress_trunk_id
                )
            })?;

        let call_route = CallRoute {
            egress_trunk: egress_trunk.clone(),
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
            routes: vec![call_route],
            jurisdiction: CallJurisdiction::IndeterminateJurisdiction,
            lrn: None,
            total_routes: 1,
        })
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
            effective_time: None, // Use current time
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
                "NO_ROUTES_AVAILABLE".to_string()
            },
        })
    }
}

// RouteRequest and other types are imported from existing routing module