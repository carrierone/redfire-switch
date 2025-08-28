//! Origination Routing Engine
//! Handles incoming call routing decisions based on ANI (caller) information

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use tracing::{debug, info, warn};

use crate::lcr::types::{EgressTrunk, RouteType};
use crate::security_utils::validate_phone_number;

/// Origination routing request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginationRequest {
    pub ani: String,              // Calling number
    pub dnis: String,             // Dialed number
    pub source_ip: IpAddr,        // Source IP address
    pub ingress_trunk_id: i32,    // Ingress trunk identifier
    pub customer_id: Option<i32>, // Customer account ID
    pub route_type: RouteType,    // Routing preference
    pub timestamp: DateTime<Utc>, // Request timestamp
}

/// Origination routing response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginationResponse {
    pub allowed: bool,                   // Call permitted
    pub customer_id: Option<i32>,        // Resolved customer
    pub rate_plan_id: Option<i32>,       // Rate plan to use
    pub routing_plan_id: Option<i32>,    // Routing plan to use
    pub auth_result: AuthResult,         // Authentication result
    pub fraud_check_result: FraudResult, // Fraud analysis
    pub routing_preference: RouteType,   // Preferred routing type
    pub tech_prefix: Option<String>,     // Tech prefix to strip/add
    pub reason: String,                  // Decision reason
}

/// Authentication result for origination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResult {
    pub authenticated: bool,
    pub auth_method: AuthMethod,
    pub customer_account: Option<String>,
    pub credit_limit: Option<f64>,
    pub current_balance: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    IpBased,          // IP-based authentication
    TechnicalPrefix,  // Tech prefix authentication
    DigitalSignature, // STIR/SHAKEN authentication
    None,             // No authentication required
}

/// Fraud detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudResult {
    pub risk_level: RiskLevel,
    pub fraud_score: f32, // 0.0-1.0 fraud probability
    pub checks_performed: Vec<String>,
    pub flags: Vec<FraudFlag>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FraudFlag {
    SuspiciousAni,
    HighVolumeSource,
    InvalidNumberFormat,
    BlacklistedSource,
    RapidFireCalls,
    GeographicAnomaly,
}

/// Origination routing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginationConfig {
    pub enable_fraud_detection: bool,
    pub enable_ip_authentication: bool,
    pub enable_tech_prefix_auth: bool,
    pub default_route_type: RouteType,
    pub max_fraud_score: f32,
    pub rate_limit_per_minute: u32,
}

impl Default for OriginationConfig {
    fn default() -> Self {
        Self {
            enable_fraud_detection: true,
            enable_ip_authentication: true,
            enable_tech_prefix_auth: true,
            default_route_type: RouteType::NANPA,
            max_fraud_score: 0.8,
            rate_limit_per_minute: 1000,
        }
    }
}

/// Customer account information for origination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerAccount {
    pub id: i32,
    pub name: String,
    pub enabled: bool,
    pub rate_plan_id: i32,
    pub routing_plan_id: i32,
    pub credit_limit: f64,
    pub current_balance: f64,
    pub allowed_ips: Vec<IpAddr>,
    pub tech_prefixes: Vec<String>,
    pub rate_limit_per_minute: u32,
}

/// Toll-free prefix configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TollFreePrefix {
    pub prefix: String,
    pub description: String,
    pub route_type: RouteType,
    pub requires_auth: bool,
}

/// Origination routing engine
pub struct OriginationRoutingEngine {
    config: OriginationConfig,
    customers: HashMap<i32, CustomerAccount>,
    ip_to_customer: HashMap<IpAddr, i32>,
    tech_prefix_to_customer: HashMap<String, i32>,
    toll_free_prefixes: Vec<TollFreePrefix>,
    call_counts: HashMap<IpAddr, u32>, // Simple rate limiting
}

impl OriginationRoutingEngine {
    pub fn new(config: OriginationConfig) -> Self {
        Self {
            config,
            customers: HashMap::new(),
            ip_to_customer: HashMap::new(),
            tech_prefix_to_customer: HashMap::new(),
            toll_free_prefixes: Self::default_toll_free_prefixes(),
            call_counts: HashMap::new(),
        }
    }

