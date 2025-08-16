# Configuration Guide

This document provides comprehensive information about configuring Redfire Switch.

## Configuration File Format

Redfire Switch uses JSON configuration files that define SIP profiles, monitoring settings, and operational parameters.

## Configuration Structure

### Top-Level Configuration

```json
{
  "sip_profiles": [ ... ],
  "monitoring": { ... }
}
```

## SIP Profiles

SIP profiles define listening interfaces and access control for the switch.

### Basic SIP Profile

```json
{
  "name": "default",
  "bind_ip": "0.0.0.0",
  "port": 5060,
  "protocol": "Udp",
  "allowed_ips": ["127.0.0.1"]
}
```

### SIP Profile Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | - | Unique identifier for the profile |
| `bind_ip` | string | Yes | - | IP address to bind to (IPv4/IPv6) |
| `port` | number | No | 5060 | Port number to listen on |
| `protocol` | string | No | "Udp" | Transport protocol ("Udp" or "Tcp") |
| `allowed_ips` | array | Yes | - | List of allowed IP addresses/networks |

### IP Address Formats

The `allowed_ips` field supports various formats:

```json
{
  "allowed_ips": [
    "127.0.0.1",           // Single IP address
    "192.168.1.0/24",      // CIDR notation (future)
    "10.1.1.100",          // Another single IP
    "::1"                  // IPv6 localhost (future)
  ]
}
```

**Note**: Currently only single IP addresses are supported. CIDR notation support is planned for future releases.

### Multiple SIP Profiles Example

```json
{
  "sip_profiles": [
    {
      "name": "internal",
      "bind_ip": "192.168.1.10",
      "port": 5060,
      "protocol": "Udp",
      "allowed_ips": [
        "192.168.1.0",
        "192.168.1.1",
        "192.168.1.2"
      ]
    },
    {
      "name": "carrier-interface",
      "bind_ip": "10.1.1.10",
      "port": 5060,
      "protocol": "Udp",
      "allowed_ips": [
        "10.1.1.100",
        "10.1.1.101"
      ]
    },
    {
      "name": "secure-tcp",
      "bind_ip": "0.0.0.0",
      "port": 5061,
      "protocol": "Tcp",
      "allowed_ips": [
        "203.0.113.10"
      ]
    }
  ]
}
```

## Monitoring Configuration

The monitoring section configures SIP endpoint health checking using OPTIONS ping.

### Basic Monitoring Configuration

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

### Monitoring Fields

#### Monitoring Section

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `enabled` | boolean | No | true | Master enable/disable for monitoring |
| `endpoints` | array | No | [] | List of endpoints to monitor |

#### Endpoint Configuration

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | - | Unique identifier for the endpoint |
| `address` | string | Yes | - | Target address in IP:PORT format |
| `protocol` | string | No | "Udp" | Transport protocol ("Udp" or "Tcp") |
| `enabled` | boolean | No | true | Enable/disable monitoring for this endpoint |
| `ping_interval_seconds` | number | No | 30 | Seconds between health checks |
| `timeout_seconds` | number | No | 5 | Timeout for each ping attempt |

### Advanced Monitoring Example

```json
{
  "monitoring": {
    "enabled": true,
    "endpoints": [
      {
        "name": "primary-carrier",
        "address": "203.0.113.100:5060",
        "protocol": "Udp",
        "enabled": true,
        "ping_interval_seconds": 15,
        "timeout_seconds": 3
      },
      {
        "name": "backup-carrier",
        "address": "203.0.113.101:5060",
        "protocol": "Udp",
        "enabled": true,
        "ping_interval_seconds": 60,
        "timeout_seconds": 10
      },
      {
        "name": "internal-pbx",
        "address": "192.168.1.100:5060",
        "protocol": "Tcp",
        "enabled": false,
        "ping_interval_seconds": 30,
        "timeout_seconds": 5
      }
    ]
  }
}
```

## Complete Configuration Examples

### Minimal Configuration

```json
{
  "sip_profiles": [
    {
      "name": "default",
      "bind_ip": "0.0.0.0",
      "port": 5060,
      "protocol": "Udp",
      "allowed_ips": ["127.0.0.1"]
    }
  ],
  "monitoring": {
    "enabled": false,
    "endpoints": []
  }
}
```

### Production Configuration

