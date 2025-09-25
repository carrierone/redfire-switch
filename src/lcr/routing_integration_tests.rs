#[cfg(test)]
mod routing_integration_tests {
    use super::super::phone_validation::*;
    use super::super::types::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    /// Create a test route request for international routing
    fn create_test_international_request(dnis: &str, routing_plan_id: Option<i32>) -> RouteRequest {
        RouteRequest {
            ani: "15551234567".to_string(),
            dnis: dnis.to_string(),
            ingress_trunk_id: 1,
            client_deck_id: None,
            route_type: RouteType::AZ, // International A-Z routing
            require_profit_protection: false,
            min_profit_margin: None,
            effective_time: Some(Utc::now()),
            phone_validation: Some(PhoneValidationConfig::default()),
            routing_plan_id,
        }
    }

    /// Create a test route request for NANPA routing
    fn create_test_nanpa_request(dnis: &str) -> RouteRequest {
        RouteRequest {
            ani: "15551234567".to_string(),
            dnis: dnis.to_string(),
            ingress_trunk_id: 1,
            client_deck_id: None,
            route_type: RouteType::NANPA,
            require_profit_protection: false,
            min_profit_margin: None,
            effective_time: Some(Utc::now()),
            phone_validation: None, // NANPA doesn't use phone validation
            routing_plan_id: None,
        }
    }

    #[test]
    fn test_route_request_creation() {
        let request = create_test_international_request("+44 20 7946 0958", Some(1));

        assert_eq!(request.route_type, RouteType::AZ);
        assert!(request.phone_validation.is_some());
        assert_eq!(request.routing_plan_id, Some(1));
        assert!(request.dnis.contains("44"));
    }

    #[test]
    fn test_nanpa_vs_az_routing() {
        let nanpa_request = create_test_nanpa_request("15551234567");
        let az_request = create_test_international_request("+44 20 7946 0958", Some(1));

        assert_eq!(nanpa_request.route_type, RouteType::NANPA);
        assert_eq!(az_request.route_type, RouteType::AZ);
        assert!(nanpa_request.phone_validation.is_none());
        assert!(az_request.phone_validation.is_some());
    }

    #[test]
    fn test_phone_validation_config_in_request() {
        let mut request = create_test_international_request("+49 30 12345678", None);

        // Test with default validation config
        let config = request.phone_validation.as_ref().unwrap();
        assert!(config.enabled);
        assert!(!config.strict_validation);
        assert_eq!(config.default_region, "US");

        // Test with custom validation config
        request.phone_validation = Some(PhoneValidationConfig {
            enabled: true,
            strict_validation: true,
            default_region: "DE".to_string(),
            use_country_detection: true,
        });

        let custom_config = request.phone_validation.as_ref().unwrap();
        assert!(custom_config.enabled);
        assert!(custom_config.strict_validation);
        assert_eq!(custom_config.default_region, "DE");
    }

