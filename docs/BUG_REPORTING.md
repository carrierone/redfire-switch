# Bug Reporting Guide

The Redfire Switch includes a comprehensive bug reporting system that helps users search for existing issues and submit detailed bug reports to the GitHub repository.

## Quick Start

### Setup Authentication

Before submitting bug reports, you need to set up GitHub authentication:

```bash
# Create a GitHub Personal Access Token at: https://github.com/settings/tokens
# Required permissions: public_repo

redfire-switch bug setup --token YOUR_GITHUB_TOKEN
```

### Search for Existing Issues

Always search for existing issues before reporting a new bug:

```bash
# Search for issues related to SIP
redfire-switch bug search "SIP parsing error"

# Search closed issues
redfire-switch bug search "authentication" --scope closed

# Detailed search results
redfire-switch bug search "codec negotiation" --detailed
```

### Submit a Bug Report

```bash
redfire-switch bug report \
  --title "SIP INVITE parsing fails with malformed headers" \
  --description "When receiving SIP INVITE with malformed Via header, the switch crashes" \
  --steps "1. Send INVITE with invalid Via header 2. Observe crash" \
  --expected "Switch should respond with 400 Bad Request" \
  --actual "Switch crashes with panic" \
  --severity high \
  --component sip
```

## Commands Reference

### `bug setup`

Configure GitHub authentication for bug reporting.

**Options:**
- `--token <TOKEN>` - Set GitHub personal access token
- `--test` - Test connection to GitHub API

**Examples:**
```bash
# Set up authentication
redfire-switch bug setup --token ghp_xxxxxxxxxxxx

# Test connection
redfire-switch bug setup --test
```

### `bug search`

Search for existing GitHub issues.

**Options:**
- `--scope <SCOPE>` - Search scope: open, closed, all (default: open)
- `--limit <NUMBER>` - Number of results (default: 10)
- `--detailed` - Show detailed results

**Examples:**
```bash
# Basic search
redfire-switch bug search "RTP proxy"

# Search all issues
redfire-switch bug search "memory leak" --scope all --limit 20

# Detailed results
redfire-switch bug search "authentication" --detailed
```

### `bug show`

Display details of a specific issue.

**Options:**
- `--comments` - Show issue comments

**Examples:**
```bash
# Show issue details
redfire-switch bug show 123

# Show with comments
redfire-switch bug show 123 --comments
```

### `bug list`

List recent issues.

**Options:**
- `--limit <NUMBER>` - Number of issues (default: 20)
- `--state <STATE>` - Filter by state: open, closed, all (default: open)
- `--label <LABEL>` - Filter by label
- `--priority` - Show only high/critical priority issues

**Examples:**
```bash
# List recent open issues
redfire-switch bug list

# List closed issues
redfire-switch bug list --state closed --limit 50

# List critical issues
redfire-switch bug list --priority
```

### `bug similar`

Check for similar issues before reporting.

**Options:**
- `--description <TEXT>` - Issue description for comparison
- `--threshold <FLOAT>` - Similarity threshold 0.0-1.0 (default: 0.7)

**Examples:**
```bash
# Check for similar issues
redfire-switch bug similar "SIP parsing error"

# With description
redfire-switch bug similar "Memory leak" --description "Memory usage increases over time"

# Lower threshold for more matches
redfire-switch bug similar "Codec issue" --threshold 0.5
```

### `bug report`

Submit a new bug report to GitHub.

**Required:**
- `--title <TITLE>` - Bug title/summary
- `--description <TEXT>` - Detailed description

**Optional:**
- `--steps <TEXT>` - Steps to reproduce
- `--expected <TEXT>` - Expected behavior
- `--actual <TEXT>` - Actual behavior
- `--severity <LEVEL>` - Severity: low, medium, high, critical (default: medium)
- `--component <COMPONENT>` - Affected component
- `--no-interactive` - Skip confirmations
- `--attach-sysinfo` - Include system information (default: true)
- `--attach-config` - Include configuration (sanitized) (default: true)
- `--attach-logs` - Include recent logs (default: true)

**Examples:**
```bash
# Basic bug report
redfire-switch bug report \
  --title "Switch crashes on startup" \
  --description "Switch fails to start with segmentation fault"

# Detailed bug report
redfire-switch bug report \
  --title "Authentication bypass vulnerability" \
  --description "Certain malformed requests bypass IP authentication" \
  --steps "1. Send request with crafted headers 2. Observe bypass" \
  --expected "Request should be blocked" \
  --actual "Request is processed normally" \
  --severity critical \
  --component security
```

### `bug diagnostics`

Generate comprehensive system diagnostic report.

