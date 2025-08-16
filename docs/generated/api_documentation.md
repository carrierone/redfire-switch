# Redfire Switch API Documentation

## Module Overview

### sip_authentication Module

**Location**: `./src/sip_authentication.rs`
**Lines of Code**: 636

#### Public Functions

- `is_ip_authorized` (async)
- `validate_tech_prefix` (async)
- `get_tech_prefix` (async)
- `verify_digest` (async)
- `new` (async)
- `is_rate_limited` (async)
- `new` (async)
- `set_fail2ban_service` (async)
- `add_ip_auth_config` (async)
- `add_digest_credentials` (async)
- `authenticate_request` (async)
- `create_challenge_response` (async)
- `load_ip_auth_configs` (async)

#### Public Structs

- `IpAuthConfig`
- `DigestCredentials`
- `RateLimiter`
- `SipAuthenticator`

#### Dependencies

- fail2ban
- anyhow
- tracing
- std
- ipnet
- proper
- rand
- serde
- rsip

### emergency_cli Module

**Location**: `./src/emergency_cli.rs`
**Lines of Code**: 514

#### Public Functions

- `handle_emergency_command` (async)

#### Dependencies

- anyhow
- clap
- std
- emergency_routing

### handlers Module

**Location**: `./src/handlers.rs`
**Lines of Code**: 233

#### Public Functions

- `handle_routing_command` (async)
- `handle_cdr_command` (async)

#### Dependencies

- config
- anyhow
- std
- routing
- cli
- at
- connection
- chrono
- cdr

### sipt_sipi Module

**Location**: `./src/sipt_sipi.rs`
**Lines of Code**: 1222

#### Public Functions

- `new`
- `parse_isup_message`
- `create_isup_message`
- `create_sipt_body`
- `parse_sipt_body`
- `create_sipi_body`
- `parse_sipi_body`
- `extract_calling_number`
- `extract_called_number`
- `sip_to_iam`
- `is_sipt_enabled`
- `is_sipi_enabled`
- `get_config`
- `isup_to_sip_method`
- `sip_to_isup_type`
- `get_next_cic`
- `validate_isup_message`
- `format_isup_debug`

#### Public Structs

- `IsupParameter`
- `IsupMessage`
- `ForwardCallIndicators`
- `BackwardCallIndicators`
- `SipTSipIConfig`
- `SipTSipIService`

#### Dependencies

- serde
- Indicators
- super
- anyhow
- nom
- std
- tracing
- bitflags

### sip_compliance Module

# SIP RFC Compliance and Compatibility Documentation
//! This module documents all RFC implementations and compatibility requirements
for interoperating with major SIP stacks in Class 4 switching environments.
//! ## Core RFCs Implemented
//! ### **RFC 3261 - SIP: Session Initiation Protocol**
- **Status**: REQUIRED - Fully Implemented
- **Purpose**: Core SIP 2.0 specification
- **Key Features**:
- Request/Response message structure
- Transaction state machines
- Dialog state management
- Routing and record routing
- Authentication framework
- Registration procedures
- **Interop Notes**: 
- SOFIA SIP: Strict header validation required
- PJSIP: Flexible Contact header handling
- Asterisk: Custom header support needed
- FreeSWITCH: Symmetric RTP support required
//! ### **RFC 3262 - Reliability of Provisional Responses in SIP (PRACK)**
- **Status**: RECOMMENDED - Implemented
- **Purpose**: Reliable delivery of 1xx responses
- **Interop Notes**:
- SOFIA SIP: Full PRACK support
- PJSIP: Configurable PRACK support
- Asterisk: Limited PRACK support
- FreeSWITCH: Full PRACK support via mod_sofia
//! ### **RFC 3263 - SIP: Locating SIP Servers**
- **Status**: REQUIRED - Implemented
- **Purpose**: DNS SRV and NAPTR resolution for SIP
- **Key Features**:
- SRV record lookups (_sip._udp, _sip._tcp, _sips._tcp)
- NAPTR record processing
- Transport protocol selection
- Failover mechanisms
//! ### **RFC 3264 - An Offer/Answer Model with SDP**
- **Status**: REQUIRED - Implemented
- **Purpose**: SDP negotiation framework
- **Interop Notes**:
- SOFIA SIP: Strict SDP format requirements
- PJSIP: Flexible SDP parsing
- Asterisk: Custom SDP attributes support
- FreeSWITCH: Advanced SDP manipulation
//! ### **RFC 3265 - Session Initiation Protocol (SIP)-Specific Event Notification**
- **Status**: RECOMMENDED - Implemented
- **Purpose**: SUBSCRIBE/NOTIFY framework
- **Key Features**:
- Event package framework
- Subscription state management
- Event notification delivery
- Subscription expiration handling
//! ### **RFC 3311 - The Session Initiation Protocol (SIP) UPDATE Method**
- **Status**: RECOMMENDED - Implemented
- **Purpose**: Mid-dialog parameter updates
- **Interop Notes**:
- SOFIA SIP: Full UPDATE support
- PJSIP: Configurable UPDATE support
- Asterisk: Basic UPDATE support
- FreeSWITCH: Full UPDATE support
//! ### **RFC 3326 - The Reason Header Field for SIP**
- **Status**: RECOMMENDED - Implemented
- **Purpose**: Call termination reason indication
- **Key Features**:
- Q.850 cause codes
- SIP response codes
- Protocol-specific reasons
- Multiple reason values
//! ### **RFC 3428 - Session Initiation Protocol (SIP) Extension for Instant Messaging**
- **Status**: OPTIONAL - Implemented
- **Purpose**: SIP MESSAGE method for instant messaging
- **Interop Notes**:
- All major stacks support MESSAGE method
- Content-Type handling varies
//! ### **RFC 3515 - The Session Initiation Protocol (SIP) REFER Method**
- **Status**: RECOMMENDED - Implemented
- **Purpose**: Call transfer and redirection
- **Key Features**:
- Refer-To header
- Referred-By header
- Transfer progress notifications
- Replaces header integration
//! ### **RFC 3581 - An Extension to SIP for Symmetric Response Routing**
- **Status**: RECOMMENDED - Implemented
- **Purpose**: NAT traversal support
- **Key Features**:
- "rport" parameter in Via header
- Symmetric response routing
- NAT detection and handling
//! ### **RFC 3841 - Caller Preferences for SIP**
- **Status**: OPTIONAL - Implemented
- **Purpose**: Feature and capability negotiation
- **Key Features**:
- Accept-Contact header
- Reject-Contact header
- Request-Disposition header
- Feature parameter matching
//! ### **RFC 3891 - The Session Initiation Protocol (SIP) "Replaces" Header**
- **Status**: RECOMMENDED - Implemented
- **Purpose**: Call replacement for transfers
- **Interop Notes**:
- Essential for attended transfers
- All major stacks support Replaces
//! ### **RFC 3903 - Session Initiation Protocol (SIP) Extension for Event State Publication**
- **Status**: OPTIONAL - Implemented
- **Purpose**: PUBLISH method for event state
- **Interop Notes**:
- SOFIA SIP: Full PUBLISH support
- PJSIP: Configurable PUBLISH support
- Asterisk: Limited PUBLISH support
- FreeSWITCH: Full PUBLISH support
//! ### **RFC 4028 - Session Timers in SIP**
- **Status**: RECOMMENDED - Implemented
- **Purpose**: Session refresh and timeout handling
- **Key Features**:
- Session-Expires header
- Min-SE header
- Refresher parameter
- Timer-based session management
- **Interop Notes**:
- Asterisk: Custom session timer implementation
- Others: Standard RFC 4028 implementation
//! ### **RFC 4235 - An INVITE-Initiated Dialog Event Package for SIP**
- **Status**: OPTIONAL - Implemented
- **Purpose**: Dialog state monitoring
- **Key Features**:
- Dialog information XML format
- Dialog state notifications
- Multiple dialog tracking
//! ### **RFC 4916 - Connected Identity in SIP**
- **Status**: RECOMMENDED - Implemented
- **Purpose**: Connected party identification
- **Key Features**:
- P-Asserted-Identity header
- P-Preferred-Identity header
- Remote-Party-ID header (legacy)
- Privacy header integration
//! ### **RFC 6026 - Correct Transaction Handling for 2xx Responses to SIP INVITE**
- **Status**: REQUIRED - Implemented
- **Purpose**: Proper INVITE transaction handling
- **Key Features**:
- Multiple 2xx response handling
- Transaction termination rules
- Forking proxy behavior
//! ### **RFC 6141 - Re-INVITE and Target-Refresh Request Handling in SIP**
- **Status**: RECOMMENDED - Implemented
- **Purpose**: Mid-dialog request handling
- **Key Features**:
- Target-refresh request rules
- Route set updates
- Contact header updates
//! ### **RFC 8224 - Authenticated Identity Management in SIP (STIR)**
- **Status**: RECOMMENDED - Implemented
- **Purpose**: Call authentication framework
- **Key Features**:
- Identity header
- PASSporT token format
- Certificate-based authentication
- Verification procedures
//! ### **RFC 8225 - PASSporT: Personal Assertion Token (SHAKEN)**
- **Status**: RECOMMENDED - Implemented
- **Purpose**: Secure caller ID framework
- **Key Features**:
- JWT-based assertions
- Attestation levels (A, B, C)
- Origination and verification
- Anti-spoofing measures
//! ## SIP Stack Specific Compatibility Requirements
//! ### **SOFIA SIP (Nokia/FreeSWITCH) Compatibility**
```text
User-Agent Detection: "sofia", "FreeSWITCH"
Key Requirements:
- Strict RFC 3261 compliance
- Contact header required in REGISTER
- Proper Route header handling
- IPv4 preference for media
- Session timer support (RFC 4028)
- PRACK support (RFC 3262)
//! Configuration:
[sofia_compatibility]
strict_rfc_compliance = true
require_contact_in_register = true
prefer_ipv4 = true
session_timers = true
prack_support = true
```
//! ### **PJSIP Compatibility**
```text
User-Agent Detection: "PJSUA", "pjsip"
Key Requirements:
- Flexible header parsing
- Compact header support
- Multiple transport support
- UPDATE method support
- Configurable PRACK
//! Configuration:
[pjsip_compatibility]
flexible_parsing = true
compact_headers = true
update_support = true
configurable_prack = true
multiple_transports = true
```
//! ### **Asterisk Compatibility**
```text
User-Agent Detection: "Asterisk"
Key Requirements:
- Custom session timer handling
- Flexible SDP parsing
- Custom authentication quirks
- Limited PRACK support
- X-Asterisk headers support
//! Configuration:
[asterisk_compatibility]
custom_session_timers = true
flexible_sdp = true
custom_auth = true
limited_prack = true
asterisk_headers = true
```
//! ### **FreeSWITCH Compatibility**
```text
User-Agent Detection: "FreeSWITCH", "mod_sofia"
Key Requirements:
- Variable header support
- Advanced routing features
- Full RFC compliance
- Media optimization
- Event framework support
//! Configuration:
[freeswitch_compatibility]
variable_headers = true
advanced_routing = true
full_rfc_compliance = true
media_optimization = true
event_framework = true
```
//! ## Class 4 Switch Specific Requirements
//! ### **Carrier-Grade Features**
- High-volume call processing (10,000+ CPS)
- Sub-millisecond routing decisions
- Advanced billing and rating
- Multiple codec support
- Transcoding capabilities
- Media anchoring/bypass
//! ### **Interconnect Requirements**
- SIP-I (ISUP over SIP) support
- SIP-T (ISUP tunneling) support
- Q.850 cause code mapping
- Billing record generation
- LCR (Least Cost Routing)
- Quality monitoring
//! ### **Regulatory Compliance**
- STIR/SHAKEN implementation
- CALEA compliance preparation
- Emergency services routing
- Number portability support
- Fraud detection and prevention

