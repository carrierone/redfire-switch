use anyhow::{anyhow, Result};
use std::sync::Arc;
use tokio::time::{timeout, Duration};

use crate::lcr::cache::LcrCache;
use crate::lcr::types::{ConfigScope, TimerConfig};

pub struct TimerManager {
    cache: Arc<LcrCache>,
}

impl TimerManager {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(LcrCache::default()),
        }
    }

    pub fn with_cache(cache: Arc<LcrCache>) -> Self {
        Self { cache }
    }

    pub fn get_timer_config(&self, scope: ConfigScope, scope_id: Option<i32>) -> TimerConfig {
        self.cache.get_timer_config(scope, scope_id)
    }

    pub fn get_100_to_183_timeout(&self, ingress_trunk_id: Option<i32>) -> Duration {
        let config = self.get_timer_config(ConfigScope::IngressTrunk, ingress_trunk_id);
        Duration::from_millis(config.timer_100_to_183_ms as u64)
    }

    pub fn get_max_call_duration(&self, ingress_trunk_id: Option<i32>) -> Duration {
        let config = self.get_timer_config(ConfigScope::IngressTrunk, ingress_trunk_id);
        Duration::from_secs(config.timer_max_call_duration_sec as u64)
    }

    pub fn get_post_dial_delay(&self, egress_trunk_id: Option<i32>) -> Duration {
        let config = self.get_timer_config(ConfigScope::EgressTrunk, egress_trunk_id);
        Duration::from_millis(config.timer_post_dial_delay_ms as u64)
    }

    pub fn get_ringing_timeout(&self, ingress_trunk_id: Option<i32>) -> Duration {
        let config = self.get_timer_config(ConfigScope::IngressTrunk, ingress_trunk_id);
        Duration::from_secs(config.timer_ringing_timeout_sec as u64)
    }

    pub fn get_transaction_timeout(&self, trunk_id: Option<i32>) -> Duration {
        let config = self.get_timer_config(ConfigScope::Global, trunk_id);
        Duration::from_millis(config.timer_transaction_timeout_ms as u64)
    }

    pub async fn wait_for_100_trying<F, T>(&self, trunk_id: Option<i32>, future: F) -> Result<T>
    where
        F: std::future::Future<Output = T>,
    {
        let timeout_duration = self.get_100_to_183_timeout(trunk_id);
        timeout(timeout_duration, future)
            .await
            .map_err(|_| anyhow!("Timeout waiting for 100 Trying response"))
    }

    pub async fn wait_for_ringing<F, T>(&self, trunk_id: Option<i32>, future: F) -> Result<T>
    where
        F: std::future::Future<Output = T>,
    {
        let timeout_duration = self.get_ringing_timeout(trunk_id);
        timeout(timeout_duration, future)
            .await
            .map_err(|_| anyhow!("Timeout waiting for ringing response"))
    }

    pub async fn enforce_max_call_duration<F, T>(
        &self,
        trunk_id: Option<i32>,
        future: F,
    ) -> Result<T>
    where
        F: std::future::Future<Output = T>,
    {
        let timeout_duration = self.get_max_call_duration(trunk_id);
        timeout(timeout_duration, future)
            .await
            .map_err(|_| anyhow!("Maximum call duration exceeded"))
    }
}

#[derive(Debug, Clone)]
pub struct CallTimers {
    pub setup_start: std::time::Instant,
    pub ringing_start: Option<std::time::Instant>,
    pub answer_time: Option<std::time::Instant>,
    pub end_time: Option<std::time::Instant>,
}

impl CallTimers {
    pub fn new() -> Self {
        Self {
            setup_start: std::time::Instant::now(),
            ringing_start: None,
            answer_time: None,
            end_time: None,
        }
    }

    pub fn mark_ringing(&mut self) {
        self.ringing_start = Some(std::time::Instant::now());
    }

    pub fn mark_answered(&mut self) {
        self.answer_time = Some(std::time::Instant::now());
    }

    pub fn mark_ended(&mut self) {
        self.end_time = Some(std::time::Instant::now());
    }

    pub fn get_setup_time(&self) -> Duration {
        if let Some(ringing) = self.ringing_start {
            ringing - self.setup_start
        } else {
            std::time::Instant::now() - self.setup_start
        }
    }

    pub fn get_ringing_time(&self) -> Option<Duration> {
        if let (Some(ringing), Some(answer)) = (self.ringing_start, self.answer_time) {
            Some(answer - ringing)
        } else if let Some(ringing) = self.ringing_start {
            Some(std::time::Instant::now() - ringing)
        } else {
            None
        }
    }

    pub fn get_call_duration(&self) -> Option<Duration> {
        if let (Some(answer), Some(end)) = (self.answer_time, self.end_time) {
            Some(end - answer)
        } else if let Some(answer) = self.answer_time {
            Some(std::time::Instant::now() - answer)
        } else {
            None
        }
    }

    pub fn get_total_duration(&self) -> Duration {
        if let Some(end) = self.end_time {
            end - self.setup_start
        } else {
            std::time::Instant::now() - self.setup_start
        }
    }
}
