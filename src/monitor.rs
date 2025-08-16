use crate::config::{Config, SipEndpoint, Protocol};
use anyhow::Result;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tokio::time::{interval, timeout};
use tracing::{info, warn, error, debug};

// Public function for one-time ping testing
pub async fn ping_endpoint_once(
    endpoint: &SipEndpoint,
    call_id_counter: Arc<RwLock<u32>>,
) -> Result<Duration> {
    let start_time = Instant::now();
    
    match endpoint.protocol {
        Protocol::Udp => SipMonitor::ping_udp(endpoint, call_id_counter).await?,
        Protocol::Tcp => SipMonitor::ping_tcp(endpoint, call_id_counter).await?,
    }
    
    Ok(start_time.elapsed())
}

#[derive(Debug, Clone)]
pub enum EndpointStatus {
    Unknown,
    Online,
    Offline,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct EndpointHealth {
    pub status: EndpointStatus,
    pub last_check: Instant,
    pub last_response_time: Option<Duration>,
    pub consecutive_failures: u32,
    pub total_pings: u64,
    pub successful_pings: u64,
}

impl Default for EndpointHealth {
    fn default() -> Self {
        EndpointHealth {
            status: EndpointStatus::Unknown,
            last_check: Instant::now(),
            last_response_time: None,
            consecutive_failures: 0,
            total_pings: 0,
            successful_pings: 0,
        }
    }
}

pub struct SipMonitor {
    config: Arc<Config>,
    endpoint_health: Arc<RwLock<HashMap<String, EndpointHealth>>>,
    call_id_counter: Arc<RwLock<u32>>,
}

impl SipMonitor {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            endpoint_health: Arc::new(RwLock::new(HashMap::new())),
            call_id_counter: Arc::new(RwLock::new(1)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        if !self.config.monitoring.enabled {
            info!("SIP monitoring is disabled");
            return Ok(());
        }

        info!("Starting SIP endpoint monitoring");
        
        let mut tasks = Vec::new();

        for endpoint in &self.config.monitoring.endpoints {
            if !endpoint.enabled {
                debug!("Skipping disabled endpoint: {}", endpoint.name);
                continue;
            }

            let endpoint_clone = endpoint.clone();
            let health_map = self.endpoint_health.clone();
            let call_id_counter = self.call_id_counter.clone();

            let task = tokio::spawn(async move {
                Self::monitor_endpoint(endpoint_clone, health_map, call_id_counter).await;
            });

            tasks.push(task);
        }

        // Wait for all monitoring tasks
        for task in tasks {
            if let Err(e) = task.await {
                error!("Monitoring task failed: {}", e);
            }
        }

        Ok(())
    }

    async fn monitor_endpoint(
        endpoint: SipEndpoint,
        health_map: Arc<RwLock<HashMap<String, EndpointHealth>>>,
        call_id_counter: Arc<RwLock<u32>>,
    ) {
        info!("Starting monitoring for endpoint: {} ({})", endpoint.name, endpoint.address);
        
        let mut interval = interval(Duration::from_secs(endpoint.ping_interval_seconds));
        
        loop {
            interval.tick().await;
            
            let start_time = Instant::now();
            let result = Self::ping_endpoint(&endpoint, call_id_counter.clone()).await;
            let response_time = start_time.elapsed();

            let mut health_guard = health_map.write().await;
            let health = health_guard.entry(endpoint.name.clone()).or_default();
            
            health.last_check = Instant::now();
            health.total_pings += 1;

            match result {
                Ok(_) => {
                    health.status = EndpointStatus::Online;
                    health.last_response_time = Some(response_time);
                    health.consecutive_failures = 0;
                    health.successful_pings += 1;
                    
                    debug!(
                        "OPTIONS ping to {} successful ({}ms)",
                        endpoint.name,
                        response_time.as_millis()
                    );
                }
                Err(e) => {
                    health.status = EndpointStatus::Error(e.to_string());
                    health.consecutive_failures += 1;
                    
                    warn!(
                        "OPTIONS ping to {} failed (attempt {}): {}",
                        endpoint.name,
                        health.consecutive_failures,
                        e
                    );

                    if health.consecutive_failures >= 3 {
                        health.status = EndpointStatus::Offline;
                        error!("Endpoint {} marked as offline after {} consecutive failures", 
                               endpoint.name, health.consecutive_failures);
                    }
                }
            }
        }
    }

