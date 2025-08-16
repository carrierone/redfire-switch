/*
 * Redfire Switch - SIP Transport Layer (UDP/TCP/TLS/WSS)
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{UdpSocket, TcpListener, TcpStream};
use tokio::sync::{RwLock, mpsc};
use tokio_rustls::{TlsAcceptor, TlsConnector, TlsStream};
use rustls::{ServerConfig, ClientConfig};
use tracing::{debug, info, warn, error, instrument};

/// SIP transport types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SipTransport {
    Udp,
    Tcp,
    Tls,
    Wss, // WebSocket Secure
}

impl SipTransport {
    /// Get default port for transport
    pub fn default_port(&self) -> u16 {
        match self {
            SipTransport::Udp => 5060,
            SipTransport::Tcp => 5060,
            SipTransport::Tls => 5061,
            SipTransport::Wss => 443,
        }
    }
    
    /// Check if transport is secure
    pub fn is_secure(&self) -> bool {
        matches!(self, SipTransport::Tls | SipTransport::Wss)
    }
    
    /// Check if transport is connection-oriented
    pub fn is_connection_oriented(&self) -> bool {
        matches!(self, SipTransport::Tcp | SipTransport::Tls | SipTransport::Wss)
    }
}

/// SIP message with transport metadata
#[derive(Debug, Clone)]
pub struct TransportMessage {
    /// The SIP message
    pub message: rsip::SipMessage,
    /// Source address
    pub source: SocketAddr,
    /// Destination address
    pub destination: SocketAddr,
    /// Transport used
    pub transport: SipTransport,
    /// Message received timestamp
    pub received_at: chrono::DateTime<chrono::Utc>,
    /// Connection ID for connection-oriented transports
    pub connection_id: Option<String>,
}

/// Transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// Transport type
    pub transport: SipTransport,
    /// Bind address
    pub bind_address: SocketAddr,
    /// Maximum message size (bytes)
    pub max_message_size: usize,
    /// Connection timeout for TCP/TLS (seconds)
    pub connection_timeout: u64,
    /// Keep-alive interval (seconds)
    pub keep_alive_interval: Option<u64>,
    /// TLS configuration (for TLS/WSS)
    pub tls_config: Option<TlsConfig>,
    /// Enable for this transport
    pub enabled: bool,
}

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Certificate file path
    pub cert_file: String,
    /// Private key file path
    pub key_file: String,
    /// CA certificate file (for client verification)
    pub ca_file: Option<String>,
    /// Require client certificates
    pub require_client_cert: bool,
    /// Supported TLS versions
    pub min_version: String,
    /// Cipher suites
    pub cipher_suites: Vec<String>,
}

/// Active connection information
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// Connection ID
    pub id: String,
    /// Remote address
    pub remote_addr: SocketAddr,
    /// Local address
    pub local_addr: SocketAddr,
    /// Transport type
    pub transport: SipTransport,
    /// Connection established time
    pub established_at: DateTime<Utc>,
    /// Last activity time
    pub last_activity: DateTime<Utc>,
    /// Bytes sent
    pub bytes_sent: u64,
    /// Bytes received
    pub bytes_received: u64,
    /// Messages sent
    pub messages_sent: u64,
    /// Messages received
    pub messages_received: u64,
}

/// Transport event types
#[derive(Debug, Clone)]
pub enum TransportEvent {
    /// New connection established
    ConnectionEstablished {
        connection_id: String,
        remote_addr: SocketAddr,
        transport: SipTransport,
    },
    /// Connection closed
    ConnectionClosed {
        connection_id: String,
        reason: String,
    },
    /// Message received
    MessageReceived {
        message: TransportMessage,
    },
    /// Message sent
    MessageSent {
        destination: SocketAddr,
        transport: SipTransport,
        size: usize,
    },
    /// Transport error
    TransportError {
        transport: SipTransport,
        error: String,
    },
}

/// SIP transport manager
pub struct SipTransportManager {
    /// Transport configurations
    configs: Vec<TransportConfig>,
    /// Active connections (for connection-oriented transports)
    connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    /// Event sender
    event_sender: mpsc::UnboundedSender<TransportEvent>,
    /// Event receiver
    event_receiver: Arc<RwLock<mpsc::UnboundedReceiver<TransportEvent>>>,
    /// Message sender for outbound messages
    outbound_sender: mpsc::UnboundedSender<(TransportMessage, SocketAddr)>,
    /// TLS acceptor for TLS transport
    tls_acceptor: Option<TlsAcceptor>,
    /// TLS connector for outbound TLS connections
    tls_connector: Option<TlsConnector>,
}

impl SipTransportManager {
    /// Create new transport manager
    pub fn new(configs: Vec<TransportConfig>) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let (outbound_sender, _outbound_receiver) = mpsc::unbounded_channel();
        
        // Setup TLS if needed
        let (tls_acceptor, tls_connector) = Self::setup_tls(&configs)?;
        
        Ok(Self {
            configs,
            connections: Arc::new(RwLock::new(HashMap::new())),
            event_sender,
            event_receiver: Arc::new(RwLock::new(event_receiver)),
            outbound_sender,
            tls_acceptor,
            tls_connector,
        })
    }
    
    /// Start all configured transports
    #[instrument(skip(self))]
    pub async fn start(&self) -> Result<()> {
        info!("Starting SIP transport manager");
        
        for config in &self.configs {
            if !config.enabled {
                debug!("Transport {:?} disabled, skipping", config.transport);
                continue;
            }
            
            match config.transport {
                SipTransport::Udp => self.start_udp_transport(config).await?,
                SipTransport::Tcp => self.start_tcp_transport(config).await?,
                SipTransport::Tls => self.start_tls_transport(config).await?,
                SipTransport::Wss => self.start_wss_transport(config).await?,
            }
        }
        
        info!("All transports started successfully");
        Ok(())
    }
    
    /// Start UDP transport
    async fn start_udp_transport(&self, config: &TransportConfig) -> Result<()> {
        let socket = UdpSocket::bind(config.bind_address).await
            .map_err(|e| anyhow!("Failed to bind UDP socket to {}: {}", config.bind_address, e))?;
        
        info!("UDP transport listening on {}", config.bind_address);
        
        let event_sender = self.event_sender.clone();
        let max_message_size = config.max_message_size;
        
        tokio::spawn(async move {
            let mut buffer = vec![0u8; max_message_size];
            
            loop {
                match socket.recv_from(&mut buffer).await {
                    Ok((size, source)) => {
                        let message_data = &buffer[..size];
                        
                        match Self::parse_sip_message(message_data) {
                            Ok(message) => {
                                let transport_msg = TransportMessage {
                                    message,
                                    source,
                                    destination: config.bind_address,
                                    transport: SipTransport::Udp,
                                    received_at: chrono::Utc::now(),
                                    connection_id: None,
                                };
                                
                                if let Err(e) = event_sender.send(TransportEvent::MessageReceived {
                                    message: transport_msg,
                                }) {
                                    error!("Failed to send UDP message event: {}", e);
                                }
                            },
                            Err(e) => {
                                debug!("Failed to parse SIP message from {}: {}", source, e);
                            }
                        }
                    },
                    Err(e) => {
                        error!("UDP socket error: {}", e);
                        let _ = event_sender.send(TransportEvent::TransportError {
                            transport: SipTransport::Udp,
                            error: e.to_string(),
                        });
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Start TCP transport
    async fn start_tcp_transport(&self, config: &TransportConfig) -> Result<()> {
        let listener = TcpListener::bind(config.bind_address).await
            .map_err(|e| anyhow!("Failed to bind TCP listener to {}: {}", config.bind_address, e))?;
        
        info!("TCP transport listening on {}", config.bind_address);
        
        let event_sender = self.event_sender.clone();
        let connections = self.connections.clone();
        let max_message_size = config.max_message_size;
        let connection_timeout = config.connection_timeout;
        
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, remote_addr)) => {
                        let connection_id = uuid::Uuid::new_v4().to_string();
                        let local_addr = config.bind_address;
                        
                        // Register connection
                        let conn_info = ConnectionInfo {
                            id: connection_id.clone(),
                            remote_addr,
                            local_addr,
                            transport: SipTransport::Tcp,
                            established_at: chrono::Utc::now(),
                            last_activity: chrono::Utc::now(),
                            bytes_sent: 0,
                            bytes_received: 0,
                            messages_sent: 0,
                            messages_received: 0,
                        };
                        
                        connections.write().await.insert(connection_id.clone(), conn_info);
                        
                        let _ = event_sender.send(TransportEvent::ConnectionEstablished {
                            connection_id: connection_id.clone(),
                            remote_addr,
                            transport: SipTransport::Tcp,
                        });
                        
                        // Handle connection in separate task
                        let event_sender = event_sender.clone();
                        let connections = connections.clone();
                        tokio::spawn(async move {
                            if let Err(e) = Self::handle_tcp_connection(
                                stream, 
                                connection_id.clone(), 
                                remote_addr,
                                local_addr,
                                max_message_size,
                                connection_timeout,
                                event_sender.clone(),
                                connections.clone()
                            ).await {
                                error!("TCP connection {} error: {}", connection_id, e);
                            }
                            
                            // Clean up connection
                            connections.write().await.remove(&connection_id);
                            let _ = event_sender.send(TransportEvent::ConnectionClosed {
                                connection_id,
                                reason: "Connection ended".to_string(),
                            });
                        });
                    },
                    Err(e) => {
                        error!("TCP accept error: {}", e);
                        let _ = event_sender.send(TransportEvent::TransportError {
                            transport: SipTransport::Tcp,
                            error: e.to_string(),
                        });
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Start TLS transport
    async fn start_tls_transport(&self, config: &TransportConfig) -> Result<()> {
        let tls_acceptor = self.tls_acceptor.as_ref()
            .ok_or_else(|| anyhow!("TLS acceptor not configured"))?;
        
        let listener = TcpListener::bind(config.bind_address).await
            .map_err(|e| anyhow!("Failed to bind TLS listener to {}: {}", config.bind_address, e))?;
        
        info!("TLS transport listening on {}", config.bind_address);
        
        let event_sender = self.event_sender.clone();
        let connections = self.connections.clone();
        let max_message_size = config.max_message_size;
        let connection_timeout = config.connection_timeout;
        let tls_acceptor = tls_acceptor.clone();
        
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, remote_addr)) => {
                        let tls_acceptor = tls_acceptor.clone();
                        let event_sender = event_sender.clone();
                        let connections = connections.clone();
                        let local_addr = config.bind_address;
                        
                        tokio::spawn(async move {
                            match tls_acceptor.accept(stream).await {
                                Ok(tls_stream) => {
                                    let connection_id = uuid::Uuid::new_v4().to_string();
                                    
                                    // Register connection
                                    let conn_info = ConnectionInfo {
                                        id: connection_id.clone(),
                                        remote_addr,
                                        local_addr,
                                        transport: SipTransport::Tls,
                                        established_at: chrono::Utc::now(),
                                        last_activity: chrono::Utc::now(),
                                        bytes_sent: 0,
                                        bytes_received: 0,
                                        messages_sent: 0,
                                        messages_received: 0,
                                    };
                                    
                                    connections.write().await.insert(connection_id.clone(), conn_info);
                                    
                                    let _ = event_sender.send(TransportEvent::ConnectionEstablished {
                                        connection_id: connection_id.clone(),
                                        remote_addr,
                                        transport: SipTransport::Tls,
                                    });
                                    
                                    // Handle TLS connection
                                    if let Err(e) = Self::handle_tls_connection(
                                        tls_stream,
                                        connection_id.clone(),
                                        remote_addr,
                                        local_addr,
                                        max_message_size,
                                        connection_timeout,
                                        event_sender.clone(),
                                        connections.clone()
                                    ).await {
                                        error!("TLS connection {} error: {}", connection_id, e);
                                    }
                                    
                                    // Clean up connection
                                    connections.write().await.remove(&connection_id);
                                    let _ = event_sender.send(TransportEvent::ConnectionClosed {
                                        connection_id,
                                        reason: "TLS connection ended".to_string(),
                                    });
                                },
                                Err(e) => {
                                    error!("TLS handshake failed from {}: {}", remote_addr, e);
                                }
                            }
                        });
                    },
                    Err(e) => {
                        error!("TLS accept error: {}", e);
                        let _ = event_sender.send(TransportEvent::TransportError {
                            transport: SipTransport::Tls,
                            error: e.to_string(),
                        });
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Start WebSocket Secure transport (placeholder)
    async fn start_wss_transport(&self, _config: &TransportConfig) -> Result<()> {
        // WebSocket support would require additional dependencies
        // This is a placeholder for future implementation
        warn!("WebSocket Secure transport not implemented yet");
        Ok(())
    }
    
    /// Handle TCP connection
    async fn handle_tcp_connection(
        mut stream: TcpStream,
        connection_id: String,
        remote_addr: SocketAddr,
        local_addr: SocketAddr,
        max_message_size: usize,
        _connection_timeout: u64,
        event_sender: mpsc::UnboundedSender<TransportEvent>,
        connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    ) -> Result<()> {
        use tokio::io::{AsyncReadExt, BufReader};
        
        let mut reader = BufReader::new(&mut stream);
        let mut buffer = vec![0u8; max_message_size];
        
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => break, // Connection closed
                Ok(size) => {
                    let message_data = &buffer[..size];
                    
                    // Update connection stats
                    if let Some(mut conn) = connections.write().await.get_mut(&connection_id) {
                        conn.bytes_received += size as u64;
                        conn.last_activity = chrono::Utc::now();
                    }
                    
                    match Self::parse_sip_message(message_data) {
                        Ok(message) => {
                            // Update message count
                            if let Some(mut conn) = connections.write().await.get_mut(&connection_id) {
                                conn.messages_received += 1;
                            }
                            
                            let transport_msg = TransportMessage {
                                message,
                                source: remote_addr,
                                destination: local_addr,
                                transport: SipTransport::Tcp,
                                received_at: chrono::Utc::now(),
                                connection_id: Some(connection_id.clone()),
                            };
                            
                            if let Err(e) = event_sender.send(TransportEvent::MessageReceived {
                                message: transport_msg,
                            }) {
                                error!("Failed to send TCP message event: {}", e);
                            }
                        },
                        Err(e) => {
                            debug!("Failed to parse SIP message from TCP connection {}: {}", connection_id, e);
                        }
                    }
                },
                Err(e) => {
                    error!("TCP read error on connection {}: {}", connection_id, e);
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    /// Handle TLS connection
    async fn handle_tls_connection(
        mut stream: TlsStream<TcpStream>,
        connection_id: String,
        remote_addr: SocketAddr,
        local_addr: SocketAddr,
        max_message_size: usize,
        _connection_timeout: u64,
        event_sender: mpsc::UnboundedSender<TransportEvent>,
        connections: Arc<RwLock<HashMap<String, ConnectionInfo>>>,
    ) -> Result<()> {
        use tokio::io::{AsyncReadExt, BufReader};
        
        let mut reader = BufReader::new(&mut stream);
        let mut buffer = vec![0u8; max_message_size];
        
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => break, // Connection closed
                Ok(size) => {
                    let message_data = &buffer[..size];
                    
                    // Update connection stats
                    if let Some(mut conn) = connections.write().await.get_mut(&connection_id) {
                        conn.bytes_received += size as u64;
                        conn.last_activity = chrono::Utc::now();
                    }
                    
                    match Self::parse_sip_message(message_data) {
                        Ok(message) => {
                            // Update message count
                            if let Some(mut conn) = connections.write().await.get_mut(&connection_id) {
                                conn.messages_received += 1;
                            }
                            
                            let transport_msg = TransportMessage {
                                message,
                                source: remote_addr,
                                destination: local_addr,
                                transport: SipTransport::Tls,
                                received_at: chrono::Utc::now(),
                                connection_id: Some(connection_id.clone()),
                            };
                            
                            if let Err(e) = event_sender.send(TransportEvent::MessageReceived {
                                message: transport_msg,
                            }) {
                                error!("Failed to send TLS message event: {}", e);
                            }
                        },
                        Err(e) => {
                            debug!("Failed to parse SIP message from TLS connection {}: {}", connection_id, e);
                        }
                    }
                },
                Err(e) => {
                    error!("TLS read error on connection {}: {}", connection_id, e);
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    /// Parse SIP message from bytes
    fn parse_sip_message(data: &[u8]) -> Result<rsip::SipMessage> {
        let message_str = std::str::from_utf8(data)
            .map_err(|e| anyhow!("Invalid UTF-8 in SIP message: {}", e))?;
        
        rsip::SipMessage::try_from(message_str.as_bytes())
            .map_err(|e| anyhow!("Failed to parse SIP message: {}", e))
    }
    
    /// Setup TLS configuration
    fn setup_tls(configs: &[TransportConfig]) -> Result<(Option<TlsAcceptor>, Option<TlsConnector>)> {
        let mut tls_acceptor = None;
        let mut tls_connector = None;
        
        // Find TLS configuration
        for config in configs {
            if matches!(config.transport, SipTransport::Tls | SipTransport::Wss) {
                if let Some(tls_config) = &config.tls_config {
                    // Setup server configuration (acceptor)
                    let certs = Self::load_certs(&tls_config.cert_file)?;
                    let key = Self::load_private_key(&tls_config.key_file)?;
                    
                    let server_config = ServerConfig::builder()
                        .with_safe_defaults()
                        .with_no_client_auth()
                        .with_single_cert(certs, key)
                        .map_err(|e| anyhow!("Failed to create TLS server config: {}", e))?;
                    
                    tls_acceptor = Some(TlsAcceptor::from(Arc::new(server_config)));
                    
                    // Setup client configuration (connector)
                    let client_config = ClientConfig::builder()
                        .with_safe_defaults()
                        .with_root_certificates(rustls::RootCertStore::empty())
                        .with_no_client_auth();
                    
                    tls_connector = Some(TlsConnector::from(Arc::new(client_config)));
                    break;
                }
            }
        }
        
        Ok((tls_acceptor, tls_connector))
    }
    
    /// Load TLS certificates
    fn load_certs(filename: &str) -> Result<Vec<rustls::Certificate>> {
        let certfile = std::fs::File::open(filename)
            .map_err(|e| anyhow!("Cannot open certificate file '{}': {}", filename, e))?;
        let mut reader = std::io::BufReader::new(certfile);
        
        rustls_pemfile::certs(&mut reader)
            .map_err(|_| anyhow!("Cannot read certificate file"))?
            .into_iter()
            .map(rustls::Certificate)
            .collect::<Vec<_>>()
            .into()
    }
    
    /// Load private key
    fn load_private_key(filename: &str) -> Result<rustls::PrivateKey> {
        let keyfile = std::fs::File::open(filename)
            .map_err(|e| anyhow!("Cannot open private key file '{}': {}", filename, e))?;
        let mut reader = std::io::BufReader::new(keyfile);
        
        let keys = rustls_pemfile::pkcs8_private_keys(&mut reader)
            .map_err(|_| anyhow!("Cannot read private key file"))?;
        
        if keys.len() != 1 {
            return Err(anyhow!("Expected exactly one private key, found {}", keys.len()));
        }
        
        Ok(rustls::PrivateKey(keys[0].clone()))
    }
    
    /// Get event receiver
    pub async fn get_event_receiver(&self) -> Arc<RwLock<mpsc::UnboundedReceiver<TransportEvent>>> {
        self.event_receiver.clone()
    }
    
    /// Send SIP message
    pub async fn send_message(&self, message: &rsip::SipMessage, destination: SocketAddr, transport: SipTransport) -> Result<()> {
        let message_bytes = message.to_string().as_bytes().to_vec();
        
        match transport {
            SipTransport::Udp => {
                // Find UDP socket for sending
                // In production, this would maintain UDP sockets per transport config
                let socket = UdpSocket::bind("0.0.0.0:0").await?;
                socket.send_to(&message_bytes, destination).await?;
                
                let _ = self.event_sender.send(TransportEvent::MessageSent {
                    destination,
                    transport,
                    size: message_bytes.len(),
                });
            },
            SipTransport::Tcp => {
                // For TCP, we would reuse existing connections or create new ones
                // This is a simplified implementation
                let mut stream = TcpStream::connect(destination).await?;
                use tokio::io::AsyncWriteExt;
                stream.write_all(&message_bytes).await?;
                
                let _ = self.event_sender.send(TransportEvent::MessageSent {
                    destination,
                    transport,
                    size: message_bytes.len(),
                });
            },
            SipTransport::Tls => {
                if let Some(connector) = &self.tls_connector {
                    let stream = TcpStream::connect(destination).await?;
                    let domain = rustls::ServerName::try_from(destination.ip().to_string().as_str())
                        .map_err(|e| anyhow!("Invalid domain: {}", e))?;
                    let mut tls_stream = connector.connect(domain, stream).await?;
                    
                    use tokio::io::AsyncWriteExt;
                    tls_stream.write_all(&message_bytes).await?;
                    
                    let _ = self.event_sender.send(TransportEvent::MessageSent {
                        destination,
                        transport,
                        size: message_bytes.len(),
                    });
                } else {
                    return Err(anyhow!("TLS connector not configured"));
                }
            },
            SipTransport::Wss => {
                return Err(anyhow!("WebSocket transport not implemented"));
            }
        }
        
        Ok(())
    }
    
    /// Get active connections
    pub async fn get_connections(&self) -> Vec<ConnectionInfo> {
        self.connections.read().await.values().cloned().collect()
    }
    
    /// Close connection
    pub async fn close_connection(&self, connection_id: &str) -> Result<()> {
        // In production, this would actually close the connection
        self.connections.write().await.remove(connection_id);
        
        let _ = self.event_sender.send(TransportEvent::ConnectionClosed {
            connection_id: connection_id.to_string(),
            reason: "Administratively closed".to_string(),
        });
        
        Ok(())
    }
}