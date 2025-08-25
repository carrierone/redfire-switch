use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use regex::Regex;
use rust_decimal::Decimal;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

use crate::lcr::cache::LcrCache;
use crate::lcr::jurisdiction::JurisdictionCalculator;
use crate::lcr::lrn_dip::LrnDipService;
use crate::lcr::phone_validation::{PhoneValidationConfig, PhoneValidator};
use crate::lcr::timers::TimerManager;
use crate::lcr::trunk_manager::TrunkManager;
use crate::lcr::types::*;
pub use crate::lcr::types::{RouteRequest, RouteResponse};

pub struct RoutingEngine {
    cache: Arc<LcrCache>,
    trunk_manager: Arc<TrunkManager>,
    timer_manager: Arc<TimerManager>,
    pool: PgPool,
    lrn_dip_service: Option<Arc<LrnDipService>>,
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
            lrn_dip_service: None,
            static_route_patterns: HashMap::new(),
        }
    }

    pub fn with_lrn_dip(
        cache: Arc<LcrCache>,
        trunk_manager: Arc<TrunkManager>,
        timer_manager: Arc<TimerManager>,
        pool: PgPool,
        lrn_dip_service: Arc<LrnDipService>,
    ) -> Self {
        Self {
            cache,
            trunk_manager,
            timer_manager,
            pool,
            lrn_dip_service: Some(lrn_dip_service),
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

        // Determine route type automatically if not specified
        let route_type = if request.route_type == RouteType::OTHER {
            // Auto-detect based on DNIS
            if self.is_international_number(&request.dnis) {
                RouteType::AZ
            } else {
                RouteType::NANPA
            }
        } else {
            request.route_type
        };

        // Get client rate using DNIS (will be updated per trunk if LRN is needed)
        let base_rating_code = if route_type == RouteType::NANPA {
            JurisdictionCalculator::normalize_nanpa_number(&request.dnis)
        } else {
            request.dnis.to_string()
        };

        // For international calls, client rate is handled separately per trunk
        let client_rate = if route_type == RouteType::NANPA {
            if let Some(deck_id) = request.client_deck_id {
                self.get_client_rate_at_time(deck_id, &base_rating_code, effective_time)
                    .await?
            } else {
                // Try to find client rate deck associated with ingress trunk at effective time
                self.get_client_rate_for_trunk_at_time(
                    request.ingress_trunk_id,
                    &base_rating_code,
                    effective_time,
                )
                .await?
            }
        } else {
            None // International rates handled per trunk
        };

        // Build list of potential routes with time-aware rates
        let mut routes: Vec<CallRoute> = Vec::new();

        // For overall response tracking
        let mut overall_jurisdiction = CallJurisdiction::Indeterminate;
        let mut overall_lrn: Option<String> = None;

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

            // Handle routing based on route type
            match route_type {
                RouteType::NANPA => {
                    // Skip if trunk doesn't support NANPA
                    if !self.trunk_supports_nanpa(&egress_trunk) {
                        continue;
                    }

                    // Get NANPA vendor rate and check if this trunk uses LRN rating
                    let vendor_rate_info = self
                        .get_vendor_rate_for_trunk_at_time(
                            egress_trunk.id,
                            &base_rating_code,
                            effective_time,
                        )
                        .await?;

                    if let Some((rate, rate_type)) = vendor_rate_info {
                        // Determine if we need LRN for this specific trunk
                        let (jurisdiction, lrn) = if rate_type == RateType::LRN {
                            // This trunk uses LRN rating - perform LRN dip
                            JurisdictionCalculator::get_jurisdiction_with_lrn_dip(
                                &self.cache,
                                self.lrn_dip_service.clone(),
                                &request.ani,
                                &request.dnis,
                                true, // use_lrn = true
                            )
                            .await
                        } else {
                            // This trunk uses DNIS rating - no LRN needed
                            // Get ANI and DNIS NANPA info for jurisdiction calculation
                            let ani_npanxx = JurisdictionCalculator::extract_npanxx(&request.ani);
                            let dnis_npanxx = JurisdictionCalculator::extract_npanxx(&request.dnis);
                            let ani_info =
                                ani_npanxx.and_then(|npanxx| self.cache.get_nanpa_info(&npanxx));
                            let dnis_info =
                                dnis_npanxx.and_then(|npanxx| self.cache.get_nanpa_info(&npanxx));

                            let jurisdiction = JurisdictionCalculator::determine_jurisdiction(
                                ani_info.as_ref(),
                                dnis_info.as_ref(),
                            );
                            (jurisdiction, None)
                        };

                        // Update overall tracking for response
                        if overall_jurisdiction == CallJurisdiction::Indeterminate {
                            overall_jurisdiction = jurisdiction;
                        }
                        if lrn.is_some() && overall_lrn.is_none() {
                            overall_lrn = lrn.clone();
                        }

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
                RouteType::AZ => {
                    // Skip if trunk doesn't support international
                    if !egress_trunk.supports_international {
                        continue;
                    }

                    // Phone number validation for international routing
                    let mut country_code = None;
                    let mut validation_passed = true;

                    // Check if phone validation is enabled in routing plan
                    if let Some(routing_plan_id) = request.routing_plan_id {
                        if let Some(routing_plan) = self.get_routing_plan(routing_plan_id).await? {
                            if routing_plan.phone_validation_enabled {
                                // Create validator with routing plan configuration
                                let phone_config = PhoneValidationConfig {
                                    enabled: routing_plan.phone_validation_enabled,
                                    strict_validation: routing_plan.phone_validation_strict,
                                    default_region: routing_plan
                                        .phone_validation_default_region
                                        .clone(),
                                    use_country_detection: routing_plan
                                        .phone_validation_use_country_detection,
                                };

                                let validator = PhoneValidator::new(phone_config);
                                let validation_result = validator.validate(&request.dnis);

                                validation_passed = validation_result.is_valid;
                                country_code = validation_result.country_code;

                                if !validation_passed && routing_plan.phone_validation_strict {
                                    warn!(
                                        "Phone validation failed for DNIS {} on routing plan {}: {}",
                                        request.dnis,
                                        routing_plan.name,
                                        validation_result.error.unwrap_or("Unknown validation error".to_string())
                                    );
                                    continue;
                                }
                            }
                        }
                    } else if let Some(validation_config) = &request.phone_validation {
                        // Use request-level phone validation if no routing plan specified
                        if validation_config.enabled {
                            let validator = PhoneValidator::new(validation_config.clone());
                            let validation_result = validator.validate(&request.dnis);

                            validation_passed = validation_result.is_valid;
                            country_code = validation_result.country_code;

                            if !validation_passed && validation_config.strict_validation {
                                warn!(
                                    "Phone validation failed for DNIS {}: {}",
                                    request.dnis,
                                    validation_result
                                        .error
                                        .unwrap_or("Unknown validation error".to_string())
                                );
                                continue;
                            }
                        }
                    }

                    // Get international vendor rate with longest-to-shortest matching
                    if let Some(vendor_deck_id) = self
                        .get_vendor_deck_for_trunk(egress_trunk.id, effective_time)
                        .await?
                    {
                        if let Some(intl_rate) = self
                            .load_vendor_international_rate_from_db(vendor_deck_id, &request.dnis)
                            .await?
                        {
                            // Get client international rate
                            let client_intl_rate = if let Some(client_deck_id) = self
                                .get_client_deck_for_trunk(request.ingress_trunk_id, effective_time)
                                .await?
                            {
                                self.load_client_international_rate_from_db(
                                    client_deck_id,
                                    &request.dnis,
                                )
                                .await?
                            } else {
                                None
                            };

                            // Apply country-specific routing preferences if available
                            let mut adjusted_cost = intl_rate.rate;
                            let mut skip_route = false;

                            if let (Some(routing_plan_id), Some(country)) =
                                (request.routing_plan_id, &country_code)
                            {
                                if let Some(country_prefs) = self
                                    .get_country_routing_preferences(routing_plan_id, country)
                                    .await?
                                {
                                    // Apply cost multiplier
                                    adjusted_cost = intl_rate.rate * country_prefs.cost_multiplier;

                                    // Check if validation is required for this country
                                    if country_prefs.require_validation && !validation_passed {
                                        skip_route = true;
                                    }
                                }
                            }

                            if skip_route {
                                continue;
                            }

                            // Calculate costs (single rate for international)
                            let cost = adjusted_cost;
                            let sell = client_intl_rate.as_ref().map(|cr| cr.rate).unwrap_or(cost);
                            let profit = sell - cost;

                            // Set jurisdiction to indeterminate for international calls
                            overall_jurisdiction = CallJurisdiction::Indeterminate;

                            // Apply profit protection if required
                            if request.require_profit_protection || ingress_trunk.profit_protection
                            {
                                let min_margin = request
                                    .min_profit_margin
                                    .unwrap_or(ingress_trunk.min_profit_margin);
                                if profit < min_margin {
                                    continue;
                                }
                            }

                            routes.push(CallRoute {
                                egress_trunk: egress_trunk.clone(),
                                vendor_rate: None, // International rates use different structure
                                cost_per_minute: cost,
                                selling_per_minute: sell,
                                profit_margin: profit,
                                priority: egress_trunk.priority,
                                setup_fee: intl_rate.setup_fee.unwrap_or(Decimal::ZERO),
                                min_increment: intl_rate.initial_increment,
                                interval: intl_rate.subsequent_increment,
                            });
                        }
                    }
                }
                RouteType::OTHER => {
                    // Skip - OTHER should have been converted to specific type above
                    continue;
                }
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
            jurisdiction: overall_jurisdiction,
            lrn: overall_lrn,
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
            "#,
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
                "#,
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
    ) -> Result<Option<(NanpaRate, RateType)>> {
        // Get the vendor deck associated with this trunk at the given time
        let deck_row = sqlx::query(
            r#"
            SELECT vrd.id, vrd.rate_type
            FROM lcr_route_trunks lrt
            JOIN vendor_rate_decks vrd ON vrd.id = lrt.vendor_deck_id
            WHERE lrt.egress_trunk_id = $1
              AND vrd.effective_date <= $2
              AND (vrd.end_date IS NULL OR vrd.end_date > $2)
              AND vrd.active = true
            ORDER BY vrd.deck_version DESC
            LIMIT 1
            "#,
        )
        .bind(trunk_id)
        .bind(effective_time)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(deck_info) = deck_row {
            let deck_id: i32 = deck_info.get("id");
            let rate_type_str: String = deck_info.get("rate_type");
            let rate_type = match rate_type_str.as_str() {
                "LRN" => RateType::LRN,
                "DNIS" => RateType::DNIS,
                _ => RateType::DNIS,
            };

            // Check cache first
            if let Some(rate) = self.cache.get_vendor_rate(deck_id, code) {
                return Ok(Some((rate, rate_type)));
            }

            // Load from database if not in cache
            if let Some(rate) = self.load_vendor_rate_from_db(deck_id, code).await? {
                return Ok(Some((rate, rate_type)));
            }
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
            "#,
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
    async fn load_vendor_rate_from_db(
        &self,
        deck_id: i32,
        code: &str,
    ) -> Result<Option<NanpaRate>> {
        // Try exact match first
        let row = sqlx::query(
            r#"
            SELECT id, deck_id, code, inter_rate, intra_rate, ij_rate, 
                   local_rate, min_increment, interval, setup_fee
            FROM vendor_nanpa_rates
            WHERE deck_id = $1 AND code = $2
            "#,
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
                local_rate: row
                    .get::<Option<f64>, _>("local_rate")
                    .map(|f| Decimal::try_from(f).unwrap_or_default()),
                min_increment: row.get("min_increment"),
                interval: row.get("interval"),
                setup_fee: row
                    .get::<Option<f64>, _>("setup_fee")
                    .map(|f| Decimal::try_from(f).unwrap_or_default()),
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
                "#,
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
                    inter_rate: Decimal::try_from(row.get::<f64, _>("inter_rate"))
                        .unwrap_or_default(),
                    intra_rate: Decimal::try_from(row.get::<f64, _>("intra_rate"))
                        .unwrap_or_default(),
                    ij_rate: Decimal::try_from(row.get::<f64, _>("ij_rate")).unwrap_or_default(),
                    local_rate: row
                        .get::<Option<f64>, _>("local_rate")
                        .map(|f| Decimal::try_from(f).unwrap_or_default()),
                    min_increment: row.get("min_increment"),
                    interval: row.get("interval"),
                    setup_fee: row
                        .get::<Option<f64>, _>("setup_fee")
                        .map(|f| Decimal::try_from(f).unwrap_or_default()),
                }));
            }
        }

        Ok(None)
    }

    /// Load client rate from database
    async fn load_client_rate_from_db(
        &self,
        deck_id: i32,
        code: &str,
    ) -> Result<Option<NanpaRate>> {
        // Try exact match first
        let row = sqlx::query(
            r#"
            SELECT id, deck_id, code, inter_rate, intra_rate, ij_rate, 
                   local_rate, min_increment, interval, setup_fee
            FROM client_nanpa_rates
            WHERE deck_id = $1 AND code = $2
            "#,
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
                local_rate: row
                    .get::<Option<f64>, _>("local_rate")
                    .map(|f| Decimal::try_from(f).unwrap_or_default()),
                min_increment: row.get("min_increment"),
                interval: row.get("interval"),
                setup_fee: row
                    .get::<Option<f64>, _>("setup_fee")
                    .map(|f| Decimal::try_from(f).unwrap_or_default()),
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
                "#,
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
                    inter_rate: Decimal::try_from(row.get::<f64, _>("inter_rate"))
                        .unwrap_or_default(),
                    intra_rate: Decimal::try_from(row.get::<f64, _>("intra_rate"))
                        .unwrap_or_default(),
                    ij_rate: Decimal::try_from(row.get::<f64, _>("ij_rate")).unwrap_or_default(),
                    local_rate: row
                        .get::<Option<f64>, _>("local_rate")
                        .map(|f| Decimal::try_from(f).unwrap_or_default()),
                    min_increment: row.get("min_increment"),
                    interval: row.get("interval"),
                    setup_fee: row
                        .get::<Option<f64>, _>("setup_fee")
                        .map(|f| Decimal::try_from(f).unwrap_or_default()),
                }));
            }
        }

        Ok(None)
    }

    /// Calculate cost based on jurisdiction
    fn calculate_cost(&self, rate: &NanpaRate, jurisdiction: CallJurisdiction) -> Decimal {
        match jurisdiction {
            CallJurisdiction::Inter => rate.inter_rate,
            CallJurisdiction::Intra => rate.intra_rate,
            CallJurisdiction::Indeterminate => rate.ij_rate,
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
            .filter(|r| r.ingress_trunk_id.is_none() || r.ingress_trunk_id == ingress_trunk_id)
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
            jurisdiction: CallJurisdiction::Indeterminate,
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
            phone_validation: None,
            routing_plan_id: None,
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

    /// Load international vendor rate from database with longest-to-shortest prefix matching
    async fn load_vendor_international_rate_from_db(
        &self,
        deck_id: i32,
        dnis: &str,
    ) -> Result<Option<InternationalRate>> {
        // Extract the international number (remove leading +, 00, or 011)
        let normalized = self.normalize_international_number(dnis);

        // Use a single query with proper longest-to-shortest matching
        let row = sqlx::query(
            r#"
            SELECT id, deck_id, country_code, destination_code, destination_name,
                   jurisdiction, 
                   rate, initial_increment, subsequent_increment, setup_fee, created_at
            FROM vendor_international_rates
            WHERE deck_id = $1 AND $2 LIKE (country_code || COALESCE(destination_code, '') || '%')
            ORDER BY LENGTH(country_code || COALESCE(destination_code, '')) DESC
            LIMIT 1
            "#,
        )
        .bind(deck_id)
        .bind(normalized)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let jurisdiction_str: String = row.get("jurisdiction");
            let jurisdiction = match jurisdiction_str.as_str() {
                "EEA" => InternationalJurisdiction::EEA,
                "ROW" => InternationalJurisdiction::ROW,
                _ => InternationalJurisdiction::ROW,
            };

            return Ok(Some(InternationalRate {
                id: row.get("id"),
                deck_id: row.get("deck_id"),
                country_code: row.get("country_code"),
                destination_code: row.get("destination_code"),
                destination_name: row.get("destination_name"),
                jurisdiction,
                rate: row.get("rate"),
                initial_increment: row.get("initial_increment"),
                subsequent_increment: row.get("subsequent_increment"),
                setup_fee: row.get("setup_fee"),
                created_at: row.get("created_at"),
            }));
        }

        Ok(None)
    }

    /// Load international client rate from database with longest-to-shortest prefix matching
    async fn load_client_international_rate_from_db(
        &self,
        deck_id: i32,
        dnis: &str,
    ) -> Result<Option<InternationalRate>> {
        // Extract the international number (remove leading +, 00, or 011)
        let normalized = self.normalize_international_number(dnis);

        // Use a single query with proper longest-to-shortest matching
        let row = sqlx::query(
            r#"
            SELECT id, deck_id, country_code, destination_code, destination_name,
                   jurisdiction, 
                   rate, initial_increment, subsequent_increment, setup_fee, created_at
            FROM client_international_rates
            WHERE deck_id = $1 AND $2 LIKE (country_code || COALESCE(destination_code, '') || '%')
            ORDER BY LENGTH(country_code || COALESCE(destination_code, '')) DESC
            LIMIT 1
            "#,
        )
        .bind(deck_id)
        .bind(normalized)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let jurisdiction_str: String = row.get("jurisdiction");
            let jurisdiction = match jurisdiction_str.as_str() {
                "EEA" => InternationalJurisdiction::EEA,
                "ROW" => InternationalJurisdiction::ROW,
                _ => InternationalJurisdiction::ROW,
            };

            return Ok(Some(InternationalRate {
                id: row.get("id"),
                deck_id: row.get("deck_id"),
                country_code: row.get("country_code"),
                destination_code: row.get("destination_code"),
                destination_name: row.get("destination_name"),
                jurisdiction,
                rate: row.get("rate"),
                initial_increment: row.get("initial_increment"),
                subsequent_increment: row.get("subsequent_increment"),
                setup_fee: row.get("setup_fee"),
                created_at: row.get("created_at"),
            }));
        }

        Ok(None)
    }

    /// Normalize international number by removing common prefixes
    fn normalize_international_number(&self, number: &str) -> String {
        let digits: String = number.chars().filter(|c| c.is_digit(10)).collect();

        // Remove international access codes
        if digits.starts_with("011") && digits.len() > 3 {
            // US international prefix
            digits[3..].to_string()
        } else if digits.starts_with("00") && digits.len() > 2 {
            // International prefix (most countries)
            digits[2..].to_string()
        } else if number.starts_with('+') {
            // Plus format
            digits
        } else {
            // Assume already normalized
            digits
        }
    }

    /// Check if a number is international (not NANPA)
    fn is_international_number(&self, dnis: &str) -> bool {
        let normalized = dnis.chars().filter(|c| c.is_digit(10)).collect::<String>();

        // International access codes
        if normalized.starts_with("011") || normalized.starts_with("00") || dnis.starts_with('+') {
            return true;
        }

        // Valid NANPA number formats:
        // - 10 digits: NPANXXNNNN (e.g., 2125551234)
        // - 11 digits starting with 1: 1NPANXXNNNN (e.g., 12125551234)
        if normalized.len() == 10 {
            // Check if it's a valid NANPA NPA (first digit 2-9)
            if let Some(first_digit) = normalized.chars().next() {
                return first_digit < '2' || first_digit > '9';
            }
        } else if normalized.len() == 11 && normalized.starts_with('1') {
            // Check if NPA is valid (second digit 2-9)
            if let Some(second_digit) = normalized.chars().nth(1) {
                return second_digit < '2' || second_digit > '9';
            }
        } else {
            // Invalid NANPA length - likely international
            return true;
        }

        false
    }

    /// Check if trunk supports NANPA routing (not international-only)
    fn trunk_supports_nanpa(&self, trunk: &EgressTrunk) -> bool {
        // If trunk supports international, it can still support NANPA unless explicitly configured otherwise
        // For now, assume all trunks support NANPA unless they are international-only
        true // This could be extended with a separate field in the future
    }

    /// Get vendor deck ID for trunk at specific time
    async fn get_vendor_deck_for_trunk(
        &self,
        trunk_id: i32,
        effective_time: DateTime<Utc>,
    ) -> Result<Option<i32>> {
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
            "#,
        )
        .bind(trunk_id)
        .bind(effective_time)
        .fetch_optional(&self.pool)
        .await?;

        Ok(deck_id.flatten())
    }

    /// Get client deck ID for trunk at specific time
    async fn get_client_deck_for_trunk(
        &self,
        trunk_id: i32,
        effective_time: DateTime<Utc>,
    ) -> Result<Option<i32>> {
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
            "#,
        )
        .bind(trunk_id)
        .bind(effective_time)
        .fetch_optional(&self.pool)
        .await?;

        Ok(deck_id.flatten())
    }

    /// Get international routing plan configuration
    async fn get_routing_plan(
        &self,
        routing_plan_id: i32,
    ) -> Result<Option<InternationalRoutingPlan>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, description,
                   phone_validation_enabled, phone_validation_strict, 
                   phone_validation_default_region, phone_validation_use_country_detection,
                   eea_routing_enabled, eea_priority_routing, eea_reduced_rates, eea_rate_reduction,
                   default_jurisdiction,
                   allow_unknown_destinations, max_rate_unknown_destinations,
                   require_strict_validation_unknown, active, created_at, updated_at
            FROM international_routing_plans
            WHERE id = $1 AND active = true
            "#,
        )
        .bind(routing_plan_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let jurisdiction_str: String = row.get("default_jurisdiction");
            let jurisdiction = match jurisdiction_str.as_str() {
                "EEA" => InternationalJurisdiction::EEA,
                "ROW" => InternationalJurisdiction::ROW,
                _ => InternationalJurisdiction::ROW,
            };

            Ok(Some(InternationalRoutingPlan {
                id: row.get("id"),
                name: row.get("name"),
                description: row.get("description"),
                phone_validation_enabled: row.get("phone_validation_enabled"),
                phone_validation_strict: row.get("phone_validation_strict"),
                phone_validation_default_region: row.get("phone_validation_default_region"),
                phone_validation_use_country_detection: row
                    .get("phone_validation_use_country_detection"),
                eea_routing_enabled: row.get("eea_routing_enabled"),
                eea_priority_routing: row.get("eea_priority_routing"),
                eea_reduced_rates: row.get("eea_reduced_rates"),
                eea_rate_reduction: row.get("eea_rate_reduction"),
                default_jurisdiction: jurisdiction,
                allow_unknown_destinations: row.get("allow_unknown_destinations"),
                max_rate_unknown_destinations: row.get("max_rate_unknown_destinations"),
                require_strict_validation_unknown: row.get("require_strict_validation_unknown"),
                active: row.get("active"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            }))
        } else {
            Ok(None)
        }
    }

    /// Get country routing preferences for a routing plan
    async fn get_country_routing_preferences(
        &self,
        routing_plan_id: i32,
        country_code: &str,
    ) -> Result<Option<CountryRoutingPreference>> {
        let row = sqlx::query(
            r#"
            SELECT id, routing_plan_id, country_code, country_name,
                   jurisdiction,
                   quality_score, cost_multiplier, require_validation,
                   max_duration_minutes, created_at
            FROM country_routing_preferences
            WHERE routing_plan_id = $1 AND country_code = $2
            "#,
        )
        .bind(routing_plan_id)
        .bind(country_code)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let jurisdiction_str: String = row.get("jurisdiction");
            let jurisdiction = match jurisdiction_str.as_str() {
                "EEA" => InternationalJurisdiction::EEA,
                "ROW" => InternationalJurisdiction::ROW,
                _ => InternationalJurisdiction::ROW,
            };

            Ok(Some(CountryRoutingPreference {
                id: row.get("id"),
                routing_plan_id: row.get("routing_plan_id"),
                country_code: row.get("country_code"),
                country_name: row.get("country_name"),
                jurisdiction,
                quality_score: row.get("quality_score"),
                cost_multiplier: row.get("cost_multiplier"),
                require_validation: row.get("require_validation"),
                max_duration_minutes: row.get("max_duration_minutes"),
                created_at: row.get("created_at"),
            }))
        } else {
            Ok(None)
        }
    }
}

// RouteRequest and other types are imported from existing routing module
