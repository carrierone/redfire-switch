//! Enhanced Database Connection Management and Patterns
//! Provides advanced PostgreSQL connectivity patterns, connection pooling,
//! transaction management, and monitoring capabilities for RedFire Switch

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Enhanced database connection pool with monitoring and health checks
#[derive(Debug, Clone)]
pub struct EnhancedDatabasePool {
    pub pool: PgPool,
    config: DatabaseConfig,
    stats: Arc<RwLock<ConnectionStats>>,
}

/// Database configuration for enhanced connectivity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub max_lifetime: Duration,
    pub idle_timeout: Duration,
    pub acquire_timeout: Duration,
    pub enable_health_checks: bool,
    pub health_check_interval: Duration,
    pub enable_query_logging: bool,
    pub slow_query_threshold: Duration,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            database_url: "postgres://localhost/redfire_switch".to_string(),
            max_connections: 20,
            min_connections: 5,
            max_lifetime: Duration::from_secs(30 * 60), // 30 minutes
            idle_timeout: Duration::from_secs(10 * 60), // 10 minutes
            acquire_timeout: Duration::from_secs(30),
            enable_health_checks: true,
            health_check_interval: Duration::from_secs(30),
            enable_query_logging: true,
            slow_query_threshold: Duration::from_millis(1000), // 1 second
        }
    }
}

/// Connection pool statistics and health metrics
#[derive(Debug, Default)]
pub struct ConnectionStats {
    pub total_connections: u32,
    pub active_connections: u32,
    pub idle_connections: u32,
    pub total_queries: u64,
    pub successful_queries: u64,
    pub failed_queries: u64,
    pub slow_queries: u64,
    pub average_query_time: Duration,
    pub last_health_check: Option<DateTime<Utc>>,
    pub consecutive_health_failures: u32,
    pub connection_pool_healthy: bool,
}

/// Database transaction wrapper with automatic rollback
pub struct DatabaseTransaction<'a> {
    transaction: Option<Transaction<'a, Postgres>>,
    committed: bool,
}

