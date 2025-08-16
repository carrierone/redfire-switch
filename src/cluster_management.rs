/*
 * High-Availability Clustering System for RedFire Switch B2BUA
 * Distributed architecture with call state synchronization and failover
 */

use anyhow::Result;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tokio::net::{UdpSocket, TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, warn, error, debug};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Node role in the cluster
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeRole {
    Primary,
    Secondary,
    Standby,
    Observer,
}

/// Node status in the cluster
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Active,
    Inactive,
    Failed,
    Maintenance,
    Joining,
    Leaving,
}

/// Cluster node information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterNode {
    pub node_id: String,
    pub node_name: String,
    pub ip_address: IpAddr,
    pub sip_port: u16,
    pub cluster_port: u16,
    pub role: NodeRole,
    pub status: NodeStatus,
    pub last_heartbeat: SystemTime,
    pub join_time: SystemTime,
    pub capabilities: NodeCapabilities,
    pub load_metrics: NodeLoadMetrics,
    pub version: String,
}

/// Node capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapabilities {
    pub max_concurrent_calls: u64,
    pub supports_stir_shaken: bool,
    pub supports_sip_i: bool,
    pub security_monitoring: bool,
    pub can_be_primary: bool,
    pub geographic_region: String,
}

/// Node load and performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLoadMetrics {
    pub active_calls: u64,
    pub cpu_usage_percent: f32,
    pub memory_usage_percent: f32,
    pub network_utilization_percent: f32,
    pub messages_per_second: f32,
    pub error_rate_percent: f32,
    pub response_time_ms: f32,
    pub last_updated: SystemTime,
}

/// Call state synchronization data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallStateSync {
    pub call_id: String,
    pub session_data: CallSessionData,
    pub sync_timestamp: SystemTime,
    pub sequence_number: u64,
    pub node_id: String,
}

/// Serializable call session data for synchronization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSessionData {
    pub call_id: String,
    pub from_number: String,
    pub to_number: String,
    pub state: String,
    pub created_at: SystemTime,
    pub last_activity: SystemTime,
    pub a_leg_addr: SocketAddr,
    pub b_leg_addr: SocketAddr,
    pub stir_shaken_verified: bool,
    pub sipi_cic: Option<u16>,
    pub custom_headers: HashMap<String, String>,
}

/// Cluster configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    pub enabled: bool,
    pub node_name: String,
    pub cluster_bind_port: u16,
    pub heartbeat_interval_seconds: u64,
    pub heartbeat_timeout_seconds: u64,
    pub call_state_sync_enabled: bool,
    pub failover_timeout_seconds: u64,
    pub split_brain_detection: bool,
    pub quorum_size: usize,
    pub auto_failover: bool,
    pub load_balancing_enabled: bool,
    pub geographic_distribution: bool,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default for single-node operation
            node_name: format!("redfire-{}", Uuid::new_v4().simple()),
            cluster_bind_port: 7946,
            heartbeat_interval_seconds: 5,
            heartbeat_timeout_seconds: 15,
            call_state_sync_enabled: true,
            failover_timeout_seconds: 30,
            split_brain_detection: true,
            quorum_size: 2,
            auto_failover: true,
            load_balancing_enabled: true,
            geographic_distribution: false,
        }
    }
}

/// Cluster membership and state management
pub struct ClusterManager {
    config: ClusterConfig,
    local_node: ClusterNode,
    cluster_nodes: Arc<RwLock<HashMap<String, ClusterNode>>>,
    call_states: Arc<RwLock<HashMap<String, CallStateSync>>>,
    socket: Arc<UdpSocket>,
    tcp_listener: Option<Arc<TcpListener>>,
    is_primary: Arc<RwLock<bool>>,
    sequence_counter: Arc<RwLock<u64>>,
    last_heartbeat_sent: Arc<RwLock<SystemTime>>,
    cluster_metrics: Arc<RwLock<ClusterMetrics>>,
}

/// Cluster-wide metrics and statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterMetrics {
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub failed_nodes: usize,
    pub total_calls: u64,
    pub calls_per_node: HashMap<String, u64>,
    pub average_response_time_ms: f32,
    pub cluster_utilization_percent: f32,
    pub last_failover: Option<SystemTime>,
    pub split_brain_events: u64,
}

