# Anycast Clustering Setup Guide

This guide explains how to set up Redfire Switch in an anycast configuration where multiple switches share the same IP address and synchronize session state.

## Overview

In an anycast deployment:
- Multiple Redfire Switch instances bind to the same **anycast IP address**
- Network infrastructure (routers) handle BGP announcements
- Session state is synchronized across all nodes
- If one node fails, traffic automatically routes to healthy nodes
- No BGP daemon runs on the switches themselves

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Router/BGP    │    │   Router/BGP    │    │   Router/BGP    │
│   Speaker       │    │   Speaker       │    │   Speaker       │
│                 │    │                 │    │                 │
│ Announces:      │    │ Announces:      │    │ Announces:      │
│ 203.0.113.100   │    │ 203.0.113.100   │    │ 203.0.113.100   │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────────┐
                    │    Internet /       │
                    │   SIP Carriers      │
                    └─────────────────────┘
                                 │
         ┌───────────────────────┼───────────────────────┐
         │                       │                       │
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│ Redfire Switch  │    │ Redfire Switch  │    │ Redfire Switch  │
│    Node 1       │◄──►│    Node 2       │◄──►│    Node 3       │
│                 │    │                 │    │                 │
│ Local IP:       │    │ Local IP:       │    │ Local IP:       │
│ 10.0.1.10       │    │ 10.0.1.11       │    │ 10.0.1.12       │
│                 │    │                 │    │                 │
│ Anycast IP:     │    │ Anycast IP:     │    │ Anycast IP:     │
│ 203.0.113.100   │    │ 203.0.113.100   │    │ 203.0.113.100   │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────────┐
                    │   Redis Cluster     │
                    │ (Session Storage)   │
                    └─────────────────────┘
```

## Prerequisites

### Network Infrastructure
- BGP-capable routers that can announce the anycast prefix
- Network routing configured to direct traffic to healthy nodes
- Shared subnet or VLAN for cluster communication

### System Requirements
- Multiple Linux servers (minimum 2, recommended 3+)
- Redis cluster or PostgreSQL for session storage
- Network connectivity between all nodes

## Step 1: Network Configuration

### Configure Anycast IP on Each Node

Add the anycast IP to the loopback interface on each node:

```bash
# Add anycast IP to loopback (survives reboot if added to network config)
sudo ip addr add 203.0.113.100/32 dev lo

# Make permanent (Ubuntu/Debian)
echo "auto lo:0
iface lo:0 inet static
    address 203.0.113.100
    netmask 255.255.255.255" | sudo tee -a /etc/network/interfaces

# Make permanent (CentOS/RHEL)
sudo nmcli con mod lo +ipv4.addresses 203.0.113.100/32

# Verify
ip addr show lo
```

### Configure BGP Announcements

Configure your network routers to announce the anycast prefix. Example configurations:

#### BIRD Router Configuration
```bash
# /etc/bird/bird.conf
protocol static anycast_routes {
    route 203.0.113.100/32 via "10.0.1.10";  # Node 1
    route 203.0.113.100/32 via "10.0.1.11";  # Node 2
    route 203.0.113.100/32 via "10.0.1.12";  # Node 3
    export all;
}

protocol bgp upstream {
    local as 65001;
    neighbor 198.51.100.1 as 65000;
    export where proto = "anycast_routes";
}
```

#### FRRouting Configuration
```bash
# /etc/frr/bgpd.conf
router bgp 65001
 neighbor 198.51.100.1 remote-as 65000
 !
 address-family ipv4 unicast
  network 203.0.113.100/32
  neighbor 198.51.100.1 activate
 exit-address-family
```

#### Cisco Router Configuration
```
router bgp 65001
 network 203.0.113.100 mask 255.255.255.255
 neighbor 198.51.100.1 remote-as 65000
```

## Step 2: Time Synchronization Setup

**CRITICAL**: Proper time synchronization is essential for accurate call timing and CDRs.

### Install and Configure NTP

Time drift between cluster nodes can cause:
- ❌ Inaccurate call start/end times in CDRs
- ❌ Billing discrepancies 
- ❌ Session state synchronization issues
- ❌ Incorrect call duration calculations

#### Ubuntu/Debian NTP Setup

```bash
# Install NTP daemon
sudo apt update
sudo apt install ntp

