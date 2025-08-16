# Redfire Switch - Comprehensive Testing Guide

## Testing Features by Complexity Level

This document provides a structured approach to testing all features of the Redfire Switch, organized from simplest to most complex. Each test includes prerequisites, test procedures, expected results, and validation criteria.

---

## Level 1: Basic Functionality Tests (Simplest)

### 1.1 Configuration Loading
**Complexity: ⭐**
**Prerequisites:** None
**Test Objective:** Verify basic configuration loading and validation

```bash
# Test Steps:
1. ./redfire-switch gen-config test-config.yaml
2. ./redfire-switch validate-config --config test-config.yaml
3. ./redfire-switch show-config --config test-config.yaml

# Expected Results:
- Configuration file generated successfully
- Validation passes without errors
- Configuration displays in readable format
```

### 1.2 Basic SIP Message Parsing
**Complexity: ⭐**
**Prerequisites:** Configuration loaded
**Test Objective:** Verify SIP message parsing functionality

```bash
# Test Steps:
1. Start switch: ./redfire-switch start --config test-config.yaml
2. Send basic SIP OPTIONS message to port 5060
3. Check logs for successful parsing

# Expected Results:
- SIP message parsed without errors
- Headers correctly extracted
- No parsing exceptions in logs
```

### 1.3 UDP Transport Layer
**Complexity: ⭐**
**Prerequisites:** Switch running
**Test Objective:** Verify UDP SIP transport

```bash
# Test Steps:
1. Configure UDP transport on port 5060
2. Send SIP message via UDP
3. Verify message received and parsed

# Test Command:
echo -e "OPTIONS sip:test@127.0.0.1 SIP/2.0\r\nVia: SIP/2.0/UDP 127.0.0.1:5080\r\nFrom: <sip:test@127.0.0.1>\r\nTo: <sip:test@127.0.0.1>\r\nCall-ID: test-123\r\nCSeq: 1 OPTIONS\r\nContent-Length: 0\r\n\r\n" | nc -u 127.0.0.1 5060

# Expected Results:
- Message received on UDP port
- Response sent back
- Transport statistics updated
```

### 1.4 Basic Authentication (IP-based)
**Complexity: ⭐⭐**
**Prerequisites:** UDP transport working
**Test Objective:** Verify IP-based authentication

```bash
# Test Steps:
1. Configure IP whitelist: 127.0.0.1/32
2. Send INVITE from allowed IP
3. Send INVITE from blocked IP
4. Verify authentication results

# Expected Results:
- Allowed IP: Authentication succeeds
- Blocked IP: 403 Forbidden response
- Logs show authentication decisions
```

---

## Level 2: Core SIP Functionality Tests

### 2.1 TCP Transport Layer
**Complexity: ⭐⭐**
**Prerequisites:** UDP transport working
**Test Objective:** Verify TCP SIP transport with connection management

```bash
# Test Steps:
1. Configure TCP transport on port 5060
2. Establish TCP connection
3. Send multiple SIP messages on same connection
4. Close connection gracefully

# Expected Results:
- TCP connection established
- Multiple messages processed
- Connection statistics tracked
- Graceful connection closure
```

### 2.2 SIP Transaction Management
**Complexity: ⭐⭐⭐**
**Prerequisites:** Transport layers working
**Test Objective:** Verify SIP transaction state management

```bash
# Test Steps:
1. Send INVITE (creates server transaction)
2. Verify transaction state: Proceeding
3. Send 180 Ringing response
4. Send 200 OK response
5. Verify transaction cleanup

# Expected Results:
- Transaction created with unique ID
- State transitions: Proceeding → Completed → Terminated
- Transaction timers handled correctly
- Memory cleanup after termination
```

### 2.3 SIP Dialog Management
**Complexity: ⭐⭐⭐**
**Prerequisites:** Transaction management working
**Test Objective:** Verify SIP dialog creation and management

```bash
# Test Steps:
1. Send INVITE (creates dialog)
2. Process 200 OK response
3. Send ACK (confirms dialog)
4. Send re-INVITE (modify dialog)
5. Send BYE (terminate dialog)

# Expected Results:
- Dialog created with From/To/Call-ID
- Dialog state: Early → Confirmed → Terminated
- Route set maintained
- Dialog cleanup on BYE
```

### 2.4 Tech Prefix Extraction
**Complexity: ⭐⭐**
**Prerequisites:** Authentication working
**Test Objective:** Verify tech prefix parsing from SIP URIs

