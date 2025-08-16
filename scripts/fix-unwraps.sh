#!/bin/bash

# Script to identify and help fix dangerous unwrap() calls
# This helps fix CVE-2024-005: Panic-based DoS attacks

echo "🔍 Analyzing unwrap() usage in RedFire Switch..."
echo "================================================"

# Count total unwraps
TOTAL=$(grep -r "\.unwrap()" src/ --include="*.rs" | wc -l)
echo "Total unwrap() calls found: $TOTAL"
echo ""

# Find unwraps in critical paths
echo "🚨 Critical Path Analysis (main message handling):"
echo "---------------------------------------------------"

# Check main B2BUA implementations
for file in "sipi_b2bua.rs" "stir_shaken_b2bua.rs" "secure_sipi_b2bua.rs" "simple_b2bua.rs" "stir_shaken.rs"; do
    if [ -f "src/$file" ]; then
        COUNT=$(grep -c "\.unwrap()" "src/$file" 2>/dev/null || echo "0")
        if [ "$COUNT" -gt "0" ]; then
            echo "  ⚠️  src/$file: $COUNT unwrap() calls"
            grep -n "\.unwrap()" "src/$file" | head -3 | while read line; do
                echo "      Line $line"
            done
        else
            echo "  ✅ src/$file: CLEAN (no unwraps)"
        fi
    fi
done

echo ""
echo "📊 Files with most unwrap() calls:"
echo "-----------------------------------"
grep -r "\.unwrap()" src/ --include="*.rs" | cut -d: -f1 | sort | uniq -c | sort -rn | head -10

echo ""
echo "🔧 Safe Replacement Patterns:"
echo "------------------------------"
echo "1. For Option types:"
echo "   UNSAFE:  value.unwrap()"
echo "   SAFE:    value.ok_or_else(|| anyhow!(\"Error message\"))?"
echo ""
echo "2. For Result types:"
echo "   UNSAFE:  result.unwrap()"
echo "   SAFE:    result.map_err(|e| anyhow!(\"Context: {}\", e))?"
echo ""
echo "3. For expect() calls:"
echo "   UNSAFE:  value.expect(\"message\")"
echo "   SAFE:    value.ok_or_else(|| anyhow!(\"message\"))?"
echo ""
echo "4. In tests (keep unwrap):"
echo "   OK:      #[test] functions can use unwrap()"

echo ""
echo "📝 Next Steps:"
echo "--------------"
echo "1. Replace unwrap() in message handling paths first"
echo "2. Replace unwrap() in header extraction next"
echo "3. Replace unwrap() in security-critical code"
echo "4. Keep unwrap() only in tests and initialization"
echo ""

# Generate a priority fix list
echo "🎯 Priority Fix List (most dangerous):"
echo "---------------------------------------"
grep -r "\.unwrap()" src/ --include="*.rs" | grep -E "(handle_|extract_|validate_|parse_|process_)" | head -10

echo ""
echo "✅ To verify fixes, run: cargo build --all-targets"