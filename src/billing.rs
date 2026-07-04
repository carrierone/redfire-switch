/*
 * Redfire Switch - A Class 4 SIP Telephone Switch
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
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use parking_lot::RwLock;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Billing service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingConfig {
    /// Enable billing checks
    pub enabled: bool,
    /// Real-time billing checks
    pub realtime_checks: bool,
    /// Credit limit enforcement
    pub enforce_credit_limits: bool,
    /// Prepaid account support
    pub prepaid_support: bool,
    /// Billing database configuration
    pub database_config: BillingDatabaseConfig,
    /// Payment required configuration
    pub payment_required_config: PaymentRequiredConfig,
    /// Account suspension settings
    pub suspension_config: SuspensionConfig,
}

impl Default for BillingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            realtime_checks: true,
            enforce_credit_limits: true,
            prepaid_support: true,
            database_config: BillingDatabaseConfig::default(),
            payment_required_config: PaymentRequiredConfig::default(),
            suspension_config: SuspensionConfig::default(),
        }
    }
}

/// Billing database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingDatabaseConfig {
    /// Whether database integration is enabled
    pub enabled: bool,
    /// Database connection string
    pub connection_string: String,
    /// Connection pool size
    pub pool_size: u32,
    /// Query timeout (seconds)
    pub query_timeout: u64,
    /// Cache duration for account status (seconds)
    pub cache_duration: u64,
}

impl Default for BillingDatabaseConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            connection_string: "postgresql://billing:password@localhost/billing".to_string(),
            pool_size: 10,
            query_timeout: 5,
            cache_duration: 300, // 5 minutes
        }
    }
}

/// Payment Required (402) configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentRequiredConfig {
    /// Enable 402 responses
    pub enabled: bool,
    /// Custom SIP headers to include
    pub custom_headers: HashMap<String, String>,
    /// Payment URL to include in response
    pub payment_url: Option<String>,
    /// Custom reason phrase
    pub reason_phrase: String,
    /// Include account balance in response
    pub include_balance: bool,
    /// Include payment instructions
    pub include_payment_instructions: bool,
}

impl Default for PaymentRequiredConfig {
    fn default() -> Self {
        let mut headers = HashMap::new();
        headers.insert("P-Billing-Info".to_string(), "Payment Required".to_string());
        headers.insert("Retry-After".to_string(), "3600".to_string()); // 1 hour

        Self {
            enabled: true,
            custom_headers: headers,
            payment_url: Some("https://billing.carrierone.com/payment".to_string()),
            reason_phrase: "Payment Required - Account Suspended".to_string(),
            include_balance: true,
            include_payment_instructions: true,
        }
    }
}

/// Account suspension configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuspensionConfig {
    /// Grace period before suspension (seconds)
    pub grace_period: u64,
    /// Allow emergency calls when suspended
    pub allow_emergency_calls: bool,
    /// Emergency number patterns
    pub emergency_numbers: Vec<String>,
    /// Suspension notification settings
    pub notification_config: NotificationConfig,
}

impl Default for SuspensionConfig {
    fn default() -> Self {
        Self {
            grace_period: 86400, // 24 hours
            allow_emergency_calls: true,
            emergency_numbers: vec![
                "911".to_string(),
                "933".to_string(), // US emergency test
                "+1911".to_string(),
            ],
            notification_config: NotificationConfig::default(),
        }
    }
}

/// Notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Enable email notifications
    pub email_enabled: bool,
    /// Enable SMS notifications
    pub sms_enabled: bool,
    /// Enable webhook notifications
    pub webhook_enabled: bool,
    /// Webhook URL
    pub webhook_url: Option<String>,
    /// Email templates
    pub email_templates: HashMap<String, String>,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        let mut templates = HashMap::new();
        templates.insert("suspension".to_string(), 
            "Your account has been suspended due to insufficient funds. Please make a payment to restore service.".to_string());
        templates.insert(
            "low_balance".to_string(),
            "Your account balance is low. Please add funds to avoid service interruption."
                .to_string(),
        );

        Self {
            email_enabled: false,
            sms_enabled: false,
            webhook_enabled: false,
            webhook_url: None,
            email_templates: templates,
        }
    }
}

/// Account status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AccountStatus {
    /// Account is active and in good standing
    Active,
    /// Account has low balance warning
    LowBalance,
    /// Account is suspended for non-payment
    Suspended,
    /// Account is closed/terminated
    Closed,
    /// Account is under review
    UnderReview,
    /// Account is in grace period
    GracePeriod,
}

/// Account type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AccountType {
    /// Postpaid account with credit limit
    Postpaid,
    /// Prepaid account with balance
    Prepaid,
    /// Test account
    Test,
    /// Internal/admin account
    Internal,
}

/// Customer account information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerAccount {
    /// Customer ID
    pub customer_id: String,
    /// Account type
    pub account_type: AccountType,
    /// Current account status
    pub status: AccountStatus,
    /// Current balance (negative for postpaid debt)
    pub balance: f64,
    /// Credit limit (for postpaid accounts)
    pub credit_limit: Option<f64>,
    /// Low balance threshold
    pub low_balance_threshold: f64,
    /// Currency code
    pub currency: String,
    /// Account creation date
    pub created_at: DateTime<Utc>,
    /// Last payment date
    pub last_payment_date: Option<DateTime<Utc>>,
    /// Account suspension date
    pub suspended_at: Option<DateTime<Utc>>,
    /// Grace period end date
    pub grace_period_end: Option<DateTime<Utc>>,
    /// Billing profile settings
    pub billing_profile: BillingProfile,
    /// Account metadata
    pub metadata: HashMap<String, String>,
}

/// Billing profile settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingProfile {
    /// Billing cycle (monthly, weekly, etc.)
    pub billing_cycle: String,
    /// Automatic payment enabled
    pub auto_pay: bool,
    /// Payment method on file
    pub payment_method: Option<String>,
    /// Tax rate applicable
    pub tax_rate: f64,
    /// Discount rate
    pub discount_rate: f64,
    /// Minimum balance for calls
    pub minimum_balance: f64,
    /// Maximum call value allowed
    pub max_call_value: Option<f64>,
}

impl Default for BillingProfile {
    fn default() -> Self {
        Self {
            billing_cycle: "monthly".to_string(),
            auto_pay: false,
            payment_method: None,
            tax_rate: 0.0,
            discount_rate: 0.0,
            minimum_balance: 0.0,
            max_call_value: Some(100.0), // $100 max per call
        }
    }
}

/// Billing check result
#[derive(Debug, Clone, PartialEq)]
pub enum BillingCheckResult {
    /// Call is approved
    Approved,
    /// Call blocked due to insufficient funds
    InsufficientFunds(String),
    /// Call blocked due to account suspension
    AccountSuspended(String),
    /// Call blocked due to exceeded credit limit
    CreditLimitExceeded(String),
    /// Call blocked due to account closure
    AccountClosed(String),
    /// Emergency call allowed despite billing issues
    EmergencyAllowed(String),
}

/// Call authorization request
#[derive(Debug, Clone)]
pub struct CallAuthRequest {
    /// Customer ID
    pub customer_id: String,
    /// Calling number
    pub from_number: String,
    /// Called number
    pub to_number: String,
    /// Estimated call cost
    pub estimated_cost: f64,
    /// Call priority (for emergency calls)
    pub is_emergency: bool,
    /// Ingress trunk ID
    pub trunk_id: Option<String>,
    /// Call metadata
    pub metadata: HashMap<String, String>,
}

/// Cached account status
#[derive(Debug, Clone)]
pub struct CachedAccountStatus {
    pub account: CustomerAccount,
    pub cached_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Billing service statistics
#[derive(Debug, Clone, Default)]
pub struct BillingStats {
    pub total_checks: u64,
    pub approved_calls: u64,
    pub blocked_calls: u64,
    pub insufficient_funds_blocks: u64,
    pub suspended_account_blocks: u64,
    pub credit_limit_blocks: u64,
    pub emergency_overrides: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub payment_required_responses: u64,
    pub last_updated: Option<DateTime<Utc>>,
}

/// Main billing service
pub struct BillingService {
    config: BillingConfig,
    /// Database connection pool
    db_pool: Option<PgPool>,
    /// Cached account statuses
    account_cache: Arc<DashMap<String, CachedAccountStatus>>,
    /// Billing statistics
    stats: Arc<RwLock<BillingStats>>,
}

impl BillingService {
    /// Create a new billing service
    pub fn new(config: BillingConfig) -> Result<Self> {
        let service = Self {
            config,
            db_pool: None,
            account_cache: Arc::new(DashMap::new()),
            stats: Arc::new(RwLock::new(BillingStats::default())),
        };

        // Start cache cleanup task when a Tokio runtime is available. Guarding
        // on the current handle keeps `new` usable from synchronous contexts
        // (e.g. unit tests) instead of panicking with "no reactor running".
        if tokio::runtime::Handle::try_current().is_ok() {
            let cleanup_service = service.clone();
            tokio::spawn(async move {
                cleanup_service.run_cache_cleanup().await;
            });
        }

        info!("Billing service initialized");
        Ok(service)
    }

    /// Initialize database connection
    pub async fn connect_database(&mut self, database_url: &str) -> Result<()> {
        if self.config.database_config.enabled {
            info!("Connecting to billing database: {}", database_url);
            let pool = PgPool::connect(database_url).await
                .map_err(|e| anyhow!("Failed to connect to billing database: {}", e))?;
            self.db_pool = Some(pool);
            info!("Successfully connected to billing database");
        }
        Ok(())
    }

    /// Check if a call should be authorized based on billing status
    pub async fn check_call_authorization(
        &self,
        request: CallAuthRequest,
    ) -> Result<BillingCheckResult> {
        if !self.config.enabled {
            return Ok(BillingCheckResult::Approved);
        }

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.total_checks += 1;
            stats.last_updated = Some(Utc::now());
        }

        // Check if this is an emergency call
        if request.is_emergency || self.is_emergency_number(&request.to_number) {
            if self.config.suspension_config.allow_emergency_calls {
                let mut stats = self.stats.write();
                stats.emergency_overrides += 1;
                stats.approved_calls += 1;
                return Ok(BillingCheckResult::EmergencyAllowed(
                    "Emergency call allowed regardless of account status".to_string(),
                ));
            }
        }

        // Get account status
        let account = match self.get_account_status(&request.customer_id).await {
            Ok(account) => account,
            Err(e) => {
                warn!(
                    "Failed to get account status for {}: {}",
                    request.customer_id, e
                );
                // Default to blocking if we can't verify account
                let mut stats = self.stats.write();
                stats.blocked_calls += 1;
                return Ok(BillingCheckResult::AccountClosed(
                    "Unable to verify account status".to_string(),
                ));
            }
        };

        // Check account status
        let result = match account.status {
            AccountStatus::Closed => {
                BillingCheckResult::AccountClosed("Account has been closed".to_string())
            }
            AccountStatus::Suspended => BillingCheckResult::AccountSuspended(
                "Account suspended for non-payment".to_string(),
            ),
            AccountStatus::UnderReview => {
                BillingCheckResult::AccountSuspended("Account under review".to_string())
            }
            AccountStatus::GracePeriod => {
                // Check if grace period has expired
                if let Some(grace_end) = account.grace_period_end {
                    if Utc::now() > grace_end {
                        BillingCheckResult::AccountSuspended("Grace period expired".to_string())
                    } else {
                        self.check_call_funds(&account, request.estimated_cost)
                    }
                } else {
                    self.check_call_funds(&account, request.estimated_cost)
                }
            }
            AccountStatus::Active | AccountStatus::LowBalance => {
                self.check_call_funds(&account, request.estimated_cost)
            }
        };

        // Update statistics based on result
        {
            let mut stats = self.stats.write();
            match result {
                BillingCheckResult::Approved => stats.approved_calls += 1,
                BillingCheckResult::InsufficientFunds(_) => {
                    stats.blocked_calls += 1;
                    stats.insufficient_funds_blocks += 1;
                }
                BillingCheckResult::AccountSuspended(_) => {
                    stats.blocked_calls += 1;
                    stats.suspended_account_blocks += 1;
                }
                BillingCheckResult::CreditLimitExceeded(_) => {
                    stats.blocked_calls += 1;
                    stats.credit_limit_blocks += 1;
                }
                BillingCheckResult::AccountClosed(_) => {
                    stats.blocked_calls += 1;
                }
                _ => {}
            }
        }

        Ok(result)
    }

    /// Check if account has sufficient funds for the call
    fn check_call_funds(
        &self,
        account: &CustomerAccount,
        estimated_cost: f64,
    ) -> BillingCheckResult {
        match account.account_type {
            AccountType::Prepaid => {
                // For prepaid, check if balance covers the call
                if account.balance < estimated_cost {
                    BillingCheckResult::InsufficientFunds(format!(
                        "Insufficient prepaid balance: ${:.2} required, ${:.2} available",
                        estimated_cost, account.balance
                    ))
                } else if account.balance < account.billing_profile.minimum_balance {
                    BillingCheckResult::InsufficientFunds(format!(
                        "Balance below minimum required: ${:.2}",
                        account.billing_profile.minimum_balance
                    ))
                } else {
                    BillingCheckResult::Approved
                }
            }
            AccountType::Postpaid => {
                // For postpaid, check credit limit
                if let Some(credit_limit) = account.credit_limit {
                    let current_debt = -account.balance; // Negative balance = debt
                    let potential_debt = current_debt + estimated_cost;

                    if potential_debt > credit_limit {
                        BillingCheckResult::CreditLimitExceeded(format!(
                            "Credit limit exceeded: ${:.2} limit, ${:.2} would be used",
                            credit_limit, potential_debt
                        ))
                    } else {
                        BillingCheckResult::Approved
                    }
                } else {
                    // No credit limit set, allow call
                    BillingCheckResult::Approved
                }
            }
            AccountType::Test | AccountType::Internal => {
                // Test and internal accounts are always allowed
                BillingCheckResult::Approved
            }
        }
    }

    /// Check if a number is an emergency number
    fn is_emergency_number(&self, number: &str) -> bool {
        let normalized = number.trim_start_matches('+').trim_start_matches('1');

        for pattern in &self.config.suspension_config.emergency_numbers {
            if normalized == pattern || number == pattern {
                return true;
            }
        }

        false
    }

    /// Get account status from cache or database
    async fn get_account_status(&self, customer_id: &str) -> Result<CustomerAccount> {
        let now = Utc::now();

        // Check cache first
        if let Some(cached) = self.account_cache.get(customer_id) {
            if now < cached.expires_at {
                let mut stats = self.stats.write();
                stats.cache_hits += 1;
                return Ok(cached.account.clone());
            } else {
                // Cache expired, remove it
                self.account_cache.remove(customer_id);
            }
        }

        // Cache miss, fetch from database
        {
            let mut stats = self.stats.write();
            stats.cache_misses += 1;
        }

        let account = self.fetch_account_from_database(customer_id).await?;

        // Cache the result
        let expires_at = now + Duration::seconds(self.config.database_config.cache_duration as i64);
        let cached = CachedAccountStatus {
            account: account.clone(),
            cached_at: now,
            expires_at,
        };

        self.account_cache.insert(customer_id.to_string(), cached);

        Ok(account)
    }

    /// Fetch account from database
    async fn fetch_account_from_database(&self, customer_id: &str) -> Result<CustomerAccount> {
        debug!("Fetching account {} from billing database", customer_id);

        if let Some(pool) = &self.db_pool {
            // Execute SQL query to fetch customer account
            let row = sqlx::query(
                r#"
                SELECT
                    customer_id,
                    account_type,
                    status,
                    balance,
                    credit_limit,
                    low_balance_threshold,
                    currency,
                    created_at,
                    last_payment_date,
                    suspended_at,
                    grace_period_end
                FROM customer_accounts
                WHERE customer_id = $1 AND active = true
                "#,
            )
            .bind(customer_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| anyhow!("Database query failed: {}", e))?;

            if let Some(row) = row {
                let account = CustomerAccount {
                    customer_id: row.get("customer_id"),
                    account_type: match row.get::<String, _>("account_type").as_str() {
                        "prepaid" => AccountType::Prepaid,
                        "postpaid" => AccountType::Postpaid,
                        _ => AccountType::Postpaid, // default
                    },
                    status: match row.get::<String, _>("status").as_str() {
                        "active" => AccountStatus::Active,
                        "suspended" => AccountStatus::Suspended,
                        "closed" => AccountStatus::Closed,
                        "low_balance" => AccountStatus::LowBalance,
                        "grace_period" => AccountStatus::GracePeriod,
                        "under_review" => AccountStatus::UnderReview,
                        _ => AccountStatus::Active, // default
                    },
                    balance: row.get::<Option<f64>, _>("balance").unwrap_or(0.0),
                    credit_limit: row.get("credit_limit"),
                    low_balance_threshold: row.get::<Option<f64>, _>("low_balance_threshold").unwrap_or(10.0),
                    currency: row.get::<Option<String>, _>("currency").unwrap_or_else(|| "USD".to_string()),
                    created_at: row.get::<Option<DateTime<Utc>>, _>("created_at").unwrap_or_else(|| Utc::now()),
                    last_payment_date: row.get("last_payment_date"),
                    suspended_at: row.get("suspended_at"),
                    grace_period_end: row.get("grace_period_end"),
                    billing_profile: BillingProfile::default(), // Would load from separate table
                    metadata: HashMap::new(), // Would load from separate table
                };

                debug!("Successfully fetched account for customer {}: status={:?}, balance={}",
                       customer_id, account.status, account.balance);
                Ok(account)
            } else {
                warn!("Customer account not found: {}", customer_id);
                Err(anyhow!("Customer account not found: {}", customer_id))
            }
        } else {
            // Fallback to mock data if database not connected
            warn!("Database not connected, using mock account data for customer {}", customer_id);

            let account = CustomerAccount {
                customer_id: customer_id.to_string(),
                account_type: AccountType::Postpaid,
                status: AccountStatus::Active,
                balance: 50.0,
                credit_limit: Some(1000.0),
                low_balance_threshold: 10.0,
                currency: "USD".to_string(),
                created_at: Utc::now() - Duration::days(30),
                last_payment_date: Some(Utc::now() - Duration::days(15)),
                suspended_at: None,
                grace_period_end: None,
                billing_profile: BillingProfile::default(),
                metadata: HashMap::new(),
            };

            Ok(account)
        }
    }

    /// Generate SIP 402 Payment Required response
    pub fn generate_payment_required_response(
        &self,
        customer_id: &str,
        reason: &str,
    ) -> PaymentRequiredResponse {
        let mut stats = self.stats.write();
        stats.payment_required_responses += 1;

        let mut headers = self.config.payment_required_config.custom_headers.clone();

        if let Some(payment_url) = &self.config.payment_required_config.payment_url {
            headers.insert("P-Payment-URL".to_string(), payment_url.clone());
        }

        headers.insert("P-Customer-ID".to_string(), customer_id.to_string());
        headers.insert("P-Block-Reason".to_string(), reason.to_string());

        PaymentRequiredResponse {
            status_code: 402,
            reason_phrase: self.config.payment_required_config.reason_phrase.clone(),
            headers,
            body: if self
                .config
                .payment_required_config
                .include_payment_instructions
            {
                Some(format!(
                    "Payment required for customer {}. Reason: {}. Please visit {} to make a payment.",
                    customer_id,
                    reason,
                    self.config.payment_required_config.payment_url
                        .as_ref()
                        .unwrap_or(&"your billing portal".to_string())
                ))
            } else {
                None
            },
        }
    }

    /// Invalidate account cache for a customer
    pub fn invalidate_account_cache(&self, customer_id: &str) {
        self.account_cache.remove(customer_id);
        debug!("Invalidated account cache for customer {}", customer_id);
    }

    /// Get billing statistics
    pub fn get_stats(&self) -> BillingStats {
        self.stats.read().clone()
    }

    /// Manually suspend an account
    pub async fn suspend_account(&self, customer_id: &str, reason: &str) -> Result<()> {
        // TODO: Update database to suspend account

        // Invalidate cache
        self.invalidate_account_cache(customer_id);

        info!("Account {} suspended: {}", customer_id, reason);
        Ok(())
    }

    /// Manually reactivate an account
    pub async fn reactivate_account(&self, customer_id: &str) -> Result<()> {
        // TODO: Update database to reactivate account

        // Invalidate cache
        self.invalidate_account_cache(customer_id);

        info!("Account {} reactivated", customer_id);
        Ok(())
    }

    /// Cache cleanup task
    async fn run_cache_cleanup(&self) {
        let mut cleanup_timer = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes

        loop {
            cleanup_timer.tick().await;
            self.cleanup_expired_cache().await;
        }
    }

    /// Remove expired entries from cache
    async fn cleanup_expired_cache(&self) {
        let now = Utc::now();
        let mut removed_count = 0;

        self.account_cache.retain(|_, cached| {
            if now >= cached.expires_at {
                removed_count += 1;
                false
            } else {
                true
            }
        });

        if removed_count > 0 {
            debug!("Cleaned up {} expired account cache entries", removed_count);
        }
    }
}

impl Clone for BillingService {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            db_pool: self.db_pool.clone(),
            account_cache: self.account_cache.clone(),
            stats: self.stats.clone(),
        }
    }
}

/// Payment Required SIP response
#[derive(Debug, Clone)]
pub struct PaymentRequiredResponse {
    pub status_code: u16,
    pub reason_phrase: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

/// Billing utilities
pub mod utils {
    use super::*;

    /// Calculate estimated call cost
    pub fn calculate_estimated_cost(
        _destination: &str,
        rate_per_minute: f64,
        estimated_duration_minutes: f64,
    ) -> f64 {
        // Basic cost calculation
        let base_cost = rate_per_minute * estimated_duration_minutes;

        // Add connection charge (typical for telecom)
        let connection_charge = 0.01; // 1 cent

        base_cost + connection_charge
    }

    /// Check if account needs balance warning
    pub fn needs_low_balance_warning(account: &CustomerAccount) -> bool {
        match account.account_type {
            AccountType::Prepaid => account.balance <= account.low_balance_threshold,
            AccountType::Postpaid => {
                if let Some(credit_limit) = account.credit_limit {
                    let current_debt = -account.balance;
                    let available_credit = credit_limit - current_debt;
                    available_credit <= account.low_balance_threshold
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Generate billing report
    pub fn generate_billing_report(service: &BillingService) -> String {
        let stats = service.get_stats();

        let approval_rate = if stats.total_checks > 0 {
            (stats.approved_calls as f64 / stats.total_checks as f64) * 100.0
        } else {
            0.0
        };

        let cache_hit_rate = if (stats.cache_hits + stats.cache_misses) > 0 {
            (stats.cache_hits as f64 / (stats.cache_hits + stats.cache_misses) as f64) * 100.0
        } else {
            0.0
        };

        format!(
            "Billing Service Report:\n\
             Total Checks: {}\n\
             Approved Calls: {} ({:.1}%)\n\
             Blocked Calls: {}\n\
             - Insufficient Funds: {}\n\
             - Account Suspended: {}\n\
             - Credit Limit Exceeded: {}\n\
             Emergency Overrides: {}\n\
             Payment Required Responses: {}\n\
             Cache Performance:\n\
             - Cache Hits: {} ({:.1}%)\n\
             - Cache Misses: {}\n\
             - Cached Accounts: {}",
            stats.total_checks,
            stats.approved_calls,
            approval_rate,
            stats.blocked_calls,
            stats.insufficient_funds_blocks,
            stats.suspended_account_blocks,
            stats.credit_limit_blocks,
            stats.emergency_overrides,
            stats.payment_required_responses,
            stats.cache_hits,
            cache_hit_rate,
            stats.cache_misses,
            service.account_cache.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emergency_number_detection() {
        let config = BillingConfig::default();
        let service = BillingService::new(config).unwrap();

        assert!(service.is_emergency_number("911"));
        assert!(service.is_emergency_number("+1911"));
        assert!(service.is_emergency_number("933"));
        assert!(!service.is_emergency_number("1234567890"));
    }

    #[test]
    fn test_prepaid_funds_check() {
        let billing_service = BillingService::new(BillingConfig::default()).unwrap();

        let account = CustomerAccount {
            customer_id: "test123".to_string(),
            account_type: AccountType::Prepaid,
            status: AccountStatus::Active,
            balance: 10.0,
            credit_limit: None,
            low_balance_threshold: 5.0,
            currency: "USD".to_string(),
            created_at: Utc::now(),
            last_payment_date: None,
            suspended_at: None,
            grace_period_end: None,
            billing_profile: BillingProfile::default(),
            metadata: HashMap::new(),
        };

        // Should approve call with sufficient balance
        let result = billing_service.check_call_funds(&account, 5.0);
        assert_eq!(result, BillingCheckResult::Approved);

        // Should block call with insufficient balance
        let result = billing_service.check_call_funds(&account, 15.0);
        assert!(matches!(result, BillingCheckResult::InsufficientFunds(_)));
    }

    #[test]
    fn test_postpaid_credit_check() {
        let billing_service = BillingService::new(BillingConfig::default()).unwrap();

        let account = CustomerAccount {
            customer_id: "test456".to_string(),
            account_type: AccountType::Postpaid,
            status: AccountStatus::Active,
            balance: -50.0, // $50 debt
            credit_limit: Some(100.0),
            low_balance_threshold: 10.0,
            currency: "USD".to_string(),
            created_at: Utc::now(),
            last_payment_date: None,
            suspended_at: None,
            grace_period_end: None,
            billing_profile: BillingProfile::default(),
            metadata: HashMap::new(),
        };

        // Should approve call within credit limit
        let result = billing_service.check_call_funds(&account, 25.0);
        assert_eq!(result, BillingCheckResult::Approved);

        // Should block call exceeding credit limit
        let result = billing_service.check_call_funds(&account, 75.0);
        assert!(matches!(result, BillingCheckResult::CreditLimitExceeded(_)));
    }

    #[test]
    fn test_cost_calculation() {
        let cost = utils::calculate_estimated_cost("+15551234567", 0.05, 10.0);
        assert_eq!(cost, 0.51); // $0.50 + $0.01 connection charge
    }

    #[tokio::test]
    async fn test_call_authorization() {
        let config = BillingConfig::default();
        let service = BillingService::new(config).unwrap();

        let request = CallAuthRequest {
            customer_id: "test123".to_string(),
            from_number: "+15551234567".to_string(),
            to_number: "+15559876543".to_string(),
            estimated_cost: 5.0,
            is_emergency: false,
            trunk_id: Some("trunk-1".to_string()),
            metadata: HashMap::new(),
        };

        let result = service.check_call_authorization(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_emergency_call_override() {
        let config = BillingConfig::default();
        let service = BillingService::new(config).unwrap();

        let request = CallAuthRequest {
            customer_id: "suspended123".to_string(),
            from_number: "+15551234567".to_string(),
            to_number: "911".to_string(),
            estimated_cost: 0.0,
            is_emergency: true,
            trunk_id: Some("trunk-1".to_string()),
            metadata: HashMap::new(),
        };

        let result = service.check_call_authorization(request).await.unwrap();
        assert!(matches!(result, BillingCheckResult::EmergencyAllowed(_)));
    }

    #[tokio::test]
    async fn test_rating_engine() {
        let engine = RatingEngine::new();

        // Add a rate table
        let rate_table = RateTable {
            destination_name: "US Local".to_string(),
            prefix: "1".to_string(),
            rate_per_minute: Decimal::from_str("0.01").unwrap(),
            minimum_duration_seconds: 6,
            billing_increment_seconds: 6,
            setup_fee: Decimal::from_str("0.005").unwrap(),
            effective_date: Utc::now(),
            expiry_date: None,
        };
        engine.update_rate(rate_table);

        // Test pricing calculation
        let pricing = engine.calculate_pricing("15551234567", 120).await.unwrap();
        assert_eq!(pricing.matched_prefix, Some("1".to_string()));
        assert_eq!(pricing.rate_per_minute, Decimal::from_str("0.01").unwrap());
    }
}

/// Real-time rating engine for call pricing
#[derive(Debug, Clone)]
pub struct RatingEngine {
    /// Rate tables indexed by destination prefix
    rate_tables: Arc<DashMap<String, RateTable>>,
    /// Default rates for unmatched prefixes
    default_rates: Arc<RwLock<DefaultRates>>,
}

/// Rate table for specific destination patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateTable {
    pub destination_name: String,
    pub prefix: String,
    pub rate_per_minute: Decimal,
    pub minimum_duration_seconds: u32,
    pub billing_increment_seconds: u32,
    pub setup_fee: Decimal,
    pub effective_date: DateTime<Utc>,
    pub expiry_date: Option<DateTime<Utc>>,
}

/// Default rates for unmatched destinations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultRates {
    pub domestic_rate_per_minute: Decimal,
    pub international_rate_per_minute: Decimal,
    pub premium_rate_per_minute: Decimal,
    pub minimum_charge: Decimal,
    pub setup_fee: Decimal,
}

impl Default for DefaultRates {
    fn default() -> Self {
        Self {
            domestic_rate_per_minute: Decimal::from_str("0.01").unwrap(), // $0.01/min
            international_rate_per_minute: Decimal::from_str("0.05").unwrap(), // $0.05/min
            premium_rate_per_minute: Decimal::from_str("0.25").unwrap(),  // $0.25/min
            minimum_charge: Decimal::from_str("0.01").unwrap(),           // $0.01 minimum
            setup_fee: Decimal::from_str("0.005").unwrap(),               // $0.005 setup
        }
    }
}

/// Call pricing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallPricing {
    pub destination: String,
    pub rate_per_minute: Decimal,
    pub setup_fee: Decimal,
    pub minimum_charge: Decimal,
    pub billing_increment: u32,
    pub estimated_cost: Decimal,
    pub matched_prefix: Option<String>,
}

impl RatingEngine {
    /// Create a new rating engine
    pub fn new() -> Self {
        Self {
            rate_tables: Arc::new(DashMap::new()),
            default_rates: Arc::new(RwLock::new(DefaultRates::default())),
        }
    }

    /// Calculate pricing for a destination number
    pub async fn calculate_pricing(
        &self,
        destination: &str,
        estimated_duration_seconds: u32,
    ) -> Result<CallPricing> {
        // Find the longest matching prefix
        let mut best_match: Option<RateTable> = None;
        let mut longest_prefix = 0;

        for entry in self.rate_tables.iter() {
            let prefix = entry.key();
            if destination.starts_with(prefix) && prefix.len() > longest_prefix {
                longest_prefix = prefix.len();
                best_match = Some(entry.value().clone());
            }
        }

        if let Some(rate_table) = best_match {
            // Use matched rate table
            let duration_minutes = Decimal::from(estimated_duration_seconds) / Decimal::from(60);
            let base_cost = rate_table.rate_per_minute * duration_minutes;
            let total_cost = base_cost + rate_table.setup_fee;

            Ok(CallPricing {
                destination: rate_table.destination_name.clone(),
                rate_per_minute: rate_table.rate_per_minute,
                setup_fee: rate_table.setup_fee,
                minimum_charge: Decimal::from_str("0.01").unwrap(),
                billing_increment: rate_table.billing_increment_seconds,
                estimated_cost: total_cost.max(Decimal::from_str("0.01").unwrap()),
                matched_prefix: Some(rate_table.prefix.clone()),
            })
        } else {
            // Use default rates
            let default_rates = self.default_rates.read();
            let rate = if destination.starts_with("1") {
                default_rates.domestic_rate_per_minute
            } else if destination.starts_with("900") || destination.starts_with("976") {
                default_rates.premium_rate_per_minute
            } else {
                default_rates.international_rate_per_minute
            };

            let duration_minutes = Decimal::from(estimated_duration_seconds) / Decimal::from(60);
            let base_cost = rate * duration_minutes;
            let total_cost = base_cost + default_rates.setup_fee;

            Ok(CallPricing {
                destination: "Unknown Destination".to_string(),
                rate_per_minute: rate,
                setup_fee: default_rates.setup_fee,
                minimum_charge: default_rates.minimum_charge,
                billing_increment: 60, // 1 minute default
                estimated_cost: total_cost.max(default_rates.minimum_charge),
                matched_prefix: None,
            })
        }
    }

    /// Add or update a rate table entry
    pub fn update_rate(&self, rate_table: RateTable) {
        info!(
            "Updated rate for prefix {}: ${}/min",
            rate_table.prefix, rate_table.rate_per_minute
        );
        self.rate_tables
            .insert(rate_table.prefix.clone(), rate_table);
    }

    /// Remove a rate table entry
    pub fn remove_rate(&self, prefix: &str) {
        if self.rate_tables.remove(prefix).is_some() {
            info!("Removed rate for prefix {}", prefix);
        }
    }

    /// Get all configured rates
    pub fn get_all_rates(&self) -> Vec<RateTable> {
        self.rate_tables
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Update default rates
    pub fn update_default_rates(&self, rates: DefaultRates) {
        *self.default_rates.write() = rates;
        info!("Updated default rates");
    }
}

use std::str::FromStr;
