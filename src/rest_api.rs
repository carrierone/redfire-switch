/*
 * Redfire Switch - REST API
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
    routing::{get, post, put, delete},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    trace::TraceLayer,
};
use tracing::{info, warn, error};
use uuid::Uuid;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use anyhow::{Result, anyhow};

/// API response wrapper
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiResponse<T> {
    /// Success status
    pub success: bool,
    /// Response data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    /// Error message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Request timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now(),
        }
    }
    
    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Pagination parameters
#[derive(Debug, Deserialize, ToSchema)]
pub struct PaginationQuery {
    /// Page number (1-based)
    #[serde(default = "default_page")]
    pub page: u32,
    /// Items per page
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_page() -> u32 { 1 }
fn default_limit() -> u32 { 50 }

/// Call information for API
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CallInfo {
    /// Unique call identifier
    pub call_id: String,
    /// Originating number
    pub from_number: String,
    /// Destination number
    pub to_number: String,
    /// Call status
    pub status: CallStatus,
    /// Call direction
    pub direction: String,
    /// Start time
    pub start_time: chrono::DateTime<chrono::Utc>,
    /// Duration in seconds
    pub duration: Option<u32>,
    /// Trunk information
    pub trunk_info: TrunkInfo,
}

/// Call status
#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CallStatus {
    Ringing,
    Answered,
    Busy,
    Failed,
    Completed,
}

/// Trunk information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct TrunkInfo {
    /// Ingress trunk ID
    pub ingress_trunk_id: String,
    /// Egress trunk ID
    pub egress_trunk_id: Option<String>,
    /// Trunk type
    pub trunk_type: String,
}

/// DID/TFN information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DidInfo {
    /// DID number
    pub number: String,
    /// Customer ID
    pub customer_id: String,
    /// Routing destination
    pub destination_type: String,
    /// Destination value
    pub destination_value: String,
    /// Active status
    pub active: bool,
}

/// Customer information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CustomerInfo {
    /// Customer ID
    pub customer_id: String,
    /// Customer name
    pub name: String,
    /// Account status
    pub status: String,
    /// Balance
    pub balance: f64,
    /// Rate plan
    pub rate_plan: String,
}

/// SMS message information
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SmsInfo {
    /// Message ID
    pub message_id: String,
    /// From number
    pub from_number: String,
    /// To number
    pub to_number: String,
    /// Message content
    pub content: String,
    /// Message status
    pub status: String,
    /// Created timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Customer ID
    pub customer_id: Option<String>,
}

/// System statistics
#[derive(Debug, Serialize, ToSchema)]
pub struct SystemStats {
    /// Current active calls
    pub active_calls: u32,
    /// Total calls processed
    pub total_calls: u64,
    /// SMS messages processed
    pub sms_messages: u64,
    /// System uptime
    pub uptime_seconds: u64,
    /// Memory usage
    pub memory_usage: MemoryUsage,
    /// Trunk statistics
    pub trunk_stats: Vec<TrunkStats>,
}

/// Memory usage information
#[derive(Debug, Serialize, ToSchema)]
pub struct MemoryUsage {
    /// Used memory in bytes
    pub used_bytes: u64,
    /// Total memory in bytes
    pub total_bytes: u64,
    /// Memory usage percentage
    pub usage_percent: f32,
}

/// Trunk statistics
#[derive(Debug, Serialize, ToSchema)]
pub struct TrunkStats {
    /// Trunk ID
    pub trunk_id: String,
    /// Trunk name
    pub name: String,
    /// Active calls
    pub active_calls: u32,
    /// Maximum calls
    pub max_calls: u32,
    /// Success rate
    pub success_rate: f32,
}

/// Create DID request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateDidRequest {
    /// DID number
    pub number: String,
    /// Customer ID
    pub customer_id: String,
    /// Destination type
    pub destination_type: String,
    /// Destination value
    pub destination_value: String,
}

/// Update DID request
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateDidRequest {
    /// Destination type
    pub destination_type: Option<String>,
    /// Destination value
    pub destination_value: Option<String>,
    /// Active status
    pub active: Option<bool>,
}

/// Send SMS request
#[derive(Debug, Deserialize, ToSchema)]
pub struct SendSmsRequest {
    /// From number
    pub from: String,
    /// To number
    pub to: String,
    /// Message content
    pub content: String,
    /// Customer ID
    pub customer_id: Option<String>,
}

/// Application state
#[derive(Clone)]
pub struct AppState {
    /// Active calls
    pub calls: Arc<RwLock<HashMap<String, CallInfo>>>,
    /// DID assignments
    pub dids: Arc<RwLock<HashMap<String, DidInfo>>>,
    /// Customers
    pub customers: Arc<RwLock<HashMap<String, CustomerInfo>>>,
    /// SMS messages
    pub sms_messages: Arc<RwLock<HashMap<String, SmsInfo>>>,
    /// System start time
    pub start_time: std::time::Instant,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            calls: Arc::new(RwLock::new(HashMap::new())),
            dids: Arc::new(RwLock::new(HashMap::new())),
            customers: Arc::new(RwLock::new(HashMap::new())),
            sms_messages: Arc::new(RwLock::new(HashMap::new())),
            start_time: std::time::Instant::now(),
        }
    }
}

/// OpenAPI documentation
#[derive(OpenApi)]
#[openapi(
    paths(
        get_system_stats,
        list_active_calls,
        get_call_info,
        list_dids,
        create_did,
        update_did,
        delete_did,
        list_customers,
        get_customer,
        list_sms_messages,
        send_sms,
        get_sms_info
    ),
    components(
        schemas(
            ApiResponse<SystemStats>,
            ApiResponse<Vec<CallInfo>>,
            ApiResponse<CallInfo>,
            ApiResponse<Vec<DidInfo>>,
            ApiResponse<DidInfo>,
            ApiResponse<Vec<CustomerInfo>>,
            ApiResponse<CustomerInfo>,
            ApiResponse<Vec<SmsInfo>>,
            ApiResponse<SmsInfo>,
            SystemStats,
            CallInfo,
            CallStatus,
            TrunkInfo,
            DidInfo,
            CustomerInfo,
            SmsInfo,
            MemoryUsage,
            TrunkStats,
            PaginationQuery,
            CreateDidRequest,
            UpdateDidRequest,
            SendSmsRequest
        )
    ),
    tags(
        (name = "system", description = "System management and statistics"),
        (name = "calls", description = "Call management and monitoring"),
        (name = "dids", description = "DID/TFN management"),
        (name = "customers", description = "Customer management"),
        (name = "sms", description = "SMS messaging")
    ),
    info(
        title = "Redfire Switch API",
        version = "1.0.0",
        description = "REST API for Redfire Switch management and monitoring",
        contact(
            name = "Carrier One Inc",
            url = "https://www.carrierone.com",
            email = "support@carrierone.com"
        )
    )
)]
pub struct ApiDoc;

/// Create REST API router
pub fn create_api_router() -> Router {
    let state = AppState::new();
    
    Router::new()
        // System endpoints
        .route("/api/v1/system/stats", get(get_system_stats))
        
        // Call endpoints
        .route("/api/v1/calls", get(list_active_calls))
        .route("/api/v1/calls/:call_id", get(get_call_info))
        
        // DID endpoints
        .route("/api/v1/dids", get(list_dids).post(create_did))
        .route("/api/v1/dids/:number", get(get_did_info).put(update_did).delete(delete_did))
        
        // Customer endpoints
        .route("/api/v1/customers", get(list_customers))
        .route("/api/v1/customers/:customer_id", get(get_customer))
        
        // SMS endpoints
        .route("/api/v1/sms/messages", get(list_sms_messages).post(send_sms))
        .route("/api/v1/sms/messages/:message_id", get(get_sms_info))
        
        // Swagger UI
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
        )
        .with_state(state)
}

/// Get system statistics
#[utoipa::path(
    get,
    path = "/api/v1/system/stats",
    responses(
        (status = 200, description = "System statistics", body = ApiResponse<SystemStats>)
    ),
    tag = "system"
)]
pub async fn get_system_stats(
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<SystemStats>>, StatusCode> {
    let calls = state.calls.read().await;
    let uptime = state.start_time.elapsed().as_secs();
    
    let stats = SystemStats {
        active_calls: calls.len() as u32,
        total_calls: 1000, // Placeholder
        sms_messages: 500, // Placeholder
        uptime_seconds: uptime,
        memory_usage: MemoryUsage {
            used_bytes: 1024 * 1024 * 256, // 256MB placeholder
            total_bytes: 1024 * 1024 * 1024, // 1GB placeholder
            usage_percent: 25.0,
        },
        trunk_stats: vec![
            TrunkStats {
                trunk_id: "trunk1".to_string(),
                name: "Primary Trunk".to_string(),
                active_calls: 5,
                max_calls: 100,
                success_rate: 98.5,
            }
        ],
    };
    
    Ok(ResponseJson(ApiResponse::success(stats)))
}

/// List active calls
#[utoipa::path(
    get,
    path = "/api/v1/calls",
    params(PaginationQuery),
    responses(
        (status = 200, description = "List of active calls", body = ApiResponse<Vec<CallInfo>>)
    ),
    tag = "calls"
)]
pub async fn list_active_calls(
    Query(pagination): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<Vec<CallInfo>>>, StatusCode> {
    let calls = state.calls.read().await;
    let mut call_list: Vec<CallInfo> = calls.values().cloned().collect();
    
    // Apply pagination
    let start = ((pagination.page - 1) * pagination.limit) as usize;
    let end = (start + pagination.limit as usize).min(call_list.len());
    
    if start < call_list.len() {
        call_list.drain(0..start);
        call_list.truncate(pagination.limit as usize);
    } else {
        call_list.clear();
    }
    
    Ok(ResponseJson(ApiResponse::success(call_list)))
}

/// Get call information
#[utoipa::path(
    get,
    path = "/api/v1/calls/{call_id}",
    params(
        ("call_id" = String, Path, description = "Call ID")
    ),
    responses(
        (status = 200, description = "Call information", body = ApiResponse<CallInfo>),
        (status = 404, description = "Call not found")
    ),
    tag = "calls"
)]
pub async fn get_call_info(
    Path(call_id): Path<String>,
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<CallInfo>>, StatusCode> {
    let calls = state.calls.read().await;
    
    if let Some(call) = calls.get(&call_id) {
        Ok(ResponseJson(ApiResponse::success(call.clone())))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// List DIDs
#[utoipa::path(
    get,
    path = "/api/v1/dids",
    params(PaginationQuery),
    responses(
        (status = 200, description = "List of DIDs", body = ApiResponse<Vec<DidInfo>>)
    ),
    tag = "dids"
)]
pub async fn list_dids(
    Query(pagination): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<Vec<DidInfo>>>, StatusCode> {
    let dids = state.dids.read().await;
    let mut did_list: Vec<DidInfo> = dids.values().cloned().collect();
    
    // Apply pagination
    let start = ((pagination.page - 1) * pagination.limit) as usize;
    if start < did_list.len() {
        did_list.drain(0..start);
        did_list.truncate(pagination.limit as usize);
    } else {
        did_list.clear();
    }
    
    Ok(ResponseJson(ApiResponse::success(did_list)))
}

/// Create DID
#[utoipa::path(
    post,
    path = "/api/v1/dids",
    request_body = CreateDidRequest,
    responses(
        (status = 201, description = "DID created", body = ApiResponse<DidInfo>),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "DID already exists")
    ),
    tag = "dids"
)]
pub async fn create_did(
    State(state): State<AppState>,
    Json(request): Json<CreateDidRequest>,
) -> Result<ResponseJson<ApiResponse<DidInfo>>, StatusCode> {
    let mut dids = state.dids.write().await;
    
    if dids.contains_key(&request.number) {
        return Err(StatusCode::CONFLICT);
    }
    
    let did = DidInfo {
        number: request.number.clone(),
        customer_id: request.customer_id,
        destination_type: request.destination_type,
        destination_value: request.destination_value,
        active: true,
    };
    
    dids.insert(request.number, did.clone());
    info!("Created DID: {}", did.number);
    
    Ok(ResponseJson(ApiResponse::success(did)))
}

/// Get DID information
pub async fn get_did_info(
    Path(number): Path<String>,
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<DidInfo>>, StatusCode> {
    let dids = state.dids.read().await;
    
    if let Some(did) = dids.get(&number) {
        Ok(ResponseJson(ApiResponse::success(did.clone())))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Update DID
#[utoipa::path(
    put,
    path = "/api/v1/dids/{number}",
    params(
        ("number" = String, Path, description = "DID number")
    ),
    request_body = UpdateDidRequest,
    responses(
        (status = 200, description = "DID updated", body = ApiResponse<DidInfo>),
        (status = 404, description = "DID not found")
    ),
    tag = "dids"
)]
pub async fn update_did(
    Path(number): Path<String>,
    State(state): State<AppState>,
    Json(request): Json<UpdateDidRequest>,
) -> Result<ResponseJson<ApiResponse<DidInfo>>, StatusCode> {
    let mut dids = state.dids.write().await;
    
    if let Some(did) = dids.get_mut(&number) {
        if let Some(dest_type) = request.destination_type {
            did.destination_type = dest_type;
        }
        if let Some(dest_value) = request.destination_value {
            did.destination_value = dest_value;
        }
        if let Some(active) = request.active {
            did.active = active;
        }
        
        info!("Updated DID: {}", did.number);
        Ok(ResponseJson(ApiResponse::success(did.clone())))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Delete DID
#[utoipa::path(
    delete,
    path = "/api/v1/dids/{number}",
    params(
        ("number" = String, Path, description = "DID number")
    ),
    responses(
        (status = 204, description = "DID deleted"),
        (status = 404, description = "DID not found")
    ),
    tag = "dids"
)]
pub async fn delete_did(
    Path(number): Path<String>,
    State(state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    let mut dids = state.dids.write().await;
    
    if dids.remove(&number).is_some() {
        info!("Deleted DID: {}", number);
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// List customers
#[utoipa::path(
    get,
    path = "/api/v1/customers",
    params(PaginationQuery),
    responses(
        (status = 200, description = "List of customers", body = ApiResponse<Vec<CustomerInfo>>)
    ),
    tag = "customers"
)]
pub async fn list_customers(
    Query(pagination): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<Vec<CustomerInfo>>>, StatusCode> {
    let customers = state.customers.read().await;
    let mut customer_list: Vec<CustomerInfo> = customers.values().cloned().collect();
    
    // Apply pagination
    let start = ((pagination.page - 1) * pagination.limit) as usize;
    if start < customer_list.len() {
        customer_list.drain(0..start);
        customer_list.truncate(pagination.limit as usize);
    } else {
        customer_list.clear();
    }
    
    Ok(ResponseJson(ApiResponse::success(customer_list)))
}

/// Get customer information
#[utoipa::path(
    get,
    path = "/api/v1/customers/{customer_id}",
    params(
        ("customer_id" = String, Path, description = "Customer ID")
    ),
    responses(
        (status = 200, description = "Customer information", body = ApiResponse<CustomerInfo>),
        (status = 404, description = "Customer not found")
    ),
    tag = "customers"
)]
pub async fn get_customer(
    Path(customer_id): Path<String>,
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<CustomerInfo>>, StatusCode> {
    let customers = state.customers.read().await;
    
    if let Some(customer) = customers.get(&customer_id) {
        Ok(ResponseJson(ApiResponse::success(customer.clone())))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// List SMS messages
#[utoipa::path(
    get,
    path = "/api/v1/sms/messages",
    params(PaginationQuery),
    responses(
        (status = 200, description = "List of SMS messages", body = ApiResponse<Vec<SmsInfo>>)
    ),
    tag = "sms"
)]
pub async fn list_sms_messages(
    Query(pagination): Query<PaginationQuery>,
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<Vec<SmsInfo>>>, StatusCode> {
    let messages = state.sms_messages.read().await;
    let mut message_list: Vec<SmsInfo> = messages.values().cloned().collect();
    
    // Apply pagination
    let start = ((pagination.page - 1) * pagination.limit) as usize;
    if start < message_list.len() {
        message_list.drain(0..start);
        message_list.truncate(pagination.limit as usize);
    } else {
        message_list.clear();
    }
    
    Ok(ResponseJson(ApiResponse::success(message_list)))
}

/// Send SMS message
#[utoipa::path(
    post,
    path = "/api/v1/sms/messages",
    request_body = SendSmsRequest,
    responses(
        (status = 201, description = "SMS sent", body = ApiResponse<SmsInfo>),
        (status = 400, description = "Invalid request")
    ),
    tag = "sms"
)]
pub async fn send_sms(
    State(state): State<AppState>,
    Json(request): Json<SendSmsRequest>,
) -> Result<ResponseJson<ApiResponse<SmsInfo>>, StatusCode> {
    let message_id = Uuid::new_v4().to_string();
    
    let sms = SmsInfo {
        message_id: message_id.clone(),
        from_number: request.from,
        to_number: request.to,
        content: request.content,
        status: "queued".to_string(),
        created_at: chrono::Utc::now(),
        customer_id: request.customer_id,
    };
    
    let mut messages = state.sms_messages.write().await;
    messages.insert(message_id, sms.clone());
    
    info!("SMS queued: {} -> {}", sms.from_number, sms.to_number);
    
    Ok(ResponseJson(ApiResponse::success(sms)))
}

/// Get SMS information
#[utoipa::path(
    get,
    path = "/api/v1/sms/messages/{message_id}",
    params(
        ("message_id" = String, Path, description = "Message ID")
    ),
    responses(
        (status = 200, description = "SMS information", body = ApiResponse<SmsInfo>),
        (status = 404, description = "Message not found")
    ),
    tag = "sms"
)]
pub async fn get_sms_info(
    Path(message_id): Path<String>,
    State(state): State<AppState>,
) -> Result<ResponseJson<ApiResponse<SmsInfo>>, StatusCode> {
    let messages = state.sms_messages.read().await;
    
    if let Some(message) = messages.get(&message_id) {
        Ok(ResponseJson(ApiResponse::success(message.clone())))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Start REST API server
pub async fn start_api_server(port: u16) -> Result<()> {
    let app = create_api_router();
    // WARNING: In clustering environments, REST API should bind to local IP only
    // Use cluster_bind.management_ip instead of 0.0.0.0 to avoid BGP anycast conflicts
    let addr = format!("0.0.0.0:{}", port);
    
    info!("Starting REST API server on {}", addr);
    info!("WARNING: REST API binding to 0.0.0.0 - configure cluster_bind.management_ip for clustering");
    info!("Swagger UI available at: http://localhost:{}/swagger-ui", port);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}