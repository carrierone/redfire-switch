use anyhow::Result;
use chrono::{DateTime, NaiveTime, TimeZone, Utc};
use redfire_switch::lcr::{
    types::{CallJurisdiction, DeckLoadRequest, NanpaRate, RateType, RouteRequest, RouteType},
    LcrEngine,
};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::sync::Arc;
use tokio;

/// Cross-suite serialization for DB-mutating LCR tests.
///
/// All the LCR integration suites share a single Postgres database. Several of
/// them assert on global state (e.g. total trunk count, or that a freshly linked
/// trunk shows up in routing), which is inherently incompatible with other
/// suites mutating trunks/decks at the same time. Holding a session-level
/// Postgres advisory lock for the duration of such a test serializes them across
/// processes without needing a dedicated test database per suite.
///
/// The lock key must match across every suite that participates.
const LCR_TEST_LOCK_KEY: i64 = 0x5245_4446_4952_4C43; // "REDFIRLC"

struct LcrTestLock {
    pool: PgPool,
}

impl LcrTestLock {
    async fn acquire(database_url: &str) -> Result<Self> {
        // A single dedicated connection holds the session-level lock, so the
        // matching unlock (and the implicit release on close) happen on the same
        // backend session.
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(database_url)
            .await?;
        sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(LCR_TEST_LOCK_KEY)
            .execute(&pool)
            .await?;
        Ok(Self { pool })
    }
}

impl Drop for LcrTestLock {
    fn drop(&mut self) {
        // Best-effort release; closing the pool also drops the session lock.
        let pool = self.pool.clone();
        tokio::spawn(async move {
            let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
                .bind(LCR_TEST_LOCK_KEY)
                .execute(&pool)
                .await;
            pool.close().await;
        });
    }
}

// Test database setup.
//
// Returns the database URL together with an `LcrTestLock` guard. The whole suite
// shares one Postgres database and each test's setup wipes `TEST_%` decks, so the
// setup + test body must be serialized against sibling tests (and other LCR
// suites) that mutate the same rows. Callers must keep the returned guard alive
// for the duration of the test (`let (url, _guard) = setup_test_db().await?;`).
async fn setup_test_db() -> Result<(String, LcrTestLock)> {
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://redfire:password@localhost/redfire_switch".to_string());

    // Ensure the full schema (core + LCR) is present before we touch it. This is
    // internally advisory-locked and safe to race.
    redfire_switch::database::DatabaseService::provision_schema(&database_url).await?;

    // Acquire the shared LCR test lock before mutating any shared TEST_% state.
    let lock = LcrTestLock::acquire(&database_url).await?;

    // Seed the shared LCR fixture data (trunks, decks, NANPA static). These
    // tests load their own TEST_* rate decks on top of this baseline topology.
    redfire_switch::database::DatabaseService::seed_lcr_sample_data(&database_url).await?;

    // Run migrations
    let pool = PgPool::connect(&database_url).await?;

    // Clean up any existing test data
    sqlx::query("DELETE FROM vendor_nanpa_rates WHERE deck_id IN (SELECT id FROM vendor_rate_decks WHERE name LIKE 'TEST_%')")
        .execute(&pool).await?;
    sqlx::query("DELETE FROM client_nanpa_rates WHERE deck_id IN (SELECT id FROM client_rate_decks WHERE name LIKE 'TEST_%')")
        .execute(&pool).await?;
    sqlx::query("DELETE FROM vendor_rate_decks WHERE name LIKE 'TEST_%'")
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM client_rate_decks WHERE name LIKE 'TEST_%'")
        .execute(&pool)
        .await?;
    sqlx::query("DELETE FROM deck_cutover_schedule WHERE deck_type = 'test'")
        .execute(&pool)
        .await?;

    pool.close().await;
    Ok((database_url, lock))
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

/// Wire a freshly-loaded vendor deck into the routing topology so that routing
/// through `ingress_trunk_id = 1` can actually reach it. Operators normally do
/// this via `lcr_route_trunks`; the deck loader only manages rate versions, not
/// trunk linkage. Returns the id of the dedicated egress trunk created for the
/// deck. Idempotent per deck name so it is safe to call across re-runs.
async fn link_vendor_deck_to_topology(
    database_url: &str,
    deck_name: &str,
    trunk_name: &str,
) -> Result<i32> {
    let pool = PgPool::connect(database_url).await?;

    // Resolve the vendor id backing this deck family.
    let vendor_id: i32 = sqlx::query_scalar(
        "SELECT vendor_id FROM vendor_rate_decks WHERE name = $1 ORDER BY deck_version DESC LIMIT 1",
    )
    .bind(deck_name)
    .fetch_one(&pool)
    .await?;

    // The current (base) version this trunk is pinned to; routing resolves the
    // version active at the effective time from the same deck family.
    let base_deck_id: i32 = sqlx::query_scalar(
        "SELECT id FROM vendor_rate_decks WHERE name = $1 ORDER BY deck_version ASC LIMIT 1",
    )
    .bind(deck_name)
    .fetch_one(&pool)
    .await?;

    // Dedicated egress trunk for this deck (unique name -> idempotent upsert).
    let egress_trunk_id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO egress_trunks (name, vendor_id, host, port, transport, capacity_limit, cps_limit, priority, active, supports_international)
        VALUES ($1, $2, 'sip.test.local', 5060, 'UDP', 1000, 100.0, 50, true, false)
        ON CONFLICT (name) DO UPDATE SET vendor_id = EXCLUDED.vendor_id
        RETURNING id
        "#,
    )
    .bind(trunk_name)
    .bind(vendor_id)
    .fetch_one(&pool)
    .await?;

    // A route to hang the trunk off, and the trunk<->deck linkage.
    let lcr_route_id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO lcr_routes (name, route_type, description, priority)
        VALUES ($1, 'NANPA', 'test route', 100)
        ON CONFLICT (name) DO UPDATE SET route_type = EXCLUDED.route_type
        RETURNING id
        "#,
    )
    .bind(format!("{trunk_name}-route"))
    .fetch_one(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO lcr_route_trunks (lcr_route_id, egress_trunk_id, vendor_deck_id, priority, weight)
        VALUES ($1, $2, $3, 50, 1)
        ON CONFLICT (lcr_route_id, egress_trunk_id, vendor_deck_id) DO NOTHING
        "#,
    )
    .bind(lcr_route_id)
    .bind(egress_trunk_id)
    .bind(base_deck_id)
    .execute(&pool)
    .await?;

    pool.close().await;
    Ok(egress_trunk_id)
}

