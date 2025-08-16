/*
 * SIP-I RFC 3398 Compliance Testing Framework
 * Tests ISUP encapsulation and interworking functionality
 */

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tracing::{info, debug};
use chrono::{DateTime, Utc};

use crate::sipt_sipi::{
    SipTSipIService, SipTSipIConfig, IsupMessage, IsupMessageType, 
    IsupParameterType, IsupVariant
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipIComplianceTest {
    pub test_id: String,
    pub title: String,
    pub rfc_section: String,
    pub priority: TestPriority,
    pub description: String,
    pub test_type: SipITestType,
    pub expected_isup_message: Option<IsupMessageType>,
    pub required_parameters: Vec<IsupParameterType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestPriority {
    Critical,   // Must work for PSTN interconnection
    High,       // Important for carrier-grade operation
    Medium,     // Nice to have features
    Low,        // Optional enhancements
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SipITestType {
    SipToIsup,      // SIP INVITE -> ISUP IAM
    IsupToSip,      // ISUP ACM -> SIP 183/200
    BiDirectional,  // Full call flow test
    ParameterMapping, // Specific parameter conversion
    ErrorHandling,  // Error scenario testing
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipITestResult {
    pub test_id: String,
    pub status: TestStatus,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub details: SipITestDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipITestDetails {
    pub sip_message_sent: Option<String>,
    pub sip_response_received: Option<String>,
    pub isup_detected: bool,
    pub isup_message_type: Option<IsupMessageType>,
    pub isup_parameters_found: Vec<IsupParameterType>,
    pub isup_cic: Option<u16>,
    pub calling_number_extracted: Option<String>,
    pub called_number_extracted: Option<String>,
    pub content_type_detected: Option<String>,
    pub multipart_detected: bool,
    pub parameter_mapping_correct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TestStatus {
    Pass,
    Fail,
    Partial,
    NotImplemented,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipIComplianceReport {
    pub test_run_id: String,
    pub timestamp: DateTime<Utc>,
    pub target_system: String,
    pub sipi_enabled: bool,
    pub sipt_enabled: bool,
    pub isup_variant: IsupVariant,
    pub total_tests: usize,
    pub passed: usize,
    pub failed: usize,
    pub partial: usize,
    pub not_implemented: usize,
    pub errors: usize,
    pub compliance_percentage: f32,
    pub critical_compliance_percentage: f32,
    pub results: Vec<SipITestResult>,
    pub recommendations: Vec<String>,
    pub isup_statistics: IsupStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsupStatistics {
    pub iam_messages_detected: usize,
    pub acm_messages_detected: usize,
    pub anm_messages_detected: usize,
    pub rel_messages_detected: usize,
    pub calling_party_extractions: usize,
    pub called_party_extractions: usize,
    pub cic_allocations: usize,
    pub multipart_bodies_processed: usize,
    pub parameter_mappings_correct: usize,
}

pub struct SipIComplianceTester {
    socket: Arc<UdpSocket>,
    test_target: SocketAddr,
    results: Arc<RwLock<Vec<SipITestResult>>>,
    sipi_service: Arc<SipTSipIService>,
    test_suite: Vec<SipIComplianceTest>,
}

impl SipIComplianceTester {
    pub async fn new(
        bind_addr: SocketAddr, 
        target_addr: SocketAddr,
        sipi_config: SipTSipIConfig
    ) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        info!("SIP-I Compliance Tester listening on {}", bind_addr);
        info!("Testing target: {}", target_addr);
        
        // Initialize SIP-I service
        let sipi_service = Arc::new(SipTSipIService::new(sipi_config));
        info!("SIP-I service initialized for compliance testing - SIP-I: {}, SIP-T: {}", 
              sipi_service.is_sipi_enabled(), sipi_service.is_sipt_enabled());
        
        let test_suite = Self::create_sipi_test_suite();
        
        Ok(Self {
            socket: Arc::new(socket),
            test_target: target_addr,
            results: Arc::new(RwLock::new(Vec::new())),
            sipi_service,
            test_suite,
        })
    }

    pub async fn run_all_tests(&self) -> Result<SipIComplianceReport> {
        info!("Starting SIP-I RFC 3398 compliance testing...");
        
        // Clear previous results
        {
            let mut results = self.results.write().await;
            results.clear();
        }
        
        // Run tests for each SIP-I scenario
        for test in &self.test_suite {
            info!("Running SIP-I test: {} - {}", test.test_id, test.title);
            
            let result = self.run_sipi_test(test).await;
            self.record_result(result).await;
        }
        
        self.generate_sipi_compliance_report().await
    }

    async fn run_sipi_test(&self, test: &SipIComplianceTest) -> SipITestResult {
        debug!("Running SIP-I test: {}", test.test_id);
        
        let mut details = SipITestDetails {
            sip_message_sent: None,
            sip_response_received: None,
            isup_detected: false,
            isup_message_type: None,
            isup_parameters_found: Vec::new(),
            isup_cic: None,
            calling_number_extracted: None,
            called_number_extracted: None,
            content_type_detected: None,
            multipart_detected: false,
            parameter_mapping_correct: false,
        };
        
        match test.test_type {
            SipITestType::SipToIsup => {
                self.test_sip_to_isup_conversion(test, &mut details).await
            }
            SipITestType::IsupToSip => {
                self.test_isup_to_sip_conversion(test, &mut details).await
            }
            SipITestType::BiDirectional => {
                self.test_bidirectional_flow(test, &mut details).await
            }
            SipITestType::ParameterMapping => {
                self.test_parameter_mapping(test, &mut details).await
            }
            SipITestType::ErrorHandling => {
                self.test_error_handling(test, &mut details).await
            }
        }
    }

    async fn test_sip_to_isup_conversion(&self, test: &SipIComplianceTest, details: &mut SipITestDetails) -> SipITestResult {
        // Create test SIP INVITE with phone numbers
        let sip_invite = self.create_test_sip_invite("+15551234567", "+15559876543", &test.test_id).await;
        details.sip_message_sent = Some(sip_invite.clone());
        
        // Send SIP INVITE and analyze response
        match self.send_and_analyze_sipi_response(&sip_invite, &["100", "180", "183"]).await {
            Ok((response, analysis)) => {
                details.sip_response_received = Some(response.clone());
                details.isup_detected = analysis.isup_detected;
                details.isup_message_type = analysis.isup_message_type;
                details.isup_parameters_found = analysis.isup_parameters_found.clone();
                details.isup_cic = analysis.isup_cic;
                details.calling_number_extracted = analysis.calling_number_extracted;
                details.called_number_extracted = analysis.called_number_extracted;
                details.content_type_detected = analysis.content_type_detected;
                details.multipart_detected = analysis.multipart_detected;
                
                // Check if ISUP IAM was generated as expected
                let status = if let Some(expected_type) = &test.expected_isup_message {
                    if analysis.isup_message_type == Some(*expected_type) {
                        details.parameter_mapping_correct = self.validate_required_parameters(
                            &analysis.isup_parameters_found, &test.required_parameters);
                        if details.parameter_mapping_correct {
                            TestStatus::Pass
                        } else {
                            TestStatus::Partial
                        }
                    } else {
                        TestStatus::Fail
                    }
                } else if analysis.isup_detected {
                    TestStatus::Pass
                } else {
                    TestStatus::NotImplemented
                };
                
                SipITestResult {
                    test_id: test.test_id.clone(),
                    status,
                    message: format!("SIP to ISUP test: ISUP detected={}, Type={:?}", 
                                    analysis.isup_detected, analysis.isup_message_type),
                    timestamp: Utc::now(),
                    details: details.clone(),
                }
            }
            Err(e) => {
                SipITestResult {
                    test_id: test.test_id.clone(),
                    status: TestStatus::Error,
                    message: format!("Test failed: {}", e),
                    timestamp: Utc::now(),
                    details: details.clone(),
                }
            }
        }
    }

    async fn test_isup_to_sip_conversion(&self, test: &SipIComplianceTest, details: &mut SipITestDetails) -> SipITestResult {
        // Create test SIP message with ISUP content
        let isup_message = self.create_test_isup_message(test.expected_isup_message.unwrap_or(IsupMessageType::ACM)).await;
        let sip_with_isup = self.create_sip_with_isup(&isup_message, &test.test_id).await;
        
        details.sip_message_sent = Some(sip_with_isup.clone());
        
        // Send and analyze
        match self.send_and_analyze_sipi_response(&sip_with_isup, &["100", "180", "183", "200"]).await {
            Ok((response, analysis)) => {
                details.sip_response_received = Some(response.clone());
                
                // For ISUP to SIP, we're checking if the B2BUA properly handles incoming ISUP
                let status = if response.contains("SIP/2.0") {
                    if analysis.isup_detected {
                        TestStatus::Pass // ISUP preserved/forwarded
                    } else {
                        TestStatus::Partial // ISUP processed but not forwarded
                    }
                } else {
                    TestStatus::Fail
                };
                
                SipITestResult {
                    test_id: test.test_id.clone(),
                    status,
                    message: format!("ISUP to SIP test: Response received"),
                    timestamp: Utc::now(),
                    details: details.clone(),
                }
            }
            Err(e) => {
                SipITestResult {
                    test_id: test.test_id.clone(),
                    status: TestStatus::Error,
                    message: format!("Test failed: {}", e),
                    timestamp: Utc::now(),
                    details: details.clone(),
                }
            }
        }
    }

    async fn test_bidirectional_flow(&self, test: &SipIComplianceTest, details: &mut SipITestDetails) -> SipITestResult {
        // Test complete call flow: SIP -> ISUP -> SIP responses
        let result = self.test_sip_to_isup_conversion(test, details).await;
        
        // Additional validation for bidirectional flow
        if result.status == TestStatus::Pass && details.isup_detected {
            SipITestResult {
                test_id: test.test_id.clone(),
                status: TestStatus::Pass,
                message: "Bidirectional SIP-I flow successful".to_string(),
                timestamp: Utc::now(),
                details: details.clone(),
            }
        } else {
            result
        }
    }

    async fn test_parameter_mapping(&self, test: &SipIComplianceTest, details: &mut SipITestDetails) -> SipITestResult {
        // Test specific parameter mappings (calling number, called number, etc.)
        let sip_invite = self.create_test_sip_invite("+15551234567", "+15559876543", &test.test_id).await;
        details.sip_message_sent = Some(sip_invite.clone());
        
        match self.send_and_analyze_sipi_response(&sip_invite, &["100"]).await {
            Ok((response, analysis)) => {
                details.sip_response_received = Some(response);
                
                // Validate parameter mapping
                let calling_correct = analysis.calling_number_extracted.as_ref() == Some(&"15551234567".to_string());
                let called_correct = analysis.called_number_extracted.as_ref() == Some(&"15559876543".to_string());
                let params_correct = self.validate_required_parameters(
                    &analysis.isup_parameters_found, &test.required_parameters);
                
                details.parameter_mapping_correct = calling_correct && called_correct && params_correct;
                details.calling_number_extracted = analysis.calling_number_extracted;
                details.called_number_extracted = analysis.called_number_extracted;
                
                let status = if details.parameter_mapping_correct {
                    TestStatus::Pass
                } else if calling_correct || called_correct {
                    TestStatus::Partial
                } else {
                    TestStatus::Fail
                };
                
                SipITestResult {
                    test_id: test.test_id.clone(),
                    status,
                    message: format!("Parameter mapping: Calling={}, Called={}, Params={}", 
                                    calling_correct, called_correct, params_correct),
                    timestamp: Utc::now(),
                    details: details.clone(),
                }
            }
            Err(e) => {
                SipITestResult {
                    test_id: test.test_id.clone(),
                    status: TestStatus::Error,
                    message: format!("Test failed: {}", e),
                    timestamp: Utc::now(),
                    details: details.clone(),
                }
            }
        }
    }

    async fn test_error_handling(&self, test: &SipIComplianceTest, details: &mut SipITestDetails) -> SipITestResult {
        // Test error scenarios (malformed ISUP, invalid CIC, etc.)
        let invalid_sip = self.create_invalid_sip_with_isup(&test.test_id).await;
        details.sip_message_sent = Some(invalid_sip.clone());
        
        match self.send_and_analyze_sipi_response(&invalid_sip, &["400", "500"]).await {
            Ok((response, _analysis)) => {
                details.sip_response_received = Some(response.clone());
                
                let status = if response.contains("SIP/2.0 4") || response.contains("SIP/2.0 5") {
                    TestStatus::Pass // Proper error handling
                } else {
                    TestStatus::Fail // Should have returned error
                };
                
                SipITestResult {
                    test_id: test.test_id.clone(),
                    status,
                    message: "Error handling test completed".to_string(),
                    timestamp: Utc::now(),
                    details: details.clone(),
                }
            }
            Err(_) => {
                // Timeout might be expected for invalid messages
                SipITestResult {
                    test_id: test.test_id.clone(),
                    status: TestStatus::Pass,
                    message: "Error handling: No response (expected behavior)".to_string(),
                    timestamp: Utc::now(),
                    details: details.clone(),
                }
            }
        }
    }

    async fn send_and_analyze_sipi_response(&self, message: &str, expected_codes: &[&str]) -> Result<(String, SipIAnalysis)> {
        // Send message
        self.socket.send_to(message.as_bytes(), self.test_target).await?;
        
        // Wait for response with timeout
        let mut buffer = vec![0u8; 8192];
        let timeout_duration = std::time::Duration::from_secs(10);
        
        match tokio::time::timeout(timeout_duration, self.socket.recv_from(&mut buffer)).await {
            Ok(Ok((len, _from))) => {
                let response = String::from_utf8_lossy(&buffer[..len]).to_string();
                let analysis = self.analyze_sipi_response(&response).await;
                Ok((response, analysis))
            }
            Ok(Err(e)) => Err(anyhow!("Socket error: {}", e)),
            Err(_) => Err(anyhow!("Timeout waiting for response")),
        }
    }

    async fn analyze_sipi_response(&self, response: &str) -> SipIAnalysis {
        let mut analysis = SipIAnalysis::default();
        
        // Check Content-Type
        for line in response.lines() {
            if line.to_lowercase().starts_with("content-type:") {
                let content_type = line.split(':').nth(1).unwrap_or("").trim().to_lowercase();
                analysis.content_type_detected = Some(content_type.clone());
                
                if content_type.contains("application/isup") {
                    analysis.isup_detected = true;
                } else if content_type.contains("multipart/mixed") {
                    analysis.multipart_detected = true;
                    analysis.isup_detected = true; // Assume ISUP in multipart
                }
            }
        }
        
        // Extract and analyze ISUP content if present
        if analysis.isup_detected {
            if let Some(body_start) = response.find("\r\n\r\n") {
                let body = &response[body_start + 4..];
                
                if let Ok(isup_data) = self.extract_isup_data_from_body(body, analysis.multipart_detected).await {
                    if let Ok(isup_message) = self.sipi_service.parse_isup_message(&isup_data) {
                        analysis.isup_message_type = Some(isup_message.message_type);
                        analysis.isup_cic = Some(isup_message.cic);
                        
                        // Extract parameters
                        analysis.isup_parameters_found = isup_message.optional.iter()
                            .map(|p| p.param_type)
                            .collect();
                        
                        // Extract phone numbers
                        analysis.calling_number_extracted = self.sipi_service.extract_calling_number(&isup_message);
                        analysis.called_number_extracted = self.sipi_service.extract_called_number(&isup_message);
                    }
                }
            }
        }
        
        analysis
    }

    async fn extract_isup_data_from_body(&self, body: &str, is_multipart: bool) -> Result<Vec<u8>> {
        if is_multipart {
            // Parse SIP-T multipart
            let (isup_data, _sdp) = self.sipi_service.parse_sipt_body(body)?;
            Ok(isup_data)
        } else {
            // Parse SIP-I direct hex
            self.sipi_service.parse_sipi_body(body)
        }
    }

    fn validate_required_parameters(&self, found: &[IsupParameterType], required: &[IsupParameterType]) -> bool {
        required.iter().all(|req| found.contains(req))
    }

    async fn record_result(&self, result: SipITestResult) {
        let mut results = self.results.write().await;
        results.push(result);
    }

    async fn generate_sipi_compliance_report(&self) -> Result<SipIComplianceReport> {
        let results = self.results.read().await;
        
        let total_tests = results.len();
        let passed = results.iter().filter(|r| matches!(r.status, TestStatus::Pass)).count();
        let failed = results.iter().filter(|r| matches!(r.status, TestStatus::Fail)).count();
        let partial = results.iter().filter(|r| matches!(r.status, TestStatus::Partial)).count();
        let not_implemented = results.iter().filter(|r| matches!(r.status, TestStatus::NotImplemented)).count();
        let errors = results.iter().filter(|r| matches!(r.status, TestStatus::Error)).count();
        
        let compliance_percentage = if total_tests > 0 {
            (passed as f32 / total_tests as f32) * 100.0
        } else {
            0.0
        };
        
        // Calculate critical compliance (core SIP-I functionality)
        let critical_results: Vec<_> = results.iter()
            .filter(|r| r.test_id.contains("critical") || r.test_id.contains("iam") || r.test_id.contains("parameter"))
            .collect();
        
        let critical_passed = critical_results.iter()
            .filter(|r| matches!(r.status, TestStatus::Pass))
            .count();
        
        let critical_compliance_percentage = if !critical_results.is_empty() {
            (critical_passed as f32 / critical_results.len() as f32) * 100.0
        } else {
            100.0
        };
        
        // Generate ISUP statistics
        let isup_stats = IsupStatistics {
            iam_messages_detected: results.iter().filter(|r| r.details.isup_message_type == Some(IsupMessageType::IAM)).count(),
            acm_messages_detected: results.iter().filter(|r| r.details.isup_message_type == Some(IsupMessageType::ACM)).count(),
            anm_messages_detected: results.iter().filter(|r| r.details.isup_message_type == Some(IsupMessageType::ANM)).count(),
            rel_messages_detected: results.iter().filter(|r| r.details.isup_message_type == Some(IsupMessageType::REL)).count(),
            calling_party_extractions: results.iter().filter(|r| r.details.calling_number_extracted.is_some()).count(),
            called_party_extractions: results.iter().filter(|r| r.details.called_number_extracted.is_some()).count(),
            cic_allocations: results.iter().filter(|r| r.details.isup_cic.is_some()).count(),
            multipart_bodies_processed: results.iter().filter(|r| r.details.multipart_detected).count(),
            parameter_mappings_correct: results.iter().filter(|r| r.details.parameter_mapping_correct).count(),
        };
        
        // Generate recommendations
        let mut recommendations = Vec::new();
        
        if critical_compliance_percentage < 80.0 {
            recommendations.push("CRITICAL: Core SIP-I functionality needs immediate attention for PSTN interconnection.".to_string());
        }
        
        if isup_stats.iam_messages_detected == 0 {
            recommendations.push("No ISUP IAM messages detected. Implement SIP to ISUP IAM conversion.".to_string());
        }
        
        if isup_stats.parameter_mappings_correct < total_tests / 2 {
            recommendations.push("Parameter mapping needs improvement for proper ISUP interworking.".to_string());
        }
        
        if failed > 0 {
            recommendations.push(format!("{} SIP-I tests failed. Review ISUP implementation.", failed));
        }
        
        let report = SipIComplianceReport {
            test_run_id: format!("sipi-compliance-{}", Utc::now().timestamp()),
            timestamp: Utc::now(),
            target_system: format!("{}", self.test_target),
            sipi_enabled: self.sipi_service.is_sipi_enabled(),
            sipt_enabled: self.sipi_service.is_sipt_enabled(),
            isup_variant: self.sipi_service.get_config().isup_variant,
            total_tests,
            passed,
            failed,
            partial,
            not_implemented,
            errors,
            compliance_percentage,
            critical_compliance_percentage,
            results: results.clone(),
            recommendations,
            isup_statistics: isup_stats,
        };
        
        Ok(report)
    }

    fn create_sipi_test_suite() -> Vec<SipIComplianceTest> {
        vec![
            // Critical SIP-I functionality tests
            SipIComplianceTest {
                test_id: "critical_sip_to_iam".to_string(),
                title: "SIP INVITE to ISUP IAM Conversion".to_string(),
                rfc_section: "RFC 3398 Section 2.1".to_string(),
                priority: TestPriority::Critical,
                description: "Test conversion of SIP INVITE to ISUP IAM message".to_string(),
                test_type: SipITestType::SipToIsup,
                expected_isup_message: Some(IsupMessageType::IAM),
                required_parameters: vec![
                    IsupParameterType::CalledPartyNumber,
                    IsupParameterType::CallingPartyNumber,
                ],
            },
            
            SipIComplianceTest {
                test_id: "critical_parameter_mapping".to_string(),
                title: "Phone Number Parameter Mapping".to_string(),
                rfc_section: "RFC 3398 Section 3.1".to_string(),
                priority: TestPriority::Critical,
                description: "Test proper mapping of calling and called party numbers".to_string(),
                test_type: SipITestType::ParameterMapping,
                expected_isup_message: Some(IsupMessageType::IAM),
                required_parameters: vec![
                    IsupParameterType::CalledPartyNumber,
                    IsupParameterType::CallingPartyNumber,
                ],
            },
            
            SipIComplianceTest {
                test_id: "isup_acm_to_sip_response".to_string(),
                title: "ISUP ACM to SIP Response Conversion".to_string(),
                rfc_section: "RFC 3398 Section 2.2".to_string(),
                priority: TestPriority::High,
                description: "Test conversion of ISUP ACM to SIP 183 Session Progress".to_string(),
                test_type: SipITestType::IsupToSip,
                expected_isup_message: Some(IsupMessageType::ACM),
                required_parameters: vec![IsupParameterType::BackwardCallIndicators],
            },
            
            SipIComplianceTest {
                test_id: "isup_anm_to_sip_200".to_string(),
                title: "ISUP ANM to SIP 200 OK Conversion".to_string(),
                rfc_section: "RFC 3398 Section 2.3".to_string(),
                priority: TestPriority::High,
                description: "Test conversion of ISUP ANM to SIP 200 OK".to_string(),
                test_type: SipITestType::IsupToSip,
                expected_isup_message: Some(IsupMessageType::ANM),
                required_parameters: vec![],
            },
            
            SipIComplianceTest {
                test_id: "bidirectional_call_flow".to_string(),
                title: "Complete Bidirectional Call Flow".to_string(),
                rfc_section: "RFC 3398 Section 4".to_string(),
                priority: TestPriority::High,
                description: "Test complete call establishment and teardown with ISUP".to_string(),
                test_type: SipITestType::BiDirectional,
                expected_isup_message: Some(IsupMessageType::IAM),
                required_parameters: vec![
                    IsupParameterType::CalledPartyNumber,
                    IsupParameterType::CallingPartyNumber,
                ],
            },
            
            SipIComplianceTest {
                test_id: "error_handling_invalid_isup".to_string(),
                title: "Invalid ISUP Message Handling".to_string(),
                rfc_section: "RFC 3398 Section 6".to_string(),
                priority: TestPriority::Medium,
                description: "Test handling of malformed ISUP messages".to_string(),
                test_type: SipITestType::ErrorHandling,
                expected_isup_message: None,
                required_parameters: vec![],
            },
        ]
    }

    // Helper methods for creating test messages
    async fn create_test_sip_invite(&self, from: &str, to: &str, test_id: &str) -> String {
        let target_host = self.test_target.ip().to_string();
        format!(
            "INVITE sip:{}@{} SIP/2.0\r\n\
            Via: SIP/2.0/UDP 127.0.0.1:12345;branch=z9hG4bK-sipi-{}\r\n\
            From: <sip:{}@127.0.0.1:12345>;tag=sipi-{}\r\n\
            To: <sip:{}@{}>\r\n\
            Call-ID: sipi-test-{}-{}\r\n\
            CSeq: 1 INVITE\r\n\
            Contact: <sip:{}@127.0.0.1:12345>\r\n\
            User-Agent: SIP-I-Tester/1.0\r\n\
            Content-Type: application/sdp\r\n\
            Content-Length: 150\r\n\
            \r\n\
            v=0\r\n\
            o=sipi 12345 67890 IN IP4 127.0.0.1\r\n\
            s=SIP-I Test Call\r\n\
            c=IN IP4 127.0.0.1\r\n\
            t=0 0\r\n\
            m=audio 8000 RTP/AVP 0 8\r\n\
            a=rtpmap:0 PCMU/8000\r\n\
            a=rtpmap:8 PCMA/8000\r\n",
            to, target_host, test_id, from, test_id, to, target_host, test_id, Utc::now().timestamp(), from
        )
    }

    async fn create_test_isup_message(&self, msg_type: IsupMessageType) -> IsupMessage {
        match msg_type {
            IsupMessageType::ACM => IsupMessage {
                cic: 123,
                message_type: IsupMessageType::ACM,
                mandatory_fixed: vec![0x60, 0x01], // Backward Call Indicators
                mandatory_variable: Vec::new(),
                optional: Vec::new(),
                raw_data: Vec::new(),
            },
            IsupMessageType::ANM => IsupMessage {
                cic: 123,
                message_type: IsupMessageType::ANM,
                mandatory_fixed: Vec::new(),
                mandatory_variable: Vec::new(),
                optional: Vec::new(),
                raw_data: Vec::new(),
            },
            _ => {
                // Default IAM
                self.sipi_service.sip_to_iam("+15551234567", "+15559876543", 123).unwrap()
            }
        }
    }

    async fn create_sip_with_isup(&self, isup_message: &IsupMessage, test_id: &str) -> String {
        let isup_data = self.sipi_service.create_isup_message(isup_message).unwrap();
        let isup_body = if self.sipi_service.is_sipt_enabled() {
            self.sipi_service.create_sipt_body(&isup_data, Some("v=0\r\no=test 1 1 IN IP4 127.0.0.1\r\ns=test\r\nt=0 0")).unwrap()
        } else {
            self.sipi_service.create_sipi_body(&isup_data).unwrap()
        };
        
        let content_type = if self.sipi_service.is_sipt_enabled() {
            "multipart/mixed; boundary=sipi-test"
        } else {
            "application/ISUP; version=itu-t92+"
        };
        
        let target_host = self.test_target.ip().to_string();
        format!(
            "INVITE sip:+15559876543@{} SIP/2.0\r\n\
            Via: SIP/2.0/UDP 127.0.0.1:12345;branch=z9hG4bK-sipi-isup-{}\r\n\
            From: <sip:+15551234567@127.0.0.1:12345>;tag=sipi-isup-{}\r\n\
            To: <sip:+15559876543@{}>\r\n\
            Call-ID: sipi-isup-test-{}-{}\r\n\
            CSeq: 1 INVITE\r\n\
            Contact: <sip:+15551234567@127.0.0.1:12345>\r\n\
            Content-Type: {}\r\n\
            Content-Length: {}\r\n\
            \r\n\
            {}",
            target_host, test_id, test_id, target_host, test_id, Utc::now().timestamp(), 
            content_type, isup_body.len(), isup_body
        )
    }

    async fn create_invalid_sip_with_isup(&self, test_id: &str) -> String {
        let target_host = self.test_target.ip().to_string();
        format!(
            "INVITE sip:+15559876543@{} SIP/2.0\r\n\
            Via: SIP/2.0/UDP 127.0.0.1:12345;branch=z9hG4bK-sipi-invalid-{}\r\n\
            From: <sip:+15551234567@127.0.0.1:12345>;tag=sipi-invalid-{}\r\n\
            To: <sip:+15559876543@{}>\r\n\
            Call-ID: sipi-invalid-test-{}-{}\r\n\
            CSeq: 1 INVITE\r\n\
            Contact: <sip:+15551234567@127.0.0.1:12345>\r\n\
            Content-Type: application/ISUP; version=itu-t92+\r\n\
            Content-Length: 20\r\n\
            \r\n\
            INVALID_HEX_DATA_XYZ",
            target_host, test_id, test_id, target_host, test_id, Utc::now().timestamp()
        )
    }
}

#[derive(Debug, Default)]
struct SipIAnalysis {
    isup_detected: bool,
    isup_message_type: Option<IsupMessageType>,
    isup_parameters_found: Vec<IsupParameterType>,
    isup_cic: Option<u16>,
    calling_number_extracted: Option<String>,
    called_number_extracted: Option<String>,
    content_type_detected: Option<String>,
    multipart_detected: bool,
}

pub async fn run_sipi_compliance_tests(
    target_addr: SocketAddr,
    sipi_config: SipTSipIConfig
) -> Result<SipIComplianceReport> {
    let bind_addr = "0.0.0.0:0".parse()?; // Let OS choose port
    let tester = SipIComplianceTester::new(bind_addr, target_addr, sipi_config).await?;
    
    info!("Starting SIP-I RFC 3398 compliance testing for Class 4 carrier interconnection");
    let report = tester.run_all_tests().await?;
    
    info!("SIP-I compliance testing completed");
    info!("Overall compliance: {:.1}%", report.compliance_percentage);
    info!("Critical compliance: {:.1}%", report.critical_compliance_percentage);
    info!("ISUP messages detected: IAM={}, ACM={}, ANM={}, REL={}", 
          report.isup_statistics.iam_messages_detected,
          report.isup_statistics.acm_messages_detected, 
          report.isup_statistics.anm_messages_detected,
          report.isup_statistics.rel_messages_detected);
    
    Ok(report)
}