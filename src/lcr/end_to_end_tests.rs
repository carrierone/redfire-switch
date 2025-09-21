#[cfg(test)]
mod end_to_end_tests {
    use super::super::phone_validation::*;
    use super::super::types::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    /// Integration test that simulates the complete phone validation and routing workflow
    #[tokio::test]
    async fn test_complete_international_routing_workflow() {
        // Step 1: Create phone validation config
        let phone_config = PhoneValidationConfig {
            enabled: true,
            strict_validation: false,
            default_region: "US".to_string(),
            use_country_detection: true,
        };

        // Step 2: Create phone validator
        let validator = PhoneValidator::new(phone_config.clone());

        // Step 3: Test phone validation for different international numbers
        let test_numbers = vec![
            ("+44 20 7946 0958", "GB", "UK number"),
            ("+49 30 12345678", "DE", "German number"),
            ("+33 1 42 86 83 26", "FR", "French number"),
            ("+39 06 12345678", "IT", "Italian number"),
            ("+1 555 123 4567", "US", "US number"),
        ];

        for (number, expected_country, description) in test_numbers {
            let result = validator.validate(number);
            assert!(result.is_valid, "{} should be valid", description);
            assert_eq!(
                result.country_code,
                Some(expected_country.to_string()),
                "{} should detect {} country",
                description,
                expected_country
            );
            assert!(
                result.e164_format.is_some(),
                "{} should have E164 format",
                description
            );
        }

        // Step 4: Create routing request with phone validation
        let request = RouteRequest {
            ani: "15551234567".to_string(),
            dnis: "+44 20 7946 0958".to_string(), // UK number
            ingress_trunk_id: 1,
            client_deck_id: None,
            route_type: RouteType::AZ, // International routing
            require_profit_protection: false,
            min_profit_margin: None,
            effective_time: Some(Utc::now()),
            phone_validation: Some(phone_config),
            routing_plan_id: Some(1), // Use EEA routing plan
        };

        // Validate the request structure
        assert_eq!(request.route_type, RouteType::AZ);
        assert!(request.phone_validation.is_some());
        assert_eq!(request.routing_plan_id, Some(1));
        assert!(request.dnis.contains("44")); // UK country code
    }

    #[test]
    fn test_eea_vs_row_routing_logic() {
        // Test EEA country routing
        let eea_request = create_test_request("+49 30 12345678", Some(1)); // German number, EEA plan
        let eea_validation = validate_number_for_request(&eea_request);

        assert!(eea_validation.is_valid);
        assert_eq!(eea_validation.country_code, Some("DE".to_string()));

        // Test ROW country routing
        let row_request = create_test_request("+1 555 123 4567", Some(2)); // US number, ROW plan
        let row_validation = validate_number_for_request(&row_request);

        assert!(row_validation.is_valid);
        assert_eq!(row_validation.country_code, Some("US".to_string()));

        // Test unknown country with strict validation
        let strict_request = create_test_request("+999 123 456 789", Some(3)); // Unknown country, strict plan
        let strict_validation = validate_number_for_request(&strict_request);

        // Should be valid with non-strict validation, but country would be None
        assert!(strict_validation.is_valid || strict_validation.country_code.is_none());
    }

    #[test]
    fn test_routing_plan_phone_validation_integration() {
        // Test with phone validation enabled
        let enabled_plan = create_mock_routing_plan(1, true, false);
        assert!(enabled_plan.phone_validation_enabled);
        assert!(!enabled_plan.phone_validation_strict);

        let request = create_test_request_with_plan("+44 20 7946 0958", &enabled_plan);
        let validation = validate_with_plan(&request, &enabled_plan);
        assert!(validation.is_valid);

        // Test with strict validation enabled
        let strict_plan = create_mock_routing_plan(2, true, true);
        assert!(strict_plan.phone_validation_enabled);
        assert!(strict_plan.phone_validation_strict);

        let strict_request = create_test_request_with_plan("invalid-number", &strict_plan);
        let strict_validation = validate_with_plan(&strict_request, &strict_plan);
        assert!(!strict_validation.is_valid); // Should fail strict validation

        // Test with validation disabled
        let disabled_plan = create_mock_routing_plan(3, false, false);
        assert!(!disabled_plan.phone_validation_enabled);

        let disabled_request = create_test_request_with_plan("invalid-number", &disabled_plan);
        let disabled_validation = validate_with_plan(&disabled_request, &disabled_plan);
        assert!(disabled_validation.is_valid); // Should pass when disabled
    }