# Configure NTP servers (edit /etc/ntp.conf)
sudo nano /etc/ntp.conf
```

Add these lines to `/etc/ntp.conf`:
```
# Use reliable time servers
server pool.ntp.org
server time.google.com
server time.cloudflare.com

# Drift file
driftfile /var/lib/ntp/ntp.drift

# Enable stats
statsdir /var/log/ntpstats/

statistics loopstats peerstats clockstats
filegen loopstats file loopstats type day enable
filegen peerstats file peerstats type day enable
filegen clockstats file clockstats type day enable
```

#### CentOS/RHEL Chrony Setup

```bash
# Install chrony
sudo yum install chrony

# Configure chrony (edit /etc/chrony.conf)
sudo nano /etc/chrony.conf
```

Add these lines to `/etc/chrony.conf`:
```
# Time servers
server pool.ntp.org iburst
server time.google.com iburst
server time.cloudflare.com iburst

# Drift file
driftfile /var/lib/chrony/drift

# Make steps larger than 1 second
makestep 1.0 3

# Enable RTC sync
rtcsync
```

#### Start and Enable NTP Services

```bash
# For NTP (Ubuntu/Debian)
sudo systemctl enable ntp
sudo systemctl start ntp

# For Chrony (CentOS/RHEL)
sudo systemctl enable chronyd
sudo systemctl start chronyd

# Verify synchronization
sudo ntpq -p  # For NTP
# or
sudo chronyc tracking  # For Chrony
```

### Configure Time Drift Monitoring

Time drift monitoring is automatically enabled in the anycast configuration:

```toml
# anycast.toml
[time_sync]
enabled = true                           # Enable time drift monitoring
check_interval_ms = 60000               # Check every 60 seconds  
warning_threshold_ms = 15               # Warn at >15ms drift
critical_threshold_ms = 1000            # Critical at >1000ms drift
warn_on_drift = true                    # Alert administrators
log_drift_details = false              # Detailed logging
```

### Monitor Time Synchronization

```bash
# Check time sync status across cluster
redfire-switch anycast time-sync status

# Check NTP daemon status
redfire-switch anycast time-sync ntp-status

# Check time drift between nodes
redfire-switch anycast time-sync drift-check
```

### Time Drift Thresholds

- **✅ Good**: < 15ms drift - No impact on call timing
- **⚠️ Warning**: 15ms - 999ms drift - May affect call timing precision
- **❌ Critical**: ≥ 1000ms drift - Causes CDR inaccuracies and billing errors

### Troubleshooting Time Sync

```bash
# Check if NTP is running
systemctl status ntp
systemctl status chronyd

# Force time sync
sudo ntpdate -s pool.ntp.org  # For NTP
sudo chronyc makestep         # For Chrony

# Check time sources
ntpq -p                       # For NTP
chronyc sources -v            # For Chrony

# View system time
timedatectl status

# Set timezone if needed
sudo timedatectl set-timezone UTC
```

## Step 3: Session Storage Setup

### Option A: Redis Cluster (Recommended)

Set up Redis cluster for session synchronization:

```bash
# Install Redis on dedicated servers
sudo apt install redis-server

# Configure Redis cluster (on each Redis node)
# /etc/redis/redis.conf
port 6379
cluster-enabled yes
cluster-config-file nodes.conf
cluster-node-timeout 5000
appendonly yes
```

Create Redis cluster:
```bash
redis-cli --cluster create \
  10.0.1.20:6379 10.0.1.21:6379 10.0.1.22:6379 \
  --cluster-replicas 0
```

### Option B: PostgreSQL with Replication

Set up PostgreSQL with streaming replication:

```bash
# Install PostgreSQL
sudo apt install postgresql postgresql-contrib

