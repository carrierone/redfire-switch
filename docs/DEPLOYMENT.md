# Deployment and Operations Guide

This document provides comprehensive guidance for deploying and operating Redfire Switch in production environments.

## Pre-Deployment Planning

### Requirements Analysis

Before deploying Redfire Switch, assess your requirements:

#### Traffic Requirements
- **Concurrent calls**: Expected simultaneous calls
- **Call volume**: Calls per hour/day
- **Peak traffic**: Maximum expected load
- **Growth projections**: Future capacity needs

#### Network Requirements
- **SIP endpoints**: Number of devices/carriers to connect
- **Network topology**: Internal/external network design
- **Bandwidth**: Required for SIP signaling and media
- **Latency**: Maximum acceptable response times

#### Availability Requirements
- **Uptime targets**: 99.9%, 99.99%, etc.
- **Recovery time**: Acceptable downtime for maintenance
- **Redundancy**: High availability requirements
- **Disaster recovery**: Backup site requirements

### System Requirements

#### Minimum Requirements
- **CPU**: 1 core, 1 GHz
- **RAM**: 512 MB
- **Storage**: 1 GB
- **OS**: Linux, macOS, Windows
- **Network**: 100 Mbps

#### Recommended Production Requirements
- **CPU**: 4+ cores, 2+ GHz
- **RAM**: 4+ GB
- **Storage**: 20+ GB SSD
- **OS**: Linux (Ubuntu 20.04+, CentOS 8+, RHEL 8+)
- **Network**: 1+ Gbps with redundancy

#### High Availability Requirements
- **CPU**: 8+ cores per instance
- **RAM**: 8+ GB per instance
- **Storage**: 50+ GB SSD with RAID
- **Network**: Redundant 1+ Gbps connections
- **Load balancer**: For multiple instances

## Installation Methods

### From Source

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone and build
git clone https://github.com/redfireswitch/redfire-switch.git
cd redfire-switch
cargo build --release

# Install binary
sudo cp target/release/redfire-switch /usr/local/bin/
sudo chmod +x /usr/local/bin/redfire-switch
```

### Binary Release

```bash
# Download release (replace VERSION with actual version)
wget https://github.com/redfireswitch/redfire-switch/releases/download/v0.1.0/redfire-switch-linux-x86_64.tar.gz

# Extract and install
tar -xzf redfire-switch-linux-x86_64.tar.gz
sudo cp redfire-switch /usr/local/bin/
sudo chmod +x /usr/local/bin/redfire-switch
```

### Package Installation (Future)

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install redfire-switch

# CentOS/RHEL
sudo yum install redfire-switch

# Docker
docker pull redfireswitch/redfire-switch:latest
```

## Configuration for Production

### Directory Structure

Create a standard directory structure:

```bash
# Configuration directory
sudo mkdir -p /etc/redfire

# Log directory
sudo mkdir -p /var/log/redfire

# Data directory (future use)
sudo mkdir -p /var/lib/redfire

# Set permissions
sudo chown -R redfire:redfire /etc/redfire /var/log/redfire /var/lib/redfire
```

### User Account

Create a dedicated user account:

```bash
# Create user and group
sudo groupadd redfire
sudo useradd -r -g redfire -s /bin/false redfire

# Create home directory
sudo mkdir -p /home/redfire
sudo chown redfire:redfire /home/redfire
```

### Production Configuration

Generate and customize configuration:

```bash
# Generate initial config
sudo -u redfire redfire-switch gen-config --output /etc/redfire/config.json

# Edit for production
sudo -u redfire vim /etc/redfire/config.json
```

#### Production Configuration Example

```json
{
  "sip_profiles": [
    {
      "name": "internal",
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
        "name": "carrier-1-backup",
        "address": "10.1.1.101:5060",
        "protocol": "Udp", 
        "enabled": true,
        "ping_interval_seconds": 30,
        "timeout_seconds": 5
      },
      {
        "name": "carrier-2-primary",
        "address": "10.2.1.100:5060",
        "protocol": "Udp",
        "enabled": true,
        "ping_interval_seconds": 30,
        "timeout_seconds": 8
      }
    ]
  }
}
```

### Configuration Validation

```bash
# Validate configuration
sudo -u redfire redfire-switch --config /etc/redfire/config.json validate-config

# Test monitoring endpoints
sudo -u redfire redfire-switch --config /etc/redfire/config.json monitor status
```

## Service Management

### Systemd Service

Create a systemd service file:

```bash
sudo vim /etc/systemd/system/redfire-switch.service
```

