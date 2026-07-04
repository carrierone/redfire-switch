# Redfire Switch Testing Guide

This comprehensive guide covers all aspects of testing Redfire Switch, from basic functionality verification to advanced performance testing.

## 🧪 Table of Contents

- [Testing Philosophy](#testing-philosophy)
- [Quick Testing Reference](#quick-testing-reference)
- [Test Environment Setup](#test-environment-setup)
- [SIPp-Based Testing](#sipp-based-testing)
- [Unit Testing](#unit-testing)
- [Integration Testing](#integration-testing)
- [Performance Testing](#performance-testing)
- [Debug Testing](#debug-testing)
- [Manual Testing](#manual-testing)
- [Continuous Testing](#continuous-testing)
- [Troubleshooting Tests](#troubleshooting-tests)

## 🎯 Testing Philosophy

Redfire Switch follows a comprehensive testing strategy:

1. **SIP Protocol First** - Use real SIP tools (SIPp) for protocol testing
2. **Automated Everything** - All tests can run automatically
3. **Multiple Levels** - Unit, integration, and end-to-end testing
4. **Real Environment** - Docker-based test environment mimics production
5. **Debug-Friendly** - Easy to isolate and debug test failures

## ⚡ Quick Testing Reference

### Essential Commands

```bash
# Setup test environment
make dev

# Run all tests
make test

# Debug single call
make debug

# Specific test types
make test-options    # OPTIONS ping test
make test-call       # Basic call flow
make test-register   # SIP registration
make test-stress     # Performance test

# Monitoring and analysis
make pcap-live       # Live packet capture
make tshark          # SIP message analysis
make docker-logs     # Container logs
```

### Test Status Indicators

- ✅ **PASS** - Test completed successfully
- ❌ **FAIL** - Test failed with errors
- ⚠️ **TIMEOUT** - Test didn't complete in time
- 🔄 **RETRY** - Test is being retried
- 📊 **STATS** - Performance statistics available

## 📞 Built-in Automated Call Testing (no external tools)

The switch ships with a self-contained SIP call-flow harness that places **real
SIP calls over UDP** through the real LCR routing engine, with no SIPp or Docker
required. It starts `LcrSipServer` in-process on an ephemeral port (backed by the
seeded test database) and drives calls as a SIP UAC.

```bash
# Provision + seed a Postgres DB first (see Test Environment Setup), then:
cargo test --test sip_call_flow_tests
```

Covered scenarios:

- **Answered call** - full `INVITE → 100 → 180 → 200 → ACK → BYE → 200` flow
- **Unroutable number** - LCR finds no route, caller gets `404 Not Found`
- **OPTIONS ping** - keepalive answered with `200 OK`
- **Concurrent calls** - several simultaneous calls all answered

The harness reuses the shipping `redfire_switch::sip_call_server::LcrSipServer`,
so it exercises the same routing/signaling code path as the `lcr_sip_server`
binary. This is the recommended first stop for validating call handling; the
SIPp scenarios below are for heavier protocol/interop and stress testing.

## 🏗️ Test Environment Setup

### Prerequisites

```bash
# Required tools
sudo apt-get update
sudo apt-get install -y \
    docker.io docker-compose \
    sipp tcpdump tshark \
    make curl wget

# Optional but recommended
sudo apt-get install -y \
    wireshark-qt gdb valgrind \
    netcat-openbsd nmap
```

### Automated Setup

```bash
# One-click setup
./setup-dev.sh

# Manual setup
make clean
make build
make docker-build
```

### Environment Verification

```bash
# Check all components
make verify-env

# Expected output:
# ✅ Docker: Available
# ✅ SIPp: Version 3.x found
# ✅ tcpdump: Available
# ✅ Redfire Switch: Built successfully
# ✅ Test environment: Ready
```

## 📞 SIPp-Based Testing

### Test Scenarios

The project includes 4 comprehensive SIPp test scenarios:

#### 1. OPTIONS Ping Test
**File**: `tests/sipp/scenarios/options_ping.xml`
**Purpose**: Test SIP OPTIONS method for endpoint health checking

```bash
# Run OPTIONS test
make test-options

# Expected behavior:
# 1. Send SIP OPTIONS to switch
# 2. Receive 200 OK response
# 3. Measure response time
# 4. Verify OPTIONS handling
```

**Success Criteria**:
- Response received within 1000ms
- Status code: 200 OK
- Valid SIP headers in response

#### 2. Basic Call Flow Test
**File**: `tests/sipp/scenarios/basic_call_uac.xml`
**Purpose**: Test complete call establishment and teardown

```bash
# Run basic call test
make test-call

# Call flow:
# UAC -> INVITE -> Switch
# UAC <- 100 Trying <- Switch
# UAC <- 200 OK <- Switch
# UAC -> ACK -> Switch
# [Call active for 5 seconds]
# UAC -> BYE -> Switch
# UAC <- 200 OK <- Switch
```

**Success Criteria**:
- Call establishes within 5 seconds
- Audio path confirmed (RTP simulation)
- Clean call teardown
- CDR record generated

#### 3. Registration Test
**File**: `tests/sipp/scenarios/register.xml`
**Purpose**: Test SIP REGISTER method for endpoint registration

```bash
# Run registration test
make test-register

# Registration flow:
# UAC -> REGISTER -> Switch
# UAC <- 401 Unauthorized <- Switch (auth challenge)
# UAC -> REGISTER (with auth) -> Switch
# UAC <- 200 OK <- Switch
```

**Success Criteria**:
- Authentication challenge received
- Registration accepted after auth
- Registration timeout handling
- Proper Contact header processing

#### 4. Stress Test
**File**: `tests/sipp/scenarios/stress_test.xml`
**Purpose**: Test switch performance under load

```bash
# Run stress test
make test-stress

# Test parameters:
# - 10 calls per second
# - 100 total calls
# - Concurrent call limit: 50
# - Test duration: ~30 seconds
```

**Success Criteria**:
- 95% call success rate
- Average response time < 100ms
- No memory leaks
- System remains stable

### Custom SIPp Tests

#### Creating New Test Scenarios

1. **Create XML scenario** in `tests/sipp/scenarios/`
```xml
<?xml version="1.0" encoding="ISO-8859-1" ?>
<!DOCTYPE scenario SYSTEM "sipp.dtd">
<scenario name="Your Custom Test">
  <send retrans="500">
    <![CDATA[
      INVITE sip:[service]@[remote_ip]:[remote_port] SIP/2.0
      Via: SIP/2.0/[transport] [local_ip]:[local_port];branch=[branch]
      From: "Test" <sip:test@[local_ip]:[local_port]>;tag=[call_number]
      To: <sip:[service]@[remote_ip]:[remote_port]>
      Call-ID: [call_id]
      CSeq: 1 INVITE
      Contact: <sip:test@[local_ip]:[local_port]>
      Content-Type: application/sdp
      Content-Length: [len]

      v=0
      o=test 123 123 IN IP4 [local_ip]
      s=-
      c=IN IP4 [local_ip]
      t=0 0
      m=audio 6000 RTP/AVP 0
      a=rtpmap:0 PCMU/8000
    ]]>
  </send>

  <recv response="100" optional="true" />
  <recv response="200" />
  
  <send>
    <![CDATA[
      ACK sip:[service]@[remote_ip]:[remote_port] SIP/2.0
      Via: SIP/2.0/[transport] [local_ip]:[local_port];branch=[branch]
      From: "Test" <sip:test@[local_ip]:[local_port]>;tag=[call_number]
      To: <sip:[service]@[remote_ip]:[remote_port]>[peer_tag_param]
      Call-ID: [call_id]
      CSeq: 1 ACK
      Content-Length: 0
    ]]>
  </send>

  <pause milliseconds="3000"/>

  <send retrans="500">
    <![CDATA[
      BYE sip:[service]@[remote_ip]:[remote_port] SIP/2.0
      Via: SIP/2.0/[transport] [local_ip]:[local_port];branch=[branch]
      From: "Test" <sip:test@[local_ip]:[local_port]>;tag=[call_number]
      To: <sip:[service]@[remote_ip]:[remote_port]>[peer_tag_param]
      Call-ID: [call_id]
      CSeq: 2 BYE
      Content-Length: 0
    ]]>
  </send>

  <recv response="200" />
</scenario>
```

2. **Add to test runner** in `tests/run-tests.sh`
```bash
# Add your test
run_sipp_test "your_test" "scenarios/your_custom_test.xml" "-m 1"
```

3. **Create Make target** in `Makefile.dev`
```makefile
test-custom: docker-up ## Run custom test
	@echo "$(GREEN)Running custom test...$(NC)"
	cd $(TEST_DIR) && ./run-tests.sh your_test
```

#### SIPp Command Reference

```bash
# Basic SIPp usage
sipp -sn uac [target_ip:port]           # User Agent Client
sipp -sn uas [local_ip:port]            # User Agent Server
sipp -sf scenario.xml [target]          # Custom scenario

# Common options
-m [count]        # Number of calls
-r [rate]         # Calls per second
-l [limit]        # Maximum concurrent calls
-d [duration]     # Call duration (ms)
-t [protocol]     # Transport (u1 for UDP, t1 for TCP)
-p [port]         # Local port
-trace_msg        # Message tracing
-trace_shortmsg   # Short message tracing
```

## 🔬 Unit Testing

### Running Unit Tests

```bash
# All unit tests
cargo test

# Specific module tests
cargo test routing
cargo test sip_server
cargo test config

# With output
cargo test -- --nocapture

# Test coverage
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

### Writing Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[test]
    fn test_route_parsing() {
        let route_str = "prefix=1234,trunk=carrier1,cost=0.01";
        let route = Route::from_string(route_str).unwrap();
        
        assert_eq!(route.prefix, "1234");
        assert_eq!(route.trunk_id, "carrier1");
        assert_eq!(route.cost, 0.01);
    }

    #[tokio::test]
    async fn test_sip_message_parsing() {
        let message = "INVITE sip:test@example.com SIP/2.0\r\n\
                      Via: SIP/2.0/UDP example.com:5060\r\n\
                      From: <sip:caller@example.com>;tag=123\r\n\
                      To: <sip:test@example.com>\r\n\r\n";
        
        let parsed = SipMessage::parse(message.as_bytes()).unwrap();
        
        assert_eq!(parsed.method, "INVITE");
        assert_eq!(parsed.request_uri, "sip:test@example.com");
        assert!(parsed.headers.contains_key("Via"));
    }

    #[tokio::test]
    async fn test_routing_engine() {
        let mut engine = RoutingEngine::new();
        engine.add_route(Route {
            prefix: "1234".to_string(),
            trunk_id: "test_trunk".to_string(),
            priority: 1,
            cost: 0.01,
        });

        let route = engine.route_call("", "12345").await.unwrap();
        assert_eq!(route.trunk_id, "test_trunk");
    }
}
```

## 🔗 Integration Testing

### Docker-Based Integration Tests

```bash
# Start integration environment
make docker-up

# Run integration test suite
make test-integration

# Components tested:
# - SIP server startup
# - Configuration loading
# - Database connections
# - Monitoring endpoints
# - Service coordination
```

### Integration Test Examples

```rust
#[tokio::test]
async fn test_complete_call_flow() {
    // Setup test environment
    let config = load_test_config().await;
    let server = SipServer::new(config.sip_profiles.clone());
    
    // Start server
    tokio::spawn(async move {
        server.start().await.unwrap();
    });
    
    // Wait for startup
    tokio::time::sleep(Duration::from_secs(1)).await;
    
    // Simulate SIP INVITE
    let invite = create_test_invite();
    let response = send_sip_message(&invite).await.unwrap();
    
    // Verify response
    assert_eq!(response.status_code, 200);
    assert!(response.headers.contains_key("Contact"));
}
```

### Database Integration Tests

```bash
# Test CDR integration
make test-cdr

# Test ClickHouse connection
make test-database

# Test configuration persistence
make test-config-db
```

## 🚀 Performance Testing

### Load Testing with SIPp

```bash
# Standard load test
make test-stress

# Custom load parameters
sipp -sf tests/sipp/scenarios/stress_test.xml \
     -r 50 -m 1000 -l 100 \
     localhost:5060

# Parameters explained:
# -r 50     : 50 calls per second
# -m 1000   : 1000 total calls
# -l 100    : Max 100 concurrent calls
```

### Performance Metrics

```bash
# Monitor during testing
make monitor

# CPU and memory usage
htop

# Network statistics
nethogs
iftop

# SIP-specific monitoring
make sip-stats
```

### Benchmarking

```bash
# Response time benchmarks
make benchmark-response

# Throughput benchmarks
make benchmark-throughput

# Memory usage benchmarks
make benchmark-memory

# Concurrent call benchmarks
make benchmark-concurrent
```

### Performance Expectations

| Metric | Target | Measurement |
|--------|--------|-------------|
| Response Time | < 50ms | Average SIP response time |
| Throughput | 1000 CPS | Calls per second |
| Concurrent Calls | 10,000 | Active simultaneous calls |
| Memory Usage | < 1GB | Resident memory |
| CPU Usage | < 80% | Average CPU utilization |

## 🐛 Debug Testing

### Single Call Debug Mode

```bash
# Debug mode (processes one call then exits)
make debug

# With packet capture
make debug-pcap

# With GDB
make debug-gdb

# With Valgrind (memory analysis)
make valgrind
```

### Debug Output Analysis

The debug mode provides comprehensive logging:

```
[2025-01-15T10:30:45Z INFO  redfire_switch] Starting Redfire Switch in debug mode
[2025-01-15T10:30:45Z DEBUG sip_server] Binding to 0.0.0.0:5060 (UDP)
[2025-01-15T10:30:45Z DEBUG sip_server] Waiting for SIP message...
[2025-01-15T10:30:46Z DEBUG sip_server] Received 354 bytes from 127.0.0.1:38472
[2025-01-15T10:30:46Z DEBUG sip_parser] Parsing SIP message:
INVITE sip:test@localhost:5060 SIP/2.0
Via: SIP/2.0/UDP 127.0.0.1:38472;branch=z9hG4bK-1234
From: "SIPp" <sip:sipp@127.0.0.1:38472>;tag=1
...
[2025-01-15T10:30:46Z INFO  routing] Routing call from sipp to test
[2025-01-15T10:30:46Z DEBUG routing] Found route: prefix=test, trunk=local
[2025-01-15T10:30:46Z INFO  stir_shaken] Creating identity header for call
[2025-01-15T10:30:46Z DEBUG sip_server] Sending 200 OK response
[2025-01-15T10:30:46Z INFO  redfire_switch] Debug mode: Processed 1 call, exiting
```

### Packet Capture Analysis

```bash
# Live capture during debug
make pcap-live

# Analyze captured packets
make tshark

# Wireshark GUI (if available)
make wireshark
```

### Common Debug Scenarios

1. **Message Parsing Issues**
   ```bash
   # Capture malformed messages
   make debug-parser
   ```

2. **Routing Problems**
   ```bash
   # Debug routing decisions
   make debug-routing
   ```

3. **Authentication Failures**
   ```bash
   # Debug auth challenges
   make debug-auth
   ```

4. **Performance Bottlenecks**
   ```bash
   # Profile performance
   make debug-perf
   ```

## 🔨 Manual Testing

### SIP Client Testing

```bash
# Using command-line tools
# Send OPTIONS ping
echo -e "OPTIONS sip:test@localhost:5060 SIP/2.0\r\n\
Via: SIP/2.0/UDP localhost:5061;branch=z9hG4bK123\r\n\
From: <sip:test@localhost:5061>;tag=test\r\n\
To: <sip:test@localhost:5060>\r\n\
Call-ID: test@localhost\r\n\
CSeq: 1 OPTIONS\r\n\
Content-Length: 0\r\n\r\n" | nc -u localhost 5060

# Using SIPp interactively
sipp -sf tests/sipp/scenarios/manual_test.xml localhost:5060
```

### REST API Testing

```bash
# Health check
curl http://localhost:8080/health

# Stats endpoint
curl http://localhost:8080/stats

# Configuration
curl http://localhost:8080/config
```

### Configuration Testing

```bash
# Validate configuration
redfire-switch validate-config --config config-dev.json

# Test configuration changes
redfire-switch reload-config --config new-config.json
```

## 🔄 Continuous Testing

### GitHub Actions Integration

The project includes CI/CD workflows for automated testing:

```yaml
# .github/workflows/test.yml
name: Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run tests
        run: |
          ./setup-dev.sh
          make test
```

### Pre-commit Hooks

```bash
# Install pre-commit hooks
./setup-dev.sh

# Hooks include:
# - Cargo fmt (code formatting)
# - Cargo clippy (linting)
# - Unit tests
# - Basic SIPp test
```

### Automated Monitoring

```bash
# Setup continuous monitoring
make monitor-continuous

# Includes:
# - Health check polling
# - Performance metrics
# - Error rate monitoring
# - Alert thresholds
```

## 🔍 Troubleshooting Tests

### Common Test Issues

#### 1. SIPp Connection Refused
```bash
# Problem: SIPp can't connect to switch
# Solution:
make debug  # Check if switch is running
netstat -ln | grep 5060  # Verify port binding
```

#### 2. Docker Environment Issues
```bash
# Problem: Docker containers not starting
# Solution:
make docker-clean  # Clean environment
make docker-build  # Rebuild containers
make docker-up     # Restart environment
```

#### 3. Test Timeouts
```bash
# Problem: Tests timing out
# Solution:
make debug-slow     # Run with extended timeouts
make monitor-perf   # Check performance metrics
```

#### 4. Packet Capture Permission Denied
```bash
# Problem: Can't capture packets
# Solution:
sudo setcap cap_net_raw,cap_net_admin=eip $(which tcpdump)
# Or run with sudo:
sudo make pcap-live
```

### Test Debugging Workflow

1. **Identify the failing test**
   ```bash
   make test  # Note which test fails
   ```

2. **Run in debug mode**
   ```bash
   make debug  # Single call debug
   ```

3. **Capture network traffic**
   ```bash
   make pcap-live  # Monitor SIP messages
   ```

4. **Analyze logs**
   ```bash
   make docker-logs  # Check container logs
   ```

5. **Manual verification**
   ```bash
   # Test manually with SIPp
   sipp -sf failing-scenario.xml localhost:5060
   ```

### Test Environment Reset

```bash
# Complete environment reset
make clean-all      # Clean everything
make docker-clean   # Remove containers
make build          # Rebuild application
make docker-build   # Rebuild containers
make test           # Run tests again
```

## 📊 Test Reporting

### Test Output Interpretation

```bash
# Successful test run example:
===============================================
Redfire Switch Test Suite
===============================================
✅ OPTIONS Ping Test      - PASSED (127ms)
✅ Basic Call Flow Test   - PASSED (1.2s)
✅ Registration Test      - PASSED (856ms)
✅ Stress Test           - PASSED (30.5s)
===============================================
Summary: 4/4 tests passed
Total time: 33.8 seconds
===============================================
```

### Performance Reports

```bash
# Generate performance report
make report-performance

# Includes:
# - Response time statistics
# - Throughput measurements
# - Resource utilization
# - Comparison with baselines
```

### Coverage Reports

```bash
# Generate test coverage report
make coverage

# View coverage in browser
firefox target/tarpaulin/tarpaulin-report.html
```

## 📚 Additional Resources

### SIP Testing Resources
- [SIPp Documentation](http://sipp.sourceforge.net/doc/reference.html)
- [RFC 3261 - SIP Protocol](https://tools.ietf.org/html/rfc3261)
- [SIP Message Examples](https://www.iana.org/assignments/sip-parameters/)

### Performance Testing
- [SIPp Performance Testing Guide](http://sipp.sourceforge.net/doc/reference.html#Performance+testing)
- [Telecommunications Testing Best Practices](https://www.tmforum.org/best-practices/)

### Debugging Tools
- [Wireshark SIP Analysis](https://wiki.wireshark.org/SIP)
- [tcpdump SIP Filtering](https://www.tcpdump.org/manpages/pcap-filter.7.html)

---

This testing guide provides comprehensive coverage of all testing aspects for Redfire Switch. The combination of automated SIPp testing, unit tests, integration tests, and debug tools ensures reliable development and deployment of the SIP switch functionality.

*For additional testing support, see the [contributing guidelines](CONTRIBUTING.md) or check the project's GitHub issues.*