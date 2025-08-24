use anyhow::Result;
use arc_swap::ArcSwap;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;

use crate::lcr::database::DatabasePool;
use crate::lcr::types::*;

impl Default for LcrCache {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LcrCache {
    // Vendor (cost) data
    vendor_decks: ArcSwap<HashMap<i32, RateDeck>>,
    vendor_nanpa_rates: DashMap<i32, Arc<Vec<NanpaRate>>>, // deck_id -> rates

    // Client (selling) data
    client_decks: ArcSwap<HashMap<i32, RateDeck>>,
    client_nanpa_rates: DashMap<i32, Arc<Vec<NanpaRate>>>, // deck_id -> rates

    // Trunk data
    egress_trunks: ArcSwap<HashMap<i32, EgressTrunk>>,
    ingress_trunks: ArcSwap<HashMap<i32, IngressTrunk>>,

    // Routing data
    lcr_routes: ArcSwap<HashMap<i32, LcrRoute>>,
    static_routes: ArcSwap<Vec<StaticRoute>>,

    // Configuration data
    route_advance_configs: ArcSwap<HashMap<(ConfigScope, Option<i32>), RouteAdvanceConfig>>,
    timer_configs: ArcSwap<HashMap<(ConfigScope, Option<i32>), TimerConfig>>,

    // NANPA static data for jurisdiction
    nanpa_static: ArcSwap<HashMap<String, NanpaStatic>>, // "NPANXX" -> data

    // Trunk-rate associations
    trunk_rate_associations: ArcSwap<Vec<TrunkRateAssociation>>,
    lcr_route_trunks: ArcSwap<HashMap<i32, Vec<LcrRouteTrunk>>>, // route_id -> trunks

    // LRN cache
    lrn_cache: DashMap<String, LrnCacheEntry>,
}

impl LcrCache {
    pub fn new() -> Self {
        Self {
            vendor_decks: ArcSwap::new(Arc::new(HashMap::new())),
            vendor_nanpa_rates: DashMap::new(),
            client_decks: ArcSwap::new(Arc::new(HashMap::new())),
            client_nanpa_rates: DashMap::new(),
            egress_trunks: ArcSwap::new(Arc::new(HashMap::new())),
            ingress_trunks: ArcSwap::new(Arc::new(HashMap::new())),
            lcr_routes: ArcSwap::new(Arc::new(HashMap::new())),
            static_routes: ArcSwap::new(Arc::new(Vec::new())),
            route_advance_configs: ArcSwap::new(Arc::new(HashMap::new())),
            timer_configs: ArcSwap::new(Arc::new(HashMap::new())),
            nanpa_static: ArcSwap::new(Arc::new(HashMap::new())),
            trunk_rate_associations: ArcSwap::new(Arc::new(Vec::new())),
            lcr_route_trunks: ArcSwap::new(Arc::new(HashMap::new())),
            lrn_cache: DashMap::new(),
        }
    }

