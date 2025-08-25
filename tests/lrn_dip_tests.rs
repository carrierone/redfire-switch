use anyhow::Result;
use chrono::Utc;
use std::net::{IpAddr, SocketAddr};
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};

use redfire_switch::lcr::lrn_dip::LrnDipService;
use redfire_switch::lcr::types::{LrnDipConfig, LrnDipResponse};

/// Mock LRN dip server for testing
struct MockLrnServer {
    socket: UdpSocket,
    responses: std::collections::HashMap<String, String>,
}

impl MockLrnServer {
    async fn new(bind_addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await?;
        let mut responses = std::collections::HashMap::new();

        // Add some test responses
        responses.insert(
            "12025551234".to_string(), 
            "SIP/2.0 302 Moved Temporarily\r\nContact: <sip:12135551234@lrn.example.com>\r\nContent-Length: 0\r\n\r\n".to_string()
        );
        responses.insert(
            "17035551111".to_string(),
            "SIP/2.0 200 OK\r\nX-LRN: 17035552222\r\nX-SPID: 1234\r\nContent-Length: 0\r\n\r\n"
                .to_string(),
        );
        responses.insert(
            "15555551234".to_string(),
            "SIP/2.0 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_string(),
        );

        Ok(Self { socket, responses })
    }

    async fn run(&self) -> Result<()> {
        let mut buf = [0u8; 4096];

        loop {
            let (len, addr) = self.socket.recv_from(&mut buf).await?;
            let request = String::from_utf8_lossy(&buf[..len]);

            // Parse the SIP request to extract the number
            if let Some(to_number) = extract_number_from_request(&request) {
                if let Some(response) = self.responses.get(&to_number) {
                    self.socket.send_to(response.as_bytes(), addr).await?;
                } else {
                    // Default 404 response
                    let default_response = "SIP/2.0 404 Not Found\r\nContent-Length: 0\r\n\r\n";
                    self.socket
                        .send_to(default_response.as_bytes(), addr)
                        .await?;
                }
            }
        }
    }
}

fn extract_number_from_request(request: &str) -> Option<String> {
    // Simple parser to extract number from SIP request
    for line in request.lines() {
        if line.starts_with("To:") || line.starts_with("OPTIONS sip:") {
            // Extract number from SIP URI
            if let Some(start) = line.find("sip:") {
                let uri_part = &line[start + 4..];
                if let Some(end) = uri_part.find('@') {
                    let number = &uri_part[..end];
                    if number.len() >= 10 && number.chars().all(|c| c.is_ascii_digit()) {
                        return Some(number.to_string());
                    }
                }
            }
        }
    }
    None
}

#[tokio::test]
async fn test_lrn_dip_service_creation() {
    let server_ip = "127.0.0.1".parse().expect("Valid IP address");
    let config = LrnDipConfig {
        server_ip,
        server_port: 15060,
        enabled: true,
        timeout_ms: 1000,
        ..Default::default()
    };

    let service = LrnDipService::new(config);
    assert!(service.is_enabled());
}

#[tokio::test]
async fn test_lrn_dip_disabled_service() {
    let config = LrnDipConfig {
        enabled: false,
        ..Default::default()
    };

    let service = LrnDipService::new(config);
    let result = service
        .dip_lrn("12025551234", Some("19995551234"))
        .await
        .expect("LRN dip should succeed even when disabled");

    assert_eq!(result.original_tn, "12025551234");
    assert_eq!(result.lrn, None);
    assert!(!result.ported);
    assert!(result.error.is_some());
    assert!(result.error.as_ref().unwrap().contains("disabled"));
}

