# Trunk UUID Integration in STIR/SHAKEN Call Signing

## Overview

This document describes the implementation of trunk UUID identifiers in STIR/SHAKEN call signing for the Redfire Switch SIP platform. This enhancement provides end-to-end trunk traceability in PASSporT tokens.

## Implementation Details

### Data Structures

#### TrunkInfo Structure
```rust
pub struct TrunkInfo {
    /// Ingress trunk UUID identifier
    #[serde(rename = "x-ingress-trunk-uuid", skip_serializing_if = "Option::is_none")]
    pub ingress_trunk_uuid: Option<String>,
    /// Egress trunk UUID identifier
    #[serde(rename = "x-egress-trunk-uuid", skip_serializing_if = "Option::is_none")]  
    pub egress_trunk_uuid: Option<String>,
    /// Trunk routing timestamp
    #[serde(rename = "x-trunk-routing-ts", skip_serializing_if = "Option::is_none")]
    pub routing_timestamp: Option<i64>,
}
```

#### Updated CallInfo Structure
```rust
pub struct CallInfo {
    pub from_number: String,
    pub to_number: String,
    pub call_id: String,
    pub attestation: AttestationLevel,
    /// NEW: Ingress trunk UUID identifier
    pub ingress_trunk_uuid: Option<String>,
    /// NEW: Egress trunk UUID identifier  
    pub egress_trunk_uuid: Option<String>,
}
```

#### Enhanced PASSporT Payload
```rust
pub struct PassportPayload {
    pub attest: AttestationLevel,
    pub dest: DestinationInfo,
    pub iat: i64,
    pub orig: OriginationInfo,
    pub orig_id: String,
    /// NEW: Custom claims for trunk identification (extension)
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub trunk_info: Option<TrunkInfo>,
}
```

### Key Functions

#### Creating Call Info with Trunk UUIDs
```rust
// Basic call info (backward compatible)
pub fn create_call_info(
    &self,
    from_number: String,
    to_number: String,
    call_id: String,
    attestation: Option<AttestationLevel>,
) -> CallInfo

// Enhanced call info with trunk UUIDs
pub fn create_call_info_with_trunks(
    &self,
    from_number: String,
    to_number: String,
    call_id: String,
    attestation: Option<AttestationLevel>,
    ingress_trunk_uuid: Option<String>,
    egress_trunk_uuid: Option<String>,
) -> CallInfo
```

#### Extracting Trunk Information from PASSporT
```rust
// Extract complete trunk information from validated PASSporT
pub async fn extract_trunk_info_from_passport(&self, identity_header: &str) -> Result<Option<TrunkInfo>>

// Get specific trunk UUIDs
pub async fn get_ingress_trunk_uuid(&self, identity_header: &str) -> Result<Option<String>>
pub async fn get_egress_trunk_uuid(&self, identity_header: &str) -> Result<Option<String>>

// Log trunk routing information for debugging
pub async fn log_trunk_routing_info(&self, identity_header: &str, call_id: &str) -> Result<()>
```

### Usage Examples

#### Signing a Call with Trunk UUIDs
```rust
let service = StirShakenService::new(config).await?;

// Create call info with trunk UUIDs
let call_info = service.create_call_info_with_trunks(
    "+12125551234".to_string(),
    "+13105554321".to_string(),
    "unique-call-id".to_string(),
    Some(AttestationLevel::Full),
    Some("550e8400-e29b-41d4-a716-446655440001".to_string()), // ingress trunk UUID
    Some("550e8400-e29b-41d4-a716-446655440002".to_string()), // egress trunk UUID
);

// Create PASSporT token (automatically includes trunk info)
let passport = service.create_passport(&call_info, None).await?;

// Create SIP Identity header
let identity_header = service.create_identity_header(&call_info, None).await?;
```

#### Validating and Extracting Trunk Information
```rust
// Validate incoming call and extract trunk routing info
let attestation = service.validate_call(&identity_header, &from_number).await?;

// Extract trunk UUIDs from validated call
let ingress_uuid = service.get_ingress_trunk_uuid(&identity_header).await?;
let egress_uuid = service.get_egress_trunk_uuid(&identity_header).await?;

// Log complete trunk routing information
service.log_trunk_routing_info(&identity_header, &call_id).await?;
```

### JSON Serialization Format

When trunk UUIDs are present, the PASSporT payload JSON includes custom extension fields:

```json
{
  "attest": "A",
  "dest": {
    "tn": ["+13105554321"]
  },
  "iat": 1609459200,
  "orig": {
    "tn": "+12125551234"
  },
  "origid": "example-sp",
  "x-ingress-trunk-uuid": "550e8400-e29b-41d4-a716-446655440001",
  "x-egress-trunk-uuid": "550e8400-e29b-41d4-a716-446655440002",
  "x-trunk-routing-ts": 1609459200
}
```

When no trunk UUIDs are present, these fields are omitted for backward compatibility:

```json
{
  "attest": "C",
  "dest": {
    "tn": ["+13105554321"]
  },
  "iat": 1609459200,
  "orig": {
    "tn": "+12125551234"
  },
  "origid": "example-sp"
}
```

### Benefits

1. **End-to-End Traceability**: Track calls through specific ingress and egress trunks
2. **Fraud Detection**: Identify unusual routing patterns or trunk usage
3. **Billing Accuracy**: Precise trunk-level billing and cost allocation
4. **Troubleshooting**: Rapid identification of problematic trunks in call quality issues
5. **Compliance**: Meet regulatory requirements for call path documentation
6. **Interoperability**: Standard STIR/SHAKEN extension compatible with other switches

### Backward Compatibility

- Existing code using `create_call_info()` continues to work without modification
- PASSporT tokens without trunk information are handled gracefully
- STIR/SHAKEN validation works with both trunk-aware and legacy tokens
- Extension fields use `x-` prefix following RFC conventions for custom claims

### Security Considerations

- Trunk UUIDs are included in signed PASSporT tokens, providing cryptographic integrity
- UUIDs do not expose sensitive routing information to unauthorized parties
- Verification follows standard STIR/SHAKEN certificate validation procedures
- Custom claims are protected by the same cryptographic signatures as standard claims