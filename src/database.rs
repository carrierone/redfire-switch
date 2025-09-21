//! Database service - Production implementation
//! Comprehensive database integration with connection pooling, migrations, and monitoring

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres, Row};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connection_timeout_seconds: u64,
    pub idle_timeout_seconds: u64,
    pub acquire_timeout_seconds: u64,
    pub max_lifetime_seconds: u64,
    pub enable_logging: bool,
    pub auto_migrate: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://redfire:password@localhost/redfire_switch".to_string(),
            max_connections: 100,
            min_connections: 10,
            connection_timeout_seconds: 30,
            idle_timeout_seconds: 600,
            acquire_timeout_seconds: 30,
            max_lifetime_seconds: 1800,
            enable_logging: true,
            auto_migrate: true,
        }
    }
}

pub struct DatabaseService {
    pool: Pool<Postgres>,
    config: DatabaseConfig,
    statistics: Arc<RwLock<DatabaseStatistics>>,
}

#[derive(Debug, Default)]
struct DatabaseStatistics {
    total_queries: u64,
    successful_queries: u64,
    failed_queries: u64,
    connection_errors: u64,
    average_response_time_ms: f64,
    active_connections: u32,
    idle_connections: u32,
}

impl DatabaseService {
    pub async fn new(config: DatabaseConfig) -> Result<Self> {
        info!(
            "Initializing database service with URL: {}",
            mask_database_url(&config.url)
        );

        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connection_timeout_seconds))
            .idle_timeout(Duration::from_secs(config.idle_timeout_seconds))
            .max_lifetime(Duration::from_secs(config.max_lifetime_seconds))
            .connect(&config.url)
            .await
            .map_err(|e| anyhow!("Failed to create database pool: {}", e))?;

        let service = Self {
            pool,
            config,
            statistics: Arc::new(RwLock::new(DatabaseStatistics::default())),
        };

        // Run migrations if enabled
        if service.config.auto_migrate {
            service.run_migrations().await?;
        }

        // Test database connection
        service.health_check().await?;

        info!("Database service initialized successfully");
        Ok(service)
    }

    async fn run_migrations(&self) -> Result<()> {
        info!("Running database migrations...");

        // Check if migrations table exists
        let migration_table_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'schema_migrations')"
        )
        .fetch_one(&self.pool)
        .await?;

        if !migration_table_exists {
            // Create migrations table
            sqlx::query(
                r#"
                CREATE TABLE schema_migrations (
                    id SERIAL PRIMARY KEY,
                    version VARCHAR(100) NOT NULL UNIQUE,
                    applied_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
                )
                "#,
            )
            .execute(&self.pool)
            .await?;

            info!("Created schema_migrations table");
        }

        // Check if initial schema has been applied
        let initial_migration_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = '001_initial_schema')",
        )
        .fetch_one(&self.pool)
        .await?;

        if !initial_migration_exists {
            // Read and execute initial schema migration
            let migration_sql = include_str!("../migrations/001_initial_schema.sql");

            // Execute the migration in a transaction
            let mut tx = self.pool.begin().await?;

            for statement in migration_sql.split(';') {
                let statement = statement.trim();
                if !statement.is_empty() && !statement.starts_with("--") {
                    sqlx::query(statement)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| {
                            anyhow!("Migration failed at statement '{}': {}", statement, e)
                        })?;
                }
            }

            // Record the migration
            sqlx::query("INSERT INTO schema_migrations (version) VALUES ('001_initial_schema')")
                .execute(&mut *tx)
                .await?;

            tx.commit().await?;
            info!("Applied initial schema migration");
        }

        Ok(())
    }

    /// Check database health and connectivity
    pub async fn health_check(&self) -> Result<DatabaseHealthStatus> {
        let start_time = std::time::Instant::now();

        let result = sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&self.pool)
            .await;

        let response_time_ms = start_time.elapsed().as_millis() as f64;

        let mut stats = self.statistics.write().await;
        stats.total_queries += 1;

        match result {
            Ok(_) => {
                stats.successful_queries += 1;
                stats.average_response_time_ms = (stats.average_response_time_ms
                    * (stats.successful_queries - 1) as f64
                    + response_time_ms)
                    / stats.successful_queries as f64;

                debug!("Database health check passed in {:.2}ms", response_time_ms);

                Ok(DatabaseHealthStatus {
                    connected: true,
                    response_time_ms,
                    active_connections: self.pool.num_idle() as u32,
                    error: None,
                })
            }
            Err(e) => {
                stats.failed_queries += 1;
                error!("Database health check failed: {}", e);

                Ok(DatabaseHealthStatus {
                    connected: false,
                    response_time_ms,
                    active_connections: 0,
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// Insert a new CDR record
    pub async fn insert_cdr(&self, cdr: &crate::cdr::CallDetailRecord) -> Result<()> {
        let start_time = std::time::Instant::now();

        let result = sqlx::query!(
            r#"
            INSERT INTO call_detail_records (
                id, call_id, session_id, from_number, to_number, from_ip, to_ip,
                start_time, end_time, duration_seconds, disposition, hangup_cause,
                trunk_id, route_id, codec_in, codec_out, recording_enabled, cost
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            "#,
            cdr.id,
            cdr.call_id,
            cdr.session_id,
            cdr.from_number,
            cdr.to_number,
            cdr.from_ip.map(|ip| ip.to_string()),
            cdr.to_ip.map(|ip| ip.to_string()),
            cdr.start_time,
            cdr.end_time,
            cdr.duration_seconds as i64,
            serde_json::to_string(&cdr.disposition)?,
            cdr.hangup_cause.map(|c| c as i32),
            cdr.trunk_id,
            cdr.route_id,
            cdr.codec_in,
            cdr.codec_out,
            cdr.recording_enabled,
            cdr.cost.map(|c| c.to_string())
        )
        .execute(&self.pool)
        .await;

        self.update_query_statistics(start_time, result.is_ok())
            .await;

        match result {
            Ok(_) => {
                debug!(
                    "CDR record inserted successfully for call_id: {}",
                    cdr.call_id
                );
                Ok(())
            }
            Err(e) => {
                error!("Failed to insert CDR record: {}", e);
                Err(anyhow!("Database insert failed: {}", e))
            }
        }
    }

    /// Get active sessions count
    pub async fn get_active_sessions_count(&self) -> Result<i64> {
        let start_time = std::time::Instant::now();

        let result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM active_sessions WHERE state IN ('Establishing', 'Active')",
        )
        .fetch_one(&self.pool)
        .await;

        self.update_query_statistics(start_time, result.is_ok())
            .await;

        match result {
            Ok(count) => Ok(count),
            Err(e) => {
                error!("Failed to get active sessions count: {}", e);
                Err(anyhow!("Database query failed: {}", e))
            }
        }
    }

    /// Insert active session
    pub async fn insert_active_session(
        &self,
        session: &crate::call_control::CallSession,
    ) -> Result<()> {
        let start_time = std::time::Instant::now();

        let state_str = format!("{:?}", session.state);

        let result = sqlx::query!(
            r#"
            INSERT INTO active_sessions (
                id, call_id, session_id, from_number, to_number, from_ip, to_ip,
                trunk_id, start_time, last_activity, state, codec_in, codec_out
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
            session.id,
            session.call_id,
            Some(session.id), // Using session.id as session_id
            "unknown",        // TODO: Extract from session
            "unknown",        // TODO: Extract from session
            Some(session.from_addr.to_string()),
            Some(session.to_addr.to_string()),
            session
                .trunk_id
                .as_ref()
                .and_then(|t| t.parse::<i32>().ok()),
            session.start_time,
            session.last_activity,
            state_str,
            session
                .codec_pair
                .as_ref()
                .map(|(in_codec, _)| in_codec.as_str()),
            session
                .codec_pair
                .as_ref()
                .map(|(_, out_codec)| out_codec.as_str())
        )
        .execute(&self.pool)
        .await;

        self.update_query_statistics(start_time, result.is_ok())
            .await;

        match result {
            Ok(_) => {
                debug!("Active session inserted for call_id: {}", session.call_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to insert active session: {}", e);
                Err(anyhow!("Database insert failed: {}", e))
            }
        }
    }

    /// Remove active session
    pub async fn remove_active_session(&self, session_id: Uuid) -> Result<()> {
        let start_time = std::time::Instant::now();

        let result = sqlx::query!("DELETE FROM active_sessions WHERE id = $1", session_id)
            .execute(&self.pool)
            .await;

        self.update_query_statistics(start_time, result.is_ok())
            .await;

        match result {
            Ok(_) => {
                debug!("Active session removed: {}", session_id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to remove active session: {}", e);
                Err(anyhow!("Database delete failed: {}", e))
            }
        }
    }

    /// Get LCR routes for a given prefix
    pub async fn get_lcr_routes(
        &self,
        prefix: &str,
        route_group: &str,
        limit: i32,
    ) -> Result<Vec<LcrRoute>> {
        let start_time = std::time::Instant::now();

        let result = sqlx::query_as!(
            LcrRoute,
            r#"
            SELECT
                r.id, r.prefix, r.description, r.trunk_id, r.priority,
                r.cost_per_minute, r.quality_score, r.max_call_duration,
                t.name as trunk_name
            FROM lcr_routes r
            JOIN trunks t ON r.trunk_id = t.id
            WHERE r.route_group = $1
                AND $2 LIKE (r.prefix || '%')
                AND r.enabled = true
                AND t.enabled = true
                AND (r.effective_date IS NULL OR r.effective_date <= NOW())
                AND (r.expiry_date IS NULL OR r.expiry_date > NOW())
            ORDER BY LENGTH(r.prefix) DESC, r.priority ASC, r.cost_per_minute ASC
            LIMIT $3
            "#,
            route_group,
            prefix,
            limit
        )
        .fetch_all(&self.pool)
        .await;

        self.update_query_statistics(start_time, result.is_ok())
            .await;

        match result {
            Ok(routes) => {
                debug!(
                    "Found {} LCR routes for prefix '{}' in group '{}'",
                    routes.len(),
                    prefix,
                    route_group
                );
                Ok(routes)
            }
            Err(e) => {
                error!("Failed to get LCR routes: {}", e);
                Err(anyhow!("Database query failed: {}", e))
            }
        }
    }

    /// Insert security event
    pub async fn insert_security_event(&self, event: &SecurityEvent) -> Result<()> {
        let start_time = std::time::Instant::now();

        let result = sqlx::query!(
            r#"
            INSERT INTO security_events (
                id, event_type, source_ip, severity, description, details, action_taken
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            event.id,
            event.event_type,
            event.source_ip.to_string(),
            event.severity,
            event.description,
            serde_json::to_value(&event.details)?,
            event.action_taken
        )
        .execute(&self.pool)
        .await;

        self.update_query_statistics(start_time, result.is_ok())
            .await;

        match result {
            Ok(_) => {
                debug!("Security event inserted: {}", event.id);
                Ok(())
            }
            Err(e) => {
                error!("Failed to insert security event: {}", e);
                Err(anyhow!("Database insert failed: {}", e))
            }
        }
    }

    /// Insert health check result
    pub async fn insert_health_check_result(&self, result: &HealthCheckResult) -> Result<()> {
        let start_time = std::time::Instant::now();

        let query_result = sqlx::query!(
            r#"
            INSERT INTO health_check_results (id, component, status, response_time_ms, details)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            result.id,
            result.component,
            result.status,
            result.response_time_ms,
            serde_json::to_value(&result.details)?
        )
        .execute(&self.pool)
        .await;

        self.update_query_statistics(start_time, query_result.is_ok())
            .await;

        match query_result {
            Ok(_) => {
                debug!(
                    "Health check result inserted for component: {}",
                    result.component
                );
                Ok(())
            }
            Err(e) => {
                error!("Failed to insert health check result: {}", e);
                Err(anyhow!("Database insert failed: {}", e))
            }
        }
    }

    /// Get system configuration value
    pub async fn get_config_value(&self, key: &str) -> Result<Option<serde_json::Value>> {
        let start_time = std::time::Instant::now();

        let result = sqlx::query!(
            "SELECT config_value FROM system_config WHERE config_key = $1",
            key
        )
        .fetch_optional(&self.pool)
        .await;

        self.update_query_statistics(start_time, result.is_ok())
            .await;

        match result {
            Ok(Some(row)) => Ok(Some(row.config_value)),
            Ok(None) => Ok(None),
            Err(e) => {
                error!("Failed to get config value for key '{}': {}", key, e);
                Err(anyhow!("Database query failed: {}", e))
            }
        }
    }

    /// Set system configuration value
    pub async fn set_config_value(&self, key: &str, value: serde_json::Value) -> Result<()> {
        let start_time = std::time::Instant::now();

        let result = sqlx::query!(
            r#"
            INSERT INTO system_config (config_key, config_value, config_type)
            VALUES ($1, $2, 'dynamic')
            ON CONFLICT (config_key)
            DO UPDATE SET config_value = $2, updated_at = NOW()
            "#,
            key,
            value
        )
        .execute(&self.pool)
        .await;

        self.update_query_statistics(start_time, result.is_ok())
            .await;

        match result {
            Ok(_) => {
                debug!("Configuration value set for key: {}", key);
                Ok(())
            }
            Err(e) => {
                error!("Failed to set config value for key '{}': {}", key, e);
                Err(anyhow!("Database update failed: {}", e))
            }
        }
    }

    /// Get database statistics
    pub async fn get_statistics(&self) -> DatabaseStatistics {
        let stats = self.statistics.read().await;
        DatabaseStatistics {
            total_queries: stats.total_queries,
            successful_queries: stats.successful_queries,
            failed_queries: stats.failed_queries,
            connection_errors: stats.connection_errors,
            average_response_time_ms: stats.average_response_time_ms,
            active_connections: self.pool.size() as u32,
            idle_connections: self.pool.num_idle() as u32,
        }
    }

    /// Clean up old data (CDRs, health checks, etc.)
    pub async fn cleanup_old_data(&self) -> Result<()> {
        let start_time = std::time::Instant::now();

        let result = sqlx::query("SELECT cleanup_old_data()")
            .execute(&self.pool)
            .await;

        self.update_query_statistics(start_time, result.is_ok())
            .await;

        match result {
            Ok(_) => {
                info!("Database cleanup completed successfully");
                Ok(())
            }
            Err(e) => {
                error!("Database cleanup failed: {}", e);
                Err(anyhow!("Database cleanup failed: {}", e))
            }
        }
    }

    async fn update_query_statistics(&self, start_time: std::time::Instant, success: bool) {
        let response_time_ms = start_time.elapsed().as_millis() as f64;
        let mut stats = self.statistics.write().await;

        stats.total_queries += 1;
        if success {
            stats.successful_queries += 1;
            stats.average_response_time_ms = (stats.average_response_time_ms
                * (stats.successful_queries - 1) as f64
                + response_time_ms)
                / stats.successful_queries as f64;
        } else {
            stats.failed_queries += 1;
        }
    }

    /// Get database pool for advanced operations
    pub fn get_pool(&self) -> &Pool<Postgres> {
        &self.pool
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealthStatus {
    pub connected: bool,
    pub response_time_ms: f64,
    pub active_connections: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LcrRoute {
    pub id: i32,
    pub prefix: String,
    pub description: Option<String>,
    pub trunk_id: Option<i32>,
    pub priority: Option<i32>,
    pub cost_per_minute: Option<rust_decimal::Decimal>,
    pub quality_score: Option<i32>,
    pub max_call_duration: Option<i32>,
    pub trunk_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    pub id: Uuid,
    pub event_type: String,
    pub source_ip: std::net::IpAddr,
    pub severity: String,
    pub description: String,
    pub details: serde_json::Value,
    pub action_taken: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub id: Uuid,
    pub component: String,
    pub status: String,
    pub response_time_ms: Option<i32>,
    pub details: serde_json::Value,
}

fn mask_database_url(url: &str) -> String {
    if let Some(at_pos) = url.find('@') {
        if let Some(colon_pos) = url[..at_pos].rfind(':') {
            let mut masked = url.to_string();
            masked.replace_range(colon_pos + 1..at_pos, "****");
            return masked;
        }
    }
    url.to_string()
}