impl ClusterManager {
    pub async fn new(
        config: ClusterConfig,
        local_ip: IpAddr,
        sip_port: u16,
    ) -> Result<Self> {
        if !config.enabled {
            info!("🏢 Cluster management disabled - running in single-node mode");
        }

        let local_node = ClusterNode {
            node_id: Uuid::new_v4().to_string(),
            node_name: config.node_name.clone(),
            ip_address: local_ip,
            sip_port,
            cluster_port: config.cluster_bind_port,
            role: NodeRole::Standby, // Start as standby, election will determine primary
            status: NodeStatus::Joining,
            last_heartbeat: SystemTime::now(),
            join_time: SystemTime::now(),
            capabilities: NodeCapabilities {
                max_concurrent_calls: 10000,
                supports_stir_shaken: true,
                supports_sip_i: true,
                security_monitoring: true,
                can_be_primary: true,
                geographic_region: "default".to_string(),
            },
            load_metrics: NodeLoadMetrics {
                active_calls: 0,
                cpu_usage_percent: 0.0,
                memory_usage_percent: 0.0,
                network_utilization_percent: 0.0,
                messages_per_second: 0.0,
                error_rate_percent: 0.0,
                response_time_ms: 0.0,
                last_updated: SystemTime::now(),
            },
            version: "1.0.0".to_string(),
        };

        let socket = UdpSocket::bind(format!("{}:{}", local_ip, config.cluster_bind_port)).await?;
        info!("🏢 Cluster node {} listening on {}:{}", 
              local_node.node_name, local_ip, config.cluster_bind_port);

        // Setup TCP listener for call state synchronization
        let tcp_listener = if config.call_state_sync_enabled {
            let listener = TcpListener::bind(format!("{}:{}", local_ip, config.cluster_bind_port + 1)).await?;
            info!("📞 Call state sync listening on TCP {}:{}", local_ip, config.cluster_bind_port + 1);
            Some(Arc::new(listener))
        } else {
            None
        };

        Ok(Self {
            config,
            local_node,
            cluster_nodes: Arc::new(RwLock::new(HashMap::new())),
            call_states: Arc::new(RwLock::new(HashMap::new())),
            socket: Arc::new(socket),
            tcp_listener,
            is_primary: Arc::new(RwLock::new(false)),
            sequence_counter: Arc::new(RwLock::new(0)),
            last_heartbeat_sent: Arc::new(RwLock::new(SystemTime::now())),
            cluster_metrics: Arc::new(RwLock::new(ClusterMetrics {
                total_nodes: 1,
                active_nodes: 1,
                failed_nodes: 0,
                total_calls: 0,
                calls_per_node: HashMap::new(),
                average_response_time_ms: 0.0,
                cluster_utilization_percent: 0.0,
                last_failover: None,
                split_brain_events: 0,
            })),
        })
    }