```bash
# Test Cases:
1. INVITE sip:1001*15551234567@carrier.com (prefix: 1001)
2. INVITE sip:*1001*15551234567@carrier.com (prefix: *1001)
3. INVITE sip:+100115551234567@carrier.com (prefix: +1001)
4. INVITE sip:15551234567@carrier.com (no prefix)

# Expected Results:
- Correct tech prefix extracted for each pattern
- Calling number properly parsed (without prefix)
- Authentication rules applied per prefix
- Routing decisions based on prefix
```

---

## Level 3: Media Plane Tests

### 3.1 RTP Proxy Port Allocation
**Complexity: ⭐⭐**
**Prerequisites:** Core SIP working
**Test Objective:** Verify RTP port allocation and management

```bash
# Test Steps:
1. Create media session
2. Verify RTP/RTCP port pair allocated (even/odd)
3. Create multiple sessions
4. Verify no port conflicts
5. Cleanup sessions and verify port release

# Expected Results:
- Port pairs allocated from configured range
- RTP port is even, RTCP port is RTP+1
- No duplicate port assignments
- Ports released when sessions end
```

### 3.2 Basic RTP Relay
**Complexity: ⭐⭐⭐**
**Prerequisites:** Port allocation working
**Test Objective:** Verify RTP packet forwarding

```bash
# Test Steps:
1. Setup media session with two endpoints
2. Send RTP packets to proxy RTP port
3. Verify packets forwarded to remote endpoint
4. Monitor packet statistics
5. Test bidirectional forwarding

# Tools:
- Use ffmpeg or gstreamer to generate RTP
- Wireshark to capture forwarded packets
- Switch statistics to verify relay counts

# Expected Results:
- Packets forwarded without modification
- Statistics updated (packets_a_to_b, packets_b_to_a)
- No packet loss in relay process
- Jitter and delay metrics collected
```

### 3.3 G.711 Codec Transcoding
**Complexity: ⭐⭐⭐⭐**
**Prerequisites:** RTP relay working
**Test Objective:** Verify μ-law to A-law transcoding

```bash
# Test Steps:
1. Configure session with μ-law input, A-law output
2. Send G.711 μ-law RTP packets
3. Verify output packets are G.711 A-law
4. Test audio quality (optional with actual audio)
5. Monitor transcoding statistics

# Expected Results:
- Payload type changed from 0 (μ-law) to 8 (A-law)
- Audio samples properly converted
- Transcoding operations counter incremented
- No significant delay introduced
```

### 3.4 DTMF Relay (RFC 4733)
**Complexity: ⭐⭐⭐**
**Prerequisites:** RTP relay working
**Test Objective:** Verify DTMF event relay

```bash
# Test Steps:
1. Configure session with DTMF payload type 101
2. Send DTMF event packets (digits 0-9, *, #)
3. Verify events properly relayed
4. Test DTMF duration and end events
5. Monitor DTMF relay statistics

# Expected Results:
- DTMF events correctly parsed and relayed
- Event duration preserved
- End flag properly handled
- DTMF statistics updated
```

### 3.5 RTP Monitoring and MOS Scoring
**Complexity: ⭐⭐⭐⭐**
**Prerequisites:** RTP relay working
**Test Objective:** Verify voice quality assessment

```bash
# Test Steps:
1. Start RTP session with monitoring enabled
2. Introduce packet loss (5%, 10%)
3. Introduce jitter (20ms, 50ms)
4. Introduce delay variation
5. Verify MOS scores calculated correctly

# Expected Results:
- MOS score degrades with packet loss
- Jitter affects MOS calculation
- Quality classification (Excellent/Good/Fair/Poor/Bad)
- Real-time quality alerts triggered
```

---

## Level 4: Advanced Routing and Billing Tests

### 4.1 NANPA Database Integration
**Complexity: ⭐⭐⭐**
**Prerequisites:** Basic routing working
**Test Objective:** Verify LERG/NANPA lookups

```bash
# Test Steps:
1. Configure LERG database connection
2. Lookup NPA-NXX: 555-123-XXXX
3. Verify LRN, carrier, and jurisdiction returned
4. Test invalid number handling
5. Verify lookup caching

# Expected Results:
- Valid numbers return LRN and carrier info
- Invalid numbers handled gracefully
- Lookup results cached for performance
- Database connection resilient to failures
```

### 4.2 Jurisdiction Determination
**Complexity: ⭐⭐⭐**
**Prerequisites:** NANPA lookups working
**Test Objective:** Verify call jurisdiction classification

