# BGP Anycast Clustering with SIP Session Sharing

## Overview

The BGP Anycast clustering feature allows multiple Redfire Switch instances to share the same IP address using BGP Anycast routing while maintaining consistent SIP call state across the cluster. This provides high availability, geographic load distribution, and seamless failover for SIP services.

## Architecture

```text
Internet -> BGP Anycast IP (192.0.2.100) 
                    |
        +-----------+-----------+
        |           |           |
    Switch-1    Switch-2    Switch-3
   (us-east-1a) (us-east-1b) (us-west-2a)
        |           |           |
        +-----------+-----------+
                    |
            Shared Session Store
            (Redis/etcd/Consul)
```

## Key Features

### 1. BGP Route Advertisement
- Automatic BGP route advertisement based on node health
- Support for ExaBGP, BIRD, FRRouting, and GoBGP
- MED (Multi-Exit Discriminator) based traffic preference
- BGP community tagging for traffic engineering

### 2. Distributed Session Storage
- Redis Cluster support for high-performance session storage
- etcd support for strongly consistent storage
- Consul support for service mesh integration
- Local RocksDB fallback for single-node deployments
- Configurable compression (LZ4, Gzip) for large sessions

### 3. Cluster Membership
- SWIM-based gossip protocol for failure detection
- Automatic node discovery and health monitoring
- Geographic awareness for optimal routing
- Graceful node join/leave operations

### 4. SIP Session Synchronization
- Real-time session replication across cluster nodes
- Automatic session handoff during node failures
- RTP session tracking and migration
- Dialog state preservation during handoffs

### 5. Health Monitoring
- Multi-tier health checks (SIP stack, session store, system resources)
- Automatic BGP withdrawal on health failures
- Gradual recovery with delayed BGP re-advertisement
- Configurable failure and recovery thresholds

## Configuration

### Enable BGP Anycast Feature

Add to `Cargo.toml`:
```toml
[features]
default = ["bgp-anycast"]
bgp-anycast = ["redis", "etcd-rs", "consul", "bgpkit-parser", "memberlist", "raft", "rocksdb"]
```

### Basic Configuration

```json
{
  "bgp_anycast": {
    "enabled": true,
    "node": {
      "node_id": "switch-us-east-1a-001",
      "name": "redfire-switch-east1a", 
      "local_ip": "10.1.1.10",
      "anycast_ip": "192.0.2.100",
      "priority": 100,
      "capacity": 50000,
      "region": "us-east-1",
      "zone": "us-east-1a"
    },
    "session_store": {
      "store_type": "Redis",
      "connection": {
        "urls": ["redis://redis-cluster:6379"]
      }
    }
  }
}
```

## Deployment Scenarios

### 1. Multi-Region High Availability

Deploy switches in multiple regions with shared anycast IP:

```text
Region: us-east-1          Region: us-west-2
┌─────────────────┐       ┌─────────────────┐
│  Switch-East-1  │       │  Switch-West-1  │
│  Priority: 100  │       │  Priority: 90   │
│  MED: 100       │       │  MED: 200       │
└─────────────────┘       └─────────────────┘
         |                          |
         └──────────┬─────────────────┘
                    │
            Redis Cluster
            (Cross-region)
```

Traffic prefers `us-east-1` (lower MED), fails over to `us-west-2` automatically.

### 2. Load Balancing within Region

Multiple switches in same region for capacity scaling:

```text
Region: us-east-1
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│  Switch-1a  │  │  Switch-1b  │  │  Switch-1c  │
│ Priority:100│  │ Priority:100│  │ Priority:100│
│ MED: 100    │  │ MED: 100    │  │ MED: 100    │
└─────────────┘  └─────────────┘  └─────────────┘
       │                │                │
       └────────────────┼────────────────┘
                        │
                Local Redis Cluster
```

Equal preference routing distributes load based on BGP ECMP.

### 3. Geographic Optimization

Route calls to geographically closest switch:

```json
{
  "geo": {
    "enabled": true,
    "prefer_same_region": true,
    "prefer_same_zone": true,
    "weight_factor": 0.3
  }
}
```

## Session Store Backends

### Redis Cluster (Recommended for Production)

```json
{
  "session_store": {
    "store_type": "Redis",
    "connection": {
      "urls": [
        "redis://redis-1:6379",
        "redis://redis-2:6379", 
        "redis://redis-3:6379"
      ],
      "password": "secure-password",
      "tls": {
        "enabled": true,
        "cert_file": "/etc/ssl/redis-client.pem",
        "key_file": "/etc/ssl/redis-client.key"
      }
    },
    "compression": "Lz4",
    "replication": {
      "factor": 3,
      "consistency": "Quorum"
    }
  }
}
```

### etcd (For Strong Consistency)

```json
{
  "session_store": {
    "store_type": "Etcd",
    "connection": {
      "urls": [
        "https://etcd-1:2379",
        "https://etcd-2:2379",
        "https://etcd-3:2379"
      ]
    },
    "replication": {
      "consistency": "Strong"
    }
  }
}
```

