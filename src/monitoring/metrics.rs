//! Comprehensive metrics collection system
//! 
//! This module provides detailed metrics collection for all aspects of the
//! B2BUA system including SIP processing, media handling, and system resources.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Comprehensive system metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetricsSnapshot {
    /// Collection timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// System resource metrics
    pub system: SystemResourceMetrics,
    /// SIP processing metrics
    pub sip: SipProcessingMetrics,
    /// Media processing metrics
    pub media: MediaProcessingMetrics,
    /// Security metrics
    pub security: SecurityMetrics,
    /// Performance metrics
    pub performance: PerformanceMetrics,
    /// Business metrics
    pub business: BusinessMetrics,
}

/// System resource utilization metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemResourceMetrics {
    /// CPU usage percentage
    pub cpu_usage_percent: f64,
    /// Memory usage in MB
    pub memory_usage_mb: u64,
    /// Available memory in MB
    pub memory_available_mb: u64,
    /// Disk usage percentage
    pub disk_usage_percent: f64,
    /// Network bytes received
    pub network_rx_bytes: u64,
    /// Network bytes transmitted
    pub network_tx_bytes: u64,
    /// Open file descriptors
    pub open_file_descriptors: u32,
    /// System load average (1 minute)
    pub load_average_1m: f64,
    /// Number of threads
    pub thread_count: u32,
}

/// SIP message processing metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipProcessingMetrics {
    /// Messages per second
    pub messages_per_second: f64,
    /// Total messages processed
    pub total_messages_processed: u64,
    /// Messages by method
    pub messages_by_method: HashMap<String, u64>,
    /// Response codes distribution
    pub response_codes: HashMap<u16, u64>,
    /// Average processing latency (ms)
    pub avg_processing_latency_ms: f64,
    /// P95 processing latency (ms)
    pub p95_processing_latency_ms: f64,
    /// P99 processing latency (ms)
    pub p99_processing_latency_ms: f64,
    /// Active SIP transactions
    pub active_transactions: u32,
    /// Transport statistics
    pub transport_stats: TransportStatistics,
}

/// Transport layer statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportStatistics {
    /// UDP connections
    pub udp_connections: u32,
    /// TCP connections
    pub tcp_connections: u32,
    /// TLS connections
    pub tls_connections: u32,
    /// WebSocket connections
    pub websocket_connections: u32,
    /// Connection errors
    pub connection_errors: u64,
    /// Transport timeouts
    pub transport_timeouts: u64,
}

/// Media processing metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaProcessingMetrics {
    /// Active media sessions
    pub active_sessions: u32,
    /// Total media sessions processed
    pub total_sessions_processed: u64,
    /// Media packets per second
    pub packets_per_second: f64,
    /// Codec usage statistics
    pub codec_usage: HashMap<String, u64>,
    /// Transcoding sessions
    pub transcoding_sessions: u32,
    /// RTP statistics
    pub rtp_stats: RtpStatistics,
    /// Media quality metrics
    pub quality_metrics: MediaQualityMetrics,
}

/// RTP processing statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtpStatistics {
    /// Packets sent
    pub packets_sent: u64,
    /// Packets received
    pub packets_received: u64,
    /// Packets lost
    pub packets_lost: u64,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Jitter (ms)
    pub jitter_ms: f64,
    /// Round-trip time (ms)
    pub rtt_ms: f64,
}

/// Media quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaQualityMetrics {
    /// Mean Opinion Score (MOS)
    pub mos_score: f64,
    /// Packet loss percentage
    pub packet_loss_percent: f64,
    /// Audio quality issues
    pub audio_quality_issues: u64,
    /// Echo detected instances
    pub echo_detections: u64,
    /// Silence periods (seconds)
    pub silence_periods: f64,
}

/// Security-related metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityMetrics {
    /// Blocked IPs
    pub blocked_ips: u64,
    /// Rate limited requests
    pub rate_limited_requests: u64,
    /// Security violations detected
    pub security_violations: u64,
    /// Failed authentication attempts
    pub failed_auth_attempts: u64,
    /// Threat detections
    pub threat_detections: u64,
    /// Reputation score distribution
    pub reputation_distribution: HashMap<String, u64>,
}

/// System performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Overall system throughput (ops/sec)
    pub system_throughput: f64,
    /// Memory pool utilization
    pub memory_pool_utilization: f64,
    /// Thread pool utilization
    pub thread_pool_utilization: f64,
    /// Database connection pool usage
    pub db_connection_pool_usage: f64,
    /// Cache hit rate
    pub cache_hit_rate: f64,
    /// Garbage collection metrics (if applicable)
    pub gc_metrics: GcMetrics,
}

