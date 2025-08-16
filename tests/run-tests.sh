#!/bin/bash

# Redfire Switch SIPp Test Runner
# This script runs various SIP tests against the Redfire Switch

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
SWITCH_HOST=${SWITCH_HOST:-localhost}
SWITCH_PORT=${SWITCH_PORT:-5060}
SIPP_PATH=${SIPP_PATH:-sipp}
SCENARIOS_DIR="./sipp/scenarios"
LOGS_DIR="./sipp/logs"
RESULTS_DIR="./results"

# Test configuration
CALL_RATE=${CALL_RATE:-1}       # Calls per second
CALL_LIMIT=${CALL_LIMIT:-1}     # Max simultaneous calls
NUM_CALLS=${NUM_CALLS:-10}      # Total number of calls
TIMEOUT=${TIMEOUT:-30}          # Test timeout in seconds

# Create directories
mkdir -p "$LOGS_DIR" "$RESULTS_DIR"

# Function to print colored output
print_color() {
    local color=$1
    local message=$2
    echo -e "${color}${message}${NC}"
}

# Function to run a single test
run_test() {
    local test_name=$1
    local scenario_file=$2
    local extra_args=$3
    
    print_color "$YELLOW" "\n🧪 Running test: $test_name"
    print_color "$YELLOW" "   Scenario: $scenario_file"
    
    local log_file="$LOGS_DIR/${test_name}_$(date +%Y%m%d_%H%M%S).log"
    local stats_file="$RESULTS_DIR/${test_name}_stats.csv"
    
    # Build SIPp command
    local cmd="$SIPP_PATH $SWITCH_HOST:$SWITCH_PORT"
    cmd="$cmd -sf $scenario_file"
    cmd="$cmd -r $CALL_RATE"
    cmd="$cmd -l $CALL_LIMIT"
    cmd="$cmd -m $NUM_CALLS"
    cmd="$cmd -timeout ${TIMEOUT}s"
    cmd="$cmd -timeout_error"
    cmd="$cmd -trace_msg"
    cmd="$cmd -message_file $log_file"
    cmd="$cmd -trace_stat"
    cmd="$cmd -stf $stats_file"
    cmd="$cmd $extra_args"
    
    print_color "$YELLOW" "   Command: $cmd"
    
    # Run the test
    if eval $cmd; then
        print_color "$GREEN" "✅ Test $test_name PASSED"
        return 0
    else
        print_color "$RED" "❌ Test $test_name FAILED"
        return 1
    fi
}

# Function to run all tests
run_all_tests() {
    local failed_tests=()
    local passed_tests=()
    
    print_color "$GREEN" "=========================================="
    print_color "$GREEN" "    Redfire Switch SIPp Test Suite"
    print_color "$GREEN" "=========================================="
    print_color "$YELLOW" "Target: $SWITCH_HOST:$SWITCH_PORT"
    print_color "$YELLOW" "Call Rate: $CALL_RATE cps"
    print_color "$YELLOW" "Total Calls: $NUM_CALLS"
    print_color "$GREEN" "=========================================="
    
    # Test 1: OPTIONS Ping
    if run_test "options_ping" "$SCENARIOS_DIR/options_ping.xml" "-m 5"; then
        passed_tests+=("options_ping")
    else
        failed_tests+=("options_ping")
    fi
    
    # Test 2: Basic Call Flow
    if run_test "basic_call" "$SCENARIOS_DIR/basic_call_uac.xml" "-m 5"; then
        passed_tests+=("basic_call")
    else
        failed_tests+=("basic_call")
    fi
    
    # Test 3: Registration
    if run_test "registration" "$SCENARIOS_DIR/register_test.xml" "-m 3"; then
        passed_tests+=("registration")
    else
        failed_tests+=("registration")
    fi
    
    # Test 4: Stress Test (optional)
    if [ "$RUN_STRESS_TEST" = "true" ]; then
        if run_test "stress_test" "$SCENARIOS_DIR/stress_test.xml" "-r 10 -m 100"; then
            passed_tests+=("stress_test")
        else
            failed_tests+=("stress_test")
        fi
    fi
    
    # Print summary
    print_color "$GREEN" "\n=========================================="
    print_color "$GREEN" "              Test Summary"
    print_color "$GREEN" "=========================================="
    
    if [ ${#passed_tests[@]} -gt 0 ]; then
        print_color "$GREEN" "✅ Passed Tests (${#passed_tests[@]}):"
        for test in "${passed_tests[@]}"; do
            print_color "$GREEN" "   - $test"
        done
    fi
    
    if [ ${#failed_tests[@]} -gt 0 ]; then
        print_color "$RED" "❌ Failed Tests (${#failed_tests[@]}):"
        for test in "${failed_tests[@]}"; do
            print_color "$RED" "   - $test"
        done
        return 1
    else
        print_color "$GREEN" "\n🎉 All tests passed successfully!"
        return 0
    fi
}

# Function to run a single scenario
run_single_scenario() {
    local scenario=$1
    shift
    local extra_args="$@"
    
    if [ ! -f "$SCENARIOS_DIR/$scenario" ]; then
        print_color "$RED" "Error: Scenario file not found: $SCENARIOS_DIR/$scenario"
        exit 1
    fi
    
    run_test "$(basename $scenario .xml)" "$SCENARIOS_DIR/$scenario" "$extra_args"
}

# Function to start monitoring
start_monitoring() {
    print_color "$YELLOW" "Starting packet capture..."
    tcpdump -i any -w "$RESULTS_DIR/capture_$(date +%Y%m%d_%H%M%S).pcap" port 5060 &
    TCPDUMP_PID=$!
    print_color "$GREEN" "Packet capture started (PID: $TCPDUMP_PID)"
}

# Function to stop monitoring
stop_monitoring() {
    if [ ! -z "$TCPDUMP_PID" ]; then
        print_color "$YELLOW" "Stopping packet capture..."
        kill $TCPDUMP_PID 2>/dev/null || true
        print_color "$GREEN" "Packet capture stopped"
    fi
}

# Cleanup function
cleanup() {
    stop_monitoring
    print_color "$YELLOW" "Cleanup completed"
}

# Set trap for cleanup
trap cleanup EXIT

# Main script logic
case "${1:-all}" in
    all)
        run_all_tests
        ;;
    single)
        if [ -z "$2" ]; then
            print_color "$RED" "Error: Please specify a scenario file"
            echo "Usage: $0 single <scenario_file> [extra_sipp_args]"
            exit 1
        fi
        shift
        run_single_scenario "$@"
        ;;
    monitor)
        start_monitoring
        shift
        run_all_tests
        ;;
    stress)
        RUN_STRESS_TEST=true
        CALL_RATE=10
        NUM_CALLS=100
        run_all_tests
        ;;
    *)
        print_color "$YELLOW" "Usage: $0 [all|single|monitor|stress]"
        echo "  all     - Run all standard tests"
        echo "  single  - Run a single scenario"
        echo "  monitor - Run tests with packet capture"
        echo "  stress  - Run stress tests"
        exit 1
        ;;
esac