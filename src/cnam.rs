/*
 * Redfire Switch - CNAM (Caller ID Name) Service
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc, Duration};
use dashmap::DashMap;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::time::timeout;
use tracing::{debug, info, warn, error};

use crate::lerg_nanpa::LergNanpaService;

/// CNAM service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CnamConfig {
    /// Enable CNAM lookups
    pub enabled: bool,
    /// CNAM provider configuration
    pub provider: CnamProviderConfig,
    /// Local caching configuration
    pub cache: CnamCacheConfig,
    /// CNAM surcharge per lookup in cents
    pub surcharge_cents: f64,
    /// Enable CNAM insertion into From header
    pub insert_in_from_header: bool,
    /// Timeout for CNAM lookups in milliseconds
    pub timeout_ms: u64,
    /// Maximum concurrent lookups
    pub max_concurrent_lookups: u32,
    /// Enable CNAM for DID calls
    pub enable_for_did: bool,
    /// Enable CNAM for toll free calls  
    pub enable_for_toll_free: bool,
    /// Number patterns to exclude from CNAM lookup
    pub exclude_patterns: Vec<String>,
    /// Countries to enable CNAM dipping for (ISO 3166-1 alpha-2 codes)
    pub enabled_countries: Vec<String>,
}

impl Default for CnamConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: CnamProviderConfig::default(),
            cache: CnamCacheConfig::default(),
            surcharge_cents: 0.50, // 0.5 cents per lookup
            insert_in_from_header: true,
            timeout_ms: 5000, // 5 second timeout
            max_concurrent_lookups: 100,
            enable_for_did: true,
            enable_for_toll_free: true,
            exclude_patterns: vec![
                "anonymous".to_string(),
                "unknown".to_string(),
                "restricted".to_string(),
                "unavailable".to_string(),
            ],
            enabled_countries: vec![
                "US".to_string(), // United States
                // Canada can be added by user configuration
                // "CA".to_string(), 
            ],
        }
    }
}

/// CNAM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CnamProviderConfig {
    /// Provider name
    pub name: String,
    /// HTTP GET URL template
    /// Variables: {number}, {api_key}, {user_id}
    pub url_template: String,
    /// HTTP headers to send
    pub headers: HashMap<String, String>,
    /// Authentication method
    pub auth: CnamAuthConfig,
    /// Response format
    pub response_format: CnamResponseFormat,
    /// Retry configuration
    pub retry_config: CnamRetryConfig,
}

impl Default for CnamProviderConfig {
    fn default() -> Self {
        Self {
            name: "default-cnam-provider".to_string(),
            url_template: "https://api.cnam-provider.com/lookup?number={number}&api_key={api_key}".to_string(),
            headers: HashMap::new(),
            auth: CnamAuthConfig::default(),
            response_format: CnamResponseFormat::Json,
            retry_config: CnamRetryConfig::default(),
        }
    }
}

/// CNAM authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CnamAuthConfig {
    /// API key
    pub api_key: Option<String>,
    /// User ID
    pub user_id: Option<String>,
    /// HTTP Basic auth username
    pub basic_username: Option<String>,
    /// HTTP Basic auth password
    pub basic_password: Option<String>,
    /// Bearer token
    pub bearer_token: Option<String>,
}

impl Default for CnamAuthConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            user_id: None,
            basic_username: None,
            basic_password: None,
            bearer_token: None,
        }
    }
}

/// CNAM response formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CnamResponseFormat {
    Json,
    Xml,
    Text,
    Csv,
}

/// CNAM retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CnamRetryConfig {
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Initial retry delay in milliseconds
    pub initial_delay_ms: u64,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Maximum delay between retries in milliseconds
    pub max_delay_ms: u64,
}

impl Default for CnamRetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 2,
            initial_delay_ms: 100,
            backoff_multiplier: 2.0,
            max_delay_ms: 1000,
        }
    }
}

/// CNAM cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CnamCacheConfig {
    /// Enable local caching
    pub enabled: bool,
    /// Cache TTL in seconds for successful lookups
    pub success_ttl_seconds: u64,
    /// Cache TTL in seconds for failed lookups
    pub failure_ttl_seconds: u64,
    /// Maximum cache size (number of entries)
    pub max_size: usize,
    /// Enable persistent cache (save to disk)
    pub persistent: bool,
    /// Persistent cache file path
    pub cache_file: Option<String>,
}

impl Default for CnamCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            success_ttl_seconds: 86400, // 24 hours
            failure_ttl_seconds: 3600,  // 1 hour
            max_size: 100000, // 100k entries
            persistent: false,
            cache_file: Some("data/cnam_cache.json".to_string()),
        }
    }
}

/// CNAM lookup result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CnamResult {
    /// Phone number queried
    pub number: String,
    /// Caller name (if found)
    pub name: Option<String>,
    /// Lookup was successful
    pub success: bool,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Provider used
    pub provider: String,
    /// Lookup cost in cents
    pub cost_cents: f64,
    /// Response time in milliseconds
    pub response_time_ms: u64,
    /// Timestamp of lookup
    pub timestamp: DateTime<Utc>,
    /// Cache hit (not a fresh lookup)
    pub cache_hit: bool,
}

/// CNAM cache entry
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CnamCacheEntry {
    result: CnamResult,
    expires_at: DateTime<Utc>,
}

/// CNAM service statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CnamStats {
    /// Total lookups attempted
    pub total_lookups: u64,
    /// Successful lookups
    pub successful_lookups: u64,
    /// Failed lookups
    pub failed_lookups: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Total cost in cents
    pub total_cost_cents: f64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// Success rate percentage
    pub success_rate: f64,
    /// Cache hit rate percentage
    pub cache_hit_rate: f64,
}

impl Default for CnamStats {
    fn default() -> Self {
        Self {
            total_lookups: 0,
            successful_lookups: 0,
            failed_lookups: 0,
            cache_hits: 0,
            cache_misses: 0,
            total_cost_cents: 0.0,
            avg_response_time_ms: 0.0,
            success_rate: 0.0,
            cache_hit_rate: 0.0,
        }
    }
}

/// CNAM service implementation
pub struct CnamService {
    config: CnamConfig,
    client: Client,
    cache: Arc<DashMap<String, CnamCacheEntry>>,
    stats: Arc<parking_lot::RwLock<CnamStats>>,
    active_lookups: Arc<dashmap::DashMap<String, Instant>>,
    lerg_nanpa_service: Option<Arc<LergNanpaService>>,
}

impl CnamService {
    /// Create a new CNAM service
    pub fn new(config: CnamConfig) -> Result<Self> {
        Self::with_lerg_nanpa(config, None)
    }

    /// Create a new CNAM service with LERG/NANPA integration
    pub fn with_lerg_nanpa(config: CnamConfig, lerg_nanpa_service: Option<Arc<LergNanpaService>>) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_millis(config.timeout_ms))
            .build()?;

        let service = Self {
            config,
            client,
            cache: Arc::new(DashMap::new()),
            stats: Arc::new(parking_lot::RwLock::new(CnamStats::default())),
            active_lookups: Arc::new(DashMap::new()),
            lerg_nanpa_service,
        };

        // Load persistent cache if enabled
        if service.config.cache.persistent {
            if let Err(e) = service.load_cache() {
                warn!("Failed to load CNAM cache: {}", e);
            }
        }

        info!("CNAM service initialized with provider: {}", service.config.provider.name);
        Ok(service)
    }

    /// Perform CNAM lookup
    pub async fn lookup(&self, number: &str, call_id: &str) -> Result<CnamResult> {
        if !self.config.enabled {
            return Ok(CnamResult {
                number: number.to_string(),
                name: None,
                success: false,
                error: Some("CNAM service disabled".to_string()),
                provider: "disabled".to_string(),
                cost_cents: 0.0,
                response_time_ms: 0,
                timestamp: Utc::now(),
                cache_hit: false,
            });
        }

        // Check if number should be excluded
        if self.should_exclude_number(number) {
            debug!("CNAM lookup excluded for number: {}", number);
            return Ok(CnamResult {
                number: number.to_string(),
                name: None,
                success: false,
                error: Some("Number excluded from CNAM lookup".to_string()),
                provider: "excluded".to_string(),
                cost_cents: 0.0,
                response_time_ms: 0,
                timestamp: Utc::now(),
                cache_hit: false,
            });
        }

        let start_time = Instant::now();

        // Check cache first
        if self.config.cache.enabled {
            if let Some(cached) = self.get_from_cache(number) {
                debug!("CNAM cache hit for number: {}", number);
                self.update_stats_cache_hit();
                return Ok(cached);
            }
        }

        // Track active lookup
        self.active_lookups.insert(call_id.to_string(), start_time);

        // Perform fresh lookup
        let result = self.perform_lookup(number, start_time).await;

        // Remove from active lookups
        self.active_lookups.remove(call_id);

        // Cache result if caching is enabled
        if self.config.cache.enabled {
            self.cache_result(&result).await;
        }

        // Update statistics
        self.update_stats(&result);

        Ok(result)
    }

    /// Check if number should be excluded from CNAM lookup
    fn should_exclude_number(&self, number: &str) -> bool {
        let number_lower = number.to_lowercase();
        
        for pattern in &self.config.exclude_patterns {
            if number_lower.contains(&pattern.to_lowercase()) {
                return true;
            }
        }

        // Check for anonymous/restricted patterns
        if number_lower == "anonymous" || 
           number_lower == "unknown" || 
           number_lower == "restricted" ||
           number_lower == "unavailable" ||
           number.is_empty() {
            return true;
        }

        // Check if country is enabled for CNAM dipping
        if let Some(country_code) = self.detect_country_from_number(number) {
            if !self.config.enabled_countries.contains(&country_code) {
                debug!("CNAM lookup disabled for country: {} (number: {})", country_code, number);
                return true;
            }
        } else {
            debug!("Could not determine country for number: {}", number);
            return true; // Exclude if country cannot be determined
        }

        false
    }

    /// Detect country code from phone number using NANPA data
    fn detect_country_from_number(&self, number: &str) -> Option<String> {
        let cleaned = number.trim_start_matches('+')
            .replace(['(', ')', '-', ' ', '.'], "");

        // NANPA (North American Numbering Plan) - US and Canada
        if self.is_nanpa_number(&cleaned) {
            let npa = if cleaned.len() == 11 && cleaned.starts_with('1') {
                &cleaned[1..4]
            } else if cleaned.len() == 10 {
                &cleaned[0..3]
            } else {
                return None;
            };

            // Use NANPA data if available for more accurate country determination
            if let Some(ref lerg_service) = self.lerg_nanpa_service {
                if let Some(nanpa_entry) = lerg_service.get_nanpa_entry(npa) {
                    // Determine country from NANPA location/country field
                    if nanpa_entry.country.to_uppercase().contains("CANADA") || 
                       nanpa_entry.country.to_uppercase() == "CA" {
                        return Some("CA".to_string());
                    } else if nanpa_entry.country.to_uppercase().contains("UNITED STATES") || 
                              nanpa_entry.country.to_uppercase() == "US" {
                        return Some("US".to_string());
                    } else {
                        // Check location field for other NANPA territories
                        let location_upper = nanpa_entry.location.to_uppercase();
                        if location_upper.contains("PUERTO RICO") {
                            return Some("PR".to_string());
                        } else if location_upper.contains("US VIRGIN ISLANDS") {
                            return Some("VI".to_string());
                        } else if location_upper.contains("GUAM") {
                            return Some("GU".to_string());
                        } else if location_upper.contains("NORTHERN MARIANA") {
                            return Some("MP".to_string());
                        } else if location_upper.contains("AMERICAN SAMOA") {
                            return Some("AS".to_string());
                        } else if location_upper.contains("BAHAMAS") {
                            return Some("BS".to_string());
                        } else if location_upper.contains("BARBADOS") {
                            return Some("BB".to_string());
                        } else if location_upper.contains("BERMUDA") {
                            return Some("BM".to_string());
                        } else if location_upper.contains("JAMAICA") {
                            return Some("JM".to_string());
                        }
                        // Default to US for unknown NANPA locations
                        return Some("US".to_string());
                    }
                }
            }

            // Fallback to hardcoded Canadian NPAs if NANPA data not available
            if self.is_canadian_npa(npa) {
                Some("CA".to_string()) // Canada
            } else {
                Some("US".to_string()) // United States
            }
        } else {
            // International numbers - basic country code detection
            self.detect_international_country(&cleaned)
        }
    }

    /// Check if number is in NANPA format
    fn is_nanpa_number(&self, number: &str) -> bool {
        (number.len() == 10 && number.chars().all(|c| c.is_ascii_digit())) ||
        (number.len() == 11 && number.starts_with('1') && number[1..].chars().all(|c| c.is_ascii_digit()))
    }

    /// Check if NPA belongs to Canada
    fn is_canadian_npa(&self, npa: &str) -> bool {
        matches!(npa,
            "204" | "226" | "236" | "249" | "250" | "289" | "306" | "343" | "365" | "403" |
            "416" | "418" | "431" | "437" | "438" | "450" | "468" | "474" | "506" | "514" |
            "519" | "548" | "579" | "581" | "587" | "604" | "613" | "639" | "647" | "672" |
            "705" | "709" | "742" | "778" | "780" | "782" | "807" | "819" | "825" | "867" |
            "873" | "902" | "905"
        )
    }

    /// Detect country from international number
    fn detect_international_country(&self, number: &str) -> Option<String> {
        if number.len() < 2 {
            return None;
        }

        // Basic international country code detection
        match &number[0..2] {
            "44" => Some("GB".to_string()), // United Kingdom
            "49" => Some("DE".to_string()), // Germany
            "33" => Some("FR".to_string()), // France
            "39" => Some("IT".to_string()), // Italy
            "34" => Some("ES".to_string()), // Spain
            "81" => Some("JP".to_string()), // Japan
            "86" => Some("CN".to_string()), // China
            "61" => Some("AU".to_string()), // Australia
            "55" => Some("BR".to_string()), // Brazil
            _ => {
                // Check single digit country codes
                match &number[0..1] {
                    "7" => Some("RU".to_string()), // Russia (and others)
                    _ => None, // Unknown country
                }
            }
        }
    }

    /// Get result from cache
    fn get_from_cache(&self, number: &str) -> Option<CnamResult> {
        if let Some(entry) = self.cache.get(number) {
            if entry.expires_at > Utc::now() {
                let mut result = entry.result.clone();
                result.cache_hit = true;
                result.timestamp = Utc::now(); // Update timestamp for this request
                return Some(result);
            } else {
                // Remove expired entry
                self.cache.remove(number);
            }
        }
        None
    }

    /// Perform actual CNAM lookup
    async fn perform_lookup(&self, number: &str, start_time: Instant) -> CnamResult {
        debug!("Performing CNAM lookup for number: {}", number);

        let mut attempts = 0;
        let mut delay = self.config.provider.retry_config.initial_delay_ms;

        while attempts <= self.config.provider.retry_config.max_retries {
            match self.make_http_request(number).await {
                Ok(name) => {
                    let response_time_ms = start_time.elapsed().as_millis() as u64;
                    info!("CNAM lookup successful for {}: {} ({}ms)", number, name.as_deref().unwrap_or("N/A"), response_time_ms);
                    
                    return CnamResult {
                        number: number.to_string(),
                        name,
                        success: true,
                        error: None,
                        provider: self.config.provider.name.clone(),
                        cost_cents: self.config.surcharge_cents,
                        response_time_ms,
                        timestamp: Utc::now(),
                        cache_hit: false,
                    };
                }
                Err(e) => {
                    attempts += 1;
                    if attempts <= self.config.provider.retry_config.max_retries {
                        warn!("CNAM lookup failed for {} (attempt {}), retrying in {}ms: {}", 
                              number, attempts, delay, e);
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                        delay = (delay as f64 * self.config.provider.retry_config.backoff_multiplier) as u64;
                        delay = delay.min(self.config.provider.retry_config.max_delay_ms);
                    } else {
                        let response_time_ms = start_time.elapsed().as_millis() as u64;
                        error!("CNAM lookup failed for {} after {} attempts: {}", number, attempts, e);
                        
                        return CnamResult {
                            number: number.to_string(),
                            name: None,
                            success: false,
                            error: Some(e.to_string()),
                            provider: self.config.provider.name.clone(),
                            cost_cents: 0.0, // No charge for failed lookups
                            response_time_ms,
                            timestamp: Utc::now(),
                            cache_hit: false,
                        };
                    }
                }
            }
        }

        unreachable!()
    }

    /// Make HTTP request to CNAM provider
    async fn make_http_request(&self, number: &str) -> Result<Option<String>> {
        // Build URL from template
        let url = self.build_url(number)?;
        
        debug!("Making CNAM HTTP request to: {}", url);

        let mut request = self.client.get(&url);

        // Add headers
        for (key, value) in &self.config.provider.headers {
            request = request.header(key, value);
        }

        // Add authentication
        if let Some(ref username) = self.config.provider.auth.basic_username {
            if let Some(ref password) = self.config.provider.auth.basic_password {
                request = request.basic_auth(username, Some(password));
            }
        }

        if let Some(ref token) = self.config.provider.auth.bearer_token {
            request = request.bearer_auth(token);
        }

        // Make request with timeout
        let response = timeout(
            tokio::time::Duration::from_millis(self.config.timeout_ms),
            request.send()
        ).await??;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP {} from CNAM provider", response.status()));
        }

        let response_text = response.text().await?;
        
        // Parse response based on format
        self.parse_response(&response_text)
    }

    /// Build URL from template
    fn build_url(&self, number: &str) -> Result<String> {
        let mut url = self.config.provider.url_template.clone();
        
        // Replace variables
        url = url.replace("{number}", number);
        
        if let Some(ref api_key) = self.config.provider.auth.api_key {
            url = url.replace("{api_key}", api_key);
        }
        
        if let Some(ref user_id) = self.config.provider.auth.user_id {
            url = url.replace("{user_id}", user_id);
        }

        // Basic URL validation
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(anyhow!("Invalid CNAM URL: must start with http:// or https://"));
        }

        Ok(url)
    }

    /// Parse CNAM response based on format
    fn parse_response(&self, response: &str) -> Result<Option<String>> {
        match self.config.provider.response_format {
            CnamResponseFormat::Json => self.parse_json_response(response),
            CnamResponseFormat::Text => Ok(Some(response.trim().to_string())),
            CnamResponseFormat::Xml => self.parse_xml_response(response),
            CnamResponseFormat::Csv => self.parse_csv_response(response),
        }
    }

    /// Parse JSON response
    fn parse_json_response(&self, response: &str) -> Result<Option<String>> {
        let json: serde_json::Value = serde_json::from_str(response)?;
        
        // Try common field names
        let name_fields = ["name", "caller_name", "display_name", "cnam", "caller_id_name"];
        
        for field in &name_fields {
            if let Some(name) = json.get(field) {
                if let Some(name_str) = name.as_str() {
                    if !name_str.is_empty() && name_str != "UNKNOWN" && name_str != "N/A" {
                        return Ok(Some(name_str.to_string()));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Parse XML response (simplified)
    fn parse_xml_response(&self, response: &str) -> Result<Option<String>> {
        // Simple XML parsing - in production, use proper XML parser
        if let Some(start) = response.find("<name>") {
            if let Some(end) = response[start + 6..].find("</name>") {
                let name = &response[start + 6..start + 6 + end];
                if !name.is_empty() && name != "UNKNOWN" {
                    return Ok(Some(name.to_string()));
                }
            }
        }
        Ok(None)
    }

    /// Parse CSV response
    fn parse_csv_response(&self, response: &str) -> Result<Option<String>> {
        let lines: Vec<&str> = response.lines().collect();
        if lines.len() >= 2 {
            // Assume first column is number, second is name
            let fields: Vec<&str> = lines[1].split(',').collect();
            if fields.len() >= 2 {
                let name = fields[1].trim().trim_matches('"');
                if !name.is_empty() && name != "UNKNOWN" {
                    return Ok(Some(name.to_string()));
                }
            }
        }
        Ok(None)
    }

    /// Cache result
    async fn cache_result(&self, result: &CnamResult) {
        if !self.config.cache.enabled {
            return;
        }

        let ttl = if result.success {
            self.config.cache.success_ttl_seconds
        } else {
            self.config.cache.failure_ttl_seconds
        };

        let expires_at = Utc::now() + Duration::seconds(ttl as i64);
        
        let entry = CnamCacheEntry {
            result: result.clone(),
            expires_at,
        };

        // Check cache size limit
        if self.cache.len() >= self.config.cache.max_size {
            self.evict_expired_entries();
            
            // If still at limit, remove oldest entries
            if self.cache.len() >= self.config.cache.max_size {
                // Simple eviction - remove 10% of entries
                let to_remove = self.cache.len() / 10;
                let keys_to_remove: Vec<String> = self.cache.iter()
                    .take(to_remove)
                    .map(|entry| entry.key().clone())
                    .collect();
                
                for key in keys_to_remove {
                    self.cache.remove(&key);
                }
            }
        }

        self.cache.insert(result.number.clone(), entry);
        debug!("Cached CNAM result for {}, expires at {}", result.number, expires_at);
    }

    /// Evict expired cache entries
    fn evict_expired_entries(&self) {
        let now = Utc::now();
        let expired_keys: Vec<String> = self.cache.iter()
            .filter(|entry| entry.value().expires_at <= now)
            .map(|entry| entry.key().clone())
            .collect();

        for key in expired_keys {
            self.cache.remove(&key);
        }
    }

    /// Update statistics for cache hit
    fn update_stats_cache_hit(&self) {
        let mut stats = self.stats.write();
        stats.total_lookups += 1;
        stats.cache_hits += 1;
        stats.cache_hit_rate = (stats.cache_hits as f64 / stats.total_lookups as f64) * 100.0;
    }

    /// Update statistics
    fn update_stats(&self, result: &CnamResult) {
        let mut stats = self.stats.write();
        stats.total_lookups += 1;
        stats.cache_misses += 1;
        
        if result.success {
            stats.successful_lookups += 1;
            stats.total_cost_cents += result.cost_cents;
        } else {
            stats.failed_lookups += 1;
        }

        // Update averages
        stats.success_rate = (stats.successful_lookups as f64 / stats.total_lookups as f64) * 100.0;
        stats.cache_hit_rate = (stats.cache_hits as f64 / stats.total_lookups as f64) * 100.0;
        
        let total_response_time = stats.avg_response_time_ms * (stats.cache_misses - 1) as f64 + result.response_time_ms as f64;
        stats.avg_response_time_ms = total_response_time / stats.cache_misses as f64;
    }

    /// Load persistent cache from disk
    fn load_cache(&self) -> Result<()> {
        if let Some(ref cache_file) = self.config.cache.cache_file {
            if std::path::Path::new(cache_file).exists() {
                let content = std::fs::read_to_string(cache_file)?;
                let entries: HashMap<String, CnamCacheEntry> = serde_json::from_str(&content)?;
                
                let now = Utc::now();
                let mut loaded_count = 0;
                
                for (number, entry) in entries {
                    if entry.expires_at > now {
                        self.cache.insert(number, entry);
                        loaded_count += 1;
                    }
                }
                
                info!("Loaded {} CNAM cache entries from {}", loaded_count, cache_file);
            }
        }
        Ok(())
    }

    /// Save persistent cache to disk
    pub async fn save_cache(&self) -> Result<()> {
        if !self.config.cache.persistent {
            return Ok(());
        }

        if let Some(ref cache_file) = self.config.cache.cache_file {
            // Create directory if it doesn't exist
            if let Some(parent) = std::path::Path::new(cache_file).parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Convert DashMap to HashMap for serialization
            let cache_data: HashMap<String, CnamCacheEntry> = self.cache.iter()
                .map(|entry| (entry.key().clone(), entry.value().clone()))
                .collect();

            let content = serde_json::to_string_pretty(&cache_data)?;
            std::fs::write(cache_file, content)?;
            
            info!("Saved {} CNAM cache entries to {}", cache_data.len(), cache_file);
        }

        Ok(())
    }

    /// Check if CNAM is enabled for a specific country
    pub fn is_country_enabled(&self, country_code: &str) -> bool {
        self.config.enabled_countries.contains(&country_code.to_uppercase())
    }

    /// Get list of enabled countries
    pub fn get_enabled_countries(&self) -> &Vec<String> {
        &self.config.enabled_countries
    }

    /// Test country detection for a number
    pub fn test_country_detection(&self, number: &str) -> Option<String> {
        self.detect_country_from_number(number)
    }

    /// Check if LERG/NANPA integration is available
    pub fn has_lerg_nanpa_integration(&self) -> bool {
        self.lerg_nanpa_service.is_some()
    }

    /// Get LERG/NANPA data status
    pub fn get_lerg_nanpa_status(&self) -> (bool, usize, usize) {
        match &self.lerg_nanpa_service {
            Some(service) => (true, service.get_lerg_count(), service.get_nanpa_count()),
            None => (false, 0, 0),
        }
    }

    /// Get service statistics
    pub fn get_stats(&self) -> CnamStats {
        self.stats.read().clone()
    }

    /// Get cache size
    pub fn get_cache_size(&self) -> usize {
        self.cache.len()
    }

    /// Clear cache
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Get active lookups count
    pub fn get_active_lookups_count(&self) -> usize {
        self.active_lookups.len()
    }


    /// Format From header with CNAM
    pub fn format_from_header_with_cnam(&self, original_from: &str, cnam_result: &CnamResult) -> String {
        if !self.config.insert_in_from_header || !cnam_result.success {
            return original_from.to_string();
        }

        if let Some(ref name) = cnam_result.name {
            // Parse existing From header
            if let Some(uri_start) = original_from.find('<') {
                let display_name = original_from[..uri_start].trim();
                let uri_part = &original_from[uri_start..];
                
                // Replace or add display name
                if display_name.is_empty() {
                    format!("\"{}\" {}", name, uri_part)
                } else {
                    format!("\"{}\" {}", name, uri_part)
                }
            } else {
                // No angle brackets, assume it's just a URI
                format!("\"{}\" <{}>", name, original_from)
            }
        } else {
            original_from.to_string()
        }
    }
}

/// CNAM service utilities
pub mod utils {
    use super::*;

    /// Create default CNAM configuration
    pub fn create_default_cnam_config() -> CnamConfig {
        CnamConfig::default()
    }

    /// Create CNAM configuration for specific provider
    pub fn create_provider_config(provider_name: &str) -> CnamProviderConfig {
        match provider_name.to_lowercase().as_str() {
            "opencnam" => CnamProviderConfig {
                name: "OpenCNAM".to_string(),
                url_template: "https://api.opencnam.com/v3/phone/{number}?format=text".to_string(),
                headers: [("User-Agent".to_string(), "Redfire-Switch/1.0".to_string())].iter().cloned().collect(),
                auth: CnamAuthConfig::default(),
                response_format: CnamResponseFormat::Text,
                retry_config: CnamRetryConfig::default(),
            },
            "neustar" => CnamProviderConfig {
                name: "Neustar".to_string(),
                url_template: "https://api.neustar.biz/cnam/lookup?tn={number}&user_id={user_id}&api_key={api_key}".to_string(),
                headers: HashMap::new(),
                auth: CnamAuthConfig::default(),
                response_format: CnamResponseFormat::Json,
                retry_config: CnamRetryConfig::default(),
            },
            "iconectiv" => CnamProviderConfig {
                name: "iconectiv".to_string(),
                url_template: "https://api.iconectiv.com/cnam/v1/lookup/{number}".to_string(),
                headers: [("Content-Type".to_string(), "application/json".to_string())].iter().cloned().collect(),
                auth: CnamAuthConfig::default(),
                response_format: CnamResponseFormat::Json,
                retry_config: CnamRetryConfig::default(),
            },
            _ => CnamProviderConfig::default(),
        }
    }

    /// Validate phone number for CNAM lookup
    pub fn is_valid_for_cnam_lookup(number: &str) -> bool {
        // Remove common prefixes and formatting
        let cleaned = number.trim_start_matches('+')
            .trim_start_matches('1')
            .replace(['(', ')', '-', ' ', '.'], "");

        // Must be 10 digits for NANPA numbers
        cleaned.len() == 10 && cleaned.chars().all(|c| c.is_ascii_digit())
    }

    /// Extract NANPA number for CNAM lookup
    pub fn extract_nanpa_number(number: &str) -> Option<String> {
        let cleaned = number.trim_start_matches('+')
            .replace(['(', ')', '-', ' ', '.'], "");

        if cleaned.len() == 11 && cleaned.starts_with('1') {
            Some(cleaned[1..].to_string()) // Return 10-digit number
        } else if cleaned.len() == 10 && cleaned.chars().all(|c| c.is_ascii_digit()) {
            Some(cleaned)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phone_number_validation() {
        assert!(utils::is_valid_for_cnam_lookup("15551234567"));
        assert!(utils::is_valid_for_cnam_lookup("+1 (555) 123-4567"));
        assert!(utils::is_valid_for_cnam_lookup("555-123-4567"));
        assert!(!utils::is_valid_for_cnam_lookup("442071234567")); // UK number
        assert!(!utils::is_valid_for_cnam_lookup("123")); // Too short
    }

    #[test]
    fn test_nanpa_number_extraction() {
        assert_eq!(utils::extract_nanpa_number("15551234567"), Some("5551234567".to_string()));
        assert_eq!(utils::extract_nanpa_number("+1-555-123-4567"), Some("5551234567".to_string()));
        assert_eq!(utils::extract_nanpa_number("(555) 123-4567"), Some("5551234567".to_string()));
        assert_eq!(utils::extract_nanpa_number("442071234567"), None);
    }

    #[tokio::test]
    async fn test_country_detection() {
        let service = CnamService::new(CnamConfig::default()).unwrap();
        
        // US numbers
        assert_eq!(service.test_country_detection("15551234567"), Some("US".to_string()));
        assert_eq!(service.test_country_detection("+1-555-123-4567"), Some("US".to_string()));
        assert_eq!(service.test_country_detection("(555) 123-4567"), Some("US".to_string()));
        
        // Canadian numbers (using Toronto area code 416)
        assert_eq!(service.test_country_detection("14161234567"), Some("CA".to_string()));
        assert_eq!(service.test_country_detection("+1-416-123-4567"), Some("CA".to_string()));
        
        // International numbers
        assert_eq!(service.test_country_detection("442071234567"), Some("GB".to_string()));
        assert_eq!(service.test_country_detection("+44-20-7123-4567"), Some("GB".to_string()));
        assert_eq!(service.test_country_detection("49301234567"), Some("DE".to_string()));
        
        // Unknown/invalid numbers
        assert_eq!(service.test_country_detection("123"), None);
        assert_eq!(service.test_country_detection("999123456789"), None);
    }

    #[tokio::test]
    async fn test_country_filtering() {
        let mut config = CnamConfig::default();
        config.enabled_countries = vec!["US".to_string()]; // Only US enabled
        
        let service = CnamService::new(config).unwrap();
        
        // US numbers should not be excluded
        assert!(!service.should_exclude_number("15551234567"));
        assert!(!service.should_exclude_number("+1-555-123-4567"));
        
        // Canadian numbers should be excluded (not in enabled_countries)
        assert!(service.should_exclude_number("14161234567"));
        assert!(service.should_exclude_number("+1-416-123-4567"));
        
        // International numbers should be excluded
        assert!(service.should_exclude_number("442071234567"));
        assert!(service.should_exclude_number("+44-20-7123-4567"));
    }

    #[tokio::test]
    async fn test_enabled_countries_check() {
        let mut config = CnamConfig::default();
        config.enabled_countries = vec!["US".to_string(), "CA".to_string()];
        
        let service = CnamService::new(config).unwrap();
        
        assert!(service.is_country_enabled("US"));
        assert!(service.is_country_enabled("us")); // Case insensitive
        assert!(service.is_country_enabled("CA"));
        assert!(!service.is_country_enabled("GB"));
        assert!(!service.is_country_enabled("DE"));
        
        assert_eq!(service.get_enabled_countries(), &vec!["US".to_string(), "CA".to_string()]);
    }

    #[test]
    fn test_from_header_formatting() {
        let config = CnamConfig::default();
        let service = CnamService::new(config).unwrap();
        
        let cnam_result = CnamResult {
            number: "5551234567".to_string(),
            name: Some("John Doe".to_string()),
            success: true,
            error: None,
            provider: "test".to_string(),
            cost_cents: 0.5,
            response_time_ms: 100,
            timestamp: Utc::now(),
            cache_hit: false,
        };

        let formatted = service.format_from_header_with_cnam(
            "<sip:5551234567@example.com>",
            &cnam_result
        );
        
        assert_eq!(formatted, "\"John Doe\" <sip:5551234567@example.com>");
    }
}