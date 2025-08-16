# Troubleshooting Guide

This document provides comprehensive troubleshooting information for Redfire Switch. Use this guide to diagnose and resolve common issues.

## Quick Diagnosis

### System Health Check

Run these commands to quickly assess system health:

```bash
# Check service status
systemctl status redfire-switch

# Validate configuration
redfire-switch validate-config

# Check monitoring status
redfire-switch monitor status

# Test connectivity
redfire-switch monitor ping 127.0.0.1:5060
```

### Log Analysis

```bash
# View recent logs
journalctl -u redfire-switch --since "1 hour ago"

# Follow logs in real-time
journalctl -u redfire-switch -f

# Search for errors
journalctl -u redfire-switch | grep -i error

# Check startup logs
journalctl -u redfire-switch --since "$(systemctl show redfire-switch --property=ActiveEnterTimestamp | cut -d= -f2)"
```

## Service Issues

### Service Won't Start

#### Symptoms
- `systemctl start redfire-switch` fails
- Service shows "failed" status
- No response on SIP ports

#### Diagnostic Steps

1. **Check service status**:
   ```bash
   systemctl status redfire-switch
   ```

2. **View detailed logs**:
   ```bash
   journalctl -u redfire-switch --since "5 minutes ago"
   ```

3. **Check configuration**:
   ```bash
   redfire-switch --config /etc/redfire/config.json validate-config
   ```

4. **Test manual startup**:
   ```bash
   sudo -u redfire redfire-switch --config /etc/redfire/config.json --verbose start
   ```

#### Common Causes and Solutions

##### Configuration Errors
```bash
# Error: Invalid JSON syntax
Error: expected `,` or `}` at line 15

# Solution: Fix JSON syntax
redfire-switch validate-config
```

##### Permission Issues
```bash
# Error: Permission denied
Error: Permission denied (os error 13)

# Solution: Fix file permissions
sudo chown -R redfire:redfire /etc/redfire
sudo chmod 640 /etc/redfire/config.json
```

##### Port Already in Use
```bash
# Error: Address already in use
Error: Address already in use (os error 98)

# Solution: Check what's using the port
sudo netstat -tulnp | grep :5060
sudo lsof -i :5060

# Kill conflicting process or change port
```

##### Network Interface Issues
```bash
# Error: Cannot assign requested address
Error: Cannot assign requested address (os error 99)

# Solution: Check interface configuration
ip addr show
# Verify bind_ip in configuration matches available interfaces
```

### Service Crashes

#### Symptoms
- Service starts but stops unexpectedly
- Random failures under load
- Memory or resource exhaustion

#### Diagnostic Steps

1. **Check crash logs**:
   ```bash
   journalctl -u redfire-switch | grep -A 10 -B 10 "panic\|segfault\|killed"
   ```

2. **Monitor resource usage**:
   ```bash
   # Memory usage
   ps aux | grep redfire-switch
   
   # System resources
   top -p $(pgrep redfire-switch)
   ```

3. **Check system limits**:
   ```bash
   # File descriptor limits
   cat /proc/$(pgrep redfire-switch)/limits
   
   # System memory
   free -h
   ```

#### Solutions

##### Memory Issues
```bash
# Increase memory limits in systemd service
[Service]
MemoryLimit=1G

# Monitor memory usage over time
while true; do 
    ps -o pid,vsz,rss,comm -p $(pgrep redfire-switch)
    sleep 60
done
```

##### File Descriptor Limits
```bash
# Increase limits in service file
[Service]
LimitNOFILE=65536

# Check current usage
lsof -p $(pgrep redfire-switch) | wc -l
```

### Service Unresponsive

#### Symptoms
- Service appears running but doesn't respond
- SIP messages not processed
- Monitoring fails

#### Diagnostic Steps

1. **Check process state**:
   ```bash
   ps aux | grep redfire-switch
   ```

2. **Test network connectivity**:
   ```bash
   netstat -tulnp | grep :5060
   telnet localhost 5060
   ```

