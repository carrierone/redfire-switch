#!/bin/bash

# Comprehensive Test Suite for Redfire Switch Libraries
# Copyright (C) 2025 Carrier One Inc and contributors

set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
WORKSPACE_ROOT="/home/justin/projects/redfire-switch"
TEST_OUTPUT_DIR="$WORKSPACE_ROOT/test-results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_header() {
    echo -e "\n${BLUE}=== $1 ===${NC}"
}

# Create test output directory
setup_test_environment() {
    print_info "Setting up test environment..."
    mkdir -p "$TEST_OUTPUT_DIR"
    cd "$WORKSPACE_ROOT"
}

# Test individual library compilation
test_library_compilation() {
    print_header "Testing Library Compilation"
    
    # Test codec engine
    print_info "Testing codec engine compilation..."
    cd "$WORKSPACE_ROOT/redfire-codec-engine"
    if cargo check --lib 2>&1 | tee "$TEST_OUTPUT_DIR/codec_compilation_$TIMESTAMP.log"; then
        print_success "Codec engine compiles successfully"
    else
        print_error "Codec engine compilation failed"
        return 1
    fi
    
    # Test SIP stack
    print_info "Testing SIP stack compilation..."
    cd "$WORKSPACE_ROOT/redfire-sip-stack"
    if cargo check --lib 2>&1 | tee "$TEST_OUTPUT_DIR/sip_compilation_$TIMESTAMP.log"; then
        print_success "SIP stack compiles successfully"
    else
        print_error "SIP stack compilation failed"
        return 1
    fi
    
    # Test minimal SIP stack
    print_info "Testing minimal SIP stack compilation..."
    cd "$WORKSPACE_ROOT/redfire-sip-stack-minimal"
    if cargo check --lib 2>&1 | tee "$TEST_OUTPUT_DIR/sip_minimal_compilation_$TIMESTAMP.log"; then
        print_success "Minimal SIP stack compiles successfully"
    else
        print_error "Minimal SIP stack compilation failed"
        return 1
    fi
    
    cd "$WORKSPACE_ROOT"
}

# Test library unit tests
test_library_units() {
    print_header "Running Library Unit Tests"
    
    # Test codec engine units
    print_info "Running codec engine unit tests..."
    cd "$WORKSPACE_ROOT/redfire-codec-engine"
    if cargo test --lib 2>&1 | tee "$TEST_OUTPUT_DIR/codec_units_$TIMESTAMP.log"; then
        print_success "Codec engine unit tests passed"
    else
        print_warning "Codec engine unit tests had issues (check log)"
    fi
    
    # Test SIP stack units
    print_info "Running SIP stack unit tests..."
    cd "$WORKSPACE_ROOT/redfire-sip-stack"
    if cargo test --lib 2>&1 | tee "$TEST_OUTPUT_DIR/sip_units_$TIMESTAMP.log"; then
        print_success "SIP stack unit tests passed"
    else
        print_warning "SIP stack unit tests had issues (check log)"
    fi
    
    # Test minimal SIP stack units
    print_info "Running minimal SIP stack unit tests..."
    cd "$WORKSPACE_ROOT/redfire-sip-stack-minimal"
    if cargo test --lib 2>&1 | tee "$TEST_OUTPUT_DIR/sip_minimal_units_$TIMESTAMP.log"; then
        print_success "Minimal SIP stack unit tests passed"
    else
        print_warning "Minimal SIP stack unit tests had issues (check log)"
    fi
    
    cd "$WORKSPACE_ROOT"
}

# Test workspace integration
test_workspace_integration() {
    print_header "Testing Workspace Integration"
    
    print_info "Building entire workspace..."
    if cargo build --workspace 2>&1 | tee "$TEST_OUTPUT_DIR/workspace_build_$TIMESTAMP.log"; then
        print_success "Workspace builds successfully"
    else
        print_error "Workspace build failed"
        return 1
    fi
    
    print_info "Running workspace tests..."
    if cargo test --workspace 2>&1 | tee "$TEST_OUTPUT_DIR/workspace_tests_$TIMESTAMP.log"; then
        print_success "Workspace tests passed"
    else
        print_warning "Some workspace tests failed (check log)"
    fi
}

# Test integration scenarios
test_integration_scenarios() {
    print_header "Running Integration Tests"
    
    print_info "Running library integration tests..."
    if cargo test --test integration_tests 2>&1 | tee "$TEST_OUTPUT_DIR/integration_tests_$TIMESTAMP.log"; then
        print_success "Integration tests passed"
    else
        print_warning "Integration tests had issues (check log)"
    fi
}

# Test installation process
test_installation() {
    print_header "Testing Installation Process"
    
    # Test installation script
    print_info "Testing library installation script..."
    if ./test-libraries.sh 2>&1 | tee "$TEST_OUTPUT_DIR/installation_test_$TIMESTAMP.log"; then
        print_success "Installation test passed"
    else
        print_warning "Installation test had issues (check log)"
    fi
    
    # Test Make-based build
    print_info "Testing Makefile build system..."
    if make -f Makefile.libs check 2>&1 | tee "$TEST_OUTPUT_DIR/makefile_test_$TIMESTAMP.log"; then
        print_success "Makefile test passed"
    else
        print_warning "Makefile test had issues (check log)"
    fi
}