/// Database health check result
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub healthy: bool,
    pub response_time_ms: u64,
    pub active_connections: u32,
    pub total_connections: u32,
    pub last_error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl EnhancedDatabasePool {
    /// Create a new enhanced database pool with configuration
    pub async fn new(config: DatabaseConfig) -> Result<Self> {
        info!(
            "Creating enhanced database pool with {} max connections",
            config.max_connections
        );

        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .max_lifetime(config.max_lifetime)
            .idle_timeout(config.idle_timeout)
            .acquire_timeout(config.acquire_timeout)
            .before_acquire(|_conn, _meta| {
                Box::pin(async move {
                    debug!("Acquiring database connection");
                    Ok(true)
                })
            })
            .after_release(|_conn, _meta| {
                Box::pin(async move {
                    debug!("Released database connection");
                    Ok(true)
                })
            })
            .connect(&config.database_url)
            .await?;

        let enhanced_pool = Self {
            pool,
            config,
            stats: Arc::new(RwLock::new(ConnectionStats::default())),
        };

        // Start health check background task if enabled
        if enhanced_pool.config.enable_health_checks {
            enhanced_pool.start_health_check_task();
        }

        // Run initial database migrations/setup if needed
        enhanced_pool.ensure_database_schema().await?;

        info!("Enhanced database pool created successfully");
        Ok(enhanced_pool)
    }

    /// Create from simple database URL with defaults
    pub async fn from_url(database_url: &str) -> Result<Self> {
        let config = DatabaseConfig {
            database_url: database_url.to_string(),
            ..Default::default()
        };
        Self::new(config).await
    }

    /// Begin a new database transaction with automatic rollback
    pub async fn begin_transaction(&self) -> Result<DatabaseTransaction> {
        let start_time = Instant::now();

        let transaction = self.pool.begin().await?;

        self.record_query_stats(start_time, true).await;

        Ok(DatabaseTransaction {
            transaction: Some(transaction),
            committed: false,
        })
    }

    /// Execute a query with automatic stats tracking
    pub async fn execute_query<F, R>(&self, operation_name: &str, query_fn: F) -> Result<R>
    where
        F: FnOnce(&PgPool) -> Result<R>,
    {
        let start_time = Instant::now();

        debug!("Executing database operation: {}", operation_name);

        let result = query_fn(&self.pool);
        let success = result.is_ok();

        if let Err(ref e) = result {
            error!("Database operation '{}' failed: {}", operation_name, e);
        }

        self.record_query_stats(start_time, success).await;

        if self.config.enable_query_logging {
            let duration = start_time.elapsed();
            if duration >= self.config.slow_query_threshold {
                warn!(
                    "Slow query detected: '{}' took {:?}",
                    operation_name, duration
                );
                let mut stats = self.stats.write().await;
                stats.slow_queries += 1;
            }
        }

        result
    }

    /// Perform database health check
    pub async fn health_check(&self) -> HealthCheckResult {
        let start_time = Instant::now();
        let timestamp = Utc::now();

        let result = sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&self.pool)
            .await;

        let response_time_ms = start_time.elapsed().as_millis() as u64;
        let stats = self.stats.read().await;

        match result {
            Ok(_) => {
                debug!("Database health check passed ({} ms)", response_time_ms);
                HealthCheckResult {
                    healthy: true,
                    response_time_ms,
                    active_connections: stats.active_connections,
                    total_connections: stats.total_connections,
                    last_error: None,
                    timestamp,
                }
            }
            Err(e) => {
                error!("Database health check failed: {}", e);
                HealthCheckResult {
                    healthy: false,
                    response_time_ms,
                    active_connections: stats.active_connections,
                    total_connections: stats.total_connections,
                    last_error: Some(e.to_string()),
                    timestamp,
                }
            }
        }
    }

    /// Get current connection pool statistics
    pub async fn get_stats(&self) -> ConnectionStats {
        let mut stats = self.stats.write().await;

        // Update connection counts from pool
        stats.total_connections = self.pool.size();
        stats.active_connections = self.pool.size() - self.pool.num_idle() as u32;
        stats.idle_connections = self.pool.num_idle() as u32;

        // Clone the stats to return (need to manually implement the fields)
        ConnectionStats {
            total_connections: stats.total_connections,
            active_connections: stats.active_connections,
            idle_connections: stats.idle_connections,
            total_queries: stats.total_queries,
            successful_queries: stats.successful_queries,
            failed_queries: stats.failed_queries,
            slow_queries: stats.slow_queries,
            average_query_time: stats.average_query_time,
            last_health_check: stats.last_health_check,
            consecutive_health_failures: stats.consecutive_health_failures,
            connection_pool_healthy: stats.connection_pool_healthy,
        }
    }

    /// Close the database pool gracefully
    pub async fn close(&self) {
        info!("Closing enhanced database pool");
        self.pool.close().await;
    }

    /// Record query execution statistics
    async fn record_query_stats(&self, start_time: Instant, success: bool) {
        let duration = start_time.elapsed();
        let mut stats = self.stats.write().await;

        stats.total_queries += 1;
        if success {
            stats.successful_queries += 1;
        } else {
            stats.failed_queries += 1;
        }

        // Update average query time (simple moving average)
        if stats.total_queries == 1 {
            stats.average_query_time = duration;
        } else {
            let total_time = stats.average_query_time.as_nanos()
                * (stats.total_queries - 1) as u128
                + duration.as_nanos();
            stats.average_query_time =
                Duration::from_nanos((total_time / stats.total_queries as u128) as u64);
        }
    }

    /// Start background health check task
    fn start_health_check_task(&self) {
        let pool_clone = self.clone();
        let interval = self.config.health_check_interval;

        tokio::spawn(async move {
            let mut interval_timer = tokio::time::interval(interval);

            loop {
                interval_timer.tick().await;

                let health_result = pool_clone.health_check().await;
                let mut stats = pool_clone.stats.write().await;

                stats.last_health_check = Some(health_result.timestamp);

                if health_result.healthy {
                    stats.consecutive_health_failures = 0;
                    stats.connection_pool_healthy = true;
                } else {
                    stats.consecutive_health_failures += 1;
                    if stats.consecutive_health_failures >= 3 {
                        stats.connection_pool_healthy = false;
                        error!(
                            "Database pool marked unhealthy after {} consecutive failures",
                            stats.consecutive_health_failures
                        );
                    }
                }
            }
        });
    }

    /// Ensure database schema is properly initialized
    async fn ensure_database_schema(&self) -> Result<()> {
        debug!("Ensuring database schema is initialized");

        // Check if our monitoring table exists, create if not
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS database_connection_log (
                id SERIAL PRIMARY KEY,
                event_type VARCHAR(50) NOT NULL,
                connection_count INTEGER,
                query_count BIGINT,
                error_message TEXT,
                created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create indexes for performance
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_db_conn_log_created_at 
            ON database_connection_log(created_at);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Log database connection events
    pub async fn log_connection_event(
        &self,
        event_type: &str,
        error_message: Option<&str>,
    ) -> Result<()> {
        let stats = self.get_stats().await;

        sqlx::query(
            r#"
            INSERT INTO database_connection_log (event_type, connection_count, query_count, error_message)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(event_type)
        .bind(stats.total_connections as i32)
        .bind(stats.total_queries as i64)
        .bind(error_message)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get recent connection events for monitoring
    pub async fn get_recent_connection_events(&self, limit: i64) -> Result<Vec<ConnectionEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT event_type, connection_count, query_count, error_message, created_at
            FROM database_connection_log
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let events = rows
            .into_iter()
            .map(|row| ConnectionEvent {
                event_type: row.get("event_type"),
                connection_count: row.get::<Option<i32>, _>("connection_count").unwrap_or(0) as u32,
                query_count: row.get::<Option<i64>, _>("query_count").unwrap_or(0) as u64,
                error_message: row.get("error_message"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(events)
    }
}

impl<'a> DatabaseTransaction<'a> {
    /// Commit the transaction
    pub async fn commit(mut self) -> Result<()> {
        if let Some(tx) = self.transaction.take() {
            tx.commit().await?;
            self.committed = true;
        }
        Ok(())
    }

    /// Rollback the transaction manually
    pub async fn rollback(mut self) -> Result<()> {
        if let Some(tx) = self.transaction.take() {
            tx.rollback().await?;
        }
        Ok(())
    }

    /// Execute a query within the transaction
    pub async fn execute_in_transaction<F, R>(&mut self, query_fn: F) -> Result<R>
    where
        F: FnOnce(&mut Transaction<'_, Postgres>) -> Result<R>,
    {
        if let Some(ref mut tx) = self.transaction {
            query_fn(tx)
        } else {
            Err(anyhow!(
                "Transaction has already been committed or rolled back"
            ))
        }
    }
}

impl<'a> Drop for DatabaseTransaction<'a> {
    fn drop(&mut self) {
        if !self.committed && self.transaction.is_some() {
            // Transaction will be automatically rolled back when dropped
            warn!("Database transaction was dropped without explicit commit - rolling back");
        }
    }
}

/// Database connection event for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionEvent {
    pub event_type: String,
    pub connection_count: u32,
    pub query_count: u64,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Database connection pool builder for easier configuration
#[derive(Debug, Default)]
pub struct DatabasePoolBuilder {
    config: DatabaseConfig,
}

impl DatabasePoolBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn database_url<S: Into<String>>(mut self, url: S) -> Self {
        self.config.database_url = url.into();
        self
    }

    pub fn max_connections(mut self, max: u32) -> Self {
        self.config.max_connections = max;
        self
    }

    pub fn min_connections(mut self, min: u32) -> Self {
        self.config.min_connections = min;
        self
    }

    pub fn max_lifetime(mut self, lifetime: Duration) -> Self {
        self.config.max_lifetime = lifetime;
        self
    }

    pub fn idle_timeout(mut self, timeout: Duration) -> Self {
        self.config.idle_timeout = timeout;
        self
    }

    pub fn enable_health_checks(mut self, enabled: bool) -> Self {
        self.config.enable_health_checks = enabled;
        self
    }

    pub fn health_check_interval(mut self, interval: Duration) -> Self {
        self.config.health_check_interval = interval;
        self
    }

    pub fn enable_query_logging(mut self, enabled: bool) -> Self {
        self.config.enable_query_logging = enabled;
        self
    }

    pub fn slow_query_threshold(mut self, threshold: Duration) -> Self {
        self.config.slow_query_threshold = threshold;
        self
    }

    pub async fn build(self) -> Result<EnhancedDatabasePool> {
        EnhancedDatabasePool::new(self.config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires a PostgreSQL database
    async fn test_enhanced_database_pool() {
        let pool = DatabasePoolBuilder::new()
            .database_url("postgres://postgres:password@localhost/test_redfire")
            .max_connections(5)
            .enable_health_checks(false) // Disable for testing
            .build()
            .await
            .expect("Failed to create database pool");

        // Test health check
        let health = pool.health_check().await;
        assert!(health.healthy);

        // Test stats
        let stats = pool.get_stats().await;
        assert!(stats.total_connections > 0);

        // Test transaction
        let mut tx = pool
            .begin_transaction()
            .await
            .expect("Failed to begin transaction");
        tx.commit().await.expect("Failed to commit transaction");

        pool.close().await;
    }

    #[test]
    fn test_database_pool_builder() {
        let builder = DatabasePoolBuilder::new()
            .database_url("postgres://localhost/test")
            .max_connections(10)
            .min_connections(2)
            .enable_health_checks(true);

        assert_eq!(builder.config.database_url, "postgres://localhost/test");
        assert_eq!(builder.config.max_connections, 10);
        assert_eq!(builder.config.min_connections, 2);
        assert!(builder.config.enable_health_checks);
    }
}