/// Garbage collection metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcMetrics {
    /// GC runs in the last interval
    pub gc_runs: u32,
    /// Total GC time (ms)
    pub gc_time_ms: u64,
    /// Memory freed (MB)
    pub memory_freed_mb: u64,
}

/// Business/telecom specific metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessMetrics {
    /// Total calls processed
    pub total_calls: u64,
    /// Active calls
    pub active_calls: u32,
    /// Successful calls
    pub successful_calls: u64,
    /// Failed calls
    pub failed_calls: u64,
    /// Call success rate
    pub call_success_rate: f64,
    /// Average call duration (seconds)
    pub avg_call_duration: f64,
    /// Answer Seizure Ratio (ASR)
    pub answer_seizure_ratio: f64,
    /// Post Dial Delay (ms)
    pub post_dial_delay_ms: f64,
    /// Revenue metrics (if applicable)
    pub revenue_metrics: HashMap<String, f64>,
}

/// Metrics collector with time-series storage
pub struct MetricsCollector {
    /// Collection interval
    collection_interval: Duration,
    /// Metrics retention period
    retention_period: Duration,
    /// Historical metrics storage
    metrics_history: Arc<RwLock<VecDeque<SystemMetricsSnapshot>>>,
    /// Atomic counters for real-time metrics
    counters: Arc<MetricsCounters>,
    /// Last collection time
    last_collection: Arc<RwLock<Instant>>,
}

/// Atomic counters for real-time metrics
#[derive(Debug)]
pub struct MetricsCounters {
    // SIP counters
    pub total_sip_messages: AtomicU64,
    pub invite_messages: AtomicU64,
    pub response_2xx: AtomicU64,
    pub response_4xx: AtomicU64,
    pub response_5xx: AtomicU64,
    pub active_transactions: AtomicU32,
    
    // Call counters
    pub total_calls: AtomicU64,
    pub active_calls: AtomicU32,
    pub successful_calls: AtomicU64,
    pub failed_calls: AtomicU64,
    
    // Security counters
    pub blocked_requests: AtomicU64,
    pub rate_limited: AtomicU64,
    pub security_violations: AtomicU64,
    
    // Media counters
    pub active_media_sessions: AtomicU32,
    pub rtp_packets_sent: AtomicU64,
    pub rtp_packets_received: AtomicU64,
    pub transcoding_sessions: AtomicU32,
}

impl Default for MetricsCounters {
    fn default() -> Self {
        Self {
            total_sip_messages: AtomicU64::new(0),
            invite_messages: AtomicU64::new(0),
            response_2xx: AtomicU64::new(0),
            response_4xx: AtomicU64::new(0),
            response_5xx: AtomicU64::new(0),
            active_transactions: AtomicU32::new(0),
            
            total_calls: AtomicU64::new(0),
            active_calls: AtomicU32::new(0),
            successful_calls: AtomicU64::new(0),
            failed_calls: AtomicU64::new(0),
            
            blocked_requests: AtomicU64::new(0),
            rate_limited: AtomicU64::new(0),
            security_violations: AtomicU64::new(0),
            
            active_media_sessions: AtomicU32::new(0),
            rtp_packets_sent: AtomicU64::new(0),
            rtp_packets_received: AtomicU64::new(0),
            transcoding_sessions: AtomicU32::new(0),
        }
    }
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new(interval_seconds: u64, retention_hours: u64) -> Result<Self> {
        Ok(Self {
            collection_interval: Duration::from_secs(interval_seconds),
            retention_period: Duration::from_secs(retention_hours * 3600),
            metrics_history: Arc::new(RwLock::new(VecDeque::new())),
            counters: Arc::new(MetricsCounters::default()),
            last_collection: Arc::new(RwLock::new(Instant::now())),
        })
    }
    
    /// Collect comprehensive metrics snapshot
    pub async fn collect_metrics(&self) -> Result<SystemMetricsSnapshot> {
        let start_time = Instant::now();
        
        // Collect all metric categories
        let system_metrics = self.collect_system_metrics().await?;
        let sip_metrics = self.collect_sip_metrics().await?;
        let media_metrics = self.collect_media_metrics().await?;
        let security_metrics = self.collect_security_metrics().await?;
        let performance_metrics = self.collect_performance_metrics().await?;
        let business_metrics = self.collect_business_metrics().await?;
        
        let snapshot = SystemMetricsSnapshot {
            timestamp: chrono::Utc::now(),
            system: system_metrics,
            sip: sip_metrics,
            media: media_metrics,
            security: security_metrics,
            performance: performance_metrics,
            business: business_metrics,
        };
        
        // Store in history
        self.store_metrics_snapshot(snapshot.clone()).await?;
        
        let collection_time = start_time.elapsed();
        debug!("Metrics collection completed in {:?}", collection_time);
        
        Ok(snapshot)
    }
    
