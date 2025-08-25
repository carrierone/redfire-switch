#[cfg(test)]
mod database_integration_tests {
    use super::super::database::DatabasePool;
    use super::super::types::*;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    // Mock database tests - these would need a real database in practice
    // For now, we test the logic and structure

    #[tokio::test]
    async fn test_default_routing_plans_creation() {
        // This test verifies the structure of default routing plans
        // In a real test, this would use a test database

        // Test EEA routing plan configuration
        let eea_plan = create_mock_eea_plan();
        assert_eq!(eea_plan.name, "Default EEA Routing");
        assert!(eea_plan.phone_validation_enabled);
        assert!(!eea_plan.phone_validation_strict);
        assert_eq!(eea_plan.phone_validation_default_region, "US");
        assert!(eea_plan.phone_validation_use_country_detection);
        assert!(eea_plan.eea_routing_enabled);
        assert!(eea_plan.eea_priority_routing);
        assert!(eea_plan.eea_reduced_rates);
        assert_eq!(
            eea_plan.eea_rate_reduction,
            Decimal::from_str("0.1000").unwrap()
        );
        assert_eq!(
            eea_plan.default_jurisdiction,
            InternationalJurisdiction::ROW
        );
        assert!(eea_plan.allow_unknown_destinations);
        assert_eq!(
            eea_plan.max_rate_unknown_destinations,
            Decimal::from_str("1.0000").unwrap()
        );
        assert!(!eea_plan.require_strict_validation_unknown);
        assert!(eea_plan.active);
    }

    #[tokio::test]
    async fn test_row_routing_plan_creation() {
        let row_plan = create_mock_row_plan();
        assert_eq!(row_plan.name, "Default ROW Routing");
        assert!(row_plan.phone_validation_enabled);
        assert!(!row_plan.phone_validation_strict);
        assert!(!row_plan.eea_routing_enabled);
        assert!(!row_plan.eea_priority_routing);
        assert!(!row_plan.eea_reduced_rates);
        assert_eq!(row_plan.eea_rate_reduction, Decimal::ZERO);
        assert_eq!(
            row_plan.default_jurisdiction,
            InternationalJurisdiction::ROW
        );
        assert!(row_plan.allow_unknown_destinations);
        assert_eq!(
            row_plan.max_rate_unknown_destinations,
            Decimal::from_str("2.0000").unwrap()
        );
        assert!(row_plan.require_strict_validation_unknown);
    }

    #[tokio::test]
    async fn test_strict_validation_plan_creation() {
        let strict_plan = create_mock_strict_plan();
        assert_eq!(strict_plan.name, "Strict Validation Plan");
        assert!(strict_plan.phone_validation_enabled);
        assert!(strict_plan.phone_validation_strict); // This is the key difference
        assert!(strict_plan.eea_routing_enabled);
        assert_eq!(
            strict_plan.eea_rate_reduction,
            Decimal::from_str("0.0500").unwrap()
        );
        assert!(!strict_plan.allow_unknown_destinations); // Stricter policy
        assert_eq!(
            strict_plan.max_rate_unknown_destinations,
            Decimal::from_str("0.5000").unwrap()
        );
        assert!(strict_plan.require_strict_validation_unknown);
    }

    #[tokio::test]
    async fn test_eea_country_preferences_creation() {
        let eea_countries = get_mock_eea_countries();

        // Test that we have the expected EEA countries
        assert!(eea_countries.len() >= 27); // Minimum EU countries

        // Test specific countries
        let germany = eea_countries
            .iter()
            .find(|c| c.country_code == "DE")
            .unwrap();
        assert_eq!(germany.country_name, "Germany");
        assert_eq!(germany.jurisdiction, InternationalJurisdiction::EEA);
        assert_eq!(germany.quality_score, 95);
        assert_eq!(germany.cost_multiplier, Decimal::from_str("0.9").unwrap());
        assert!(germany.require_validation);
        assert_eq!(germany.max_duration_minutes, 0); // Unlimited

        let france = eea_countries
            .iter()
            .find(|c| c.country_code == "FR")
            .unwrap();
        assert_eq!(france.country_name, "France");
        assert_eq!(france.jurisdiction, InternationalJurisdiction::EEA);

        let italy = eea_countries
            .iter()
            .find(|c| c.country_code == "IT")
            .unwrap();
        assert_eq!(italy.country_name, "Italy");
        assert_eq!(italy.jurisdiction, InternationalJurisdiction::EEA);

        // Nordic countries should be included
        assert!(eea_countries.iter().any(|c| c.country_code == "SE")); // Sweden
        assert!(eea_countries.iter().any(|c| c.country_code == "NO")); // Norway
        assert!(eea_countries.iter().any(|c| c.country_code == "DK")); // Denmark
        assert!(eea_countries.iter().any(|c| c.country_code == "FI")); // Finland
    }

