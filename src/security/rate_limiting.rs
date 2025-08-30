//! Rate limiting and DoS protection
//! 
//! This module provides rate limiting functionality to prevent
//! denial of service attacks and resource exhaustion.

use super::{SecurityError, SecurityConfig};
use anyhow::Result;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};
use tracing::{debug, warn, error};

/// Rate limiter for tracking request rates per IP
pub struct RateLimiter {
    /// Rate limit configuration
    config: SecurityConfig,
    /// Per-IP rate limiting buckets
    buckets: Arc<RwLock<HashMap<IpAddr, RateLimitBucket>>>,
    /// Global connection counter
    global_connections: Arc<RwLock<HashMap<IpAddr, u32>>>,
}

/// Rate limiting bucket for token bucket algorithm
#[derive(Debug, Clone)]
struct RateLimitBucket {
    /// Number of tokens available
    tokens: u32,
    /// Last refill time
    last_refill: Instant,
    /// Number of requests in current window
    requests_in_window: u32,
    /// Window start time
    window_start: Instant,
}

impl RateLimitBucket {
    /// Create a new rate limit bucket
    fn new(max_tokens: u32) -> Self {
        let now = Instant::now();
        Self {
            tokens: max_tokens,
            last_refill: now,
            requests_in_window: 0,
            window_start: now,
        }
    }
    
    /// Check if request is allowed and consume a token
    fn try_consume(&mut self, max_requests_per_minute: u32) -> bool {
        let now = Instant::now();
        
        // Reset window if needed (1-minute windows)
        if now.duration_since(self.window_start) >= Duration::from_secs(60) {
            self.requests_in_window = 0;
            self.window_start = now;
        }
        
        // Check if we're under the rate limit
        if self.requests_in_window >= max_requests_per_minute {
            return false;
        }
        
        self.requests_in_window += 1;
        true
    }
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            config,
            buckets: Arc::new(RwLock::new(HashMap::new())),
            global_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Check if a request from the given IP is allowed
    pub async fn check_rate_limit(&self, ip: IpAddr) -> Result<(), SecurityError> {
        if !self.config.enable_rate_limiting {
            return Ok(());
        }
        
        let mut buckets = self.buckets.write().await;
        
        // Get or create bucket for this IP
        let bucket = buckets.entry(ip)
            .or_insert_with(|| RateLimitBucket::new(self.config.max_requests_per_minute));
        
        if !bucket.try_consume(self.config.max_requests_per_minute) {
            warn!("Rate limit exceeded for IP: {}", ip);
            return Err(SecurityError::RateLimitExceeded(format!("Rate limit exceeded for IP: {}", ip)));
        }
        
        debug!("Rate limit check passed for IP: {}", ip);
        Ok(())
    }
    
    /// Check connection limit for IP
    pub async fn check_connection_limit(&self, ip: IpAddr) -> Result<(), SecurityError> {
        let connections = self.global_connections.read().await;
        
        if let Some(&count) = connections.get(&ip) {
            if count >= self.config.max_connections_per_ip {
                warn!("Connection limit exceeded for IP: {} ({})", ip, count);
                return Err(SecurityError::RateLimitExceeded(format!("Connection limit exceeded for IP: {} ({})", ip, count)));
            }
        }
        
        Ok(())
    }
    
    /// Register a new connection
    pub async fn register_connection(&self, ip: IpAddr) -> Result<(), SecurityError> {
        self.check_connection_limit(ip).await?;
        
        let mut connections = self.global_connections.write().await;
        *connections.entry(ip).or_insert(0) += 1;
        
        debug!("Registered connection for IP: {} (total: {})", ip, connections[&ip]);
        Ok(())
    }
    
    /// Unregister a connection
    pub async fn unregister_connection(&self, ip: IpAddr) {
        let mut connections = self.global_connections.write().await;
        
        // Fix borrowing issue by getting count first, then removing if needed
        let should_remove = if let Some(count) = connections.get_mut(&ip) {
            if *count > 0 {
                *count -= 1;
                debug!("Unregistered connection for IP: {} (remaining: {})", ip, *count);
                *count == 0
            } else {
                false
            }
        } else {
            false
        };
        
        if should_remove {
            connections.remove(&ip);
        }
    }
    
    /// Get current connection count for an IP
    pub async fn get_connection_count(&self, ip: IpAddr) -> u32 {
        let connections = self.global_connections.read().await;
        connections.get(&ip).copied().unwrap_or(0)
    }
    
    /// Clean up old entries (should be called periodically)
    pub async fn cleanup_old_entries(&self) {
        let now = Instant::now();
        let mut buckets = self.buckets.write().await;
        let mut connections = self.global_connections.write().await;
        
        // Remove buckets that haven't been used for 5 minutes
        buckets.retain(|ip, bucket| {
            let age = now.duration_since(bucket.last_refill);
            if age > Duration::from_secs(300) {
                debug!("Cleaning up rate limit bucket for IP: {}", ip);
                false
            } else {
                true
            }
        });
        
        // Remove zero-connection entries
        connections.retain(|_, &mut count| count > 0);
        
        debug!("Cleaned up rate limiter: {} active buckets, {} active connections", 
               buckets.len(), connections.len());
    }
    
    /// Get rate limiting statistics
    pub async fn get_stats(&self) -> RateLimiterStats {
        let buckets = self.buckets.read().await;
        let connections = self.global_connections.read().await;
        
        RateLimiterStats {
            active_buckets: buckets.len(),
            total_connections: connections.values().sum(),
            unique_ips: connections.len(),
        }
    }
}

