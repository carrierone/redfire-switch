/*
 * Redfire Switch - Number Manipulation and Routing
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
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Number manipulation action type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ManipulationAction {
    /// Add prefix to number
    AddPrefix(String),
    /// Remove prefix from number
    RemovePrefix(String),
    /// Add suffix to number
    AddSuffix(String),
    /// Remove suffix from number
    RemoveSuffix(String),
    /// Replace number using regex
    RegexReplace { pattern: String, replacement: String },
    /// Set number to fixed value
    SetFixed(String),
    /// Strip leading digits (count)
    StripLeading(usize),
    /// Strip trailing digits (count)
    StripTrailing(usize),
    /// Normalize to E.164 format
    NormalizeE164,
}

/// Number manipulation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumberManipulationRule {
    /// Rule identifier
    pub rule_id: String,
    /// Rule name/description
    pub name: String,
    /// Enable/disable this rule
    pub enabled: bool,
    /// Priority (lower numbers processed first)
    pub priority: u32,
    /// Match conditions (all must be true)
    pub conditions: Vec<ManipulationCondition>,
    /// Actions to apply if conditions match
    pub actions: Vec<ManipulationAction>,
    /// Apply to caller ID (from number)
    pub apply_to_caller_id: bool,
    /// Apply to dialed number (to number)
    pub apply_to_dialed_number: bool,
}

/// Condition for number manipulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManipulationCondition {
    /// Field to check
    pub field: ManipulationField,
    /// Match operator
    pub operator: MatchOperator,
    /// Value to match against
    pub value: String,
}

/// Field to apply manipulation to
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ManipulationField {
    /// Originating number (caller ID)
    CallerNumber,
    /// Destination number (called number)
    DialedNumber,
    /// Trunk group ID
    TrunkGroup,
    /// Customer ID
    CustomerId,
    /// DID/TFN range
    DidRange,
    /// Call direction (inbound/outbound)
    Direction,
}

/// Match operator for conditions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MatchOperator {
    /// Exact match
    Equals,
    /// Starts with
    StartsWith,
    /// Ends with
    EndsWith,
    /// Contains substring
    Contains,
    /// Regex match
    Regex,
    /// Number length equals
    LengthEquals,
    /// Number length greater than
    LengthGreaterThan,
    /// Number length less than
    LengthLessThan,
    /// In list (comma-separated values)
    InList,
}

/// Call direction for routing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CallDirection {
    Inbound,
    Outbound,
}

/// DID/TFN range configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidRange {
    /// Range identifier
    pub range_id: String,
    /// Range name/description
    pub name: String,
    /// Starting number in range
    pub start_number: String,
    /// Ending number in range
    pub end_number: String,
    /// Associated trunk group
    pub trunk_group_id: String,
    /// Customer ID this range belongs to
    pub customer_id: Option<String>,
    /// Manipulation rules for this range
    pub manipulation_rules: Vec<String>, // Rule IDs
}

/// Termination trunk configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminationTrunk {
    /// Trunk identifier
    pub trunk_id: String,
    /// Trunk name/description
    pub name: String,
    /// Enable/disable trunk
    pub enabled: bool,
    /// Trunk priority (lower = higher priority)
    pub priority: u32,
    /// Weight for load balancing
    pub weight: u32,
    /// Maximum concurrent calls
    pub max_calls: u32,
    /// Current active calls
    pub active_calls: u32,
    /// Destination patterns this trunk handles
    pub destination_patterns: Vec<String>,
    /// Caller ID manipulation rules
    pub caller_id_rules: Vec<String>, // Rule IDs
    /// Dialed number manipulation rules
    pub dialed_number_rules: Vec<String>, // Rule IDs
    /// Trunk gateway information
    pub gateway: TrunkGateway,
}

/// Trunk gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkGateway {
    /// Gateway hostname or IP
    pub host: String,
    /// Gateway port
    pub port: u16,
    /// SIP transport protocol
    pub transport: String, // "UDP", "TCP", "TLS"
    /// Authentication username
    pub username: Option<String>,
    /// Authentication password
    pub password: Option<String>,
    /// Contact header override
    pub contact_override: Option<String>,
}

/// Number manipulation service
pub struct NumberManipulationService {
    /// All manipulation rules
    pub rules: HashMap<String, NumberManipulationRule>,
    /// DID/TFN ranges
    pub did_ranges: HashMap<String, DidRange>,
    /// Termination trunks
    pub termination_trunks: HashMap<String, TerminationTrunk>,
    /// Compiled regex patterns cache
    regex_cache: HashMap<String, Regex>,
}

impl NumberManipulationService {
    /// Create new number manipulation service
    pub fn new() -> Self {
        NumberManipulationService {
            rules: HashMap::new(),
            did_ranges: HashMap::new(),
            termination_trunks: HashMap::new(),
            regex_cache: HashMap::new(),
        }
    }

    /// Add manipulation rule
    pub fn add_rule(&mut self, rule: NumberManipulationRule) {
        self.rules.insert(rule.rule_id.clone(), rule);
    }

    /// Add DID range
    pub fn add_did_range(&mut self, range: DidRange) {
        self.did_ranges.insert(range.range_id.clone(), range);
    }

    /// Add termination trunk
    pub fn add_termination_trunk(&mut self, trunk: TerminationTrunk) {
        self.termination_trunks.insert(trunk.trunk_id.clone(), trunk);
    }

    /// Apply number manipulation to call
    pub async fn manipulate_numbers(
        &mut self, 
        caller_number: &str, 
        dialed_number: &str,
        trunk_group_id: &str,
        customer_id: Option<&str>,
        direction: CallDirection
    ) -> Result<(String, String)> {
        let mut manipulated_caller = caller_number.to_string();
        let mut manipulated_dialed = dialed_number.to_string();

        // Get applicable rules sorted by priority (clone to avoid borrow issues)
        let mut applicable_rules: Vec<NumberManipulationRule> = self.rules.values()
            .filter(|rule| rule.enabled)
            .cloned()
            .collect();
        applicable_rules.sort_by_key(|rule| rule.priority);

        // Apply rules in priority order
        for rule in applicable_rules {
            if self.rule_matches(&rule, &manipulated_caller, &manipulated_dialed, trunk_group_id, customer_id, &direction).await? {
                debug!("Applying manipulation rule: {}", rule.name);

                let rule_actions = rule.actions.clone();
                let rule_caller_id = rule.apply_to_caller_id;
                let rule_dialed_id = rule.apply_to_dialed_number;
                let rule_dialed_number = rule.apply_to_dialed_number;
                let rule_name = rule.name.clone();

                // Apply to caller number if configured
                if rule_caller_id {
                    for action in &rule_actions {
                        manipulated_caller = self.apply_action(action, &manipulated_caller).await?;
                    }
                }

                // Apply to dialed number if configured
                if rule_dialed_number {
                    for action in &rule_actions {
                        manipulated_dialed = self.apply_action(action, &manipulated_dialed).await?;
                    }
                }

                info!("Rule '{}' applied: {} -> {}, {} -> {}", 
                    rule_name, caller_number, manipulated_caller, dialed_number, manipulated_dialed);
            }
        }

        Ok((manipulated_caller, manipulated_dialed))
    }

    /// Check if rule conditions match
    async fn rule_matches(
        &mut self,
        rule: &NumberManipulationRule,
        caller_number: &str,
        dialed_number: &str,
        trunk_group_id: &str,
        customer_id: Option<&str>,
        direction: &CallDirection
    ) -> Result<bool> {
        // All conditions must match
        for condition in &rule.conditions {
            if !self.condition_matches(condition, caller_number, dialed_number, trunk_group_id, customer_id, direction).await? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Check if individual condition matches
    async fn condition_matches(
        &mut self,
        condition: &ManipulationCondition,
        caller_number: &str,
        dialed_number: &str,
        trunk_group_id: &str,
        customer_id: Option<&str>,
        direction: &CallDirection
    ) -> Result<bool> {
        let field_value = match condition.field {
            ManipulationField::CallerNumber => caller_number,
            ManipulationField::DialedNumber => dialed_number,
            ManipulationField::TrunkGroup => trunk_group_id,
            ManipulationField::CustomerId => customer_id.unwrap_or(""),
            ManipulationField::DidRange => {
                // Check if dialed number falls in any DID range
                let ranges: Vec<_> = self.did_ranges.values().cloned().collect();
                for range in ranges {
                    if self.number_in_range(dialed_number, &range.start_number, &range.end_number)? {
                        return self.match_operator(&condition.operator, &range.range_id, &condition.value);
                    }
                }
                ""
            },
            ManipulationField::Direction => match direction {
                CallDirection::Inbound => "inbound",
                CallDirection::Outbound => "outbound",
            },
        };

        self.match_operator(&condition.operator, field_value, &condition.value)
    }

    /// Apply match operator
    fn match_operator(&mut self, operator: &MatchOperator, field_value: &str, condition_value: &str) -> Result<bool> {
        match operator {
            MatchOperator::Equals => Ok(field_value == condition_value),
            MatchOperator::StartsWith => Ok(field_value.starts_with(condition_value)),
            MatchOperator::EndsWith => Ok(field_value.ends_with(condition_value)),
            MatchOperator::Contains => Ok(field_value.contains(condition_value)),
            MatchOperator::Regex => {
                let regex = if let Some(cached_regex) = self.regex_cache.get(condition_value) {
                    cached_regex
                } else {
                    let compiled_regex = Regex::new(condition_value)?;
                    self.regex_cache.insert(condition_value.to_string(), compiled_regex);
                    self.regex_cache.get(condition_value).unwrap()
                };
                Ok(regex.is_match(field_value))
            },
            MatchOperator::LengthEquals => {
                let length: usize = condition_value.parse()?;
                Ok(field_value.len() == length)
            },
            MatchOperator::LengthGreaterThan => {
                let length: usize = condition_value.parse()?;
                Ok(field_value.len() > length)
            },
            MatchOperator::LengthLessThan => {
                let length: usize = condition_value.parse()?;
                Ok(field_value.len() < length)
            },
            MatchOperator::InList => {
                let list_items: Vec<&str> = condition_value.split(',').map(|s| s.trim()).collect();
                Ok(list_items.contains(&field_value))
            },
        }
    }

    /// Apply manipulation action to number
    async fn apply_action(&mut self, action: &ManipulationAction, number: &str) -> Result<String> {
        match action {
            ManipulationAction::AddPrefix(prefix) => {
                Ok(format!("{}{}", prefix, number))
            },
            ManipulationAction::RemovePrefix(prefix) => {
                if number.starts_with(prefix) {
                    Ok(number[prefix.len()..].to_string())
                } else {
                    Ok(number.to_string())
                }
            },
            ManipulationAction::AddSuffix(suffix) => {
                Ok(format!("{}{}", number, suffix))
            },
            ManipulationAction::RemoveSuffix(suffix) => {
                if number.ends_with(suffix) {
                    Ok(number[..number.len() - suffix.len()].to_string())
                } else {
                    Ok(number.to_string())
                }
            },
            ManipulationAction::RegexReplace { pattern, replacement } => {
                let regex = if let Some(cached_regex) = self.regex_cache.get(pattern) {
                    cached_regex
                } else {
                    let compiled_regex = Regex::new(pattern)?;
                    self.regex_cache.insert(pattern.clone(), compiled_regex);
                    self.regex_cache.get(pattern).unwrap()
                };
                Ok(regex.replace_all(number, replacement).to_string())
            },
            ManipulationAction::SetFixed(fixed_number) => {
                Ok(fixed_number.clone())
            },
            ManipulationAction::StripLeading(count) => {
                if number.len() > *count {
                    Ok(number[*count..].to_string())
                } else {
                    Ok(String::new())
                }
            },
            ManipulationAction::StripTrailing(count) => {
                if number.len() > *count {
                    Ok(number[..number.len() - count].to_string())
                } else {
                    Ok(String::new())
                }
            },
            ManipulationAction::NormalizeE164 => {
                self.normalize_to_e164(number)
            },
        }
    }

    /// Check if number falls within range
    fn number_in_range(&self, number: &str, start: &str, end: &str) -> Result<bool> {
        // Remove non-digits for comparison
        let clean_number = number.chars().filter(|c| c.is_ascii_digit()).collect::<String>();
        let clean_start = start.chars().filter(|c| c.is_ascii_digit()).collect::<String>();
        let clean_end = end.chars().filter(|c| c.is_ascii_digit()).collect::<String>();

        // Convert to numbers for comparison (handle potential overflow)
        if let (Ok(num), Ok(start_num), Ok(end_num)) = (
            clean_number.parse::<u64>(),
            clean_start.parse::<u64>(),
            clean_end.parse::<u64>()
        ) {
            Ok(num >= start_num && num <= end_num)
        } else {
            // Fallback to string comparison for very long numbers
            Ok(clean_number >= clean_start && clean_number <= clean_end)
        }
    }

    /// Normalize number to E.164 format
    fn normalize_to_e164(&self, number: &str) -> Result<String> {
        let cleaned = number.chars()
            .filter(|c| c.is_ascii_digit() || *c == '+')
            .collect::<String>();

        if cleaned.starts_with('+') {
            return Ok(cleaned);
        }

        // Add country code if missing (default to US +1)
        if cleaned.len() == 10 {
            return Ok(format!("+1{}", cleaned));
        }

        if cleaned.len() == 11 && cleaned.starts_with('1') {
            return Ok(format!("+{}", cleaned));
        }

        // Return as-is if we can't determine format
        Ok(cleaned)
    }

    /// Find best termination trunk for destination
    pub async fn find_termination_trunk(&self, destination: &str) -> Result<Option<&TerminationTrunk>> {
        let mut matching_trunks: Vec<&TerminationTrunk> = self.termination_trunks.values()
            .filter(|trunk| {
                trunk.enabled && 
                trunk.active_calls < trunk.max_calls &&
                self.trunk_matches_destination(trunk, destination)
            })
            .collect();

        if matching_trunks.is_empty() {
            return Ok(None);
        }

        // Sort by priority (lower = higher priority), then by weight
        matching_trunks.sort_by(|a, b| {
            a.priority.cmp(&b.priority)
                .then_with(|| b.weight.cmp(&a.weight)) // Higher weight = better
        });

        Ok(matching_trunks.into_iter().next())
    }

    /// Check if trunk matches destination pattern
    fn trunk_matches_destination(&self, trunk: &TerminationTrunk, destination: &str) -> bool {
        if trunk.destination_patterns.is_empty() {
            return true; // No patterns = matches all
        }

        trunk.destination_patterns.iter().any(|pattern| {
            if let Ok(regex) = Regex::new(pattern) {
                regex.is_match(destination)
            } else {
                // Fallback to simple pattern matching
                destination.starts_with(pattern)
            }
        })
    }

    /// Get manipulation rules for trunk
    pub fn get_trunk_rules(&self, trunk_id: &str) -> Result<(Vec<&NumberManipulationRule>, Vec<&NumberManipulationRule>)> {
        let trunk = self.termination_trunks.get(trunk_id)
            .ok_or_else(|| anyhow!("Trunk '{}' not found", trunk_id))?;

        let caller_id_rules: Vec<&NumberManipulationRule> = trunk.caller_id_rules.iter()
            .filter_map(|rule_id| self.rules.get(rule_id))
            .collect();

        let dialed_number_rules: Vec<&NumberManipulationRule> = trunk.dialed_number_rules.iter()
            .filter_map(|rule_id| self.rules.get(rule_id))
            .collect();

        Ok((caller_id_rules, dialed_number_rules))
    }
}

impl Default for NumberManipulationService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_number_manipulation() {
        let mut service = NumberManipulationService::new();

        // Add a rule to add +1 prefix to numbers without it
        let rule = NumberManipulationRule {
            rule_id: "add-us-prefix".to_string(),
            name: "Add US +1 prefix".to_string(),
            enabled: true,
            priority: 10,
            conditions: vec![
                ManipulationCondition {
                    field: ManipulationField::DialedNumber,
                    operator: MatchOperator::LengthEquals,
                    value: "10".to_string(),
                },
            ],
            actions: vec![
                ManipulationAction::AddPrefix("+1".to_string()),
            ],
            apply_to_caller_id: false,
            apply_to_dialed_number: true,
        };

        service.add_rule(rule);

        let (caller, dialed) = service.manipulate_numbers(
            "5551234567",
            "5559876543", 
            "trunk1",
            Some("customer1"),
            CallDirection::Outbound
        ).await.unwrap();

        assert_eq!(caller, "5551234567");
        assert_eq!(dialed, "+15559876543");
    }

    #[test]
    fn test_number_in_range() {
        let service = NumberManipulationService::new();
        
        assert!(service.number_in_range("5551234567", "5551000000", "5551999999").unwrap());
        assert!(!service.number_in_range("5552234567", "5551000000", "5551999999").unwrap());
    }

    #[test]
    fn test_e164_normalization() {
        let service = NumberManipulationService::new();
        
        assert_eq!(service.normalize_to_e164("5551234567").unwrap(), "+15551234567");
        assert_eq!(service.normalize_to_e164("15551234567").unwrap(), "+15551234567");
        assert_eq!(service.normalize_to_e164("+15551234567").unwrap(), "+15551234567");
    }
}