## BGP Configuration

### ExaBGP Setup

1. Install ExaBGP:
```bash
pip install exabgp
```

2. Create ExaBGP configuration:
```ini
# /etc/exabgp/exabgp.conf
neighbor 10.1.1.1 {
    router-id 10.1.1.10;
    local-address 10.1.1.10;
    local-as 65000;
    peer-as 64512;
    
    announce {
        ipv4 unicast;
    }
}
```

3. Configure Redfire Switch:
```json
{
  "bgp": {
    "enabled": true,
    "daemon": "ExaBgp",
    "local_asn": 65000,
    "router_id": "10.1.1.10"
  }
}
```

### BIRD Setup

1. Install BIRD:
```bash
apt-get install bird2
```

2. Configure BIRD:
```
# /etc/bird/bird.conf
router id 10.1.1.10;

protocol bgp carrier {
    neighbor 10.1.1.1 as 64512;
    local as 65000;
    ipv4 {
        import none;
        export all;
    };
}
```

## Monitoring and Metrics

### Health Check Endpoints

- `/health` - Overall cluster health
- `/health/bgp` - BGP advertisement status
- `/health/sessions` - Session store connectivity
- `/health/cluster` - Cluster membership status

### Prometheus Metrics

```prometheus
# Cluster metrics
redfire_cluster_nodes_total{region,zone,health}
redfire_cluster_sessions_total{node_id,state}
redfire_cluster_bgp_advertised{node_id}

# Session metrics  
redfire_sessions_total{state,node_id}
redfire_sessions_duration_seconds{percentile}
redfire_session_handoffs_total{from_node,to_node}

# BGP metrics
redfire_bgp_routes_advertised{prefix}
redfire_bgp_session_state{neighbor}
```

## Failover Scenarios

### 1. Node Failure

When a switch fails:
1. Health monitor detects failure
2. BGP routes withdrawn automatically
3. Traffic redirects to healthy nodes
4. Sessions handed off to backup nodes
5. RTP streams re-established

### 2. Network Partition

During network splits:
1. Split-brain detection via session store
2. Minority partition stops BGP advertisement
3. Majority partition continues service
4. Automatic rejoin when partition heals

### 3. Session Store Failure

If Redis/etcd fails:
1. Switches continue with local sessions
2. New sessions distributed based on health
3. Automatic reconnection when store recovers
4. Session resynchronization

## Performance Considerations

### Session Store Performance

- **Redis**: 100K+ ops/sec, low latency
- **etcd**: 10K ops/sec, strong consistency
- **Consul**: 5K ops/sec, service integration

### Network Overhead

- BGP updates: ~1KB per route change
- Gossip traffic: ~100 bytes/sec per node
- Session sync: ~1KB per session update

### Scaling Limits

- Max nodes per cluster: 100 (gossip limit)
- Max sessions per node: Limited by memory
- Session store: Limited by backend capacity

## Security Considerations

### BGP Security

- Use BGP authentication (TCP-AO/MD5)
- Implement route filtering
- Monitor for route hijacking
- Use RPKI validation when available

### Session Store Security

- Enable TLS encryption
- Use strong authentication
- Network isolation (VPC/VLAN)
- Regular security updates

### Inter-Node Communication

- Gossip encryption with pre-shared keys
- TLS for session replication
- Network ACLs between cluster nodes

## Troubleshooting

### Common Issues

1. **BGP Routes Not Advertised**
   - Check health status: `curl http://localhost:8080/health`
   - Verify BGP daemon connectivity
   - Check routing table: `ip route show`

2. **Sessions Not Syncing**
   - Verify session store connectivity
   - Check network connectivity between nodes
   - Review cluster membership status

3. **Split-Brain Scenarios**
   - Check session store quorum
   - Verify network connectivity
   - Review gossip protocol logs

### Debug Commands

```bash
# Check cluster status
curl http://localhost:8080/admin/cluster/status

# List active sessions
curl http://localhost:8080/admin/sessions

# View BGP status  
curl http://localhost:8080/admin/bgp/status

# Get health metrics
curl http://localhost:8080/metrics
```

## Best Practices

### 1. Deployment
- Use odd number of nodes (3, 5, 7) for quorum
- Deploy across multiple availability zones
- Implement proper network segmentation
- Use infrastructure as code

### 2. Configuration
- Set appropriate health check intervals
- Configure session TTLs based on call duration
- Use geographic preferences for routing
- Enable compression for large sessions

### 3. Monitoring
- Monitor BGP route advertisements
- Track session distribution across nodes
- Alert on cluster membership changes
- Monitor session store performance

### 4. Maintenance
- Rolling updates during low-traffic periods
- Graceful node drainage before maintenance
- Session backup before major changes
- Regular cluster health validation

## Examples

See `config-bgp-anycast-example.json` for a complete configuration example.

For more advanced configurations and deployment scenarios, refer to the individual component documentation in the `docs/` directory.