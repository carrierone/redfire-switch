/*
 * Redfire Switch - Simplified API Server (Compiles Successfully)
 * Copyright (C) 2025 Carrier One Inc and contributors
 */

use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

// Re-use types from main API modules to avoid duplication
use crate::api::auth::{LoginRequest, LoginResponse};
use crate::rest_api::{ApiResponse, SystemStats};

#[derive(Clone)]
pub struct SimpleAppState {
    pub start_time: std::time::Instant,
}

impl SimpleAppState {
    pub fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
        }
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
pub async fn get_system_stats(
    State(state): State<SimpleAppState>,
) -> Result<ResponseJson<ApiResponse<SystemStats>>, StatusCode> {
    let uptime = state.start_time.elapsed().as_secs();

    // Get real memory usage
    let memory_mb = get_memory_usage_mb().unwrap_or(64);
    let memory_bytes = memory_mb * 1024 * 1024;

    let stats = SystemStats {
        active_calls: 0, // Still using 0 as we don't have call tracking in simplified mode
        total_calls: 0,
        sms_messages: 0,
        uptime_seconds: uptime,
        memory_usage: crate::rest_api::MemoryUsage {
            used_bytes: memory_bytes,
            total_bytes: memory_bytes * 4, // Estimate total as 4x current usage
            usage_percent: ((memory_bytes as f64 / (memory_bytes * 4) as f64) * 100.0) as f32,
        },
        trunk_stats: vec![],
    };

    Ok(ResponseJson(ApiResponse::success(stats)))
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
pub async fn login(
    Json(request): Json<LoginRequest>,
) -> Result<ResponseJson<ApiResponse<LoginResponse>>, StatusCode> {
    // Simple demo login - INSECURE, for demo only
    if request.username == "admin" && request.password == "admin123" {
        let user_info = crate::api::auth::UserInfo {
            id: "admin".to_string(),
            username: "admin".to_string(),
            email: "admin@redfire-switch.local".to_string(),
            roles: vec!["admin".to_string()],
            permissions: vec!["SystemAdmin".to_string()],
            last_login: None,
        };

        let response = LoginResponse {
            token: "demo_token_1234567890".to_string(),
            expires_at: Utc::now() + chrono::Duration::hours(1),
            user: user_info,
        };
        Ok(ResponseJson(ApiResponse::success(response)))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        get_system_stats,
        login
    ),
    components(
        schemas(
            crate::rest_api::ApiResponse<crate::rest_api::SystemStats>,
            crate::rest_api::ApiResponse<crate::api::auth::LoginResponse>,
            crate::rest_api::SystemStats,
            crate::api::auth::LoginRequest,
            crate::api::auth::LoginResponse,
            crate::api::auth::UserInfo,
            crate::rest_api::MemoryUsage,
            crate::rest_api::TrunkStats
        )
    ),
    tags(
        (name = "auth", description = "Authentication"),
        (name = "system", description = "System management")
    ),
    info(
        title = "Redfire Switch API (Simplified)",
        version = "1.0.0",
        description = "Simplified REST API for Redfire Switch"
    )
)]
pub struct ApiDoc;

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

pub fn create_simple_api_router() -> Router {
    let state = SimpleAppState::new();

    Router::new()
        .route("/api/v1/system/stats", get(get_system_stats))
        .route("/api/v1/auth/login", post(login))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(state)
}