**Location**: `./src/sip_compliance.rs`
**Lines of Code**: 813

#### Public Functions

- `new`
- `get_rfc`
- `get_all_rfcs`
- `get_rfcs_by_level`
- `check_compliance`
- `generate_interop_config`
- `compliance_percentage`
- `is_fully_compliant`
- `generate_sofia_config`
- `generate_pjsip_config`
- `generate_asterisk_config`
- `generate_freeswitch_config`

#### Public Structs

- `RfcImplementation`
- `SipComplianceChecker`
- `ComplianceReport`
- `InteropConfigGenerator`

#### Dependencies

- serde
- std
- codes
- tracing
- code
- super
- anyhow

### sip_state Module

**Location**: `./src/sip_state.rs`
**Lines of Code**: 616

#### Public Functions

- `new` (async)
- `start` (async)
- `process_message` (async)
- `get_dialog` (async)
- `get_transaction` (async)
- `new` (async)
- `start` (async)

#### Public Structs

- `SipStateManager`
- `SipStateConfig`
- `TransactionTimerManager`

#### Dependencies

- anyhow
- std
- tokio
- sip_parser
- dashmap
- uuid
- rsip
- tracing

### fail2ban Module

# Fail2Ban Integration
//! This module provides fail2ban integration for Redfire Switch to automatically
ban IP addresses that exhibit patterns of authentication failures for SIP and SMS.

**Location**: `./src/fail2ban.rs`
**Lines of Code**: 670

#### Public Functions

- `new` (async)
- `start` (async)
- `record_sip_failure` (async)
- `record_sms_failure` (async)
- `is_banned` (async)
- `get_failure_stats` (async)
- `get_all_failure_stats` (async)
- `manual_ban` (async)
- `manual_unban` (async)
- `clear_failures` (async)
- `parse_ip_addr` (async)

#### Public Structs

- `Fail2BanConfig`
- `SipFailureConfig`
- `SmsFailureConfig`
- `AuthFailure`
- `IpFailureTracker`
- `Fail2BanService`

#### Dependencies

- chrono
- std
- ipnetwork
- tracing
- anyhow
- serde
- tokio
- dashmap

### emergency_routing Module

# Emergency Call Routing
//! This module provides emergency call detection and routing for Class 4 switches.
Emergency calls (911, 112, etc.) must be routed back to the originating DID provider
to ensure proper PSAP (Public Safety Answering Point) connectivity and location services.

**Location**: `./src/emergency_routing.rs`
**Lines of Code**: 461

#### Public Functions

- `new` (async)
- `add_did_mapping` (async)
- `remove_did_mapping` (async)
- `analyze_call` (async)
- `get_statistics` (async)
- `validate_config` (async)
- `is_emergency_number` (async)
- `extract_region_from_number` (async)
- `create_example_provider` (async)

#### Public Structs

- `EmergencyConfig`
- `EmergencyPattern`
- `DidProviderInfo`
- `EmergencyRoutingDecision`
- `EmergencyRoutingService`
- `EmergencyStatistics`

#### Dependencies

- tracing
- regex
- anyhow
- serde
- async
- std
- for
- super

### cnam Module

**Location**: `./src/cnam.rs`
**Lines of Code**: 1093

#### Public Functions

- `new` (async)
- `with_lerg_nanpa` (async)
- `lookup` (async)
- `save_cache` (async)
- `is_country_enabled` (async)
- `get_enabled_countries` (async)
- `test_country_detection` (async)
- `has_lerg_nanpa_integration` (async)
- `get_lerg_nanpa_status` (async)
- `get_stats` (async)
- `get_cache_size` (async)
- `clear_cache` (async)
- `get_active_lookups_count` (async)
- `format_from_header_with_cnam` (async)
- `create_default_cnam_config` (async)
- `create_provider_config` (async)
- `is_valid_for_cnam_lookup` (async)
- `extract_nanpa_number` (async)

#### Public Structs

- `CnamConfig`
- `CnamProviderConfig`
- `CnamAuthConfig`
- `CnamRetryConfig`
- `CnamCacheConfig`
- `CnamResult`
- `CnamStats`
- `CnamService`

#### Dependencies

- lerg_nanpa
- super
- serde
- proper
- anyhow
- tokio
- reqwest
- chrono
- dashmap
- std
- tracing

### enhanced_routing Module

**Location**: `./src/enhanced_routing.rs`
**Lines of Code**: 882

#### Public Functions

- `new` (async)
- `with_lerg_service` (async)
- `add_table` (async)
- `route_call` (async)
- `simple_match` (async)
- `no_match` (async)

#### Public Structs

- `EnhancedRoutingTable`
- `RoutingEntry`
- `DnisCriteria`
- `LrnCriteria`
- `JurisdictionCriteria`
- `RoutingDestination`
- `TimeRestrictions`
- `TimeRange`
- `CapacityLimits`
- `QualityRequirements`
- `DefaultRoutingBehavior`
- `LrnLookupConfig`
- `JurisdictionRoutingConfig`
- `EnhancedRoutingResult`
- `RateCenterInfo`
- `RoutingDecisionDetails`
- `LrnLookupResult`
- `EnhancedRoutingEngine`
- `CriteriaEvaluationResult`

#### Dependencies

- tracing
- std
- dashmap
- termination_routing
- lerg_nanpa
- serde
- anyhow
- default
- tokio
- chrono

### call_simulator Module

**Location**: `./src/call_simulator.rs`
**Lines of Code**: 814

#### Public Functions

- `new` (async)
- `record_call` (async)
- `update_cps` (async)
- `get_summary` (async)
- `new` (async)
- `setup_mock_trunks` (async)
- `simulate_call` (async)
- `run_batch_simulation` (async)
- `export_results_csv` (async)
- `run_load_test` (async)
- `get_mock_trunks` (async)
- `get_active_simulations_count` (async)

