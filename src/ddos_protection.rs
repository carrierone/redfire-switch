//! DDoS Protection and Traffic Shaping Service
//! Advanced protection against distributed denial of service attacks

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DDoSProtectionConfig {
    pub enabled: bool,
    pub detection_window_seconds: u64,
    pub max_requests_per_second: u32,
    pub max_concurrent_connections: u32,
    pub burst_allowance: u32,
    pub blacklist_duration_minutes: u32,
    pub whitelist_subnets: Vec<String>,
    pub geo_blocking: GeoBlockingConfig,
    pub traffic_shaping: TrafficShapingConfig,
    pub anomaly_detection: AnomalyDetectionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoBlockingConfig {
    pub enabled: bool,
    pub blocked_countries: Vec<String>,
    pub allowed_countries: Vec<String>,
    pub block_tor_exits: bool,
    pub block_vpn_providers: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrafficShapingConfig {
    pub enabled: bool,
    pub bandwidth_limit_mbps: u32,
    pub priority_queues: HashMap<String, QueueConfig>,
    pub connection_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    pub priority: u8,             // 0-255, higher is better priority
    pub bandwidth_percentage: u8, // 0-100
    pub max_queue_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyDetectionConfig {
    pub enabled: bool,
    pub baseline_window_minutes: u32,
    pub deviation_threshold: f64, // Standard deviations from baseline
    pub min_samples_required: u32,
    pub adaptive_thresholds: bool,
}

impl Default for DDoSProtectionConfig {
    fn default() -> Self {
        let mut priority_queues = HashMap::new();
        priority_queues.insert(
            "emergency".to_string(),
            QueueConfig {
                priority: 255,
                bandwidth_percentage: 20,
                max_queue_size: 1000,
            },
        );
        priority_queues.insert(
            "sip_signaling".to_string(),
            QueueConfig {
                priority: 200,
                bandwidth_percentage: 30,
                max_queue_size: 5000,
            },
        );
        priority_queues.insert(
            "rtp_media".to_string(),
            QueueConfig {
                priority: 180,
                bandwidth_percentage: 40,
                max_queue_size: 10000,
            },
        );
        priority_queues.insert(
            "management".to_string(),
            QueueConfig {
                priority: 100,
                bandwidth_percentage: 10,
                max_queue_size: 1000,
            },
        );

        Self {
            enabled: true,
            detection_window_seconds: 60,
            max_requests_per_second: 100,
            max_concurrent_connections: 10000,
            burst_allowance: 50,
            blacklist_duration_minutes: 60,
            whitelist_subnets: vec![
                "127.0.0.0/8".to_string(),
                "10.0.0.0/8".to_string(),
                "172.16.0.0/12".to_string(),
                "192.168.0.0/16".to_string(),
            ],
            geo_blocking: GeoBlockingConfig {
                enabled: false,
                blocked_countries: vec![],
                allowed_countries: vec![],
                block_tor_exits: false,
                block_vpn_providers: false,
            },
            traffic_shaping: TrafficShapingConfig {
                enabled: true,
                bandwidth_limit_mbps: 1000,
                priority_queues,
                connection_timeout_seconds: 30,
            },
            anomaly_detection: AnomalyDetectionConfig {
                enabled: true,
                baseline_window_minutes: 60,
                deviation_threshold: 3.0,
                min_samples_required: 100,
                adaptive_thresholds: true,
            },
        }
    }
}

pub struct DDoSProtectionService {
    config: DDoSProtectionConfig,
    connection_tracker: Arc<RwLock<ConnectionTracker>>,
    traffic_analyzer: Arc<RwLock<TrafficAnalyzer>>,
    blacklist: Arc<RwLock<HashMap<IpAddr, BlacklistEntry>>>,
    whitelist: Arc<RwLock<Vec<ipnetwork::IpNetwork>>>,
    statistics: Arc<RwLock<ProtectionStatistics>>,
}

#[derive(Debug)]
struct ConnectionTracker {
    active_connections: HashMap<IpAddr, u32>,
    request_windows: HashMap<IpAddr, VecDeque<Instant>>,
    burst_trackers: HashMap<IpAddr, BurstTracker>,
}

#[derive(Debug)]
struct BurstTracker {
    requests_in_burst: u32,
    burst_start: Instant,
    last_request: Instant,
}

#[derive(Debug)]
struct TrafficAnalyzer {
    baseline_metrics: HashMap<String, BaselineMetric>,
    current_metrics: HashMap<String, f64>,
    anomaly_scores: VecDeque<(DateTime<Utc>, f64)>,
}

#[derive(Debug)]
struct BaselineMetric {
    samples: VecDeque<f64>,
    mean: f64,
    std_dev: f64,
    last_updated: DateTime<Utc>,
}

#[derive(Debug)]
struct BlacklistEntry {
    reason: BlacklistReason,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    violation_count: u32,
}

#[derive(Debug, Clone)]
pub enum BlacklistReason {
    RateLimitExceeded,
    DDoSAttackDetected,
    AnomalousTraffic,
    GeoBlocked,
    ManualBlacklist,
}

#[derive(Debug, Default)]
pub struct ProtectionStatistics {
    pub total_requests: u64,
    pub blocked_requests: u64,
    pub rate_limited_requests: u64,
    pub blacklisted_ips: u64,
    pub anomalies_detected: u64,
    pub concurrent_connections: u32,
    pub bandwidth_usage_mbps: f64,
}

impl DDoSProtectionService {
    pub async fn new(config: DDoSProtectionConfig) -> Result<Self> {
        let whitelist = Self::parse_whitelist_subnets(&config.whitelist_subnets)?;

        Ok(Self {
            config,
            connection_tracker: Arc::new(RwLock::new(ConnectionTracker {
                active_connections: HashMap::new(),
                request_windows: HashMap::new(),
                burst_trackers: HashMap::new(),
            })),
            traffic_analyzer: Arc::new(RwLock::new(TrafficAnalyzer {
                baseline_metrics: HashMap::new(),
                current_metrics: HashMap::new(),
                anomaly_scores: VecDeque::new(),
            })),
            blacklist: Arc::new(RwLock::new(HashMap::new())),
            whitelist: Arc::new(RwLock::new(whitelist)),
            statistics: Arc::new(RwLock::new(ProtectionStatistics::default())),
        })
    }

    fn parse_whitelist_subnets(subnets: &[String]) -> Result<Vec<ipnetwork::IpNetwork>> {
        let mut networks = Vec::new();
        for subnet in subnets {
            let network = subnet
                .parse::<ipnetwork::IpNetwork>()
                .map_err(|e| anyhow!("Invalid subnet '{}': {}", subnet, e))?;
            networks.push(network);
        }
        Ok(networks)
    }

    /// Check if a request from the given IP should be allowed
    pub async fn should_allow_request(
        &self,
        ip: IpAddr,
        request_size: usize,
    ) -> Result<ProtectionDecision> {
        if !self.config.enabled {
            return Ok(ProtectionDecision::Allow);
        }

        // Increment total requests counter
        {
            let mut stats = self.statistics.write().await;
            stats.total_requests += 1;
        }

        // Check whitelist first
        if self.is_whitelisted(ip).await {
            return Ok(ProtectionDecision::Allow);
        }

        // Check blacklist
        if let Some(reason) = self.is_blacklisted(ip).await {
            self.increment_blocked_counter().await;
            return Ok(ProtectionDecision::Block(reason));
        }

        // Check rate limits
        if let Some(reason) = self.check_rate_limits(ip).await? {
            self.increment_rate_limited_counter().await;
            return Ok(ProtectionDecision::RateLimit(reason));
        }

        // Check for anomalous traffic patterns
        if self.config.anomaly_detection.enabled {
            if let Some(anomaly_score) = self.detect_anomaly(ip, request_size).await? {
                if anomaly_score > self.config.anomaly_detection.deviation_threshold {
                    self.add_to_blacklist(ip, BlacklistReason::AnomalousTraffic)
                        .await?;
                    return Ok(ProtectionDecision::Block(BlacklistReason::AnomalousTraffic));
                }
            }
        }

        Ok(ProtectionDecision::Allow)
    }

    async fn is_whitelisted(&self, ip: IpAddr) -> bool {
        let whitelist = self.whitelist.read().await;
        whitelist.iter().any(|network| network.contains(ip))
    }

    async fn is_blacklisted(&self, ip: IpAddr) -> Option<BlacklistReason> {
        let now = Utc::now();
        let mut blacklist = self.blacklist.write().await;

        if let Some(entry) = blacklist.get(&ip) {
            if now < entry.expires_at {
                return Some(entry.reason.clone());
            } else {
                // Entry expired, remove it
                blacklist.remove(&ip);
            }
        }

        None
    }

    async fn check_rate_limits(&self, ip: IpAddr) -> Result<Option<String>> {
        let now = Instant::now();
        let mut tracker = self.connection_tracker.write().await;

        // Check request rate in sliding window
        let request_window = tracker
            .request_windows
            .entry(ip)
            .or_insert_with(VecDeque::new);

        // Remove old requests outside the window
        let window_start = now - Duration::from_secs(self.config.detection_window_seconds);
        request_window.retain(|&request_time| request_time > window_start);

        // Add current request
        request_window.push_back(now);

        // Check if rate limit exceeded
        if request_window.len()
            > self.config.max_requests_per_second as usize
                * self.config.detection_window_seconds as usize
        {
            // Potential DDoS attack detected
            self.add_to_blacklist(ip, BlacklistReason::DDoSAttackDetected)
                .await?;
            return Ok(Some("DDoS attack pattern detected".to_string()));
        }

        // Check burst limits
        let burst_tracker = tracker
            .burst_trackers
            .entry(ip)
            .or_insert_with(|| BurstTracker {
                requests_in_burst: 0,
                burst_start: now,
                last_request: now,
            });

        // Reset burst if there's been a gap
        if now.duration_since(burst_tracker.last_request) > Duration::from_secs(5) {
            burst_tracker.requests_in_burst = 1;
            burst_tracker.burst_start = now;
        } else {
            burst_tracker.requests_in_burst += 1;
        }

        burst_tracker.last_request = now;

        if burst_tracker.requests_in_burst > self.config.burst_allowance {
            return Ok(Some("Burst limit exceeded".to_string()));
        }

        Ok(None)
    }

    async fn detect_anomaly(&self, ip: IpAddr, request_size: usize) -> Result<Option<f64>> {
        let mut analyzer = self.traffic_analyzer.write().await;
        let now = Utc::now();

        // Update current metrics
        analyzer
            .current_metrics
            .insert("request_size".to_string(), request_size as f64);

        // Update baseline for request size
        let baseline = analyzer
            .baseline_metrics
            .entry("request_size".to_string())
            .or_insert_with(|| BaselineMetric {
                samples: VecDeque::new(),
                mean: 0.0,
                std_dev: 0.0,
                last_updated: now,
            });

        baseline.samples.push_back(request_size as f64);

        // Keep only samples within the baseline window
        let window_start = now
            - chrono::Duration::minutes(
                self.config.anomaly_detection.baseline_window_minutes as i64,
            );
        baseline
            .samples
            .retain(|_| baseline.last_updated > window_start);

        if baseline.samples.len() >= self.config.anomaly_detection.min_samples_required as usize {
            // Calculate mean and standard deviation
            let sum: f64 = baseline.samples.iter().sum();
            baseline.mean = sum / baseline.samples.len() as f64;

            let variance: f64 = baseline
                .samples
                .iter()
                .map(|value| {
                    let diff = value - baseline.mean;
                    diff * diff
                })
                .sum::<f64>()
                / baseline.samples.len() as f64;

            baseline.std_dev = variance.sqrt();

            // Calculate anomaly score (z-score)
            if baseline.std_dev > 0.0 {
                let z_score = (request_size as f64 - baseline.mean) / baseline.std_dev;

                // Store anomaly score
                analyzer.anomaly_scores.push_back((now, z_score.abs()));

                // Keep only recent anomaly scores
                analyzer
                    .anomaly_scores
                    .retain(|(timestamp, _)| *timestamp > window_start);

                debug!(
                    "Anomaly detection for {}: z_score={:.2}, threshold={:.2}",
                    ip,
                    z_score.abs(),
                    self.config.anomaly_detection.deviation_threshold
                );

                return Ok(Some(z_score.abs()));
            }
        }

        Ok(None)
    }

    async fn add_to_blacklist(&self, ip: IpAddr, reason: BlacklistReason) -> Result<()> {
        let now = Utc::now();
        let expires_at =
            now + chrono::Duration::minutes(self.config.blacklist_duration_minutes as i64);

        let mut blacklist = self.blacklist.write().await;

        let violation_count = if let Some(existing) = blacklist.get(&ip) {
            existing.violation_count + 1
        } else {
            1
        };

        blacklist.insert(
            ip,
            BlacklistEntry {
                reason: reason.clone(),
                created_at: now,
                expires_at,
                violation_count,
            },
        );

        warn!(
            "IP {} blacklisted for {:?} (violation #{}) until {}",
            ip, reason, violation_count, expires_at
        );

        // Update statistics
        let mut stats = self.statistics.write().await;
        stats.blacklisted_ips += 1;

        Ok(())
    }

    async fn increment_blocked_counter(&self) {
        let mut stats = self.statistics.write().await;
        stats.blocked_requests += 1;
    }

    async fn increment_rate_limited_counter(&self) {
        let mut stats = self.statistics.write().await;
        stats.rate_limited_requests += 1;
    }

    /// Register a new connection from the given IP
    pub async fn register_connection(&self, ip: IpAddr) -> Result<bool> {
        if !self.config.enabled {
            return Ok(true);
        }

        let mut tracker = self.connection_tracker.write().await;
        // Check global connection limit first
        let total_connections: u32 = tracker.active_connections.values().sum();
        if total_connections >= self.config.max_concurrent_connections {
            warn!("Global connection limit reached: {}", total_connections);
            return Ok(false);
        }

        let current_connections = tracker.active_connections.entry(ip).or_insert(0);

        *current_connections += 1;

        // Update statistics
        let mut stats = self.statistics.write().await;
        stats.concurrent_connections = total_connections + 1;

        debug!(
            "Connection registered for {}: {} active connections",
            ip, current_connections
        );
        Ok(true)
    }

    /// Unregister a connection from the given IP
    pub async fn unregister_connection(&self, ip: IpAddr) {
        let mut tracker = self.connection_tracker.write().await;
        if let Some(connections) = tracker.active_connections.get_mut(&ip) {
            *connections = connections.saturating_sub(1);
            if *connections == 0 {
                tracker.active_connections.remove(&ip);
            }
        }

        // Update statistics
        let mut stats = self.statistics.write().await;
        stats.concurrent_connections = stats.concurrent_connections.saturating_sub(1);
    }

    /// Get current protection statistics
    pub async fn get_statistics(&self) -> ProtectionStatistics {
        let stats = self.statistics.read().await;
        ProtectionStatistics {
            total_requests: stats.total_requests,
            blocked_requests: stats.blocked_requests,
            rate_limited_requests: stats.rate_limited_requests,
            blacklisted_ips: stats.blacklisted_ips,
            anomalies_detected: stats.anomalies_detected,
            concurrent_connections: stats.concurrent_connections,
            bandwidth_usage_mbps: stats.bandwidth_usage_mbps,
        }
    }

    /// Clean up expired entries and update metrics
    pub async fn maintenance_cycle(&self) -> Result<()> {
        self.cleanup_expired_blacklist_entries().await?;
        self.cleanup_old_tracking_data().await?;
        self.update_baseline_metrics().await?;

        debug!("DDoS protection maintenance cycle completed");
        Ok(())
    }

    async fn cleanup_expired_blacklist_entries(&self) -> Result<()> {
        let now = Utc::now();
        let mut blacklist = self.blacklist.write().await;
        let initial_count = blacklist.len();

        blacklist.retain(|_, entry| now < entry.expires_at);

        let removed_count = initial_count - blacklist.len();
        if removed_count > 0 {
            debug!("Removed {} expired blacklist entries", removed_count);
        }

        Ok(())
    }

    async fn cleanup_old_tracking_data(&self) -> Result<()> {
        let mut tracker = self.connection_tracker.write().await;
        let cutoff = Instant::now() - Duration::from_secs(self.config.detection_window_seconds * 2);

        // Clean up old request windows
        for (_, window) in tracker.request_windows.iter_mut() {
            window.retain(|&request_time| request_time > cutoff);
        }
        tracker
            .request_windows
            .retain(|_, window| !window.is_empty());

        // Clean up old burst trackers
        tracker
            .burst_trackers
            .retain(|_, burst| burst.last_request > cutoff);

        Ok(())
    }

    async fn update_baseline_metrics(&self) -> Result<()> {
        let mut analyzer = self.traffic_analyzer.write().await;
        let now = Utc::now();

        for (_, baseline) in analyzer.baseline_metrics.iter_mut() {
            baseline.last_updated = now;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ProtectionDecision {
    Allow,
    Block(BlacklistReason),
    RateLimit(String),
}
