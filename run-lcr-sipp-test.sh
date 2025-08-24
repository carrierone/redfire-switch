#!/bin/bash

# LCR SIPp Test Script
# Tests call routing from ANI 17028880001 to DNIS 18002255288
# Target: 173.193.144.207:5060

set -e

# Configuration
REDFIRE_IP="localhost"        # Assuming Redfire Switch runs locally
REDFIRE_PORT="5060"          # Standard SIP port
EGRESS_TARGET_IP="173.193.144.207"
EGRESS_TARGET_PORT="5060"
SCENARIO_FILE="tests/sipp/scenarios/lcr_toll_free_test.xml"
LOG_DIR="tests/sipp/logs"
TEST_NAME="lcr_toll_free_$(date +%Y%m%d_%H%M%S)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🧪 LCR SIPp Test: ANI 17028880001 → DNIS 18002255288${NC}"
echo "=================================================="
echo -e "${YELLOW}📞 Call Details:${NC}"
echo "  ANI (Caller):    17028880001 (Las Vegas, NV)"
echo "  DNIS (Called):   18002255288 (Toll-Free)"
echo "  Expected Route:  Redfire Switch → $EGRESS_TARGET_IP:$EGRESS_TARGET_PORT"
echo "  Jurisdiction:    Indeterminate (due to toll-free DNIS)"
echo ""

# Create log directory
mkdir -p "$LOG_DIR"

# Check if SIPp is installed
if ! command -v sipp &> /dev/null; then
    echo -e "${RED}❌ Error: SIPp is not installed or not in PATH${NC}"
    echo "Please install SIPp: sudo apt-get install sipp"
    exit 1
fi

# Check if scenario file exists
if [ ! -f "$SCENARIO_FILE" ]; then
    echo -e "${RED}❌ Error: Scenario file not found: $SCENARIO_FILE${NC}"
    exit 1
fi

echo -e "${YELLOW}🔧 Pre-Test Checks:${NC}"

# Check if Redfire Switch is running
echo -n "  Checking Redfire Switch ($REDFIRE_IP:$REDFIRE_PORT)... "
if timeout 3 bash -c "</dev/tcp/$REDFIRE_IP/$REDFIRE_PORT" 2>/dev/null; then
    echo -e "${GREEN}✅ Running${NC}"
else
    echo -e "${RED}❌ Not responding${NC}"
    echo "  Please start Redfire Switch first"
    exit 1
fi

# Check if target is reachable
echo -n "  Checking egress target ($EGRESS_TARGET_IP:$EGRESS_TARGET_PORT)... "
if timeout 3 bash -c "</dev/tcp/$EGRESS_TARGET_IP/$EGRESS_TARGET_PORT" 2>/dev/null; then
    echo -e "${GREEN}✅ Reachable${NC}"
else
    echo -e "${YELLOW}⚠️  Not directly reachable (normal if behind firewall)${NC}"
fi

echo ""
echo -e "${YELLOW}🚀 Starting LCR Test...${NC}"

# Run SIPp test
SIPP_CMD="sipp -sf $SCENARIO_FILE \
    -i $REDFIRE_IP \
    -p 5061 \
    -r 1 \
    -l 1 \
    -m 5 \
    -d 8000 \
    -t u1 \
    -trace_logs \
    -trace_err \
    -log_file $LOG_DIR/${TEST_NAME}_sipp.log \
    -error_file $LOG_DIR/${TEST_NAME}_error.log \
    -message_file $LOG_DIR/${TEST_NAME}_messages.log \
    $REDFIRE_IP:$REDFIRE_PORT"

echo "Command: $SIPP_CMD"
echo ""

# Execute SIPp test
if $SIPP_CMD; then
    echo ""
    echo -e "${GREEN}✅ SIPp Test Completed Successfully!${NC}"
    echo ""
    echo -e "${YELLOW}📊 Test Results Summary:${NC}"
    
    # Show basic statistics if available
    if [ -f "$LOG_DIR/${TEST_NAME}_sipp.log" ]; then
        echo "  📄 Detailed logs: $LOG_DIR/${TEST_NAME}_sipp.log"
        
        # Extract key statistics
        echo ""
        echo -e "${BLUE}📈 Call Statistics:${NC}"
        grep -E "(Total calls|Successful calls|Failed calls|Call rate|Response time)" \
            "$LOG_DIR/${TEST_NAME}_sipp.log" 2>/dev/null || echo "  Statistics parsing in progress..."
    fi
    
    echo ""
    echo -e "${YELLOW}🔍 Expected LCR Behavior Analysis:${NC}"
    echo "  1. ✅ ANI 17028880001 should be identified as Las Vegas, NV"
    echo "  2. ✅ DNIS 18002255288 should be identified as toll-free"
    echo "  3. ✅ Jurisdiction should be Indeterminate (toll-free override)"
    echo "  4. ✅ Should route via configured toll-free egress trunk"
    echo "  5. ✅ Should apply toll-free rates (typically no cost to caller)"
    
    echo ""
    echo -e "${BLUE}💡 Next Steps:${NC}"
    echo "  • Check Redfire Switch logs for LCR routing decisions"
    echo "  • Verify call was routed to correct egress trunk"
    echo "  • Confirm jurisdiction determination was correct"
    echo "  • Review billing records for proper rate application"
    
else
    echo ""
    echo -e "${RED}❌ SIPp Test Failed!${NC}"
    echo ""
    echo -e "${YELLOW}🔍 Troubleshooting:${NC}"
    echo "  1. Check Redfire Switch configuration and logs"
    echo "  2. Verify ingress trunk is configured to accept calls"
    echo "  3. Confirm egress trunk routing to $EGRESS_TARGET_IP:$EGRESS_TARGET_PORT"
    echo "  4. Check LCR rate tables for toll-free routing"
    echo "  5. Review error logs: $LOG_DIR/${TEST_NAME}_error.log"
    
    if [ -f "$LOG_DIR/${TEST_NAME}_error.log" ]; then
        echo ""
        echo -e "${RED}🚨 Error Log Preview:${NC}"
        tail -10 "$LOG_DIR/${TEST_NAME}_error.log" 2>/dev/null || echo "No error log available"
    fi
    
    exit 1
fi

echo ""
echo -e "${GREEN}🎉 LCR Test Complete!${NC}"
echo "Log files saved in: $LOG_DIR/"