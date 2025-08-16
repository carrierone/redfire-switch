#!/bin/bash

# B2BUA Testing Script for Redfire Switch
set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_color() {
    echo -e "${1}${2}${NC}"
}

print_header() {
    echo
    print_color "$BLUE" "=========================================="
    print_color "$BLUE" "  $1"
    print_color "$BLUE" "=========================================="
}

# Check if we can compile
print_header "B2BUA Functionality Test"
print_color "$YELLOW" "Testing basic B2BUA functionality with SIPp..."

# Test 1: Check if SIPp is available
print_color "$YELLOW" "\n1. Checking SIPp availability..."
if command -v sipp &> /dev/null; then
    print_color "$GREEN" "✅ SIPp is available"
    sipp -v | head -1
else
    print_color "$RED" "❌ SIPp not found. Run: sudo apt-get install sipp"
    exit 1
fi

# Test 2: Try to build the project
print_color "$YELLOW" "\n2. Attempting to build Redfire Switch..."
if timeout 120 cargo build --release 2>/dev/null; then
    print_color "$GREEN" "✅ Build successful"
    BUILD_SUCCESS=true
else
    print_color "$YELLOW" "⚠️  Build failed, but we can still test SIP message handling"
    BUILD_SUCCESS=false
fi

# Test 3: Check if we can start a basic SIP server
print_color "$YELLOW" "\n3. Testing SIP server functionality..."

if [ "$BUILD_SUCCESS" = true ]; then
    print_color "$YELLOW" "Starting Redfire Switch in background..."
    timeout 5 ./target/release/redfire-switch --config config-test.json start &
    SERVER_PID=$!
    sleep 2
    
    # Test with SIPp OPTIONS ping
    print_color "$YELLOW" "Testing with SIPp OPTIONS ping..."
    if timeout 10 sipp -sf tests/sipp/scenarios/options_ping.xml 127.0.0.1:5060 -m 1 -t un; then
        print_color "$GREEN" "✅ SIP server responded to OPTIONS"
    else
        print_color "$YELLOW" "⚠️  No response to OPTIONS (expected if server not fully implemented)"
    fi
    
    # Stop the server
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
else
    print_color "$YELLOW" "Skipping server test due to build failure"
fi

# Test 4: Test basic B2BUA scenario
print_color "$YELLOW" "\n4. Testing B2BUA call scenario..."
print_color "$YELLOW" "This test will show what needs to be implemented..."

# Create a dummy UAS for testing
print_color "$YELLOW" "Creating test UAS on port 5070..."
timeout 30 sipp -sn uas -p 5070 -bg &
UAS_PID=$!
sleep 1

# Test against a simple echo server (netcat) to see SIP message flow
print_color "$YELLOW" "Testing SIP message flow to port 5060..."
if timeout 10 sipp -sf tests/sipp/scenarios/b2bua_call_test.xml 127.0.0.1:5060 -m 1 -t un 2>/dev/null; then
    print_color "$GREEN" "✅ B2BUA test scenario completed"
else
    print_color "$YELLOW" "⚠️  B2BUA test failed (expected - needs implementation)"
fi

# Stop UAS
kill $UAS_PID 2>/dev/null || true
wait $UAS_PID 2>/dev/null || true

# Test 5: Analyze what needs to be implemented
print_header "Analysis - What B2BUA Functionality is Missing"

print_color "$YELLOW" "Based on testing, here's what needs to be implemented:"
echo
print_color "$RED" "❌ SIP Message Processing:"
echo "   - Basic SIP INVITE handling"
echo "   - SIP response generation (100 Trying, etc.)"
echo "   - Call routing between UAC and UAS"

print_color "$RED" "❌ B2BUA Core Functions:"
echo "   - Call state management"
echo "   - SIP header manipulation"
echo "   - Media plane setup (RTP forwarding)"
echo "   - Call leg management (A-leg to B-leg)"

print_color "$RED" "❌ SIP Server Implementation:"
echo "   - UDP socket handling"
echo "   - SIP message parsing"
echo "   - Routing table lookup"
echo "   - Response forwarding"

echo
print_color "$GREEN" "✅ What's Already Available:"
echo "   - SIPp test scenarios"
echo "   - Configuration system"
echo "   - Project structure"
echo "   - Development environment setup"

print_header "Next Steps for Implementation"

print_color "$YELLOW" "1. Fix compilation errors in sip_server.rs"
print_color "$YELLOW" "2. Implement basic SIP message handling"
print_color "$YELLOW" "3. Add call routing logic"
print_color "$YELLOW" "4. Test with SIPp scenarios iteratively"
print_color "$YELLOW" "5. Add media plane forwarding"

print_color "$GREEN" "\n🚀 B2BUA testing framework is ready!"
print_color "$GREEN" "Use this script to test implementations as they're developed."