# Configure primary server
# /etc/postgresql/14/main/postgresql.conf
wal_level = replica
max_wal_senders = 3
checkpoint_segments = 8
archive_mode = on
```

## Step 4: Redfire Switch Configuration

### Install Redfire Switch on Each Node

```bash
# Install on each node
curl -fsSL https://install.redfire-switch.com | sudo bash
```

### Configure Main SIP Settings

Edit `/etc/redfire-switch/config.toml` on each node:

```toml
[sip]
# Bind to the anycast IP
bind_address = "203.0.113.100:5060"
external_ip = "203.0.113.100"
domain = "sip.example.com"

[database]
url = "postgresql://redfire:password@10.0.1.30/redfire_switch"

# Enable anycast clustering
[features]
anycast_clustering = true
```

### Configure Anycast Clustering

Create `/etc/redfire-switch/anycast.toml` on each node:

```toml
enabled = true

[node]
# Unique per node
node_id = "switch-1"                    # switch-2, switch-3, etc.
name = "Primary SIP Switch"
local_ip = "10.0.1.10"                  # Unique per node
anycast_ip = "203.0.113.100"            # Same on all nodes
priority = 100                          # Adjust per node
capacity = 10000
region = "us-east-1"
zone = "us-east-1a"

[session_store]
store_type = "Redis"

[session_store.connection]
urls = [
    "redis://10.0.1.20:6379/0",
    "redis://10.0.1.21:6379/0", 
    "redis://10.0.1.22:6379/0"
]
password = "redis_password"

[cluster]
protocol = "Gossip"

[cluster.gossip]
bind_addr = "0.0.0.0:7946"
seeds = [
    "10.0.1.11:7946",                   # Other nodes (not self)
    "10.0.1.12:7946"
]

[health]
enabled = true
check_interval_ms = 30000

[session_sync]
enabled = true
sync_interval_ms = 5000
sync_types = ["ActiveCalls", "Registrations"]
```

## Step 5: Start Services

### Enable and Start Services on Each Node

```bash
# Start main service
sudo systemctl enable --now redfire-switch

# Start anycast clustering
sudo systemctl enable --now redfire-switch-anycast

# Check status
sudo systemctl status redfire-switch
sudo systemctl status redfire-switch-anycast
```

### Verify Cluster Formation

```bash
# Check cluster members
redfire-switch anycast cluster members

# Check session synchronization
redfire-switch anycast sessions list

# Check health status
redfire-switch anycast health
```

## Step 6: Health-Based Route Management

### Automatic Route Withdrawal

Create a health check script that removes the anycast route when the node is unhealthy:

```bash
# /usr/local/bin/anycast-health-manager
#!/bin/bash

ANYCAST_IP="203.0.113.100/32"
HEALTH_CHECK_URL="http://localhost:8081/health"

check_health() {
    # Check Redfire Switch health
    if ! redfire-switch anycast health >/dev/null 2>&1; then
        return 1
    fi
    
    # Check HTTP health endpoint
    if ! curl -f -s "$HEALTH_CHECK_URL" >/dev/null; then
        return 1
    fi
    
    return 0
}

manage_route() {
    if check_health; then
        # Add route if healthy and not present
        if ! ip route show "$ANYCAST_IP" | grep -q "dev lo"; then
            ip route add "$ANYCAST_IP" dev lo 2>/dev/null
            logger "Anycast route added - node healthy"
        fi
    else
        # Remove route if unhealthy
        if ip route show "$ANYCAST_IP" | grep -q "dev lo"; then
            ip route del "$ANYCAST_IP" dev lo 2>/dev/null
            logger "Anycast route removed - node unhealthy"
        fi
    fi
}

manage_route
```

Make it executable and run periodically:

```bash
sudo chmod +x /usr/local/bin/anycast-health-manager

# Add to cron (every 30 seconds)
echo "* * * * * root /usr/local/bin/anycast-health-manager" | sudo tee -a /etc/crontab
echo "* * * * * root sleep 30; /usr/local/bin/anycast-health-manager" | sudo tee -a /etc/crontab
```

### Alternative: systemd Timer

```bash
# /etc/systemd/system/anycast-health.service
[Unit]
Description=Anycast Health Check
After=redfire-switch.service

