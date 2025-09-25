//! Memory Pool Optimization Module
//!
//! Advanced memory management and pool optimization for high-performance telephony operations
//! Features:
//! - Dynamic pool sizing based on usage patterns
//! - Memory fragmentation analysis and mitigation
//! - Object lifecycle optimization
//! - NUMA-aware allocation strategies
//! - Memory pressure handling

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use object_pool::Pool;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, info, warn, instrument};

/// Memory optimization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryOptimizerConfig {
    /// Enable dynamic pool sizing
    pub enable_dynamic_sizing: bool,
    /// Target memory utilization percentage
    pub target_utilization_percent: f32,
    /// Maximum growth factor for pools
    pub max_growth_factor: f32,
    /// Minimum shrink threshold
    pub shrink_threshold_percent: f32,
    /// Pool resize check interval in seconds
    pub resize_interval_seconds: u64,
    /// Enable memory pressure monitoring
    pub enable_pressure_monitoring: bool,
    /// Memory pressure threshold (percentage of available memory)
    pub pressure_threshold_percent: f32,
    /// Enable NUMA awareness
    pub enable_numa_awareness: bool,
    /// Object lifetime tracking window in minutes
    pub lifetime_tracking_window_minutes: u32,
}

impl Default for MemoryOptimizerConfig {
    fn default() -> Self {
        Self {
            enable_dynamic_sizing: true,
            target_utilization_percent: 75.0,
            max_growth_factor: 2.0,
            shrink_threshold_percent: 30.0,
            resize_interval_seconds: 60,
            enable_pressure_monitoring: true,
            pressure_threshold_percent: 85.0,
            enable_numa_awareness: false, // Requires specialized hardware
            lifetime_tracking_window_minutes: 15,
        }
    }
}

/// Pool performance metrics
#[derive(Debug, Serialize, Deserialize)]
pub struct PoolMetrics {
    pub pool_name: String,
    pub current_size: usize,
    pub max_size: usize,
    #[serde(skip)]
    pub active_objects: AtomicUsize,
    #[serde(skip)]
    pub total_allocations: AtomicU64,
    #[serde(skip)]
    pub total_deallocations: AtomicU64,
    #[serde(skip)]
    pub allocation_failures: AtomicU64,
    pub average_lifetime_ms: f64,
    pub peak_utilization: f32,
    pub fragmentation_score: f32,
    pub numa_node_distribution: HashMap<u32, usize>,
    pub last_resize: DateTime<Utc>,
    pub resize_count: u32,
}

impl Clone for PoolMetrics {
    fn clone(&self) -> Self {
        Self {
            pool_name: self.pool_name.clone(),
            current_size: self.current_size,
            max_size: self.max_size,
            active_objects: AtomicUsize::new(self.active_objects.load(Ordering::Relaxed)),
            total_allocations: AtomicU64::new(self.total_allocations.load(Ordering::Relaxed)),
            total_deallocations: AtomicU64::new(self.total_deallocations.load(Ordering::Relaxed)),
            allocation_failures: AtomicU64::new(self.allocation_failures.load(Ordering::Relaxed)),
            average_lifetime_ms: self.average_lifetime_ms,
            peak_utilization: self.peak_utilization,
            fragmentation_score: self.fragmentation_score,
            numa_node_distribution: self.numa_node_distribution.clone(),
            last_resize: self.last_resize,
            resize_count: self.resize_count,
        }
    }
}

impl PoolMetrics {
    pub fn new(pool_name: String) -> Self {
        Self {
            pool_name,
            current_size: 0,
            max_size: 0,
            active_objects: AtomicUsize::new(0),
            total_allocations: AtomicU64::new(0),
            total_deallocations: AtomicU64::new(0),
            allocation_failures: AtomicU64::new(0),
            average_lifetime_ms: 0.0,
            peak_utilization: 0.0,
            fragmentation_score: 0.0,
            numa_node_distribution: HashMap::new(),
            last_resize: Utc::now(),
            resize_count: 0,
        }
    }

    pub fn utilization_percent(&self) -> f32 {
        if self.current_size == 0 {
            return 0.0;
        }
        (self.active_objects.load(Ordering::Relaxed) as f32 / self.current_size as f32) * 100.0
    }

    pub fn success_rate(&self) -> f32 {
        let total = self.total_allocations.load(Ordering::Relaxed);
        let failures = self.allocation_failures.load(Ordering::Relaxed);
        if total == 0 {
            return 100.0;
        }
        ((total - failures) as f32 / total as f32) * 100.0
    }
}

