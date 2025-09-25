# Redfire Switch Configuration Guide

## Overview

This guide provides comprehensive documentation for configuring the Redfire Switch telecommunications system, including all available options, best practices, and performance tuning recommendations.

## Configuration Files

### Main Configuration (`config.toml`)

The primary configuration file using TOML format:

```toml
# /etc/redfire-switch/config.toml

[server]
# Server binding and worker configuration
bind_address = "0.0.0.0:8080"              # API server bind address
workers = 16                               # Number of worker threads
max_connections = 1000                     # Maximum concurrent connections
keep_alive = 300                           # Connection keep-alive timeout (seconds)
request_timeout = 30                       # Request timeout (seconds)
enable_cors = true                         # Enable CORS for web UI

[database]
# PostgreSQL database configuration
url = "postgresql://redfire_user:password@localhost:5432/redfire_switch"
max_connections = 50                       # Connection pool size
min_connections = 5                        # Minimum pool connections
connection_timeout = 30                    # Connection timeout (seconds)
idle_timeout = 600                         # Idle connection timeout (seconds)
enable_logging = false                     # Log SQL queries (debug only)

[sip]
# SIP protocol configuration
bind_address = "0.0.0.0:5060"             # SIP UDP bind address
bind_address_tcp = "0.0.0.0:5060"         # SIP TCP bind address (optional)
bind_address_tls = "0.0.0.0:5061"         # SIP TLS bind address (optional)
external_ip = "203.0.113.100"             # External IP for SDP
realm = "redfire-switch.local"            # SIP realm
user_agent = "Redfire Switch 1.0"         # User-Agent header
max_forwards = 70                          # Max-Forwards header value
session_expires = 3600                     # Session timer (seconds)
min_se = 90                               # Minimum session expires
supported_methods = ["INVITE", "ACK", "BYE", "CANCEL", "OPTIONS", "REGISTER"]

[media]
# RTP media configuration
rtp_port_range = "10000-20000"            # RTP port range
enable_transcoding = true                  # Enable codec transcoding
default_codec = "G711U"                   # Default codec preference
codec_priority = ["G711U", "G711A", "G729", "G722"]  # Codec negotiation order
enable_dtmf_detection = true               # Enable DTMF detection
comfort_noise_generation = true            # Generate comfort noise
jitter_buffer_size = 50                   # Jitter buffer size (ms)
packet_loss_threshold = 0.05              # Packet loss alarm threshold

[performance]
# Performance and optimization settings
max_concurrent_calls = 10000              # Maximum concurrent calls
memory_pool_size = 1000                   # Memory pool initial size
enable_gpu_acceleration = false            # Enable GPU codec acceleration
thread_pool_size = 32                     # Worker thread pool size
cache_ttl_seconds = 300                   # General cache TTL
enable_numa = false                       # NUMA awareness (requires NUMA hardware)
lock_free_operations = true               # Use lock-free data structures

[gpu]
# GPU acceleration configuration (optional)
enabled = false                           # Enable GPU processing
backend = "CUDA"                          # Backend: "CUDA" or "ROCm"
device_id = 0                             # GPU device ID
batch_size = 64                           # Batch processing size
memory_pool_size_mb = 512                 # GPU memory pool size
enable_async_processing = true            # Asynchronous GPU operations

[security]
# Security and authentication
enable_authentication = true              # Enable API authentication
jwt_secret = "your-256-bit-secret-key"   # JWT signing secret (CHANGE THIS!)
encryption_key = "your-aes-256-key"      # AES encryption key (CHANGE THIS!)
session_timeout = 28800                  # Session timeout (8 hours)
rate_limit_requests = 1000               # Rate limit per minute
rate_limit_window = 60                   # Rate limit window (seconds)
enable_tls = false                       # Enable TLS/SSL
cert_file = "/etc/redfire-switch/certs/server.crt"
key_file = "/etc/redfire-switch/certs/server.key"

[logging]
# Logging configuration
level = "INFO"                           # Log level: TRACE, DEBUG, INFO, WARN, ERROR
format = "json"                          # Format: "json" or "text"
file = "/var/log/redfire-switch/redfire.log"  # Log file path
max_size = "100MB"                       # Max log file size
max_files = 10                          # Max log files to keep
enable_syslog = false                   # Enable syslog output
syslog_facility = "daemon"              # Syslog facility

[anti_fraud]
# Anti-fraud monitoring configuration
enabled = true                          # Enable anti-fraud monitoring
risk_threshold = 0.75                   # Risk score threshold (0.0-1.0)
short_call_threshold_seconds = 6        # Short call detection threshold
enable_ml_detection = true              # Enable ML-based detection
model_path = "/etc/redfire-switch/models/fraud_detection.model"
banned_destinations_file = "/etc/redfire-switch/banned_destinations.txt"
whitelist_customers = []                # Customer IDs to whitelist

[voice_integrity]
# Voice integrity and lawful intercept
enabled = true                          # Enable voice integrity monitoring
storage_path = "/var/lib/redfire-switch/recordings"  # Recording storage path
max_storage_gb = 1000                   # Maximum storage (GB)
retention_days = 90                     # Recording retention period
encryption_enabled = true               # Encrypt stored recordings
transcription_enabled = true            # Enable speech-to-text
vosk_server_url = "http://localhost:2700"  # Vosk ASR server URL

[monitoring]
# System monitoring and metrics
enabled = true                          # Enable monitoring
metrics_bind_address = "0.0.0.0:9090"  # Prometheus metrics endpoint
health_check_interval = 30              # Health check interval (seconds)
performance_sampling_interval = 5       # Performance sampling (seconds)
enable_tracing = false                  # Enable distributed tracing
jaeger_endpoint = "http://localhost:14268"  # Jaeger collector endpoint

[cluster]
# Clustering configuration (for HA deployments)
enabled = false                         # Enable clustering
node_id = "node1"                      # Unique node identifier
bind_address = "0.0.0.0:7000"         # Cluster communication port
peers = []                             # List of peer addresses
election_timeout_ms = 5000             # Leader election timeout
heartbeat_interval_ms = 1000           # Heartbeat interval
```