```bash
# Test Cases:
1. Local: 555-123-1234 → 555-123-5678 (same NPA-NXX)
2. Intrastate: 555-123-1234 → 555-456-5678 (same state)
3. Interstate: 555-123-1234 → 212-456-7890 (different states)
4. International: 555-123-1234 → +44-20-1234-5678

# Expected Results:
- Correct jurisdiction determined for each case
- LRN used for accurate determination when available
- Jurisdiction affects routing and billing decisions
```

### 4.3 LRN/DNIS Mixed Routing
**Complexity: ⭐⭐⭐⭐**
**Prerequisites:** Jurisdiction working
**Test Objective:** Verify combined LRN and DNIS routing criteria

```bash
# Test Steps:
1. Configure routing with LRN OR DNIS criteria
2. Send call with LRN lookup available
3. Send call without LRN (DNIS only)
4. Verify route selection logic
5. Test route failover scenarios

# Expected Results:
- LRN routing preferred when available
- DNIS routing fallback works
- Route selection considers cost and quality
- Failover routes used when primary fails
```

### 4.4 Real-time Billing Engine
**Complexity: ⭐⭐⭐⭐**
**Prerequisites:** Routing working
**Test Objective:** Verify call billing and rating

```bash
# Test Steps:
1. Configure billing rates (per minute, increments)
2. Start billing session for call
3. Let call run for specific duration
4. End call and verify cost calculation
5. Test different rate structures

# Test Cases:
- 30-second call with 60-second increment
- 90-second call with 6-second increment
- Call with connection fee and minimum charge

# Expected Results:
- Billing session created at call start
- Duration calculated from answer to end
- Cost calculated per rate structure
- CDR generated with accurate billing info
```

### 4.5 Prepaid Balance Management
**Complexity: ⭐⭐⭐⭐**
**Prerequisites:** Billing engine working
**Test Objective:** Verify prepaid credit control

```bash
# Test Steps:
1. Set customer balance to $10.00
2. Configure rate at $0.05/minute
3. Start call and verify balance check
4. Let call run to deplete balance
5. Verify call termination and final balance

# Expected Results:
- Balance checked before call authorization
- Insufficient balance prevents call setup
- Balance decremented in real-time
- Call terminated when balance exhausted
```

---

## Level 5: Carrier Integration Tests

### 5.1 SS7 Link Management
**Complexity: ⭐⭐⭐⭐**
**Prerequisites:** Core systems working
**Test Objective:** Verify SS7 signaling link operations

```bash
# Test Steps:
1. Configure SS7 link to remote point code
2. Bring link into service
3. Send/receive MTP3 test messages
4. Simulate link failure and recovery
5. Verify link statistics and alarms

# Expected Results:
- Link state transitions: OOS → InService
- Heartbeat messages exchanged
- Link failure detected and reported
- Automatic recovery when link restored
```

### 5.2 ISUP Message Processing
**Complexity: ⭐⭐⭐⭐⭐**
**Prerequisites:** SS7 links working
**Test Objective:** Verify ISUP call processing

```bash
# Test Steps:
1. Send ISUP IAM (Initial Address Message)
2. Verify circuit allocation and SIP INVITE generation
3. Send ISUP ACM (Address Complete)
4. Send ISUP ANM (Answer Message)
5. Send ISUP REL (Release) and verify cleanup

# Expected Results:
- IAM creates SIP INVITE with proper number translation
- Circuit state managed through call lifecycle
- SIP-I interworking maintains call correlation
- Circuit released and available for reuse
```

### 5.3 Circuit Management
**Complexity: ⭐⭐⭐⭐**
**Prerequisites:** ISUP working
**Test Objective:** Verify circuit allocation and management

```bash
# Test Steps:
1. Allocate circuits 1-100 for destination PC
2. Make 50 concurrent calls
3. Verify circuit allocation strategy
4. Release circuits and verify availability
5. Test circuit blocking/unblocking

# Expected Results:
- Circuits allocated sequentially
- No double allocation of same circuit
- Circuit state properly tracked
- Blocked circuits not allocated
```

### 5.4 Advanced Codec Transcoding
**Complexity: ⭐⭐⭐⭐⭐**
**Prerequisites:** Media plane working
**Test Objective:** Verify G.729, G.722, Opus transcoding

