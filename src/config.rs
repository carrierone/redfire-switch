use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use crate::stir_shaken::StirShakenConfig;
use crate::routing::RoutingConfig;
use crate::cdr::CdrConfig;
use crate::sipt_sipi::SipTSipIConfig;
use crate::termination_routing::TerminationRoutingPlan;
use crate::sms::SmsConfig;
use crate::twilio_api::{TwilioApiConfig, ConversationsConfig};
use crate::security::SecurityConfig;
use crate::billing::BillingConfig;
use crate::rtp_proxy::RtpProxyConfig;
use crate::codec::CodecConfig;
use crate::call_control::CallControlConfig;
use crate::cnam::CnamConfig;
use crate::lcr::types::LrnDipConfig;
#[cfg(feature = "bgp-anycast")]
use crate::bgp_anycast::BgpAnycastConfig;

/// Cluster binding configuration to avoid BGP anycast conflicts
/// All non-SIP traffic must bind to specific local IPs, not anycast IPs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterBindConfig {
    /// Enable cluster binding enforcement
    pub enabled: bool,
    /// Local IP address for cluster communication (gossip, heartbeats)
    pub cluster_ip: IpAddr,
    /// Local IP address for management API (REST API, curl, etc.)
    pub management_ip: IpAddr,
    /// Local IP address for monitoring and health checks
    pub monitoring_ip: IpAddr,
    /// Local IP address for inter-node session synchronization
    pub session_sync_ip: IpAddr,
    /// Port for cluster gossip protocol
    pub gossip_port: u16,
    /// Port for management API
    pub management_port: u16,
    /// Port for monitoring API
    pub monitoring_port: u16,
    /// Port for session synchronization
    pub session_sync_port: u16,
    /// Validate that no service binds to anycast IP
    pub validate_no_anycast_bind: bool,
    /// List of anycast IPs that must not be used for binding
    pub prohibited_anycast_ips: Vec<IpAddr>,
}

impl Default for ClusterBindConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Enabled by default for safety
            cluster_ip: "0.0.0.0".parse().expect("Default IP address should be valid"),
            management_ip: "0.0.0.0".parse().expect("Default IP address should be valid"),
            monitoring_ip: "0.0.0.0".parse().expect("Default IP address should be valid"), 
            session_sync_ip: "0.0.0.0".parse().expect("Default IP address should be valid"),
            gossip_port: 7946,
            management_port: 8080,
            monitoring_port: 8081,
            session_sync_port: 8082,
            validate_no_anycast_bind: true,
            prohibited_anycast_ips: vec![
                "192.0.2.100".parse().expect("Example IP address should be valid"),
            ],
        }
    }
}

impl ClusterBindConfig {
    /// Validate that cluster binding configuration prevents anycast conflicts
    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        // Check that required IPs are configured (not 0.0.0.0)
        let unspecified_v4 = "0.0.0.0".parse::<IpAddr>()
            .map_err(|e| format!("Failed to parse unspecified IPv4: {}", e))?;
        let unspecified_v6 = "::".parse::<IpAddr>()
            .map_err(|e| format!("Failed to parse unspecified IPv6: {}", e))?;

        if self.cluster_ip == unspecified_v4 || self.cluster_ip == unspecified_v6 {
            return Err("cluster_ip must be explicitly configured (cannot be 0.0.0.0 or ::)".to_string());
        }

        if self.management_ip == unspecified_v4 || self.management_ip == unspecified_v6 {
            return Err("management_ip must be explicitly configured (cannot be 0.0.0.0 or ::)".to_string());
        }

        if self.monitoring_ip == unspecified_v4 || self.monitoring_ip == unspecified_v6 {
            return Err("monitoring_ip must be explicitly configured (cannot be 0.0.0.0 or ::)".to_string());
        }

        if self.session_sync_ip == unspecified_v4 || self.session_sync_ip == unspecified_v6 {
            return Err("session_sync_ip must be explicitly configured (cannot be 0.0.0.0 or ::)".to_string());
        }

