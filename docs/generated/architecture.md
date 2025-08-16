# Redfire Switch Architecture Documentation

## Overview

Redfire Switch is a carrier-grade Class 4 SIP switching platform designed for high-volume telecommunications routing.

## Key Components

### Core SIP Stack
- RFC 3261 compliant SIP parser and state management
- Transaction and dialog management
- Multi-transport support (UDP/TCP/TLS)

### Routing Engine
- ENUM-based routing with TFN/DID support
- LCR (Least Cost Routing)
- Emergency call routing
- STIR/SHAKEN fraud detection

### Authentication & Security
- IP-based authentication
- STIR/SHAKEN implementation
- Fail2ban integration
- TLS/SRTP support

### Media Handling
- RTP proxy and monitoring
- Codec transcoding
- MOS scoring
- Recording capabilities

### Billing & CDR
- Comprehensive call detail records
- ClickHouse integration
- Real-time billing
- Usage analytics

### External Integrations
- TeliQue APIs (CIC, LRN, CNAM)
- Bandwidth.com CNAM
- SMS/SMPP support
- IMS/VoLTE support

## SIP Stack Interoperability

The switch is designed to interoperate with major SIP stacks:

- **SOFIA SIP (FreeSWITCH)**: Full feature compatibility
- **PJSIP**: Flexible header handling
- **Asterisk**: Custom extensions support
- **FreeSWITCH mod_sofia**: Advanced features

## RFC Compliance

The switch implements the following RFCs:

- RFC 3261: SIP 2.0 Core ✅
- RFC 3262: PRACK ✅
- RFC 3263: DNS Resolution ✅
- RFC 4028: Session Timers ✅
- RFC 8224/8225: STIR/SHAKEN ✅
- And many more...

## Call Flows

### Basic SIP Call
Standard SIP call establishment and termination

**Participants**: User Agent A, Redfire Switch, User Agent B

### ENUM-based Call Routing
Call routing using ENUM/TFN/DID lookup with CNAM

**Participants**: Caller, Redfire Switch, ENUM Service, CNAM Service, Destination

### Emergency Call (911)
Emergency call routing back to originating provider

**Participants**: Emergency Caller, Redfire Switch, Emergency Router, PSAP

### STIR/SHAKEN Call Verification
Call authentication using STIR/SHAKEN

**Participants**: Caller, Originating Provider, Redfire Switch, Certificate Authority, Destination

