/*
 * Redfire Switch - Customer Management System
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use crate::stir_shaken::AttestationLevel;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Customer profile in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    /// Unique customer identifier
    pub customer_id: String,
    /// Customer name
    pub name: String,
    /// Customer type (Retail, Wholesale, Enterprise, etc.)
    pub customer_type: CustomerType,
    /// Whether customer account is active
    pub active: bool,
    /// Customer created date
    pub created_at: DateTime<Utc>,
    /// Last updated date
    pub updated_at: DateTime<Utc>,
    /// Billing information
    pub billing_info: BillingInfo,
    /// STIR/SHAKEN attestation settings
    pub stir_shaken_settings: StirShakenCustomerSettings,
    /// Associated trunk groups
    pub termination_trunks: Vec<String>, // Trunk IDs for termination
    pub origination_trunks: Vec<String>, // Trunk IDs for origination
    /// Owned ANI/DID ranges
    pub owned_anis: Vec<String>, // ANI numbers or ranges
    pub owned_dids: Vec<String>, // DID numbers or ranges
    /// Customer-specific routing settings
    pub routing_settings: CustomerRoutingSettings,
}

/// Customer types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CustomerType {
    /// Retail customer (end user)
    Retail,
    /// Wholesale customer (carrier)
    Wholesale,
    /// Enterprise customer (business)
    Enterprise,
    /// Origination provider
    OriginationProvider,
    /// Termination provider
    TerminationProvider,
    /// Full service provider (both origination and termination)
    FullService,
}

/// Billing information for customer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillingInfo {
    /// Billing contact name
    pub contact_name: String,
    /// Billing email
    pub contact_email: String,
    /// Billing cycle (Monthly, Weekly, etc.)
    pub billing_cycle: BillingCycle,
    /// Credit limit in cents
    pub credit_limit: i64,
    /// Current balance in cents
    pub current_balance: i64,
    /// Auto-payment enabled
    pub auto_payment: bool,
}

/// Billing cycle options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BillingCycle {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    Annual,
}

/// Customer-specific STIR/SHAKEN settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StirShakenCustomerSettings {
    /// Enable STIR/SHAKEN for this customer
    pub enabled: bool,
    /// Default attestation level for customer's calls
    pub default_attestation: AttestationLevel,
    /// Automatically set attestation to A if ANI is in DID table
    pub auto_attest_did_ani: bool,
    /// Use customer-specific ANI attestation database
    pub use_ani_database: bool,
    /// Inherit attestation from termination to origination
    pub inherit_termination_attestation: bool,
    /// Customer-specific certificate ID
    pub preferred_cert_id: Option<String>,
}

/// Customer routing settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerRoutingSettings {
    /// Priority level (1-10, 1 = highest)
    pub priority: u8,
    /// Enable LCR (Least Cost Routing)
    pub enable_lcr: bool,
    /// Enable quality-based routing
    pub enable_quality_routing: bool,
    /// Maximum concurrent calls allowed
    pub max_concurrent_calls: u32,
    /// Rate limiting (calls per second)
    pub rate_limit_cps: Option<u32>,
    /// Allowed destination patterns
    pub allowed_destinations: Vec<String>,
    /// Blocked destination patterns
    pub blocked_destinations: Vec<String>,
}

/// ANI ownership and attestation mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AniOwnership {
    /// ANI number or range
    pub ani: String,
    /// Customer who owns this ANI
    pub customer_id: String,
    /// Attestation level to use automatically
    pub attestation_level: AttestationLevel,
    /// Whether this ANI is verified
    pub verified: bool,
    /// Source of verification (Internal, Carrier, etc.)
    pub verification_source: VerificationSource,
    /// Date added/updated
    pub updated_at: DateTime<Utc>,
    /// Associated termination trunk (where calls from this ANI originate)
    pub termination_trunk_id: Option<String>,
    /// Associated origination trunk (where calls to this ANI terminate)
    pub origination_trunk_id: Option<String>,
}

/// Source of ANI verification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VerificationSource {
    /// Internally verified (customer owns the number)
    Internal,
    /// Verified by upstream carrier
    Carrier,
    /// Verified by regulatory authority
    Regulatory,
    /// Third-party verification service
    ThirdParty,
    /// Manual verification by admin
    Manual,
}

/// Customer management service
pub struct CustomerManagementService {
    /// All customers in the system
    customers: HashMap<String, Customer>,
    /// ANI ownership database
    ani_ownership: HashMap<String, AniOwnership>,
    /// Customer ID to name mapping for fast lookups
    customer_names: HashMap<String, String>,
}

impl CustomerManagementService {
    /// Create new customer management service
    pub fn new() -> Self {
        CustomerManagementService {
            customers: HashMap::new(),
            ani_ownership: HashMap::new(),
            customer_names: HashMap::new(),
        }
    }

    /// Add new customer
    pub fn add_customer(&mut self, customer: Customer) -> Result<()> {
        if self.customers.contains_key(&customer.customer_id) {
            return Err(anyhow!("Customer '{}' already exists", customer.customer_id));
        }

        self.customer_names.insert(customer.customer_id.clone(), customer.name.clone());
        self.customers.insert(customer.customer_id.clone(), customer);
        
        info!("Added customer: {} ({})", self.customer_names.get(&customer.customer_id).unwrap(), customer.customer_id);
        Ok(())
    }

    /// Get customer by ID
    pub fn get_customer(&self, customer_id: &str) -> Option<&Customer> {
        self.customers.get(customer_id)
    }

    /// Update customer
    pub fn update_customer(&mut self, customer: Customer) -> Result<()> {
        if !self.customers.contains_key(&customer.customer_id) {
            return Err(anyhow!("Customer '{}' not found", customer.customer_id));
        }

        self.customer_names.insert(customer.customer_id.clone(), customer.name.clone());
        self.customers.insert(customer.customer_id.clone(), customer);
        Ok(())
    }

    /// Add ANI ownership record
    pub fn add_ani_ownership(&mut self, ani_record: AniOwnership) -> Result<()> {
        // Verify customer exists
        if !self.customers.contains_key(&ani_record.customer_id) {
            return Err(anyhow!("Customer '{}' not found", ani_record.customer_id));
        }

        self.ani_ownership.insert(ani_record.ani.clone(), ani_record);
        Ok(())
    }

    /// Get ANI ownership
    pub fn get_ani_ownership(&self, ani: &str) -> Option<&AniOwnership> {
        self.ani_ownership.get(ani)
    }

    /// Get attestation level for ANI
    pub fn get_ani_attestation(&self, ani: &str, customer_id: &str) -> Result<AttestationLevel> {
        // First check if ANI is in ownership database
        if let Some(ownership) = self.ani_ownership.get(ani) {
            if ownership.customer_id == customer_id && ownership.verified {
                return Ok(ownership.attestation_level.clone());
            }
        }

        // Check if customer has specific settings
        if let Some(customer) = self.customers.get(customer_id) {
            // If auto_attest_did_ani is enabled, check if ANI is in customer's DID list
            if customer.stir_shaken_settings.auto_attest_did_ani {
                if customer.owned_dids.iter().any(|did| self.ani_matches_pattern(ani, did)) {
                    return Ok(AttestationLevel::Full); // Automatic A attestation for owned DIDs
                }
            }

            // Return customer's default attestation
            return Ok(customer.stir_shaken_settings.default_attestation.clone());
        }

        // Default to gateway attestation
        Ok(AttestationLevel::Gateway)
    }

    /// Check if ANI matches a pattern (supports wildcards and ranges)
    fn ani_matches_pattern(&self, ani: &str, pattern: &str) -> bool {
        // Simple pattern matching
        if pattern.contains('*') {
            let pattern_regex = pattern.replace('*', ".*");
            if let Ok(regex) = regex::Regex::new(&format!("^{}$", pattern_regex)) {
                return regex.is_match(ani);
            }
        }

        // Range matching (e.g., "5551000000-5551999999")
        if pattern.contains('-') {
            let parts: Vec<&str> = pattern.split('-').collect();
            if parts.len() == 2 {
                if let (Ok(start), Ok(end), Ok(ani_num)) = (
                    parts[0].parse::<u64>(),
                    parts[1].parse::<u64>(),
                    ani.parse::<u64>()
                ) {
                    return ani_num >= start && ani_num <= end;
                }
            }
        }

        // Exact match
        ani == pattern
    }

    /// Get customer's trunks
    pub fn get_customer_trunks(&self, customer_id: &str) -> Option<(Vec<String>, Vec<String>)> {
        self.customers.get(customer_id)
            .map(|customer| (customer.termination_trunks.clone(), customer.origination_trunks.clone()))
    }

    /// Map termination call to origination ANI
    pub fn map_termination_to_origination(&self, 
        termination_ani: &str, 
        termination_customer_id: &str,
        origination_customer_id: &str
    ) -> Result<AttestationLevel> {
        debug!("Mapping termination ANI {} from customer {} to origination customer {}", 
               termination_ani, termination_customer_id, origination_customer_id);

        // Get termination customer settings
        let termination_customer = self.customers.get(termination_customer_id)
            .ok_or_else(|| anyhow!("Termination customer '{}' not found", termination_customer_id))?;

        // Get origination customer settings
        let origination_customer = self.customers.get(origination_customer_id)
            .ok_or_else(|| anyhow!("Origination customer '{}' not found", origination_customer_id))?;

        // If origination customer allows inheriting attestation from termination
        if origination_customer.stir_shaken_settings.inherit_termination_attestation {
            // Get the attestation level from termination
            let termination_attestation = self.get_ani_attestation(termination_ani, termination_customer_id)?;
            
            info!("Inherited attestation {} from termination customer {} to origination customer {}", 
                  attestation_to_string(&termination_attestation), 
                  termination_customer_id, 
                  origination_customer_id);
            
            return Ok(termination_attestation);
        }

        // Otherwise use origination customer's default attestation for the ANI
        self.get_ani_attestation(termination_ani, origination_customer_id)
    }

    /// List all customers
    pub fn list_customers(&self) -> Vec<&Customer> {
        self.customers.values().collect()
    }

    /// Get customer statistics
    pub fn get_customer_stats(&self, customer_id: &str) -> Option<CustomerStats> {
        self.customers.get(customer_id).map(|customer| {
            let ani_count = self.ani_ownership.values()
                .filter(|ownership| ownership.customer_id == customer_id)
                .count();

            CustomerStats {
                customer_id: customer_id.to_string(),
                name: customer.name.clone(),
                active_calls: 0, // This would be populated from call state
                total_anis: ani_count,
                termination_trunks: customer.termination_trunks.len(),
                origination_trunks: customer.origination_trunks.len(),
                current_balance: customer.billing_info.current_balance,
                last_call_time: None, // This would come from CDR data
            }
        })
    }
}

/// Customer statistics
#[derive(Debug, Clone)]
pub struct CustomerStats {
    pub customer_id: String,
    pub name: String,
    pub active_calls: u32,
    pub total_anis: usize,
    pub termination_trunks: usize,
    pub origination_trunks: usize,
    pub current_balance: i64,
    pub last_call_time: Option<DateTime<Utc>>,
}

impl Default for CustomerManagementService {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for BillingInfo {
    fn default() -> Self {
        BillingInfo {
            contact_name: "".to_string(),
            contact_email: "".to_string(),
            billing_cycle: BillingCycle::Monthly,
            credit_limit: 0,
            current_balance: 0,
            auto_payment: false,
        }
    }
}

impl Default for StirShakenCustomerSettings {
    fn default() -> Self {
        StirShakenCustomerSettings {
            enabled: true,
            default_attestation: AttestationLevel::Gateway,
            auto_attest_did_ani: true,
            use_ani_database: true,
            inherit_termination_attestation: false,
            preferred_cert_id: None,
        }
    }
}

impl Default for CustomerRoutingSettings {
    fn default() -> Self {
        CustomerRoutingSettings {
            priority: 5,
            enable_lcr: true,
            enable_quality_routing: false,
            max_concurrent_calls: 100,
            rate_limit_cps: None,
            allowed_destinations: vec![],
            blocked_destinations: vec![],
        }
    }
}

/// Helper function to convert attestation level to string
fn attestation_to_string(level: &AttestationLevel) -> &'static str {
    match level {
        AttestationLevel::Full => "A",
        AttestationLevel::Partial => "B",
        AttestationLevel::Gateway => "C",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_customer_management() {
        let mut service = CustomerManagementService::new();

        // Create a test customer
        let customer = Customer {
            customer_id: "test-customer-001".to_string(),
            name: "Test Customer".to_string(),
            customer_type: CustomerType::Enterprise,
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            billing_info: BillingInfo::default(),
            stir_shaken_settings: StirShakenCustomerSettings::default(),
            termination_trunks: vec!["trunk-term-001".to_string()],
            origination_trunks: vec!["trunk-orig-001".to_string()],
            owned_anis: vec!["555123*".to_string()],
            owned_dids: vec!["555123*".to_string()],
            routing_settings: CustomerRoutingSettings::default(),
        };

        assert!(service.add_customer(customer.clone()).is_ok());
        assert!(service.get_customer("test-customer-001").is_some());

        // Test ANI ownership
        let ani_ownership = AniOwnership {
            ani: "5551234567".to_string(),
            customer_id: "test-customer-001".to_string(),
            attestation_level: AttestationLevel::Full,
            verified: true,
            verification_source: VerificationSource::Internal,
            updated_at: Utc::now(),
            termination_trunk_id: Some("trunk-term-001".to_string()),
            origination_trunk_id: Some("trunk-orig-001".to_string()),
        };

        assert!(service.add_ani_ownership(ani_ownership).is_ok());
        
        let attestation = service.get_ani_attestation("5551234567", "test-customer-001").unwrap();
        assert_eq!(attestation, AttestationLevel::Full);
    }

    #[test]
    fn test_ani_pattern_matching() {
        let service = CustomerManagementService::new();
        
        assert!(service.ani_matches_pattern("5551234567", "555123*"));
        assert!(service.ani_matches_pattern("5551234567", "5551234567"));
        assert!(!service.ani_matches_pattern("5551234567", "555124*"));
    }
}