    pub async fn load_from_database(&self, db: &DatabasePool) -> Result<()> {
        // Load vendor data
        let vendor_decks = db.load_vendor_rate_decks().await?;
        let mut vendor_deck_map = HashMap::new();
        for deck in vendor_decks {
            let deck_id = deck.id;
            vendor_deck_map.insert(deck_id, deck);

            // Load rates for this deck
            let rates = db.load_vendor_nanpa_rates(deck_id).await?;
            self.vendor_nanpa_rates.insert(deck_id, Arc::new(rates));
        }
        self.vendor_decks.store(Arc::new(vendor_deck_map));

        // Load client data
        let client_decks = db.load_client_rate_decks().await?;
        let mut client_deck_map = HashMap::new();
        for deck in client_decks {
            let deck_id = deck.id;
            client_deck_map.insert(deck_id, deck);

            // Load rates for this deck
            let rates = db.load_client_nanpa_rates(deck_id).await?;
            self.client_nanpa_rates.insert(deck_id, Arc::new(rates));
        }
        self.client_decks.store(Arc::new(client_deck_map));

        // Load trunks
        let egress_trunks = db.load_egress_trunks().await?;
        let mut egress_trunk_map = HashMap::new();
        for trunk in egress_trunks {
            egress_trunk_map.insert(trunk.id, trunk);
        }
        self.egress_trunks.store(Arc::new(egress_trunk_map));

        let ingress_trunks = db.load_ingress_trunks().await?;
        let mut ingress_trunk_map = HashMap::new();
        for trunk in ingress_trunks {
            ingress_trunk_map.insert(trunk.id, trunk);
        }
        self.ingress_trunks.store(Arc::new(ingress_trunk_map));

        // Load routing data
        let lcr_routes = db.load_lcr_routes().await?;
        let mut lcr_route_map = HashMap::new();
        for route in lcr_routes {
            lcr_route_map.insert(route.id, route);
        }
        self.lcr_routes.store(Arc::new(lcr_route_map));

        let static_routes = db.load_static_routes().await?;
        self.static_routes.store(Arc::new(static_routes));

        // Load configurations
        let route_advance_configs = db.load_route_advance_configs().await?;
        let mut advance_config_map = HashMap::new();
        for config in route_advance_configs {
            advance_config_map.insert((config.scope, config.scope_id), config);
        }
        self.route_advance_configs
            .store(Arc::new(advance_config_map));

        let timer_configs = db.load_timer_configs().await?;
        let mut timer_config_map = HashMap::new();
        for config in timer_configs {
            timer_config_map.insert((config.scope, config.scope_id), config);
        }
        self.timer_configs.store(Arc::new(timer_config_map));

        // Load NANPA static data
        let nanpa_entries = db.load_nanpa_static().await?;
        let mut nanpa_map = HashMap::new();
        for entry in nanpa_entries {
            let key = if let Some(nxx) = &entry.nxx {
                format!("{}{}", entry.npa, nxx)
            } else {
                entry.npa.clone()
            };
            nanpa_map.insert(key, entry);
        }
        self.nanpa_static.store(Arc::new(nanpa_map));

        // Load trunk-rate associations
        let associations = db.load_trunk_rate_associations().await?;
        self.trunk_rate_associations.store(Arc::new(associations));

        // Load LCR route-trunk associations
        let route_trunks = db.load_lcr_route_trunks().await?;
        let mut route_trunk_map = HashMap::new();
        for rt in route_trunks {
            route_trunk_map
                .entry(rt.lcr_route_id)
                .or_insert_with(Vec::new)
                .push(rt);
        }
        self.lcr_route_trunks.store(Arc::new(route_trunk_map));

        Ok(())
    }

    pub fn get_vendor_rate(&self, deck_id: i32, code: &str) -> Option<NanpaRate> {
        let rates = self.vendor_nanpa_rates.get(&deck_id)?;

        // LCR Longest Match: Try progressively shorter prefixes starting from the full code
        // Example: For 1702777, try 1702777, then 170277, then 17027, then 1702, then 170, then 17, then 1
        for prefix_len in (1..=code.len()).rev() {
            let prefix = &code[0..prefix_len];

            // Look for exact match of this prefix length
            if let Some(rate) = rates.iter().find(|r| r.code == prefix) {
                return Some(rate.clone());
            }
        }

        None
    }

    pub fn get_client_rate(&self, deck_id: i32, code: &str) -> Option<NanpaRate> {
        let rates = self.client_nanpa_rates.get(&deck_id)?;

        // LCR Longest Match: Try progressively shorter prefixes starting from the full code
        // Example: For 1702777, try 1702777, then 170277, then 17027, then 1702, then 170, then 17, then 1
        for prefix_len in (1..=code.len()).rev() {
            let prefix = &code[0..prefix_len];

            // Look for exact match of this prefix length
            if let Some(rate) = rates.iter().find(|r| r.code == prefix) {
                return Some(rate.clone());
            }
        }

        None
    }

    pub fn get_egress_trunk(&self, trunk_id: i32) -> Option<EgressTrunk> {
        self.egress_trunks.load().get(&trunk_id).cloned()
    }

    pub fn get_ingress_trunk(&self, trunk_id: i32) -> Option<IngressTrunk> {
        self.ingress_trunks.load().get(&trunk_id).cloned()
    }

    pub fn get_all_egress_trunks(&self) -> Vec<EgressTrunk> {
        self.egress_trunks.load().values().cloned().collect()
    }

    pub fn get_all_ingress_trunks(&self) -> Vec<IngressTrunk> {
        self.ingress_trunks.load().values().cloned().collect()
    }

    pub fn get_static_routes(&self) -> Arc<Vec<StaticRoute>> {
        Arc::clone(&self.static_routes.load())
    }

