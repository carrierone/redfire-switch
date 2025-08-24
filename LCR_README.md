# Redfire Switch - Least Cost Routing (LCR) Engine

A high-performance least cost routing engine for NANPA calls with PostgreSQL storage, real-time caching, and comprehensive Class 4 switch functionality.

## Features

### Core LCR Functionality
- **Multiple Route Types**: NANPA, A-Z, and custom routing
- **LRN and DNIS Rating**: Support for both Local Routing Number and DNIS-based rating
- **Jurisdiction Detection**: Automatic determination of interstate, intrastate, indeterminate, and local call types
- **Profit Protection**: Configurable minimum profit margins per trunk/client
- **Route Advancement**: Intelligent failover based on SIP response codes
- **Static Routes**: Regex-based special case routing (emergency, toll-free, etc.)

### Performance & Scalability
- **In-Memory Caching**: Fast Redis-like cache for rate lookups
- **PostgreSQL Storage**: High-precision decimal rates (up to 7 decimal places)
- **Cluster Synchronization**: Multi-node cache synchronization
- **Capacity Management**: Real-time trunk capacity and CPS limit enforcement
- **Hot Reloading**: Live cache updates without service interruption

### Telecommunications Features
- **NANPA Compliance**: Full support for North American numbering plan
- **Timer Management**: Comprehensive Class 4 timer configurations
- **Trunk Management**: Ingress/egress trunk monitoring and statistics
- **Call Simulation**: Comprehensive routing simulation and testing
- **Rate Deck Management**: Multiple vendor/client rate decks with effective dates

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   CLI/API       │    │  Routing Engine │    │  Cache Layer    │
│                 │────│                 │────│                 │
│ • Call Sim      │    │ • LCR Logic     │    │ • Rate Tables   │
│ • Management    │    │ • Route Advance │    │ • Trunk Data    │
│ • Monitoring    │    │ • Jurisdiction  │    │ • NANPA Static  │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         │              ┌─────────────────┐               │
         │              │ Trunk Manager   │               │
         └──────────────│                 │───────────────┘
                        │ • Capacity Mgmt │
                        │ • CPS Limiting  │
                        │ • Statistics    │
                        └─────────────────┘
                                 │
                    ┌─────────────────┐
                    │  PostgreSQL DB  │
                    │                 │
                    │ • Rate Decks    │
                    │ • Trunk Config  │
                    │ • NANPA Data    │
                    │ • Statistics    │
                    └─────────────────┘
```

## Quick Start

### 1. Database Setup

```bash
# Run migrations
psql -d lcr_db -f migrations/lcr_schema.sql

# Load sample data
psql -d lcr_db -f migrations/lcr_sample_data.sql

# Load NANPA data from CSV files
cargo run --bin lcr_cli -- --database-url "postgresql://user:pass@localhost/lcr_db" load-nanpa
```

### 2. Build and Run

```bash
# Build the LCR CLI
cargo build --bin lcr_cli

# Run call simulation
./target/debug/lcr_cli --database-url "postgresql://user:pass@localhost/lcr_db" \
    simulate --ani 12125551234 --dnis 14155555678 --format detailed

# Start API server
./target/debug/lcr_cli --database-url "postgresql://user:pass@localhost/lcr_db" \
    api-server --bind 0.0.0.0:8080
```

### 3. API Usage

```bash
# Simulate a call via API
curl -X POST http://localhost:8080/simulate \
  -H "Content-Type: application/json" \
  -d '{"ani": "12125551234", "dnis": "14155555678"}'

# Get route for specific call
curl -X POST http://localhost:8080/route \
  -H "Content-Type: application/json" \
  -d '{
    "ani": "12125551234",
    "dnis": "14155555678", 
    "ingress_trunk_id": 1,
    "route_type": "NANPA",
    "require_profit_protection": true
  }'

# Reload cache
curl -X POST http://localhost:8080/cache/reload

# Get trunk statistics
curl http://localhost:8080/trunks/stats
```

## Configuration

### Rate Decks

The system supports multiple rate decks for vendors (costs) and clients (selling rates):

```sql
-- Vendor rate deck (costs)
INSERT INTO vendor_rate_decks (name, vendor_id, rate_type) 
VALUES ('Vendor A NANPA', 1, 'DNIS');

-- NANPA rates with jurisdiction-specific pricing
INSERT INTO vendor_nanpa_rates (deck_id, code, inter_rate, intra_rate, ij_rate, local_rate) 
VALUES (1, '1212', 0.0035, 0.0040, 0.0038, 0.0020);
```

### Trunk Configuration

```sql
-- Egress trunk (vendor)
INSERT INTO egress_trunks (name, vendor_id, host, capacity_limit, cps_limit) 
VALUES ('Vendor-A-Primary', 1, 'sip.vendor-a.com', 1000, 100.0);

-- Ingress trunk (client) with profit protection
INSERT INTO ingress_trunks (name, client_id, ip_address, profit_protection, min_profit_margin) 
VALUES ('Client-A', 1, '192.168.1.10', true, 0.0020);
```

### Route Advance Configuration

```sql
-- Custom route advance codes per trunk
INSERT INTO route_advance_configs (scope, scope_id, advance_on_codes, stop_on_codes) 
VALUES ('INGRESS_TRUNK', 1, 
        ARRAY['503', '504', '480', '487'], 
        ARRAY['404', '486', '600']);
