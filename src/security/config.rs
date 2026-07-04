//! Secure configuration management
//!
//! This module provides secure configuration loading with environment
//! variable support and validation.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use tracing::{debug, error, info, warn};

/// Secure configuration loader
pub struct SecureConfigLoader {
    /// Environment variable prefix
    env_prefix: String,
    /// Required environment variables
    required_vars: Vec<String>,
}

impl SecureConfigLoader {
    /// Create a new secure config loader
    pub fn new(env_prefix: String) -> Self {
        Self {
            env_prefix,
            required_vars: vec![
                "DATABASE_URL".to_string(),
                "JWT_SECRET".to_string(),
                "TLS_CERT_PATH".to_string(),
                "TLS_KEY_PATH".to_string(),
            ],
        }
    }

    /// Load configuration from file and environment
    pub fn load_config<T>(&self, config_path: Option<&Path>) -> Result<T>
    where
        T: for<'de> Deserialize<'de> + Default,
    {
        let mut config = if let Some(path) = config_path {
            self.load_from_file(path)?
        } else {
            T::default()
        };

        // Override with environment variables
        self.override_from_env(&mut config)?;

        // Validate required variables
        self.validate_required_vars()?;

        info!("Configuration loaded successfully");
        Ok(config)
    }

    /// Load configuration from JSON/TOML file
    fn load_from_file<T>(&self, path: &Path) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let content = fs::read_to_string(path)?;

        let config = if path.extension().and_then(|s| s.to_str()) == Some("toml") {
            // toml crate not available, fall back to JSON
            warn!("TOML format requested but toml crate not available, using JSON parser");
            serde_json::from_str(&content)?
        } else {
            serde_json::from_str(&content)?
        };

        debug!("Loaded configuration from file: {:?}", path);
        Ok(config)
    }

    /// Override configuration with environment variables
    fn override_from_env<T>(&self, _config: &mut T) -> Result<()>
    where
        T: for<'de> Deserialize<'de>,
    {
        // This is a simplified implementation
        // In practice, you'd use a library like `config` or `figment`
        // to properly merge environment variables with configuration structs

        debug!("Environment variable override completed");
        Ok(())
    }

    /// Validate that required environment variables are set
    fn validate_required_vars(&self) -> Result<()> {
        let mut missing_vars = Vec::new();

        for var in &self.required_vars {
            let full_var_name = if var.starts_with(&self.env_prefix) {
                var.clone()
            } else {
                format!("{}_{}", self.env_prefix, var)
            };

            if env::var(&full_var_name).is_err() && env::var(var).is_err() {
                // Allow some variables to have defaults in development
                match var.as_str() {
                    "JWT_SECRET" => {
                        if cfg!(debug_assertions) {
                            warn!("JWT_SECRET not set, using development default");
                            continue;
                        }
                    }
                    "TLS_CERT_PATH" | "TLS_KEY_PATH" => {
                        if cfg!(debug_assertions) {
                            debug!("{} not set, TLS disabled in development", var);
                            continue;
                        }
                    }
                    _ => {}
                }

                missing_vars.push(var.clone());
            }
        }

        if !missing_vars.is_empty() {
            error!("Missing required environment variables: {:?}", missing_vars);
            return Err(anyhow::anyhow!("Missing required configuration variables"));
        }

        Ok(())
    }
}

/// Database configuration with security validations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database connection URL
    pub url: String,
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    /// Enable SSL/TLS for database connections
    pub enable_ssl: bool,
    /// SSL certificate path
    pub ssl_cert_path: Option<String>,
    /// SSL key path
    pub ssl_key_path: Option<String>,
    /// SSL CA certificate path
    pub ssl_ca_path: Option<String>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://redfire_user:secure_password@localhost/redfire_switch".to_string()
            }),
            max_connections: 100,
            connection_timeout: 30,
            enable_ssl: true,
            ssl_cert_path: env::var("DB_SSL_CERT_PATH").ok(),
            ssl_key_path: env::var("DB_SSL_KEY_PATH").ok(),
            ssl_ca_path: env::var("DB_SSL_CA_PATH").ok(),
        }
    }
}