    pub fn get_static_routes_for_ingress(&self, ingress_trunk_id: Option<i32>) -> Vec<StaticRoute> {
        self.static_routes
            .load()
            .iter()
            .filter(|r| r.ingress_trunk_id == ingress_trunk_id || r.ingress_trunk_id.is_none())
            .cloned()
            .collect()
    }

    pub fn get_route_advance_config(
        &self,
        scope: ConfigScope,
        scope_id: Option<i32>,
    ) -> RouteAdvanceConfig {
        let configs = self.route_advance_configs.load();

        // Try specific config first
        if let Some(config) = configs.get(&(scope, scope_id)) {
            return config.clone();
        }

        // Fall back to global config
        if let Some(config) = configs.get(&(ConfigScope::Global, None)) {
            return config.clone();
        }

        // Return default if no config found
        RouteAdvanceConfig {
            id: 0,
            scope: ConfigScope::Global,
            scope_id: None,
            advance_on_codes: vec![
                "503".to_string(),
                "504".to_string(),
                "603".to_string(),
                "606".to_string(),
                "480".to_string(),
                "487".to_string(),
                "502".to_string(),
                "500".to_string(),
            ],
            stop_on_codes: vec![
                "404".to_string(),
                "486".to_string(),
                "600".to_string(),
                "604".to_string(),
                "403".to_string(),
                "401".to_string(),
                "402".to_string(),
            ],
        }
    }

    pub fn get_timer_config(&self, scope: ConfigScope, scope_id: Option<i32>) -> TimerConfig {
        let configs = self.timer_configs.load();

        // Try specific config first
        if let Some(config) = configs.get(&(scope, scope_id)) {
            return config.clone();
        }

        // Fall back to global config
        if let Some(config) = configs.get(&(ConfigScope::Global, None)) {
            return config.clone();
        }

        // Return default if no config found
        TimerConfig {
            id: 0,
            scope: ConfigScope::Global,
            scope_id: None,
            timer_100_to_183_ms: 30000,
            timer_max_call_duration_sec: 10800,
            timer_post_dial_delay_ms: 5000,
            timer_ringing_timeout_sec: 120,
            timer_transaction_timeout_ms: 32000,
        }
    }

    pub fn get_nanpa_info(&self, number: &str) -> Option<NanpaStatic> {
        let nanpa_data = self.nanpa_static.load();

        // Try NPANXX first (first 6 digits)
        if number.len() >= 6 {
            let npanxx = &number[0..6];
            if let Some(entry) = nanpa_data.get(npanxx) {
                return Some(entry.clone());
            }
        }

        // Try NPA only (first 3 digits)
        if number.len() >= 3 {
            let npa = &number[0..3];
            if let Some(entry) = nanpa_data.get(npa) {
                return Some(entry.clone());
            }
        }

        None
    }

    pub fn get_lrn_cached(&self, tn: &str) -> Option<LrnCacheEntry> {
        self.lrn_cache.get(tn).map(|e| e.clone())
    }

    pub fn update_lrn_cache(&self, entry: LrnCacheEntry) {
        self.lrn_cache.insert(entry.tn.clone(), entry);
    }

    /// Get vendor rate deck IDs associated with an egress trunk
    pub fn get_vendor_decks_for_trunk(&self, egress_trunk_id: i32) -> Vec<i32> {
        self.trunk_rate_associations
            .load()
            .iter()
            .filter(|assoc| assoc.egress_trunk_id == Some(egress_trunk_id))
            .filter_map(|assoc| assoc.vendor_deck_id)
            .collect()
    }

    /// Get client rate deck IDs associated with an ingress trunk
    pub fn get_client_decks_for_trunk(&self, ingress_trunk_id: i32) -> Vec<i32> {
        self.trunk_rate_associations
            .load()
            .iter()
            .filter(|assoc| assoc.ingress_trunk_id == Some(ingress_trunk_id))
            .filter_map(|assoc| assoc.client_deck_id)
            .collect()
    }

    /// Get egress trunks for an LCR route with their associated vendor decks
    pub fn get_route_trunks(&self, lcr_route_id: i32) -> Vec<LcrRouteTrunk> {
        self.lcr_route_trunks
            .load()
            .get(&lcr_route_id)
            .cloned()
            .unwrap_or_default()
    }
}
