# RedFire Switch - Corrected Billing Examples

## Trunk Configuration Examples

### Customer Trunk (We Bill Them)
```rust
TrunkRateConfig {
    trunk_id: 200,
    trunk_name: "Customer_Corp_SIP".to_string(),
    direction: TrunkDirection::Bidirectional,
    ip_addresses: vec!["192.168.1.100".parse().unwrap()],
    
    // Customer rates
    default_rate_per_minute: 0.12,  // We charge customer $0.12/min for outbound
    is_revenue_trunk: true,          // Customer pays us
    trunk_type: TrunkType::Customer,
    
    // Customer's assigned DIDs
    our_number_blocks: vec!["2125551234".to_string()], // DID they rent from us
}
```

### Carrier Origination Trunk (We Pay Them)
```rust
TrunkRateConfig {
    trunk_id: 10,
    trunk_name: "Verizon_Origination".to_string(), 
    direction: TrunkDirection::Ingress,
    ip_addresses: vec!["10.1.1.1".parse().unwrap()],
    
    // Origination rates
    default_rate_per_minute: 0.015,  // We pay Verizon $0.015/min for DID delivery
    is_revenue_trunk: false,         // We pay them
    trunk_type: TrunkType::Carrier,
}
```

### Carrier Termination Trunk (We Pay Them)
```rust
TrunkRateConfig {
    trunk_id: 11,
    trunk_name: "Verizon_Termination".to_string(),
    direction: TrunkDirection::Egress, 
    ip_addresses: vec!["10.1.1.2".parse().unwrap()],
    
    // Termination rates
    default_rate_per_minute: 0.085,  // We pay Verizon $0.085/min for termination
    is_revenue_trunk: false,         // We pay them
    trunk_type: TrunkType::Carrier,
}
```

## Billing Examples

### Origination Call (DID Inbound)
**Flow**: `Random Caller → Verizon → RedFire → Customer Corp`

```rust
CallDetailRecord {
    ani: "5551234567",     // Random caller
    dnis: "2125551234",    // Customer's DID
    duration_seconds: Some(3000), // 50 minutes
    
    // INGRESS: We PAY Verizon for origination
    ingress_trunk_id: 10,  // Verizon Origination
    ingress_rate_per_minute: 0.015,
    ingress_cost: 0.75,    // 50 min × $0.015 = $0.75 (we pay)
    ingress_revenue: None, // No revenue from ingress
    
    // EGRESS: We BILL Customer for DID service  
    egress_trunk_name: Some("Customer_Corp_SIP".to_string()),
    egress_rate_per_minute: 0.10,  // DID service rate
    egress_cost: 0.0,      // No cost to deliver
    egress_revenue: Some(5.00), // 50 min × $0.10 = $5.00 (we bill customer)
    
    // NET RESULT
    total_cost: 0.75,      // What we pay Verizon
    total_revenue: 5.00,   // What customer pays us
    net_margin: 4.25,      // $4.25 profit
    profit_margin_percent: 85.0, // 85% margin
}
```

### Termination Call (Customer Outbound)
**Flow**: `Customer Corp → RedFire → Verizon → Destination`

```rust
CallDetailRecord {
    ani: "2125551234",     // Customer
    dnis: "3105551234",    // LA number they're calling
    duration_seconds: Some(3000), // 50 minutes
    
    // INGRESS: We BILL Customer for outbound service
    ingress_trunk_id: 200, // Customer trunk
    ingress_rate_per_minute: 0.12,
    ingress_cost: -6.00,   // Negative = revenue (50 min × $0.12)
    ingress_revenue: Some(6.00), // We bill customer
    
    // EGRESS: We PAY Verizon for termination
    egress_trunk_id: Some(11), // Verizon Termination
    egress_rate_per_minute: 0.085,
    egress_cost: 4.25,     // 50 min × $0.085 = $4.25 (we pay)
    egress_revenue: None,  // No revenue from egress
    
    // NET RESULT
    total_cost: 4.25,      // What we pay Verizon
    total_revenue: 6.00,   // What customer pays us  
    net_margin: 1.75,      // $1.75 profit
    profit_margin_percent: 29.17, // 29% margin
}
```

### Transit Call (Carrier to Carrier)
**Flow**: `AT&T → RedFire → Verizon`

```rust
CallDetailRecord {
    ani: "8005551234",     // Toll-free
    dnis: "2125559999",    // NYC number
    duration_seconds: Some(3000), // 50 minutes
    
    // INGRESS: AT&T PAYS US for transit
    ingress_trunk_id: 20,  // AT&T Transit
    ingress_rate_per_minute: 0.020,
    ingress_cost: -1.00,   // Negative = revenue (50 min × $0.020)
    ingress_revenue: Some(1.00), // AT&T pays us
    
    // EGRESS: We PAY Verizon for completion
    egress_trunk_id: Some(11), // Verizon Termination
    egress_rate_per_minute: 0.015,
    egress_cost: 0.75,     // 50 min × $0.015 = $0.75 (we pay)
    egress_revenue: None,
    
    // NET RESULT
    total_cost: 0.75,      // What we pay Verizon
    total_revenue: 1.00,   // What AT&T pays us
    net_margin: 0.25,      // $0.25 profit  
    profit_margin_percent: 25.0, // 25% margin
}
```

## Key Points

1. **Origination (DID Inbound)**: 
   - **Ingress cost**: We pay carrier for delivery
   - **Egress revenue**: We bill customer for DID service

2. **Termination (Customer Outbound)**:
   - **Ingress revenue**: We bill customer for outbound  
   - **Egress cost**: We pay carrier for completion

3. **Transit (Carrier to Carrier)**:
   - **Ingress revenue**: Origin carrier pays us
   - **Egress cost**: We pay destination carrier

4. **Revenue Trunk Flag**:
   - `true` = Customer trunk (we bill them)
   - `false` = Carrier trunk (we pay them)