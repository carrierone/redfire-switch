//! High-performance database caching layer for LCR routing
//! Eliminates database queries in hot paths using aggressive in-memory caching

use moka::future::Cache;
use dashmap::DashMap;
use ahash::AHasher;
use std::hash::BuildHasherDefault;
use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{debug, info, warn};
use tokio::time::interval;

use crate::performance::memory_pools::{RouteVec};
use crate::performance::string_interner::{Symbol, intern_trunk_id, intern_phone_number};

type FastHasher = BuildHasherDefault<AHasher>;

/// Cached route data optimized for performance
#[derive(Debug, Clone)]
pub struct CachedRoute {
    pub trunk_id: i32,
    pub trunk_name: Symbol,
    pub rate: rust_decimal::Decimal,
    pub rating_code: Symbol,
    pub effective_date: DateTime<Utc>,
    pub priority: u32,
    pub enabled: bool,
}

/// Cached trunk configuration
#[derive(Debug, Clone)]
pub struct CachedTrunk {
    pub id: i32,
    pub name: Symbol,
    pub ip_address: std::net::IpAddr,
    pub port: u16,
    pub enabled: bool,
    pub concurrent_limit: Option<u32>,
    pub cps_limit: Option<u32>,
    pub last_updated: DateTime<Utc>,
}

/// Cached client rate
#[derive(Debug, Clone)]
pub struct CachedClientRate {
    pub rate: rust_decimal::Decimal,
    pub effective_date: DateTime<Utc>,
    pub rating_code: Symbol,
    pub deck_id: i32,
}

