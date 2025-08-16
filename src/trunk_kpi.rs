/*
 * Trunk-level KPI monitoring for SIP switches
 * 
 * Monitors key performance indicators (KPIs) at the trunk level including:
 * - ACD (Average Call Duration)
 * - ASR (Answer-Seizure Ratio)
 * - PDD (Post Dial Delay)
 * - CCR (Call Completion Ratio)
 * - FAS (False Answer Supervision) detection
 * - Call volume metrics
 * - Quality metrics
 */

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, info, warn, error};
use anyhow::Result;

/// Time window for KPI calculations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeWindow {
    OneMinute,
    FiveMinutes,
    FifteenMinutes,
}

impl TimeWindow {
    pub fn duration(&self) -> Duration {
        match self {
            TimeWindow::OneMinute => Duration::from_secs(60),
            TimeWindow::FiveMinutes => Duration::from_secs(300),
            TimeWindow::FifteenMinutes => Duration::from_secs(900),
        }
    }
    
    pub fn as_str(&self) -> &'static str {
        match self {
            TimeWindow::OneMinute => "1min",
            TimeWindow::FiveMinutes => "5min",
            TimeWindow::FifteenMinutes => "15min",
        }
    }
}

/// Call event for KPI tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallEvent {
    pub call_id: String,
    pub trunk_id: String,
    pub direction: CallDirection,
    pub event_type: CallEventType,
    pub timestamp: SystemTime,
    pub from_number: String,
    pub to_number: String,
    pub sip_response_code: Option<u16>,
    pub hangup_cause: Option<String>,
    pub duration: Option<Duration>,
    pub pdd: Option<Duration>, // Post Dial Delay
    pub media_quality: Option<MediaQuality>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CallDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CallEventType {
    Attempt,      // Call attempt (INVITE sent/received)
    Answer,       // Call answered (200 OK)
    Hangup,       // Call ended (BYE)
    Reject,       // Call rejected (4xx, 5xx, 6xx)
    Timeout,      // Call timeout
    Cancel,       // Call cancelled
    FasDetected,  // False Answer Supervision detected
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaQuality {
    pub mos_score: Option<f32>,        // Mean Opinion Score
    pub packet_loss: Option<f32>,      // Packet loss percentage
    pub jitter: Option<f32>,          // Jitter in ms
    pub rtt: Option<f32>,             // Round-trip time in ms
}

/// Trunk KPIs for a specific time window
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkKPIs {
    pub trunk_id: String,
    pub window: TimeWindow,
    pub window_start: SystemTime,
    pub window_end: SystemTime,
    
    // Call volume metrics
    pub total_attempts: u32,
    pub total_completions: u32,
    pub total_answers: u32,
    pub total_failures: u32,
    
    // Quality metrics
    pub asr: f32,                    // Answer-Seizure Ratio (%)
    pub ccr: f32,                    // Call Completion Ratio (%)
    pub acd: Duration,               // Average Call Duration
    pub pdd_avg: Duration,           // Average Post Dial Delay
    pub pdd_max: Duration,           // Maximum Post Dial Delay
    
    // Response code distribution
    pub response_codes: HashMap<u16, u32>,
    
    // Direction breakdown
    pub inbound_attempts: u32,
    pub inbound_answers: u32,
    pub outbound_attempts: u32,
    pub outbound_answers: u32,
    
    // Quality metrics
    pub avg_mos: Option<f32>,
    pub avg_packet_loss: Option<f32>,
    pub avg_jitter: Option<f32>,
    pub avg_rtt: Option<f32>,
    
    // FAS detection
    pub fas_detected_count: u32,
    pub fas_percentage: f32,
    
    // Billing impact
    pub total_duration: Duration,
    pub billable_duration: Duration,
}

impl Default for TrunkKPIs {
    fn default() -> Self {
        Self {
            trunk_id: String::new(),
            window: TimeWindow::OneMinute,
            window_start: UNIX_EPOCH,
            window_end: UNIX_EPOCH,
            total_attempts: 0,
            total_completions: 0,
            total_answers: 0,
            total_failures: 0,
            asr: 0.0,
            ccr: 0.0,
            acd: Duration::from_secs(0),
            pdd_avg: Duration::from_secs(0),
            pdd_max: Duration::from_secs(0),
            response_codes: HashMap::new(),
            inbound_attempts: 0,
            inbound_answers: 0,
            outbound_attempts: 0,
            outbound_answers: 0,
            avg_mos: None,
            avg_packet_loss: None,
            avg_jitter: None,
            avg_rtt: None,
            fas_detected_count: 0,
            fas_percentage: 0.0,
            total_duration: Duration::from_secs(0),
            billable_duration: Duration::from_secs(0),
        }
    }
}

/// FAS detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FasDetectionConfig {
    pub enabled: bool,
    /// Minimum call duration to consider for FAS (in milliseconds)
    pub min_duration_ms: u64,
    /// Maximum call duration for FAS detection (in milliseconds)
    pub max_duration_ms: u64,
    /// Percentage of short calls that triggers FAS alarm
    pub threshold_percentage: f32,
    /// Number of calls in window to evaluate FAS
    pub min_sample_size: u32,
}