    async fn ping_endpoint(
        endpoint: &SipEndpoint,
        call_id_counter: Arc<RwLock<u32>>,
    ) -> Result<()> {
        match endpoint.protocol {
            Protocol::Udp => Self::ping_udp(endpoint, call_id_counter).await,
            Protocol::Tcp => Self::ping_tcp(endpoint, call_id_counter).await,
        }
    }

    pub async fn ping_udp(
        endpoint: &SipEndpoint,
        call_id_counter: Arc<RwLock<u32>>,
    ) -> Result<()> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        
        let call_id = {
            let mut counter = call_id_counter.write().await;
            *counter += 1;
            *counter
        };

        let options_request = Self::create_options_request(endpoint.address, call_id)?;
        
        socket.send_to(options_request.as_bytes(), endpoint.address).await?;
        
        let mut buf = [0; 4096];
        
        let result = timeout(
            Duration::from_secs(endpoint.timeout_seconds),
            socket.recv_from(&mut buf)
        ).await;

        match result {
            Ok(Ok((len, _addr))) => {
                let response = String::from_utf8_lossy(&buf[..len]);
                
                if response.starts_with("SIP/2.0 200") {
                    debug!("Received 200 OK response from {}", endpoint.address);
                    Ok(())
                } else if response.starts_with("SIP/2.0") {
                    // Any SIP response is considered a successful ping
                    debug!("Received SIP response from {}: {}", endpoint.address, 
                           response.lines().next().unwrap_or(""));
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Invalid SIP response"))
                }
            }
            Ok(Err(e)) => Err(anyhow::anyhow!("Socket error: {}", e)),
            Err(_) => Err(anyhow::anyhow!("Ping timeout")),
        }
    }

    pub async fn ping_tcp(
        _endpoint: &SipEndpoint,
        _call_id_counter: Arc<RwLock<u32>>,
    ) -> Result<()> {
        // TODO: Implement TCP OPTIONS ping
        Err(anyhow::anyhow!("TCP monitoring not yet implemented"))
    }

    fn create_options_request(target: SocketAddr, call_id: u32) -> Result<String> {
        let request = format!(
            "OPTIONS sip:{}:{} SIP/2.0\r\n\
             Via: SIP/2.0/UDP redfire-switch:5060;branch=z9hG4bK{}\r\n\
             Max-Forwards: 70\r\n\
             To: <sip:{}:{}>\r\n\
             From: <sip:redfire-switch@redfire-switch>;tag=rs{}\r\n\
             Call-ID: {}@redfire-switch\r\n\
             CSeq: 1 OPTIONS\r\n\
             Contact: <sip:redfire-switch@redfire-switch:5060>\r\n\
             User-Agent: Redfire-Switch/0.1.0\r\n\
             Content-Length: 0\r\n\
             \r\n",
            target.ip(),
            target.port(),
            call_id,
            target.ip(),
            target.port(),
            call_id,
            call_id
        );

        Ok(request)
    }

    pub async fn get_endpoint_status(&self, endpoint_name: &str) -> Option<EndpointHealth> {
        let health_guard = self.endpoint_health.read().await;
        health_guard.get(endpoint_name).cloned()
    }

    pub async fn get_all_endpoint_status(&self) -> HashMap<String, EndpointHealth> {
        let health_guard = self.endpoint_health.read().await;
        health_guard.clone()
    }

    pub async fn enable_endpoint(&self, endpoint_name: &str) -> Result<()> {
        // TODO: Implement dynamic endpoint enabling
        info!("Enable endpoint {} (requires config update)", endpoint_name);
        Ok(())
    }

    pub async fn disable_endpoint(&self, endpoint_name: &str) -> Result<()> {
        // TODO: Implement dynamic endpoint disabling
        info!("Disable endpoint {} (requires config update)", endpoint_name);
        Ok(())
    }
}