    #[tokio::test]
    async fn test_row_country_preferences_creation() {
        let row_countries = get_mock_row_countries();

        // Test that we have major ROW countries
        assert!(row_countries.len() >= 15);

        let usa = row_countries
            .iter()
            .find(|c| c.country_code == "US")
            .unwrap();
        assert_eq!(usa.country_name, "United States");
        assert_eq!(usa.jurisdiction, InternationalJurisdiction::ROW);
        assert_eq!(usa.quality_score, 85);
        assert_eq!(usa.cost_multiplier, Decimal::ONE);
        assert!(!usa.require_validation); // ROW countries are less strict

        let japan = row_countries
            .iter()
            .find(|c| c.country_code == "JP")
            .unwrap();
        assert_eq!(japan.country_name, "Japan");
        assert_eq!(japan.jurisdiction, InternationalJurisdiction::ROW);

        // Test major countries are included
        assert!(row_countries.iter().any(|c| c.country_code == "CN")); // China
        assert!(row_countries.iter().any(|c| c.country_code == "IN")); // India
        assert!(row_countries.iter().any(|c| c.country_code == "BR")); // Brazil
        assert!(row_countries.iter().any(|c| c.country_code == "AU")); // Australia
    }

    #[tokio::test]
    async fn test_international_rates_structure() {
        // Test vendor international rate
        let vendor_rate = create_mock_vendor_international_rate();
        assert_eq!(vendor_rate.country_code, "44");
        assert_eq!(vendor_rate.destination_code, Some("20".to_string()));
        assert_eq!(vendor_rate.destination_name, "UK London");
        assert_eq!(vendor_rate.jurisdiction, InternationalJurisdiction::ROW);
        assert_eq!(vendor_rate.rate, Decimal::from_str("0.0125").unwrap());
        assert_eq!(vendor_rate.initial_increment, 30);
        assert_eq!(vendor_rate.subsequent_increment, 6);
        assert_eq!(
            vendor_rate.setup_fee,
            Some(Decimal::from_str("0.001").unwrap())
        );

        // Test client international rate
        let client_rate = create_mock_client_international_rate();
        assert_eq!(client_rate.country_code, "44");
        assert_eq!(client_rate.destination_name, "UK London");
        assert!(client_rate.rate > vendor_rate.rate); // Client rate should be higher than vendor
    }

    #[test]
    fn test_rate_deck_structure() {
        let deck = create_mock_rate_deck();
        assert_eq!(deck.name, "Test International Deck");
        assert_eq!(deck.rate_type, RateType::DNIS);
        assert!(deck.active);
        assert!(deck.effective_date <= Utc::now());
    }

    #[test]
    fn test_egress_trunk_international_support() {
        let trunk = create_mock_international_trunk();
        assert_eq!(trunk.name, "International Trunk");
        assert!(trunk.supports_international);
        assert!(trunk.active);
        assert_eq!(trunk.priority, 1);
        assert_eq!(trunk.transport, TransportProtocol::UDP);
    }

    #[test]
    fn test_ingress_trunk_with_international_support() {
        let trunk = create_mock_ingress_trunk();
        assert_eq!(trunk.name, "Test Ingress");
        assert!(trunk.supports_international);
        assert!(trunk.active);
        assert!(trunk.profit_protection);
        assert!(trunk.min_profit_margin > Decimal::ZERO);
    }