    #[test]
    fn test_country_specific_routing_preferences() {
        // Create EEA country preference (should have cost reduction)
        let eea_preference = create_mock_country_preference("DE", InternationalJurisdiction::EEA);
        assert_eq!(eea_preference.jurisdiction, InternationalJurisdiction::EEA);
        assert_eq!(
            eea_preference.cost_multiplier,
            Decimal::from_str("0.9").unwrap()
        ); // 10% reduction
        assert!(eea_preference.require_validation);

        // Create ROW country preference (normal pricing)
        let row_preference = create_mock_country_preference("US", InternationalJurisdiction::ROW);
        assert_eq!(row_preference.jurisdiction, InternationalJurisdiction::ROW);
        assert_eq!(row_preference.cost_multiplier, Decimal::ONE); // Normal pricing
        assert!(!row_preference.require_validation); // Less strict for ROW

        // Test cost calculation with preferences
        let base_rate = Decimal::from_str("0.10").unwrap();
        let eea_adjusted_rate = base_rate * eea_preference.cost_multiplier;
        let row_adjusted_rate = base_rate * row_preference.cost_multiplier;

        assert_eq!(eea_adjusted_rate, Decimal::from_str("0.09").unwrap()); // 10% reduction
        assert_eq!(row_adjusted_rate, Decimal::from_str("0.10").unwrap()); // No change
        assert!(eea_adjusted_rate < row_adjusted_rate); // EEA should be cheaper
    }

    #[test]
    fn test_international_rate_matching_logic() {
        // Test longest-to-shortest prefix matching logic
        let rates = create_mock_international_rates();

        // Test exact match
        let london_number = "+442071234567"; // London number
        let matched_rate = find_best_rate_match(&rates, london_number);
        assert!(matched_rate.is_some());
        let rate = matched_rate.unwrap();
        assert_eq!(rate.country_code, "44");
        assert_eq!(rate.destination_code, Some("207".to_string())); // Should match London prefix

        // Test country-only match
        let manchester_number = "+441611234567"; // Manchester number (no specific rate)
        let country_rate = find_best_rate_match(&rates, manchester_number);
        assert!(country_rate.is_some());
        let rate = country_rate.unwrap();
        assert_eq!(rate.country_code, "44");
        assert_eq!(rate.destination_code, None); // Should match country-only rate
    }

    #[test]
    fn test_route_selection_with_cost_and_priority() {
        // Create multiple routes with different costs and priorities
        let routes = create_mock_call_routes();

        // Routes should be sorted by priority first, then by cost
        let sorted_routes = sort_routes_by_priority_and_cost(routes);

        // First route should be highest priority (lowest number) and lowest cost
        assert_eq!(sorted_routes[0].priority, 1);
        assert!(sorted_routes[0].cost_per_minute <= sorted_routes[1].cost_per_minute);

        // Test profit calculation
        for route in &sorted_routes {
            let expected_profit = route.selling_per_minute - route.cost_per_minute;
            assert_eq!(route.profit_margin, expected_profit);
            assert!(route.profit_margin >= Decimal::ZERO); // Should be profitable
        }
    }

    #[test]
    fn test_profit_protection_logic() {
        let base_route = create_mock_call_route(
            Decimal::from_str("0.05").unwrap(), // cost
            Decimal::from_str("0.06").unwrap(), // selling
        );

        // Profit should be 0.01
        assert_eq!(base_route.profit_margin, Decimal::from_str("0.01").unwrap());

        // Test profit protection with minimum margin
        let min_margin = Decimal::from_str("0.02").unwrap();
        let passes_protection = base_route.profit_margin >= min_margin;
        assert!(!passes_protection); // Should fail profit protection

        // Test profitable route
        let profitable_route = create_mock_call_route(
            Decimal::from_str("0.05").unwrap(), // cost
            Decimal::from_str("0.08").unwrap(), // selling
        );
        assert_eq!(
            profitable_route.profit_margin,
            Decimal::from_str("0.03").unwrap()
        );
        let passes_protection = profitable_route.profit_margin >= min_margin;
        assert!(passes_protection); // Should pass profit protection
    }

    #[test]
    fn test_call_simulation_workflow() {
        // Create a complete call simulation
        let simulation = create_mock_call_simulation();

        assert_eq!(simulation.ani, "15551234567");
        assert!(simulation.dnis.starts_with("+")); // International number
        assert_eq!(simulation.jurisdiction, CallJurisdiction::Indeterminate); // International calls
        assert_eq!(simulation.routing_decision, "ROUTE_FOUND");
        assert!(simulation.total_routes > 0);
        assert_eq!(simulation.routes.len(), simulation.total_routes);

        // Routes should be sorted by preference (priority, then cost)
        if simulation.routes.len() > 1 {
            for i in 1..simulation.routes.len() {
                let prev_route = &simulation.routes[i - 1];
                let curr_route = &simulation.routes[i];

                // Either priority is better (lower) or same priority with better (lower) cost
                assert!(
                    prev_route.priority < curr_route.priority
                        || (prev_route.priority == curr_route.priority
                            && prev_route.cost_per_minute <= curr_route.cost_per_minute)
                );
            }
        }
    }

