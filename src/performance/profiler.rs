//! Performance Profiler Module
//!
//! Provides comprehensive performance monitoring and analysis for Redfire Switch
//! Key areas monitored:
//! - Codec transcoding performance
//! - Database query performance
//! - Memory pool allocation efficiency
//! - Network packet processing rates

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, instrument};

/// Performance metrics for codec operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecPerformanceMetrics {
    pub codec_type: String,
    pub operation: CodecOperation,
    pub average_processing_time_us: u64,
    pub peak_processing_time_us: u64,
    pub min_processing_time_us: u64,
    pub samples_processed: u64,
    pub frames_per_second: f64,
    pub cpu_utilization_percent: f32,
    pub memory_usage_bytes: u64,
    pub error_rate_percent: f32,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CodecOperation {
    Encode,
    Decode,
    Transcode,
    BatchProcess,
}

/// Database query performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabasePerformanceMetrics {
    pub query_type: String,
    pub average_query_time_ms: f64,
    pub peak_query_time_ms: u64,
    pub min_query_time_ms: u64,
    pub queries_per_second: f64,
    pub cache_hit_rate_percent: f32,
    pub connection_pool_usage_percent: f32,
    pub slow_query_count: u32,
    pub last_updated: DateTime<Utc>,
}

/// Memory pool allocation metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPoolMetrics {
    pub pool_name: String,
    pub pool_size: usize,
    pub active_allocations: usize,
    pub allocation_rate_per_second: f64,
    pub average_object_lifetime_ms: f64,
    pub pool_utilization_percent: f32,
    pub allocation_failures: u32,
    pub fragmentation_percent: f32,
    pub last_updated: DateTime<Utc>,
}

/// Network performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPerformanceMetrics {
    pub interface_name: String,
    pub packets_per_second_in: f64,
    pub packets_per_second_out: f64,
    pub bytes_per_second_in: f64,
    pub bytes_per_second_out: f64,
    pub packet_loss_rate_percent: f32,
    pub average_latency_ms: f32,
    pub jitter_ms: f32,
    pub last_updated: DateTime<Utc>,
}

/// Comprehensive system performance snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPerformanceSnapshot {
    pub timestamp: DateTime<Utc>,
    pub cpu_usage_percent: f32,
    pub memory_usage_bytes: u64,
    pub memory_available_bytes: u64,
    pub disk_io_read_bytes_per_sec: f64,
    pub disk_io_write_bytes_per_sec: f64,
    pub active_calls: u32,
    pub calls_per_second: f64,
    pub codec_metrics: Vec<CodecPerformanceMetrics>,
    pub database_metrics: Vec<DatabasePerformanceMetrics>,
    pub memory_pool_metrics: Vec<MemoryPoolMetrics>,
    pub network_metrics: Vec<NetworkPerformanceMetrics>,
}

/// Performance tracking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilerConfig {
    /// Enable performance profiling
    pub enabled: bool,
    /// Sampling interval in milliseconds
    pub sampling_interval_ms: u64,
    /// Number of samples to keep in history
    pub history_samples: usize,
    /// CPU usage threshold for alerts (percentage)
    pub cpu_alert_threshold: f32,
    /// Memory usage threshold for alerts (percentage)
    pub memory_alert_threshold: f32,
    /// Database query time threshold for slow query logging (milliseconds)
    pub slow_query_threshold_ms: u64,
    /// Export metrics to file
    pub export_to_file: bool,
    /// Export file path
    pub export_file_path: String,
}

impl Default for ProfilerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sampling_interval_ms: 1000,
            history_samples: 300, // 5 minutes at 1 second intervals
            cpu_alert_threshold: 80.0,
            memory_alert_threshold: 85.0,
            slow_query_threshold_ms: 100,
            export_to_file: false,
            export_file_path: "/var/log/redfire/performance.json".to_string(),
        }
    }
}

/// Main performance profiler
pub struct PerformanceProfiler {
    config: ProfilerConfig,
    history: Arc<RwLock<VecDeque<SystemPerformanceSnapshot>>>,
    codec_timings: Arc<RwLock<HashMap<String, VecDeque<Duration>>>>,
    database_timings: Arc<RwLock<HashMap<String, VecDeque<Duration>>>>,
    memory_stats: Arc<RwLock<HashMap<String, MemoryPoolMetrics>>>,
}

