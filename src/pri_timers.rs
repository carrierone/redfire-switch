/*
 * PRI Timer Management System
 *
 * Complete implementation of ISDN PRI timers per ITU-T Q.931 and variant specifications.
 * Supports both NI-2 and Euro ISDN timer values and behaviors.
 *
 * Features:
 * - All standard PRI timers (T301, T303, T305, T308, T310, T313, etc.)
 * - Variant-specific timer values and behaviors
 * - Proper timer lifecycle management
 * - Timer expiration handling with configurable actions
 * - Integration with Q.931 call state machine
 */

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tokio::time::{sleep_until, Instant as TokioInstant};
use tracing::{debug, info, warn};

use crate::q931_messages::{IsdnSideType, IsdnVariant, Q931MessageType};

/// PRI Timer Types per ITU-T Q.931
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PriTimerType {
    /// T301 - Alert timer (180-240s)
    /// Time allowed for called party to answer after receiving ALERTING
    T301,

    /// T302 - Overlap receiving timer (10-15s)
    /// Maximum time to wait for additional digits in overlap receiving
    T302,

    /// T303 - Setup timer (4s)
    /// Maximum time to wait for a response to SETUP message
    T303,

    /// T304 - Overlap sending timer (10-15s)
    /// Maximum time allowed for overlap sending
    T304,

    /// T305 - Disconnect timer (30s)
    /// Time to wait for response to DISCONNECT message
    T305,

    /// T308 - Release timer (4s)
    /// Time to wait for RELEASE COMPLETE after sending RELEASE
    T308,

    /// T310 - Call proceeding timer (10-60s)
    /// Time to wait for more signaling after CALL PROCEEDING
    T310,

    /// T313 - Connect timer (4s)
    /// Time to wait for CONNECT ACK after sending CONNECT
    T313,

    /// T314 - Restart timer (4s)
    /// Time to wait for RESTART ACK after sending RESTART
    T314,

    /// T316 - Restart timer (120s)
    /// Time between sending RESTART messages
    T316,

    /// T317 - Restart timer (100-120s)
    /// Maximum time for restart procedure
    T317,

    /// T322 - Status enquiry timer (4s)
    /// Time to wait for STATUS response to STATUS ENQUIRY
    T322,
}

impl PriTimerType {
    /// Get default duration for timer based on ISDN variant and side
    pub fn default_duration(&self, variant: IsdnVariant, side: IsdnSideType) -> Duration {
        match (self, variant, side) {
            (PriTimerType::T301, IsdnVariant::NI2, _) => Duration::from_secs(180), // 3 minutes
            (PriTimerType::T301, IsdnVariant::EuroIsdn, _) => Duration::from_secs(240), // 4 minutes

            (PriTimerType::T302, _, _) => Duration::from_secs(15),

            (PriTimerType::T303, IsdnVariant::NI2, _) => Duration::from_secs(4),
            (PriTimerType::T303, IsdnVariant::EuroIsdn, _) => Duration::from_secs(4),

            (PriTimerType::T304, _, _) => Duration::from_secs(15),

            (PriTimerType::T305, IsdnVariant::NI2, _) => Duration::from_secs(30),
            (PriTimerType::T305, IsdnVariant::EuroIsdn, _) => Duration::from_secs(30),

            (PriTimerType::T308, _, _) => Duration::from_secs(4),

            (PriTimerType::T310, IsdnVariant::NI2, IsdnSideType::Network) => {
                Duration::from_secs(40)
            }
            (PriTimerType::T310, IsdnVariant::NI2, IsdnSideType::User) => Duration::from_secs(10),
            (PriTimerType::T310, IsdnVariant::EuroIsdn, _) => Duration::from_secs(30),

            (PriTimerType::T313, _, _) => Duration::from_secs(4),
            (PriTimerType::T314, _, _) => Duration::from_secs(4),
            (PriTimerType::T316, _, _) => Duration::from_secs(120),
            (PriTimerType::T317, _, _) => Duration::from_secs(100),
            (PriTimerType::T322, _, _) => Duration::from_secs(4),
        }
    }

