# Redfire Switch Development Environment

This document describes the SIPp-based development and testing environment for the Redfire Switch.

## Overview

The development environment provides:
- **Docker containerized setup** for consistent testing
- **SIPp scenarios** for comprehensive SIP testing
- **Debug mode** for single-call testing and troubleshooting
- **Automated test scripts** for CI/CD integration
- **Packet capture and monitoring** tools
- **Wireshark integration** for protocol analysis

## Quick Start

### Prerequisites

Install required tools:
```bash
# Install dependencies
make install-deps

# Or manually:
sudo apt-get install -y sipp tcpdump tshark docker.io docker-compose
```

### Basic Usage

1. **Start development environment:**
   ```bash
   make dev
   ```

2. **Run all tests:**
   ```bash
   make test
   ```

3. **Test single call in debug mode:**
   ```bash
   make debug
   ```

4. **Monitor SIP traffic:**
   ```bash
   make pcap-live
   ```

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   SIPp UAC      │    │ Redfire Switch  │    │   SIPp UAS      │
│  (Test Client)  │───▶│   (Debug Mode)  │───▶│ (Test Server)   │
│   Port 5xxx     │    │   Port 5060     │    │   Port 5061     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                        ┌─────────────────┐
                        │   Wireshark     │
                        │ (Packet Capture)│
                        │ http://localhost│
                        │      :3000      │
                        └─────────────────┘
```

## Test Scenarios

### 1. OPTIONS Ping Test
**File:** `tests/sipp/scenarios/options_ping.xml`
**Purpose:** Basic connectivity and capability testing
```bash
make test-options
```

### 2. Basic Call Flow Test
**File:** `tests/sipp/scenarios/basic_call_uac.xml`
**Purpose:** End-to-end call establishment and teardown
```bash
make test-call
```

### 3. REGISTER Test
**File:** `tests/sipp/scenarios/register_test.xml`
**Purpose:** Registration flow with authentication
```bash
make test-register
```

### 4. Stress Test
**File:** `tests/sipp/scenarios/stress_test.xml`
**Purpose:** High-volume call testing
```bash
make test-stress
```

## Debug Mode

The Redfire Switch includes a special debug mode designed for development:

### Features
- **Single call mode** - Process one call then exit
- **Verbose logging** - Detailed SIP message logging
- **Message dumping** - Complete packet dumps
- **Call flow tracing** - Step-by-step call progression
- **Exit after call** - Automatic shutdown for testing

### Configuration
Debug configuration: `config-dev.json`
```json
{
  "debug": {
    "enabled": true,
    "single_call_mode": true,
    "exit_after_call": true,
    "verbose_logging": true,
    "log_file": "/app/logs/debug.log",
    "pcap_file": "/app/pcaps/debug.pcap"
  }
}
```

### Usage
```bash
# Run in debug mode (local)
make debug

# Run in debug mode (Docker)
make docker-up
make debug-docker

# Debug with GDB
make debug-gdb
```

## Docker Environment

### Services

1. **redfire-switch** - The switch in debug mode
   - Ports: 5060/UDP, 5060/TCP, 8080 (REST API)
   - Debug features enabled
   - Logs mounted to `./logs`

2. **sipp-uas** - SIPp User Agent Server
   - Port: 5061
   - Responds to incoming calls
   - Scenarios in `./tests/sipp/scenarios`

3. **wireshark** - Web-based packet analyzer
   - Port: 3000
   - GUI accessible via browser
   - Captures saved to `./pcaps`

### Commands
```bash
# Start environment
make docker-up

# View logs
make docker-logs

# Stop environment
make docker-down

# Rebuild containers
make docker-build
```

## Testing Workflows

### Development Cycle
```bash
# 1. Start development environment
make dev

# 2. Make code changes
# ... edit files ...

# 3. Run tests
make test

# 4. Debug specific issues
make debug

