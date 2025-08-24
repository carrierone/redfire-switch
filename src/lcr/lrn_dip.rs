use anyhow::{anyhow, Result};
use chrono::Utc;
use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::lcr::types::{LrnDipConfig, LrnDipServer, LrnDipRequest, LrnDipResponse, LrnCacheEntry, SipRedirectResponse};

/// LRN Dip Service using SIP 302 redirects
pub struct LrnDipService {
    config: LrnDipConfig,
    cache: Arc<DashMap<String, LrnCacheEntry>>,
    active_requests: Arc<DashMap<String, LrnDipRequest>>,
    client_socket: Arc<RwLock<Option<Arc<UdpSocket>>>>,
    round_robin_index: Arc<std::sync::atomic::AtomicUsize>,
}

impl LrnDipService {
    pub fn new(config: LrnDipConfig) -> Self {
        Self {
            config,
            cache: Arc::new(DashMap::new()),
            active_requests: Arc::new(DashMap::new()),
            client_socket: Arc::new(RwLock::new(None)),
            round_robin_index: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Initialize the SIP client socket
    pub async fn initialize(&self) -> Result<()> {
        if !self.config.enabled {
            info!("LRN dip service is disabled");
            return Ok(());
        }

        let local_addr = SocketAddr::new(
            self.config.local_ip.unwrap_or_else(|| "0.0.0.0".parse().unwrap()),
            self.config.local_port.unwrap_or(0),
        );

        let socket = UdpSocket::bind(local_addr).await?;
        let actual_addr = socket.local_addr()?;
        
        info!("LRN dip service initialized on {}", actual_addr);
        
        let mut client_socket = self.client_socket.write().await;
        *client_socket = Some(Arc::new(socket));
        
        Ok(())
    }

    /// Perform LRN dip for a telephone number
    pub async fn dip_lrn(&self, tn: &str, ani: Option<&str>) -> Result<LrnDipResponse> {
        if !self.config.enabled {
            return Ok(LrnDipResponse {
                original_tn: tn.to_string(),
                lrn: None,
                ported: false,
                spid: None,
                response_time_ms: 0,
                redirect_count: 0,
                error: Some("LRN dip service disabled".to_string()),
            });
        }

        let start_time = Instant::now();

        // Check cache first using ANI/DNIS pair
        if let Some(cached_entry) = self.get_cached_lrn(tn, ani) {
            if cached_entry.expires_at > Utc::now() {
                debug!("LRN cache hit for ANI/DNIS {}:{} -> {}", 
                       ani.unwrap_or("anonymous"), tn, cached_entry.lrn);
                return Ok(LrnDipResponse {
                    original_tn: tn.to_string(),
                    lrn: Some(cached_entry.lrn),
                    ported: cached_entry.ported,
                    spid: cached_entry.spid,
                    response_time_ms: start_time.elapsed().as_millis() as u64,
                    redirect_count: 0,
                    error: None,
                });
            } else {
                // Remove expired entry
                let cache_key = self.generate_cache_key(tn, ani);
                self.cache.remove(&cache_key);
            }
        }

        // Perform SIP dip
        let request_id = Uuid::new_v4().to_string();
        let dip_request = LrnDipRequest {
            tn: tn.to_string(),
            ani: ani.map(|s| s.to_string()),
            request_id: request_id.clone(),
        };

        self.active_requests.insert(request_id.clone(), dip_request);

        let result = self.perform_sip_dip(tn, ani, &request_id).await;
        self.active_requests.remove(&request_id);

        let elapsed = start_time.elapsed().as_millis() as u64;

        match result {
            Ok((response, redirect_count)) => {
                let is_ported = response.lrn.is_some();
                
                // Cache the result if successful
                if let Some(ref lrn) = response.lrn {
                    self.cache_lrn_result(tn, ani, lrn, response.spid.as_deref(), true);
                }
                
                Ok(LrnDipResponse {
                    original_tn: tn.to_string(),
                    lrn: response.lrn,
                    ported: is_ported,
                    spid: response.spid,
                    response_time_ms: elapsed,
                    redirect_count,
                    error: None,
                })
            }
            Err(e) => {
                warn!("LRN dip failed for {}: {}", tn, e);
                Ok(LrnDipResponse {
                    original_tn: tn.to_string(),
                    lrn: None,
                    ported: false,
                    spid: None,
                    response_time_ms: elapsed,
                    redirect_count: 0,
                    error: Some(e.to_string()),
                })
            }
        }
    }

    /// Get next server to use based on load balancing strategy
    fn get_next_server(&self) -> Option<LrnDipServer> {
        let servers = self.config.get_servers();
        if servers.is_empty() {
            return None;
        }

        if self.config.load_balancing == "round_robin" {
            let index = self.round_robin_index.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Some(servers[index % servers.len()].clone())
        } else {
            // Priority/failover: always start with first server (highest priority)
            Some(servers[0].clone())
        }
    }

    /// Try LRN dip with multiple servers (failover/round-robin)
    async fn perform_sip_dip(&self, tn: &str, ani: Option<&str>, request_id: &str) -> Result<(SipRedirectResponse, u8)> {
        let servers = self.config.get_servers();
        if servers.is_empty() {
            return Err(anyhow!("No LRN servers configured"));
        }

        let socket_guard = self.client_socket.read().await;
        let socket = socket_guard
            .as_ref()
            .ok_or_else(|| anyhow!("SIP client not initialized"))?;

        let local_addr = socket.local_addr()?;

        // Try servers in order (priority) or use round-robin selection
        let start_server = if self.config.load_balancing == "round_robin" {
            self.get_next_server()
        } else {
            servers.first().cloned()
        };

        if let Some(primary_server) = start_server {
            // Try primary server first
            match self.try_server(&primary_server, tn, ani, request_id, socket, &local_addr).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    warn!("Primary LRN server {}:{} failed: {}", 
                          primary_server.server_ip, primary_server.server_port, e);
                    
                    // For round-robin, don't try other servers
                    if self.config.load_balancing == "round_robin" {
                        return Err(e);
                    }
                }
            }
        }

        // For priority/failover mode, try remaining servers
        if self.config.load_balancing == "priority" {
            for server in servers.iter().skip(1) {
                debug!("Trying backup LRN server {}:{}", server.server_ip, server.server_port);
                match self.try_server(server, tn, ani, request_id, socket, &local_addr).await {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        warn!("Backup LRN server {}:{} failed: {}", 
                              server.server_ip, server.server_port, e);
                    }
                }
            }
        }

