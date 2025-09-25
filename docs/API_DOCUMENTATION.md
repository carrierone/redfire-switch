# Redfire Switch API Documentation

## Overview

The Redfire Switch provides a comprehensive REST API for managing a Class 4 telecommunications switch with advanced features including:

- **Call Management**: Real-time call control and monitoring
- **SIP Processing**: Advanced SIP protocol handling with B2BUA capabilities
- **Route Management**: Least Cost Routing (LCR) with dynamic route optimization
- **Codec Transcoding**: High-performance audio codec conversion with GPU acceleration
- **Compliance**: ECPA-compliant lawful intercept and voice integrity monitoring
- **Anti-Fraud**: Real-time fraud detection and prevention
- **Performance Monitoring**: Comprehensive system performance and optimization

## Base URL

```
https://api.redfire-switch.local/api/v1
```

## Authentication

All API endpoints require authentication using JWT tokens.

### Login

```http
POST /auth/login
Content-Type: application/json

{
  "username": "admin",
  "password": "secure_password"
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
    "expires_at": "2025-01-25T12:00:00Z",
    "user": {
      "id": "admin",
      "username": "admin",
      "email": "admin@redfire-switch.local",
      "roles": ["admin"],
      "permissions": ["SystemAdmin"],
      "last_login": null
    }
  }
}
```

### Using the Token

Include the token in the Authorization header:

```http
Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...
```

## Core Endpoints

### System Status

#### Get System Statistics

```http
GET /system/stats
```

**Response:**
```json
{
  "success": true,
  "data": {
    "active_calls": 1250,
    "total_calls": 45678,
    "sms_messages": 0,
    "uptime_seconds": 86400,
    "memory_usage": {
      "used_bytes": 2147483648,
      "total_bytes": 8589934592,
      "usage_percent": 25.0
    },
    "trunk_stats": [
      {
        "trunk_id": 1,
        "name": "Primary SIP Trunk",
        "active_calls": 150,
        "total_capacity": 500,
        "utilization_percent": 30.0,
        "status": "online"
      }
    ]
  }
}
```

#### Health Check

```http
GET /system/health
```

**Response:**
```json
{
  "success": true,
  "data": {
    "status": "healthy",
    "timestamp": "2025-01-24T10:30:00Z",
    "components": {
      "database": "healthy",
      "sip_stack": "healthy",
      "media_engine": "healthy",
      "performance_monitor": "healthy"
    }
  }
}
```

### Call Management

#### Get Active Calls

```http
GET /calls?status=active&limit=100&offset=0
```

**Parameters:**
- `status`: Filter by call status (`active`, `ringing`, `completed`)
- `limit`: Maximum results (default: 50, max: 1000)
- `offset`: Pagination offset (default: 0)
- `trunk_id`: Filter by trunk ID
- `customer_id`: Filter by customer ID

**Response:**
```json
{
  "success": true,
  "data": {
    "calls": [
      {
        "call_id": "call-123e4567-e89b-12d3-a456-426614174000",
        "session_id": "session-456",
        "from": "+18001234567",
        "to": "+18009876543",
        "status": "active",
        "start_time": "2025-01-24T10:25:00Z",
        "duration_seconds": 300,
        "trunk_id": 1,
        "customer_id": "cust-001",
        "codec": "G711U",
        "quality_score": 4.2
      }
    ],
    "pagination": {
      "total": 1250,
      "limit": 100,
      "offset": 0,
      "has_more": true
    }
  }
}
```

#### Terminate Call

```http
DELETE /calls/{call_id}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "call_id": "call-123e4567-e89b-12d3-a456-426614174000",
    "status": "terminated",
    "reason": "admin_disconnect"
  }
}
```

### Route Management

#### Get Routes

```http
GET /routes?ani=18001234567&dnis=18009876543
```

**Parameters:**
- `ani`: Automatic Number Identification
- `dnis`: Dialed Number Identification Service
- `customer_id`: Customer identifier
- `active_only`: Only return active routes (boolean)

**Response:**
```json
{
  "success": true,
  "data": {
    "routes": [
      {
        "route_id": 1001,
        "name": "Premium Route",
        "trunk_id": 1,
        "priority": 1,
        "cost_per_minute": "0.0085",
        "jurisdiction": "interstate",
        "quality_score": 4.5,
        "capacity_percent": 75.0,
        "active": true
      }
    ],
    "jurisdiction": "interstate",
    "lrn": "18009876543",
    "total_routes": 5
  }
}
```