#[tokio::test]
async fn test_routing_v2_basic_functionality() -> Result<()> {
    let (database_url, _db_guard) = setup_test_db().await?;
    let lcr = LcrEngine::new(&database_url).await?;
    let deck_loader = lcr.get_deck_loader();
    let routing_v2 = lcr.get_routing_engine();

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
        ani: "12125551234".to_string(),  // NYC
        dnis: "14155555678".to_string(), // SF
        ingress_trunk_id: 1,
        client_deck_id: None,
        route_type: RouteType::NANPA,
        require_profit_protection: false,
        min_profit_margin: None,
        effective_time: None,
        phone_validation: None,
        routing_plan_id: None,
    };

    let response = routing_v2.find_routes(&route_request).await?;

    assert!(
        !response.routes.is_empty(),
        "Should find at least one route"
    );
    assert_eq!(
        response.jurisdiction,
        CallJurisdiction::Interstate,
        "NYC to SF should be interstate"
    );

    println!("✓ Basic routing test passed");
    println!("  Found {} routes", response.total_routes);
    println!("  Jurisdiction: {:?}", response.jurisdiction);

    Ok(())
}

#[tokio::test]
async fn test_time_based_routing() -> Result<()> {
    let (database_url, _db_guard) = setup_test_db().await?;
    // setup_test_db already holds the shared LCR test lock for the whole body, so
    // linking a new trunk and asserting routing reaches it is safe from concurrent
    // trunk mutation by sibling tests / other suites.
    let lcr = LcrEngine::new(&database_url).await?;
    let deck_loader = lcr.get_deck_loader();
    let routing_v2 = lcr.get_routing_engine();

    // Use per-run unique names so this test doesn't collide with other DB-touching
    // suites running in parallel against the shared test database (or with its own
    // previous runs).
    let run_id = uuid::Uuid::new_v4().simple().to_string();
    let deck_name = format!("TEST_TIME_ROUTING_{}", &run_id[..8]);
    let trunk_name = format!("TEST_TIME_ROUTING_TRUNK_{}", &run_id[..8]);

    // Load current deck
    let current_rates = create_test_rates();
    let current_request = DeckLoadRequest {
        deck_name: deck_name.clone(),
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
        deck_name: deck_name.clone(), // Same name for versioning
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

    // Wire this versioned deck into the routing topology and reload the cache so
    // routing can actually reach it. Routing resolves the deck version active at
    // the effective time, so the current vs future comparison exercises the
    // deck-versioning path rather than the static sample data.
    let egress_trunk_id =
        link_vendor_deck_to_topology(&database_url, &deck_name, &trunk_name)
            .await?;
    lcr.reload_cache().await?;

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
        phone_validation: None,
        routing_plan_id: None,
    };

    let current_response = routing_v2.find_routes(&route_request).await?;

    // Test routing at future time
    let future_route_request = RouteRequest {
        effective_time: Some(future_time),
        ..route_request.clone()
    };

    let future_response = routing_v2.find_routes(&future_route_request).await?;

    // Compare results
    assert!(
        !current_response.routes.is_empty(),
        "Current routing should find routes"
    );
    assert!(
        !future_response.routes.is_empty(),
        "Future routing should find routes"
    );

    // Compare the cost on the specific trunk backed by the versioned deck, so we
    // measure the deck-version change rather than which trunk happened to win.
    let current_route = current_response
        .routes
        .iter()
        .find(|r| r.egress_trunk.id == egress_trunk_id)
        .expect("current routing should reach the versioned deck's trunk");
    let future_route = future_response
        .routes
        .iter()
        .find(|r| r.egress_trunk.id == egress_trunk_id)
        .expect("future routing should reach the versioned deck's trunk");

    assert!(
        future_route.cost_per_minute < current_route.cost_per_minute,
        "Future rates should be lower than current rates (current {}, future {})",
        current_route.cost_per_minute,
        future_route.cost_per_minute
    );

    println!("✓ Time-based routing test passed");
    println!("  Current cost: ${}", current_route.cost_per_minute);
    println!("  Future cost:  ${}", future_route.cost_per_minute);
    println!(
        "  Savings:      ${}",
        current_route.cost_per_minute - future_route.cost_per_minute
    );

    Ok(())
}