3. **Send test SIP message**:
   ```bash
   redfire-switch monitor ping localhost:5060
   ```

4. **Check system resources**:
   ```bash
   top -p $(pgrep redfire-switch)
   iostat 1
   ```

#### Solutions

##### Process Deadlock
```bash
# Send SIGTERM to restart gracefully
sudo systemctl restart redfire-switch

# Force kill if unresponsive
sudo kill -9 $(pgrep redfire-switch)
sudo systemctl start redfire-switch
```

##### Resource Exhaustion
```bash
# Check CPU usage
top -p $(pgrep redfire-switch)

# Check I/O wait
iostat -x 1

# Reduce monitoring frequency if needed
```

## Configuration Issues

### Invalid Configuration

#### Symptoms
- Configuration validation fails
- Service starts but behaves unexpectedly
- Missing or incorrect settings

#### Diagnostic Steps

1. **Validate configuration**:
   ```bash
   redfire-switch validate-config
   ```

2. **Check configuration syntax**:
   ```bash
   jq . /etc/redfire/config.json
   ```

3. **Compare with default**:
   ```bash
   redfire-switch gen-config --output /tmp/default.json
   diff /etc/redfire/config.json /tmp/default.json
   ```

#### Common Configuration Errors

##### JSON Syntax Errors
```bash
# Error: Invalid JSON
Error: expected `,` at line 25

# Solution: Use JSON validator
jq . /etc/redfire/config.json

# Common issues:
# - Missing commas
# - Trailing commas
# - Unmatched braces/brackets
# - Unquoted strings
```

##### Invalid IP Addresses
```bash
# Error: Invalid IP address format
Error: invalid IP address: "192.168.1.300"

# Solution: Use valid IP addresses
# IPv4: 192.168.1.1
# IPv6: ::1 (future support)
```

##### Invalid Port Numbers
```bash
# Error: Port out of range
Error: port must be between 1 and 65535

# Solution: Use valid port numbers
# Avoid ports < 1024 without root privileges
# Common SIP ports: 5060, 5061
```

##### Missing Required Fields
```bash
# Error: Missing required field
Error: missing field `name` at line 10

# Solution: Ensure all required fields are present
# Required for SIP profiles: name, bind_ip, allowed_ips
# Required for endpoints: name, address
```

### Network Configuration Issues

#### Symptoms
- Cannot bind to specified addresses
- Traffic not reaching switch
- Monitoring fails to connect

#### Diagnostic Steps

1. **Check network interfaces**:
   ```bash
   ip addr show
   ip route show
   ```

2. **Test connectivity**:
   ```bash
   ping <target_ip>
   telnet <target_ip> 5060
   ```

3. **Check firewall**:
   ```bash
   sudo iptables -L
   sudo ufw status
   ```

#### Solutions

##### Interface Not Available
```bash
# Error: Cannot assign requested address
# Solution: Check available interfaces
ip addr show

# Update configuration to use available IP
{
  "bind_ip": "0.0.0.0",  // Listen on all interfaces
  "bind_ip": "192.168.1.10"  // Specific interface
}
```

##### Firewall Blocking
```bash
# Allow SIP traffic
sudo ufw allow 5060/udp
sudo ufw allow 5060/tcp

# Or with iptables
sudo iptables -A INPUT -p udp --dport 5060 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 5060 -j ACCEPT
```

## Network Issues

### Connectivity Problems

#### Symptoms
- Cannot reach SIP endpoints
- Monitoring shows endpoints offline
- Timeouts on SIP messages

#### Diagnostic Steps

1. **Basic connectivity**:
   ```bash
   ping <endpoint_ip>
   traceroute <endpoint_ip>
   ```

2. **Port connectivity**:
   ```bash
   telnet <endpoint_ip> 5060
   nc -u <endpoint_ip> 5060
   ```

