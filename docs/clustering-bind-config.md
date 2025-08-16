# Cluster Binding Configuration for BGP Anycast

## Overview

When using BGP anycast with Redfire Switch, it's critical that **only SIP traffic** uses the anycast IP addresses. All other traffic (clustering, management APIs, monitoring, etc.) must bind to specific local IP addresses to prevent routing conflicts.

## The Problem

BGP anycast works by advertising the same IP address from multiple locations. Routers direct traffic to the "nearest" node based on BGP metrics. However, this creates problems for non-SIP traffic:

1. **Cluster communication** (gossip protocol) can route to the wrong node
2. **Management APIs** (REST API, curl) become unpredictable  
3. **Session synchronization** between nodes fails
4. **Monitoring** health checks hit random nodes

## Solution: Explicit IP Binding

The `cluster_bind` configuration ensures all non-SIP services bind to local IPs:

```json
{
  "cluster_bind": {
    "enabled": true,
    "cluster_ip": "10.1.1.100",           // Local IP for gossip/heartbeats
    "management_ip": "10.1.1.100",        // Local IP for REST API
    "monitoring_ip": "10.1.1.100",        // Local IP for health checks
    "session_sync_ip": "10.1.1.100",      // Local IP for session sync
    "gossip_port": 7946,
    "management_port": 8080,
    "monitoring_port": 8081,
    "session_sync_port": 8082,
    "validate_no_anycast_bind": true,
    "prohibited_anycast_ips": [
      "192.0.2.100"                       // Anycast IP - forbidden for binding
    ]
  },
  "sip_profiles": [
    {
      "name": "anycast-sip",
      "bind_ip": "192.0.2.100",           // Anycast IP - OK for SIP only
      "port": 5060,
      "protocol": "Udp"
    }
  ]
}
```

## Per-Node Configuration Example

### Node 1 (Chicago)
```json
{
  "cluster_bind": {
    "enabled": true,
    "cluster_ip": "10.1.1.100",         // Chicago local IP
    "management_ip": "10.1.1.100",
    "monitoring_ip": "10.1.1.100", 
    "session_sync_ip": "10.1.1.100",
    "prohibited_anycast_ips": ["192.0.2.100"]
  },
  "anycast": {
    "node": {
      "local_ip": "10.1.1.100",          // Chicago local IP
      "anycast_ip": "192.0.2.100"        // Shared anycast IP
    }
  }
}
```

### Node 2 (Dallas)
```json
{
  "cluster_bind": {
    "enabled": true,
    "cluster_ip": "10.2.1.100",         // Dallas local IP
    "management_ip": "10.2.1.100",
    "monitoring_ip": "10.2.1.100",
    "session_sync_ip": "10.2.1.100",
    "prohibited_anycast_ips": ["192.0.2.100"]
  },
  "anycast": {
    "node": {
      "local_ip": "10.2.1.100",          // Dallas local IP  
      "anycast_ip": "192.0.2.100"        // Shared anycast IP
    }
  }
}
```

## Validation

The configuration validates that:

1. All cluster IPs are explicitly configured (not `0.0.0.0`)
2. No service binds to prohibited anycast IPs
3. SIP profiles can still use anycast IPs (that's intended)

```rust
// Validation automatically runs on config load
let config = Config::load_from_file("config.json")?;
config.validate_anycast_safety()?; // Ensures no conflicts
```

## Service Binding

Services automatically use the correct IPs:

```rust
// Gossip protocol
let bind_addr = config.cluster_bind.get_bind_address(ClusterServiceType::Gossip);
// Returns: 10.1.1.100:7946

// Management API  
let bind_addr = config.cluster_bind.get_bind_address(ClusterServiceType::Management);
// Returns: 10.1.1.100:8080

// Check if IP is safe for binding
if !config.cluster_bind.is_ip_allowed_for_binding(&some_ip) {
    return Err("Cannot bind to anycast IP - use local IP instead");
}
```

## Network Topology

```
Internet → BGP Anycast (192.0.2.100) → Nearest Node
                 ↓
         ┌─────────────────┐         ┌─────────────────┐
         │  Chicago Node   │◄──────►│   Dallas Node   │
         │ Local: 10.1.1.100│ Gossip │ Local: 10.2.1.100│
         │ Anycast: 192.0.2.100│      │ Anycast: 192.0.2.100│
         └─────────────────┘         └─────────────────┘
                 ▲                           ▲
                 │ Management API            │ Management API  
                 │ (10.1.1.100:8080)        │ (10.2.1.100:8080)
                 │                           │
         ┌─────────────────┐         ┌─────────────────┐
         │  Ops Team       │         │  Ops Team       │
         │  curl Chicago   │         │  curl Dallas    │
         └─────────────────┘         └─────────────────┘
```

## Benefits

1. **Reliable cluster communication** - gossip always reaches intended node
2. **Predictable management** - curl/API calls go to specific nodes  
3. **Accurate monitoring** - health checks hit the right endpoints
4. **Faster session sync** - direct node-to-node communication
5. **BGP safety** - anycast only used for SIP traffic as intended

## Error Prevention

Common errors prevented by this configuration:

```bash
# ❌ BAD: Management API on anycast IP
curl http://192.0.2.100:8080/api/calls  # Unpredictable which node responds

# ✅ GOOD: Management API on local IP  
curl http://10.1.1.100:8080/api/calls   # Always hits Chicago node
curl http://10.2.1.100:8080/api/calls   # Always hits Dallas node

# ❌ BAD: Gossip on anycast IP
# Cluster heartbeats might route to wrong node, causing split-brain

# ✅ GOOD: Gossip on local IP
# Cluster communication always reaches intended nodes
```

This configuration ensures reliable clustering while maintaining the benefits of BGP anycast for SIP traffic.