#### Public Structs

- `CallSimulationConfig`
- `TestCallPattern`
- `CallSimulationResult`
- `BatchSimulationStats`
- `LoadTestStats`
- `CallSimulator`

#### Dependencies

- tracing
- origination_routing
- default
- tokio
- anyhow
- parking_lot
- termination_routing
- std
- csv
- chrono
- uuid
- serde

### advanced_routing Module

# Phase 3: Advanced Routing & Billing
//! This module implements Phase 3 of the dependency optimization plan:
- LRN/DNIS mixed routing
- Real-time billing engine
- Jurisdiction determination
- Rating engine
- CDR streaming

**Location**: `./src/advanced_routing.rs`
**Lines of Code**: 941

#### Public Functions

- `calculate_cost` (async)
- `new` (async)
- `start` (async)
- `add_billing_rate` (async)
- `get_billing_rate` (async)
- `create_billing_session` (async)
- `on_call_answered` (async)
- `finalize_billing_session` (async)
- `get_billing_session` (async)
- `list_active_sessions` (async)
- `get_customer_balance` (async)
- `add_customer_credit` (async)
- `new` (async)
- `start` (async)
- `route_call_with_billing` (async)
- `on_call_answered` (async)
- `on_call_ended` (async)
- `get_billing_session` (async)

#### Public Structs

- `BillingRate`
- `BillingRoute`
- `BillingSession`
- `AdvancedRoutingRequest`
- `AdvancedRoutingResponse`
- `RealTimeBillingEngine`
- `BillingEngineConfig`
- `AdvancedRoutingBillingEngine`
- `AdvancedEngineConfig`

#### Dependencies

- proper
- enhanced_routing
- the
- serde
- tracing
- dashmap
- lerg_nanpa
- anyhow
- std
- tokio
- chrono

### simulation_cli Module

**Location**: `./src/simulation_cli.rs`
**Lines of Code**: 782

#### Public Functions

- `handle_simulate_command` (async)
- `handle_single_call_simulation` (async)
- `handle_batch_simulation` (async)
- `handle_load_test` (async)
- `handle_interactive_simulation` (async)
- `handle_mock_trunk_setup` (async)
- `handle_jurisdiction_testing` (async)

#### Dependencies

- std
- call_simulator
- anyhow
- origination_routing
- termination_routing
- console
- cli
- dialoguer
- tracing
- indicatif

### mcp_server Module

**Location**: `./src/mcp_server.rs`
**Lines of Code**: 869

#### Public Functions

- `new` (async)
- `handle_request` (async)
- `create_session` (async)
- `remove_session` (async)
- `start_mcp_server` (async)

#### Public Structs

- `McpResponse`
- `McpError`
- `InitializeParams`
- `ClientCapabilities`
- `ClientInfo`
- `CallToolParams`
- `ReadResourceParams`
- `GetPromptParams`
- `Tool`
- `Resource`
- `Prompt`
- `PromptArgument`
- `McpServer`
- `ServerCapabilities`
- `PromptsCapability`
- `ResourcesCapability`
- `ToolsCapability`
- `McpSession`

#### Dependencies

- tokio
- anyhow
- serde_json
- std
- tracing
- to
- serde
- uuid

### ims_core Module

# IMS Core Support
//! This module provides basic IMS (IP Multimedia Subsystem) functionality for VoLTE
support in a Class 4 switch environment. It handles IMS-specific SIP extensions,
3GPP headers, and media negotiation for VoLTE calls.

**Location**: `./src/ims_core.rs`
**Lines of Code**: 764

#### Public Functions

- `new` (async)
- `process_registration` (async)
- `create_session` (async)
- `negotiate_media` (async)
- `update_session_state` (async)
- `terminate_session` (async)
- `process_emergency_call` (async)
- `generate_ims_sdp` (async)
- `get_statistics` (async)
- `is_ims_uri` (async)
- `extract_impu` (async)
- `parse_feature_tags` (async)
- `generate_ims_call_id` (async)

#### Public Structs

- `ImsConfig`
- `ImsSecurityConfig`
- `ImsMediaConfig`
- `ImsSession`
- `ImsMediaInfo`
- `ImsSecurityAssociation`
- `ImsService`
- `ImsRegistration`
- `FeatureTag`
- `ImsStatistics`

#### Dependencies

- super
- uuid
- std
- tracing
- chrono
- anyhow
- serde
- proper

### config Module

**Location**: `./src/config.rs`
**Lines of Code**: 291

#### Public Functions

- `load_from_file`
- `save_to_file`

#### Public Structs

- `TlsConfig`
- `SipEndpoint`
- `MonitoringConfig`
- `SipProfile`
- `Config`
- `OriginationRoutingConfig`

#### Dependencies

- routing
- serde
- std
- codec
- termination_routing
- cnam
- billing
- cdr
- security
- twilio_api
- rcs
- sms
- sipt_sipi
- stir_shaken
- call_control
- rtp_proxy

### routing_engine Module

**Location**: `./src/routing_engine.rs`
**Lines of Code**: 567

#### Public Functions

- `new` (async)
- `add_ingress_trunk` (async)
- `add_did_ownership` (async)
- `route_call` (async)
- `get_trunk_stats` (async)
- `is_did_available` (async)
- `set_did_ownership` (async)
- `remove_did_ownership` (async)
- `get_customer_dids` (async)
- `increment_trunk_calls` (async)
- `decrement_trunk_calls` (async)

#### Public Structs

- `GatewayInfo`
- `IngressTrunkConfig`
- `DidOwnership`
- `CallContext`
- `RoutingEngine`
- `RouteStats`

#### Dependencies

- number_manipulation
- std
- serde
- tracing
- chrono
- super
- anyhow

### enhanced_cli Module

**Location**: `./src/enhanced_cli.rs`
**Lines of Code**: 1253

#### Public Functions

- `new` (async)
- `new` (async)
- `run` (async)

#### Public Structs

- `EnhancedCli`
- `StartArgs`
- `DashboardArgs`
- `SmsCommands`
- `RoutingCommands`
- `SecurityCommands`
- `BillingCommands`
- `MonitorCommands`
- `CodecCommands`
- `ConfigCommands`
- `HelpArgs`
- `SystemStats`
- `DashboardState`
- `EnhancedCliManager`

#### Dependencies

- sms
- security
- indicatif
- tokio
- billing
- codec
- serde
- cdr
- anyhow
- console
- clap
- dialoguer
- ratatui
- crossterm
- tracing
- std

### sms Module

**Location**: `./src/sms.rs`
**Lines of Code**: 1896

#### Public Functions

- `new` (async)
- `set_fail2ban_service` (async)
- `send_sms` (async)
- `send_sms_advanced` (async)
- `receive_sms` (async)
- `get_message_status` (async)
- `get_stats` (async)
- `check_li_required` (async)
- `generate_message_id` (async)
- `validate_phone_number` (async)
- `calculate_sms_cost` (async)
- `sms_to_cdr_fields` (async)
- `encode_message_content` (async)
- `decode_message_content` (async)
- `apply_firewall_rules` (async)
- `check_rate_limits` (async)

#### Public Structs

- `SmsMessage`
- `MultipartInfo`
- `MessageStoreEntry`
- `RateLimitTracker`
- `SmppConfig`
- `SipSmsConfig`
- `SmsRoutingConfig`
- `SmsRoute`
- `RouteSelectionCriteria`
- `RouteStats`
- `RetryProfile`
- `LawfulInterceptConfig`
- `SmsFirewallConfig`
- `ContentFilter`
- `RateLimit`
- `SmsAlarmConfig`
- `AlarmThresholds`
- `TrafficMonitoringConfig`
- `SmsConfig`
- `SmppSession`
- `QueuedMessage`
- `SmsStats`
- `SmsService`
- `RouteSelector`
- `SmsSubmission`
- `LiSmsRecord`

#### Dependencies

- tokio
- chrono
- rand
- parking_lot
- uuid
- dashmap
- tracing
- fail2ban
- std
- anyhow
- serde
- standardized
- super

### rest_api Module

**Location**: `./src/rest_api.rs`
**Lines of Code**: 779

#### Public Functions

- `success` (async)
- `error` (async)
- `new` (async)
- `create_api_router` (async)
- `get_system_stats` (async)
- `list_active_calls` (async)
- `get_call_info` (async)
- `list_dids` (async)
- `create_did` (async)
- `get_did_info` (async)
- `update_did` (async)
- `delete_did` (async)
- `list_customers` (async)
- `get_customer` (async)
- `list_sms_messages` (async)
- `send_sms` (async)
- `get_sms_info` (async)
- `start_api_server` (async)

#### Public Structs