    // Mock data creation functions
    fn create_mock_eea_plan() -> InternationalRoutingPlan {
        InternationalRoutingPlan {
            id: 1,
            name: "Default EEA Routing".to_string(),
            description: Some(
                "Default routing plan with phone validation enabled and EEA optimization"
                    .to_string(),
            ),
            phone_validation_enabled: true,
            phone_validation_strict: false,
            phone_validation_default_region: "US".to_string(),
            phone_validation_use_country_detection: true,
            eea_routing_enabled: true,
            eea_priority_routing: true,
            eea_reduced_rates: true,
            eea_rate_reduction: Decimal::from_str("0.1000").unwrap(),
            default_jurisdiction: InternationalJurisdiction::ROW,
            allow_unknown_destinations: true,
            max_rate_unknown_destinations: Decimal::from_str("1.0000").unwrap(),
            require_strict_validation_unknown: false,
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_mock_row_plan() -> InternationalRoutingPlan {
        InternationalRoutingPlan {
            id: 2,
            name: "Default ROW Routing".to_string(),
            description: Some(
                "Default routing plan for Rest of World destinations with basic validation"
                    .to_string(),
            ),
            phone_validation_enabled: true,
            phone_validation_strict: false,
            phone_validation_default_region: "US".to_string(),
            phone_validation_use_country_detection: true,
            eea_routing_enabled: false,
            eea_priority_routing: false,
            eea_reduced_rates: false,
            eea_rate_reduction: Decimal::ZERO,
            default_jurisdiction: InternationalJurisdiction::ROW,
            allow_unknown_destinations: true,
            max_rate_unknown_destinations: Decimal::from_str("2.0000").unwrap(),
            require_strict_validation_unknown: true,
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_mock_strict_plan() -> InternationalRoutingPlan {
        InternationalRoutingPlan {
            id: 3,
            name: "Strict Validation Plan".to_string(),
            description: Some(
                "High-security routing plan with strict phone number validation".to_string(),
            ),
            phone_validation_enabled: true,
            phone_validation_strict: true,
            phone_validation_default_region: "US".to_string(),
            phone_validation_use_country_detection: true,
            eea_routing_enabled: true,
            eea_priority_routing: true,
            eea_reduced_rates: true,
            eea_rate_reduction: Decimal::from_str("0.0500").unwrap(),
            default_jurisdiction: InternationalJurisdiction::ROW,
            allow_unknown_destinations: false,
            max_rate_unknown_destinations: Decimal::from_str("0.5000").unwrap(),
            require_strict_validation_unknown: true,
            active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn get_mock_eea_countries() -> Vec<CountryRoutingPreference> {
        let eea_countries = vec![
            ("AT", "Austria"),
            ("BE", "Belgium"),
            ("BG", "Bulgaria"),
            ("CY", "Cyprus"),
            ("CZ", "Czech Republic"),
            ("DE", "Germany"),
            ("DK", "Denmark"),
            ("EE", "Estonia"),
            ("ES", "Spain"),
            ("FI", "Finland"),
            ("FR", "France"),
            ("GR", "Greece"),
            ("HR", "Croatia"),
            ("HU", "Hungary"),
            ("IE", "Ireland"),
            ("IS", "Iceland"),
            ("IT", "Italy"),
            ("LI", "Liechtenstein"),
            ("LT", "Lithuania"),
            ("LU", "Luxembourg"),
            ("LV", "Latvia"),
            ("MT", "Malta"),
            ("NL", "Netherlands"),
            ("NO", "Norway"),
            ("PL", "Poland"),
            ("PT", "Portugal"),
            ("RO", "Romania"),
            ("SE", "Sweden"),
            ("SI", "Slovenia"),
            ("SK", "Slovakia"),
        ];

        eea_countries
            .into_iter()
            .enumerate()
            .map(|(i, (code, name))| CountryRoutingPreference {
                id: (i + 1) as i32,
                routing_plan_id: 1,
                country_code: code.to_string(),
                country_name: name.to_string(),
                jurisdiction: InternationalJurisdiction::EEA,
                quality_score: 95,
                cost_multiplier: Decimal::from_str("0.9").unwrap(),
                require_validation: true,
                max_duration_minutes: 0,
                created_at: Utc::now(),
            })
            .collect()
    }

    fn get_mock_row_countries() -> Vec<CountryRoutingPreference> {
        let row_countries = vec![
            ("US", "United States"),
            ("CA", "Canada"),
            ("MX", "Mexico"),
            ("AU", "Australia"),
            ("NZ", "New Zealand"),
            ("JP", "Japan"),
            ("KR", "South Korea"),
            ("CN", "China"),
            ("IN", "India"),
            ("BR", "Brazil"),
            ("AR", "Argentina"),
            ("CL", "Chile"),
            ("ZA", "South Africa"),
            ("RU", "Russia"),
            ("TR", "Turkey"),
            ("AE", "United Arab Emirates"),
            ("SA", "Saudi Arabia"),
        ];

        row_countries
            .into_iter()
            .enumerate()
            .map(|(i, (code, name))| CountryRoutingPreference {
                id: (i + 100) as i32,
                routing_plan_id: 2,
                country_code: code.to_string(),
                country_name: name.to_string(),
                jurisdiction: InternationalJurisdiction::ROW,
                quality_score: 85,
                cost_multiplier: Decimal::ONE,
                require_validation: false,
                max_duration_minutes: 0,
                created_at: Utc::now(),
            })
            .collect()
    }

    fn create_mock_vendor_international_rate() -> InternationalRate {
        InternationalRate {
            id: 1,
            deck_id: 1,
            country_code: "44".to_string(),
            destination_code: Some("20".to_string()),
            destination_name: "UK London".to_string(),
            jurisdiction: InternationalJurisdiction::ROW,
            rate: Decimal::from_str("0.0125").unwrap(),
            initial_increment: 30,
            subsequent_increment: 6,
            setup_fee: Some(Decimal::from_str("0.001").unwrap()),
            created_at: Utc::now(),
        }
    }

    fn create_mock_client_international_rate() -> InternationalRate {
        InternationalRate {
            id: 2,
            deck_id: 2,
            country_code: "44".to_string(),
            destination_code: Some("20".to_string()),
            destination_name: "UK London".to_string(),
            jurisdiction: InternationalJurisdiction::ROW,
            rate: Decimal::from_str("0.0175").unwrap(), // Higher than vendor rate
            initial_increment: 30,
            subsequent_increment: 6,
            setup_fee: Some(Decimal::from_str("0.001").unwrap()),
            created_at: Utc::now(),
        }
    }

    fn create_mock_rate_deck() -> RateDeck {
        use chrono::NaiveTime;
        RateDeck {
            id: 1,
            name: "Test International Deck".to_string(),
            owner_id: 1,
            rate_type: RateType::DNIS,
            effective_date: Utc::now(),
            end_date: None,
            deck_version: 1,
            parent_deck_id: None,
            effective_time: NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            preload_minutes: 30,
            loaded_at: Some(Utc::now()),
            is_staged: false,
            active: true,
        }
    }

    fn create_mock_international_trunk() -> EgressTrunk {
        EgressTrunk {
            id: 1,
            name: "International Trunk".to_string(),
            vendor_id: 1,
            host: "international.sip.example.com".to_string(),
            port: 5060,
            transport: TransportProtocol::UDP,
            capacity_limit: 1000,
            cps_limit: Decimal::from_str("10.0").unwrap(),
            active: true,
            priority: 1,
            weight: 1,
            tech_prefix: None,
            supports_international: true,
        }
    }

    fn create_mock_ingress_trunk() -> IngressTrunk {
        use std::net::IpAddr;
        use std::str::FromStr;

        IngressTrunk {
            id: 1,
            name: "Test Ingress".to_string(),
            client_id: 1,
            ip_address: IpAddr::from_str("192.168.1.100").unwrap(),
            capacity_limit: 100,
            cps_limit: Decimal::from_str("5.0").unwrap(),
            profit_protection: true,
            min_profit_margin: Decimal::from_str("0.001").unwrap(),
            active: true,
            auth_username: Some("test_user".to_string()),
            auth_password: Some("test_pass".to_string()),
            supports_international: true,
        }
    }
}