## Environment Variables

Environment variables override configuration file settings:

```bash
# Core settings
DATABASE_URL="postgresql://user:pass@localhost:5432/redfire_switch"
REDIS_URL="redis://localhost:6379/0"
BIND_ADDRESS="0.0.0.0:8080"
SIP_BIND_ADDRESS="0.0.0.0:5060"
EXTERNAL_IP="203.0.113.100"

# Security
JWT_SECRET="your-jwt-secret-key"
ENCRYPTION_KEY="your-encryption-key"

# Performance
MAX_CONCURRENT_CALLS=10000
WORKERS=16
THREAD_POOL_SIZE=32
MEMORY_POOL_SIZE=1000

# Features
ENABLE_GPU_ACCELERATION=false
ENABLE_ANTI_FRAUD=true
ENABLE_VOICE_INTEGRITY=true
ENABLE_CLUSTERING=false

# Logging
RUST_LOG="info,redfire_switch=debug"
LOG_LEVEL="INFO"
LOG_FORMAT="json"
```

## Performance Tuning

### High-Volume Configuration

For deployments handling 50,000+ concurrent calls:

```toml
[server]
workers = 32
max_connections = 5000

[performance]
max_concurrent_calls = 50000
memory_pool_size = 10000
thread_pool_size = 64
enable_numa = true

[database]
max_connections = 200
min_connections = 50

[sip]
session_expires = 1800  # Shorter session timers
```

### Low-Latency Configuration

For minimizing call setup and media latency:

```toml
[performance]
thread_pool_size = 64
lock_free_operations = true
cache_ttl_seconds = 60

[media]
jitter_buffer_size = 20  # Smaller jitter buffer
enable_transcoding = false  # Direct codec passthrough when possible

[database]
connection_timeout = 5
```

### Memory-Optimized Configuration

For memory-constrained environments:

```toml
[performance]
memory_pool_size = 500
max_concurrent_calls = 5000

[database]
max_connections = 20
min_connections = 5

[logging]
level = "WARN"  # Reduce log volume
```

## Security Configuration

### TLS/SSL Setup

```toml
[security]
enable_tls = true
cert_file = "/etc/redfire-switch/certs/server.crt"
key_file = "/etc/redfire-switch/certs/server.key"

[sip]
bind_address_tls = "0.0.0.0:5061"
# Require TLS for signaling
force_tls = true
```

### Authentication & Authorization

```toml
[security]
enable_authentication = true
jwt_secret = "your-very-secure-256-bit-key"
session_timeout = 14400  # 4 hours
rate_limit_requests = 500  # Conservative rate limiting

# API access control
allowed_origins = ["https://admin.company.com"]
require_client_certs = false
```

## Codec Configuration

### GPU Acceleration

```toml
[gpu]
enabled = true
backend = "CUDA"  # or "ROCm" for AMD
device_id = 0
batch_size = 128
memory_pool_size_mb = 1024

[media]
enable_transcoding = true
# Prioritize GPU-accelerated codecs
codec_priority = ["G711U", "G711A", "G722"]
```

### Codec Quality Settings