3. **SIP-specific testing**:
   ```bash
   redfire-switch monitor ping <endpoint_ip>:5060
   ```

4. **Network path analysis**:
   ```bash
   mtr <endpoint_ip>
   tcptraceroute <endpoint_ip> 5060
   ```

#### Solutions

##### Network Path Issues
```bash
# Check routing
ip route get <endpoint_ip>

# Check for packet loss
ping -c 10 <endpoint_ip>

# Adjust timeouts for slow networks
{
  "timeout_seconds": 15  // Increase timeout
}
```

##### DNS Resolution
```bash
# Test DNS resolution
nslookup <hostname>
dig <hostname>

# Use IP addresses if DNS issues
{
  "address": "10.1.1.100:5060"  // Use IP instead of hostname
}
```

### Packet Loss and Latency

#### Symptoms
- High response times
- Intermittent monitoring failures
- SIP message timeouts

#### Diagnostic Steps

1. **Measure packet loss**:
   ```bash
   ping -c 100 <endpoint_ip>
   ```

2. **Check network latency**:
   ```bash
   ping <endpoint_ip>
   mtr <endpoint_ip>
   ```

3. **Monitor network quality**:
   ```bash
   iperf3 -c <endpoint_ip> -u
   ```

#### Solutions

##### High Latency Networks
```json
{
  "timeout_seconds": 30,
  "ping_interval_seconds": 120
}
```

##### Unreliable Networks
```json
{
  "timeout_seconds": 15,
  "ping_interval_seconds": 60
}
```

### Firewall and Security

#### Symptoms
- Connections refused
- Partial connectivity
- Security blocking messages

#### Diagnostic Steps

1. **Check local firewall**:
   ```bash
   sudo iptables -L -n
   sudo ufw status verbose
   ```

2. **Test from different sources**:
   ```bash
   # From localhost
   redfire-switch monitor ping 127.0.0.1:5060
   
   # From external
   redfire-switch monitor ping <external_ip>:5060
   ```

3. **Check security logs**:
   ```bash
   grep -i "drop\|reject\|deny" /var/log/syslog
   journalctl -u ufw
   ```

#### Solutions

##### Configure Firewall Rules
```bash
# Allow specific source IPs
sudo iptables -A INPUT -s 10.1.1.0/24 -p udp --dport 5060 -j ACCEPT

# Allow established connections
sudo iptables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
```

##### Security Group Configuration (Cloud)
```bash
# AWS Security Group example
aws ec2 authorize-security-group-ingress \
  --group-id sg-12345678 \
  --protocol udp \
  --port 5060 \
  --source-group sg-87654321
```

## Monitoring Issues

### Monitoring Not Working

#### Symptoms
- No monitoring status
- All endpoints show unknown
- Monitoring disabled message

#### Diagnostic Steps

1. **Check monitoring configuration**:
   ```bash
   redfire-switch show-config | jq '.monitoring'
   ```

2. **Verify monitoring is enabled**:
   ```bash
   redfire-switch monitor status
   ```

3. **Test individual endpoints**:
   ```bash
   redfire-switch monitor ping <endpoint>:5060
   ```

#### Solutions

##### Monitoring Disabled
```json
{
  "monitoring": {
    "enabled": true,  // Ensure this is true
    "endpoints": [...]
  }
}
```

##### No Endpoints Configured
```json
{
  "monitoring": {
    "enabled": true,
    "endpoints": [
      {
        "name": "test-endpoint",
        "address": "192.168.1.100:5060",
        "enabled": true
      }
    ]
  }
}
```

### Endpoints Always Offline

#### Symptoms
- All endpoints show offline
- High failure rates
- Timeout errors

#### Diagnostic Steps

1. **Test connectivity manually**:
   ```bash
   redfire-switch monitor ping <endpoint>:5060 --timeout 10
   ```

2. **Check endpoint configuration**:
   ```bash
   redfire-switch monitor show <endpoint_name>
   ```