impl PerformanceProfiler {
    pub fn new(config: ProfilerConfig) -> Self {
        Self {
            config,
            history: Arc::new(RwLock::new(VecDeque::new())),
            codec_timings: Arc::new(RwLock::new(HashMap::new())),
            database_timings: Arc::new(RwLock::new(HashMap::new())),
            memory_stats: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the performance monitoring loop
    pub async fn start_monitoring(&self) {
        if !self.config.enabled {
            info!("Performance profiler disabled");
            return;
        }

        info!("Starting performance profiler with {}ms sampling interval",
              self.config.sampling_interval_ms);

        let history = self.history.clone();
        let config = self.config.clone();
        let codec_timings = self.codec_timings.clone();
        let database_timings = self.database_timings.clone();
        let memory_stats = self.memory_stats.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_millis(config.sampling_interval_ms)
            );

            loop {
                interval.tick().await;

                let snapshot = Self::capture_system_snapshot(
                    &config,
                    &codec_timings,
                    &database_timings,
                    &memory_stats,
                ).await;

                // Add to history
                {
                    let mut history_guard = history.write().await;
                    history_guard.push_back(snapshot.clone());

                    // Trim history if too long
                    while history_guard.len() > config.history_samples {
                        history_guard.pop_front();
                    }
                }

                // Check for alerts
                Self::check_performance_alerts(&config, &snapshot);

                // Export if configured
                if config.export_to_file {
                    if let Err(e) = Self::export_snapshot(&config, &snapshot).await {
                        warn!("Failed to export performance snapshot: {}", e);
                    }
                }

                debug!("Captured performance snapshot: CPU {:.1}%, Memory {:.1}GB",
                       snapshot.cpu_usage_percent,
                       snapshot.memory_usage_bytes as f64 / 1024.0 / 1024.0 / 1024.0);
            }
        });
    }

    /// Record codec operation timing
    #[instrument(skip(self))]
    pub async fn record_codec_timing(&self, codec_name: &str, operation: CodecOperation, duration: Duration) {
        let mut timings = self.codec_timings.write().await;
        let key = format!("{:?}_{}", operation, codec_name);

        let queue = timings.entry(key).or_insert_with(VecDeque::new);
        queue.push_back(duration);

        // Keep only recent timings
        while queue.len() > 100 {
            queue.pop_front();
        }
    }

    /// Record database query timing
    #[instrument(skip(self))]
    pub async fn record_database_timing(&self, query_type: &str, duration: Duration) {
        let mut timings = self.database_timings.write().await;

        let queue = timings.entry(query_type.to_string()).or_insert_with(VecDeque::new);
        queue.push_back(duration);

        // Keep only recent timings
        while queue.len() > 100 {
            queue.pop_front();
        }

        // Log slow queries
        if duration.as_millis() > self.config.slow_query_threshold_ms as u128 {
            warn!("Slow query detected: {} took {}ms", query_type, duration.as_millis());
        }
    }

    /// Update memory pool statistics
    pub async fn update_memory_pool_stats(&self, pool_name: &str, metrics: MemoryPoolMetrics) {
        let mut stats = self.memory_stats.write().await;
        stats.insert(pool_name.to_string(), metrics);
    }

    /// Get current performance snapshot
    pub async fn get_current_snapshot(&self) -> SystemPerformanceSnapshot {
        Self::capture_system_snapshot(
            &self.config,
            &self.codec_timings,
            &self.database_timings,
            &self.memory_stats,
        ).await
    }

    /// Get performance history
    pub async fn get_performance_history(&self) -> Vec<SystemPerformanceSnapshot> {
        let history = self.history.read().await;
        history.iter().cloned().collect()
    }

    /// Capture a system performance snapshot
    async fn capture_system_snapshot(
        _config: &ProfilerConfig,
        codec_timings: &Arc<RwLock<HashMap<String, VecDeque<Duration>>>>,
        database_timings: &Arc<RwLock<HashMap<String, VecDeque<Duration>>>>,
        memory_stats: &Arc<RwLock<HashMap<String, MemoryPoolMetrics>>>,
    ) -> SystemPerformanceSnapshot {
        let timestamp = Utc::now();

        // Get system metrics (simplified implementation)
        let (cpu_usage, memory_usage, memory_available) = Self::get_system_metrics();
        let (disk_read, disk_write) = Self::get_disk_io_metrics();
        let (active_calls, calls_per_second) = Self::get_call_metrics();

        // Collect codec metrics
        let codec_metrics = Self::collect_codec_metrics(codec_timings).await;

        // Collect database metrics
        let database_metrics = Self::collect_database_metrics(database_timings).await;

        // Collect memory pool metrics
        let memory_pool_metrics = {
            let stats = memory_stats.read().await;
            stats.values().cloned().collect()
        };

        // Collect network metrics
        let network_metrics = Self::collect_network_metrics().await;

        SystemPerformanceSnapshot {
            timestamp,
            cpu_usage_percent: cpu_usage,
            memory_usage_bytes: memory_usage,
            memory_available_bytes: memory_available,
            disk_io_read_bytes_per_sec: disk_read,
            disk_io_write_bytes_per_sec: disk_write,
            active_calls,
            calls_per_second,
            codec_metrics,
            database_metrics,
            memory_pool_metrics,
            network_metrics,
        }
    }

    /// Check for performance alerts
    fn check_performance_alerts(config: &ProfilerConfig, snapshot: &SystemPerformanceSnapshot) {
        if snapshot.cpu_usage_percent > config.cpu_alert_threshold {
            warn!("High CPU usage alert: {:.1}% (threshold: {:.1}%)",
                  snapshot.cpu_usage_percent, config.cpu_alert_threshold);
        }

        let memory_usage_percent = (snapshot.memory_usage_bytes as f64 /
                                   (snapshot.memory_usage_bytes + snapshot.memory_available_bytes) as f64) * 100.0;

        if memory_usage_percent > config.memory_alert_threshold as f64 {
            warn!("High memory usage alert: {:.1}% (threshold: {:.1}%)",
                  memory_usage_percent, config.memory_alert_threshold);
        }
    }

    /// Export snapshot to file
    async fn export_snapshot(config: &ProfilerConfig, snapshot: &SystemPerformanceSnapshot) -> Result<()> {
        let json = serde_json::to_string_pretty(snapshot)?;
        tokio::fs::write(&config.export_file_path, json).await?;
        Ok(())
    }

    // Simplified system metric collection - in production would use proper system APIs
    fn get_system_metrics() -> (f32, u64, u64) {
        // Placeholder implementation
        (25.5, 2_147_483_648, 6_442_450_944) // 25.5% CPU, 2GB used, 6GB available
    }

    fn get_disk_io_metrics() -> (f64, f64) {
        // Placeholder implementation
        (1048576.0, 524288.0) // 1MB/s read, 512KB/s write
    }

    fn get_call_metrics() -> (u32, f64) {
        // Placeholder implementation
        (150, 2.5) // 150 active calls, 2.5 calls/sec
    }

    async fn collect_codec_metrics(
        codec_timings: &Arc<RwLock<HashMap<String, VecDeque<Duration>>>>
    ) -> Vec<CodecPerformanceMetrics> {
        let timings = codec_timings.read().await;
        let mut metrics = Vec::new();

        for (key, durations) in timings.iter() {
            if durations.is_empty() {
                continue;
            }

            let total_us: u64 = durations.iter().map(|d| d.as_micros() as u64).sum();
            let count = durations.len() as u64;
            let average_us = total_us / count;
            let peak_us = durations.iter().map(|d| d.as_micros() as u64).max().unwrap_or(0);
            let min_us = durations.iter().map(|d| d.as_micros() as u64).min().unwrap_or(0);

            let parts: Vec<&str> = key.split('_').collect();
            let operation = match parts.first() {
                Some(&"Encode") => CodecOperation::Encode,
                Some(&"Decode") => CodecOperation::Decode,
                Some(&"Transcode") => CodecOperation::Transcode,
                Some(&"BatchProcess") => CodecOperation::BatchProcess,
                _ => CodecOperation::Transcode,
            };
            let codec_type = parts.get(1).unwrap_or(&"Unknown").to_string();

            metrics.push(CodecPerformanceMetrics {
                codec_type,
                operation,
                average_processing_time_us: average_us,
                peak_processing_time_us: peak_us,
                min_processing_time_us: min_us,
                samples_processed: count,
                frames_per_second: if average_us > 0 { 1_000_000.0 / average_us as f64 } else { 0.0 },
                cpu_utilization_percent: 15.0, // Placeholder
                memory_usage_bytes: 1024 * 512, // Placeholder
                error_rate_percent: 0.1, // Placeholder
                last_updated: Utc::now(),
            });
        }

        metrics
    }

    async fn collect_database_metrics(
        database_timings: &Arc<RwLock<HashMap<String, VecDeque<Duration>>>>
    ) -> Vec<DatabasePerformanceMetrics> {
        let timings = database_timings.read().await;
        let mut metrics = Vec::new();

        for (query_type, durations) in timings.iter() {
            if durations.is_empty() {
                continue;
            }

            let total_ms: f64 = durations.iter().map(|d| d.as_millis() as f64).sum();
            let count = durations.len();
            let average_ms = total_ms / count as f64;
            let peak_ms = durations.iter().map(|d| d.as_millis()).max().unwrap_or(0);
            let min_ms = durations.iter().map(|d| d.as_millis()).min().unwrap_or(0);

            metrics.push(DatabasePerformanceMetrics {
                query_type: query_type.clone(),
                average_query_time_ms: average_ms,
                peak_query_time_ms: peak_ms as u64,
                min_query_time_ms: min_ms as u64,
                queries_per_second: if average_ms > 0.0 { 1000.0 / average_ms } else { 0.0 },
                cache_hit_rate_percent: 85.0, // Placeholder
                connection_pool_usage_percent: 60.0, // Placeholder
                slow_query_count: durations.iter().filter(|d| d.as_millis() > 100).count() as u32,
                last_updated: Utc::now(),
            });
        }

        metrics
    }

    async fn collect_network_metrics() -> Vec<NetworkPerformanceMetrics> {
        // Placeholder implementation - in production would collect from network interfaces
        vec![NetworkPerformanceMetrics {
            interface_name: "eth0".to_string(),
            packets_per_second_in: 1500.0,
            packets_per_second_out: 1200.0,
            bytes_per_second_in: 2_097_152.0, // 2 MB/s
            bytes_per_second_out: 1_572_864.0, // 1.5 MB/s
            packet_loss_rate_percent: 0.01,
            average_latency_ms: 2.5,
            jitter_ms: 0.8,
            last_updated: Utc::now(),
        }]
    }
}