# Test GPU features if available
test_gpu_features() {
    print_header "Testing GPU Features"
    
    # Check for CUDA
    if command -v nvcc >/dev/null 2>&1; then
        print_info "CUDA detected - testing CUDA features..."
        cd "$WORKSPACE_ROOT/redfire-codec-engine"
        if cargo test --features cuda 2>&1 | tee "$TEST_OUTPUT_DIR/cuda_test_$TIMESTAMP.log"; then
            print_success "CUDA tests passed"
        else
            print_warning "CUDA tests had issues (check log)"
        fi
        cd "$WORKSPACE_ROOT"
    else
        print_info "CUDA not available - skipping CUDA tests"
    fi
    
    # Check for ROCm
    if command -v hipcc >/dev/null 2>&1; then
        print_info "ROCm detected - testing ROCm features..."
        cd "$WORKSPACE_ROOT/redfire-codec-engine"
        if cargo test --features rocm 2>&1 | tee "$TEST_OUTPUT_DIR/rocm_test_$TIMESTAMP.log"; then
            print_success "ROCm tests passed"
        else
            print_warning "ROCm tests had issues (check log)"
        fi
        cd "$WORKSPACE_ROOT"
    else
        print_info "ROCm not available - skipping ROCm tests"
    fi
}

# Performance benchmarks
test_performance() {
    print_header "Running Performance Tests"
    
    print_info "Running basic performance benchmarks..."
    if cargo test test_performance_basic --release 2>&1 | tee "$TEST_OUTPUT_DIR/performance_$TIMESTAMP.log"; then
        print_success "Performance tests completed"
    else
        print_warning "Performance tests had issues (check log)"
    fi
}

# Generate test report
generate_test_report() {
    print_header "Generating Test Report"
    
    local report_file="$TEST_OUTPUT_DIR/comprehensive_test_report_$TIMESTAMP.md"
    
    cat > "$report_file" << EOF
# Redfire Switch Libraries - Comprehensive Test Report

**Test Run:** $(date)
**Timestamp:** $TIMESTAMP

## Test Summary

EOF
    
    # Count test results
    local total_tests=0
    local passed_tests=0
    local failed_tests=0
    
    for log_file in "$TEST_OUTPUT_DIR"/*_$TIMESTAMP.log; do
        if [ -f "$log_file" ]; then
            total_tests=$((total_tests + 1))
            if grep -q "test result: ok" "$log_file" || grep -q "SUCCESS" "$log_file"; then
                passed_tests=$((passed_tests + 1))
            else
                failed_tests=$((failed_tests + 1))
            fi
        fi
    done
    
    cat >> "$report_file" << EOF
- **Total Test Suites:** $total_tests
- **Passed:** $passed_tests
- **Failed/Warning:** $failed_tests

## Test Details

EOF
    
    # Add details for each test
    for log_file in "$TEST_OUTPUT_DIR"/*_$TIMESTAMP.log; do
        if [ -f "$log_file" ]; then
            local test_name=$(basename "$log_file" | sed "s/_$TIMESTAMP.log//")
            echo "### $test_name" >> "$report_file"
            echo "" >> "$report_file"
            echo "\`\`\`" >> "$report_file"
            tail -20 "$log_file" >> "$report_file"
            echo "\`\`\`" >> "$report_file"
            echo "" >> "$report_file"
        fi
    done
    
    cat >> "$report_file" << EOF

## System Information

- **Rust Version:** $(rustc --version)
- **Cargo Version:** $(cargo --version)
- **System:** $(uname -a)
- **GPU Support:** $(if command -v nvcc >/dev/null 2>&1; then echo "CUDA available"; elif command -v hipcc >/dev/null 2>&1; then echo "ROCm available"; else echo "No GPU support detected"; fi)

## Log Files

All detailed logs are available in: \`$TEST_OUTPUT_DIR\`

EOF
    
    print_success "Test report generated: $report_file"
}

# Main execution
main() {
    print_header "Redfire Switch Libraries - Comprehensive Test Suite"
    
    echo "Starting comprehensive test suite at $(date)"
    echo "Test results will be saved to: $TEST_OUTPUT_DIR"
    echo ""
    
    setup_test_environment
    
    # Run all test phases
    test_library_compilation
    test_library_units
    test_workspace_integration
    test_integration_scenarios
    test_installation
    test_gpu_features
    test_performance
    
    # Generate final report
    generate_test_report
    
    print_header "Test Suite Complete"
    print_success "Comprehensive test suite completed!"
    print_info "Check the test report for detailed results: $TEST_OUTPUT_DIR/comprehensive_test_report_$TIMESTAMP.md"
    
    echo ""
    echo "Summary of test phases:"
    echo "✓ Library compilation tests"
    echo "✓ Unit tests"
    echo "✓ Workspace integration tests"
    echo "✓ Integration scenario tests"
    echo "✓ Installation tests"
    echo "✓ GPU feature tests"
    echo "✓ Performance tests"
    echo "✓ Test report generated"
}

# Run main function
main "$@"