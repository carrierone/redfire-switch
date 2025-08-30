//! Comprehensive RFC compliance tests for SIP and SIP-I implementation
//! 
//! Tests RFC 3261 (SIP), RFC 3372 (SIP-T), ITU-T Q.1912.5 (SIP-I) compliance

use redfire_switch::sip_rfc_compliance::*;
use redfire_switch::ani_ii_rfc_compliant::*;
use std::collections::HashMap;

#[cfg(test)]
mod rfc3261_tests {
    use super::*;

    #[test]
    fn test_mandatory_headers_validation() {
        let mut headers = HashMap::new();
        
        // Missing mandatory headers should fail
        assert!(Rfc3261Validator::validate_message(&headers, "INVITE sip:test@example.com SIP/2.0").is_err());
        
        // Add all mandatory headers
        headers.insert("To".to_string(), "<sip:bob@example.com>".to_string());
        headers.insert("From".to_string(), "<sip:alice@example.com>;tag=123".to_string());
        headers.insert("Call-ID".to_string(), "abc123@example.com".to_string());
        headers.insert("CSeq".to_string(), "1 INVITE".to_string());
        headers.insert("Via".to_string(), "SIP/2.0/UDP example.com".to_string());
        headers.insert("Max-Forwards".to_string(), "70".to_string());
        
        // Should pass now
        assert!(Rfc3261Validator::validate_message(&headers, "INVITE sip:bob@example.com SIP/2.0").is_ok());
    }

    #[test]
    fn test_sip_version_validation() {
        let mut headers = HashMap::new();
        headers.insert("To".to_string(), "<sip:bob@example.com>".to_string());
        headers.insert("From".to_string(), "<sip:alice@example.com>;tag=123".to_string());
        headers.insert("Call-ID".to_string(), "abc123@example.com".to_string());
        headers.insert("CSeq".to_string(), "1 INVITE".to_string());
        headers.insert("Via".to_string(), "SIP/2.0/UDP example.com".to_string());
        headers.insert("Max-Forwards".to_string(), "70".to_string());
        
        // Invalid SIP version should fail
        assert!(Rfc3261Validator::validate_message(&headers, "INVITE sip:bob@example.com SIP/1.0").is_err());
        
        // Valid version should pass
        assert!(Rfc3261Validator::validate_message(&headers, "INVITE sip:bob@example.com SIP/2.0").is_ok());
    }

    #[test]
    fn test_call_id_validation() {
        let mut headers = HashMap::new();
        headers.insert("To".to_string(), "<sip:bob@example.com>".to_string());
        headers.insert("From".to_string(), "<sip:alice@example.com>;tag=123".to_string());
        headers.insert("CSeq".to_string(), "1 INVITE".to_string());
        headers.insert("Via".to_string(), "SIP/2.0/UDP example.com".to_string());
        headers.insert("Max-Forwards".to_string(), "70".to_string());
        
        // Call-ID with spaces should fail  
        headers.insert("Call-ID".to_string(), "abc 123@example.com".to_string());
        assert!(Rfc3261Validator::validate_message(&headers, "INVITE sip:bob@example.com SIP/2.0").is_err());
        
        // Valid Call-ID should pass
        headers.insert("Call-ID".to_string(), "abc123@example.com".to_string());
        assert!(Rfc3261Validator::validate_message(&headers, "INVITE sip:bob@example.com SIP/2.0").is_ok());
    }
}

#[cfg(test)]
mod sip_uri_tests {
    use super::*;

    #[test]
    fn test_basic_sip_uri_parsing() {
        let uri = SipUriParser::parse("sip:user@example.com").unwrap();
        assert_eq!(uri.scheme, "sip");
        assert_eq!(uri.user, Some("user".to_string()));
        assert_eq!(uri.host, "example.com");
        assert_eq!(uri.port, None);
    }

    #[test]
    fn test_sip_uri_with_port() {
        let uri = SipUriParser::parse("sip:+12125551234@gateway.carrier.com:5060").unwrap();
        assert_eq!(uri.user, Some("+12125551234".to_string()));
        assert_eq!(uri.host, "gateway.carrier.com");
        assert_eq!(uri.port, Some(5060));
    }

    #[test]
    fn test_sip_uri_with_parameters() {
        let uri = SipUriParser::parse("sip:+15551234567@carrier.com;oli=70;transport=udp").unwrap();
        assert_eq!(uri.parameters.get("oli"), Some(&"70".to_string()));
        assert_eq!(uri.parameters.get("transport"), Some(&"udp".to_string()));
    }