    /// Add customer account
    pub fn add_customer(&mut self, customer: CustomerAccount) {
        let customer_id = customer.id;

        // Map IP addresses to customer
        for ip in &customer.allowed_ips {
            self.ip_to_customer.insert(*ip, customer_id);
        }

        // Map tech prefixes to customer
        for prefix in &customer.tech_prefixes {
            self.tech_prefix_to_customer
                .insert(prefix.clone(), customer_id);
        }

        let customer_name = customer.name.clone();
        self.customers.insert(customer_id, customer);
        info!("Added customer {} with ID {}", customer_name, customer_id);
    }

    /// Route origination request
    pub async fn route_origination(
        &mut self,
        request: OriginationRequest,
    ) -> Result<OriginationResponse> {
        info!(
            "Processing origination request: {} -> {} from {}",
            request.ani, request.dnis, request.source_ip
        );

        // 1. Rate limiting check
        if !self.check_rate_limit(&request.source_ip) {
            return Ok(OriginationResponse {
                allowed: false,
                customer_id: None,
                rate_plan_id: None,
                routing_plan_id: None,
                auth_result: AuthResult {
                    authenticated: false,
                    auth_method: AuthMethod::None,
                    customer_account: None,
                    credit_limit: None,
                    current_balance: None,
                },
                fraud_check_result: FraudResult {
                    risk_level: RiskLevel::Critical,
                    fraud_score: 1.0,
                    checks_performed: vec!["rate_limit".to_string()],
                    flags: vec![FraudFlag::RapidFireCalls],
                },
                routing_preference: self.config.default_route_type,
                tech_prefix: None,
                reason: "Rate limit exceeded".to_string(),
            });
        }

        // 2. Authentication
        let auth_result = self.authenticate(&request).await?;

        // 3. Fraud detection
        let fraud_result = if self.config.enable_fraud_detection {
            self.detect_fraud(&request, &auth_result).await?
        } else {
            FraudResult {
                risk_level: RiskLevel::Low,
                fraud_score: 0.0,
                checks_performed: vec![],
                flags: vec![],
            }
        };

        // 4. Authorization decision
        let allowed = auth_result.authenticated
            && fraud_result.fraud_score <= self.config.max_fraud_score
            && fraud_result.risk_level != RiskLevel::Critical;

        // 5. Determine routing preference
        let routing_preference = self.determine_route_type(&request);

        // 6. Get customer details for routing/rating
        let (rate_plan_id, routing_plan_id) =
            if let Some(customer_id) = auth_result.customer_account.as_ref() {
                if let Ok(id) = customer_id.parse::<i32>() {
                    if let Some(customer) = self.customers.get(&id) {
                        (Some(customer.rate_plan_id), Some(customer.routing_plan_id))
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

        let customer_id = auth_result
            .customer_account
            .clone()
            .and_then(|id| id.parse().ok());

        let response = OriginationResponse {
            allowed,
            customer_id,
            rate_plan_id,
            routing_plan_id,
            auth_result,
            fraud_check_result: fraud_result,
            routing_preference,
            tech_prefix: self.extract_tech_prefix(&request.dnis),
            reason: if allowed {
                "Call authorized".to_string()
            } else {
                "Call blocked by security policy".to_string()
            },
        };

        debug!("Origination response: {:?}", response);
        Ok(response)
    }

    /// Authenticate origination request
    async fn authenticate(&self, request: &OriginationRequest) -> Result<AuthResult> {
        // Try IP-based authentication first
        if self.config.enable_ip_authentication {
            if let Some(&customer_id) = self.ip_to_customer.get(&request.source_ip) {
                if let Some(customer) = self.customers.get(&customer_id) {
                    if customer.enabled {
                        return Ok(AuthResult {
                            authenticated: true,
                            auth_method: AuthMethod::IpBased,
                            customer_account: Some(customer_id.to_string()),
                            credit_limit: Some(customer.credit_limit),
                            current_balance: Some(customer.current_balance),
                        });
                    }
                }
            }
        }

        // Try tech prefix authentication
        if self.config.enable_tech_prefix_auth {
            if let Some(tech_prefix) = self.extract_tech_prefix(&request.dnis) {
                if let Some(&customer_id) = self.tech_prefix_to_customer.get(&tech_prefix) {
                    if let Some(customer) = self.customers.get(&customer_id) {
                        if customer.enabled {
                            return Ok(AuthResult {
                                authenticated: true,
                                auth_method: AuthMethod::TechnicalPrefix,
                                customer_account: Some(customer_id.to_string()),
                                credit_limit: Some(customer.credit_limit),
                                current_balance: Some(customer.current_balance),
                            });
                        }
                    }
                }
            }
        }

        // No authentication found
        Ok(AuthResult {
            authenticated: false,
            auth_method: AuthMethod::None,
            customer_account: None,
            credit_limit: None,
            current_balance: None,
        })
    }

    /// Detect potential fraud
    async fn detect_fraud(
        &self,
        request: &OriginationRequest,
        auth_result: &AuthResult,
    ) -> Result<FraudResult> {
        let mut fraud_score = 0.0;
        let mut flags = Vec::new();
        let mut checks = Vec::new();

        // Check ANI format
        checks.push("ani_format".to_string());
        if validate_phone_number(&request.ani).is_err() {
            fraud_score += 0.3;
            flags.push(FraudFlag::InvalidNumberFormat);
        }

        // Check for suspicious ANI patterns
        checks.push("ani_pattern".to_string());
        if self.is_suspicious_ani(&request.ani) {
            fraud_score += 0.4;
            flags.push(FraudFlag::SuspiciousAni);
        }

        // Check call volume from source
        checks.push("call_volume".to_string());
        if let Some(&count) = self.call_counts.get(&request.source_ip) {
            if count > 100 {
                // High volume threshold
                fraud_score += 0.2;
                flags.push(FraudFlag::HighVolumeSource);
            }
        }

        // Check if authenticated but from suspicious source
        if !auth_result.authenticated {
            fraud_score += 0.5;
        }

        // Determine risk level
        let risk_level = match fraud_score {
            score if score >= 0.8 => RiskLevel::Critical,
            score if score >= 0.6 => RiskLevel::High,
            score if score >= 0.3 => RiskLevel::Medium,
            _ => RiskLevel::Low,
        };

        Ok(FraudResult {
            risk_level,
            fraud_score,
            checks_performed: checks,
            flags,
        })
    }

    /// Check rate limiting
    fn check_rate_limit(&mut self, source_ip: &IpAddr) -> bool {
        let count = self.call_counts.entry(*source_ip).or_insert(0);
        *count += 1;

        // Simple rate limiting - in production this would be time-based
        *count <= self.config.rate_limit_per_minute
    }

    /// Determine route type based on DNIS
    fn determine_route_type(&self, request: &OriginationRequest) -> RouteType {
        // Check for toll-free
        for tf_prefix in &self.toll_free_prefixes {
            if request.dnis.starts_with(&tf_prefix.prefix) {
                return tf_prefix.route_type;
            }
        }

        // Check for international
        if request.dnis.starts_with("+") || request.dnis.starts_with("011") {
            return RouteType::AZ;
        }

        // Default to specified or NANPA
        if request.route_type != RouteType::OTHER {
            request.route_type
        } else {
            self.config.default_route_type
        }
    }

    /// Extract tech prefix from DNIS
    fn extract_tech_prefix(&self, dnis: &str) -> Option<String> {
        // Look for patterns like *1001* or 1001*
        if dnis.starts_with('*') {
            if let Some(end) = dnis[1..].find('*') {
                return Some(dnis[1..end + 1].to_string());
            }
        } else if let Some(star_pos) = dnis.find('*') {
            return Some(dnis[..star_pos].to_string());
        }
        None
    }

    /// Check for suspicious ANI patterns
    fn is_suspicious_ani(&self, ani: &str) -> bool {
        // Common suspicious patterns
        ani == "0000000000"
            || ani == "1111111111"
            || ani.starts_with("000")
            || ani.len() < 7
            || ani.chars().all(|c| c == ani.chars().next().unwrap())
    }

    /// Default toll-free prefixes
    fn default_toll_free_prefixes() -> Vec<TollFreePrefix> {
        vec![
            TollFreePrefix {
                prefix: "800".to_string(),
                description: "Toll Free 800".to_string(),
                route_type: RouteType::NANPA,
                requires_auth: false,
            },
            TollFreePrefix {
                prefix: "888".to_string(),
                description: "Toll Free 888".to_string(),
                route_type: RouteType::NANPA,
                requires_auth: false,
            },
            TollFreePrefix {
                prefix: "877".to_string(),
                description: "Toll Free 877".to_string(),
                route_type: RouteType::NANPA,
                requires_auth: false,
            },
            TollFreePrefix {
                prefix: "866".to_string(),
                description: "Toll Free 866".to_string(),
                route_type: RouteType::NANPA,
                requires_auth: false,
            },
        ]
    }

    /// Reset call counts (would be called periodically)
    pub fn reset_call_counts(&mut self) {
        self.call_counts.clear();
    }
}

/// Utility functions
pub mod utils {
    use super::*;

    /// Create test customer account
    pub fn create_test_customer(id: i32, name: &str, ip: IpAddr) -> CustomerAccount {
        CustomerAccount {
            id,
            name: name.to_string(),
            enabled: true,
            rate_plan_id: 1,
            routing_plan_id: 1,
            credit_limit: 10000.0,
            current_balance: 5000.0,
            allowed_ips: vec![ip],
            tech_prefixes: vec![format!("{}*", id)],
            rate_limit_per_minute: 1000,
        }
    }

    /// Create test origination request
    pub fn create_test_request(ani: &str, dnis: &str, source_ip: IpAddr) -> OriginationRequest {
        OriginationRequest {
            ani: ani.to_string(),
            dnis: dnis.to_string(),
            source_ip,
            ingress_trunk_id: 1,
            customer_id: None,
            route_type: RouteType::NANPA,
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_origination_routing_auth_success() {
        let config = OriginationConfig::default();
        let mut engine = OriginationRoutingEngine::new(config);

        let test_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        let customer = utils::create_test_customer(1, "Test Customer", test_ip);
        engine.add_customer(customer);

        let request = utils::create_test_request("15551234567", "18005551234", test_ip);
        let response = engine.route_origination(request).await.unwrap();

        assert!(response.allowed);
        assert!(response.auth_result.authenticated);
        assert_eq!(response.customer_id, Some(1));
    }

    #[tokio::test]
    async fn test_origination_routing_auth_failure() {
        let config = OriginationConfig::default();
        let mut engine = OriginationRoutingEngine::new(config);

        let unauthorized_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let request = utils::create_test_request("15551234567", "18005551234", unauthorized_ip);
        let response = engine.route_origination(request).await.unwrap();

        assert!(!response.allowed);
        assert!(!response.auth_result.authenticated);
        assert_eq!(response.customer_id, None);
    }

    #[tokio::test]
    async fn test_fraud_detection() {
        let config = OriginationConfig::default();
        let mut engine = OriginationRoutingEngine::new(config);

        let test_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        let request = utils::create_test_request("0000000000", "18005551234", test_ip);
        let response = engine.route_origination(request).await.unwrap();

        assert!(!response.allowed);
        assert!(response.fraud_check_result.fraud_score > 0.0);
        assert!(response
            .fraud_check_result
            .flags
            .contains(&FraudFlag::SuspiciousAni));
    }

    #[test]
    fn test_tech_prefix_extraction() {
        let config = OriginationConfig::default();
        let engine = OriginationRoutingEngine::new(config);

        assert_eq!(
            engine.extract_tech_prefix("*1001*15551234567"),
            Some("1001".to_string())
        );
        assert_eq!(
            engine.extract_tech_prefix("1001*15551234567"),
            Some("1001".to_string())
        );
        assert_eq!(engine.extract_tech_prefix("15551234567"), None);
    }
}
