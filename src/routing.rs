//! Routing module for call routing and destination resolution

use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod engine {
    pub use super::*;
}

pub mod core {
    use super::*;

    /// Result of a routing operation
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum RouteResult {
        /// Route found successfully
        Found {
            destination: RouteDestination,
            rule_id: String,
            priority: RoutePriority,
        },
        /// No matching route found
        NotFound { reason: String },
        /// Default route used as fallback
        DefaultRoute { destination: RouteDestination },
    }

    /// Enhanced routing engine with rule processing capabilities
    pub struct RoutingEngine {
        config: RoutingConfig,
        rules: Vec<RoutingRule>,
        compiled_patterns: HashMap<String, Regex>,
    }

    impl RoutingEngine {
        /// Create a new routing engine with default configuration
        pub fn new() -> Self {
            let mut engine = Self {
                config: RoutingConfig::default(),
                rules: Vec::new(),
                compiled_patterns: HashMap::new(),
            };

            // Load default routing rules
            engine.load_default_rules();
            engine
        }

        /// Create routing engine with custom configuration
        pub fn with_config(config: RoutingConfig) -> Self {
            let mut engine = Self {
                config,
                rules: Vec::new(),
                compiled_patterns: HashMap::new(),
            };

            engine.load_default_rules();
            engine
        }

        /// Add a routing rule to the engine
        pub fn add_rule(&mut self, rule: RoutingRule) -> Result<()> {
            // Compile regex pattern if it's a regex pattern
            if rule.pattern.pattern.starts_with("regex:") {
                let regex_pattern = &rule.pattern.pattern[6..]; // Remove "regex:" prefix
                let compiled = Regex::new(regex_pattern)
                    .map_err(|e| anyhow!("Invalid regex pattern {}: {}", regex_pattern, e))?;
                self.compiled_patterns.insert(rule.id.clone(), compiled);
            }

            self.rules.push(rule);

            // Sort rules by priority (High -> Medium -> Low)
            self.rules.sort_by(|a, b| {
                use RoutePriority::*;
                match (a.priority, b.priority) {
                    (High, High) | (Medium, Medium) | (Low, Low) => std::cmp::Ordering::Equal,
                    (High, _) => std::cmp::Ordering::Less,
                    (_, High) => std::cmp::Ordering::Greater,
                    (Medium, Low) => std::cmp::Ordering::Less,
                    (Low, Medium) => std::cmp::Ordering::Greater,
                }
            });

            Ok(())
        }

        /// Resolve a routing request to find the best destination
        pub fn resolve_route(&self, request: &RoutingRequest) -> Result<RouteResult> {
            if !self.config.enabled {
                return Ok(RouteResult::NotFound {
                    reason: "Routing engine is disabled".to_string(),
                });
            }

            // Try to find matching rules in priority order
            for rule in &self.rules {
                if self.matches_pattern(&rule.pattern, &request.to)? {
                    return Ok(RouteResult::Found {
                        destination: rule.destination.clone(),
                        rule_id: rule.id.clone(),
                        priority: rule.priority,
                    });
                }
            }

            // If no rules match, try default route
            if !self.config.default_route.is_empty() {
                return Ok(RouteResult::DefaultRoute {
                    destination: RouteDestination {
                        uri: self.config.default_route.clone(),
                    },
                });
            }

            Ok(RouteResult::NotFound {
                reason: "No matching routes found and no default route configured".to_string(),
            })
        }

        /// Check if a number matches a routing pattern
        fn matches_pattern(&self, pattern: &RoutePattern, number: &str) -> Result<bool> {
            let pattern_str = &pattern.pattern;

            // Handle different pattern types
            if pattern_str.starts_with("regex:") {
                // Regex pattern matching - need to use the rule ID as key, not the pattern
                let rule_id = self
                    .rules
                    .iter()
                    .find(|rule| rule.pattern.pattern == *pattern_str)
                    .map(|rule| rule.id.as_str())
                    .unwrap_or("unknown");

                if let Some(regex) = self.compiled_patterns.get(rule_id) {
                    Ok(regex.is_match(number))
                } else {
                    Err(anyhow!(
                        "Compiled regex not found for pattern: {}",
                        pattern_str
                    ))
                }
            } else if pattern_str.ends_with('*') {
                // Prefix matching (e.g., "1800*" matches "18001234567")
                let prefix = &pattern_str[..pattern_str.len() - 1];
                Ok(number.starts_with(prefix))
            } else if pattern_str.contains('*') {
                // Wildcard matching
                let regex_pattern = pattern_str.replace('*', ".*");
                let regex = Regex::new(&format!("^{}$", regex_pattern))?;
                Ok(regex.is_match(number))
            } else {
                // Exact matching
                Ok(pattern_str == number)
            }
        }

