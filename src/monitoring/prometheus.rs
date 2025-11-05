//! Prometheus metrics exporter
//!
//! This module provides Prometheus-compatible metrics export for all system metrics.

use super::{MetricsCollector, SystemMetricsSnapshot};
use anyhow::Result;
use prometheus::{
    Counter, CounterVec, Encoder, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, Opts,
    Registry, TextEncoder,
};
use std::sync::Arc;
use tracing::{debug, error};

/// Prometheus metrics exporter
pub struct PrometheusExporter {
    /// Prometheus registry
    registry: Registry,

    // System metrics
    cpu_usage: Gauge,
    memory_usage_bytes: Gauge,
    memory_available_bytes: Gauge,
    disk_usage_percent: Gauge,
    network_rx_bytes: Counter,
    network_tx_bytes: Counter,
    open_file_descriptors: Gauge,
    load_average: Gauge,
    thread_count: Gauge,

    // SIP metrics
    sip_messages_total: CounterVec,
    sip_messages_per_second: Gauge,
    sip_response_codes_total: CounterVec,
    sip_processing_latency: HistogramVec,
    sip_active_transactions: Gauge,
    sip_transport_connections: GaugeVec,
    sip_transport_errors_total: CounterVec,

    // Call metrics
    calls_total: CounterVec,
    calls_active: Gauge,
    call_success_rate: Gauge,
    call_duration_seconds: Histogram,
    answer_seizure_ratio: Gauge,
    post_dial_delay_ms: Gauge,

    // Media metrics
    media_sessions_active: Gauge,
    media_sessions_total: Counter,
    media_packets_per_second: Gauge,
    media_codec_usage: GaugeVec,
    media_transcoding_sessions: Gauge,

    // RTP metrics
    rtp_packets_total: CounterVec,
    rtp_bytes_total: CounterVec,
    rtp_packets_lost: Counter,
    rtp_jitter_ms: Gauge,
    rtp_round_trip_time_ms: Gauge,

    // Media quality metrics
    media_mos_score: Gauge,
    media_packet_loss_percent: Gauge,
    media_quality_issues: Counter,
    media_echo_detections: Counter,

    // Security metrics
    security_blocked_ips: Gauge,
    security_rate_limited_requests: Counter,
    security_violations: Counter,
    security_failed_auth_attempts: Counter,
    security_threat_detections: Counter,

    // Performance metrics
    system_throughput: Gauge,
    memory_pool_utilization: Gauge,
    thread_pool_utilization: Gauge,
    db_connection_pool_usage: Gauge,
    cache_hit_rate: Gauge,

    // Metrics collector reference
    metrics_collector: Arc<MetricsCollector>,
}

impl PrometheusExporter {
    /// Create new Prometheus exporter
    pub fn new(metrics_collector: Arc<MetricsCollector>) -> Result<Self> {
        let registry = Registry::new();

        // System metrics
        let cpu_usage = Gauge::with_opts(Opts::new("redfire_cpu_usage_percent", "CPU usage percentage"))?;
        registry.register(Box::new(cpu_usage.clone()))?;

        let memory_usage_bytes = Gauge::with_opts(Opts::new("redfire_memory_usage_bytes", "Memory usage in bytes"))?;
        registry.register(Box::new(memory_usage_bytes.clone()))?;

        let memory_available_bytes = Gauge::with_opts(Opts::new("redfire_memory_available_bytes", "Available memory in bytes"))?;
        registry.register(Box::new(memory_available_bytes.clone()))?;

        let disk_usage_percent = Gauge::with_opts(Opts::new("redfire_disk_usage_percent", "Disk usage percentage"))?;
        registry.register(Box::new(disk_usage_percent.clone()))?;

        let network_rx_bytes = Counter::with_opts(Opts::new("redfire_network_rx_bytes_total", "Network bytes received"))?;
        registry.register(Box::new(network_rx_bytes.clone()))?;

        let network_tx_bytes = Counter::with_opts(Opts::new("redfire_network_tx_bytes_total", "Network bytes transmitted"))?;
        registry.register(Box::new(network_tx_bytes.clone()))?;

        let open_file_descriptors = Gauge::with_opts(Opts::new("redfire_open_file_descriptors", "Number of open file descriptors"))?;
        registry.register(Box::new(open_file_descriptors.clone()))?;

        let load_average = Gauge::with_opts(Opts::new("redfire_load_average_1m", "System load average (1 minute)"))?;
        registry.register(Box::new(load_average.clone()))?;

        let thread_count = Gauge::with_opts(Opts::new("redfire_thread_count", "Number of threads"))?;
        registry.register(Box::new(thread_count.clone()))?;

        // SIP metrics
        let sip_messages_total = CounterVec::new(
            Opts::new("redfire_sip_messages_total", "Total SIP messages processed"),
            &["method"],
        )?;
        registry.register(Box::new(sip_messages_total.clone()))?;

        let sip_messages_per_second = Gauge::with_opts(Opts::new("redfire_sip_messages_per_second", "SIP messages per second"))?;
        registry.register(Box::new(sip_messages_per_second.clone()))?;

        let sip_response_codes_total = CounterVec::new(
            Opts::new("redfire_sip_response_codes_total", "SIP response codes"),
            &["code"],
        )?;
        registry.register(Box::new(sip_response_codes_total.clone()))?;

        let sip_processing_latency = HistogramVec::new(
            HistogramOpts::new("redfire_sip_processing_latency_ms", "SIP message processing latency in milliseconds")
                .buckets(vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0]),
            &["percentile"],
        )?;
        registry.register(Box::new(sip_processing_latency.clone()))?;

