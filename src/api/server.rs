/*
 * Redfire Switch - API Server Implementation
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
    serve, Router,
};
use std::fs;
use std::net::SocketAddr;
use std::os::unix::fs::PermissionsExt;
use tokio::net::{TcpListener, UnixListener as TokioUnixListener};
use tracing::{error, info, warn};

use crate::api::config::{ApiServerConfig, HttpListener, HttpProtocol, UnixListener};
use crate::api::endpoints::create_additional_routes;
use crate::rest_api::AppState;

pub struct ApiServer {
    config: ApiServerConfig,
    state: AppState,
}

impl ApiServer {
    pub fn new(config: ApiServerConfig, state: AppState) -> Self {
        Self { config, state }
    }

    pub async fn start(&self) -> Result<()> {
        // Validate configuration
        self.config
            .validate()
            .map_err(|e| anyhow::anyhow!("Configuration validation failed: {}", e))?;

        let mut handles = Vec::new();

        // Start HTTP/HTTPS listeners
        for listener in self.config.get_enabled_http_listeners() {
            let router = self.create_router().await;
            let handle = self.start_http_listener(listener, router).await?;
            handles.push(handle);
        }

        // Start Unix socket listeners
        for listener in self.config.get_enabled_unix_listeners() {
            let router = self.create_router().await;
            let handle = self.start_unix_listener(listener, router).await?;
            handles.push(handle);
        }

        if handles.is_empty() {
            return Err(anyhow!("No listeners configured"));
        }

        info!("Started {} API listeners", handles.len());

        // Wait for all listeners
        futures::future::try_join_all(handles).await?;

        Ok(())
    }

    async fn create_router(&self) -> Router {
        use crate::rest_api::create_api_router_with_state;
        let base_router = create_api_router_with_state(self.state.clone());
        let additional_router = create_additional_routes().with_state(self.state.clone());

        // Simple merge without complex middleware for now
        base_router.merge(additional_router)
    }

    async fn start_http_listener(
        &self,
        listener_config: &HttpListener,
        router: Router,
    ) -> Result<tokio::task::JoinHandle<Result<()>>> {
        let addr = SocketAddr::new(listener_config.bind_address, listener_config.port);
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| anyhow!("Failed to bind HTTP listener to {}: {}", addr, e))?;

        let protocol_str = match listener_config.protocol {
            HttpProtocol::Http => "HTTP",
            HttpProtocol::Https => "HTTPS",
        };

        info!(
            "Starting {} listener '{}' on {} ({})",
            protocol_str, listener_config.name, addr, listener_config.description
        );

        let listener_name = listener_config.name.clone();
        let handle = tokio::spawn(async move {
            // Convert router to make service for axum serve
            if let Err(e) = serve(listener, router).await {
                error!("HTTP listener '{}' failed: {}", listener_name, e);
                return Err(anyhow!("HTTP listener failed: {}", e));
            }
            Ok(())
        });

        Ok(handle)
    }

    async fn start_unix_listener(
        &self,
        listener_config: &UnixListener,
        router: Router,
    ) -> Result<tokio::task::JoinHandle<Result<()>>> {
        let socket_path = &listener_config.socket_path;

        // Remove existing socket if it exists
        if socket_path.exists() {
            fs::remove_file(socket_path).map_err(|e| {
                anyhow!(
                    "Failed to remove existing socket {}: {}",
                    socket_path.display(),
                    e
                )
            })?;
        }

        // Create parent directory if it doesn't exist
        if let Some(parent) = socket_path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                anyhow!(
                    "Failed to create socket directory {}: {}",
                    parent.display(),
                    e
                )
            })?;
        }

        let listener = TokioUnixListener::bind(socket_path).map_err(|e| {
            anyhow!(
                "Failed to bind Unix listener to {}: {}",
                socket_path.display(),
                e
            )
        })?;

        // Set socket permissions
        let permissions = fs::Permissions::from_mode(listener_config.file_permissions);
        fs::set_permissions(socket_path, permissions)
            .map_err(|e| anyhow!("Failed to set socket permissions: {}", e))?;

        info!(
            "Starting Unix socket listener '{}' on {} ({})",
            listener_config.name,
            socket_path.display(),
            listener_config.description
        );

        let listener_name = listener_config.name.clone();
        let _socket_path_str = socket_path.display().to_string();

        let handle = tokio::spawn(async move {
            if let Err(e) = serve_unix(listener, router).await {
                error!("Unix listener '{}' failed: {}", listener_name, e);
                return Err(anyhow!("Unix listener failed: {}", e));
            }
            Ok(())
        });

        Ok(handle)
    }
}

// Helper function to serve on Unix socket (simplified for compilation)
async fn serve_unix(_listener: TokioUnixListener, _router: Router) -> Result<()> {
    // TODO: Implement proper Unix socket serving
    // For now, just return Ok to allow compilation
    warn!("Unix socket serving not fully implemented yet");
    Ok(())
}

// Convenience functions for common configurations

pub async fn start_development_server(state: AppState) -> Result<()> {
    let config = ApiServerConfig::development();
    let server = ApiServer::new(config, state);
    server.start().await
}

pub async fn start_production_server(state: AppState) -> Result<()> {
    let config = ApiServerConfig::production();
    let server = ApiServer::new(config, state);
    server.start().await
}

pub async fn start_unix_only_server(state: AppState) -> Result<()> {
    let config = ApiServerConfig::unix_only();
    let server = ApiServer::new(config, state);
    server.start().await
}

pub async fn start_custom_server(config: ApiServerConfig, state: AppState) -> Result<()> {
    let server = ApiServer::new(config, state);
    server.start().await
}