        /// Load default routing rules for demonstration
        fn load_default_rules(&mut self) {
            let default_rules = vec![
                // Emergency numbers - highest priority
                RoutingRule {
                    id: "emergency_911".to_string(),
                    pattern: RoutePattern {
                        pattern: "911".to_string(),
                    },
                    destination: RouteDestination {
                        uri: "sip:emergency@psap.local:5060".to_string(),
                    },
                    priority: RoutePriority::High,
                },
                // Toll-free numbers
                RoutingRule {
                    id: "tollfree_800".to_string(),
                    pattern: RoutePattern {
                        pattern: "1800*".to_string(),
                    },
                    destination: RouteDestination {
                        uri: "sip:tollfree@carrier.com:5060".to_string(),
                    },
                    priority: RoutePriority::Medium,
                },
                RoutingRule {
                    id: "tollfree_888".to_string(),
                    pattern: RoutePattern {
                        pattern: "1888*".to_string(),
                    },
                    destination: RouteDestination {
                        uri: "sip:tollfree@carrier.com:5060".to_string(),
                    },
                    priority: RoutePriority::Medium,
                },
                // Local numbers (10-digit NANP)
                RoutingRule {
                    id: "local_nanp".to_string(),
                    pattern: RoutePattern {
                        pattern: "regex:^[2-9][0-8][0-9][2-9][0-9][0-9][0-9][0-9][0-9][0-9]$"
                            .to_string(),
                    },
                    destination: RouteDestination {
                        uri: "sip:local@pstn.carrier.com:5060".to_string(),
                    },
                    priority: RoutePriority::Medium,
                },
                // International numbers
                RoutingRule {
                    id: "international".to_string(),
                    pattern: RoutePattern {
                        pattern: "011*".to_string(),
                    },
                    destination: RouteDestination {
                        uri: "sip:international@global.carrier.com:5060".to_string(),
                    },
                    priority: RoutePriority::Low,
                },
            ];

            for rule in default_rules {
                if let Err(e) = self.add_rule(rule) {
                    eprintln!("Warning: Failed to load default rule: {}", e);
                }
            }
        }

        /// Get all configured routing rules
        pub fn get_rules(&self) -> &[RoutingRule] {
            &self.rules
        }

        /// Get routing engine configuration
        pub fn get_config(&self) -> &RoutingConfig {
            &self.config
        }

        /// Update routing configuration
        pub fn update_config(&mut self, config: RoutingConfig) {
            self.config = config;
        }

        /// Clear all routing rules
        pub fn clear_rules(&mut self) {
            self.rules.clear();
            self.compiled_patterns.clear();
        }

        /// Get statistics about the routing engine
        pub fn get_stats(&self) -> HashMap<String, u64> {
            let mut stats = HashMap::new();
            stats.insert("total_rules".to_string(), self.rules.len() as u64);
            stats.insert(
                "high_priority_rules".to_string(),
                self.rules
                    .iter()
                    .filter(|r| matches!(r.priority, RoutePriority::High))
                    .count() as u64,
            );
            stats.insert(
                "medium_priority_rules".to_string(),
                self.rules
                    .iter()
                    .filter(|r| matches!(r.priority, RoutePriority::Medium))
                    .count() as u64,
            );
            stats.insert(
                "low_priority_rules".to_string(),
                self.rules
                    .iter()
                    .filter(|r| matches!(r.priority, RoutePriority::Low))
                    .count() as u64,
            );
            stats.insert(
                "regex_patterns".to_string(),
                self.compiled_patterns.len() as u64,
            );
            stats
        }
    }
}

pub mod enhanced {
    pub use super::*;
}

pub mod emergency {
    pub use super::*;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub enabled: bool,
    pub default_route: String,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_route: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    pub id: String,
    pub pattern: RoutePattern,
    pub destination: RouteDestination,
    pub priority: RoutePriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutePattern {
    pub pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDestination {
    pub uri: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RoutePriority {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRequest {
    pub from: String,
    pub to: String,
}
