# Redfire Switch Installation Guide

This guide covers installation and setup of Redfire Switch for development and production environments.

## 📋 Table of Contents

- [Development Setup](#development-setup)
- [Production Installation](#production-installation)
- [System Requirements](#system-requirements)
- [Platform-Specific Installation](#platform-specific-installation)
- [Configuration](#configuration)
- [Verification](#verification)
- [Troubleshooting](#troubleshooting)

## 🚀 Development Setup

### Quick Development Setup

The fastest way to get started with development:

```bash
# Clone the repository
git clone https://github.com/carrierone/redfire-switch.git
cd redfire-switch

# One-click setup (installs dependencies, sets up environment)
./setup-dev.sh

# Start development environment
make dev

# Run tests to verify everything works
make test
```

This sets up:
- ✅ Rust toolchain and dependencies
- ✅ Docker environment for testing
- ✅ SIPp for protocol testing
- ✅ Network tools for debugging
- ✅ Complete development environment

### Manual Development Setup

If you prefer manual setup:

```bash
# 1. Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 2. Install system dependencies
sudo apt-get update
sudo apt-get install -y \
    build-essential pkg-config libssl-dev \
    docker.io docker-compose \
    sipp tcpdump tshark \
    git curl wget

# 3. Clone and build
git clone https://github.com/carrierone/redfire-switch.git
cd redfire-switch
cargo build

# 4. Generate development config
./target/debug/redfire-switch gen-config -o config-dev.json

# 5. Test the setup
cargo test
```

## 🏭 Production Installation

### System Requirements

**Minimum Requirements:**
- **CPU**: 2 cores (4+ recommended)
- **RAM**: 2GB (8GB+ recommended for production)
- **Storage**: 10GB (SSD recommended)
- **Network**: 1Gbps+ for high-volume traffic
- **OS**: Linux kernel 5.x+ (Ubuntu 20.04+, RHEL 8+, etc.)

**Network Requirements:**
- **SIP**: 5060/udp, 5060/tcp
- **RTP**: 10000-20000/udp (configurable range)
- **Management**: 8080/tcp (REST API)
- **Monitoring**: 9090/tcp (metrics)

**Recommended External Services:**
- **Database**: ClickHouse for CDR storage
- **Monitoring**: Prometheus + Grafana
- **Load Balancer**: HAProxy or Nginx
- **Certificate Authority**: For STIR/SHAKEN certificates

### Production Build

```bash
# 1. Install Rust (production version)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# 2. Install system dependencies
sudo apt-get update
sudo apt-get install -y \
    build-essential pkg-config libssl-dev \
    ca-certificates curl wget

# 3. Clone and build release version
git clone https://github.com/carrierone/redfire-switch.git
cd redfire-switch
cargo build --release

# 4. Install binary
sudo cp target/release/redfire-switch /usr/local/bin/
sudo chmod +x /usr/local/bin/redfire-switch

# 5. Create system user
sudo useradd --system --home /var/lib/redfire-switch redfire-switch
sudo mkdir -p /var/lib/redfire-switch /var/log/redfire-switch
sudo chown redfire-switch:redfire-switch /var/lib/redfire-switch /var/log/redfire-switch
```

### SystemD Service Setup

```bash
# Create systemd service file
sudo tee /etc/systemd/system/redfire-switch.service > /dev/null << 'EOF'
[Unit]
Description=Redfire Switch SIP Server
After=network.target
Wants=network.target

[Service]
Type=simple
User=redfire-switch
Group=redfire-switch
WorkingDirectory=/var/lib/redfire-switch
ExecStart=/usr/local/bin/redfire-switch --config /etc/redfire-switch/config.json start
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=5
LimitNOFILE=65536

# Security settings
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/var/lib/redfire-switch /var/log/redfire-switch

[Install]
WantedBy=multi-user.target
EOF

# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable redfire-switch
```

## 🖥️ Platform-Specific Installation

### Ubuntu/Debian

```bash
# Install dependencies
sudo apt-get update
sudo apt-get install -y \
    build-essential pkg-config libssl-dev \
    git curl wget ca-certificates

# Install Rust and build
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
git clone https://github.com/carrierone/redfire-switch.git
cd redfire-switch
cargo build --release
```

### CentOS/RHEL/Rocky Linux

```bash
# Install dependencies
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y \
    pkg-config openssl-devel \
    git curl wget ca-certificates

# Install Rust and build
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
git clone https://github.com/carrierone/redfire-switch.git
cd redfire-switch
cargo build --release
```

### NixOS

A NixOS module is provided for easy installation:

```nix
# /etc/nixos/configuration.nix
{ config, pkgs, ... }:

{
  imports = [
    ./redfire-switch/nixos/module.nix
  ];

  services.redfire-switch = {
    enable = true;
    package = pkgs.callPackage ./redfire-switch/nixos/package.nix {};
    config = {
      sip_profiles = [{
        name = "default";
        bind_ip = "0.0.0.0";
        port = 5060;
        protocol = "Udp";
        allowed_ips = ["0.0.0.0/0"];
      }];
    };
  };
}
```

### Docker Deployment

For containerized deployment:

```bash
# Build Docker image
docker build -t redfire-switch .

# Run with Docker Compose
cat > docker-compose.yml << 'EOF'
version: '3.8'

services:
  redfire-switch:
    image: redfire-switch:latest
    ports:
      - "5060:5060/udp"
      - "5060:5060/tcp"
      - "8080:8080"
    volumes:
      - ./config.json:/app/config.json
      - ./logs:/app/logs
    environment:
      - RUST_LOG=info
    restart: unless-stopped

  clickhouse:
    image: clickhouse/clickhouse-server:latest
    ports:
      - "8123:8123"
      - "9000:9000"
    volumes:
      - clickhouse_data:/var/lib/clickhouse
    environment:
      - CLICKHOUSE_USER=default
      - CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT=1

volumes:
  clickhouse_data:
EOF

docker-compose up -d
```

## ⚙️ Configuration

### Basic Configuration

Generate a basic configuration file:

```bash
# Generate default configuration
redfire-switch gen-config --output /etc/redfire-switch/config.json

# Example minimal configuration
cat > /etc/redfire-switch/config.json << 'EOF'
{
  "sip_profiles": [
    {
      "name": "default",
      "bind_ip": "0.0.0.0",
      "port": 5060,
      "protocol": "Udp",
      "allowed_ips": ["0.0.0.0/0"]
    }
  ],
  "routing": {
    "routes": [
      {
        "prefix": "1",
        "trunk_id": "carrier1",
        "priority": 1,
        "cost": 0.01
      }
    ]
  },
  "trunks": [
    {
      "id": "carrier1",
      "name": "Primary Carrier",
      "host": "sip.carrier.com",
      "port": 5060,
      "protocol": "Udp",
      "enabled": true
    }
  ],
  "cdr": {
    "enabled": true,
    "storage_type": "clickhouse",
    "clickhouse": {
      "url": "http://localhost:8123",
      "database": "redfire_cdr",
      "table": "call_records"
    }
  },
  "stir_shaken": {
    "enabled": false
  }
}
EOF
```

### Production Configuration

For production, you'll need:

1. **Multiple SIP Profiles** for different networks
2. **Proper Routing Tables** with your carriers
3. **STIR/SHAKEN Configuration** with valid certificates
4. **CDR Storage** with ClickHouse or database
5. **Monitoring Configuration** for health checks

Example production configuration:

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
    }
  ],
  "routing": {
    "routes": [
      {"prefix": "1", "trunk_id": "tier1_carrier", "priority": 1, "cost": 0.008},
      {"prefix": "1", "trunk_id": "tier2_carrier", "priority": 2, "cost": 0.012},
      {"prefix": "011", "trunk_id": "international", "priority": 1, "cost": 0.15},
      {"prefix": "911", "trunk_id": "emergency", "priority": 1, "cost": 0.0}
    ]
  },
  "monitoring": {
    "enabled": true,
    "endpoints": [
      {
        "name": "tier1_carrier",
        "address": "203.0.113.100:5060",
        "protocol": "Udp",
        "enabled": true,
        "ping_interval_seconds": 30,
        "timeout_seconds": 5
      }
    ]
  },
  "stir_shaken": {
    "enabled": true,
    "service_provider_id": "your-sp-id",
    "authority": "your-authority.com",
    "certificate_url": "https://your-authority.com/cert.pem",
    "private_key_path": "/etc/ssl/private/stir-shaken.key"
  }
}
```

### Environment Variables

Set these environment variables for production:

```bash
# Application settings
export RUST_LOG=info
export RUST_BACKTRACE=1

# Configuration
export REDFIRE_CONFIG=/etc/redfire-switch/config.json

# Security
export REDFIRE_PRIVATE_KEY_PATH=/etc/ssl/private/
export REDFIRE_CERT_PATH=/etc/ssl/certs/

# Performance
export REDFIRE_WORKER_THREADS=4
export REDFIRE_MAX_CONNECTIONS=10000
```

## ✅ Verification

### Test Installation

```bash
# 1. Check binary installation
redfire-switch --version

# 2. Validate configuration
redfire-switch validate-config --config /etc/redfire-switch/config.json

# 3. Test SIP server (debug mode)
redfire-switch --debug --config /etc/redfire-switch/config.json start

# 4. In another terminal, test with SIPp
sipp -sn uac -m 1 localhost:5060

# 5. Check logs
journalctl -u redfire-switch -f
```

### Health Check

```bash
# Check if service is running
sudo systemctl status redfire-switch

# Test SIP OPTIONS ping
redfire-switch monitor ping localhost:5060

# Check REST API
curl http://localhost:8080/health

# View real-time stats
redfire-switch monitor status
```

### Performance Test

```bash
# Basic load test with SIPp
sipp -sn uac -r 10 -m 100 localhost:5060

# Monitor performance
htop
iotop
nethogs
```

## 🔧 Troubleshooting

### Common Issues

1. **Permission Denied on Port 5060**
   ```bash
   # Allow binding to privileged ports
   sudo setcap 'cap_net_bind_service=+ep' /usr/local/bin/redfire-switch
   ```

2. **Configuration Validation Errors**
   ```bash
   # Check configuration syntax
   redfire-switch validate-config --config config.json
   
   # View detailed error messages
   RUST_LOG=debug redfire-switch --config config.json start
   ```

3. **SIP Messages Not Processing**
   ```bash
   # Check firewall rules
   sudo ufw allow 5060/udp
   sudo ufw allow 5060/tcp
   
   # Monitor network traffic
   sudo tcpdump -i any port 5060
   ```

4. **High Memory Usage**
   ```bash
   # Check for memory leaks
   valgrind --leak-check=full redfire-switch --config config.json start
   
   # Monitor memory usage
   ps aux | grep redfire-switch
   ```

5. **Database Connection Issues**
   ```bash
   # Test ClickHouse connection
   curl "http://localhost:8123/?query=SELECT%201"
   
   # Check database logs
   docker logs clickhouse
   ```

### Debug Mode

For troubleshooting, use debug mode:

```bash
# Enable debug logging
RUST_LOG=debug redfire-switch --config config.json start

# Single call debugging
redfire-switch --debug --config config.json start

# Packet capture during debug
sudo tcpdump -i any -w debug.pcap port 5060 &
redfire-switch --debug --config config.json start
```

### Log Analysis

```bash
# View logs in real-time
journalctl -u redfire-switch -f

# Filter for errors
journalctl -u redfire-switch | grep ERROR

# Export logs for analysis
journalctl -u redfire-switch --since "1 hour ago" > redfire-debug.log
```

### Getting Help

1. **Check logs** first for error messages
2. **Use debug mode** for detailed troubleshooting
3. **Test with SIPp** to isolate issues
4. **Review configuration** for syntax errors
5. **Check GitHub issues** for known problems
6. **Join discussions** for community support

## 📚 Additional Resources

- **[Development Environment](DEV-ENVIRONMENT.md)** - Complete development setup
- **[Testing Guide](TESTING.md)** - How to test the installation
- **[Configuration Guide](CONFIGURATION.md)** - Detailed configuration options
- **[Architecture](ARCHITECTURE.md)** - System design and components
- **[Contributing](CONTRIBUTING.md)** - How to contribute to the project

## 🔄 Upgrade Process

### Development to Production

When moving from development to production:

1. **Build release version**: `cargo build --release`
2. **Update configuration**: Remove debug settings
3. **Set up monitoring**: Configure health checks
4. **Configure logging**: Set appropriate log levels
5. **Security hardening**: Restrict network access
6. **Performance tuning**: Optimize for your load

### Version Updates

To update to a new version:

```bash
# 1. Stop the service
sudo systemctl stop redfire-switch

# 2. Backup configuration
sudo cp /etc/redfire-switch/config.json /etc/redfire-switch/config.json.bak

# 3. Update code and rebuild
git pull
cargo build --release

# 4. Update binary
sudo cp target/release/redfire-switch /usr/local/bin/

# 5. Validate configuration (check for breaking changes)
redfire-switch validate-config --config /etc/redfire-switch/config.json

# 6. Start service
sudo systemctl start redfire-switch

# 7. Verify operation
sudo systemctl status redfire-switch
```

---

*For additional support, see the [troubleshooting section](#troubleshooting) or check the project's GitHub issues.*