3. **Verify network path**:
   ```bash
   traceroute <endpoint_ip>
   telnet <endpoint_ip> 5060
   ```

#### Solutions

##### Increase Timeouts
```json
{
  "timeout_seconds": 15,
  "ping_interval_seconds": 60
}
```

##### Check Endpoint Responses
```bash
# Some endpoints may not respond to OPTIONS
# Check what method they support
nmap -sU -p 5060 <endpoint_ip>
```

##### Network Issues
```bash
# Check for asymmetric routing
# Check NAT/firewall configuration
# Verify SIP ALG settings
```

### High Response Times

#### Symptoms
- Slow monitoring responses
- Variable response times
- Performance warnings

#### Diagnostic Steps

1. **Measure baseline latency**:
   ```bash
   ping <endpoint_ip>
   ```

2. **Compare SIP response times**:
   ```bash
   time redfire-switch monitor ping <endpoint>:5060
   ```

3. **Check network conditions**:
   ```bash
   mtr <endpoint_ip>
   iperf3 -c <endpoint_ip>
   ```

#### Solutions

##### Adjust Monitoring Intervals
```json
{
  "ping_interval_seconds": 60,  // Reduce frequency
  "timeout_seconds": 10         // Increase timeout
}
```

##### Network Optimization
```bash
# Optimize UDP buffer sizes
echo 'net.core.rmem_max = 134217728' >> /etc/sysctl.conf
echo 'net.core.wmem_max = 134217728' >> /etc/sysctl.conf
sysctl -p
```

## Performance Issues

### High CPU Usage

#### Symptoms
- High CPU utilization
- Slow response times
- System performance degradation

#### Diagnostic Steps

1. **Monitor CPU usage**:
   ```bash
   top -p $(pgrep redfire-switch)
   htop -p $(pgrep redfire-switch)
   ```

2. **Check monitoring frequency**:
   ```bash
   redfire-switch show-config | jq '.monitoring.endpoints[].ping_interval_seconds'
   ```

3. **Profile the application**:
   ```bash
   perf top -p $(pgrep redfire-switch)
   ```

#### Solutions

##### Reduce Monitoring Frequency
```json
{
  "ping_interval_seconds": 60,  // Increase from 30
  "endpoints": [
    // Remove unnecessary endpoints
  ]
}
```

##### Optimize System Resources
```bash
# CPU governor
echo performance > /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor

# Process priority
renice -10 $(pgrep redfire-switch)
```

### Memory Issues

#### Symptoms
- High memory usage
- Memory growth over time
- Out of memory errors

#### Diagnostic Steps

1. **Monitor memory usage**:
   ```bash
   ps -o pid,vsz,rss,comm -p $(pgrep redfire-switch)
   ```

2. **Check for memory leaks**:
   ```bash
   valgrind --tool=memcheck redfire-switch start
   ```

3. **System memory status**:
   ```bash
   free -h
   cat /proc/meminfo
   ```

#### Solutions

##### Memory Limits
```bash
# Set memory limits in systemd
[Service]
MemoryLimit=512M
MemoryHigh=400M
```

##### Reduce Monitoring Data
```json
{
  "ping_interval_seconds": 120,  // Reduce frequency
  // Remove unused endpoints
}
```

### Network Performance

#### Symptoms
- High network utilization
- Packet loss
- Network congestion

#### Diagnostic Steps

1. **Monitor network usage**:
   ```bash
   iftop -i eth0
   nethogs
   ```

2. **Check packet statistics**:
   ```bash
   cat /proc/net/udp
   ss -u -a
   ```

3. **Analyze traffic patterns**:
   ```bash
   tcpdump -i any port 5060 -c 100
   ```

#### Solutions

##### Network Optimization
```bash
# Increase buffer sizes
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728

# Increase connection tracking
net.netfilter.nf_conntrack_max = 65536
```

##### Traffic Shaping
```bash
# Rate limit monitoring traffic
tc qdisc add dev eth0 root handle 1: htb default 20
tc class add dev eth0 parent 1: classid 1:1 htb rate 1000kbit
```

