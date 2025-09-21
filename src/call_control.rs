//! Call Control module - High-performance production implementation
//! Handles call admission control, session management, and resource allocation
//! Optimized for carrier-grade performance with lock-free data structures

use ahash::AHasher;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::hash::BuildHasherDefault;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::performance::memory_pools::{pools, CallState, PooledCallSession};
use crate::performance::string_interner::{
    intern_phone_number, intern_trunk_id, resolve_trunk_id, Symbol,
};

type FastHasher = BuildHasherDefault<AHasher>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallControlConfig {
    pub enabled: bool,
    pub max_concurrent_calls: usize,
    pub max_calls_per_second: usize,
    pub call_timeout_seconds: u64,
    pub enable_admission_control: bool,
    pub enable_resource_monitoring: bool,
}

impl Default for CallControlConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_concurrent_calls: 10000,
            max_calls_per_second: 100,
            call_timeout_seconds: 1800, // 30 minutes
            enable_admission_control: true,
            enable_resource_monitoring: true,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TrunkDirection {
    Inbound,
    Outbound,
    Bidirectional,
}

#[derive(Debug, Clone)]
pub struct TrunkGroupLimits {
    pub max_concurrent: usize,
    pub cps_limit: usize,
    pub max_bandwidth_kbps: Option<u32>,
}

/// High-performance call session using memory pools
pub type CallSession = PooledCallSession;

/// Lock-free call control service optimized for high throughput
#[derive(Debug)]
pub struct CallControlService {
    config: CallControlConfig,
    /// Lock-free session storage using DashMap
    active_sessions: DashMap<Uuid, CallSession, FastHasher>,
    /// Lock-free trunk limits storage
    trunk_limits: DashMap<Symbol, TrunkGroupLimits, FastHasher>,
    /// Atomic counters for call statistics
    active_calls: AtomicUsize,
    total_calls_established: AtomicU64,
    total_calls_failed: AtomicU64,
    calls_per_second_current: AtomicUsize,
    last_cps_reset: AtomicU64, // Unix timestamp in seconds
}

/// Fast call admission decision
#[derive(Debug, Clone, Copy)]
pub struct AdmissionDecision {
    pub admit: bool,
    pub reason: AdmissionReason,
    pub current_load: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum AdmissionReason {
    Admitted,
    GlobalLimit,
    TrunkLimit,
    CpsLimit,
    ResourceLimit,
    Disabled,
}

impl CallControlService {
    pub fn new(config: CallControlConfig) -> Self {
        Self {
            config,
            active_sessions: DashMap::with_hasher(FastHasher::default()),
            trunk_limits: DashMap::with_hasher(FastHasher::default()),
            active_calls: AtomicUsize::new(0),
            total_calls_established: AtomicU64::new(0),
            total_calls_failed: AtomicU64::new(0),
            calls_per_second_current: AtomicUsize::new(0),
            last_cps_reset: AtomicU64::new(chrono::Utc::now().timestamp() as u64),
        }
    }

    /// High-performance call admission check (lock-free)
    pub fn can_admit_call_fast(
        &self,
        from_addr: IpAddr,
        trunk_id: Option<&str>,
    ) -> AdmissionDecision {
        if !self.config.enabled {
            return AdmissionDecision {
                admit: true,
                reason: AdmissionReason::Admitted,
                current_load: 0.0,
            };
        }

        // Check global call limit (atomic operation)
        let current_calls = self.active_calls.load(Ordering::Relaxed);
        if current_calls >= self.config.max_concurrent_calls {
            return AdmissionDecision {
                admit: false,
                reason: AdmissionReason::GlobalLimit,
                current_load: current_calls as f32 / self.config.max_concurrent_calls as f32,
            };
        }

        // Check CPS limit (atomic operation)
        let now = chrono::Utc::now().timestamp() as u64;
        let last_reset = self.last_cps_reset.load(Ordering::Relaxed);

        // Reset CPS counter if needed (once per second)
        if now > last_reset {
            if self
                .last_cps_reset
                .compare_exchange_weak(last_reset, now, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                self.calls_per_second_current.store(0, Ordering::Relaxed);
            }
        }

        let current_cps = self.calls_per_second_current.load(Ordering::Relaxed);
        if current_cps >= self.config.max_calls_per_second {
            return AdmissionDecision {
                admit: false,
                reason: AdmissionReason::CpsLimit,
                current_load: current_cps as f32 / self.config.max_calls_per_second as f32,
            };
        }

        // Check trunk-specific limits (lock-free lookup)
        if let Some(trunk_name) = trunk_id {
            let trunk_symbol = intern_trunk_id(trunk_name);
            if let Some(limits) = self.trunk_limits.get(&trunk_symbol) {
                // Count active calls for this trunk (fast iteration)
                let trunk_calls = self
                    .active_sessions
                    .iter()
                    .filter(|entry| {
                        entry
                            .value()
                            .trunk_id
                            .as_ref()
                            .map(|id| id.as_str() == trunk_name)
                            .unwrap_or(false)
                    })
                    .count();

                if trunk_calls >= limits.max_concurrent {
                    return AdmissionDecision {
                        admit: false,
                        reason: AdmissionReason::TrunkLimit,
                        current_load: trunk_calls as f32 / limits.max_concurrent as f32,
                    };
                }
            }
        }

        AdmissionDecision {
            admit: true,
            reason: AdmissionReason::Admitted,
            current_load: current_calls as f32 / self.config.max_concurrent_calls as f32,
        }
    }