impl Default for FasDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_duration_ms: 1000,    // 1 second minimum
            max_duration_ms: 10000,   // 10 seconds maximum for FAS
            threshold_percentage: 15.0, // 15% of calls being short = FAS
            min_sample_size: 10,      // Need at least 10 calls to evaluate
        }
    }
}

/// Trunk KPI monitor
pub struct TrunkKpiMonitor {
    config: FasDetectionConfig,
    // Store call events for each time window
    events: Arc<RwLock<HashMap<String, Vec<CallEvent>>>>, // trunk_id -> events
    // Calculated KPIs for each trunk and time window
    kpis: Arc<RwLock<HashMap<(String, TimeWindow), TrunkKPIs>>>, // (trunk_id, window) -> KPIs
    running: Arc<RwLock<bool>>,
}

impl TrunkKpiMonitor {
    pub fn new(config: FasDetectionConfig) -> Self {
        Self {
            config,
            events: Arc::new(RwLock::new(HashMap::new())),
            kpis: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Record a call event
    pub async fn record_call_event(&self, event: CallEvent) -> Result<()> {
        let mut events = self.events.write().await;
        let trunk_events = events.entry(event.trunk_id.clone()).or_insert_with(Vec::new);
        trunk_events.push(event);
        
        // Keep only events within the maximum time window (15 minutes + buffer)
        let cutoff_time = SystemTime::now() - Duration::from_secs(1200); // 20 minutes buffer
        trunk_events.retain(|e| e.timestamp > cutoff_time);
        
        Ok(())
    }
    
    /// Calculate KPIs for a specific trunk and time window
    pub async fn calculate_kpis(&self, trunk_id: &str, window: TimeWindow) -> Result<TrunkKPIs> {
        let events = self.events.read().await;
        let trunk_events = events.get(trunk_id).unwrap_or(&Vec::new());
        
        let window_start = SystemTime::now() - window.duration();
        let window_end = SystemTime::now();
        
        // Filter events to the time window
        let window_events: Vec<&CallEvent> = trunk_events
            .iter()
            .filter(|e| e.timestamp >= window_start && e.timestamp <= window_end)
            .collect();
        
        let mut kpis = TrunkKPIs {
            trunk_id: trunk_id.to_string(),
            window,
            window_start,
            window_end,
            ..Default::default()
        };
        
        // Group events by call_id to track call lifecycle
        let mut call_lifecycle: HashMap<String, Vec<&CallEvent>> = HashMap::new();
        for event in &window_events {
            call_lifecycle.entry(event.call_id.clone()).or_insert_with(Vec::new).push(event);
        }
        
        // Calculate metrics
        let mut total_call_duration = Duration::from_secs(0);
        let mut total_billable_duration = Duration::from_secs(0);
        let mut total_pdd = Duration::from_secs(0);
        let mut pdd_count = 0;
        let mut quality_scores = Vec::new();
        let mut short_calls = 0; // For FAS detection
        
        for (call_id, call_events) in call_lifecycle {
            let mut has_attempt = false;
            let mut has_answer = false;
            let mut has_completion = false;
            let mut call_duration = None;
            let mut pdd = None;
            let mut direction = None;
            let mut response_code = None;
            let mut is_fas = false;
            
            // Analyze call lifecycle
            for event in &call_events {
                direction = Some(event.direction.clone());
                
                match event.event_type {
                    CallEventType::Attempt => {
                        has_attempt = true;
                        kpis.total_attempts += 1;
                        match event.direction {
                            CallDirection::Inbound => kpis.inbound_attempts += 1,
                            CallDirection::Outbound => kpis.outbound_attempts += 1,
                        }
                    }
                    CallEventType::Answer => {
                        has_answer = true;
                        kpis.total_answers += 1;
                        match event.direction {
                            CallDirection::Inbound => kpis.inbound_answers += 1,
                            CallDirection::Outbound => kpis.outbound_answers += 1,
                        }
                        
                        // Calculate PDD (time from attempt to answer)
                        if let Some(attempt_event) = call_events.iter().find(|e| matches!(e.event_type, CallEventType::Attempt)) {
                            if let Ok(pdd_duration) = event.timestamp.duration_since(attempt_event.timestamp) {
                                pdd = Some(pdd_duration);
                                total_pdd += pdd_duration;
                                pdd_count += 1;
                                
                                if pdd_duration > kpis.pdd_max {
                                    kpis.pdd_max = pdd_duration;
                                }
                            }
                        }
                    }
                    CallEventType::Hangup => {
                        has_completion = true;
                        kpis.total_completions += 1;
                        
                        if let Some(duration) = event.duration {
                            call_duration = Some(duration);
                            total_call_duration += duration;
                            
                            // Check for potential FAS
                            if self.config.enabled {
                                let duration_ms = duration.as_millis() as u64;
                                if duration_ms >= self.config.min_duration_ms && 
                                   duration_ms <= self.config.max_duration_ms {
                                    short_calls += 1;
                                }
                            }
                            
                            // Billable duration (exclude very short calls)
                            if duration.as_secs() >= 1 {
                                total_billable_duration += duration;
                            }
                        }
                    }
                    CallEventType::Reject => {
                        kpis.total_failures += 1;
                        if let Some(code) = event.sip_response_code {
                            response_code = Some(code);
                            *kpis.response_codes.entry(code).or_insert(0) += 1;
                        }
                    }
                    CallEventType::FasDetected => {
                        is_fas = true;
                        kpis.fas_detected_count += 1;
                    }
                    _ => {}
                }
                
                // Collect quality metrics
                if let Some(quality) = &event.media_quality {
                    quality_scores.push(quality.clone());
                }
            }
        }
        
        // Calculate derived metrics
        if kpis.total_attempts > 0 {
            kpis.asr = (kpis.total_answers as f32 / kpis.total_attempts as f32) * 100.0;
            kpis.ccr = (kpis.total_completions as f32 / kpis.total_attempts as f32) * 100.0;
        }
        
        if kpis.total_answers > 0 {
            kpis.acd = total_call_duration / kpis.total_answers;
        }
        
        if pdd_count > 0 {
            kpis.pdd_avg = total_pdd / pdd_count;
        }
        
        // Calculate quality averages
        if !quality_scores.is_empty() {
            let mos_scores: Vec<f32> = quality_scores.iter().filter_map(|q| q.mos_score).collect();
            if !mos_scores.is_empty() {
                kpis.avg_mos = Some(mos_scores.iter().sum::<f32>() / mos_scores.len() as f32);
            }
            
            let packet_losses: Vec<f32> = quality_scores.iter().filter_map(|q| q.packet_loss).collect();
            if !packet_losses.is_empty() {
                kpis.avg_packet_loss = Some(packet_losses.iter().sum::<f32>() / packet_losses.len() as f32);
            }
            
            let jitters: Vec<f32> = quality_scores.iter().filter_map(|q| q.jitter).collect();
            if !jitters.is_empty() {
                kpis.avg_jitter = Some(jitters.iter().sum::<f32>() / jitters.len() as f32);
            }
            
            let rtts: Vec<f32> = quality_scores.iter().filter_map(|q| q.rtt).collect();
            if !rtts.is_empty() {
                kpis.avg_rtt = Some(rtts.iter().sum::<f32>() / rtts.len() as f32);
            }
        }
        
        // FAS detection
        if self.config.enabled && kpis.total_answers >= self.config.min_sample_size {
            kpis.fas_percentage = (short_calls as f32 / kpis.total_answers as f32) * 100.0;
            
            if kpis.fas_percentage > self.config.threshold_percentage {
                warn!("🚨 FAS detected on trunk '{}': {:.1}% of calls are short duration", 
                      trunk_id, kpis.fas_percentage);
                
                // Record FAS detection event
                let fas_event = CallEvent {
                    call_id: format!("fas-detection-{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()),
                    trunk_id: trunk_id.to_string(),
                    direction: CallDirection::Outbound, // FAS typically affects outbound
                    event_type: CallEventType::FasDetected,
                    timestamp: SystemTime::now(),
                    from_number: "system".to_string(),
                    to_number: "fas-detection".to_string(),
                    sip_response_code: None,
                    hangup_cause: Some("FAS_DETECTED".to_string()),
                    duration: None,
                    pdd: None,
                    media_quality: None,
                };
                
                // Record the FAS event
                if let Err(e) = self.record_call_event(fas_event).await {
                    error!("Failed to record FAS detection event: {}", e);
                }
            }
        }
        
        kpis.total_duration = total_call_duration;
        kpis.billable_duration = total_billable_duration;
        
        // Store calculated KPIs
        {
            let mut kpi_store = self.kpis.write().await;
            kpi_store.insert((trunk_id.to_string(), window), kpis.clone());
        }
        
        Ok(kpis)
    }
    
    /// Get stored KPIs for a trunk and time window
    pub async fn get_kpis(&self, trunk_id: &str, window: TimeWindow) -> Option<TrunkKPIs> {
        let kpis = self.kpis.read().await;
        kpis.get(&(trunk_id.to_string(), window)).cloned()
    }
    
    /// Get KPIs for all trunks and time windows
    pub async fn get_all_kpis(&self) -> HashMap<(String, TimeWindow), TrunkKPIs> {
        let kpis = self.kpis.read().await;
        kpis.clone()
    }
    
    /// Start periodic KPI calculation
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }
        
        *running = true;
        info!("Starting trunk KPI monitoring");
        
        let monitor = Arc::new(self.clone());
        
        // Calculate KPIs every 10 seconds
        tokio::spawn(async move {
            let mut interval_timer = interval(Duration::from_secs(10));
            
            loop {
                interval_timer.tick().await;
                
                let running = monitor.running.read().await;
                if !*running {
                    break;
                }
                drop(running);
                
                // Get list of active trunks
                let events = monitor.events.read().await;
                let trunk_ids: Vec<String> = events.keys().cloned().collect();
                drop(events);
                
                // Calculate KPIs for each trunk and time window
                for trunk_id in trunk_ids {
                    for &window in &[TimeWindow::OneMinute, TimeWindow::FiveMinutes, TimeWindow::FifteenMinutes] {
                        if let Err(e) = monitor.calculate_kpis(&trunk_id, window).await {
                            error!("Failed to calculate KPIs for trunk {}: {}", trunk_id, e);
                        }
                    }
                }
            }
            
            debug!("Trunk KPI monitoring stopped");
        });
        
        Ok(())
    }
    
    /// Stop KPI monitoring
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        info!("Trunk KPI monitoring stopped");
        Ok(())
    }
    
