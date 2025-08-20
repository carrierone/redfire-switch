# Lawful Intercept Technical Setup Guide

## Quick Start Implementation

This guide provides step-by-step technical setup for both ETSI LI and CALEA lawful intercept capabilities.

## Table of Contents

1. [System Prerequisites](#system-prerequisites)
2. [ETSI LI Quick Setup](#etsi-li-quick-setup)
3. [CALEA/J-STD-025 Setup](#caleaj-std-025-setup)
4. [Configuration Examples](#configuration-examples)
5. [API Reference](#api-reference)
6. [Testing and Validation](#testing-and-validation)
7. [Production Deployment](#production-deployment)

## System Prerequisites

### Dependencies

Add to your `Cargo.toml`:

```toml
[dependencies]
redfire-switch = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
anyhow = "1.0"
chrono = { version = "0.4", features = ["serde"] }
serde = { version = "1.0", features = ["derive"] }
tracing = "0.1"
uuid = { version = "1.0", features = ["v4"] }
```

### Environment Setup

```bash
# Set required environment variables
export REDFIRE_LI_CONFIG_PATH="/etc/redfire/li-config.toml"
export REDFIRE_LI_KEYS_PATH="/secure/li-keys/"
export REDFIRE_AUDIT_PATH="/var/log/redfire/audit/"

# Create secure directories
sudo mkdir -p /secure/li-keys/
sudo mkdir -p /var/log/redfire/audit/
sudo chmod 700 /secure/li-keys/
sudo chmod 755 /var/log/redfire/audit/
```

## ETSI LI Quick Setup

### 1. Basic Configuration File

Create `/etc/redfire/etsi-li-config.toml`:

```toml
[etsi_li]
country_code = "US"
network_element_id = "REDFIRE-NE-001"
lawful_intercept_identifier = "LI-SYSTEM-001"
encryption_mandatory = true
audit_logging = true

[delivery.lea_001]
lea_id = "LEA-001"
hi2_endpoint = "https://lea.example.gov/hi2/receive"
hi3_endpoint = "https://lea.example.gov/hi3/receive"
delivery_format = "xml"
encryption_algorithm = "aes256gcm"
authentication_cert = "/secure/certs/lea-001-client.crt"
authentication_key = "/secure/keys/lea-001-client.key"
ca_cert = "/secure/certs/ca.crt"

[delivery.lea_001.retry]
max_retries = 3
retry_interval_seconds = 30
exponential_backoff = true

[encryption]
key_derivation = "pbkdf2"
key_rotation_interval_hours = 24
auto_rotation = true
hsm_enabled = false

[audit]
log_all_access = true
tamper_evident = true
retention_days = 2555  # 7 years
```

### 2. Rust Implementation

```rust
use redfire_switch::{
    etsi_li::{EtsiLiManager, EtsiLiConfig, DeliveryEndpoint, InterceptWarrant},
    compliance_framework::ComplianceFramework,
};
use std::sync::Arc;
use tokio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::init();
    
    // 1. Initialize compliance framework
    let compliance_framework = Arc::new(
        ComplianceFramework::new().await?
    );
    
    // 2. Load ETSI LI configuration
    let etsi_config = load_etsi_config_from_file(
        "/etc/redfire/etsi-li-config.toml"
    ).await?;
    
    // 3. Create ETSI LI manager
    let etsi_li = EtsiLiManager::new(
        etsi_config, 
        compliance_framework.clone()
    ).await?;
    
    // 4. Start LI services
    etsi_li.start().await?;
    
    println!("ETSI LI system started successfully");
    
    // 5. Example: Add intercept warrant
    let warrant = create_example_warrant().await?;
    etsi_li.add_warrant(warrant).await?;
    
    // 6. Keep running
    tokio::signal::ctrl_c().await?;
    println!("Shutting down ETSI LI system");
    
    Ok(())
}

async fn create_example_warrant() -> Result<InterceptWarrant> {
    use redfire_switch::etsi_li::{WarrantType, WarrantStatus};
    use chrono::{Utc, Duration};
    
    Ok(InterceptWarrant {
        warrant_id: "W-2024-001".to_string(),
        lea_id: "LEA-001".to_string(),
        target_identifier: "+15551234567".to_string(),
        warrant_type: WarrantType::ContentAndMetadata,
        start_date: Utc::now(),
        end_date: Utc::now() + Duration::days(30),
        issuing_authority: "Federal District Court".to_string(),
        case_reference: "CASE-2024-CR-001".to_string(),
        status: WarrantStatus::Active,
        encryption_required: true,
    })
}
```

### 3. Integration with SIP Stack

```rust
use redfire_switch::{
    sip_stack::core::{SipCoreEngine, SipCoreConfig},
    calea_sip_bridge::CaleaSipBridge,
};

async fn setup_sip_integration(
    compliance_framework: Arc<ComplianceFramework>
) -> Result<SipCoreEngine> {
    // 1. Create CALEA SIP bridge
    let calea_bridge = Arc::new(
        CaleaSipBridge::new(compliance_framework)
    );
    
    // 2. Configure SIP engine
    let sip_config = SipCoreConfig {
        auth_realm: "carrier.example.com".to_string(),
        enable_authentication: true,
        user_agent: "Redfire-Switch-LI/1.0".to_string(),
        ..Default::default()
    };
    
    // 3. Initialize SIP engine
    let mut sip_engine = SipCoreEngine::new(sip_config).await?;
    
    // 4. Integrate compliance framework
    sip_engine.set_compliance_framework(calea_bridge);
    
    // 5. Start SIP processing
    sip_engine.start().await?;
    
    Ok(sip_engine)
}
```

## CALEA/J-STD-025 Setup

### 1. J-STD-025 Configuration

Create `/etc/redfire/j-std-025-config.toml`:

```toml
[j_std_025]
service_provider_id = "SP-EXAMPLE-001"
network_element_id = "NE-REDFIRE-001"
format_version = "J-STD-025-2007"
lawful_intercept_enabled = true
retention_period_days = 90

[cdr_delivery]
delivery_method = "real_time"
batch_size = 100
delivery_interval_seconds = 60
endpoint = "https://lea.example.gov/cdr/receive"
format = "json"
encryption_enabled = true
compression_enabled = true

[cdr_fields]
include_calling_party = true
include_called_party = true
include_call_duration = true
include_termination_cause = true
include_routing_info = true
include_trunk_info = true
include_codec_info = true
include_qos_metrics = true

[audit]
log_all_cdrs = true
audit_delivery_attempts = true
retention_audit_logs_days = 2555
```

### 2. J-STD-025 Implementation

```rust
use redfire_switch::{
    j_std_025::{JStd025Manager, CdrConfig, CallDetailRecord},
    sipi_b2bua::SipIB2BUA,
};

async fn setup_calea_compliance() -> Result<()> {
    // 1. Load J-STD-025 configuration
    let cdr_config = load_cdr_config_from_file(
        "/etc/redfire/j-std-025-config.toml"
    ).await?;
    
    // 2. Initialize J-STD-025 manager
    let j_std_025 = JStd025Manager::new(cdr_config).await?;
    
    // 3. Start CDR processing
    j_std_025.start().await?;
    
    // 4. Setup B2BUA with compliance
    let compliance_framework = Arc::new(
        ComplianceFramework::new().await?
    );
    
    let b2bua = SipIB2BUA::new(
        "0.0.0.0:5060".parse()?,
        "termination.example.com".to_string(),
        5070,
        sipi_config,
        "TG-001".to_string(),
        compliance_framework,
    ).await?;
    
    // 5. Start B2BUA processing
    tokio::spawn(async move {
        if let Err(e) = b2bua.start().await {
            eprintln!("B2BUA error: {}", e);
        }
    });
    
    println!("CALEA compliance system operational");
    Ok(())
}
```

### 3. CDR Generation Example

```rust
use redfire_switch::j_std_025::{CallDetailRecord, CallEventType};
use chrono::Utc;
use std::collections::HashMap;

async fn generate_sample_cdr() -> CallDetailRecord {
    let mut sip_headers = HashMap::new();
    sip_headers.insert("User-Agent".to_string(), "SIP-Client/1.0".to_string());
    sip_headers.insert("From-URI".to_string(), "sip:+15551234567@carrier.com".to_string());
    sip_headers.insert("To-URI".to_string(), "sip:+15559876543@carrier.com".to_string());
    
    CallDetailRecord {
        // Basic call information
        call_id: "call-12345-abcdef".to_string(),
        calling_number: "+15551234567".to_string(),
        called_number: "+15559876543".to_string(),
        
        // Timestamps
        call_start_time: Utc::now(),
        call_end_time: Some(Utc::now() + chrono::Duration::minutes(5)),
        call_duration: Some(300), // 5 minutes in seconds
        
        // Technical details
        originating_trunk_group: Some("TG-001".to_string()),
        terminating_trunk_group: Some("TG-002".to_string()),
        codec_used: Some("G.711-ULAW".to_string()),
        
        // CALEA specific
        lawful_intercept_indicators: Some(HashMap::from([
            ("warrant_active".to_string(), "true".to_string()),
            ("lea_id".to_string(), "LEA-001".to_string()),
        ])),
        
        // SIP details
        sip_method: Some("INVITE".to_string()),
        sip_response_code: Some(200),
        sip_headers,
        
        // QoS metrics
        rtp_stats: Some(HashMap::from([
            ("packets_sent".to_string(), "15000".to_string()),
            ("packets_lost".to_string(), "12".to_string()),
            ("jitter_ms".to_string(), "2.1".to_string()),
        ])),
        
        // Routing information
        source_ip: Some("192.168.1.100".parse().unwrap()),
        destination_ip: Some("192.168.1.200".parse().unwrap()),
        
        // Administrative
        service_provider_id: "SP-EXAMPLE-001".to_string(),
        network_element_id: "NE-REDFIRE-001".to_string(),
        record_sequence_number: 12345,
        
        // Call outcome
        termination_cause: Some("Normal Clearing".to_string()),
        billing_correlation_id: Some("BILL-2024-001234".to_string()),
    }
}
```

## Configuration Examples

### Complete System Configuration

```rust
// File: src/bin/li_system_complete.rs

use redfire_switch::*;
use std::sync::Arc;
use tokio;
use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    
    // 1. Initialize compliance framework
    let compliance_framework = Arc::new(ComplianceFramework::new().await?);
    
    // 2. Setup ETSI LI (European/International)
    let etsi_li = setup_etsi_li(compliance_framework.clone()).await?;
    
    // 3. Setup J-STD-025 (North American)
    let j_std_025 = setup_j_std_025().await?;
    
    // 4. Setup SIP stack with CALEA bridge
    let sip_engine = setup_sip_stack(compliance_framework.clone()).await?;
    
    // 5. Setup B2BUA with LI integration
    let b2bua = setup_b2bua(compliance_framework.clone()).await?;
    
    // 6. Start all services concurrently
    let (etsi_result, j_std_result, sip_result, b2bua_result) = tokio::join!(
        etsi_li.start(),
        j_std_025.start(), 
        async { sip_engine.start().await },
        async { b2bua.start().await }
    );
    
    // Check all started successfully
    etsi_result?;
    j_std_result?; 
    sip_result?;
    b2bua_result?;
    
    println!("🚨 Complete Lawful Intercept System Operational");
    println!("   • ETSI LI: HI2/HI3 interfaces active");
    println!("   • CALEA: J-STD-025 CDR generation active");
    println!("   • SIP Stack: Compliance monitoring active");
    println!("   • B2BUA: Call processing with LI active");
    
    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    println!("Shutting down LI system...");
    
    Ok(())
}

async fn setup_etsi_li(
    compliance_framework: Arc<ComplianceFramework>
) -> Result<etsi_li::EtsiLiManager> {
    use etsi_li::*;
    
    let config = EtsiLiConfig {
        country_code: "US".to_string(),
        network_element_id: "REDFIRE-NE-001".to_string(),
        lawful_intercept_identifier: "LI-SYS-001".to_string(),
        encryption_mandatory: true,
        delivery_endpoints: vec![
            DeliveryEndpoint {
                lea_id: "LEA-001".to_string(),
                hi2_endpoint: "https://lea.example.gov/hi2".to_string(),
                hi3_endpoint: "https://lea.example.gov/hi3".to_string(),
                encryption_key: load_encryption_key("lea-001")?,
                delivery_format: DeliveryFormat::Xml,
            }
        ],
        audit_logging: true,
    };
    
    let etsi_li = EtsiLiManager::new(config, compliance_framework).await?;
    
    // Add example warrant for testing
    let warrant = InterceptWarrant {
        warrant_id: "W-2024-TEST-001".to_string(),
        lea_id: "LEA-001".to_string(),
        target_identifier: "+15551234567".to_string(),
        warrant_type: WarrantType::ContentAndMetadata,
        start_date: chrono::Utc::now(),
        end_date: chrono::Utc::now() + chrono::Duration::days(30),
        issuing_authority: "Test Court".to_string(),
        case_reference: "TEST-CASE-001".to_string(),
        status: WarrantStatus::Active,
        encryption_required: true,
    };
    
    etsi_li.add_warrant(warrant).await?;
    println!("✅ ETSI LI configured with test warrant");
    
    Ok(etsi_li)
}

async fn setup_j_std_025() -> Result<j_std_025::JStd025Manager> {
    use j_std_025::*;
    
    let config = CdrConfig {
        service_provider_id: "SP-TEST-001".to_string(),
        network_element_id: "NE-REDFIRE-001".to_string(),
        cdr_format_version: "J-STD-025-2007".to_string(),
        lawful_intercept_enabled: true,
        delivery_method: DeliveryMethod::RealTime,
        retention_period_days: 90,
    };
    
    let manager = JStd025Manager::new(config).await?;
    println!("✅ J-STD-025 CDR system configured");
    
    Ok(manager)
}

async fn setup_sip_stack(
    compliance_framework: Arc<ComplianceFramework>
) -> Result<sip_stack::core::SipCoreEngine> {
    use sip_stack::core::*;
    use calea_sip_bridge::CaleaSipBridge;
    
    // Create CALEA bridge
    let calea_bridge = Arc::new(CaleaSipBridge::new(compliance_framework));
    
    // Configure SIP stack
    let config = SipCoreConfig {
        auth_realm: "li-test.example.com".to_string(),
        enable_authentication: true,
        user_agent: "Redfire-Switch-LI/1.0".to_string(),
        ..Default::default()
    };
    
    let mut sip_engine = SipCoreEngine::new(config).await?;
    sip_engine.set_compliance_framework(calea_bridge);
    
    println!("✅ SIP stack configured with CALEA compliance bridge");
    Ok(sip_engine)
}

async fn setup_b2bua(
    compliance_framework: Arc<ComplianceFramework>
) -> Result<sipi_b2bua::SipIB2BUA> {
    use sipi_b2bua::SipIB2BUA;
    use redfire_sip_stack::sipt_sipi::{SipTSipIConfig, SipTSipIService};
    
    let sipi_config = SipTSipIConfig {
        enable_sipt: true,
        enable_sipi: true,
        cic_range_start: 1,
        cic_range_end: 100,
        ..Default::default()
    };
    
    let b2bua = SipIB2BUA::new(
        "0.0.0.0:5060".parse()?,
        "termination.example.com".to_string(),
        5070,
        sipi_config,
        "TG-LI-001".to_string(),
        compliance_framework,
    ).await?;
    
    println!("✅ B2BUA configured with compliance framework integration");
    Ok(b2bua)
}

// Helper function to load encryption keys
fn load_encryption_key(lea_id: &str) -> Result<String> {
    // In production, load from secure key management system
    // For testing, generate a placeholder key
    Ok(format!("AES256-KEY-{}-{}", lea_id, chrono::Utc::now().timestamp()))
}
```

## API Reference

### Warrant Management API

```rust
// Add intercept warrant
etsi_li.add_warrant(warrant).await?;

// List active warrants
let active_warrants = etsi_li.get_active_warrants().await?;

// Update warrant status
etsi_li.update_warrant_status(&warrant_id, WarrantStatus::Suspended).await?;

// Remove expired warrants
etsi_li.cleanup_expired_warrants().await?;

// Validate warrant
let validation = etsi_li.validate_warrant(&warrant).await?;
if !validation.is_valid {
    println!("Validation errors: {:?}", validation.errors);
}
```

### Data Delivery API

```rust
// Test LEA endpoint connectivity
let test_result = etsi_li.test_delivery_endpoint("LEA-001").await?;
println!("Endpoint test: {}", test_result.success);

// Get delivery statistics
let stats = etsi_li.get_delivery_statistics().await?;
println!("Messages delivered: {}", stats.total_delivered);
println!("Failed deliveries: {}", stats.failed_deliveries);

// Manual data delivery
let hi2_data = create_hi2_message(&call_context);
etsi_li.deliver_hi2_data("LEA-001", &hi2_data).await?;
```

### Audit and Monitoring API

```rust
// Get system status
let status = etsi_li.get_system_status().await?;
println!("System operational: {}", status.operational);
println!("Active warrants: {}", status.active_warrants);
println!("Delivery endpoints: {}", status.endpoint_count);

// Export audit logs
let audit_logs = etsi_li.export_audit_logs(
    start_date,
    end_date,
    Some("LEA-001".to_string())
).await?;

// Get compliance report
let report = etsi_li.generate_compliance_report(
    chrono::Utc::now() - chrono::Duration::days(30),
    chrono::Utc::now()
).await?;
```

## Testing and Validation

### Unit Tests

```rust
#[cfg(test)]
mod li_tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_warrant_validation() {
        let compliance_framework = Arc::new(
            ComplianceFramework::new().await.unwrap()
        );
        
        let config = EtsiLiConfig::default();
        let etsi_li = EtsiLiManager::new(config, compliance_framework).await.unwrap();
        
        // Test valid warrant
        let valid_warrant = create_valid_test_warrant();
        let validation = etsi_li.validate_warrant(&valid_warrant).await.unwrap();
        assert!(validation.is_valid);
        
        // Test invalid warrant (expired)
        let expired_warrant = create_expired_test_warrant();
        let validation = etsi_li.validate_warrant(&expired_warrant).await.unwrap();
        assert!(!validation.is_valid);
    }
    
    #[tokio::test]
    async fn test_hi2_message_generation() {
        let call_context = create_test_call_context();
        let hi2_message = generate_hi2_message(&call_context);
        
        assert!(hi2_message.contains("callAttempt"));
        assert!(hi2_message.contains("targetIdentifier"));
        assert!(hi2_message.contains("timestamp"));
    }
    
    #[tokio::test]
    async fn test_cdr_generation() {
        let j_std_025 = JStd025Manager::new(CdrConfig::default()).await.unwrap();
        
        let call_event = create_test_call_event();
        j_std_025.process_call_event(call_event).await.unwrap();
        
        let generated_cdrs = j_std_025.get_generated_cdrs().await.unwrap();
        assert_eq!(generated_cdrs.len(), 1);
        
        let cdr = &generated_cdrs[0];
        assert_eq!(cdr.calling_number, "+15551234567");
        assert_eq!(cdr.called_number, "+15559876543");
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_full_li_integration() {
    // Setup complete LI system
    let compliance_framework = Arc::new(ComplianceFramework::new().await.unwrap());
    let etsi_li = setup_test_etsi_li(compliance_framework.clone()).await.unwrap();
    let j_std_025 = setup_test_j_std_025().await.unwrap();
    
    // Start services
    etsi_li.start().await.unwrap();
    j_std_025.start().await.unwrap();
    
    // Add test warrant
    let warrant = create_test_warrant();
    etsi_li.add_warrant(warrant).await.unwrap();
    
    // Simulate call processing
    let call_event = create_test_call_event();
    compliance_framework.submit_call_event(call_event).await.unwrap();
    
    // Wait for processing
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    
    // Verify HI2 data was generated
    let hi2_data = etsi_li.get_generated_hi2_data().await.unwrap();
    assert!(!hi2_data.is_empty());
    
    // Verify CDR was generated
    let cdrs = j_std_025.get_generated_cdrs().await.unwrap();
    assert!(!cdrs.is_empty());
    
    // Cleanup
    etsi_li.stop().await.unwrap();
    j_std_025.stop().await.unwrap();
}
```

### Load Testing

```rust
#[tokio::test]
#[ignore = "Load test - run manually"]
async fn test_li_performance_under_load() {
    let compliance_framework = Arc::new(ComplianceFramework::new().await.unwrap());
    let etsi_li = setup_test_etsi_li(compliance_framework.clone()).await.unwrap();
    etsi_li.start().await.unwrap();
    
    // Add multiple warrants
    for i in 0..100 {
        let warrant = create_test_warrant_with_id(i);
        etsi_li.add_warrant(warrant).await.unwrap();
    }
    
    // Generate high volume of call events
    let start_time = std::time::Instant::now();
    let mut tasks = Vec::new();
    
    for i in 0..10000 {
        let compliance_framework = compliance_framework.clone();
        let task = tokio::spawn(async move {
            let call_event = create_test_call_event_with_id(i);
            compliance_framework.submit_call_event(call_event).await
        });
        tasks.push(task);
    }
    
    // Wait for all events to be processed
    for task in tasks {
        task.await.unwrap().unwrap();
    }
    
    let duration = start_time.elapsed();
    println!("Processed 10,000 call events in {:?}", duration);
    
    // Verify system is still operational
    let status = etsi_li.get_system_status().await.unwrap();
    assert!(status.operational);
    
    etsi_li.stop().await.unwrap();
}
```

## Production Deployment

### Docker Configuration

```dockerfile
# Dockerfile for LI system
FROM rust:1.75 as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

# Install security updates
RUN apt-get update && apt-get install -y \
    ca-certificates \
    openssl \
    && rm -rf /var/lib/apt/lists/*

# Create secure directories
RUN mkdir -p /secure/li-keys /var/log/redfire/audit /etc/redfire
RUN chmod 700 /secure/li-keys
RUN useradd -r -s /bin/false redfire

COPY --from=builder /app/target/release/redfire-switch /usr/local/bin/
COPY --chown=redfire:redfire configs/ /etc/redfire/

USER redfire
EXPOSE 5060/udp 5060/tcp 8080/tcp

CMD ["redfire-switch", "--config", "/etc/redfire/li-production.toml"]
```

### Kubernetes Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: redfire-li-system
  namespace: telecommunications
spec:
  replicas: 3
  selector:
    matchLabels:
      app: redfire-li
  template:
    metadata:
      labels:
        app: redfire-li
    spec:
      securityContext:
        runAsNonRoot: true
        runAsUser: 65534
        fsGroup: 65534
      containers:
      - name: redfire-li
        image: redfire-switch:li-1.0.0
        ports:
        - containerPort: 5060
          protocol: UDP
        - containerPort: 5060
          protocol: TCP
        - containerPort: 8080
          protocol: TCP
        env:
        - name: RUST_LOG
          value: "info"
        - name: REDFIRE_LI_CONFIG_PATH
          value: "/etc/redfire/li-production.toml"
        volumeMounts:
        - name: li-config
          mountPath: /etc/redfire
          readOnly: true
        - name: li-keys
          mountPath: /secure/li-keys
          readOnly: true
        - name: audit-logs
          mountPath: /var/log/redfire/audit
        resources:
          requests:
            memory: "1Gi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "1"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
      volumes:
      - name: li-config
        configMap:
          name: li-config
      - name: li-keys
        secret:
          secretName: li-keys
          defaultMode: 0400
      - name: audit-logs
        persistentVolumeClaim:
          claimName: audit-logs-pvc

---
apiVersion: v1
kind: Service
metadata:
  name: redfire-li-service
  namespace: telecommunications
spec:
  selector:
    app: redfire-li
  ports:
  - name: sip-udp
    port: 5060
    protocol: UDP
  - name: sip-tcp
    port: 5060
    protocol: TCP
  - name: management
    port: 8080
    protocol: TCP
  type: LoadBalancer
```

### Security Hardening

```bash
#!/bin/bash
# Production security hardening script

# 1. Set up secure file permissions
sudo chown -R redfire:redfire /etc/redfire/
sudo chmod -R 640 /etc/redfire/
sudo chmod 700 /secure/li-keys/
sudo chmod -R 600 /secure/li-keys/*

# 2. Configure audit logging
sudo mkdir -p /var/log/redfire/audit/
sudo chown redfire:redfire /var/log/redfire/audit/
sudo chmod 755 /var/log/redfire/audit/

# 3. Set up log rotation
cat << EOF | sudo tee /etc/logrotate.d/redfire-li
/var/log/redfire/audit/*.log {
    daily
    rotate 2555  # 7 years retention
    compress
    delaycompress
    missingok
    create 644 redfire redfire
    postrotate
        systemctl reload redfire-li
    endscript
}
EOF

# 4. Configure firewall
sudo ufw allow from 192.168.1.0/24 to any port 5060 comment "SIP traffic"
sudo ufw allow from 10.0.0.0/8 to any port 8080 comment "Management API"
sudo ufw deny 5060 comment "Block external SIP"

# 5. Set up systemd service
cat << EOF | sudo tee /etc/systemd/system/redfire-li.service
[Unit]
Description=Redfire Switch Lawful Intercept System
After=network.target
Requires=network.target

[Service]
Type=exec
User=redfire
Group=redfire
ExecStart=/usr/local/bin/redfire-switch --config /etc/redfire/li-production.toml
Restart=always
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=redfire-li

# Security settings
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/redfire
PrivateTmp=true
ProtectKernelTunables=true
ProtectControlGroups=true
RestrictSUIDSGID=true
RestrictRealtime=true
MemoryDenyWriteExecute=true

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable redfire-li
```

### Monitoring and Alerting

```rust
// Health check endpoint implementation
use warp::Filter;
use serde_json::json;

async fn setup_health_monitoring(
    etsi_li: Arc<EtsiLiManager>,
    j_std_025: Arc<JStd025Manager>
) {
    let health = warp::path("health")
        .and(warp::get())
        .and_then({
            let etsi_li = etsi_li.clone();
            let j_std_025 = j_std_025.clone();
            move || {
                let etsi_li = etsi_li.clone();
                let j_std_025 = j_std_025.clone();
                async move {
                    let etsi_status = etsi_li.get_system_status().await;
                    let j_std_status = j_std_025.get_system_status().await;
                    
                    let healthy = etsi_status.map(|s| s.operational).unwrap_or(false) &&
                                 j_std_status.map(|s| s.operational).unwrap_or(false);
                    
                    if healthy {
                        Ok(warp::reply::with_status(
                            warp::reply::json(&json!({
                                "status": "healthy",
                                "timestamp": chrono::Utc::now()
                            })),
                            warp::http::StatusCode::OK
                        ))
                    } else {
                        Ok(warp::reply::with_status(
                            warp::reply::json(&json!({
                                "status": "unhealthy",
                                "timestamp": chrono::Utc::now()
                            })),
                            warp::http::StatusCode::SERVICE_UNAVAILABLE
                        ))
                    }
                }
            }
        });
    
    let routes = health;
    warp::serve(routes)
        .run(([0, 0, 0, 0], 8080))
        .await;
}
```

This completes the comprehensive technical setup guide for Lawful Intercept capabilities. The documentation covers everything from basic configuration to production deployment with security hardening.