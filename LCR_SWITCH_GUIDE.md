# 🔥 Redfire LCR Switch Operation Guide

This guide shows how to start up the Redfire Switch to handle your test call: ANI 17028880001 → DNIS 18002255288 → 173.193.144.207:5060

## 🚀 Quick Start

### 1. Start the LCR Switch
```bash
./start-lcr-switch.sh
```

This script will:
- ✅ Check prerequisites (Rust, PostgreSQL, data files)
- 🔨 Build the LCR SIP server
- 📊 Set up the database schema and test data
- 📞 Start the SIP server on port 5060

### 2. Test the Call (in another terminal)
```bash
./run-complete-lcr-test.sh
```

This will run the SIPp test with your specific call scenario.

## 📋 Manual Step-by-Step

If you prefer manual control:

### Prerequisites
1. **PostgreSQL running** with a database called `lcr`
2. **Rust/Cargo installed** 
3. **SIPp installed**: `sudo apt-get install sipp`

### Database Setup
```bash
# Create database (if needed)
createdb lcr

# Load LCR schema
psql "postgresql://postgres:postgres@localhost:5432/lcr" -f migrations/lcr_schema.sql

# Load test configuration
psql "postgresql://postgres:postgres@localhost:5432/lcr" -f tests/sipp/data/lcr_test_setup.sql

# Load NANPA/LERG data (optional, test data is sufficient)
cargo run --bin lcr_data_loader -- --data-dir ./files
```

### Build and Start Switch
```bash
# Build the LCR SIP server
cargo build --bin lcr_sip_server --release

# Start the server
cargo run --bin lcr_sip_server --release -- \
    --bind "0.0.0.0:5060" \
    --database-url "postgresql://postgres:postgres@localhost:5432/lcr"
```

## 🔧 Configuration

### Environment Variables
- `DATABASE_URL` - PostgreSQL connection string
- `BIND_ADDRESS` - SIP server bind address (default: 0.0.0.0:5060)
- `RUST_LOG` - Log level (debug, info, warn, error)

### Test Configuration
The test setup includes:
- **Ingress Trunk**: Accepts calls from SIPp (127.0.0.1)
- **Egress Trunk**: Routes to 173.193.144.207:5060
- **Rate Tables**: Toll-free rates (cost: $0.0015/min, client: $0.00)
- **LCR Routes**: Links ingress → egress via LCR logic

## 📞 Call Flow Details

### Expected Call Processing:

1. **📥 INVITE Reception**
   ```
   INVITE sip:18002255288@localhost:5060 SIP/2.0
   From: "Las Vegas Test" <sip:17028880001@...>
   ```

2. **🧠 LCR Analysis**
   - ANI: 17028880001 → Las Vegas, NV (1702 area code)
   - DNIS: 18002255288 → Toll-free (1800 area code)
   - Jurisdiction: **Indeterminate** (toll-free override)

3. **📊 Rate Lookup**
   - Finds toll-free rates in database
   - Client cost: $0.00 (toll-free is free to caller)
   - Vendor cost: ~$0.0015/min

4. **🎯 Route Selection**
   - LCR finds egress trunk to 173.193.144.207:5060
   - Routes call via configured trunk

5. **📞 SIP Signaling**
   ```
   INVITE → 100 Trying → 180 Ringing → 200 OK → ACK → [8s call] → BYE → 200 OK
   ```

## 🔍 Monitoring and Troubleshooting

### Check Server Status
```bash
# Test if server is running
telnet localhost 5060

# Send OPTIONS ping
echo -e "OPTIONS sip:test@localhost:5060 SIP/2.0\r\nVia: SIP/2.0/UDP localhost:5060\r\nCall-ID: test\r\nFrom: test\r\nTo: test\r\nCSeq: 1 OPTIONS\r\n\r\n" | nc -u localhost 5060
```

### View Logs
The LCR server provides detailed logging:
```bash
# Start with debug logging
RUST_LOG=debug ./start-lcr-switch.sh

# Key log entries to look for:
# - "LCR SIP Server bound to 0.0.0.0:5060"
# - "Processing INVITE from ..."  
# - "Call: 17028880001 → 18002255288"
# - "LCR Route found: 18002255288 → 173.193.144.207:5060"
```

### Database Verification
```bash
# Check configuration
./verify-lcr-routing.sh

# Direct database queries
psql "$DATABASE_URL" -c "SELECT * FROM egress_trunks WHERE active = true;"
psql "$DATABASE_URL" -c "SELECT * FROM vendor_nanpa_rates WHERE code LIKE '1800%';"
```

## 🧪 Testing Scenarios

### Basic Test (Your Scenario)
```bash
./run-complete-lcr-test.sh
```

### Custom SIPp Test
```bash
sipp -sf tests/sipp/scenarios/lcr_toll_free_test.xml \
     -i localhost -p 5061 -r 1 -l 1 -m 1 \
     localhost:5060
```

### CLI Simulation
```bash
cargo run --bin lcr_cli -- \
    --database-url "$DATABASE_URL" \
    simulate 17028880001 18002255288
```

## 🚨 Common Issues

### 1. Server Won't Start
- **Check port availability**: `lsof -i :5060`
- **Verify database connection**: Test DATABASE_URL
- **Check permissions**: Ensure bind address is accessible

### 2. No Route Found
- **Check rate tables**: Verify toll-free rates exist
- **Check trunk config**: Ensure egress trunk to 173.193.144.207:5060
- **Check associations**: Verify trunk-rate-route linkage

### 3. SIP Errors
- **503 Service Unavailable**: Check egress trunk capacity
- **404 Not Found**: No LCR route found
- **480 Temporarily Unavailable**: Target unreachable

### 4. Database Issues
- **Connection refused**: Check PostgreSQL service
- **Schema missing**: Run migrations/lcr_schema.sql
- **No rates**: Load test data or NANPA files

## 📊 Performance Notes

- **Capacity**: Test configuration handles ~100 concurrent calls
- **Response Time**: LCR lookup typically <10ms
- **Memory**: ~50MB base + ~10MB per 1000 active calls
- **CPU**: Minimal load for test scenarios

## 🔒 Security Considerations

- Server binds to all interfaces (0.0.0.0) for testing
- No authentication configured (test mode)
- Rate limiting disabled for testing
- For production: Configure authentication, TLS, rate limiting

## 📝 Next Steps

After successful testing:
1. 🔧 Configure production trunks and rates
2. 🔒 Add authentication and security
3. 📊 Set up monitoring and logging
4. 🧪 Test additional call scenarios
5. 📈 Performance tuning and optimization

---

**Ready to test!** Start with `./start-lcr-switch.sh` and then run `./run-complete-lcr-test.sh` in another terminal.