        Err(anyhow!("All LRN servers failed"))
    }

    /// Try a single LRN server
    async fn try_server(
        &self,
        server: &LrnDipServer,
        tn: &str,
        ani: Option<&str>,
        request_id: &str,
        socket: &UdpSocket,
        local_addr: &SocketAddr,
    ) -> Result<(SipRedirectResponse, u8)> {
        let server_addr = SocketAddr::new(server.server_ip, server.server_port);

        // Create SIP OPTIONS request for LRN dip
        let sip_message = self.create_lrn_options_request(tn, ani, request_id, local_addr, &server_addr)?;
        
        debug!("Sending LRN dip request for {} to {}", tn, server_addr);

        // Send request to this server
        socket.send_to(sip_message.as_bytes(), server_addr).await?;

        // Use backup timeout for backup servers, full timeout for primary
        let response_timeout = Duration::from_millis(
            if server.priority == 0 {
                self.config.timeout_ms as u64
            } else {
                self.config.get_backup_timeout_ms() as u64
            }
        );
        
        let response = timeout(response_timeout, self.receive_sip_response(socket, request_id)).await??;

        let mut redirect_count = 0;
        
        // Handle 302 redirect
        if self.is_302_redirect(&response) {
            redirect_count = 1;
            let redirect_result = self.handle_302_redirect(&response, tn, request_id).await?;
            Ok((redirect_result, redirect_count))
        } else {
            // Parse direct response
            let direct_result = self.parse_lrn_response(&response)?;
            Ok((direct_result, redirect_count))
        }
    }

    /// Create SIP OPTIONS request for LRN dip
    fn create_lrn_options_request(
        &self,
        tn: &str,
        ani: Option<&str>,
        request_id: &str,
        local_addr: &SocketAddr,
        server_addr: &SocketAddr,
    ) -> Result<String> {
        let normalized_tn = self.normalize_tn(tn);
        let normalized_ani = ani.map(|a| self.normalize_tn(a)).unwrap_or_else(|| "anonymous".to_string());
        let call_id = format!("lrn-dip-{}", request_id);

        // Create SIP OPTIONS request with ANI in From header
        let sip_request = format!(
            "OPTIONS sip:{}@{} SIP/2.0\r\n\
            Via: SIP/2.0/UDP {};branch=z9hG4bK{}\r\n\
            Max-Forwards: 70\r\n\
            From: <sip:{}@{}>;tag={}\r\n\
            To: <sip:{}@{}>\r\n\
            Call-ID: {}\r\n\
            CSeq: 1 OPTIONS\r\n\
            Contact: <sip:{}@{}>\r\n\
            User-Agent: Redfire-Switch/1.0 LRN-Dip\r\n\
            Accept: application/sdp\r\n\
            Content-Length: 0\r\n\
            \r\n",
            normalized_tn,
            server_addr.ip(),
            local_addr,
            self.generate_branch(),
            normalized_ani,
            local_addr.ip(),
            self.generate_tag(),
            normalized_tn,
            server_addr.ip(),
            call_id,
            normalized_ani,
            local_addr.ip()
        );

        Ok(sip_request)
    }

    /// Receive SIP response
    async fn receive_sip_response(&self, socket: &UdpSocket, request_id: &str) -> Result<String> {
        let mut buffer = [0u8; 4096];
        let mut redirect_count = 0;

        loop {
            let (len, _) = socket.recv_from(&mut buffer).await?;
            let response = String::from_utf8_lossy(&buffer[..len]).to_string();
            
            debug!("Received SIP response for {}: {}", request_id, response.lines().next().unwrap_or(""));

            // Check if this response is for our request
            if response.contains(request_id) || response.contains("lrn-dip") {
                return Ok(response);
            }

            // Handle redirects up to max limit
            if self.is_302_redirect(&response) && redirect_count < self.config.max_redirects {
                redirect_count += 1;
                continue;
            }

            // If not our response, continue listening
            if !response.contains(&format!("lrn-dip-{}", request_id)) {
                continue;
            }

            return Ok(response);
        }
    }

    /// Check if response is 302 redirect
    fn is_302_redirect(&self, response: &str) -> bool {
        response.starts_with("SIP/2.0 302") || response.contains("302 Moved Temporarily")
    }

    /// Handle SIP 302 redirect for LRN
    async fn handle_302_redirect(&self, response: &str, tn: &str, _request_id: &str) -> Result<SipRedirectResponse> {
        debug!("Handling 302 redirect for LRN dip: {}", tn);

        // Parse Contact header for redirect URI
        let contact_uri = self.extract_contact_uri(response)?;
        
        // Parse LRN from Contact URI or other headers
        let lrn = self.extract_lrn_from_contact(&contact_uri);
        let spid = self.extract_spid_from_headers(response);

        debug!("302 redirect parsed - LRN: {:?}, SPID: {:?}", lrn, spid);

        Ok(SipRedirectResponse {
            contact_uri,
            lrn,
            spid,
        })
    }

    /// Parse LRN response from SIP message
    fn parse_lrn_response(&self, response: &str) -> Result<SipRedirectResponse> {
        if response.starts_with("SIP/2.0 200") {
            // Parse successful response for LRN info
            let lrn = self.extract_lrn_from_headers(response);
            let spid = self.extract_spid_from_headers(response);
            
            Ok(SipRedirectResponse {
                contact_uri: "".to_string(),
                lrn,
                spid,
            })
        } else {
            Err(anyhow!("Non-200 response: {}", response.lines().next().unwrap_or("")))
        }
    }

    /// Extract Contact URI from 302 response
    fn extract_contact_uri(&self, response: &str) -> Result<String> {
        for line in response.lines() {
            if line.to_lowercase().starts_with("contact:") {
                let contact_line = line.trim_start_matches(|c: char| c.is_ascii_alphabetic() || c == ':' || c.is_whitespace());
                
                // Extract URI from angle brackets if present
                if let Some(start) = contact_line.find('<') {
                    if let Some(end) = contact_line.find('>') {
                        return Ok(contact_line[start + 1..end].to_string());
                    }
                }
                
                // Otherwise take the whole value
                return Ok(contact_line.split(';').next().unwrap_or(contact_line).trim().to_string());
            }
        }
        
        Err(anyhow!("No Contact header found in 302 response"))
    }

    /// Extract LRN from Contact URI (common format: sip:lrn@host)
    fn extract_lrn_from_contact(&self, contact_uri: &str) -> Option<String> {
        // Parse SIP URI to extract user part as potential LRN
        if contact_uri.starts_with("sip:") {
            let uri_part = contact_uri.trim_start_matches("sip:");
            if let Some(at_pos) = uri_part.find('@') {
                let user_part = &uri_part[..at_pos];
                
                // Validate as potential phone number/LRN
                if user_part.len() >= 10 && user_part.chars().all(|c| c.is_ascii_digit()) {
                    return Some(self.normalize_tn(user_part));
                }
            }
        }
        
        None
    }

    /// Extract LRN from custom SIP headers
    fn extract_lrn_from_headers(&self, response: &str) -> Option<String> {
        // Look for custom LRN headers (X-LRN, P-LRN, etc.)
        for line in response.lines() {
            let line_lower = line.to_lowercase();
            if line_lower.starts_with("x-lrn:") || line_lower.starts_with("p-lrn:") {
                let value = line.split(':').nth(1)?.trim();
                if value.len() >= 10 && value.chars().all(|c| c.is_ascii_digit() || c == '+') {
                    return Some(self.normalize_tn(value));
                }
            }
        }
        
        None
    }

    /// Extract SPID from SIP headers
    fn extract_spid_from_headers(&self, response: &str) -> Option<String> {
        for line in response.lines() {
            let line_lower = line.to_lowercase();
            if line_lower.starts_with("x-spid:") || line_lower.starts_with("p-spid:") {
                return line.split(':').nth(1).map(|s| s.trim().to_string());
            }
        }
        
        None
    }

    /// Get cached LRN entry using ANI/DNIS pair
    fn get_cached_lrn(&self, tn: &str, ani: Option<&str>) -> Option<LrnCacheEntry> {
        let cache_key = self.generate_cache_key(tn, ani);
        self.cache.get(&cache_key).map(|entry| entry.clone())
    }

    /// Cache LRN result with ANI/DNIS pair
    fn cache_lrn_result(&self, tn: &str, ani: Option<&str>, lrn: &str, spid: Option<&str>, ported: bool) {
        let now = Utc::now();
        let expires_at = now + chrono::Duration::seconds(self.config.cache_timeout_sec as i64);
        let cache_key = self.generate_cache_key(tn, ani);
        
        let cache_entry = LrnCacheEntry {
            tn: tn.to_string(),
            lrn: lrn.to_string(),
            spid: spid.map(|s| s.to_string()),
            ocn: None,
            lata: None,
            state: None,
            jurisdiction: None,
            cached_at: now,
            expires_at,
            ported,
            dip_response_time_ms: None,
        };
        
        self.cache.insert(cache_key.clone(), cache_entry);
        debug!("Cached LRN result for ANI/DNIS {}:{} -> {}", 
               ani.unwrap_or("anonymous"), tn, lrn);
    }

    /// Normalize telephone number for LRN dips
    fn normalize_tn(&self, tn: &str) -> String {
        let digits: String = tn.chars().filter(|c| c.is_ascii_digit()).collect();
        
        // Convert to 11-digit format (1NPANXXNNNN)
        if digits.len() == 10 {
            format!("1{}", digits)
        } else if digits.len() == 11 && digits.starts_with('1') {
            digits
        } else {
            digits
        }
    }

    /// Generate SIP branch parameter
    fn generate_branch(&self) -> String {
        format!("lrndip{}", Uuid::new_v4().simple())
    }

    /// Generate SIP tag parameter
    fn generate_tag(&self) -> String {
        format!("tag{}", Uuid::new_v4().simple().to_string()[..8].to_string())
    }

    /// Generate cache key for ANI/DNIS pair
    fn generate_cache_key(&self, tn: &str, ani: Option<&str>) -> String {
        match ani {
            Some(ani) => format!("{}:{}", self.normalize_tn(ani), self.normalize_tn(tn)),
            None => format!("anonymous:{}", self.normalize_tn(tn)),
        }
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> (usize, usize) {
        let total = self.cache.len();
        let now = Utc::now();
        let expired = self.cache
            .iter()
            .filter(|entry| entry.value().expires_at <= now)
            .count();
        
        (total, expired)
    }

    /// Clear expired cache entries
    pub fn cleanup_cache(&self) {
        let now = Utc::now();
        let expired_keys: Vec<String> = self.cache
            .iter()
            .filter(|entry| entry.value().expires_at <= now)
            .map(|entry| entry.key().clone())
            .collect();
        
        let expired_count = expired_keys.len();
        
        for key in expired_keys {
            self.cache.remove(&key);
        }
        
        debug!("Cleaned up {} expired LRN cache entries", expired_count);
    }

    /// Check if service is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Update configuration
    pub async fn update_config(&self, new_config: LrnDipConfig) -> Result<()> {
        let old_enabled = self.config.enabled;
        let old_local_ip = self.config.local_ip;
        let old_local_port = self.config.local_port;
        
        // If enabling service or changing network config, reinitialize
        if new_config.enabled && (!old_enabled || 
            new_config.local_ip != old_local_ip || 
            new_config.local_port != old_local_port) {
            
            let mut socket_guard = self.client_socket.write().await;
            *socket_guard = None;
            drop(socket_guard);
            
            // Update config in a new service instance would be needed,
            // but since we can't mutate self, we'll return an error instead
            return Err(anyhow!("Configuration update requires service restart"));
        }
        
        info!("LRN dip configuration validated (restart required for network changes)");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_lrn_dip_service_creation() {
        let config = LrnDipConfig::default();
        let service = LrnDipService::new(config);
        
        assert!(!service.is_enabled());
    }
    
    #[test]
    fn test_tn_normalization() {
        let config = LrnDipConfig::default();
        let service = LrnDipService::new(config);
        
        assert_eq!(service.normalize_tn("2125551234"), "12125551234");
        assert_eq!(service.normalize_tn("12125551234"), "12125551234");
        assert_eq!(service.normalize_tn("+1-212-555-1234"), "12125551234");
    }
    
    #[test]
    fn test_contact_uri_parsing() {
        let config = LrnDipConfig::default();
        let service = LrnDipService::new(config);
        
        let response_302 = "SIP/2.0 302 Moved Temporarily\r\n\
                           Contact: <sip:12025551234@lrn.example.com>\r\n\
                           Content-Length: 0\r\n";
        
        let contact_uri = service.extract_contact_uri(response_302).unwrap();
        assert_eq!(contact_uri, "sip:12025551234@lrn.example.com");
        
        let lrn = service.extract_lrn_from_contact(&contact_uri);
        assert_eq!(lrn, Some("12025551234".to_string()));
    }
}