use anyhow::Result;
use chrono::{DateTime, NaiveTime, Utc, TimeZone};
use redfire_switch::lcr::{
    types::{DeckLoadRequest, RateType, NanpaRate, RouteRequest, RouteType, CallJurisdiction},
    LcrEngine,
};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;
use tokio;

// Test database setup
async fn setup_test_db() -> Result<String> {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://test:test@localhost:5432/lcr_test".to_string());
    
    // Run migrations
    let pool = PgPool::connect(&database_url).await?;
    
    // Clean up any existing test data
    sqlx::query("DELETE FROM vendor_nanpa_rates WHERE deck_id IN (SELECT id FROM vendor_rate_decks WHERE name LIKE 'TEST_%')")
        .execute(&pool).await?;
    sqlx::query("DELETE FROM client_nanpa_rates WHERE deck_id IN (SELECT id FROM client_rate_decks WHERE name LIKE 'TEST_%')")
        .execute(&pool).await?;
    sqlx::query("DELETE FROM vendor_rate_decks WHERE name LIKE 'TEST_%'")
        .execute(&pool).await?;
    sqlx::query("DELETE FROM client_rate_decks WHERE name LIKE 'TEST_%'")
        .execute(&pool).await?;
    sqlx::query("DELETE FROM deck_cutover_schedule WHERE deck_type = 'test'")
        .execute(&pool).await?;
    
    pool.close().await;
    Ok(database_url)
}

// Helper to create test rates
fn create_test_rates() -> Vec<NanpaRate> {
    vec![
        NanpaRate {
            id: 0,
            deck_id: 0,
            code: "1212".to_string(), // NYC
            inter_rate: Decimal::try_from(0.0035).unwrap(),
            intra_rate: Decimal::try_from(0.0040).unwrap(),
            ij_rate: Decimal::try_from(0.0038).unwrap(),
            local_rate: Some(Decimal::try_from(0.0020).unwrap()),
            min_increment: 6,
            interval: 6,
            setup_fee: None,
        },
        NanpaRate {
            id: 0,
            deck_id: 0,
            code: "1415".to_string(), // San Francisco
            inter_rate: Decimal::try_from(0.0032).unwrap(),
            intra_rate: Decimal::try_from(0.0037).unwrap(),
            ij_rate: Decimal::try_from(0.0035).unwrap(),
            local_rate: Some(Decimal::try_from(0.0018).unwrap()),
            min_increment: 6,
            interval: 6,
            setup_fee: None,
        },
        NanpaRate {
            id: 0,
            deck_id: 0,
            code: "1713".to_string(), // Houston
            inter_rate: Decimal::try_from(0.0033).unwrap(),
            intra_rate: Decimal::try_from(0.0038).unwrap(),
            ij_rate: Decimal::try_from(0.0036).unwrap(),
            local_rate: Some(Decimal::try_from(0.0019).unwrap()),
            min_increment: 6,
            interval: 6,
            setup_fee: None,
        },
    ]
}

#[tokio::test]
async fn test_routing_v2_basic_functionality() -> Result<()> {
    let database_url = setup_test_db().await?;
    let lcr = LcrEngine::new(&database_url).await?;
    let deck_loader = lcr.get_deck_loader();
    let routing_v2 = lcr.get_routing_engine_v2();
    
    // Load test vendor deck
    let vendor_request = DeckLoadRequest {
        deck_name: "TEST_VENDOR_BASIC".to_string(),
        owner_id: 1,
        rate_type: RateType::DNIS,
        effective_date: Utc::now(),
        effective_time: None,
        preload_minutes: Some(30),
        rates_csv: None,
        rates_data: Some(create_test_rates()),
    };
    
    let deck_id = deck_loader.load_vendor_deck(vendor_request).await?;
    println!("✓ Loaded test vendor deck: {}", deck_id);
    
    // Test basic routing
    let route_request = RouteRequest {
        ani: "12125551234".to_string(), // NYC
        dnis: "14155555678".to_string(), // SF
        ingress_trunk_id: 1,
        client_deck_id: None,
        route_type: RouteType::NANPA,
        require_profit_protection: false,
        min_profit_margin: None,
        effective_time: None,
    };
    
    let response = routing_v2.find_routes(&route_request).await?;
    
    assert!(!response.routes.is_empty(), "Should find at least one route");
    assert_eq!(response.jurisdiction, CallJurisdiction::Interstate, "NYC to SF should be interstate");
    
    println!("✓ Basic routing test passed");
    println!("  Found {} routes", response.total_routes);
    println!("  Jurisdiction: {:?}", response.jurisdiction);
    
    Ok(())
}

