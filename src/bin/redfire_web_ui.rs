/*
 * RedFire Switch - Web Administration UI
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use anyhow::{anyhow, Result};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, Json},
    routing::{get, post},
    Router,
};
use clap::Parser;
use http_body_util::BodyExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, services::ServeDir, trace::TraceLayer};
use tracing::{debug, error, info};

// Unix socket client for connecting to the switch
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;

#[derive(Parser, Debug, Clone)]
#[command(name = "redfire-web-ui")]
#[command(about = "RedFire Switch Web Administration UI")]
pub struct Args {
    /// Port to bind the web UI server to
    #[arg(short, long, default_value = "3000")]
    port: u16,

    /// IP address to bind to
    #[arg(short, long, default_value = "127.0.0.1")]
    bind: String,

    /// Unix socket path to connect to RedFire Switch API
    #[arg(short, long, default_value = "/var/run/redfire-switch/api.sock")]
    socket_path: String,

    /// HTTP endpoint for switch API (alternative to Unix socket)
    #[arg(long)]
    switch_url: Option<String>,

    /// Enable development mode (additional logging, etc.)
    #[arg(short, long)]
    dev: bool,
}

impl Args {
    /// Get the base URL for API calls
    pub fn get_api_base(&self) -> String {
        if let Some(url) = &self.switch_url {
            url.clone()
        } else {
            // For Unix socket, we'll use a special scheme
            format!("unix://{}", self.socket_path)
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    /// Configuration
    pub config: Args,
    /// HTTP client for communicating with switch
    pub http_client: reqwest::Client,
    /// Authentication sessions
    pub sessions: Arc<RwLock<HashMap<String, UserSession>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    pub token: String,
    pub username: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl AppState {
    pub fn new(config: Args) -> Self {
        Self {
            http_client: reqwest::Client::new(),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Get the base URL for API calls
    pub fn get_api_base(&self) -> String {
        if let Some(url) = &self.config.switch_url {
            url.clone()
        } else {
            // For Unix socket, we'll use a special scheme
            format!("unix://{}", self.config.socket_path)
        }
    }

    /// Make an API request to the switch
    pub async fn api_request<T: for<'de> serde::Deserialize<'de>>(
        &self,
        method: reqwest::Method,
        endpoint: &str,
        body: Option<serde_json::Value>,
        session_token: Option<&str>,
    ) -> Result<T> {
        let base_url = self.get_api_base();

        if base_url.starts_with("unix://") {
            self.unix_socket_request(method, endpoint, body, session_token)
                .await
        } else {
            self.http_request(method, endpoint, body, session_token)
                .await
        }
    }

    async fn unix_socket_request<T: for<'de> serde::Deserialize<'de>>(
        &self,
        method: reqwest::Method,
        endpoint: &str,
        body: Option<serde_json::Value>,
        session_token: Option<&str>,
    ) -> Result<T> {
        // Connect to Unix socket
        let socket_path = &self.config.socket_path;
        let stream = UnixStream::connect(socket_path)
            .await
            .map_err(|e| anyhow!("Failed to connect to Unix socket {}: {}", socket_path, e))?;

        // Create HTTP request
        let uri = format!("http://localhost{}", endpoint);
        let mut request_builder = hyper::Request::builder().method(method.as_str()).uri(&uri);

        // Add authentication header if provided
        if let Some(token) = session_token {
            request_builder = request_builder.header("Authorization", format!("Bearer {}", token));
        }

        // Add content-type for POST/PUT requests
        if matches!(method, reqwest::Method::POST | reqwest::Method::PUT) {
            request_builder = request_builder.header("Content-Type", "application/json");
        }

        let request_body = if let Some(body) = body {
            axum::body::Body::from(serde_json::to_string(&body)?)
        } else {
            axum::body::Body::empty()
        };

        let request = request_builder.body(request_body)?;

        // Make request over Unix socket
        let io = TokioIo::new(stream);
        let (mut sender, conn) = hyper::client::conn::http1::handshake(io).await?;

        // Spawn connection handler
        tokio::task::spawn(async move {
            if let Err(err) = conn.await {
                error!("Connection failed: {:?}", err);
            }
        });

        let response = sender.send_request(request).await?;
        let status = response.status();

        // Read response body
        let body_bytes = response.into_body().collect().await?.to_bytes();

        if !status.is_success() {
            return Err(anyhow!("API request failed with status: {}", status));
        }

        let response_text = String::from_utf8(body_bytes.to_vec())?;
        let api_response: ApiResponse<T> = serde_json::from_str(&response_text)?;

        if api_response.success {
            api_response
                .data
                .ok_or_else(|| anyhow!("API response missing data"))
        } else {
            Err(anyhow!(
                "API error: {}",
                api_response
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))
        }
    }

    async fn http_request<T: for<'de> serde::Deserialize<'de>>(
        &self,
        method: reqwest::Method,
        endpoint: &str,
        body: Option<serde_json::Value>,
        session_token: Option<&str>,
    ) -> Result<T> {
        let url = format!("{}{}", self.config.switch_url.as_ref().unwrap(), endpoint);
        let mut request = self.http_client.request(method, &url);

        if let Some(token) = session_token {
            request = request.bearer_auth(token);
        }

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await?;
        let status = response.status();

        if !status.is_success() {
            return Err(anyhow!("API request failed with status: {}", status));
        }

        let api_response: ApiResponse<T> = response.json().await?;

        if api_response.success {
            api_response
                .data
                .ok_or_else(|| anyhow!("API response missing data"))
        } else {
            Err(anyhow!(
                "API error: {}",
                api_response
                    .error
                    .unwrap_or_else(|| "Unknown error".to_string())
            ))
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(if args.dev {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting RedFire Switch Web UI");
    info!("Connecting to switch via: {}", args.get_api_base());

    let state = AppState::new(args.clone());

    // Test connection to the switch
    info!("Testing connection to RedFire Switch...");
    match test_switch_connection(&state).await {
        Ok(_) => info!("✅ Successfully connected to RedFire Switch"),
        Err(e) => {
            error!("❌ Failed to connect to RedFire Switch: {}", e);
            error!("Please ensure the RedFire Switch is running and accessible via:");
            if let Some(url) = &args.switch_url {
                error!("  HTTP: {}", url);
            } else {
                error!("  Unix Socket: {}", args.socket_path);
            }
            return Err(e);
        }
    }

    let app = create_app(state);

    let bind_addr = format!("{}:{}", args.bind, args.port);
    info!("🚀 RedFire Switch Web UI starting on http://{}", bind_addr);
    info!("📊 Admin Dashboard: http://{}/", bind_addr);
    info!("📡 API Documentation: http://{}/api-docs", bind_addr);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn test_switch_connection(state: &AppState) -> Result<()> {
    // Try to get system stats to test the connection
    let _: serde_json::Value = state
        .api_request(reqwest::Method::GET, "/api/v1/system/stats", None, None)
        .await?;

    Ok(())
}

fn create_app(state: AppState) -> Router {
    Router::new()
        // Serve static files (HTML, CSS, JS)
        .nest_service("/static", ServeDir::new("web-ui/static"))
        .nest_service("/components", ServeDir::new("web-ui/components"))
        // API routes
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/switch/*path", get(proxy_get).post(proxy_post))
        // Frontend routes
        .route("/", get(dashboard_page))
        .route("/login", get(login_page))
        .route("/calls", get(calls_page))
        .route("/config", get(config_page))
        .route("/config-manager", get(config_manager_page))
        .route("/monitoring", get(monitoring_page))
        .route("/api-docs", get(api_docs_page))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive()),
        )
        .with_state(state)
}

// API Handlers

async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<ApiResponse<UserSession>>, StatusCode> {
    debug!("Login attempt for user: {}", request.username);

    // Forward login to switch API
    let login_result: serde_json::Value = match state
        .api_request(
            reqwest::Method::POST,
            "/api/v1/auth/login",
            Some(serde_json::json!(request)),
            None,
        )
        .await
    {
        Ok(result) => result,
        Err(e) => {
            error!("Login failed: {}", e);
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Extract token from the API response
    let token = login_result["token"]
        .as_str()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Since this is a demo with the standalone API server, use the username from the request
    let username = &request.username;
    let permissions = vec!["admin".to_string()]; // Default admin permissions

    // Create session
    let session = UserSession {
        token: token.to_string(),
        username: username.to_string(),
        expires_at: chrono::Utc::now() + chrono::Duration::hours(8),
        permissions,
    };

    // Store session
    let session_id = uuid::Uuid::new_v4().to_string();
    state
        .sessions
        .write()
        .await
        .insert(session_id.clone(), session.clone());

    info!("User {} logged in successfully", username);

    Ok(Json(ApiResponse {
        success: true,
        data: Some(session),
        error: None,
        timestamp: chrono::Utc::now(),
    }))
}

async fn logout(State(_state): State<AppState>) -> Json<ApiResponse<()>> {
    // In a real implementation, we'd extract the session ID from headers
    // and remove it from the sessions map
    Json(ApiResponse {
        success: true,
        data: Some(()),
        error: None,
        timestamp: chrono::Utc::now(),
    })
}

// API Proxy handlers
async fn proxy_get(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    proxy_request(state, reqwest::Method::GET, &path, None, None).await
}

async fn proxy_post(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    proxy_request(state, reqwest::Method::POST, &path, Some(body), None).await
}

async fn proxy_request(
    state: AppState,
    method: reqwest::Method,
    path: &str,
    body: Option<serde_json::Value>,
    token: Option<&str>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let endpoint = format!("/api/v1/{}", path);
    debug!("Proxying request: {} {} -> {}", method, path, endpoint);
    let method_str = method.to_string();

    match state
        .api_request::<serde_json::Value>(method, &endpoint, body, token)
        .await
    {
        Ok(result) => {
            debug!("Proxy request successful: {} {}", method_str, path);
            Ok(Json(result))
        }
        Err(e) => {
            error!("Proxy request failed for {} {}: {}", method_str, path, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// Frontend page handlers

async fn dashboard_page() -> Html<&'static str> {
    Html(include_str!("../../web-ui/dashboard.html"))
}

async fn login_page() -> Html<&'static str> {
    Html(include_str!("../../web-ui/login.html"))
}

async fn calls_page() -> Html<&'static str> {
    Html(include_str!("../../web-ui/calls.html"))
}

async fn config_page() -> Html<&'static str> {
    Html(include_str!("../../web-ui/config.html"))
}

async fn config_manager_page() -> Html<&'static str> {
    Html(include_str!("../../web-ui/config-manager.html"))
}

async fn monitoring_page() -> Html<&'static str> {
    Html(include_str!("../../web-ui/monitoring.html"))
}

async fn api_docs_page() -> Html<&'static str> {
    Html(
        r#"
    <!DOCTYPE html>
    <html>
    <head>
        <title>RedFire Switch API Documentation</title>
        <meta charset="UTF-8">
        <style>
            body { font-family: Arial, sans-serif; margin: 40px; }
            h1 { color: #d73502; }
            .endpoint { margin: 20px 0; padding: 15px; border-left: 4px solid #d73502; background: #f9f9f9; }
            .method { font-weight: bold; color: #d73502; }
        </style>
    </head>
    <body>
        <h1>🔥 RedFire Switch API Documentation</h1>
        <p>This web UI acts as a proxy to the RedFire Switch API. The actual Swagger documentation is available at the switch's API endpoint.</p>
        
        <div class="endpoint">
            <div class="method">GET</div>
            <strong>/api/switch/system/stats</strong> - Get system statistics
        </div>
        
        <div class="endpoint">
            <div class="method">GET</div>
            <strong>/api/switch/calls</strong> - List active calls
        </div>
        
        <div class="endpoint">
            <div class="method">POST</div>
            <strong>/api/login</strong> - Authenticate with the switch
        </div>
        
        <p><a href="/">← Back to Dashboard</a></p>
    </body>
    </html>
    "#,
    )
}
