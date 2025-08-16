# Third-Party Interoperability Testing

This directory contains test configurations and scripts for testing Redfire Switch interoperability with major SIP implementations:

## Supported Test Platforms

### Asterisk
- **Version**: 18+ (configured for SIP and PJSIP channels)
- **Test Scenarios**: Basic calls, authentication, codec negotiation, DTMF
- **Configuration**: `asterisk/` directory

### PJSIP
- **Version**: 2.10+ 
- **Test Scenarios**: SIP compliance testing, transaction handling, transport protocols
- **Configuration**: `pjsip/` directory

### FreeSWITCH
- **Version**: 1.10+ (Sofia SIP stack)
- **Test Scenarios**: Advanced routing, media handling, protocol compliance
- **Configuration**: `freeswitch/` directory

## Quick Start

1. **Setup Test Environment**:
   ```bash
   sudo ./scripts/setup-test-environment.sh
   ```

2. **Run Basic Interop Tests**:
   ```bash
   ./scripts/run-interop-tests.sh
   ```

3. **View Test Results**:
   ```bash
   ./scripts/generate-test-report.sh
   ```

## Test Scenarios

### Level 1: Basic Connectivity
- SIP OPTIONS ping
- Basic INVITE/200/ACK/BYE flow
- Transport protocol testing (UDP/TCP)

### Level 2: Authentication Testing
- Digest authentication challenges
- Tech prefix authentication
- IP-based authentication

### Level 3: Media Interoperability
- Codec negotiation (G.711, G.729, G.722)
- RTP media flow
- DTMF transmission (RFC 4733)

### Level 4: Advanced Features
- Call transfer scenarios
- Conference calling
- Call hold/resume

### Level 5: Error Handling
- Malformed SIP message handling
- Network failure recovery
- Protocol compliance edge cases

## Directory Structure

```
third-party-tests/
├── asterisk/           # Asterisk configurations
│   ├── sip.conf
│   ├── pjsip.conf
│   ├── extensions.conf
│   └── modules.conf
├── pjsip/             # PJSIP test tools
│   ├── scenarios/
│   └── config/
├── freeswitch/        # FreeSWITCH configurations
│   ├── sofia.conf.xml
│   ├── dialplan/
│   └── directory/
├── scripts/           # Test automation scripts
│   ├── setup-test-environment.sh
│   ├── run-interop-tests.sh
│   └── generate-test-report.sh
└── logs/              # Test execution logs
    ├── asterisk/
    ├── pjsip/
    ├── freeswitch/
    └── redfire/
```

## Test Results

All test results are logged to `logs/` with timestamps and categorized by:
- **PASS**: Test completed successfully
- **FAIL**: Test failed with error details
- **WARN**: Test completed with warnings
- **ERROR**: Test could not execute

Log files include:
- SIP message traces
- Media flow statistics  
- Error conditions and stack traces
- Performance metrics

## Continuous Integration

These tests can be integrated into CI/CD pipelines using:
- Docker containers for each SIP platform
- Automated test execution on commits
- Test result reporting to dashboards