[Service]
Type=oneshot
ExecStart=/usr/local/bin/anycast-health-manager

# /etc/systemd/system/anycast-health.timer
[Unit]
Description=Run anycast health check every 30 seconds
Requires=anycast-health.service

[Timer]
OnBootSec=30sec
OnUnitActiveSec=30sec

[Install]
WantedBy=timers.target
```

Enable the timer:
```bash
sudo systemctl enable --now anycast-health.timer
```

## Step 7: Firewall Configuration

### Open Required Ports

```bash
# SIP traffic (anycast IP)
sudo ufw allow in on lo to 203.0.113.100 port 5060
sudo ufw allow in on lo to 203.0.113.100 port 5061

# Cluster communication
sudo ufw allow from 10.0.1.0/24 to any port 7946 proto udp

# Redis access (if on separate servers)
sudo ufw allow from 10.0.1.10,10.0.1.11,10.0.1.12 to any port 6379

# Management API
sudo ufw allow from 10.0.1.0/24 to any port 8081
```

## Step 8: Testing and Validation

### Test Anycast Functionality

1. **Test SIP connectivity to anycast IP:**
```bash
# From external client
sipsak -vvv -s sip:test@203.0.113.100
```

2. **Test failover:**
```bash
# Stop one node
sudo systemctl stop redfire-switch-anycast

# Verify traffic still works
# Check cluster status from other nodes
redfire-switch anycast cluster members
```

3. **Test session synchronization:**
```bash
# Make a call through one node
# Check session exists on other nodes
redfire-switch anycast sessions list
```

4. **Test time synchronization:**
```bash
# Check time drift across cluster
redfire-switch anycast time-sync drift-check

# Verify NTP status on each node
redfire-switch anycast time-sync ntp-status

# Monitor for time drift warnings
tail -f /var/log/redfire-switch/redfire-switch.log | grep -i "time drift"
```

### Monitor Cluster Health

```bash
# Check cluster status
redfire-switch anycast cluster status

# Monitor logs
sudo journalctl -u redfire-switch-anycast -f

# Check session store connectivity
redfire-switch anycast sessions health
```

## Troubleshooting

### Common Issues

1. **Anycast IP not reachable:**
   - Verify IP is added to loopback: `ip addr show lo`
   - Check BGP announcement at routers
   - Verify routing tables: `ip route show`

2. **Cluster nodes not discovering each other:**
   - Check firewall rules for port 7946/udp
   - Verify seed node configuration
   - Check network connectivity between nodes

3. **Session synchronization not working:**
   - Verify Redis cluster status
   - Check authentication credentials
   - Monitor session store logs

4. **Health checks failing:**
   - Check health check configuration
   - Verify dependencies (Redis, database)
   - Review health check logs

### Debug Commands

```bash
# Check anycast configuration
redfire-switch anycast check-config

# Debug cluster communication
redfire-switch anycast cluster debug

# Test session store connectivity
redfire-switch anycast sessions test

# Check route announcements
ip route show table all | grep 203.0.113.100
```

## Best Practices

### High Availability
- Deploy at least 3 nodes for redundancy
- Use separate Redis cluster for session storage
- Monitor cluster health continuously
- Implement automated failover

### Security
- Use TLS for Redis connections
- Restrict cluster communication to private networks
- Enable authentication for all services
- Regular security updates

### Performance
- Monitor session synchronization latency
- Tune Redis cluster for your workload
- Use SSD storage for session data
- Monitor network bandwidth usage

### Monitoring
- Set up Prometheus metrics collection
- Monitor cluster membership changes
- Alert on session sync failures
- Track anycast route changes

## Production Deployment Checklist

- [ ] Network infrastructure configured for BGP anycast
- [ ] All nodes have anycast IP configured
- [ ] Redis cluster deployed and tested
- [ ] Anycast configuration validated on all nodes
- [ ] Health-based route management implemented
- [ ] Firewall rules configured
- [ ] Monitoring and alerting set up
- [ ] Failover testing completed
- [ ] Documentation updated with environment-specific details