```ini
[Unit]
Description=Redfire Switch SIP Server
After=network.target
Wants=network.target

[Service]
Type=simple
User=redfire
Group=redfire
ExecStart=/usr/local/bin/redfire-switch --config /etc/redfire/config.json start
ExecReload=/bin/kill -HUP $MAINPID
Restart=always
RestartSec=5
StandardOutput=journal
StandardError=journal
SyslogIdentifier=redfire-switch

# Security settings
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/var/log/redfire /var/lib/redfire
PrivateTmp=yes

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

[Install]
WantedBy=multi-user.target
```

### Service Management Commands

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service
sudo systemctl enable redfire-switch

# Start service
sudo systemctl start redfire-switch

# Check status
sudo systemctl status redfire-switch

# View logs
sudo journalctl -u redfire-switch -f

# Stop service
sudo systemctl stop redfire-switch

# Restart service
sudo systemctl restart redfire-switch
```

### Init.d Script (Legacy Systems)

For systems without systemd:

```bash
sudo vim /etc/init.d/redfire-switch
```

```bash
#!/bin/bash
# redfire-switch        Redfire Switch SIP Server
# chkconfig: 35 80 20
# description: Redfire Switch SIP Server

. /etc/rc.d/init.d/functions

USER="redfire"
DAEMON="redfire-switch"
ROOT_DIR="/home/redfire"
SERVER="$ROOT_DIR/$DAEMON"
LOCK_FILE="/var/lock/subsys/redfire-switch"
CONFIG_FILE="/etc/redfire/config.json"

start() {
    if [ -f $LOCK_FILE ]; then
        echo "$DAEMON is locked."
        return 1
    fi
    
    echo -n $"Shutting down $DAEMON: "
    pid=`ps -aefw | grep "$DAEMON" | grep -v " grep " | awk '{print $2}'`
    kill -9 $pid > /dev/null 2>&1
    [ $? -eq 0 ] && echo "OK" || echo "FAILED"

    echo -n $"Starting $DAEMON: "
    daemon --user "$USER" --pidfile="$LOCK_FILE" \
           "$SERVER" --config "$CONFIG_FILE" start
    RETVAL=$?
    echo
    [ $RETVAL -eq 0 ] && touch $LOCK_FILE
    return $RETVAL
}

stop() {
    echo -n $"Shutting down $DAEMON: "
    pid=`ps -aefw | grep "$DAEMON" | grep -v " grep " | awk '{print $2}'`
    kill -15 $pid > /dev/null 2>&1
    sleep 5
    pid=`ps -aefw | grep "$DAEMON" | grep -v " grep " | awk '{print $2}'`
    kill -9 $pid > /dev/null 2>&1
    [ $? -eq 0 ] && echo "OK" || echo "FAILED"
    rm -f $LOCK_FILE
}

status() {
    if [ -f $LOCK_FILE ]; then
        echo "$DAEMON is running."
    else
        echo "$DAEMON is stopped."
    fi
}

case "$1" in
    start)
        start
        ;;
    stop)
        stop
        ;;
    status)
        status
        ;;
    restart)
        stop
        start
        ;;
    *)
        echo "Usage: {start|stop|status|restart}"
        exit 1
        ;;
esac

exit $?
```

```bash
# Make executable and enable
sudo chmod +x /etc/init.d/redfire-switch
sudo chkconfig --add redfire-switch
sudo chkconfig redfire-switch on
```

## Docker Deployment

### Docker Image

Create a Dockerfile:

```dockerfile
FROM rust:1.70 as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Create user
RUN groupadd -r redfire && \
    useradd -r -g redfire redfire

# Copy binary
COPY --from=builder /app/target/release/redfire-switch /usr/local/bin/
RUN chmod +x /usr/local/bin/redfire-switch

# Create directories
RUN mkdir -p /etc/redfire /var/log/redfire && \
    chown -R redfire:redfire /etc/redfire /var/log/redfire

# Switch to non-root user
USER redfire

# Expose ports
EXPOSE 5060/udp 5060/tcp

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD redfire-switch monitor ping localhost:5060 || exit 1

# Default command
CMD ["redfire-switch", "--config", "/etc/redfire/config.json", "start"]
```

### Docker Compose

```yaml
version: '3.8'

