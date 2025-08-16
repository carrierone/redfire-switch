# SIP Monitoring System

This document provides comprehensive information about the SIP monitoring capabilities of Redfire Switch.

## Overview

Redfire Switch includes a sophisticated SIP endpoint monitoring system that uses SIP OPTIONS messages to perform health checks on remote SIP endpoints. This system provides real-time visibility into the health and performance of your SIP infrastructure.

## How SIP Monitoring Works

### SIP OPTIONS Ping

The monitoring system sends SIP OPTIONS requests to configured endpoints at regular intervals. This approach:

- **Uses standard SIP protocol** - Compatible with all SIP servers
- **Measures response time** - Provides performance metrics
- **Detects failures** - Identifies unresponsive endpoints
- **Tracks statistics** - Maintains success/failure ratios
- **Non-intrusive** - Doesn't affect call processing

### OPTIONS Request Format

The monitoring system sends properly formatted SIP OPTIONS requests:

```
OPTIONS sip:192.168.1.100:5060 SIP/2.0
Via: SIP/2.0/UDP redfire-switch:5060;branch=z9hG4bK12345
Max-Forwards: 70
To: <sip:192.168.1.100:5060>
From: <sip:redfire-switch@redfire-switch>;tag=rs12345
Call-ID: 12345@redfire-switch
CSeq: 1 OPTIONS
Contact: <sip:redfire-switch@redfire-switch:5060>
User-Agent: Redfire-Switch/0.1.0
Content-Length: 0
```

## Endpoint States

The monitoring system tracks each endpoint in one of four states:

### Unknown
- **Description**: Initial state when monitoring starts
- **Causes**: No pings sent yet, or monitoring just enabled
- **Duration**: Temporary state until first ping completes

### Online
- **Description**: Endpoint is responding to OPTIONS pings
- **Criteria**: Last ping received valid SIP response
- **Response Codes**: Any valid SIP response (200, 405, etc.)

### Offline
- **Description**: Endpoint is not responding after multiple failures
- **Criteria**: 3 or more consecutive ping failures
- **Recovery**: Automatically returns to Online when pings succeed

### Error
- **Description**: Specific error condition encountered
- **Examples**: Network timeout, connection refused, invalid response
- **Information**: Error details stored for troubleshooting

## Health Metrics

For each monitored endpoint, the system tracks:

### Response Time Metrics
- **Last Response Time**: Duration of most recent successful ping
- **Average Response Time**: Historical average (planned feature)
- **Min/Max Response Time**: Performance bounds (planned feature)

### Reliability Metrics
- **Total Pings**: Count of all ping attempts
- **Successful Pings**: Count of successful responses
- **Success Rate**: Percentage of successful pings
- **Consecutive Failures**: Current failure streak

### Timing Information
- **Last Check**: Timestamp of most recent ping attempt
- **Next Check**: When the next ping will be sent (planned feature)
- **Uptime**: Duration since endpoint went online (planned feature)

## Configuration

### Basic Monitoring Setup

```json
{
  "monitoring": {
    "enabled": true,
    "endpoints": [
      {
        "name": "upstream-carrier",
        "address": "10.1.1.100:5060",
        "protocol": "Udp",
        "enabled": true,
        "ping_interval_seconds": 30,
        "timeout_seconds": 5
      }
    ]
  }
}
```

### Configuration Parameters

#### Global Settings

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `enabled` | boolean | `true` | Master enable/disable for monitoring |

#### Endpoint Settings

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `name` | string | Required | Unique identifier for endpoint |
| `address` | string | Required | Target IP:PORT address |
| `protocol` | string | `"Udp"` | Transport protocol (Udp/Tcp) |
| `enabled` | boolean | `true` | Enable monitoring for this endpoint |
| `ping_interval_seconds` | number | `30` | Seconds between health checks |
| `timeout_seconds` | number | `5` | Timeout for each ping attempt |

### Advanced Configuration Examples

#### High-Frequency Monitoring
```json
{
  "name": "critical-carrier",
  "address": "203.0.113.100:5060",
  "protocol": "Udp",
  "enabled": true,
  "ping_interval_seconds": 10,
  "timeout_seconds": 2
}
```

#### Conservative Monitoring
```json
{
  "name": "backup-system",
  "address": "192.168.1.200:5060",
  "protocol": "Udp",
  "enabled": true,
  "ping_interval_seconds": 120,
  "timeout_seconds": 15
}
```

#### TCP Monitoring (Future)
```json
{
  "name": "secure-endpoint",
  "address": "10.1.1.200:5061",
  "protocol": "Tcp",
  "enabled": true,
  "ping_interval_seconds": 60,
  "timeout_seconds": 10
}
```

## Monitoring Operations

### Starting Monitoring

Monitoring starts automatically when the switch starts if enabled in configuration:

```bash
# Start switch with monitoring
redfire-switch start

# Start with verbose monitoring logs
redfire-switch --verbose start
```

### Viewing Monitoring Status

```bash
# Show all endpoint status
redfire-switch monitor status

# Show specific endpoint details
redfire-switch monitor show upstream-carrier
```

### Manual Testing

```bash
# Test connectivity to any endpoint
redfire-switch monitor ping 10.1.1.100:5060

# Test with custom timeout
redfire-switch monitor ping 10.1.1.100:5060 --timeout 10

# Test TCP endpoint
redfire-switch monitor ping 10.1.1.200:5061 --protocol tcp
```

## Monitoring Logs

### Log Levels

The monitoring system produces logs at different levels:

#### INFO Level
```
Starting SIP endpoint monitoring
Starting monitoring for endpoint: upstream-carrier (10.1.1.100:5060)
```

