# Command Line Interface (CLI) Reference

This document provides comprehensive information about using the Redfire Switch command-line interface.

## Overview

The Redfire Switch CLI provides a complete set of commands for managing, monitoring, and operating the SIP switch. All operations can be performed through the command line interface.

## Global Options

These options are available for all commands:

```bash
redfire-switch [GLOBAL OPTIONS] [COMMAND] [COMMAND OPTIONS]
```

### Global Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--config` | `-c` | Configuration file path | `config.json` |
| `--verbose` | `-v` | Enable verbose logging | `false` |
| `--help` | `-h` | Print help information | - |
| `--version` | `-V` | Print version information | - |

### Examples

```bash
# Use custom config file
redfire-switch --config /etc/redfire/config.json start

# Enable verbose logging
redfire-switch --verbose start

# Show version
redfire-switch --version

# Show help
redfire-switch --help
```

## Core Commands

### start

Start the SIP switch with optional monitoring.

```bash
redfire-switch start
```

**Description**: Starts the SIP server and monitoring system based on the configuration file. The process runs in the foreground and will continue until interrupted.

**Behavior**:
- Loads and validates configuration
- Starts SIP server on configured profiles
- Starts monitoring system if enabled
- Runs both systems concurrently
- Logs all activity to stdout

**Examples**:
```bash
# Start with default config
redfire-switch start

# Start with custom config and verbose logging
redfire-switch --config production.json --verbose start

# Start in background (Unix)
redfire-switch start &

# Start with systemd
systemctl start redfire-switch
```

### gen-config

Generate a default configuration file.

```bash
redfire-switch gen-config [OPTIONS]
```

**Options**:
| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--output` | `-o` | Output file path | `config.json` |

**Examples**:
```bash
# Generate default config
redfire-switch gen-config

# Generate with custom filename
redfire-switch gen-config --output /etc/redfire/config.json

# Generate and immediately validate
redfire-switch gen-config && redfire-switch validate-config
```

**Output**: Creates a JSON configuration file with:
- Default SIP profile (UDP, port 5060, localhost only)
- Example monitoring endpoint (disabled)
- All default values populated

### validate-config

Validate the configuration file for syntax and semantic errors.

```bash
redfire-switch validate-config
```

**Description**: Checks the configuration file for:
- Valid JSON syntax
- Required fields present
- Valid field values and ranges
- IP address format validation
- Port number validation

**Exit Codes**:
- `0`: Configuration is valid
- `1`: Configuration is invalid

**Examples**:
```bash
# Validate default config
redfire-switch validate-config

# Validate specific config
redfire-switch --config production.json validate-config

# Use in scripts
if redfire-switch validate-config; then
    echo "Config is valid"
    redfire-switch start
else
    echo "Config has errors"
    exit 1
fi
```

### show-config

Display the current configuration in JSON format.

```bash
redfire-switch show-config
```

**Description**: Loads and displays the configuration file with pretty-printing. Useful for:
- Verifying configuration values
- Debugging configuration issues
- Sharing configuration examples

**Examples**:
```bash
# Show default config
redfire-switch show-config

# Show specific config
redfire-switch --config production.json show-config

# Save config to file
redfire-switch show-config > current-config.json

# Format config file
redfire-switch --config messy.json show-config > clean.json
```

## Monitoring Commands

All monitoring commands are accessed through the `monitor` subcommand:

```bash
redfire-switch monitor [MONITOR_COMMAND] [OPTIONS]
```

### monitor status

Show status of all monitored endpoints.

```bash
redfire-switch monitor status
```

**Description**: Displays a summary of all configured monitoring endpoints with their current status and settings.

**Output Format**:
```
SIP Endpoint Monitoring Status:
==============================
  endpoint-name (ip:port) - Status - Interval: XXs
```

**Examples**:
```bash
# Show monitoring status
redfire-switch monitor status