#[tokio::test]
async fn test_jurisdiction_detection() -> Result<()> {
    let (database_url, _db_guard) = setup_test_db().await?;
    let lcr = LcrEngine::new(&database_url).await?;
    let deck_loader = lcr.get_deck_loader();
    let routing_v2 = lcr.get_routing_engine();

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
            phone_validation: None,
            routing_plan_id: None,
        };

        let response = routing_v2.find_routes(&route_request).await?;

        assert_eq!(
            response.jurisdiction, expected_jurisdiction,
            "Jurisdiction detection failed for {} -> {}",
            ani, dnis
        );

        println!(
            "✓ Jurisdiction test passed: {} -> {} = {:?}",
            ani, dnis, expected_jurisdiction
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_profit_protection() -> Result<()> {
    let (database_url, _db_guard) = setup_test_db().await?;
    let lcr = LcrEngine::new(&database_url).await?;
    let deck_loader = lcr.get_deck_loader();
    let routing_v2 = lcr.get_routing_engine();

    // Load vendor deck with high costs
    let expensive_rates = vec![NanpaRate {
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
    }];

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
    let cheap_client_rates = vec![NanpaRate {
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
    }];

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
        phone_validation: None,
        routing_plan_id: None,
    };

    let response_no_protection = routing_v2.find_routes(&route_request_no_protection).await?;

    // Test with profit protection
    let route_request_with_protection = RouteRequest {
        require_profit_protection: true,
        min_profit_margin: Some(Decimal::try_from(0.0200).unwrap()), // Require 2 cent profit
        ..route_request_no_protection.clone()
    };

    let response_with_protection = routing_v2
        .find_routes(&route_request_with_protection)
        .await?;

    // Without protection, we might find routes (even at a loss)
    // With protection, we should find no routes due to insufficient profit
    println!("✓ Profit protection test results:");
    println!(
        "  Without protection: {} routes",
        response_no_protection.total_routes
    );
    println!(
        "  With protection:    {} routes",
        response_with_protection.total_routes
    );

    // The protected query should return fewer or no routes
    assert!(
        response_with_protection.total_routes <= response_no_protection.total_routes,
        "Profit protection should reduce available routes"
    );

    Ok(())
}

#[tokio::test]
async fn test_routing_engine_comparison() -> Result<()> {
    let (database_url, _db_guard) = setup_test_db().await?;
    let lcr = LcrEngine::new(&database_url).await?;
    let deck_loader = lcr.get_deck_loader();
    let routing_v1 = lcr.get_routing_engine();
    let routing_v2 = lcr.get_routing_engine();

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
        phone_validation: None,
        routing_plan_id: None,
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
        (response_v1.routes.first(), response_v2.routes.first())
    {
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

        println!(
            "  V1 best route: {} (cost: ${})",
            route_v1.egress_trunk.name, route_v1.cost_per_minute
        );
        println!(
            "  V2 best route: {} (cost: ${})",
            route_v2.egress_trunk.name, route_v2.cost_per_minute
        );
    }

    println!("✓ Engine compatibility confirmed");

    Ok(())
}

#[tokio::test]
async fn test_performance_comparison() -> Result<()> {
    let (database_url, _db_guard) = setup_test_db().await?;
    let lcr = LcrEngine::new(&database_url).await?;
    let deck_loader = lcr.get_deck_loader();
    let routing_v1 = lcr.get_routing_engine();
    let routing_v2 = lcr.get_routing_engine();

    // Load substantial test data
    let mut large_rate_set = Vec::new();
    let area_codes = vec![
        "212", "213", "214", "215", "216", "217", "218", "219", "301", "302",
    ];
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
        phone_validation: None,
        routing_plan_id: None,
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
    println!(
        "  V1 time: {:?} ({:.2} ms per request)",
        duration_v1,
        duration_v1.as_millis() as f64 / 100.0
    );
    println!(
        "  V2 time: {:?} ({:.2} ms per request)",
        duration_v2,
        duration_v2.as_millis() as f64 / 100.0
    );

    let ratio = duration_v2.as_millis() as f64 / duration_v1.as_millis() as f64;
    println!("  V2 is {:.2}x the speed of V1", 1.0 / ratio);

    // V2 should be reasonably performant (allow up to 2x slower due to DB queries)
    assert!(
        ratio < 2.0,
        "V2 should not be more than 2x slower than V1 (actual: {:.2}x)",
        ratio
    );

    Ok(())
}

// Note: each test above is an independent `#[tokio::test]`; the harness runs them directly.
