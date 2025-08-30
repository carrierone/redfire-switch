//! Class 4 B2BUA Integration Module
//! Integrates the Class 4 B2BUA with routing engines and provides a unified interface

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

use crate::class4_b2bua::{Class4B2BUA, Class4Config};
use crate::database_connections::EnhancedDatabasePool;
use crate::lcr::LcrEngine;
use crate::origination_routing::OriginationRoutingEngine;
use crate::route_advancement::RouteAdvancementEngine;
use crate::termination_routing::TerminationRoutingService;

/// Class 4 Switch Integration Service
/// This is the main entry point for the Class 4 switching functionality
pub struct Class4SwitchService {
    b2bua: Arc<Class4B2BUA>,
    config: Class4Config,
    database_pool: Arc<EnhancedDatabasePool>,
}

/// Class 4 Switch Builder for easy configuration and initialization
pub struct Class4SwitchBuilder {
    config: Class4Config,
    database_url: Option<String>,
    lcr_engine: Option<Arc<LcrEngine>>,
}

impl Class4SwitchBuilder {
    /// Create a new Class 4 Switch Builder
    pub fn new() -> Self {
        Self {
            config: Class4Config::default(),
            database_url: None,
            lcr_engine: None,
        }
    }

    /// Set the bind address for the B2BUA
    pub fn bind_address(mut self, addr: std::net::IpAddr) -> Self {
        self.config.bind_address = addr;
        self
    }

    /// Set the bind port for the B2BUA
    pub fn bind_port(mut self, port: u16) -> Self {
        self.config.bind_port = port;
        self
    }

    /// Set maximum concurrent calls
    pub fn max_concurrent_calls(mut self, max: u32) -> Self {
        self.config.max_concurrent_calls = max;
        self
    }

    /// Set call timeout in seconds
    pub fn call_timeout_seconds(mut self, timeout: u64) -> Self {
        self.config.call_timeout_seconds = timeout;
        self
    }

    /// Enable or disable CDR generation
    pub fn enable_cdr_generation(mut self, enable: bool) -> Self {
        self.config.enable_cdr_generation = enable;
        self
    }

    /// Enable or disable codec translation
    pub fn enable_codec_translation(mut self, enable: bool) -> Self {
        self.config.enable_codec_translation = enable;
        self
    }

    /// Set maximum route attempts
    pub fn max_route_attempts(mut self, max: u32) -> Self {
        self.config.max_route_attempts = max;
        self
    }

    /// Set database URL for routing and configuration
    pub fn database_url<S: Into<String>>(mut self, url: S) -> Self {
        self.database_url = Some(url.into());
        self
    }

    /// Set pre-initialized LCR engine
    pub fn lcr_engine(mut self, engine: Arc<LcrEngine>) -> Self {
        self.lcr_engine = Some(engine);
        self
    }

    /// Set RTP proxy configuration for media handling
    pub fn rtp_proxy(mut self, host: String, port: u16) -> Self {
        self.config.rtp_proxy_host = Some(host);
        self.config.rtp_proxy_port = Some(port);
        self
    }

    /// Build the Class 4 Switch Service
    pub async fn build(self) -> Result<Class4SwitchService> {
        info!("Building Class 4 Switch Service");

        // Initialize database connection
        let database_url = self
            .database_url
            .or_else(|| std::env::var("DATABASE_URL").ok())
            .unwrap_or_else(|| {
                tracing::warn!("No DATABASE_URL configured, using default localhost connection");
                "postgres://redfire_user:secure_password@localhost/redfire_switch".to_string()
            });

        let database_pool = Arc::new(EnhancedDatabasePool::from_url(&database_url).await?);

        // Initialize LCR engine if not provided
        let lcr_engine = if let Some(engine) = self.lcr_engine {
            engine
        } else {
            Arc::new(LcrEngine::new(&database_url).await?)
        };

        // Initialize routing engines
        let origination_config = crate::origination_routing::OriginationConfig::default();
        let origination_engine = Arc::new(Mutex::new(OriginationRoutingEngine::new(
            origination_config,
        )));
        let termination_service = Arc::new(Mutex::new(TerminationRoutingService::new(
            lcr_engine.clone(),
        )));
        let route_advancement = Arc::new(Mutex::new(RouteAdvancementEngine::new(
            termination_service.clone(),
            self.config.max_route_attempts,
        )));

        // Create Class 4 B2BUA
        let b2bua = Arc::new(
            Class4B2BUA::new(
                self.config.clone(),
                origination_engine,
                termination_service,
                route_advancement,
                vec![], // TODO: Add trunk rate configurations
            )
            .await?,
        );

        let service = Class4SwitchService {
            b2bua,
            config: self.config,
            database_pool,
        };

        info!("Class 4 Switch Service built successfully");
        Ok(service)
    }
}