# Show status with custom config
redfire-switch --config production.json monitor status
```

**Sample Output**:
```
SIP Endpoint Monitoring Status:
==============================
  upstream-carrier (10.1.1.100:5060) - Enabled - Interval: 30s
  backup-carrier (10.2.1.100:5060) - Disabled - Interval: 60s
  internal-pbx (192.168.1.100:5060) - Enabled - Interval: 30s
```

### monitor show

Show detailed information about a specific endpoint.

```bash
redfire-switch monitor show <endpoint-name>
```

**Arguments**:
- `endpoint-name`: Name of the endpoint as defined in configuration

**Examples**:
```bash
# Show details of specific endpoint
redfire-switch monitor show upstream-carrier

# Show endpoint from custom config
redfire-switch --config production.json monitor show backup-carrier
```

**Sample Output**:
```
Endpoint: upstream-carrier
Address: 10.1.1.100:5060
Protocol: Udp
Enabled: true
Ping Interval: 30 seconds
Timeout: 5 seconds
```

### monitor enable/disable

Enable or disable monitoring for a specific endpoint.

```bash
redfire-switch monitor enable <endpoint-name>
redfire-switch monitor disable <endpoint-name>
```

**Arguments**:
- `endpoint-name`: Name of the endpoint as defined in configuration

**Note**: These commands currently provide guidance for manual configuration updates. Dynamic enable/disable functionality is planned for future releases.

**Examples**:
```bash
# Enable endpoint monitoring
redfire-switch monitor enable upstream-carrier

# Disable endpoint monitoring  
redfire-switch monitor disable backup-carrier
```

**Sample Output**:
```
Enable endpoint 'upstream-carrier' (requires manual config update)
Set 'enabled: true' for endpoint 'upstream-carrier' in config.json
```

### monitor ping

Test SIP OPTIONS ping to any endpoint.

```bash
redfire-switch monitor ping <target> [OPTIONS]
```

**Arguments**:
- `target`: Target address in IP:PORT format (e.g., `192.168.1.100:5060`)

**Options**:
| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--protocol` | `-p` | Protocol (udp/tcp) | `udp` |
| `--timeout` | `-t` | Timeout in seconds | `5` |

**Description**: Sends a single SIP OPTIONS request to test connectivity and measure response time. Useful for:
- Testing connectivity to SIP endpoints
- Troubleshooting network issues
- Verifying SIP server responses
- Measuring response times

**Exit Codes**:
- `0`: Ping successful
- `1`: Ping failed

**Examples**:
```bash
# Basic ping test
redfire-switch monitor ping 192.168.1.100:5060

# Ping with TCP and custom timeout
redfire-switch monitor ping 10.1.1.100:5060 --protocol tcp --timeout 10

# Quick connectivity test
redfire-switch monitor ping 203.0.113.100:5060 --timeout 3

# Use in scripts
if redfire-switch monitor ping 192.168.1.100:5060; then
    echo "Endpoint is reachable"
else
    echo "Endpoint is down"
fi
```

**Sample Output (Success)**:
```
Pinging 192.168.1.100:5060 via Udp...
✓ Ping successful - Response time: 45ms
```

**Sample Output (Failure)**:
```
Pinging 192.168.1.100:5060 via Udp...
✗ Ping failed: Ping timeout
```

## Usage Patterns

### Development Workflow

```bash
# 1. Generate initial configuration
redfire-switch gen-config

# 2. Edit configuration as needed
vim config.json

# 3. Validate configuration
redfire-switch validate-config

# 4. Test connectivity to endpoints
redfire-switch monitor ping 192.168.1.100:5060

# 5. Start with verbose logging
redfire-switch --verbose start
```

### Production Deployment

```bash
# 1. Generate production config
redfire-switch gen-config --output /etc/redfire/config.json

# 2. Edit production configuration
vim /etc/redfire/config.json

# 3. Validate production config
redfire-switch --config /etc/redfire/config.json validate-config

# 4. Test all monitoring endpoints
redfire-switch --config /etc/redfire/config.json monitor status

# 5. Start production service
systemctl start redfire-switch
```

