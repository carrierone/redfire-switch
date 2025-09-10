# RedFire Switch Web UI - Usage Guide

## Overview

The RedFire Switch Web UI is a modern, responsive web administration interface for managing the RedFire Switch telephony system. Built with Rust and featuring a clean, intuitive design, it provides real-time monitoring, call management, and system configuration capabilities.

## Features

✅ **Real-time Dashboard** - Live system statistics and call monitoring  
✅ **Active Call Management** - View, filter, and control active calls  
✅ **System Configuration** - Modify API, authentication, and rate limiting settings  
✅ **Performance Monitoring** - System health, resource usage, and performance metrics  
✅ **Unix Socket Support** - Secure local communication by default  
✅ **JWT Authentication** - Role-based access control with secure sessions  
✅ **Responsive Design** - Works on desktop, tablet, and mobile devices  

## Quick Start

### Prerequisites

1. **RedFire Switch** must be running with the API server enabled
2. **Rust toolchain** (cargo) for building the web UI
3. A **modern web browser** (Chrome, Firefox, Safari, Edge)

### 1. Start the RedFire Switch API Server

First, ensure the RedFire Switch is running with the Unix socket API enabled:

```bash
# Start the standalone API server (for testing)
cargo run --bin standalone-api-server

# OR start the full RedFire Switch with API enabled
cargo run --bin redfire-cli -- --enable-api --unix-socket /var/run/redfire-switch/api.sock
```

The API server will create a Unix socket at `/var/run/redfire-switch/api.sock` by default.

### 2. Build and Start the Web UI

```bash
# Build the web UI
cargo build --bin redfire-web-ui --release

# Start the web UI server
cargo run --bin redfire-web-ui
```

### 3. Access the Web Interface

Open your web browser and navigate to:

```
http://localhost:3000
```

You'll be redirected to the login page automatically.

## Default Credentials

For initial setup and testing, use the default administrator credentials:

- **Username**: `admin`
- **Password**: `admin123`

⚠️ **Important**: Change these credentials immediately in a production environment!

## Configuration Options

### Web UI Command Line Options

```bash
cargo run --bin redfire-web-ui -- --help
```

Available options:

```
RedFire Switch Web Administration UI

Usage: redfire-web-ui [OPTIONS]

Options:
  -p, --port <PORT>              Port to bind the web UI server to [default: 3000]
  -b, --bind <BIND>              IP address to bind to [default: 127.0.0.1]
  -s, --socket-path <SOCKET_PATH>  Unix socket path to connect to RedFire Switch API [default: /var/run/redfire-switch/api.sock]
      --switch-url <SWITCH_URL>  HTTP endpoint for switch API (alternative to Unix socket)
  -d, --dev                      Enable development mode (additional logging, etc.)
  -h, --help                     Print help
```

### Example Configurations

#### Local Development Setup
```bash
cargo run --bin redfire-web-ui -- --dev --port 3000 --socket-path /tmp/redfire-switch-dev.sock
```

#### Production Setup (HTTP API)
```bash
cargo run --bin redfire-web-ui --release -- --bind 0.0.0.0 --port 8080 --switch-url http://localhost:8080
```

#### Custom Unix Socket
```bash
cargo run --bin redfire-web-ui -- --socket-path /custom/path/to/api.sock
```

## Using the Web UI

### Dashboard

The main dashboard provides:

- **System Status Cards**: Active calls, calls per second, memory usage, system status
- **Recent Activity**: Last 10 calls with status information
- **System Information**: Version, uptime, total calls, connection status
- **Quick Actions**: Direct access to common operations
- **Trunk Status**: Real-time status of configured trunks

Data refreshes automatically every 5 seconds.

### Active Calls Management

Access via the "Active Calls" tab:

- **Real-time Call List**: All active calls with detailed information
- **Call Filtering**: Filter by status, search by number or call ID
- **Call Details**: Click any call ID to view complete call information
- **Call Control**: Hang up calls directly from the interface
- **Statistics**: Call volume, success rates, and duration metrics

### System Configuration

Access via the "Configuration" tab:

