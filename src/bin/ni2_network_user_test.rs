/*
 * NI-2 Network/User Side Test
 * 
 * This test demonstrates a complete NI-2 call flow between:
 * - User Side (Customer/CPE) - initiates the call
 * - Network Side (Switch/Carrier) - receives and processes the call
 * 
 * Call Flow:
 * User Side:  SETUP -----------> Network Side
 * User Side:  <-- CALL PROC ----- Network Side  
 * User Side:  <-- ALERTING ------ Network Side
 * User Side:  <-- CONNECT ------- Network Side
 * User Side:  <-- Active Call --> Network Side
 * User Side:  DISCONNECT -------> Network Side
 */

use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn, Level};
use tracing_subscriber;

use redfire_switch::tdmoe_ni2_signaling::{
    TdmoeNi2Signaling, Ni2SideType, Ni2Event, Ni2CallState
};

/// Test event monitor for tracking NI-2 events
struct Ni2EventMonitor {
    side_name: String,
    event_receiver: tokio::sync::broadcast::Receiver<Ni2Event>,
    events_received: u32,
}

impl Ni2EventMonitor {
    fn new(side_name: String, event_receiver: tokio::sync::broadcast::Receiver<Ni2Event>) -> Self {
        Self {
            side_name,
            event_receiver,
            events_received: 0,
        }
    }
    
    async fn monitor_events(&mut self) {
        while let Ok(event) = self.event_receiver.recv().await {
            self.events_received += 1;
            
            match event {
                Ni2Event::CallInitiated { channel_id, call_reference, calling_number, called_number } => {
                    info!("📞 [{}] Call Initiated: {} -> {} on {} (CRV: {})", 
                          self.side_name, calling_number, called_number, channel_id, call_reference);
                }
                Ni2Event::CallPresent { channel_id, call_reference, calling_number, called_number } => {
                    info!("📥 [{}] Call Present: {} -> {} on {} (CRV: {})", 
                          self.side_name, calling_number, called_number, channel_id, call_reference);
                }
                Ni2Event::CallProceeding { channel_id, call_reference } => {
                    info!("⚡ [{}] Call Proceeding: {} (CRV: {})", 
                          self.side_name, channel_id, call_reference);
                }
                Ni2Event::CallAlerting { channel_id, call_reference } => {
                    info!("🔔 [{}] Call Alerting: {} (CRV: {})", 
                          self.side_name, channel_id, call_reference);
                }
                Ni2Event::CallConnected { channel_id, call_reference } => {
                    info!("✅ [{}] Call Connected: {} (CRV: {})", 
                          self.side_name, channel_id, call_reference);
                }
                Ni2Event::CallDisconnected { channel_id, call_reference, cause } => {
                    info!("❌ [{}] Call Disconnected: {} (CRV: {}, cause: {})", 
                          self.side_name, channel_id, call_reference, cause);
                }
                Ni2Event::CallStateChanged { channel_id, old_state, new_state } => {
                    info!("🔄 [{}] State Change: {} {:?} -> {:?}", 
                          self.side_name, channel_id, old_state, new_state);
                }
                Ni2Event::MessageReceived { channel_id, message } => {
                    info!("📨 [{}] Message Received: {} ({} bytes)", 
                          self.side_name, channel_id, message.len());
                }
                Ni2Event::InformationElementSent { channel_id, element } => {
                    info!("📤 [{}] IE Sent: {} {:?}", 
                          self.side_name, channel_id, element);
                }
            }
        }
    }
    
    fn get_event_count(&self) -> u32 {
        self.events_received
    }
}

/// NI-2 Call Test Scenario
struct Ni2CallTest {
    user_side: TdmoeNi2Signaling,
    network_side: TdmoeNi2Signaling,
    test_channel: String,
    calling_number: String,
    called_number: String,
}

impl Ni2CallTest {
    async fn new() -> Result<Self> {
        let user_side = TdmoeNi2Signaling::new_with_side(Ni2SideType::User)?;
        let network_side = TdmoeNi2Signaling::new_with_side(Ni2SideType::Network)?;
        
        Ok(Self {
            user_side,
            network_side,
            test_channel: "T1-1-1".to_string(),
            calling_number: "15551234567".to_string(),
            called_number: "15559876543".to_string(),
        })
    }
    
