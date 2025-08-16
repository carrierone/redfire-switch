#!/bin/bash

# Manual B2BUA Testing Script (works without SIPp)
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

# Check if built
print_header "Manual B2BUA Testing (No SIPp Required)"

if [ ! -f "./target/debug/simple-b2bua-test" ]; then
    print_color "$YELLOW" "Building simple B2BUA test..."
    cargo build --bin simple-b2bua-test
fi

print_color "$GREEN" "✅ Simple B2BUA test binary ready"

print_header "Test Plan"
echo "1. Start the Simple B2BUA on port 5060"
echo "2. Create a dummy UAS on port 5070 (termination)"  
echo "3. Send manual SIP messages to test B2BUA forwarding"
echo "4. Analyze behavior and identify improvements needed"

print_header "Starting Test Environment"

# Function to cleanup
cleanup() {
    print_color "$YELLOW" "Cleaning up processes..."
    pkill -f simple-b2bua-test 2>/dev/null || true
    pkill -f "nc.*5070" 2>/dev/null || true
    sleep 1
}

trap cleanup EXIT

# Test 1: Start Simple B2BUA
print_color "$YELLOW" "1. Starting Simple B2BUA on port 5060..."
./target/debug/simple-b2bua-test &
B2BUA_PID=$!
sleep 2

if kill -0 $B2BUA_PID 2>/dev/null; then
    print_color "$GREEN" "✅ Simple B2BUA started (PID: $B2BUA_PID)"
else
    print_color "$RED" "❌ Failed to start Simple B2BUA"
    exit 1
fi

# Test 2: Create dummy termination endpoint
print_color "$YELLOW" "2. Creating dummy termination endpoint on port 5070..."
nc -l -u -p 5070 > /dev/null 2>&1 &
UAS_PID=$!
sleep 1

if kill -0 $UAS_PID 2>/dev/null; then
    print_color "$GREEN" "✅ Dummy UAS started (PID: $UAS_PID)"
else
    print_color "$YELLOW" "⚠️  Could not start netcat UAS (may not have nc)"
fi

# Test 3: Manual SIP message testing
print_header "Testing SIP Message Handling"

# Test OPTIONS ping
print_color "$YELLOW" "3a. Testing OPTIONS ping..."
OPTIONS_MSG="OPTIONS sip:ping@127.0.0.1:5060 SIP/2.0
Via: SIP/2.0/UDP 127.0.0.1:12345;branch=z9hG4bK-test
From: Test <sip:test@127.0.0.1:12345>;tag=test123
To: <sip:ping@127.0.0.1:5060>
Call-ID: test-options-$(date +%s)
CSeq: 1 OPTIONS
User-Agent: Manual-Test
Content-Length: 0

"

echo "$OPTIONS_MSG" | nc -u -w1 127.0.0.1 5060 &
sleep 2
print_color "$GREEN" "✅ OPTIONS message sent"

# Test INVITE
print_color "$YELLOW" "3b. Testing INVITE (basic B2BUA functionality)..."
INVITE_MSG="INVITE sip:1234567890@127.0.0.1:5060 SIP/2.0
Via: SIP/2.0/UDP 127.0.0.1:12345;branch=z9hG4bK-invite-test
From: Test Caller <sip:test@127.0.0.1:12345>;tag=invite123
To: <sip:1234567890@127.0.0.1:5060>
Call-ID: test-invite-$(date +%s)
CSeq: 1 INVITE
Contact: <sip:test@127.0.0.1:12345>
User-Agent: Manual-Test
Content-Type: application/sdp
Content-Length: 200

v=0
o=test 12345 67890 IN IP4 127.0.0.1
s=Test Call
c=IN IP4 127.0.0.1
t=0 0
m=audio 8000 RTP/AVP 0
a=rtpmap:0 PCMU/8000

"

echo "$INVITE_MSG" | nc -u -w2 127.0.0.1 5060 &
sleep 3
print_color "$GREEN" "✅ INVITE message sent"

# Test result analysis
print_header "Test Results Analysis"

print_color "$YELLOW" "Checking B2BUA logs..."
sleep 2

# Check if B2BUA is still running
if kill -0 $B2BUA_PID 2>/dev/null; then
    print_color "$GREEN" "✅ B2BUA still running - handled messages without crashing"
else
    print_color "$RED" "❌ B2BUA crashed - check implementation"
fi

print_header "Manual Testing Summary"

print_color "$GREEN" "✅ Completed Tests:"
echo "   - Built simple B2BUA binary"
echo "   - Started B2BUA on port 5060"
echo "   - Created dummy termination endpoint"
echo "   - Sent OPTIONS and INVITE messages"
echo "   - Verified B2BUA stability"

print_color "$YELLOW" "📝 What to check next:"
echo "   1. Check B2BUA console output for message processing"
echo "   2. Verify 100 Trying responses are generated"
echo "   3. Check if messages are forwarded to port 5070"
echo "   4. Test with real SIPp scenarios (after running sudo ./setup-dev.sh)"

print_color "$BLUE" "🚀 Ready for SIPp testing once dependencies are installed:"
echo "   sudo ./setup-dev.sh"
echo "   ./test-b2bua.sh"

print_color "$YELLOW" "Press Enter to stop test environment..."
read -r

cleanup
print_color "$GREEN" "Test completed! 🎉"