        let sip_active_transactions = Gauge::with_opts(Opts::new("redfire_sip_active_transactions", "Active SIP transactions"))?;
        registry.register(Box::new(sip_active_transactions.clone()))?;

        let sip_transport_connections = GaugeVec::new(
            Opts::new("redfire_sip_transport_connections", "SIP transport connections"),
            &["transport"],
        )?;
        registry.register(Box::new(sip_transport_connections.clone()))?;

        let sip_transport_errors_total = CounterVec::new(
            Opts::new("redfire_sip_transport_errors_total", "SIP transport errors"),
            &["error_type"],
        )?;
        registry.register(Box::new(sip_transport_errors_total.clone()))?;

        // Call metrics
        let calls_total = CounterVec::new(
            Opts::new("redfire_calls_total", "Total calls processed"),
            &["status"],
        )?;
        registry.register(Box::new(calls_total.clone()))?;

        let calls_active = Gauge::with_opts(Opts::new("redfire_calls_active", "Active calls"))?;
        registry.register(Box::new(calls_active.clone()))?;

        let call_success_rate = Gauge::with_opts(Opts::new("redfire_call_success_rate_percent", "Call success rate percentage"))?;
        registry.register(Box::new(call_success_rate.clone()))?;

        let call_duration_seconds = Histogram::with_opts(
            HistogramOpts::new("redfire_call_duration_seconds", "Call duration in seconds")
                .buckets(vec![10.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0]),
        )?;
        registry.register(Box::new(call_duration_seconds.clone()))?;

        let answer_seizure_ratio = Gauge::with_opts(Opts::new("redfire_answer_seizure_ratio_percent", "Answer Seizure Ratio percentage"))?;
        registry.register(Box::new(answer_seizure_ratio.clone()))?;

        let post_dial_delay_ms = Gauge::with_opts(Opts::new("redfire_post_dial_delay_ms", "Post Dial Delay in milliseconds"))?;
        registry.register(Box::new(post_dial_delay_ms.clone()))?;

        // Media metrics
        let media_sessions_active = Gauge::with_opts(Opts::new("redfire_media_sessions_active", "Active media sessions"))?;
        registry.register(Box::new(media_sessions_active.clone()))?;

        let media_sessions_total = Counter::with_opts(Opts::new("redfire_media_sessions_total", "Total media sessions processed"))?;
        registry.register(Box::new(media_sessions_total.clone()))?;

        let media_packets_per_second = Gauge::with_opts(Opts::new("redfire_media_packets_per_second", "Media packets per second"))?;
        registry.register(Box::new(media_packets_per_second.clone()))?;

        let media_codec_usage = GaugeVec::new(
            Opts::new("redfire_media_codec_usage", "Media codec usage"),
            &["codec"],
        )?;
        registry.register(Box::new(media_codec_usage.clone()))?;

        let media_transcoding_sessions = Gauge::with_opts(Opts::new("redfire_media_transcoding_sessions", "Active transcoding sessions"))?;
        registry.register(Box::new(media_transcoding_sessions.clone()))?;

        // RTP metrics
        let rtp_packets_total = CounterVec::new(
            Opts::new("redfire_rtp_packets_total", "RTP packets"),
            &["direction"],
        )?;
        registry.register(Box::new(rtp_packets_total.clone()))?;

        let rtp_bytes_total = CounterVec::new(
            Opts::new("redfire_rtp_bytes_total", "RTP bytes"),
            &["direction"],
        )?;
        registry.register(Box::new(rtp_bytes_total.clone()))?;

        let rtp_packets_lost = Counter::with_opts(Opts::new("redfire_rtp_packets_lost_total", "RTP packets lost"))?;
        registry.register(Box::new(rtp_packets_lost.clone()))?;

        let rtp_jitter_ms = Gauge::with_opts(Opts::new("redfire_rtp_jitter_ms", "RTP jitter in milliseconds"))?;
        registry.register(Box::new(rtp_jitter_ms.clone()))?;

