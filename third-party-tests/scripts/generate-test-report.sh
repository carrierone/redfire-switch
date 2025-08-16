#!/bin/bash

# Redfire Switch - Test Report Generator
# Analyzes test results and generates comprehensive reports

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
LOGS_DIR="logs"
REPORTS_DIR="logs/reports"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

log() {
    echo -e "${BLUE}[$(date +'%H:%M:%S')] $1${NC}"
}

success() {
    echo -e "${GREEN}[SUCCESS] $1${NC}"
}

error() {
    echo -e "${RED}[ERROR] $1${NC}"
}

warn() {
    echo -e "${YELLOW}[WARNING] $1${NC}"
}

# Create reports directory
setup_reporting() {
    log "Setting up reporting environment..."
    mkdir -p "$REPORTS_DIR"
    mkdir -p "$REPORTS_DIR/assets"
    mkdir -p "$REPORTS_DIR/data"
}

# Find latest test results
find_latest_results() {
    log "Searching for test results..."
    
    # Find the most recent interop test directory
    LATEST_TEST_DIR=$(find "$LOGS_DIR" -maxdepth 1 -type d -name "interop-tests-*" | sort | tail -1)
    
    if [[ -z "$LATEST_TEST_DIR" ]]; then
        error "No test results found. Run interop tests first: ./run-interop-tests.sh"
        exit 1
    fi
    
    log "Found test results in: $LATEST_TEST_DIR"
    
    # Check required files
    RESULTS_JSON="${LATEST_TEST_DIR}/test-results.json"
    TEST_LOG="${LATEST_TEST_DIR}/test.log"
    SUMMARY_FILE="${LATEST_TEST_DIR}/test-summary.txt"
    
    if [[ ! -f "$RESULTS_JSON" ]]; then
        error "Results file not found: $RESULTS_JSON"
        exit 1
    fi
}

# Parse test results
parse_results() {
    log "Parsing test results..."
    
    # Extract basic statistics using grep/awk (fallback if jq not available)
    if command -v jq >/dev/null 2>&1; then
        TOTAL_TESTS=$(jq '.tests | length' "$RESULTS_JSON" 2>/dev/null || echo "0")
        PASSED_TESTS=$(jq '.tests | map(select(.result == "PASS")) | length' "$RESULTS_JSON" 2>/dev/null || echo "0")
        FAILED_TESTS=$(jq '.tests | map(select(.result == "FAIL")) | length' "$RESULTS_JSON" 2>/dev/null || echo "0")
        SKIPPED_TESTS=$(jq '.tests | map(select(.result == "SKIP")) | length' "$RESULTS_JSON" 2>/dev/null || echo "0")
    else
        # Fallback parsing without jq
        TOTAL_TESTS=$(grep -o '"test":' "$RESULTS_JSON" | wc -l)
        PASSED_TESTS=$(grep -o '"result": "PASS"' "$RESULTS_JSON" | wc -l)
        FAILED_TESTS=$(grep -o '"result": "FAIL"' "$RESULTS_JSON" | wc -l)
        SKIPPED_TESTS=$(grep -o '"result": "SKIP"' "$RESULTS_JSON" | wc -l)
    fi
    
    # Calculate success rate
    if [[ $((TOTAL_TESTS - SKIPPED_TESTS)) -gt 0 ]]; then
        SUCCESS_RATE=$(( PASSED_TESTS * 100 / (TOTAL_TESTS - SKIPPED_TESTS) ))
    else
        SUCCESS_RATE=0
    fi
    
    log "Parsed results: $TOTAL_TESTS total, $PASSED_TESTS passed, $FAILED_TESTS failed, $SKIPPED_TESTS skipped"
}

