use anyhow::Result;
use chrono::{DateTime, NaiveTime, TimeZone, Utc};
use redfire_switch::lcr::types::{DeckLoadRequest, NanpaRate, RateType, RouteRequest, RouteType};
use redfire_switch::lcr::LcrEngine;
use rust_decimal::Decimal;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the LCR engine
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://lcr_user:lcr_pass@localhost:5432/lcr_db".to_string());

    let lcr = LcrEngine::new(&database_url).await?;
    let deck_loader = lcr.get_deck_loader();
    let routing = lcr.get_routing_engine();

    println!("🚀 LCR Deck Versioning Demo");
    println!("============================");

    // Demo 1: Load a current deck
    println!("\n1. Loading current vendor deck...");
    let current_rates = vec![
        NanpaRate {
            id: 0,
            deck_id: 0,
            code: "1212".to_string(),
            inter_rate: Decimal::try_from(0.0035)?,
            intra_rate: Decimal::try_from(0.0040)?,
            ij_rate: Decimal::try_from(0.0038)?,
            local_rate: Some(Decimal::try_from(0.0020)?),
            min_increment: 6,
            interval: 6,
            setup_fee: None,
        },
        NanpaRate {
            id: 0,
            deck_id: 0,
            code: "1415".to_string(),
            inter_rate: Decimal::try_from(0.0032)?,
            intra_rate: Decimal::try_from(0.0037)?,
            ij_rate: Decimal::try_from(0.0035)?,
            local_rate: Some(Decimal::try_from(0.0018)?),
            min_increment: 6,
            interval: 6,
            setup_fee: None,
        },
    ];

    let current_request = DeckLoadRequest {
        deck_name: "Demo-Vendor-Current".to_string(),
        owner_id: 1,
        rate_type: RateType::DNIS,
        effective_date: Utc::now(),
        effective_time: Some(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
        preload_minutes: Some(30),
        rates_csv: None,
        rates_data: Some(current_rates),
    };

    let current_deck_id = deck_loader.load_vendor_deck(current_request).await?;
    println!("✓ Loaded current deck with ID: {}", current_deck_id);

    // Demo 2: Load a future deck (will auto-version and set end dates)
    println!("\n2. Loading future vendor deck...");
    let future_rates = vec![
        NanpaRate {
            id: 0,
            deck_id: 0,
            code: "1212".to_string(),
            inter_rate: Decimal::try_from(0.0030)?, // Reduced rates
            intra_rate: Decimal::try_from(0.0035)?,
            ij_rate: Decimal::try_from(0.0033)?,
            local_rate: Some(Decimal::try_from(0.0018)?),
            min_increment: 6,
            interval: 6,
            setup_fee: None,
        },
        NanpaRate {
            id: 0,
            deck_id: 0,
            code: "1415".to_string(),
            inter_rate: Decimal::try_from(0.0028)?, // Reduced rates
            intra_rate: Decimal::try_from(0.0033)?,
            ij_rate: Decimal::try_from(0.0031)?,
            local_rate: Some(Decimal::try_from(0.0016)?),
            min_increment: 6,
            interval: 6,
            setup_fee: None,
        },
    ];

    // Future effective date (tomorrow at midnight)
    let tomorrow = Utc::now() + chrono::Duration::days(1);
    let date = tomorrow.date_naive();
    let future_effective = date.and_hms_opt(0, 0, 0).unwrap().and_utc();

    let future_request = DeckLoadRequest {
        deck_name: "Demo-Vendor-Current".to_string(), // Same name = auto-versioning
        owner_id: 1,
        rate_type: RateType::DNIS,
        effective_date: future_effective,
        effective_time: Some(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
        preload_minutes: Some(30),
        rates_csv: None,
        rates_data: Some(future_rates),
    };

    let future_deck_id = deck_loader.load_vendor_deck(future_request).await?;
    println!("✓ Loaded future deck with ID: {}", future_deck_id);
    println!("  Current deck end date automatically set");
    println!("  Future deck scheduled for: {}", future_effective);

    // Demo 3: Test routing at current time
    println!("\n3. Testing routing with current rates...");
    let route_request = RouteRequest {
        ani: "12125551234".to_string(),
        dnis: "14155555678".to_string(),
        ingress_trunk_id: 1,
        client_deck_id: None,
        route_type: RouteType::NANPA,
        require_profit_protection: false,
        min_profit_margin: None,
        effective_time: None, // Use current time
        phone_validation: None,
        routing_plan_id: None,
    };

    match routing.find_routes(&route_request).await {
        Ok(response) => {
            println!(
                "✓ Found {} routes using current rates",
                response.total_routes
            );
            println!("  Jurisdiction: {:?}", response.jurisdiction);
            if let Some(route) = response.routes.first() {
                println!(
                    "  Best route: {} (cost: ${}/min)",
                    route.egress_trunk.name, route.cost_per_minute
                );
            }
        }
        Err(e) => println!("⚠ Routing failed: {}", e),
    }

    // Demo 4: Test routing at future time
    println!("\n4. Testing routing with future rates...");
    let future_route_request = RouteRequest {
        ani: "12125551234".to_string(),
        dnis: "14155555678".to_string(),
        ingress_trunk_id: 1,
        client_deck_id: None,
        route_type: RouteType::NANPA,
        require_profit_protection: false,
        min_profit_margin: None,
        effective_time: Some(future_effective),
        phone_validation: None,
        routing_plan_id: None, // Use future time
    };

    match routing.find_routes(&future_route_request).await {
        Ok(response) => {
            println!(
                "✓ Found {} routes using future rates",
                response.total_routes
            );
            if let Some(route) = response.routes.first() {
                println!(
                    "  Best route: {} (cost: ${}/min)",
                    route.egress_trunk.name, route.cost_per_minute
                );
            }
        }
        Err(e) => println!("⚠ Future routing failed: {}", e),
    }

    // Demo 5: Check upcoming cutovers
    println!("\n5. Checking upcoming cutovers...");
    match deck_loader.get_decks_to_preload().await {
        Ok(schedules) => {
            println!("✓ Found {} upcoming cutovers", schedules.len());
            for schedule in schedules {
                println!(
                    "  Schedule #{}: {} deck {} -> {} at {}",
                    schedule.id,
                    schedule.deck_type,
                    schedule.current_deck_id,
                    schedule.new_deck_id,
                    schedule.cutover_date.format("%Y-%m-%d %H:%M:%S")
                );
            }
        }
        Err(e) => println!("⚠ Failed to get cutovers: {}", e),
    }

    println!("\n✅ Demo completed successfully!");
    println!("\nKey Features Demonstrated:");
    println!("• Automatic deck versioning");
    println!("• End date management");
    println!("• Time-based routing");
    println!("• Cutover scheduling");
    println!("• Lazy loading support");

    Ok(())
}

// Helper function to demonstrate rate comparison
fn compare_rates(current: &NanpaRate, future: &NanpaRate) {
    println!("Rate comparison for {}:", current.code);
    println!(
        "  Current: ${} -> Future: ${} ({})",
        current.inter_rate,
        future.inter_rate,
        if future.inter_rate < current.inter_rate {
            "REDUCED"
        } else {
            "INCREASED"
        }
    );
}