        let rtp_round_trip_time_ms = Gauge::with_opts(Opts::new("redfire_rtp_round_trip_time_ms", "RTP round trip time in milliseconds"))?;
        registry.register(Box::new(rtp_round_trip_time_ms.clone()))?;

        // Media quality metrics
        let media_mos_score = Gauge::with_opts(Opts::new("redfire_media_mos_score", "Mean Opinion Score (MOS)"))?;
        registry.register(Box::new(media_mos_score.clone()))?;

        let media_packet_loss_percent = Gauge::with_opts(Opts::new("redfire_media_packet_loss_percent", "Packet loss percentage"))?;
        registry.register(Box::new(media_packet_loss_percent.clone()))?;

        let media_quality_issues = Counter::with_opts(Opts::new("redfire_media_quality_issues_total", "Media quality issues detected"))?;
        registry.register(Box::new(media_quality_issues.clone()))?;

        let media_echo_detections = Counter::with_opts(Opts::new("redfire_media_echo_detections_total", "Echo detections"))?;
        registry.register(Box::new(media_echo_detections.clone()))?;

        // Security metrics
        let security_blocked_ips = Gauge::with_opts(Opts::new("redfire_security_blocked_ips", "Number of blocked IPs"))?;
        registry.register(Box::new(security_blocked_ips.clone()))?;

        let security_rate_limited_requests = Counter::with_opts(Opts::new("redfire_security_rate_limited_requests_total", "Rate limited requests"))?;
        registry.register(Box::new(security_rate_limited_requests.clone()))?;

        let security_violations = Counter::with_opts(Opts::new("redfire_security_violations_total", "Security violations detected"))?;
        registry.register(Box::new(security_violations.clone()))?;

        let security_failed_auth_attempts = Counter::with_opts(Opts::new("redfire_security_failed_auth_attempts_total", "Failed authentication attempts"))?;
        registry.register(Box::new(security_failed_auth_attempts.clone()))?;

        let security_threat_detections = Counter::with_opts(Opts::new("redfire_security_threat_detections_total", "Threat detections"))?;
        registry.register(Box::new(security_threat_detections.clone()))?;

        // Performance metrics
        let system_throughput = Gauge::with_opts(Opts::new("redfire_system_throughput_ops_per_second", "System throughput (operations per second)"))?;
        registry.register(Box::new(system_throughput.clone()))?;

        let memory_pool_utilization = Gauge::with_opts(Opts::new("redfire_memory_pool_utilization_percent", "Memory pool utilization percentage"))?;
        registry.register(Box::new(memory_pool_utilization.clone()))?;

        let thread_pool_utilization = Gauge::with_opts(Opts::new("redfire_thread_pool_utilization_percent", "Thread pool utilization percentage"))?;
        registry.register(Box::new(thread_pool_utilization.clone()))?;

        let db_connection_pool_usage = Gauge::with_opts(Opts::new("redfire_db_connection_pool_usage_percent", "Database connection pool usage percentage"))?;
        registry.register(Box::new(db_connection_pool_usage.clone()))?;

        let cache_hit_rate = Gauge::with_opts(Opts::new("redfire_cache_hit_rate_percent", "Cache hit rate percentage"))?;
        registry.register(Box::new(cache_hit_rate.clone()))?;