# Generate HTML report
generate_html_report() {
    log "Generating HTML report..."
    
    local html_file="$REPORTS_DIR/interop-report-${TIMESTAMP}.html"
    
    cat > "$html_file" << EOF
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Redfire Switch Interoperability Test Report</title>
    <style>
        body {
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            margin: 0;
            padding: 20px;
            background-color: #f5f5f5;
        }
        .container {
            max-width: 1200px;
            margin: 0 auto;
            background: white;
            border-radius: 8px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            overflow: hidden;
        }
        .header {
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 30px;
            text-align: center;
        }
        .header h1 {
            margin: 0;
            font-size: 2.5em;
        }
        .header p {
            margin: 10px 0 0 0;
            opacity: 0.9;
        }
        .stats {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 20px;
            padding: 30px;
            background: #f8f9fa;
        }
        .stat-card {
            background: white;
            padding: 20px;
            border-radius: 8px;
            text-align: center;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        .stat-number {
            font-size: 2.5em;
            font-weight: bold;
            margin-bottom: 5px;
        }
        .stat-label {
            color: #666;
            font-size: 0.9em;
            text-transform: uppercase;
            letter-spacing: 1px;
        }
        .passed { color: #28a745; }
        .failed { color: #dc3545; }
        .skipped { color: #ffc107; }
        .total { color: #007bff; }
        .content {
            padding: 30px;
        }
        .test-section {
            margin-bottom: 30px;
        }
        .test-section h2 {
            border-bottom: 2px solid #667eea;
            padding-bottom: 10px;
            color: #333;
        }
        .test-item {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 15px;
            margin: 10px 0;
            border-radius: 6px;
            border-left: 4px solid;
        }
        .test-pass {
            background: #d4edda;
            border-color: #28a745;
        }
        .test-fail {
            background: #f8d7da;
            border-color: #dc3545;
        }
        .test-skip {
            background: #fff3cd;
            border-color: #ffc107;
        }
        .test-name {
            font-weight: 500;
        }
        .test-duration {
            color: #666;
            font-size: 0.9em;
        }
        .progress-bar {
            width: 100%;
            height: 20px;
            background: #e9ecef;
            border-radius: 10px;
            overflow: hidden;
            margin: 20px 0;
        }
        .progress-fill {
            height: 100%;
            background: linear-gradient(90deg, #28a745, #20c997);
            transition: width 0.3s ease;
        }
        .footer {
            background: #343a40;
            color: white;
            padding: 20px;
            text-align: center;
        }
        .error-details {
            background: #f8f9fa;
            padding: 15px;
            border-radius: 4px;
            margin-top: 10px;
            font-family: monospace;
            font-size: 0.85em;
            color: #666;
        }
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>Redfire Switch</h1>
            <p>Interoperability Test Report - $(date)</p>
        </div>
        
        <div class="stats">
            <div class="stat-card">
                <div class="stat-number total">$TOTAL_TESTS</div>
                <div class="stat-label">Total Tests</div>
            </div>
            <div class="stat-card">
                <div class="stat-number passed">$PASSED_TESTS</div>
                <div class="stat-label">Passed</div>
            </div>
            <div class="stat-card">
                <div class="stat-number failed">$FAILED_TESTS</div>
                <div class="stat-label">Failed</div>
            </div>
            <div class="stat-card">
                <div class="stat-number skipped">$SKIPPED_TESTS</div>
                <div class="stat-label">Skipped</div>
            </div>
        </div>
        
        <div class="content">
            <div class="test-section">
                <h2>Overall Progress</h2>
                <div class="progress-bar">
                    <div class="progress-fill" style="width: ${SUCCESS_RATE}%"></div>
                </div>
                <p style="text-align: center; color: #666;">
                    Success Rate: ${SUCCESS_RATE}% (${PASSED_TESTS} of $((TOTAL_TESTS - SKIPPED_TESTS)) tests passed)
                </p>
            </div>
            
            <div class="test-section">
                <h2>Test Results</h2>
EOF

    # Add individual test results
    if command -v jq >/dev/null 2>&1; then
        jq -r '.tests[] | "\(.test)|\(.result)|\(.duration)|\(.details // "")"' "$RESULTS_JSON" 2>/dev/null | while IFS='|' read -r test result duration details; do
            local class="test-skip"
            case "$result" in
                "PASS") class="test-pass" ;;
                "FAIL") class="test-fail" ;;
                "SKIP") class="test-skip" ;;
            esac
            
            echo "                <div class=\"test-item $class\">" >> "$html_file"
            echo "                    <div class=\"test-name\">$test</div>" >> "$html_file"
            echo "                    <div class=\"test-duration\">${duration}s</div>" >> "$html_file"
            echo "                </div>" >> "$html_file"
            
            if [[ "$result" == "FAIL" && -n "$details" ]]; then
                echo "                <div class=\"error-details\">$details</div>" >> "$html_file"
            fi
        done
    else
        # Fallback: extract from log file
        grep -E "\\[(PASS|FAIL|SKIP)\\]" "$TEST_LOG" | while read -r line; do
            if [[ $line =~ \[PASS\] ]]; then
                class="test-pass"
                test_name=$(echo "$line" | sed 's/.*\[PASS\] //')
            elif [[ $line =~ \[ERROR\] ]]; then
                class="test-fail"
                test_name=$(echo "$line" | sed 's/.*\[ERROR\] //')
            else
                class="test-skip"
                test_name=$(echo "$line" | sed 's/.*\[SKIP\] //')
            fi
            
            echo "                <div class=\"test-item $class\">" >> "$html_file"
            echo "                    <div class=\"test-name\">$test_name</div>" >> "$html_file"
            echo "                </div>" >> "$html_file"
        done
    fi

    cat >> "$html_file" << EOF
            </div>
            
            <div class="test-section">
                <h2>Test Environment</h2>
                <ul>
                    <li><strong>Target:</strong> $(grep "target" "$RESULTS_JSON" 2>/dev/null | cut -d'"' -f4 || echo "Unknown")</li>
                    <li><strong>Test Date:</strong> $(date)</li>
                    <li><strong>Log Files:</strong> ${LATEST_TEST_DIR}</li>
                </ul>
            </div>
        </div>
        
        <div class="footer">
            <p>Generated by Redfire Switch Test Suite</p>
        </div>
    </div>
</body>
</html>
EOF

    success "HTML report generated: $html_file"
    HTML_REPORT="$html_file"
}

# Generate CSV report for data analysis
generate_csv_report() {
    log "Generating CSV report..."
    
    local csv_file="$REPORTS_DIR/data/test-results-${TIMESTAMP}.csv"
    
    echo "Test Name,Result,Duration (s),Timestamp,Details" > "$csv_file"
    
    if command -v jq >/dev/null 2>&1; then
        jq -r '.tests[] | [.test, .result, .duration, .timestamp, (.details // "")] | @csv' "$RESULTS_JSON" >> "$csv_file" 2>/dev/null
    else
        warn "jq not available, CSV will have limited data"
    fi
    
    success "CSV report generated: $csv_file"
}

# Generate performance metrics
generate_performance_report() {
    log "Analyzing performance metrics..."
    
    local perf_file="$REPORTS_DIR/performance-${TIMESTAMP}.txt"
    
    cat > "$perf_file" << EOF
Redfire Switch Performance Analysis
==================================
Generated: $(date)

Test Execution Metrics:
- Total test duration: $(grep -o "Test Duration: [0-9]* seconds" "$SUMMARY_FILE" 2>/dev/null | grep -o "[0-9]*" || echo "Unknown") seconds
- Average test duration: $(if [[ $TOTAL_TESTS -gt 0 ]]; then echo "scale=2; $(grep -o '"duration": [0-9]*' "$RESULTS_JSON" 2>/dev/null | grep -o '[0-9]*' | awk '{sum+=$1; count++} END {if(count>0) print sum/count; else print 0}')"; else echo "0"; fi | bc 2>/dev/null || echo "Unknown") seconds per test

Success Rate Analysis:
- Overall success rate: ${SUCCESS_RATE}%
- Connectivity tests: $(grep -c "Basic SIP Connectivity.*PASS" "$TEST_LOG" 2>/dev/null || echo "0")/1
- Call flow tests: $(grep -c "Call Flow.*PASS" "$TEST_LOG" 2>/dev/null || echo "0")/1  
- Load tests: $(grep -c "Load Testing.*PASS" "$TEST_LOG" 2>/dev/null || echo "0")/1

Failure Analysis:
EOF

    if [[ $FAILED_TESTS -gt 0 ]]; then
        echo "Failed tests found:" >> "$perf_file"
        grep "ERROR" "$TEST_LOG" 2>/dev/null | sed 's/^/  - /' >> "$perf_file" || echo "  (Could not extract failure details)" >> "$perf_file"
    else
        echo "No test failures detected." >> "$perf_file"
    fi
    
    cat >> "$perf_file" << EOF

Recommendations:
$(if [[ $SUCCESS_RATE -ge 90 ]]; then echo "✓ Excellent test coverage and success rate"; elif [[ $SUCCESS_RATE -ge 75 ]]; then echo "⚠ Good success rate, investigate failed tests"; else echo "⚠ Low success rate, review implementation and test environment"; fi)
$(if [[ $FAILED_TESTS -eq 0 ]]; then echo "✓ No failures detected"; else echo "⚠ Address $FAILED_TESTS failed test(s)"; fi)
$(if [[ $SKIPPED_TESTS -gt $((TOTAL_TESTS / 2)) ]]; then echo "⚠ High number of skipped tests - ensure test environment is properly configured"; else echo "✓ Good test coverage"; fi)
EOF

    success "Performance report generated: $perf_file"
}

# Generate comparison report if previous results exist
generate_comparison_report() {
    log "Looking for previous test results for comparison..."
    
    local comparison_file="$REPORTS_DIR/comparison-${TIMESTAMP}.txt"
    local previous_results=$(find "$LOGS_DIR" -maxdepth 1 -type d -name "interop-tests-*" | sort | tail -2 | head -1)
    
    if [[ -n "$previous_results" && "$previous_results" != "$LATEST_TEST_DIR" ]]; then
        log "Found previous results: $previous_results"
        
        # Extract previous stats
        local prev_json="${previous_results}/test-results.json"
        if [[ -f "$prev_json" ]]; then
            local prev_total prev_passed prev_failed prev_skipped
            
            if command -v jq >/dev/null 2>&1; then
                prev_total=$(jq '.tests | length' "$prev_json" 2>/dev/null || echo "0")
                prev_passed=$(jq '.tests | map(select(.result == "PASS")) | length' "$prev_json" 2>/dev/null || echo "0")
                prev_failed=$(jq '.tests | map(select(.result == "FAIL")) | length' "$prev_json" 2>/dev/null || echo "0")
                prev_skipped=$(jq '.tests | map(select(.result == "SKIP")) | length' "$prev_json" 2>/dev/null || echo "0")
            else
                prev_total=$(grep -o '"test":' "$prev_json" | wc -l)
                prev_passed=$(grep -o '"result": "PASS"' "$prev_json" | wc -l)
                prev_failed=$(grep -o '"result": "FAIL"' "$prev_json" | wc -l)
                prev_skipped=$(grep -o '"result": "SKIP"' "$prev_json" | wc -l)
            fi
            
            local prev_success_rate
            if [[ $((prev_total - prev_skipped)) -gt 0 ]]; then
                prev_success_rate=$(( prev_passed * 100 / (prev_total - prev_skipped) ))
            else
                prev_success_rate=0
            fi
            
            cat > "$comparison_file" << EOF
Redfire Switch Test Comparison Report  
====================================
Generated: $(date)

Current Test Results vs Previous Run:

                    Previous    Current    Change
Total Tests:        $prev_total           $TOTAL_TESTS        $(( TOTAL_TESTS - prev_total ))
Passed Tests:       $prev_passed           $PASSED_TESTS        $(( PASSED_TESTS - prev_passed ))
Failed Tests:       $prev_failed           $FAILED_TESTS        $(( FAILED_TESTS - prev_failed ))
Skipped Tests:      $prev_skipped          $SKIPPED_TESTS       $(( SKIPPED_TESTS - prev_skipped ))
Success Rate:       ${prev_success_rate}%         ${SUCCESS_RATE}%         $(( SUCCESS_RATE - prev_success_rate ))%

Trend Analysis:
$(if [[ $SUCCESS_RATE -gt $prev_success_rate ]]; then echo "✓ Success rate improved by $(( SUCCESS_RATE - prev_success_rate ))%"; elif [[ $SUCCESS_RATE -lt $prev_success_rate ]]; then echo "⚠ Success rate decreased by $(( prev_success_rate - SUCCESS_RATE ))%"; else echo "→ Success rate unchanged"; fi)
$(if [[ $FAILED_TESTS -lt $prev_failed ]]; then echo "✓ Fewer test failures (improvement)"; elif [[ $FAILED_TESTS -gt $prev_failed ]]; then echo "⚠ More test failures (regression)"; else echo "→ Same number of test failures"; fi)
EOF
            
            success "Comparison report generated: $comparison_file"
        else
            warn "Previous results file not found, skipping comparison"
        fi
    else
        log "No previous results found for comparison"
    fi
}

# Create master index of all reports
create_report_index() {
    log "Creating report index..."
    
    local index_file="$REPORTS_DIR/index.html"
    
    cat > "$index_file" << EOF
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Redfire Switch Test Reports</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; background: #f5f5f5; }
        .container { max-width: 800px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
        h1 { color: #333; border-bottom: 3px solid #667eea; padding-bottom: 10px; }
        .report-item { margin: 15px 0; padding: 15px; border: 1px solid #ddd; border-radius: 4px; background: #fafafa; }
        .report-item h3 { margin-top: 0; color: #667eea; }
        .report-item a { color: #007bff; text-decoration: none; }
        .report-item a:hover { text-decoration: underline; }
        .timestamp { color: #666; font-size: 0.9em; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Redfire Switch Test Reports</h1>
        <p>Generated on $(date)</p>
        
        <div class="report-item">
            <h3>Latest Test Report</h3>
            <p><a href="$(basename "$HTML_REPORT")">Interoperability Test Report</a></p>
            <p class="timestamp">Generated: $(date)</p>
        </div>
        
        <div class="report-item">
            <h3>Raw Data</h3>
            <p><a href="data/test-results-${TIMESTAMP}.csv">CSV Data Export</a></p>
            <p><a href="performance-${TIMESTAMP}.txt">Performance Analysis</a></p>
            <p class="timestamp">For detailed analysis and data processing</p>
        </div>
        
        <div class="report-item">
            <h3>Historical Reports</h3>
            <p>Previous test reports and trend analysis</p>
            <ul>
EOF

    # List other HTML reports
    find "$REPORTS_DIR" -name "interop-report-*.html" -not -name "$(basename "$HTML_REPORT")" | sort -r | head -5 | while read -r report; do
        echo "                <li><a href=\"$(basename "$report")\">$(basename "$report")</a></li>" >> "$index_file"
    done

    cat >> "$index_file" << EOF
            </ul>
        </div>
        
        <div class="report-item">
            <h3>Documentation</h3>
            <p><a href="../README.md">Test Suite Documentation</a></p>
            <p><a href="../../../docs/TESTING_GUIDE.md">Complete Testing Guide</a></p>
        </div>
    </div>
</body>
</html>
EOF

    success "Report index created: $index_file"
    log "Open in browser: file://$(pwd)/$index_file"
}

# Main function
main() {
    echo
    log "==================================================="
    log "  Redfire Switch Test Report Generator"
    log "==================================================="
    echo
    
    setup_reporting
    find_latest_results
    parse_results
    
    echo
    log "Generating reports..."
    generate_html_report
    generate_csv_report
    generate_performance_report
    generate_comparison_report
    create_report_index
    
    echo
    success "Report generation complete!"
    echo
    log "Generated reports:"
    echo "  📊 HTML Report: $HTML_REPORT"
    echo "  📈 Performance: $REPORTS_DIR/performance-${TIMESTAMP}.txt"  
    echo "  📋 CSV Data: $REPORTS_DIR/data/test-results-${TIMESTAMP}.csv"
    echo "  🔍 Index: $REPORTS_DIR/index.html"
    echo
    log "Open the HTML report in your browser to view detailed results"
}

# Handle command line arguments
case "${1:-}" in
    --help|-h)
        echo "Redfire Switch Test Report Generator"
        echo
        echo "Usage: $0 [options]"
        echo
        echo "Options:"
        echo "  --help, -h    Show this help message"
        echo
        echo "This script analyzes the latest test results and generates:"
        echo "  - HTML report with visual charts and statistics"
        echo "  - CSV data export for further analysis"
        echo "  - Performance analysis and recommendations"
        echo "  - Comparison with previous test runs"
        echo
        exit 0
        ;;
    *)
        main "$@"
        ;;
esac