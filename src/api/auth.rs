/*
 * Redfire Switch - Authentication and Authorization System
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
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use chrono::{DateTime, Duration, Utc};
use hex;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use ring::digest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Permission {
    // System management
    SystemRead,
    SystemWrite,
    SystemAdmin,

    // Call management
    CallsRead,
    CallsWrite,
    CallsHangup,

    // Configuration
    ConfigRead,
    ConfigWrite,
    ConfigReload,

    // Monitoring
    MonitoringRead,
    MonitoringWrite,

    // Customer management
    CustomerRead,
    CustomerWrite,
    CustomerAdmin,

    // DID management
    DidRead,
    DidWrite,
    DidAdmin,

    // SMS management
    SmsRead,
    SmsWrite,
    SmsAdmin,

    // Routing
    RoutingRead,
    RoutingWrite,
    RoutingAdmin,

    // Billing
    BillingRead,
    BillingWrite,
    BillingAdmin,

    // Security
    SecurityRead,
    SecurityWrite,
    SecurityAdmin,

    // API access
    ApiRead,
    ApiWrite,
    ApiAdmin,

    // Voice Integrity and Lawful Intercept
    VoiceIntegrityRead,
    VoiceIntegrityWrite,
    VoiceIntegrityAdmin,
    LawfulInterceptManage,
    LegalAuthorizationIssue,
    LegalAuthorizationReview,
    RecordingAccess,
    RecordingDownload,
    RecordingLegalHold,
    TranscriptionAccess,
    ComplianceReportGenerate,
    ComplianceAuditAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub description: String,
    pub permissions: Vec<Permission>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub salt: String,
    pub roles: Vec<String>, // Role IDs
    pub enabled: bool,
    pub last_login: Option<DateTime<Utc>>,
    pub failed_login_attempts: u32,
    pub locked_until: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthClaims {
    pub sub: String, // User ID
    pub username: String,
    pub roles: Vec<String>,
    pub permissions: Vec<Permission>,
    pub exp: usize,  // Expiration time
    pub iat: usize,  // Issued at
    pub jti: String, // JWT ID
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
    pub max_failed_attempts: u32,
    pub lockout_duration_minutes: i64,
    pub session_timeout_minutes: i64,
    pub require_mfa: bool,
    pub password_min_length: u32,
    pub password_require_special: bool,
    pub api_rate_limit_per_minute: u32,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: Uuid::new_v4().to_string(), // Generate random secret - should be configured
            jwt_expiration_hours: 8,
            max_failed_attempts: 5,
            lockout_duration_minutes: 30,
            session_timeout_minutes: 60,
            require_mfa: false,
            password_min_length: 8,
            password_require_special: true,
            api_rate_limit_per_minute: 100,
        }
    }
}

#[derive(Clone)]
pub struct AuthState {
    pub config: AuthConfig,
    pub users: Arc<RwLock<HashMap<String, User>>>,
    pub roles: Arc<RwLock<HashMap<String, Role>>>,
    pub active_sessions: Arc<RwLock<HashMap<String, AuthSession>>>,
    pub encoding_key: EncodingKey,
    pub decoding_key: DecodingKey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub user_id: String,
    pub jwt_id: String,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

impl std::fmt::Debug for AuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthState")
            .field("config", &self.config)
            .field("users", &"<HashMap>")
            .field("roles", &"<HashMap>")
            .field("active_sessions", &"<HashMap>")
            .field("encoding_key", &"<EncodingKey>")
            .field("decoding_key", &"<DecodingKey>")
            .finish()
    }
}

impl AuthState {
    pub fn new(config: AuthConfig) -> Self {
        let encoding_key = EncodingKey::from_secret(config.jwt_secret.as_bytes());
        let decoding_key = DecodingKey::from_secret(config.jwt_secret.as_bytes());

        let auth_state = Self {
            config,
            users: Arc::new(RwLock::new(HashMap::new())),
            roles: Arc::new(RwLock::new(HashMap::new())),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            encoding_key,
            decoding_key,
        };

        // Note: Default roles and admin user will be initialized on first use
        // to avoid async initialization issues in constructor

        auth_state
    }

    async fn initialize_defaults(&self) -> Result<()> {
        self.create_default_roles().await?;
        self.create_admin_user().await?;
        Ok(())
    }

    async fn create_default_roles(&self) -> Result<()> {
        let mut roles = self.roles.write().await;

        // Admin role - full access
        let admin_role = Role {
            id: "admin".to_string(),
            name: "Administrator".to_string(),
            description: "Full system access".to_string(),
            permissions: vec![
                Permission::SystemAdmin,
                Permission::CallsWrite,
                Permission::ConfigWrite,
                Permission::MonitoringWrite,
                Permission::CustomerAdmin,
                Permission::DidAdmin,
                Permission::SmsAdmin,
                Permission::RoutingAdmin,
                Permission::BillingAdmin,
                Permission::SecurityAdmin,
                Permission::ApiAdmin,
                Permission::VoiceIntegrityAdmin,
                Permission::LawfulInterceptManage,
                Permission::LegalAuthorizationIssue,
                Permission::LegalAuthorizationReview,
                Permission::RecordingAccess,
                Permission::RecordingDownload,
                Permission::RecordingLegalHold,
                Permission::TranscriptionAccess,
                Permission::ComplianceReportGenerate,
                Permission::ComplianceAuditAccess,
            ],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Operator role - call and system monitoring
        let operator_role = Role {
            id: "operator".to_string(),
            name: "Operator".to_string(),
            description: "Call management and monitoring".to_string(),
            permissions: vec![
                Permission::SystemRead,
                Permission::CallsWrite,
                Permission::CallsHangup,
                Permission::MonitoringRead,
                Permission::CustomerRead,
                Permission::DidRead,
                Permission::SmsRead,
                Permission::ApiRead,
            ],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Voice Integrity Officer role - specialized legal/compliance role
        let voice_integrity_role = Role {
            id: "voice_integrity_officer".to_string(),
            name: "Voice Integrity Officer".to_string(),
            description: "Legal authorization and lawful intercept management".to_string(),
            permissions: vec![
                Permission::SystemRead,
                Permission::MonitoringRead,
                Permission::VoiceIntegrityAdmin,
                Permission::LawfulInterceptManage,
                Permission::LegalAuthorizationIssue,
                Permission::LegalAuthorizationReview,
                Permission::RecordingAccess,
                Permission::RecordingDownload,
                Permission::RecordingLegalHold,
                Permission::TranscriptionAccess,
                Permission::ComplianceReportGenerate,
                Permission::ComplianceAuditAccess,
            ],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Compliance Officer role - audit and oversight
        let compliance_officer_role = Role {
            id: "compliance_officer".to_string(),
            name: "Compliance Officer".to_string(),
            description: "Compliance monitoring and audit oversight".to_string(),
            permissions: vec![
                Permission::SystemRead,
                Permission::MonitoringRead,
                Permission::VoiceIntegrityRead,
                Permission::LegalAuthorizationReview,
                Permission::RecordingAccess,
                Permission::TranscriptionAccess,
                Permission::ComplianceReportGenerate,
                Permission::ComplianceAuditAccess,
            ],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Read-only role - monitoring and reporting
        let readonly_role = Role {
            id: "readonly".to_string(),
            name: "Read Only".to_string(),
            description: "Read-only access for monitoring".to_string(),
            permissions: vec![
                Permission::SystemRead,
                Permission::CallsRead,
                Permission::MonitoringRead,
                Permission::CustomerRead,
                Permission::DidRead,
                Permission::SmsRead,
                Permission::RoutingRead,
                Permission::BillingRead,
                Permission::ApiRead,
                Permission::VoiceIntegrityRead,
                Permission::RecordingAccess,
                Permission::TranscriptionAccess,
            ],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        roles.insert("admin".to_string(), admin_role);
        roles.insert("operator".to_string(), operator_role);
        roles.insert("voice_integrity_officer".to_string(), voice_integrity_role);
        roles.insert("compliance_officer".to_string(), compliance_officer_role);
        roles.insert("readonly".to_string(), readonly_role);

        Ok(())
    }

    async fn create_admin_user(&self) -> Result<()> {
        // Check first with read lock to avoid unnecessary write lock
        {
            let users = self.users.read().await;
            if users
                .values()
                .any(|u| u.roles.contains(&"admin".to_string()))
            {
                return Ok(());
            }
        }

        let mut users = self.users.write().await;

        // Double-check after acquiring write lock to prevent race conditions
        if users
            .values()
            .any(|u| u.roles.contains(&"admin".to_string()))
        {
            return Ok(());
        }

        let salt = Uuid::new_v4().to_string();
        let password_hash = self.hash_password("admin123", &salt)?;

        let admin_user = User {
            id: "admin".to_string(),
            username: "admin".to_string(),
            email: "admin@redfire-switch.local".to_string(),
            password_hash,
            salt,
            roles: vec!["admin".to_string()],
            enabled: true,
            last_login: None,
            failed_login_attempts: 0,
            locked_until: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        users.insert("admin".to_string(), admin_user);
        warn!("Created default admin user with password 'admin123' - CHANGE THIS IMMEDIATELY");

        Ok(())
    }

    pub fn hash_password(&self, password: &str, salt: &str) -> Result<String> {
        let salted_password = format!("{}{}", password, salt);
        let digest = digest::digest(&digest::SHA256, salted_password.as_bytes());
        Ok(hex::encode(digest.as_ref()))
    }

    pub async fn ensure_initialized(&self) -> Result<()> {
        let users = self.users.read().await;
        if users.is_empty() {
            drop(users); // Release read lock
            self.initialize_defaults().await?;
        }
        Ok(())
    }

    pub async fn authenticate(&self, username: &str, password: &str) -> Result<String> {
        self.ensure_initialized().await?;
        let mut users = self.users.write().await;
        let user = users
            .get_mut(username)
            .ok_or_else(|| anyhow!("Invalid credentials"))?;

        // Check if user is locked
        if let Some(locked_until) = user.locked_until {
            if Utc::now() < locked_until {
                return Err(anyhow!("Account is locked"));
            } else {
                user.locked_until = None;
                user.failed_login_attempts = 0;
            }
        }

        if !user.enabled {
            return Err(anyhow!("Account is disabled"));
        }

        let password_hash = self.hash_password(password, &user.salt)?;
        if password_hash != user.password_hash {
            user.failed_login_attempts += 1;

            if user.failed_login_attempts >= self.config.max_failed_attempts {
                user.locked_until =
                    Some(Utc::now() + Duration::minutes(self.config.lockout_duration_minutes));
                warn!("User {} locked due to too many failed attempts", username);
            }

            return Err(anyhow!("Invalid credentials"));
        }

        // Reset failed attempts on successful login
        user.failed_login_attempts = 0;
        user.last_login = Some(Utc::now());

        // Get user permissions
        let roles = self.roles.read().await;
        let mut permissions = Vec::new();
        for role_id in &user.roles {
            if let Some(role) = roles.get(role_id) {
                permissions.extend(role.permissions.clone());
            }
        }

        // Remove duplicates
        permissions.sort_by_key(|p| format!("{:?}", p));
        permissions.dedup();

        // Create JWT
        let jwt_id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let exp = (now + Duration::hours(self.config.jwt_expiration_hours)).timestamp() as usize;

        let claims = AuthClaims {
            sub: user.id.clone(),
            username: user.username.clone(),
            roles: user.roles.clone(),
            permissions,
            exp,
            iat: now.timestamp() as usize,
            jti: jwt_id.clone(),
        };

        let token = encode(&Header::default(), &claims, &self.encoding_key)?;

        // Store active session
        let session = AuthSession {
            user_id: user.id.clone(),
            jwt_id: jwt_id.clone(),
            created_at: now,
            last_activity: now,
            ip_address: None,
            user_agent: None,
        };

        let mut sessions = self.active_sessions.write().await;
        sessions.insert(jwt_id, session);

        debug!("User {} authenticated successfully", username);
        Ok(token)
    }

    pub async fn verify_token(&self, token: &str) -> Result<AuthClaims> {
        let validation = Validation::new(Algorithm::HS256);
        let token_data = decode::<AuthClaims>(token, &self.decoding_key, &validation)?;

        let sessions = self.active_sessions.read().await;
        if !sessions.contains_key(&token_data.claims.jti) {
            return Err(anyhow!("Session not found"));
        }

        Ok(token_data.claims)
    }

    pub async fn logout(&self, jwt_id: &str) -> Result<()> {
        let mut sessions = self.active_sessions.write().await;
        sessions.remove(jwt_id);
        Ok(())
    }

    pub fn has_permission(&self, claims: &AuthClaims, required_permission: &Permission) -> bool {
        claims.permissions.contains(required_permission)
    }
}

#[derive(Debug)]
pub struct AuthUser {
    pub claims: AuthClaims,
}

impl AuthUser {
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.claims.permissions.contains(permission)
    }

    pub fn require_permission(&self, permission: &Permission) -> Result<(), StatusCode> {
        if self.has_permission(permission) {
            Ok(())
        } else {
            Err(StatusCode::FORBIDDEN)
        }
    }
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Extract the authorization header
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                .await
                .map_err(|_| StatusCode::UNAUTHORIZED)?;

        // For now, create a simple demo claims structure
        // TODO: Integrate with actual AuthState when available in request context
        let claims = AuthClaims {
            sub: "demo".to_string(),
            username: "admin".to_string(),
            roles: vec!["admin".to_string()],
            permissions: vec![Permission::SystemAdmin, Permission::CallsWrite],
            exp: (chrono::Utc::now() + chrono::Duration::hours(1)).timestamp() as usize,
            iat: chrono::Utc::now().timestamp() as usize,
            jti: "demo-token".to_string(),
        };

        Ok(AuthUser { claims })
    }
}

// Request/Response types for API
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub user: UserInfo,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub email: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub last_login: Option<DateTime<Utc>>,
}

impl From<(&User, &[Permission])> for UserInfo {
    fn from((user, permissions): (&User, &[Permission])) -> Self {
        Self {
            id: user.id.clone(),
            username: user.username.clone(),
            email: user.email.clone(),
            roles: user.roles.clone(),
            permissions: permissions.iter().map(|p| format!("{:?}", p)).collect(),
            last_login: user.last_login,
        }
    }
}

// Middleware for requiring specific permissions
pub fn require_permission(
    permission: Permission,
) -> impl Fn(AuthUser) -> Result<AuthUser, StatusCode> {
    move |user: AuthUser| {
        user.require_permission(&permission)?;
        Ok(user)
    }
}
