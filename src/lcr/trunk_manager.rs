use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::lcr::types::{TrunkType, TrunkUsageStats};

#[derive(Debug, Clone)]
pub struct TrunkStats {
    pub current_calls: i32,
    pub capacity_limit: i32,
    pub current_cps: Decimal,
    pub cps_limit: Decimal,
    pub total_calls: i64,
    pub total_minutes: Decimal,
    pub last_call_at: Option<DateTime<Utc>>,
    pub cps_window: Vec<DateTime<Utc>>, // Track call timestamps for CPS calculation
}

pub struct TrunkManager {
    egress_stats: DashMap<i32, Arc<RwLock<TrunkStats>>>,
    ingress_stats: DashMap<i32, Arc<RwLock<TrunkStats>>>,
}

impl TrunkManager {
    pub fn new() -> Self {
        Self {
            egress_stats: DashMap::new(),
            ingress_stats: DashMap::new(),
        }
    }

    pub async fn can_accept_call(&self, trunk_id: i32, trunk_type: TrunkType) -> bool {
        let stats_map = match trunk_type {
            TrunkType::Egress => &self.egress_stats,
            TrunkType::Ingress => &self.ingress_stats,
        };

        if let Some(stats_lock) = stats_map.get(&trunk_id) {
            let stats = stats_lock.read().await;

            // Check capacity limit
            if stats.current_calls >= stats.capacity_limit {
                return false;
            }

            // Check CPS limit
            if stats.current_cps >= stats.cps_limit {
                return false;
            }

            true
        } else {
            // If no stats exist, assume trunk can accept calls
            true
        }
    }

    pub async fn increment_call(
        &self,
        trunk_id: i32,
        trunk_type: TrunkType,
        capacity_limit: i32,
        cps_limit: Decimal,
    ) -> Result<()> {
        let stats_map = match trunk_type {
            TrunkType::Egress => &self.egress_stats,
            TrunkType::Ingress => &self.ingress_stats,
        };

        let stats_lock = stats_map.entry(trunk_id).or_insert_with(|| {
            Arc::new(RwLock::new(TrunkStats {
                current_calls: 0,
                capacity_limit,
                current_cps: Decimal::ZERO,
                cps_limit,
                total_calls: 0,
                total_minutes: Decimal::ZERO,
                last_call_at: None,
                cps_window: Vec::new(),
            }))
        });

        let mut stats = stats_lock.write().await;
        let now = Utc::now();

        // Update call count
        stats.current_calls += 1;
        stats.total_calls += 1;
        stats.last_call_at = Some(now);

        // Update CPS window (keep only last second of calls)
        let one_second_ago = now - Duration::seconds(1);
        stats.cps_window.retain(|&t| t > one_second_ago);
        stats.cps_window.push(now);
        stats.current_cps = Decimal::from(stats.cps_window.len());

        Ok(())
    }

    pub async fn decrement_call(
        &self,
        trunk_id: i32,
        trunk_type: TrunkType,
        call_duration_seconds: i64,
    ) -> Result<()> {
        let stats_map = match trunk_type {
            TrunkType::Egress => &self.egress_stats,
            TrunkType::Ingress => &self.ingress_stats,
        };

        if let Some(stats_lock) = stats_map.get(&trunk_id) {
            let mut stats = stats_lock.write().await;

            // Update call count
            if stats.current_calls > 0 {
                stats.current_calls -= 1;
            }

            // Update total minutes
            let minutes = Decimal::from(call_duration_seconds) / Decimal::from(60);
            stats.total_minutes += minutes;
        }

        Ok(())
    }

    pub async fn get_trunk_stats(
        &self,
        trunk_id: i32,
        trunk_type: TrunkType,
    ) -> Option<TrunkUsageStats> {
        let stats_map = match trunk_type {
            TrunkType::Egress => &self.egress_stats,
            TrunkType::Ingress => &self.ingress_stats,
        };

        if let Some(stats_lock) = stats_map.get(&trunk_id) {
            let stats = stats_lock.read().await;

            Some(TrunkUsageStats {
                trunk_id,
                trunk_type,
                current_calls: stats.current_calls,
                current_cps: stats.current_cps,
                total_calls: stats.total_calls,
                total_minutes: stats.total_minutes,
                last_call_at: stats.last_call_at,
            })
        } else {
            None
        }
    }

    pub async fn reset_trunk_stats(&self, trunk_id: i32, trunk_type: TrunkType) {
        let stats_map = match trunk_type {
            TrunkType::Egress => &self.egress_stats,
            TrunkType::Ingress => &self.ingress_stats,
        };

        if let Some(stats_lock) = stats_map.get(&trunk_id) {
            let mut stats = stats_lock.write().await;
            stats.current_calls = 0;
            stats.current_cps = Decimal::ZERO;
            stats.cps_window.clear();
        }
    }

    pub async fn get_all_stats(&self) -> Vec<TrunkUsageStats> {
        let mut all_stats = Vec::new();

        // Get egress stats
        for entry in self.egress_stats.iter() {
            let stats = entry.value().read().await;
            all_stats.push(TrunkUsageStats {
                trunk_id: *entry.key(),
                trunk_type: TrunkType::Egress,
                current_calls: stats.current_calls,
                current_cps: stats.current_cps,
                total_calls: stats.total_calls,
                total_minutes: stats.total_minutes,
                last_call_at: stats.last_call_at,
            });
        }

        // Get ingress stats
        for entry in self.ingress_stats.iter() {
            let stats = entry.value().read().await;
            all_stats.push(TrunkUsageStats {
                trunk_id: *entry.key(),
                trunk_type: TrunkType::Ingress,
                current_calls: stats.current_calls,
                current_cps: stats.current_cps,
                total_calls: stats.total_calls,
                total_minutes: stats.total_minutes,
                last_call_at: stats.last_call_at,
            });
        }

        all_stats
    }
}