## Error Messages

### Common Error Messages

#### "Address already in use"
```bash
Error: Address already in use (os error 98)

# Solution:
sudo netstat -tulnp | grep :5060
# Kill process using port or change configuration
```

#### "Permission denied"
```bash
Error: Permission denied (os error 13)

# Solution:
sudo chown -R redfire:redfire /etc/redfire
sudo chmod 640 /etc/redfire/config.json
```

#### "Invalid IP address"
```bash
Error: invalid IP address: "192.168.1.300"

# Solution: Use valid IP address format
{
  "bind_ip": "192.168.1.1"  // Valid IP
}
```

#### "Connection timed out"
```bash
Error: Connection timed out

# Solution: Check network connectivity
ping <target_ip>
redfire-switch monitor ping <target>:5060 --timeout 15
```

#### "No route to host"
```bash
Error: No route to host

# Solution: Check routing and firewall
ip route get <target_ip>
traceroute <target_ip>
```

### Parsing Error Messages

#### JSON Configuration Errors
```bash
# Missing comma
Error: expected `,` at line 15 column 10

# Solution: Add missing comma
{
  "name": "test",    // Add comma here
  "address": "..."
}
```

#### SIP Message Errors
```bash
# Invalid SIP message
Error: Failed to parse SIP message

# Solution: Check endpoint compatibility
# Some devices may not support OPTIONS method
```

## Recovery Procedures

### Service Recovery

#### Quick Recovery
```bash
# Restart service
sudo systemctl restart redfire-switch

# Check status
sudo systemctl status redfire-switch

# Verify operation
redfire-switch monitor status
```

#### Configuration Recovery
```bash
# Restore from backup
sudo cp /etc/redfire/config.json.backup /etc/redfire/config.json

# Validate restored config
redfire-switch validate-config

# Restart with restored config
sudo systemctl restart redfire-switch
```

### System Recovery

#### Full System Recovery
```bash
# Stop service
sudo systemctl stop redfire-switch

# Restore configuration
sudo cp /opt/backups/redfire/config_latest.json /etc/redfire/config.json

# Check file permissions
sudo chown redfire:redfire /etc/redfire/config.json
sudo chmod 640 /etc/redfire/config.json

# Validate configuration
sudo -u redfire redfire-switch validate-config

# Start service
sudo systemctl start redfire-switch

# Verify operation
sudo systemctl status redfire-switch
redfire-switch monitor status
```

#### Emergency Procedures
```bash
# Generate minimal working config
redfire-switch gen-config --output /etc/redfire/emergency.json

# Start with minimal config
sudo systemctl stop redfire-switch
sudo cp /etc/redfire/emergency.json /etc/redfire/config.json
sudo systemctl start redfire-switch

# Gradually restore full configuration
```

## Getting Help

### Information to Collect

When reporting issues, collect:

1. **Version information**:
   ```bash
   redfire-switch --version
   ```

2. **Configuration** (sanitized):
   ```bash
   redfire-switch show-config
   ```

3. **System information**:
   ```bash
   uname -a
   cat /etc/os-release
   ```

4. **Service logs**:
   ```bash
   journalctl -u redfire-switch --since "1 hour ago"
   ```

5. **Network information**:
   ```bash
   ip addr show
   netstat -tulnp | grep :5060
   ```

### Support Channels

- **GitHub Issues**: https://github.com/redfireswitch/redfire-switch/issues
- **Documentation**: https://docs.redfireswitch.com
- **Community Forum**: https://community.redfireswitch.com

### Debug Mode

Enable verbose logging for detailed troubleshooting:

```bash
# Debug logging
RUST_LOG=debug redfire-switch --verbose start

# Module-specific debugging
RUST_LOG=redfire_switch::monitor=debug redfire-switch start

# Trace level (very verbose)
RUST_LOG=trace redfire-switch start
```