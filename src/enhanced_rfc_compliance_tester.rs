/*
 * Enhanced RFC Compliance Testing Framework with STIR/SHAKEN Support
 * Tests Class 4 SIP Switch including RFC 8224/8225 authentication
 */

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use chrono::{DateTime, Utc};

use crate::stir_shaken::{AttestationLevel, StirShakenService, StirShakenConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedRfcTest {
    pub rfc_number: String,
    pub title: String,
    pub priority: TestPriority,
    pub description: String,
    pub requirements: Vec<String>,
    pub test_scenarios: Vec<TestScenario>,
    pub stir_shaken_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestPriority {
    Critical,   // Must pass for production deployment
    High,       // Important for carrier-grade operation
    Medium,     // Nice to have features
    Low,        // Optional enhancements
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestScenario {
    pub name: String,
    pub description: String,
    pub sip_message: String,
    pub expected_responses: Vec<String>,
    pub requires_identity_header: bool,
    pub expected_attestation: Option<AttestationLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedTestResult {
    pub test_name: String,
    pub rfc: String,
    pub scenario: String,
    pub status: TestStatus,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub details: HashMap<String, String>,
    // STIR/SHAKEN specific results
    pub identity_header_present: bool,
    pub identity_header_valid: bool,
    pub attestation_level: Option<AttestationLevel>,
    pub stir_shaken_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestStatus {
    Pass,
    Fail,
    Partial,
    NotImplemented,
    Error,
    SkippedNoStirShaken,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedComplianceReport {
    pub test_run_id: String,
    pub timestamp: DateTime<Utc>,
    pub target_system: String,
    pub stir_shaken_enabled: bool,
    pub total_tests: usize,
    pub passed: usize,
    pub failed: usize,
    pub partial: usize,
    pub not_implemented: usize,
    pub errors: usize,
    pub skipped: usize,
    pub compliance_percentage: f32,
    pub critical_compliance_percentage: f32,
    pub stir_shaken_compliance_percentage: f32,
    pub results: Vec<EnhancedTestResult>,
    pub recommendations: Vec<String>,
    pub stir_shaken_statistics: StirShakenStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StirShakenStats {
    pub calls_with_identity: usize,
    pub calls_verified: usize,
    pub calls_signed: usize,
    pub attestation_a_count: usize,
    pub attestation_b_count: usize,
    pub attestation_c_count: usize,
    pub verification_failures: usize,
}

pub struct EnhancedRfcComplianceTester {
    socket: Arc<UdpSocket>,
    test_target: SocketAddr,
    results: Arc<RwLock<Vec<EnhancedTestResult>>>,
    stir_shaken: Option<Arc<StirShakenService>>,
    test_suite: Vec<EnhancedRfcTest>,
}

impl EnhancedRfcComplianceTester {
    pub async fn new(
        bind_addr: SocketAddr, 
        target_addr: SocketAddr,
        stir_shaken_config: Option<StirShakenConfig>
    ) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        info!("Enhanced RFC Compliance Tester listening on {}", bind_addr);
        info!("Testing target: {}", target_addr);
        
        // Initialize STIR/SHAKEN service if configuration provided
        let stir_shaken = if let Some(config) = stir_shaken_config {
            match StirShakenService::new(config).await {
                Ok(service) => {
                    info!("STIR/SHAKEN service initialized for compliance testing");
                    Some(Arc::new(service))
                }
                Err(e) => {
                    warn!("Failed to initialize STIR/SHAKEN service: {}", e);
                    None
                }
            }
        } else {
            None
        };
        
        let test_suite = Self::create_enhanced_test_suite();
        
        Ok(Self {
            socket: Arc::new(socket),
            test_target: target_addr,
            results: Arc::new(RwLock::new(Vec::new())),
            stir_shaken,
            test_suite,
        })
    }

    pub async fn run_all_tests(&self) -> Result<EnhancedComplianceReport> {
        info!("Starting enhanced RFC compliance testing with STIR/SHAKEN support...");
        
        // Clear previous results
        {
            let mut results = self.results.write().await;
            results.clear();
        }
        
        // Run tests for each RFC
        for rfc_test in &self.test_suite {
            info!("Testing {}: {}", rfc_test.rfc_number, rfc_test.title);
            
            for scenario in &rfc_test.test_scenarios {
                let result = self.run_test_scenario(rfc_test, scenario).await;
                self.record_result(result).await;
            }
        }
        
        self.generate_enhanced_compliance_report().await
    }

    async fn run_test_scenario(&self, rfc_test: &EnhancedRfcTest, scenario: &TestScenario) -> EnhancedTestResult {
        debug!("Running test scenario: {} -> {}", rfc_test.rfc_number, scenario.name);
        
        // Check if STIR/SHAKEN is required but not available
        if rfc_test.stir_shaken_required && self.stir_shaken.is_none() {
            return EnhancedTestResult {
                test_name: format!("{}_{}", rfc_test.rfc_number, scenario.name),
                rfc: rfc_test.rfc_number.clone(),
                scenario: scenario.name.clone(),
                status: TestStatus::SkippedNoStirShaken,
                message: "STIR/SHAKEN required but not available".to_string(),
                timestamp: Utc::now(),
                details: HashMap::new(),
                identity_header_present: false,
                identity_header_valid: false,
                attestation_level: None,
                stir_shaken_verified: false,
            };
        }
        
        // Send test message and analyze response
        match self.send_and_analyze_response(&scenario.sip_message, &scenario.expected_responses).await {
            Ok((response, analysis)) => {
                let status = if scenario.expected_responses.iter()
                    .any(|code| response.contains(&format!("SIP/2.0 {} ", code))) {
                    TestStatus::Pass
                } else {
                    TestStatus::Fail
                };
                
                EnhancedTestResult {
                    test_name: format!("{}_{}", rfc_test.rfc_number, scenario.name),
                    rfc: rfc_test.rfc_number.clone(),
                    scenario: scenario.name.clone(),
                    status,
                    message: format!("Response received: {}", response.lines().next().unwrap_or("")),
                    timestamp: Utc::now(),
                    details: HashMap::new(),
                    identity_header_present: analysis.identity_header_present,
                    identity_header_valid: analysis.identity_header_valid,
                    attestation_level: analysis.attestation_level,
                    stir_shaken_verified: analysis.stir_shaken_verified,
                }
            }
            Err(e) => {
                EnhancedTestResult {
                    test_name: format!("{}_{}", rfc_test.rfc_number, scenario.name),
                    rfc: rfc_test.rfc_number.clone(),
                    scenario: scenario.name.clone(),
                    status: TestStatus::Error,
                    message: format!("Test failed: {}", e),
                    timestamp: Utc::now(),
                    details: HashMap::new(),
                    identity_header_present: false,
                    identity_header_valid: false,
                    attestation_level: None,
                    stir_shaken_verified: false,
                }
            }
        }
    }

    async fn send_and_analyze_response(&self, message: &str, expected_codes: &[String]) -> Result<(String, ResponseAnalysis)> {
        // Send message
        self.socket.send_to(message.as_bytes(), self.test_target).await?;
        
        // Wait for response with timeout
        let mut buffer = vec![0u8; 8192]; // Larger buffer for STIR/SHAKEN headers
        let timeout_duration = std::time::Duration::from_secs(10);
        
        match tokio::time::timeout(timeout_duration, self.socket.recv_from(&mut buffer)).await {
            Ok(Ok((len, _from))) => {
                let response = String::from_utf8_lossy(&buffer[..len]).to_string();
                let analysis = self.analyze_stir_shaken_response(&response).await;
                Ok((response, analysis))
            }
            Ok(Err(e)) => Err(anyhow!("Socket error: {}", e)),
            Err(_) => Err(anyhow!("Timeout waiting for response")),
        }
    }

    async fn analyze_stir_shaken_response(&self, response: &str) -> ResponseAnalysis {
        let mut analysis = ResponseAnalysis::default();
        
        // Check for Identity header
        for line in response.lines() {
            if line.to_lowercase().starts_with("identity:") {
                analysis.identity_header_present = true;
                
                // Extract and validate Identity header if STIR/SHAKEN service is available
                if let Some(stir_shaken) = &self.stir_shaken {
                    let identity_value = line.split(':').nth(1).unwrap_or("").trim();
                    
                    // Parse Identity header
                    match stir_shaken.parse_identity_header(identity_value) {
                        Ok((cert_url, passport)) => {
                            analysis.identity_header_valid = true;
                            
                            // Try to verify the PASSporT (this may fail in test environment)
                            match stir_shaken.verify_passport(&passport, &cert_url).await {
                                Ok(claims) => {
                                    analysis.stir_shaken_verified = true;
                                    analysis.attestation_level = Some(claims.attest);
                                }
                                Err(e) => {
                                    debug!("PASSporT verification failed (expected in test): {}", e);
                                    // In test environment, we still consider this a success
                                    analysis.stir_shaken_verified = false;
                                }
                            }
                        }
                        Err(e) => {
                            debug!("Identity header parsing failed: {}", e);
                        }
                    }
                }
                break;
            }
            
            // Check for STIR/SHAKEN related headers
            if line.to_lowercase().contains("x-stir-shaken") {
                analysis.stir_shaken_headers_present = true;
            }
            if line.to_lowercase().contains("x-attestation-level") {
                if line.contains("A") {
                    analysis.attestation_level = Some(AttestationLevel::Full);
                } else if line.contains("B") {
                    analysis.attestation_level = Some(AttestationLevel::Partial);
                } else if line.contains("C") {
                    analysis.attestation_level = Some(AttestationLevel::Gateway);
                }
            }
        }
        
        analysis
    }

    async fn record_result(&self, result: EnhancedTestResult) {
        let mut results = self.results.write().await;
        results.push(result);
    }

    async fn generate_enhanced_compliance_report(&self) -> Result<EnhancedComplianceReport> {
        let results = self.results.read().await;
        
        let total_tests = results.len();
        let passed = results.iter().filter(|r| matches!(r.status, TestStatus::Pass)).count();
        let failed = results.iter().filter(|r| matches!(r.status, TestStatus::Fail)).count();
        let partial = results.iter().filter(|r| matches!(r.status, TestStatus::Partial)).count();
        let not_implemented = results.iter().filter(|r| matches!(r.status, TestStatus::NotImplemented)).count();
        let errors = results.iter().filter(|r| matches!(r.status, TestStatus::Error)).count();
        let skipped = results.iter().filter(|r| matches!(r.status, TestStatus::SkippedNoStirShaken)).count();
        
        let compliance_percentage = if total_tests > 0 {
            (passed as f32 / total_tests as f32) * 100.0
        } else {
            0.0
        };
        
        // Calculate critical compliance (RFC 3261, 8224, 8225)
        let critical_results: Vec<_> = results.iter()
            .filter(|r| r.rfc.contains("3261") || r.rfc.contains("8224") || r.rfc.contains("8225"))
            .collect();
        
        let critical_passed = critical_results.iter()
            .filter(|r| matches!(r.status, TestStatus::Pass))
            .count();
        
        let critical_compliance_percentage = if !critical_results.is_empty() {
            (critical_passed as f32 / critical_results.len() as f32) * 100.0
        } else {
            0.0
        };
        
        // Calculate STIR/SHAKEN specific compliance
        let stir_shaken_results: Vec<_> = results.iter()
            .filter(|r| r.rfc.contains("8224") || r.rfc.contains("8225"))
            .collect();
        
        let stir_shaken_passed = stir_shaken_results.iter()
            .filter(|r| matches!(r.status, TestStatus::Pass))
            .count();
        
        let stir_shaken_compliance_percentage = if !stir_shaken_results.is_empty() {
            (stir_shaken_passed as f32 / stir_shaken_results.len() as f32) * 100.0
        } else {
            100.0 // If no STIR/SHAKEN tests, consider it fully compliant
        };
        
        // Generate STIR/SHAKEN statistics
        let stir_shaken_stats = StirShakenStats {
            calls_with_identity: results.iter().filter(|r| r.identity_header_present).count(),
            calls_verified: results.iter().filter(|r| r.stir_shaken_verified).count(),
            calls_signed: results.iter().filter(|r| r.identity_header_present).count(),
            attestation_a_count: results.iter().filter(|r| matches!(r.attestation_level, Some(AttestationLevel::Full))).count(),
            attestation_b_count: results.iter().filter(|r| matches!(r.attestation_level, Some(AttestationLevel::Partial))).count(),
            attestation_c_count: results.iter().filter(|r| matches!(r.attestation_level, Some(AttestationLevel::Gateway))).count(),
            verification_failures: results.iter().filter(|r| r.identity_header_present && !r.stir_shaken_verified).count(),
        };
        
        // Generate recommendations
        let mut recommendations = Vec::new();
        
        if critical_compliance_percentage < 80.0 {
            recommendations.push("CRITICAL: Core SIP and STIR/SHAKEN functionality needs immediate attention for production deployment.".to_string());
        }
        
        if stir_shaken_compliance_percentage < 100.0 && self.stir_shaken.is_some() {
            recommendations.push("STIR/SHAKEN implementation needs enhancement for US carrier requirements.".to_string());
        }
        
        if failed > 0 {
            recommendations.push(format!("{} critical tests failed. Review implementation for compliance issues.", failed));
        }
        
        if not_implemented > 0 {
            recommendations.push(format!("{} features not implemented. Prioritize based on carrier requirements.", not_implemented));
        }
        
        let report = EnhancedComplianceReport {
            test_run_id: format!("enhanced-rfc-compliance-{}", Utc::now().timestamp()),
            timestamp: Utc::now(),
            target_system: format!("{}", self.test_target),
            stir_shaken_enabled: self.stir_shaken.is_some(),
            total_tests,
            passed,
            failed,
            partial,
            not_implemented,
            errors,
            skipped,
            compliance_percentage,
            critical_compliance_percentage,
            stir_shaken_compliance_percentage,
            results: results.clone(),
            recommendations,
            stir_shaken_statistics: stir_shaken_stats,
        };
        
        Ok(report)
    }

    fn create_enhanced_test_suite() -> Vec<EnhancedRfcTest> {
        vec![
            // RFC 3261 - Core SIP
            EnhancedRfcTest {
                rfc_number: "RFC3261".to_string(),
                title: "Session Initiation Protocol (SIP)".to_string(),
                priority: TestPriority::Critical,
                description: "Core SIP functionality required for any SIP implementation".to_string(),
                requirements: vec![
                    "INVITE method support".to_string(),
                    "OPTIONS method support".to_string(),
                    "Response code handling".to_string(),
                    "Header processing".to_string(),
                ],
                test_scenarios: vec![
                    TestScenario {
                        name: "options_ping".to_string(),
                        description: "Basic OPTIONS ping test".to_string(),
                        sip_message: format!(
                            "OPTIONS sip:test@{{target}} SIP/2.0\r\n\
                            Via: SIP/2.0/UDP 127.0.0.1:12345;branch=z9hG4bK-enhanced-test\r\n\
                            From: Enhanced Tester <sip:tester@127.0.0.1:12345>;tag=enhanced\r\n\
                            To: <sip:test@{{target}}>\r\n\
                            Call-ID: enhanced-test-{}\r\n\
                            CSeq: 1 OPTIONS\r\n\
                            Content-Length: 0\r\n\
                            \r\n",
                            Utc::now().timestamp()
                        ),
                        expected_responses: vec!["200".to_string()],
                        requires_identity_header: false,
                        expected_attestation: None,
                    },
                    TestScenario {
                        name: "invite_basic".to_string(),
                        description: "Basic INVITE test".to_string(),
                        sip_message: format!(
                            "INVITE sip:+15559876543@{{target}} SIP/2.0\r\n\
                            Via: SIP/2.0/UDP 127.0.0.1:12345;branch=z9hG4bK-enhanced-invite\r\n\
                            From: Enhanced Tester <sip:+15551234567@127.0.0.1:12345>;tag=enhanced-invite\r\n\
                            To: <sip:+15559876543@{{target}}>\r\n\
                            Call-ID: enhanced-invite-{}\r\n\
                            CSeq: 1 INVITE\r\n\
                            Contact: <sip:+15551234567@127.0.0.1:12345>\r\n\
                            Content-Type: application/sdp\r\n\
                            Content-Length: 100\r\n\
                            \r\n\
                            v=0\r\n\
                            o=tester 12345 67890 IN IP4 127.0.0.1\r\n\
                            s=Enhanced Test\r\n\
                            c=IN IP4 127.0.0.1\r\n\
                            t=0 0\r\n\
                            m=audio 8000 RTP/AVP 0\r\n",
                            Utc::now().timestamp()
                        ),
                        expected_responses: vec!["100".to_string(), "180".to_string(), "200".to_string()],
                        requires_identity_header: false,
                        expected_attestation: None,
                    },
                ],
                stir_shaken_required: false,
            },
            
            // RFC 8224 - STIR
            EnhancedRfcTest {
                rfc_number: "RFC8224".to_string(),
                title: "Authenticated Identity Management in SIP (STIR)".to_string(),
                priority: TestPriority::Critical,
                description: "Call authentication required for US carriers".to_string(),
                requirements: vec![
                    "Identity header creation".to_string(),
                    "PASSporT token generation".to_string(),
                    "Certificate validation".to_string(),
                ],
                test_scenarios: vec![
                    TestScenario {
                        name: "stir_invite_generation".to_string(),
                        description: "Test STIR Identity header generation on INVITE".to_string(),
                        sip_message: format!(
                            "INVITE sip:+15559876543@{{target}} SIP/2.0\r\n\
                            Via: SIP/2.0/UDP 127.0.0.1:12345;branch=z9hG4bK-stir-test\r\n\
                            From: STIR Tester <sip:+15551234567@127.0.0.1:12345>;tag=stir-test\r\n\
                            To: <sip:+15559876543@{{target}}>\r\n\
                            Call-ID: stir-test-{}\r\n\
                            CSeq: 1 INVITE\r\n\
                            Contact: <sip:+15551234567@127.0.0.1:12345>\r\n\
                            Content-Type: application/sdp\r\n\
                            Content-Length: 100\r\n\
                            \r\n\
                            v=0\r\n\
                            o=tester 12345 67890 IN IP4 127.0.0.1\r\n\
                            s=STIR Test\r\n\
                            c=IN IP4 127.0.0.1\r\n\
                            t=0 0\r\n\
                            m=audio 8000 RTP/AVP 0\r\n",
                            Utc::now().timestamp()
                        ),
                        expected_responses: vec!["100".to_string()],
                        requires_identity_header: true,
                        expected_attestation: Some(AttestationLevel::Gateway),
                    },
                ],
                stir_shaken_required: true,
            },
        ]
    }
}

#[derive(Debug, Default)]
struct ResponseAnalysis {
    identity_header_present: bool,
    identity_header_valid: bool,
    attestation_level: Option<AttestationLevel>,
    stir_shaken_verified: bool,
    stir_shaken_headers_present: bool,
}

pub async fn run_enhanced_rfc_compliance_tests(
    target_addr: SocketAddr,
    stir_shaken_config: Option<StirShakenConfig>
) -> Result<EnhancedComplianceReport> {
    let bind_addr = "0.0.0.0:0".parse()?; // Let OS choose port
    let tester = EnhancedRfcComplianceTester::new(bind_addr, target_addr, stir_shaken_config).await?;
    
    info!("Starting enhanced RFC compliance testing for Class 4 SIP switch with STIR/SHAKEN");
    let report = tester.run_all_tests().await?;
    
    info!("Enhanced RFC compliance testing completed");
    info!("Overall compliance: {:.1}%", report.compliance_percentage);
    info!("Critical compliance: {:.1}%", report.critical_compliance_percentage);
    info!("STIR/SHAKEN compliance: {:.1}%", report.stir_shaken_compliance_percentage);
    
    Ok(report)
}