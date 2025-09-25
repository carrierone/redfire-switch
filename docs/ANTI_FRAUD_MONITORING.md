# Anti-Fraud Voice Monitoring System

## Overview

The RedFire Switch Anti-Fraud Voice Monitoring System provides ECPA-compliant voice monitoring and analysis capabilities for detecting fraudulent activities through automatic speech recognition (ASR) and banned word detection.

## Features

### Core Capabilities
- **Real-time Fraud Detection**: Configurable percentage-based call sampling
- **ASR Transcription**: Using Vosk speech recognition engine
- **Keyword Analysis**: Banned word detection and risk scoring
- **Two-Tier Storage**: Memory (/dev/shm) for fraud detection, disk for legal authorization
- **ECPA Compliance**: Built-in safeguards and audit logging
- **Admin Interface**: Web-based dashboard for monitoring and review

### Legal Compliance
- **ECPA 18 U.S.C. § 2511(2)(a)(i)**: Provider exception for fraud prevention
- **Data Minimization**: Only store fraud-relevant portions
- **Audit Logging**: Complete access and activity tracking
- **Legal Hold**: Automatic escalation for high-risk calls

## Architecture

### System Components

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   SIP Calls     │───▶│  Media Proxy     │───▶│  Recording      │
│                 │    │  Integration     │    │  Engine         │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                                        │
                                                        ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Admin UI      │◀───│  Anti-Fraud      │◀───│  Storage        │
│                 │    │  Monitoring      │    │  System         │
└─────────────────┘    │  Service         │    └─────────────────┘
                       └──────────────────┘            │
                                │                      ▼
                                ▼               ┌─────────────────┐
                       ┌──────────────────┐    │  Vosk ASR       │
                       │  Database        │    │  Server         │
                       │  Storage         │    └─────────────────┘
                       └──────────────────┘
```

### Storage Architecture

#### Memory Storage (/dev/shm)
- **Purpose**: Real-time fraud detection
- **Legal Basis**: 18 U.S.C. § 2511(2)(a)(i) - Provider exception
- **Retention**: 24 hours maximum
- **Auto-cleanup**: Scheduled cleanup process

#### Disk Storage (/var/lib/redfire/legal-recordings)
- **Purpose**: Legal authorization cases and high-risk calls
- **Legal Basis**: Court order, warrant, or high-confidence fraud indicators (≥8.5 risk score)
- **Retention**: Up to 7 years for legal cases
- **Security**: Encrypted at rest, access logging

## Installation

### Prerequisites
- RedFire Switch base system
- PostgreSQL database
- Vosk speech recognition model
- Sufficient /dev/shm space (10GB recommended)

### 1. Install Dependencies

```bash
# Add required crates to Cargo.toml
vosk = "0.3"
hound = "3.5"
tokio-cron-scheduler = "0.9"
```

### 2. Database Setup

```bash
# Run migration
psql -d redfire -f migrations/add_anti_fraud_monitoring.sql
```

### 3. Install Vosk Server

```bash
# Run the setup script
sudo ./scripts/setup-vosk-server.sh
```

### 4. Configure System

```toml
# Add to config/redfire.toml
[anti_fraud_monitoring]
enabled = true
monitoring_purpose = "fraud_prevention"
legal_basis = "18_USC_2511_2_a_i"
vosk_model_path = "/opt/vosk-model"
memory_storage_path = "/dev/shm/redfire-fraud-detection"
disk_storage_path = "/var/lib/redfire/legal-recordings"
batch_processing_interval_minutes = 2
```

## Configuration

### Trunk Configuration

Enable monitoring for specific ingress trunks:

```sql
-- Enable fraud detection monitoring (5% sampling)
UPDATE ingress_trunks SET
    anti_fraud_monitoring_enabled = true,
    monitoring_sample_percentage = 5.0,
    legal_authorization_reference = NULL,
    ecpa_compliance_enabled = true
WHERE id = 1;

