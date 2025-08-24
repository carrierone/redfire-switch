pub mod api;
pub mod api_deck;
pub mod cache;
pub mod cli_commands;
pub mod cluster;
pub mod data_loader;
pub mod database;
pub mod deck_loader;
pub mod jurisdiction;
pub mod nanpa_loader;
pub mod routing;
pub mod timers;
pub mod trunk_manager;
pub mod types;
#[cfg(test)]
pub mod test_local_rates;
#[cfg(test)]
pub mod integration_test;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::lcr::cache::LcrCache;
use crate::lcr::database::DatabasePool;
use crate::lcr::deck_loader::DeckLoader;
use crate::lcr::routing::RoutingEngine;
use crate::lcr::timers::TimerManager;
use crate::lcr::trunk_manager::TrunkManager;

pub struct LcrEngine {
    db_pool: Arc<DatabasePool>,
    pub cache: Arc<LcrCache>,
    routing_engine: Arc<RoutingEngine>,
    deck_loader: Arc<DeckLoader>,
    trunk_manager: Arc<TrunkManager>,
    timer_manager: Arc<TimerManager>,
}

impl LcrEngine {
    pub async fn new(database_url: &str) -> Result<Self> {
        let db_pool = Arc::new(DatabasePool::new(database_url).await?);
        let pool = db_pool.pool.clone(); // Get the underlying PgPool
        let cache = Arc::new(LcrCache::new());
        let trunk_manager = Arc::new(TrunkManager::new());
        let timer_manager = Arc::new(TimerManager::new());

        // Load initial data into cache
        cache.load_from_database(&db_pool).await?;

        let routing_engine = Arc::new(RoutingEngine::new(
            cache.clone(),
            trunk_manager.clone(),
            timer_manager.clone(),
            pool.clone(),
        ));

        let deck_loader = Arc::new(DeckLoader::with_cache_and_db(pool, cache.clone(), db_pool.clone()));

        Ok(Self {
            db_pool,
            cache,
            routing_engine,
            deck_loader,
            trunk_manager,
            timer_manager,
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
}