```json
{
  "sip_profiles": [
    {
      "name": "customer-interface",
      "bind_ip": "192.168.100.10",
      "port": 5060,
      "protocol": "Udp",
      "allowed_ips": [
        "192.168.100.20",
        "192.168.100.21",
        "192.168.100.22"
      ]
    },
    {
      "name": "carrier-primary",
      "bind_ip": "10.1.1.10",
      "port": 5060,
      "protocol": "Udp",
      "allowed_ips": [
        "10.1.1.100",
        "10.1.1.101"
      ]
    },
    {
      "name": "carrier-backup",
      "bind_ip": "10.2.1.10",
      "port": 5060,
      "protocol": "Udp",
      "allowed_ips": [
        "10.2.1.100",
        "10.2.1.101"
      ]
    }
  ],
  "monitoring": {
    "enabled": true,
    "endpoints": [
      {
        "name": "carrier-1-primary",
        "address": "10.1.1.100:5060",
        "protocol": "Udp",
        "enabled": true,
        "ping_interval_seconds": 20,
        "timeout_seconds": 5
      },
      {
        "name": "carrier-1-secondary",
        "address": "10.1.1.101:5060",
        "protocol": "Udp",
        "enabled": true,
        "ping_interval_seconds": 20,
        "timeout_seconds": 5
      },
      {
        "name": "carrier-2-primary",
        "address": "10.2.1.100:5060",
        "protocol": "Udp",
        "enabled": true,
        "ping_interval_seconds": 30,
        "timeout_seconds": 8
      },
      {
        "name": "customer-pbx-1",
        "address": "192.168.100.20:5060",
        "protocol": "Udp",
        "enabled": true,
        "ping_interval_seconds": 60,
        "timeout_seconds": 10
      }
    ]
  }
}
```

## Configuration Management

### Generating Default Configuration

```bash
# Generate default config file
redfire-switch gen-config

# Generate with custom filename
redfire-switch gen-config --output /etc/redfire/config.json
```

### Validating Configuration

```bash
# Validate default config file
redfire-switch validate-config

# Validate specific config file
redfire-switch --config /path/to/config.json validate-config
```

### Viewing Configuration

```bash
# Show current configuration
redfire-switch show-config

# Show specific configuration file
redfire-switch --config /path/to/config.json show-config
```

## Configuration Best Practices

### Security

1. **Restrict IP Access**: Always specify allowed IPs, avoid using "0.0.0.0" in production
2. **Use Specific Interfaces**: Bind to specific IP addresses rather than 0.0.0.0
3. **Port Management**: Use non-standard ports for enhanced security

### Performance

1. **Monitoring Intervals**: 
   - Critical endpoints: 15-30 seconds
   - Less critical: 60-120 seconds
   - Development: 5-10 seconds

2. **Timeout Settings**:
   - Local network: 2-5 seconds
   - Internet endpoints: 5-15 seconds
   - Unreliable networks: 10-30 seconds

### Reliability

1. **Multiple Profiles**: Use separate profiles for different network segments
2. **Monitoring Coverage**: Monitor all critical upstream endpoints
3. **Graceful Degradation**: Enable monitoring for backup endpoints

### Example Tuning

```json
{
  "monitoring": {
    "enabled": true,
    "endpoints": [
      {
        "name": "critical-carrier",
        "ping_interval_seconds": 15,
        "timeout_seconds": 3
      },
      {
        "name": "backup-carrier", 
        "ping_interval_seconds": 60,
        "timeout_seconds": 10
      },
      {
        "name": "internal-service",
        "ping_interval_seconds": 30,
        "timeout_seconds": 5
      }
    ]
  }
}
```

## Configuration Schema Reference

### JSON Schema

For development and validation tools, here's the JSON schema structure:

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["sip_profiles"],
  "properties": {
    "sip_profiles": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["name", "bind_ip", "allowed_ips"],
        "properties": {
          "name": {"type": "string"},
          "bind_ip": {"type": "string"},
          "port": {"type": "integer", "minimum": 1, "maximum": 65535},
          "protocol": {"enum": ["Udp", "Tcp"]},
          "allowed_ips": {
            "type": "array",
            "items": {"type": "string"}
          }
        }
      }
    },
    "monitoring": {
      "type": "object",
      "properties": {
        "enabled": {"type": "boolean"},
        "endpoints": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["name", "address"],
            "properties": {
              "name": {"type": "string"},
              "address": {"type": "string"},
              "protocol": {"enum": ["Udp", "Tcp"]},
              "enabled": {"type": "boolean"},
              "ping_interval_seconds": {"type": "integer", "minimum": 1},
              "timeout_seconds": {"type": "integer", "minimum": 1}
            }
          }
        }
      }
    }
  }
}
```

## Troubleshooting Configuration

### Common Issues

1. **Invalid IP Address Format**
   ```
   Error: invalid IP address
   ```
   Solution: Ensure IP addresses are in valid IPv4 format (e.g., "192.168.1.1")

2. **Port Already in Use**
   ```
   Error: Address already in use
   ```
   Solution: Change port number or stop conflicting service

3. **Permission Denied**
   ```
   Error: Permission denied
   ```
   Solution: Use ports > 1024 or run with appropriate privileges

4. **JSON Syntax Error**
   ```
   Error: expected `,` or `}` at line X
   ```
   Solution: Validate JSON syntax using `redfire-switch validate-config`

### Validation Errors

Common validation errors and solutions:

```bash
# Missing required field
Error: missing field `name` at line 5

# Invalid protocol value  
Error: unknown variant `TCP`, expected `Udp` or `Tcp`

# Invalid port range
Error: port must be between 1 and 65535
```