        // Validate that no service IPs match prohibited anycast IPs
        if self.validate_no_anycast_bind {
            for anycast_ip in &self.prohibited_anycast_ips {
                if self.cluster_ip == *anycast_ip {
                    return Err(format!("cluster_ip {} conflicts with anycast IP - clustering traffic must use local IP", anycast_ip));
                }
                if self.management_ip == *anycast_ip {
                    return Err(format!("management_ip {} conflicts with anycast IP - API traffic must use local IP", anycast_ip));
                }
                if self.monitoring_ip == *anycast_ip {
                    return Err(format!("monitoring_ip {} conflicts with anycast IP - monitoring traffic must use local IP", anycast_ip));
                }
                if self.session_sync_ip == *anycast_ip {
                    return Err(format!("session_sync_ip {} conflicts with anycast IP - session sync traffic must use local IP", anycast_ip));
                }
            }
        }

        Ok(())
    }

    /// Get the appropriate bind address for a service type
    pub fn get_bind_address(&self, service_type: ClusterServiceType) -> SocketAddr {
        match service_type {
            ClusterServiceType::Gossip => SocketAddr::new(self.cluster_ip, self.gossip_port),
            ClusterServiceType::Management => SocketAddr::new(self.management_ip, self.management_port),
            ClusterServiceType::Monitoring => SocketAddr::new(self.monitoring_ip, self.monitoring_port),
            ClusterServiceType::SessionSync => SocketAddr::new(self.session_sync_ip, self.session_sync_port),
        }
    }

    /// Check if a given IP is allowed for binding (not an anycast IP)
    pub fn is_ip_allowed_for_binding(&self, ip: &IpAddr) -> bool {
        if !self.validate_no_anycast_bind {
            return true;
        }

        !self.prohibited_anycast_ips.contains(ip)
    }
}

