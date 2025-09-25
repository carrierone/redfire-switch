//! Memory Management Service for Voice Recordings
//!
//! This service manages memory usage for stored voice recordings, automatically
//! cleaning up memory when RAM is low and implementing intelligent caching strategies.
//!
//! Key features:
//! - Real-time memory monitoring
//! - Automatic cleanup of memory-stored recordings
//! - LRU-based eviction policy
//! - Configurable memory thresholds
//! - Graceful degradation under memory pressure

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn, instrument};
use uuid::Uuid;

use crate::events::{EventBus, TelecomEvent};
use crate::services::AudioStorageType as StorageType;

/// Memory management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryManagementConfig {
    /// Memory usage warning threshold (percentage)
    pub memory_warning_threshold: f64,
    /// Memory usage critical threshold (percentage)
    pub memory_critical_threshold: f64,
    /// Maximum memory for recordings (bytes)
    pub max_recording_memory: u64,
    /// Minimum free memory to maintain (bytes)
    pub min_free_memory: u64,
    /// Memory check interval (seconds)
    pub memory_check_interval: u64,
    /// Enable automatic cleanup
    pub enable_auto_cleanup: bool,
    /// Age threshold for automatic cleanup (seconds)
    pub cleanup_age_threshold: u64,
    /// Maximum recordings to keep in memory
    pub max_memory_recordings: usize,
}

impl Default for MemoryManagementConfig {
    fn default() -> Self {
        Self {
            memory_warning_threshold: 80.0,   // 80%
            memory_critical_threshold: 90.0,  // 90%
            max_recording_memory: 2 * 1024 * 1024 * 1024, // 2GB
            min_free_memory: 512 * 1024 * 1024, // 512MB
            memory_check_interval: 30, // 30 seconds
            enable_auto_cleanup: true,
            cleanup_age_threshold: 300, // 5 minutes
            max_memory_recordings: 1000,
        }
    }
}

/// Memory statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatistics {
    pub total_memory: u64,
    pub available_memory: u64,
    pub used_memory: u64,
    pub memory_usage_percent: f64,
    pub recording_memory_usage: u64,
    pub recording_count_in_memory: usize,
    pub total_recordings_cleaned: u64,
    pub last_cleanup_time: Option<DateTime<Utc>>,
}

/// Recording metadata for memory management
#[derive(Debug, Clone)]
pub struct RecordingMemoryInfo {
    pub recording_id: Uuid,
    pub size_bytes: u64,
    pub last_accessed: Instant,
    pub created_at: DateTime<Utc>,
    pub priority: RecordingPriority,
    pub storage_type: StorageType,
}

/// Recording priority for memory management
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecordingPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Memory pressure level
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryPressure {
    Normal,
    Warning,
    Critical,
    Emergency,
}

/// Memory management service
pub struct MemoryManagementService {
    config: MemoryManagementConfig,
    event_bus: Arc<EventBus>,

    // Memory tracking
    memory_recordings: Arc<RwLock<HashMap<Uuid, RecordingMemoryInfo>>>,
    access_order: Arc<RwLock<VecDeque<Uuid>>>, // LRU tracking
    current_memory_usage: Arc<RwLock<u64>>,

    // Statistics
    statistics: Arc<RwLock<MemoryStatistics>>,
    cleanup_counter: Arc<RwLock<u64>>,
}

impl MemoryManagementService {
    /// Create new memory management service
    pub fn new(config: MemoryManagementConfig, event_bus: Arc<EventBus>) -> Self {
        Self {
            config,
            event_bus,
            memory_recordings: Arc::new(RwLock::new(HashMap::new())),
            access_order: Arc::new(RwLock::new(VecDeque::new())),
            current_memory_usage: Arc::new(RwLock::new(0)),
            statistics: Arc::new(RwLock::new(MemoryStatistics {
                total_memory: 0,
                available_memory: 0,
                used_memory: 0,
                memory_usage_percent: 0.0,
                recording_memory_usage: 0,
                recording_count_in_memory: 0,
                total_recordings_cleaned: 0,
                last_cleanup_time: None,
            })),
            cleanup_counter: Arc::new(RwLock::new(0)),
        }
    }

    /// Start memory monitoring
    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting memory management monitoring");

