#!/bin/bash

echo "🛠️  Fixing Critical Unwraps in Production Code"
echo "=============================================="

# Function to fix parse() unwraps for IP addresses
fix_ip_parse_unwraps() {
    local file="$1"
    echo "Fixing IP address parsing unwraps in $file..."
    
    # Replace parse().unwrap() for IP addresses with proper error handling
    sed -i 's/"192\.168\.1\.11:20000"\.parse()\.unwrap()/"192.168.1.11:20000".parse().map_err(|e| anyhow!("Invalid address: {}", e))?/g' "$file"
    sed -i 's/"192\.168\.1\.10:20000"\.parse()\.unwrap()/"192.168.1.10:20000".parse().map_err(|e| anyhow!("Invalid address: {}", e))?/g' "$file"
    sed -i 's/"127\.0\.0\.1:.*"\.parse()\.unwrap()/"127.0.0.1:5080".parse().map_err(|e| anyhow!("Invalid address: {}", e))?/g' "$file"
    sed -i 's/"0\.0\.0\.0:0"\.parse()\.unwrap()/"0.0.0.0:0".parse().map_err(|e| anyhow!("Invalid address: {}", e))?/g' "$file"
}

# Function to fix service initialization unwraps
fix_service_init_unwraps() {
    local file="$1"
    echo "Fixing service initialization unwraps in $file..."
    
    # Replace service.new().unwrap() with proper error propagation (but only in non-test code)
    if ! grep -q "#\[test\]" "$file"; then
        sed -i 's/Service::new([^)]*).unwrap()/Service::new(config)?/g' "$file"
        sed -i 's/::new([^)]*).unwrap()/::new(config)?/g' "$file"
    fi
}

# Check which files need fixes
echo "🔍 Scanning for critical unwraps..."

CRITICAL_FILES=$(rg "\.unwrap\(\)" src/ --type rust -l | grep -v test | grep -v "src/bin/" | head -10)

for file in $CRITICAL_FILES; do
    echo ""
    echo "📝 Processing: $file"
    
    # Skip if file doesn't exist
    if [ ! -f "$file" ]; then
        echo "   ❌ File not found"
        continue
    fi
    
    # Count unwraps before
    BEFORE=$(grep -c "\.unwrap()" "$file" 2>/dev/null || echo "0")
    
    # Apply fixes based on file type
    case "$file" in
        *isdn_cli.rs|*rtp*.rs)
            fix_ip_parse_unwraps "$file"
            ;;
        *service*.rs|*billing*.rs|*cnam*.rs)
            fix_service_init_unwraps "$file"
            ;;
    esac
    
    # Count unwraps after
    AFTER=$(grep -c "\.unwrap()" "$file" 2>/dev/null || echo "0")
    FIXED=$((BEFORE - AFTER))
    
    echo "   Before: $BEFORE unwraps"
    echo "   After: $AFTER unwraps"
    if [ $FIXED -gt 0 ]; then
        echo "   ✅ Fixed $FIXED unwraps"
    else
        echo "   ℹ️  No automatic fixes applied"
    fi
done

echo ""
echo "📊 Overall Progress:"
TOTAL_UNWRAPS=$(rg "\.unwrap\(\)" src/ --type rust | grep -v test | grep -v "src/bin/" | wc -l)
echo "   Remaining unwraps in production code: $TOTAL_UNWRAPS"

if [ "$TOTAL_UNWRAPS" -lt 50 ]; then
    echo "   🎉 Target achieved! Less than 50 unwraps in production code."
else
    echo "   🔧 Still needs work. Target: <50 unwraps"
fi

echo ""
echo "🚀 Manual fixes still needed:"
echo "   1. Review message parsing unwraps"
echo "   2. Replace Option unwraps with proper error handling"
echo "   3. Add validation for user inputs"
echo "   4. Use anyhow! for error context"

echo ""
echo "✅ Critical unwrap fix pass complete!"