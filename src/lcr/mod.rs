pub mod api;
pub mod api_deck;
pub mod cache;
pub mod cli_commands;
pub mod cluster;
pub mod data_loader;
pub mod database;
#[cfg(test)]
pub mod database_integration_tests;
pub mod deck_loader;
#[cfg(test)]
pub mod end_to_end_tests;
#[cfg(test)]
pub mod integration_test;
pub mod jurisdiction;
pub mod lrn_dip;
pub mod nanpa_loader;
pub mod phone_validation;
#[cfg(test)]
pub mod phone_validation_tests;
pub mod routing;
#[cfg(test)]
pub mod routing_integration_tests;
#[cfg(test)]
pub mod test_local_rates;
pub mod timers;
pub mod trunk_manager;
pub mod types;

use anyhow::Result;
use std::sync::Arc;

use crate::lcr::cache::LcrCache;
use crate::lcr::database::DatabasePool;
use crate::lcr::deck_loader::DeckLoader;
use crate::lcr::lrn_dip::LrnDipService;
use crate::lcr::routing::RoutingEngine;
use crate::lcr::timers::TimerManager;
use crate::lcr::trunk_manager::TrunkManager;
use crate::lcr::types::LrnDipConfig;

pub struct LcrEngine {
    db_pool: Arc<DatabasePool>,
    pub cache: Arc<LcrCache>,
    routing_engine: Arc<RoutingEngine>,
    deck_loader: Arc<DeckLoader>,
    trunk_manager: Arc<TrunkManager>,
    timer_manager: Arc<TimerManager>,
    lrn_dip_service: Arc<LrnDipService>,
}

impl LcrEngine {
    pub async fn new(database_url: &str) -> Result<Self> {
        Self::new_with_lrn_config(database_url, LrnDipConfig::default()).await
    }

    /// Build an engine backed by a lazily-connected pool and empty caches,
    /// without performing any database I/O. Intended for unit tests that only
    /// exercise in-memory routing logic. LRN dip is disabled.
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        let db_pool = Arc::new(DatabasePool::new_lazy_for_test());
        let pool = db_pool.pool.clone();
        let cache = Arc::new(LcrCache::new());
        let trunk_manager = Arc::new(TrunkManager::new());
        let timer_manager = Arc::new(TimerManager::new());

        let lrn_dip_service = Arc::new(LrnDipService::new(LrnDipConfig::default()));

        let routing_engine = Arc::new(RoutingEngine::new(
            cache.clone(),
            trunk_manager.clone(),
            timer_manager.clone(),
            pool.clone(),
        ));

        let deck_loader = Arc::new(DeckLoader::with_cache_and_db(
            pool,
            cache.clone(),
            db_pool.clone(),
        ));

        Self {
            db_pool,
            cache,
            routing_engine,
            deck_loader,
            trunk_manager,
            timer_manager,
            lrn_dip_service,
        }
    }

    pub async fn new_with_lrn_config(database_url: &str, lrn_config: LrnDipConfig) -> Result<Self> {
        let db_pool = Arc::new(DatabasePool::new(database_url).await?);
        let pool = db_pool.pool.clone(); // Get the underlying PgPool
        let cache = Arc::new(LcrCache::new());
        let trunk_manager = Arc::new(TrunkManager::new());
        let timer_manager = Arc::new(TimerManager::new());

        // Initialize LRN dip service
        let lrn_dip_service = Arc::new(LrnDipService::new(lrn_config));
        if lrn_dip_service.is_enabled() {
            lrn_dip_service.initialize().await?;
        }

        // Ensure default international routing plans exist
        db_pool.ensure_default_routing_plans().await?;

        // Load initial data into cache
        cache.load_from_database(&db_pool).await?;

        let routing_engine = if lrn_dip_service.is_enabled() {
            Arc::new(RoutingEngine::with_lrn_dip(
                cache.clone(),
                trunk_manager.clone(),
                timer_manager.clone(),
                pool.clone(),
                lrn_dip_service.clone(),
            ))
        } else {
            Arc::new(RoutingEngine::new(
                cache.clone(),
                trunk_manager.clone(),
                timer_manager.clone(),
                pool.clone(),
            ))
        };

        let deck_loader = Arc::new(DeckLoader::with_cache_and_db(
            pool,
            cache.clone(),
            db_pool.clone(),
        ));

        Ok(Self {
            db_pool,
            cache,
            routing_engine,
            deck_loader,
            trunk_manager,
            timer_manager,
            lrn_dip_service,
        })
    }

    pub async fn reload_cache(&self) -> Result<()> {
        self.cache.load_from_database(&self.db_pool).await
    }

    pub fn get_routing_engine(&self) -> Arc<RoutingEngine> {
        self.routing_engine.clone()
    }

    pub fn get_deck_loader(&self) -> Arc<DeckLoader> {
        self.deck_loader.clone()
    }

    pub fn get_trunk_manager(&self) -> Arc<TrunkManager> {
        self.trunk_manager.clone()
    }

    pub fn get_timer_manager(&self) -> Arc<TimerManager> {
        self.timer_manager.clone()
    }

    pub fn get_lrn_dip_service(&self) -> Arc<LrnDipService> {
        self.lrn_dip_service.clone()
    }
}
