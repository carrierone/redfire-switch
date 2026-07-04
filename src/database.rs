//! Database service - Production implementation
//! Comprehensive database integration with connection pooling, migrations, and monitoring

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres, Row};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
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
pub struct DatabaseStatistics {
    pub total_queries: u64,
    pub successful_queries: u64,
    pub failed_queries: u64,
    pub connection_errors: u64,
    pub average_response_time_ms: f64,
    pub active_connections: u32,
    pub idle_connections: u32,
}

impl DatabaseService {
    /// Database connection URL this service was configured with.
    pub fn database_url(&self) -> &str {
        &self.config.url
    }

    /// Shared connection pool.
    pub fn pool(&self) -> &Pool<Postgres> {
        &self.pool
    }

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

    /// Ensure the full schema (all migrations) is applied to the database at
    /// `url`, returning a live pool. This is the canonical entry point for
    /// integration tests that talk to a shared PostgreSQL instance: it applies
    /// migrations 001 (core) and 002 (LCR) idempotently, so a fresh or partially
    /// provisioned database ends up with every table the code expects.
    pub async fn provision_schema(url: &str) -> Result<Pool<Postgres>> {
        let config = DatabaseConfig {
            url: url.to_string(),
            // Keep the pool small; tests just need the schema in place.
            max_connections: 5,
            min_connections: 1,
            auto_migrate: true,
            ..DatabaseConfig::default()
        };
        let service = Self::new(config).await?;
        Ok(service.pool().clone())
    }

    async fn run_migrations(&self) -> Result<()> {
        info!("Running database migrations...");

        // Serialize migration application across processes/connections with a
        // Postgres advisory lock. Without this, multiple switch instances (or
        // parallel integration tests) racing to CREATE the same types/tables hit
        // duplicate-key errors on the system catalogs. The lock key is an
        // arbitrary constant shared by all redfire instances.
        //
        // Advisory session locks are per-connection, so we hold one dedicated
        // connection for the entire migration sequence and lock/unlock on it.
        const MIGRATION_LOCK_KEY: i64 = 0x5245_4446_4952_4501; // "REDFIRE\x01"
        let mut lock_conn = self.pool.acquire().await?;
        sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(MIGRATION_LOCK_KEY)
            .execute(&mut *lock_conn)
            .await?;

        let result = self.run_migrations_locked().await;

        // Always release the lock on the same connection, even on failure.
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(MIGRATION_LOCK_KEY)
            .execute(&mut *lock_conn)
            .await;

        result
    }

    async fn run_migrations_locked(&self) -> Result<()> {
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

        // Apply migrations in order. Each entry is (version, sql). Adding a new
        // migration is a matter of appending to this list; already-applied
        // versions are skipped via the schema_migrations bookkeeping table.
        const MIGRATIONS: &[(&str, &str)] = &[
            (
                "001_initial_schema",
                include_str!("../migrations/001_initial_schema.sql"),
            ),
            (
                "002_lcr_schema",
                include_str!("../migrations/002_lcr_schema.sql"),
            ),
            (
                "003_anti_fraud_monitoring",
                include_str!("../migrations/003_anti_fraud_monitoring.sql"),
            ),
        ];

        for (version, migration_sql) in MIGRATIONS {
            let already_applied = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM schema_migrations WHERE version = $1)",
            )
            .bind(version)
            .fetch_one(&self.pool)
            .await?;

            if already_applied {
                continue;
            }

            // Execute the migration in a transaction so a failure leaves no
            // partially-applied schema behind.
            let mut tx = self.pool.begin().await?;

            for statement in split_sql_statements(migration_sql) {
                let statement = statement.trim();
                if !statement.is_empty() && !statement.starts_with("--") {
                    sqlx::query(statement).execute(&mut *tx).await.map_err(|e| {
                        anyhow!(
                            "Migration {} failed at statement '{}': {}",
                            version,
                            statement,
                            e
                        )
                    })?;
                }
            }

            sqlx::query("INSERT INTO schema_migrations (version) VALUES ($1)")
                .bind(version)
                .execute(&mut *tx)
                .await?;

