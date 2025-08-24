use anyhow::Result;
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::lcr::routing::CallSimulation;
use crate::lcr::types::*;
use crate::lcr::LcrEngine;

#[derive(Clone)]
pub struct ApiState {
    lcr_engine: Arc<LcrEngine>,
}

impl ApiState {
    pub fn new(lcr_engine: Arc<LcrEngine>) -> Self {
        Self { lcr_engine }
    }
}

pub fn create_router(state: ApiState) -> Router {
    Router::new()
        // Health check
        .route("/health", get(health_check))
        // Cache management
        .route("/cache/reload", post(reload_cache))
        // Call simulation
        .route("/simulate", post(simulate_call))
        .route("/simulate/:ani/:dnis", get(simulate_call_get))
        // Routing
        .route("/route", post(find_route))
        // Trunk management
        .route("/trunks/stats", get(get_trunk_stats))
        .route("/trunks/ingress", get(list_ingress_trunks))
        .route("/trunks/egress", get(list_egress_trunks))
        // Rate deck management
        .route("/rates/vendor/:deck_id/:code", get(get_vendor_rate))
        .route("/rates/client/:deck_id/:code", get(get_client_rate))
        .with_state(state)
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "lcr-engine"
    }))
}

async fn reload_cache(State(state): State<ApiState>) -> impl IntoResponse {
    match state.lcr_engine.reload_cache().await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "success",
                "message": "Cache reloaded successfully"
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": format!("Failed to reload cache: {}", e)
            })),
        ),
    }
}

#[derive(Deserialize)]
struct SimulateCallRequest {
    ani: String,
    dnis: String,
    ingress_trunk: Option<String>,
}

async fn simulate_call(
    State(state): State<ApiState>,
    Json(req): Json<SimulateCallRequest>,
) -> impl IntoResponse {
    let routing_engine = state.lcr_engine.get_routing_engine();

    match routing_engine
        .simulate_call(&req.ani, &req.dnis, req.ingress_trunk.as_deref())
        .await
    {
        Ok(simulation) => (StatusCode::OK, Json(simulation)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Simulation failed: {}", e)
            })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct SimulateCallQuery {
    ingress_trunk: Option<String>,
}

async fn simulate_call_get(
    State(state): State<ApiState>,
    Path((ani, dnis)): Path<(String, String)>,
    Query(query): Query<SimulateCallQuery>,
) -> impl IntoResponse {
    let routing_engine = state.lcr_engine.get_routing_engine();

    match routing_engine
        .simulate_call(&ani, &dnis, query.ingress_trunk.as_deref())
        .await
    {
        Ok(simulation) => (StatusCode::OK, Json(simulation)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Simulation failed: {}", e)
            })),
        )
            .into_response(),
    }
}

async fn find_route(
    State(state): State<ApiState>,
    Json(req): Json<RouteRequest>,
) -> impl IntoResponse {
    let routing_engine = state.lcr_engine.get_routing_engine();

    match routing_engine.find_routes(&req).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Route finding failed: {}", e)
            })),
        )
            .into_response(),
    }
}

async fn get_trunk_stats(State(state): State<ApiState>) -> impl IntoResponse {
    let trunk_manager = state.lcr_engine.get_trunk_manager();
    let stats = trunk_manager.get_all_stats().await;

    Json(stats)
}

async fn list_ingress_trunks(State(state): State<ApiState>) -> impl IntoResponse {
    let trunks = state.lcr_engine.cache.get_all_ingress_trunks();
    Json(trunks)
}

async fn list_egress_trunks(State(state): State<ApiState>) -> impl IntoResponse {
    let trunks = state.lcr_engine.cache.get_all_egress_trunks();
    Json(trunks)
}

async fn get_vendor_rate(
    State(state): State<ApiState>,
    Path((deck_id, code)): Path<(i32, String)>,
) -> impl IntoResponse {
    if let Some(rate) = state.lcr_engine.cache.get_vendor_rate(deck_id, &code) {
        (StatusCode::OK, Json(rate)).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Rate not found"
            })),
        )
            .into_response()
    }
}

async fn get_client_rate(
    State(state): State<ApiState>,
    Path((deck_id, code)): Path<(i32, String)>,
) -> impl IntoResponse {
    if let Some(rate) = state.lcr_engine.cache.get_client_rate(deck_id, &code) {
        (StatusCode::OK, Json(rate)).into_response()
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Rate not found"
            })),
        )
            .into_response()
    }
}

pub async fn start_api_server(lcr_engine: Arc<LcrEngine>, bind_addr: &str) -> Result<()> {
    let state = ApiState::new(lcr_engine);
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;

    tracing::info!("LCR API server listening on {}", bind_addr);

    axum::serve(listener, app).await?;

    Ok(())
}
