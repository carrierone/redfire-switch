//! Prometheus metrics API endpoints
//!
//! This module provides HTTP endpoints for exposing Prometheus metrics.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::sync::Arc;
use tracing::{debug, error};

use crate::monitoring::{MonitoringSystem, PrometheusExporter};

/// Metrics endpoint state
#[derive(Clone)]
pub struct MetricsState {
    pub prometheus_exporter: Arc<PrometheusExporter>,
    pub monitoring_system: Arc<MonitoringSystem>,
}

/// Create metrics router
pub fn create_metrics_router(state: MetricsState) -> Router {
    Router::new()
        .route("/metrics", get(prometheus_metrics_handler))
        .route("/health", get(health_check_handler))
        .route("/health/ready", get(readiness_check_handler))
        .route("/health/live", get(liveness_check_handler))
        .with_state(state)
}

/// Prometheus metrics endpoint
///
/// Returns metrics in Prometheus text exposition format
#[utoipa::path(
    get,
    path = "/metrics",
    responses(
        (status = 200, description = "Prometheus metrics", content_type = "text/plain"),
        (status = 500, description = "Failed to export metrics")
    ),
    tag = "metrics"
)]
async fn prometheus_metrics_handler(
    State(state): State<MetricsState>,
) -> Result<Response, StatusCode> {
    debug!("Prometheus metrics requested");

    match state.prometheus_exporter.export_metrics().await {
        Ok(metrics_text) => {
            Ok((
                [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
                metrics_text,
            )
                .into_response())
        }
        Err(e) => {
            error!("Failed to export Prometheus metrics: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Health check endpoint
///
/// Returns overall system health status
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "System is healthy"),
        (status = 503, description = "System is unhealthy")
    ),
    tag = "health"
)]
async fn health_check_handler(
    State(state): State<MetricsState>,
) -> Result<Response, StatusCode> {
    let system_status = state.monitoring_system.get_system_status().await;

    match system_status {
        crate::monitoring::SystemStatus::Healthy => {
            Ok((StatusCode::OK, "OK").into_response())
        }
        crate::monitoring::SystemStatus::Degraded => {
            Ok((StatusCode::OK, "DEGRADED").into_response())
        }
        _ => {
            Ok((StatusCode::SERVICE_UNAVAILABLE, "UNHEALTHY").into_response())
        }
    }
}

/// Readiness check endpoint
///
/// Indicates if the service is ready to accept traffic
#[utoipa::path(
    get,
    path = "/health/ready",
    responses(
        (status = 200, description = "Service is ready"),
        (status = 503, description = "Service is not ready")
    ),
    tag = "health"
)]
async fn readiness_check_handler(
    State(state): State<MetricsState>,
) -> Result<Response, StatusCode> {
    let system_status = state.monitoring_system.get_system_status().await;
    let health_checker = state.monitoring_system.health();

    // Check if critical components are healthy
    match health_checker.check_all_health().await {
        Ok(health_results) => {
            let all_healthy = health_results.values().all(|status| {
                matches!(
                    status,
                    crate::monitoring::HealthStatus::Healthy | crate::monitoring::HealthStatus::Warning
                )
            });

            if all_healthy && !matches!(system_status, crate::monitoring::SystemStatus::Starting | crate::monitoring::SystemStatus::Stopping) {
                Ok((StatusCode::OK, "READY").into_response())
            } else {
                Ok((StatusCode::SERVICE_UNAVAILABLE, "NOT_READY").into_response())
            }
        }
        Err(_) => {
            Ok((StatusCode::SERVICE_UNAVAILABLE, "NOT_READY").into_response())
        }
    }
}

/// Liveness check endpoint
///
/// Indicates if the service is alive (for Kubernetes liveness probes)
#[utoipa::path(
    get,
    path = "/health/live",
    responses(
        (status = 200, description = "Service is alive"),
        (status = 503, description = "Service is not alive")
    ),
    tag = "health"
)]
async fn liveness_check_handler(
    State(_state): State<MetricsState>,
) -> Result<Response, StatusCode> {
    // If we can respond, we're alive
    Ok((StatusCode::OK, "ALIVE").into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitoring::{MetricsCollector, MonitoringConfig};
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_prometheus_metrics_endpoint() {
        let config = MonitoringConfig::default();
        let monitoring = Arc::new(MonitoringSystem::new(config).unwrap());
        let prometheus = Arc::new(
            PrometheusExporter::new(monitoring.metrics()).unwrap()
        );

        let state = MetricsState {
            prometheus_exporter: prometheus,
            monitoring_system: monitoring,
        };

        let app = create_metrics_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let content_type = response.headers().get(header::CONTENT_TYPE).unwrap();
        assert!(content_type.to_str().unwrap().contains("text/plain"));
    }

    #[tokio::test]
    async fn test_health_check_endpoint() {
        let config = MonitoringConfig::default();
        let monitoring = Arc::new(MonitoringSystem::new(config).unwrap());
        let prometheus = Arc::new(
            PrometheusExporter::new(monitoring.metrics()).unwrap()
        );

        let state = MetricsState {
            prometheus_exporter: prometheus,
            monitoring_system: monitoring,
        };

        let app = create_metrics_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should return OK or SERVICE_UNAVAILABLE
        assert!(
            response.status() == StatusCode::OK
                || response.status() == StatusCode::SERVICE_UNAVAILABLE
        );
    }
}