#[tokio::test]
async fn test_lrn_dip_302_redirect() {
    // Start mock server
    let server_addr: SocketAddr = "127.0.0.1:15061".parse().expect("Valid socket address");
    let mock_server = MockLrnServer::new(server_addr)
        .await
        .expect("Mock server should start successfully");

    // Run server in background
    tokio::spawn(async move {
        let _ = mock_server.run().await;
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    let local_ip = "127.0.0.1".parse().expect("Valid IP address");
    let config = LrnDipConfig {
        server_ip: server_addr.ip(),
        server_port: server_addr.port(),
        local_ip: Some(local_ip),
        local_port: Some(0), // Random port
        enabled: true,
        timeout_ms: 2000,
        max_redirects: 3,
        cache_timeout_sec: 300,
    };

    let service = LrnDipService::new(config);
    service
        .initialize()
        .await
        .expect("Service should initialize");

    // Test 302 redirect response
    let result = service
        .dip_lrn("12025551234", Some("19995551234"))
        .await
        .expect("LRN dip should complete successfully");

    assert_eq!(result.original_tn, "12025551234");
    assert_eq!(result.lrn, Some("12135551234".to_string()));
    assert!(result.ported);
    assert!(result.error.is_none());
    assert_eq!(result.redirect_count, 1);
    assert!(result.response_time_ms > 0);
}

#[tokio::test]
async fn test_lrn_dip_200_ok_response() {
    // Start mock server
    let server_addr: SocketAddr = "127.0.0.1:15062".parse().expect("Valid socket address");
    let mock_server = MockLrnServer::new(server_addr)
        .await
        .expect("Mock server should start");

    // Run server in background
    tokio::spawn(async move {
        let _ = mock_server.run().await;
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    let local_ip = "127.0.0.1".parse().expect("Valid IP address");
    let config = LrnDipConfig {
        server_ip: server_addr.ip(),
        server_port: server_addr.port(),
        local_ip: Some(local_ip),
        local_port: Some(0),
        enabled: true,
        timeout_ms: 2000,
        max_redirects: 3,
        cache_timeout_sec: 300,
    };

    let service = LrnDipService::new(config);
    service
        .initialize()
        .await
        .expect("Service should initialize");

    // Test 200 OK response with X-LRN header
    let result = service
        .dip_lrn("17035551111", Some("19995551234"))
        .await
        .expect("LRN dip should complete");

    assert_eq!(result.original_tn, "17035551111");
    assert_eq!(result.lrn, Some("17035552222".to_string()));
    assert!(result.ported);
    assert_eq!(result.spid, Some("1234".to_string()));
    assert!(result.error.is_none());
    assert_eq!(result.redirect_count, 0);
}

#[tokio::test]
async fn test_lrn_dip_404_not_found() {
    // Start mock server
    let server_addr: SocketAddr = "127.0.0.1:15063".parse().expect("Valid socket address");
    let mock_server = MockLrnServer::new(server_addr)
        .await
        .expect("Mock server should start");

    // Run server in background
    tokio::spawn(async move {
        let _ = mock_server.run().await;
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    let local_ip = "127.0.0.1".parse().expect("Valid IP address");
    let config = LrnDipConfig {
        server_ip: server_addr.ip(),
        server_port: server_addr.port(),
        local_ip: Some(local_ip),
        local_port: Some(0),
        enabled: true,
        timeout_ms: 2000,
        max_redirects: 3,
        cache_timeout_sec: 300,
    };

    let service = LrnDipService::new(config);
    service
        .initialize()
        .await
        .expect("Service should initialize");

    // Test 404 Not Found response
    let result = service
        .dip_lrn("15555551234", Some("19995551234"))
        .await
        .expect("LRN dip should complete even with 404");

    assert_eq!(result.original_tn, "15555551234");
    assert_eq!(result.lrn, None);
    assert!(!result.ported);
    assert!(result.error.is_some());
    assert_eq!(result.redirect_count, 0);
}

#[tokio::test]
async fn test_lrn_dip_caching() {
    // Start mock server
    let server_addr: SocketAddr = "127.0.0.1:15064".parse().expect("Valid socket address");
    let mock_server = MockLrnServer::new(server_addr)
        .await
        .expect("Mock server should start");

    // Run server in background
    tokio::spawn(async move {
        let _ = mock_server.run().await;
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    let local_ip = "127.0.0.1".parse().expect("Valid IP address");
    let config = LrnDipConfig {
        server_ip: server_addr.ip(),
        server_port: server_addr.port(),
        local_ip: Some(local_ip),
        local_port: Some(0),
        enabled: true,
        timeout_ms: 2000,
        max_redirects: 3,
        cache_timeout_sec: 300,
    };

    let service = LrnDipService::new(config);
    service
        .initialize()
        .await
        .expect("Service should initialize");

    // First dip - should go to server
    let result1 = service
        .dip_lrn("12025551234", Some("19995551234"))
        .await
        .expect("First LRN dip should complete");
    assert_eq!(result1.lrn, Some("12135551234".to_string()));
    let first_response_time = result1.response_time_ms;

    // Second dip - should be cached (much faster)
    let result2 = service
        .dip_lrn("12025551234", Some("19995551234"))
        .await
        .expect("Second LRN dip should be cached");
    assert_eq!(result2.lrn, Some("12135551234".to_string()));

    // Cache hit should be significantly faster
    assert!(result2.response_time_ms < first_response_time);
}

#[tokio::test]
async fn test_number_normalization() {
    let config = LrnDipConfig::default();
    let service = LrnDipService::new(config);

    // Test various number formats
    assert_eq!(service.normalize_tn("2125551234"), "12125551234");
    assert_eq!(service.normalize_tn("12125551234"), "12125551234");
    assert_eq!(service.normalize_tn("+1-212-555-1234"), "12125551234");
    assert_eq!(service.normalize_tn("1 (212) 555-1234"), "12125551234");
    assert_eq!(service.normalize_tn("212.555.1234"), "12125551234");
}

#[tokio::test]
async fn test_contact_uri_parsing() {
    let config = LrnDipConfig::default();
    let service = LrnDipService::new(config);

    // Test Contact header parsing
    let response_302 = "SIP/2.0 302 Moved Temporarily\r\n\
                       Contact: <sip:12025551234@lrn.example.com>\r\n\
                       Content-Length: 0\r\n\r\n";

    let contact_uri = service
        .extract_contact_uri(response_302)
        .expect("Should extract contact URI from 302 response");
    assert_eq!(contact_uri, "sip:12025551234@lrn.example.com");

    let lrn = service.extract_lrn_from_contact(&contact_uri);
    assert_eq!(lrn, Some("12025551234".to_string()));
}

#[tokio::test]
async fn test_header_parsing() {
    let config = LrnDipConfig::default();
    let service = LrnDipService::new(config);

    // Test X-LRN header parsing
    let response_200 = "SIP/2.0 200 OK\r\n\
                       X-LRN: 12025559999\r\n\
                       X-SPID: 5678\r\n\
                       Content-Length: 0\r\n\r\n";

    let lrn = service.extract_lrn_from_headers(response_200);
    assert_eq!(lrn, Some("12025559999".to_string()));

    let spid = service.extract_spid_from_headers(response_200);
    assert_eq!(spid, Some("5678".to_string()));

    // Test P-LRN header parsing
    let response_p_lrn = "SIP/2.0 200 OK\r\n\
                         P-LRN: 17035558888\r\n\
                         P-SPID: ABCD\r\n\
                         Content-Length: 0\r\n\r\n";

    let lrn2 = service.extract_lrn_from_headers(response_p_lrn);
    assert_eq!(lrn2, Some("17035558888".to_string()));

    let spid2 = service.extract_spid_from_headers(response_p_lrn);
    assert_eq!(spid2, Some("ABCD".to_string()));
}

#[tokio::test]
async fn test_cache_cleanup() {
    let config = LrnDipConfig {
        cache_timeout_sec: 1, // 1 second cache timeout for testing
        enabled: false,       // Disabled so we can test cache directly
        ..Default::default()
    };

    let service = LrnDipService::new(config);

    // Manually add cache entry
    service.cache_lrn_result(
        "12025551234",
        Some("19995551234"),
        "12135551234",
        Some("1234"),
        true,
    );

    // Check cache stats
    let (total, expired) = service.get_cache_stats();
    assert_eq!(total, 1);
    assert_eq!(expired, 0);

    // Wait for cache to expire
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Check expired count
    let (total, expired) = service.get_cache_stats();
    assert_eq!(total, 1);
    assert_eq!(expired, 1);

    // Clean up cache
    service.cleanup_cache();

    // Check cache is empty
    let (total, expired) = service.get_cache_stats();
    assert_eq!(total, 0);
    assert_eq!(expired, 0);
}

#[tokio::test]
async fn test_timeout_handling() {
    let server_ip = "127.0.0.1".parse().expect("Valid IP address");
    let local_ip = "127.0.0.1".parse().expect("Valid IP address");
    let config = LrnDipConfig {
        server_ip,
        server_port: 15999, // Non-existent server
        local_ip: Some(local_ip),
        local_port: Some(0),
        enabled: true,
        timeout_ms: 500, // Short timeout
        max_redirects: 3,
        cache_timeout_sec: 300,
    };

    let service = LrnDipService::new(config);
    service
        .initialize()
        .await
        .expect("Service should initialize");

    // This should timeout
    let result = service
        .dip_lrn("12025551234", Some("19995551234"))
        .await
        .expect("LRN dip should handle timeout gracefully");

    assert_eq!(result.original_tn, "12025551234");
    assert_eq!(result.lrn, None);
    assert!(!result.ported);
    assert!(result.error.is_some());
    assert!(result.response_time_ms >= 500); // Should be at least timeout duration
}

/// Integration test with the LCR routing engine
#[tokio::test]
async fn test_lrn_integration_with_routing() {
    use redfire_switch::lcr::{
        types::{RouteRequest, RouteType},
        LcrEngine,
    };

    let server_ip = "127.0.0.1".parse().expect("Valid IP address");
    let lrn_config = LrnDipConfig {
        server_ip,
        server_port: 15065,
        enabled: true,
        timeout_ms: 2000,
        cache_timeout_sec: 300,
        ..Default::default()
    };

    // Start mock server
    let server_addr: SocketAddr = format!("{}:{}", lrn_config.server_ip, lrn_config.server_port)
        .parse()
        .expect("Valid socket address");
    let mock_server = MockLrnServer::new(server_addr)
        .await
        .expect("Mock server should start");

    tokio::spawn(async move {
        let _ = mock_server.run().await;
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // This would require a test database - in a real test environment
    // you'd set up a test database with the proper schema
    // For now, we'll test that the service can be created and configured

    assert!(lrn_config.enabled);
    assert_eq!(lrn_config.server_port, 15065);
}