            tx.commit().await?;
            info!("Applied migration {}", version);
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

        let result = sqlx::query(
            r#"
            INSERT INTO call_detail_records (
                id, call_id, session_id, from_number, to_number, from_ip, to_ip,
                start_time, end_time, duration_seconds, disposition, hangup_cause,
                trunk_id, route_id, codec_in, codec_out, recording_enabled, cost
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
            "#,
        )
        .bind(cdr.id.as_ref().unwrap_or(&uuid::Uuid::new_v4().to_string()))
        .bind(&cdr.call_id)
        .bind(&cdr.session_id)
        .bind(&cdr.from_number)
        .bind(&cdr.to_number)
        .bind(cdr.from_ip.map(|ip| ip.to_string()))
        .bind(cdr.to_ip.map(|ip| ip.to_string()))
        .bind(cdr.start_time)
        .bind(cdr.end_time)
        .bind(cdr.duration_seconds as i64)
        .bind(serde_json::to_string(&cdr.disposition)?)
        .bind(cdr.hangup_cause.map(|c| c as i32))
        .bind(&cdr.trunk_id)
        .bind(&cdr.route_id)
        .bind(&cdr.codec_in)
        .bind(&cdr.codec_out)
        .bind(cdr.recording_enabled)
        .bind(cdr.cost)
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

        let result = sqlx::query(
            r#"
            INSERT INTO active_sessions (
                id, call_id, session_id, from_number, to_number, from_ip, to_ip,
                trunk_id, start_time, last_activity, state, codec_in, codec_out
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            "#,
        )
        .bind(session.id)
        .bind(session.call_id.as_str())
        .bind(Some(session.id)) // Using session.id as session_id
        .bind("unknown") // TODO: Extract from session
        .bind("unknown") // TODO: Extract from session
        .bind(Some(session.from_addr.to_string()))
        .bind(Some(session.to_addr.to_string()))
        .bind(
            session
                .trunk_id
                .as_ref()
                .and_then(|t| t.parse::<i32>().ok()),
        )
        .bind(session.start_time)
        .bind(session.last_activity)
        .bind(state_str)
        .bind(
            session
                .codec_pair
                .as_ref()
                .map(|(in_codec, _)| in_codec.as_str()),
        )
        .bind(
            session
                .codec_pair
                .as_ref()
                .map(|(_, out_codec)| out_codec.as_str()),
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

        let result = sqlx::query("DELETE FROM active_sessions WHERE id = $1")
            .bind(session_id)
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

    /// Insert security event
    pub async fn insert_security_event(&self, event: &SecurityEvent) -> Result<()> {
        let start_time = std::time::Instant::now();

        let result = sqlx::query(
            r#"
            INSERT INTO security_events (
                id, event_type, source_ip, severity, description, details, action_taken
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(event.id)
        .bind(&event.event_type)
        .bind(event.source_ip.to_string())
        .bind(&event.severity)
        .bind(&event.description)
        .bind(serde_json::to_value(&event.details)?)
        .bind(&event.action_taken)
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

        let query_result = sqlx::query(
            r#"
            INSERT INTO health_check_results (id, component, status, response_time_ms, details)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(result.id)
        .bind(&result.component)
        .bind(&result.status)
        .bind(result.response_time_ms)
        .bind(serde_json::to_value(&result.details)?)
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

        let result = sqlx::query("SELECT config_value FROM system_config WHERE config_key = $1")
            .bind(key)
            .fetch_optional(&self.pool)
            .await;

        self.update_query_statistics(start_time, result.is_ok())
            .await;

        match result {
            Ok(Some(row)) => {
                // Parse the JSONB value from database
                if let Ok(config_value) = row.try_get::<String, _>("config_value") {
                    match serde_json::from_str::<serde_json::Value>(&config_value) {
                        Ok(value) => Ok(Some(value)),
                        Err(e) => {
                            error!("Failed to parse config value for key '{}': {}", key, e);
                            Err(anyhow!("Failed to parse JSON config value: {}", e))
                        }
                    }
                } else {
                    error!("Failed to get config_value column for key '{}'", key);
                    Err(anyhow!("Column access failed"))
                }
            }
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

        let result = sqlx::query(
            r#"
            INSERT INTO system_config (config_key, config_value, config_type)
            VALUES ($1, $2, 'dynamic')
            ON CONFLICT (config_key)
            DO UPDATE SET config_value = $2, updated_at = NOW()
            "#,
        )
        .bind(key)
        .bind(value)
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

    /// Shutdown the database service gracefully
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down database service");

        // Close the connection pool
        self.pool.close().await;

        info!("Database service shutdown completed");
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseHealthStatus {
    pub connected: bool,
    pub response_time_ms: f64,
    pub active_connections: u32,
    pub error: Option<String>,
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

/// Split a SQL script into individual statements on top-level semicolons,
/// respecting PostgreSQL dollar-quoted string bodies (e.g. `$$ ... $$` used by
/// plpgsql function definitions), single-quoted strings, and line comments.
///
/// A naive `split(';')` corrupts function bodies whose statements contain
/// semicolons; this splitter keeps such bodies intact.
fn split_sql_statements(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut i = 0;
    let mut in_single_quote = false;
    let mut dollar_tag: Option<String> = None;

    while i < bytes.len() {
        let c = bytes[i] as char;

        // Inside a dollar-quoted body, look only for the matching close tag.
        if let Some(tag) = &dollar_tag {
            if sql[i..].starts_with(tag.as_str()) {
                current.push_str(tag);
                i += tag.len();
                dollar_tag = None;
                continue;
            }
            current.push(c);
            i += 1;
            continue;
        }

        if in_single_quote {
            current.push(c);
            i += 1;
            if c == '\'' {
                in_single_quote = false;
            }
            continue;
        }

        // Line comment: consume through end of line without copying it, so a
        // statement is never left with a leading comment (which callers would
        // otherwise mistake for a comment-only, skippable fragment).
        if c == '-' && i + 1 < bytes.len() && bytes[i + 1] as char == '-' {
            while i < bytes.len() && bytes[i] as char != '\n' {
                i += 1;
            }
            continue;
        }

        // Start of a dollar-quote tag like `$$` or `$body$`.
        if c == '$' {
            if let Some(end) = sql[i + 1..].find('$') {
                let candidate = &sql[i..i + 1 + end + 1]; // includes both '$'
                if candidate[1..candidate.len() - 1]
                    .chars()
                    .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
                {
                    dollar_tag = Some(candidate.to_string());
                    current.push_str(candidate);
                    i += candidate.len();
                    continue;
                }
            }
        }

        if c == '\'' {
            in_single_quote = true;
            current.push(c);
            i += 1;
            continue;
        }

        if c == ';' {
            statements.push(current.clone());
            current.clear();
            i += 1;
            continue;
        }

        current.push(c);
        i += 1;
    }

    if !current.trim().is_empty() {
        statements.push(current);
    }

    statements
}

#[cfg(test)]
mod migration_split_tests {
    use super::split_sql_statements;

    #[test]
    fn splits_simple_statements() {
        let sql = "CREATE TABLE a (id int); CREATE TABLE b (id int);";
        let stmts: Vec<String> = split_sql_statements(sql)
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn keeps_dollar_quoted_function_body_intact() {
        let sql = "CREATE FUNCTION f() RETURNS trigger AS $$\nBEGIN\n  NEW.x = NOW();\n  RETURN NEW;\nEND;\n$$ language 'plpgsql';\nCREATE TABLE t (id int);";
        let stmts: Vec<String> = split_sql_statements(sql)
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(stmts.len(), 2, "function body must not be split");
        assert!(stmts[0].contains("RETURN NEW;"));
        assert!(stmts[0].contains("NEW.x = NOW();"));
    }

    #[test]
    fn ignores_semicolons_in_single_quoted_strings() {
        let sql = "INSERT INTO t VALUES ('a;b'); SELECT 1;";
        let stmts: Vec<String> = split_sql_statements(sql)
            .into_iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].contains("'a;b'"));
    }
}