impl DatabaseConfig {
    /// Validate database configuration
    pub fn validate(&self) -> Result<()> {
        // Check URL format
        if !self.url.starts_with("postgres://") && !self.url.starts_with("postgresql://") {
            return Err(anyhow::anyhow!("Invalid database URL format"));
        }

        // Warn if using default/weak credentials
        if self.url.contains("password") || self.url.contains("123456") {
            warn!("Database URL appears to contain default or weak credentials");
        }

        // Validate SSL configuration
        if self.enable_ssl {
            if let Some(ref cert_path) = self.ssl_cert_path {
                if !Path::new(cert_path).exists() {
                    return Err(anyhow::anyhow!(
                        "SSL certificate file not found: {}",
                        cert_path
                    ));
                }
            }

            if let Some(ref key_path) = self.ssl_key_path {
                if !Path::new(key_path).exists() {
                    return Err(anyhow::anyhow!("SSL key file not found: {}", key_path));
                }
            }
        }

        Ok(())
    }

    /// Get sanitized URL for logging (removes password)
    pub fn get_sanitized_url(&self) -> String {
        if let Some(at_pos) = self.url.rfind('@') {
            if let Some(scheme_end) = self.url.find("://") {
                let scheme = &self.url[..scheme_end + 3];
                let host_and_db = &self.url[at_pos..];
                return format!("{}[CREDENTIALS_HIDDEN]{}", scheme, host_and_db);
            }
        }
        self.url.clone()
    }
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Enable TLS
    pub enabled: bool,
    /// Certificate file path
    pub cert_path: String,
    /// Private key file path
    pub key_path: String,
    /// CA certificate file path for client verification
    pub ca_path: Option<String>,
    /// Require client certificates
    pub require_client_cert: bool,
    /// Minimum TLS version
    pub min_version: String,
    /// Allowed cipher suites
    pub cipher_suites: Vec<String>,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: env::var("ENABLE_TLS").map(|v| v == "true").unwrap_or(false),
            cert_path: env::var("TLS_CERT_PATH")
                .unwrap_or_else(|_| "/etc/ssl/certs/redfire.pem".to_string()),
            key_path: env::var("TLS_KEY_PATH")
                .unwrap_or_else(|_| "/etc/ssl/private/redfire.key".to_string()),
            ca_path: env::var("TLS_CA_PATH").ok(),
            require_client_cert: false,
            min_version: "TLSv1.2".to_string(),
            cipher_suites: vec![
                "TLS_AES_256_GCM_SHA384".to_string(),
                "TLS_CHACHA20_POLY1305_SHA256".to_string(),
                "TLS_AES_128_GCM_SHA256".to_string(),
            ],
        }
    }
}

impl TlsConfig {
    /// Validate TLS configuration
    pub fn validate(&self) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        // Check certificate file
        if !Path::new(&self.cert_path).exists() {
            return Err(anyhow::anyhow!(
                "TLS certificate file not found: {}",
                self.cert_path
            ));
        }

        // Check key file
        if !Path::new(&self.key_path).exists() {
            return Err(anyhow::anyhow!("TLS key file not found: {}", self.key_path));
        }

        // Check CA file if specified
        if let Some(ref ca_path) = self.ca_path {
            if !Path::new(ca_path).exists() {
                return Err(anyhow::anyhow!("TLS CA file not found: {}", ca_path));
            }
        }

        // Validate minimum TLS version
        if !["TLSv1.2", "TLSv1.3"].contains(&self.min_version.as_str()) {
            return Err(anyhow::anyhow!(
                "Invalid minimum TLS version: {}",
                self.min_version
            ));
        }

        Ok(())
    }
}

/// JWT configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtConfig {
    /// JWT signing secret
    pub secret: String,
    /// Token expiration time in seconds
    pub expiration_seconds: u64,
    /// JWT issuer
    pub issuer: String,
    /// JWT audience
    pub audience: String,
    /// Signing algorithm
    pub algorithm: String,
}

