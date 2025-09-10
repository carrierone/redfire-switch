# Redfire Switch REST API

This document describes the comprehensive REST API for the Redfire Switch, a high-performance Class 4 SIP telephone switch.

## Features

- **Complete Authentication System** - JWT-based authentication with role-based access control
- **Multiple Network Listeners** - IPv4, IPv6, and Unix socket support with configurable bindings
- **Live Call Monitoring** - Real-time call tracking and management
- **Configuration Management** - Dynamic configuration reloading
- **Endpoint Monitoring** - SIP endpoint health checking and status reporting
- **OpenAPI/Swagger Documentation** - Complete API documentation with interactive testing
- **Security Features** - Rate limiting, permission-based access control, secure defaults

## Quick Start

### Development Mode

```bash
# Start the API server in development mode
cargo run --bin redfire-api-server -- --mode development

# API available at:
# - HTTP: http://127.0.0.1:8080
# - Unix socket: /tmp/redfire-switch-dev.sock
# - Swagger UI: http://127.0.0.1:8080/swagger-ui
```

### Default Credentials

- **Username**: `admin`
- **Password**: `admin123`

⚠️ **IMPORTANT**: Change the default password immediately in production!

## Authentication

### Login

```bash
curl -X POST http://127.0.0.1:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "password": "admin123"
  }'
```

Response:
```json
{
  "success": true,
  "data": {
    "token": "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...",
    "expires_at": "2025-01-08T12:00:00Z",
    "user": {
      "id": "admin",
      "username": "admin",
      "email": "admin@redfire-switch.local",
      "roles": ["admin"],
      "permissions": ["SystemAdmin", "CallsWrite", ...]
    }
  },
  "timestamp": "2025-01-07T12:00:00Z"
}
```

### Using the JWT Token

Include the token in the Authorization header for protected endpoints:

```bash
export JWT_TOKEN="eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9..."
curl -H "Authorization: Bearer $JWT_TOKEN" \
  http://127.0.0.1:8080/api/v1/system/health
```

## API Endpoints

### Authentication
- `POST /api/v1/auth/login` - User authentication
- `POST /api/v1/auth/logout` - User logout  
- `GET /api/v1/auth/me` - Get current user information

### System Management
- `GET /api/v1/system/stats` - Get system statistics
- `GET /api/v1/system/health` - Get system health status
- `POST /api/v1/system/config/reload` - Reload system configuration

### Call Management
- `GET /api/v1/calls` - List active calls (paginated)
- `GET /api/v1/calls/live` - Get live call information with metrics
- `GET /api/v1/calls/{call_id}` - Get specific call information
- `POST /api/v1/calls/{call_id}/hangup` - Hangup a specific call

### DID/Number Management
- `GET /api/v1/dids` - List DIDs (paginated)
- `POST /api/v1/dids` - Create new DID
- `GET /api/v1/dids/{number}` - Get DID information
- `PUT /api/v1/dids/{number}` - Update DID configuration
- `DELETE /api/v1/dids/{number}` - Delete DID

### Customer Management
- `GET /api/v1/customers` - List customers (paginated)
- `GET /api/v1/customers/{customer_id}` - Get customer information

### SMS Management
- `GET /api/v1/sms/messages` - List SMS messages (paginated)
- `POST /api/v1/sms/messages` - Send SMS message
- `GET /api/v1/sms/messages/{message_id}` - Get SMS message information

### Endpoint Monitoring
- `GET /api/v1/monitoring/endpoints` - Get SIP endpoint health status

## Network Listeners Configuration

The API server supports multiple network binding configurations:

### IPv4 and IPv6 HTTP/HTTPS Listeners

```rust
HttpListener {
    enabled: true,
    bind_address: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
    port: 8080,
    protocol: HttpProtocol::Http,
    name: "localhost-http".to_string(),
    description: "Local HTTP API endpoint".to_string(),
}
```

### Unix Socket Listeners (Default)

```rust
UnixListener {
    enabled: true,
    socket_path: "/var/run/redfire-switch/api.sock".into(),
    name: "main-unix".to_string(),
    description: "Main Unix socket API endpoint".to_string(),
    file_permissions: 0o600, // Secure by default
}
```

### Custom Configuration Example

```bash
# Custom HTTP binding
cargo run --bin redfire-api-server -- \
  --mode custom \
  --bind-http "0.0.0.0:8080" \
  --bind-unix "/tmp/custom.sock" \
  --enable-ipv6
```

## Examples

### Live Call Monitoring

```bash
# Get live calls with authentication
curl -H "Authorization: Bearer $JWT_TOKEN" \
  "http://127.0.0.1:8080/api/v1/calls/live?page=1&limit=10"
```

Response:
```json
{
  "success": true,
  "data": {
    "active_calls": [
      {
        "call_id": "abc123-def456-ghi789",
        "from": "+15551234567",
        "to": "+15557654321",
        "status": "Answered",
        "start_time": "2025-01-07T12:00:00Z",
        "duration_seconds": 145,
        "ingress_trunk": "trunk-001",
        "egress_trunk": "trunk-002",
        "codec": "G.711",
        "quality_metrics": {
          "packet_loss_percent": 0.1,
          "jitter_ms": 2.5,
          "rtt_ms": 45.0,
          "mos_score": 4.2
        }
      }
    ],
    "total_count": 1,
    "status_breakdown": {
      "Answered": 1
    },
    "last_updated": "2025-01-07T12:05:00Z"
  },
  "timestamp": "2025-01-07T12:05:00Z"
}
```