#### DEBUG Level
```
OPTIONS ping to upstream-carrier successful (45ms)
Received 200 OK response from 10.1.1.100:5060
```

#### WARN Level
```
OPTIONS ping to upstream-carrier failed (attempt 1): Ping timeout
```

#### ERROR Level
```
Endpoint upstream-carrier marked as offline after 3 consecutive failures
```

### Log Configuration

```bash
# Enable debug logging for detailed monitoring info
RUST_LOG=debug redfire-switch --verbose start

# Monitor-specific logging
RUST_LOG=redfire_switch::monitor=debug redfire-switch start

# JSON structured logging
RUST_LOG=debug redfire-switch start 2>&1 | jq
```

## Performance Considerations

### Interval Selection

Choose ping intervals based on requirements:

- **Critical systems**: 10-30 seconds
- **Standard monitoring**: 30-60 seconds  
- **Background monitoring**: 60-300 seconds

### Timeout Selection

Set timeouts based on network conditions:

- **Local network**: 2-5 seconds
- **Internet/WAN**: 5-15 seconds
- **Satellite/slow links**: 15-30 seconds

### Resource Usage

Monitoring resource consumption:

- **CPU**: Minimal per endpoint (< 0.1% per endpoint)
- **Memory**: ~1KB per endpoint for metrics
- **Network**: ~200 bytes per ping + response
- **Disk**: Log entries only

### Scaling Guidelines

| Endpoints | Recommended Intervals | Resource Impact |
|-----------|---------------------|-----------------|
| 1-10 | 10-30 seconds | Minimal |
| 10-50 | 30-60 seconds | Low |
| 50-100 | 60-120 seconds | Moderate |
| 100+ | 120-300 seconds | Plan accordingly |

## Integration with Switch Operations

### Automatic Startup

When the switch starts:

1. Configuration is loaded and validated
2. SIP server starts on configured profiles  
3. Monitoring system starts concurrently
4. Each enabled endpoint begins ping cycles
5. Health status updates in real-time

### Concurrent Operation

The monitoring system runs independently of call processing:

- **Non-blocking**: Monitoring doesn't affect SIP call handling
- **Separate threads**: Each endpoint monitored independently
- **Graceful failures**: Monitoring failures don't crash the switch
- **Resource isolation**: Monitoring has separate error handling

## Troubleshooting Monitoring

### Common Issues

#### Endpoint Shows as Offline
```bash
# Check connectivity manually
redfire-switch monitor ping 10.1.1.100:5060

# Verify endpoint configuration
redfire-switch monitor show upstream-carrier

# Check network connectivity
ping 10.1.1.100
telnet 10.1.1.100 5060
```

#### High Response Times
```bash
# Test with longer timeout
redfire-switch monitor ping 10.1.1.100:5060 --timeout 15

# Check network latency
ping 10.1.1.100

# Verify endpoint is responding
nmap -p 5060 10.1.1.100
```

#### Monitoring Not Starting
```bash
# Verify monitoring is enabled
redfire-switch show-config | grep -A 5 "monitoring"

# Check configuration validity
redfire-switch validate-config

# Review logs for errors
redfire-switch --verbose start
```

### Debugging Commands

```bash
# Test individual endpoint
redfire-switch monitor ping 10.1.1.100:5060 --protocol udp --timeout 5

# Show all monitoring configuration
redfire-switch show-config | jq '.monitoring'

# Check specific endpoint config
redfire-switch monitor show endpoint-name

# Monitor logs in real-time
redfire-switch --verbose start | grep -i monitor
```

## Future Enhancements

### Planned Features

#### Advanced Metrics
- Historical response time tracking
- Success rate trending
- Performance baselines
- Anomaly detection

#### Enhanced Monitoring
- TCP SIP monitoring support
- Custom SIP method monitoring
- Authentication support
- TLS/SIPS monitoring

#### Alerting Integration
- SNMP trap support
- Webhook notifications
- Email alerts
- Syslog integration

#### Management Interface
- REST API for monitoring data
- Web dashboard
- Real-time status updates
- Historical reporting

### Configuration Preview (Future)

```json
{
  "monitoring": {
    "enabled": true,
    "global_settings": {
      "max_concurrent_pings": 50,
      "retry_attempts": 3,
      "failure_threshold": 3,
      "recovery_threshold": 2
    },
    "alerting": {
      "enabled": true,
      "webhook_url": "https://alerts.example.com/webhook",
      "email_notifications": ["admin@example.com"]
    },
    "endpoints": [
      {
        "name": "carrier-1",
        "address": "10.1.1.100:5060",
        "protocol": "Udp",
        "enabled": true,
        "ping_interval_seconds": 30,
        "timeout_seconds": 5,
        "authentication": {
          "enabled": false,
          "username": "",
          "password": ""
        },
        "alerts": {
          "on_failure": true,
          "on_recovery": true,
          "threshold_ms": 1000
        }
      }
    ]
  }
}
```

## Best Practices

### Monitoring Strategy

1. **Monitor critical paths**: Focus on endpoints that affect service
2. **Appropriate intervals**: Balance timeliness with resource usage
3. **Reasonable timeouts**: Account for network conditions
4. **Gradual scaling**: Start with fewer endpoints, add incrementally

### Operational Procedures

1. **Regular review**: Check monitoring status periodically
2. **Baseline establishment**: Know normal response times
3. **Alert tuning**: Adjust thresholds to minimize false positives
4. **Documentation**: Keep endpoint purposes and contacts updated

### Configuration Management

1. **Version control**: Track configuration changes
2. **Testing**: Validate changes in development environment
3. **Rollback plan**: Keep working configurations backed up
4. **Documentation**: Document monitoring requirements and procedures