- `ApiResponse`
- `PaginationQuery`
- `CallInfo`
- `TrunkInfo`
- `DidInfo`
- `CustomerInfo`
- `SmsInfo`
- `SystemStats`
- `MemoryUsage`
- `TrunkStats`
- `CreateDidRequest`
- `UpdateDidRequest`
- `SendSmsRequest`
- `AppState`
- `ApiDoc`

#### Dependencies

- std
- tracing
- tokio
- serde
- uuid
- utoipa_swagger_ui
- axum
- anyhow
- tower_http
- utoipa
- tower

### rcs Module

**Location**: `./src/rcs.rs`
**Lines of Code**: 1112

#### Public Functions

- `new` (async)
- `send_rcs_message` (async)
- `check_rcs_capability` (async)
- `get_message_status` (async)
- `get_stats` (async)
- `process_delivery_report` (async)
- `create_text_message` (async)
- `create_quick_reply` (async)
- `create_url_action` (async)
- `create_rich_card` (async)
- `validate_rcs_number` (async)
- `calculate_rcs_cost` (async)

#### Public Structs

- `RcsConfig`
- `RcsRetryConfig`
- `RcsRateLimit`
- `RcsTemplateConfig`
- `RcsMedia`
- `RcsRichCard`
- `RcsMessage`
- `RcsDeliveryReport`
- `RcsError`
- `RcsCapabilityResult`
- `RcsStats`
- `RcsService`

#### Dependencies

- tracing
- uuid
- dashmap
- tokio
- chrono
- serde
- std
- reqwest
- super
- anyhow
- parking_lot
- cdr

### enum_routing Module

# ENUM Routing Module
//! Provides ENUM (E.164 Number Mapping) routing support for TFN (Toll-Free Numbers) 
and DID (Direct Inward Dialing) origination. When an ENUM record exists for a 
number, it takes precedence over standard SIP routes.
//! Features:
- TFN database lookups for toll-free number routing
- DID ownership verification and routing
- ENUM DNS lookups for E.164 numbers
- Route precedence: ENUM > DID database > TFN database > SIP routes
- Caching for performance optimization
- Support for multiple ENUM domains

**Location**: `./src/enum_routing.rs`
**Lines of Code**: 938

#### Public Functions

- `new` (async)
- `set_tfn_database` (async)
- `set_did_database` (async)
- `set_cnam_service` (async)
- `set_telique_client` (async)
- `start` (async)
- `comprehensive_lookup` (async)
- `route_number` (async)
- `get_statistics` (async)
- `new` (async)
- `is_toll_free_number` (async)
- `is_valid_e164` (async)
- `format_number` (async)

#### Public Structs

- `EnumRoutingConfig`
- `TfnDatabaseConfig`
- `DidDatabaseConfig`
- `EnumDnsConfig`
- `EnumCacheConfig`
- `EnumRoute`
- `TfnRecord`
- `DidRecord`
- `EnumRoutingResult`
- `ComprehensiveLookupResult`
- `EnumRoutingService`
- `NaptrRecord`
- `EnumRoutingStats`
- `DefaultEnumDnsResolver`

#### Dependencies

- tokio
- serde
- std
- super
- actual
- tracing
- cnam_service
- proper
- telique_api
- anyhow

### codec Module

**Location**: `./src/codec.rs`
**Lines of Code**: 875

#### Public Functions

- `sample_rate` (async)
- `frame_size` (async)
- `payload_size` (async)
- `payload_type` (async)
- `from_payload_type` (async)
- `encode_ulaw` (async)
- `decode_ulaw` (async)
- `encode_alaw` (async)
- `decode_alaw` (async)
- `ulaw_to_alaw` (async)
- `alaw_to_ulaw` (async)
- `new` (async)
- `decode` (async)
- `encode` (async)
- `new` (async)
- `encode` (async)
- `decode` (async)
- `new` (async)
- `translate_g711_gpu` (async)
- `new` (async)
- `start_session` (async)
- `transcode_frame` (async)
- `end_session` (async)
- `get_session_stats` (async)
- `get_active_sessions` (async)
- `get_statistics` (async)

#### Public Structs

- `CodecConfig`
- `CodecTranslation`
- `AudioFrame`
- `TranscodedFrame`
- `G711Codec`
- `G729Codec`
- `OpusCodec`
- `CudaCodecProcessor`
- `CodecService`
- `TranscodingSession`
- `CodecStatistics`

#### Dependencies

- serde
- anyhow
- std
- dasp
- tracing
- actual
- super
- tokio
- cudarc

### main Module

**Location**: `./src/main.rs`
**Lines of Code**: 460

#### Dependencies

- monitor
- clap
- call_control_cli
- cli
- std
- config
- tokio
- tracing_subscriber
- handlers
- sip_server
- stir_shaken
- anyhow

### rtp_proxy Module

**Location**: `./src/rtp_proxy.rs`
**Lines of Code**: 1226

#### Public Functions

- `unmarshal` (async)
- `new` (async)
- `create_session` (async)
- `get_session` (async)
- `update_endpoint` (async)
- `terminate_session` (async)
- `get_session_stats` (async)
- `get_active_sessions` (async)
- `calculate_bandwidth` (async)
- `should_record_call` (async)
- `generate_sdp` (async)

#### Public Structs

- `RtpPacket`
- `RtpHeader`
- `RtcpReceiverReport`
- `RtcpSenderReport`
- `RtcpReceptionReport`
- `RtcpSenderInfo`
- `RtpProxyConfig`
- `RecordingConfig`
- `SpeechProcessingConfig`
- `RtpSession`
- `TrunkRecordingConfig`
- `RtpEndpoint`
- `RtpStats`
- `RecordingSession`
- `SpeechAnalysisResult`
- `RtpProxyService`

#### Dependencies

- parking_lot
- tracing
- bytes
- uuid
- chrono
- anyhow
- std
- tokio
- super
- serde
- cdr
- hound
- dashmap

### sip_server Module

**Location**: `./src/sip_server.rs`
**Lines of Code**: 532

#### Public Functions

- `new` (async)
- `start` (async)

#### Public Structs

- `SipServer`

#### Dependencies

- tracing
- stir_shaken
- config
- std
- termination_routing
- cdr
- tokio
- anyhow
- origination_routing

### sip_parser Module

**Location**: `./src/sip_parser.rs`
**Lines of Code**: 704

#### Public Functions

- `new`
- `parse_message`
- `create_transaction_id`
- `create_dialog_id`
- `extract_tag`
- `create_response`
- `create_ack_for_2xx`
- `is_retransmission`
- `extract_call_id`
- `extract_from_tag`
- `extract_to_tag`
- `extract_cseq_number`
- `is_provisional_response`
- `is_success_response`
- `is_failure_response`

#### Public Structs

- `SipMessage`
- `SipDialog`
- `SipTransaction`
- `TransactionTimers`
- `SipParser`

#### Dependencies

- std
- serde
- uuid
- super
- tracing
- rsip
- anyhow

### dependency_analysis Module

# Dependency Analysis Report
//! This module provides analysis of library dependencies in the Redfire Switch project
and recommendations for optimization.

**Location**: `./src/dependency_analysis.rs`
**Lines of Code**: 498

#### Public Functions

- `analyze_dependencies`
- `get_implementation_priorities`
- `generate_report`

#### Public Structs

- `DependencyAnalysis`
- `DependencyRecommendation`
- `ImplementationPhase`

#### Dependencies

- serde
- std
- excellent
- cases
- for

### sip_interop Module

# SIP Interoperability Layer
//! Implements RFC requirements and compatibility features for interoperating with:
- SOFIA SIP (Nokia/FreeSWITCH)
- PJSIP (PJSUA2/Asterisk chan_pjsip)
- Asterisk SIP (chan_sip/chan_pjsip)
- FreeSWITCH SIP (mod_sofia)
//! Key RFCs implemented for interoperability:
- RFC 3261: SIP 2.0 Core
- RFC 3262: PRACK (Provisional Response Acknowledgement)
- RFC 3263: SIP DNS Resolution
- RFC 3264: Offer/Answer Model
- RFC 3265: SIP Event Notification
- RFC 3311: SIP UPDATE Method
- RFC 3326: Reason Header
- RFC 3428: SIP MESSAGE Method
- RFC 3515: SIP REFER Method
- RFC 3581: Symmetric RTP
- RFC 3608: Session Initiation Protocol Extension Header Field for Service Route Discovery
- RFC 3841: Caller Preferences
- RFC 3891: Replaces Header
- RFC 3903: SIP PUBLISH Method
- RFC 4028: Session Timers
- RFC 4235: Dialog Event Package
- RFC 4320: SIP Non-INVITE Transaction Timeout
- RFC 4474: Enhancements for Authenticated Identity Management (deprecated by STIR/SHAKEN)
- RFC 4916: Connected Identity in SIP
- RFC 5027: Security Preconditions
- RFC 5373: Requesting Answering Modes for SIP
- RFC 6026: Correct Transaction Handling for 2xx Responses to SIP INVITE Requests
- RFC 6141: Re-INVITE and Target-Refresh Request Handling
- RFC 8224: Authenticated Identity Management (STIR)
- RFC 8225: PASSporT (SHAKEN)

