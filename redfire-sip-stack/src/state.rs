/*
 * Redfire Switch - SIP Dialog and Transaction State Management
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use crate::parser::{
    DialogState, InviteTransactionState, NonInviteTransactionState, SipDialog, SipMessage,
    SipTransaction, TransactionState, TransactionTimers,
};
use anyhow::{anyhow, Result};
use dashmap::DashMap;
use rsip::{
    message::{HeadersExt, SipMessage as RsipMessage},
    Method, Request, Response,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, info, warn};

/// SIP state manager for dialogs and transactions
pub struct SipStateManager {
    /// Active dialogs indexed by dialog ID
    dialogs: Arc<DashMap<String, SipDialog>>,
    /// Active transactions indexed by transaction ID
    transactions: Arc<DashMap<String, SipTransaction>>,
    /// Transaction timers
    timer_manager: Arc<TransactionTimerManager>,
    /// Configuration
    config: SipStateConfig,
}

/// SIP state manager configuration
#[derive(Debug, Clone)]
pub struct SipStateConfig {
    /// Timer T1 (RTT estimate) in milliseconds
    pub timer_t1: u64,
    /// Timer T2 (maximum retransmit interval) in milliseconds
    pub timer_t2: u64,
    /// Timer T4 (maximum duration a message will remain in the network) in milliseconds
    pub timer_t4: u64,
    /// Maximum dialog lifetime in seconds
    pub max_dialog_lifetime: u64,
    /// Maximum transaction lifetime in seconds
    pub max_transaction_lifetime: u64,
    /// Cleanup interval in seconds
    pub cleanup_interval: u64,
}

impl Default for SipStateConfig {
    fn default() -> Self {
        Self {
            timer_t1: 500,                 // 500ms default RTT
            timer_t2: 4000,                // 4s maximum retransmit interval
            timer_t4: 5000,                // 5s maximum network duration
            max_dialog_lifetime: 3600,     // 1 hour
            max_transaction_lifetime: 300, // 5 minutes
            cleanup_interval: 60,          // 1 minute
        }
    }
}

impl SipStateManager {
    /// Create new SIP state manager
    pub fn new(config: SipStateConfig) -> Self {
        let dialogs = Arc::new(DashMap::new());
        let transactions = Arc::new(DashMap::new());
        let timer_manager = Arc::new(TransactionTimerManager::new(
            config.clone(),
            transactions.clone(),
        ));

        Self {
            dialogs,
            transactions,
            timer_manager,
            config,
        }
    }

    /// Start the state manager (timer management, cleanup, etc.)
    pub async fn start(&self) -> Result<()> {
        info!("Starting SIP state manager");

        // Start timer manager
        let timer_manager = self.timer_manager.clone();
        tokio::spawn(async move {
            timer_manager.start().await;
        });

        // Start cleanup task
        let dialogs = self.dialogs.clone();
        let transactions = self.transactions.clone();
        let cleanup_interval = self.config.cleanup_interval;
        let max_dialog_lifetime = self.config.max_dialog_lifetime;
        let max_transaction_lifetime = self.config.max_transaction_lifetime;

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(cleanup_interval));
            loop {
                interval.tick().await;
                Self::cleanup_expired_state(
                    &dialogs,
                    &transactions,
                    max_dialog_lifetime,
                    max_transaction_lifetime,
                )
                .await;
            }
        });

        Ok(())
    }

    /// Process incoming SIP message and update state
    pub async fn process_message(&self, message: &SipMessage) -> Result<SipStateAction> {
        match &message.message {
            RsipMessage::Request(req) => self.process_request(message, req).await,
            RsipMessage::Response(resp) => self.process_response(message, resp).await,
        }
    }

    /// Process SIP request
    async fn process_request(
        &self,
        message: &SipMessage,
        request: &Request,
    ) -> Result<SipStateAction> {
        let call_id = crate::parser::utils::extract_call_id(&message.message)?;
        let from_tag = crate::parser::utils::extract_from_tag(&message.message)?;
        let to_tag = crate::parser::utils::extract_to_tag(&message.message)?;

        match request.method() {
            Method::Invite => {
                self.process_invite_request(message, request, &call_id, from_tag, to_tag)
                    .await
            }
            Method::Ack => {
                self.process_ack_request(message, request, &call_id, from_tag, to_tag)
                    .await
            }
            Method::Bye => {
                self.process_bye_request(message, request, &call_id, from_tag, to_tag)
                    .await
            }
            Method::Cancel => {
                self.process_cancel_request(message, request, &call_id, from_tag, to_tag)
                    .await
            }
            _ => {
                self.process_other_request(message, request, &call_id, from_tag, to_tag)
                    .await
            }
        }
    }

    /// Process SIP response
    async fn process_response(
        &self,
        message: &SipMessage,
        response: &Response,
    ) -> Result<SipStateAction> {
        let call_id = crate::parser::utils::extract_call_id(&message.message)?;
        let from_tag = crate::parser::utils::extract_from_tag(&message.message)?;
        let to_tag = crate::parser::utils::extract_to_tag(&message.message)?;

        // Find transaction
        let transaction_id = self.extract_transaction_id_from_response(response)?;

        if let Some(mut transaction) = self.transactions.get_mut(&transaction_id) {
            // Update transaction state
            self.update_transaction_state_for_response(&mut transaction, response)?;

            // Handle dialog creation/update for 2xx responses
            if crate::parser::utils::is_success_response(&message.message) {
                if let Some(to_tag) = to_tag {
                    let dialog_id = format!(
                        "{}:{}:{}",
                        call_id,
                        from_tag.clone().unwrap_or_default(),
                        to_tag
                    );
                    self.create_or_update_dialog(
                        &dialog_id,
                        &call_id,
                        from_tag,
                        Some(to_tag),
                        message,
                    )
                    .await?;
                }
            }

            Ok(SipStateAction::ProcessResponse {
                transaction_id: transaction_id.clone(),
                dialog_id: None, // Will be set if dialog exists
                requires_ack: crate::parser::utils::is_success_response(&message.message)
                    && transaction.method == Method::Invite,
            })
        } else {
            warn!(
                "Received response for unknown transaction: {}",
                transaction_id
            );
            Ok(SipStateAction::DropMessage)
        }
    }

    /// Process INVITE request
    async fn process_invite_request(
        &self,
        _message: &SipMessage,
        request: &Request,
        call_id: &str,
        from_tag: Option<String>,
        to_tag: Option<String>,
    ) -> Result<SipStateAction> {
        let transaction_id = self.extract_transaction_id_from_request(request)?;

        // Check for existing transaction (retransmission)
        if self.transactions.contains_key(&transaction_id) {
            debug!("INVITE retransmission detected: {}", transaction_id);
            return Ok(SipStateAction::RetransmitLastResponse { transaction_id });
        }

        // Create new transaction
        let transaction = SipTransaction {
            transaction_id: transaction_id.clone(),
            method: Method::Invite,
            state: TransactionState::Invite(InviteTransactionState::Proceeding),
            request: request.clone(),
            responses: Vec::new(),
            timers: TransactionTimers::default(),
            created_at: chrono::Utc::now(),
        };

        self.transactions
            .insert(transaction_id.clone(), transaction);

        // Check if this is a re-INVITE (dialog exists)
        if let Some(from_tag) = from_tag {
            if let Some(to_tag) = to_tag {
                let dialog_id = format!("{}:{}:{}", call_id, from_tag, to_tag);
                if self.dialogs.contains_key(&dialog_id) {
                    return Ok(SipStateAction::ProcessReInvite {
                        transaction_id,
                        dialog_id,
                    });
                }
            }
        }

        Ok(SipStateAction::ProcessNewInvite { transaction_id })
    }

    /// Process ACK request
    async fn process_ack_request(
        &self,
        _message: &SipMessage,
        _request: &Request,
        call_id: &str,
        from_tag: Option<String>,
        to_tag: Option<String>,
    ) -> Result<SipStateAction> {
        if let (Some(from_tag), Some(to_tag)) = (from_tag, to_tag) {
            let dialog_id = format!("{}:{}:{}", call_id, from_tag, to_tag);

            if let Some(mut dialog) = self.dialogs.get_mut(&dialog_id) {
                // Update dialog state to confirmed
                dialog.state = DialogState::Confirmed;
                dialog.last_activity = chrono::Utc::now();

                return Ok(SipStateAction::ProcessAck { dialog_id });
            }
        }

        // ACK for error response or unknown dialog
        Ok(SipStateAction::ProcessAck {
            dialog_id: "unknown".to_string(),
        })
    }

    /// Process BYE request
    async fn process_bye_request(
        &self,
        _message: &SipMessage,
        request: &Request,
        call_id: &str,
        from_tag: Option<String>,
        to_tag: Option<String>,
    ) -> Result<SipStateAction> {
        let transaction_id = self.extract_transaction_id_from_request(request)?;

        // Create transaction
        let transaction = SipTransaction {
            transaction_id: transaction_id.clone(),
            method: Method::Bye,
            state: TransactionState::NonInvite(NonInviteTransactionState::Trying),
            request: request.clone(),
            responses: Vec::new(),
            timers: TransactionTimers::default(),
            created_at: chrono::Utc::now(),
        };

        self.transactions
            .insert(transaction_id.clone(), transaction);

        // Find and terminate dialog
        if let (Some(from_tag), Some(to_tag)) = (from_tag, to_tag) {
            let dialog_id = format!("{}:{}:{}", call_id, from_tag, to_tag);

            if let Some(mut dialog) = self.dialogs.get_mut(&dialog_id) {
                dialog.state = DialogState::Terminated;
                dialog.last_activity = chrono::Utc::now();

                return Ok(SipStateAction::ProcessBye {
                    transaction_id,
                    dialog_id,
                });
            }
        }

        Ok(SipStateAction::ProcessBye {
            transaction_id,
            dialog_id: "unknown".to_string(),
        })
    }

    /// Process CANCEL request
    async fn process_cancel_request(
        &self,
        _message: &SipMessage,
        request: &Request,
        _call_id: &str,
        _from_tag: Option<String>,
        _to_tag: Option<String>,
    ) -> Result<SipStateAction> {
        let transaction_id = self.extract_transaction_id_from_request(request)?;

        // Find original INVITE transaction to cancel
        // CANCEL has same branch as original INVITE
        if let Some(mut invite_transaction) = self.transactions.get_mut(&transaction_id) {
            if invite_transaction.method == Method::Invite {
                invite_transaction.state =
                    TransactionState::Invite(InviteTransactionState::Completed);

                return Ok(SipStateAction::ProcessCancel {
                    transaction_id: transaction_id.clone(),
                    invite_transaction_id: transaction_id,
                });
            }
        }

        warn!(
            "CANCEL received for unknown INVITE transaction: {}",
            transaction_id
        );
        Ok(SipStateAction::DropMessage)
    }

    /// Process other requests (REGISTER, OPTIONS, etc.)
    async fn process_other_request(
        &self,
        _message: &SipMessage,
        request: &Request,
        _call_id: &str,
        _from_tag: Option<String>,
        _to_tag: Option<String>,
    ) -> Result<SipStateAction> {
        let transaction_id = self.extract_transaction_id_from_request(request)?;

        // Check for retransmission
        if self.transactions.contains_key(&transaction_id) {
            return Ok(SipStateAction::RetransmitLastResponse { transaction_id });
        }

        // Create new transaction
        let transaction = SipTransaction {
            transaction_id: transaction_id.clone(),
            method: request.method().clone(),
            state: TransactionState::NonInvite(NonInviteTransactionState::Trying),
            request: request.clone(),
            responses: Vec::new(),
            timers: TransactionTimers::default(),
            created_at: chrono::Utc::now(),
        };

        self.transactions
            .insert(transaction_id.clone(), transaction);

        Ok(SipStateAction::ProcessOtherRequest {
            transaction_id,
            method: request.method().clone(),
        })
    }

    /// Create or update dialog
    async fn create_or_update_dialog(
        &self,
        dialog_id: &str,
        call_id: &str,
        from_tag: Option<String>,
        to_tag: Option<String>,
        _message: &SipMessage,
    ) -> Result<()> {
        if let Some(mut dialog) = self.dialogs.get_mut(dialog_id) {
            // Update existing dialog
            dialog.last_activity = chrono::Utc::now();
            dialog.state = DialogState::Confirmed;
        } else {
            // Create new dialog
            let dialog = SipDialog {
                dialog_id: dialog_id.to_string(),
                call_id: call_id.to_string(),
                local_tag: from_tag.unwrap_or_default(),
                remote_tag: to_tag.unwrap_or_default(),
                local_uri: rsip::Uri::default(), // TODO: Extract from message
                remote_uri: rsip::Uri::default(), // TODO: Extract from message
                local_seq: 1,
                remote_seq: 1,
                route_set: Vec::new(),
                state: DialogState::Early,
                created_at: chrono::Utc::now(),
                last_activity: chrono::Utc::now(),
            };

            self.dialogs.insert(dialog_id.to_string(), dialog);
        }

        Ok(())
    }

    /// Extract transaction ID from request
    fn extract_transaction_id_from_request(&self, request: &Request) -> Result<String> {
        // Get branch parameter from top Via header
        let _via = request
            .via_header()
            .map_err(|e| anyhow!("No Via header in request: {}", e))?;

        // TODO: Fix Param::Branch pattern matching with rsip library API
        // For now, create a default transaction ID
        warn!("Branch parameter extraction not implemented due to rsip API compatibility");

        Err(anyhow!("No branch parameter in Via header"))
    }

    /// Extract transaction ID from response
    fn extract_transaction_id_from_response(&self, response: &Response) -> Result<String> {
        // Get branch parameter from top Via header
        let _via = response
            .via_header()
            .map_err(|e| anyhow!("No Via header in response: {}", e))?;

        // TODO: Fix Param::Branch pattern matching with rsip library API
        // For now, create a default transaction ID
        warn!("Branch parameter extraction not implemented due to rsip API compatibility");

        Err(anyhow!("No branch parameter in Via header"))
    }

    /// Update transaction state for response
    fn update_transaction_state_for_response(
        &self,
        transaction: &mut SipTransaction,
        response: &Response,
    ) -> Result<()> {
        let status_code = response.status_code().code();

        match &mut transaction.state {
            TransactionState::Invite(invite_state) => {
                match invite_state {
                    InviteTransactionState::Calling => {
                        if status_code >= 100 && status_code < 200 {
                            *invite_state = InviteTransactionState::Proceeding;
                        } else if status_code >= 200 {
                            *invite_state = InviteTransactionState::Completed;
                        }
                    }
                    InviteTransactionState::Proceeding => {
                        if status_code >= 200 {
                            *invite_state = InviteTransactionState::Completed;
                        }
                    }
                    _ => {} // No state change for other states
                }
            }
            TransactionState::NonInvite(non_invite_state) => {
                match non_invite_state {
                    NonInviteTransactionState::Trying => {
                        if status_code >= 200 {
                            *non_invite_state = NonInviteTransactionState::Completed;
                        }
                    }
                    _ => {} // No state change for other states
                }
            }
        }

        // Add response to transaction
        transaction.responses.push(response.clone());

        Ok(())
    }

    /// Get dialog by ID
    pub fn get_dialog(&self, dialog_id: &str) -> Option<SipDialog> {
        self.dialogs.get(dialog_id).map(|d| d.clone())
    }

    /// Get transaction by ID
    pub fn get_transaction(&self, transaction_id: &str) -> Option<SipTransaction> {
        self.transactions.get(transaction_id).map(|t| t.clone())
    }

    /// Cleanup expired dialogs and transactions
    async fn cleanup_expired_state(
        dialogs: &Arc<DashMap<String, SipDialog>>,
        transactions: &Arc<DashMap<String, SipTransaction>>,
        max_dialog_lifetime: u64,
        max_transaction_lifetime: u64,
    ) {
        let now = chrono::Utc::now();
        let dialog_cutoff = now - chrono::Duration::seconds(max_dialog_lifetime as i64);
        let transaction_cutoff = now - chrono::Duration::seconds(max_transaction_lifetime as i64);

        // Cleanup expired dialogs
        let mut expired_dialogs = Vec::new();
        for entry in dialogs.iter() {
            if entry.last_activity < dialog_cutoff || entry.state == DialogState::Terminated {
                expired_dialogs.push(entry.key().clone());
            }
        }

        for dialog_id in expired_dialogs {
            dialogs.remove(&dialog_id);
            debug!("Cleaned up expired dialog: {}", dialog_id);
        }

        // Cleanup expired transactions
        let mut expired_transactions = Vec::new();
        for entry in transactions.iter() {
            if entry.created_at < transaction_cutoff {
                expired_transactions.push(entry.key().clone());
            }
        }

        for transaction_id in expired_transactions {
            transactions.remove(&transaction_id);
            debug!("Cleaned up expired transaction: {}", transaction_id);
        }
    }
}

/// Actions to take based on SIP message processing
#[derive(Debug, Clone)]
pub enum SipStateAction {
    /// Process new INVITE request
    ProcessNewInvite { transaction_id: String },
    /// Process re-INVITE within existing dialog
    ProcessReInvite {
        transaction_id: String,
        dialog_id: String,
    },
    /// Process ACK request
    ProcessAck { dialog_id: String },
    /// Process BYE request
    ProcessBye {
        transaction_id: String,
        dialog_id: String,
    },
    /// Process CANCEL request
    ProcessCancel {
        transaction_id: String,
        invite_transaction_id: String,
    },
    /// Process other request types
    ProcessOtherRequest {
        transaction_id: String,
        method: Method,
    },
    /// Process response
    ProcessResponse {
        transaction_id: String,
        dialog_id: Option<String>,
        requires_ack: bool,
    },
    /// Retransmit last response (for retransmitted requests)
    RetransmitLastResponse { transaction_id: String },
    /// Drop message (invalid or unknown)
    DropMessage,
}

/// Transaction timer manager
pub struct TransactionTimerManager {
    config: SipStateConfig,
    transactions: Arc<DashMap<String, SipTransaction>>,
}

impl TransactionTimerManager {
    pub fn new(config: SipStateConfig, transactions: Arc<DashMap<String, SipTransaction>>) -> Self {
        Self {
            config,
            transactions,
        }
    }

    /// Start timer management
    pub async fn start(&self) {
        let mut timer_interval = interval(Duration::from_millis(100)); // Check timers every 100ms

        loop {
            timer_interval.tick().await;
            self.process_timers().await;
        }
    }

    /// Process timer events
    async fn process_timers(&self) {
        let _now = chrono::Utc::now();

        for transaction in self.transactions.iter_mut() {
            // Check various timers based on transaction state
            match &transaction.state {
                TransactionState::Invite(invite_state) => {
                    match invite_state {
                        InviteTransactionState::Calling => {
                            // Timer A (INVITE retransmission)
                            // Timer B (INVITE timeout)
                        }
                        InviteTransactionState::Completed => {
                            // Timer D (wait for ACK)
                        }
                        _ => {}
                    }
                }
                TransactionState::NonInvite(non_invite_state) => {
                    match non_invite_state {
                        NonInviteTransactionState::Trying => {
                            // Timer E (non-INVITE retransmission)
                            // Timer F (non-INVITE timeout)
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
