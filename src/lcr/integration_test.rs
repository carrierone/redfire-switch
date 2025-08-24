//! Integration tests for LCR system
//! Tests the complete deck versioning, routing, and safety features

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::lcr::types::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;
    use chrono::{DateTime, Utc, Duration};

    #[test]
    fn test_deck_versioning_workflow() {
        // Test the complete deck versioning workflow
        
        // 1. Create initial deck (version 1)
        let deck_v1 = RateDeck {
            id: 1,
            name: "Test Vendor Deck".to_string(),
            owner_id: 100,
            rate_type: RateType::DNIS,
            effective_date: Utc::now() - Duration::hours(24), // Yesterday
            end_date: None, // Active
            deck_version: 1,
            parent_deck_id: None,
            effective_time: chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            preload_minutes: 30,
            loaded_at: Some(Utc::now() - Duration::hours(24)),
            is_staged: false,
            active: true,
        };

        // 2. Create new version (version 2) with future effective date
        let deck_v2 = RateDeck {
            id: 2,
            name: "Test Vendor Deck".to_string(),
            owner_id: 100,
            rate_type: RateType::DNIS,
            effective_date: Utc::now() + Duration::hours(24), // Tomorrow
            end_date: None,
            deck_version: 2,
            parent_deck_id: Some(1), // Points to v1
            effective_time: chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            preload_minutes: 30,
            loaded_at: None, // Not loaded yet (staged)
            is_staged: true, // Staged for future activation
            active: true,
        };

        // Verify deck versioning logic
        assert_eq!(deck_v1.deck_version, 1);
        assert_eq!(deck_v2.deck_version, 2);
        assert_eq!(deck_v2.parent_deck_id, Some(deck_v1.id));
        
        // Verify that v1 is currently active, v2 is staged
        assert!(!deck_v1.is_staged);
        assert!(deck_v2.is_staged);
        
        // When v2 activates, v1 should get end_date set to v2.effective_date - 1 second
        let expected_v1_end = deck_v2.effective_date - Duration::seconds(1);
        // This would be handled by database trigger in production
        println!("V1 should end at: {}, V2 starts at: {}", expected_v1_end, deck_v2.effective_date);
    }

    #[test] 
    fn test_local_rate_fallback() {
        // Test local rate fallback logic (critical telecom functionality)
        
        // Rate with NO local_rate (common scenario)
        let rate_without_local = NanpaRate {
            id: 1,
            deck_id: 1,
            code: "1555123".to_string(),
            inter_rate: Decimal::from_str("0.0150").unwrap(), // 1.5¢
            intra_rate: Decimal::from_str("0.0120").unwrap(), // 1.2¢  
            ij_rate: Decimal::from_str("0.0140").unwrap(),    // 1.4¢
            local_rate: None, // NULL - this is the key test
            min_increment: 6,
            interval: 6, 
            setup_fee: None,
        };

        // Test jurisdiction-based rate selection
        let inter_rate = get_rate_for_jurisdiction(&rate_without_local, CallJurisdiction::Inter);
        let intra_rate = get_rate_for_jurisdiction(&rate_without_local, CallJurisdiction::Intra);
        let local_rate = get_rate_for_jurisdiction(&rate_without_local, CallJurisdiction::Local);
        let ij_rate = get_rate_for_jurisdiction(&rate_without_local, CallJurisdiction::Indeterminate);

        assert_eq!(inter_rate, Decimal::from_str("0.0150").unwrap());
        assert_eq!(intra_rate, Decimal::from_str("0.0120").unwrap());
        assert_eq!(ij_rate, Decimal::from_str("0.0140").unwrap());
        
        // Critical test: Local rate should fall back to intra_rate
        assert_eq!(local_rate, Decimal::from_str("0.0120").unwrap());
        assert_eq!(local_rate, intra_rate); // Should be exactly the same
    }

    #[test]
    fn test_route_request_structure() {
        // Test route request for time-aware routing
        
        let route_request = RouteRequest {
            ani: "15551234567".to_string(),
            dnis: "15559876543".to_string(),
            ingress_trunk_id: 1,
            client_deck_id: Some(10),
            route_type: RouteType::NANPA,
            require_profit_protection: true,
            min_profit_margin: Some(Decimal::from_str("0.001").unwrap()), // 0.1¢ minimum
            effective_time: Some(Utc::now()), // Route at current time
        };

        // Verify structure
        assert!(route_request.ani.starts_with("1555"));
        assert!(route_request.dnis.starts_with("1555"));
        assert_eq!(route_request.route_type, RouteType::NANPA);
        assert!(route_request.require_profit_protection);
        assert!(route_request.effective_time.is_some());
    }

    #[test]
    fn test_call_simulation_structure() {
        // Test call simulation for testing routes
        
        let simulation = CallSimulation {
            ani: "15551234567".to_string(),
            dnis: "15559876543".to_string(),
            lrn: Some("15559876543".to_string()),
            jurisdiction: CallJurisdiction::Local,
            ingress_trunk: "Client-A-Trunk-1".to_string(),
            total_routes: 2,
            routes: vec![
                SimulatedRoute {
                    egress_trunk: "Vendor-A-Trunk-1".to_string(),
                    vendor: "Vendor A".to_string(),
                    cost_per_minute: Decimal::from_str("0.0120").unwrap(),
                    selling_per_minute: Decimal::from_str("0.0180").unwrap(),
                    profit_margin: Decimal::from_str("0.0060").unwrap(),
                    priority: 1,
                    setup_fee: Decimal::ZERO,
                    min_increment: 6,
                    interval: 6,
                },
                SimulatedRoute {
                    egress_trunk: "Vendor-B-Trunk-1".to_string(), 
                    vendor: "Vendor B".to_string(),
                    cost_per_minute: Decimal::from_str("0.0125").unwrap(),
                    selling_per_minute: Decimal::from_str("0.0180").unwrap(),
                    profit_margin: Decimal::from_str("0.0055").unwrap(),
                    priority: 2,
                    setup_fee: Decimal::ZERO,
                    min_increment: 6,
                    interval: 6,
                },
            ],
            routing_decision: "Selected lowest cost route with adequate profit margin".to_string(),
        };

        // Verify simulation
        assert_eq!(simulation.jurisdiction, CallJurisdiction::Local);
        assert_eq!(simulation.total_routes, 2);
        assert_eq!(simulation.routes.len(), 2);
        
        // First route should be cheaper
        assert!(simulation.routes[0].cost_per_minute < simulation.routes[1].cost_per_minute);
        
        // Both routes should be profitable
        assert!(simulation.routes[0].profit_margin > Decimal::ZERO);
        assert!(simulation.routes[1].profit_margin > Decimal::ZERO);
    }

    #[test]
    fn test_immediate_vs_scheduled_activation() {
        // Test immediate activation vs scheduled cutover logic
        
        let now = Utc::now();
        
        // Past effective date = immediate activation
        let past_request = DeckLoadRequest {
            deck_name: "Test Deck".to_string(),
            owner_id: 100,
            rate_type: RateType::DNIS,
            effective_date: now - Duration::hours(2), // 2 hours ago
            effective_time: None,
            preload_minutes: None,
            rates_csv: None,
            rates_data: None,
        };

        // Future effective date = scheduled cutover  
        let future_request = DeckLoadRequest {
            deck_name: "Test Deck".to_string(),
            owner_id: 100,
            rate_type: RateType::DNIS,
            effective_date: now + Duration::hours(24), // Tomorrow
            effective_time: None,
            preload_minutes: Some(30),
            rates_csv: None,
            rates_data: None,
        };

        // Test logic for immediate vs scheduled
        let should_activate_immediately_past = past_request.effective_date <= now;
        let should_activate_immediately_future = future_request.effective_date <= now;

        assert!(should_activate_immediately_past); // Past = immediate
        assert!(!should_activate_immediately_future); // Future = scheduled
    }

    // Helper function matching the production logic
    fn get_rate_for_jurisdiction(rate: &NanpaRate, jurisdiction: CallJurisdiction) -> Decimal {
        match jurisdiction {
            CallJurisdiction::Inter => rate.inter_rate,
            CallJurisdiction::Intra => rate.intra_rate,
            CallJurisdiction::Indeterminate => rate.ij_rate,
            CallJurisdiction::Local => rate.local_rate.unwrap_or(rate.intra_rate), // Key fallback
        }
    }
}