**Location**: `./src/sip_interop.rs`
**Lines of Code**: 1046

#### Public Functions

- `new`
- `detect_stack`
- `apply_outgoing_fixes`
- `apply_outgoing_response_fixes`
- `process_incoming_request`
- `get_supported_methods`
- `get_sdp_preferences`
- `is_extension_supported`
- `generate_supported_header`
- `get_statistics`
- `new`
- `validate_request`
- `validate_response`
- `detect_stack_from_user_agent`
- `is_method_supported`
- `generate_user_agent`

#### Public Structs

- `SipInteropConfig`
- `StackSpecificConfig`
- `SessionTimerConfig`
- `PrackConfig`
- `DialogEventConfig`
- `SipSecurityConfig`
- `SdpPreferences`
- `SipInteropManager`
- `SipInteropStats`
- `RfcComplianceChecker`

#### Dependencies

- serde
- super
- rsip
- IPv4
- tracing
- anyhow
- std

### carrier_integration Module

# Phase 4: Carrier Integration
//! This module implements Phase 4 of the dependency optimization plan:
- SS7 stack for SIP-I interworking
- ISUP message processing
- Advanced codec support
- Network management (SNMP)
- Performance optimization

**Location**: `./src/carrier_integration.rs`
**Lines of Code**: 1127

#### Public Functions

- `new` (async)
- `to_u32` (async)
- `from_string` (async)
- `to_string` (async)
- `new` (async)
- `to_string` (async)
- `new` (async)
- `start` (async)
- `add_link` (async)
- `process_isup_message` (async)
- `generate_iam` (async)
- `send_isup_message` (async)
- `get_circuit_stats` (async)
- `list_active_circuits` (async)
- `get_link_stats` (async)
- `new` (async)
- `start_transcoding_session` (async)
- `process_audio_frame` (async)
- `get_session_stats` (async)
- `list_supported_codecs` (async)
- `get_codec_capability` (async)
- `new` (async)
- `start` (async)
- `update_mib_object` (async)
- `get_mib_object` (async)
- `send_trap` (async)
- `add_trap_destination` (async)
- `new` (async)
- `get_packet` (async)
- `return_packet` (async)
- `set_cpu_affinity` (async)
- `aligned_buffer` (async)

#### Public Structs

- `PointCode`
- `Ss7Link`
- `Ss7LinkStats`
- `CircuitCode`
- `IsupCircuit`
- `CircuitStats`
- `IsupMessage`
- `Ss7Stack`
- `Ss7Config`
- `AdvancedCodecProcessor`
- `CodecCapability`
- `TranscodingSession`
- `SnmpAgent`
- `SnmpConfig`
- `PacketPool`

#### Dependencies

- proper
- dashmap
- tracing
- chrono
- super
- tokio
- libc
- serde
- std
- anyhow

### rtp_monitor Module

**Location**: `./src/rtp_monitor.rs`
**Lines of Code**: 864

#### Public Functions

- `parse` (async)
- `header_length` (async)
- `new` (async)
- `update_with_packet` (async)
- `packet_loss_percentage` (async)
- `jitter_ms` (async)
- `to_quality_metrics` (async)
- `new` (async)
- `start` (async)
- `register_stream` (async)
- `process_rtp_packet` (async)
- `get_stream_stats` (async)
- `get_stream_mos` (async)
- `get_active_streams` (async)
- `unregister_stream` (async)
- `get_alert_receiver` (async)
- `get_quality_report` (async)
- `calculate_codec_overhead` (async)
- `estimate_bandwidth` (async)
- `rtp_timestamp_to_time` (async)
- `detect_voice_activity` (async)

#### Public Structs

- `RtpHeader`
- `RtpStreamStats`
- `RtpMonitorConfig`
- `QualityThresholds`
- `QualityAlert`
- `RtpMonitor`
- `QualityReport`

#### Dependencies

- dashmap
- other
- tracing
- super
- serde
- anyhow
- mos_scoring
- std
- tokio

### twilio_api Module

**Location**: `./src/twilio_api.rs`
**Lines of Code**: 894

#### Public Functions

- `create_twilio_router` (async)
- `start_twilio_api_server` (async)

#### Public Structs

- `TwilioApiConfig`
- `ConversationsConfig`
- `CreateMessageRequest`
- `MessageResponse`
- `Conversation`
- `Participant`
- `MessagingBinding`
- `ConversationMessage`
- `MessageMedia`
- `MessageDelivery`
- `CreateConversationRequest`
- `AddParticipantRequest`
- `SendConversationMessageRequest`
- `TwilioApiState`

#### Dependencies

- chrono
- uuid
- base64
- hmac
- sha2
- tower_http
- axum
- std
- tracing
- sms
- anyhow
- serde

### lerg_nanpa_cli Module

**Location**: `./src/lerg_nanpa_cli.rs`
**Lines of Code**: 533

#### Public Functions

- `handle_lerg_nanpa_command` (async)

#### Dependencies

- cli
- dialoguer
- std
- termination_routing
- anyhow
- console
- lerg_nanpa
- indicatif

### production_hardening Module

# Phase 5: Production Hardening
//! This module implements Phase 5 of the dependency optimization plan:
- High availability and clustering
- Performance monitoring and metrics
- Security hardening
- Load testing framework
- Comprehensive documentation and operational tools

**Location**: `./src/production_hardening.rs`
**Lines of Code**: 1645

#### Public Functions

- `new` (async)
- `check_split_brain` (async)
- `is_split_brain` (async)
- `new` (async)
- `start` (async)
- `trigger_failover` (async)
- `get_cluster_status` (async)
- `new` (async)
- `trigger_alert` (async)
- `clear_alert` (async)
- `list_active_alerts` (async)
- `new` (async)
- `start` (async)
- `record_metric` (async)
- `increment_counter` (async)
- `get_counter` (async)
- `get_recent_metrics` (async)
- `new` (async)
- `is_allowed` (async)
- `new` (async)
- `analyze_request` (async)
- `get_active_threats` (async)
- `new` (async)
- `start` (async)
- `validate_request` (async)
- `get_security_stats` (async)
- `new` (async)
- `add_scenario` (async)
- `run_test` (async)

#### Public Structs

- `ClusterNode`
- `HaManager`
- `HaConfig`
- `HeartbeatMessage`
- `NodeMetrics`
- `SplitBrainDetector`
- `ClusterStatus`
- `PerformanceMonitor`
- `MonitoringConfig`
- `AlertThresholds`
- `MetricValue`
- `PerformanceAlert`
- `AlertManager`
- `AlertConfig`
- `EmailAlertHandler`
- `SlackAlertHandler`
- `SecurityManager`
- `SecurityConfig`
- `SecurityPolicy`
- `SecurityRule`
- `IntrusionDetector`
- `ThreatPattern`
- `ThreatEvent`
- `SecurityRateLimiter`
- `SecurityStats`
- `LoadTester`
- `LoadTestConfig`
- `TestScenario`
- `TestResults`

#### Dependencies

- anyhow
- std
- tokio
- dashmap
- serde
- proper
- tracing
- chrono

### sip_core Module

# Phase 1: Core SIP Stack
//! This module implements Phase 1 of the dependency optimization plan:
- Core SIP Stack with IP-based authentication
- Transaction management
- Dialog management  
- Transport layer (UDP/TCP/TLS)
- Basic authentication with tech prefix support

**Location**: `./src/sip_core.rs`
**Lines of Code**: 633

#### Public Functions

- `new` (async)
- `start` (async)
- `process_request` (async)
- `get_call_context` (async)
- `list_active_calls` (async)
- `send_response` (async)
- `send_request` (async)

#### Public Structs

- `SipCoreConfig`
- `SipCallContext`
- `SipCoreEngine`

#### Dependencies

- sip_state
- sip_authentication
- anyhow
- tracing
- tokio
- serde
- dashmap
- std
- sip_transport
- sip_parser

### stir_shaken Module

**Location**: `./src/stir_shaken.rs`
**Lines of Code**: 1736

#### Public Functions