    /// Get timer description
    pub fn description(&self) -> &'static str {
        match self {
            PriTimerType::T301 => "Alert timer - called party answer timeout",
            PriTimerType::T302 => "Overlap receiving timer",
            PriTimerType::T303 => "Setup timer - response to SETUP timeout",
            PriTimerType::T304 => "Overlap sending timer",
            PriTimerType::T305 => "Disconnect timer - response to DISCONNECT timeout",
            PriTimerType::T308 => "Release timer - RELEASE COMPLETE timeout",
            PriTimerType::T310 => "Call proceeding timer",
            PriTimerType::T313 => "Connect timer - CONNECT ACK timeout",
            PriTimerType::T314 => "Restart timer - RESTART ACK timeout",
            PriTimerType::T316 => "Restart interval timer",
            PriTimerType::T317 => "Restart procedure timer",
            PriTimerType::T322 => "Status enquiry timer",
        }
    }
}

/// PRI Timer Actions on expiration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PriTimerAction {
    /// Clear the call with specified cause
    ClearCall(u8), // Cause value

    /// Send Q.931 message
    SendMessage(Q931MessageType),

    /// Retry operation (with retry count)
    Retry(u8), // Max retries

    /// Restart circuit/channel
    RestartCircuit,

    /// Custom action with description
    Custom(String),

    /// No action (just log)
    None,
}

/// Active PRI Timer instance
#[derive(Debug, Clone)]
pub struct ActivePriTimer {
    /// Timer type
    pub timer_type: PriTimerType,
    /// Call reference value (0 for global timers)
    pub call_reference: u16,
    /// Circuit ID
    pub circuit_id: u16,
    /// When timer was started
    pub start_time: Instant,
    /// Timer duration
    pub duration: Duration,
    /// Action to take on expiration
    pub action: PriTimerAction,
    /// Current retry count (for retry actions)
    pub retry_count: u8,
    /// Maximum retries allowed
    pub max_retries: u8,
}

impl ActivePriTimer {
    pub fn new(
        timer_type: PriTimerType,
        call_reference: u16,
        circuit_id: u16,
        duration: Duration,
        action: PriTimerAction,
    ) -> Self {
        Self {
            timer_type,
            call_reference,
            circuit_id,
            start_time: Instant::now(),
            duration,
            action,
            retry_count: 0,
            max_retries: 0,
        }
    }

    /// Check if timer has expired
    pub fn is_expired(&self) -> bool {
        self.start_time.elapsed() >= self.duration
    }

    /// Get remaining time
    pub fn remaining(&self) -> Duration {
        let elapsed = self.start_time.elapsed();
        if elapsed >= self.duration {
            Duration::ZERO
        } else {
            self.duration - elapsed
        }
    }

    /// Get timer expiration instant (for tokio scheduling)
    pub fn expiration_instant(&self) -> TokioInstant {
        TokioInstant::now() + self.remaining()
    }
}

/// Timer expiration event
#[derive(Debug, Clone)]
pub struct PriTimerExpiredEvent {
    pub timer: ActivePriTimer,
    pub timestamp: Instant,
}

/// PRI Timer Manager
pub struct PriTimerManager {
    /// ISDN variant configuration
    variant: IsdnVariant,
    /// ISDN side type
    side_type: IsdnSideType,
    /// Active timers by unique ID
    active_timers: Arc<RwLock<HashMap<String, ActivePriTimer>>>,
    /// Timer expiration event sender
    expiration_sender: broadcast::Sender<PriTimerExpiredEvent>,
    /// Timer task handles
    timer_handles: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
}