#### API Server Settings
- HTTP and Unix socket listener configuration
- Request size limits and timeouts
- CORS settings

#### Authentication Configuration
- JWT token expiration settings
- Failed login attempt limits
- Multi-factor authentication options

#### Rate Limiting
- Per-IP and per-user request limits
- Burst size and window duration settings

#### Monitoring Options
- Request logging configuration
- Metrics and documentation endpoints

### Performance Monitoring

Access via the "Monitoring" tab:

- **System Health**: CPU, memory, disk, and network usage
- **Call Statistics**: 24-hour call volumes and success rates
- **Error Logs**: Recent warnings and errors
- **Network Connections**: Status of all network listeners
- **Performance Metrics**: Request rates, response times, queue lengths
- **System Resources**: Detailed resource utilization

Data refreshes automatically every 10 seconds.

## Security Considerations

### Authentication

- Web UI uses JWT tokens for session management
- Sessions expire after 8 hours by default
- Failed login attempts are tracked and accounts can be locked
- All API calls require valid authentication

### Network Security

- **Unix Socket** (Default): Most secure option, local communication only
- **HTTP API**: For remote access, ensure proper network security
- **HTTPS**: Use a reverse proxy (nginx, Apache) for SSL/TLS termination
- **Firewall**: Restrict access to the web UI port (default 3000)

### Production Deployment

For production environments:

1. Change default credentials immediately
2. Use Unix socket communication when possible
3. Deploy behind a reverse proxy with HTTPS
4. Configure appropriate firewall rules
5. Enable rate limiting
6. Disable development mode
7. Monitor system logs

## Troubleshooting

### Connection Issues

**"Cannot connect to switch"**
- Verify RedFire Switch is running
- Check Unix socket path is correct
- Ensure web UI has permissions to access the socket
- For HTTP mode, verify the switch URL is accessible

**"Authentication failed"**
- Verify credentials are correct
- Check if account is locked due to failed attempts
- Ensure RedFire Switch authentication is configured

### Performance Issues

**Slow loading times**
- Check system resources on the server
- Verify network connectivity
- Monitor for high call volumes affecting API performance

**UI not updating**
- Check browser console for JavaScript errors
- Verify WebSocket connections (if applicable)
- Clear browser cache and cookies

### Common Error Messages

| Error | Cause | Solution |
|-------|--------|----------|
| "Session expired" | JWT token expired | Re-login to the interface |
| "Permission denied" | Insufficient user permissions | Contact administrator for role updates |
| "API timeout" | Switch not responding | Check switch health and restart if needed |
| "Connection refused" | Socket/port unavailable | Verify switch is running and accessible |

## API Integration

The web UI acts as a proxy to the RedFire Switch API. All endpoints are available via:

```
http://localhost:3000/api/switch/{endpoint}
```

Example API calls from external tools:

```bash
# Get system statistics
curl http://localhost:3000/api/switch/system/stats \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"

# List active calls  
curl http://localhost:3000/api/switch/calls \
  -H "Authorization: Bearer YOUR_JWT_TOKEN"
```

## File Structure

```
web-ui/
├── dashboard.html          # Main dashboard page
├── login.html             # Authentication page
├── calls.html             # Call management page
├── config.html            # Configuration page
├── monitoring.html        # System monitoring page
└── static/
    ├── css/
    │   └── main.css       # Main stylesheet
    ├── js/
    │   ├── main.js        # Core JavaScript functions
    │   ├── login.js       # Login page functionality
    │   ├── dashboard.js   # Dashboard page functionality
    │   └── calls.js       # Calls page functionality
    └── img/               # Static images
```

## Contributing

To contribute to the web UI:

1. Follow the existing code structure and patterns
2. Test all functionality thoroughly
3. Ensure responsive design compatibility
4. Update documentation for new features
5. Follow security best practices

## Version Information

- **Web UI Version**: 1.0.0
- **Compatible with**: RedFire Switch v0.1.0+
- **Minimum Browser**: Chrome 90+, Firefox 88+, Safari 14+

## Support

For support and bug reports, please visit the RedFire Switch GitHub repository.

---

**© 2025 Carrier One Inc. - RedFire Switch Web UI**