- `has_regulatory_body` (async)
- `is_stir_shaken_mandated` (async)
- `is_call_authentication_required` (async)
- `get_regulatory_body` (async)
- `get_countries_with_regulatory_bodies` (async)
- `get_stir_shaken_mandated_countries` (async)
- `add_regulatory_body` (async)
- `remove_regulatory_body` (async)
- `update_regulatory_status` (async)
- `new` (async)
- `create_passport` (async)
- `create_identity_header` (async)
- `verify_passport` (async)
- `parse_identity_header` (async)
- `validate_call` (async)
- `is_enabled` (async)
- `get_cert_for_trunk` (async)
- `get_ingress_policy` (async)
- `select_certificate` (async)
- `should_enable_for_call` (async)
- `get_call_regulatory_info` (async)
- `refresh_trust_list` (async)
- `needs_trust_list_refresh` (async)
- `is_certificate_trusted` (async)
- `process_ingress_call` (async)
- `start_trust_list_refresh_task` (async)
- `add_ani_attestation` (async)
- `get_ani_attestation` (async)
- `determine_attestation_level` (async)
- `update_ani_attestation_on_success` (async)
- `get_ani_attestation_stats` (async)
- `get_service_provider_id` (async)
- `generate_call_id` (async)
- `create_call_info` (async)
- `validate_phone_number` (async)
- `extract_phone_number` (async)
- `get_supported_regulatory_bodies` (async)
- `get_stir_shaken_mandated_countries` (async)
- `is_stir_shaken_mandated_for_country` (async)
- `get_regulatory_body_for_country` (async)
- `update_regulatory_registry` (async)
- `add_regulatory_body` (async)
- `update_country_regulatory_status` (async)
- `get_regulatory_statistics` (async)
- `get_certificate_authority_for_country` (async)
- `get_certificate_authorities_for_country` (async)
- `get_trust_list_url_for_call` (async)
- `get_crl_url_for_call` (async)
- `refresh_country_trust_list` (async)
- `add_certificate_authority` (async)
- `get_certificate_authority_by_id` (async)
- `get_active_certificate_authorities` (async)
- `attestation_to_string` (async)
- `string_to_attestation` (async)
- `normalize_phone_number` (async)
- `generate_cert_url` (async)
- `validate_cert_url` (async)

#### Public Structs

- `RegulatoryBody`
- `RegulatoryRegistry`
- `PassportHeader`
- `PassportPayload`
- `DestinationInfo`
- `OriginationInfo`
- `CertificateAuthority`
- `StirShakenCertificate`
- `TrunkStirShakenConfig`
- `StirShakenConfig`
- `CallInfo`
- `TrustListEntry`
- `AniAttestationEntry`
- `CallRegulatoryInfo`
- `RegulatoryStatistics`
- `StirShakenService`

#### Dependencies

- uuid
- when
- super
- tracing
- tokio
- chrono
- cases
- serde
- a
- US
- std
- anyhow
- ANSI
- jsonwebtoken
- for
- Canada
- A

### call_control_cli Module

**Location**: `./src/call_control_cli.rs`
**Lines of Code**: 146

#### Public Functions

- `handle_call_control_command` (async)

#### Dependencies

- chrono
- clap
- anyhow
- config
- call_control
- std

### security Module

**Location**: `./src/security.rs`
**Lines of Code**: 959

#### Public Functions

- `new` (async)
- `is_rate_limited` (async)
- `new` (async)
- `check_sip_request` (async)
- `get_stats` (async)
- `block_ip` (async)
- `unblock_ip` (async)
- `ip_in_cidr` (async)
- `generate_security_report` (async)
- `extract_call_path_from_via` (async)

#### Public Structs

- `SecurityConfig`
- `LoopDetectionConfig`
- `SpamDetectionConfig`
- `RateLimitConfig`
- `IpBlockingConfig`
- `GeoRestrictionConfig`
- `CallPath`
- `IpReputation`
- `RateTracker`
- `SecurityService`
- `SecurityStats`

#### Dependencies

- anyhow
- parking_lot
- regex
- dashmap
- tracing
- super
- chrono
- serde
- std

### media_plane Module

# Phase 2: Media Plane
//! This module implements Phase 2 of the dependency optimization plan:
- RTP proxy/relay
- Basic codec transcoding (G.711)
- DTMF relay
- Video passthrough
- SRTP support

**Location**: `./src/media_plane.rs`
**Lines of Code**: 1090

#### Public Functions

- `default_payload_type` (async)
- `sample_rate` (async)
- `bit_rate` (async)
- `from_char` (async)
- `to_char` (async)
- `parse` (async)
- `serialize` (async)
- `new` (async)
- `is_conversion_supported` (async)
- `transcode_audio` (async)
- `new` (async)
- `start` (async)
- `create_session` (async)
- `start_session` (async)
- `stop_session` (async)
- `create_video_session` (async)
- `get_session_stats` (async)
- `list_sessions` (async)
- `payload_type` (async)
- `new` (async)
- `allocate_port_pair` (async)
- `deallocate_ports` (async)

#### Public Structs

- `DtmfPacket`
- `CodecTranscoder`
- `MediaPlaneSession`
- `MediaEndpoint`
- `SrtpParams`
- `MediaSessionStats`
- `MediaPlaneConfig`
- `MediaPlane`
- `PortAllocator`

#### Dependencies

- std
- tracing
- serde
- dashmap
- rtp_monitor
- video_passthrough
- proper
- anyhow
- tokio

### routing Module

**Location**: `./src/routing.rs`
**Lines of Code**: 629

#### Public Functions

- `new` (async)
- `start` (async)
- `find_routes` (async)
- `add_rule` (async)
- `remove_rule` (async)
- `update_stats` (async)
- `track_call` (async)
- `end_call` (async)
- `get_rules` (async)
- `get_stats` (async)
- `get_active_calls` (async)
- `create_numeric_pattern` (async)
- `business_hours` (async)
- `weekend_hours` (async)
- `generate_route_id` (async)

#### Public Structs

- `RoutePattern`
- `TimeRestriction`
- `RouteDestination`
- `RoutingRule`
- `RoutingRequest`
- `RoutingResponse`
- `SelectedRoute`
- `RoutingConfig`
- `RouteStats`
- `RoutingEngine`
- `ActiveCall`

#### Dependencies

- tracing
- dashmap
- axum
- serde
- regex
- chrono
- super
- anyhow
- std
- tower_http
- parking_lot

### cli Module

**Location**: `./src/cli.rs`
**Lines of Code**: 585

#### Public Structs

- `Cli`

#### Dependencies

- clap

### cdr Module

**Location**: `./src/cdr.rs`
**Lines of Code**: 1681

#### Public Functions

- `new` (async)
- `answer` (async)
- `set_ingress_media` (async)
- `set_egress_media` (async)
- `update_ingress_rtp_stats` (async)
- `update_egress_rtp_stats` (async)
- `update_ingress_quality` (async)
- `update_egress_quality` (async)
- `set_sip_info` (async)
- `set_routing_info` (async)
- `set_fraud_info` (async)
- `mark_setup_start` (async)
- `mark_ringing_start` (async)
- `calculate_answer_timing` (async)
- `track_transfer` (async)
- `track_hold` (async)
- `set_recording_info` (async)
- `set_emergency_info` (async)
- `set_disconnect_info` (async)
- `duration_ms` (async)
- `billable_duration_seconds` (async)
- `to_cdr` (async)
- `new` (async)
- `start_call` (async)
- `answer_call` (async)
- `end_call` (async)
- `active_call_count` (async)
- `get_active_calls` (async)
- `get_stats` (async)
- `create_sms_cdr` (async)
- `create_stir_shaken_cdr` (async)
- `log_stir_shaken_validation` (async)
- `log_stir_shaken_signing` (async)
- `generate_call_id` (async)
- `calculate_cost` (async)
- `disposition_from_sip_code` (async)
- `ms_to_billable_seconds` (async)

#### Public Structs

- `CallDetailRecord`
- `ActiveCallSession`
- `CallTimingInfo`
- `SipSignalingInfo`
- `NetworkRoutingInfo`
- `CallFeaturesInfo`
- `MediaInfo`
- `MediaLegInfo`
- `CdrConfig`
- `CdrService`
- `CdrStats`

#### Dependencies

- table
- database
- chrono
- parking_lot
- serde
- pub
- csv
- a
- at
- tracing
- and
- super
- anyhow
- std
- proper
- clickhouse
- tokio
- client
- let

### monitor Module

**Location**: `./src/monitor.rs`
**Lines of Code**: 274

#### Public Functions

- `ping_endpoint_once` (async)
- `new` (async)
- `start` (async)
- `ping_udp` (async)
- `ping_tcp` (async)
- `get_endpoint_status` (async)
- `get_all_endpoint_status` (async)
- `enable_endpoint` (async)
- `disable_endpoint` (async)

#### Public Structs

- `EndpointHealth`
- `SipMonitor`

#### Dependencies

- std
- tokio
- config
- anyhow
- tracing

### cnam_cli Module

**Location**: `./src/cnam_cli.rs`
**Lines of Code**: 384

#### Public Functions