    /// Start cluster management services
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            info!("🏢 Cluster management disabled - operating in single-node mode");
            return Ok(());
        }

        info!("🏢 Starting cluster management for node {}", self.local_node.node_name);

        // Start heartbeat system
        self.start_heartbeat_system().await;

        // Start cluster membership management
        self.start_membership_management().await;

        // Start call state synchronization if enabled
        if self.config.call_state_sync_enabled {
            self.start_call_state_sync().await;
        }

        // Start cluster monitoring
        self.start_cluster_monitoring().await;

        // Perform initial cluster discovery
        self.discover_cluster_nodes().await?;

        // Start leader election
        self.start_leader_election().await;

        info!("✅ Cluster management started successfully");
        Ok(())
    }

    /// Synchronize call state across cluster
    pub async fn sync_call_state(&self, call_id: &str, session_data: CallSessionData) -> Result<()> {
        if !self.config.enabled || !self.config.call_state_sync_enabled {
            return Ok(());
        }

        let sequence = {
            let mut counter = self.sequence_counter.write().await;
            *counter += 1;
            *counter
        };

        let sync_data = CallStateSync {
            call_id: call_id.to_string(),
            session_data,
            sync_timestamp: SystemTime::now(),
            sequence_number: sequence,
            node_id: self.local_node.node_id.clone(),
        };

        // Store locally
        {
            let mut call_states = self.call_states.write().await;
            call_states.insert(call_id.to_string(), sync_data.clone());
        }

        // Broadcast to cluster
        self.broadcast_call_state_sync(sync_data).await?;

        debug!("📞 Call state synchronized for call {}", call_id);
        Ok(())
    }

    /// Handle node failure and trigger failover
    pub async fn handle_node_failure(&self, failed_node_id: &str) -> Result<()> {
        warn!("🚨 Node failure detected: {}", failed_node_id);

        // Update node status
        {
            let mut nodes = self.cluster_nodes.write().await;
            if let Some(node) = nodes.get_mut(failed_node_id) {
                node.status = NodeStatus::Failed;
            }
        }

        // Check if failed node was primary
        let was_primary = {
            let nodes = self.cluster_nodes.read().await;
            nodes.get(failed_node_id)
                .map(|n| n.role == NodeRole::Primary)
                .unwrap_or(false)
        };

        if was_primary {
            warn!("🔄 Primary node failed, initiating failover");
            self.initiate_failover().await?;
        }

        // Update cluster metrics
        {
            let mut metrics = self.cluster_metrics.write().await;
            metrics.failed_nodes += 1;
            metrics.active_nodes = metrics.active_nodes.saturating_sub(1);
            metrics.last_failover = Some(SystemTime::now());
        }

        Ok(())
    }

    /// Get cluster status and health
    pub async fn get_cluster_status(&self) -> Result<ClusterStatus> {
        let nodes = self.cluster_nodes.read().await;
        let metrics = self.cluster_metrics.read().await;
        let is_primary = *self.is_primary.read().await;

        let active_nodes: Vec<_> = nodes.values()
            .filter(|n| n.status == NodeStatus::Active)
            .cloned()
            .collect();

        let total_calls = active_nodes.iter()
            .map(|n| n.load_metrics.active_calls)
            .sum();

        let cluster_healthy = active_nodes.len() >= self.config.quorum_size;

        Ok(ClusterStatus {
            cluster_healthy,
            total_nodes: nodes.len(),
            active_nodes: active_nodes.len(),
            failed_nodes: nodes.values().filter(|n| n.status == NodeStatus::Failed).count(),
            local_node_role: self.local_node.role.clone(),
            is_local_primary: is_primary,
            total_cluster_calls: total_calls,
            average_response_time: metrics.average_response_time_ms,
            cluster_utilization: metrics.cluster_utilization_percent,
            last_failover: metrics.last_failover,
            nodes: active_nodes,
        })
    }

    /// Start heartbeat system
    async fn start_heartbeat_system(&self) {
        let socket = Arc::clone(&self.socket);
        let config = self.config.clone();
        let local_node = self.local_node.clone();
        let last_heartbeat_sent = Arc::clone(&self.last_heartbeat_sent);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(config.heartbeat_interval_seconds));

            loop {
                interval.tick().await;

                let heartbeat = ClusterMessage::Heartbeat {
                    node: local_node.clone(),
                    timestamp: SystemTime::now(),
                };

                if let Ok(serialized) = serde_json::to_vec(&heartbeat) {
                    // Broadcast heartbeat to cluster
                    let broadcast_addr: std::net::SocketAddr = "255.255.255.255:7946".parse().unwrap();
                    if let Err(e) = socket.send_to(&serialized, broadcast_addr).await {
                        error!("Failed to send heartbeat: {}", e);
                    } else {
                        let mut last_sent = last_heartbeat_sent.write().await;
                        *last_sent = SystemTime::now();
                        debug!("💓 Heartbeat sent from {}", local_node.node_name);
                    }
                }
            }
        });
    }

    /// Start membership management
    async fn start_membership_management(&self) {
        let socket = Arc::clone(&self.socket);
        let cluster_nodes = Arc::clone(&self.cluster_nodes);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut buffer = vec![0u8; 65536];

            loop {
                match socket.recv_from(&mut buffer).await {
                    Ok((len, from)) => {
                        if let Ok(message) = serde_json::from_slice::<ClusterMessage>(&buffer[..len]) {
                            match message {
                                ClusterMessage::Heartbeat { node, timestamp } => {
                                    debug!("💓 Received heartbeat from {}", node.node_name);
                                    
                                    let mut nodes = cluster_nodes.write().await;
                                    let mut updated_node = node;
                                    updated_node.last_heartbeat = timestamp;
                                    updated_node.status = NodeStatus::Active;
                                    nodes.insert(updated_node.node_id.clone(), updated_node);
                                }
                                ClusterMessage::NodeJoin { node } => {
                                    info!("🔗 Node joining cluster: {}", node.node_name);
                                    let mut nodes = cluster_nodes.write().await;
                                    nodes.insert(node.node_id.clone(), node);
                                }
                                ClusterMessage::NodeLeave { node_id } => {
                                    info!("👋 Node leaving cluster: {}", node_id);
                                    let mut nodes = cluster_nodes.write().await;
                                    nodes.remove(&node_id);
                                }
                                _ => {
                                    // Handle other message types
                                    debug!("📨 Received cluster message: {:?}", message);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error receiving cluster message: {}", e);
                    }
                }
            }
        });
    }

    /// Start call state synchronization
    async fn start_call_state_sync(&self) {
        if let Some(listener) = &self.tcp_listener {
            let listener = Arc::clone(listener);
            let call_states = Arc::clone(&self.call_states);

            tokio::spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok((mut stream, addr)) => {
                            debug!("📞 Call state sync connection from {}", addr);
                            
                            let call_states = Arc::clone(&call_states);
                            tokio::spawn(async move {
                                let mut buffer = vec![0u8; 65536];
                                
                                while let Ok(len) = stream.read(&mut buffer).await {
                                    if len == 0 {
                                        break;
                                    }
                                    
                                    if let Ok(sync_data) = serde_json::from_slice::<CallStateSync>(&buffer[..len]) {
                                        debug!("📞 Received call state sync for {}", sync_data.call_id);
                                        
                                        let mut states = call_states.write().await;
                                        states.insert(sync_data.call_id.clone(), sync_data);
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            error!("Error accepting call state sync connection: {}", e);
                        }
                    }
                }
            });
        }
    }

    /// Start cluster monitoring
    async fn start_cluster_monitoring(&self) {
        let cluster_nodes = Arc::clone(&self.cluster_nodes);
        let cluster_metrics = Arc::clone(&self.cluster_metrics);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));

            loop {
                interval.tick().await;

                let now = SystemTime::now();
                let timeout_threshold = Duration::from_secs(config.heartbeat_timeout_seconds);

                let mut nodes = cluster_nodes.write().await;
                let mut failed_nodes = 0;
                let mut active_nodes = 0;

                // Check for failed nodes
                for node in nodes.values_mut() {
                    if let Ok(elapsed) = now.duration_since(node.last_heartbeat) {
                        if elapsed > timeout_threshold && node.status == NodeStatus::Active {
                            warn!("🚨 Node {} appears to have failed (last heartbeat: {:?} ago)", 
                                  node.node_name, elapsed);
                            node.status = NodeStatus::Failed;
                            failed_nodes += 1;
                        } else if node.status == NodeStatus::Active {
                            active_nodes += 1;
                        }
                    }
                }

                // Update cluster metrics
                {
                    let mut metrics = cluster_metrics.write().await;
                    metrics.total_nodes = nodes.len();
                    metrics.active_nodes = active_nodes;
                    metrics.failed_nodes = failed_nodes;
                }

                debug!("📊 Cluster health check: {} active, {} failed nodes", active_nodes, failed_nodes);
            }
        });
    }

    /// Discover existing cluster nodes
    async fn discover_cluster_nodes(&self) -> Result<()> {
        info!("🔍 Discovering cluster nodes...");

        let join_message = ClusterMessage::NodeJoin {
            node: self.local_node.clone(),
        };

        if let Ok(serialized) = serde_json::to_vec(&join_message) {
            let broadcast_addr: std::net::SocketAddr = "255.255.255.255:7946".parse().unwrap();
            self.socket.send_to(&serialized, broadcast_addr).await?;
        }

        Ok(())
    }

    /// Start leader election process
    async fn start_leader_election(&self) {
        let cluster_nodes = Arc::clone(&self.cluster_nodes);
        let is_primary = Arc::clone(&self.is_primary);
        let local_node_id = self.local_node.node_id.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                let nodes = cluster_nodes.read().await;
                let active_nodes: Vec<_> = nodes.values()
                    .filter(|n| n.status == NodeStatus::Active && n.capabilities.can_be_primary)
                    .collect();

                if !active_nodes.is_empty() {
                    // Simple leader election: node with lowest ID becomes primary
                    let leader = active_nodes.iter()
                        .min_by(|a, b| a.node_id.cmp(&b.node_id))
                        .unwrap();

                    let should_be_primary = leader.node_id == local_node_id;
                    let mut current_primary = is_primary.write().await;
                    
                    if *current_primary != should_be_primary {
                        *current_primary = should_be_primary;
                        if should_be_primary {
                            info!("👑 This node is now the cluster primary");
                        } else {
                            info!("🎯 This node is now a cluster secondary");
                        }
                    }
                }
            }
        });
    }

    /// Initiate cluster failover
    async fn initiate_failover(&self) -> Result<()> {
        info!("🔄 Initiating cluster failover...");

        // Elect new primary
        let nodes = self.cluster_nodes.read().await;
        let candidates: Vec<_> = nodes.values()
            .filter(|n| n.status == NodeStatus::Active && 
                       n.capabilities.can_be_primary &&
                       n.node_id != self.local_node.node_id)
            .collect();

        if let Some(new_primary) = candidates.iter().min_by(|a, b| a.node_id.cmp(&b.node_id)) {
            info!("👑 New primary elected: {}", new_primary.node_name);

            // Broadcast failover message
            let failover_message = ClusterMessage::Failover {
                new_primary_id: new_primary.node_id.clone(),
                timestamp: SystemTime::now(),
            };

            if let Ok(serialized) = serde_json::to_vec(&failover_message) {
                let broadcast_addr: std::net::SocketAddr = "255.255.255.255:7946".parse().unwrap();
                self.socket.send_to(&serialized, broadcast_addr).await?;
            }
        }

        Ok(())
    }

    /// Broadcast call state sync to cluster
    async fn broadcast_call_state_sync(&self, sync_data: CallStateSync) -> Result<()> {
        let nodes = self.cluster_nodes.read().await;
        
        for node in nodes.values() {
            if node.status == NodeStatus::Active && node.node_id != self.local_node.node_id {
                let addr = format!("{}:{}", node.ip_address, node.cluster_port + 1);
                
                if let Ok(mut stream) = TcpStream::connect(addr).await {
                    if let Ok(serialized) = serde_json::to_vec(&sync_data) {
                        if let Err(e) = stream.write_all(&serialized).await {
                            error!("Failed to sync call state to {}: {}", node.node_name, e);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Cluster message types for inter-node communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClusterMessage {
    Heartbeat {
        node: ClusterNode,
        timestamp: SystemTime,
    },
    NodeJoin {
        node: ClusterNode,
    },
    NodeLeave {
        node_id: String,
    },
    CallStateSync {
        sync_data: CallStateSync,
    },
    Failover {
        new_primary_id: String,
        timestamp: SystemTime,
    },
    LoadBalance {
        node_loads: HashMap<String, NodeLoadMetrics>,
    },
    SplitBrainDetection {
        node_id: String,
        timestamp: SystemTime,
    },
}

/// Cluster status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatus {
    pub cluster_healthy: bool,
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub failed_nodes: usize,
    pub local_node_role: NodeRole,
    pub is_local_primary: bool,
    pub total_cluster_calls: u64,
    pub average_response_time: f32,
    pub cluster_utilization: f32,
    pub last_failover: Option<SystemTime>,
    pub nodes: Vec<ClusterNode>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_cluster_manager_creation() {
        crate::security_utils::init_security();
        let config = ClusterConfig::default();
        let cluster = ClusterManager::new(
            config, 
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 
            5060
        ).await.unwrap();

        assert_eq!(cluster.local_node.sip_port, 5060);
        assert!(cluster.local_node.capabilities.supports_stir_shaken);
    }

    #[tokio::test]
    async fn test_call_state_sync() {
        crate::security_utils::init_security();
        let mut config = ClusterConfig::default();
        config.enabled = true;
        config.call_state_sync_enabled = true;

        let cluster = ClusterManager::new(
            config, 
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 
            5060
        ).await.unwrap();

        let session_data = CallSessionData {
            call_id: "test-call-123".to_string(),
            from_number: "+15551234567".to_string(),
            to_number: "+15559876543".to_string(),
            state: "connected".to_string(),
            created_at: SystemTime::now(),
            last_activity: SystemTime::now(),
            a_leg_addr: "127.0.0.1:5060".parse().unwrap(),
            b_leg_addr: "127.0.0.1:5061".parse().unwrap(),
            stir_shaken_verified: true,
            sipi_cic: Some(100),
            custom_headers: HashMap::new(),
        };

        cluster.sync_call_state("test-call-123", session_data).await.unwrap();

        let call_states = cluster.call_states.read().await;
        assert!(call_states.contains_key("test-call-123"));
    }

    #[tokio::test]
    async fn test_cluster_status() {
        crate::security_utils::init_security();
        let config = ClusterConfig::default();
        let cluster = ClusterManager::new(
            config, 
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 
            5060
        ).await.unwrap();

        let status = cluster.get_cluster_status().await.unwrap();
        assert_eq!(status.total_nodes, 0); // No other nodes discovered yet
        assert_eq!(status.local_node_role, NodeRole::Standby);
    }
}