```toml
[media]
# High-quality settings
jitter_buffer_size = 100
comfort_noise_generation = true
enable_dtmf_detection = true
packet_loss_threshold = 0.01  # Strict quality requirements

# Codec-specific settings
[media.g729]
annex_a_enabled = true
annex_b_enabled = true  # Silence suppression
vad_enabled = true      # Voice activity detection

[media.g722]
complexity = "high"     # Higher CPU usage, better quality
```

## Database Optimization

### Connection Pooling

```toml
[database]
max_connections = 100
min_connections = 10
connection_timeout = 15
idle_timeout = 300
# Enable prepared statement caching
enable_prepared_statements = true
max_prepared_statements = 1000
```

### Query Optimization

```toml
[database.optimization]
enable_query_analysis = true
slow_query_threshold_ms = 100
enable_bulk_operations = true
bulk_batch_size = 1000
```

## Anti-Fraud Configuration

### Machine Learning Detection

```toml
[anti_fraud]
enabled = true
risk_threshold = 0.8
enable_ml_detection = true
model_path = "/etc/redfire-switch/models/fraud_detection.onnx"

# Detection rules
short_call_threshold_seconds = 6
max_calls_per_minute = 10
suspicious_destinations = [
    "900*",  # Premium numbers
    "976*",  # Adult services
]

# Actions
auto_block_threshold = 0.95
notification_webhook = "https://alerts.company.com/fraud"
```

### Rule-Based Detection

```toml
[anti_fraud.rules]
# Geographic restrictions
blocked_countries = ["XX", "YY"]
allowed_countries = ["US", "CA", "UK"]

# Time-based restrictions
business_hours_only = false
weekend_restrictions = false

# Volume limits
max_calls_per_customer_per_hour = 1000
max_minutes_per_customer_per_day = 10000
```

## Voice Integrity & Compliance

### Legal Authorization Setup

```toml
[voice_integrity]
enabled = true
storage_path = "/var/lib/redfire-switch/recordings"
encryption_enabled = true
encryption_algorithm = "AES-256-GCM"

# Retention policies
retention_days = 90
automatic_deletion = true
audit_trail = true

# Transcription
transcription_enabled = true
vosk_server_url = "http://localhost:2700"
banned_words_file = "/etc/redfire-switch/banned_words.json"
```

### Compliance Features

```toml
[voice_integrity.compliance]
# ECPA compliance
ecpa_enabled = true
require_authorization = true
chain_of_custody = true

# Call recording triggers
record_on_keywords = ["warrant", "subpoena"]
record_by_phone_number = ["+1234567890"]
record_by_customer_id = ["high-risk-customer-001"]
```

## Monitoring Configuration

### Metrics and Alerting

```toml
[monitoring]
enabled = true
metrics_bind_address = "0.0.0.0:9090"
export_interval_seconds = 15

# Performance thresholds for alerts
cpu_alert_threshold = 80.0
memory_alert_threshold = 85.0
call_failure_rate_threshold = 0.05

# Webhook notifications
alert_webhook = "https://alerts.company.com/webhook"
```

### Health Checks

```toml
[monitoring.health]
interval_seconds = 30
timeout_seconds = 10
endpoints = [
    "database",
    "redis",
    "sip_stack",
    "media_engine"
]
```

## Best Practices

### Production Deployment

1. **Security**
   - Always change default secrets
   - Enable TLS for all communications
   - Use strong authentication
   - Regular security updates

2. **Performance**
   - Size memory pools based on expected load
   - Enable GPU acceleration for high-volume deployments
   - Use database connection pooling
   - Monitor and tune based on metrics

3. **Reliability**
   - Configure appropriate timeouts
   - Enable health checks
   - Set up comprehensive monitoring
   - Plan for graceful degradation

4. **Compliance**
   - Configure retention policies appropriately
   - Enable audit trails
   - Secure recording storage
   - Regular compliance audits

### Configuration Validation

```bash
# Validate configuration
redfire-switch --config /etc/redfire-switch/config.toml --validate

# Test database connection
redfire-switch --test-database

# Check SIP binding
redfire-switch --test-sip-bind
```

## Troubleshooting

### Common Configuration Issues

1. **Database Connection Failures**
   - Verify DATABASE_URL format
   - Check network connectivity
   - Validate credentials

2. **SIP Binding Issues**
   - Check port availability
   - Verify external_ip setting
   - Firewall configuration

3. **Performance Problems**
   - Review thread pool sizes
   - Check memory pool utilization
   - Monitor database connection usage

4. **Security Issues**
   - Verify JWT secret configuration
   - Check TLS certificate validity
   - Review rate limiting settings