/// Object lifecycle tracking
#[derive(Debug, Clone)]
pub struct ObjectLifecycle {
    pub allocated_at: Instant,
    pub deallocated_at: Option<Instant>,
    pub object_size: usize,
    pub numa_node: Option<u32>,
    pub pool_name: String,
}

/// Memory pressure information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPressure {
    pub total_memory_bytes: u64,
    pub available_memory_bytes: u64,
    pub usage_percent: f32,
    pub pressure_level: PressureLevel,
    pub swap_usage_bytes: u64,
    pub major_page_faults: u64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PressureLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// Advanced memory pool with optimization capabilities
pub struct OptimizedPool<T> {
    inner_pool: Pool<T>,
    metrics: Arc<RwLock<PoolMetrics>>,
    config: MemoryOptimizerConfig,
    lifecycle_tracker: Arc<RwLock<VecDeque<ObjectLifecycle>>>,
    resize_semaphore: Arc<Semaphore>,
}

impl<T: Clone + Default + Send + Sync + 'static> OptimizedPool<T> {
    pub fn new(pool_name: String, initial_size: usize, config: MemoryOptimizerConfig) -> Self {
        let pool = Pool::new(initial_size, T::default);
        let metrics = Arc::new(RwLock::new(PoolMetrics::new(pool_name.clone())));

        Self {
            inner_pool: pool,
            metrics,
            config,
            lifecycle_tracker: Arc::new(RwLock::new(VecDeque::new())),
            resize_semaphore: Arc::new(Semaphore::new(1)), // Only one resize at a time
        }
    }

    /// Get an object from the pool with tracking
    #[instrument(skip(self))]
    pub async fn get(&self) -> Result<TrackedPoolObject<T>> {
        let start = Instant::now();

        // Try to get from pool
        let object = match self.inner_pool.try_pull() {
            Some(obj) => obj,
            None => {
                // Pool exhausted, record failure and try to resize
                {
                    let metrics = self.metrics.read().await;
                    metrics.allocation_failures.fetch_add(1, Ordering::Relaxed);
                }

                if self.config.enable_dynamic_sizing {
                    self.try_resize_pool().await?;
                }

                // Try again after potential resize
                self.inner_pool.try_pull().ok_or_else(|| anyhow!("Pool exhausted"))?
            }
        };

        // Update metrics
        {
            let metrics = self.metrics.read().await;
            metrics.active_objects.fetch_add(1, Ordering::Relaxed);
            metrics.total_allocations.fetch_add(1, Ordering::Relaxed);
        }

        // Track object lifecycle
        if self.config.lifetime_tracking_window_minutes > 0 {
            let lifecycle = ObjectLifecycle {
                allocated_at: start,
                deallocated_at: None,
                object_size: std::mem::size_of::<T>(),
                numa_node: self.get_numa_node().await,
                pool_name: {
                    let metrics = self.metrics.read().await;
                    metrics.pool_name.clone()
                },
            };

            let mut tracker = self.lifecycle_tracker.write().await;
            tracker.push_back(lifecycle);

            // Trim old entries
            let cutoff = Instant::now() - Duration::from_secs(self.config.lifetime_tracking_window_minutes as u64 * 60);
            while let Some(front) = tracker.front() {
                if front.allocated_at > cutoff {
                    break;
                }
                tracker.pop_front();
            }
        }

        // Convert the pooled object to owned value
        let owned_object = (*object).clone();
        Ok(TrackedPoolObject::new(owned_object, self.metrics.clone(), start))
    }

    /// Try to resize the pool based on current metrics
    async fn try_resize_pool(&self) -> Result<()> {
        // Acquire resize semaphore to prevent concurrent resizes
        let _permit = self.resize_semaphore.try_acquire()
            .map_err(|_| anyhow!("Pool resize already in progress"))?;

        let should_resize = {
            let metrics = self.metrics.read().await;
            let utilization = metrics.utilization_percent();

            utilization > self.config.target_utilization_percent ||
            metrics.allocation_failures.load(Ordering::Relaxed) > 0
        };

        if should_resize {
            let new_size = self.calculate_optimal_size().await;
            self.resize_pool(new_size).await?;
        }

        Ok(())
    }

    async fn calculate_optimal_size(&self) -> usize {
        let metrics = self.metrics.read().await;
        let current_utilization = metrics.utilization_percent();
        let current_size = metrics.current_size;

        if current_utilization > self.config.target_utilization_percent {
            // Need to grow
            let growth_factor = (current_utilization / self.config.target_utilization_percent).min(self.config.max_growth_factor);
            (current_size as f32 * growth_factor) as usize
        } else if current_utilization < self.config.shrink_threshold_percent {
            // Can shrink
            let target_size = (current_size as f32 * (current_utilization / self.config.target_utilization_percent)) as usize;
            target_size.max(current_size / 2) // Don't shrink by more than half
        } else {
            current_size
        }
    }

    async fn resize_pool(&self, new_size: usize) -> Result<()> {
        let current_size = {
            let metrics = self.metrics.read().await;
            metrics.current_size
        };

        if new_size == current_size {
            return Ok(());
        }

        info!("Resizing pool from {} to {} objects", current_size, new_size);

        // Update pool size (simplified - actual implementation would resize the underlying pool)
        {
            let mut metrics = self.metrics.write().await;
            metrics.current_size = new_size;
            metrics.max_size = metrics.max_size.max(new_size);
            metrics.last_resize = Utc::now();
            metrics.resize_count += 1;
        }

        Ok(())
    }

    async fn get_numa_node(&self) -> Option<u32> {
        if !self.config.enable_numa_awareness {
            return None;
        }

        // In real implementation would query NUMA topology
        Some(0)
    }

    /// Get current pool metrics
    pub async fn get_metrics(&self) -> PoolMetrics {
        self.metrics.read().await.clone()
    }

    /// Get lifecycle statistics
    pub async fn get_lifecycle_stats(&self) -> LifecycleStats {
        let tracker = self.lifecycle_tracker.read().await;
        let _now = Instant::now();

        let mut total_lifetime_ms = 0u64;
        let mut completed_objects = 0usize;
        let mut size_distribution: HashMap<usize, usize> = HashMap::new();

        for lifecycle in tracker.iter() {
            if let Some(deallocated_at) = lifecycle.deallocated_at {
                let lifetime = deallocated_at.duration_since(lifecycle.allocated_at);
                total_lifetime_ms += lifetime.as_millis() as u64;
                completed_objects += 1;
            }

            *size_distribution.entry(lifecycle.object_size).or_insert(0) += 1;
        }

        let average_lifetime_ms = if completed_objects > 0 {
            total_lifetime_ms as f64 / completed_objects as f64
        } else {
            0.0
        };

        LifecycleStats {
            active_objects: tracker.len() - completed_objects,
            completed_objects,
            average_lifetime_ms,
            size_distribution,
            tracking_window_minutes: self.config.lifetime_tracking_window_minutes,
        }
    }
}