```bash
# Test Steps:
1. Configure transcoding session G.729 → G.711
2. Send G.729 compressed audio packets
3. Verify G.711 output with proper decompression
4. Test multiple codec combinations
5. Monitor transcoding performance

# Expected Results:
- Audio quality maintained through transcoding
- Proper frame size and timing
- CPU usage within acceptable limits
- Transcoding statistics accurate
```

### 5.5 SNMP Management Interface
**Complexity: ⭐⭐⭐**
**Prerequisites:** System running
**Test Objective:** Verify SNMP monitoring capabilities

```bash
# Test Steps:
1. Query system MIB objects (sysDescr, sysUpTime)
2. Query custom switch MIBs (call counts, link status)
3. Set writable MIB objects
4. Verify SNMP trap generation
5. Test SNMP v2c and v3 authentication

# SNMP Commands:
snmpget -v2c -c public 127.0.0.1 1.3.6.1.2.1.1.1.0
snmpwalk -v2c -c public 127.0.0.1 1.3.6.1.4.1.12345

# Expected Results:
- System information returned correctly
- Switch-specific metrics available
- Traps sent for critical events
- Proper SNMP authentication handling
```

---

## Level 6: Production Hardening Tests

### 6.1 High Availability Clustering
**Complexity: ⭐⭐⭐⭐⭐**
**Prerequisites:** Multiple switch instances
**Test Objective:** Verify HA failover and clustering

```bash
# Test Steps:
1. Start primary and secondary switch nodes
2. Verify heartbeat communication
3. Stop primary node (simulate failure)
4. Verify automatic failover to secondary
5. Test split-brain prevention

# Expected Results:
- Secondary promotes to primary within failover timeout
- Active calls maintained during failover
- Virtual IP migrates to new primary
- Split-brain condition detected and prevented
```

### 6.2 Performance Monitoring
**Complexity: ⭐⭐⭐⭐**
**Prerequisites:** Switch running under load
**Test Objective:** Verify metrics collection and alerting

```bash
# Test Steps:
1. Generate moderate load (100 CPS)
2. Monitor CPU, memory, network metrics
3. Trigger high load (500 CPS)
4. Verify performance alerts triggered
5. Test Prometheus metrics endpoint

# Expected Results:
- Metrics collected at configured intervals
- Performance trends visible in dashboards
- Alerts triggered when thresholds exceeded
- Prometheus scraping successful
```

### 6.3 Security Features
**Complexity: ⭐⭐⭐⭐**
**Prerequisites:** Switch accessible from network
**Test Objective:** Verify security hardening

```bash
# Test Steps:
1. Attempt connection from unauthorized IP
2. Send malformed SIP messages (fuzzing)
3. Exceed rate limits from single IP
4. Test SQL injection in custom headers
5. Verify intrusion detection alerts

# Expected Results:
- Unauthorized IPs blocked
- Malformed messages handled gracefully
- Rate limiting prevents DoS attacks
- Security threats detected and logged
```

### 6.4 Load Testing Framework
**Complexity: ⭐⭐⭐⭐⭐**
**Prerequisites:** Switch in test environment
**Test Objective:** Verify switch performance under load

```bash
# Test Steps:
1. Configure load test: 1000 concurrent calls
2. Run test for 60 minutes
3. Monitor call success rate
4. Verify media quality maintained
5. Check for memory leaks or crashes

# Load Test Scenarios:
- Basic INVITE-200-ACK-BYE
- INVITE with authentication challenge
- Calls with media transcoding
- Mixed call durations (10s to 300s)

# Expected Results:
- >99% call success rate
- <100ms average setup time
- Stable memory usage
- No crashes or service degradation
```

### 6.5 End-to-End Integration Tests
**Complexity: ⭐⭐⭐⭐⭐**
**Prerequisites:** All components working
**Test Objective:** Verify complete call flow integration

```bash
# Test Steps:
1. Incoming SS7 call (ISUP IAM)
2. LRN lookup and jurisdiction determination
3. Routing decision with billing rate lookup
4. SIP call setup with authentication
5. Media establishment with transcoding
6. Call answer and billing start
7. DTMF collection during call
8. Call termination and final billing
9. CDR generation and SS7 circuit release

# Expected Results:
- Complete call flow successful
- All systems integrated properly
- Accurate billing and CDR generation
- No component failures or timeouts
```

---

## Level 7: Stress and Edge Case Tests (Most Complex)

### 7.1 Memory Leak Detection
**Complexity: ⭐⭐⭐⭐⭐**
**Test Duration:** 24+ hours continuous operation
**Test Objective:** Verify long-term stability