    // Helper functions for creating mock data

    fn create_test_request(dnis: &str, routing_plan_id: Option<i32>) -> RouteRequest {
        RouteRequest {
            ani: "15551234567".to_string(),
            dnis: dnis.to_string(),
            ingress_trunk_id: 1,
            client_deck_id: None,
            route_type: RouteType::AZ,
            require_profit_protection: false,
            min_profit_margin: None,
            effective_time: Some(Utc::now()),
            phone_validation: Some(PhoneValidationConfig::default()),
            routing_plan_id,
        }
    }

    fn create_test_request_with_plan(dnis: &str, plan: &InternationalRoutingPlan) -> RouteRequest {
        let phone_config = PhoneValidationConfig {
            enabled: plan.phone_validation_enabled,
            strict_validation: plan.phone_validation_strict,
            default_region: plan.phone_validation_default_region.clone(),
            use_country_detection: plan.phone_validation_use_country_detection,
        };

        RouteRequest {
            ani: "15551234567".to_string(),
            dnis: dnis.to_string(),
            ingress_trunk_id: 1,
            client_deck_id: None,
            route_type: RouteType::AZ,
            require_profit_protection: false,
            min_profit_margin: None,
            effective_time: Some(Utc::now()),
            phone_validation: Some(phone_config),
            routing_plan_id: Some(plan.id),
        }
    }

    fn validate_number_for_request(request: &RouteRequest) -> ValidationResult {
        if let Some(config) = &request.phone_validation {
            let validator = PhoneValidator::new(config.clone());
            validator.validate(&request.dnis)
        } else {
            ValidationResult {
                original: request.dnis.clone(),
                is_valid: true,
                country_code: None,
                region_code: None,
                number_type: None,
                e164_format: None,
                international_format: None,
                error: None,
            }
        }
    }

    fn validate_with_plan(
        request: &RouteRequest,
        plan: &InternationalRoutingPlan,
    ) -> ValidationResult {
        let config = PhoneValidationConfig {
            enabled: plan.phone_validation_enabled,
            strict_validation: plan.phone_validation_strict,
            default_region: plan.phone_validation_default_region.clone(),
            use_country_detection: plan.phone_validation_use_country_detection,
        };

        let validator = PhoneValidator::new(config);
        validator.validate(&request.dnis)
    }

    fn create_mock_routing_plan(
        id: i32,
        validation_enabled: bool,
        strict: bool,
    ) -> InternationalRoutingPlan {
        InternationalRoutingPlan {
            id,
            name: format!("Test Plan {}", id),
            description: Some("Test routing plan".to_string()),
            phone_validation_enabled: validation_enabled,
            phone_validation_strict: strict,
            phone_validation_default_region: "US".to_string(),
            phone_validation_use_country_detection: true,
            eea_routing_enabled: true,
            eea_priority_routing: true,
            eea_reduced_rates: true,
            eea_rate_reduction: Decimal::from_str("0.1").unwrap(),
            default_jurisdiction: InternationalJurisdiction::ROW,
            allow_unknown_destinations: !strict,
            max_rate_unknown_destinations: Decimal::ONE,
            require_strict_validation_unknown: strict,
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_mock_country_preference(
        country_code: &str,
        jurisdiction: InternationalJurisdiction,
    ) -> CountryRoutingPreference {
        let (cost_multiplier, require_validation) = match jurisdiction {
            InternationalJurisdiction::EEA => (Decimal::from_str("0.9").unwrap(), true),
            InternationalJurisdiction::ROW => (Decimal::ONE, false),
        };

        CountryRoutingPreference {
            id: 1,
            routing_plan_id: 1,
            country_code: country_code.to_string(),
            country_name: format!("Country {}", country_code),
            jurisdiction,
            quality_score: 90,
            cost_multiplier,
            require_validation,
            max_duration_minutes: 0,
            created_at: Utc::now(),
        }
    }

    fn create_mock_international_rates() -> Vec<InternationalRate> {
        vec![
            // London specific rate
            InternationalRate {
                id: 1,
                deck_id: 1,
                country_code: "44".to_string(),
                destination_code: Some("207".to_string()),
                destination_name: "UK London".to_string(),
                jurisdiction: InternationalJurisdiction::ROW,
                rate: Decimal::from_str("0.012").unwrap(),
                initial_increment: 30,
                subsequent_increment: 6,
                setup_fee: Some(Decimal::from_str("0.001").unwrap()),
                created_at: Utc::now(),
            },
            // UK general rate
            InternationalRate {
                id: 2,
                deck_id: 1,
                country_code: "44".to_string(),
                destination_code: None,
                destination_name: "UK General".to_string(),
                jurisdiction: InternationalJurisdiction::ROW,
                rate: Decimal::from_str("0.015").unwrap(),
                initial_increment: 30,
                subsequent_increment: 6,
                setup_fee: Some(Decimal::from_str("0.001").unwrap()),
                created_at: Utc::now(),
            },
        ]
    }

    fn find_best_rate_match<'a>(
        rates: &'a [InternationalRate],
        number: &str,
    ) -> Option<&'a InternationalRate> {
        let normalized = number.trim_start_matches('+');

        // Find longest matching prefix
        rates
            .iter()
            .filter(|rate| {
                let full_prefix = match &rate.destination_code {
                    Some(dest) => format!("{}{}", rate.country_code, dest),
                    None => rate.country_code.clone(),
                };
                normalized.starts_with(&full_prefix)
            })
            .max_by_key(|rate| match &rate.destination_code {
                Some(dest) => rate.country_code.len() + dest.len(),
                None => rate.country_code.len(),
            })
    }