```

## Call Simulation Examples

### Basic Simulation
```bash
# Simulate NYC to LA call
./lcr_cli simulate --ani 12125551234 --dnis 12135555678 --format table
```

Output:
```
ANI: 12125551234 -> DNIS: 12135555678 (LRN: N/A)
Jurisdiction: Interstate | Decision: ROUTE_FOUND

Egress Trunk                   Vendor          Cost/min     Sell/min     Profit/min
---------------------------------------------------------------------------------
Vendor-A-Primary              Vendor 1        $0.0035      $0.0100      $0.0065
Vendor-A-Secondary            Vendor 1        $0.0035      $0.0100      $0.0065
Vendor-B-Primary              Vendor 2        $0.0050      $0.0110      $0.0060
```

### Detailed Simulation with Profit Protection
```bash
./lcr_cli route --ani 12125551234 --dnis 14155555678 \
    --trunk-id 1 --profit-protection --min-profit 0.0050
```

### API Call Simulation
```bash
curl -X GET "http://localhost:8080/simulate/12125551234/14155555678?ingress_trunk=Client-A"
```

## Jurisdiction Logic

The system automatically determines call jurisdiction:

- **Interstate**: ANI and DNIS in different states
- **Intrastate**: ANI and DNIS in same state  
- **Local**: Same rate center or local calling area
- **Indeterminate**: When jurisdiction cannot be determined

### LRN Integration
```bash
# With LRN lookup enabled, the system:
# 1. Checks LRN cache for DNIS
# 2. Uses LRN for rating if available
# 3. Falls back to original DNIS
# 4. Determines jurisdiction based on actual routing
```

## Profit Protection

Configurable profit protection ensures minimum margins:

```sql
-- Global profit protection
UPDATE ingress_trunks SET profit_protection = true, min_profit_margin = 0.0010;

-- Per-call profit checking
{
  "require_profit_protection": true,
  "min_profit_margin": 0.0050
}
```

Routes with insufficient profit margins are automatically excluded.

## Route Advancement

When calls fail, the system advances to the next route based on SIP codes:

**Default Advance Codes**: 503, 504, 603, 606, 480, 487, 502, 500
**Default Stop Codes**: 404, 486, 600, 604, 403, 401, 402

```bash
# Custom per-trunk route advance configuration
INSERT INTO route_advance_configs (scope, scope_id, advance_on_codes) 
VALUES ('INGRESS_TRUNK', 1, ARRAY['503', '504']);
```

## Timer Management

Comprehensive Class 4 switch timers:

- **100-183 Timer**: Max time between 100 Trying and 183 Session Progress
- **Ringing Timer**: Maximum ringing duration  
- **Call Duration**: Maximum call length before forced disconnect
- **Transaction Timer**: SIP transaction timeout

```sql
-- Custom timers per trunk
INSERT INTO timer_configs (scope, scope_id, timer_max_call_duration_sec) 
VALUES ('INGRESS_TRUNK', 1, 7200); -- 2 hours max
```

## Monitoring & Statistics

### Trunk Statistics
```bash
./lcr_cli trunk-stats --trunk-type all
```

### API Endpoints
- `GET /trunks/stats` - Current trunk usage
- `GET /health` - Service health check
- `POST /cache/reload` - Reload configuration
- `GET /trunks/ingress` - List ingress trunks
- `GET /trunks/egress` - List egress trunks

## Database Schema

### Key Tables
- `vendor_rate_decks` / `client_rate_decks` - Rate deck metadata
- `vendor_nanpa_rates` / `client_nanpa_rates` - NANPA-specific rates
- `egress_trunks` / `ingress_trunks` - Trunk configurations
- `lcr_routes` - Dynamic routing configurations  
- `static_routes` - Special case routing
- `nanpa_static` - NANPA numbering plan data
- `trunk_usage_stats` - Real-time usage statistics

### Rate Precision
All rates support up to 7 decimal places (DECIMAL(10,7)) for high-precision billing.

## Performance

### Benchmarks
- **Route Lookup**: < 1ms average for 100,000+ rate entries
- **Call Simulation**: < 5ms end-to-end with jurisdiction determination
- **Cache Reload**: < 10 seconds for 1M+ rate entries
- **Concurrent Calls**: 10,000+ simultaneous route calculations

### Optimization
- In-memory caching of all routing data
- Efficient prefix matching for rate lookups
- Minimal database queries during call processing
- Async processing for non-critical operations

## Cluster Deployment

For high availability, deploy multiple LCR nodes:

```bash
# Node 1
./lcr_cli api-server --bind 192.168.1.10:8080

# Node 2  
./lcr_cli api-server --bind 192.168.1.11:8080

# Load balancer routes between nodes
# Cache synchronization via PostgreSQL notifications
```

## Development

### Running Tests
```bash
# Integration tests
cargo test lcr_tests --features postgres

# Load test with sample data
DATABASE_URL="postgresql://test:test@localhost/lcr_test" cargo test
```

### Adding New Rate Types
1. Extend `RouteType` enum in `types.rs`
2. Add rate tables following the NANPA pattern
3. Implement rating logic in `routing.rs`
4. Update jurisdiction determination if needed

### Custom Jurisdiction Logic
Override `determine_local_jurisdiction()` in `jurisdiction.rs` for carrier-specific local calling rules.

## License

GPL-3.0-or-later - See LICENSE file for details.

Copyright (C) 2025 Carrier One Inc and contributors.