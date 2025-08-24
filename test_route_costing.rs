// Test realistic route costing scenarios
fn main() {
    println!("💰 Testing LCR Route Costing and Selection");
    println!("==========================================");
    
    test_route_costing_scenarios();
    test_profit_protection();
    test_route_ordering();
    
    println!("\n✅ Route costing tests completed!");
}

fn test_route_costing_scenarios() {
    println!("\n📊 Testing Route Costing for Real Scenarios");
    
    // Create realistic vendor routes with different cost structures
    let routes = vec![
        Route::new("Vendor-A-Tier1", 0.0045, 0.0100, 6, 6, 100, "Premium tier 1"),
        Route::new("Vendor-B-Value", 0.0055, 0.0050, 6, 6, 110, "Value with low setup"),
        Route::new("Vendor-C-Bulk", 0.0040, 0.0200, 30, 6, 120, "Bulk with high setup"),
        Route::new("Vendor-D-Mobile", 0.0060, 0.0000, 6, 6, 105, "Mobile carrier, no setup"),
        Route::new("Vendor-E-Premium", 0.0035, 0.0300, 60, 1, 90, "Premium low rate, high setup"),
    ];
    
    let call_durations = vec![30, 60, 120, 300, 600]; // 30s, 1min, 2min, 5min, 10min
    
    for duration in call_durations {
        println!("\n  🕐 {} second call costs:", duration);
        
        let mut costs: Vec<_> = routes.iter().map(|route| {
            let total_cost = calculate_call_cost(route, duration);
            (route.name.clone(), total_cost, route.priority)
        }).collect();
        
        // Sort by total cost (LCR logic)
        costs.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap().then(a.2.cmp(&b.2)));
        
        for (i, (name, cost, priority)) in costs.iter().enumerate() {
            println!("    {}. {} - ${:.4} (priority {})", i + 1, name, cost, priority);
        }
        
        // Show winner and analysis
        if let Some((winner, cost, _)) = costs.first() {
            println!("    🏆 Winner: {} at ${:.4} total cost", winner, cost);
        }
    }
}

fn test_profit_protection() {
    println!("\n🛡️  Testing Profit Protection Logic");
    
    let vendor_routes = vec![
        Route::new("Cheap-Vendor", 0.0020, 0.0050, 6, 6, 100, "Very cheap"),
        Route::new("Medium-Vendor", 0.0040, 0.0100, 6, 6, 110, "Medium cost"),
        Route::new("Expensive-Vendor", 0.0080, 0.0200, 6, 6, 120, "Expensive"),
    ];
    
    // Client selling rates
    let client_rate = 0.0050; // Client pays $0.005/min
    let min_profit_margin = 0.0010; // Minimum $0.001/min profit
    
    println!("  Client rate: ${:.4}/min, Minimum profit: ${:.4}/min", client_rate, min_profit_margin);
    
    for route in vendor_routes {
        let cost_60s = calculate_call_cost(&route, 60);
        let revenue_60s = client_rate + 0.0100; // Assume same setup fee structure
        let profit_per_minute = client_rate - route.rate_per_minute;
        let total_profit_60s = revenue_60s - cost_60s;
        
        let passes_protection = profit_per_minute >= min_profit_margin;
        
        println!("  {} - Cost: ${:.4}, Revenue: ${:.4}, Profit/min: ${:.4} {}",
            route.name,
            cost_60s,
            revenue_60s,
            profit_per_minute,
            if passes_protection { "✅ PASS" } else { "❌ FAIL" }
        );
    }
}

fn test_route_ordering() {
    println!("\n📈 Testing LCR Route Ordering Logic");
    
    let routes = vec![
        Route::new("High-Setup-Low-Rate", 0.0030, 0.0500, 6, 6, 100, "High setup, low rate"),
        Route::new("Low-Setup-High-Rate", 0.0070, 0.0020, 6, 6, 100, "Low setup, high rate"),
        Route::new("Medium-Both", 0.0050, 0.0100, 6, 6, 100, "Medium both"),
        Route::new("Premium-Quality", 0.0055, 0.0080, 6, 6, 50, "Higher rate but premium quality"),
    ];
    
    let test_durations = vec![30, 180, 600]; // 30s, 3min, 10min
    
    for duration in test_durations {
        println!("\n  For {}s calls:", duration);
        
        let mut route_costs: Vec<_> = routes.iter().map(|route| {
            let total_cost = calculate_call_cost(route, duration);
            (route, total_cost)
        }).collect();
        
        // Sort by LCR logic: total cost, then priority
        route_costs.sort_by(|a, b| {
            a.1.partial_cmp(&b.1).unwrap().then(a.0.priority.cmp(&b.0.priority))
        });
        
        for (i, (route, cost)) in route_costs.iter().enumerate() {
            let cost_breakdown = format!("${:.4} rate + ${:.4} setup",
                route.rate_per_minute * (calculate_billed_duration(duration, route.min_increment, route.interval) as f64 / 60.0),
                route.setup_fee
            );
            println!("    {}. {} - ${:.4} total ({})", 
                i + 1, route.name, cost, cost_breakdown);
        }
    }
}

// Helper structures and functions

#[derive(Debug, Clone)]
struct Route {
    name: String,
    rate_per_minute: f64,
    setup_fee: f64,
    min_increment: i32,
    interval: i32,
    priority: i32,
    description: String,
}

impl Route {
    fn new(name: &str, rate: f64, setup: f64, min_inc: i32, interval: i32, priority: i32, desc: &str) -> Self {
        Self {
            name: name.to_string(),
            rate_per_minute: rate,
            setup_fee: setup,
            min_increment: min_inc,
            interval,
            priority,
            description: desc.to_string(),
        }
    }
}

fn calculate_call_cost(route: &Route, duration_seconds: i32) -> f64 {
    let billed_duration = calculate_billed_duration(duration_seconds, route.min_increment, route.interval);
    let billed_minutes = billed_duration as f64 / 60.0;
    route.setup_fee + (route.rate_per_minute * billed_minutes)
}

fn calculate_billed_duration(actual_seconds: i32, min_increment: i32, interval: i32) -> i32 {
    if actual_seconds <= min_increment {
        min_increment
    } else {
        let excess = actual_seconds - min_increment;
        let additional_intervals = (excess + interval - 1) / interval; // Ceiling division
        min_increment + (additional_intervals * interval)
    }
}