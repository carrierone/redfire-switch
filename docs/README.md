# Redfire Switch Documentation

Welcome to the Redfire Switch documentation. This directory contains comprehensive guides for installing, configuring, operating, and troubleshooting Redfire Switch.

## Documentation Overview

### Getting Started
- **[Main README](../README.md)** - Project overview, quick start, and feature summary
- **[Configuration Guide](CONFIGURATION.md)** - Complete configuration reference and examples
- **[CLI Reference](CLI.md)** - Command-line interface documentation

### Operations
- **[Deployment Guide](DEPLOYMENT.md)** - Production deployment, service management, and security
- **[SIP Monitoring](MONITORING.md)** - SIP endpoint monitoring system documentation
- **[Troubleshooting](TROUBLESHOOTING.md)** - Common issues, diagnostics, and solutions

## Quick Navigation

### For New Users
1. Start with the [Main README](../README.md) for project overview
2. Follow the [Quick Start](../README.md#quick-start) section
3. Review [Configuration basics](CONFIGURATION.md#basic-sip-profile)
4. Learn [CLI commands](CLI.md#core-commands)

### For System Administrators
1. Read the [Deployment Guide](DEPLOYMENT.md) for production setup
2. Review [Security considerations](DEPLOYMENT.md#security-considerations)
3. Set up [Monitoring and logging](DEPLOYMENT.md#monitoring-and-logging)
4. Familiarize yourself with [Troubleshooting procedures](TROUBLESHOOTING.md)

### For Operations Teams
1. Learn [SIP Monitoring](MONITORING.md) system capabilities
2. Review [Performance tuning](DEPLOYMENT.md#performance-tuning)
3. Understand [Maintenance procedures](DEPLOYMENT.md#maintenance-procedures)
4. Know [Recovery procedures](TROUBLESHOOTING.md#recovery-procedures)

## Documentation Structure

```
docs/
├── README.md              # This file - documentation index
├── CONFIGURATION.md       # Configuration reference
├── CLI.md                 # Command-line interface guide
├── MONITORING.md          # SIP monitoring system
├── DEPLOYMENT.md          # Production deployment guide
└── TROUBLESHOOTING.md     # Troubleshooting and diagnostics
```

## Common Tasks

### Configuration Management
- [Generate default config](CLI.md#gen-config) - `redfire-switch gen-config`
- [Validate configuration](CLI.md#validate-config) - `redfire-switch validate-config`
- [View current config](CLI.md#show-config) - `redfire-switch show-config`

### Monitoring Operations
- [Check endpoint status](CLI.md#monitor-status) - `redfire-switch monitor status`
- [Test connectivity](CLI.md#monitor-ping) - `redfire-switch monitor ping <target>`
- [View endpoint details](CLI.md#monitor-show) - `redfire-switch monitor show <endpoint>`

### Service Management
- [Start the switch](DEPLOYMENT.md#service-management) - `systemctl start redfire-switch`
- [Check service status](DEPLOYMENT.md#service-management) - `systemctl status redfire-switch`
- [View service logs](DEPLOYMENT.md#monitoring-and-logging) - `journalctl -u redfire-switch -f`

### Troubleshooting
- [Service issues](TROUBLESHOOTING.md#service-issues)
- [Configuration problems](TROUBLESHOOTING.md#configuration-issues)
- [Network connectivity](TROUBLESHOOTING.md#network-issues)
- [Performance problems](TROUBLESHOOTING.md#performance-issues)

## Configuration Examples

### Basic Setup
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

### Production Setup
```json
{
  "sip_profiles": [
    {
      "name": "carrier-interface",
      "bind_ip": "10.1.1.10",
      "port": 5060,
      "protocol": "Udp",
      "allowed_ips": ["10.1.1.100", "10.1.1.101"]
    }
  ],
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

## CLI Command Examples

### Basic Operations
```bash
# Start the switch
redfire-switch start

# Generate configuration
redfire-switch gen-config --output config.json

# Validate configuration
redfire-switch validate-config

# Show configuration
redfire-switch show-config
```

### Monitoring Commands
```bash
# Check monitoring status
redfire-switch monitor status

# Test endpoint connectivity
redfire-switch monitor ping 192.168.1.100:5060

# Show endpoint details
redfire-switch monitor show upstream-carrier

# Enable/disable endpoint
redfire-switch monitor enable upstream-carrier
redfire-switch monitor disable upstream-carrier
```

### Advanced Usage
```bash
# Start with custom config and verbose logging
redfire-switch --config /etc/redfire/config.json --verbose start

# Test with custom timeout
redfire-switch monitor ping 10.1.1.100:5060 --protocol udp --timeout 10

# Use in scripts
if redfire-switch validate-config; then
    systemctl restart redfire-switch
fi
```

## Deployment Scenarios

### Development Environment
- [Local development setup](DEPLOYMENT.md#installation-methods)
- [Configuration for testing](CONFIGURATION.md#minimal-configuration)
- [Debug logging](CLI.md#environment-variables)

### Production Environment
- [Service installation](DEPLOYMENT.md#service-management)
- [Security hardening](DEPLOYMENT.md#security-considerations)
- [Performance tuning](DEPLOYMENT.md#performance-tuning)
- [Monitoring setup](DEPLOYMENT.md#monitoring-and-logging)

### High Availability
- [Load balancer configuration](DEPLOYMENT.md#high-availability-setup)
- [Redundant deployment](DEPLOYMENT.md#disaster-recovery)
- [Health checks](DEPLOYMENT.md#health-checks)

## Monitoring and Alerting

### Health Monitoring
- [SIP endpoint monitoring](MONITORING.md#sip-options-ping)
- [Service health checks](DEPLOYMENT.md#health-checks)
- [Performance metrics](MONITORING.md#health-metrics)

### Log Analysis
- [Log configuration](DEPLOYMENT.md#log-configuration)
- [Structured logging](DEPLOYMENT.md#system-monitoring)
- [Error analysis](TROUBLESHOOTING.md#log-analysis)

### Alerting Integration
- [Nagios/Icinga checks](DEPLOYMENT.md#alerting-integration)
- [Script integration](CLI.md#script-integration)
- [Future alerting features](MONITORING.md#future-enhancements)

## Support and Community

### Getting Help
- **GitHub Issues**: Report bugs and request features
- **Documentation**: Comprehensive guides and references
- **Community**: User discussions and best practices

### Contributing
- **Code contributions**: Bug fixes and feature development
- **Documentation**: Improvements and additions
- **Testing**: Bug reports and testing feedback

### Resources
- **Main Repository**: https://github.com/redfireswitch/redfire-switch
- **Issue Tracker**: https://github.com/redfireswitch/redfire-switch/issues
- **Website**: https://redfireswitch.com
- **Documentation**: https://docs.redfireswitch.com

## Version History

### v0.1.0 (Current)
- Basic SIP server functionality
- UDP/TCP transport support
- IP-based authentication
- SIP OPTIONS monitoring
- Command-line interface
- JSON configuration
- Multiple SIP profiles

### Planned Features
- TCP SIP monitoring
- SIP authentication (digest)
- Call routing and forwarding
- RESTful API
- Web management interface
- High availability clustering