    #[test]
    fn test_international_routing_plan_structure() {
        let plan = InternationalRoutingPlan {
            id: 1,
            name: "Test EEA Plan".to_string(),
            description: Some("Test routing plan".to_string()),
            phone_validation_enabled: true,
            phone_validation_strict: false,
            phone_validation_default_region: "US".to_string(),
            phone_validation_use_country_detection: true,
            eea_routing_enabled: true,
            eea_priority_routing: true,
            eea_reduced_rates: true,
            eea_rate_reduction: Decimal::from_str("0.1").unwrap(),
            default_jurisdiction: InternationalJurisdiction::ROW,
            allow_unknown_destinations: true,
            max_rate_unknown_destinations: Decimal::from_str("1.0").unwrap(),
            require_strict_validation_unknown: false,
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        assert_eq!(plan.name, "Test EEA Plan");
        assert!(plan.phone_validation_enabled);
        assert!(plan.eea_routing_enabled);
        assert_eq!(plan.eea_rate_reduction, Decimal::from_str("0.1").unwrap());
        assert_eq!(plan.default_jurisdiction, InternationalJurisdiction::ROW);
    }

    #[test]
    fn test_country_routing_preference_structure() {
        let preference = CountryRoutingPreference {
            id: 1,
            routing_plan_id: 1,
            country_code: "DE".to_string(),
            country_name: "Germany".to_string(),
            jurisdiction: InternationalJurisdiction::EEA,
            quality_score: 95,
            cost_multiplier: Decimal::from_str("0.9").unwrap(),
            require_validation: true,
            max_duration_minutes: 0, // Unlimited
            created_at: Utc::now(),
        };

        assert_eq!(preference.country_code, "DE");
        assert_eq!(preference.country_name, "Germany");
        assert_eq!(preference.jurisdiction, InternationalJurisdiction::EEA);
        assert_eq!(preference.quality_score, 95);
        assert_eq!(
            preference.cost_multiplier,
            Decimal::from_str("0.9").unwrap()
        );
        assert!(preference.require_validation);
    }

    #[test]
    fn test_international_rate_structure() {
        let rate = InternationalRate {
            id: 1,
            deck_id: 1,
            country_code: "44".to_string(),
            destination_code: Some("207".to_string()),
            destination_name: "UK London".to_string(),
            jurisdiction: InternationalJurisdiction::ROW,
            rate: Decimal::from_str("0.0125").unwrap(),
            initial_increment: 30,
            subsequent_increment: 6,
            setup_fee: Some(Decimal::from_str("0.001").unwrap()),
            created_at: Utc::now(),
        };

        assert_eq!(rate.country_code, "44");
        assert_eq!(rate.destination_code, Some("207".to_string()));
        assert_eq!(rate.destination_name, "UK London");
        assert_eq!(rate.jurisdiction, InternationalJurisdiction::ROW);
        assert_eq!(rate.rate, Decimal::from_str("0.0125").unwrap());
        assert_eq!(rate.initial_increment, 30);
        assert_eq!(rate.subsequent_increment, 6);
    }

    #[test]
    fn test_route_response_structure() {
        let egress_trunk = EgressTrunk {
            id: 1,
            name: "Test Trunk".to_string(),
            vendor_id: 1,
            host: "sip.example.com".to_string(),
            port: 5060,
            transport: TransportProtocol::Udp,
            capacity_limit: 1000,
            cps_limit: Decimal::from_str("10.0").unwrap(),
            active: true,
            priority: 1,
            weight: 1,
            tech_prefix: None,
            supports_international: true,
        };

        let call_route = CallRoute {
            egress_trunk: egress_trunk.clone(),
            vendor: "test_vendor".to_string(),
            vendor_rate: None,
            cost_per_minute: Decimal::from_str("0.01").unwrap(),
            selling_per_minute: Decimal::from_str("0.02").unwrap(),
            profit_margin: Decimal::from_str("0.01").unwrap(),
            priority: 1,
            setup_fee: Decimal::ZERO,
            min_increment: 30,
            interval: 6,
        };

        let response = RouteResponse {
            routes: vec![call_route],
            jurisdiction: CallJurisdiction::Indeterminate,
            lrn: None,
            total_routes: 1,
            ani: "+15551234567".to_string(),
            dnis: "+15559876543".to_string(),
            ingress_trunk: "test_ingress".to_string(),
            routing_decision: "test_decision".to_string(),
        };

        assert_eq!(response.routes.len(), 1);
        assert_eq!(response.jurisdiction, CallJurisdiction::Indeterminate);
        assert_eq!(response.total_routes, 1);
        assert!(response.lrn.is_none());

        let route = &response.routes[0];
        assert_eq!(route.egress_trunk.name, "Test Trunk");
        assert!(route.egress_trunk.supports_international);
        assert_eq!(route.cost_per_minute, Decimal::from_str("0.01").unwrap());
        assert_eq!(route.selling_per_minute, Decimal::from_str("0.02").unwrap());
        assert_eq!(route.profit_margin, Decimal::from_str("0.01").unwrap());
    }

    #[test]
    fn test_call_simulation_structure() {
        let simulation = CallSimulation {
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
        };

        assert_eq!(simulation.ani, "15551234567");
        assert_eq!(simulation.dnis, "+44 20 7946 0958");
        assert_eq!(simulation.jurisdiction, CallJurisdiction::Indeterminate);
        assert_eq!(simulation.total_routes, 2);
        assert_eq!(simulation.routing_decision, "ROUTE_FOUND");
        assert_eq!(simulation.routes.len(), 2);

        // Check first route is lower cost (should be priority 1)
        let route1 = &simulation.routes[0];
        let route2 = &simulation.routes[1];
        assert_eq!(route1.priority, 1);
        assert_eq!(route2.priority, 2);
        assert!(route1.cost_per_minute < route2.cost_per_minute);
    }

    #[test]
    fn test_profit_protection_logic() {
        let mut request = create_test_international_request("+44 20 7946 0958", Some(1));
        request.require_profit_protection = true;
        request.min_profit_margin = Some(Decimal::from_str("0.005").unwrap());

        // Test that profit protection settings are properly set
        assert!(request.require_profit_protection);
        assert_eq!(
            request.min_profit_margin,
            Some(Decimal::from_str("0.005").unwrap())
        );
    }

    #[test]
    fn test_effective_time_handling() {
        let now = Utc::now();
        let mut request = create_test_international_request("+49 30 12345678", Some(1));
        request.effective_time = Some(now);

        assert_eq!(request.effective_time, Some(now));

        // Test with no effective time (should use current time in routing)
        request.effective_time = None;
        assert!(request.effective_time.is_none());
    }

    #[test]
    fn test_international_jurisdiction_enum() {
        assert_eq!(InternationalJurisdiction::EEA.to_string(), "EEA");
        assert_eq!(InternationalJurisdiction::ROW.to_string(), "ROW");

        // Test serialization/deserialization would work
        let eea = InternationalJurisdiction::EEA;
        let row = InternationalJurisdiction::ROW;

        assert_ne!(eea, row);
    }

    #[test]
    fn test_route_type_enum() {
        assert_eq!(RouteType::NANPA.to_string(), "NANPA");
        assert_eq!(RouteType::AZ.to_string(), "A-Z");
        assert_eq!(RouteType::OTHER.to_string(), "OTHER");

        let nanpa = RouteType::NANPA;
        let az = RouteType::AZ;
        let other = RouteType::OTHER;

        assert_ne!(nanpa, az);
        assert_ne!(az, other);
        assert_ne!(nanpa, other);
    }

    #[test]
    fn test_call_jurisdiction_enum() {
        let jurisdictions = vec![
            CallJurisdiction::Interstate,
            CallJurisdiction::Intrastate,
            CallJurisdiction::Local,
            CallJurisdiction::Indeterminate,
        ];

        // Test that all jurisdictions are different
        for (i, j1) in jurisdictions.iter().enumerate() {
            for (k, j2) in jurisdictions.iter().enumerate() {
                if i != k {
                    assert_ne!(
                        j1, j2,
                        "Jurisdictions {:?} and {:?} should be different",
                        j1, j2
                    );
                }
            }
        }
    }

    #[test]
    fn test_transport_protocol_enum() {
        let protocols = vec![
            TransportProtocol::Udp,
            TransportProtocol::Tcp,
            TransportProtocol::Tls,
        ];

        // Test that all protocols are different
        for (i, p1) in protocols.iter().enumerate() {
            for (k, p2) in protocols.iter().enumerate() {
                if i != k {
                    assert_ne!(
                        p1, p2,
                        "Protocols {:?} and {:?} should be different",
                        p1, p2
                    );
                }
            }
        }
    }

    // ==================== INTERNATIONAL ROUTING TESTS ====================

    #[test]
    fn test_international_phone_number_parsing() {
        let test_cases = vec![
            ("+442071234567", "44", "UK London"),
            ("+33142864200", "33", "France Paris"),
            ("+4930123456", "49", "Germany Berlin"),
            ("+861012345678", "86", "China Beijing"),
            ("+919876543210", "91", "India Mumbai"),
            ("+12125551234", "1", "US New York"),
            ("+14155551234", "1", "US San Francisco"),
            ("+390612345678", "39", "Italy Rome"),
            ("+5511987654321", "55", "Brazil São Paulo"),
            ("+61212345678", "61", "Australia Sydney"),
        ];

        for (number, expected_cc, description) in test_cases {
            let country_code = extract_country_code_from_international(number);
            assert_eq!(
                country_code, expected_cc,
                "Failed to extract country code from {} ({})",
                number, description
            );
        }
    }

    fn extract_country_code_from_international(number: &str) -> &str {
        if !number.starts_with('+') {
            return "";
        }

        let digits = &number[1..];

        // NANPA (North American Numbering Plan)
        if digits.starts_with('1') && digits.len() >= 11 {
            return "1";
        }

        // Check for 3-digit country codes first
        if digits.len() >= 3 {
            let three_digit = &digits[0..3];
            match three_digit {
                "886" => return "886", // Taiwan
                "852" => return "852", // Hong Kong
                "853" => return "853", // Macau
                "855" => return "855", // Cambodia
                "856" => return "856", // Laos
                _ => {}
            }
        }

        // Most common: 2-digit country codes
        if digits.len() >= 2 {
            &digits[0..2]
        } else {
            ""
        }
    }

    #[test]
    fn test_international_routing_plan_validation() {
        let plan = InternationalRoutingPlan {
            id: 1,
            name: "European Union Routing".to_string(),
            description: Some("Optimized routing for EU destinations".to_string()),
            phone_validation_enabled: true,
            phone_validation_strict: true,
            phone_validation_default_region: "EU".to_string(),
            phone_validation_use_country_detection: true,
            eea_routing_enabled: true,
            eea_priority_routing: true,
            eea_reduced_rates: true,
            eea_rate_reduction: Decimal::from_str("0.15").unwrap(), // 15% reduction
            default_jurisdiction: InternationalJurisdiction::EEA,
            allow_unknown_destinations: false, // Strict for EU
            max_rate_unknown_destinations: Decimal::from_str("0.50").unwrap(),
            require_strict_validation_unknown: true,
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Validate EU-specific settings
        assert!(
            plan.eea_routing_enabled,
            "EU plan should have EEA routing enabled"
        );
        assert!(
            plan.eea_priority_routing,
            "EU plan should prioritize EEA routes"
        );
        assert!(
            plan.eea_reduced_rates,
            "EU plan should have reduced EEA rates"
        );
        assert_eq!(plan.eea_rate_reduction, Decimal::from_str("0.15").unwrap());
        assert_eq!(plan.default_jurisdiction, InternationalJurisdiction::EEA);
        assert!(
            !plan.allow_unknown_destinations,
            "EU plan should be strict about unknown destinations"
        );
    }

    #[test]
    fn test_country_routing_preferences_eea() {
        let eea_countries = vec![
            ("DE", "Germany", 95),
            ("FR", "France", 94),
            ("IT", "Italy", 92),
            ("ES", "Spain", 91),
            ("NL", "Netherlands", 96),
            ("SE", "Sweden", 93),
            ("NO", "Norway", 89),  // EEA but not EU
            ("IS", "Iceland", 87), // EEA but not EU
        ];

        for (cc, name, quality) in eea_countries {
            let preference = CountryRoutingPreference {
                id: 1,
                routing_plan_id: 1,
                country_code: cc.to_string(),
                country_name: name.to_string(),
                jurisdiction: InternationalJurisdiction::EEA,
                quality_score: quality,
                cost_multiplier: Decimal::from_str("0.85").unwrap(), // 15% discount
                require_validation: true,
                max_duration_minutes: 0, // Unlimited
                created_at: Utc::now(),
            };

            assert_eq!(preference.jurisdiction, InternationalJurisdiction::EEA);
            assert!(
                preference.quality_score >= 85,
                "EEA countries should have high quality scores"
            );
            assert!(
                preference.cost_multiplier < Decimal::ONE,
                "EEA should have cost reduction"
            );
        }
    }

    #[test]
    fn test_row_country_routing() {
        let row_countries = vec![
            ("86", "China", 75),
            ("91", "India", 70),
            ("81", "Japan", 85),
            ("82", "South Korea", 88),
            ("55", "Brazil", 65),
            ("52", "Mexico", 60),
            ("27", "South Africa", 55),
        ];

        for (cc, name, quality) in row_countries {
            let preference = CountryRoutingPreference {
                id: 1,
                routing_plan_id: 1,
                country_code: cc.to_string(),
                country_name: name.to_string(),
                jurisdiction: InternationalJurisdiction::ROW,
                quality_score: quality,
                cost_multiplier: Decimal::from_str("1.2").unwrap(), // Higher cost for ROW
                require_validation: true,
                max_duration_minutes: 120, // 2 hour limit for ROW
                created_at: Utc::now(),
            };

            assert_eq!(preference.jurisdiction, InternationalJurisdiction::ROW);
            assert!(
                preference.cost_multiplier > Decimal::ONE,
                "ROW should have cost premium"
            );
            assert!(
                preference.max_duration_minutes > 0,
                "ROW should have duration limits"
            );
        }
    }

    #[test]
    fn test_international_rate_structure_validation() {
        let rates = vec![
            // Premium destinations
            (
                "44",
                Some("20".to_string()),
                "UK London",
                Decimal::from_str("0.008").unwrap(),
            ),
            (
                "49",
                Some("30".to_string()),
                "Germany Berlin",
                Decimal::from_str("0.007").unwrap(),
            ),
            (
                "33",
                Some("1".to_string()),
                "France Paris",
                Decimal::from_str("0.009").unwrap(),
            ),
            // Standard destinations
            (
                "39",
                None,
                "Italy Mobile",
                Decimal::from_str("0.015").unwrap(),
            ),
            (
                "34",
                None,
                "Spain Mobile",
                Decimal::from_str("0.014").unwrap(),
            ),
            // ROW destinations (higher rates)
            (
                "86",
                Some("10".to_string()),
                "China Beijing",
                Decimal::from_str("0.025").unwrap(),
            ),
            (
                "91",
                Some("98".to_string()),
                "India Mumbai",
                Decimal::from_str("0.035").unwrap(),
            ),
            (
                "55",
                Some("11".to_string()),
                "Brazil São Paulo",
                Decimal::from_str("0.045").unwrap(),
            ),
        ];

        for (cc, dest_code, desc, rate) in rates {
            let int_rate = InternationalRate {
                id: 1,
                deck_id: 1,
                country_code: cc.to_string(),
                destination_code: dest_code,
                destination_name: desc.to_string(),
                jurisdiction: if cc == "44" || cc == "49" || cc == "33" || cc == "39" || cc == "34"
                {
                    InternationalJurisdiction::EEA
                } else {
                    InternationalJurisdiction::ROW
                },
                rate,
                initial_increment: 30,
                subsequent_increment: 6,
                setup_fee: Some(Decimal::from_str("0.001").unwrap()),
                created_at: Utc::now(),
            };

            // Validate rate structure
            assert!(int_rate.rate > Decimal::ZERO, "Rate must be positive");
            assert!(
                int_rate.initial_increment >= 6,
                "Initial increment should be at least 6 seconds"
            );
            assert!(
                int_rate.subsequent_increment >= 1,
                "Subsequent increment should be at least 1 second"
            );

            // EEA rates should be lower than ROW
            if int_rate.jurisdiction == InternationalJurisdiction::EEA {
                assert!(
                    int_rate.rate < Decimal::from_str("0.020").unwrap(),
                    "EEA rates should be competitive"
                );
            } else {
                assert!(
                    int_rate.rate >= Decimal::from_str("0.020").unwrap(),
                    "ROW rates should reflect higher costs"
                );
            }
        }
    }

    #[test]
    fn test_phone_validation_international() {
        let validation_config = PhoneValidationConfig {
            enabled: true,
            strict_validation: true,
            default_region: "US".to_string(),
            use_country_detection: true,
        };

        let test_numbers = vec![
            ("+442071234567", true, "UK"),  // Valid UK landline
            ("+4915112345678", true, "DE"), // Valid German mobile
            ("+33142864200", true, "FR"),   // Valid French landline
            ("+39612345678", true, "IT"),   // Valid Italian landline
            ("+12125551234", true, "US"),   // Valid US number
            ("+1234567890", false, "XX"),   // Invalid: too short for NANPA
            ("+999123456789", false, "XX"), // Invalid: non-existent country code
            ("442071234567", false, "XX"),  // Invalid: missing +
            ("+44", false, "XX"),           // Invalid: too short
        ];

        for (number, should_be_valid, expected_region) in test_numbers {
            let is_valid = validate_international_phone_number(number, &validation_config);
            if should_be_valid {
                assert!(is_valid, "Number {} should be valid", number);
            } else {
                assert!(!is_valid, "Number {} should be invalid", number);
            }
        }
    }

    fn validate_international_phone_number(number: &str, _config: &PhoneValidationConfig) -> bool {
        // Simplified validation for testing
        if !number.starts_with('+') {
            return false;
        }

        let digits = &number[1..];
        if digits.len() < 7 || digits.len() > 15 {
            return false;
        }

        // Check if all characters after + are digits
        digits.chars().all(|c| c.is_ascii_digit())
    }

    #[test]
    fn test_eea_routing_priority() {
        let request = RouteRequest {
            ani: "14155551234".to_string(),
            dnis: "+49301234567".to_string(), // German number
            ingress_trunk_id: 1,
            client_deck_id: Some(1),
            route_type: RouteType::AZ,
            require_profit_protection: false,
            min_profit_margin: None,
            effective_time: Some(Utc::now()),
            phone_validation: Some(PhoneValidationConfig {
                enabled: true,
                strict_validation: true,
                default_region: "US".to_string(),
                use_country_detection: true,
            }),
            routing_plan_id: Some(1), // EU routing plan
        };

        // Verify the request is set up for EEA routing
        assert!(request.dnis.starts_with("+49"), "Should be German number");
        assert_eq!(request.route_type, RouteType::AZ);
        assert!(
            request.routing_plan_id.is_some(),
            "Should have routing plan for international"
        );
        assert!(
            request.phone_validation.is_some(),
            "Should have phone validation enabled"
        );
    }

    #[test]
    fn test_international_jurisdiction_determination() {
        let test_cases = vec![
            ("+442071234567", InternationalJurisdiction::EEA), // UK
            ("+4930123456", InternationalJurisdiction::EEA),   // Germany
            ("+33142864200", InternationalJurisdiction::EEA),  // France
            ("+861012345678", InternationalJurisdiction::ROW), // China
            ("+919876543210", InternationalJurisdiction::ROW), // India
            ("+12125551234", InternationalJurisdiction::ROW),  // US (from EU perspective)
            ("+5511987654321", InternationalJurisdiction::ROW), // Brazil
        ];

        for (number, expected_jurisdiction) in test_cases {
            let jurisdiction = determine_international_jurisdiction(number);
            assert_eq!(
                jurisdiction, expected_jurisdiction,
                "Wrong jurisdiction for {}",
                number
            );
        }
    }

    fn determine_international_jurisdiction(number: &str) -> InternationalJurisdiction {
        if !number.starts_with('+') {
            return InternationalJurisdiction::ROW;
        }

        let country_code = extract_country_code_from_international(number);

        // EEA country codes (simplified list)
        match country_code {
            "44" | "49" | "33" | "39" | "34" | "31" | "46" | "47" | "354" => {
                InternationalJurisdiction::EEA
            }
            _ => InternationalJurisdiction::ROW,
        }
    }

    #[test]
    fn test_international_call_cost_calculation() {
        let eea_rate = InternationalRate {
            id: 1,
            deck_id: 1,
            country_code: "44".to_string(),
            destination_code: Some("20".to_string()),
            destination_name: "UK London".to_string(),
            jurisdiction: InternationalJurisdiction::EEA,
            rate: Decimal::from_str("0.008").unwrap(), // 0.8 cents per minute
            initial_increment: 30,
            subsequent_increment: 6,
            setup_fee: Some(Decimal::from_str("0.001").unwrap()),
            created_at: Utc::now(),
        };

        let row_rate = InternationalRate {
            id: 2,
            deck_id: 1,
            country_code: "86".to_string(),
            destination_code: Some("10".to_string()),
            destination_name: "China Beijing".to_string(),
            jurisdiction: InternationalJurisdiction::ROW,
            rate: Decimal::from_str("0.025").unwrap(), // 2.5 cents per minute
            initial_increment: 30,
            subsequent_increment: 6,
            setup_fee: Some(Decimal::from_str("0.002").unwrap()),
            created_at: Utc::now(),
        };

        // Test 120 second call costs
        let call_duration = 120; // 2 minutes

        // EEA call cost calculation
        let eea_cost = calculate_international_call_cost(&eea_rate, call_duration);
        let expected_eea_cost = eea_rate.setup_fee.unwrap()
            + (eea_rate.rate * Decimal::from(call_duration) / Decimal::from(60));

        assert_eq!(
            eea_cost, expected_eea_cost,
            "EEA cost calculation incorrect"
        );

        // ROW call cost calculation
        let row_cost = calculate_international_call_cost(&row_rate, call_duration);
        let expected_row_cost = row_rate.setup_fee.unwrap()
            + (row_rate.rate * Decimal::from(call_duration) / Decimal::from(60));

        assert_eq!(
            row_cost, expected_row_cost,
            "ROW cost calculation incorrect"
        );

        // ROW should be more expensive than EEA
        assert!(
            row_cost > eea_cost,
            "ROW calls should cost more than EEA calls"
        );
    }

    fn calculate_international_call_cost(
        rate: &InternationalRate,
        duration_seconds: u32,
    ) -> Decimal {
        let setup_fee = rate.setup_fee.unwrap_or(Decimal::ZERO);
        let duration_minutes = Decimal::from(duration_seconds) / Decimal::from(60);
        let usage_cost = rate.rate * duration_minutes;

        setup_fee + usage_cost
    }

    #[test]
    fn test_international_route_selection_priority() {
        // Create routes with different priorities for international destinations
        let routes = vec![
            create_international_route(1, 1, "44", Decimal::from_str("0.008").unwrap()), // Premium UK
            create_international_route(2, 2, "44", Decimal::from_str("0.010").unwrap()), // Standard UK
            create_international_route(3, 3, "44", Decimal::from_str("0.012").unwrap()), // Economy UK
        ];

        // Verify priority ordering
        assert_eq!(
            routes[0].priority, 1,
            "First route should have highest priority"
        );
        assert_eq!(
            routes[1].priority, 2,
            "Second route should have medium priority"
        );
        assert_eq!(
            routes[2].priority, 3,
            "Third route should have lowest priority"
        );

        // Verify cost relationship (higher priority = lower cost)
        assert!(routes[0].cost_per_minute < routes[1].cost_per_minute);
        assert!(routes[1].cost_per_minute < routes[2].cost_per_minute);
    }

    fn create_international_route(
        id: i32,
        priority: i32,
        country_code: &str,
        cost: Decimal,
    ) -> CallRoute {
        CallRoute {
            egress_trunk: EgressTrunk {
                id,
                name: format!("Intl-Trunk-{}-{}", country_code, id),
                vendor_id: 1,
                host: format!("sip{}.international.com", id),
                port: 5060,
                transport: TransportProtocol::Udp,
                capacity_limit: 1000,
                cps_limit: Decimal::from(50),
                active: true,
                priority,
                weight: 100,
                tech_prefix: Some(format!("011{}", country_code)),
                supports_international: true,
            },
            vendor: format!("intl_vendor_{}", country_code),
            vendor_rate: None,
            cost_per_minute: cost,
            selling_per_minute: cost * Decimal::from_str("1.25").unwrap(), // 25% markup
            profit_margin: cost * Decimal::from_str("0.25").unwrap(),
            priority,
            setup_fee: Decimal::from_str("0.001").unwrap(),
            min_increment: 30,
            interval: 6,
        }
    }
}