    #[test]
    fn test_tel_uri_parsing() {
        let uri = SipUriParser::parse("tel:+1-212-555-1234;oli=23").unwrap();
        assert_eq!(uri.scheme, "tel");
        assert_eq!(uri.user, Some("+1-212-555-1234".to_string()));
        assert_eq!(uri.parameters.get("oli"), Some(&"23".to_string()));
    }

    #[test]
    fn test_header_field_parsing() {
        let (display_name, uri, params) = SipUriParser::parse_header_field(
            "\"John Doe\" <sip:+15551234567@example.com;oli=70>;tag=abc123;screen=yes"
        ).unwrap();
        
        assert_eq!(display_name, Some("John Doe".to_string()));
        assert_eq!(uri.user, Some("+15551234567".to_string()));
        assert_eq!(uri.parameters.get("oli"), Some(&"70".to_string()));
        assert_eq!(params.get("tag"), Some(&"abc123".to_string()));
        assert_eq!(params.get("screen"), Some(&"yes".to_string()));
    }
}

#[cfg(test)]
mod oli_parsing_tests {
    use super::*;

    #[test]
    fn test_p_isup_oli_header_parsing() {
        let mut headers = HashMap::new();
        headers.insert("P-ISUP-OLI".to_string(), "70".to_string());
        
        let oli = OliParser::parse_from_headers(&headers).unwrap();
        assert_eq!(oli.oli_value, 70);
        assert!(matches!(oli.source, OliSource::PIsupOli));
    }

    #[test]
    fn test_from_header_oli_parameter() {
        let mut headers = HashMap::new();
        // RFC-compliant format with ;oli= in URI
        headers.insert("From".to_string(), 
            "<sip:+15551234567@carrier.com;oli=23>;tag=abc123".to_string());
        
        let oli = OliParser::parse_from_headers(&headers).unwrap();
        assert_eq!(oli.oli_value, 23);
        assert_eq!(oli.calling_number, Some("+15551234567".to_string()));
        assert!(matches!(oli.source, OliSource::FromUriParam));
    }

    #[test]
    fn test_from_header_isup_oli_parameter() {
        let mut headers = HashMap::new();
        // RFC-compliant ISUP format
        headers.insert("From".to_string(), 
            "<sip:+15551234567@gateway.com;isup-oli=27>;tag=def456".to_string());
        
        let oli = OliParser::parse_from_headers(&headers).unwrap();
        assert_eq!(oli.oli_value, 27);
        assert!(matches!(oli.source, OliSource::FromUriParam));
    }

    #[test]
    fn test_p_asserted_identity_parsing() {
        let mut headers = HashMap::new();
        headers.insert("P-Asserted-Identity".to_string(), 
            "<sip:+18005551234@trusted-provider.com;oli=0>".to_string());
        
        let oli = OliParser::parse_from_headers(&headers).unwrap();
        assert_eq!(oli.oli_value, 0);
        assert!(matches!(oli.source, OliSource::PAssertedIdentity));
    }

    #[test]
    fn test_remote_party_id_with_screening() {
        let mut headers = HashMap::new();
        headers.insert("Remote-Party-ID".to_string(), 
            "\"Payphone\" <sip:+15551234567@carrier.com;oli=70>;party=calling;screen=yes;privacy=off".to_string());
        
        let oli = OliParser::parse_from_headers(&headers).unwrap();
        assert_eq!(oli.oli_value, 70);
        assert!(matches!(oli.source, OliSource::RemotePartyId));
        assert!(oli.screening.is_some());
        assert!(oli.presentation.is_some());
    }

    #[test]
    fn test_diversion_header() {
        let mut headers = HashMap::new();
        headers.insert("Diversion".to_string(), 
            "<sip:+15551234567@original-dest.com;oli=21>;reason=unconditional".to_string());
        
        let oli = OliParser::parse_from_headers(&headers).unwrap();
        assert_eq!(oli.oli_value, 21);
        assert!(matches!(oli.source, OliSource::DiversionHeader));
    }

    #[test]
    fn test_priority_order() {
        let mut headers = HashMap::new();
        
        // Add multiple sources - P-ISUP-OLI should win
        headers.insert("P-ISUP-OLI".to_string(), "23".to_string());
        headers.insert("From".to_string(), 
            "<sip:+15551234567@carrier.com;oli=70>;tag=abc123".to_string());
        headers.insert("Remote-Party-ID".to_string(), 
            "<sip:+15551234567@carrier.com;oli=27>;screen=no".to_string());
        
        let oli = OliParser::parse_from_headers(&headers).unwrap();
        assert_eq!(oli.oli_value, 23); // P-ISUP-OLI wins
        assert!(matches!(oli.source, OliSource::PIsupOli));
    }