/// Types of cluster services that need specific IP binding
#[derive(Debug, Clone, Copy)]
pub enum ClusterServiceType {
    /// Gossip protocol for cluster membership
    Gossip,
    /// Management API (REST, curl access)
    Management,
    /// Monitoring and health checks
    Monitoring,
    /// Inter-node session synchronization
    SessionSync,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Protocol {
    Udp,
    Tcp,
    Tls,
    Dtls,
}

impl Default for Protocol {
    fn default() -> Self {
        Protocol::Udp
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// TLS certificate file path
    pub cert_file: String,
    /// TLS private key file path
    pub key_file: String,
    /// CA certificate file path (optional)
    pub ca_file: Option<String>,
    /// Verify peer certificate
    pub verify_peer: bool,
    /// Allowed TLS versions
    pub min_tls_version: String,
    pub max_tls_version: String,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert_file: "certs/server.crt".to_string(),
            key_file: "certs/server.key".to_string(),
            ca_file: Some("certs/ca.crt".to_string()),
            verify_peer: true,
            min_tls_version: "1.2".to_string(),
            max_tls_version: "1.3".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipEndpoint {
    pub name: String,
    pub address: SocketAddr,
    #[serde(default)]
    pub protocol: Protocol,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_ping_interval")]
    pub ping_interval_seconds: u64,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    /// TLS configuration (required if protocol is TLS/DTLS)
    pub tls_config: Option<TlsConfig>,
}

fn default_enabled() -> bool {
    true
}

fn default_ping_interval() -> u64 {
    30
}

fn default_timeout() -> u64 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    pub endpoints: Vec<SipEndpoint>,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        MonitoringConfig {
            enabled: true,
            endpoints: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipProfile {
    pub name: String,
    pub bind_ip: IpAddr,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub protocol: Protocol,
    pub allowed_ips: Vec<IpAddr>,
    /// Enable IPv6 dual-stack support
    #[serde(default = "default_dual_stack")]
    pub dual_stack: bool,
    /// IPv6 bind address (when dual_stack is true)
    pub bind_ipv6: Option<IpAddr>,
    /// IPv6 port (defaults to same as IPv4 port)
    #[serde(default)]
    pub ipv6_port: Option<u16>,
    /// TLS configuration (required if protocol is TLS/DTLS)
    pub tls_config: Option<TlsConfig>,
}

fn default_dual_stack() -> bool {
    false
}

fn default_port() -> u16 {
    5060
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub sip_profiles: Vec<SipProfile>,
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    #[serde(default)]
    pub stir_shaken: StirShakenConfig,
    #[serde(default)]
    pub routing: RoutingConfig,
    #[serde(default)]
    pub cdr: CdrConfig,
    #[serde(default)]
    pub sipt_sipi: SipTSipIConfig,
    /// Termination routing plans
    pub termination_routing: Vec<TerminationRoutingPlan>,
    /// Origination routing service configuration
    #[serde(default)]
    pub origination_routing: OriginationRoutingConfig,
    /// SMS service configuration
    #[serde(default)]
    pub sms: SmsConfig,
    /// Twilio-compatible API configuration
    #[serde(default)]
    pub twilio_api: TwilioApiConfig,
    /// Twilio Conversations configuration
    #[serde(default)]
    pub conversations: ConversationsConfig,
    /// Security configuration
    #[serde(default)]
    pub security: SecurityConfig,
    /// Billing configuration
    #[serde(default)]
    pub billing: BillingConfig,
    /// RTP proxy configuration
    #[serde(default)]
    pub rtp_proxy: RtpProxyConfig,
    /// Codec translation configuration
    #[serde(default)]
    pub codec: CodecConfig,
    /// RCS messaging configuration
    #[serde(default)]
    // RCS removed - feature not implemented
    // pub rcs: RcsConfig,
    /// Call control configuration
    pub call_control: CallControlConfig,
    /// CNAM (Caller ID Name) configuration
    pub cnam: CnamConfig,
    /// BGP Anycast clustering configuration (optional)
    #[cfg(feature = "bgp-anycast")]
    #[serde(default)]
    pub bgp_anycast: BgpAnycastConfig,
    /// SIP clustering configuration
    #[serde(default)]
    // SIP cluster removed - feature not implemented
    // pub sip_cluster: SipClusterConfig,
    /// Cluster binding configuration for non-SIP traffic
    pub cluster_bind: ClusterBindConfig,
    /// LRN dip service configuration
    #[serde(default)]
    pub lrn_dip: LrnDipConfig,
}

/// Origination routing configuration wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginationRoutingConfig {
    /// Enable origination routing
    pub enabled: bool,
    /// DID inventory file path
    pub did_inventory_file: Option<String>,
    /// Toll Free inventory file path  
    pub toll_free_inventory_file: Option<String>,
    /// Vendor configuration file path
    pub vendor_config_file: Option<String>,
}

impl Default for OriginationRoutingConfig {
    fn default() -> Self {
        OriginationRoutingConfig {
            enabled: false,
            did_inventory_file: Some("config/did_inventory.json".to_string()),
            toll_free_inventory_file: Some("config/toll_free_inventory.json".to_string()),
            vendor_config_file: Some("config/vendors.json".to_string()),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            sip_profiles: vec![
                SipProfile {
                    name: "default".to_string(),
                    bind_ip: "0.0.0.0".parse().expect("Invalid default bind IP 0.0.0.0"),
                    port: 5060,
                    protocol: Protocol::Udp,
                    allowed_ips: vec!["127.0.0.1".parse().expect("Invalid default allowed IP 127.0.0.1")],
                    dual_stack: false,
                    bind_ipv6: None,
                    ipv6_port: None,
                    tls_config: None,
                },
                SipProfile {
                    name: "dual-stack-udp".to_string(),
                    bind_ip: "0.0.0.0".parse().expect("Invalid default bind IP 0.0.0.0"),
                    port: 5060,
                    protocol: Protocol::Udp,
                    allowed_ips: vec!["127.0.0.1".parse().expect("Invalid default allowed IP 127.0.0.1"), "::1".parse().expect("Invalid default allowed IPv6 ::1")],
                    dual_stack: true,
                    bind_ipv6: Some("::".parse().expect("Invalid default IPv6 bind address ::")),
                    ipv6_port: Some(5060),
                    tls_config: None,
                },
                SipProfile {
                    name: "tls-ipv6".to_string(),
                    bind_ip: "0.0.0.0".parse().expect("Invalid default bind IP 0.0.0.0"),
                    port: 5061,
                    protocol: Protocol::Tls,
                    allowed_ips: vec!["127.0.0.1".parse().expect("Invalid default allowed IP 127.0.0.1"), "::1".parse().expect("Invalid default allowed IPv6 ::1")],
                    dual_stack: true,
                    bind_ipv6: Some("::".parse().expect("Invalid default IPv6 bind address ::")),
                    ipv6_port: Some(5061),
                    tls_config: Some(TlsConfig::default()),
                }
            ],
            monitoring: MonitoringConfig {
                enabled: true,
                endpoints: vec![
                    SipEndpoint {
                        name: "example-endpoint".to_string(),
                        address: "192.168.1.100:5060".parse().expect("Invalid default monitoring endpoint address"),
                        protocol: Protocol::Udp,
                        enabled: false,
                        ping_interval_seconds: 30,
                        timeout_seconds: 5,
                        tls_config: None,
                    }
                ],
            },
            stir_shaken: StirShakenConfig::default(),
            routing: RoutingConfig::default(),
            cdr: CdrConfig::default(),
            sipt_sipi: SipTSipIConfig::default(),
            termination_routing: vec![],
            origination_routing: OriginationRoutingConfig::default(),
            sms: SmsConfig::default(),
            twilio_api: TwilioApiConfig::default(),
            conversations: ConversationsConfig::default(),
            security: SecurityConfig::default(),
            billing: BillingConfig::default(),
            rtp_proxy: RtpProxyConfig::default(),
            codec: CodecConfig::default(),
            // rcs: RcsConfig::default(),
            call_control: CallControlConfig::default(),
            cnam: CnamConfig::default(),
            cluster_bind: ClusterBindConfig::default(),
            lrn_dip: LrnDipConfig::default(),
        }
    }
}

impl Config {
    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        
        // Comprehensive configuration validation
        config.validate()?;
        
        Ok(config)
    }

    pub fn save_to_file(&self, path: &str) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
    
    /// Comprehensive configuration validation with bounds checking
    pub fn validate(&self) -> anyhow::Result<()> {
        // Validate SIP profiles
        self.validate_sip_profiles()?;
        
        // Validate monitoring configuration
        self.validate_monitoring_config()?;
        
        // Validate TLS configurations
        self.validate_tls_configs()?;
        
        // Validate anycast safety
        self.validate_anycast_safety()
            .map_err(|e| anyhow::anyhow!("Anycast safety validation failed: {}", e))?;
        
        // Validate cluster binding
        self.cluster_bind.validate()
            .map_err(|e| anyhow::anyhow!("Cluster bind validation failed: {}", e))?;
        
        Ok(())
    }

    /// Validate SIP profiles with comprehensive bounds checking
    fn validate_sip_profiles(&self) -> anyhow::Result<()> {
        if self.sip_profiles.is_empty() {
            return Err(anyhow::anyhow!("At least one SIP profile must be configured"));
        }

        for (i, profile) in self.sip_profiles.iter().enumerate() {
            // Validate profile name
            if profile.name.is_empty() {
                return Err(anyhow::anyhow!("SIP profile {} has empty name", i));
            }
            
            if profile.name.len() > 64 {
                return Err(anyhow::anyhow!("SIP profile '{}' name too long (max 64 characters)", profile.name));
            }

            // Validate port ranges
            if profile.port == 0 {
                return Err(anyhow::anyhow!("SIP profile '{}' has invalid port 0", profile.name));
            }
            
            if let Some(ipv6_port) = profile.ipv6_port {
                if ipv6_port == 0 {
                    return Err(anyhow::anyhow!("SIP profile '{}' has invalid IPv6 port 0", profile.name));
                }
            }

            // Validate dual-stack configuration
            if profile.dual_stack {
                if profile.bind_ipv6.is_none() {
                    return Err(anyhow::anyhow!(
                        "SIP profile '{}' has dual_stack enabled but no bind_ipv6 address", 
                        profile.name
                    ));
                }
            }

            // Validate allowed IPs are not empty
            if profile.allowed_ips.is_empty() {
                return Err(anyhow::anyhow!(
                    "SIP profile '{}' must have at least one allowed IP", 
                    profile.name
                ));
            }

            // Validate TLS configuration if protocol requires it
            match profile.protocol {
                Protocol::Tls | Protocol::Dtls => {
                    if profile.tls_config.is_none() {
                        return Err(anyhow::anyhow!(
                            "SIP profile '{}' uses {:?} protocol but has no TLS configuration", 
                            profile.name, profile.protocol
                        ));
                    }
                }
                _ => {}
            }
        }

        // Check for duplicate profile names
        let mut names = std::collections::HashSet::new();
        for profile in &self.sip_profiles {
            if !names.insert(&profile.name) {
                return Err(anyhow::anyhow!("Duplicate SIP profile name: '{}'", profile.name));
            }
        }

        Ok(())
    }

    /// Validate monitoring configuration
    fn validate_monitoring_config(&self) -> anyhow::Result<()> {
        for (i, endpoint) in self.monitoring.endpoints.iter().enumerate() {
            // Validate endpoint name
            if endpoint.name.is_empty() {
                return Err(anyhow::anyhow!("Monitoring endpoint {} has empty name", i));
            }

            // Validate timeout and ping interval bounds
            if endpoint.timeout_seconds == 0 || endpoint.timeout_seconds > 300 {
                return Err(anyhow::anyhow!(
                    "Monitoring endpoint '{}' timeout must be between 1-300 seconds, got {}", 
                    endpoint.name, endpoint.timeout_seconds
                ));
            }

            if endpoint.ping_interval_seconds == 0 || endpoint.ping_interval_seconds > 3600 {
                return Err(anyhow::anyhow!(
                    "Monitoring endpoint '{}' ping interval must be between 1-3600 seconds, got {}", 
                    endpoint.name, endpoint.ping_interval_seconds
                ));
            }

            // Timeout should be less than ping interval
            if endpoint.timeout_seconds >= endpoint.ping_interval_seconds {
                return Err(anyhow::anyhow!(
                    "Monitoring endpoint '{}' timeout ({}) must be less than ping interval ({})", 
                    endpoint.name, endpoint.timeout_seconds, endpoint.ping_interval_seconds
                ));
            }
        }

        Ok(())
    }

    /// Validate TLS configurations
    fn validate_tls_configs(&self) -> anyhow::Result<()> {
        // Check SIP profile TLS configs
        for profile in &self.sip_profiles {
            if let Some(tls) = &profile.tls_config {
                self.validate_tls_config(tls, &format!("SIP profile '{}'", profile.name))?;
            }
        }

        // Check monitoring endpoint TLS configs
        for endpoint in &self.monitoring.endpoints {
            if let Some(tls) = &endpoint.tls_config {
                self.validate_tls_config(tls, &format!("Monitoring endpoint '{}'", endpoint.name))?;
            }
        }

        Ok(())
    }

    /// Validate individual TLS configuration
    fn validate_tls_config(&self, tls: &TlsConfig, context: &str) -> anyhow::Result<()> {
        // Validate certificate file paths
        if tls.cert_file.is_empty() {
            return Err(anyhow::anyhow!("{} has empty cert_file path", context));
        }

        if tls.key_file.is_empty() {
            return Err(anyhow::anyhow!("{} has empty key_file path", context));
        }

        // Validate TLS version format
        let valid_versions = ["1.0", "1.1", "1.2", "1.3"];
        if !valid_versions.contains(&tls.min_tls_version.as_str()) {
            return Err(anyhow::anyhow!(
                "{} has invalid min_tls_version '{}', must be one of: {:?}", 
                context, tls.min_tls_version, valid_versions
            ));
        }

        if !valid_versions.contains(&tls.max_tls_version.as_str()) {
            return Err(anyhow::anyhow!(
                "{} has invalid max_tls_version '{}', must be one of: {:?}", 
                context, tls.max_tls_version, valid_versions
            ));
        }

        // Check min <= max
        let min_version = tls.min_tls_version.replace(".", "").parse::<u32>()
            .map_err(|_| anyhow::anyhow!("{} has invalid min_tls_version format", context))?;
        let max_version = tls.max_tls_version.replace(".", "").parse::<u32>()
            .map_err(|_| anyhow::anyhow!("{} has invalid max_tls_version format", context))?;

        if min_version > max_version {
            return Err(anyhow::anyhow!(
                "{} min_tls_version ({}) must be <= max_tls_version ({})", 
                context, tls.min_tls_version, tls.max_tls_version
            ));
        }

        Ok(())
    }

    /// Validate that the configuration prevents anycast conflicts
    pub fn validate_anycast_safety(&self) -> Result<(), String> {
        // Check SIP profiles don't bind to anycast IPs if clustering is enabled
        if self.cluster_bind.enabled {
            for profile in &self.sip_profiles {
                if !self.cluster_bind.is_ip_allowed_for_binding(&profile.bind_ip) {
                    return Err(format!(
                        "SIP profile '{}' binds to anycast IP {} - only local IPs should be used for SIP when clustering is enabled",
                        profile.name, profile.bind_ip
                    ));
                }
            }
        }
        
        self.cluster_bind.validate()
    }
}