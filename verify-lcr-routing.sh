#!/bin/bash

# Quick LCR Routing Verification
# Tests the specific call scenario: 17028880001 → 18002255288

set -e

DATABASE_URL=${DATABASE_URL:-"postgresql://postgres:postgres@localhost:5432/lcr"}

echo "🔍 LCR Routing Verification for ANI 17028880001 → DNIS 18002255288"
echo "================================================================="

# Test using LCR CLI if available
if [ -f "target/debug/lcr_cli" ] || [ -f "target/release/lcr_cli" ]; then
    echo ""
    echo "📞 Testing via LCR CLI..."
    
    # Try to run LCR simulation
    if cargo run --bin lcr_cli -- --database-url "$DATABASE_URL" simulate-call 17028880001 18002255288 2>/dev/null; then
        echo "✅ LCR CLI test completed"
    else
        echo "⚠️  LCR CLI simulation not available or failed"
    fi
else
    echo "ℹ️  LCR CLI not built - run 'cargo build --bin lcr_cli' first"
fi

echo ""
echo "🗄️  Database Verification..."

# Check if we can connect to database
if command -v psql &> /dev/null; then
    # Test database connection
    if psql "$DATABASE_URL" -c "SELECT 1;" > /dev/null 2>&1; then
        echo "✅ Database connection successful"
        
        echo ""
        echo "📊 Checking LCR configuration:"
        
        # Check for test trunks
        echo -n "  Egress trunk to 173.193.144.207:5060... "
        EGRESS_COUNT=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM egress_trunks WHERE host = '173.193.144.207' AND port = 5060 AND active = true;" 2>/dev/null | xargs)
        if [ "$EGRESS_COUNT" -gt 0 ]; then
            echo "✅ Configured ($EGRESS_COUNT found)"
        else
            echo "❌ Missing"
        fi
        
        # Check for toll-free rates
        echo -n "  Toll-free rates (1800*)... "
        TOLL_FREE_RATES=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM vendor_nanpa_rates WHERE code LIKE '1800%';" 2>/dev/null | xargs)
        if [ "$TOLL_FREE_RATES" -gt 0 ]; then
            echo "✅ Found ($TOLL_FREE_RATES entries)"
        else
            echo "❌ Missing"
        fi
        
        # Check for Las Vegas rates  
        echo -n "  Las Vegas rates (1702*)... "
        LV_RATES=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM vendor_nanpa_rates WHERE code LIKE '1702%';" 2>/dev/null | xargs)
        if [ "$LV_RATES" -gt 0 ]; then
            echo "✅ Found ($LV_RATES entries)"
        else
            echo "❌ Missing"
        fi
        
        # Check special service codes
        echo -n "  Special service codes (toll-free)... "
        SPECIAL_CODES=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM special_service_codes WHERE service_type = 'TOLL_FREE' AND active = true;" 2>/dev/null | xargs)
        if [ "$SPECIAL_CODES" -gt 0 ]; then
            echo "✅ Found ($SPECIAL_CODES entries)"
        else
            echo "❌ Missing"
        fi
        
        echo ""
        echo "🔍 Route Analysis:"
        
        # Show expected routing
        echo "Expected call flow:"
        echo "  1. ANI 17028880001 (Las Vegas, NV) → Identified as US domestic"
        echo "  2. DNIS 18002255288 (Toll-Free) → Identified as special service"
        echo "  3. Jurisdiction: Indeterminate (toll-free override)"
        echo "  4. Rate lookup: Use IJ (Indeterminate Jurisdiction) rates"
        echo "  5. Route: Via egress trunk to 173.193.144.207:5060"
        
        echo ""
        echo "📋 Rate Information:"
        
        # Show rates for this call
        psql "$DATABASE_URL" -c "
        SELECT 
            'Vendor Cost' as rate_type,
            code as destination,
            ij_rate as rate_per_minute,
            setup_fee as setup_cost,
            min_increment || '/' || interval as billing
        FROM vendor_nanpa_rates 
        WHERE code IN ('1800', '18002255288', '1702', '17028880001')
        ORDER BY LENGTH(code) DESC;
        " 2>/dev/null || echo "Could not retrieve rate information"
        
    else
        echo "❌ Database connection failed"
        echo "   Please check DATABASE_URL: $DATABASE_URL"
    fi
else
    echo "⚠️  psql not available - skipping database checks"
fi

echo ""
echo "🌐 Network Connectivity:"

# Test connectivity to target
echo -n "  Testing connection to 173.193.144.207:5060... "
if timeout 3 bash -c "</dev/tcp/173.193.144.207/5060" 2>/dev/null; then
    echo "✅ Reachable"
else
    echo "⚠️  Not directly reachable (may be behind firewall)"
fi

echo ""
echo "🚀 Ready to run SIPp test!"
echo "Execute: ./run-complete-lcr-test.sh"