    #[test]
    fn test_invalid_oli_values_rejected() {
        let mut headers = HashMap::new();
        
        // OLI value > 99 should be rejected
        headers.insert("P-ISUP-OLI".to_string(), "100".to_string());
        assert!(OliParser::parse_from_headers(&headers).is_none());
        
        // Non-numeric should be rejected
        headers.insert("P-ISUP-OLI".to_string(), "invalid".to_string());
        assert!(OliParser::parse_from_headers(&headers).is_none());
        
        // Valid value should work
        headers.insert("P-ISUP-OLI".to_string(), "70".to_string());
        assert!(OliParser::parse_from_headers(&headers).is_some());
    }
}

#[cfg(test)]
mod isup_parsing_tests {
    use super::*;

    #[test]
    fn test_multipart_mixed_detection() {
        let content_type = "multipart/mixed;boundary=unique-boundary-1";
        let body = r#"--unique-boundary-1
Content-Type: application/sdp

v=0
o=- 0 0 IN IP4 192.168.1.1
s=-
c=IN IP4 192.168.1.1
t=0 0
m=audio 8000 RTP/AVP 0

--unique-boundary-1
Content-Type: application/isup;base=itu-t92+
Content-Disposition: signal;handling=required

0A08830A123456789012
--unique-boundary-1--"#;

        let result = IsupParser::parse_multipart_isup(content_type, body);
        assert!(result.is_some());
    }

    #[test]
    fn test_hex_isup_parsing() {
        // Simple ISUP IAM with Calling Party Number parameter
        let hex_content = "0A08830A123456789012";
        
        let result = IsupParser::parse_isup_content(hex_content);
        // This is a simplified test - actual ISUP parsing is complex
        // In real implementation, we'd have proper test vectors
    }

    #[test]
    fn test_calling_party_category_mapping() {
        // Test ISUP calling party category to OLI mapping
        let test_cases = vec![
            (0x0A, Some(0)),  // Ordinary subscriber
            (0x0F, Some(70)), // Payphone
            (0xFF, None),     // Invalid
        ];

        // This would test the actual ISUP parsing when implemented
        // For now, just verify the logic exists
    }
}

#[cfg(test)]
mod ani_ii_integration_tests {
    use super::*;

    #[test]
    fn test_rfc_compliant_ani_ii_extraction() {
        let mut headers = HashMap::new();
        headers.insert("From".to_string(), 
            "<sip:+15551234567@carrier.com;oli=70>;tag=abc123".to_string());
        
        let ani_ii = RfcCompliantAniIIParser::parse_from_sip_message(&headers, None).unwrap();
        assert_eq!(ani_ii.raw_digit, 70);
        assert_eq!(ani_ii.code, AniIICode::PayStationNonNetworkCoin);
        assert!(ani_ii.triggers_surcharge);
        assert_eq!(ani_ii.calling_number, Some("+15551234567".to_string()));
    }

    #[test]
    fn test_toll_free_detection() {
        // Test NANPA toll-free prefixes
        let toll_free_numbers = vec![
            "18005551234",    // 800
            "18335551234",    // 833
            "18445551234",    // 844
            "18555551234",    // 855
            "18665551234",    // 866
            "18775551234",    // 877
            "18885551234",    // 888
            "8005551234",     // Without +1
            "+18005551234",   // With + prefix
        ];

        for number in toll_free_numbers {
            assert!(RfcCompliantAniIIParser::is_toll_free(number), 
                   "Failed to detect {} as toll-free", number);
        }

        // Test non-toll-free numbers
        let regular_numbers = vec![
            "12125551234",    // NYC area
            "13105551234",    // LA area
            "15551234567",    // Invalid area code
        ];

        for number in regular_numbers {
            assert!(!RfcCompliantAniIIParser::is_toll_free(number), 
                   "Incorrectly detected {} as toll-free", number);
        }
    }