        let config = self.config.clone();
        let event_bus = self.event_bus.clone();
        let memory_recordings = self.memory_recordings.clone();
        let access_order = self.access_order.clone();
        let current_memory_usage = self.current_memory_usage.clone();
        let statistics = self.statistics.clone();
        let cleanup_counter = self.cleanup_counter.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(config.memory_check_interval)
            );

            loop {
                interval.tick().await;

                if let Err(e) = Self::perform_memory_check(
                    &config,
                    &event_bus,
                    &memory_recordings,
                    &access_order,
                    &current_memory_usage,
                    &statistics,
                    &cleanup_counter,
                ).await {
                    error!("Memory check failed: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Perform memory check and cleanup
    async fn perform_memory_check(
        config: &MemoryManagementConfig,
        event_bus: &Arc<EventBus>,
        memory_recordings: &Arc<RwLock<HashMap<Uuid, RecordingMemoryInfo>>>,
        access_order: &Arc<RwLock<VecDeque<Uuid>>>,
        current_memory_usage: &Arc<RwLock<u64>>,
        statistics: &Arc<RwLock<MemoryStatistics>>,
        cleanup_counter: &Arc<RwLock<u64>>,
    ) -> Result<()> {
        // Get system memory info
        let memory_info = Self::get_system_memory_info().await?;
        let memory_pressure = Self::calculate_memory_pressure(config, &memory_info);

        // Update statistics
        {
            let mut stats = statistics.write().await;
            stats.total_memory = memory_info.total_memory;
            stats.available_memory = memory_info.available_memory;
            stats.used_memory = memory_info.used_memory;
            stats.memory_usage_percent = memory_info.memory_usage_percent;
            stats.recording_memory_usage = *current_memory_usage.read().await;
            stats.recording_count_in_memory = memory_recordings.read().await.len();
        }

        // Check if cleanup is needed
        if config.enable_auto_cleanup {
            let need_cleanup = match memory_pressure {
                MemoryPressure::Critical | MemoryPressure::Emergency => true,
                MemoryPressure::Warning => {
                    let recording_memory = *current_memory_usage.read().await;
                    recording_memory > config.max_recording_memory / 2
                }
                MemoryPressure::Normal => {
                    // Periodic cleanup based on age
                    Self::has_old_recordings(memory_recordings, config.cleanup_age_threshold).await
                }
            };

            if need_cleanup {
                let cleaned = Self::perform_cleanup(
                    config,
                    event_bus,
                    memory_recordings,
                    access_order,
                    current_memory_usage,
                    &memory_pressure,
                ).await?;

                if cleaned > 0 {
                    let mut counter = cleanup_counter.write().await;
                    *counter += cleaned;

                    let mut stats = statistics.write().await;
                    stats.total_recordings_cleaned = *counter;
                    stats.last_cleanup_time = Some(Utc::now());

                    info!("Memory cleanup completed: {} recordings cleaned", cleaned);
                }
            }
        }

        // Emit alerts for high memory usage
        match memory_pressure {
            MemoryPressure::Critical | MemoryPressure::Emergency => {
                error!("CRITICAL memory pressure: {:.1}% usage", memory_info.memory_usage_percent);
                let event = TelecomEvent::VoiceIntegrityAudit {
                    user_id: None,
                    action_type: "memory_pressure_critical".to_string(),
                    resource_type: "system_memory".to_string(),
                    resource_id: "memory_management".to_string(),
                    authorization_id: None,
                    ecpa_compliant: true,
                };
                let _ = event_bus.publish(event).await;
            }
            MemoryPressure::Warning => {
                warn!("HIGH memory pressure: {:.1}% usage", memory_info.memory_usage_percent);
            }
            MemoryPressure::Normal => {}
        }

        Ok(())
    }

    /// Get system memory information
    async fn get_system_memory_info() -> Result<MemoryStatistics> {
        // In a real implementation, this would use system APIs
        // For now, we'll simulate memory info
        let total_memory = 16 * 1024 * 1024 * 1024u64; // 16GB
        let available_memory = 4 * 1024 * 1024 * 1024u64; // 4GB
        let used_memory = total_memory - available_memory;
        let memory_usage_percent = (used_memory as f64 / total_memory as f64) * 100.0;

        Ok(MemoryStatistics {
            total_memory,
            available_memory,
            used_memory,
            memory_usage_percent,
            recording_memory_usage: 0,
            recording_count_in_memory: 0,
            total_recordings_cleaned: 0,
            last_cleanup_time: None,
        })
    }

    /// Calculate memory pressure level
    fn calculate_memory_pressure(config: &MemoryManagementConfig, memory_info: &MemoryStatistics) -> MemoryPressure {
        if memory_info.memory_usage_percent >= 95.0 || memory_info.available_memory < config.min_free_memory / 2 {
            MemoryPressure::Emergency
        } else if memory_info.memory_usage_percent >= config.memory_critical_threshold {
            MemoryPressure::Critical
        } else if memory_info.memory_usage_percent >= config.memory_warning_threshold {
            MemoryPressure::Warning
        } else {
            MemoryPressure::Normal
        }
    }

    /// Check if there are old recordings that need cleanup
    async fn has_old_recordings(
        memory_recordings: &Arc<RwLock<HashMap<Uuid, RecordingMemoryInfo>>>,
        age_threshold: u64,
    ) -> bool {
        let recordings = memory_recordings.read().await;
        let threshold = Instant::now() - Duration::from_secs(age_threshold);

        recordings.values().any(|recording| recording.last_accessed < threshold)
    }

    /// Perform memory cleanup
    async fn perform_cleanup(
        config: &MemoryManagementConfig,
        event_bus: &Arc<EventBus>,
        memory_recordings: &Arc<RwLock<HashMap<Uuid, RecordingMemoryInfo>>>,
        access_order: &Arc<RwLock<VecDeque<Uuid>>>,
        current_memory_usage: &Arc<RwLock<u64>>,
        memory_pressure: &MemoryPressure,
    ) -> Result<u64> {
        let mut cleaned_count = 0u64;
        let mut freed_memory = 0u64;

        // Determine cleanup strategy based on memory pressure
        let (target_cleanup_count, aggressive_cleanup) = match memory_pressure {
            MemoryPressure::Emergency => {
                // Emergency cleanup - remove up to 50% of recordings
                let recording_count = memory_recordings.read().await.len();
                (recording_count / 2, true)
            }
            MemoryPressure::Critical => {
                // Critical cleanup - remove up to 30% of recordings
                let recording_count = memory_recordings.read().await.len();
                (recording_count * 3 / 10, true)
            }
            MemoryPressure::Warning => {
                // Warning cleanup - remove old and low priority recordings
                (50, false)
            }
            MemoryPressure::Normal => {
                // Normal cleanup - remove only very old recordings
                (10, false)
            }
        };

        // Collect candidates for cleanup
        let mut cleanup_candidates = Vec::new();
        {
            let recordings = memory_recordings.read().await;
            let age_threshold = Instant::now() - Duration::from_secs(config.cleanup_age_threshold);

            for (id, recording) in recordings.iter() {
                let should_cleanup = if aggressive_cleanup {
                    // In aggressive mode, consider all non-critical recordings
                    recording.priority != RecordingPriority::Critical
                } else {
                    // In normal mode, only old or low priority recordings
                    recording.last_accessed < age_threshold || recording.priority == RecordingPriority::Low
                };

                if should_cleanup {
                    cleanup_candidates.push((*id, recording.clone()));
                }
            }
        }

        // Sort candidates by priority (lowest first) and age (oldest first)
        cleanup_candidates.sort_by(|a, b| {
            a.1.priority.cmp(&b.1.priority)
                .then(a.1.last_accessed.cmp(&b.1.last_accessed))
        });

        // Remove recordings up to target count
        let candidates_to_remove = cleanup_candidates.into_iter()
            .take(target_cleanup_count)
            .collect::<Vec<_>>();

        for (recording_id, recording_info) in candidates_to_remove {
            // Only remove memory-stored recordings
            if recording_info.storage_type == StorageType::Memory {
                {
                    let mut recordings = memory_recordings.write().await;
                    recordings.remove(&recording_id);
                }

                {
                    let mut order = access_order.write().await;
                    order.retain(|&id| id != recording_id);
                }

                freed_memory += recording_info.size_bytes;
                cleaned_count += 1;

                debug!("Cleaned recording {} ({} bytes)", recording_id, recording_info.size_bytes);

                // Emit cleanup event
                let event = TelecomEvent::VoiceIntegrityAudit {
                    user_id: None,
                    action_type: "memory_cleanup".to_string(),
                    resource_type: "voice_recording".to_string(),
                    resource_id: recording_id.to_string(),
                    authorization_id: None,
                    ecpa_compliant: true,
                };
                let _ = event_bus.publish(event).await;
            }
        }

        // Update memory usage
        {
            let mut usage = current_memory_usage.write().await;
            *usage = usage.saturating_sub(freed_memory);
        }

        if cleaned_count > 0 {
            info!("Memory cleanup completed: {} recordings removed, {} bytes freed",
                  cleaned_count, freed_memory);
        }

        Ok(cleaned_count)
    }

    /// Register a recording in memory
    #[instrument(skip(self), fields(recording_id = %recording_id))]
    pub async fn register_recording(
        &self,
        recording_id: Uuid,
        size_bytes: u64,
        priority: RecordingPriority,
        storage_type: StorageType,
    ) -> Result<()> {
        let recording_info = RecordingMemoryInfo {
            recording_id,
            size_bytes,
            last_accessed: Instant::now(),
            created_at: Utc::now(),
            priority,
            storage_type,
        };

        // Check if we would exceed memory limits
        if storage_type == StorageType::Memory {
            let current_usage = *self.current_memory_usage.read().await;
            if current_usage + size_bytes > self.config.max_recording_memory {
                warn!("Recording {} would exceed memory limit, triggering cleanup", recording_id);

                // Force cleanup before adding
                let _ = Self::perform_cleanup(
                    &self.config,
                    &self.event_bus,
                    &self.memory_recordings,
                    &self.access_order,
                    &self.current_memory_usage,
                    &MemoryPressure::Warning,
                ).await;
            }
        }

        // Register the recording
        {
            let mut recordings = self.memory_recordings.write().await;
            recordings.insert(recording_id, recording_info);
        }

        // Add to access order (LRU)
        {
            let mut order = self.access_order.write().await;
            order.push_back(recording_id);

            // Limit access order size
            while order.len() > self.config.max_memory_recordings {
                order.pop_front();
            }
        }

        // Update memory usage
        if storage_type == StorageType::Memory {
            let mut usage = self.current_memory_usage.write().await;
            *usage += size_bytes;
        }

        debug!("Registered recording {} in memory ({} bytes)", recording_id, size_bytes);
        Ok(())
    }

    /// Mark a recording as accessed (update LRU)
    pub async fn mark_accessed(&self, recording_id: Uuid) -> Result<()> {
        // Update last accessed time
        {
            let mut recordings = self.memory_recordings.write().await;
            if let Some(recording) = recordings.get_mut(&recording_id) {
                recording.last_accessed = Instant::now();
            }
        }

        // Update LRU order
        {
            let mut order = self.access_order.write().await;
            order.retain(|&id| id != recording_id);
            order.push_back(recording_id);
        }

        Ok(())
    }

    /// Remove a recording from memory tracking
    pub async fn unregister_recording(&self, recording_id: Uuid) -> Result<()> {
        let freed_bytes = {
            let mut recordings = self.memory_recordings.write().await;
            if let Some(recording) = recordings.remove(&recording_id) {
                recording.size_bytes
            } else {
                0
            }
        };

        {
            let mut order = self.access_order.write().await;
            order.retain(|&id| id != recording_id);
        }

        if freed_bytes > 0 {
            let mut usage = self.current_memory_usage.write().await;
            *usage = usage.saturating_sub(freed_bytes);
        }

        debug!("Unregistered recording {} from memory ({} bytes freed)", recording_id, freed_bytes);
        Ok(())
    }

    /// Force cleanup of old recordings
    pub async fn force_cleanup(&self) -> Result<u64> {
        info!("Forcing memory cleanup");

        Self::perform_cleanup(
            &self.config,
            &self.event_bus,
            &self.memory_recordings,
            &self.access_order,
            &self.current_memory_usage,
            &MemoryPressure::Warning,
        ).await
    }

    /// Get memory statistics
    pub async fn get_statistics(&self) -> MemoryStatistics {
        self.statistics.read().await.clone()
    }

    /// Get current memory pressure level
    pub async fn get_memory_pressure(&self) -> Result<MemoryPressure> {
        let memory_info = Self::get_system_memory_info().await?;
        Ok(Self::calculate_memory_pressure(&self.config, &memory_info))
    }

    /// Get recordings in memory by priority
    pub async fn get_recordings_by_priority(&self, priority: RecordingPriority) -> Vec<Uuid> {
        let recordings = self.memory_recordings.read().await;
        recordings.iter()
            .filter(|(_, info)| info.priority == priority)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Check if recording is in memory
    pub async fn is_recording_in_memory(&self, recording_id: Uuid) -> bool {
        let recordings = self.memory_recordings.read().await;
        recordings.contains_key(&recording_id)
    }
}