-- Enable legal authorization monitoring (100% sampling)
UPDATE ingress_trunks SET
    anti_fraud_monitoring_enabled = true,
    monitoring_sample_percentage = 100.0,
    legal_authorization_reference = 'COURT_ORDER_2024_001',
    ecpa_compliance_enabled = true
WHERE id = 2;
```

### Banned Words Configuration

```sql
-- Add fraud detection keywords
INSERT INTO banned_words_config (word_pattern, category, risk_weight, description) VALUES
('credit card', 'financial_fraud', 8.0, 'Credit card fraud indicator'),
('social security', 'identity_theft', 9.0, 'SSN harvesting attempt'),
('wire transfer', 'financial_fraud', 7.5, 'Wire transfer scam'),
('you have won', 'robocall', 6.5, 'Prize scam indicator');
```

## Usage

### Starting the Service

```rust
use redfire_switch::services::AntiFraudMonitoringService;

// Initialize service
let config = AntiFraudConfig::default();
let event_bus = Arc::new(EventBus::new());
let database_pool = Arc::new(/* database pool */);

let service = AntiFraudMonitoringService::new(
    config,
    event_bus,
    database_pool,
).await?;

// Start monitoring
service.start().await?;
```

### Call Monitoring

```rust
// Check if call should be monitored
if service.should_monitor_call(trunk_id).await {
    // Determine storage type
    let storage_type = service.determine_storage_type(trunk_id, None).await;

    // Start recording
    let recording_path = service.start_recording(MonitoringRequest {
        call_id: "call_123".to_string(),
        session_id: "session_456".to_string(),
        ingress_trunk_id: trunk_id,
        audio_stream: audio_data,
        codec: "PCMU".to_string(),
        sample_rate: 8000,
        channels: 1,
    }).await?;

    // ... during call ...

    // Stop recording
    service.stop_recording("call_123".to_string()).await?;
}
```

### Admin Interface

Access the admin interface at: `http://your-server/admin/anti-fraud-monitoring`

**Features:**
- Real-time dashboard with statistics
- Call recordings browser with filters
- Transcription viewer with risk analysis
- Alert management and acknowledgment
- System status monitoring
- ECPA compliance indicators

## API Reference

### REST Endpoints

#### Get Statistics
```
GET /api/v1/anti-fraud/statistics?days=7
```

#### List Recordings
```
GET /api/v1/anti-fraud/recordings?page=1&limit=20&storage_type=memory
```

#### View Recording
```
GET /api/v1/anti-fraud/recordings/:id
```

#### Get Transcription
```
GET /api/v1/anti-fraud/transcriptions/:id
```

#### Escalate to Legal Hold
```
POST /api/v1/anti-fraud/recordings/:id/escalate
{
    "reason": "High fraud risk detected",
    "legal_reference": "CASE_2024_001"
}
```

### WebSocket Events

```javascript
const ws = new WebSocket('ws://localhost:8080/ws/anti-fraud');

ws.onmessage = function(event) {
    const data = JSON.parse(event.data);

    switch(data.type) {
        case 'new_alert':
            handleNewAlert(data.alert);
            break;
        case 'recording_completed':
            updateRecordingStatus(data.recording);
            break;
        case 'transcription_completed':
            updateTranscriptionStatus(data.transcription);
            break;
    }
};
```

## Risk Scoring

### Risk Score Calculation

```
Risk Score = Base Score (0.0) +
    (Banned Words Count * Keyword Multiplier * Risk Weight) +
    (Pattern Matches * Pattern Multiplier) +
    (Call Frequency * Frequency Multiplier)
```

### Risk Levels
- **0.0 - 4.9**: Low Risk (memory storage)
- **5.0 - 6.9**: Medium Risk (memory storage, flagged)
- **7.0 - 8.4**: High Risk (alert generated)
- **8.5 - 8.9**: Critical Risk (automatic disk storage)
- **9.0+**: Legal Hold (immediate legal review)