services:
  redfire-switch:
    build: .
    ports:
      - "5060:5060/udp"
      - "5060:5060/tcp"
    volumes:
      - ./config:/etc/redfire
      - redfire-logs:/var/log/redfire
    environment:
      - RUST_LOG=info
    restart: unless-stopped
    networks:
      - sip-network
    healthcheck:
      test: ["CMD", "redfire-switch", "monitor", "ping", "localhost:5060"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 60s

volumes:
  redfire-logs:

networks:
  sip-network:
    driver: bridge
```

### Running with Docker

```bash
# Build image
docker build -t redfire-switch .

# Run container
docker run -d \
  --name redfire-switch \
  -p 5060:5060/udp \
  -p 5060:5060/tcp \
  -v $(pwd)/config.json:/etc/redfire/config.json \
  redfire-switch

# View logs
docker logs -f redfire-switch

# Execute commands
docker exec redfire-switch redfire-switch monitor status
```

## Monitoring and Logging

### Log Configuration

Configure structured logging:

```bash
# Set log level
export RUST_LOG=info

# JSON structured logs
export RUST_LOG=info
redfire-switch start 2>&1 | jq .

# Module-specific logging
export RUST_LOG=redfire_switch::monitor=debug,redfire_switch::sip_server=info
```

### Log Rotation

Configure log rotation with logrotate:

```bash
sudo vim /etc/logrotate.d/redfire-switch
```

```
/var/log/redfire/*.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    create 644 redfire redfire
    postrotate
        systemctl reload redfire-switch
    endscript
}
```

### System Monitoring

#### Health Checks

```bash
# Service health
systemctl is-active redfire-switch

# Endpoint monitoring
redfire-switch --config /etc/redfire/config.json monitor status

# Process monitoring
ps aux | grep redfire-switch

# Network monitoring
netstat -tulnp | grep :5060
ss -tulnp | grep :5060
```

#### Performance Monitoring

```bash
# CPU and memory usage
top -p $(pgrep redfire-switch)
htop -p $(pgrep redfire-switch)

# Network statistics
iftop -f "port 5060"
tcpdump -i any port 5060

# System resources
iostat 1
vmstat 1
```

### Alerting Integration

#### SNMP Monitoring (Future)

```bash
# Install SNMP agent
sudo apt install snmp snmp-mibs-downloader

# Configure SNMP for Redfire Switch
# (Future feature)
```

#### Nagios/Icinga Checks

```bash
#!/bin/bash
# Nagios check script
CONFIG_FILE="/etc/redfire/config.json"

# Check service status
if ! systemctl is-active --quiet redfire-switch; then
    echo "CRITICAL - Redfire Switch service is not running"
    exit 2
fi

# Check endpoint health
if ! redfire-switch --config "$CONFIG_FILE" monitor ping 10.1.1.100:5060 > /dev/null 2>&1; then
    echo "WARNING - Primary carrier endpoint unreachable"
    exit 1
fi

echo "OK - Redfire Switch is running and endpoints are healthy"
exit 0
```

## Security Considerations

### Network Security

#### Firewall Configuration

```bash
# UFW (Ubuntu)
sudo ufw allow 5060/udp
sudo ufw allow 5060/tcp
sudo ufw enable

# iptables
sudo iptables -A INPUT -p udp --dport 5060 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 5060 -j ACCEPT

# Restrict to specific sources
sudo iptables -A INPUT -p udp -s 10.1.1.0/24 --dport 5060 -j ACCEPT
sudo iptables -A INPUT -p udp --dport 5060 -j DROP
```

#### Network Segmentation

- **SIP DMZ**: Isolate SIP traffic in dedicated network segment
- **Management Network**: Separate network for administration
- **Monitoring Network**: Dedicated network for health checks

### Access Control

#### IP-based Authentication

Configure allowed IPs in SIP profiles:

```json
{
  "allowed_ips": [
    "10.1.1.100",
    "10.1.1.101",
    "192.168.100.0"
  ]
}
```

#### File Permissions

```bash
# Secure configuration files
sudo chmod 640 /etc/redfire/config.json
sudo chown root:redfire /etc/redfire/config.json

# Secure binary
sudo chmod 755 /usr/local/bin/redfire-switch
sudo chown root:root /usr/local/bin/redfire-switch
```

### Hardening

#### System Hardening

```bash
# Disable unnecessary services
sudo systemctl disable bluetooth
sudo systemctl disable cups

# Configure fail2ban
sudo apt install fail2ban
# Configure for SIP brute force protection

# Regular security updates
sudo apt update && sudo apt upgrade
```

#### Application Hardening

- **Process isolation**: Run as non-root user
- **Resource limits**: Configure systemd limits
- **Capability dropping**: Minimal required privileges
- **Secure defaults**: Conservative configuration defaults

## Backup and Recovery

### Configuration Backup

```bash
#!/bin/bash
# Backup script
BACKUP_DIR="/opt/backups/redfire"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p "$BACKUP_DIR"

# Backup configuration
cp /etc/redfire/config.json "$BACKUP_DIR/config_$DATE.json"

# Backup logs (optional)
tar -czf "$BACKUP_DIR/logs_$DATE.tar.gz" /var/log/redfire/

# Cleanup old backups (keep 30 days)
find "$BACKUP_DIR" -name "*.json" -mtime +30 -delete
find "$BACKUP_DIR" -name "*.tar.gz" -mtime +30 -delete
```

### Disaster Recovery

#### Recovery Procedures

1. **Service Recovery**:
   ```bash
   # Restore configuration
   sudo cp /opt/backups/redfire/config_YYYYMMDD_HHMMSS.json /etc/redfire/config.json
   
   # Validate configuration
   sudo -u redfire redfire-switch --config /etc/redfire/config.json validate-config
   
   # Restart service
   sudo systemctl restart redfire-switch
   ```

2. **Full System Recovery**:
   - Restore system from backup
   - Reinstall Redfire Switch
   - Restore configuration files
   - Verify network connectivity
   - Test all SIP profiles

#### High Availability Setup

```bash
# Load balancer configuration (HAProxy example)
backend redfire_switches
    balance roundrobin
    server switch1 10.1.1.10:5060 check
    server switch2 10.1.1.11:5060 check backup
    
# Keepalived for VIP management
vrrp_instance VI_1 {
    state MASTER
    interface eth0
    virtual_router_id 51
    priority 100
    virtual_ipaddress {
        10.1.1.100
    }
}
```

## Performance Tuning

### System Tuning

#### Network Tuning

```bash
# Increase UDP buffer sizes
echo 'net.core.rmem_max = 134217728' >> /etc/sysctl.conf
echo 'net.core.wmem_max = 134217728' >> /etc/sysctl.conf
echo 'net.core.netdev_max_backlog = 5000' >> /etc/sysctl.conf

# Apply changes
sudo sysctl -p
```

#### File Descriptor Limits

```bash
# Increase file descriptor limits
echo 'redfire soft nofile 65536' >> /etc/security/limits.conf
echo 'redfire hard nofile 65536' >> /etc/security/limits.conf

# For systemd services, use LimitNOFILE in service file
```

### Application Tuning

#### Monitoring Intervals

Optimize based on requirements:

```json
{
  "monitoring": {
    "endpoints": [
      {
        "ping_interval_seconds": 15,
        "timeout_seconds": 3
      }
    ]
  }
}
```

#### Resource Allocation

- **CPU Cores**: Allocate appropriate cores for expected load
- **Memory**: Monitor memory usage and adjust as needed
- **Network**: Ensure adequate bandwidth for SIP traffic

## Maintenance Procedures

### Regular Maintenance

#### Daily Tasks
- Check service status
- Review monitoring alerts
- Verify endpoint health

#### Weekly Tasks
- Review logs for errors
- Check system resources
- Validate configuration

#### Monthly Tasks
- Update system packages
- Rotate and archive logs
- Performance review

### Update Procedures

#### Application Updates

```bash
# Stop service
sudo systemctl stop redfire-switch

# Backup current binary
sudo cp /usr/local/bin/redfire-switch /usr/local/bin/redfire-switch.backup

# Install new version
sudo cp redfire-switch /usr/local/bin/
sudo chmod +x /usr/local/bin/redfire-switch

# Validate configuration with new version
sudo -u redfire redfire-switch --config /etc/redfire/config.json validate-config

# Start service
sudo systemctl start redfire-switch

# Verify operation
sudo systemctl status redfire-switch
```

#### Configuration Updates

```bash
# Backup current config
sudo cp /etc/redfire/config.json /etc/redfire/config.json.backup

# Update configuration
sudo vim /etc/redfire/config.json

# Validate new configuration
sudo -u redfire redfire-switch --config /etc/redfire/config.json validate-config

# Reload service (when hot-reload is available)
sudo systemctl reload redfire-switch
```

## Troubleshooting

### Common Issues

#### Service Won't Start
1. Check configuration validity
2. Verify file permissions
3. Check port availability
4. Review system logs

#### High CPU Usage
1. Check monitoring frequency
2. Review endpoint count
3. Monitor network latency
4. Check for connection issues

#### Memory Leaks
1. Monitor memory usage over time
2. Review log levels
3. Check for configuration issues
4. Restart service if necessary

### Diagnostic Commands

```bash
# Service diagnostics
sudo systemctl status redfire-switch
sudo journalctl -u redfire-switch --since "1 hour ago"

# Network diagnostics
sudo netstat -tulnp | grep :5060
sudo tcpdump -i any port 5060

# Application diagnostics
redfire-switch --config /etc/redfire/config.json validate-config
redfire-switch --config /etc/redfire/config.json monitor status
```