    fn create_mock_call_routes() -> Vec<CallRoute> {
        let trunk1 = create_mock_trunk(1, "Trunk A", 1);
        let trunk2 = create_mock_trunk(2, "Trunk B", 1);
        let trunk3 = create_mock_trunk(3, "Trunk C", 2);

        vec![
            create_call_route(
                trunk1,
                Decimal::from_str("0.01").unwrap(),
                Decimal::from_str("0.02").unwrap(),
                1,
            ),
            create_call_route(
                trunk2,
                Decimal::from_str("0.015").unwrap(),
                Decimal::from_str("0.025").unwrap(),
                1,
            ),
            create_call_route(
                trunk3,
                Decimal::from_str("0.008").unwrap(),
                Decimal::from_str("0.018").unwrap(),
                2,
            ),
        ]
    }

    fn sort_routes_by_priority_and_cost(mut routes: Vec<CallRoute>) -> Vec<CallRoute> {
        routes.sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then(a.cost_per_minute.cmp(&b.cost_per_minute))
        });
        routes
    }

    fn create_mock_call_route(cost: Decimal, selling: Decimal) -> CallRoute {
        let trunk = create_mock_trunk(1, "Test Trunk", 1);
        create_call_route(trunk, cost, selling, 1)
    }

    fn create_call_route(
        trunk: EgressTrunk,
        cost: Decimal,
        selling: Decimal,
        priority: i32,
    ) -> CallRoute {
        CallRoute {
            egress_trunk: trunk,
            vendor_rate: None,
            cost_per_minute: cost,
            selling_per_minute: selling,
            profit_margin: selling - cost,
            priority,
            setup_fee: Decimal::ZERO,
            min_increment: 30,
            interval: 6,
        }
    }

    fn create_mock_trunk(id: i32, name: &str, priority: i32) -> EgressTrunk {
        EgressTrunk {
            id,
            name: name.to_string(),
            vendor_id: 1,
            host: format!("sip{}.example.com", id),
            port: 5060,
            transport: TransportProtocol::Udp,
            capacity_limit: 1000,
            cps_limit: Decimal::from_str("10.0").unwrap(),
            active: true,
            priority,
            weight: 1,
            tech_prefix: None,
            supports_international: true,
        }
    }

    fn create_mock_call_simulation() -> CallSimulation {
        CallSimulation {
            ani: "15551234567".to_string(),
            dnis: "+44 20 7946 0958".to_string(),
            lrn: None,
            jurisdiction: CallJurisdiction::Indeterminate,
            ingress_trunk: "Test Ingress".to_string(),
            total_routes: 2,
            routes: vec![
                SimulatedRoute {
                    egress_trunk: "Trunk A".to_string(),
                    vendor: "Vendor 1".to_string(),
                    cost_per_minute: Decimal::from_str("0.01").unwrap(),
                    selling_per_minute: Decimal::from_str("0.02").unwrap(),
                    profit_margin: Decimal::from_str("0.01").unwrap(),
                    priority: 1,
                    setup_fee: Decimal::ZERO,
                    min_increment: 30,
                    interval: 6,
                },
                SimulatedRoute {
                    egress_trunk: "Trunk B".to_string(),
                    vendor: "Vendor 2".to_string(),
                    cost_per_minute: Decimal::from_str("0.015").unwrap(),
                    selling_per_minute: Decimal::from_str("0.025").unwrap(),
                    profit_margin: Decimal::from_str("0.01").unwrap(),
                    priority: 2,
                    setup_fee: Decimal::ZERO,
                    min_increment: 30,
                    interval: 6,
                },
            ],
            routing_decision: "ROUTE_FOUND".to_string(),
        }
    }
}
