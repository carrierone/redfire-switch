# Redfire Switch Third-Party Test Suite Makefile

.PHONY: help setup test quick-test clean docker-up docker-down logs report

# Default target
help:
	@echo "Redfire Switch Third-Party Test Suite"
	@echo "====================================="
	@echo ""
	@echo "Available targets:"
	@echo "  setup        Setup test environment and dependencies"
	@echo "  quick-test   Run basic connectivity tests"
	@echo "  test         Run full interoperability test suite"
	@echo "  report       Generate test reports from latest results"
	@echo "  docker-up    Start test containers (Asterisk, FreeSWITCH, etc.)"
	@echo "  docker-down  Stop test containers"
	@echo "  logs         View test logs"
	@echo "  clean        Clean up test artifacts and logs"
	@echo "  monitor      Start traffic monitoring"
	@echo ""
	@echo "Environment Variables:"
	@echo "  REDFIRE_IP=127.0.0.1    Target IP address"
	@echo "  REDFIRE_PORT=5060       Target port"
	@echo ""

# Setup test environment
setup:
	@echo "Setting up third-party test environment..."
	./scripts/setup-test-environment.sh

# Run quick connectivity test
quick-test:
	@echo "Running quick connectivity test..."
	./scripts/quick-test.sh

# Run full test suite
test:
	@echo "Running full interoperability test suite..."
	./scripts/run-interop-tests.sh

# Generate test reports
report:
	@echo "Generating test reports..."
	./scripts/generate-test-report.sh

# Start Docker containers
docker-up:
	@echo "Starting test containers..."
	cd docker && docker-compose up -d

# Start Docker containers with monitoring
docker-up-full:
	@echo "Starting test containers with monitoring..."
	cd docker && docker-compose --profile testing --profile monitoring --profile reports up -d

# Stop Docker containers
docker-down:
	@echo "Stopping test containers..."
	cd docker && docker-compose down

# View logs
logs:
	@echo "Recent test logs:"
	@echo "=================="
	@find logs -name "*.log" -type f -mtime -1 -exec echo "--- {} ---" \; -exec tail -n 10 {} \; -exec echo "" \; || echo "No recent log files found"

# Start traffic monitoring
monitor:
	@echo "Starting traffic monitoring (Ctrl+C to stop)..."
	./scripts/monitor-traffic.sh

# Clean up test artifacts
clean:
	@echo "Cleaning up test artifacts..."
	rm -rf logs/interop-tests-*
	rm -rf logs/reports/interop-report-*
	rm -rf logs/reports/data/test-results-*
	rm -rf logs/*.log
	rm -rf recordings/*/*.wav
	@echo "Cleanup complete"

# Clean Docker volumes and containers
clean-docker:
	@echo "Cleaning Docker test environment..."
	cd docker && docker-compose down -v --remove-orphans
	docker system prune -f

# Run tests in Docker environment
test-docker: docker-up
	@echo "Waiting for containers to start..."
	@sleep 10
	@echo "Running tests against containerized services..."
	REDFIRE_IP=127.0.0.1 ./scripts/run-interop-tests.sh
	@$(MAKE) docker-down

# Continuous testing (runs tests every 5 minutes)
test-continuous:
	@echo "Starting continuous testing (Ctrl+C to stop)..."
	@while true; do \
		echo "Running test cycle at $$(date)"; \
		$(MAKE) quick-test; \
		echo "Next test in 5 minutes..."; \
		sleep 300; \
	done

# Performance test with load
test-performance:
	@echo "Running performance tests..."
	@if command -v sipp >/dev/null 2>&1; then \
		echo "Testing with 50 concurrent calls..."; \
		sipp -sf pjsip/scenarios/basic_call.xml $(REDFIRE_IP):$(REDFIRE_PORT) -l 50 -m 100 -r 10 -s 996; \
	else \
		echo "sipp not found, install with: sudo apt-get install sipp"; \
	fi

# Validate configurations
validate:
	@echo "Validating test configurations..."
	@echo "Checking Asterisk config..."
	@if command -v asterisk >/dev/null 2>&1; then \
		asterisk -T -f -x "core show settings" >/dev/null 2>&1 && echo "✓ Asterisk config valid" || echo "✗ Asterisk config invalid"; \
	else \
		echo "⚠ Asterisk not installed"; \
	fi
	@echo "Checking FreeSWITCH config..."
	@if command -v freeswitch >/dev/null 2>&1; then \
		freeswitch -nonat -nocal -nort -t >/dev/null 2>&1 && echo "✓ FreeSWITCH config valid" || echo "✗ FreeSWITCH config invalid"; \
	else \
		echo "⚠ FreeSWITCH not installed"; \
	fi

# Show test status and recent results
status:
	@echo "Redfire Switch Test Status"
	@echo "=========================="
	@echo "Target: $(REDFIRE_IP):$(REDFIRE_PORT)"
	@echo ""
	@echo "Recent test runs:"
	@find logs -name "interop-tests-*" -type d -exec basename {} \; | sort -r | head -5 || echo "No test results found"
	@echo ""
	@echo "Docker containers:"
	@cd docker && docker-compose ps 2>/dev/null || echo "Docker not running"
	@echo ""
	@echo "Available reports:"
	@find logs/reports -name "*.html" -type f -exec basename {} \; | sort -r | head -3 || echo "No reports found"

# Install development dependencies
install-deps:
	@echo "Installing test dependencies..."
	@if command -v apt-get >/dev/null 2>&1; then \
		sudo apt-get update; \
		sudo apt-get install -y sipp netcat-openbsd tcpdump wireshark-common sox; \
	elif command -v yum >/dev/null 2>&1; then \
		sudo yum install -y sipp netcat tcpdump wireshark sox; \
	elif command -v pacman >/dev/null 2>&1; then \
		sudo pacman -S --noconfirm sipp netcat tcpdump wireshark-cli sox; \
	else \
		echo "Please install dependencies manually: sipp, netcat, tcpdump, sox"; \
	fi

# Show help for Docker usage
docker-help:
	@echo "Docker Test Environment Usage"
	@echo "============================"
	@echo ""
	@echo "Basic usage:"
	@echo "  make docker-up     # Start Asterisk and FreeSWITCH"
	@echo "  make test          # Run tests against containers"
	@echo "  make docker-down   # Stop containers"
	@echo ""
	@echo "Advanced usage:"
	@echo "  make docker-up-full  # Start with monitoring (Prometheus/Grafana)"
	@echo ""
	@echo "Access URLs:"
	@echo "  http://localhost:8080  # Test reports"
	@echo "  http://localhost:9090  # Prometheus"
	@echo "  http://localhost:3000  # Grafana (admin/redfire123)"
	@echo ""
	@echo "Container IPs:"
	@echo "  Asterisk:   172.20.0.10:5061"
	@echo "  FreeSWITCH: 172.20.0.11:5063"
	@echo "  SIPp:       172.20.0.12"
	@echo ""

# Environment setup
REDFIRE_IP ?= 127.0.0.1
REDFIRE_PORT ?= 5060

# Export variables for scripts
export REDFIRE_IP REDFIRE_PORT