### Troubleshooting Workflow

```bash
# 1. Check configuration
redfire-switch validate-config
redfire-switch show-config

# 2. Test individual endpoints
redfire-switch monitor ping 10.1.1.100:5060
redfire-switch monitor show upstream-carrier

# 3. Check monitoring status
redfire-switch monitor status

# 4. Start with debug logging
redfire-switch --verbose start
```

### Monitoring Health Checks

```bash
# Check all endpoints
redfire-switch monitor status

# Test specific endpoint
redfire-switch monitor ping 10.1.1.100:5060

# Detailed endpoint info
redfire-switch monitor show critical-carrier

# Quick connectivity test
redfire-switch monitor ping 192.168.1.100:5060 --timeout 2
```

## Environment Variables

The CLI respects certain environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Logging level | `info` |
| `REDFIRE_CONFIG` | Default config file path | `config.json` |

**Examples**:
```bash
# Enable debug logging
RUST_LOG=debug redfire-switch start

# Use environment config path
REDFIRE_CONFIG=/etc/redfire/config.json redfire-switch start

# Combine environment variables
RUST_LOG=debug REDFIRE_CONFIG=test.json redfire-switch --verbose start
```

## Exit Codes

Standard exit codes used by Redfire Switch:

| Code | Meaning | Commands |
|------|---------|----------|
| `0` | Success | All commands |
| `1` | General error | All commands |
| `2` | Configuration error | validate-config, start |
| `3` | Network error | monitor ping, start |
| `4` | Permission error | start |

## Script Integration

### Bash Examples

```bash
#!/bin/bash

# Health check script
check_endpoint() {
    local endpoint=$1
    if redfire-switch monitor ping "$endpoint" --timeout 5; then
        echo "✓ $endpoint is healthy"
        return 0
    else
        echo "✗ $endpoint is down"
        return 1
    fi
}

# Check multiple endpoints
endpoints=("10.1.1.100:5060" "10.2.1.100:5060" "192.168.1.100:5060")
for endpoint in "${endpoints[@]}"; do
    check_endpoint "$endpoint"
done
```

### Service Management

```bash
# Systemd service script
#!/bin/bash
REDFIRE_CONFIG="/etc/redfire/config.json"

case "$1" in
    start)
        redfire-switch --config "$REDFIRE_CONFIG" validate-config
        if [ $? -eq 0 ]; then
            redfire-switch --config "$REDFIRE_CONFIG" start
        else
            echo "Configuration validation failed"
            exit 1
        fi
        ;;
    validate)
        redfire-switch --config "$REDFIRE_CONFIG" validate-config
        ;;
    status)
        redfire-switch --config "$REDFIRE_CONFIG" monitor status
        ;;
    *)
        echo "Usage: $0 {start|validate|status}"
        exit 1
        ;;
esac
```

## Advanced Usage

### Configuration Management

```bash
# Backup current config
cp config.json config.json.backup.$(date +%Y%m%d)

# Generate new config and compare
redfire-switch gen-config --output config.json.new
diff config.json config.json.new

# Validate multiple configs
for config in configs/*.json; do
    echo "Validating $config..."
    redfire-switch --config "$config" validate-config
done
```

### Monitoring Automation

```bash
# Monitor all configured endpoints
redfire-switch monitor status | grep -E "(Enabled|Disabled)"

# Test connectivity to all endpoints
redfire-switch show-config | jq -r '.monitoring.endpoints[].address' | while read endpoint; do
    redfire-switch monitor ping "$endpoint"
done

# Health check with notification
if ! redfire-switch monitor ping 10.1.1.100:5060; then
    mail -s "SIP Endpoint Down" admin@example.com < /dev/null
fi
```