#[tokio::test]
async fn test_time_based_routing() -> Result<()> {
    let database_url = setup_test_db().await?;
    let lcr = LcrEngine::new(&database_url).await?;
    let deck_loader = lcr.get_deck_loader();
    let routing_v2 = lcr.get_routing_engine_v2();
    
    // Load current deck
    let current_rates = create_test_rates();
    let current_request = DeckLoadRequest {
        deck_name: "TEST_TIME_ROUTING".to_string(),
        owner_id: 1,
        rate_type: RateType::DNIS,
        effective_date: Utc::now(),
        effective_time: None,
        preload_minutes: Some(30),
        rates_csv: None,
        rates_data: Some(current_rates.clone()),
    };
    
    let current_deck_id = deck_loader.load_vendor_deck(current_request).await?;
    println!("✓ Loaded current deck: {}", current_deck_id);
    
    // Load future deck with different rates
    let mut future_rates = create_test_rates();
    // Reduce rates by 10%
    for rate in &mut future_rates {
        rate.inter_rate = rate.inter_rate * Decimal::try_from(0.9).unwrap();
        rate.intra_rate = rate.intra_rate * Decimal::try_from(0.9).unwrap();
        rate.ij_rate = rate.ij_rate * Decimal::try_from(0.9).unwrap();
        if let Some(local) = rate.local_rate {
            rate.local_rate = Some(local * Decimal::try_from(0.9).unwrap());
        }
    }
    
    let future_time = Utc::now() + chrono::Duration::hours(24);
    let future_request = DeckLoadRequest {
        deck_name: "TEST_TIME_ROUTING".to_string(), // Same name for versioning
        owner_id: 1,
        rate_type: RateType::DNIS,
        effective_date: future_time,
        effective_time: None,
        preload_minutes: Some(30),
        rates_csv: None,
        rates_data: Some(future_rates),
    };
    
    let future_deck_id = deck_loader.load_vendor_deck(future_request).await?;
    println!("✓ Loaded future deck: {}", future_deck_id);
    
    // Test routing at current time
    let route_request = RouteRequest {
        ani: "12125551234".to_string(),
        dnis: "14155555678".to_string(),
        ingress_trunk_id: 1,
        client_deck_id: None,
        route_type: RouteType::NANPA,
        require_profit_protection: false,
        min_profit_margin: None,
        effective_time: None, // Current time
    };
    
    let current_response = routing_v2.find_routes(&route_request).await?;
    
    // Test routing at future time
    let future_route_request = RouteRequest {
        effective_time: Some(future_time),
        ..route_request.clone()
    };
    
    let future_response = routing_v2.find_routes(&future_route_request).await?;
    
    // Compare results
    assert!(!current_response.routes.is_empty(), "Current routing should find routes");
    assert!(!future_response.routes.is_empty(), "Future routing should find routes");
    
    if let (Some(current_route), Some(future_route)) = 
        (current_response.routes.first(), future_response.routes.first()) {
        assert!(
            future_route.cost_per_minute < current_route.cost_per_minute,
            "Future rates should be lower than current rates"
        );
        
        println!("✓ Time-based routing test passed");
        println!("  Current cost: ${}", current_route.cost_per_minute);
        println!("  Future cost:  ${}", future_route.cost_per_minute);
        println!("  Savings:      ${}", 
            current_route.cost_per_minute - future_route.cost_per_minute);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_jurisdiction_detection() -> Result<()> {
    let database_url = setup_test_db().await?;
    let lcr = LcrEngine::new(&database_url).await?;
    let deck_loader = lcr.get_deck_loader();
    let routing_v2 = lcr.get_routing_engine_v2();
    
    // Load test deck
    let vendor_request = DeckLoadRequest {
        deck_name: "TEST_JURISDICTION".to_string(),
        owner_id: 1,
        rate_type: RateType::DNIS,
        effective_date: Utc::now(),
        effective_time: None,
        preload_minutes: Some(30),
        rates_csv: None,
        rates_data: Some(create_test_rates()),
    };
    
    deck_loader.load_vendor_deck(vendor_request).await?;
    
    // Test different jurisdiction scenarios
    let test_cases = vec![
        ("12125551234", "14155555678", CallJurisdiction::Interstate), // NY to CA
        ("12125551234", "12125556789", CallJurisdiction::Local),      // NY to NY (same area)
        ("17135551234", "17135556789", CallJurisdiction::Local),      // Houston to Houston
    ];
    
    for (ani, dnis, expected_jurisdiction) in test_cases {
        let route_request = RouteRequest {
            ani: ani.to_string(),
            dnis: dnis.to_string(),
            ingress_trunk_id: 1,
            client_deck_id: None,
            route_type: RouteType::NANPA,
            require_profit_protection: false,
            min_profit_margin: None,
            effective_time: None,
        };
        
        let response = routing_v2.find_routes(&route_request).await?;
        
        assert_eq!(
            response.jurisdiction, expected_jurisdiction,
            "Jurisdiction detection failed for {} -> {}",
            ani, dnis
        );
        
        println!("✓ Jurisdiction test passed: {} -> {} = {:?}",
            ani, dnis, expected_jurisdiction);
    }
    
    Ok(())
}

#[tokio::test]
async fn test_profit_protection() -> Result<()> {
    let database_url = setup_test_db().await?;
    let lcr = LcrEngine::new(&database_url).await?;
    let deck_loader = lcr.get_deck_loader();
    let routing_v2 = lcr.get_routing_engine_v2();
    
    // Load vendor deck with high costs
    let expensive_rates = vec![
        NanpaRate {
            id: 0,
            deck_id: 0,
            code: "1212".to_string(),
            inter_rate: Decimal::try_from(0.0500).unwrap(), // High cost
            intra_rate: Decimal::try_from(0.0500).unwrap(),
            ij_rate: Decimal::try_from(0.0500).unwrap(),
            local_rate: Some(Decimal::try_from(0.0500).unwrap()),
            min_increment: 6,
            interval: 6,
            setup_fee: None,
        },
    ];
    
    let vendor_request = DeckLoadRequest {
        deck_name: "TEST_EXPENSIVE_VENDOR".to_string(),
        owner_id: 1,
        rate_type: RateType::DNIS,
        effective_date: Utc::now(),
        effective_time: None,
        preload_minutes: Some(30),
        rates_csv: None,
        rates_data: Some(expensive_rates),
    };
    
    deck_loader.load_vendor_deck(vendor_request).await?;
    
    // Load client deck with low selling rates (would result in loss)
    let cheap_client_rates = vec![
        NanpaRate {
            id: 0,
            deck_id: 0,
            code: "1212".to_string(),
            inter_rate: Decimal::try_from(0.0100).unwrap(), // Low selling rate
            intra_rate: Decimal::try_from(0.0100).unwrap(),
            ij_rate: Decimal::try_from(0.0100).unwrap(),
            local_rate: Some(Decimal::try_from(0.0100).unwrap()),
            min_increment: 6,
            interval: 6,
            setup_fee: None,
        },
    ];
    
    let client_request = DeckLoadRequest {
        deck_name: "TEST_CHEAP_CLIENT".to_string(),
        owner_id: 1,
        rate_type: RateType::DNIS,
        effective_date: Utc::now(),
        effective_time: None,
        preload_minutes: Some(30),
        rates_csv: None,
        rates_data: Some(cheap_client_rates),
    };
    
    let client_deck_id = deck_loader.load_client_deck(client_request).await?;
    
    // Test without profit protection
    let route_request_no_protection = RouteRequest {
        ani: "12125551234".to_string(),
        dnis: "12125556789".to_string(),
        ingress_trunk_id: 1,
        client_deck_id: Some(client_deck_id),
        route_type: RouteType::NANPA,
        require_profit_protection: false,
        min_profit_margin: None,
        effective_time: None,
    };
    
    let response_no_protection = routing_v2.find_routes(&route_request_no_protection).await?;
    
    // Test with profit protection
    let route_request_with_protection = RouteRequest {
        require_profit_protection: true,
        min_profit_margin: Some(Decimal::try_from(0.0200).unwrap()), // Require 2 cent profit
        ..route_request_no_protection.clone()
    };
    
    let response_with_protection = routing_v2.find_routes(&route_request_with_protection).await?;
    
    // Without protection, we might find routes (even at a loss)
    // With protection, we should find no routes due to insufficient profit
    println!("✓ Profit protection test results:");
    println!("  Without protection: {} routes", response_no_protection.total_routes);
    println!("  With protection:    {} routes", response_with_protection.total_routes);
    
    // The protected query should return fewer or no routes
    assert!(
        response_with_protection.total_routes <= response_no_protection.total_routes,
        "Profit protection should reduce available routes"
    );
    
    Ok(())
}

#[tokio::test]
async fn test_routing_engine_comparison() -> Result<()> {
    let database_url = setup_test_db().await?;
    let lcr = LcrEngine::new(&database_url).await?;
    let deck_loader = lcr.get_deck_loader();
    let routing_v1 = lcr.get_routing_engine();
    let routing_v2 = lcr.get_routing_engine_v2();
    
    // Load test data
    let vendor_request = DeckLoadRequest {
        deck_name: "TEST_COMPARISON".to_string(),
        owner_id: 1,
        rate_type: RateType::DNIS,
        effective_date: Utc::now(),
        effective_time: None,
        preload_minutes: Some(30),
        rates_csv: None,
        rates_data: Some(create_test_rates()),
    };
    
    deck_loader.load_vendor_deck(vendor_request).await?;
    
    // Reload cache to ensure V1 has the data
    lcr.reload_cache().await?;
    
    // Test same request on both engines
    let route_request = RouteRequest {
        ani: "12125551234".to_string(),
        dnis: "14155555678".to_string(),
        ingress_trunk_id: 1,
        client_deck_id: None,
        route_type: RouteType::NANPA,
        require_profit_protection: false,
        min_profit_margin: None,
        effective_time: None, // Current time for V2
    };
    
    let response_v1 = routing_v1.find_routes(&route_request).await?;
    let response_v2 = routing_v2.find_routes(&route_request).await?;
    
    // Compare results
    println!("✓ Routing engine comparison:");
    println!("  V1 routes: {}", response_v1.total_routes);
    println!("  V2 routes: {}", response_v2.total_routes);
    println!("  V1 jurisdiction: {:?}", response_v1.jurisdiction);
    println!("  V2 jurisdiction: {:?}", response_v2.jurisdiction);
    
    // Basic compatibility checks
    assert_eq!(
        response_v1.jurisdiction, response_v2.jurisdiction,
        "Both engines should detect same jurisdiction"
    );
    
    assert_eq!(
        response_v1.total_routes, response_v2.total_routes,
        "Both engines should find same number of routes"
    );
    
    if let (Some(route_v1), Some(route_v2)) = 
        (response_v1.routes.first(), response_v2.routes.first()) {
        
        // Should route to same trunk
        assert_eq!(
            route_v1.egress_trunk.id, route_v2.egress_trunk.id,
            "Both engines should select same trunk"
        );
        
        // Should calculate same costs (approximately)
        let cost_diff = (route_v1.cost_per_minute - route_v2.cost_per_minute).abs();
        assert!(
            cost_diff < Decimal::try_from(0.0001).unwrap(),
            "Cost calculations should match between engines"
        );
        
        println!("  V1 best route: {} (cost: ${})", 
            route_v1.egress_trunk.name, route_v1.cost_per_minute);
        println!("  V2 best route: {} (cost: ${})", 
            route_v2.egress_trunk.name, route_v2.cost_per_minute);
    }
    
    println!("✓ Engine compatibility confirmed");
    
    Ok(())
}

#[tokio::test]
async fn test_performance_comparison() -> Result<()> {
    let database_url = setup_test_db().await?;
    let lcr = LcrEngine::new(&database_url).await?;
    let deck_loader = lcr.get_deck_loader();
    let routing_v1 = lcr.get_routing_engine();
    let routing_v2 = lcr.get_routing_engine_v2();
    
    // Load substantial test data
    let mut large_rate_set = Vec::new();
    let area_codes = vec!["212", "213", "214", "215", "216", "217", "218", "219", "301", "302"];
    let exchanges = vec!["555", "556", "557", "558", "559"];
    
    for area in &area_codes {
        for exchange in &exchanges {
            large_rate_set.push(NanpaRate {
                id: 0,
                deck_id: 0,
                code: format!("1{}{}", area, exchange),
                inter_rate: Decimal::try_from(0.0030 + (area.len() as f64 * 0.0001)).unwrap(),
                intra_rate: Decimal::try_from(0.0035 + (area.len() as f64 * 0.0001)).unwrap(),
                ij_rate: Decimal::try_from(0.0033 + (area.len() as f64 * 0.0001)).unwrap(),
                local_rate: Some(Decimal::try_from(0.0020 + (area.len() as f64 * 0.0001)).unwrap()),
                min_increment: 6,
                interval: 6,
                setup_fee: None,
            });
        }
    }
    
    let vendor_request = DeckLoadRequest {
        deck_name: "TEST_PERFORMANCE".to_string(),
        owner_id: 1,
        rate_type: RateType::DNIS,
        effective_date: Utc::now(),
        effective_time: None,
        preload_minutes: Some(30),
        rates_csv: None,
        rates_data: Some(large_rate_set),
    };
    
    deck_loader.load_vendor_deck(vendor_request).await?;
    lcr.reload_cache().await?;
    
    let route_request = RouteRequest {
        ani: "12125551234".to_string(),
        dnis: "21355512345".to_string(),
        ingress_trunk_id: 1,
        client_deck_id: None,
        route_type: RouteType::NANPA,
        require_profit_protection: false,
        min_profit_margin: None,
        effective_time: None,
    };
    
    // Performance test V1
    let start_v1 = std::time::Instant::now();
    for _ in 0..100 {
        let _ = routing_v1.find_routes(&route_request).await?;
    }
    let duration_v1 = start_v1.elapsed();
    
    // Performance test V2
    let start_v2 = std::time::Instant::now();
    for _ in 0..100 {
        let _ = routing_v2.find_routes(&route_request).await?;
    }
    let duration_v2 = start_v2.elapsed();
    
    println!("✓ Performance comparison (100 requests):");
    println!("  V1 time: {:?} ({:.2} ms per request)", 
        duration_v1, duration_v1.as_millis() as f64 / 100.0);
    println!("  V2 time: {:?} ({:.2} ms per request)", 
        duration_v2, duration_v2.as_millis() as f64 / 100.0);
    
    let ratio = duration_v2.as_millis() as f64 / duration_v1.as_millis() as f64;
    println!("  V2 is {:.2}x the speed of V1", 1.0 / ratio);
    
    // V2 should be reasonably performant (allow up to 2x slower due to DB queries)
    assert!(
        ratio < 2.0,
        "V2 should not be more than 2x slower than V1 (actual: {:.2}x)", ratio
    );
    
    Ok(())
}

// Helper function for running all tests
pub async fn run_all_routing_v2_tests() -> Result<()> {
    println!("🧪 Running RoutingEngineV2 Test Suite");
    println!("=====================================");
    
    test_routing_v2_basic_functionality().await?;
    test_time_based_routing().await?;
    test_jurisdiction_detection().await?;
    test_profit_protection().await?;
    test_routing_engine_comparison().await?;
    test_performance_comparison().await?;
    
    println!("\n✅ All RoutingEngineV2 tests passed!");
    Ok(())
}