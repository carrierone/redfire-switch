# SIPp Testing Environment for Redfire Switch

## Quick Setup

1. **Run setup script:**
   ```bash
   ./setup-dev.sh
   ```

2. **Start testing:**
   ```bash
   make dev
   make test
   ```

## Available Commands

```bash
# Environment
make dev              # Start development environment
make docker-up        # Start Docker services
make docker-down      # Stop Docker services

# Testing
make test             # Run all SIPp tests
make test-call        # Test single call
make test-options     # Test OPTIONS ping
make test-stress      # Run stress test

# Debug
make debug            # Run switch in debug mode
make debug-gdb        # Debug with GDB
make pcap-live        # Monitor SIP packets

# Development
make build            # Build project
make clean            # Clean artifacts
make check            # Check compilation
```

## Test Scenarios

1. **OPTIONS Ping** - Basic connectivity test
2. **Basic Call** - Full call establishment and teardown
3. **REGISTER** - Registration with authentication
4. **Stress Test** - High-volume call testing

## Debug Mode Features

- **Single call mode** - Process one call then exit
- **Verbose logging** - Complete SIP message logging
- **Packet capture** - Automatic PCAP generation
- **Call flow tracing** - Step-by-step progression

## Quick Test

```bash
# 1. Start the switch in debug mode
make debug

# 2. In another terminal, send a test call
sipp -sn uac -m 1 localhost:5060

# 3. Monitor traffic (optional)
make pcap-live
```

## Files

- `docker-compose.dev.yml` - Docker environment
- `config-dev.json` - Debug configuration
- `tests/sipp/scenarios/` - SIPp test scenarios
- `Makefile.dev` - Development commands
- `DEV-ENVIRONMENT.md` - Full documentation

## Support

For detailed documentation, see: `DEV-ENVIRONMENT.md`

For help: `make help`