    /// Async wrapper for compatibility
    pub async fn can_admit_call(&self, from_addr: IpAddr, trunk_id: Option<&str>) -> Result<bool> {
        let decision = self.can_admit_call_fast(from_addr, trunk_id);
        Ok(decision.admit)
    }

    /// Register a new call session (optimized with memory pools)
    pub async fn register_call(
        &self,
        call_id: String,
        from_addr: IpAddr,
        to_addr: IpAddr,
        trunk_id: Option<String>,
    ) -> Result<Uuid> {
        // Get a pooled call session object
        let mut session = pools().get_call_session();

        // Initialize with call data
        session.id = Uuid::new_v4();
        session.call_id.clear();
        session.call_id.push_str(&call_id);
        session.from_addr = from_addr;
        session.to_addr = to_addr;
        session.start_time = chrono::Utc::now();
        session.last_activity = chrono::Utc::now();
        session.state = CallState::Establishing;
        session.trunk_id = trunk_id.map(|id| {
            let mut fast_string = crate::performance::memory_pools::FastString::new();
            fast_string.push_str(&id);
            fast_string
        });

        let session_id = session.id;

        // Store in lock-free map (clone the session data)
        let stored_session = CallSession {
            id: session.id,
            call_id: session.call_id.clone(),
            from_addr: session.from_addr,
            to_addr: session.to_addr,
            start_time: session.start_time,
            last_activity: session.last_activity,
            state: session.state.clone(),
            trunk_id: session.trunk_id.clone(),
            codec_pair: session.codec_pair.clone(),
        };

        self.active_sessions.insert(session_id, stored_session);

        // Update atomic counters
        self.active_calls.fetch_add(1, Ordering::Relaxed);
        self.total_calls_established.fetch_add(1, Ordering::Relaxed);
        self.calls_per_second_current
            .fetch_add(1, Ordering::Relaxed);

        info!(
            "Registered call {} from {} to {} (total active: {})",
            call_id,
            from_addr,
            to_addr,
            self.active_calls.load(Ordering::Relaxed)
        );

        Ok(session_id)
    }

    /// End a call session (lock-free)
    pub async fn end_call(&self, session_id: Uuid) -> Result<()> {
        if let Some((_, session)) = self.active_sessions.remove(&session_id) {
            // Update atomic counters
            self.active_calls.fetch_sub(1, Ordering::Relaxed);

            info!(
                "Ended call {} (remaining active: {})",
                session.call_id,
                self.active_calls.load(Ordering::Relaxed)
            );
        }

        Ok(())
    }

    /// Update call state (lock-free)
    pub async fn update_call_state(&self, session_id: Uuid, new_state: CallState) -> Result<()> {
        if let Some(mut session) = self.active_sessions.get_mut(&session_id) {
            session.state = new_state;
            session.last_activity = chrono::Utc::now();
        }

        Ok(())
    }

    /// Get current call statistics (lock-free)
    pub fn get_call_stats(&self) -> CallStats {
        CallStats {
            active_calls: self.active_calls.load(Ordering::Relaxed),
            total_calls_established: self.total_calls_established.load(Ordering::Relaxed),
            total_calls_failed: self.total_calls_failed.load(Ordering::Relaxed),
            calls_per_second_current: self.calls_per_second_current.load(Ordering::Relaxed),
            session_count: self.active_sessions.len(),
        }
    }

    /// Add or update trunk limits (lock-free)
    pub fn set_trunk_limits(&self, trunk_id: &str, limits: TrunkGroupLimits) {
        let trunk_symbol = intern_trunk_id(trunk_id);
        self.trunk_limits.insert(trunk_symbol, limits);
    }

    /// Get a call session (lock-free read)
    pub fn get_call_session(&self, session_id: &Uuid) -> Option<CallSession> {
        self.active_sessions
            .get(session_id)
            .map(|entry| entry.value().clone())
    }

    /// Count active calls for a specific trunk (optimized)
    pub fn get_trunk_call_count(&self, trunk_id: &str) -> usize {
        self.active_sessions
            .iter()
            .filter(|entry| {
                entry
                    .value()
                    .trunk_id
                    .as_ref()
                    .map(|id| id.as_str() == trunk_id)
                    .unwrap_or(false)
            })
            .count()
    }
}

/// Call statistics (lock-free access)
#[derive(Debug, Clone)]
pub struct CallStats {
    pub active_calls: usize,
    pub total_calls_established: u64,
    pub total_calls_failed: u64,
    pub calls_per_second_current: usize,
    pub session_count: usize,
}
