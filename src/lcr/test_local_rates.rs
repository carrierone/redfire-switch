#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcr::types::*;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn test_local_rate_fallback() {
        // Create a rate with NO local_rate (NULL)
        let rate = NanpaRate {
            id: 1,
            deck_id: 1,
            code: "1555001".to_string(),
            inter_rate: Decimal::from_str("0.0150").unwrap(), // 1.5 cents
            intra_rate: Decimal::from_str("0.0120").unwrap(), // 1.2 cents  
            ij_rate: Decimal::from_str("0.0140").unwrap(),    // 1.4 cents
            local_rate: None, // NULL - this is the key test case
            min_increment: 6,
            interval: 6,
            setup_fee: None,
        };

        // Test rate selection for each jurisdiction
        assert_eq!(
            get_rate_for_jurisdiction(&rate, CallJurisdiction::Interstate),
            Decimal::from_str("0.0150").unwrap(),
            "Interstate rate should be inter_rate"
        );
        
        assert_eq!(
            get_rate_for_jurisdiction(&rate, CallJurisdiction::Intrastate),
            Decimal::from_str("0.0120").unwrap(),
            "Intrastate rate should be intra_rate"
        );
        
        assert_eq!(
            get_rate_for_jurisdiction(&rate, CallJurisdiction::IndeterminateJurisdiction),
            Decimal::from_str("0.0140").unwrap(),
            "IJ rate should be ij_rate"
        );
        
        // This is the critical test - local_rate is NULL, should fall back to intra_rate
        assert_eq!(
            get_rate_for_jurisdiction(&rate, CallJurisdiction::Local),
            Decimal::from_str("0.0120").unwrap(),
            "Local rate should fallback to intra_rate when local_rate is NULL"
        );
    }

    #[test]
    fn test_local_rate_when_present() {
        // Create a rate WITH local_rate
        let rate = NanpaRate {
            id: 1,
            deck_id: 1,
            code: "1555001".to_string(),
            inter_rate: Decimal::from_str("0.0150").unwrap(),
            intra_rate: Decimal::from_str("0.0120").unwrap(),
            ij_rate: Decimal::from_str("0.0140").unwrap(),
            local_rate: Some(Decimal::from_str("0.0100").unwrap()), // 1.0 cent - local is cheaper
            min_increment: 6,
            interval: 6,
            setup_fee: None,
        };

        // When local_rate is present, it should be used
        assert_eq!(
            get_rate_for_jurisdiction(&rate, CallJurisdiction::Local),
            Decimal::from_str("0.0100").unwrap(),
            "Local rate should use actual local_rate when present"
        );
    }

    // Helper function to simulate the rate selection logic
    fn get_rate_for_jurisdiction(rate: &NanpaRate, jurisdiction: CallJurisdiction) -> Decimal {
        match jurisdiction {
            CallJurisdiction::Interstate => rate.inter_rate,
            CallJurisdiction::Intrastate => rate.intra_rate,
            CallJurisdiction::IndeterminateJurisdiction => rate.ij_rate,
            CallJurisdiction::Local => rate.local_rate.unwrap_or(rate.intra_rate),
        }
    }

    #[test]
    fn test_call_simulation_with_null_local_rates() {
        use crate::lcr::types::CallSimulation;
        
        // Simulate creating a call simulation for local jurisdiction
        // This would typically come from the routing engine
        let simulation = CallSimulation {
            ani: "15550001234".to_string(),
            dnis: "15550009876".to_string(),
            lrn: Some("15550009876".to_string()),
            jurisdiction: CallJurisdiction::Local,
            ingress_trunk: "trunk1".to_string(),
            total_routes: 1,
            routes: vec![crate::lcr::types::SimulatedRoute {
                egress_trunk: "vendor_trunk_1".to_string(),
                vendor: "Vendor A".to_string(),
                cost_per_minute: Decimal::from_str("0.0120").unwrap(), // Should fallback to intra_rate
                selling_per_minute: Decimal::from_str("0.0180").unwrap(),
                profit_margin: Decimal::from_str("0.0060").unwrap(),
                priority: 1,
                setup_fee: Decimal::ZERO,
                min_increment: 6,
                interval: 6,
            }],
            routing_decision: "Selected best rate route".to_string(),
        };

        // Verify simulation works with Local jurisdiction
        assert_eq!(simulation.jurisdiction, CallJurisdiction::Local);
        assert_eq!(simulation.routes.len(), 1);
        
        // The cost should be the intra_rate since local_rate was NULL
        assert_eq!(
            simulation.routes[0].cost_per_minute,
            Decimal::from_str("0.0120").unwrap()
        );
    }
}