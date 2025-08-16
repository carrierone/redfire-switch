/*
 * MCP Server Handlers for Redfire Switch
 * Provides AI-accessible tools for telecommunications operations via HTTP
 */

use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use warp::{Filter, Reply};

use crate::codec_tools::CodecTools;
use crate::sip_tools::SipTools;

#[derive(Clone)]
pub struct RedfireMcpServer {
    codec_tools: Arc<CodecTools>,
    sip_tools: Arc<SipTools>,
    session_data: Arc<RwLock<HashMap<String, Value>>>,
}

impl RedfireMcpServer {
    pub async fn new(gpu_enabled: bool, gpu_device: u32) -> Result<Self> {
        info!("Initializing Redfire MCP Server");
        
        let codec_tools = Arc::new(CodecTools::new(gpu_enabled, gpu_device).await?);
        let sip_tools = Arc::new(SipTools::new().await?);
        
        Ok(Self {
            codec_tools,
            sip_tools,
            session_data: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    pub async fn run(&self, addr: &str) -> Result<()> {
        let server = Arc::new(self.clone());
        
        // List tools endpoint
        let list_tools = warp::path("list_tools")
            .and(warp::get())
            .map(|| {
                warp::reply::json(&json!({
                    "tools": [
                        "transcode_audio",
                        "get_codec_info", 
                        "benchmark_transcoding",
                        "parse_sip_message", 
                        "generate_sip_message"
                    ],
                    "description": "Redfire Switch CLI-accessible Tools"
                }))
            });
        
        // Transcode audio endpoint
        let server_clone = server.clone();
        let transcode_audio = warp::path("transcode_audio")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |args: Value| {
                let server = server_clone.clone();
                async move {
                    match server.codec_tools.transcode_audio(args).await {
                        Ok(result) => Ok(warp::reply::json(&result)),
                        Err(e) => {
                            error!("Transcode error: {}", e);
                            Ok(warp::reply::json(&json!({
                                "success": false,
                                "error": e.to_string()
                            })))
                        }
                    }
                }
            });
        
        // Get codec info endpoint
        let server_clone = server.clone();
        let get_codec_info = warp::path("get_codec_info")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |args: Value| {
                let server = server_clone.clone();
                async move {
                    match server.codec_tools.get_codec_info(args).await {
                        Ok(result) => Ok(warp::reply::json(&result)),
                        Err(e) => {
                            error!("Codec info error: {}", e);
                            Ok(warp::reply::json(&json!({
                                "success": false,
                                "error": e.to_string()
                            })))
                        }
                    }
                }
            });
        
        // Benchmark transcoding endpoint
        let server_clone = server.clone();
        let benchmark_transcoding = warp::path("benchmark_transcoding")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |args: Value| {
                let server = server_clone.clone();
                async move {
                    match server.codec_tools.benchmark_transcoding(args).await {
                        Ok(result) => Ok(warp::reply::json(&result)),
                        Err(e) => {
                            error!("Benchmark error: {}", e);
                            Ok(warp::reply::json(&json!({
                                "success": false,
                                "error": e.to_string()
                            })))
                        }
                    }
                }
            });
        
        // Parse SIP message endpoint
        let server_clone = server.clone();
        let parse_sip_message = warp::path("parse_sip_message")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |args: Value| {
                let server = server_clone.clone();
                async move {
                    match server.sip_tools.parse_sip_message(args).await {
                        Ok(result) => Ok(warp::reply::json(&result)),
                        Err(e) => {
                            error!("SIP parse error: {}", e);
                            Ok(warp::reply::json(&json!({
                                "success": false,
                                "error": e.to_string()
                            })))
                        }
                    }
                }
            });
        
        // Generate SIP message endpoint
        let server_clone = server.clone();
        let generate_sip_message = warp::path("generate_sip_message")
            .and(warp::post())
            .and(warp::body::json())
            .and_then(move |args: Value| {
                let server = server_clone.clone();
                async move {
                    match server.sip_tools.generate_sip_message(args).await {
                        Ok(result) => Ok(warp::reply::json(&result)),
                        Err(e) => {
                            error!("SIP generate error: {}", e);
                            Ok(warp::reply::json(&json!({
                                "success": false,
                                "error": e.to_string()
                            })))
                        }
                    }
                }
            });
        
        // Combine all routes
        let routes = list_tools
            .or(transcode_audio)
            .or(get_codec_info)
            .or(benchmark_transcoding)
            .or(parse_sip_message)
            .or(generate_sip_message)
            .with(warp::cors().allow_any_origin());
        
        // Parse address
        let socket_addr: std::net::SocketAddr = addr.parse()
            .map_err(|e| anyhow::anyhow!("Invalid address {}: {}", addr, e))?;
        
        info!("HTTP MCP Server starting on {}", addr);
        warp::serve(routes).run(socket_addr).await;
        
        Ok(())
    }
}

impl Clone for RedfireMcpServer {
    fn clone(&self) -> Self {
        Self {
            codec_tools: Arc::clone(&self.codec_tools),
            sip_tools: Arc::clone(&self.sip_tools),
            session_data: Arc::clone(&self.session_data),
        }
    }
}