#### Update Route Priority

```http
PATCH /routes/{route_id}
Content-Type: application/json

{
  "priority": 2,
  "active": true
}
```

### Trunk Management

#### Get Trunk Status

```http
GET /trunks
```

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "trunk_id": 1,
      "name": "Primary SIP Trunk",
      "host": "sip.carrier.com",
      "port": 5060,
      "protocol": "UDP",
      "active_calls": 150,
      "total_capacity": 500,
      "status": "online",
      "last_heartbeat": "2025-01-24T10:30:00Z",
      "quality_metrics": {
        "packet_loss": 0.01,
        "jitter_ms": 2.5,
        "latency_ms": 45.2
      }
    }
  ]
}
```

#### Update Trunk Configuration

```http
PUT /trunks/{trunk_id}
Content-Type: application/json

{
  "name": "Updated Trunk Name",
  "capacity": 600,
  "active": true,
  "priority": 1
}
```

### Performance Monitoring

#### Get Performance Metrics

```http
GET /performance/metrics
```

**Response:**
```json
{
  "success": true,
  "data": {
    "timestamp": "2025-01-24T10:30:00Z",
    "cpu_usage_percent": 25.5,
    "memory_usage_bytes": 2147483648,
    "memory_available_bytes": 6442450944,
    "active_calls": 1250,
    "calls_per_second": 5.2,
    "codec_metrics": [
      {
        "codec_type": "G711U",
        "operation": "Transcode",
        "average_processing_time_us": 150,
        "frames_per_second": 6666.7,
        "samples_processed": 10000
      }
    ],
    "database_metrics": [
      {
        "query_type": "route_lookup",
        "average_query_time_ms": 2.5,
        "queries_per_second": 400.0,
        "cache_hit_rate_percent": 92.5
      }
    ]
  }
}
```

#### Get Optimization Recommendations

```http
GET /performance/optimizations
```

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "category": "Database",
      "priority": "High",
      "title": "Slow route_lookup Queries",
      "description": "Route lookup queries average 15.2ms. Consider adding composite index on (ani, dnis).",
      "estimated_improvement": "70-90% query time reduction",
      "implementation_effort": "Low"
    }
  ]
}
```

### Anti-Fraud Monitoring

#### Get Fraud Statistics

```http
GET /anti-fraud/statistics
```

**Response:**
```json
{
  "success": true,
  "data": {
    "period": "24h",
    "total_calls_analyzed": 25000,
    "suspicious_calls": 15,
    "blocked_calls": 3,
    "false_positive_rate": 0.02,
    "detection_categories": {
      "short_duration": 8,
      "unusual_patterns": 4,
      "banned_destinations": 3
    },
    "cost_savings": "1250.00"
  }
}
```

#### Get Suspicious Call Details

```http
GET /anti-fraud/suspicious-calls?limit=50
```

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "call_id": "call-suspicious-001",
      "from": "+18005551234",
      "to": "+19005551234",
      "detected_at": "2025-01-24T10:15:00Z",
      "risk_score": 0.85,
      "detection_reasons": [
        "Short duration call",
        "Unusual call pattern"
      ],
      "action_taken": "monitored",
      "cost_impact": "25.00"
    }
  ]
}
```

### Voice Integrity & Legal Authorization

#### Get Legal Authorizations

```http
GET /voice-integrity/authorizations
```

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "authorization_id": 1001,
      "case_number": "CASE-2025-001",
      "authorization_type": "CourtOrder",
      "status": "Active",
      "target_numbers": ["+18005551234"],
      "authorized_by": "Judge Smith",
      "start_date": "2025-01-20T00:00:00Z",
      "end_date": "2025-02-20T23:59:59Z",
      "recording_enabled": true,
      "transcription_enabled": true
    }
  ]
}
```

#### Get Voice Recordings

```http
GET /voice-integrity/recordings?authorization_id=1001
```

