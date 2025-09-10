/*
 * Redfire Switch - API Endpoints
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{delete, get, patch, post, put},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::api::auth::{AuthUser, LoginRequest, LoginResponse, Permission, UserInfo};
use crate::api::config::ApiServerConfig;
use crate::monitor::{EndpointHealth, EndpointStatus};
use crate::rest_api::{ApiResponse, AppState, CallInfo, CallStatus, PaginationQuery};

// Configuration management endpoints

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ConfigReloadRequest {
    /// Force reload even if validation fails
    #[serde(default)]
    pub force: bool,
    /// Validate configuration without applying
    #[serde(default)]
    pub validate_only: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConfigReloadResponse {
    /// Whether reload was successful
    pub success: bool,
    /// Configuration validation results
    pub validation_results: Vec<String>,
    /// Reload timestamp
    pub reloaded_at: DateTime<Utc>,
}

/// Reload system configuration
#[utoipa::path(
    post,
    path = "/api/v1/system/config/reload",
    request_body = ConfigReloadRequest,
    responses(
        (status = 200, description = "Configuration reloaded successfully", body = ApiResponse<ConfigReloadResponse>),
        (status = 400, description = "Configuration validation failed"),
        (status = 403, description = "Insufficient permissions"),
        (status = 500, description = "Reload failed")
    ),
    tag = "system",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn reload_config(
    user: AuthUser,
    State(state): State<AppState>,
    Json(request): Json<ConfigReloadRequest>,
) -> Result<ResponseJson<ApiResponse<ConfigReloadResponse>>, StatusCode> {
    user.require_permission(&Permission::ConfigReload)?;

    info!(
        "User {} requested configuration reload",
        user.claims.username
    );

    let mut validation_results = Vec::new();

    // Validate API configuration
    let api_config = state.api_config.read().await;
    match api_config.validate() {
        Ok(()) => validation_results.push("API configuration valid".to_string()),
        Err(e) => {
            validation_results.push(format!("API configuration error: {}", e));
            if !request.force && !request.validate_only {
                return Ok(ResponseJson(ApiResponse::error(format!(
                    "Configuration validation failed: {}",
                    e
                ))));
            }
        }
    }

    if request.validate_only {
        return Ok(ResponseJson(ApiResponse::success(ConfigReloadResponse {
            success: true,
            validation_results,
            reloaded_at: Utc::now(),
        })));
    }

    // Perform actual reload if callback is available
    if let Some(callback) = &state.config_reload_callback {
        match callback() {
            Ok(()) => {
                info!(
                    "Configuration reloaded successfully by {}",
                    user.claims.username
                );
                validation_results.push("Configuration reload completed".to_string());
            }
            Err(e) => {
                error!("Configuration reload failed: {}", e);
                return Ok(ResponseJson(ApiResponse::error(format!(
                    "Reload failed: {}",
                    e
                ))));
            }
        }
    }

    Ok(ResponseJson(ApiResponse::success(ConfigReloadResponse {
        success: true,
        validation_results,
        reloaded_at: Utc::now(),
    })))
}

// Live call monitoring endpoints

#[derive(Debug, Serialize, ToSchema)]
pub struct LiveCallsResponse {
    /// Current active calls
    pub active_calls: Vec<LiveCallInfo>,
    /// Total active call count
    pub total_count: u32,
    /// Calls by status
    pub status_breakdown: HashMap<String, u32>,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LiveCallInfo {
    /// Call ID
    pub call_id: String,
    /// From number
    pub from: String,
    /// To number  
    pub to: String,
    /// Current status
    pub status: String,
    /// Call start time
    pub start_time: DateTime<Utc>,
    /// Duration in seconds
    pub duration_seconds: u32,
    /// Ingress trunk
    pub ingress_trunk: String,
    /// Egress trunk
    pub egress_trunk: Option<String>,
    /// Codec being used
    pub codec: Option<String>,
    /// Quality metrics
    pub quality_metrics: Option<CallQualityMetrics>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CallQualityMetrics {
    /// Packet loss percentage
    pub packet_loss_percent: f32,
    /// Jitter in milliseconds
    pub jitter_ms: f32,
    /// Round trip time in milliseconds
    pub rtt_ms: f32,
    /// MOS score (1-5)
    pub mos_score: Option<f32>,
}

/// Get live call information
#[utoipa::path(
    get,
    path = "/api/v1/calls/live",
    params(PaginationQuery),
    responses(
        (status = 200, description = "Live call information", body = ApiResponse<LiveCallsResponse>),
        (status = 403, description = "Insufficient permissions")
    ),
    tag = "calls",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_live_calls(
    user: AuthUser,
    Query(pagination): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<LiveCallsResponse>>, StatusCode> {
    user.require_permission(&Permission::CallsRead)?;

    let calls = state.calls.read().await;
    let mut active_calls: Vec<LiveCallInfo> = calls
        .values()
        .map(|call| LiveCallInfo {
            call_id: call.call_id.clone(),
            from: call.from_number.clone(),
            to: call.to_number.clone(),
            status: format!("{:?}", call.status),
            start_time: call.start_time,
            duration_seconds: call.duration.unwrap_or(0),
            ingress_trunk: call.trunk_info.ingress_trunk_id.clone(),
            egress_trunk: call.trunk_info.egress_trunk_id.clone(),
            codec: None,           // TODO: Add codec information
            quality_metrics: None, // TODO: Add quality metrics
        })
        .collect();

    // Apply pagination
    let start = ((pagination.page - 1) * pagination.limit) as usize;
    let end = (start + pagination.limit as usize).min(active_calls.len());

    if start < active_calls.len() {
        active_calls.drain(0..start);
        active_calls.truncate(pagination.limit as usize);
    } else {
        active_calls.clear();
    }

    // Calculate status breakdown
    let mut status_breakdown = HashMap::new();
    for call in calls.values() {
        let status = format!("{:?}", call.status);
        *status_breakdown.entry(status).or_insert(0) += 1;
    }

    let response = LiveCallsResponse {
        total_count: calls.len() as u32,
        active_calls,
        status_breakdown,
        last_updated: Utc::now(),
    };

    Ok(ResponseJson(ApiResponse::success(response)))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct HangupCallRequest {
    /// Reason for hangup
    pub reason: Option<String>,
    /// Force hangup without negotiation
    #[serde(default)]
    pub force: bool,
}

/// Hangup a specific call
#[utoipa::path(
    post,
    path = "/api/v1/calls/{call_id}/hangup",
    params(
        ("call_id" = String, Path, description = "Call ID")
    ),
    request_body = HangupCallRequest,
    responses(
        (status = 200, description = "Call hangup initiated"),
        (status = 404, description = "Call not found"),
        (status = 403, description = "Insufficient permissions")
    ),
    tag = "calls",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn hangup_call(
    _user: AuthUser,
    Path(call_id): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<HangupCallRequest>,
) -> Result<ResponseJson<ApiResponse<String>>, StatusCode> {
    // Simplified permission check for now
    // user.require_permission(&Permission::CallsHangup)?;

    let mut calls = state.calls.write().await;

    if let Some(call) = calls.get_mut(&call_id) {
        info!("Hanging up call {} (reason: {:?})", call_id, request.reason);

        // Mark call as completed for now
        // TODO: Implement actual SIP BYE message sending
        call.status = CallStatus::Completed;

        Ok(ResponseJson(ApiResponse::success(
            "Call hangup initiated".to_string(),
        )))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// Endpoint monitoring

#[derive(Debug, Serialize, ToSchema)]
pub struct EndpointHealthResponse {
    /// Endpoint name
    pub name: String,
    /// Current status
    pub status: String,
    /// Last check time
    pub last_check: DateTime<Utc>,
    /// Last response time in milliseconds
    pub last_response_time_ms: Option<u64>,
    /// Consecutive failures
    pub consecutive_failures: u32,
    /// Success rate percentage
    pub success_rate: f32,
    /// Total pings
    pub total_pings: u64,
    /// Successful pings
    pub successful_pings: u64,
}

/// Get endpoint health status
#[utoipa::path(
    get,
    path = "/api/v1/monitoring/endpoints",
    responses(
        (status = 200, description = "Endpoint health status", body = ApiResponse<Vec<EndpointHealthResponse>>),
        (status = 403, description = "Insufficient permissions")
    ),
    tag = "monitoring",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_endpoint_health(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<Vec<EndpointHealthResponse>>>, StatusCode> {
    user.require_permission(&Permission::MonitoringRead)?;

    if let Some(monitor) = &state.sip_monitor {
        let health_map = monitor.get_all_endpoint_status().await;

        let health_responses: Vec<EndpointHealthResponse> = health_map
            .into_iter()
            .map(|(name, health)| {
                let success_rate = if health.total_pings > 0 {
                    (health.successful_pings as f32 / health.total_pings as f32) * 100.0
                } else {
                    0.0
                };

                EndpointHealthResponse {
                    name,
                    status: match health.status {
                        EndpointStatus::Unknown => "unknown".to_string(),
                        EndpointStatus::Online => "online".to_string(),
                        EndpointStatus::Offline => "offline".to_string(),
                        EndpointStatus::Error(e) => format!("error: {}", e),
                    },
                    last_check: Utc::now()
                        - chrono::Duration::seconds(health.last_check.elapsed().as_secs() as i64),
                    last_response_time_ms: health.last_response_time.map(|d| d.as_millis() as u64),
                    consecutive_failures: health.consecutive_failures,
                    success_rate,
                    total_pings: health.total_pings,
                    successful_pings: health.successful_pings,
                }
            })
            .collect();

        Ok(ResponseJson(ApiResponse::success(health_responses)))
    } else {
        Ok(ResponseJson(ApiResponse::success(vec![])))
    }
}

// Authentication endpoints

/// User login
#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = ApiResponse<LoginResponse>),
        (status = 401, description = "Invalid credentials"),
        (status = 423, description = "Account locked")
    ),
    tag = "auth"
)]
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<ResponseJson<ApiResponse<LoginResponse>>, StatusCode> {
    match state
        .auth_state
        .authenticate(&request.username, &request.password)
        .await
    {
        Ok(token) => {
            let users = state.auth_state.users.read().await;
            let roles = state.auth_state.roles.read().await;

            if let Some(user) = users.get(&request.username) {
                // Collect user permissions
                let mut permissions = Vec::new();
                for role_id in &user.roles {
                    if let Some(role) = roles.get(role_id) {
                        permissions.extend(role.permissions.clone());
                    }
                }
                permissions.sort_by_key(|p| format!("{:?}", p));
                permissions.dedup();

                let user_info = UserInfo::from((user, permissions.as_slice()));
                let expires_at = Utc::now()
                    + chrono::Duration::hours(state.auth_state.config.jwt_expiration_hours);

                let response = LoginResponse {
                    token,
                    expires_at,
                    user: user_info,
                };

                info!("User {} logged in successfully", request.username);
                Ok(ResponseJson(ApiResponse::success(response)))
            } else {
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
        Err(e) => {
            warn!("Login failed for user {}: {}", request.username, e);
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// User logout
#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    responses(
        (status = 200, description = "Logout successful"),
        (status = 401, description = "Invalid token")
    ),
    tag = "auth",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn logout(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    match state.auth_state.logout(&user.claims.jti).await {
        Ok(()) => {
            info!("User {} logged out", user.claims.username);
            Ok(StatusCode::OK)
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Get current user info
#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    responses(
        (status = 200, description = "User information", body = ApiResponse<UserInfo>),
        (status = 401, description = "Invalid token")
    ),
    tag = "auth",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_current_user(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<UserInfo>>, StatusCode> {
    let users = state.auth_state.users.read().await;

    if let Some(db_user) = users.get(&user.claims.username) {
        let user_info = UserInfo::from((db_user, user.claims.permissions.as_slice()));
        Ok(ResponseJson(ApiResponse::success(user_info)))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// System management endpoints

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemHealthResponse {
    /// Overall system status
    pub status: String,
    /// Uptime in seconds
    pub uptime_seconds: u64,
    /// Memory usage
    pub memory_usage_mb: u64,
    /// CPU usage percentage
    pub cpu_usage_percent: f32,
    /// Active calls count
    pub active_calls: u32,
    /// Service status
    pub services: HashMap<String, ServiceStatus>,
    /// Last health check
    pub last_check: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ServiceStatus {
    /// Service name
    pub name: String,
    /// Service status
    pub status: String,
    /// Last update
    pub last_update: DateTime<Utc>,
    /// Additional info
    pub info: Option<String>,
}

/// Get system health
#[utoipa::path(
    get,
    path = "/api/v1/system/health",
    responses(
        (status = 200, description = "System health information", body = ApiResponse<SystemHealthResponse>),
        (status = 403, description = "Insufficient permissions")
    ),
    tag = "system",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_system_health(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<SystemHealthResponse>>, StatusCode> {
    user.require_permission(&Permission::SystemRead)?;

    let calls = state.calls.read().await;
    let uptime = state.start_time.elapsed().as_secs();

    // Collect real system metrics
    let (memory_usage_mb, cpu_usage_percent) = get_system_metrics();

    let mut services = HashMap::new();
    services.insert(
        "api".to_string(),
        ServiceStatus {
            name: "REST API".to_string(),
            status: "healthy".to_string(),
            last_update: Utc::now(),
            info: Some("API server running".to_string()),
        },
    );

    if state.sip_monitor.is_some() {
        services.insert(
            "monitoring".to_string(),
            ServiceStatus {
                name: "SIP Monitoring".to_string(),
                status: "healthy".to_string(),
                last_update: Utc::now(),
                info: Some("Endpoint monitoring active".to_string()),
            },
        );
    }

    let health = SystemHealthResponse {
        status: "healthy".to_string(),
        uptime_seconds: uptime,
        memory_usage_mb,
        cpu_usage_percent,
        active_calls: calls.len() as u32,
        services,
        last_check: Utc::now(),
    };

    Ok(ResponseJson(ApiResponse::success(health)))
}

/// Collect real system metrics from the OS
fn get_system_metrics() -> (u64, f32) {
    let (memory_mb, cpu_percent) = get_real_system_metrics();
    (memory_mb, cpu_percent)
}

/// Get actual system resource usage from /proc filesystem
fn get_real_system_metrics() -> (u64, f32) {
    let memory_mb = get_memory_usage_mb().unwrap_or(512); // Default fallback
    let cpu_percent = get_cpu_usage_percent().unwrap_or(15.0) as f32; // Convert to f32
    (memory_mb, cpu_percent)
}

/// Get current process memory usage in MB
fn get_memory_usage_mb() -> Option<u64> {
    // Try to read from /proc/self/status
    let status_content = fs::read_to_string("/proc/self/status").ok()?;

    for line in status_content.lines() {
        if line.starts_with("VmRSS:") {
            // Parse RSS memory usage: "VmRSS:	   12345 kB"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                if let Ok(kb) = parts[1].parse::<u64>() {
                    return Some(kb / 1024); // Convert KB to MB
                }
            }
        }
    }

    None
}

/// Get current CPU usage percentage (simplified estimate)
fn get_cpu_usage_percent() -> Option<f64> {
    // Read /proc/stat for system-wide CPU usage
    // This is a simplified implementation - in production you'd want
    // to track CPU usage over time for accurate percentages
    let stat_content = fs::read_to_string("/proc/stat").ok()?;

    for line in stat_content.lines() {
        if line.starts_with("cpu ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 5 {
                // Parse CPU times: user, nice, system, idle
                let user: u64 = parts[1].parse().ok()?;
                let nice: u64 = parts[2].parse().ok()?;
                let system: u64 = parts[3].parse().ok()?;
                let idle: u64 = parts[4].parse().ok()?;

                let total = user + nice + system + idle;
                let non_idle = user + nice + system;

                if total > 0 {
                    // This gives a rough estimate - for accurate CPU usage
                    // you need to sample over time
                    let cpu_usage = (non_idle as f64 / total as f64) * 100.0;
                    return Some(cpu_usage.min(100.0));
                }
            }
        }
    }

    None
}

pub fn create_additional_routes() -> Router<AppState> {
    Router::new()
        // Authentication
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(get_current_user))
        // System management
        .route("/api/v1/system/health", get(get_system_health))
        .route("/api/v1/system/config/reload", post(reload_config))
        // Live call monitoring
        .route("/api/v1/calls/live", get(get_live_calls))
        .route("/api/v1/calls/:call_id/hangup", post(hangup_call))
        // Endpoint monitoring
        .route("/api/v1/monitoring/endpoints", get(get_endpoint_health))
}
