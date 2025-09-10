/*
 * Redfire Switch - API Server Configuration
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiServerConfig {
    /// HTTP/HTTPS listeners
    pub http_listeners: Vec<HttpListener>,
    /// Unix socket listeners
    pub unix_listeners: Vec<UnixListener>,
    /// Global API settings
    pub settings: ApiSettings,
    /// TLS configuration
    pub tls: Option<TlsConfig>,
    /// Rate limiting configuration
    pub rate_limiting: RateLimitConfig,
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            http_listeners: vec![HttpListener {
                enabled: true,
                bind_address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                port: 8080,
                protocol: HttpProtocol::Http,
                name: "localhost-http".to_string(),
                description: "Local HTTP API endpoint".to_string(),
            }],
            unix_listeners: vec![UnixListener {
                enabled: true,
                socket_path: "/var/run/redfire-switch/api.sock".into(),
                name: "main-unix".to_string(),
                description: "Main Unix socket API endpoint".to_string(),
                file_permissions: 0o600,
            }],
            settings: ApiSettings::default(),
            tls: None,
            rate_limiting: RateLimitConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct HttpListener {
    /// Enable this listener
    pub enabled: bool,
    /// IP address to bind to
    #[schema(value_type = String, example = "127.0.0.1")]
    pub bind_address: IpAddr,
    /// Port to bind to
    pub port: u16,
    /// HTTP or HTTPS
    pub protocol: HttpProtocol,
    /// Listener name for identification
    pub name: String,
    /// Human readable description
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum HttpProtocol {
    Http,
    Https,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UnixListener {
    /// Enable this listener
    pub enabled: bool,
    /// Unix socket path
    #[serde(
        serialize_with = "serialize_pathbuf",
        deserialize_with = "deserialize_pathbuf"
    )]
    #[schema(value_type = String, example = "/var/run/redfire-switch/api.sock")]
    pub socket_path: PathBuf,
    /// Listener name for identification
    pub name: String,
    /// Human readable description
    pub description: String,
    /// File permissions for socket (octal, e.g., 0o600)
    pub file_permissions: u32,
}

fn serialize_pathbuf<S>(path: &PathBuf, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    path.to_string_lossy().serialize(serializer)
}

fn deserialize_pathbuf<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(PathBuf::from(s))
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ApiSettings {
    /// API version
    pub version: String,
    /// Maximum request size in bytes
    pub max_request_size_bytes: u64,
    /// Request timeout in seconds
    pub request_timeout_seconds: u64,
    /// Enable CORS
    pub enable_cors: bool,
    /// CORS allowed origins
    pub cors_allowed_origins: Vec<String>,
    /// Enable API documentation endpoints
    pub enable_docs: bool,
    /// Enable metrics endpoint
    pub enable_metrics: bool,
    /// Log all API requests
    pub log_requests: bool,
    /// Log request/response bodies (security risk)
    pub log_request_bodies: bool,
}

impl Default for ApiSettings {
    fn default() -> Self {
        Self {
            version: "v1".to_string(),
            max_request_size_bytes: 10 * 1024 * 1024, // 10MB
            request_timeout_seconds: 30,
            enable_cors: true,
            cors_allowed_origins: vec!["*".to_string()], // Should be restricted in production
            enable_docs: true,
            enable_metrics: true,
            log_requests: true,
            log_request_bodies: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TlsConfig {
    /// Certificate file path
    #[serde(
        serialize_with = "serialize_pathbuf",
        deserialize_with = "deserialize_pathbuf"
    )]
    #[schema(value_type = String, example = "/etc/ssl/certs/server.crt")]
    pub cert_path: PathBuf,
    /// Private key file path
    #[serde(
        serialize_with = "serialize_pathbuf",
        deserialize_with = "deserialize_pathbuf"
    )]
    #[schema(value_type = String, example = "/etc/ssl/private/server.key")]
    pub key_path: PathBuf,
    /// CA certificate for client authentication
    #[serde(
        serialize_with = "serialize_option_pathbuf",
        deserialize_with = "deserialize_option_pathbuf"
    )]
    #[schema(value_type = Option<String>, example = "/etc/ssl/certs/ca.crt")]
    pub ca_cert_path: Option<PathBuf>,
    /// Require client certificates
    pub require_client_cert: bool,
    /// TLS version minimum
    pub min_version: TlsVersion,
    /// Cipher suites (empty for defaults)
    pub cipher_suites: Vec<String>,
}

fn serialize_option_pathbuf<S>(path: &Option<PathBuf>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match path {
        Some(p) => Some(p.to_string_lossy().to_string()).serialize(serializer),
        None => None::<String>.serialize(serializer),
    }
}

fn deserialize_option_pathbuf<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt_s: Option<String> = Option::deserialize(deserializer)?;
    Ok(opt_s.map(PathBuf::from))
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TlsVersion {
    #[serde(rename = "1.2")]
    V1_2,
    #[serde(rename = "1.3")]
    V1_3,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RateLimitConfig {
    /// Enable rate limiting
    pub enabled: bool,
    /// Requests per minute per IP
    pub requests_per_minute_per_ip: u32,
    /// Requests per minute per authenticated user
    pub requests_per_minute_per_user: u32,
    /// Burst size for token bucket
    pub burst_size: u32,
    /// Rate limit window duration in seconds
    pub window_duration_seconds: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_minute_per_ip: 100,
            requests_per_minute_per_user: 1000,
            burst_size: 10,
            window_duration_seconds: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct NetworkListenerConfig {
    /// IPv4 listeners
    pub ipv4: Vec<Ipv4ListenerConfig>,
    /// IPv6 listeners  
    pub ipv6: Vec<Ipv6ListenerConfig>,
    /// Unix socket listeners
    pub unix: Vec<UnixListener>,
}

impl Default for NetworkListenerConfig {
    fn default() -> Self {
        Self {
            ipv4: vec![Ipv4ListenerConfig {
                enabled: true,
                address: Ipv4Addr::new(127, 0, 0, 1),
                port: 8080,
                name: "localhost-v4".to_string(),
                description: "IPv4 localhost API".to_string(),
            }],
            ipv6: vec![Ipv6ListenerConfig {
                enabled: true,
                address: Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1),
                port: 8080,
                name: "localhost-v6".to_string(),
                description: "IPv6 localhost API".to_string(),
            }],
            unix: vec![UnixListener {
                enabled: true,
                socket_path: "/var/run/redfire-switch/api.sock".into(),
                name: "main-unix".to_string(),
                description: "Main Unix socket API endpoint".to_string(),
                file_permissions: 0o600,
            }],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Ipv4ListenerConfig {
    /// Enable this listener
    pub enabled: bool,
    /// IPv4 address to bind (as string)
    #[serde(
        serialize_with = "serialize_ipv4",
        deserialize_with = "deserialize_ipv4"
    )]
    #[schema(value_type = String, example = "127.0.0.1")]
    pub address: Ipv4Addr,
    /// Port to bind
    pub port: u16,
    /// Listener name
    pub name: String,
    /// Description
    pub description: String,
}

fn serialize_ipv4<S>(addr: &Ipv4Addr, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    addr.to_string().serialize(serializer)
}

fn deserialize_ipv4<'de, D>(deserializer: D) -> Result<Ipv4Addr, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Ipv6ListenerConfig {
    /// Enable this listener
    pub enabled: bool,
    /// IPv6 address to bind (as string)
    #[serde(
        serialize_with = "serialize_ipv6",
        deserialize_with = "deserialize_ipv6"
    )]
    #[schema(value_type = String, example = "::1")]
    pub address: Ipv6Addr,
    /// Port to bind
    pub port: u16,
    /// Listener name
    pub name: String,
    /// Description
    pub description: String,
}

fn serialize_ipv6<S>(addr: &Ipv6Addr, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    addr.to_string().serialize(serializer)
}

fn deserialize_ipv6<'de, D>(deserializer: D) -> Result<Ipv6Addr, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}

impl ApiServerConfig {
    pub fn validate(&self) -> Result<(), String> {
        // Validate at least one listener is enabled
        let has_enabled_listener = self.http_listeners.iter().any(|l| l.enabled)
            || self.unix_listeners.iter().any(|l| l.enabled);

        if !has_enabled_listener {
            return Err("At least one listener must be enabled".to_string());
        }

        // Validate HTTP listeners
        for (i, listener) in self.http_listeners.iter().enumerate() {
            if listener.enabled {
                if listener.name.is_empty() {
                    return Err(format!("HTTP listener {} name cannot be empty", i));
                }
                if listener.port == 0 {
                    return Err(format!("HTTP listener {} port must be non-zero", i));
                }
            }
        }

        // Validate Unix listeners
        for (i, listener) in self.unix_listeners.iter().enumerate() {
            if listener.enabled {
                if listener.name.is_empty() {
                    return Err(format!("Unix listener {} name cannot be empty", i));
                }
                if listener.socket_path.as_os_str().is_empty() {
                    return Err(format!("Unix listener {} socket_path cannot be empty", i));
                }
            }
        }

        // Validate TLS configuration
        if let Some(tls) = &self.tls {
            if !tls.cert_path.exists() {
                return Err(format!(
                    "TLS certificate file does not exist: {:?}",
                    tls.cert_path
                ));
            }
            if !tls.key_path.exists() {
                return Err(format!("TLS key file does not exist: {:?}", tls.key_path));
            }
            if let Some(ca_cert) = &tls.ca_cert_path {
                if !ca_cert.exists() {
                    return Err(format!(
                        "TLS CA certificate file does not exist: {:?}",
                        ca_cert
                    ));
                }
            }
        }

        Ok(())
    }

    pub fn get_enabled_http_listeners(&self) -> Vec<&HttpListener> {
        self.http_listeners.iter().filter(|l| l.enabled).collect()
    }

    pub fn get_enabled_unix_listeners(&self) -> Vec<&UnixListener> {
        self.unix_listeners.iter().filter(|l| l.enabled).collect()
    }
}

// Default configurations for different deployment scenarios
impl ApiServerConfig {
    /// Development configuration - localhost only
    pub fn development() -> Self {
        Self {
            http_listeners: vec![HttpListener {
                enabled: true,
                bind_address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                port: 8080,
                protocol: HttpProtocol::Http,
                name: "dev-http".to_string(),
                description: "Development HTTP API".to_string(),
            }],
            unix_listeners: vec![UnixListener {
                enabled: true,
                socket_path: "/tmp/redfire-switch-dev.sock".into(),
                name: "dev-unix".to_string(),
                description: "Development Unix socket".to_string(),
                file_permissions: 0o666, // More permissive for development
            }],
            ..Default::default()
        }
    }

    /// Production configuration - secure defaults
    pub fn production() -> Self {
        Self {
            http_listeners: vec![HttpListener {
                enabled: true,
                bind_address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                port: 8443,
                protocol: HttpProtocol::Https,
                name: "prod-https".to_string(),
                description: "Production HTTPS API".to_string(),
            }],
            unix_listeners: vec![UnixListener {
                enabled: true,
                socket_path: "/var/run/redfire-switch/api.sock".into(),
                name: "prod-unix".to_string(),
                description: "Production Unix socket".to_string(),
                file_permissions: 0o600, // Restrictive permissions
            }],
            settings: ApiSettings {
                cors_allowed_origins: vec![], // No CORS in production
                log_request_bodies: false,    // Security: don't log bodies
                ..Default::default()
            },
            tls: Some(TlsConfig {
                cert_path: "/etc/redfire-switch/tls/server.crt".into(),
                key_path: "/etc/redfire-switch/tls/server.key".into(),
                ca_cert_path: None,
                require_client_cert: false,
                min_version: TlsVersion::V1_3,
                cipher_suites: vec![],
            }),
            rate_limiting: RateLimitConfig {
                enabled: true,
                requests_per_minute_per_ip: 60, // Stricter limits
                requests_per_minute_per_user: 600,
                burst_size: 5,
                window_duration_seconds: 60,
            },
        }
    }

    /// Unix socket only configuration - no network exposure
    pub fn unix_only() -> Self {
        Self {
            http_listeners: vec![], // No HTTP listeners
            unix_listeners: vec![UnixListener {
                enabled: true,
                socket_path: "/var/run/redfire-switch/api.sock".into(),
                name: "unix-only".to_string(),
                description: "Unix socket only API".to_string(),
                file_permissions: 0o600,
            }],
            settings: ApiSettings {
                enable_cors: false, // No CORS needed for Unix sockets
                ..Default::default()
            },
            ..Default::default()
        }
    }
}