/// Rate limiter statistics
#[derive(Debug, Clone)]
pub struct RateLimiterStats {
    pub active_buckets: usize,
    pub total_connections: u32,
    pub unique_ips: usize,
}

/// Connection tracking for resource management
pub struct ConnectionTracker {
    rate_limiter: Arc<RateLimiter>,
    ip: IpAddr,
    registered: bool,
}

impl ConnectionTracker {
    /// Create a new connection tracker
    pub async fn new(rate_limiter: Arc<RateLimiter>, ip: IpAddr) -> Result<Self, SecurityError> {
        rate_limiter.register_connection(ip).await?;
        
        Ok(Self {
            rate_limiter,
            ip,
            registered: true,
        })
    }
    
    /// Check rate limit for this connection
    pub async fn check_rate_limit(&self) -> Result<(), SecurityError> {
        self.rate_limiter.check_rate_limit(self.ip).await
    }
    
    /// Get connection count for this IP
    pub async fn get_connection_count(&self) -> u32 {
        self.rate_limiter.get_connection_count(self.ip).await
    }
}

impl Drop for ConnectionTracker {
    fn drop(&mut self) {
        if self.registered {
            let rate_limiter = self.rate_limiter.clone();
            let ip = self.ip;
            
            // Spawn a task to unregister the connection
            tokio::spawn(async move {
                rate_limiter.unregister_connection(ip).await;
            });
        }
    }
}

/// DoS protection middleware
pub struct DosProtection {
    rate_limiter: Arc<RateLimiter>,
    /// Suspicious IPs that are temporarily blocked
    blocked_ips: Arc<RwLock<HashMap<IpAddr, Instant>>>,
    /// Block duration for suspicious activity
    block_duration: Duration,
}

impl DosProtection {
    /// Create new DoS protection
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            rate_limiter: Arc::new(RateLimiter::new(config)),
            blocked_ips: Arc::new(RwLock::new(HashMap::new())),
            block_duration: Duration::from_secs(300), // 5 minutes
        }
    }
    
    /// Check if request should be allowed
    pub async fn check_request(&self, ip: IpAddr) -> Result<ConnectionTracker, SecurityError> {
        // Check if IP is temporarily blocked
        {
            let blocked = self.blocked_ips.read().await;
            if let Some(&blocked_time) = blocked.get(&ip) {
                if blocked_time.elapsed() < self.block_duration {
                    error!("Blocked IP attempted connection: {}", ip);
                    return Err(SecurityError::AccessDenied("IP is blacklisted".to_string()));
                }
            }
        }
        
        // Create connection tracker (this also checks rate limits)
        match ConnectionTracker::new(self.rate_limiter.clone(), ip).await {
            Ok(tracker) => Ok(tracker),
            Err(SecurityError::RateLimitExceeded(msg)) => {
                // Temporarily block aggressive IPs
                self.temporarily_block_ip(ip).await;
                Err(SecurityError::RateLimitExceeded(msg))
            }
            Err(e) => Err(e),
        }
    }
    
    /// Temporarily block an IP for suspicious activity
    async fn temporarily_block_ip(&self, ip: IpAddr) {
        let mut blocked = self.blocked_ips.write().await;
        blocked.insert(ip, Instant::now());
        warn!("Temporarily blocked suspicious IP: {}", ip);
    }
    
    /// Clean up expired blocks and old rate limit data
    pub async fn cleanup(&self) {
        // Clean up expired blocks
        {
            let mut blocked = self.blocked_ips.write().await;
            let now = Instant::now();
            blocked.retain(|ip, &mut blocked_time| {
                if now.duration_since(blocked_time) >= self.block_duration {
                    debug!("Unblocking IP: {}", ip);
                    false
                } else {
                    true
                }
            });
        }
        
        // Clean up rate limiter
        self.rate_limiter.cleanup_old_entries().await;
    }
    
    /// Get DoS protection statistics
    pub async fn get_stats(&self) -> DosProtectionStats {
        let rate_stats = self.rate_limiter.get_stats().await;
        let blocked = self.blocked_ips.read().await;
        
        DosProtectionStats {
            rate_limiter: rate_stats,
            blocked_ips: blocked.len(),
        }
    }
}

/// DoS protection statistics
#[derive(Debug, Clone)]
pub struct DosProtectionStats {
    pub rate_limiter: RateLimiterStats,
    pub blocked_ips: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_rate_limiting() {
        let config = SecurityConfig {
            enable_rate_limiting: true,
            max_requests_per_minute: 2,
            ..Default::default()
        };
        
        let limiter = RateLimiter::new(config);
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        
        // First two requests should pass
        assert!(limiter.check_rate_limit(ip).await.is_ok());
        assert!(limiter.check_rate_limit(ip).await.is_ok());
        
        // Third request should be rate limited
        assert!(matches!(limiter.check_rate_limit(ip).await, Err(SecurityError::RateLimitExceeded(_))));
    }
    
    #[tokio::test]
    async fn test_connection_tracking() {
        let config = SecurityConfig {
            max_connections_per_ip: 2,
            ..Default::default()
        };
        
        let limiter = Arc::new(RateLimiter::new(config));
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        
        // First two connections should succeed
        let _conn1 = ConnectionTracker::new(limiter.clone(), ip).await.unwrap();
        let _conn2 = ConnectionTracker::new(limiter.clone(), ip).await.unwrap();
        
        // Third connection should fail
        assert!(matches!(ConnectionTracker::new(limiter.clone(), ip).await, 
                        Err(SecurityError::RateLimitExceeded(_))));
    }
}