# 5. Monitor traffic
make pcap-live
```

### CI Pipeline
```bash
# Complete CI simulation
make ci
```

### Manual Testing
```bash
# Single call test
sipp -sn uac -r 1 -m 1 localhost:5060

# Stress test
sipp -sn uac -r 10 -m 100 localhost:5060

# Custom scenario
sipp -sf tests/sipp/scenarios/basic_call_uac.xml localhost:5060
```

## Monitoring and Debugging

### Packet Capture
```bash
# Live monitoring
make pcap-live

# Background capture
make pcap

# View with Wireshark
make wireshark
```

### Log Analysis
```bash
# Tail all logs
make logs

# View specific logs
tail -f logs/debug.log

# Docker logs
make docker-logs
```

### SIP Message Analysis
```bash
# Real-time SIP monitoring
make tshark

# Raw packet dump
tcpdump -i any -A port 5060
```

## Configuration Files

### Development Config (`config-dev.json`)
- Debug mode enabled
- Single call mode
- Verbose logging
- Test routing table
- Simplified features

### SIPp Scenarios
- **OPTIONS ping** - Basic connectivity
- **UAC call** - Outbound call testing
- **REGISTER** - Authentication flow
- **Stress test** - Performance testing

### Docker Compose (`docker-compose.dev.yml`)
- Multi-service setup
- Network isolation
- Volume mounts for logs/pcaps
- Debug ports exposed

## Troubleshooting

### Common Issues

1. **Port conflicts**
   ```bash
   # Check port usage
   netstat -tulpn | grep 5060
   
   # Stop conflicting services
   sudo systemctl stop asterisk
   ```

2. **Docker permission issues**
   ```bash
   # Add user to docker group
   sudo usermod -aG docker $USER
   ```

3. **SIPp not found**
   ```bash
   # Install SIPp
   sudo apt-get install sipp
   ```

4. **Switch not responding**
   ```bash
   # Check logs
   make logs
   
   # Verify network
   ping localhost
   ```

### Debug Commands

```bash
# Check compilation
make check

# Format code
make fmt

# Run linter
make clippy

# Generate docs
make doc

# Show statistics
make stats
```

## Advanced Usage

### Custom SIPp Scenarios

Create new scenarios in `tests/sipp/scenarios/`:
```xml
<?xml version="1.0" encoding="ISO-8859-1" ?>
<!DOCTYPE scenario SYSTEM "sipp.dtd">
<scenario name="Custom Test">
  <!-- Your scenario here -->
</scenario>
```

### Performance Testing

```bash
# High call rate
CALL_RATE=50 make test-stress

# Long duration
NUM_CALLS=1000 make test-stress

# Custom stress test
sipp -r 100 -d 30000 -l 50 localhost:5060
```

### Integration Testing

```bash
# Test with external systems
SWITCH_HOST=192.168.1.100 make test

# Test specific protocols
sipp -t t1 localhost:5060  # TCP
sipp -t u1 localhost:5060  # UDP
```

## File Structure

```
├── docker-compose.dev.yml     # Docker environment
├── Dockerfile.dev             # Development container
├── config-dev.json           # Debug configuration
├── Makefile.dev              # Development commands
├── tests/
│   ├── run-tests.sh          # Test runner script
│   └── sipp/
│       ├── scenarios/        # SIPp test scenarios
│       ├── data/            # Test data files
│       └── logs/            # SIPp logs
├── logs/                    # Switch logs
├── pcaps/                   # Packet captures
└── DEV-ENVIRONMENT.md       # This documentation
```

## Contributing

When adding new test scenarios:

1. Create XML scenario in `tests/sipp/scenarios/`
2. Add test command to `Makefile.dev`
3. Update `run-tests.sh` if needed
4. Document the scenario in this file
5. Test with `make test`

## Support

For issues with the development environment:
1. Check logs: `make logs`
2. Verify Docker: `make docker-logs`
3. Test connectivity: `make test-options`
4. Review configuration: `config-dev.json`
5. Report issues with debug output