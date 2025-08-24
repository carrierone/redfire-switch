/*
 * Complete ISDN PRI Stack Manager
 *
 * Integrates all ISDN components into a unified stack:
 * - Q.921 LAPD data link layer
 * - Q.931 network layer message handling
 * - PRI timer management
 * - CESoPSN circuit emulation
 * - Event coordination between layers
 * - Health monitoring and diagnostics
 */

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::cesopsn_ni2_integration::{
    CesopsnNi2CircuitConfig, CesopsnNi2Event, CesopsnNi2Integration,
};
use crate::pri_timers::{
    clear_call_action, send_message_action, ActivePriTimer, PriTimerAction, PriTimerExpiredEvent,
    PriTimerManager, PriTimerType,
};
use crate::q921_lapd::{LapdEvent, LapdStatistics, Q921LapdManager};
use crate::q931_messages::{
    CauseValue, IsdnConfig, IsdnSideType, IsdnVariant, Q931Message, Q931MessageType,
};
// use crate::isdn_cli::{IsdnStackStatus, CircuitStatus, HealthStatus, LapdStatusInfo, Q931StatusInfo, TimerStatusInfo, CesopsnStatusInfo};

// Define the missing types locally until isdn_cli is available
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsdnStackStatus {
    pub circuits: Vec<CircuitStatus>,
    pub overall_health: HealthStatus,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitStatus {
    pub circuit_id: u16,
    pub description: String,
    pub variant: IsdnVariant,
    pub side_type: IsdnSideType,
    pub pcm_codec: crate::cesopsn_ni2_integration::PcmCodec,
    pub lapd_status: LapdStatusInfo,
    pub q931_status: Q931StatusInfo,
    pub timer_status: TimerStatusInfo,
    pub cesopsn_status: CesopsnStatusInfo,
    pub health: HealthStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Warning,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LapdStatusInfo {
    pub connections: u32,
    pub established: u32,
    pub state_summary: HashMap<String, u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Q931StatusInfo {
    pub active_calls: u32,
    pub call_states: HashMap<String, u32>,
    pub messages_sent: u64,
    pub messages_received: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerStatusInfo {
    pub active_timers: u32,
    pub timer_types: HashMap<String, u32>,
    pub expired_timers: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CesopsnStatusInfo {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub jitter_ms: f32,
    pub loss_rate: f32,
}
use crate::memory_safety::MemoryTracker;

/// ISDN Call State per ITU-T Q.931
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsdnCallState {
    Null = 0,
    CallInitiated = 1,
    OverlapSending = 2,
    OutgoingCallProceeding = 3,
    CallDelivered = 4,
    CallPresent = 6,
    CallReceived = 7,
    ConnectRequest = 8,
    IncomingCallProceeding = 9,
    Active = 10,
    DisconnectRequest = 11,
    DisconnectIndication = 12,
    SuspendRequest = 15,
    ResumeRequest = 17,
    ReleaseRequest = 19,
    CallAbort = 22,
    OverlapReceiving = 25,
    RestartRequest = 61,
    Restart = 62,
}

/// ISDN Call Context
#[derive(Debug, Clone)]
pub struct IsdnCall {
    /// Call reference value
    pub call_reference: u16,
    /// Circuit ID  
    pub circuit_id: u16,
    /// Current call state
    pub state: IsdnCallState,
    /// Calling party number
    pub calling_number: String,
    /// Called party number
    pub called_number: String,
    /// Call start time
    pub start_time: std::time::Instant,
    /// Last state change time
    pub last_activity: std::time::Instant,
    /// Active timers for this call
    pub active_timers: Vec<String>,
    /// Bearer channel assigned
    pub bearer_channel: Option<u8>,
}

impl IsdnCall {
    pub fn new(call_reference: u16, circuit_id: u16) -> Self {
        let now = std::time::Instant::now();
        Self {
            call_reference,
            circuit_id,
            state: IsdnCallState::Null,
            calling_number: String::new(),
            called_number: String::new(),
            start_time: now,
            last_activity: now,
            active_timers: Vec::new(),
            bearer_channel: None,
        }
    }

    pub fn update_state(&mut self, new_state: IsdnCallState) {
        self.state = new_state;
        self.last_activity = std::time::Instant::now();
    }
}

/// ISDN Circuit Configuration (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsdnCircuitConfig {
    pub description: String,
    pub variant: IsdnVariant,
    pub side_type: IsdnSideType,
    pub pcm_codec: crate::cesopsn_ni2_integration::PcmCodec,
}

/// ISDN Stack Events
#[derive(Debug, Clone)]
pub enum IsdnStackEvent {
    /// Call state changed
    CallStateChanged {
        circuit_id: u16,
        call_reference: u16,
        old_state: IsdnCallState,
        new_state: IsdnCallState,
    },
    /// Call established  
    CallEstablished {
        circuit_id: u16,
        call_reference: u16,
        calling_number: String,
        called_number: String,
        bearer_channel: u8,
    },
    /// Call released
    CallReleased {
        circuit_id: u16,
        call_reference: u16,
        cause: CauseValue,
        duration: Duration,
    },
    /// DTMF detected
    DtmfDetected {
        circuit_id: u16,
        channel: u8,
        digit: char,
        confidence: f32,
    },
    /// Circuit state changed
    CircuitStateChanged {
        circuit_id: u16,
        old_state: String,
        new_state: String,
    },
    /// Stack error
    StackError {
        circuit_id: u16,
        layer: String,
        error: String,
    },
}

/// Complete ISDN PRI Stack Manager
pub struct IsdnStackManager {
    /// Stack configuration
    config: IsdnStackConfig,
    /// Q.921 LAPD managers by circuit
    lapd_managers: Arc<tokio::sync::RwLock<HashMap<u16, Arc<Q921LapdManager>>>>,
    /// PRI timer manager
    timer_manager: Arc<PriTimerManager>,
    /// CESoPSN integration
    cesopsn_integration: Arc<tokio::sync::Mutex<CesopsnNi2Integration>>,
    /// Active calls by circuit and call reference
    active_calls: Arc<tokio::sync::RwLock<HashMap<u16, HashMap<u16, IsdnCall>>>>,
    /// Stack event broadcaster
    event_sender: broadcast::Sender<IsdnStackEvent>,
    /// Health monitoring
    health_monitor: Arc<tokio::sync::RwLock<HashMap<u16, HealthStatus>>>,
    /// Statistics
    statistics: Arc<tokio::sync::RwLock<IsdnStackStatistics>>,
    /// Memory tracker for leak detection
    memory_tracker: MemoryTracker,
}

/// ISDN Stack Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsdnStackConfig {
    /// Stack name/identifier
    pub name: String,
    /// ISDN variant
    pub variant: IsdnVariant,
    /// Side type
    pub side_type: IsdnSideType,
    /// Circuit configurations
    pub circuits: HashMap<u16, CesopsnNi2CircuitConfig>,
    /// Enable health monitoring
    pub enable_health_monitoring: bool,
    /// Health check interval
    pub health_check_interval: Duration,
}

/// ISDN Stack Statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IsdnStackStatistics {
    /// Total calls processed
    pub total_calls: u64,
    /// Currently active calls
    pub active_calls: u64,
    /// Q.931 messages by type
    pub q931_messages: HashMap<String, u64>,
    /// Timer expirations by type
    pub timer_expirations: HashMap<String, u64>,
    /// LAPD frames processed
    pub lapd_frames: u64,
    /// Errors by layer
    pub errors: HashMap<String, u64>,
}

impl IsdnStackManager {
    /// Create new ISDN stack manager
    pub async fn new(config: IsdnStackConfig) -> Result<Self> {
        let timer_manager = Arc::new(PriTimerManager::new(config.variant, config.side_type));
        let cesopsn_integration =
            Arc::new(tokio::sync::Mutex::new(CesopsnNi2Integration::new().await?));
        let (event_sender, _) = broadcast::channel(1000);

        let stack = Self {
            config,
            lapd_managers: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            timer_manager,
            cesopsn_integration,
            active_calls: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            event_sender,
            health_monitor: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            statistics: Arc::new(tokio::sync::RwLock::new(IsdnStackStatistics::default())),
            memory_tracker: MemoryTracker::new(),
        };

        Ok(stack)
    }

    /// Initialize the ISDN stack
    pub async fn initialize(&mut self) -> Result<()> {
        info!("Initializing ISDN PRI Stack: {}", self.config.name);

        // Initialize circuits
        for (&circuit_id, circuit_config) in &self.config.circuits {
            self.initialize_circuit(circuit_id, circuit_config.clone())
                .await?;
        }

        // Start event processing
        self.start_event_processing().await?;

        // Start health monitoring if enabled
        if self.config.enable_health_monitoring {
            self.start_health_monitoring().await;
        }

        info!("ISDN PRI Stack initialized successfully");
        Ok(())
    }

    /// Initialize a single circuit
    async fn initialize_circuit(
        &self,
        circuit_id: u16,
        config: CesopsnNi2CircuitConfig,
    ) -> Result<()> {
        info!(
            "Initializing ISDN circuit {}: {}",
            circuit_id, config.description
        );

        // Add CESoPSN circuit - use safe reference instead of dangerous try_unwrap
        self.cesopsn_integration
            .lock()
            .await
            .add_circuit(config.clone())
            .await?;

        // Create D-channel sender for LAPD
        let (d_channel_sender, mut d_channel_receiver) = mpsc::unbounded_channel();

        // Create LAPD manager
        let lapd_manager = Arc::new(Q921LapdManager::new(
            config.isdn_config.variant,
            config.isdn_config.side_type,
            d_channel_sender,
        ));

        // Establish LAPD connection for signaling
        lapd_manager.establish_connection(0, 0).await?; // SAPI 0, TEI 0

        self.lapd_managers
            .write()
            .await
            .insert(circuit_id, lapd_manager);

        // Initialize call tracking for this circuit
        self.active_calls
            .write()
            .await
            .insert(circuit_id, HashMap::new());

        // Set initial health status
        self.health_monitor
            .write()
            .await
            .insert(circuit_id, HealthStatus::Healthy);

        // Track circuit allocation for memory monitoring
        self.memory_tracker
            .track_allocation(&format!("circuit_{}", circuit_id), 1024)?;

        info!("Circuit {} initialized successfully", circuit_id);
        Ok(())
    }

    /// Start event processing loops
    async fn start_event_processing(&self) -> Result<()> {
        // Process CESoPSN events
        let cesopsn_events = self.cesopsn_integration.lock().await.subscribe_events();
        self.start_cesopsn_event_processing(cesopsn_events).await;

        // Process timer expiration events
        let timer_events = self.timer_manager.subscribe_expiration_events();
        self.start_timer_event_processing(timer_events).await;

        // Process LAPD events from all circuits
        let lapd_managers = self.lapd_managers.read().await;
        for (&circuit_id, lapd_manager) in lapd_managers.iter() {
            let lapd_events = lapd_manager.subscribe_events();
            self.start_lapd_event_processing(circuit_id, lapd_events)
                .await;
        }

        Ok(())
    }

    /// Process CESoPSN events
    async fn start_cesopsn_event_processing(
        &self,
        mut events: broadcast::Receiver<CesopsnNi2Event>,
    ) {
        let event_sender = self.event_sender.clone();
        let statistics = Arc::clone(&self.statistics);

        tokio::spawn(async move {
            async {
                while let Ok(event) = events.recv().await {
                    match event {
                        CesopsnNi2Event::DtmfDetected {
                            circuit_id,
                            channel,
                            digit,
                            duration: _,
                            confidence,
                        } => {
                            let stack_event = IsdnStackEvent::DtmfDetected {
                                circuit_id,
                                channel,
                                digit,
                                confidence,
                            };
                            let _ = event_sender.send(stack_event);
                        }
                        CesopsnNi2Event::CircuitStateChanged {
                            circuit_id,
                            old_state,
                            new_state,
                        } => {
                            let stack_event = IsdnStackEvent::CircuitStateChanged {
                                circuit_id,
                                old_state,
                                new_state,
                            };
                            let _ = event_sender.send(stack_event);
                        }
                        _ => {}
                    }
                }
            }
            .await;
        });
    }

    /// Process timer expiration events
    async fn start_timer_event_processing(
        &self,
        mut events: broadcast::Receiver<PriTimerExpiredEvent>,
    ) {
        let active_calls = Arc::clone(&self.active_calls);
        let statistics = Arc::clone(&self.statistics);
        let event_sender = self.event_sender.clone();

        tokio::spawn(async move {
            async {
                while let Ok(timer_event) = events.recv().await {
                    let timer = timer_event.timer;

                    // Update statistics
                    {
                        let mut stats = statistics.write().await;
                        let timer_name = format!("{:?}", timer.timer_type);
                        *stats.timer_expirations.entry(timer_name).or_insert(0) += 1;
                    }

                    // Handle timer expiration based on action
                    match timer.action {
                        PriTimerAction::ClearCall(cause) => {
                            // Clear the call due to timer expiration
                            {
                                let mut calls_guard = active_calls.write().await;
                                if let Some(calls) = calls_guard.get_mut(&timer.circuit_id) {
                                    if let Some(call) = calls.remove(&timer.call_reference) {
                                        let stack_event = IsdnStackEvent::CallReleased {
                                            circuit_id: timer.circuit_id,
                                            call_reference: timer.call_reference,
                                            cause: match cause {
                                                102 => CauseValue::RecoveryOnTimerExpiry,
                                                19 => CauseValue::NoAnswer,
                                                _ => CauseValue::NormalUnspecified,
                                            },
                                            duration: call.start_time.elapsed(),
                                        };
                                        let _ = event_sender.send(stack_event);
                                    }
                                }
                            }
                        }
                        PriTimerAction::SendMessage(message_type) => {
                            info!(
                                "Timer {} expired - should send {:?} message",
                                timer.timer_type.description(),
                                message_type
                            );
                            // In full implementation, would send Q.931 message
                        }
                        _ => {
                            warn!("Unhandled timer expiration: {:?}", timer.timer_type);
                        }
                    }
                }
            }
            .await;
        });
    }

    /// Process LAPD events for a circuit
    async fn start_lapd_event_processing(
        &self,
        circuit_id: u16,
        mut events: broadcast::Receiver<LapdEvent>,
    ) {
        let active_calls = Arc::clone(&self.active_calls);
        let timer_manager = Arc::clone(&self.timer_manager);
        let event_sender = self.event_sender.clone();
        let statistics = Arc::clone(&self.statistics);

        tokio::spawn(async move {
            async {
                while let Ok(lapd_event) = events.recv().await {
                    match lapd_event {
                        LapdEvent::DataReceived {
                            sapi: _,
                            tei: _,
                            data,
                        } => {
                            // Parse Q.931 message
                            match Q931Message::parse(&data) {
                                Ok(message) => {
                                    // Update statistics
                                    {
                                        let mut stats = statistics.write().await;
                                        let msg_name = format!("{:?}", message.message_type);
                                        *stats.q931_messages.entry(msg_name).or_insert(0) += 1;
                                    }

                                    // Process Q.931 message
                                    if let Err(e) = Self::process_q931_message(
                                        circuit_id,
                                        message,
                                        &active_calls,
                                        &timer_manager,
                                        &event_sender,
                                    )
                                    .await
                                    {
                                        warn!("Error processing Q.931 message: {}", e);
                                    }
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to parse Q.931 message on circuit {}: {}",
                                        circuit_id, e
                                    );
                                }
                            }

                            // Update LAPD frame statistics
                            {
                                let mut stats = statistics.write().await;
                                stats.lapd_frames += 1;
                            }
                        }
                        LapdEvent::LinkEstablished { sapi, tei } => {
                            info!(
                                "LAPD link established on circuit {}: SAPI {} TEI {}",
                                circuit_id, sapi, tei
                            );
                        }
                        LapdEvent::LinkReleased { sapi, tei } => {
                            info!(
                                "LAPD link released on circuit {}: SAPI {} TEI {}",
                                circuit_id, sapi, tei
                            );
                        }
                        LapdEvent::Error { sapi, tei, error } => {
                            warn!(
                                "LAPD error on circuit {} SAPI {} TEI {}: {}",
                                circuit_id, sapi, tei, error
                            );

                            let stack_event = IsdnStackEvent::StackError {
                                circuit_id,
                                layer: "Q.921 LAPD".to_string(),
                                error,
                            };
                            let _ = event_sender.send(stack_event);
                        }
                    }
                }
            }
            .await;
        });
    }

    /// Process Q.931 message and update call state
    async fn process_q931_message(
        circuit_id: u16,
        message: Q931Message,
        active_calls: &Arc<tokio::sync::RwLock<HashMap<u16, HashMap<u16, IsdnCall>>>>,
        timer_manager: &Arc<PriTimerManager>,
        event_sender: &broadcast::Sender<IsdnStackEvent>,
    ) -> Result<()> {
        let call_ref = message.call_reference.value;

        debug!(
            "Processing Q.931 {:?} message for CRV {} on circuit {}",
            message.message_type, call_ref, circuit_id
        );

        let mut calls = active_calls.write().await;
        let circuit_calls = calls.entry(circuit_id).or_insert_with(HashMap::new);

        match message.message_type {
            Q931MessageType::Setup => {
                // Incoming call setup
                let mut call = IsdnCall::new(call_ref, circuit_id);
                call.update_state(IsdnCallState::CallPresent);

                // Extract calling/called numbers from IEs
                if let Some(calling_ie) = message
                    .find_ie(crate::q931_messages::InformationElementType::CallingPartyNumber)
                {
                    // Parse calling party number
                    call.calling_number = String::from_utf8_lossy(&calling_ie.data).to_string();
                }
                if let Some(called_ie) =
                    message.find_ie(crate::q931_messages::InformationElementType::CalledPartyNumber)
                {
                    // Parse called party number
                    call.called_number = String::from_utf8_lossy(&called_ie.data).to_string();
                }

                // Start T303 timer for setup response
                if let Ok(timer_id) = timer_manager
                    .start_timer(
                        PriTimerType::T303,
                        call_ref,
                        circuit_id,
                        None,
                        clear_call_action(102), // Recovery on timer expiry
                    )
                    .await
                {
                    call.active_timers.push(timer_id);
                }

                circuit_calls.insert(call_ref, call.clone());

                // Send call state change event
                let stack_event = IsdnStackEvent::CallStateChanged {
                    circuit_id,
                    call_reference: call_ref,
                    old_state: IsdnCallState::Null,
                    new_state: IsdnCallState::CallPresent,
                };
                let _ = event_sender.send(stack_event);
            }
            Q931MessageType::CallProceeding => {
                if let Some(call) = circuit_calls.get_mut(&call_ref) {
                    let old_state = call.state;
                    call.update_state(IsdnCallState::IncomingCallProceeding);

                    // Cancel T303, start T310
                    timer_manager.cancel_call_timers(call_ref).await;
                    if let Ok(timer_id) = timer_manager
                        .start_timer(
                            PriTimerType::T310,
                            call_ref,
                            circuit_id,
                            None,
                            clear_call_action(102),
                        )
                        .await
                    {
                        call.active_timers.push(timer_id);
                    }

                    let stack_event = IsdnStackEvent::CallStateChanged {
                        circuit_id,
                        call_reference: call_ref,
                        old_state,
                        new_state: call.state,
                    };
                    let _ = event_sender.send(stack_event);
                }
            }
            Q931MessageType::Alerting => {
                if let Some(call) = circuit_calls.get_mut(&call_ref) {
                    let old_state = call.state;
                    call.update_state(IsdnCallState::CallDelivered);

                    // Start T301 alerting timer
                    if let Ok(timer_id) = timer_manager
                        .start_timer(
                            PriTimerType::T301,
                            call_ref,
                            circuit_id,
                            None,
                            clear_call_action(19), // No answer
                        )
                        .await
                    {
                        call.active_timers.push(timer_id);
                    }

                    let stack_event = IsdnStackEvent::CallStateChanged {
                        circuit_id,
                        call_reference: call_ref,
                        old_state,
                        new_state: call.state,
                    };
                    let _ = event_sender.send(stack_event);
                }
            }
            Q931MessageType::Connect => {
                if let Some(call) = circuit_calls.get_mut(&call_ref) {
                    let old_state = call.state;
                    call.update_state(IsdnCallState::Active);

                    // Cancel all timers, call is now active
                    timer_manager.cancel_call_timers(call_ref).await;
                    call.active_timers.clear();

                    let stack_event = IsdnStackEvent::CallEstablished {
                        circuit_id,
                        call_reference: call_ref,
                        calling_number: call.calling_number.clone(),
                        called_number: call.called_number.clone(),
                        bearer_channel: call.bearer_channel.unwrap_or(1),
                    };
                    let _ = event_sender.send(stack_event);
                }
            }
            Q931MessageType::Disconnect => {
                if let Some(call) = circuit_calls.get_mut(&call_ref) {
                    call.update_state(IsdnCallState::DisconnectIndication);

                    // Start T305 disconnect timer
                    if let Ok(timer_id) = timer_manager
                        .start_timer(
                            PriTimerType::T305,
                            call_ref,
                            circuit_id,
                            None,
                            send_message_action(Q931MessageType::Release),
                        )
                        .await
                    {
                        call.active_timers.push(timer_id);
                    }
                }
            }
            Q931MessageType::Release => {
                if let Some(call) = circuit_calls.remove(&call_ref) {
                    timer_manager.cancel_call_timers(call_ref).await;

                    let stack_event = IsdnStackEvent::CallReleased {
                        circuit_id,
                        call_reference: call_ref,
                        cause: CauseValue::Normal,
                        duration: call.start_time.elapsed(),
                    };
                    let _ = event_sender.send(stack_event);
                }
            }
            Q931MessageType::ReleaseComplete => {
                if let Some(call) = circuit_calls.remove(&call_ref) {
                    timer_manager.cancel_call_timers(call_ref).await;

                    let stack_event = IsdnStackEvent::CallReleased {
                        circuit_id,
                        call_reference: call_ref,
                        cause: CauseValue::Normal,
                        duration: call.start_time.elapsed(),
                    };
                    let _ = event_sender.send(stack_event);
                }
            }
            _ => {
                debug!("Unhandled Q.931 message type: {:?}", message.message_type);
            }
        }

        Ok(())
    }

    /// Start health monitoring
    async fn start_health_monitoring(&self) {
        let health_monitor = Arc::clone(&self.health_monitor);
        let statistics = Arc::clone(&self.statistics);
        let lapd_managers = Arc::clone(&self.lapd_managers);
        let cesopsn_integration = Arc::clone(&self.cesopsn_integration);
        let timer_manager = Arc::clone(&self.timer_manager);
        let interval_duration = self.config.health_check_interval;

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            loop {
                interval.tick().await;

                // Check each circuit's health
                {
                    let managers = lapd_managers.read().await;
                    for (&circuit_id, _) in managers.iter() {
                        let health = Self::assess_circuit_health(
                            circuit_id,
                            &statistics,
                            &cesopsn_integration,
                            &timer_manager,
                        )
                        .await;

                        {
                            let mut monitor = health_monitor.write().await;
                            monitor.insert(circuit_id, health);
                        }
                    }
                }
            }
        });
    }

    /// Assess circuit health status
    async fn assess_circuit_health(
        circuit_id: u16,
        statistics: &Arc<tokio::sync::RwLock<IsdnStackStatistics>>,
        cesopsn_integration: &Arc<tokio::sync::Mutex<CesopsnNi2Integration>>,
        timer_manager: &Arc<PriTimerManager>,
    ) -> HealthStatus {
        // Get circuit statistics
        let circuit_stats = match cesopsn_integration
            .lock()
            .await
            .get_circuit_stats(circuit_id)
            .await
        {
            Ok(stats) => stats,
            Err(_) => return HealthStatus::Critical,
        };

        // Check packet loss rate
        if let Some(cesopsn_stats) = circuit_stats.cesopsn_stats.get(&circuit_id) {
            if cesopsn_stats.loss_rate > 0.05 {
                // > 5% loss
                return HealthStatus::Critical;
            } else if cesopsn_stats.loss_rate > 0.01 {
                // > 1% loss
                return HealthStatus::Warning;
            }
        }

        // Check timer health
        let active_timers = timer_manager.get_all_timers().await;
        let circuit_timers: Vec<_> = active_timers
            .iter()
            .filter(|timer| timer.circuit_id == circuit_id)
            .collect();

        // Too many active timers might indicate issues
        if circuit_timers.len() > 10 {
            return HealthStatus::Warning;
        }

        // Check for expired timers that might indicate problems
        let expired_timers = circuit_timers
            .iter()
            .filter(|timer| timer.is_expired())
            .count();

        if expired_timers > 3 {
            return HealthStatus::Critical;
        } else if expired_timers > 0 {
            return HealthStatus::Warning;
        }

        HealthStatus::Healthy
    }

    /// Get complete stack status
    pub async fn get_stack_status(&self) -> Result<IsdnStackStatus> {
        let mut circuits = Vec::new();

        for (&circuit_id, circuit_config) in &self.config.circuits {
            let circuit_status = self.get_circuit_status(circuit_id).await?;
            circuits.push(circuit_status);
        }

        let overall_health = if circuits.iter().any(|c| c.health == HealthStatus::Critical) {
            HealthStatus::Critical
        } else if circuits.iter().any(|c| c.health == HealthStatus::Warning) {
            HealthStatus::Warning
        } else if circuits.iter().all(|c| c.health == HealthStatus::Healthy) {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unknown
        };

        Ok(IsdnStackStatus {
            circuits,
            overall_health,
            timestamp: chrono::Utc::now(),
        })
    }

    /// Get status for a specific circuit
    async fn get_circuit_status(&self, circuit_id: u16) -> Result<CircuitStatus> {
        let config = self
            .config
            .circuits
            .get(&circuit_id)
            .ok_or_else(|| anyhow!("Circuit {} not configured", circuit_id))?;

        let health = self
            .health_monitor
            .read()
            .await
            .get(&circuit_id)
            .copied()
            .unwrap_or(HealthStatus::Unknown);

        // Get LAPD statistics
        let lapd_status = {
            let lapd_managers = self.lapd_managers.read().await;
            if let Some(lapd_manager) = lapd_managers.get(&circuit_id) {
                let lapd_stats = lapd_manager.get_statistics().await;
                LapdStatusInfo {
                    connections: lapd_stats.total_connections as u32,
                    established: lapd_stats.established_connections as u32,
                    state_summary: HashMap::new(), // Would be populated with actual states
                }
            } else {
                LapdStatusInfo {
                    connections: 0,
                    established: 0,
                    state_summary: HashMap::new(),
                }
            }
        };

        // Get Q.931 call statistics
        let active_calls = self.active_calls.read().await;
        let circuit_calls = active_calls
            .get(&circuit_id)
            .map(|calls| calls.len())
            .unwrap_or(0);

        let q931_status = Q931StatusInfo {
            active_calls: circuit_calls as u32,
            call_states: HashMap::new(), // Would be populated with actual call states
            messages_sent: 0,            // Would track from actual statistics
            messages_received: 0,
        };

        // Get timer statistics
        let all_timers = self.timer_manager.get_all_timers().await;
        let circuit_timers: Vec<_> = all_timers
            .iter()
            .filter(|timer| timer.circuit_id == circuit_id)
            .collect();

        let timer_status = TimerStatusInfo {
            active_timers: circuit_timers.len() as u32,
            timer_types: HashMap::new(), // Would be populated with timer type counts
            expired_timers: 0,
        };

        // Get CESoPSN statistics
        let cesopsn_stats = self
            .cesopsn_integration
            .lock()
            .await
            .get_circuit_stats(circuit_id)
            .await
            .unwrap_or_else(|_| crate::cesopsn_ni2_integration::CesopsnCircuitStats {
                circuit_id,
                cesopsn_stats: HashMap::new(),
                ni2_active_calls: 0,
                dtmf_events_detected: 0,
                dtmf_events_generated: 0,
            });

        let cesopsn_status = if let Some(stats) = cesopsn_stats.cesopsn_stats.get(&circuit_id) {
            CesopsnStatusInfo {
                packets_sent: stats.packets_sent,
                packets_received: stats.packets_received,
                bytes_sent: stats.bytes_sent,
                bytes_received: stats.bytes_received,
                jitter_ms: stats.jitter_us as f32 / 1000.0,
                loss_rate: stats.loss_rate,
            }
        } else {
            CesopsnStatusInfo {
                packets_sent: 0,
                packets_received: 0,
                bytes_sent: 0,
                bytes_received: 0,
                jitter_ms: 0.0,
                loss_rate: 0.0,
            }
        };

        Ok(CircuitStatus {
            circuit_id,
            description: config.description.clone(),
            variant: config.isdn_config.variant,
            side_type: config.isdn_config.side_type,
            pcm_codec: config.pcm_codec,
            lapd_status,
            q931_status,
            timer_status,
            cesopsn_status,
            health,
        })
    }

    /// Subscribe to stack events
    pub fn subscribe_events(&self) -> broadcast::Receiver<IsdnStackEvent> {
        self.event_sender.subscribe()
    }

    /// Get stack statistics
    pub async fn get_statistics(&self) -> Result<IsdnStackStatistics> {
        Ok(self.statistics.read().await.clone())
    }

    /// Shutdown the stack gracefully
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down ISDN PRI Stack: {}", self.config.name);

        // Cancel all active timers
        let active_calls = self.active_calls.read().await;
        for circuit_calls in active_calls.values() {
            for call in circuit_calls.values() {
                self.timer_manager
                    .cancel_call_timers(call.call_reference)
                    .await;
            }
        }

        info!("ISDN PRI Stack shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_isdn_stack_creation() -> Result<()> {
        let config = IsdnStackConfig {
            name: "Test Stack".to_string(),
            variant: IsdnVariant::NI2,
            side_type: IsdnSideType::User,
            circuits: HashMap::new(),
            enable_health_monitoring: false,
            health_check_interval: Duration::from_secs(30),
        };

        let stack = IsdnStackManager::new(config).await?;
        assert_eq!(stack.config.name, "Test Stack");
        Ok(())
    }

    #[tokio::test]
    async fn test_call_state_transitions() {
        let mut call = IsdnCall::new(123, 1);
        assert_eq!(call.state, IsdnCallState::Null);

        call.update_state(IsdnCallState::CallPresent);
        assert_eq!(call.state, IsdnCallState::CallPresent);

        call.update_state(IsdnCallState::Active);
        assert_eq!(call.state, IsdnCallState::Active);
    }
}