impl PriTimerManager {
    /// Create new PRI timer manager
    pub fn new(variant: IsdnVariant, side_type: IsdnSideType) -> Self {
        let (expiration_sender, _) = broadcast::channel(1000);

        Self {
            variant,
            side_type,
            active_timers: Arc::new(RwLock::new(HashMap::new())),
            expiration_sender,
            timer_handles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start a PRI timer
    pub async fn start_timer(
        &self,
        timer_type: PriTimerType,
        call_reference: u16,
        circuit_id: u16,
        duration: Option<Duration>,
        action: PriTimerAction,
    ) -> Result<String> {
        let timer_duration =
            duration.unwrap_or_else(|| timer_type.default_duration(self.variant, self.side_type));

        let timer = ActivePriTimer::new(
            timer_type,
            call_reference,
            circuit_id,
            timer_duration,
            action,
        );

        let timer_id = self.generate_timer_id(timer_type, call_reference, circuit_id);

        // Cancel existing timer with same ID if present
        self.cancel_timer(&timer_id).await;

        // Add to active timers
        self.active_timers
            .write()
            .await
            .insert(timer_id.clone(), timer.clone());

        // Start timer task
        let timer_handle = self.spawn_timer_task(timer_id.clone(), timer).await;
        self.timer_handles
            .write()
            .await
            .insert(timer_id.clone(), timer_handle);

        info!(
            "Started PRI timer {}: {} on CRV {} ({})",
            timer_id,
            timer_type.description(),
            call_reference,
            timer_duration.as_secs()
        );

        Ok(timer_id)
    }

    /// Cancel a PRI timer
    pub async fn cancel_timer(&self, timer_id: &str) -> bool {
        let removed = self.active_timers.write().await.remove(timer_id).is_some();

        if let Some(handle) = self.timer_handles.write().await.remove(timer_id) {
            handle.abort();
        }

        if removed {
            debug!("Cancelled PRI timer {}", timer_id);
        }

        removed
    }

    /// Cancel all timers for a specific call reference
    pub async fn cancel_call_timers(&self, call_reference: u16) -> usize {
        let mut cancelled_count = 0;
        let timer_ids_to_cancel: Vec<String> = {
            let timers = self.active_timers.read().await;
            timers
                .iter()
                .filter(|(_, timer)| timer.call_reference == call_reference)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for timer_id in timer_ids_to_cancel {
            if self.cancel_timer(&timer_id).await {
                cancelled_count += 1;
            }
        }

        if cancelled_count > 0 {
            info!(
                "Cancelled {} PRI timers for CRV {}",
                cancelled_count, call_reference
            );
        }

        cancelled_count
    }

    /// Cancel all timers for a circuit
    pub async fn cancel_circuit_timers(&self, circuit_id: u16) -> usize {
        let mut cancelled_count = 0;
        let timer_ids_to_cancel: Vec<String> = {
            let timers = self.active_timers.read().await;
            timers
                .iter()
                .filter(|(_, timer)| timer.circuit_id == circuit_id)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for timer_id in timer_ids_to_cancel {
            if self.cancel_timer(&timer_id).await {
                cancelled_count += 1;
            }
        }

        if cancelled_count > 0 {
            info!(
                "Cancelled {} PRI timers for circuit {}",
                cancelled_count, circuit_id
            );
        }

        cancelled_count
    }

    /// Get active timer by ID
    pub async fn get_timer(&self, timer_id: &str) -> Option<ActivePriTimer> {
        self.active_timers.read().await.get(timer_id).cloned()
    }

    /// Get all active timers for a call reference
    pub async fn get_call_timers(&self, call_reference: u16) -> Vec<ActivePriTimer> {
        self.active_timers
            .read()
            .await
            .values()
            .filter(|timer| timer.call_reference == call_reference)
            .cloned()
            .collect()
    }

    /// Get all active timers
    pub async fn get_all_timers(&self) -> Vec<ActivePriTimer> {
        self.active_timers.read().await.values().cloned().collect()
    }

    /// Subscribe to timer expiration events
    pub fn subscribe_expiration_events(&self) -> broadcast::Receiver<PriTimerExpiredEvent> {
        self.expiration_sender.subscribe()
    }

    /// Generate unique timer ID
    fn generate_timer_id(
        &self,
        timer_type: PriTimerType,
        call_reference: u16,
        circuit_id: u16,
    ) -> String {
        format!("{:?}-CRV{}-C{}", timer_type, call_reference, circuit_id)
    }

    /// Spawn timer task for specific timer
    async fn spawn_timer_task(
        &self,
        timer_id: String,
        timer: ActivePriTimer,
    ) -> tokio::task::JoinHandle<()> {
        let expiration_sender = self.expiration_sender.clone();
        let active_timers = Arc::clone(&self.active_timers);
        let timer_handles = Arc::clone(&self.timer_handles);

        tokio::spawn(async move {
            // Sleep until timer expires
            sleep_until(timer.expiration_instant()).await;

            // Remove from active timers
            if active_timers.write().await.remove(&timer_id).is_some() {
                timer_handles.write().await.remove(&timer_id);

                // Send expiration event
                let event = PriTimerExpiredEvent {
                    timer: timer.clone(),
                    timestamp: Instant::now(),
                };

                if let Err(e) = expiration_sender.send(event) {
                    warn!("Failed to send timer expiration event: {}", e);
                }

                warn!(
                    "PRI Timer {} expired: {} on CRV {} - Action: {:?}",
                    timer_id,
                    timer.timer_type.description(),
                    timer.call_reference,
                    timer.action
                );
            }
        })
    }

    /// Get timer statistics
    pub async fn get_statistics(&self) -> PriTimerStatistics {
        let active_timers = self.active_timers.read().await;
        let total_active = active_timers.len();

        let mut by_type = HashMap::new();
        for timer in active_timers.values() {
            *by_type.entry(timer.timer_type).or_insert(0) += 1;
        }

        PriTimerStatistics {
            total_active,
            by_type,
            variant: self.variant,
            side_type: self.side_type,
        }
    }
}

/// PRI Timer Statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriTimerStatistics {
    pub total_active: usize,
    pub by_type: HashMap<PriTimerType, usize>,
    pub variant: IsdnVariant,
    pub side_type: IsdnSideType,
}

/// Helper function to create timer action for clearing call with cause
pub fn clear_call_action(cause: u8) -> PriTimerAction {
    PriTimerAction::ClearCall(cause)
}

/// Helper function to create timer action for sending message
pub fn send_message_action(message_type: Q931MessageType) -> PriTimerAction {
    PriTimerAction::SendMessage(message_type)
}

/// Helper function to create retry action
pub fn retry_action(max_retries: u8) -> PriTimerAction {
    PriTimerAction::Retry(max_retries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration as TokioDuration};

    #[tokio::test]
    async fn test_timer_creation_and_expiration() -> Result<()> {
        let manager = PriTimerManager::new(IsdnVariant::NI2, IsdnSideType::User);
        let mut expiration_rx = manager.subscribe_expiration_events();

        // Start a very short timer for testing
        let timer_id = manager
            .start_timer(
                PriTimerType::T303,
                123, // CRV
                1,   // Circuit ID
                Some(Duration::from_millis(100)),
                clear_call_action(31), // Normal, unspecified
            )
            .await?;

        // Wait for expiration event
        let event = match timeout(TokioDuration::from_millis(200), expiration_rx.recv()).await {
            Ok(Ok(event)) => event,
            Ok(Err(_)) => return Err(anyhow!("Receive error")),
            Err(_) => return Err(anyhow!("Timer should expire")),
        };

        assert_eq!(event.timer.timer_type, PriTimerType::T303);
        assert_eq!(event.timer.call_reference, 123);

        // Timer should be removed from active list
        assert!(manager.get_timer(&timer_id).await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn test_timer_cancellation() -> Result<()> {
        let manager = PriTimerManager::new(IsdnVariant::NI2, IsdnSideType::User);

        let timer_id = manager
            .start_timer(
                PriTimerType::T301,
                456,
                2,
                Some(Duration::from_secs(60)),
                clear_call_action(19), // No answer
            )
            .await?;

        // Verify timer is active
        assert!(manager.get_timer(&timer_id).await.is_some());

        // Cancel timer
        assert!(manager.cancel_timer(&timer_id).await);

        // Verify timer is removed
        assert!(manager.get_timer(&timer_id).await.is_none());
        Ok(())
    }

    #[test]
    fn test_timer_default_durations() {
        // Test NI-2 vs Euro ISDN differences
        assert_eq!(
            PriTimerType::T301.default_duration(IsdnVariant::NI2, IsdnSideType::User),
            Duration::from_secs(180)
        );
        assert_eq!(
            PriTimerType::T301.default_duration(IsdnVariant::EuroIsdn, IsdnSideType::User),
            Duration::from_secs(240)
        );

        // Test Network vs User differences for T310
        assert_eq!(
            PriTimerType::T310.default_duration(IsdnVariant::NI2, IsdnSideType::Network),
            Duration::from_secs(40)
        );
        assert_eq!(
            PriTimerType::T310.default_duration(IsdnVariant::NI2, IsdnSideType::User),
            Duration::from_secs(10)
        );
    }
}