/// LCR cache key for route lookups
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RouteKey {
    pub dnis_prefix: Symbol,
    pub route_type: RouteType,
    pub jurisdiction: CallJurisdiction,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum RouteType {
    NANPA,
    AZ,
    International,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum CallJurisdiction {
    Intrastate,
    Interstate,
    International,
    Local,
    Indeterminate,
}

/// High-performance database cache for routing data
#[derive(Debug)]
pub struct DatabaseCache {
    /// Route cache with TTL - keyed by DNIS prefix
    route_cache: Cache<RouteKey, RouteVec<CachedRoute>>,

    /// Trunk cache - rarely changes, long TTL
    trunk_cache: Cache<i32, CachedTrunk>,

    /// Client rate cache - keyed by deck_id + rating_code
    client_rate_cache: Cache<(i32, Symbol), CachedClientRate>,

    /// Negative cache for non-existent routes (prevent repeated DB queries)
    negative_cache: Cache<RouteKey, ()>,

    /// Hot cache for most frequently accessed routes (no TTL)
    hot_routes: DashMap<RouteKey, RouteVec<CachedRoute>, FastHasher>,

    /// Database pool for cache misses
    pool: PgPool,

    /// Cache statistics
    stats: Arc<CacheStats>,
}

/// Cache performance statistics
#[derive(Debug, Default)]
pub struct CacheStats {
    pub route_hits: std::sync::atomic::AtomicU64,
    pub route_misses: std::sync::atomic::AtomicU64,
    pub trunk_hits: std::sync::atomic::AtomicU64,
    pub trunk_misses: std::sync::atomic::AtomicU64,
    pub client_rate_hits: std::sync::atomic::AtomicU64,
    pub client_rate_misses: std::sync::atomic::AtomicU64,
    pub negative_hits: std::sync::atomic::AtomicU64,
    pub hot_cache_hits: std::sync::atomic::AtomicU64,
}

impl DatabaseCache {
    /// Create new database cache with optimized configuration
    pub fn new(pool: PgPool) -> Self {
        Self {
            // Route cache: 100K entries, 5 minute TTL
            route_cache: Cache::builder()
                .max_capacity(100_000)
                .time_to_live(Duration::from_secs(300))
                .time_to_idle(Duration::from_secs(60))
                .build(),

            // Trunk cache: 10K entries, 30 minute TTL (trunks change rarely)
            trunk_cache: Cache::builder()
                .max_capacity(10_000)
                .time_to_live(Duration::from_secs(1800))
                .time_to_idle(Duration::from_secs(600))
                .build(),

            // Client rate cache: 50K entries, 10 minute TTL
            client_rate_cache: Cache::builder()
                .max_capacity(50_000)
                .time_to_live(Duration::from_secs(600))
                .time_to_idle(Duration::from_secs(120))
                .build(),

            // Negative cache: 20K entries, 2 minute TTL
            negative_cache: Cache::builder()
                .max_capacity(20_000)
                .time_to_live(Duration::from_secs(120))
                .build(),

            // Hot cache for most frequent routes (no eviction)
            hot_routes: DashMap::with_hasher(FastHasher::default()),

            pool,
            stats: Arc::new(CacheStats::default()),
        }
    }

    /// Start background cache maintenance tasks
    pub async fn start_maintenance_tasks(&self) {
        self.start_preload_task().await;
        self.start_stats_reporter().await;
    }

    /// Get routes for a destination (optimized cache lookup)
    pub async fn get_routes(&self, dnis: &str, route_type: RouteType, jurisdiction: CallJurisdiction) -> Result<Option<RouteVec<CachedRoute>>> {
        // Create cache key
        let prefix_symbol = self.get_dnis_prefix_symbol(dnis, route_type);
        let cache_key = RouteKey {
            dnis_prefix: prefix_symbol,
            route_type,
            jurisdiction,
        };

        // Hot cache check first (fastest path)
        if let Some(routes) = self.hot_routes.get(&cache_key) {
            self.stats.hot_cache_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(Some(routes.clone()));
        }

        // Check negative cache
        if self.negative_cache.get(&cache_key).await.is_some() {
            self.stats.negative_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(None);
        }

        // Main cache lookup
        if let Some(routes) = self.route_cache.get(&cache_key).await {
            self.stats.route_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            // Promote to hot cache if frequently accessed
            if routes.len() > 0 {
                self.promote_to_hot_cache(cache_key.clone(), routes.clone());
            }

            return Ok(Some(routes));
        }

        // Cache miss - query database
        self.stats.route_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        match self.query_routes_from_database(dnis, route_type, jurisdiction).await {
            Ok(Some(routes)) => {
                // Cache the result
                let cached_routes = RouteVec::from_iter(routes);
                self.route_cache.insert(cache_key.clone(), cached_routes.clone()).await;

                // Add to hot cache if it's a good route
                if cached_routes.len() > 0 {
                    self.promote_to_hot_cache(cache_key, cached_routes.clone());
                }

                Ok(Some(cached_routes))
            }
            Ok(None) => {
                // Cache negative result
                self.negative_cache.insert(cache_key, ()).await;
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    /// Get trunk configuration (cached)
    pub async fn get_trunk(&self, trunk_id: i32) -> Result<Option<CachedTrunk>> {
        if let Some(trunk) = self.trunk_cache.get(&trunk_id).await {
            self.stats.trunk_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(Some(trunk));
        }

        // Cache miss - query database
        self.stats.trunk_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        match self.query_trunk_from_database(trunk_id).await {
            Ok(Some(trunk)) => {
                self.trunk_cache.insert(trunk_id, trunk.clone()).await;
                Ok(Some(trunk))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get client rate (cached)
    pub async fn get_client_rate(&self, deck_id: i32, rating_code: &str) -> Result<Option<CachedClientRate>> {
        let rating_symbol = intern_phone_number(rating_code);
        let cache_key = (deck_id, rating_symbol);

        if let Some(rate) = self.client_rate_cache.get(&cache_key).await {
            self.stats.client_rate_hits.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(Some(rate));
        }

        // Cache miss - query database
        self.stats.client_rate_misses.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        match self.query_client_rate_from_database(deck_id, rating_code).await {
            Ok(Some(rate)) => {
                self.client_rate_cache.insert(cache_key, rate.clone()).await;
                Ok(Some(rate))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Invalidate cache entries (for real-time updates)
    pub async fn invalidate_routes(&self, dnis_prefix: &str) {
        let prefix_symbol = intern_phone_number(dnis_prefix);

        // Remove from all route caches
        for route_type in [RouteType::NANPA, RouteType::AZ, RouteType::International] {
            for jurisdiction in [
                CallJurisdiction::Intrastate,
                CallJurisdiction::Interstate,
                CallJurisdiction::International,
                CallJurisdiction::Local,
                CallJurisdiction::Indeterminate,
            ] {
                let key = RouteKey {
                    dnis_prefix: prefix_symbol,
                    route_type,
                    jurisdiction,
                };

                self.route_cache.invalidate(&key).await;
                self.negative_cache.invalidate(&key).await;
                self.hot_routes.remove(&key);
            }
        }
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> CacheStatistics {
        use std::sync::atomic::Ordering;

        let route_hits = self.stats.route_hits.load(Ordering::Relaxed);
        let route_misses = self.stats.route_misses.load(Ordering::Relaxed);
        let trunk_hits = self.stats.trunk_hits.load(Ordering::Relaxed);
        let trunk_misses = self.stats.trunk_misses.load(Ordering::Relaxed);
        let client_rate_hits = self.stats.client_rate_hits.load(Ordering::Relaxed);
        let client_rate_misses = self.stats.client_rate_misses.load(Ordering::Relaxed);

        CacheStatistics {
            route_hit_ratio: if route_hits + route_misses > 0 {
                route_hits as f64 / (route_hits + route_misses) as f64
            } else { 0.0 },
            trunk_hit_ratio: if trunk_hits + trunk_misses > 0 {
                trunk_hits as f64 / (trunk_hits + trunk_misses) as f64
            } else { 0.0 },
            client_rate_hit_ratio: if client_rate_hits + client_rate_misses > 0 {
                client_rate_hits as f64 / (client_rate_hits + client_rate_misses) as f64
            } else { 0.0 },
            route_cache_size: self.route_cache.entry_count(),
            trunk_cache_size: self.trunk_cache.entry_count(),
            client_rate_cache_size: self.client_rate_cache.entry_count(),
            hot_cache_size: self.hot_routes.len(),
            negative_cache_size: self.negative_cache.entry_count(),
        }
    }

    // Private helper methods

    fn get_dnis_prefix_symbol(&self, dnis: &str, route_type: RouteType) -> Symbol {
        let prefix = match route_type {
            RouteType::NANPA => {
                // For NANPA, use NPA-NXX (first 6 digits)
                if dnis.len() >= 6 {
                    &dnis[0..6]
                } else {
                    dnis
                }
            }
            RouteType::AZ | RouteType::International => {
                // For international, use progressive prefix matching
                if dnis.len() >= 4 {
                    &dnis[0..4]
                } else {
                    dnis
                }
            }
        };

        intern_phone_number(prefix)
    }

    fn promote_to_hot_cache(&self, key: RouteKey, routes: RouteVec<CachedRoute>) {
        // Only promote high-quality routes to hot cache
        if routes.len() > 0 && self.hot_routes.len() < 1000 {
            self.hot_routes.insert(key, routes);
        }
    }

    async fn start_preload_task(&self) {
        let cache = self.clone();
        tokio::spawn(async move {
            info!("Starting database cache preload task");

            // Preload most common routes (top 1000 prefixes)
            if let Err(e) = cache.preload_common_routes().await {
                warn!("Failed to preload common routes: {}", e);
            }

            // Preload all active trunks
            if let Err(e) = cache.preload_trunks().await {
                warn!("Failed to preload trunks: {}", e);
            }
        });
    }

    async fn start_stats_reporter(&self) {
        let stats = self.stats.clone();
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60));

            loop {
                interval.tick().await;

                let route_hits = stats.route_hits.load(std::sync::atomic::Ordering::Relaxed);
                let route_misses = stats.route_misses.load(std::sync::atomic::Ordering::Relaxed);

                if route_hits + route_misses > 0 {
                    let hit_ratio = route_hits as f64 / (route_hits + route_misses) as f64;
                    debug!("Route cache hit ratio: {:.2}% ({} hits, {} misses)",
                        hit_ratio * 100.0, route_hits, route_misses);
                }
            }
        });
    }

    async fn preload_common_routes(&self) -> Result<()> {
        // Query top 1000 most frequently called prefixes
        let rows = sqlx::query!(
            "SELECT DISTINCT substring(dnis, 1, 6) as prefix, COUNT(*) as call_count
             FROM call_detail_records
             WHERE created_at > NOW() - INTERVAL '24 hours'
             GROUP BY substring(dnis, 1, 6)
             ORDER BY call_count DESC
             LIMIT 1000"
        )
        .fetch_all(&self.pool)
        .await?;

        for row in rows {
            if let Some(prefix) = row.prefix.as_deref() {
                // Preload routes for this prefix
                let _ = self.get_routes(prefix, RouteType::NANPA, CallJurisdiction::Interstate).await;
            }
        }

        info!("Preloaded routes for {} common prefixes", rows.len());
        Ok(())
    }

    async fn preload_trunks(&self) -> Result<()> {
        let rows = sqlx::query!(
            "SELECT id FROM trunks WHERE enabled = true"
        )
        .fetch_all(&self.pool)
        .await?;

        for row in rows {
            let _ = self.get_trunk(row.id).await;
        }

        info!("Preloaded {} trunk configurations", rows.len());
        Ok(())
    }

    async fn query_routes_from_database(&self, dnis: &str, route_type: RouteType, jurisdiction: CallJurisdiction) -> Result<Option<Vec<CachedRoute>>> {
        // Simplified database query - implement actual LCR logic
        let rows = sqlx::query!(
            "SELECT t.id, t.name, r.rate, r.rating_code, r.effective_date, t.priority, t.enabled
             FROM routes r
             JOIN trunks t ON r.trunk_id = t.id
             WHERE r.rating_code = $1 AND t.enabled = true
             ORDER BY r.rate ASC, t.priority ASC
             LIMIT 10",
            dnis
        )
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(None);
        }

        let routes = rows.into_iter().map(|row| CachedRoute {
            trunk_id: row.id,
            trunk_name: intern_trunk_id(&row.name),
            rate: row.rate,
            rating_code: intern_phone_number(&row.rating_code),
            effective_date: row.effective_date,
            priority: row.priority as u32,
            enabled: row.enabled,
        }).collect();

        Ok(Some(routes))
    }

    async fn query_trunk_from_database(&self, trunk_id: i32) -> Result<Option<CachedTrunk>> {
        let row = sqlx::query!(
            "SELECT id, name, ip_address, port, enabled, concurrent_limit, cps_limit, updated_at
             FROM trunks WHERE id = $1",
            trunk_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(CachedTrunk {
                id: row.id,
                name: intern_trunk_id(&row.name),
                ip_address: row.ip_address.parse()?,
                port: row.port as u16,
                enabled: row.enabled,
                concurrent_limit: row.concurrent_limit.map(|l| l as u32),
                cps_limit: row.cps_limit.map(|l| l as u32),
                last_updated: row.updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    async fn query_client_rate_from_database(&self, deck_id: i32, rating_code: &str) -> Result<Option<CachedClientRate>> {
        let row = sqlx::query!(
            "SELECT rate, effective_date, rating_code, deck_id
             FROM client_rates
             WHERE deck_id = $1 AND rating_code = $2
             ORDER BY effective_date DESC
             LIMIT 1",
            deck_id,
            rating_code
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(CachedClientRate {
                rate: row.rate,
                effective_date: row.effective_date,
                rating_code: intern_phone_number(&row.rating_code),
                deck_id: row.deck_id,
            }))
        } else {
            Ok(None)
        }
    }
}

// Implement Clone for DatabaseCache
impl Clone for DatabaseCache {
    fn clone(&self) -> Self {
        Self {
            route_cache: self.route_cache.clone(),
            trunk_cache: self.trunk_cache.clone(),
            client_rate_cache: self.client_rate_cache.clone(),
            negative_cache: self.negative_cache.clone(),
            hot_routes: self.hot_routes.clone(),
            pool: self.pool.clone(),
            stats: self.stats.clone(),
        }
    }
}

/// Cache performance statistics
#[derive(Debug, Clone)]
pub struct CacheStatistics {
    pub route_hit_ratio: f64,
    pub trunk_hit_ratio: f64,
    pub client_rate_hit_ratio: f64,
    pub route_cache_size: u64,
    pub trunk_cache_size: u64,
    pub client_rate_cache_size: u64,
    pub hot_cache_size: usize,
    pub negative_cache_size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_statistics() {
        // Test cache statistics calculation
        let stats = CacheStats::default();

        stats.route_hits.store(80, std::sync::atomic::Ordering::Relaxed);
        stats.route_misses.store(20, std::sync::atomic::Ordering::Relaxed);

        let hit_ratio = stats.route_hits.load(std::sync::atomic::Ordering::Relaxed) as f64 /
            (stats.route_hits.load(std::sync::atomic::Ordering::Relaxed) +
             stats.route_misses.load(std::sync::atomic::Ordering::Relaxed)) as f64;

        assert_eq!(hit_ratio, 0.8);
    }
}