**Response:**
```json
{
  "success": true,
  "data": [
    {
      "recording_id": "rec-001",
      "call_id": "call-intercept-001",
      "authorization_id": 1001,
      "start_time": "2025-01-24T09:30:00Z",
      "duration_seconds": 180,
      "file_size_bytes": 1440000,
      "storage_type": "encrypted_disk",
      "transcription_available": true,
      "chain_of_custody": [
        {
          "timestamp": "2025-01-24T09:30:00Z",
          "action": "recorded",
          "user": "system"
        }
      ]
    }
  ]
}
```

## WebSocket API

For real-time updates, connect to the WebSocket endpoint:

```
wss://api.redfire-switch.local/ws
```

### Event Types

#### Call Events

```json
{
  "event_type": "call_started",
  "timestamp": "2025-01-24T10:30:00Z",
  "data": {
    "call_id": "call-123",
    "from": "+18001234567",
    "to": "+18009876543",
    "trunk_id": 1
  }
}
```

#### System Events

```json
{
  "event_type": "system_alert",
  "timestamp": "2025-01-24T10:30:00Z",
  "data": {
    "alert_type": "high_cpu",
    "severity": "warning",
    "message": "CPU usage at 85%",
    "metrics": {
      "cpu_percent": 85.2
    }
  }
}
```

## Error Handling

All API responses follow a consistent format:

### Success Response
```json
{
  "success": true,
  "data": { ... }
}
```

### Error Response
```json
{
  "success": false,
  "error": {
    "code": "INVALID_CREDENTIALS",
    "message": "Invalid username or password",
    "details": {
      "field": "password",
      "reason": "incorrect"
    }
  }
}
```

### Common Error Codes

- `INVALID_CREDENTIALS` - Authentication failed
- `INSUFFICIENT_PERMISSIONS` - User lacks required permissions
- `RESOURCE_NOT_FOUND` - Requested resource doesn't exist
- `VALIDATION_ERROR` - Request validation failed
- `RATE_LIMIT_EXCEEDED` - Too many requests
- `SYSTEM_ERROR` - Internal server error
- `MAINTENANCE_MODE` - System in maintenance mode

## Rate Limits

- **General API**: 1000 requests per minute per token
- **Real-time endpoints**: 60 requests per minute per token
- **Bulk operations**: 10 requests per minute per token

Rate limit information is included in response headers:

```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 995
X-RateLimit-Reset: 1642694400
```

## Pagination

List endpoints support pagination:

```http
GET /calls?limit=100&offset=200
```

**Parameters:**
- `limit`: Items per page (max: 1000, default: 50)
- `offset`: Number of items to skip (default: 0)

**Response includes pagination metadata:**
```json
{
  "success": true,
  "data": {
    "items": [...],
    "pagination": {
      "total": 5000,
      "limit": 100,
      "offset": 200,
      "has_more": true
    }
  }
}
```

## SDK Examples

### cURL

```bash
# Login and get token
TOKEN=$(curl -s -X POST https://api.redfire-switch.local/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"password"}' | \
  jq -r '.data.token')

# Get system stats
curl -H "Authorization: Bearer $TOKEN" \
  https://api.redfire-switch.local/api/v1/system/stats
```

### Python

```python
import requests

# Login
response = requests.post(
    'https://api.redfire-switch.local/api/v1/auth/login',
    json={'username': 'admin', 'password': 'password'}
)
token = response.json()['data']['token']

# Get active calls
headers = {'Authorization': f'Bearer {token}'}
calls = requests.get(
    'https://api.redfire-switch.local/api/v1/calls',
    headers=headers,
    params={'status': 'active', 'limit': 100}
)
```

### JavaScript

```javascript
// Login
const loginResponse = await fetch('/api/v1/auth/login', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    username: 'admin',
    password: 'password'
  })
});
const { token } = (await loginResponse.json()).data;

// Get system metrics
const metricsResponse = await fetch('/api/v1/performance/metrics', {
  headers: { 'Authorization': `Bearer ${token}` }
});
const metrics = await metricsResponse.json();
```

## OpenAPI Specification

The complete OpenAPI 3.0 specification is available at:

```
GET /api-docs/openapi.json
```

Interactive Swagger UI documentation:

```
GET /swagger-ui/
```

## Support

For API support and questions:

- **Documentation**: https://docs.redfire-switch.local
- **Support**: support@carrierone.com
- **Status Page**: https://status.redfire-switch.local