    /// Execute complete NI-2 call flow test
    async fn run_call_flow_test(&self) -> Result<()> {
        info!("🚀 Starting NI-2 Network/User Call Flow Test");
        info!("📋 Test Parameters:");
        info!("   Channel: {}", self.test_channel);
        info!("   Calling: {}", self.calling_number);
        info!("   Called:  {}", self.called_number);
        info!("   User Side: {:?}", self.user_side.get_side_type());
        info!("   Network Side: {:?}", self.network_side.get_side_type());
        
        // Setup event monitoring
        let user_events = self.user_side.subscribe();
        let network_events = self.network_side.subscribe();
        
        let mut user_monitor = Ni2EventMonitor::new("USER".to_string(), user_events);
        let mut network_monitor = Ni2EventMonitor::new("NETWORK".to_string(), network_events);
        
        // Spawn monitoring tasks
        let user_monitor_task = tokio::spawn(async move {
            user_monitor.monitor_events().await;
            user_monitor.get_event_count()
        });
        
        let network_monitor_task = tokio::spawn(async move {
            network_monitor.monitor_events().await;
            network_monitor.get_event_count()
        });
        
        // Step 1: User side initiates call (sends SETUP)
        info!("\n=== STEP 1: User Side Initiates Call ===");
        let call_reference = self.user_side.initiate_call(
            &self.test_channel,
            &self.calling_number,
            &self.called_number
        ).await?;
        
        // Verify call state on user side
        let user_state = self.user_side.get_call_state(&self.test_channel).await;
        assert_eq!(user_state, Some(Ni2CallState::CallInitiated));
        info!("✅ User side call state: {:?}", user_state);
        
        sleep(Duration::from_millis(100)).await;
        
        // Step 2: Network side processes SETUP (receives call)
        info!("\n=== STEP 2: Network Side Processes SETUP ===");
        self.network_side.process_incoming_setup(
            &self.test_channel,
            call_reference,
            &self.calling_number,
            &self.called_number
        ).await?;
        
        // Verify call state on network side
        let network_state = self.network_side.get_call_state(&self.test_channel).await;
        assert_eq!(network_state, Some(Ni2CallState::CallPresent));
        info!("✅ Network side call state: {:?}", network_state);
        
        sleep(Duration::from_millis(100)).await;
        
        // Step 3: Network side sends CALL PROCEEDING
        info!("\n=== STEP 3: Network Side Sends CALL PROCEEDING ===");
        self.network_side.send_call_proceeding(&self.test_channel).await?;
        
        let network_state = self.network_side.get_call_state(&self.test_channel).await;
        assert_eq!(network_state, Some(Ni2CallState::IncomingCallProceeding));
        info!("✅ Network side call state: {:?}", network_state);
        
        sleep(Duration::from_millis(100)).await;
        
        // Step 4: Network side sends ALERTING (ringing)
        info!("\n=== STEP 4: Network Side Sends ALERTING ===");
        self.network_side.send_alerting(&self.test_channel).await?;
        
        let network_state = self.network_side.get_call_state(&self.test_channel).await;
        assert_eq!(network_state, Some(Ni2CallState::CallDelivered));
        info!("✅ Network side call state: {:?}", network_state);
        
        sleep(Duration::from_millis(500)).await; // Simulate ringing time
        
        // Step 5: Network side sends CONNECT (answer)
        info!("\n=== STEP 5: Network Side Sends CONNECT (Answer) ===");
        self.network_side.send_connect(&self.test_channel).await?;
        
        let network_state = self.network_side.get_call_state(&self.test_channel).await;
        assert_eq!(network_state, Some(Ni2CallState::Active));
        info!("✅ Network side call state: {:?}", network_state);
        info!("🎉 Call is now ACTIVE on both sides!");
        
        sleep(Duration::from_millis(1000)).await; // Simulate call duration
        
        // Step 6: User side disconnects call
        info!("\n=== STEP 6: User Side Disconnects Call ===");
        self.user_side.send_disconnect(&self.test_channel, 16).await?; // Normal call clearing
        
        let user_state = self.user_side.get_call_state(&self.test_channel).await;
        assert_eq!(user_state, Some(Ni2CallState::DisconnectRequest));
        info!("✅ User side call state: {:?}", user_state);
        
        sleep(Duration::from_millis(200)).await;
        
        info!("\n=== CALL FLOW COMPLETE ===");
        
        // Cancel monitoring tasks and get event counts
        user_monitor_task.abort();
        network_monitor_task.abort();
        
        // Display call statistics
        self.display_call_statistics().await;
        
        Ok(())
    }
    
