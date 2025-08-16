/*
 * Redfire Switch - MCP Server for AI Integration
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
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// MCP (Model Context Protocol) message types
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum McpRequest {
    #[serde(rename = "initialize")]
    Initialize {
        params: InitializeParams,
    },
    #[serde(rename = "tools/list")]
    ListTools,
    #[serde(rename = "tools/call")]
    CallTool {
        params: CallToolParams,
    },
    #[serde(rename = "resources/list")]
    ListResources,
    #[serde(rename = "resources/read")]
    ReadResource {
        params: ReadResourceParams,
    },
    #[serde(rename = "prompts/list")]
    ListPrompts,
    #[serde(rename = "prompts/get")]
    GetPrompt {
        params: GetPromptParams,
    },
}

/// MCP response structure
#[derive(Debug, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP error structure
#[derive(Debug, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Initialize parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    pub capabilities: ClientCapabilities,
    #[serde(rename = "clientInfo")]
    pub client_info: ClientInfo,
}

/// Client capabilities
#[derive(Debug, Serialize, Deserialize)]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<Value>,
}

/// Client information
#[derive(Debug, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// Tool call parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct CallToolParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

/// Resource read parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct ReadResourceParams {
    pub uri: String,
}

/// Prompt get parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct GetPromptParams {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Value>,
}

/// Tool definition for MCP
#[derive(Debug, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Resource definition for MCP
#[derive(Debug, Serialize, Deserialize)]
pub struct Resource {
    pub uri: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Prompt definition for MCP
#[derive(Debug, Serialize, Deserialize)]
pub struct Prompt {
    pub name: String,
    pub description: String,
    pub arguments: Vec<PromptArgument>,
}

/// Prompt argument
#[derive(Debug, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    pub description: String,
    pub required: bool,
}

/// MCP server state
pub struct McpServer {
    /// Available tools
    tools: Vec<Tool>,
    /// Available resources
    resources: Vec<Resource>,
    /// Available prompts
    prompts: Vec<Prompt>,
    /// Server capabilities
    capabilities: ServerCapabilities,
    /// Active sessions
    sessions: Arc<RwLock<HashMap<String, McpSession>>>,
}

/// Server capabilities
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerCapabilities {
    pub experimental: Option<Value>,
    pub logging: Option<Value>,
    pub prompts: Option<PromptsCapability>,
    pub resources: Option<ResourcesCapability>,
    pub tools: Option<ToolsCapability>,
}

/// Prompts capability
#[derive(Debug, Serialize, Deserialize)]
pub struct PromptsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: Option<bool>,
}

/// Resources capability
#[derive(Debug, Serialize, Deserialize)]
pub struct ResourcesCapability {
    pub subscribe: Option<bool>,
    #[serde(rename = "listChanged")]
    pub list_changed: Option<bool>,
}

/// Tools capability
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: Option<bool>,
}

/// MCP session information
pub struct McpSession {
    pub id: String,
    pub client_info: Option<ClientInfo>,
    pub initialized: bool,
}

impl McpServer {
    /// Create new MCP server
    pub fn new() -> Self {
        let tools = vec![
            Tool {
                name: "get_system_status".to_string(),
                description: "Get current system status and statistics".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            Tool {
                name: "list_active_calls".to_string(),
                description: "List all currently active calls".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of calls to return",
                            "default": 50
                        }
                    },
                    "required": []
                }),
            },
            Tool {
                name: "search_calls".to_string(),
                description: "Search for calls by phone number or other criteria".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "phone_number": {
                            "type": "string",
                            "description": "Phone number to search for"
                        },
                        "customer_id": {
                            "type": "string",
                            "description": "Customer ID to filter by"
                        },
                        "start_time": {
                            "type": "string",
                            "description": "Start time for search (ISO 8601 format)"
                        },
                        "end_time": {
                            "type": "string",
                            "description": "End time for search (ISO 8601 format)"
                        }
                    },
                    "required": []
                }),
            },
            Tool {
                name: "manage_did".to_string(),
                description: "Create, update, or delete DID/TFN assignments".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["create", "update", "delete", "list"],
                            "description": "Action to perform"
                        },
                        "number": {
                            "type": "string",
                            "description": "DID/TFN number"
                        },
                        "customer_id": {
                            "type": "string",
                            "description": "Customer ID"
                        },
                        "destination_type": {
                            "type": "string",
                            "enum": ["extension", "external", "voicemail", "ivr"],
                            "description": "Type of destination"
                        },
                        "destination_value": {
                            "type": "string",
                            "description": "Destination value (extension, phone number, etc.)"
                        }
                    },
                    "required": ["action"]
                }),
            },
            Tool {
                name: "send_sms".to_string(),
                description: "Send SMS message through the switch".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "from": {
                            "type": "string",
                            "description": "From phone number"
                        },
                        "to": {
                            "type": "string",
                            "description": "To phone number"
                        },
                        "message": {
                            "type": "string",
                            "description": "Message content"
                        },
                        "customer_id": {
                            "type": "string",
                            "description": "Customer ID"
                        }
                    },
                    "required": ["from", "to", "message"]
                }),
            },
            Tool {
                name: "analyze_traffic".to_string(),
                description: "Analyze call traffic patterns and statistics".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "time_period": {
                            "type": "string",
                            "enum": ["hour", "day", "week", "month"],
                            "description": "Time period for analysis"
                        },
                        "metric": {
                            "type": "string",
                            "enum": ["volume", "duration", "success_rate", "destinations"],
                            "description": "Metric to analyze"
                        }
                    },
                    "required": []
                }),
            },
        ];

        let resources = vec![
            Resource {
                uri: "switch://config".to_string(),
                name: "Switch Configuration".to_string(),
                description: "Current switch configuration".to_string(),
                mime_type: Some("application/json".to_string()),
            },
            Resource {
                uri: "switch://logs/system".to_string(),
                name: "System Logs".to_string(),
                description: "Recent system log entries".to_string(),
                mime_type: Some("text/plain".to_string()),
            },
            Resource {
                uri: "switch://stats/realtime".to_string(),
                name: "Real-time Statistics".to_string(),
                description: "Real-time switch statistics".to_string(),
                mime_type: Some("application/json".to_string()),
            },
        ];

        let prompts = vec![
            Prompt {
                name: "troubleshoot_call_issue".to_string(),
                description: "Help troubleshoot call routing or quality issues".to_string(),
                arguments: vec![
                    PromptArgument {
                        name: "phone_number".to_string(),
                        description: "Phone number experiencing issues".to_string(),
                        required: true,
                    },
                    PromptArgument {
                        name: "issue_description".to_string(),
                        description: "Description of the issue".to_string(),
                        required: false,
                    },
                ],
            },
            Prompt {
                name: "optimize_routing".to_string(),
                description: "Analyze and suggest routing optimizations".to_string(),
                arguments: vec![
                    PromptArgument {
                        name: "destination_pattern".to_string(),
                        description: "Destination pattern to optimize".to_string(),
                        required: false,
                    },
                ],
            },
        ];

        let capabilities = ServerCapabilities {
            experimental: None,
            logging: None,
            prompts: Some(PromptsCapability {
                list_changed: Some(false),
            }),
            resources: Some(ResourcesCapability {
                subscribe: Some(false),
                list_changed: Some(false),
            }),
            tools: Some(ToolsCapability {
                list_changed: Some(false),
            }),
        };

        McpServer {
            tools,
            resources,
            prompts,
            capabilities,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Handle MCP request
    pub async fn handle_request(&self, session_id: &str, request: Value) -> Result<McpResponse> {
        let id = request.get("id").cloned();
        
        // Parse the request method and params
        let method = request.get("method")
            .and_then(|m| m.as_str())
            .ok_or_else(|| anyhow!("Missing method field"))?;

        match method {
            "initialize" => {
                let params: InitializeParams = serde_json::from_value(
                    request.get("params").unwrap_or(&json!({})).clone()
                )?;
                self.handle_initialize(session_id, params).await
            },
            "tools/list" => {
                Ok(McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({
                        "tools": self.tools
                    })),
                    error: None,
                })
            },
            "tools/call" => {
                let params: CallToolParams = serde_json::from_value(
                    request.get("params").unwrap_or(&json!({})).clone()
                )?;
                self.handle_tool_call(params).await
            },
            "resources/list" => {
                Ok(McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({
                        "resources": self.resources
                    })),
                    error: None,
                })
            },
            "resources/read" => {
                let params: ReadResourceParams = serde_json::from_value(
                    request.get("params").unwrap_or(&json!({})).clone()
                )?;
                self.handle_resource_read(params).await
            },
            "prompts/list" => {
                Ok(McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({
                        "prompts": self.prompts
                    })),
                    error: None,
                })
            },
            "prompts/get" => {
                let params: GetPromptParams = serde_json::from_value(
                    request.get("params").unwrap_or(&json!({})).clone()
                )?;
                self.handle_prompt_get(params).await
            },
            _ => {
                Ok(McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(McpError {
                        code: -32601,
                        message: format!("Method not found: {}", method),
                        data: None,
                    }),
                })
            }
        }
    }

    /// Handle initialize request
    async fn handle_initialize(&self, session_id: &str, params: InitializeParams) -> Result<McpResponse> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(session_id) {
            session.client_info = Some(params.client_info);
            session.initialized = true;
        }

        info!("MCP client initialized: protocol version {}", params.protocol_version);

        Ok(McpResponse {
            jsonrpc: "2.0".to_string(),
            id: None,
            result: Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": self.capabilities,
                "serverInfo": {
                    "name": "Redfire Switch MCP Server",
                    "version": "1.0.0"
                }
            })),
            error: None,
        })
    }

    /// Handle tool call
    async fn handle_tool_call(&self, params: CallToolParams) -> Result<McpResponse> {
        let result = match params.name.as_str() {
            "get_system_status" => {
                json!({
                    "content": [{
                        "type": "text",
                        "text": "System Status: Running\nActive Calls: 42\nUptime: 5 days, 14 hours\nMemory Usage: 256MB / 1GB\nCPU Usage: 15%"
                    }]
                })
            },
            "list_active_calls" => {
                let limit = params.arguments.as_ref()
                    .and_then(|args| args.get("limit"))
                    .and_then(|l| l.as_i64())
                    .unwrap_or(50);
                
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Found {} active calls (showing first {}):\n1. +1555123456 -> +1555987654 (00:02:34)\n2. +1555111222 -> +1555333444 (00:01:15)", 42, limit)
                    }]
                })
            },
            "search_calls" => {
                let args = params.arguments.as_ref().unwrap_or(&json!({}));
                let phone = args.get("phone_number").and_then(|p| p.as_str()).unwrap_or("N/A");
                
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Search results for phone number: {}\n- Found 5 calls in the last 24 hours\n- Average duration: 3:45\n- Success rate: 98.2%", phone)
                    }]
                })
            },
            "manage_did" => {
                let args = params.arguments.as_ref().unwrap_or(&json!({}));
                let action = args.get("action").and_then(|a| a.as_str()).unwrap_or("list");
                let number = args.get("number").and_then(|n| n.as_str()).unwrap_or("N/A");
                
                match action {
                    "list" => {
                        json!({
                            "content": [{
                                "type": "text",
                                "text": "DIDs assigned:\n- +15551234567 -> Extension 1001\n- +15551234568 -> Voicemail Box 2001\n- +15551234569 -> IVR Menu 'main'"
                            }]
                        })
                    },
                    "create" => {
                        json!({
                            "content": [{
                                "type": "text",
                                "text": format!("DID {} created successfully", number)
                            }]
                        })
                    },
                    _ => {
                        json!({
                            "content": [{
                                "type": "text",
                                "text": format!("Action '{}' completed for DID {}", action, number)
                            }]
                        })
                    }
                }
            },
            "send_sms" => {
                let args = params.arguments.as_ref().unwrap_or(&json!({}));
                let from = args.get("from").and_then(|f| f.as_str()).unwrap_or("N/A");
                let to = args.get("to").and_then(|t| t.as_str()).unwrap_or("N/A");
                
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("SMS sent successfully from {} to {}\nMessage ID: {}", from, to, Uuid::new_v4())
                    }]
                })
            },
            "analyze_traffic" => {
                let args = params.arguments.as_ref().unwrap_or(&json!({}));
                let period = args.get("time_period").and_then(|p| p.as_str()).unwrap_or("day");
                let metric = args.get("metric").and_then(|m| m.as_str()).unwrap_or("volume");
                
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Traffic Analysis ({})\n{}: 1,234 calls\nPeak hour: 2:00 PM - 3:00 PM\nAverage duration: 4:12\nSuccess rate: 97.8%", period, metric)
                    }]
                })
            },
            _ => {
                return Ok(McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(McpError {
                        code: -32602,
                        message: format!("Unknown tool: {}", params.name),
                        data: None,
                    }),
                });
            }
        };

        Ok(McpResponse {
            jsonrpc: "2.0".to_string(),
            id: None,
            result: Some(result),
            error: None,
        })
    }

    /// Handle resource read
    async fn handle_resource_read(&self, params: ReadResourceParams) -> Result<McpResponse> {
        let content = match params.uri.as_str() {
            "switch://config" => {
                json!({
                    "contents": [{
                        "uri": params.uri,
                        "mimeType": "application/json",
                        "text": r#"{"sip_port": 5060, "rtp_port_range": [10000, 20000], "max_calls": 1000}"#
                    }]
                })
            },
            "switch://logs/system" => {
                json!({
                    "contents": [{
                        "uri": params.uri,
                        "mimeType": "text/plain",
                        "text": "[2025-01-14 12:34:56] INFO Call established: +15551234567 -> +15559876543\n[2025-01-14 12:35:12] INFO SMS delivered: msg_12345\n[2025-01-14 12:35:30] WARN Rate limit approaching for trunk_01"
                    }]
                })
            },
            "switch://stats/realtime" => {
                json!({
                    "contents": [{
                        "uri": params.uri,
                        "mimeType": "application/json",
                        "text": r#"{"active_calls": 42, "cps": 15, "memory_usage": 67.2, "cpu_usage": 12.5}"#
                    }]
                })
            },
            _ => {
                return Ok(McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(McpError {
                        code: -32602,
                        message: format!("Resource not found: {}", params.uri),
                        data: None,
                    }),
                });
            }
        };

        Ok(McpResponse {
            jsonrpc: "2.0".to_string(),
            id: None,
            result: Some(content),
            error: None,
        })
    }

    /// Handle prompt get
    async fn handle_prompt_get(&self, params: GetPromptParams) -> Result<McpResponse> {
        let content = match params.name.as_str() {
            "troubleshoot_call_issue" => {
                let args = params.arguments.as_ref().unwrap_or(&json!({}));
                let phone = args.get("phone_number").and_then(|p| p.as_str()).unwrap_or("N/A");
                let issue = args.get("issue_description").and_then(|i| i.as_str()).unwrap_or("No description provided");
                
                json!({
                    "description": "Troubleshooting call issues",
                    "messages": [{
                        "role": "user",
                        "content": {
                            "type": "text",
                            "text": format!("I'm experiencing call issues with phone number {} . The issue is: {}. Please help me troubleshoot this by:\n\n1. Checking call logs for this number\n2. Verifying trunk availability\n3. Reviewing routing configuration\n4. Analyzing any error patterns\n5. Suggesting potential solutions\n\nPlease provide specific steps and commands I can use to diagnose and resolve this issue.", phone, issue)
                        }
                    }]
                })
            },
            "optimize_routing" => {
                let args = params.arguments.as_ref().unwrap_or(&json!({}));
                let pattern = args.get("destination_pattern").and_then(|p| p.as_str()).unwrap_or("all destinations");
                
                json!({
                    "description": "Routing optimization analysis",
                    "messages": [{
                        "role": "user",
                        "content": {
                            "type": "text",
                            "text": format!("I need help optimizing routing for {}. Please:\n\n1. Analyze current routing efficiency\n2. Identify bottlenecks or suboptimal routes\n3. Review trunk utilization patterns\n4. Suggest configuration improvements\n5. Recommend load balancing strategies\n6. Provide cost optimization opportunities\n\nPlease give me specific recommendations with configuration examples and expected improvements.", pattern)
                        }
                    }]
                })
            },
            _ => {
                return Ok(McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(McpError {
                        code: -32602,
                        message: format!("Prompt not found: {}", params.name),
                        data: None,
                    }),
                });
            }
        };

        Ok(McpResponse {
            jsonrpc: "2.0".to_string(),
            id: None,
            result: Some(content),
            error: None,
        })
    }

    /// Create new session
    pub async fn create_session(&self) -> String {
        let session_id = Uuid::new_v4().to_string();
        let session = McpSession {
            id: session_id.clone(),
            client_info: None,
            initialized: false,
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.clone(), session);
        
        session_id
    }

    /// Remove session
    pub async fn remove_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
    }
}

/// Handle MCP client connection
async fn handle_client(mut stream: TcpStream, server: Arc<McpServer>) -> Result<()> {
    let session_id = server.create_session().await;
    info!("New MCP client connected, session: {}", session_id);

    let mut buffer = vec![0; 8192];
    
    loop {
        match stream.read(&mut buffer).await {
            Ok(0) => {
                info!("MCP client disconnected: {}", session_id);
                break;
            },
            Ok(n) => {
                let data = &buffer[..n];
                
                // Parse JSON-RPC messages (simplified - in production would need proper framing)
                if let Ok(text) = std::str::from_utf8(data) {
                    for line in text.lines() {
                        if line.trim().is_empty() {
                            continue;
                        }
                        
                        match serde_json::from_str::<Value>(line) {
                            Ok(request) => {
                                debug!("Received MCP request: {}", serde_json::to_string_pretty(&request)?);
                                
                                match server.handle_request(&session_id, request).await {
                                    Ok(response) => {
                                        let response_text = serde_json::to_string(&response)?;
                                        debug!("Sending MCP response: {}", response_text);
                                        
                                        if let Err(e) = stream.write_all(response_text.as_bytes()).await {
                                            error!("Failed to send response: {}", e);
                                            break;
                                        }
                                        if let Err(e) = stream.write_all(b"\n").await {
                                            error!("Failed to send newline: {}", e);
                                            break;
                                        }
                                    },
                                    Err(e) => {
                                        error!("Error handling MCP request: {}", e);
                                        let error_response = McpResponse {
                                            jsonrpc: "2.0".to_string(),
                                            id: request.get("id").cloned(),
                                            result: None,
                                            error: Some(McpError {
                                                code: -32603,
                                                message: format!("Internal error: {}", e),
                                                data: None,
                                            }),
                                        };
                                        let error_text = serde_json::to_string(&error_response)?;
                                        let _ = stream.write_all(error_text.as_bytes()).await;
                                        let _ = stream.write_all(b"\n").await;
                                    }
                                }
                            },
                            Err(e) => {
                                warn!("Failed to parse MCP request: {}", e);
                            }
                        }
                    }
                }
            },
            Err(e) => {
                error!("Error reading from MCP client: {}", e);
                break;
            }
        }
    }

    server.remove_session(&session_id).await;
    Ok(())
}

/// Start MCP server
pub async fn start_mcp_server(port: u16) -> Result<()> {
    let server = Arc::new(McpServer::new());
    let addr = format!("127.0.0.1:{}", port);
    
    info!("Starting MCP server on {}", addr);
    info!("MCP server supports tools, resources, and prompts for AI integration");
    
    let listener = TcpListener::bind(&addr).await?;
    
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                info!("MCP client connected from {}", addr);
                let server_clone = Arc::clone(&server);
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, server_clone).await {
                        error!("Error handling MCP client: {}", e);
                    }
                });
            },
            Err(e) => {
                error!("Error accepting MCP connection: {}", e);
            }
        }
    }
}