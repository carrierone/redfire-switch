use crate::config::{Config, Protocol, SipProfile};
use crate::stir_shaken::StirShakenService;
use crate::termination_routing::TerminationRoutingService;
use crate::origination_routing::OriginationRoutingService;
use crate::cdr::CdrService;
use anyhow::{Result, anyhow};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use tokio::net::{UdpSocket, TcpListener};
use tracing::{info, warn, error, debug};

// Import the SIP core engine
use redfire_sip_stack::{SipCoreEngine, SipCoreConfig, TransportMessage, SipRequestResult};

pub struct SipServer {
    config: Arc<Config>,
    stir_shaken: Option<Arc<StirShakenService>>,
    termination_routing: Option<Arc<TerminationRoutingService>>,
    origination_routing: Option<Arc<OriginationRoutingService>>,
    cdr_service: Option<Arc<CdrService>>,
    sip_core: Arc<SipCoreEngine>,
}

impl SipServer {
    pub async fn new(config: Config) -> Result<Self> {
        // Initialize STIR/SHAKEN service if enabled
        let stir_shaken = if config.stir_shaken.enabled {
            match StirShakenService::new(config.stir_shaken.clone()) {
                Ok(service) => {
                    info!("STIR/SHAKEN service initialized");
                    Some(Arc::new(service))
                }
                Err(e) => {
                    error!("Failed to initialize STIR/SHAKEN service: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Initialize CDR service
        let cdr_service = match CdrService::new(config.cdr.clone()).await {
            Ok(service) => {
                info!("CDR service initialized");
                Some(Arc::new(service))
            }
            Err(e) => {
                error!("Failed to initialize CDR service: {}", e);
                None
            }
        };

        // Initialize termination routing service
        let termination_routing = if !config.termination_routing.is_empty() {
            let service = TerminationRoutingService::new(config.termination_routing.clone());
            info!("Termination routing service initialized with {} plans", config.termination_routing.len());
            Some(Arc::new(service))
        } else {
            None
        };

        // Initialize origination routing service
        let origination_routing = if config.origination_routing.enabled {
            match OriginationRoutingService::new(config.origination_routing.clone()).await {
                Ok(service) => {
                    info!("Origination routing service initialized");
                    Some(Arc::new(service))
                }
                Err(e) => {
                    error!("Failed to initialize origination routing service: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Initialize SIP core engine
        let sip_core_config = SipCoreConfig {
            bind_addresses: config.sip_profiles.iter()
                .map(|p| format!("{}:{}", p.bind_ip, p.port))
                .collect(),
            enable_authentication: true,
            enable_tls: config.sip_profiles.iter().any(|p| matches!(p.protocol, Protocol::Tls | Protocol::Dtls)),
            max_concurrent_calls: config.max_concurrent_calls.unwrap_or(10000),
            call_timeout_seconds: 300,
        };
        
        let sip_core = SipCoreEngine::new(sip_core_config).await
            .map_err(|e| anyhow!("Failed to initialize SIP core engine: {}", e))?;
        
        info!("SIP core engine initialized");

        Ok(Self {
            config: Arc::new(config),
            stir_shaken,
            termination_routing,
            origination_routing,
            cdr_service,
            sip_core: Arc::new(sip_core),
        })
    }

    pub async fn start(self: Arc<Self>) -> Result<()> {
        info!("Starting Redfire Switch SIP Server");
        
        let mut tasks = Vec::new();

        for profile in &self.config.sip_profiles {
            let profile_clone = profile.clone();
            let server_clone = self.clone();
            
            let task = tokio::spawn(async move {
                if let Err(e) = server_clone.start_profile(profile_clone).await {
                    error!("Failed to start SIP profile: {}", e);
                }
            });
            
            tasks.push(task);
        }

        // Wait for all tasks to complete
        for task in tasks {
            if let Err(e) = task.await {
                error!("SIP profile task failed: {}", e);
            }
        }

        Ok(())
    }

    async fn start_profile(self: Arc<Self>, profile: SipProfile) -> Result<()> {
        let ipv4_addr = SocketAddr::new(profile.bind_ip, profile.port);
        
        info!(
            "Starting SIP profile '{}' on {}:{} ({})",
            profile.name,
            profile.bind_ip,
            profile.port,
            match profile.protocol {
                Protocol::Udp => "UDP",
                Protocol::Tcp => "TCP",
                Protocol::Tls => "TLS",
                Protocol::Dtls => "DTLS",
            }
        );

        // Start IPv4 listener
        let server_clone = self.clone();
        let profile_clone = profile.clone();
        let ipv4_task = tokio::spawn(async move {
            match profile_clone.protocol {
                Protocol::Udp => server_clone.handle_udp(profile_clone.clone(), ipv4_addr, false).await,
                Protocol::Tcp => server_clone.handle_tcp(profile_clone.clone(), ipv4_addr, false).await,
                Protocol::Tls => server_clone.handle_tcp_tls(profile_clone.clone(), ipv4_addr, false).await,
                Protocol::Dtls => server_clone.handle_udp_dtls(profile_clone.clone(), ipv4_addr, false).await,
            }
        });

        // Start IPv6 listener if dual-stack is enabled
        let ipv6_task = if profile.dual_stack && profile.bind_ipv6.is_some() {
            let ipv6_ip = profile.bind_ipv6.ok_or_else(|| anyhow!("IPv6 bind address required for dual-stack"))?;
            let ipv6_port = profile.ipv6_port.unwrap_or(profile.port);
            let ipv6_addr = SocketAddr::new(ipv6_ip, ipv6_port);
            
            info!(
                "Starting IPv6 SIP profile '{}' on [{}]:{} ({})",
                profile.name,
                ipv6_ip,
                ipv6_port,
                match profile.protocol {
                    Protocol::Udp => "UDP",
                    Protocol::Tcp => "TCP",
                    Protocol::Tls => "TLS",
                    Protocol::Dtls => "DTLS",
                }
            );
            
            let server_clone = self.clone();
            let profile_clone = profile.clone();
            Some(tokio::spawn(async move {
                match profile_clone.protocol {
                    Protocol::Udp => server_clone.handle_udp(profile_clone.clone(), ipv6_addr, true).await,
                    Protocol::Tcp => server_clone.handle_tcp(profile_clone.clone(), ipv6_addr, true).await,
                    Protocol::Tls => server_clone.handle_tcp_tls(profile_clone.clone(), ipv6_addr, true).await,
                    Protocol::Dtls => server_clone.handle_udp_dtls(profile_clone.clone(), ipv6_addr, true).await,
                }
            }))
        } else {
            None
        };

        // Wait for both tasks
        let ipv4_result = ipv4_task.await?;
        if let Some(ipv6_task) = ipv6_task {
            let ipv6_result = ipv6_task.await?;
            // Return first error if any
            if let Err(e) = ipv4_result {
                return Err(e);
            }
            ipv6_result
        } else {
            ipv4_result
        }
    }

    async fn handle_udp(self: Arc<Self>, profile: SipProfile, bind_addr: SocketAddr, is_ipv6: bool) -> Result<()> {
        let socket = UdpSocket::bind(bind_addr).await?;
        let protocol_info = if is_ipv6 { "UDP/IPv6" } else { "UDP/IPv4" };
        info!("{} SIP server listening on {}", protocol_info, bind_addr);

        let mut buf = [0; 4096];
        
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    let data = &buf[..len];
                    
                    if !Self::is_ip_authorized(&profile, addr.ip()) {
                        warn!("Unauthorized SIP request from {}", addr.ip());
                        continue;
                    }

                    debug!("Received {} bytes from {}", len, addr);
                    
                    if let Err(e) = self.handle_sip_message(data, addr, &socket, &profile).await {
                        error!("Error handling SIP message: {}", e);
                    }
                }
                Err(e) => {
                    error!("UDP receive error: {}", e);
                }
            }
        }
    }

    async fn handle_tcp(self: Arc<Self>, profile: SipProfile, bind_addr: SocketAddr, is_ipv6: bool) -> Result<()> {
        let listener = TcpListener::bind(bind_addr).await?;
        let protocol_info = if is_ipv6 { "TCP/IPv6" } else { "TCP/IPv4" };
        info!("{} SIP server listening on {}", protocol_info, bind_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    if !Self::is_ip_authorized(&profile, addr.ip()) {
                        warn!("Unauthorized TCP connection from {}", addr.ip());
                        continue;
                    }

                    debug!("Accepted TCP connection from {}", addr);
                    
                    let profile_clone = profile.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_tcp_connection(stream, addr, profile_clone).await {
                            error!("TCP connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("TCP accept error: {}", e);
                }
            }
        }
    }

    async fn handle_tcp_connection(
        _stream: tokio::net::TcpStream,
        addr: SocketAddr,
        _profile: SipProfile,
    ) -> Result<()> {
        debug!("Handling TCP connection from {}", addr);
        // TODO: Implement TCP SIP message handling
        Ok(())
    }

    async fn handle_tcp_tls(self: Arc<Self>, profile: SipProfile, bind_addr: SocketAddr, is_ipv6: bool) -> Result<()> {
        let listener = TcpListener::bind(bind_addr).await?;
        let protocol_info = if is_ipv6 { "TLS/IPv6" } else { "TLS/IPv4" };
        info!("{} SIP server listening on {}", protocol_info, bind_addr);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    if !Self::is_ip_authorized(&profile, addr.ip()) {
                        warn!("Unauthorized TLS connection from {}", addr.ip());
                        continue;
                    }

                    debug!("Accepted TLS connection from {}", addr);
                    
                    let profile_clone = profile.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_tls_connection(stream, addr, profile_clone).await {
                            error!("TLS connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("TLS accept error: {}", e);
                }
            }
        }
    }

    async fn handle_tls_connection(
        _stream: tokio::net::TcpStream,
        addr: SocketAddr,
        _profile: SipProfile,
    ) -> Result<()> {
        debug!("TLS connection from {}: [TODO: TLS SIP message handling]", addr);
        Ok(())
    }

    async fn handle_udp_dtls(self: Arc<Self>, profile: SipProfile, bind_addr: SocketAddr, is_ipv6: bool) -> Result<()> {
        let socket = UdpSocket::bind(bind_addr).await?;
        let protocol_info = if is_ipv6 { "DTLS/IPv6" } else { "DTLS/IPv4" };
        info!("{} SIP server listening on {}", protocol_info, bind_addr);

        let mut buf = [0; 4096];
        
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    let data = &buf[..len];
                    
                    if !Self::is_ip_authorized(&profile, addr.ip()) {
                        warn!("Unauthorized DTLS request from {}", addr.ip());
                        continue;
                    }

                    debug!("Received DTLS {} bytes from {}", len, addr);
                    debug!("DTLS data from {}: [TODO: DTLS SIP message handling]", addr);
                }
                Err(e) => {
                    error!("DTLS recv error: {}", e);
                }
            }
        }
    }

    async fn handle_sip_message(
        &self,
        data: &[u8],
        addr: SocketAddr,
        socket: &UdpSocket,
        _profile: &SipProfile,
    ) -> Result<()> {
        let message_str = String::from_utf8_lossy(data);
        debug!("SIP message from {}: {}", addr, message_str);

        // Basic SIP message parsing (simplified for now)
        if message_str.starts_with("SIP/2.0") {
            info!("Received SIP response from {}", addr);
            // Don't respond to responses
        } else if Self::is_sip_request(&message_str) {
            let method = Self::extract_method(&message_str);
            info!("Received SIP {} request from {}", method, addr);
            
            // Handle STIR/SHAKEN verification for INVITE requests
            let mut response = if method == "INVITE" {
                self.handle_invite_request(&message_str).await?
            } else {
                Self::create_simple_response(&message_str)?
            };

            // Add STIR/SHAKEN Identity header for outgoing calls if enabled
            if method == "INVITE" && self.stir_shaken.is_some() {
                if let Some(identity_header) = self.create_identity_header_for_call(&message_str).await {
                    response = self.add_identity_header_to_response(response, identity_header);
                }
            }
            
            socket.send_to(response.as_bytes(), addr).await?;
            
            debug!("Sent response to {}", addr);
        } else {
            warn!("Received invalid SIP message from {}", addr);
        }

        Ok(())
    }

    fn is_sip_request(message: &str) -> bool {
        let methods = ["INVITE", "ACK", "BYE", "CANCEL", "OPTIONS", "REGISTER", "PRACK", "SUBSCRIBE", "NOTIFY", "PUBLISH", "INFO", "REFER", "MESSAGE", "UPDATE"];
        methods.iter().any(|method| message.starts_with(method))
    }

    fn extract_method(message: &str) -> String {
        message.lines()
            .next()
            .and_then(|line| line.split_whitespace().next())
            .unwrap_or("UNKNOWN")
            .to_string()
    }
    
    // New improved SIP method handlers
    
    fn extract_sip_method(&self, message: &str) -> Option<String> {
        message.lines()
            .next()
            .and_then(|line| line.split_whitespace().next())
            .map(|s| s.to_string())
    }
    
    async fn handle_register_request(
        &self,
        message: &str,
        addr: SocketAddr,
        socket: &UdpSocket,
    ) -> Result<()> {
        info!("Processing REGISTER request from {}", addr);
        
        // Extract Contact header and other registration details
        let call_id = self.extract_header(message, "Call-ID").unwrap_or_default();
        let from = self.extract_header(message, "From").unwrap_or_default();
        let expires = self.extract_header(message, "Expires").unwrap_or("3600".to_string());
        
        // For now, accept all registrations (in production, check authentication)
        let response = format!(
            "SIP/2.0 200 OK\r\n\
             Call-ID: {}\r\n\
             From: {}\r\n\
             To: {}\r\n\
             CSeq: 1 REGISTER\r\n\
             Expires: {}\r\n\
             Contact: <sip:{}>\r\n\
             Content-Length: 0\r\n\r\n",
            call_id, from, from, expires, addr
        );
        
        socket.send_to(response.as_bytes(), addr).await?;
        info!("Registration accepted for {}", from);
        Ok(())
    }
    
    async fn handle_invite_request(
        &self,
        message: &str,
        addr: SocketAddr,
        socket: &UdpSocket,
    ) -> Result<()> {
        info!("Processing INVITE request from {}", addr);
        
        let call_id = self.extract_header(message, "Call-ID").unwrap_or_default();
        let from = self.extract_header(message, "From").unwrap_or_default();
        let to = self.extract_header(message, "To").unwrap_or_default();
        
        // STIR/SHAKEN verification if enabled
        if let Some(stir_shaken) = &self.stir_shaken {
            if let Some(identity_header) = self.extract_header(message, "Identity") {
                match stir_shaken.verify_identity(&identity_header).await {
                    Ok(verification) => {
                        info!("STIR/SHAKEN verification successful: {:?}", verification);
                    }
                    Err(e) => {
                        warn!("STIR/SHAKEN verification failed: {}", e);
                        // Could reject call here based on policy
                    }
                }
            }
        }
        
        // For now, send 200 OK with basic SDP
        let response = format!(
            "SIP/2.0 200 OK\r\n\
             Call-ID: {}\r\n\
             From: {}\r\n\
             To: {}\r\n\
             CSeq: 1 INVITE\r\n\
             Content-Type: application/sdp\r\n\
             Content-Length: 200\r\n\r\n\
             v=0\r\n\
             o=redfire 123456 123456 IN IP4 {}\r\n\
             s=Redfire Session\r\n\
             c=IN IP4 {}\r\n\
             t=0 0\r\n\
             m=audio 8000 RTP/AVP 0 8\r\n\
             a=rtpmap:0 PCMU/8000\r\n\
             a=rtpmap:8 PCMA/8000\r\n",
            call_id, from, to, addr.ip(), addr.ip()
        );
        
        socket.send_to(response.as_bytes(), addr).await?;
        info!("Call established for {}", call_id);
        Ok(())
    }
    
    async fn handle_bye_request(
        &self,
        message: &str,
        addr: SocketAddr,
        socket: &UdpSocket,
    ) -> Result<()> {
        info!("Processing BYE request from {}", addr);
        
        let call_id = self.extract_header(message, "Call-ID").unwrap_or_default();
        let from = self.extract_header(message, "From").unwrap_or_default();
        let to = self.extract_header(message, "To").unwrap_or_default();
        
        // Send 200 OK to confirm call termination
        let response = format!(
            "SIP/2.0 200 OK\r\n\
             Call-ID: {}\r\n\
             From: {}\r\n\
             To: {}\r\n\
             CSeq: 1 BYE\r\n\
             Content-Length: 0\r\n\r\n",
            call_id, from, to
        );
        
        socket.send_to(response.as_bytes(), addr).await?;
        info!("Call terminated for {}", call_id);
        Ok(())
    }
    
    async fn handle_options_request(
        &self,
        message: &str,
        addr: SocketAddr,
        socket: &UdpSocket,
    ) -> Result<()> {
        debug!("Processing OPTIONS request from {}", addr);
        
        let call_id = self.extract_header(message, "Call-ID").unwrap_or_default();
        let from = self.extract_header(message, "From").unwrap_or_default();
        let to = self.extract_header(message, "To").unwrap_or_default();
        
        // Send capabilities response
        let response = format!(
            "SIP/2.0 200 OK\r\n\
             Call-ID: {}\r\n\
             From: {}\r\n\
             To: {}\r\n\
             CSeq: 1 OPTIONS\r\n\
             Allow: INVITE,ACK,CANCEL,BYE,OPTIONS,REGISTER,INFO\r\n\
             Accept: application/sdp\r\n\
             Accept-Encoding: identity\r\n\
             Content-Length: 0\r\n\r\n",
            call_id, from, to
        );
        
        socket.send_to(response.as_bytes(), addr).await?;
        Ok(())
    }
    
    fn extract_header(&self, message: &str, header_name: &str) -> Option<String> {
        for line in message.lines() {
            if line.to_lowercase().starts_with(&format!("{}:", header_name.to_lowercase())) {
                return Some(line.split(':').nth(1)?.trim().to_string());
            }
        }
        None
    }

    fn extract_header(message: &str, header_name: &str) -> String {
        for line in message.lines() {
            if line.to_lowercase().starts_with(&header_name.to_lowercase()) {
                if let Some(value) = line.split(':').nth(1) {
                    return value.trim().to_string();
                }
            }
        }
        "unknown".to_string()
    }

    fn create_simple_response(request_message: &str) -> Result<String> {
        let method = Self::extract_method(request_message);
        
        // Extract headers from the request
        let call_id = Self::extract_header(request_message, "Call-ID");
        let from = Self::extract_header(request_message, "From");
        let to = Self::extract_header(request_message, "To");
        let cseq = Self::extract_header(request_message, "CSeq");
        let via = Self::extract_header(request_message, "Via");
        let contact = Self::extract_header(request_message, "Contact");

        let mut response = format!(
            "SIP/2.0 200 OK\r\n\
             Via: {}\r\n\
             From: {}\r\n\
             To: {}\r\n\
             Call-ID: {}\r\n\
             CSeq: {}\r\n\
             Server: Redfire-Switch/0.1.0\r\n",
            via, from, to, call_id, cseq
        );

        // Add specific headers for OPTIONS responses
        if method == "OPTIONS" {
            response.push_str("Allow: INVITE,ACK,BYE,CANCEL,OPTIONS,REGISTER,PRACK,SUBSCRIBE,NOTIFY,PUBLISH,INFO,REFER,MESSAGE,UPDATE\r\n");
            response.push_str("Accept: application/sdp,message/sipfrag\r\n");
            response.push_str("Accept-Language: en\r\n");
            response.push_str("Supported: replaces,timer,path\r\n");
            
            if contact != "unknown" {
                response.push_str(&format!("Contact: {}\r\n", contact));
            } else {
                response.push_str("Contact: <sip:redfire-switch@redfire-switch:5060>\r\n");
            }
        }

        response.push_str("Content-Length: 0\r\n\r\n");

        Ok(response)
    }

    fn is_ip_authorized(profile: &SipProfile, ip: IpAddr) -> bool {
        if profile.allowed_ips.is_empty() {
            return true; // Allow all if no restrictions
        }
        
        profile.allowed_ips.contains(&ip)
    }

    // STIR/SHAKEN helper methods
    async fn handle_invite_request(&self, message: &str) -> Result<String> {
        // Check for Identity header in incoming INVITE
        let identity_header = Self::extract_header(message, "Identity");
        
        if !identity_header.is_empty() && identity_header != "unknown" {
            // Verify STIR/SHAKEN if we have a service and verification is enabled
            if let Some(stir_shaken) = &self.stir_shaken {
                let from_number = self.extract_from_number(message);
                
                match stir_shaken.validate_call(&identity_header, &from_number).await {
                    Ok(attestation) => {
                        info!("STIR/SHAKEN validation successful for {} with attestation {:?}", 
                              from_number, attestation);
                    }
                    Err(e) => {
                        warn!("STIR/SHAKEN validation failed for {}: {}", from_number, e);
                        // Continue processing even if validation fails
                    }
                }
            }
        } else {
            debug!("No Identity header found in INVITE request");
        }

        // Create standard response for INVITE
        Self::create_simple_response(message)
    }

    async fn create_identity_header_for_call(&self, message: &str) -> Option<String> {
        if let Some(stir_shaken) = &self.stir_shaken {
            let from_number = self.extract_from_number(message);
            let to_number = self.extract_to_number(message);
            let call_id = Self::extract_header(message, "Call-ID");

            if !from_number.is_empty() && !to_number.is_empty() && !call_id.is_empty() {
                let call_info = stir_shaken.create_call_info(
                    from_number,
                    to_number,
                    call_id,
                    None, // Use default attestation
                );

                match stir_shaken.create_identity_header(&call_info) {
                    Ok(identity) => {
                        debug!("Created Identity header for outgoing call");
                        return Some(identity);
                    }
                    Err(e) => {
                        error!("Failed to create Identity header: {}", e);
                    }
                }
            }
        }
        None
    }

    fn add_identity_header_to_response(&self, mut response: String, identity: String) -> String {
        // Insert Identity header before Content-Length
        if let Some(content_length_pos) = response.find("Content-Length:") {
            response.insert_str(content_length_pos, &format!("Identity: {}\r\n", identity));
        }
        response
    }

    fn extract_from_number(&self, message: &str) -> String {
        let from_header = Self::extract_header(message, "From");
        if let Some(stir_shaken) = &self.stir_shaken {
            // Extract URI from From header: "From: <sip:+12345@example.com>"
            if let Some(start) = from_header.find('<') {
                if let Some(end) = from_header.find('>') {
                    let uri = &from_header[start + 1..end];
                    if let Some(number) = stir_shaken.extract_phone_number(uri) {
                        return number;
                    }
                }
            }
        }
        "unknown".to_string()
    }

    fn extract_to_number(&self, message: &str) -> String {
        let to_header = Self::extract_header(message, "To");
        if let Some(stir_shaken) = &self.stir_shaken {
            // Extract URI from To header: "To: <sip:+12345@example.com>"
            if let Some(start) = to_header.find('<') {
                if let Some(end) = to_header.find('>') {
                    let uri = &to_header[start + 1..end];
                    if let Some(number) = stir_shaken.extract_phone_number(uri) {
                        return number;
                    }
                }
            }
        }
        "unknown".to_string()
    }
}