- `handle_cnam_command` (async)

#### Dependencies

- indicatif
- config
- lerg_nanpa
- these
- console
- cli
- std
- anyhow
- dialoguer
- cnam

### mos_scoring Module

# MOS Scoring Implementation
//! Implements real-time MOS (Mean Opinion Score) calculation for RTP voice streams
based on ITU-T standards including P.862 (PESQ), G.107 (E-Model), and G.113.
//! MOS Scale:
- 5.0: Excellent
- 4.0-4.5: Good  
- 3.0-4.0: Fair
- 2.0-3.0: Poor
- 1.0-2.0: Bad

**Location**: `./src/mos_scoring.rs`
**Lines of Code**: 907

#### Public Functions

- `from_mos` (async)
- `description` (async)
- `is_acceptable` (async)
- `g711` (async)
- `g729` (async)
- `g722` (async)
- `opus` (async)
- `from_name` (async)
- `calculate_packet_loss` (async)
- `update_packet_stats` (async)
- `new` (async)
- `register_stream` (async)
- `update_metrics` (async)
- `get_current_mos` (async)
- `get_mos_history` (async)
- `get_average_mos` (async)
- `unregister_stream` (async)
- `get_global_stats` (async)
- `r_factor_to_mos` (async)
- `mos_to_r_factor` (async)
- `packet_loss_to_mos_impact` (async)
- `jitter_to_mos_impact` (async)
- `get_codec_baseline_mos` (async)
- `calculate_overall_call_quality` (async)

#### Public Structs

- `AudioCodec`
- `RtpQualityMetrics`
- `MosConfig`
- `MosResult`
- `MosFactors`
- `MosCalculator`
- `GlobalMosStats`

#### Dependencies

- dashmap
- tokio
- anyhow
- serde
- let
- tracing
- super
- std

### sip_debug_cli Module

# SIP Debugging CLI
//! Provides real-time SIP message debugging with:
- Color-coded message display
- Filtering by trunk, ANI, DNIS, IP, response codes
- Message flow visualization
- Performance statistics
- Export capabilities

**Location**: `./src/sip_debug_cli.rs`
**Lines of Code**: 823

#### Public Functions

- `new` (async)
- `start` (async)
- `apply_filter` (async)
- `clear_filters` (async)
- `extract_call_info` (async)
- `create_debug_message` (async)

#### Public Structs

- `SipDebugConfig`
- `SipDebugFilter`
- `SipDebugMessage`
- `TrunkInfo`
- `CallInfo`
- `MessageTiming`
- `ProcessingResult`
- `SipDebugCli`
- `DebugStatistics`

#### Dependencies

- regex
- serde
- super
- tracing
- sip_parser
- colored
- tokio
- anyhow
- termion
- std

### cli_utils Module

**Location**: `./src/cli_utils.rs`
**Lines of Code**: 183

#### Public Functions

- `new`
- `success`
- `error`
- `warning`
- `info`
- `header`
- `section`
- `detail`
- `detail_styled`
- `blank_line`
- `create_spinner`
- `create_progress_bar`
- `clear_screen`
- `term`
- `service_status`
- `statistics`

#### Public Structs

- `CliOutput`
- `StatusDisplay`

#### Dependencies

- indicatif
- std
- console
- anyhow

### origination_routing Module

**Location**: `./src/origination_routing.rs`
**Lines of Code**: 1191

#### Public Functions

- `from_number` (async)
- `country_code` (async)
- `is_nanpa` (async)
- `from_number` (async)
- `is_toll_free` (async)
- `as_str` (async)
- `from_str` (async)
- `description` (async)
- `has_surcharge` (async)
- `new` (async)
- `add_did` (async)
- `add_toll_free` (async)
- `add_vendor` (async)
- `route_call` (async)
- `start_call` (async)
- `end_call` (async)
- `get_stats` (async)
- `get_active_calls` (async)
- `generate_vendor_mismatch_report` (async)
- `create_default_ani_ii_surcharges` (async)
- `is_nanpa_number` (async)
- `extract_area_code` (async)
- `create_sample_did` (async)
- `create_sample_toll_free` (async)

#### Public Structs

- `DidSmsConfig`
- `OriginationRoutingService`
- `VerstatMapping`
- `VerstatConfig`
- `DidEntry`
- `TollFreeEntry`
- `VendorCodecConfig`
- `OriginationVendor`
- `VendorContact`
- `VendorBilling`
- `VendorQuality`
- `DidStats`
- `TollFreeStats`
- `OriginationRoutingRequest`
- `OriginationRoutingResponse`
- `OriginationRateInfo`
- `ActiveOriginationCall`
- `OriginationStats`
- `VendorMismatchReport`

#### Dependencies

- dashmap
- anyhow
- std
- tracing
- chrono
- parking_lot
- codec
- super
- serde

### lerg_nanpa Module

**Location**: `./src/lerg_nanpa.rs`
**Lines of Code**: 712

#### Public Functions

- `new` (async)
- `load_lerg_file` (async)
- `download_nanpa_npa_table` (async)
- `determine_jurisdiction` (async)
- `get_company_info` (async)
- `get_rate_center` (async)
- `get_lerg_entry` (async)
- `get_nanpa_entry` (async)
- `get_stats` (async)
- `get_lerg_count` (async)
- `get_nanpa_count` (async)
- `export_lerg_data` (async)
- `is_nanpa_number` (async)
- `extract_npa` (async)
- `create_sample_lerg_entries` (async)

#### Public Structs

- `LergEntry`
- `NanpaNpaEntry`
- `LergNanpaService`
- `LergNanpaStats`

#### Dependencies

- tokio
- std
- csv
- chrono
- dashmap
- serde
- tracing
- super
- reqwest
- termination_routing
- anyhow

### call_control Module

**Location**: `./src/call_control.rs`
**Lines of Code**: 1328

#### Public Functions

- `new` (async)
- `start` (async)
- `stop` (async)
- `should_block_call` (async)
- `check_trunk_limits` (async)
- `start_call` (async)
- `update_call_state` (async)
- `end_call` (async)
- `set_egress_trunk` (async)
- `start_reinvite` (async)
- `complete_reinvite` (async)
- `add_dno_ani_block` (async)
- `add_sti_ocn_block` (async)
- `remove_dno_ani_block` (async)
- `remove_sti_ocn_block` (async)
- `get_statistics` (async)

#### Public Structs

- `DnoAniBlockRow`
- `StiOcnBlockRow`
- `DnoAniBlockConfig`
- `StiOcnBlockConfig`
- `TrunkGroupLimits`
- `CallTimeoutConfig`
- `MinDurationExtendConfig`
- `CallControlConfig`
- `DnoAniBlock`
- `StiOcnBlock`
- `CallState`
- `TrunkCallStats`
- `CallControlService`
- `CallControlStatistics`

#### Dependencies

- std
- chrono
- uuid
- row
- client
- dashmap
- database
- super
- clickhouse
- anyhow
- serde
- tokio
- tracing

### bug_reporter Module

# Bug Reporter Module
//! This module provides functionality to search for existing GitHub issues,
generate system diagnostics, and submit bug reports to the repository.

**Location**: `./src/bug_reporter.rs`
**Lines of Code**: 1174

#### Public Functions

- `new` (async)
- `save_token` (async)
- `test_connection` (async)
- `search_issues` (async)
- `get_issue` (async)
- `get_issue_comments` (async)
- `list_issues` (async)
- `submit_bug_report` (async)
- `find_similar_issues` (async)
- `generate_diagnostics` (async)
- `handle_bug_command` (async)

#### Public Structs

- `GitHubIssue`
- `GitHubLabel`
- `GitHubUser`
- `GitHubComment`
- `SystemDiagnostics`
- `OsInfo`
- `HardwareInfo`
- `DiskUsage`
- `NetworkInfo`
- `NetworkInterface`
- `ListeningPort`
- `ProcessInfo`
- `ConfigSummary`
- `LogSummary`
- `LogFileInfo`
- `LogEntry`
- `MetricsSummary`
- `TestResultsSummary`
- `BugReport`
- `BugReporter`

#### Dependencies

- anyhow
- serde
- std
- chrono
- tokio
- tracing
- cli

### number_manipulation Module

**Location**: `./src/number_manipulation.rs`
**Lines of Code**: 582

#### Public Functions

- `new` (async)
- `add_rule` (async)
- `add_did_range` (async)
- `add_termination_trunk` (async)
- `manipulate_numbers` (async)
- `find_termination_trunk` (async)
- `get_trunk_rules` (async)

#### Public Structs

- `NumberManipulationRule`
- `ManipulationCondition`
- `DidRange`
- `TerminationTrunk`
- `TrunkGateway`
- `NumberManipulationService`

#### Dependencies

- regex
- std
- tracing
- super
- anyhow
- serde