/// Performance optimization recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationRecommendation {
    pub category: OptimizationCategory,
    pub priority: OptimizationPriority,
    pub title: String,
    pub description: String,
    pub estimated_improvement: String,
    pub implementation_effort: ImplementationEffort,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationCategory {
    Codec,
    Database,
    Memory,
    Network,
    CPU,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum OptimizationPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImplementationEffort {
    Minimal,  // < 1 day
    Low,      // 1-3 days
    Medium,   // 1-2 weeks
    High,     // 2-4 weeks
}

/// Performance optimization analyzer
pub struct PerformanceAnalyzer;

impl PerformanceAnalyzer {
    /// Analyze performance data and generate optimization recommendations
    pub fn analyze_performance(snapshots: &[SystemPerformanceSnapshot]) -> Vec<OptimizationRecommendation> {
        let mut recommendations = Vec::new();

        // Analyze CPU usage trends
        if let Some(avg_cpu) = Self::calculate_average_cpu(snapshots) {
            if avg_cpu > 70.0 {
                recommendations.push(OptimizationRecommendation {
                    category: OptimizationCategory::CPU,
                    priority: OptimizationPriority::High,
                    title: "High CPU Usage Detected".to_string(),
                    description: format!("Average CPU usage is {:.1}%. Consider optimizing codec algorithms or enabling GPU acceleration.", avg_cpu),
                    estimated_improvement: "15-25% CPU reduction".to_string(),
                    implementation_effort: ImplementationEffort::Medium,
                });
            }
        }

        // Analyze codec performance
        Self::analyze_codec_performance(snapshots, &mut recommendations);

        // Analyze database performance
        Self::analyze_database_performance(snapshots, &mut recommendations);

        // Analyze memory usage
        Self::analyze_memory_performance(snapshots, &mut recommendations);

        // Sort by priority
        recommendations.sort_by(|a, b| b.priority.partial_cmp(&a.priority).unwrap());

        recommendations
    }

    fn calculate_average_cpu(snapshots: &[SystemPerformanceSnapshot]) -> Option<f32> {
        if snapshots.is_empty() {
            return None;
        }

        let total: f32 = snapshots.iter().map(|s| s.cpu_usage_percent).sum();
        Some(total / snapshots.len() as f32)
    }

    fn analyze_codec_performance(snapshots: &[SystemPerformanceSnapshot], recommendations: &mut Vec<OptimizationRecommendation>) {
        for snapshot in snapshots.iter().rev().take(10) { // Look at recent snapshots
            for codec_metric in &snapshot.codec_metrics {
                if codec_metric.frames_per_second < 50.0 && codec_metric.samples_processed > 10 {
                    recommendations.push(OptimizationRecommendation {
                        category: OptimizationCategory::Codec,
                        priority: OptimizationPriority::Medium,
                        title: format!("Slow {} Performance", codec_metric.codec_type),
                        description: format!("Codec {} is processing at {:.1} FPS. Consider GPU acceleration or algorithm optimization.",
                                           codec_metric.codec_type, codec_metric.frames_per_second),
                        estimated_improvement: "200-400% throughput increase".to_string(),
                        implementation_effort: ImplementationEffort::High,
                    });
                    break;
                }
            }
        }
    }

    fn analyze_database_performance(snapshots: &[SystemPerformanceSnapshot], recommendations: &mut Vec<OptimizationRecommendation>) {
        for snapshot in snapshots.iter().rev().take(5) {
            for db_metric in &snapshot.database_metrics {
                if db_metric.average_query_time_ms > 50.0 {
                    recommendations.push(OptimizationRecommendation {
                        category: OptimizationCategory::Database,
                        priority: OptimizationPriority::High,
                        title: format!("Slow {} Queries", db_metric.query_type),
                        description: format!("Query type '{}' averages {:.1}ms. Consider indexing, query optimization, or caching.",
                                           db_metric.query_type, db_metric.average_query_time_ms),
                        estimated_improvement: "50-80% query time reduction".to_string(),
                        implementation_effort: ImplementationEffort::Low,
                    });
                    break;
                }
            }
        }
    }

    fn analyze_memory_performance(snapshots: &[SystemPerformanceSnapshot], recommendations: &mut Vec<OptimizationRecommendation>) {
        if let Some(latest) = snapshots.last() {
            let memory_usage_percent = (latest.memory_usage_bytes as f64 /
                                      (latest.memory_usage_bytes + latest.memory_available_bytes) as f64) * 100.0;

            if memory_usage_percent > 80.0 {
                recommendations.push(OptimizationRecommendation {
                    category: OptimizationCategory::Memory,
                    priority: OptimizationPriority::High,
                    title: "High Memory Usage".to_string(),
                    description: format!("Memory usage is {:.1}%. Consider increasing memory pool sizes or optimizing object lifetimes.", memory_usage_percent),
                    estimated_improvement: "20-40% memory efficiency".to_string(),
                    implementation_effort: ImplementationEffort::Medium,
                });
            }

            // Check memory pool efficiency
            for pool_metric in &latest.memory_pool_metrics {
                if pool_metric.pool_utilization_percent > 90.0 {
                    recommendations.push(OptimizationRecommendation {
                        category: OptimizationCategory::Memory,
                        priority: OptimizationPriority::Medium,
                        title: format!("{} Pool Near Capacity", pool_metric.pool_name),
                        description: format!("Memory pool '{}' is {:.1}% utilized. Consider increasing pool size.",
                                           pool_metric.pool_name, pool_metric.pool_utilization_percent),
                        estimated_improvement: "Reduced allocation latency".to_string(),
                        implementation_effort: ImplementationEffort::Minimal,
                    });
                }
            }
        }
    }
}