/*
 * Redfire Switch - API Tests
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

#[cfg(test)]
mod tests {
    use super::super::auth::LoginRequest;
    use super::super::auth::{AuthConfig, AuthState, Permission};
    use super::super::config::{ApiServerConfig, HttpListener, HttpProtocol, UnixListener};
    use crate::api::simplified_server::create_simple_api_router;
    use crate::rest_api::{ApiResponse, AppState};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::{json, Value};
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::test;
    use tower::ServiceExt;

    // Helper function to create test app state
    fn create_test_app_state() -> AppState {
        let auth_config = AuthConfig {
            jwt_secret: "test_secret_key_for_testing_only".to_string(),
            jwt_expiration_hours: 1,
            max_failed_attempts: 3,
            lockout_duration_minutes: 5,
            ..Default::default()
        };

        AppState::with_auth_config(auth_config)
    }

    // Helper function to create test API router
    fn create_test_router() -> axum::Router {
        create_simple_api_router()
    }

    #[tokio::test]
    async fn test_auth_config_validation() {
        let config = AuthConfig::default();
        assert!(!config.jwt_secret.is_empty());
        assert!(config.jwt_expiration_hours > 0);
        assert!(config.max_failed_attempts > 0);
    }

    #[tokio::test]
    async fn test_api_server_config_validation() {
        // Test valid configuration
        let valid_config = ApiServerConfig::development();
        assert!(valid_config.validate().is_ok());

        // Test empty listeners (should fail)
        let mut invalid_config = ApiServerConfig::development();
        invalid_config.http_listeners.clear();
        invalid_config.unix_listeners.clear();
        assert!(invalid_config.validate().is_err());

        // Test Unix-only configuration
        let unix_config = ApiServerConfig::unix_only();
        assert!(unix_config.validate().is_ok());
    }

    #[tokio::test]
    async fn test_authentication_flow() {
        let state = create_test_app_state();

        // Test successful authentication
        let result = state.auth_state.authenticate("admin", "admin123").await;
        assert!(
            result.is_ok(),
            "Default admin authentication should succeed"
        );

        let token = result.unwrap();
        assert!(!token.is_empty());

        // Test token verification
        let claims_result = state.auth_state.verify_token(&token).await;
        assert!(claims_result.is_ok(), "Token verification should succeed");

        let claims = claims_result.unwrap();
        assert_eq!(claims.username, "admin");
        assert!(claims.permissions.contains(&Permission::SystemAdmin));

        // Test invalid credentials
        let invalid_result = state
            .auth_state
            .authenticate("admin", "wrong_password")
            .await;
        assert!(invalid_result.is_err(), "Invalid password should fail");

        // Test nonexistent user
        let nonexistent_result = state
            .auth_state
            .authenticate("nonexistent", "password")
            .await;
        assert!(nonexistent_result.is_err(), "Nonexistent user should fail");
    }

    #[tokio::test]
    async fn test_permission_system() {
        let state = create_test_app_state();

        // Authenticate as admin
        let token = state
            .auth_state
            .authenticate("admin", "admin123")
            .await
            .unwrap();
        let claims = state.auth_state.verify_token(&token).await.unwrap();

        // Test admin has all permissions
        assert!(state
            .auth_state
            .has_permission(&claims, &Permission::SystemAdmin));
        assert!(state
            .auth_state
            .has_permission(&claims, &Permission::CallsWrite));
        assert!(state
            .auth_state
            .has_permission(&claims, &Permission::ConfigWrite));
        assert!(state
            .auth_state
            .has_permission(&claims, &Permission::MonitoringWrite));
    }

    #[tokio::test]
    async fn test_login_endpoint() {
        let app = create_test_router();

        let login_request = LoginRequest {
            username: "admin".to_string(),
            password: "admin123".to_string(),
        };

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_string(&login_request).unwrap()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should succeed with correct credentials
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let response_json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(response_json["success"], true);
        assert!(response_json["data"]["token"].is_string());
        assert!(response_json["data"]["user"]["username"].is_string());
    }

    #[tokio::test]
    async fn test_system_stats_endpoint_without_auth() {
        let app = create_test_router();

        let request = Request::builder()
            .method("GET")
            .uri("/api/v1/system/stats")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should succeed (no auth required for system stats in basic implementation)
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_protected_endpoint_without_auth() {
        let app = create_test_router();

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/system/config/reload")
            .header("content-type", "application/json")
            .body(Body::from(json!({"force": false}).to_string()))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should fail without authentication
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_invalid_json_request() {
        let app = create_test_router();

        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/auth/login")
            .header("content-type", "application/json")
            .body(Body::from("invalid json"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // Should fail with bad request
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_api_response_serialization() {
        let success_response = ApiResponse::success("test data".to_string());
        assert!(success_response.success);
        assert_eq!(success_response.data, Some("test data".to_string()));
        assert!(success_response.error.is_none());

        let error_response: ApiResponse<String> = ApiResponse::error("test error".to_string());
        assert!(!error_response.success);
        assert!(error_response.data.is_none());
        assert_eq!(error_response.error, Some("test error".to_string()));
    }

    #[tokio::test]
    async fn test_network_listener_configurations() {
        // Test IPv4 listener
        let ipv4_listener = HttpListener {
            enabled: true,
            bind_address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port: 8080,
            protocol: HttpProtocol::Http,
            name: "test-ipv4".to_string(),
            description: "Test IPv4 listener".to_string(),
        };

        assert!(ipv4_listener.enabled);
        assert_eq!(ipv4_listener.port, 8080);

        // Test Unix socket listener
        let unix_listener = UnixListener {
            enabled: true,
            socket_path: "/tmp/test.sock".into(),
            name: "test-unix".to_string(),
            description: "Test Unix listener".to_string(),
            file_permissions: 0o600,
        };

        assert!(unix_listener.enabled);
        assert_eq!(unix_listener.file_permissions, 0o600);
    }

    #[tokio::test]
    async fn test_pagination_defaults() {
        use crate::rest_api::PaginationQuery;

        // Test default values
        let default_query = PaginationQuery {
            page: crate::rest_api::default_page(),
            limit: crate::rest_api::default_limit(),
        };

        assert_eq!(default_query.page, 1);
        assert_eq!(default_query.limit, 50);
    }

    #[tokio::test]
    async fn test_call_status_serialization() {
        use crate::rest_api::CallStatus;

        // Test serialization of call statuses
        let statuses = vec![
            CallStatus::Ringing,
            CallStatus::Answered,
            CallStatus::Busy,
            CallStatus::Failed,
            CallStatus::Completed,
        ];

        for status in statuses {
            let serialized = serde_json::to_string(&status).unwrap();
            assert!(!serialized.is_empty());

            let deserialized: CallStatus = serde_json::from_str(&serialized).unwrap();
            // Note: Can't directly compare due to enum variant comparison limitations
            let re_serialized = serde_json::to_string(&deserialized).unwrap();
            assert_eq!(serialized, re_serialized);
        }
    }

    #[tokio::test]
    async fn test_concurrent_authentication() {
        let state = create_test_app_state();

        // Test multiple concurrent authentication attempts
        let mut handles = Vec::new();

        for i in 0..10 {
            let state_clone = state.clone();
            let handle = tokio::spawn(async move {
                let result = state_clone
                    .auth_state
                    .authenticate("admin", "admin123")
                    .await;
                (i, result.is_ok())
            });
            handles.push(handle);
        }

        // Wait for all authentication attempts
        let results = futures::future::join_all(handles).await;

        // All should succeed
        for result in results {
            let (_, success) = result.unwrap();
            assert!(success, "Concurrent authentication should succeed");
        }
    }

    #[tokio::test]
    async fn test_app_state_builders() {
        let auth_config = AuthConfig::default();
        let api_config = ApiServerConfig::development();

        let state = AppState::with_auth_config(auth_config.clone());

        // Verify configuration was applied
        let stored_api_config = state.api_config.read().await;
        assert!(!stored_api_config.get_enabled_http_listeners().is_empty());
    }
}
