#!/bin/bash

# Redfire LCR Switch Startup Script
# Sets up database, loads data, and starts the LCR-enabled SIP server

set -e

# Configuration
DATABASE_URL=${DATABASE_URL:-"postgresql://postgres:postgres@localhost:5432/lcr"}
BIND_ADDRESS=${BIND_ADDRESS:-"0.0.0.0:5060"}
DATA_DIR=${DATA_DIR:-"./files"}
LOG_LEVEL=${LOG_LEVEL:-"info"}

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m'

echo -e "${BLUE}🔥 Redfire LCR Switch Startup${NC}"
echo "==============================="
echo -e "${PURPLE}Configuration:${NC}"
echo "  📍 Bind Address: $BIND_ADDRESS"
echo "  🗄️  Database URL: $DATABASE_URL"
echo "  📁 Data Directory: $DATA_DIR"
echo "  📊 Log Level: $LOG_LEVEL"
echo ""

# Step 1: Check Prerequisites
echo -e "${YELLOW}🔍 Step 1: Checking Prerequisites${NC}"

# Check if Rust/Cargo is available
echo -n "  Checking Rust/Cargo... "
if command -v cargo &> /dev/null; then
    echo -e "${GREEN}✅ Available${NC}"
else
    echo -e "${RED}❌ Missing${NC}"
    echo "  Please install Rust: https://rustup.rs/"
    exit 1
fi

# Check if PostgreSQL is running
echo -n "  Checking PostgreSQL... "
if command -v psql &> /dev/null; then
    if psql "$DATABASE_URL" -c "SELECT 1;" > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Connected${NC}"
    else
        echo -e "${RED}❌ Connection failed${NC}"
        echo "  Please check:"
        echo "    - PostgreSQL is running"
        echo "    - Database exists: $DATABASE_URL"
        echo "    - Credentials are correct"
        exit 1
    fi
else
    echo -e "${YELLOW}⚠️  psql not found${NC}"
    echo "  PostgreSQL client recommended for database operations"
fi

# Check if NANPA/LERG data files exist
echo -n "  Checking NANPA/LERG data files... "
if [ -f "$DATA_DIR/npa_report.csv" ] && [ -f "$DATA_DIR/npa-nxx-companytype-ocn.csv" ]; then
    echo -e "${GREEN}✅ Found${NC}"
    DATA_FILES_AVAILABLE=true
else
    echo -e "${YELLOW}⚠️  Not found${NC}"
    echo "    Expected files: $DATA_DIR/npa_report.csv, $DATA_DIR/npa-nxx-companytype-ocn.csv"
    DATA_FILES_AVAILABLE=false
fi

echo ""

# Step 2: Build the LCR Switch
echo -e "${YELLOW}🔨 Step 2: Building LCR Switch${NC}"
echo "  Building lcr_sip_server binary..."

if cargo build --bin lcr_sip_server --release; then
    echo -e "  ${GREEN}✅ Build successful${NC}"
else
    echo -e "  ${RED}❌ Build failed${NC}"
    echo "  Please fix compilation errors and try again"
    exit 1
fi

echo ""

# Step 3: Database Setup
echo -e "${YELLOW}📊 Step 3: Database Setup${NC}"

# Check if LCR schema exists
echo -n "  Checking LCR schema... "
if psql "$DATABASE_URL" -c "SELECT 1 FROM information_schema.tables WHERE table_name = 'egress_trunks';" 2>/dev/null | grep -q "1"; then
    echo -e "${GREEN}✅ Schema exists${NC}"
    SCHEMA_EXISTS=true
else
    echo -e "${YELLOW}⚠️  Schema missing${NC}"
    SCHEMA_EXISTS=false
fi

if [ "$SCHEMA_EXISTS" = false ]; then
    echo "  Loading LCR schema..."
    if [ -f "migrations/lcr_schema.sql" ]; then
        if psql "$DATABASE_URL" -f "migrations/lcr_schema.sql" > /dev/null 2>&1; then
            echo -e "  ${GREEN}✅ Schema loaded successfully${NC}"
        else
            echo -e "  ${RED}❌ Schema loading failed${NC}"
            exit 1
        fi
    else
        echo -e "  ${RED}❌ Schema file not found: migrations/lcr_schema.sql${NC}"
        exit 1
    fi
fi

# Load test configuration
echo -n "  Setting up test configuration... "
if [ -f "tests/sipp/data/lcr_test_setup.sql" ]; then
    if psql "$DATABASE_URL" -f "tests/sipp/data/lcr_test_setup.sql" > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Test config loaded${NC}"
    else
        echo -e "${YELLOW}⚠️  Test config failed (continuing anyway)${NC}"
    fi
else
    echo -e "${YELLOW}⚠️  Test config file not found${NC}"
fi

# Load NANPA/LERG data if available
if [ "$DATA_FILES_AVAILABLE" = true ]; then
    echo "  Loading NANPA/LERG data..."
    if cargo run --bin lcr_data_loader --release -- --data-dir "$DATA_DIR" --database-url "$DATABASE_URL" > /dev/null 2>&1; then
        echo -e "  ${GREEN}✅ NANPA/LERG data loaded${NC}"
    else
        echo -e "  ${YELLOW}⚠️  NANPA/LERG data loading failed (continuing with test data)${NC}"
    fi
fi

echo ""

# Step 4: Show Configuration Summary
echo -e "${YELLOW}📋 Step 4: Configuration Summary${NC}"

if command -v psql &> /dev/null; then
    echo "  Database Status:"
    
    # Check trunk configuration
    EGRESS_COUNT=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM egress_trunks WHERE active = true;" 2>/dev/null | xargs || echo "0")
    INGRESS_COUNT=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM ingress_trunks WHERE active = true;" 2>/dev/null | xargs || echo "0")
    RATE_COUNT=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM vendor_nanpa_rates;" 2>/dev/null | xargs || echo "0")
    
    echo "    📞 Active Egress Trunks: $EGRESS_COUNT"
    echo "    📞 Active Ingress Trunks: $INGRESS_COUNT"  
    echo "    💰 Rate Entries: $RATE_COUNT"
    
    # Show test egress trunk
    if [ "$EGRESS_COUNT" -gt 0 ]; then
        echo ""
        echo "  🎯 Test Egress Trunk:"
        psql "$DATABASE_URL" -c "SELECT name, host || ':' || port as destination FROM egress_trunks WHERE host = '173.193.144.207' AND active = true;" 2>/dev/null || echo "    No test trunk configured"
    fi
fi

echo ""

# Step 5: Start the LCR Switch
echo -e "${YELLOW}🚀 Step 5: Starting LCR Switch${NC}"
echo "  Starting LCR SIP Server on $BIND_ADDRESS..."
echo ""
echo -e "${BLUE}📞 Ready to handle calls!${NC}"
echo -e "${PURPLE}Test Call Configuration:${NC}"
echo "  📞 ANI: 17028880001 (Las Vegas, NV)"
echo "  📞 DNIS: 18002255288 (Toll-Free)"  
echo "  🎯 Egress: 173.193.144.207:5060"
echo ""
echo -e "${GREEN}To test the call, run in another terminal:${NC}"
echo "  ./run-complete-lcr-test.sh"
echo ""
echo -e "${YELLOW}Press Ctrl+C to stop the server${NC}"
echo "=================================="

# Set log level
export RUST_LOG="$LOG_LEVEL"

# Start the server
exec cargo run --bin lcr_sip_server --release -- \
    --bind "$BIND_ADDRESS" \
    --database-url "$DATABASE_URL"