    /// Get trunk performance summary
    pub async fn get_trunk_summary(&self, trunk_id: &str) -> Result<TrunkSummary> {
        let kpis_1min = self.get_kpis(trunk_id, TimeWindow::OneMinute).await;
        let kpis_5min = self.get_kpis(trunk_id, TimeWindow::FiveMinutes).await;
        let kpis_15min = self.get_kpis(trunk_id, TimeWindow::FifteenMinutes).await;
        
        Ok(TrunkSummary {
            trunk_id: trunk_id.to_string(),
            kpis_1min,
            kpis_5min,
            kpis_15min,
            timestamp: SystemTime::now(),
        })
    }
}

// Custom Clone implementation
impl Clone for TrunkKpiMonitor {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            events: self.events.clone(),
            kpis: self.kpis.clone(),
            running: self.running.clone(),
        }
    }
}

/// Trunk performance summary across all time windows
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrunkSummary {
    pub trunk_id: String,
    pub kpis_1min: Option<TrunkKPIs>,
    pub kpis_5min: Option<TrunkKPIs>,
    pub kpis_15min: Option<TrunkKPIs>,
    pub timestamp: SystemTime,
}

/// Create a new trunk KPI monitor
pub fn create_trunk_kpi_monitor(config: FasDetectionConfig) -> Arc<TrunkKpiMonitor> {
    Arc::new(TrunkKpiMonitor::new(config))
}