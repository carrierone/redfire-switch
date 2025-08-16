# Redfire Switch Configuration Guide

This guide provides comprehensive documentation for configuring Redfire Switch, covering all configuration options, best practices, and examples for different deployment scenarios.

## 📋 Table of Contents

- [Configuration Overview](#configuration-overview)
- [Configuration File Format](#configuration-file-format)
- [Core Configuration Sections](#core-configuration-sections)
- [SIP Profiles](#sip-profiles)
- [Routing Configuration](#routing-configuration)
- [STIR/SHAKEN Configuration](#stirshaken-configuration)
- [CDR Configuration](#cdr-configuration)
- [Monitoring Configuration](#monitoring-configuration)
- [Security Configuration](#security-configuration)
- [Performance Configuration](#performance-configuration)
- [Development Configuration](#development-configuration)
- [Environment Variables](#environment-variables)
- [Configuration Examples](#configuration-examples)
- [Validation and Testing](#validation-and-testing)

## 🎯 Configuration Overview

Redfire Switch uses JSON-based configuration files that are:

- **Human-readable** - Easy to edit and understand
- **Validated** - Automatic validation on startup
- **Reloadable** - Hot-reload without restart (planned)
- **Environment-aware** - Support for environment variables
- **Documented** - Every option is documented and explained

### Configuration Philosophy

1. **Sensible Defaults** - Works out of the box with minimal configuration
2. **Progressive Enhancement** - Start simple, add complexity as needed
3. **Environment-Specific** - Different configs for dev/test/prod
4. **Validation First** - Configuration errors caught early
5. **Documentation-Driven** - Every option is documented

## 📁 Configuration File Format

### Basic Structure

```json
{
  "sip_profiles": [...],      // SIP server listening profiles
  "routing": {...},           // Call routing configuration
  "trunks": [...],           // Termination providers
  "cdr": {...},              // Call Detail Records
  "stir_shaken": {...},      // STIR/SHAKEN authentication
  "monitoring": {...},        // Health monitoring
  "security": {...},         // Security policies
  "performance": {...},      // Performance tuning
  "debug": {...}             // Debug and development options
}
```

### Configuration Generation

```bash
# Generate default configuration
redfire-switch gen-config --output config.json

# Generate development configuration
redfire-switch gen-config --dev --output config-dev.json

# Generate production configuration
redfire-switch gen-config --prod --output config-prod.json
```

## 🏗️ Core Configuration Sections

### SIP Profiles

SIP profiles define how the switch listens for incoming SIP messages.

```json
{
  "sip_profiles": [
    {
      "name": "default",
      "bind_ip": "0.0.0.0",
      "port": 5060,
      "protocol": "Udp",
      "allowed_ips": ["0.0.0.0/0"],
      "max_connections": 10000,
      "timeout_seconds": 30,
      "buffer_size": 65536
    }
  ]
}
```

#### SIP Profile Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `name` | string | required | Unique profile identifier |
| `bind_ip` | string | "0.0.0.0" | IP address to bind to |
| `port` | number | 5060 | Port number to listen on |
| `protocol` | enum | "Udp" | Transport protocol (Udp, Tcp, Tls) |
| `allowed_ips` | array | ["0.0.0.0/0"] | Allowed IP ranges (CIDR notation) |
| `max_connections` | number | 10000 | Maximum concurrent connections |
| `timeout_seconds` | number | 30 | Connection timeout |
| `buffer_size` | number | 65536 | UDP receive buffer size |

#### Multiple Profiles Example

```json
{
  "sip_profiles": [
    {
      "name": "internal",
      "bind_ip": "10.1.1.10",
      "port": 5060,
      "protocol": "Udp",
      "allowed_ips": ["10.1.0.0/16", "192.168.0.0/16"]
    },
    {
      "name": "external",
      "bind_ip": "203.0.113.10",
      "port": 5060,
      "protocol": "Udp",
      "allowed_ips": ["203.0.113.0/24", "198.51.100.0/24"]
    },
    {
      "name": "secure",
      "bind_ip": "203.0.113.10",
      "port": 5061,
      "protocol": "Tls",
      "allowed_ips": ["0.0.0.0/0"],
      "tls_cert_path": "/etc/ssl/certs/sip.crt",
      "tls_key_path": "/etc/ssl/private/sip.key"
    }
  ]
}
```

### Routing Configuration

The routing section defines how calls are routed to termination providers.

```json
{
  "routing": {
    "engine": "simple",
    "routes": [
      {
        "prefix": "1",
        "trunk_id": "tier1_carrier",
        "priority": 1,
        "cost": 0.008,
        "capacity": 1000,
        "enabled": true
      }
    ],
    "emergency_routes": [
      {
        "numbers": ["911", "112", "999"],
        "trunk_id": "emergency_services",
        "priority": 1,
        "cost": 0.0
      }
    ],
    "default_route": {
      "trunk_id": "default_carrier",
      "cost": 0.02
    }
  }
}
```

#### Routing Options

| Option | Type | Description |
|--------|------|-------------|
| `engine` | string | Routing engine type ("simple", "advanced") |
| `routes` | array | List of routing rules |
| `emergency_routes` | array | Emergency number routing |
| `default_route` | object | Fallback route for unmatched calls |

#### Route Object Structure

| Field | Type | Description |
|-------|------|-------------|
| `prefix` | string | Dialed number prefix to match |
| `trunk_id` | string | Trunk identifier for termination |
| `priority` | number | Route priority (1 = highest) |
| `cost` | number | Cost per minute |
| `capacity` | number | Maximum concurrent calls |
| `enabled` | boolean | Route enabled/disabled |

### Trunk Configuration

Trunks define termination providers and their connection details.

```json
{
  "trunks": [
    {
      "id": "tier1_carrier",
      "name": "Tier 1 Carrier",
      "host": "sip.carrier.com",
      "port": 5060,
      "protocol": "Udp",
      "enabled": true,
      "authentication": {
        "type": "digest",
        "username": "your_username",
        "password": "your_password",
        "realm": "carrier.com"
      },
      "codec_preferences": ["PCMU", "PCMA", "G729"],
      "max_concurrent_calls": 1000,
      "quality_threshold": 0.95
    }
  ]
}
```

#### Trunk Options

| Option | Type | Description |
|--------|------|-------------|
| `id` | string | Unique trunk identifier |
| `name` | string | Human-readable trunk name |
| `host` | string | Termination provider hostname/IP |
| `port` | number | SIP port |
| `protocol` | enum | Transport protocol |
| `enabled` | boolean | Trunk enabled/disabled |
| `authentication` | object | Authentication credentials |
| `codec_preferences` | array | Preferred audio codecs |
| `max_concurrent_calls` | number | Capacity limit |
| `quality_threshold` | number | Minimum quality score |

## 🔐 STIR/SHAKEN Configuration

STIR/SHAKEN provides call authentication using JWT PASSporT tokens.

```json
{
  "stir_shaken": {
    "enabled": true,
    "service_provider_id": "your-sp-id",
    "authority": "your-authority.com",
    "certificate_url": "https://your-authority.com/cert.pem",
    "private_key_path": "/etc/ssl/private/stir-shaken.key",
    "cache_certificates": true,
    "cache_duration_hours": 24,
    "verification_timeout_seconds": 5,
    "attestation_level": "B"
  }
}
```

#### STIR/SHAKEN Options

| Option | Type | Description |
|--------|------|-------------|
| `enabled` | boolean | Enable/disable STIR/SHAKEN |
| `service_provider_id` | string | Your service provider ID |
| `authority` | string | Certificate authority domain |
| `certificate_url` | string | Public certificate URL |
| `private_key_path` | string | Private key file path |
| `cache_certificates` | boolean | Cache downloaded certificates |
| `cache_duration_hours` | number | Certificate cache duration |
| `verification_timeout_seconds` | number | Verification timeout |
| `attestation_level` | enum | Default attestation level (A, B, C) |

## 📊 CDR Configuration

Call Detail Records (CDR) configuration for storing call information.

```json
{
  "cdr": {
    "enabled": true,
    "storage_type": "clickhouse",
    "clickhouse": {
      "url": "http://localhost:8123",
      "database": "redfire_cdr",
      "table": "call_records",
      "username": "default",
      "password": "",
      "batch_size": 1000,
      "flush_interval_seconds": 60
    },
    "csv_backup": {
      "enabled": true,
      "directory": "/var/log/redfire-switch/cdr",
      "rotation": "daily",
      "compression": "gzip",
      "retention_days": 90
    },
    "fields": {
      "include_all": true,
      "custom_fields": {
        "customer_id": "string",
        "billing_code": "string"
      }
    }
  }
}
```

#### CDR Storage Options

| Option | Type | Description |
|--------|------|-------------|
| `enabled` | boolean | Enable CDR recording |
| `storage_type` | enum | Storage backend (clickhouse, postgresql, csv) |
| `batch_size` | number | Records per batch insert |
| `flush_interval_seconds` | number | Maximum time before flush |

#### ClickHouse Options

| Option | Type | Description |
|--------|------|-------------|
| `url` | string | ClickHouse server URL |
| `database` | string | Database name |
| `table` | string | Table name |
| `username` | string | Database username |
| `password` | string | Database password |

## 📡 Monitoring Configuration

Health monitoring and endpoint checking configuration.

```json
{
  "monitoring": {
    "enabled": true,
    "endpoints": [
      {
        "name": "tier1_carrier",
        "address": "203.0.113.100:5060",
        "protocol": "Udp",
        "enabled": true,
        "ping_interval_seconds": 30,
        "timeout_seconds": 5,
        "failure_threshold": 3,
        "recovery_threshold": 2
      }
    ],
    "health_checks": {
      "database": true,
      "disk_space": true,
      "memory_usage": true,
      "cpu_usage": true
    },
    "metrics": {
      "enabled": true,
      "endpoint": "/metrics",
      "port": 9090,
      "format": "prometheus"
    }
  }
}
```

#### Monitoring Options

| Option | Type | Description |
|--------|------|-------------|
| `enabled` | boolean | Enable monitoring |
| `endpoints` | array | SIP endpoints to monitor |
| `health_checks` | object | System health checks |
| `metrics` | object | Metrics export configuration |

## 🔒 Security Configuration

Security policies and access controls.

```json
{
  "security": {
    "authentication": {
      "enabled": true,
      "realm": "redfire-switch",
      "users": [
        {
          "username": "user1",
          "password_hash": "sha256_hash",
          "permissions": ["call", "register"]
        }
      ]
    },
    "rate_limiting": {
      "enabled": true,
      "calls_per_second": 10,
      "calls_per_minute": 100,
      "window_size_seconds": 60,
      "block_duration_seconds": 300
    },
    "fraud_detection": {
      "enabled": true,
      "max_call_duration_minutes": 180,
      "unusual_destinations": ["900", "976"],
      "geographic_restrictions": {
        "allowed_countries": ["US", "CA"],
        "blocked_countries": ["XX"]
      }
    },
    "firewall": {
      "enabled": true,
      "whitelist_mode": false,
      "blocked_ips": ["192.0.2.1"],
      "allowed_ips": ["203.0.113.0/24"]
    }
  }
}
```

## ⚡ Performance Configuration

Performance tuning and optimization settings.

```json
{
  "performance": {
    "worker_threads": 4,
    "max_connections": 10000,
    "connection_pool_size": 100,
    "message_queue_size": 10000,
    "timeout_settings": {
      "invite_timeout_seconds": 32,
      "register_timeout_seconds": 3600,
      "options_timeout_seconds": 5
    },
    "memory_management": {
      "max_memory_mb": 2048,
      "gc_threshold_mb": 1024,
      "connection_cache_size": 10000
    },
    "optimization": {
      "enable_message_pooling": true,
      "enable_connection_reuse": true,
      "enable_compression": false
    }
  }
}
```

## 🛠️ Development Configuration

Configuration options specific to development and debugging.

```json
{
  "debug": {
    "enabled": true,
    "single_call_mode": true,
    "verbose_logging": true,
    "packet_capture": {
      "enabled": true,
      "interface": "any",
      "filter": "port 5060",
      "output_file": "/tmp/sip-debug.pcap"
    },
    "sip_message_logging": {
      "log_all_messages": true,
      "log_message_body": true,
      "truncate_large_messages": true,
      "max_message_size": 2048
    },
    "test_mode": {
      "enabled": false,
      "mock_carriers": true,
      "simulate_network_delay": 50,
      "inject_errors": false
    }
  }
}
```

## 🌍 Environment Variables

Configuration can be overridden using environment variables:

### Core Settings
```bash
# Application settings
export RUST_LOG=debug
export RUST_BACKTRACE=1
export REDFIRE_CONFIG_PATH=/etc/redfire-switch/config.json

# SIP settings
export REDFIRE_SIP_BIND_IP=0.0.0.0
export REDFIRE_SIP_PORT=5060
export REDFIRE_SIP_PROTOCOL=udp

# Database settings
export REDFIRE_CDR_URL=http://localhost:8123
export REDFIRE_CDR_DATABASE=redfire_cdr
export REDFIRE_CDR_USERNAME=default
export REDFIRE_CDR_PASSWORD=secret

# Security settings
export REDFIRE_STIR_SHAKEN_ENABLED=true
export REDFIRE_STIR_SHAKEN_PRIVATE_KEY=/etc/ssl/private/stir-shaken.key

# Performance settings
export REDFIRE_WORKER_THREADS=4
export REDFIRE_MAX_CONNECTIONS=10000

# Debug settings
export REDFIRE_DEBUG_MODE=false
export REDFIRE_SINGLE_CALL_MODE=false
```

### Environment Variable Priority

1. Command-line flags (highest priority)
2. Environment variables
3. Configuration file
4. Default values (lowest priority)

## 📝 Configuration Examples

### Minimal Configuration

```json
{
  "sip_profiles": [
    {
      "name": "default",
      "bind_ip": "0.0.0.0",
      "port": 5060,
      "protocol": "Udp"
    }
  ],
  "routing": {
    "routes": [
      {
        "prefix": "1",
        "trunk_id": "default",
        "priority": 1,
        "cost": 0.01
      }
    ]
  },
  "trunks": [
    {
      "id": "default",
      "name": "Default Carrier",
      "host": "sip.carrier.com",
      "port": 5060,
      "protocol": "Udp",
      "enabled": true
    }
  ]
}
```

### Development Configuration

```json
{
  "sip_profiles": [
    {
      "name": "dev",
      "bind_ip": "127.0.0.1",
      "port": 5060,
      "protocol": "Udp",
      "allowed_ips": ["127.0.0.0/8"]
    }
  ],
  "routing": {
    "routes": [
      {
        "prefix": "test",
        "trunk_id": "local",
        "priority": 1,
        "cost": 0.0
      }
    ]
  },
  "trunks": [
    {
      "id": "local",
      "name": "Local Test",
      "host": "127.0.0.1",
      "port": 5061,
      "protocol": "Udp",
      "enabled": true
    }
  ],
  "cdr": {
    "enabled": false
  },
  "stir_shaken": {
    "enabled": false
  },
  "debug": {
    "enabled": true,
    "single_call_mode": true,
    "verbose_logging": true
  }
}
```

### Production Configuration

```json
{
  "sip_profiles": [
    {
      "name": "internal",
      "bind_ip": "10.1.1.10",
      "port": 5060,
      "protocol": "Udp",
      "allowed_ips": ["10.1.0.0/16"],
      "max_connections": 5000
    },
    {
      "name": "external",
      "bind_ip": "203.0.113.10",
      "port": 5060,
      "protocol": "Udp",
      "allowed_ips": ["203.0.113.0/24", "198.51.100.0/24"],
      "max_connections": 10000
    }
  ],
  "routing": {
    "routes": [
      {
        "prefix": "1",
        "trunk_id": "tier1_primary",
        "priority": 1,
        "cost": 0.008,
        "capacity": 1000
      },
      {
        "prefix": "1",
        "trunk_id": "tier1_backup",
        "priority": 2,
        "cost": 0.012,
        "capacity": 500
      },
      {
        "prefix": "011",
        "trunk_id": "international",
        "priority": 1,
        "cost": 0.15,
        "capacity": 100
      }
    ],
    "emergency_routes": [
      {
        "numbers": ["911"],
        "trunk_id": "emergency_911",
        "priority": 1,
        "cost": 0.0
      }
    ]
  },
  "trunks": [
    {
      "id": "tier1_primary",
      "name": "Tier 1 Primary",
      "host": "primary.carrier.com",
      "port": 5060,
      "protocol": "Udp",
      "enabled": true,
      "authentication": {
        "type": "digest",
        "username": "your_username",
        "password": "your_password"
      }
    },
    {
      "id": "tier1_backup",
      "name": "Tier 1 Backup",
      "host": "backup.carrier.com",
      "port": 5060,
      "protocol": "Udp",
      "enabled": true
    }
  ],
  "cdr": {
    "enabled": true,
    "storage_type": "clickhouse",
    "clickhouse": {
      "url": "http://clickhouse:8123",
      "database": "redfire_cdr",
      "table": "call_records"
    }
  },
  "stir_shaken": {
    "enabled": true,
    "service_provider_id": "your-sp-id",
    "authority": "your-authority.com",
    "certificate_url": "https://your-authority.com/cert.pem",
    "private_key_path": "/etc/ssl/private/stir-shaken.key"
  },
  "monitoring": {
    "enabled": true,
    "endpoints": [
      {
        "name": "tier1_primary",
        "address": "primary.carrier.com:5060",
        "protocol": "Udp",
        "enabled": true,
        "ping_interval_seconds": 30
      }
    ]
  },
  "security": {
    "rate_limiting": {
      "enabled": true,
      "calls_per_second": 100,
      "calls_per_minute": 1000
    },
    "fraud_detection": {
      "enabled": true,
      "max_call_duration_minutes": 180
    }
  },
  "performance": {
    "worker_threads": 8,
    "max_connections": 20000
  }
}
```

### High-Availability Configuration

```json
{
  "sip_profiles": [
    {
      "name": "primary",
      "bind_ip": "203.0.113.10",
      "port": 5060,
      "protocol": "Udp",
      "max_connections": 15000
    }
  ],
  "routing": {
    "routes": [
      {
        "prefix": "1",
        "trunk_id": "carrier_group_1",
        "priority": 1,
        "cost": 0.008
      }
    ]
  },
  "trunks": [
    {
      "id": "carrier_1_primary",
      "name": "Carrier 1 Primary",
      "host": "primary-1.carrier.com",
      "port": 5060,
      "enabled": true,
      "max_concurrent_calls": 2000
    },
    {
      "id": "carrier_1_backup",
      "name": "Carrier 1 Backup", 
      "host": "backup-1.carrier.com",
      "port": 5060,
      "enabled": true,
      "max_concurrent_calls": 1000
    }
  ],
  "monitoring": {
    "enabled": true,
    "endpoints": [
      {
        "name": "carrier_1_primary",
        "address": "primary-1.carrier.com:5060",
        "ping_interval_seconds": 15,
        "failure_threshold": 2
      },
      {
        "name": "carrier_1_backup",
        "address": "backup-1.carrier.com:5060",
        "ping_interval_seconds": 30,
        "failure_threshold": 3
      }
    ],
    "health_checks": {
      "database": true,
      "disk_space": true,
      "memory_usage": true
    }
  },
  "cdr": {
    "enabled": true,
    "storage_type": "clickhouse",
    "clickhouse": {
      "url": "http://clickhouse-cluster:8123",
      "database": "redfire_cdr_ha",
      "table": "call_records",
      "replication": true
    }
  }
}
```

## ✅ Validation and Testing

### Configuration Validation

```bash
# Validate configuration syntax
redfire-switch validate-config --config config.json

# Test configuration with dry-run
redfire-switch --config config.json --dry-run start

# Validate specific sections
redfire-switch validate-config --section routing --config config.json
```

### Configuration Testing

```bash
# Test SIP profile binding
redfire-switch test-bind --config config.json

# Test trunk connectivity
redfire-switch test-trunks --config config.json

# Test database connections
redfire-switch test-database --config config.json

# Test complete configuration
redfire-switch test-config --config config.json
```

### Common Validation Errors

1. **Invalid IP Address**
   ```json
   // ❌ Invalid
   "bind_ip": "999.999.999.999"
   
   // ✅ Correct
   "bind_ip": "192.168.1.10"
   ```

2. **Invalid Port Range**
   ```json
   // ❌ Invalid
   "port": 70000
   
   // ✅ Correct
   "port": 5060
   ```

3. **Missing Required Fields**
   ```json
   // ❌ Missing trunk_id
   {
     "prefix": "1",
     "priority": 1,
     "cost": 0.01
   }
   
   // ✅ Complete
   {
     "prefix": "1",
     "trunk_id": "carrier1",
     "priority": 1,
     "cost": 0.01
   }
   ```

### Configuration Migration

```bash
# Migrate from older version
redfire-switch migrate-config --from v0.0.1 --to v0.1.0 --config old-config.json

# Backup before migration
cp config.json config.json.backup

# Validate after migration
redfire-switch validate-config --config config.json
```

## 📚 Additional Resources

### Configuration Tools
- [JSON Schema Validator](https://www.jsonschemavalidator.net/)
- [JSON Formatter](https://jsonformatter.curiousconcept.com/)
- [CIDR Calculator](https://www.subnet-calculator.com/cidr.php)

### SIP Configuration
- [RFC 3261 - SIP Protocol](https://tools.ietf.org/html/rfc3261)
- [SIP URI Format](https://tools.ietf.org/html/rfc3261#section-19.1)
- [SIP Transport](https://tools.ietf.org/html/rfc3261#section-18)

### Security Configuration
- [STIR/SHAKEN Standards](https://tools.ietf.org/html/rfc8224)
- [SIP Security Best Practices](https://tools.ietf.org/html/rfc6189)

---

This configuration guide provides comprehensive coverage of all Redfire Switch configuration options. The combination of detailed explanations, practical examples, and validation tools ensures reliable configuration management for all deployment scenarios.

*For additional configuration support, see the [installation guide](INSTALLATION.md) or check the project's GitHub documentation.*