### Automatic Actions
- **Risk ≥ 7.0**: Generate alert
- **Risk ≥ 8.5**: Move to disk storage
- **Risk ≥ 9.0**: Set legal hold, notify compliance team

## Compliance and Legal

### ECPA Compliance Framework

#### Fraud Prevention (18 U.S.C. § 2511(2)(a)(i))
- **Scope**: Real-time keyword detection for fraud indicators
- **Storage**: Memory only (/dev/shm), 24-hour retention
- **Purpose**: Protect provider network and customers from fraud
- **Safeguards**: Data minimization, automatic cleanup

#### Legal Authorization
- **Scope**: Full call recording and transcription
- **Storage**: Encrypted disk storage, extended retention
- **Purpose**: Comply with court orders, warrants, legal process
- **Safeguards**: Legal hold protection, audit logging

### Documentation Requirements

#### Mandatory Records
- Legal authorization documents
- Fraud detection configurations
- Access control logs
- Data retention decisions
- Incident response actions

#### Audit Schedule
- **Daily**: Alert review, storage compliance check
- **Weekly**: Statistical reports, system health
- **Monthly**: Compliance review, security audit
- **Quarterly**: Legal framework review
- **Annually**: Comprehensive compliance audit

## Troubleshooting

### Common Issues

#### Vosk Server Not Responding
```bash
# Check service status
systemctl status vosk-speech-recognition

# View logs
journalctl -u vosk-speech-recognition -f

# Test connection
/opt/vosk-server/venv/bin/python /opt/vosk-server/test_client.py
```

#### Memory Storage Full
```bash
# Check /dev/shm usage
df -h /dev/shm

# Force cleanup
curl -X POST http://localhost:8080/api/v1/anti-fraud/system/cleanup
```

#### Database Connection Issues
```sql
-- Check monitoring tables
SELECT COUNT(*) FROM call_recordings WHERE recorded_at > NOW() - INTERVAL '1 day';
SELECT COUNT(*) FROM call_transcriptions WHERE transcribed_at > NOW() - INTERVAL '1 day';
```

### Performance Tuning

#### Database Optimization
```sql
-- Index maintenance
REINDEX TABLE call_recordings;
REINDEX TABLE call_transcriptions;

-- Statistics update
ANALYZE call_recordings;
ANALYZE call_transcriptions;
```

#### Memory Management
```bash
# Increase /dev/shm size
mount -o remount,size=20G /dev/shm

# Monitor memory usage
watch -n 1 'du -sh /dev/shm/redfire-fraud-detection/*'
```

## Security Considerations

### Access Control
- Role-based access control (RBAC)
- Two-factor authentication required
- Session timeout enforcement
- Activity logging and monitoring

### Data Protection
- Encryption at rest and in transit
- Secure key management
- Regular security audits
- Compliance monitoring

### Network Security
- TLS/SSL for all communications
- Firewall rules for Vosk server
- VPN access for remote administration
- Network segmentation

## Monitoring and Alerting

### System Metrics
- Call monitoring rate
- Storage utilization
- ASR processing time
- Alert generation rate
- System availability

### Alert Thresholds
- High storage usage (>80%)
- ASR processing delays (>5 seconds)
- Failed transcriptions (>5%)
- Compliance violations (any)

### Integration
- SNMP monitoring support
- Syslog integration
- Email/SMS notifications
- Webhook callbacks

## Support and Documentation

### Additional Resources
- [ECPA Compliance Guide](docs/ECPA_COMPLIANCE.md)
- [API Documentation](docs/API.md)
- [Security Best Practices](docs/SECURITY.md)
- [Troubleshooting Guide](docs/TROUBLESHOOTING.md)

### Support Contacts
- **Technical Support**: support@redfire.com
- **Compliance Team**: compliance@redfire.com
- **Security Issues**: security@redfire.com
- **Emergency**: +1-XXX-XXX-XXXX

---

**Document Version**: 1.0
**Last Updated**: November 2024
**Next Review**: February 2025