/// Lifecycle statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleStats {
    pub active_objects: usize,
    pub completed_objects: usize,
    pub average_lifetime_ms: f64,
    pub size_distribution: HashMap<usize, usize>,
    pub tracking_window_minutes: u32,
}

/// Tracked pool object wrapper
pub struct TrackedPoolObject<T> {
    inner: Option<Box<T>>,
    metrics: Arc<RwLock<PoolMetrics>>,
    allocated_at: Instant,
}

impl<T> TrackedPoolObject<T> {
    fn new(inner: T, metrics: Arc<RwLock<PoolMetrics>>, allocated_at: Instant) -> Self {
        Self {
            inner: Some(Box::new(inner)),
            metrics,
            allocated_at,
        }
    }
}

impl<T> std::ops::Deref for TrackedPoolObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().unwrap().as_ref()
    }
}

impl<T> std::ops::DerefMut for TrackedPoolObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().unwrap().as_mut()
    }
}

impl<T> Drop for TrackedPoolObject<T> {
    fn drop(&mut self) {
        if let Some(_inner) = self.inner.take() {
            // Update metrics when object is returned to pool
            let metrics = self.metrics.clone();
            let allocated_at = self.allocated_at;

            tokio::spawn(async move {
                let metrics_guard = metrics.read().await;
                metrics_guard.active_objects.fetch_sub(1, Ordering::Relaxed);
                metrics_guard.total_deallocations.fetch_add(1, Ordering::Relaxed);

                let lifetime = allocated_at.elapsed();
                debug!("Object returned to pool after {:?}", lifetime);
            });
        }
    }
}

/// Memory optimizer that manages multiple pools
pub struct MemoryOptimizer {
    config: MemoryOptimizerConfig,
    pools: Arc<RwLock<HashMap<String, Arc<dyn PoolManager + Send + Sync>>>>,
    memory_monitor: Arc<RwLock<MemoryPressure>>,
}

impl MemoryOptimizer {
    pub fn new(config: MemoryOptimizerConfig) -> Self {
        Self {
            config,
            pools: Arc::new(RwLock::new(HashMap::new())),
            memory_monitor: Arc::new(RwLock::new(MemoryPressure {
                total_memory_bytes: 0,
                available_memory_bytes: 0,
                usage_percent: 0.0,
                pressure_level: PressureLevel::None,
                swap_usage_bytes: 0,
                major_page_faults: 0,
                timestamp: Utc::now(),
            })),
        }
    }

