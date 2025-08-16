# Quick Installation Guide

## One-Line Installation (Debian/Ubuntu)

```bash
curl -fsSL https://raw.githubusercontent.com/carrierone/redfire-switch/main/install-debian.sh | sudo bash
```

## Platform-Specific Installation

### Debian/Ubuntu
```bash
# Quick install
./install-debian.sh

# Manual install
sudo apt update
sudo apt install redfire-switch
sudo /usr/share/redfire-switch/scripts/post-install.sh
```

### NixOS
```nix
# Add to configuration.nix
services.redfire-switch = {
  enable = true;
  openFirewall = true;
  settings = {
    sip.bind_address = "0.0.0.0:5060";
    sip.external_ip = "YOUR_PUBLIC_IP";
    sip.domain = "sip.example.com";
  };
  database.createLocally = true;
};
```

### From Source
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build and install
git clone https://github.com/carrierone/redfire-switch.git
cd redfire-switch
cargo build --release --features "bgp-anycast,redis-cluster"
sudo ./scripts/install-from-source.sh
```

## Quick Configuration

1. **Edit configuration:**
```bash
sudo nano /etc/redfire-switch/config.toml
```

2. **Set your IP and domain:**
```toml
[sip]
external_ip = "YOUR_PUBLIC_IP"
domain = "sip.example.com"
```

3. **Start service:**
```bash
sudo systemctl start redfire-switch
sudo systemctl enable redfire-switch
```

## Files and Directories

| Path | Description |
|------|-------------|
| `/etc/redfire-switch/config.toml` | Main configuration |
| `/etc/redfire-switch/bgp-anycast.toml` | BGP Anycast config |
| `/var/lib/redfire-switch/` | Data directory |
| `/var/log/redfire-switch/` | Log files |

## Service Management

```bash
# Status and logs
systemctl status redfire-switch
journalctl -u redfire-switch -f

# Control
systemctl start/stop/restart redfire-switch
systemctl reload redfire-switch  # Reload config
```

## Quick Setup Commands

```bash
# Check installation
redfire-switch --version
redfire-switch check-config

# Health check
redfire-switch health
curl http://localhost:8081/health

# View trunks and routes  
redfire-switch routing list-routes
redfire-switch trunks list
```

## Default Ports

- **SIP**: 5060/udp, 5060/tcp
- **SIP TLS**: 5061/tcp  
- **RTP**: 10000-20000/udp
- **Web UI**: 8080/tcp
- **API**: 8081/tcp

## BGP Anycast (Optional)

```bash
# Enable BGP Anycast
sudo nano /etc/redfire-switch/bgp-anycast.toml
# Set enabled = true and configure your BGP settings

sudo systemctl enable --now redfire-switch-bgp
redfire-switch bgp-anycast status
```

## Web Interface (Optional)

```bash
# Enable web interface
sudo touch /etc/redfire-switch/web-enabled
sudo systemctl enable --now redfire-switch-web

# Access at http://localhost:8080
```

## Troubleshooting

```bash
# Check configuration
redfire-switch check-config

# Test database connection
psql -h localhost -U redfire -d redfire_switch

# Check if ports are listening
ss -tlun | grep 5060

# View detailed logs
journalctl -u redfire-switch --no-pager -l
```

## Security Notes

🔒 **Important**: The installer generates secure passwords automatically:
- Database password: `/etc/redfire-switch/db-credentials`
- Redis password: Generated automatically
- API keys: Generated automatically  
- SSL certificates: Self-signed (replace for production)

For production deployments, see the [full installation guide](INSTALLATION.md).

## Support

- 📖 [Full Documentation](INSTALLATION.md)
- 🐛 [Report Issues](https://github.com/carrierone/redfire-switch/issues)
- 💬 [Community Support](https://discord.gg/redfire-switch)
- 📧 [Commercial Support](mailto:support@carrierone.com)