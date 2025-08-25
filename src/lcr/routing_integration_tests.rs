#[cfg(test)]
mod routing_integration_tests {
    use super::super::types::*;
    use super::super::phone_validation::*;
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
        assert_eq!(preference.cost_multiplier, Decimal::from_str("0.9").unwrap());
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
            transport: TransportProtocol::UDP,
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
                }
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
        assert_eq!(request.min_profit_margin, Some(Decimal::from_str("0.005").unwrap()));
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
            CallJurisdiction::Inter,
            CallJurisdiction::Intra,
            CallJurisdiction::Local,
            CallJurisdiction::Indeterminate,
        ];

        // Test that all jurisdictions are different
        for (i, j1) in jurisdictions.iter().enumerate() {
            for (k, j2) in jurisdictions.iter().enumerate() {
                if i != k {
                    assert_ne!(j1, j2, "Jurisdictions {:?} and {:?} should be different", j1, j2);
                }
            }
        }
    }

    #[test]
    fn test_transport_protocol_enum() {
        let protocols = vec![
            TransportProtocol::UDP,
            TransportProtocol::TCP,
            TransportProtocol::TLS,
        ];

        // Test that all protocols are different
        for (i, p1) in protocols.iter().enumerate() {
            for (k, p2) in protocols.iter().enumerate() {
                if i != k {
                    assert_ne!(p1, p2, "Protocols {:?} and {:?} should be different", p1, p2);
                }
            }
        }
    }
}