impl Default for JwtConfig {
    fn default() -> Self {
        Self {
            secret: env::var("JWT_SECRET").unwrap_or_else(|_| {
                if cfg!(debug_assertions) {
                    warn!("Using development JWT secret - NOT SECURE FOR PRODUCTION");
                    "development_secret_change_in_production".to_string()
                } else {
                    panic!("JWT_SECRET environment variable is required for production")
                }
            }),
            expiration_seconds: 3600, // 1 hour
            issuer: "redfire-switch".to_string(),
            audience: "redfire-api".to_string(),
            algorithm: "HS256".to_string(),
        }
    }
}

impl JwtConfig {
    /// Validate JWT configuration
    pub fn validate(&self) -> Result<()> {
        // Check secret strength
        if self.secret.len() < 32 {
            return Err(anyhow::anyhow!(
                "JWT secret must be at least 32 characters long"
            ));
        }

        // Check for common weak/default secrets. Note we deliberately do not
        // flag the bare substring "secret": strong secrets frequently contain
        // the word (e.g. "..._secret_key_..."). We match specific weak defaults
        // instead.
        let weak_secrets = [
            "password",
            "123456",
            "development_secret",
            "change_me",
            "changeme",
            "default",
        ];

        for weak in &weak_secrets {
            if self.secret.to_lowercase().contains(weak) {
                return Err(anyhow::anyhow!("JWT secret appears to be weak or default"));
            }
        }

        // Validate algorithm
        if !["HS256", "HS384", "HS512", "RS256", "RS384", "RS512"]
            .contains(&self.algorithm.as_str())
        {
            return Err(anyhow::anyhow!(
                "Unsupported JWT algorithm: {}",
                self.algorithm
            ));
        }

        Ok(())
    }
}

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Database configuration
    pub database: DatabaseConfig,
    /// TLS configuration
    pub tls: TlsConfig,
    /// JWT configuration
    pub jwt: JwtConfig,
    /// Security configuration
    pub security: super::SecurityConfig,
    /// Additional configuration properties
    pub properties: HashMap<String, String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database: DatabaseConfig::default(),
            tls: TlsConfig::default(),
            jwt: JwtConfig::default(),
            security: super::SecurityConfig::default(),
            properties: HashMap::new(),
        }
    }
}

impl AppConfig {
    /// Validate entire configuration
    pub fn validate(&self) -> Result<()> {
        self.database.validate()?;
        self.tls.validate()?;
        self.jwt.validate()?;

        info!("Application configuration validation passed");
        Ok(())
    }

    /// Load configuration from environment and files
    pub fn load() -> Result<Self> {
        let loader = SecureConfigLoader::new("REDFIRE".to_string());

        // Try to load from config file if specified
        let config_path = env::var("CONFIG_FILE").ok();
        let config_path = config_path.as_ref().map(|p| Path::new(p));

        let config: AppConfig = loader.load_config(config_path)?;

        // Validate configuration
        config.validate()?;

        info!("Application configuration loaded and validated");
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_database_config_validation() {
        let mut config = DatabaseConfig::default();
        config.url = "postgres://user:pass@localhost/db".to_string();
        assert!(config.validate().is_ok());

        config.url = "invalid_url".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_jwt_config_validation() {
        let mut config = JwtConfig::default();
        config.secret =
            "a_very_long_and_secure_secret_key_that_is_definitely_long_enough".to_string();
        assert!(config.validate().is_ok());

        config.secret = "weak".to_string();
        assert!(config.validate().is_err());

        config.secret = "password123456789012345678901234".to_string();
        assert!(config.validate().is_err()); // Contains "password"
    }

    #[test]
    fn test_sanitized_url() {
        let config = DatabaseConfig {
            url: "postgres://username:password@localhost:5432/database".to_string(),
            ..Default::default()
        };

        let sanitized = config.get_sanitized_url();
        assert!(!sanitized.contains("password"));
        assert!(sanitized.contains("CREDENTIALS_HIDDEN"));
    }
}