```bash
# Test Steps:
1. Start switch with memory profiling
2. Run continuous load (100 CPS) for 24 hours
3. Monitor memory usage trends
4. Analyze heap dumps for leaks
5. Verify graceful degradation under memory pressure

# Tools:
- Valgrind for memory leak detection
- heaptrack for heap profiling
- System memory monitoring

# Expected Results:
- Stable memory usage over time
- No detectable memory leaks
- Proper cleanup of call sessions
- Graceful handling of memory exhaustion
```

### 7.2 Concurrent Connection Limits
**Complexity: ⭐⭐⭐⭐⭐**
**Test Objective:** Verify scalability limits

```bash
# Test Steps:
1. Open 10,000 TCP connections simultaneously
2. Send SIP messages on all connections
3. Monitor connection state and response times
4. Test connection cleanup on timeout
5. Verify file descriptor management

# Expected Results:
- All connections accepted up to configured limit
- Response times remain acceptable
- Proper connection cleanup
- No file descriptor leaks
```

### 7.3 Database Failover and Recovery
**Complexity: ⭐⭐⭐⭐⭐**
**Test Objective:** Verify database resilience

```bash
# Test Steps:
1. Start switch with primary database
2. Generate routing and billing traffic
3. Simulate database failure
4. Verify failover to secondary database
5. Restore primary and verify recovery

# Expected Results:
- Automatic failover with minimal service disruption
- No data loss during failover
- Proper synchronization on recovery
- Error handling for database unavailability
```

### 7.4 Geographic Redundancy
**Complexity: ⭐⭐⭐⭐⭐**
**Test Objective:** Verify disaster recovery

```bash
# Test Steps:
1. Deploy switch in multiple data centers
2. Configure geographic load balancing
3. Simulate complete data center failure
4. Verify traffic redirection to backup site
5. Test data replication and consistency

# Expected Results:
- Traffic automatically redirected
- No significant service interruption
- Data consistency maintained
- Recovery procedures successful
```

### 7.5 Protocol Compliance Testing
**Complexity: ⭐⭐⭐⭐⭐**
**Test Objective:** Verify RFC compliance and interoperability

```bash
# Test Steps:
1. RFC 3261 compliance testing with SIP torture tests
2. RFC 3550 RTP compliance with various codecs
3. SS7/ISUP compliance with telecom test tools
4. Interoperability testing with major switch vendors
5. Protocol fuzzing and edge case handling

# Tools:
- SIPp for SIP protocol testing
- SIPP torture tests for RFC compliance
- Commercial SS7 test equipment
- Protocol analyzers and validators

# Expected Results:
- Full RFC compliance demonstrated
- Interoperability with major vendors
- Graceful handling of malformed messages
- Proper protocol error responses
```

---

## Test Automation and CI/CD

### Automated Test Suites
```bash
# Unit Tests (Level 1-2)
cargo test unit_tests

# Integration Tests (Level 3-4)
cargo test integration_tests

# End-to-End Tests (Level 5-6)
./scripts/run_e2e_tests.sh

# Performance Tests (Level 7)
./scripts/run_performance_tests.sh
```

### Continuous Integration Pipeline
1. **Build and Unit Tests** (every commit)
2. **Integration Tests** (every PR)
3. **Security Scanning** (nightly)
4. **Performance Regression Tests** (weekly)
5. **Full End-to-End Tests** (before release)

### Test Environment Requirements
- **Development:** Single node, basic features
- **Staging:** Multi-node, full feature set
- **Production:** Geographic redundancy, full load

---

## Validation Criteria

### Performance Benchmarks
- **Call Setup Time:** <100ms average
- **Call Success Rate:** >99.5%
- **Concurrent Calls:** 10,000+ per node
- **Calls Per Second:** 1,000+ sustained
- **Memory Usage:** <8GB per 10,000 calls
- **CPU Usage:** <80% under full load

### Reliability Targets
- **Uptime:** 99.999% (5.26 minutes downtime/year)
- **Failover Time:** <30 seconds
- **MTBF:** >8760 hours (1 year)
- **MTTR:** <15 minutes

### Quality Metrics
- **Voice Quality:** MOS >4.0 average
- **Packet Loss:** <0.1%
- **Jitter:** <20ms
- **End-to-End Delay:** <150ms

This testing guide provides a systematic approach to validating all switch features from basic functionality to complex carrier-grade operations. Each test level builds upon the previous, ensuring a solid foundation before advancing to more complex scenarios.