impl Default for Class4SwitchBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Class4SwitchService {
    /// Create a new Class 4 Switch Builder
    pub fn builder() -> Class4SwitchBuilder {
        Class4SwitchBuilder::new()
    }

    /// Start the Class 4 Switch Service
    pub async fn start(&self) -> Result<()> {
        info!(
            "Starting Class 4 Switch Service on {}:{}",
            self.config.bind_address, self.config.bind_port
        );

        // Perform health checks
        self.perform_health_checks().await?;

        // Start the B2BUA main loop
        self.b2bua.run().await
    }

    /// Perform startup health checks
    async fn perform_health_checks(&self) -> Result<()> {
        info!("Performing Class 4 Switch health checks");

        // Database health check
        let db_health = self.database_pool.health_check().await;
        if !db_health.healthy {
            return Err(anyhow::anyhow!(
                "Database health check failed: {:?}",
                db_health.last_error
            ));
        }

        info!("All health checks passed");
        Ok(())
    }

    /// Get runtime statistics
    pub async fn get_statistics(&self) -> Result<Class4Statistics> {
        let session_stats = self.b2bua.session_manager().get_stats().await;
        let db_stats = self.database_pool.get_stats().await;

        Ok(Class4Statistics {
            active_calls: session_stats.active_sessions,
            total_calls: session_stats.total_sessions,
            successful_calls: session_stats.successful_calls,
            failed_calls: session_stats.failed_calls,
            peak_concurrent_calls: session_stats.peak_concurrent_calls,
            total_call_minutes: session_stats.total_call_minutes,
            average_setup_time_ms: session_stats.average_setup_time_ms,
            database_connections: db_stats.total_connections,
            database_queries: db_stats.total_queries,
            database_healthy: db_stats.connection_pool_healthy,
        })
    }

    /// Gracefully shutdown the service
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down Class 4 Switch Service");

        // Close database connections
        self.database_pool.close().await;

        info!("Class 4 Switch Service shut down completed");
        Ok(())
    }
}

/// Runtime statistics for the Class 4 Switch
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Class4Statistics {
    pub active_calls: u32,
    pub total_calls: u64,
    pub successful_calls: u64,
    pub failed_calls: u64,
    pub peak_concurrent_calls: u32,
    pub total_call_minutes: u64,
    pub average_setup_time_ms: u64,
    pub database_connections: u32,
    pub database_queries: u64,
    pub database_healthy: bool,
}

/// High-level Class 4 Switch API for external systems
pub struct Class4SwitchAPI {
    service: Arc<Class4SwitchService>,
}

impl Class4SwitchAPI {
    /// Create a new API instance
    pub fn new(service: Arc<Class4SwitchService>) -> Self {
        Self { service }
    }

    /// Get current call statistics
    pub async fn get_call_stats(&self) -> Result<Class4Statistics> {
        self.service.get_statistics().await
    }