### stir_shaken_fraud Module

# STIR/SHAKEN Fraud Detection
//! This module provides fraud detection for STIR/SHAKEN attestation validation.
It cross-references ANI (Automatic Number Identification) with LERG (Local Exchange 
Routing Guide) OCN (Operating Company Number) data to detect suspicious attestation 
levels that don't match the actual number assignment.

**Location**: `./src/stir_shaken_fraud.rs`
**Lines of Code**: 657

#### Public Functions

- `from_str` (async)
- `to_string` (async)
- `new` (async)
- `set_lerg_data` (async)
- `analyze_call` (async)
- `get_stats` (async)
- `add_fraud_pattern` (async)

#### Public Structs

- `StirShakenFraudConfig`
- `FraudDetectionResult`
- `AniInfo`
- `StirShakenCallInfo`
- `StirShakenFraudDetector`
- `FraudStats`
- `FraudPattern`

#### Dependencies

- tokio
- super
- lerg_nanpa
- serde
- anyhow
- chrono
- tracing
- std

### video_passthrough Module

**Location**: `./src/video_passthrough.rs`
**Lines of Code**: 683

#### Public Functions

- `default_payload_type` (async)
- `mime_type` (async)
- `clock_rate` (async)
- `from_sdp_format` (async)
- `h264` (async)
- `vp8` (async)
- `to_sdp_format` (async)
- `new` (async)
- `configure` (async)
- `process_video_offer` (async)
- `update_video_session` (async)
- `start_video_passthrough` (async)
- `stop_video_session` (async)
- `get_video_stats` (async)
- `list_active_sessions` (async)
- `sdp_contains_video` (async)
- `extract_video_codecs_from_sdp` (async)
- `calculate_video_bandwidth` (async)
- `generate_video_sdp_offer` (async)

#### Public Structs

- `VideoParameters`
- `VideoSession`
- `VideoCallStats`
- `VideoPassthroughManager`

#### Dependencies

- serde
- anyhow
- sequential
- tracing
- std
- super
- it
- proper
- tokio
- dynamic

### telique_api Module

# TeliQue API Integration
//! Provides integration with Teliax TeliQue APIs for telecommunications data lookups:
- CIC (Carrier Identification Code) lookups
- LRN (Location Routing Number) lookups  
- CNAM (Caller Name) lookups
//! Based on the TeliQue API documentation: https://teliax.github.io/DBQ/
//! Features:
- REST API integration with authentication
- Response caching for performance
- Rate limiting compliance
- Bulk lookup support
- Error handling and retry logic

**Location**: `./src/telique_api.rs`
**Lines of Code**: 762

#### Public Functions

- `new` (async)
- `start` (async)
- `lookup_cic` (async)
- `lookup_lrn` (async)
- `lookup_cnam` (async)
- `bulk_lookup` (async)
- `get_bulk_results` (async)
- `get_statistics` (async)
- `is_valid_cic` (async)
- `extract_cic_from_ani` (async)
- `format_lrn_result` (async)
- `is_wireless_number` (async)

#### Public Structs

- `TeliQueConfig`
- `RateLimitConfig`
- `RetryConfig`
- `CicLookupResult`
- `LrnLookupResult`
- `CnamLookupResult`
- `BulkLookupRequest`
- `BulkLookupResponse`
- `TeliQueClient`
- `TeliQueStats`

#### Dependencies

- reqwest
- std
- super
- anyhow
- url
- tracing
- serde
- tokio

### sip_transport Module

**Location**: `./src/sip_transport.rs`
**Lines of Code**: 745

#### Public Functions

- `default_port` (async)
- `is_secure` (async)
- `is_connection_oriented` (async)
- `new` (async)
- `start` (async)
- `get_event_receiver` (async)
- `send_message` (async)
- `get_connections` (async)
- `close_connection` (async)

#### Public Structs

- `TransportMessage`
- `TransportConfig`
- `TlsConfig`
- `ConnectionInfo`
- `SipTransportManager`

#### Dependencies

- anyhow
- tokio_rustls
- rustls
- tracing
- tokio
- serde
- rsip
- std
- existing

### customer_management Module

**Location**: `./src/customer_management.rs`
**Lines of Code**: 472

#### Public Functions

- `new`
- `add_customer`
- `get_customer`
- `update_customer`
- `add_ani_ownership`
- `get_ani_ownership`
- `get_ani_attestation`
- `get_customer_trunks`
- `map_termination_to_origination`
- `list_customers`
- `get_customer_stats`

#### Public Structs

- `Customer`
- `BillingInfo`
- `StirShakenCustomerSettings`
- `CustomerRoutingSettings`
- `AniOwnership`
- `CustomerManagementService`
- `CustomerStats`

#### Dependencies

- anyhow
- std
- stir_shaken
- super
- automatically
- serde
- tracing
- chrono
- origination

### cnam_service Module

# Comprehensive CNAM Service
//! Provides CNAM (Caller Name) lookups using multiple providers:
- TeliQue APIs (Teliax) for CIC, LRN, and CNAM
- Bandwidth.com CNAM per-dip API
- Local CNAM database
- Failover between providers
//! Features:
- Provider prioritization and failover
- Response caching and deduplication
- Rate limiting per provider
- Bulk lookup support
- CURL command examples in config

**Location**: `./src/cnam_service.rs`
**Lines of Code**: 795

#### Public Functions

- `new` (async)
- `start` (async)
- `lookup_cnam` (async)
- `get_statistics` (async)

#### Public Structs

- `CnamServiceConfig`
- `CnamProviderConfig`
- `BandwidthConfig`
- `LocalCnamConfig`
- `CustomProviderConfig`
- `ProviderRateLimit`
- `CnamCacheConfig`
- `CurlExamples`
- `CnamResult`
- `CnamService`
- `CnamServiceStats`

#### Dependencies

- std
- base64
- reqwest
- tokio
- telique_api
- super
- anyhow
- serde
- tracing

### billing Module

**Location**: `./src/billing.rs`
**Lines of Code**: 891

#### Public Functions

- `new` (async)
- `check_call_authorization` (async)
- `generate_payment_required_response` (async)
- `invalidate_account_cache` (async)
- `get_stats` (async)
- `suspend_account` (async)
- `reactivate_account` (async)
- `calculate_estimated_cost` (async)
- `needs_low_balance_warning` (async)
- `generate_billing_report` (async)

#### Public Structs

- `BillingConfig`
- `BillingDatabaseConfig`
- `PaymentRequiredConfig`
- `SuspensionConfig`
- `NotificationConfig`
- `CustomerAccount`
- `BillingProfile`
- `CallAuthRequest`
- `CachedAccountStatus`
- `BillingStats`
- `BillingService`
- `PaymentRequiredResponse`

#### Dependencies

- chrono
- super
- anyhow
- parking_lot
- dashmap
- serde
- tracing
- std

### termination_routing Module

**Location**: `./src/termination_routing.rs`
**Lines of Code**: 1838

#### Public Functions

- `description` (async)
- `requires_special_routing` (async)
- `new` (async)
- `add_call` (async)
- `current_cps` (async)
- `new` (async)
- `add_routing_plan` (async)
- `add_routing_group` (async)
- `add_rate_deck` (async)
- `route_call` (async)
- `start_call` (async)
- `end_call` (async)
- `get_stats` (async)
- `get_active_calls` (async)
- `determine_nanpa_jurisdiction` (async)
- `route_call_with_nanpa_jurisdiction` (async)
- `create_default_routing_plan` (async)
- `create_default_static_table` (async)
- `create_default_rate_deck` (async)
- `create_nanpa_dynamic_routing_config` (async)
- `create_nanpa_dynamic_table` (async)
- `create_lerg_provider_config` (async)
- `create_nanpa_routing_plan` (async)
- `get_area_code_state_mapping` (async)
- `requires_nanpa_jurisdiction_routing` (async)
- `create_jurisdiction_routing_groups` (async)

#### Public Structs

- `TerminationRoutingPlan`
- `StaticRoutingTable`
- `StaticRoutingEntry`
- `NanpaDynamicRoutingConfig`
- `LergProviderConfig`
- `DynamicRoutingTable`
- `LrnProviderConfig`
- `LrnResult`
- `RoutingGroup`
- `TrunkCnamConfig`
- `TrunkCodecConfig`
- `TerminationTrunk`
- `CpsTracker`
- `QosRequirements`
- `RateDeck`
- `RateEntry`
- `TerminationRoutingRequest`
- `TerminationRoutingResponse`
- `RateInfo`
- `TerminationRoutingService`
- `ActiveTerminationCall`
- `TerminationStats`

#### Dependencies

- for
- dashmap
- codec
- a
- parking_lot
- chrono
- anyhow
- tracing
- serde
- std
- super
- pub
- LRN
- origination_routing