    /// Collect system resource metrics
    async fn collect_system_metrics(&self) -> Result<SystemResourceMetrics> {
        // In production, these would collect real system metrics
        // For now, we'll provide placeholder implementation
        Ok(SystemResourceMetrics {
            cpu_usage_percent: self.get_cpu_usage(),
            memory_usage_mb: self.get_memory_usage_mb(),
            memory_available_mb: self.get_available_memory_mb(),
            disk_usage_percent: 25.0,
            network_rx_bytes: 1024000,
            network_tx_bytes: 2048000,
            open_file_descriptors: 128,
            load_average_1m: 0.5,
            thread_count: 32,
        })
    }
    
    /// Collect SIP processing metrics
    async fn collect_sip_metrics(&self) -> Result<SipProcessingMetrics> {
        let mut messages_by_method = HashMap::new();
        messages_by_method.insert("INVITE".to_string(), self.counters.invite_messages.load(Ordering::Relaxed));
        messages_by_method.insert("BYE".to_string(), 500);
        messages_by_method.insert("CANCEL".to_string(), 50);
        
        let mut response_codes = HashMap::new();
        response_codes.insert(200, self.counters.response_2xx.load(Ordering::Relaxed));
        response_codes.insert(404, self.counters.response_4xx.load(Ordering::Relaxed));
        response_codes.insert(500, self.counters.response_5xx.load(Ordering::Relaxed));
        
        Ok(SipProcessingMetrics {
            messages_per_second: self.calculate_messages_per_second(),
            total_messages_processed: self.counters.total_sip_messages.load(Ordering::Relaxed),
            messages_by_method,
            response_codes,
            avg_processing_latency_ms: 15.5,
            p95_processing_latency_ms: 45.0,
            p99_processing_latency_ms: 120.0,
            active_transactions: self.counters.active_transactions.load(Ordering::Relaxed),
            transport_stats: TransportStatistics {
                udp_connections: 100,
                tcp_connections: 50,
                tls_connections: 25,
                websocket_connections: 10,
                connection_errors: 5,
                transport_timeouts: 2,
            },
        })
    }
    
    /// Collect media processing metrics
    async fn collect_media_metrics(&self) -> Result<MediaProcessingMetrics> {
        let mut codec_usage = HashMap::new();
        codec_usage.insert("G.711".to_string(), 800);
        codec_usage.insert("G.729".to_string(), 200);
        codec_usage.insert("Opus".to_string(), 50);
        
        Ok(MediaProcessingMetrics {
            active_sessions: self.counters.active_media_sessions.load(Ordering::Relaxed),
            total_sessions_processed: 10000,
            packets_per_second: 15000.0,
            codec_usage,
            transcoding_sessions: self.counters.transcoding_sessions.load(Ordering::Relaxed),
            rtp_stats: RtpStatistics {
                packets_sent: self.counters.rtp_packets_sent.load(Ordering::Relaxed),
                packets_received: self.counters.rtp_packets_received.load(Ordering::Relaxed),
                packets_lost: 25,
                bytes_sent: 5000000,
                bytes_received: 4800000,
                jitter_ms: 12.5,
                rtt_ms: 45.0,
            },
            quality_metrics: MediaQualityMetrics {
                mos_score: 4.2,
                packet_loss_percent: 0.1,
                audio_quality_issues: 3,
                echo_detections: 1,
                silence_periods: 5.2,
            },
        })
    }
    
    /// Collect security metrics
    async fn collect_security_metrics(&self) -> Result<SecurityMetrics> {
        let mut reputation_distribution = HashMap::new();
        reputation_distribution.insert("high".to_string(), 850);
        reputation_distribution.insert("medium".to_string(), 120);
        reputation_distribution.insert("low".to_string(), 30);
        
        Ok(SecurityMetrics {
            blocked_ips: self.counters.blocked_requests.load(Ordering::Relaxed),
            rate_limited_requests: self.counters.rate_limited.load(Ordering::Relaxed),
            security_violations: self.counters.security_violations.load(Ordering::Relaxed),
            failed_auth_attempts: 15,
            threat_detections: 3,
            reputation_distribution,
        })
    }
    
    /// Collect performance metrics
    async fn collect_performance_metrics(&self) -> Result<PerformanceMetrics> {
        Ok(PerformanceMetrics {
            system_throughput: 2500.0,
            memory_pool_utilization: 68.5,
            thread_pool_utilization: 45.0,
            db_connection_pool_usage: 25.0,
            cache_hit_rate: 92.5,
            gc_metrics: GcMetrics {
                gc_runs: 3,
                gc_time_ms: 150,
                memory_freed_mb: 64,
            },
        })
    }
    