### Configuration Reload

```bash
# Reload configuration with validation
curl -X POST -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  http://127.0.0.1:8080/api/v1/system/config/reload \
  -d '{
    "force": false,
    "validate_only": false
  }'
```

### Hangup Call

```bash
# Hangup a specific call
curl -X POST -H "Authorization: Bearer $JWT_TOKEN" \
  -H "Content-Type: application/json" \
  http://127.0.0.1:8080/api/v1/calls/abc123-def456-ghi789/hangup \
  -d '{
    "reason": "Administrative hangup",
    "force": false
  }'
```

### Endpoint Health Check

```bash
# Get SIP endpoint health
curl -H "Authorization: Bearer $JWT_TOKEN" \
  http://127.0.0.1:8080/api/v1/monitoring/endpoints
```

Response:
```json
{
  "success": true,
  "data": [
    {
      "name": "sip-gateway-001",
      "status": "online",
      "last_check": "2025-01-07T12:00:00Z",
      "last_response_time_ms": 45,
      "consecutive_failures": 0,
      "success_rate": 98.5,
      "total_pings": 1000,
      "successful_pings": 985
    }
  ],
  "timestamp": "2025-01-07T12:05:00Z"
}
```

## Unix Socket Usage

```bash
# Using curl with Unix socket
curl --unix-socket /var/run/redfire-switch/api.sock \
  -X POST -H "Content-Type: application/json" \
  http://localhost/api/v1/auth/login \
  -d '{"username": "admin", "password": "admin123"}'

# Using socat for interactive testing
socat - UNIX-CONNECT:/var/run/redfire-switch/api.sock
```

## Permission System

The API implements a comprehensive role-based access control (RBAC) system:

### Built-in Roles

1. **Admin** - Full system access
2. **Operator** - Call management and monitoring  
3. **ReadOnly** - View-only access for monitoring

### Available Permissions

- `SystemAdmin` - Full system administration
- `CallsRead/Write/Hangup` - Call management
- `ConfigRead/Write/Reload` - Configuration management
- `MonitoringRead/Write` - System monitoring
- `CustomerAdmin` - Customer management
- `DidAdmin` - DID/number management
- `SmsAdmin` - SMS management
- And more...

## Production Deployment

### Production Mode

```bash
cargo run --bin redfire-api-server -- --mode production
```

Production mode includes:
- HTTPS-only binding (port 8443)
- Secure Unix socket with restrictive permissions
- No CORS enabled
- Stricter rate limiting
- Security-focused logging

### Security Considerations

1. **Change default credentials** immediately
2. **Use HTTPS** in production with valid certificates
3. **Configure firewall** rules appropriately
4. **Set secure file permissions** on Unix sockets
5. **Use environment variables** for sensitive configuration
6. **Enable audit logging** for security events
7. **Regular security updates** and monitoring

### TLS Configuration

```toml
[tls]
cert_path = "/etc/redfire-switch/tls/server.crt"
key_path = "/etc/redfire-switch/tls/server.key"
ca_cert_path = "/etc/redfire-switch/tls/ca.crt"  # Optional
require_client_cert = false
min_version = "1.3"
```

## API Documentation

Interactive API documentation is available via Swagger UI:

- **Development**: http://127.0.0.1:8080/swagger-ui
- **OpenAPI JSON**: http://127.0.0.1:8080/api-docs/openapi.json

The documentation includes:
- Complete endpoint descriptions
- Request/response schemas
- Authentication requirements
- Interactive testing interface
- Code examples in multiple languages

## Error Handling

All API responses follow a consistent format:

```json
{
  "success": false,
  "data": null,
  "error": "Detailed error message",
  "timestamp": "2025-01-07T12:00:00Z"
}
```

Common HTTP status codes:
- `200` - Success
- `201` - Created
- `400` - Bad Request
- `401` - Unauthorized
- `403` - Forbidden
- `404` - Not Found
- `409` - Conflict
- `429` - Rate Limited
- `500` - Internal Server Error

## Rate Limiting

Default rate limits:
- **Per IP**: 100 requests/minute
- **Per User**: 1000 requests/minute
- **Burst Size**: 10 requests

Rate limit headers are included in responses:
- `X-RateLimit-Limit`
- `X-RateLimit-Remaining`
- `X-RateLimit-Reset`

## Testing

Run the comprehensive test suite:

```bash
cargo test api
```

The tests cover:
- Authentication flows
- Permission validation
- API endpoint functionality
- Configuration validation
- Error handling
- Concurrent operations

## Contributing

See the main README for contribution guidelines. For API-specific contributions:

1. Add tests for new endpoints
2. Update OpenAPI documentation
3. Follow existing patterns for authentication/authorization
4. Ensure backward compatibility
5. Add appropriate logging and error handling

## License

GPL-3.0-or-later - See LICENSE file for details.

## Support

- **Issues**: https://github.com/carrierone/redfire-switch/issues
- **Email**: support@carrierone.com
- **Website**: https://www.carrierone.com