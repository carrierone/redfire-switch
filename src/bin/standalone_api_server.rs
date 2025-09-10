/*
 * Redfire Switch - Standalone API Server (Working Version)
 * Copyright (C) 2025 Carrier One Inc and contributors
 */

use anyhow::Result;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{get, post},
    serve, Router,
};
use chrono::{DateTime, Utc};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::fs;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tracing::info;
use tracing_subscriber;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

#[derive(Parser, Debug)]
#[command(name = "standalone-api-server")]
#[command(about = "RedFire Switch Standalone API Server")]
struct Args {
    /// Port to bind the API server to
    #[arg(short, long, default_value = "8080")]
    port: u16,

    /// IP address to bind to
    #[arg(short, long, default_value = "127.0.0.1")]
    bind: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
    timestamp: DateTime<Utc>,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: Utc::now(),
        }
    }

    #[allow(dead_code)]
    fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
struct SystemStats {
    active_calls: u32,
    uptime_seconds: u64,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct LoginResponse {
    token: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
struct ConfigGenerationRequest {
    /// Configuration type to generate
    config_type: String,
    /// Configuration parameters
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
struct ConfigurationSaveRequest {
    /// Configuration path (e.g., "sip.profiles.internal")
    path: String,
    /// Template name used for this configuration
    template: Option<String>,
    /// Configuration data
    configuration: serde_json::Value,
    /// Timestamp of the save request
    timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
struct ConfigurationData {
    /// Configuration path
    path: String,
    /// Template name
    template: Option<String>,
    /// Configuration data
    configuration: serde_json::Value,
    /// Last modification timestamp
    last_modified: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
struct ConfigGenerationResponse {
    /// Generated configuration as JSON
    config: serde_json::Value,
    /// Configuration file name
    filename: String,
    /// Configuration type
    config_type: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct ConfigTemplateInfo {
    /// Template name
    name: String,
    /// Template description
    description: String,
    /// Required parameters
    required_params: Vec<String>,
    /// Optional parameters with defaults
    optional_params: Vec<ConfigParam>,
}

#[derive(Debug, Serialize, ToSchema)]
struct ConfigParam {
    /// Parameter name
    name: String,
    /// Parameter type
    param_type: String,
    /// Default value
    default_value: Option<serde_json::Value>,
    /// Parameter description
    description: String,
}

#[derive(Clone)]
struct AppState {
    start_time: std::time::Instant,
    configurations: Arc<Mutex<HashMap<String, ConfigurationData>>>,
    config_manager: ConfigurationManager,
}

impl AppState {
    fn new(config_file: String) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            configurations: Arc::new(Mutex::new(HashMap::new())),
            config_manager: ConfigurationManager::new(config_file),
        }
    }
}

// Configuration Management System
#[derive(Debug, Clone)]
struct ConfigurationManager {
    config_file_path: String,
    config_data: Arc<RwLock<serde_json::Value>>,
    id_counters: Arc<RwLock<HashMap<String, AtomicU32>>>,
}

impl ConfigurationManager {
    fn new(config_file_path: String) -> Self {
        Self {
            config_file_path,
            config_data: Arc::new(RwLock::new(serde_json::json!({}))),
            id_counters: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn load_config(&self) -> Result<()> {
        if Path::new(&self.config_file_path).exists() {
            let content = fs::read_to_string(&self.config_file_path).await?;
            let mut config: serde_json::Value = serde_json::from_str(&content)?;

            // Process and assign missing IDs
            let modified = self.ensure_ids(&mut config).await;

            *self.config_data.write().await = config.clone();

            // Save back if we modified anything
            if modified {
                self.save_config().await?;
                info!("Configuration loaded and missing IDs assigned");
            } else {
                info!("Configuration loaded without changes needed");
            }
        } else {
            // Generate default configuration with IDs
            let mut default_config = generate_full_system_config(&serde_json::json!({}))
                .unwrap_or(serde_json::json!({}));
            self.ensure_ids(&mut default_config).await;
            *self.config_data.write().await = default_config;
            self.save_config().await?;
            info!("Default configuration created with auto-generated IDs");
        }
        Ok(())
    }

    async fn save_config(&self) -> Result<()> {
        let config = self.config_data.read().await;
        let content = serde_json::to_string_pretty(&*config)?;

        // Create directory if it doesn't exist
        if let Some(parent) = Path::new(&self.config_file_path).parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&self.config_file_path, content).await?;
        info!("Configuration saved to: {}", self.config_file_path);
        Ok(())
    }

    async fn ensure_ids(&self, config: &mut serde_json::Value) -> bool {
        let mut modified = false;

        // Process SIP Profiles
        if let Some(profiles) = config
            .get_mut("sip_profiles")
            .and_then(|p| p.as_array_mut())
        {
            for profile in profiles {
                if self.ensure_entity_id(profile, "sip_profile", 1).await {
                    modified = true;
                }
            }
        }

        // Process Trunks
        if let Some(trunks) = config.get_mut("trunks").and_then(|t| t.as_object_mut()) {
            for (trunk_type, trunk_list) in trunks {
                if let Some(trunks_array) = trunk_list.as_array_mut() {
                    let id_start = match trunk_type.as_str() {
                        "termination" => 1000,
                        "origination" => 2000,
                        _ => 1000,
                    };
                    for trunk in trunks_array {
                        if self
                            .ensure_entity_id(trunk, &format!("trunk_{}", trunk_type), id_start)
                            .await
                        {
                            modified = true;
                        }
                    }
                }
            }
        }

        // Process Vendors/Customers
        if let Some(vendors_customers) = config
            .get_mut("vendors_customers")
            .and_then(|v| v.as_object_mut())
        {
            for (_, entity) in vendors_customers {
                if self.ensure_entity_id(entity, "vendor_customer", 3000).await {
                    modified = true;
                }
            }
        }

        // Process Carrier Interconnects
        if let Some(interconnects) = config
            .get_mut("carrier_interconnects")
            .and_then(|c| c.as_object_mut())
        {
            for (interconnect_type, interconnect_list) in interconnects {
                if let Some(interconnects_array) = interconnect_list.as_array_mut() {
                    let id_start = match interconnect_type.as_str() {
                        "termination" => 4000,
                        "origination" => 5000,
                        _ => 4000,
                    };
                    for interconnect in interconnects_array {
                        if self
                            .ensure_entity_id(
                                interconnect,
                                &format!("carrier_interconnect_{}", interconnect_type),
                                id_start,
                            )
                            .await
                        {
                            modified = true;
                        }
                    }
                }
            }
        }

        modified
    }

    async fn ensure_entity_id(
        &self,
        entity: &mut serde_json::Value,
        entity_type: &str,
        id_start: u32,
    ) -> bool {
        if entity.get("id").is_none() {
            let new_id = self.get_next_id(entity_type, id_start).await;
            entity["id"] = serde_json::json!(new_id);
            if let Some(name) = entity.get("name").and_then(|n| n.as_str()) {
                info!("Assigned ID {} to {} '{}'", new_id, entity_type, name);
            } else {
                info!("Assigned ID {} to {}", new_id, entity_type);
            }
            true
        } else {
            false
        }
    }

    async fn get_next_id(&self, entity_type: &str, id_start: u32) -> u32 {
        let mut counters = self.id_counters.write().await;
        let counter = counters
            .entry(entity_type.to_string())
            .or_insert_with(|| AtomicU32::new(id_start));
        counter.fetch_add(1, Ordering::SeqCst)
    }

    async fn get_config(&self) -> serde_json::Value {
        self.config_data.read().await.clone()
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/system/stats",
    responses(
        (status = 200, description = "System statistics", body = ApiResponse<SystemStats>)
    ),
    tag = "system"
)]
async fn get_system_stats(
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<SystemStats>>, StatusCode> {
    let uptime = state.start_time.elapsed().as_secs();

    let stats = SystemStats {
        active_calls: 0,
        uptime_seconds: uptime,
        timestamp: Utc::now(),
    };

    Ok(ResponseJson(ApiResponse::success(stats)))
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
struct ActiveCall {
    call_id: String,
    from: String,
    to: String,
    start_time: DateTime<Utc>,
    duration_seconds: u64,
    status: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/calls",
    responses(
        (status = 200, description = "List of active calls", body = ApiResponse<Vec<ActiveCall>>)
    ),
    tag = "calls"
)]
async fn get_active_calls(
    State(_state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<Vec<ActiveCall>>>, StatusCode> {
    // Return empty array for demo - in real implementation would query call manager
    let calls = vec![];

    Ok(ResponseJson(ApiResponse::success(calls)))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = ApiResponse<LoginResponse>)
    ),
    tag = "auth"
)]
async fn login(
    Json(request): Json<LoginRequest>,
) -> Result<ResponseJson<ApiResponse<LoginResponse>>, StatusCode> {
    if request.username == "admin" && request.password == "admin123" {
        let response = LoginResponse {
            token: "demo_token_1234567890".to_string(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
        };
        Ok(ResponseJson(ApiResponse::success(response)))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/config/templates",
    responses(
        (status = 200, description = "Available configuration templates", body = ApiResponse<Vec<ConfigTemplateInfo>>)
    ),
    tag = "config"
)]
async fn get_config_templates(
) -> Result<ResponseJson<ApiResponse<Vec<ConfigTemplateInfo>>>, StatusCode> {
    let templates = vec![
        ConfigTemplateInfo {
            name: "basic_sip".to_string(),
            description: "Class 4 SIP profile configuration".to_string(),
            required_params: vec!["name".to_string(), "bind_ip".to_string()],
            optional_params: vec![
                ConfigParam {
                    name: "port".to_string(),
                    param_type: "number".to_string(),
                    default_value: Some(serde_json::json!(5060)),
                    description: "SIP listen port".to_string(),
                },
                ConfigParam {
                    name: "transport".to_string(),
                    param_type: "string".to_string(),
                    default_value: Some(serde_json::json!("udp")),
                    description: "Transport protocol (udp/tcp/tls)".to_string(),
                },
                ConfigParam {
                    name: "max_sessions".to_string(),
                    param_type: "number".to_string(),
                    default_value: Some(serde_json::json!(100000)),
                    description: "Maximum concurrent sessions".to_string(),
                },
                ConfigParam {
                    name: "session_timer".to_string(),
                    param_type: "number".to_string(),
                    default_value: Some(serde_json::json!(1800)),
                    description: "Session timer in seconds".to_string(),
                },
                ConfigParam {
                    name: "enable_registration".to_string(),
                    param_type: "boolean".to_string(),
                    default_value: Some(serde_json::json!(false)),
                    description: "Enable SIP registration (typically disabled for Class 4)"
                        .to_string(),
                },
                ConfigParam {
                    name: "transit_mode".to_string(),
                    param_type: "boolean".to_string(),
                    default_value: Some(serde_json::json!(true)),
                    description: "Enable transit mode for inter-carrier traffic".to_string(),
                },
                ConfigParam {
                    name: "sip_i_support".to_string(),
                    param_type: "boolean".to_string(),
                    default_value: Some(serde_json::json!(true)),
                    description: "Enable SIP-I (SIP for ISUP) support for SS7 interworking"
                        .to_string(),
                },
                ConfigParam {
                    name: "isup_interworking".to_string(),
                    param_type: "boolean".to_string(),
                    default_value: Some(serde_json::json!(true)),
                    description: "Enable ISUP interworking for TDM network integration".to_string(),
                },
                ConfigParam {
                    name: "ss7_protocol".to_string(),
                    param_type: "string".to_string(),
                    default_value: Some(serde_json::json!("itu_t")),
                    description: "SS7 protocol variant (itu_t/ansi/china)".to_string(),
                },
                ConfigParam {
                    name: "circuit_group_support".to_string(),
                    param_type: "boolean".to_string(),
                    default_value: Some(serde_json::json!(true)),
                    description: "Enable circuit group management for TDM trunks".to_string(),
                },
                ConfigParam {
                    name: "cic_range".to_string(),
                    param_type: "string".to_string(),
                    default_value: Some(serde_json::json!("1-31")),
                    description: "Circuit identification codes range (e.g., 1-31,33-62)"
                        .to_string(),
                },
            ],
        },
        ConfigTemplateInfo {
            name: "stir_shaken".to_string(),
            description: "STIR/SHAKEN authentication configuration".to_string(),
            required_params: vec![
                "enabled".to_string(),
                "cert_path".to_string(),
                "key_path".to_string(),
            ],
            optional_params: vec![ConfigParam {
                name: "validation_cache_ttl".to_string(),
                param_type: "number".to_string(),
                default_value: Some(serde_json::json!(300)),
                description: "Validation cache TTL in seconds".to_string(),
            }],
        },
        ConfigTemplateInfo {
            name: "routing_lcr".to_string(),
            description: "Least Cost Routing configuration".to_string(),
            required_params: vec!["enabled".to_string()],
            optional_params: vec![
                ConfigParam {
                    name: "database_url".to_string(),
                    param_type: "string".to_string(),
                    default_value: Some(serde_json::json!("postgresql://user:pass@localhost/lcr")),
                    description: "Database connection string".to_string(),
                },
                ConfigParam {
                    name: "route_limit".to_string(),
                    param_type: "number".to_string(),
                    default_value: Some(serde_json::json!(10)),
                    description: "Maximum routes to return".to_string(),
                },
            ],
        },
        ConfigTemplateInfo {
            name: "trunk".to_string(),
            description: "Trunk configuration for call routing and manipulation".to_string(),
            required_params: vec![
                "name".to_string(),
                "carrier_interconnect".to_string(),
                "vendor_customer".to_string(),
            ],
            optional_params: vec![
                ConfigParam {
                    name: "sip_profile_id".to_string(),
                    param_type: "association".to_string(),
                    default_value: Some(serde_json::json!(1)),
                    description: "SIP Profile association for trunk".to_string(),
                },
                ConfigParam {
                    name: "tech_prefix".to_string(),
                    param_type: "string".to_string(),
                    default_value: Some(serde_json::json!("")),
                    description: "Technology prefix for trunk identification".to_string(),
                },
                ConfigParam {
                    name: "trunk_type".to_string(),
                    param_type: "string".to_string(),
                    default_value: Some(serde_json::json!("termination")),
                    description: "Trunk type (termination/origination)".to_string(),
                },
                ConfigParam {
                    name: "max_concurrent_calls".to_string(),
                    param_type: "number".to_string(),
                    default_value: Some(serde_json::json!(100)),
                    description: "Maximum concurrent calls allowed on this trunk".to_string(),
                },
                ConfigParam {
                    name: "calls_per_second".to_string(),
                    param_type: "number".to_string(),
                    default_value: Some(serde_json::json!(5)),
                    description: "Maximum calls per second rate limit".to_string(),
                },
                ConfigParam {
                    name: "strip_digits".to_string(),
                    param_type: "number".to_string(),
                    default_value: Some(serde_json::json!(0)),
                    description: "Number of digits to strip from dialed number".to_string(),
                },
                ConfigParam {
                    name: "add_prefix".to_string(),
                    param_type: "string".to_string(),
                    default_value: Some(serde_json::json!("")),
                    description: "Prefix to add to dialed number".to_string(),
                },
                ConfigParam {
                    name: "allowed_codecs".to_string(),
                    param_type: "array".to_string(),
                    default_value: Some(serde_json::json!(["g711u", "g711a", "g729"])),
                    description: "List of allowed audio codecs".to_string(),
                },
                ConfigParam {
                    name: "stir_shaken_enabled".to_string(),
                    param_type: "boolean".to_string(),
                    default_value: Some(serde_json::json!(true)),
                    description: "Enable STIR/SHAKEN for this trunk".to_string(),
                },
            ],
        },
        ConfigTemplateInfo {
            name: "security".to_string(),
            description: "Security and fraud protection configuration".to_string(),
            required_params: vec!["enabled".to_string()],
            optional_params: vec![
                ConfigParam {
                    name: "max_call_rate".to_string(),
                    param_type: "number".to_string(),
                    default_value: Some(serde_json::json!(100)),
                    description: "Maximum calls per minute per IP".to_string(),
                },
                ConfigParam {
                    name: "blacklist_enabled".to_string(),
                    param_type: "boolean".to_string(),
                    default_value: Some(serde_json::json!(true)),
                    description: "Enable IP blacklisting".to_string(),
                },
            ],
        },
        ConfigTemplateInfo {
            name: "database".to_string(),
            description: "Database configuration for CDR and operational data".to_string(),
            required_params: vec!["database_url".to_string()],
            optional_params: vec![
                ConfigParam {
                    name: "max_connections".to_string(),
                    param_type: "number".to_string(),
                    default_value: Some(serde_json::json!(100)),
                    description: "Maximum database connections in pool".to_string(),
                },
                ConfigParam {
                    name: "connection_timeout".to_string(),
                    param_type: "number".to_string(),
                    default_value: Some(serde_json::json!(30)),
                    description: "Database connection timeout in seconds".to_string(),
                },
                ConfigParam {
                    name: "ssl_enabled".to_string(),
                    param_type: "boolean".to_string(),
                    default_value: Some(serde_json::json!(true)),
                    description: "Enable SSL/TLS for database connections".to_string(),
                },
            ],
        },
        ConfigTemplateInfo {
            name: "monitoring".to_string(),
            description: "System monitoring and metrics configuration".to_string(),
            required_params: vec!["enabled".to_string()],
            optional_params: vec![
                ConfigParam {
                    name: "prometheus_port".to_string(),
                    param_type: "number".to_string(),
                    default_value: Some(serde_json::json!(9090)),
                    description: "Prometheus metrics server port".to_string(),
                },
                ConfigParam {
                    name: "snmp_enabled".to_string(),
                    param_type: "boolean".to_string(),
                    default_value: Some(serde_json::json!(false)),
                    description: "Enable SNMP monitoring".to_string(),
                },
                ConfigParam {
                    name: "health_check_interval".to_string(),
                    param_type: "number".to_string(),
                    default_value: Some(serde_json::json!(60)),
                    description: "Health check interval in seconds".to_string(),
                },
            ],
        },
        ConfigTemplateInfo {
            name: "billing".to_string(),
            description: "Rating and CDR processing configuration for B/OSS integration"
                .to_string(),
            required_params: vec!["enabled".to_string()],
            optional_params: vec![
                ConfigParam {
                    name: "currency".to_string(),
                    param_type: "string".to_string(),
                    default_value: Some(serde_json::json!("USD")),
                    description: "Default currency for rate calculations".to_string(),
                },
                ConfigParam {
                    name: "rating_precision".to_string(),
                    param_type: "number".to_string(),
                    default_value: Some(serde_json::json!(6)),
                    description: "Decimal precision for rate calculations".to_string(),
                },
                ConfigParam {
                    name: "cdr_format".to_string(),
                    param_type: "string".to_string(),
                    default_value: Some(serde_json::json!("json")),
                    description: "CDR export format (json, csv, xml)".to_string(),
                },
            ],
        },
    ];

    Ok(ResponseJson(ApiResponse::success(templates)))
}

#[utoipa::path(
    post,
    path = "/api/v1/config/generate",
    request_body = ConfigGenerationRequest,
    responses(
        (status = 200, description = "Generated configuration", body = ApiResponse<ConfigGenerationResponse>)
    ),
    tag = "config"
)]
async fn generate_config(
    State(state): State<AppState>,
    Json(request): Json<ConfigGenerationRequest>,
) -> Result<ResponseJson<ApiResponse<ConfigGenerationResponse>>, StatusCode> {
    let config = match request.config_type.as_str() {
        "basic_sip" => generate_basic_sip_config(&request.parameters),
        "trunk" => generate_trunk_config(&request.parameters),
        "stir_shaken" => generate_stir_shaken_config(&request.parameters),
        "routing_lcr" => generate_routing_lcr_config(&request.parameters),
        "security" => generate_security_config(&request.parameters),
        "database" => generate_database_config(&request.parameters),
        "monitoring" => generate_monitoring_config(&request.parameters),
        "billing" => generate_billing_config(&request.parameters),
        "full_system" => Some(state.config_manager.get_config().await),
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let response = ConfigGenerationResponse {
        config: config.unwrap_or_else(|| serde_json::json!({})),
        filename: format!("{}.json", request.config_type),
        config_type: request.config_type,
    };

    Ok(ResponseJson(ApiResponse::success(response)))
}

fn generate_basic_sip_config(params: &serde_json::Value) -> Option<serde_json::Value> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let ip = params
        .get("ip")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0.0");
    let port = params.get("port").and_then(|v| v.as_u64()).unwrap_or(5060) as u16;
    let transport = params
        .get("transport")
        .and_then(|v| v.as_str())
        .unwrap_or("udp");

    Some(serde_json::json!({
        "sip_profiles": [
            {
                "name": name,
                "ip": ip,
                "port": port,
                "transport": transport,
                "max_sessions": 1000,
                "session_timer": 1800,
                "use_rport": true,
                "auth_calls": false,
                "apply_inbound_acl": "domains",
                "apply_register_acl": "domains"
            }
        ]
    }))
}

fn generate_trunk_config(params: &serde_json::Value) -> Option<serde_json::Value> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("default_trunk");
    let sip_profile_id = params
        .get("sip_profile_id")
        .and_then(|v| v.as_i64())
        .unwrap_or(1);
    let carrier_interconnect = params
        .get("carrier_interconnect")
        .and_then(|v| v.as_str())
        .unwrap_or("default_interconnect");
    let vendor_customer = params
        .get("vendor_customer")
        .and_then(|v| v.as_str())
        .unwrap_or("default_vendor");
    let tech_prefix = params
        .get("tech_prefix")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let trunk_type = params
        .get("trunk_type")
        .and_then(|v| v.as_str())
        .unwrap_or("termination");
    let max_concurrent_calls = params
        .get("max_concurrent_calls")
        .and_then(|v| v.as_i64())
        .unwrap_or(100);
    let calls_per_second = params
        .get("calls_per_second")
        .and_then(|v| v.as_i64())
        .unwrap_or(5);
    let strip_digits = params
        .get("strip_digits")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let add_prefix = params
        .get("add_prefix")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    Some(serde_json::json!({
        "name": name,
        "description": format!("{} trunk configuration", name),
        "tech_prefix": tech_prefix,
        "sip_profile_id": sip_profile_id,
        "carrier_interconnect": carrier_interconnect,
        "vendor_customer": vendor_customer,
        "trunk_type": trunk_type,
        "digit_manipulation": {
            "strip_digits": strip_digits,
            "add_prefix": add_prefix,
            "add_suffix": "",
            "number_translation": []
        },
        "call_limits": {
            "max_concurrent_calls": max_concurrent_calls,
            "calls_per_second": calls_per_second,
            "max_call_duration": 7200
        },
        "allowed_codecs": ["g711u", "g711a", "g729"],
        "stir_shaken": {
            "enabled": true,
            "attestation_level": "B",
            "verify_incoming": true
        },
        "lcr_group": "default"
    }))
}

fn generate_stir_shaken_config(params: &serde_json::Value) -> Option<serde_json::Value> {
    let enabled = params.get("enabled")?.as_bool().unwrap_or(true);
    let cert_path = params.get("cert_path")?.as_str()?;
    let key_path = params.get("key_path")?.as_str()?;
    let cache_ttl = params
        .get("validation_cache_ttl")
        .and_then(|v| v.as_u64())
        .unwrap_or(300);

    Some(serde_json::json!({
        "stir_shaken": {
            "enabled": enabled,
            "certificate_path": cert_path,
            "private_key_path": key_path,
            "validation_cache_ttl": cache_ttl,
            "attest_level": "A",
            "verify_incoming": true,
            "sign_outgoing": true,
            "passport_cache_size": 1000
        }
    }))
}

fn generate_routing_lcr_config(params: &serde_json::Value) -> Option<serde_json::Value> {
    let enabled = params.get("enabled")?.as_bool().unwrap_or(true);
    let database_url = params
        .get("database_url")
        .and_then(|v| v.as_str())
        .unwrap_or("postgresql://user:pass@localhost/lcr");
    let route_limit = params
        .get("route_limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(10);

    Some(serde_json::json!({
        "routing": {
            "enabled": enabled,
            "database_url": database_url,
            "max_routes": route_limit,
            "failover_enabled": true,
            "route_timeout": 5,
            "quality_threshold": 4.5,
            "cost_optimization": true
        }
    }))
}

fn generate_security_config(params: &serde_json::Value) -> Option<serde_json::Value> {
    let enabled = params.get("enabled")?.as_bool().unwrap_or(true);
    let max_call_rate = params
        .get("max_call_rate")
        .and_then(|v| v.as_u64())
        .unwrap_or(100);
    let blacklist_enabled = params.get("blacklist_enabled")?.as_bool().unwrap_or(true);

    Some(serde_json::json!({
        "security": {
            "enabled": enabled,
            "max_calls_per_minute": max_call_rate,
            "blacklist": {
                "enabled": blacklist_enabled,
                "auto_block_threshold": 50,
                "block_duration": 3600
            },
            "rate_limiting": {
                "enabled": true,
                "calls_per_second": 10,
                "burst_size": 20
            },
            "fraud_detection": {
                "enabled": true,
                "short_duration_threshold": 6,
                "high_pdd_threshold": 10000,
                "sequential_failure_threshold": 10
            }
        }
    }))
}

fn generate_database_config(params: &serde_json::Value) -> Option<serde_json::Value> {
    let database_url = params
        .get("database_url")
        .and_then(|v| v.as_str())
        .unwrap_or("postgresql://redfire:password@localhost/redfire");
    let max_connections = params
        .get("max_connections")
        .and_then(|v| v.as_u64())
        .unwrap_or(100);
    let connection_timeout = params
        .get("connection_timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(30);
    let ssl_enabled = params
        .get("ssl_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    Some(serde_json::json!({
        "database": {
            "connections": {
                "cdr_database": {
                    "url": database_url,
                    "max_connections": max_connections,
                    "connection_timeout": connection_timeout,
                    "ssl_mode": if ssl_enabled { "require" } else { "disable" }
                },
                "config_database": {
                    "url": database_url.replace("/redfire", "/config"),
                    "max_connections": max_connections / 2,
                    "connection_timeout": connection_timeout,
                    "ssl_mode": if ssl_enabled { "require" } else { "disable" }
                }
            },
            "cdr": {
                "table_name": "call_detail_records",
                "retention_days": 365,
                "partition_by": "month",
                "compression": "enabled"
            },
            "backup": {
                "enabled": true,
                "frequency": "daily",
                "retention_count": 7,
                "backup_path": "/opt/redfire/backups/"
            }
        }
    }))
}

fn generate_monitoring_config(params: &serde_json::Value) -> Option<serde_json::Value> {
    let enabled = params
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let prometheus_port = params
        .get("prometheus_port")
        .and_then(|v| v.as_u64())
        .unwrap_or(9090);
    let snmp_enabled = params
        .get("snmp_enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let health_check_interval = params
        .get("health_check_interval")
        .and_then(|v| v.as_u64())
        .unwrap_or(60);

    Some(serde_json::json!({
        "monitoring": {
            "enabled": enabled,
            "metrics": {
                "prometheus": {
                    "enabled": enabled,
                    "port": prometheus_port,
                    "path": "/metrics"
                },
                "snmp": {
                    "enabled": snmp_enabled,
                    "port": 161,
                    "community": "public"
                },
                "collection_interval": 30,
                "retention_days": 30
            },
            "alerts": {
                "enabled": enabled,
                "rules": [
                    {
                        "name": "high_call_volume",
                        "threshold": 1000,
                        "duration": "5m",
                        "severity": "warning"
                    },
                    {
                        "name": "system_memory_high",
                        "threshold": 85,
                        "duration": "2m",
                        "severity": "critical"
                    }
                ],
                "notification_channels": ["email", "webhook"]
            },
            "health_checks": {
                "enabled": enabled,
                "interval": health_check_interval,
                "endpoints": [
                    "/health",
                    "/metrics",
                    "/api/v1/system/stats"
                ]
            }
        }
    }))
}

fn generate_billing_config(params: &serde_json::Value) -> Option<serde_json::Value> {
    let enabled = params
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let currency = params
        .get("currency")
        .and_then(|v| v.as_str())
        .unwrap_or("USD");
    let rating_precision = params
        .get("rating_precision")
        .and_then(|v| v.as_u64())
        .unwrap_or(6);
    let cdr_format = params
        .get("cdr_format")
        .and_then(|v| v.as_str())
        .unwrap_or("json");

    Some(serde_json::json!({
        "billing": {
            "enabled": enabled,
            "rating": {
                "currency": currency,
                "precision": rating_precision,
                "rounding_mode": "half_up",
                "minimum_duration": 1,
                "billing_increment": 1,
                "rate_table_update_interval": 3600
            },
            "cdr_processing": {
                "format": cdr_format,
                "export_path": "/opt/redfire/exports/cdr/",
                "batch_size": 1000,
                "compression": "gzip",
                "export_interval": 300,
                "boss_integration": {
                    "enabled": true,
                    "endpoint": "http://boss-system/api/cdr/import",
                    "auth_method": "api_key",
                    "retry_attempts": 3
                }
            },
            "rate_engine": {
                "lcr_enabled": true,
                "rate_cache_ttl": 3600,
                "fallback_rate": 0.01,
                "rate_deck_priority": ["premium", "standard", "wholesale"]
            }
        }
    }))
}

fn generate_full_system_config(params: &serde_json::Value) -> Option<serde_json::Value> {
    // Generate a comprehensive system configuration
    let system_name = params
        .get("system_name")
        .and_then(|v| v.as_str())
        .unwrap_or("redfire-switch");
    let bind_ip = params
        .get("bind_ip")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0.0");

    Some(serde_json::json!({
        "system": {
            "name": system_name,
            "domain": "redfire.local",
            "description": "RedFire Switch PBX System"
        },
        "sip_profiles": [
            {
                "id": 1,
                "name": "default",
                "description": "Default SIP profile for Class 4 switch operations",
                "bind_ip": "0.0.0.0",
                "port": 5060,
                "transport": "udp",
                "max_sessions": 100000,
                "session_timer": 1800,
                "enable_registration": false,
                "auth_calls": false,
                "transit_mode": true,
                "codec_negotiation": "transparent",
                "dtmf_relay": "rfc2833",
                "record_route": true,
                "proxy_media": false,
                "sip_i_support": true,
                "isup_interworking": true,
                "ss7_gateway_support": true,
                "cause_code_mapping": "itu_t",
                "release_source_header": true
            }
        ],
        "carrier_interconnects": {
            "termination": [
                {
                    "name": "tier1_termination",
                    "description": "Tier 1 carrier termination interconnect",
                    "carrier_id": "tier1_001",
                    "carrier_name": "Tier1 Carrier Corp",
                    "direction": "termination",
                    "remote_ip": "203.0.113.10",
                    "remote_port": 5060,
                    "transport": "udp",
                    "lcr_group": "tier1",
                    "quality_score": 95,
                    "capacity_limit": 1000,
                    "codec_preference": ["g711u", "g711a", "g729"],
                    "sip_i_support": true,
                    "isup_interworking": true,
                    "ss7_protocol": "itu_t",
                    "circuit_group_support": true,
                    "cic_range": "1-31,33-62",
                    "authentication": {
                        "type": "ip_auth",
                        "trusted_ips": ["203.0.113.0/24"]
                    }
                }
            ],
            "origination": [
                {
                    "name": "wholesale_origination",
                    "description": "Wholesale partner origination interconnect",
                    "carrier_id": "wholesale_001",
                    "carrier_name": "Wholesale Partner LLC",
                    "direction": "origination",
                    "remote_ip": "198.51.100.20",
                    "remote_port": 5060,
                    "transport": "udp",
                    "lcr_group": "wholesale",
                    "quality_score": 88,
                    "capacity_limit": 500,
                    "codec_preference": ["g711u", "g729"],
                    "sip_i_support": true,
                    "isup_interworking": true,
                    "ss7_protocol": "ansi",
                    "circuit_group_support": true,
                    "cic_range": "1-24",
                    "authentication": {
                        "type": "digest_auth",
                        "username": "wholesale_user",
                        "password": "secure_password"
                    }
                }
            ]
        },
        "trunks": {
            "termination": [
                {
                    "id": 1001,
                    "name": "tier1_premium_trunk",
                    "description": "Premium trunk for Tier 1 carrier termination",
                    "tech_prefix": "1001",
                    "sip_profile_id": 1,
                    "carrier_interconnect": "tier1_termination",
                    "vendor_customer": "tier1_vendor",
                    "trunk_type": "termination",
                    "digit_manipulation": {
                        "strip_digits": 0,
                        "add_prefix": "",
                        "add_suffix": "",
                        "number_translation": []
                    },
                    "call_limits": {
                        "max_concurrent_calls": 500,
                        "calls_per_second": 10,
                        "max_call_duration": 7200
                    },
                    "allowed_codecs": ["g711u", "g711a", "g729"],
                    "stir_shaken": {
                        "enabled": true,
                        "attestation_level": "A",
                        "verify_incoming": true
                    },
                    "lcr_group": "tier1"
                },
                {
                    "id": 1002,
                    "name": "wholesale_premium_trunk",
                    "description": "Premium trunk for wholesale termination",
                    "tech_prefix": "1002",
                    "sip_profile_id": 1,
                    "carrier_interconnect": "tier1_termination",
                    "vendor_customer": "wholesale_customer",
                    "trunk_type": "termination",
                    "digit_manipulation": {
                        "strip_digits": 1,
                        "add_prefix": "1",
                        "add_suffix": "",
                        "number_translation": []
                    },
                    "call_limits": {
                        "max_concurrent_calls": 250,
                        "calls_per_second": 5,
                        "max_call_duration": 3600
                    },
                    "allowed_codecs": ["g711u", "g729"],
                    "stir_shaken": {
                        "enabled": false,
                        "attestation_level": "C",
                        "verify_incoming": false
                    },
                    "lcr_group": "wholesale"
                }
            ],
            "origination": [
                {
                    "id": 2001,
                    "name": "wholesale_origination_trunk",
                    "description": "Trunk for wholesale partner origination",
                    "tech_prefix": "2001",
                    "sip_profile_id": 1,
                    "carrier_interconnect": "wholesale_origination",
                    "vendor_customer": "wholesale_partner",
                    "trunk_type": "origination",
                    "digit_manipulation": {
                        "strip_digits": 0,
                        "add_prefix": "",
                        "add_suffix": "",
                        "number_translation": []
                    },
                    "call_limits": {
                        "max_concurrent_calls": 200,
                        "calls_per_second": 8,
                        "max_call_duration": 1800
                    },
                    "allowed_codecs": ["g711u", "g729"],
                    "stir_shaken": {
                        "enabled": true,
                        "attestation_level": "B",
                        "verify_incoming": true
                    },
                    "lcr_group": "wholesale"
                }
            ]
        },
        "vendors_customers": {
            "tier1_vendor": {
                "id": 3001,
                "name": "Tier1 Carrier Corp",
                "type": "vendor",
                "contact_info": {
                    "technical_contact": "noc@tier1carrier.com",
                    "billing_contact": "billing@tier1carrier.com",
                    "emergency_contact": "+1-555-0100"
                }
            },
            "wholesale_customer": {
                "id": 3002,
                "name": "Wholesale Customer Inc",
                "type": "customer",
                "contact_info": {
                    "technical_contact": "support@wholesale.com",
                    "billing_contact": "finance@wholesale.com",
                    "emergency_contact": "+1-555-0200"
                }
            },
            "wholesale_partner": {
                "id": 3003,
                "name": "Wholesale Partner LLC",
                "type": "partner",
                "contact_info": {
                    "technical_contact": "ops@partner.com",
                    "billing_contact": "accounts@partner.com",
                    "emergency_contact": "+1-555-0300"
                }
            }
        },
        "lcr_groups": {
            "tier1": {
                "name": "Tier 1 Carriers",
                "description": "Premium Tier 1 carrier group for high-quality termination",
                "priority": 1,
                "max_cost": 0.01,
                "quality_threshold": 90
            },
            "wholesale": {
                "name": "Wholesale Partners",
                "description": "Wholesale partner group for origination traffic",
                "priority": 2,
                "max_cost": 0.02,
                "quality_threshold": 85
            }
        },
        "monitoring": {
            "enabled": true,
            "endpoints": []
        },
        "stir_shaken": {
            "enabled": false,
            "certificate_path": "/etc/redfire-switch/certs/stir_shaken.crt",
            "private_key_path": "/etc/redfire-switch/certs/stir_shaken.key"
        },
        "routing": {
            "enabled": true,
            "database_url": "postgresql://redfire:password@localhost/routing",
            "max_routes": 10
        },
        "cdr": {
            "enabled": true,
            "database_url": "postgresql://redfire:password@localhost/cdr",
            "batch_size": 100,
            "flush_interval": 30
        },
        "security": {
            "enabled": true,
            "max_calls_per_minute": 1000,
            "blacklist": {
                "enabled": true,
                "auto_block_threshold": 100
            }
        },
        "billing": {
            "enabled": false,
            "currency": "USD",
            "precision": 6
        },
        "rtp_proxy": {
            "enabled": true,
            "bind_ip": bind_ip,
            "port_range": "10000-20000"
        },
        "codec": {
            "enabled": true,
            "preferred_codecs": ["PCMU", "PCMA", "G729"],
            "transcoding_enabled": true
        }
    }))
}

#[derive(OpenApi)]
#[openapi(
    paths(
        get_system_stats,
        get_active_calls,
        login,
        get_config_templates,
        generate_config
    ),
    components(
        schemas(
            ApiResponse<SystemStats>,
            ApiResponse<Vec<ActiveCall>>,
            ApiResponse<LoginResponse>,
            ApiResponse<Vec<ConfigTemplateInfo>>,
            ApiResponse<ConfigGenerationResponse>,
            SystemStats,
            ActiveCall,
            LoginRequest,
            LoginResponse,
            ConfigGenerationRequest,
            ConfigGenerationResponse,
            ConfigTemplateInfo,
            ConfigParam
        )
    ),
    tags(
        (name = "auth", description = "Authentication"),
        (name = "system", description = "System management"),
        (name = "config", description = "Configuration generation")
    ),
    info(
        title = "Redfire Switch API",
        version = "1.0.0",
        description = "Standalone REST API for Redfire Switch"
    )
)]
struct ApiDoc;

#[utoipa::path(
    get,
    path = "/api/v1/config/current",
    responses(
        (status = 200, description = "Current configuration", body = ApiResponse<HashMap<String, ConfigurationData>>)
    ),
    tag = "config"
)]
async fn get_current_config(
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<HashMap<String, ConfigurationData>>>, StatusCode> {
    let configs = state.configurations.lock().unwrap();
    let configs_clone = configs.clone();
    Ok(ResponseJson(ApiResponse::success(configs_clone)))
}

#[utoipa::path(
    post,
    path = "/api/v1/config/save",
    request_body = ConfigurationSaveRequest,
    responses(
        (status = 200, description = "Configuration saved successfully", body = ApiResponse<String>)
    ),
    tag = "config"
)]
async fn save_configuration(
    State(state): State<AppState>,
    Json(request): Json<ConfigurationSaveRequest>,
) -> Result<ResponseJson<ApiResponse<String>>, StatusCode> {
    let config_data = ConfigurationData {
        path: request.path.clone(),
        template: request.template,
        configuration: request.configuration,
        last_modified: Utc::now(),
    };

    {
        let mut configs = state.configurations.lock().unwrap();
        configs.insert(request.path.clone(), config_data);
    }

    Ok(ResponseJson(ApiResponse::success(format!(
        "Configuration saved for path: {}",
        request.path
    ))))
}