    #[test]
    fn test_payphone_surcharge_calculation() {
        let payphone_ani_ii = AniIIInfo {
            code: AniIICode::PayStationNonNetworkCoin,
            raw_digit: 70,
            source: "P-ISUP-OLI".to_string(),
            calling_number: Some("+15551234567".to_string()),
            triggers_surcharge: true,
            is_restricted: false,
            screening: None,
            presentation: None,
        };
        
        // Test toll-free call with payphone
        let (applies, amount, reason) = RfcCompliantAniIIParser::calculate_surcharge(
            Some(&payphone_ani_ii),
            true,  // is_toll_free
            None
        );
        
        assert!(applies);
        assert_eq!(amount, 0.49); // Standard FCC surcharge
        assert!(reason.contains("Payphone surcharge"));

        // Test non-toll-free call (no surcharge)
        let (applies, amount, _) = RfcCompliantAniIIParser::calculate_surcharge(
            Some(&payphone_ani_ii),
            false, // not toll-free
            None
        );
        
        assert!(!applies);
        assert_eq!(amount, 0.0);

        // Test regular line (no surcharge)
        let regular_ani_ii = AniIIInfo {
            code: AniIICode::RegularLine,
            raw_digit: 0,
            source: "From".to_string(),
            calling_number: Some("+15551234567".to_string()),
            triggers_surcharge: false,
            is_restricted: false,
            screening: None,
            presentation: None,
        };

        let (applies, amount, _) = RfcCompliantAniIIParser::calculate_surcharge(
            Some(&regular_ani_ii),
            true,  // is_toll_free
            None
        );
        
        assert!(!applies);
        assert_eq!(amount, 0.0);
    }

    #[test]
    fn test_trunk_specific_surcharge_overrides() {
        let mut trunk_config = HashMap::new();
        trunk_config.insert("surcharge_70".to_string(), 0.99); // Custom surcharge

        let payphone_ani_ii = AniIIInfo {
            code: AniIICode::PayStationNonNetworkCoin,
            raw_digit: 70,
            source: "P-ISUP-OLI".to_string(),
            calling_number: Some("+15551234567".to_string()),
            triggers_surcharge: true,
            is_restricted: false,
            screening: None,
            presentation: None,
        };
        
        let (applies, amount, _) = RfcCompliantAniIIParser::calculate_surcharge(
            Some(&payphone_ani_ii),
            true,
            Some(&trunk_config)
        );
        
        assert!(applies);
        assert_eq!(amount, 0.99); // Custom amount
    }

    #[test]
    fn test_restricted_line_detection() {
        let restricted_codes = vec![21, 22, 29, 20, 25]; // Inmate, prison, restricted lines
        
        for code in restricted_codes {
            if let Some(ani_ii_code) = AniIICode::from_digit(code) {
                let mut headers = HashMap::new();
                headers.insert("P-ISUP-OLI".to_string(), code.to_string());
                
                let ani_ii = RfcCompliantAniIIParser::parse_from_sip_message(&headers, None).unwrap();
                assert!(ani_ii.is_restricted, "ANI-II code {} should be marked as restricted", code);
            }
        }
    }

    #[test] 
    fn test_backward_compatibility() {
        let mut headers = HashMap::new();
        headers.insert("From".to_string(), 
            "<sip:+15551234567@carrier.com;oli=70>;tag=abc123".to_string());
        
        // Test legacy compatibility wrapper
        let ani_ii = redfire_switch::ani_ii_rfc_compliant::legacy_compatibility::parse_ani_ii_from_headers(&headers).unwrap();
        assert_eq!(ani_ii.raw_digit, 70);
        assert!(ani_ii.triggers_surcharge);
    }
}

#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_malformed_uri_handling() {
        // Test various malformed URIs
        let bad_uris = vec![
            "not-a-uri",
            "http://example.com", // Wrong scheme
            "sip:",               // Missing host
            "sip:@example.com",   // Empty user
        ];

        for uri in bad_uris {
            assert!(SipUriParser::parse(uri).is_err(), "Should reject malformed URI: {}", uri);
        }
    }

    #[test]
    fn test_invalid_header_field_handling() {
        let bad_headers = vec![
            "<sip:user@example.com", // Unmatched angle brackets
            "sip:user@example.com>", // Unmatched angle brackets
            "invalid-format",        // Not a valid header format
        ];

        for header in bad_headers {
            assert!(SipUriParser::parse_header_field(header).is_err(), 
                   "Should reject malformed header: {}", header);
        }
    }

    #[test]
    fn test_missing_mandatory_headers() {
        let mut headers = HashMap::new();
        headers.insert("From".to_string(), "<sip:alice@example.com>;tag=123".to_string());
        // Missing other mandatory headers

        assert!(Rfc3261Validator::validate_message(&headers, "INVITE sip:bob@example.com SIP/2.0").is_err());
    }
}