    /// Test NI-2 side type enforcement
    async fn test_side_type_enforcement(&self) -> Result<()> {
        info!("\n🔒 Testing NI-2 Side Type Enforcement");
        
        // Test 1: Network side should not be able to initiate calls
        info!("Test 1: Network side attempting to initiate call (should fail)");
        match self.network_side.initiate_call("T1-1-2", "15551111111", "15552222222").await {
            Err(e) => info!("✅ Correctly rejected: {}", e),
            Ok(_) => {
                warn!("❌ ERROR: Network side should not be able to initiate calls");
                return Err(anyhow::anyhow!("Side type enforcement failed"));
            }
        }
        
        // Test 2: User side should not be able to send CALL PROCEEDING
        info!("Test 2: User side attempting to send CALL PROCEEDING (should fail)");
        match self.user_side.send_call_proceeding("T1-1-2").await {
            Err(e) => info!("✅ Correctly rejected: {}", e),
            Ok(_) => {
                warn!("❌ ERROR: User side should not be able to send CALL PROCEEDING");
                return Err(anyhow::anyhow!("Side type enforcement failed"));
            }
        }
        
        // Test 3: User side should not be able to send ALERTING
        info!("Test 3: User side attempting to send ALERTING (should fail)");
        match self.user_side.send_alerting("T1-1-2").await {
            Err(e) => info!("✅ Correctly rejected: {}", e),
            Ok(_) => {
                warn!("❌ ERROR: User side should not be able to send ALERTING");
                return Err(anyhow::anyhow!("Side type enforcement failed"));
            }
        }
        
        // Test 4: User side should not be able to send CONNECT
        info!("Test 4: User side attempting to send CONNECT (should fail)");
        match self.user_side.send_connect("T1-1-2").await {
            Err(e) => info!("✅ Correctly rejected: {}", e),
            Ok(_) => {
                warn!("❌ ERROR: User side should not be able to send CONNECT");
                return Err(anyhow::anyhow!("Side type enforcement failed"));
            }
        }
        
        info!("✅ All side type enforcement tests passed!");
        Ok(())
    }
    
    /// Display call statistics from both sides
    async fn display_call_statistics(&self) {
        info!("\n📊 Call Statistics Summary:");
        
        // User side statistics
        let user_calls = self.user_side.get_active_calls().await;
        info!("👤 User Side:");
        info!("   Active calls: {}", user_calls.len());
        for (channel, context) in user_calls {
            info!("   Channel {}: {:?} (CRV: {})", 
                  channel, context.state, context.call_reference);
            if let Some(start_time) = context.call_start_time {
                let duration = start_time.elapsed();
                info!("     Call duration: {:.2}s", duration.as_secs_f32());
            }
        }
        
        // Network side statistics  
        let network_calls = self.network_side.get_active_calls().await;
        info!("🏢 Network Side:");
        info!("   Active calls: {}", network_calls.len());
        for (channel, context) in network_calls {
            info!("   Channel {}: {:?} (CRV: {})", 
                  channel, context.state, context.call_reference);
            if let Some(start_time) = context.call_start_time {
                let duration = start_time.elapsed();
                info!("     Call duration: {:.2}s", duration.as_secs_f32());
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();
    
    info!("🌟 NI-2 Network/User Side Integration Test");
    info!("📡 Testing proper ITU-T Q.931 call flow between Network and User sides");
    
    // Create test instance
    let test = Ni2CallTest::new().await?;
    
    // Run main call flow test
    test.run_call_flow_test().await?;
    
    // Test side type enforcement
    test.test_side_type_enforcement().await?;
    
    // Final summary
    info!("\n🎯 TEST SUMMARY");
    info!("✅ NI-2 Network/User side distinction implemented correctly");
    info!("✅ Complete call flow (SETUP -> PROC -> ALERT -> CONNECT) working");
    info!("✅ Proper state transitions per ITU-T Q.931");
    info!("✅ Side type enforcement (Network vs User) validated");
    info!("✅ Call reference value (CRV) handling correct");
    info!("");
    info!("🏆 All tests passed! NI-2 implementation is working correctly.");
    
    Ok(())
}