async fn create_router() -> Result<Router> {
    let config_file_path = std::env::var("REDFIRE_CONFIG_PATH")
        .unwrap_or_else(|_| "./config/redfire-switch.json".to_string());

    let state = AppState::new(config_file_path);

    // Load configuration and ensure IDs are assigned
    info!("Loading configuration and ensuring all entities have IDs...");
    state.config_manager.load_config().await?;

    Ok(Router::new()
        .route("/api/v1/system/stats", get(get_system_stats))
        .route("/api/v1/calls", get(get_active_calls))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/config/templates", get(get_config_templates))
        .route("/api/v1/config/generate", post(generate_config))
        .route("/api/v1/config/current", get(get_current_config))
        .route("/api/v1/config/save", post(save_configuration))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(CorsLayer::permissive())
        .with_state(state))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("🔥 RedFire Switch Standalone API Server v1.0.0");

    let app = create_router().await?;
    let addr: SocketAddr = format!("{}:{}", args.bind, args.port)
        .parse()
        .expect("Invalid bind address or port");

    info!("Server starting on http://{}", addr);
    info!("API Documentation: http://{}/swagger-ui", addr);
    info!("");
    info!("Available endpoints:");
    info!("  GET  /api/v1/system/stats      - System statistics");
    info!("  POST /api/v1/auth/login        - User authentication");
    info!("  GET  /api/v1/config/templates  - Available config templates");
    info!("  POST /api/v1/config/generate   - Generate configuration files");
    info!("");
    info!("Demo credentials:");
    info!("  Username: admin");
    info!("  Password: admin123");
    info!("");

    let listener = TcpListener::bind(addr).await?;
    serve(listener, app).await?;

    Ok(())
}