**Options:**
- `--output <FILE>` - Output file (default: diagnostics-TIMESTAMP.json)
- `--include-config` - Include configuration (default: true)
- `--include-logs` - Include logs (default: true)
- `--include-metrics` - Include metrics (default: true)
- `--include-tests` - Include test results (default: true)

**Examples:**
```bash
# Generate diagnostics
redfire-switch bug diagnostics

# Custom output file
redfire-switch bug diagnostics --output my-diagnostics.json

# Minimal diagnostics
redfire-switch bug diagnostics --no-include-logs --no-include-tests
```

## Bug Report Best Practices

### 1. Search First

Always search for existing issues before submitting a new report:

```bash
redfire-switch bug search "your error message"
redfire-switch bug similar "Short description of your issue"
```

### 2. Provide Clear Information

**Good Title:**
- ❌ "Bug in SIP"
- ✅ "SIP INVITE parser crashes with malformed Contact header"

**Good Description:**
- What were you trying to do?
- What happened?
- What did you expect to happen?
- How can the issue be reproduced?

### 3. Include System Information

The bug reporter automatically includes:
- System information (OS, hardware, network)
- Redfire Switch version and configuration
- Recent logs and error messages
- Performance metrics

### 4. Choose Appropriate Severity

- **Critical** - Security vulnerabilities, data loss, complete system failure
- **High** - Major functionality broken, significant impact
- **Medium** - Moderate impact, workaround available
- **Low** - Minor issue, cosmetic problems

### 5. Specify Component

Help developers route your issue by specifying the affected component:
- `sip` - SIP protocol handling
- `routing` - Call routing engine
- `billing` - Billing and rating
- `media` - RTP/media handling
- `security` - Authentication and security
- `performance` - Performance issues
- `cli` - Command line interface

## Automatic Issue Detection

The bug reporter includes several features to improve issue quality:

### Duplicate Detection

Before submitting, the system checks for similar existing issues and warns you:

```bash
⚠ Found 2 potentially similar issues:
  #145: SIP parser crashes with malformed headers (open)
  #67: Authentication bypass in SIP handling (closed)

Continue with bug report submission? [y/N]:
```

### System Diagnostics

Automatically collected information includes:

- **OS Information**: Distribution, version, architecture, uptime
- **Hardware**: CPU cores, memory, disk usage
- **Network**: Interfaces, listening ports
- **Process**: Memory usage, file descriptors, threads
- **Configuration**: Endpoint count, enabled features (sanitized)
- **Logs**: Recent errors and warnings
- **Metrics**: Performance data, success rates
- **Tests**: Recent test results if available

### Privacy Protection

Sensitive information is automatically sanitized:
- Passwords and tokens are removed
- IP addresses are anonymized where appropriate
- Personally identifiable information is filtered

## Configuration

Bug reporter settings are stored in:
- Linux: `~/.config/redfire-switch/`
- macOS: `~/Library/Application Support/redfire-switch/`
- Windows: `%APPDATA%\redfire-switch\`

### Environment Variables

- `GITHUB_TOKEN` - GitHub personal access token
- `REDFIRE_BUG_REPO` - Override repository (default: carrierone/redfire-switch)

## Troubleshooting

### Authentication Issues

```bash
# Test GitHub connection
redfire-switch bug setup --test

# Check token permissions
# Token needs 'public_repo' permission
```

### Search Not Finding Issues

```bash
# Try broader search terms
redfire-switch bug search "SIP" --scope all

# Use similar issue detection
redfire-switch bug similar "your issue title"
```

### Report Submission Failed

1. Verify GitHub token: `redfire-switch bug setup --test`
2. Check network connectivity
3. Try submitting manually at: https://github.com/carrierone/redfire-switch/issues

## Integration with Development

### CI/CD Integration

Bug reports can be automatically created from test failures:

```bash
# In CI script
if ./run-tests.sh; then
  echo "Tests passed"
else
  redfire-switch bug report \
    --title "Test failure in CI build #$BUILD_NUMBER" \
    --description "Automated test suite failed" \
    --component testing \
    --no-interactive
fi
```

### Log Monitoring

Monitor logs and automatically report issues:

```bash
# Example monitoring script
tail -f /var/log/redfire-switch.log | while read line; do
  if echo "$line" | grep -q "FATAL"; then
    redfire-switch bug report \
      --title "Fatal error detected in logs" \
      --description "Fatal error: $line" \
      --severity high \
      --no-interactive
  fi
done
```

## Contributing

The bug reporter is open source and contributions are welcome:

1. Report issues with the bug reporter itself
2. Submit pull requests for improvements
3. Help triage and respond to bug reports

The bug reporter code is located in `src/bug_reporter.rs` and uses:
- GitHub REST API v3
- Fuzzy text matching for similarity detection
- System information collection from `/proc` and system commands
- Secure token storage with appropriate file permissions