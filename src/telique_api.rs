/*
 * Redfire Switch - TeliQue API Integration for CIC, LRN, and CNAM Lookups
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

//! # TeliQue API Integration
//! 
//! Provides integration with Teliax TeliQue APIs for telecommunications data lookups:
//! - CIC (Carrier Identification Code) lookups
//! - LRN (Location Routing Number) lookups  
//! - CNAM (Caller Name) lookups
//!
//! Based on the TeliQue API documentation: https://teliax.github.io/DBQ/
//!
//! Features:
//! - REST API integration with authentication
//! - Response caching for performance
//! - Rate limiting compliance
//! - Bulk lookup support
//! - Error handling and retry logic

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use tracing::{debug, info, warn, error};
use tokio::sync::RwLock;
use std::sync::Arc;
use reqwest::{Client, Response};
use url::Url;

/// TeliQue API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeliQueConfig {
    /// API base URL
    pub base_url: String,
    /// API key for authentication
    pub api_key: String,
    /// API secret for authentication
    pub api_secret: Option<String>,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Enable response caching
    pub enable_caching: bool,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    /// Maximum cache entries
    pub max_cache_entries: usize,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    /// Retry configuration
    pub retry_config: RetryConfig,
}

impl Default for TeliQueConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.teliax.com/v1".to_string(),
            api_key: "your_api_key".to_string(),
            api_secret: None,
            timeout_seconds: 10,
            enable_caching: true,
            cache_ttl_seconds: 3600, // 1 hour
            max_cache_entries: 10000,
            rate_limit: RateLimitConfig::default(),
            retry_config: RetryConfig::default(),
        }
    }
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per minute
    pub max_requests_per_minute: u32,
    /// Burst capacity
    pub burst_capacity: u32,
    /// Enable rate limiting
    pub enabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests_per_minute: 1000,
            burst_capacity: 50,
            enabled: true,
        }
    }
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum retry attempts
    pub max_attempts: u32,
    /// Initial retry delay in milliseconds
    pub initial_delay_ms: u64,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Maximum delay in milliseconds
    pub max_delay_ms: u64,
    /// Enable retries
    pub enabled: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 1000,
            backoff_multiplier: 2.0,
            max_delay_ms: 30000,
            enabled: true,
        }
    }
}

/// CIC (Carrier Identification Code) lookup result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CicLookupResult {
    /// CIC code
    pub cic: String,
    /// Carrier name
    pub carrier_name: String,
    /// Carrier type (IXC, LEC, WIRELESS, etc.)
    pub carrier_type: String,
    /// Operating Company Number
    pub ocn: Option<String>,
    /// State/jurisdiction
    pub state: Option<String>,
    /// Status (ACTIVE, INACTIVE, etc.)
    pub status: String,
    /// Last updated timestamp
    pub last_updated: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// LRN (Location Routing Number) lookup result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LrnLookupResult {
    /// Original number
    pub number: String,
    /// LRN (Location Routing Number)
    pub lrn: String,
    /// SPID (Service Provider ID)
    pub spid: Option<String>,
    /// Jurisdiction/LATA
    pub lata: Option<String>,
    /// Rate center
    pub rate_center: Option<String>,
    /// State
    pub state: Option<String>,
    /// Porting status
    pub ported: bool,
    /// Port date if ported
    pub port_date: Option<String>,
    /// Wireless indicator
    pub wireless: bool,
    /// Service provider name
    pub provider_name: Option<String>,
}

/// CNAM (Caller Name) lookup result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CnamLookupResult {
    /// Phone number
    pub number: String,
    /// Caller name
    pub name: Option<String>,
    /// Name source (LIDB, CNAM, etc.)
    pub source: Option<String>,
    /// Confidence score (0-100)
    pub confidence: Option<u8>,
    /// Privacy indicator
    pub private: bool,
    /// Business/residential indicator
    pub business: Option<bool>,
}

/// Bulk lookup request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkLookupRequest {
    /// List of numbers to lookup
    pub numbers: Vec<String>,
    /// Lookup type
    pub lookup_type: LookupType,
    /// Optional callback URL for async results
    pub callback_url: Option<String>,
}

/// Lookup type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LookupType {
    Cic,
    Lrn,
    Cnam,
}

/// Bulk lookup response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BulkLookupResponse {
    /// Request ID for tracking
    pub request_id: String,
    /// Status of the bulk request
    pub status: String,
    /// Results if synchronous
    pub results: Option<Vec<LookupResult>>,
    /// Estimated completion time for async requests
    pub estimated_completion: Option<String>,
}

/// Generic lookup result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LookupResult {
    Cic(CicLookupResult),
    Lrn(LrnLookupResult),
    Cnam(CnamLookupResult),
}

/// Cache entry
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    data: T,
    created_at: SystemTime,
    ttl: Duration,
}

/// TeliQue API client
pub struct TeliQueClient {
    config: TeliQueConfig,
    client: Client,
    /// CIC lookup cache
    cic_cache: Arc<RwLock<HashMap<String, CacheEntry<CicLookupResult>>>>,
    /// LRN lookup cache
    lrn_cache: Arc<RwLock<HashMap<String, CacheEntry<LrnLookupResult>>>>,
    /// CNAM lookup cache
    cnam_cache: Arc<RwLock<HashMap<String, CacheEntry<CnamLookupResult>>>>,
    /// Rate limiter
    rate_limiter: Arc<RwLock<RateLimiter>>,
}

/// Simple rate limiter
#[derive(Debug)]
struct RateLimiter {
    tokens: f64,
    last_refill: SystemTime,
    max_tokens: f64,
    refill_rate: f64, // tokens per second
}

impl RateLimiter {
    fn new(max_requests_per_minute: u32, burst_capacity: u32) -> Self {
        Self {
            tokens: burst_capacity as f64,
            last_refill: SystemTime::now(),
            max_tokens: burst_capacity as f64,
            refill_rate: max_requests_per_minute as f64 / 60.0,
        }
    }
    
    fn try_acquire(&mut self) -> bool {
        self.refill();
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
    
    fn refill(&mut self) {
        let now = SystemTime::now();
        let elapsed = now.duration_since(self.last_refill).unwrap_or(Duration::from_secs(0));
        let new_tokens = elapsed.as_secs_f64() * self.refill_rate;
        
        self.tokens = (self.tokens + new_tokens).min(self.max_tokens);
        self.last_refill = now;
    }
}

impl TeliQueClient {
    /// Create new TeliQue API client
    pub fn new(config: TeliQueConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .user_agent("Redfire-Switch/1.0")
            .build()?;
        
        let rate_limiter = Arc::new(RwLock::new(RateLimiter::new(
            config.rate_limit.max_requests_per_minute,
            config.rate_limit.burst_capacity,
        )));
        
        Ok(Self {
            config,
            client,
            cic_cache: Arc::new(RwLock::new(HashMap::new())),
            lrn_cache: Arc::new(RwLock::new(HashMap::new())),
            cnam_cache: Arc::new(RwLock::new(HashMap::new())),
            rate_limiter,
        })
    }
    
    /// Start the TeliQue client (background tasks)
    pub async fn start(&self) -> Result<()> {
        info!("Starting TeliQue API client");
        
        if self.config.enable_caching {
            self.start_cache_cleanup().await;
        }
        
        Ok(())
    }
    
    /// Lookup CIC information
    pub async fn lookup_cic(&self, cic: &str) -> Result<CicLookupResult> {
        // Check cache first
        if self.config.enable_caching {
            if let Some(cached) = self.get_cached_cic(cic).await {
                debug!("CIC lookup cache hit for {}", cic);
                return Ok(cached);
            }
        }
        
        // Rate limiting
        self.check_rate_limit().await?;
        
        let url = format!("{}/cic/{}", self.config.base_url, cic);
        let response = self.make_request(&url).await?;
        
        let result: CicLookupResult = response.json().await
            .map_err(|e| anyhow!("Failed to parse CIC response: {}", e))?;
        
        // Cache result
        if self.config.enable_caching {
            self.cache_cic(cic, &result).await;
        }
        
        info!("CIC lookup successful for {}: {}", cic, result.carrier_name);
        Ok(result)
    }
    
    /// Lookup LRN information
    pub async fn lookup_lrn(&self, number: &str) -> Result<LrnLookupResult> {
        let normalized_number = self.normalize_number(number)?;
        
        // Check cache first
        if self.config.enable_caching {
            if let Some(cached) = self.get_cached_lrn(&normalized_number).await {
                debug!("LRN lookup cache hit for {}", normalized_number);
                return Ok(cached);
            }
        }
        
        // Rate limiting
        self.check_rate_limit().await?;
        
        let url = format!("{}/lrn/{}", self.config.base_url, normalized_number);
        let response = self.make_request(&url).await?;
        
        let result: LrnLookupResult = response.json().await
            .map_err(|e| anyhow!("Failed to parse LRN response: {}", e))?;
        
        // Cache result
        if self.config.enable_caching {
            self.cache_lrn(&normalized_number, &result).await;
        }
        
        info!("LRN lookup successful for {}: LRN={}, ported={}", 
              normalized_number, result.lrn, result.ported);
        Ok(result)
    }
    
    /// Lookup CNAM information
    pub async fn lookup_cnam(&self, number: &str) -> Result<CnamLookupResult> {
        let normalized_number = self.normalize_number(number)?;
        
        // Check cache first
        if self.config.enable_caching {
            if let Some(cached) = self.get_cached_cnam(&normalized_number).await {
                debug!("CNAM lookup cache hit for {}", normalized_number);
                return Ok(cached);
            }
        }
        
        // Rate limiting
        self.check_rate_limit().await?;
        
        let url = format!("{}/cnam/{}", self.config.base_url, normalized_number);
        let response = self.make_request(&url).await?;
        
        let result: CnamLookupResult = response.json().await
            .map_err(|e| anyhow!("Failed to parse CNAM response: {}", e))?;
        
        // Cache result
        if self.config.enable_caching {
            self.cache_cnam(&normalized_number, &result).await;
        }
        
        info!("CNAM lookup successful for {}: name={:?}", 
              normalized_number, result.name);
        Ok(result)
    }
    
    /// Bulk lookup multiple numbers
    pub async fn bulk_lookup(&self, request: BulkLookupRequest) -> Result<BulkLookupResponse> {
        // Rate limiting for bulk requests
        self.check_rate_limit().await?;
        
        let url = format!("{}/bulk", self.config.base_url);
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(anyhow!("Bulk lookup failed: {}", response.status()));
        }
        
        let result: BulkLookupResponse = response.json().await?;
        
        info!("Bulk lookup submitted: request_id={}, numbers={}", 
              result.request_id, request.numbers.len());
        
        Ok(result)
    }
    
    /// Get bulk lookup results by request ID
    pub async fn get_bulk_results(&self, request_id: &str) -> Result<BulkLookupResponse> {
        let url = format!("{}/bulk/{}", self.config.base_url, request_id);
        let response = self.make_request(&url).await?;
        
        let result: BulkLookupResponse = response.json().await?;
        Ok(result)
    }
    
    /// Make authenticated HTTP request with retry logic
    async fn make_request(&self, url: &str) -> Result<Response> {
        let mut attempts = 0;
        let mut delay = Duration::from_millis(self.config.retry_config.initial_delay_ms);
        
        loop {
            let response = self.client
                .get(url)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .send()
                .await;
            
            match response {
                Ok(resp) if resp.status().is_success() => {
                    return Ok(resp);
                }
                Ok(resp) if resp.status().is_server_error() && attempts < self.config.retry_config.max_attempts => {
                    warn!("Server error {}, retrying in {:?} (attempt {}/{})", 
                          resp.status(), delay, attempts + 1, self.config.retry_config.max_attempts);
                    
                    tokio::time::sleep(delay).await;
                    attempts += 1;
                    delay = Duration::from_millis(
                        ((delay.as_millis() as f64) * self.config.retry_config.backoff_multiplier) as u64
                    ).min(Duration::from_millis(self.config.retry_config.max_delay_ms));
                }
                Ok(resp) => {
                    return Err(anyhow!("API request failed: {}", resp.status()));
                }
                Err(e) if attempts < self.config.retry_config.max_attempts => {
                    warn!("Request error {}, retrying in {:?} (attempt {}/{})", 
                          e, delay, attempts + 1, self.config.retry_config.max_attempts);
                    
                    tokio::time::sleep(delay).await;
                    attempts += 1;
                    delay = Duration::from_millis(
                        ((delay.as_millis() as f64) * self.config.retry_config.backoff_multiplier) as u64
                    ).min(Duration::from_millis(self.config.retry_config.max_delay_ms));
                }
                Err(e) => {
                    return Err(anyhow!("Request failed after {} attempts: {}", attempts + 1, e));
                }
            }
        }
    }
    
    /// Check rate limit
    async fn check_rate_limit(&self) -> Result<()> {
        if !self.config.rate_limit.enabled {
            return Ok(());
        }
        
        let mut limiter = self.rate_limiter.write().await;
        if !limiter.try_acquire() {
            return Err(anyhow!("Rate limit exceeded"));
        }
        
        Ok(())
    }
    
    /// Normalize phone number for API
    fn normalize_number(&self, number: &str) -> Result<String> {
        let cleaned = number.replace(&['-', ' ', '(', ')', '.'], "");
        let mut normalized = cleaned.trim_start_matches('+').to_string();
        
        // Add country code if missing (assume US/Canada +1)
        if normalized.len() == 10 && normalized.chars().all(|c| c.is_ascii_digit()) {
            normalized = format!("1{}", normalized);
        }
        
        if !normalized.chars().all(|c| c.is_ascii_digit()) || normalized.len() < 10 {
            return Err(anyhow!("Invalid number format: {}", number));
        }
        
        Ok(normalized)
    }
    
    /// Cache management methods
    async fn get_cached_cic(&self, cic: &str) -> Option<CicLookupResult> {
        let cache = self.cic_cache.read().await;
        if let Some(entry) = cache.get(cic) {
            if entry.created_at.elapsed().unwrap_or(Duration::MAX) < entry.ttl {
                return Some(entry.data.clone());
            }
        }
        None
    }
    
    async fn cache_cic(&self, cic: &str, result: &CicLookupResult) {
        let mut cache = self.cic_cache.write().await;
        
        if cache.len() >= self.config.max_cache_entries {
            // Remove oldest entries
            let cutoff = SystemTime::now() - Duration::from_secs(self.config.cache_ttl_seconds / 2);
            cache.retain(|_, entry| entry.created_at > cutoff);
        }
        
        cache.insert(cic.to_string(), CacheEntry {
            data: result.clone(),
            created_at: SystemTime::now(),
            ttl: Duration::from_secs(self.config.cache_ttl_seconds),
        });
    }
    
    async fn get_cached_lrn(&self, number: &str) -> Option<LrnLookupResult> {
        let cache = self.lrn_cache.read().await;
        if let Some(entry) = cache.get(number) {
            if entry.created_at.elapsed().unwrap_or(Duration::MAX) < entry.ttl {
                return Some(entry.data.clone());
            }
        }
        None
    }
    
    async fn cache_lrn(&self, number: &str, result: &LrnLookupResult) {
        let mut cache = self.lrn_cache.write().await;
        
        if cache.len() >= self.config.max_cache_entries {
            let cutoff = SystemTime::now() - Duration::from_secs(self.config.cache_ttl_seconds / 2);
            cache.retain(|_, entry| entry.created_at > cutoff);
        }
        
        cache.insert(number.to_string(), CacheEntry {
            data: result.clone(),
            created_at: SystemTime::now(),
            ttl: Duration::from_secs(self.config.cache_ttl_seconds),
        });
    }
    
    async fn get_cached_cnam(&self, number: &str) -> Option<CnamLookupResult> {
        let cache = self.cnam_cache.read().await;
        if let Some(entry) = cache.get(number) {
            if entry.created_at.elapsed().unwrap_or(Duration::MAX) < entry.ttl {
                return Some(entry.data.clone());
            }
        }
        None
    }
    
    async fn cache_cnam(&self, number: &str, result: &CnamLookupResult) {
        let mut cache = self.cnam_cache.write().await;
        
        if cache.len() >= self.config.max_cache_entries {
            let cutoff = SystemTime::now() - Duration::from_secs(self.config.cache_ttl_seconds / 2);
            cache.retain(|_, entry| entry.created_at > cutoff);
        }
        
        cache.insert(number.to_string(), CacheEntry {
            data: result.clone(),
            created_at: SystemTime::now(),
            ttl: Duration::from_secs(self.config.cache_ttl_seconds),
        });
    }
    
    /// Start cache cleanup task
    async fn start_cache_cleanup(&self) {
        let cic_cache = self.cic_cache.clone();
        let lrn_cache = self.lrn_cache.clone();
        let cnam_cache = self.cnam_cache.clone();
        let ttl_seconds = self.config.cache_ttl_seconds;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
            loop {
                interval.tick().await;
                
                let cutoff = SystemTime::now() - Duration::from_secs(ttl_seconds);
                
                // Clean CIC cache
                {
                    let mut cache = cic_cache.write().await;
                    let initial_size = cache.len();
                    cache.retain(|_, entry| entry.created_at > cutoff);
                    let removed = initial_size - cache.len();
                    if removed > 0 {
                        debug!("CIC cache cleanup: removed {} expired entries", removed);
                    }
                }
                
                // Clean LRN cache
                {
                    let mut cache = lrn_cache.write().await;
                    let initial_size = cache.len();
                    cache.retain(|_, entry| entry.created_at > cutoff);
                    let removed = initial_size - cache.len();
                    if removed > 0 {
                        debug!("LRN cache cleanup: removed {} expired entries", removed);
                    }
                }
                
                // Clean CNAM cache
                {
                    let mut cache = cnam_cache.write().await;
                    let initial_size = cache.len();
                    cache.retain(|_, entry| entry.created_at > cutoff);
                    let removed = initial_size - cache.len();
                    if removed > 0 {
                        debug!("CNAM cache cleanup: removed {} expired entries", removed);
                    }
                }
            }
        });
    }
    
    /// Get client statistics
    pub async fn get_statistics(&self) -> TeliQueStats {
        let cic_cache_size = self.cic_cache.read().await.len();
        let lrn_cache_size = self.lrn_cache.read().await.len();
        let cnam_cache_size = self.cnam_cache.read().await.len();
        
        TeliQueStats {
            cic_cache_entries: cic_cache_size,
            lrn_cache_entries: lrn_cache_size,
            cnam_cache_entries: cnam_cache_size,
            cache_enabled: self.config.enable_caching,
            rate_limit_enabled: self.config.rate_limit.enabled,
        }
    }
}

/// TeliQue client statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeliQueStats {
    pub cic_cache_entries: usize,
    pub lrn_cache_entries: usize,
    pub cnam_cache_entries: usize,
    pub cache_enabled: bool,
    pub rate_limit_enabled: bool,
}

/// Utility functions for TeliQue integration
pub mod utils {
    use super::*;
    
    /// Check if CIC is valid format
    pub fn is_valid_cic(cic: &str) -> bool {
        cic.len() == 4 && cic.chars().all(|c| c.is_ascii_alphanumeric())
    }
    
    /// Extract CIC from ANI/billing number
    pub fn extract_cic_from_ani(ani: &str) -> Option<String> {
        // CIC is typically in the first 4 digits after country code
        let normalized = ani.trim_start_matches('+');
        let cleaned: String = normalized.chars().filter(|c| c.is_ascii_digit()).collect();
        
        if cleaned.len() >= 15 { // Country code + area code + number + CIC
            Some(cleaned[11..15].to_string())
        } else {
            None
        }
    }
    
    /// Format LRN result for display
    pub fn format_lrn_result(result: &LrnLookupResult) -> String {
        format!(
            "Number: {} -> LRN: {} ({}{}{})",
            result.number,
            result.lrn,
            if result.ported { "PORTED" } else { "NATIVE" },
            if result.wireless { ", WIRELESS" } else { "" },
            result.provider_name.as_ref().map(|p| format!(", {}", p)).unwrap_or_default()
        )
    }
    
    /// Check if number is likely wireless based on LRN result
    pub fn is_wireless_number(result: &LrnLookupResult) -> bool {
        result.wireless || 
        result.provider_name.as_ref()
            .map(|name| name.to_lowercase().contains("wireless") || 
                       name.to_lowercase().contains("mobile") ||
                       name.to_lowercase().contains("cellular"))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_is_valid_cic() {
        assert!(utils::is_valid_cic("1234"));
        assert!(utils::is_valid_cic("ABCD"));
        assert!(utils::is_valid_cic("12AB"));
        assert!(!utils::is_valid_cic("123"));
        assert!(!utils::is_valid_cic("12345"));
        assert!(!utils::is_valid_cic(""));
    }
    
    #[test]
    fn test_normalize_number() {
        let config = TeliQueConfig::default();
        let client = TeliQueClient::new(config).unwrap();
        
        assert_eq!(client.normalize_number("(212) 555-1234").unwrap(), "12125551234");
        assert_eq!(client.normalize_number("+1-212-555-1234").unwrap(), "12125551234");
        assert_eq!(client.normalize_number("2125551234").unwrap(), "12125551234");
    }
    
    #[tokio::test]
    async fn test_rate_limiter() {
        let mut limiter = RateLimiter::new(60, 10); // 60 requests per minute, burst of 10
        
        // Should allow burst requests
        for _ in 0..10 {
            assert!(limiter.try_acquire());
        }
        
        // Should deny further requests
        assert!(!limiter.try_acquire());
    }
}