    /// Get health status
    pub async fn health_check(&self) -> Result<HealthStatus> {
        let db_health = self.service.database_pool.health_check().await;

        Ok(HealthStatus {
            overall_healthy: db_health.healthy,
            database_healthy: db_health.healthy,
            database_response_time_ms: db_health.response_time_ms,
            active_calls: self.service.get_statistics().await?.active_calls,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Force cleanup of expired sessions
    pub async fn cleanup_sessions(&self) -> Result<()> {
        let timeout = std::time::Duration::from_secs(self.service.config.call_timeout_seconds);
        self.service
            .b2bua
            .session_manager()
            .cleanup_expired_sessions(timeout)
            .await;
        Ok(())
    }
}

/// Health status for monitoring systems
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct HealthStatus {
    pub overall_healthy: bool,
    pub database_healthy: bool,
    pub database_response_time_ms: u64,
    pub active_calls: u32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_class4_builder() {
        let builder = Class4SwitchBuilder::new()
            .bind_port(5080)
            .max_concurrent_calls(5000)
            .enable_cdr_generation(true)
            .enable_codec_translation(true);

        assert_eq!(builder.config.bind_port, 5080);
        assert_eq!(builder.config.max_concurrent_calls, 5000);
        assert!(builder.config.enable_cdr_generation);
        assert!(builder.config.enable_codec_translation);
    }

    #[test]
    fn test_class4_statistics_serialization() {
        let stats = Class4Statistics {
            active_calls: 100,
            total_calls: 10000,
            successful_calls: 9500,
            failed_calls: 500,
            peak_concurrent_calls: 250,
            total_call_minutes: 50000,
            average_setup_time_ms: 1200,
            database_connections: 20,
            database_queries: 1000000,
            database_healthy: true,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: Class4Statistics = serde_json::from_str(&json).unwrap();

        assert_eq!(stats.active_calls, deserialized.active_calls);
        assert_eq!(stats.total_calls, deserialized.total_calls);
    }
}

/// Example usage and demonstration functions
pub mod examples {
    use super::*;

    /// Example: Basic Class 4 Switch setup
    pub async fn basic_class4_switch_example() -> Result<()> {
        let service = Class4SwitchService::builder()
            .bind_address("127.0.0.1".parse()?)
            .bind_port(5060)
            .max_concurrent_calls(1000)
            .call_timeout_seconds(3600) // 1 hour
            .enable_cdr_generation(true)
            .enable_codec_translation(true)
            .database_url(std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://redfire_user:secure_password@localhost/redfire_switch".to_string()))
            .build()
            .await?;

        // This would start the service (blocks until shutdown)
        // service.start().await?;

        Ok(())
    }

    /// Example: Class 4 Switch with custom configuration
    pub async fn advanced_class4_switch_example() -> Result<()> {
        // Pre-initialize LCR engine with custom configuration
        let lcr_engine = Arc::new(
            LcrEngine::new(&std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://redfire_user:secure_password@localhost/redfire_switch".to_string())).await?,
        );

        let service = Class4SwitchService::builder()
            .bind_address("0.0.0.0".parse()?)
            .bind_port(5080)
            .max_concurrent_calls(10000)
            .max_route_attempts(5)
            .call_timeout_seconds(1800) // 30 minutes
            .enable_cdr_generation(true)
            .enable_codec_translation(true)
            .rtp_proxy("192.168.1.100".to_string(), 7000)
            .lcr_engine(lcr_engine)
            .database_url(std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://redfire_user:secure_password@localhost/redfire_switch".to_string()))
            .build()
            .await?;

        // Create API interface for monitoring
        let api = Class4SwitchAPI::new(Arc::new(service));

        // Example monitoring operations
        let stats = api.get_call_stats().await?;
        println!("Current stats: {:?}", stats);

        let health = api.health_check().await?;
        println!("Health status: {:?}", health);

        Ok(())
    }

    /// Example: Production deployment setup
    pub async fn production_class4_switch_example() -> Result<()> {
        let service = Class4SwitchService::builder()
            .bind_address("0.0.0.0".parse()?)
            .bind_port(5060)
            .max_concurrent_calls(50000) // High capacity
            .max_route_attempts(3)
            .call_timeout_seconds(10800) // 3 hours max call duration
            .enable_cdr_generation(true)
            .enable_codec_translation(true)
            .rtp_proxy("rtp-proxy.example.com".to_string(), 7000)
            .database_url(
                &std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://redfire_user:secure_password@db.example.com:5432/redfire_production".to_string()),
            )
            .build()
            .await?;

        info!("Starting production Class 4 Switch");

        // In production, this would be wrapped with proper signal handling
        // for graceful shutdown
        match service.start().await {
            Ok(()) => info!("Class 4 Switch stopped normally"),
            Err(e) => error!("Class 4 Switch stopped with error: {}", e),
        }

        service.shutdown().await?;
        Ok(())
    }
}