        Ok(Self {
            registry,
            cpu_usage,
            memory_usage_bytes,
            memory_available_bytes,
            disk_usage_percent,
            network_rx_bytes,
            network_tx_bytes,
            open_file_descriptors,
            load_average,
            thread_count,
            sip_messages_total,
            sip_messages_per_second,
            sip_response_codes_total,
            sip_processing_latency,
            sip_active_transactions,
            sip_transport_connections,
            sip_transport_errors_total,
            calls_total,
            calls_active,
            call_success_rate,
            call_duration_seconds,
            answer_seizure_ratio,
            post_dial_delay_ms,
            media_sessions_active,
            media_sessions_total,
            media_packets_per_second,
            media_codec_usage,
            media_transcoding_sessions,
            rtp_packets_total,
            rtp_bytes_total,
            rtp_packets_lost,
            rtp_jitter_ms,
            rtp_round_trip_time_ms,
            media_mos_score,
            media_packet_loss_percent,
            media_quality_issues,
            media_echo_detections,
            security_blocked_ips,
            security_rate_limited_requests,
            security_violations,
            security_failed_auth_attempts,
            security_threat_detections,
            system_throughput,
            memory_pool_utilization,
            thread_pool_utilization,
            db_connection_pool_usage,
            cache_hit_rate,
            metrics_collector,
        })
    }

    /// Update Prometheus metrics from metrics snapshot
    pub async fn update_from_snapshot(&self, snapshot: &SystemMetricsSnapshot) -> Result<()> {
        // System metrics
        self.cpu_usage.set(snapshot.system.cpu_usage_percent);
        self.memory_usage_bytes.set((snapshot.system.memory_usage_mb * 1024 * 1024) as f64);
        self.memory_available_bytes.set((snapshot.system.memory_available_mb * 1024 * 1024) as f64);
        self.disk_usage_percent.set(snapshot.system.disk_usage_percent);
        self.open_file_descriptors.set(snapshot.system.open_file_descriptors as f64);
        self.load_average.set(snapshot.system.load_average_1m);
        self.thread_count.set(snapshot.system.thread_count as f64);

        // SIP metrics
        for (method, count) in &snapshot.sip.messages_by_method {
            self.sip_messages_total.with_label_values(&[method]).inc_by(*count as f64);
        }
        self.sip_messages_per_second.set(snapshot.sip.messages_per_second);

        for (code, count) in &snapshot.sip.response_codes {
            self.sip_response_codes_total.with_label_values(&[&code.to_string()]).inc_by(*count as f64);
        }

        self.sip_active_transactions.set(snapshot.sip.active_transactions as f64);

        // Transport stats
        self.sip_transport_connections.with_label_values(&["udp"]).set(snapshot.sip.transport_stats.udp_connections as f64);
        self.sip_transport_connections.with_label_values(&["tcp"]).set(snapshot.sip.transport_stats.tcp_connections as f64);
        self.sip_transport_connections.with_label_values(&["tls"]).set(snapshot.sip.transport_stats.tls_connections as f64);
        self.sip_transport_connections.with_label_values(&["websocket"]).set(snapshot.sip.transport_stats.websocket_connections as f64);

        // Call metrics
        self.calls_active.set(snapshot.business.active_calls as f64);
        self.call_success_rate.set(snapshot.business.call_success_rate);
        self.call_duration_seconds.observe(snapshot.business.avg_call_duration);
        self.answer_seizure_ratio.set(snapshot.business.answer_seizure_ratio);
        self.post_dial_delay_ms.set(snapshot.business.post_dial_delay_ms);

        // Media metrics
        self.media_sessions_active.set(snapshot.media.active_sessions as f64);
        self.media_packets_per_second.set(snapshot.media.packets_per_second);

        for (codec, count) in &snapshot.media.codec_usage {
            self.media_codec_usage.with_label_values(&[codec]).set(*count as f64);
        }

        self.media_transcoding_sessions.set(snapshot.media.transcoding_sessions as f64);

        // RTP metrics
        self.rtp_jitter_ms.set(snapshot.media.rtp_stats.jitter_ms);
        self.rtp_round_trip_time_ms.set(snapshot.media.rtp_stats.rtt_ms);

        // Media quality
        self.media_mos_score.set(snapshot.media.quality_metrics.mos_score);
        self.media_packet_loss_percent.set(snapshot.media.quality_metrics.packet_loss_percent);

        // Security metrics
        self.security_blocked_ips.set(snapshot.security.blocked_ips as f64);

        // Performance metrics
        self.system_throughput.set(snapshot.performance.system_throughput);
        self.memory_pool_utilization.set(snapshot.performance.memory_pool_utilization);
        self.thread_pool_utilization.set(snapshot.performance.thread_pool_utilization);
        self.db_connection_pool_usage.set(snapshot.performance.db_connection_pool_usage);
        self.cache_hit_rate.set(snapshot.performance.cache_hit_rate);

        Ok(())
    }

    /// Export metrics in Prometheus text format
    pub async fn export_metrics(&self) -> Result<String> {
        // Get latest metrics and update Prometheus metrics
        match self.metrics_collector.get_latest_metrics().await {
            Ok(snapshot) => {
                if let Err(e) = self.update_from_snapshot(&snapshot).await {
                    error!("Failed to update Prometheus metrics: {}", e);
                }
            }
            Err(e) => {
                debug!("No metrics available yet: {}", e);
            }
        }

        // Encode metrics to Prometheus text format
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;

        Ok(String::from_utf8(buffer)?)
    }

    /// Get registry for custom metrics
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::MetricsCollector;

    #[tokio::test]
    async fn test_prometheus_exporter_creation() {
        let collector = Arc::new(MetricsCollector::new(30, 24).unwrap());
        let exporter = PrometheusExporter::new(collector).unwrap();

        assert!(exporter.registry().gather().len() > 0);
    }

    #[tokio::test]
    async fn test_metrics_export() {
        let collector = Arc::new(MetricsCollector::new(30, 24).unwrap());
        let exporter = PrometheusExporter::new(collector.clone()).unwrap();

        // Collect some metrics first
        let _ = collector.collect_metrics().await;

        // Export metrics
        let output = exporter.export_metrics().await.unwrap();

        // Should contain Prometheus-formatted metrics
        assert!(output.contains("redfire_"));
        assert!(output.contains("# HELP"));
        assert!(output.contains("# TYPE"));
    }
}