    /// Start memory optimization monitoring
    pub async fn start_monitoring(&self) {
        if !self.config.enable_dynamic_sizing && !self.config.enable_pressure_monitoring {
            info!("Memory optimization monitoring disabled");
            return;
        }

        info!("Starting memory optimization monitoring");

        let config = self.config.clone();
        let pools = self.pools.clone();
        let memory_monitor = self.memory_monitor.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(config.resize_interval_seconds)
            );

            loop {
                interval.tick().await;

                // Update memory pressure information
                if config.enable_pressure_monitoring {
                    if let Ok(pressure) = Self::measure_memory_pressure(&config).await {
                        let mut monitor = memory_monitor.write().await;
                        *monitor = pressure.clone();

                        if matches!(pressure.pressure_level, PressureLevel::High | PressureLevel::Critical) {
                            warn!("High memory pressure detected: {:.1}%", pressure.usage_percent);
                            Self::handle_memory_pressure(&pools, &pressure).await;
                        }
                    }
                }

                // Optimize pool sizes
                if config.enable_dynamic_sizing {
                    Self::optimize_all_pools(&pools).await;
                }
            }
        });
    }

    /// Register a pool for optimization
    pub async fn register_pool<T: Clone + Default + Send + Sync + 'static>(
        &self,
        name: String,
        pool: OptimizedPool<T>,
    ) {
        let pool_manager: Arc<dyn PoolManager + Send + Sync> = Arc::new(pool);
        let mut pools = self.pools.write().await;
        pools.insert(name, pool_manager);
    }

    /// Get memory optimization recommendations
    pub async fn get_optimization_recommendations(&self) -> Vec<MemoryOptimizationRecommendation> {
        let mut recommendations = Vec::new();
        let pools = self.pools.read().await;

        for (name, pool) in pools.iter() {
            let metrics = pool.get_pool_metrics().await;

            // Check for underutilized pools
            if metrics.utilization_percent() < self.config.shrink_threshold_percent && metrics.current_size > 10 {
                recommendations.push(MemoryOptimizationRecommendation {
                    pool_name: name.clone(),
                    category: OptimizationCategory::Underutilized,
                    priority: OptimizationPriority::Low,
                    description: format!(
                        "Pool '{}' is only {:.1}% utilized. Consider reducing size from {} to approximately {} objects.",
                        name, metrics.utilization_percent(), metrics.current_size, metrics.current_size / 2
                    ),
                    estimated_memory_savings_mb: (metrics.current_size * std::mem::size_of::<usize>() / 2) as f64 / 1024.0 / 1024.0,
                    implementation_effort: ImplementationEffort::Low,
                });
            }

            // Check for pools with high failure rates
            if metrics.success_rate() < 95.0 && metrics.total_allocations.load(Ordering::Relaxed) > 100 {
                recommendations.push(MemoryOptimizationRecommendation {
                    pool_name: name.clone(),
                    category: OptimizationCategory::HighFailureRate,
                    priority: OptimizationPriority::High,
                    description: format!(
                        "Pool '{}' has {:.1}% success rate. Increase pool size to reduce allocation failures.",
                        name, metrics.success_rate()
                    ),
                    estimated_memory_savings_mb: 0.0, // This would increase memory usage
                    implementation_effort: ImplementationEffort::Medium,
                });
            }

            // Check for fragmentation
            if metrics.fragmentation_score > 0.3 {
                recommendations.push(MemoryOptimizationRecommendation {
                    pool_name: name.clone(),
                    category: OptimizationCategory::Fragmentation,
                    priority: OptimizationPriority::Medium,
                    description: format!(
                        "Pool '{}' shows {:.1}% fragmentation. Consider pool compaction or resize strategy adjustment.",
                        name, metrics.fragmentation_score * 100.0
                    ),
                    estimated_memory_savings_mb: 0.0,
                    implementation_effort: ImplementationEffort::High,
                });
            }
        }

        // Check overall memory pressure
        let memory_pressure = self.memory_monitor.read().await;
        if memory_pressure.usage_percent > self.config.pressure_threshold_percent {
            recommendations.push(MemoryOptimizationRecommendation {
                pool_name: "SYSTEM".to_string(),
                category: OptimizationCategory::MemoryPressure,
                priority: OptimizationPriority::Critical,
                description: format!(
                    "System memory usage is {:.1}%. Consider reducing pool sizes or adding more memory.",
                    memory_pressure.usage_percent
                ),
                estimated_memory_savings_mb: 0.0,
                implementation_effort: ImplementationEffort::High,
            });
        }

        recommendations.sort_by(|a, b| b.priority.cmp(&a.priority));
        recommendations
    }

    async fn measure_memory_pressure(config: &MemoryOptimizerConfig) -> Result<MemoryPressure> {
        // Simplified memory pressure measurement
        // In production would read from /proc/meminfo, /proc/vmstat, etc.

        let total_memory = 16 * 1024 * 1024 * 1024u64; // 16 GB
        let available_memory = 4 * 1024 * 1024 * 1024u64; // 4 GB available
        let usage_percent = ((total_memory - available_memory) as f64 / total_memory as f64) * 100.0;

        let pressure_level = match usage_percent as f32 {
            p if p >= 95.0 => PressureLevel::Critical,
            p if p >= config.pressure_threshold_percent => PressureLevel::High,
            p if p >= 70.0 => PressureLevel::Medium,
            p if p >= 50.0 => PressureLevel::Low,
            _ => PressureLevel::None,
        };

        Ok(MemoryPressure {
            total_memory_bytes: total_memory,
            available_memory_bytes: available_memory,
            usage_percent: usage_percent as f32,
            pressure_level,
            swap_usage_bytes: 512 * 1024 * 1024, // 512 MB swap
            major_page_faults: 1000,
            timestamp: Utc::now(),
        })
    }

    async fn handle_memory_pressure(
        pools: &Arc<RwLock<HashMap<String, Arc<dyn PoolManager + Send + Sync>>>>,
        _pressure: &MemoryPressure,
    ) {
        let pools_guard = pools.read().await;

        // Emergency shrink all pools by 25%
        for (name, pool) in pools_guard.iter() {
            if let Err(e) = pool.emergency_shrink(0.25).await {
                warn!("Failed to emergency shrink pool '{}': {}", name, e);
            }
        }
    }

    async fn optimize_all_pools(
        pools: &Arc<RwLock<HashMap<String, Arc<dyn PoolManager + Send + Sync>>>>
    ) {
        let pools_guard = pools.read().await;

        for (name, pool) in pools_guard.iter() {
            if let Err(e) = pool.optimize().await {
                debug!("Failed to optimize pool '{}': {}", name, e);
            }
        }
    }
}