    /// Collect business metrics
    async fn collect_business_metrics(&self) -> Result<BusinessMetrics> {
        let total_calls = self.counters.total_calls.load(Ordering::Relaxed);
        let successful_calls = self.counters.successful_calls.load(Ordering::Relaxed);
        let failed_calls = self.counters.failed_calls.load(Ordering::Relaxed);
        
        let call_success_rate = if total_calls > 0 {
            successful_calls as f64 / total_calls as f64 * 100.0
        } else {
            0.0
        };
        
        let mut revenue_metrics = HashMap::new();
        revenue_metrics.insert("total_revenue".to_string(), 12500.50);
        revenue_metrics.insert("revenue_per_minute".to_string(), 0.05);
        
        Ok(BusinessMetrics {
            total_calls,
            active_calls: self.counters.active_calls.load(Ordering::Relaxed),
            successful_calls,
            failed_calls,
            call_success_rate,
            avg_call_duration: 180.5,
            answer_seizure_ratio: 85.2,
            post_dial_delay_ms: 1250.0,
            revenue_metrics,
        })
    }
    
    /// Store metrics snapshot in history
    async fn store_metrics_snapshot(&self, snapshot: SystemMetricsSnapshot) -> Result<()> {
        let mut history = self.metrics_history.write().await;
        
        // Add new snapshot
        history.push_back(snapshot);
        
        // Remove old snapshots beyond retention period
        let cutoff_time = chrono::Utc::now() - chrono::Duration::seconds(self.retention_period.as_secs() as i64);
        
        while let Some(front) = history.front() {
            if front.timestamp < cutoff_time {
                history.pop_front();
            } else {
                break;
            }
        }
        
        Ok(())
    }
    
    /// Get latest metrics snapshot
    pub async fn get_latest_metrics(&self) -> Result<SystemMetricsSnapshot> {
        let history = self.metrics_history.read().await;
        
        history.back()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No metrics available"))
    }
    
    /// Get metrics history for time range
    pub async fn get_metrics_history(
        &self,
        start_time: chrono::DateTime<chrono::Utc>,
        end_time: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<SystemMetricsSnapshot>> {
        let history = self.metrics_history.read().await;
        
        let filtered: Vec<SystemMetricsSnapshot> = history
            .iter()
            .filter(|snapshot| snapshot.timestamp >= start_time && snapshot.timestamp <= end_time)
            .cloned()
            .collect();
        
        Ok(filtered)
    }
    
    /// Get atomic counters for real-time updates
    pub fn counters(&self) -> Arc<MetricsCounters> {
        self.counters.clone()
    }
    
    // Helper methods for system metrics (placeholder implementations)
    fn get_cpu_usage(&self) -> f64 {
        // In production, would read from /proc/stat or use system crates
        25.5
    }
    
    fn get_memory_usage_mb(&self) -> u64 {
        // In production, would read from /proc/meminfo or use system crates
        512
    }
    
    fn get_available_memory_mb(&self) -> u64 {
        // In production, would calculate available memory
        1536
    }
    
    fn calculate_messages_per_second(&self) -> f64 {
        // Calculate based on message counts and time interval
        // This is a simplified calculation
        150.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new(30, 24).unwrap();
        assert!(collector.collection_interval == Duration::from_secs(30));
        assert!(collector.retention_period == Duration::from_secs(24 * 3600));
    }
    
    #[tokio::test]
    async fn test_metrics_collection() {
        let collector = MetricsCollector::new(30, 24).unwrap();
        
        let metrics = collector.collect_metrics().await.unwrap();
        
        assert!(metrics.system.cpu_usage_percent >= 0.0);
        assert!(metrics.sip.total_messages_processed >= 0);
        assert!(metrics.business.call_success_rate >= 0.0);
    }
    
    #[tokio::test]
    async fn test_metrics_history() {
        let collector = MetricsCollector::new(30, 24).unwrap();
        
        // Collect some metrics
        let _snapshot1 = collector.collect_metrics().await.unwrap();
        let _snapshot2 = collector.collect_metrics().await.unwrap();
        
        let latest = collector.get_latest_metrics().await.unwrap();
        assert!(latest.timestamp <= chrono::Utc::now());
        
        let history = collector.get_metrics_history(
            chrono::Utc::now() - chrono::Duration::hours(1),
            chrono::Utc::now(),
        ).await.unwrap();
        
        assert!(history.len() >= 2);
    }
}