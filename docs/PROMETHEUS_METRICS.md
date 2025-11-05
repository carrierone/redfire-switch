# Prometheus Metrics Guide

This guide explains how to use the Prometheus metrics system in Redfire Switch for monitoring and observability.

## Table of Contents

- [Overview](#overview)
- [Quick Start](#quick-start)
- [Available Metrics](#available-metrics)
- [Configuration](#configuration)
- [Integration with Prometheus](#integration-with-prometheus)
- [Grafana Dashboards](#grafana-dashboards)
- [Alert Rules](#alert-rules)
- [API Endpoints](#api-endpoints)

## Overview

Redfire Switch provides comprehensive Prometheus-compatible metrics for monitoring all aspects of the SIP switch, including:

- **System Resources**: CPU, memory, disk, network
- **SIP Processing**: Message rates, response codes, latency
- **Call Metrics**: Active calls, success rate, ASR, PDD
- **Media Processing**: RTP stats, codec usage, quality metrics
- **Security**: Failed auth, rate limiting, violations
- **Performance**: Cache hit rate, pool utilization

## Quick Start

### 1. Run the Standalone Metrics Server

The quickest way to get started is with the standalone metrics server:

```bash
cargo run --bin prometheus_metrics_server
```

This starts an HTTP server on port 9090 with:
- Metrics endpoint: `http://localhost:9090/metrics`
- Health check: `http://localhost:9090/health`
- Readiness check: `http://localhost:9090/health/ready`
- Liveness check: `http://localhost:9090/health/live`

### 2. View Metrics

Open your browser and navigate to `http://localhost:9090/metrics` to see the Prometheus-formatted metrics.

Example output:
```
# HELP redfire_cpu_usage_percent CPU usage percentage
# TYPE redfire_cpu_usage_percent gauge
redfire_cpu_usage_percent 25.5

# HELP redfire_calls_active Active calls
# TYPE redfire_calls_active gauge
redfire_calls_active 42

# HELP redfire_sip_messages_per_second SIP messages per second
# TYPE redfire_sip_messages_per_second gauge
redfire_sip_messages_per_second 150.5
```

### 3. Configure Prometheus

Add this scrape configuration to your `prometheus.yml`:

```yaml
scrape_configs:
  - job_name: 'redfire-switch'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 15s
```

### 4. Import Grafana Dashboard

Import the pre-built dashboard from `grafana/dashboards/redfire-overview.json` into your Grafana instance.

## Available Metrics

### System Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `redfire_cpu_usage_percent` | Gauge | CPU usage percentage |
| `redfire_memory_usage_bytes` | Gauge | Memory usage in bytes |
| `redfire_memory_available_bytes` | Gauge | Available memory in bytes |
| `redfire_disk_usage_percent` | Gauge | Disk usage percentage |
| `redfire_network_rx_bytes_total` | Counter | Network bytes received |
| `redfire_network_tx_bytes_total` | Counter | Network bytes transmitted |
| `redfire_open_file_descriptors` | Gauge | Number of open file descriptors |
| `redfire_load_average_1m` | Gauge | System load average (1 minute) |
| `redfire_thread_count` | Gauge | Number of threads |

### SIP Metrics

| Metric | Type | Description | Labels |
|--------|------|-------------|--------|
| `redfire_sip_messages_total` | Counter | Total SIP messages processed | `method` |
| `redfire_sip_messages_per_second` | Gauge | SIP messages per second | - |
| `redfire_sip_response_codes_total` | Counter | SIP response codes | `code` |
| `redfire_sip_processing_latency_ms` | Histogram | SIP message processing latency | `percentile` |
| `redfire_sip_active_transactions` | Gauge | Active SIP transactions | - |
| `redfire_sip_transport_connections` | Gauge | SIP transport connections | `transport` |
| `redfire_sip_transport_errors_total` | Counter | SIP transport errors | `error_type` |

### Call Metrics

| Metric | Type | Description | Labels |
|--------|------|-------------|--------|
| `redfire_calls_total` | Counter | Total calls processed | `status` |
| `redfire_calls_active` | Gauge | Active calls | - |
| `redfire_call_success_rate_percent` | Gauge | Call success rate percentage | - |
| `redfire_call_duration_seconds` | Histogram | Call duration in seconds | - |
| `redfire_answer_seizure_ratio_percent` | Gauge | Answer Seizure Ratio (ASR) | - |
| `redfire_post_dial_delay_ms` | Gauge | Post Dial Delay in milliseconds | - |

### Media Metrics

| Metric | Type | Description | Labels |
|--------|------|-------------|--------|
| `redfire_media_sessions_active` | Gauge | Active media sessions | - |
| `redfire_media_sessions_total` | Counter | Total media sessions | - |
| `redfire_media_packets_per_second` | Gauge | Media packets per second | - |
| `redfire_media_codec_usage` | Gauge | Media codec usage | `codec` |
| `redfire_media_transcoding_sessions` | Gauge | Active transcoding sessions | - |

### RTP Metrics

| Metric | Type | Description | Labels |
|--------|------|-------------|--------|
| `redfire_rtp_packets_total` | Counter | RTP packets | `direction` |
| `redfire_rtp_bytes_total` | Counter | RTP bytes | `direction` |
| `redfire_rtp_packets_lost_total` | Counter | RTP packets lost | - |
| `redfire_rtp_jitter_ms` | Gauge | RTP jitter in milliseconds | - |
| `redfire_rtp_round_trip_time_ms` | Gauge | RTP round trip time | - |

### Media Quality Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `redfire_media_mos_score` | Gauge | Mean Opinion Score (MOS) |
| `redfire_media_packet_loss_percent` | Gauge | Packet loss percentage |
| `redfire_media_quality_issues_total` | Counter | Media quality issues detected |
| `redfire_media_echo_detections_total` | Counter | Echo detections |

### Security Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `redfire_security_blocked_ips` | Gauge | Number of blocked IPs |
| `redfire_security_rate_limited_requests_total` | Counter | Rate limited requests |
| `redfire_security_violations_total` | Counter | Security violations detected |
| `redfire_security_failed_auth_attempts_total` | Counter | Failed authentication attempts |
| `redfire_security_threat_detections_total` | Counter | Threat detections |

### Performance Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `redfire_system_throughput_ops_per_second` | Gauge | System throughput (ops/sec) |
| `redfire_memory_pool_utilization_percent` | Gauge | Memory pool utilization |
| `redfire_thread_pool_utilization_percent` | Gauge | Thread pool utilization |
| `redfire_db_connection_pool_usage_percent` | Gauge | Database connection pool usage |
| `redfire_cache_hit_rate_percent` | Gauge | Cache hit rate |

## Configuration

### Monitoring Configuration

```rust
use redfire_switch::monitoring::{MonitoringConfig, NotificationEndpoint, NotificationEndpointType};

let config = MonitoringConfig {
    enabled: true,
    metrics_interval_seconds: 15,
    health_check_interval_seconds: 30,
    alert_evaluation_interval_seconds: 30,
    metrics_retention_hours: 24,
    enable_dashboard: false,
    enable_external_export: true,
    enable_alerting: true,
    notification_endpoints: vec![
        NotificationEndpoint {
            name: "console".to_string(),
            endpoint_type: NotificationEndpointType::Console,
            config: HashMap::new(),
            enabled: true,
        },
    ],
};
```

### Integration in Your Application

```rust
use redfire_switch::monitoring::{MonitoringSystem, PrometheusExporter};
use redfire_switch::api::metrics_endpoints::{create_metrics_router, MetricsState};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Create monitoring system
    let monitoring_config = MonitoringConfig::default();
    let monitoring_system = Arc::new(MonitoringSystem::new(monitoring_config)?);
    monitoring_system.start().await?;

    // Create Prometheus exporter
    let prometheus_exporter = Arc::new(
        PrometheusExporter::new(monitoring_system.metrics())?
    );

    // Create metrics state
    let metrics_state = MetricsState {
        prometheus_exporter,
        monitoring_system: monitoring_system.clone(),
    };

    // Create metrics router
    let metrics_router = create_metrics_router(metrics_state);

    // Merge with your main router
    let app = Router::new()
        .merge(metrics_router)
        .merge(your_api_router);

    // Start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:9090").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

## Integration with Prometheus

### Docker Compose Setup

```yaml
version: '3.8'

services:
  redfire-switch:
    build: .
    ports:
      - "5060:5060/udp"
      - "9090:9090"
    environment:
      - RUST_LOG=info

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9091:9090"
    volumes:
      - ./prometheus/prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    volumes:
      - ./grafana/provisioning:/etc/grafana/provisioning
      - ./grafana/dashboards:/var/lib/grafana/dashboards
      - grafana_data:/var/lib/grafana
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - GF_USERS_ALLOW_SIGN_UP=false

volumes:
  prometheus_data:
  grafana_data:
```

### Prometheus Configuration

`prometheus/prometheus.yml`:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'redfire-switch'
    static_configs:
      - targets: ['redfire-switch:9090']
    metrics_path: /metrics
```

## Grafana Dashboards

### Import Pre-built Dashboard

1. Open Grafana (`http://localhost:3000`)
2. Go to **Dashboards** → **Import**
3. Upload `grafana/dashboards/redfire-overview.json`
4. Select your Prometheus data source
5. Click **Import**

### Key Dashboard Panels

- **CPU Usage**: Real-time CPU utilization
- **Memory Usage**: Memory consumption tracking
- **Active Calls**: Current active calls and call rate
- **SIP Message Rate**: SIP messages per second by method
- **Call Success Rate**: Percentage of successful calls
- **ASR**: Answer Seizure Ratio
- **Media Sessions**: Active and transcoding sessions
- **Media Quality**: Jitter, RTT, and PDD metrics

## Alert Rules

### Example Prometheus Alert Rules

Create `prometheus/alerts/redfire-alerts.yml`:

```yaml
groups:
  - name: redfire_alerts
    interval: 30s
    rules:
      - alert: HighCPUUsage
        expr: redfire_cpu_usage_percent > 80
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High CPU usage detected"
          description: "CPU usage is {{ $value }}%"

      - alert: LowCallSuccessRate
        expr: redfire_call_success_rate_percent < 95
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Low call success rate"
          description: "Call success rate is {{ $value }}%"

      - alert: HighMemoryUsage
        expr: redfire_memory_usage_bytes > 1800000000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High memory usage"
          description: "Memory usage is {{ $value | humanize }}B"

      - alert: SecurityViolations
        expr: rate(redfire_security_violations_total[5m]) > 0.1
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Security violations detected"
          description: "{{ $value }} violations/sec"
```

Reference this file in your `prometheus.yml`:

```yaml
rule_files:
  - /etc/prometheus/alerts/*.yml
```

## API Endpoints

### Metrics Endpoint

**GET** `/metrics`

Returns Prometheus-formatted metrics in text exposition format.

**Response Content-Type**: `text/plain; version=0.0.4`

**Example**:
```bash
curl http://localhost:9090/metrics
```

### Health Check Endpoints

#### Overall Health
**GET** `/health`

Returns: `OK`, `DEGRADED`, or `UNHEALTHY`

**Status Codes**:
- 200: Healthy or Degraded
- 503: Unhealthy

#### Readiness Check
**GET** `/health/ready`

Indicates if the service is ready to accept traffic.

**Status Codes**:
- 200: Ready
- 503: Not ready

#### Liveness Check
**GET** `/health/live`

Indicates if the service is alive (for Kubernetes).

**Status Codes**:
- 200: Alive

## Best Practices

### 1. Metrics Collection Interval

- **Recommendation**: 15-30 seconds
- Balance between granularity and overhead
- Adjust based on your monitoring needs

### 2. Retention Period

- **Default**: 24 hours in-memory
- For long-term storage, rely on Prometheus TSDB
- Configure Prometheus retention: `--storage.tsdb.retention.time=30d`

### 3. Alert Thresholds

- Set thresholds based on baseline performance
- Use percentiles (P95, P99) for latency alerts
- Avoid alert fatigue with appropriate `for` durations

### 4. Dashboard Organization

- Create separate dashboards for different roles (ops, business, security)
- Use variables for filtering (e.g., by SIP method, codec)
- Set appropriate refresh rates (5s for real-time, 30s for overview)

### 5. Security

- Protect metrics endpoints with authentication in production
- Restrict access to internal networks only
- Sanitize sensitive data from metric labels

## Troubleshooting

### Metrics Not Updating

1. Check if monitoring system is started:
   ```rust
   monitoring_system.start().await?;
   ```

2. Verify metrics collection is running:
   ```bash
   curl http://localhost:9090/health
   ```

3. Check logs for errors:
   ```bash
   RUST_LOG=debug cargo run --bin prometheus_metrics_server
   ```

### High Memory Usage

- Reduce `metrics_retention_hours`
- Increase scrape interval in Prometheus
- Check for metric cardinality explosion (too many unique label combinations)

### Missing Metrics

- Ensure all counters are incremented in your application code
- Check that the `MetricsCollector` is accessible where needed
- Verify no errors in metrics collection logs

## Next Steps

- Set up Alertmanager for notifications
- Create custom dashboards for specific use cases
- Integrate with your existing monitoring infrastructure
- Add custom metrics for application-specific needs

## Resources

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Prometheus Best Practices](https://prometheus.io/docs/practices/naming/)
- [PromQL Cheat Sheet](https://promlabs.com/promql-cheat-sheet/)
