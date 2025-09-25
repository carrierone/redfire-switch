# Redfire Switch Deployment Guide

## Overview

This guide covers production deployment of the Redfire Switch telecommunications system, including infrastructure requirements, installation procedures, configuration, and operational considerations.

## System Requirements

### Production Requirements

| Component | Specification |
|-----------|--------------|
| **CPU** | 16+ cores, 3.0+ GHz (Intel Xeon or AMD EPYC) |
| **RAM** | 64+ GB DDR4/DDR5 |
| **Storage** | 1+ TB NVMe SSD (RAID 10 recommended) |
| **Network** | Dual 10 Gbps Ethernet (bonded) |
| **GPU** | NVIDIA Tesla/Quadro or AMD Instinct (optional, for codec acceleration) |
| **OS** | Ubuntu 22.04 LTS (recommended) |

## Installation Methods

### Method 1: Docker Deployment (Recommended)

```yaml
# docker-compose.yml
version: '3.8'

services:
  redfire-switch:
    image: redfireswitch/redfire-switch:latest
    ports:
      - "5060:5060/udp"  # SIP signaling
      - "8080:8080"      # API
      - "10000-20000:10000-20000/udp"  # RTP media
    environment:
      - DATABASE_URL=postgresql://redfire:password@db:5432/redfire_switch
      - REDIS_URL=redis://redis:6379
      - RUST_LOG=info
    depends_on:
      - db
      - redis
    restart: unless-stopped

  db:
    image: postgres:15
    environment:
      POSTGRES_DB: redfire_switch
      POSTGRES_USER: redfire
      POSTGRES_PASSWORD: password
    volumes:
      - postgres_data:/var/lib/postgresql/data

  redis:
    image: redis:7-alpine
    volumes:
      - redis_data:/data

volumes:
  postgres_data:
  redis_data:
```

Deploy with:
```bash
docker compose up -d
```

### Method 2: Binary Installation

```bash
# Download and install
wget https://github.com/redfire-switch/releases/latest/download/redfire-switch-linux-x86_64.tar.gz
tar -xzf redfire-switch-linux-x86_64.tar.gz
sudo cp redfire-switch /usr/local/bin/

# Create systemd service
sudo tee /etc/systemd/system/redfire-switch.service << 'EOSF'
[Unit]
Description=Redfire Switch
After=network.target

[Service]
Type=exec
ExecStart=/usr/local/bin/redfire-switch
Restart=always
User=redfire
Group=redfire

[Install]
WantedBy=multi-user.target
EOSF

sudo systemctl enable --now redfire-switch
```

## Configuration

### Main Configuration File

Create `/etc/redfire-switch/config.toml`:

```toml
[server]
bind_address = "0.0.0.0:8080"
workers = 16

[database]
url = "postgresql://redfire:password@localhost:5432/redfire_switch"
max_connections = 50

[sip]
bind_address = "0.0.0.0:5060"
external_ip = "203.0.113.100"

[performance]
max_concurrent_calls = 10000
enable_gpu_acceleration = false

[security]
jwt_secret = "your-secret-key"
enable_authentication = true

[logging]
level = "INFO"
format = "json"
```

## Performance Optimization

The system includes advanced performance optimization features:

### 1. Codec Optimization
- GPU-accelerated transcoding for G.711, G.722, and G.729 codecs
- Batch processing for high-volume scenarios
- Memory-efficient codec state management

### 2. Database Optimization
- Query performance monitoring and analysis
- Automatic index recommendations
- Connection pool optimization
- Bulk operation enhancements

### 3. Memory Pool Optimization
- Dynamic pool sizing based on usage patterns
- NUMA-aware allocation strategies
- Memory pressure handling
- Object lifecycle optimization

## Monitoring & Performance

### Get Performance Metrics

```bash
# System performance
curl http://localhost:8080/api/v1/performance/metrics

# Optimization recommendations
curl http://localhost:8080/api/v1/performance/optimizations
```

### Available Metrics

- **System**: CPU, memory, disk I/O
- **Codecs**: Processing times, throughput, error rates
- **Database**: Query performance, connection pool usage
- **Memory**: Pool utilization, allocation patterns
- **Network**: Packet rates, latency, jitter

## Support

For technical support and documentation:
- Documentation: https://docs.redfire-switch.local
- GitHub: https://github.com/redfire-switch/redfire-switch
- Support: support@carrierone.com