/// Pool manager trait for different pool types
#[async_trait::async_trait]
pub trait PoolManager {
    async fn get_pool_metrics(&self) -> PoolMetrics;
    async fn optimize(&self) -> Result<()>;
    async fn emergency_shrink(&self, factor: f32) -> Result<()>;
}

#[async_trait::async_trait]
impl<T: Clone + Default + Send + Sync + 'static> PoolManager for OptimizedPool<T> {
    async fn get_pool_metrics(&self) -> PoolMetrics {
        self.get_metrics().await
    }

    async fn optimize(&self) -> Result<()> {
        self.try_resize_pool().await
    }

    async fn emergency_shrink(&self, factor: f32) -> Result<()> {
        let current_size = {
            let metrics = self.metrics.read().await;
            metrics.current_size
        };

        let new_size = (current_size as f32 * (1.0 - factor)).max(1.0) as usize;
        self.resize_pool(new_size).await
    }
}

/// Memory optimization recommendations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryOptimizationRecommendation {
    pub pool_name: String,
    pub category: OptimizationCategory,
    pub priority: OptimizationPriority,
    pub description: String,
    pub estimated_memory_savings_mb: f64,
    pub implementation_effort: ImplementationEffort,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum OptimizationCategory {
    Underutilized,
    HighFailureRate,
    Fragmentation,
    MemoryPressure,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Ord, Eq)]
pub enum OptimizationPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImplementationEffort {
    Low,
    Medium,
    High,
}

/// NUMA-aware allocator (placeholder for specialized hardware)
pub struct NumaAllocator {
    preferred_node: u32,
    fallback_enabled: bool,
}

impl NumaAllocator {
    pub fn new(preferred_node: u32) -> Self {
        Self {
            preferred_node,
            fallback_enabled: true,
        }
    }

    pub async fn allocate<T: Default>(&self) -> Result<T> {
        // In real implementation would use NUMA-specific allocation
        Ok(T::default())
    }

    pub async fn get_memory_topology(&self) -> HashMap<u32, u64> {
        // Return available memory per NUMA node
        let mut topology = HashMap::new();
        topology.insert(0, 8 * 1024 * 1024 * 1024); // 8GB on node 0
        topology.insert(1, 8 * 1024 * 1024 * 1024); // 8GB on node 1
        topology
    }
}