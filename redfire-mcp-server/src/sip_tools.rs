/*
 * SIP Tools Module for Redfire MCP Server
 * Provides AI-accessible SIP protocol operations
 */

use anyhow::Result;
use redfire_sip_stack::utils;
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::{info, debug};
use uuid::Uuid;
use chrono::Utc;

pub struct SipTools {
    // Simple SIP tools without complex parsing
}

impl SipTools {
    pub async fn new() -> Result<Self> {
        info!("Initializing SIP tools");
        
        Ok(Self {
            // No complex components needed for CLI tools
        })
    }
    
    pub async fn parse_sip_message(&self, args: Value) -> Result<Value> {
        let message = args["message"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing message"))?;
        
        let _validate = args["validate"].as_bool().unwrap_or(true);
        
        debug!("Parsing SIP message ({} bytes)", message.len());
        
        // Simple SIP message parsing
        let lines: Vec<&str> = message.lines().collect();
        if lines.is_empty() {
            return Err(anyhow::anyhow!("Empty SIP message"));
        }
        
        let first_line = lines[0];
        let (message_type, method, status_code, request_uri) = self.parse_first_line(first_line)?;
        
        // Parse headers
        let mut headers = HashMap::new();
        let mut body_start = 0;
        
        for (i, line) in lines.iter().enumerate().skip(1) {
            if line.is_empty() {
                body_start = i + 1;
                break;
            }
            
            if let Some(colon_pos) = line.find(':') {
                let header_name = line[..colon_pos].trim();
                let header_value = line[colon_pos + 1..].trim();
                headers.insert(header_name.to_string(), header_value.to_string());
            }
        }
        
        // Extract body
        let body = if body_start < lines.len() {
            lines[body_start..].join("\n")
        } else {
            String::new()
        };
        
        let result = json!({
            "success": true,
            "message_type": message_type,
            "method": method,
            "status_code": status_code,
            "request_uri": request_uri,
            "headers": headers,
            "body": body,
            "sdp": self.parse_sdp_simple(&body),
            "validation": {
                "basic_structure": true,
                "has_required_headers": self.check_required_headers(&headers),
                "valid_uris": self.validate_uris(&headers)
            }
        });
        
        Ok(result)
    }
    
    pub async fn generate_sip_message(&self, args: Value) -> Result<Value> {
        let method = args["method"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing method"))?;
        
        let from_uri = args["from_uri"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing from_uri"))?;
        
        let to_uri = args["to_uri"].as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing to_uri"))?;
        
        let call_id = args["call_id"].as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| utils::generate_call_id());
        
        debug!("Generating {} message from {} to {}", method, from_uri, to_uri);
        
        let from_tag = utils::generate_tag();
        let branch = utils::generate_branch();
        
        // Build SIP message
        let mut message = if method.to_uppercase() == "REGISTER" {
            format!("REGISTER {} SIP/2.0\r\n", to_uri)
        } else {
            format!("{} {} SIP/2.0\r\n", method.to_uppercase(), to_uri)
        };
        
        // Add required headers
        message.push_str(&format!("Via: SIP/2.0/UDP localhost:5060;branch={}\r\n", branch));
        message.push_str(&format!("From: <{}>;tag={}\r\n", from_uri, from_tag));
        message.push_str(&format!("To: <{}>\r\n", to_uri));
        message.push_str(&format!("Call-ID: {}\r\n", call_id));
        message.push_str(&format!("CSeq: 1 {}\r\n", method.to_uppercase()));
        message.push_str("Max-Forwards: 70\r\n");
        message.push_str("User-Agent: Redfire-MCP-Server/0.1.0\r\n");
        message.push_str(&format!("Contact: <sip:redfire@localhost:5060>\r\n"));
        
        // Add SDP for INVITE
        let mut content_length = 0;
        if method.to_uppercase() == "INVITE" {
            if let Some(sdp_config) = args.get("sdp") {
                let sdp = self.generate_sdp(sdp_config)?;
                message.push_str("Content-Type: application/sdp\r\n");
                content_length = sdp.len();
                message.push_str(&format!("Content-Length: {}\r\n\r\n", content_length));
                message.push_str(&sdp);
            } else {
                message.push_str("Content-Length: 0\r\n\r\n");
            }
        } else {
            message.push_str("Content-Length: 0\r\n\r\n");
        }
        
        Ok(json!({
            "success": true,
            "message": message,
            "call_id": call_id,
            "message_length": message.len(),
            "has_sdp": content_length > 0,
            "method": method.to_uppercase()
        }))
    }
    
    pub async fn validate_sip_headers(&self, args: Value) -> Result<Value> {
        let headers_obj = args["headers"].as_object()
            .ok_or_else(|| anyhow::anyhow!("Missing or invalid headers"))?;
        
        debug!("Validating {} SIP headers", headers_obj.len());
        
        let mut headers = HashMap::new();
        for (key, value) in headers_obj {
            if let Some(val_str) = value.as_str() {
                headers.insert(key.clone(), val_str.to_string());
            }
        }
        
        let mut header_details = json!({});
        for (header, value) in &headers {
            header_details[header] = json!({
                "value": value,
                "required": self.is_required_header(header),
                "format_valid": self.validate_header_format(header, value)
            });
        }
        
        let missing_required = self.get_missing_required_headers(&headers);
        let has_required = missing_required.is_empty();
        
        Ok(json!({
            "success": true,
            "overall_valid": has_required,
            "headers_count": headers.len(),
            "header_details": header_details,
            "missing_required": missing_required,
            "validation_summary": {
                "has_all_required": has_required,
                "valid_structure": true
            }
        }))
    }
    
    // Helper methods
    
    fn parse_first_line(&self, first_line: &str) -> Result<(String, Option<String>, Option<u16>, Option<String>)> {
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        
        if parts.len() < 3 {
            return Err(anyhow::anyhow!("Invalid SIP first line: {}", first_line));
        }
        
        if first_line.starts_with("SIP/2.0") {
            // Response line: SIP/2.0 200 OK
            let status_code = parts[1].parse::<u16>()
                .map_err(|_| anyhow::anyhow!("Invalid status code: {}", parts[1]))?;
            Ok(("response".to_string(), None, Some(status_code), None))
        } else {
            // Request line: INVITE sip:alice@example.com SIP/2.0
            let method = parts[0].to_string();
            let request_uri = parts[1].to_string();
            Ok(("request".to_string(), Some(method), None, Some(request_uri)))
        }
    }
    
    fn parse_sdp_simple(&self, body: &str) -> Option<Value> {
        if !body.starts_with("v=") {
            return None;
        }
        
        let mut session_name = String::new();
        let mut connection_info = String::new();
        let mut media_descriptions = Vec::new();
        
        for line in body.lines() {
            if line.starts_with("s=") {
                session_name = line[2..].to_string();
            } else if line.starts_with("c=") {
                connection_info = line[2..].to_string();
            } else if line.starts_with("m=") {
                let parts: Vec<&str> = line[2..].split_whitespace().collect();
                if parts.len() >= 4 {
                    media_descriptions.push(json!({
                        "media": parts[0],
                        "port": parts[1],
                        "protocol": parts[2],
                        "formats": parts[3..].join(" ")
                    }));
                }
            }
        }
        
        Some(json!({
            "session_name": session_name,
            "connection_info": connection_info,
            "media_descriptions": media_descriptions
        }))
    }
    
    fn generate_sdp(&self, config: &Value) -> Result<String> {
        let codecs = config["codecs"].as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
            .unwrap_or_else(|| vec!["PCMU", "PCMA"]);
        
        let session_id = Utc::now().timestamp();
        let mut sdp = format!(
            "v=0\r\n\
             o=redfire-mcp {} {} IN IP4 127.0.0.1\r\n\
             s=Redfire MCP Audio Call\r\n\
             c=IN IP4 127.0.0.1\r\n\
             t=0 0\r\n",
            session_id, session_id
        );
        
        // Add audio media description
        let mut payload_types = Vec::new();
        let mut rtpmap_lines = Vec::new();
        
        for (i, codec) in codecs.iter().enumerate() {
            let pt = if *codec == "PCMU" { 0 } else if *codec == "PCMA" { 8 } else { 96 + i };
            payload_types.push(pt.to_string());
            
            let rtpmap = match *codec {
                "PCMU" => "a=rtpmap:0 PCMU/8000\r\n".to_string(),
                "PCMA" => "a=rtpmap:8 PCMA/8000\r\n".to_string(),
                "G729" => format!("a=rtpmap:{} G729/8000\r\n", pt),
                "G722" => format!("a=rtpmap:{} G722/8000\r\n", pt),
                "OPUS" => format!("a=rtpmap:{} opus/48000/2\r\n", pt),
                _ => format!("a=rtpmap:{} {}/8000\r\n", pt, codec),
            };
            rtpmap_lines.push(rtpmap);
        }
        
        sdp.push_str(&format!(
            "m=audio 5004 RTP/AVP {}\r\n",
            payload_types.join(" ")
        ));
        
        for rtpmap in rtpmap_lines {
            sdp.push_str(&rtpmap);
        }
        
        sdp.push_str("a=sendrecv\r\n");
        
        Ok(sdp)
    }
    
    fn check_required_headers(&self, headers: &HashMap<String, String>) -> bool {
        let required = ["Via", "From", "To", "Call-ID", "CSeq"];
        required.iter().all(|&header| 
            headers.contains_key(header) || 
            headers.keys().any(|k| k.to_lowercase() == header.to_lowercase())
        )
    }
    
    fn validate_uris(&self, headers: &HashMap<String, String>) -> bool {
        for (header, value) in headers {
            if header.to_lowercase() == "from" || header.to_lowercase() == "to" || header.to_lowercase() == "contact" {
                // Look for URIs in angle brackets or raw
                let uri = if value.contains('<') && value.contains('>') {
                    if let Some(start) = value.find('<') {
                        if let Some(end) = value.find('>') {
                            &value[start + 1..end]
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    }
                } else {
                    value.split_whitespace().next().unwrap_or(value)
                };
                
                if !utils::validate_sip_uri(uri) {
                    return false;
                }
            }
        }
        true
    }
    
    fn is_required_header(&self, header: &str) -> bool {
        matches!(header.to_lowercase().as_str(), 
                "from" | "to" | "call-id" | "cseq" | "via" | "max-forwards" | "content-length")
    }
    
    fn validate_header_format(&self, header: &str, value: &str) -> bool {
        match header.to_lowercase().as_str() {
            "content-length" => value.parse::<usize>().is_ok(),
            "max-forwards" => value.parse::<u8>().is_ok(),
            "cseq" => {
                let parts: Vec<&str> = value.split_whitespace().collect();
                parts.len() == 2 && parts[0].parse::<u32>().is_ok()
            }
            "from" | "to" | "contact" => {
                // Basic URI validation
                value.contains("sip:") || value.contains("sips:")
            }
            _ => true  // Other headers are assumed valid
        }
    }
    
    fn get_missing_required_headers(&self, headers: &HashMap<String, String>) -> Vec<String> {
        let required = ["Via", "From", "To", "Call-ID", "CSeq", "Max-Forwards", "Content-Length"];
        let mut missing = Vec::new();
        
        for &req_header in &required {
            if !headers.contains_key(req_header) && 
               !headers.keys().any(|k| k.to_lowercase() == req_header.to_lowercase()) {
                missing.push(req_header.to_string());
            }
        }
        
        missing
    }
}