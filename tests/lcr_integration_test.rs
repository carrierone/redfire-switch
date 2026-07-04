#[cfg(test)]
mod lcr_tests {
    use redfire_switch::lcr::{routing::RouteRequest, types::*, LcrEngine};
    use rust_decimal::Decimal;
    use sqlx::PgPool;
    use std::str::FromStr;
    use std::sync::Arc;

    /// Cross-suite serialization lock for DB-mutating LCR tests. Must use the same
    /// key as the other LCR suites (see routing_engine_v2_tests). Tests that assert
    /// on global topology state (e.g. trunk counts) hold this for their duration so
    /// concurrent suites can't mutate trunks out from under them.
    const LCR_TEST_LOCK_KEY: i64 = 0x5245_4446_4952_4C43; // "REDFIRLC"

    struct LcrTestLock {
        pool: PgPool,
    }

    impl LcrTestLock {
        async fn acquire(database_url: &str) -> LcrTestLock {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(1)
                .connect(database_url)
                .await
                .expect("connect for test lock");
            sqlx::query("SELECT pg_advisory_lock($1)")
                .bind(LCR_TEST_LOCK_KEY)
                .execute(&pool)
                .await
                .expect("acquire test advisory lock");
            LcrTestLock { pool }
        }
    }

    impl Drop for LcrTestLock {
        fn drop(&mut self) {
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

    fn test_database_url() -> String {
        std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://redfire:password@localhost/redfire_switch".to_string())
    }

    async fn setup_test_engine() -> Arc<LcrEngine> {
        // Use test database or in-memory cache
        let database_url = test_database_url();

        // Ensure the full schema (core + LCR) is present before the engine loads.
        redfire_switch::database::DatabaseService::provision_schema(&database_url)
            .await
            .expect("Failed to provision test schema");

        // Seed the shared LCR fixture data (trunks, decks, NANPA static, static
        // routes) these tests route against. Idempotent + advisory-locked.
        redfire_switch::database::DatabaseService::seed_lcr_sample_data(&database_url)
            .await
            .expect("Failed to seed LCR sample data");

        Arc::new(
            LcrEngine::new(&database_url)
                .await
                .expect("Failed to create LCR engine"),
        )
    }

    #[tokio::test]
    async fn test_call_simulation() {
        let engine = setup_test_engine().await;
        let routing = engine.get_routing_engine();

        // Test NYC to LA call
        let simulation = routing
            .simulate_call(
                "12125551234", // NYC ANI
                "12135555678", // LA DNIS
                None,
            )
            .await
            .expect("Simulation failed");

        assert!(!simulation.routes.is_empty(), "Should find routes");
        assert_eq!(simulation.jurisdiction, CallJurisdiction::Interstate);

        // Verify least cost ordering
        let mut prev_cost = Decimal::ZERO;
        for route in &simulation.routes {
            assert!(
                route.cost_per_minute >= prev_cost,
                "Routes should be ordered by cost"
            );
            prev_cost = route.cost_per_minute;
        }
    }

    #[tokio::test]
    async fn test_profit_protection() {
        let engine = setup_test_engine().await;
        let routing = engine.get_routing_engine();

        let request = RouteRequest {
            ani: "12125551234".to_string(),
            dnis: "14155555678".to_string(),
            ingress_trunk_id: 1, // Client-X-Retail with profit protection
            client_deck_id: Some(1),
            route_type: RouteType::NANPA,
            require_profit_protection: true,
            min_profit_margin: Some(Decimal::from_str("0.0050").unwrap()),
            effective_time: None,
            phone_validation: None,
            routing_plan_id: None,
        };

        let response = routing
            .find_routes(&request)
            .await
            .expect("Route finding failed");

        // All routes should meet minimum profit margin
        for route in &response.routes {
            assert!(
                route.profit_margin >= Decimal::from_str("0.0050").unwrap(),
                "Route profit margin {} should meet minimum",
                route.profit_margin
            );
        }
    }

    #[tokio::test]
    async fn test_jurisdiction_determination() {
        let engine = setup_test_engine().await;
        let routing = engine.get_routing_engine();

        // Intrastate call (both in CA)
        let sim1 = routing
            .simulate_call(
                "14155551234", // SF ANI
                "12135555678", // LA DNIS
                None,
            )
            .await
            .expect("Simulation failed");

        assert_eq!(sim1.jurisdiction, CallJurisdiction::Intrastate);

        // Interstate call (NY to CA)
        let sim2 = routing
            .simulate_call(
                "12125551234", // NYC ANI
                "14155555678", // SF DNIS
                None,
            )
            .await
            .expect("Simulation failed");

        assert_eq!(sim2.jurisdiction, CallJurisdiction::Interstate);
    }

    #[tokio::test]
    async fn test_trunk_capacity_management() {
        let engine = setup_test_engine().await;
        let trunk_mgr = engine.get_trunk_manager();

        // Simulate adding calls to a trunk
        let trunk_id = 1;
        let capacity_limit = 100;
        // Use a high CPS limit so this test isolates *capacity* behaviour; a
        // 100-call burst would otherwise legitimately trip a low CPS limit.
        let cps_limit = Decimal::from(10_000);

        // Add calls up to capacity
        for _ in 0..capacity_limit {
            trunk_mgr
                .increment_call(trunk_id, TrunkType::Egress, capacity_limit, cps_limit)
                .await
                .expect("Failed to increment call");
        }

        // Should not accept more calls
        assert!(
            !trunk_mgr.can_accept_call(trunk_id, TrunkType::Egress).await,
            "Trunk should reject calls when at capacity"
        );

        // Remove a call
        trunk_mgr
            .decrement_call(trunk_id, TrunkType::Egress, 60)
            .await
            .expect("Failed to decrement");

        // Should accept calls again
        assert!(
            trunk_mgr.can_accept_call(trunk_id, TrunkType::Egress).await,
            "Trunk should accept calls after decrement"
        );
    }

    #[tokio::test]
    async fn test_route_advance() {
        let engine = setup_test_engine().await;
        let routing = engine.get_routing_engine();

        // Find routes first
        let request = RouteRequest {
            ani: "12125551234".to_string(),
            dnis: "14155555678".to_string(),
            ingress_trunk_id: 1,
            client_deck_id: Some(1),
            route_type: RouteType::NANPA,
            require_profit_protection: false,
            min_profit_margin: None,
            effective_time: None,
            phone_validation: None,
            routing_plan_id: None,
        };

        let response = routing
            .find_routes(&request)
            .await
            .expect("Route finding failed");
        assert!(
            response.routes.len() > 1,
            "Need multiple routes to test advancement"
        );

        // TODO: implement handle_route_advance method
        // Test advancing on 503 Service Unavailable
        // let next_route = routing
        //     .handle_route_advance(SipResponseCode::ServiceUnavailable, 1, 0, &response.routes)
        //     .await;
        //
        // assert_eq!(next_route, Some(1), "Should advance to next route on 503");
        //
        // // Test stopping on 404 Not Found
        // let stop_route = routing
        //     .handle_route_advance(SipResponseCode::NotFound, 1, 0, &response.routes)
        //     .await;
        //
        // assert_eq!(stop_route, None, "Should stop routing on 404");
    }

    #[tokio::test]
    async fn test_cache_reload() {
        // Asserts the trunk count is stable across a reload, so it must not run
        // while another suite is adding/removing trunks. Serialize on the shared
        // LCR test lock.
        let _lock = LcrTestLock::acquire(&test_database_url()).await;
        let engine = setup_test_engine().await;

        // Initial cache should be loaded
        let trunk_count_before = engine.cache.get_all_egress_trunks().len();
        assert!(trunk_count_before > 0, "Should have trunks loaded");

        // Reload cache
        engine.reload_cache().await.expect("Cache reload failed");

        // Should still have trunks
        let trunk_count_after = engine.cache.get_all_egress_trunks().len();
        assert_eq!(
            trunk_count_before, trunk_count_after,
            "Trunk count should be consistent"
        );
    }

    #[tokio::test]
    async fn test_static_routes() {
        let engine = setup_test_engine().await;
        let routing = engine.get_routing_engine();

        // Test 911 emergency call (should use static route)
        let sim = routing
            .simulate_call("12125551234", "1911", None)
            .await
            .expect("Simulation failed");

        assert_eq!(sim.total_routes, 1, "911 should have exactly one route");
        // Static routes would be handled specially
    }

    #[tokio::test]
    async fn test_billing_increments() {
        let engine = setup_test_engine().await;

        // Get a vendor rate
        let rate = engine
            .cache
            .get_vendor_rate(1, "1212")
            .expect("Should find rate");

        assert_eq!(rate.min_increment, 6, "Should have 6 second minimum");
        assert_eq!(rate.interval, 6, "Should have 6 second billing interval");

        // Calculate billing for various durations
        let test_cases = vec![
            (1, 6),   // 1 second -> 6 seconds billed
            (6, 6),   // 6 seconds -> 6 seconds billed
            (7, 12),  // 7 seconds -> 12 seconds billed
            (13, 18), // 13 seconds -> 18 seconds billed
        ];

        for (actual, expected) in test_cases {
            let billed = calculate_billed_duration(actual, rate.min_increment, rate.interval);
            assert_eq!(
                billed, expected,
                "Duration {} should bill as {}",
                actual, expected
            );
        }
    }

    fn calculate_billed_duration(actual_seconds: i32, min_increment: i32, interval: i32) -> i32 {
        if actual_seconds <= min_increment {
            min_increment
        } else {
            let excess = actual_seconds - min_increment;
            let intervals = (excess + interval - 1) / interval;
            min_increment + (intervals * interval)
        }
    }
}
