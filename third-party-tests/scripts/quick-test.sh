#!/bin/bash

# Redfire Switch - Quick Interoperability Test
# Runs a minimal set of tests to verify basic functionality

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
REDFIRE_IP=${REDFIRE_IP:-127.0.0.1}
REDFIRE_PORT=${REDFIRE_PORT:-5060}

log() {
    echo -e "${BLUE}[$(date +'%H:%M:%S')] $1${NC}"
}

success() {
    echo -e "${GREEN}✓ $1${NC}"
}

error() {
    echo -e "${RED}✗ $1${NC}"
}

warn() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

# Test basic connectivity
test_connectivity() {
    log "Testing basic connectivity to $REDFIRE_IP:$REDFIRE_PORT"
    
    # UDP test
    if timeout 3 bash -c "</dev/udp/$REDFIRE_IP/$REDFIRE_PORT" 2>/dev/null; then
        success "UDP connectivity - OK"
    else
        error "UDP connectivity - FAILED"
        return 1
    fi
    
    # TCP test
    if timeout 3 bash -c "</dev/tcp/$REDFIRE_IP/$REDFIRE_PORT" 2>/dev/null; then
        success "TCP connectivity - OK"
    else
        warn "TCP connectivity - FAILED (may not be enabled)"
    fi
}

# Send SIP OPTIONS request
test_sip_options() {
    log "Testing SIP OPTIONS request"
    
    local options_msg="OPTIONS sip:$REDFIRE_IP:$REDFIRE_PORT SIP/2.0\r
Via: SIP/2.0/UDP 127.0.0.1:5070;branch=z9hG4bK-test-$(date +%s)\r
Max-Forwards: 70\r
To: <sip:$REDFIRE_IP:$REDFIRE_PORT>\r
From: <sip:quicktest@127.0.0.1:5070>;tag=quicktest-$(date +%s)\r
Call-ID: quicktest-$(date +%s)@127.0.0.1\r
CSeq: 1 OPTIONS\r
Content-Length: 0\r
\r
"

    local response=$(echo -e "$options_msg" | timeout 5 nc -u "$REDFIRE_IP" "$REDFIRE_PORT" 2>/dev/null | head -1)
    
    if [[ "$response" =~ "SIP/2.0 200" ]]; then
        success "SIP OPTIONS - Got 200 OK response"
    elif [[ "$response" =~ "SIP/2.0" ]]; then
        warn "SIP OPTIONS - Got SIP response: $response"
    else
        error "SIP OPTIONS - No valid SIP response"
        return 1
    fi
}

# Test basic INVITE (without expecting answer)
test_sip_invite() {
    log "Testing basic SIP INVITE"
    
    local call_id="quicktest-invite-$(date +%s)"
    local invite_msg="INVITE sip:999@$REDFIRE_IP:$REDFIRE_PORT SIP/2.0\r
Via: SIP/2.0/UDP 127.0.0.1:5070;branch=z9hG4bK-invite-$(date +%s)\r
Max-Forwards: 70\r
To: <sip:999@$REDFIRE_IP:$REDFIRE_PORT>\r
From: <sip:quicktest@127.0.0.1:5070>;tag=quicktest-$(date +%s)\r
Call-ID: $call_id\r
CSeq: 1 INVITE\r
Contact: <sip:quicktest@127.0.0.1:5070>\r
Content-Type: application/sdp\r
Content-Length: 195\r
\r
v=0\r
o=quicktest 12345 12345 IN IP4 127.0.0.1\r
s=-\r
c=IN IP4 127.0.0.1\r
t=0 0\r
m=audio 12000 RTP/AVP 0 8\r
a=rtpmap:0 PCMU/8000\r
a=rtpmap:8 PCMA/8000\r
a=sendrecv\r
"

    local response=$(echo -e "$invite_msg" | timeout 5 nc -u "$REDFIRE_IP" "$REDFIRE_PORT" 2>/dev/null | head -1)
    
    if [[ "$response" =~ "SIP/2.0 100" ]] || [[ "$response" =~ "SIP/2.0 180" ]] || [[ "$response" =~ "SIP/2.0 200" ]]; then
        success "SIP INVITE - Got positive response: $(echo "$response" | tr -d '\r\n')"
        
        # Send CANCEL to clean up
        local cancel_msg="CANCEL sip:999@$REDFIRE_IP:$REDFIRE_PORT SIP/2.0\r
Via: SIP/2.0/UDP 127.0.0.1:5070;branch=z9hG4bK-invite-$(date +%s)\r
Max-Forwards: 70\r
To: <sip:999@$REDFIRE_IP:$REDFIRE_PORT>\r
From: <sip:quicktest@127.0.0.1:5070>;tag=quicktest-$(date +%s)\r
Call-ID: $call_id\r
CSeq: 1 CANCEL\r
Content-Length: 0\r
\r
"
        echo -e "$cancel_msg" | timeout 2 nc -u "$REDFIRE_IP" "$REDFIRE_PORT" >/dev/null 2>&1
        
    elif [[ "$response" =~ "SIP/2.0 4" ]] || [[ "$response" =~ "SIP/2.0 5" ]]; then
        warn "SIP INVITE - Got error response: $(echo "$response" | tr -d '\r\n')"
    else
        error "SIP INVITE - No valid SIP response"
        return 1
    fi
}

# Check if required tools are available
check_tools() {
    log "Checking required tools..."
    
    if ! command -v nc >/dev/null 2>&1; then
        error "netcat (nc) not found - required for testing"
        echo "Install with: sudo apt-get install netcat-openbsd"
        return 1
    fi
    
    success "Required tools available"
}

# Main test function
main() {
    echo
    log "=================================================="
    log "  Redfire Switch - Quick Interoperability Test"
    log "=================================================="
    echo
    log "Target: $REDFIRE_IP:$REDFIRE_PORT"
    echo
    
    local tests_passed=0
    local total_tests=4
    
    # Run tests
    if check_tools; then
        ((tests_passed++))
    fi
    
    if test_connectivity; then
        ((tests_passed++))
    fi
    
    if test_sip_options; then
        ((tests_passed++))
    fi
    
    if test_sip_invite; then
        ((tests_passed++))
    fi
    
    echo
    log "=================================================="
    log "  Quick Test Results"
    log "=================================================="
    
    if [[ $tests_passed -eq $total_tests ]]; then
        success "All tests passed! ($tests_passed/$total_tests)"
        echo
        log "Redfire Switch appears to be working correctly"
        log "Run full test suite with: ./run-interop-tests.sh"
        exit 0
    else
        error "Some tests failed: $tests_passed/$total_tests passed"
        echo
        log "Possible issues:"
        echo "  - Redfire Switch not running"
        echo "  - Configuration problems"
        echo "  - Network connectivity issues"
        echo
        log "Next steps:"
        echo "  1. Check if Redfire Switch is running: ps aux | grep redfire"
        echo "  2. Check configuration: cd .. && cargo run -- validate-config"
        echo "  3. Check logs for errors"
        echo "  4. Run full test suite: ./run-interop-tests.sh"
        exit 1
    fi
}

# Help message
if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    echo "Redfire Switch - Quick Interoperability Test"
    echo
    echo "Usage: $0 [options]"
    echo
    echo "Environment Variables:"
    echo "  REDFIRE_IP    IP address of Redfire Switch (default: 127.0.0.1)"
    echo "  REDFIRE_PORT  Port of Redfire Switch (default: 5060)"
    echo
    echo "This script performs basic connectivity and protocol tests:"
    echo "  ✓ TCP/UDP connectivity"
    echo "  ✓ SIP OPTIONS request"
    echo "  ✓ Basic SIP INVITE"
    echo
    echo "For comprehensive testing, use: ./run-interop-tests.sh"
    exit 0
fi

# Run main function
main "$@"