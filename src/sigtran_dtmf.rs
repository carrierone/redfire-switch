/*
 * Sigtran DTMF/Telephony Event Support
 *
 * This module implements DTMF and telephony event handling for Sigtran protocols:
 * - M3UA (MTP3 User Adaptation Layer)
 * - SCCP (Signalling Connection Control Part)
 * - TCAP (Transaction Capabilities Application Part)
 * - ISUP (ISDN User Part) over Sigtran
 * - INAP (Intelligent Network Application Protocol)
 *
 * Supports various DTMF transport methods in SS7/Sigtran:
 * - ISUP User-to-User Information (UUI)
 * - ISUP Generic Digits Parameter
 * - INAP CollectedInfo
 * - TCAP Invoke components
 */

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

use crate::dtmf_processor::{DtmfEvent, DtmfSource};

/// Sigtran protocol types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SigtranProtocol {
    /// M3UA - MTP3 User Adaptation Layer
    M3ua,
    /// SUA - SCCP User Adaptation Layer
    Sua,
    /// IUA - ISDN Q.921 User Adaptation Layer
    Iua,
    /// V5UA - V5.2 User Adaptation Layer
    V5ua,
}

/// ISUP parameter types for DTMF/digits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsupParameterType {
    /// User-to-User Information (0x20)
    UserToUserInfo = 0x20,
    /// Generic Digits (0xC1)
    GenericDigits = 0xC1,
    /// Generic Notification Indicator (0x2C)
    GenericNotification = 0x2C,
    /// Operator Services Information (0xF6)
    OperatorServicesInfo = 0xF6,
}

/// ISUP Generic Digits encoding schemes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenericDigitsEncoding {
    /// BCD even (0)
    BcdEven = 0,
    /// BCD odd (1)
    BcdOdd = 1,
    /// IA5 character (2)
    Ia5Character = 2,
    /// Binary coded (3)
    BinaryCoded = 3,
}

/// ISUP Generic Digits type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenericDigitsType {
    /// Account code (0)
    AccountCode = 0,
    /// Authorization code (1)
    AuthorizationCode = 1,
    /// Private network travelling class mark (2)
    PrivateNetworkTravellingClassMark = 2,
    /// Business communication group identity (3)
    BusinessCommunicationGroupId = 3,
    /// Generic digits (4)
    GenericDigits = 4,
    /// Routing digits (5)
    RoutingDigits = 5,
    /// Called party number (6)
    CalledPartyNumber = 6,
    /// Calling party number (7)
    CallingPartyNumber = 7,
    /// DTMF digits (custom extension)
    DtmfDigits = 0xFF,
}

/// TCAP component types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TcapComponentType {
    /// Invoke (0xA1)
    Invoke = 0xA1,
    /// Return Result Last (0xA2)
    ReturnResultLast = 0xA2,
    /// Return Error (0xA3)
    ReturnError = 0xA3,
    /// Reject (0xA4)
    Reject = 0xA4,
}

/// INAP operation codes for digit collection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InapOperationCode {
    /// CollectInformation (2)
    CollectInformation = 2,
    /// PromptAndCollectUserInformation (23)
    PromptAndCollectUserInformation = 23,
    /// CollectedInformation (48)
    CollectedInformation = 48,
    /// SpecializedResourceReport (24)
    SpecializedResourceReport = 24,
}

/// Sigtran DTMF message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigtranDtmfMessage {
    /// Protocol used for transport
    pub protocol: SigtranProtocol,
    /// Message type
    pub message_type: SigtranDtmfMessageType,
    /// Collected digits
    pub digits: String,
    /// Encoding used
    pub encoding: GenericDigitsEncoding,
    /// Circuit identification code
    pub cic: Option<u32>,
    /// Transaction ID (for TCAP)
    pub transaction_id: Option<u32>,
    /// Additional parameters
    pub parameters: HashMap<String, Vec<u8>>,
}

/// Types of Sigtran DTMF messages
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SigtranDtmfMessageType {
    /// ISUP User-to-User Information
    IsupUserToUser,
    /// ISUP Generic Digits
    IsupGenericDigits,
    /// TCAP Invoke for digit collection
    TcapInvoke,
    /// TCAP Return Result with collected digits
    TcapReturnResult,
    /// INAP CollectedInformation
    InapCollectedInfo,
    /// INAP PromptAndCollect
    InapPromptAndCollect,
}

/// Sigtran DTMF processor
pub struct SigtranDtmfProcessor {
    /// Event sender for integration with DTMF processor
    event_sender: mpsc::UnboundedSender<DtmfEvent>,
    /// Active transactions
    active_transactions: Arc<RwLock<HashMap<u32, SigtranTransaction>>>,
    /// Protocol configuration
    config: SigtranDtmfConfig,
}

/// Sigtran transaction state
#[derive(Debug, Clone)]
struct SigtranTransaction {
    transaction_id: u32,
    protocol: SigtranProtocol,
    cic: Option<u32>,
    start_time: Instant,
    collected_digits: String,
    state: TransactionState,
}

/// Transaction states
#[derive(Debug, Clone, PartialEq, Eq)]
enum TransactionState {
    /// Waiting for digit collection
    DigitCollectionActive,
    /// Collection completed
    CollectionComplete,
    /// Transaction terminated
    Terminated,
}

/// Sigtran DTMF configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigtranDtmfConfig {
    /// Maximum digits to collect
    pub max_digits: u8,
    /// Collection timeout (seconds)
    pub collection_timeout: u32,
    /// Supported protocols
    pub supported_protocols: Vec<SigtranProtocol>,
    /// Default encoding for digits
    pub default_encoding: GenericDigitsEncoding,
}

impl Default for SigtranDtmfConfig {
    fn default() -> Self {
        Self {
            max_digits: 20,
            collection_timeout: 30,
            supported_protocols: vec![SigtranProtocol::M3ua, SigtranProtocol::Sua],
            default_encoding: GenericDigitsEncoding::Ia5Character,
        }
    }
}

impl SigtranDtmfProcessor {
    /// Create new Sigtran DTMF processor
    pub fn new(event_sender: mpsc::UnboundedSender<DtmfEvent>, config: SigtranDtmfConfig) -> Self {
        Self {
            event_sender,
            active_transactions: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Process incoming Sigtran DTMF message
    pub async fn process_incoming_message(&self, message: SigtranDtmfMessage) -> Result<()> {
        debug!(
            "Processing Sigtran DTMF message: {:?}",
            message.message_type
        );

        match message.message_type {
            SigtranDtmfMessageType::IsupGenericDigits => {
                self.process_isup_generic_digits(&message).await?;
            }
            SigtranDtmfMessageType::IsupUserToUser => {
                self.process_isup_user_to_user(&message).await?;
            }
            SigtranDtmfMessageType::TcapReturnResult => {
                self.process_tcap_return_result(&message).await?;
            }
            SigtranDtmfMessageType::InapCollectedInfo => {
                self.process_inap_collected_info(&message).await?;
            }
            _ => {
                debug!(
                    "Unhandled Sigtran DTMF message type: {:?}",
                    message.message_type
                );
            }
        }

        Ok(())
    }

    /// Process ISUP Generic Digits parameter
    async fn process_isup_generic_digits(&self, message: &SigtranDtmfMessage) -> Result<()> {
        let digits = self.decode_digits(&message.digits, message.encoding)?;

        // Generate DTMF events for each digit
        for (i, digit) in digits.chars().enumerate() {
            let dtmf_event = DtmfEvent::DigitDetected {
                digit,
                duration: Duration::from_millis(100), // Default duration
                timestamp: Instant::now(),
                confidence: 0.9, // High confidence from SS7
                source: DtmfSource::Sigtran,
            };

            if let Err(e) = self.event_sender.send(dtmf_event) {
                warn!("Failed to send DTMF event from Sigtran: {}", e);
            } else {
                debug!("Sigtran DTMF digit '{}' detected (position {})", digit, i);
            }
        }

        // Generate sequence complete event
        let sequence_event = DtmfEvent::SequenceComplete {
            sequence: digits.clone(),
            total_duration: Duration::from_millis(digits.len() as u64 * 100),
            source: DtmfSource::Sigtran,
        };

        if let Err(e) = self.event_sender.send(sequence_event) {
            warn!("Failed to send DTMF sequence complete event: {}", e);
        }

        info!(
            "Sigtran ISUP Generic Digits processed: '{}' (CIC: {:?})",
            digits, message.cic
        );

        Ok(())
    }

    /// Process ISUP User-to-User Information
    async fn process_isup_user_to_user(&self, message: &SigtranDtmfMessage) -> Result<()> {
        // UUI can contain DTMF digits in various formats
        let digits = &message.digits;

        // Try to extract DTMF digits from UUI content
        let extracted_digits = self.extract_dtmf_from_uui(digits)?;

        if !extracted_digits.is_empty() {
            for digit in extracted_digits.chars() {
                let dtmf_event = DtmfEvent::DigitDetected {
                    digit,
                    duration: Duration::from_millis(100),
                    timestamp: Instant::now(),
                    confidence: 0.8, // Moderate confidence from UUI
                    source: DtmfSource::Sigtran,
                };

                if let Err(e) = self.event_sender.send(dtmf_event) {
                    warn!("Failed to send DTMF event from UUI: {}", e);
                }
            }

            info!("Sigtran UUI DTMF processed: '{}'", extracted_digits);
        }

        Ok(())
    }

    /// Process TCAP Return Result with collected digits
    async fn process_tcap_return_result(&self, message: &SigtranDtmfMessage) -> Result<()> {
        if let Some(transaction_id) = message.transaction_id {
            let mut transactions = self.active_transactions.write().await;

            if let Some(transaction) = transactions.get_mut(&transaction_id) {
                transaction.collected_digits.push_str(&message.digits);
                transaction.state = TransactionState::CollectionComplete;

                // Generate DTMF events
                for digit in message.digits.chars() {
                    let dtmf_event = DtmfEvent::DigitDetected {
                        digit,
                        duration: Duration::from_millis(100),
                        timestamp: Instant::now(),
                        confidence: 0.95, // Very high confidence from TCAP
                        source: DtmfSource::Sigtran,
                    };

                    if let Err(e) = self.event_sender.send(dtmf_event) {
                        warn!("Failed to send DTMF event from TCAP: {}", e);
                    }
                }

                info!(
                    "TCAP digit collection completed: '{}' (TxID: {})",
                    transaction.collected_digits, transaction_id
                );
            }
        }

        Ok(())
    }

    /// Process INAP CollectedInformation
    async fn process_inap_collected_info(&self, message: &SigtranDtmfMessage) -> Result<()> {
        let digits = &message.digits;

        // INAP CollectedInformation contains the final collected digits
        for digit in digits.chars() {
            let dtmf_event = DtmfEvent::DigitDetected {
                digit,
                duration: Duration::from_millis(100),
                timestamp: Instant::now(),
                confidence: 0.95, // Very high confidence from INAP
                source: DtmfSource::Sigtran,
            };

            if let Err(e) = self.event_sender.send(dtmf_event) {
                warn!("Failed to send DTMF event from INAP: {}", e);
            }
        }

        let sequence_event = DtmfEvent::SequenceComplete {
            sequence: digits.clone(),
            total_duration: Duration::from_millis(digits.len() as u64 * 100),
            source: DtmfSource::Sigtran,
        };

        if let Err(e) = self.event_sender.send(sequence_event) {
            warn!("Failed to send INAP sequence complete event: {}", e);
        }

        info!("INAP CollectedInformation processed: '{}'", digits);

        Ok(())
    }

    /// Generate outgoing Sigtran DTMF message
    pub async fn generate_outgoing_message(
        &self,
        protocol: SigtranProtocol,
        message_type: SigtranDtmfMessageType,
        digits: &str,
        cic: Option<u32>,
    ) -> Result<SigtranDtmfMessage> {
        let message = SigtranDtmfMessage {
            protocol,
            message_type: message_type.clone(),
            digits: digits.to_string(),
            encoding: self.config.default_encoding,
            cic,
            transaction_id: None,
            parameters: HashMap::new(),
        };

        info!(
            "Generated Sigtran DTMF message: {:?} with digits '{}'",
            message_type, digits
        );

        Ok(message)
    }

    /// Create ISUP Generic Digits parameter
    pub fn create_isup_generic_digits(
        &self,
        digits: &str,
        digits_type: GenericDigitsType,
    ) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        // Parameter header
        data.push(IsupParameterType::GenericDigits as u8);

        // Encode digits based on configuration
        let encoded_digits = self.encode_digits(digits, self.config.default_encoding)?;

        // Parameter length
        data.push((encoded_digits.len() + 1) as u8); // +1 for type/encoding byte

        // Type of digits and encoding scheme
        let type_encoding = ((digits_type as u8) << 4) | (self.config.default_encoding as u8);
        data.push(type_encoding);

        // Encoded digits
        data.extend(encoded_digits);

        Ok(data)
    }

    /// Create TCAP Invoke for digit collection
    pub fn create_tcap_invoke_collect_digits(
        &self,
        transaction_id: u32,
        max_digits: u8,
    ) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        // TCAP component tag
        data.push(TcapComponentType::Invoke as u8);

        // Component length (will be calculated)
        let length_pos = data.len();
        data.push(0); // Placeholder

        // Invoke ID
        data.push(0x02); // INTEGER tag
        data.push(0x01); // Length
        data.push((transaction_id & 0xFF) as u8);

        // Operation code
        data.push(0x02); // INTEGER tag
        data.push(0x01); // Length
        data.push(InapOperationCode::PromptAndCollectUserInformation as u8);

        // Parameters (simplified)
        data.push(0x30); // SEQUENCE tag
        data.push(0x03); // Length
        data.push(0x02); // INTEGER tag (max digits)
        data.push(0x01); // Length
        data.push(max_digits);

        // Update length
        data[length_pos] = (data.len() - length_pos - 1) as u8;

        Ok(data)
    }

    /// Decode digits from various encodings
    fn decode_digits(&self, encoded: &str, encoding: GenericDigitsEncoding) -> Result<String> {
        match encoding {
            GenericDigitsEncoding::Ia5Character => {
                // Already ASCII/IA5
                Ok(encoded
                    .chars()
                    .filter(|c| c.is_ascii_digit() || "ABCD*#".contains(*c))
                    .collect())
            }
            GenericDigitsEncoding::BcdEven | GenericDigitsEncoding::BcdOdd => {
                self.decode_bcd(encoded.as_bytes())
            }
            GenericDigitsEncoding::BinaryCoded => {
                // Custom binary format
                self.decode_binary(encoded)
            }
        }
    }

    /// Encode digits to specified format
    fn encode_digits(&self, digits: &str, encoding: GenericDigitsEncoding) -> Result<Vec<u8>> {
        match encoding {
            GenericDigitsEncoding::Ia5Character => Ok(digits.as_bytes().to_vec()),
            GenericDigitsEncoding::BcdEven | GenericDigitsEncoding::BcdOdd => {
                self.encode_bcd(digits, encoding == GenericDigitsEncoding::BcdOdd)
            }
            GenericDigitsEncoding::BinaryCoded => self.encode_binary(digits),
        }
    }

    /// Decode BCD digits
    fn decode_bcd(&self, bytes: &[u8]) -> Result<String> {
        let mut digits = String::new();

        for &byte in bytes {
            let high_nibble = (byte >> 4) & 0x0F;
            let low_nibble = byte & 0x0F;

            if high_nibble <= 9 {
                digits.push((b'0' + high_nibble) as char);
            } else if high_nibble == 0x0A {
                digits.push('*');
            } else if high_nibble == 0x0B {
                digits.push('#');
            } else if high_nibble >= 0x0C && high_nibble <= 0x0F {
                digits.push((b'A' + (high_nibble - 0x0C)) as char);
            }

            if low_nibble <= 9 {
                digits.push((b'0' + low_nibble) as char);
            } else if low_nibble == 0x0A {
                digits.push('*');
            } else if low_nibble == 0x0B {
                digits.push('#');
            } else if low_nibble >= 0x0C && low_nibble <= 0x0F {
                digits.push((b'A' + (low_nibble - 0x0C)) as char);
            }
        }

        Ok(digits)
    }

    /// Encode digits as BCD
    fn encode_bcd(&self, digits: &str, odd_length: bool) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        let chars: Vec<char> = digits.chars().collect();

        for i in (0..chars.len()).step_by(2) {
            let high_nibble = self.char_to_bcd_nibble(chars[i])?;
            let low_nibble = if i + 1 < chars.len() {
                self.char_to_bcd_nibble(chars[i + 1])?
            } else if odd_length {
                0x0F // Padding for odd length
            } else {
                0x00
            };

            bytes.push((high_nibble << 4) | low_nibble);
        }

        Ok(bytes)
    }

    /// Convert character to BCD nibble
    fn char_to_bcd_nibble(&self, c: char) -> Result<u8> {
        match c {
            '0'..='9' => Ok((c as u8) - b'0'),
            '*' => Ok(0x0A),
            '#' => Ok(0x0B),
            'A'..='D' => Ok((c as u8) - b'A' + 0x0C),
            'a'..='d' => Ok((c as u8) - b'a' + 0x0C),
            _ => Err(anyhow!("Invalid BCD digit: {}", c)),
        }
    }

    /// Decode binary format (custom implementation)
    fn decode_binary(&self, encoded: &str) -> Result<String> {
        // Implementation depends on specific binary encoding used
        // This is a placeholder for custom binary formats
        Ok(encoded.to_string())
    }

    /// Encode as binary format (custom implementation)
    fn encode_binary(&self, digits: &str) -> Result<Vec<u8>> {
        // Implementation depends on specific binary encoding used
        // This is a placeholder for custom binary formats
        Ok(digits.as_bytes().to_vec())
    }

    /// Extract DTMF digits from UUI content
    fn extract_dtmf_from_uui(&self, uui_content: &str) -> Result<String> {
        // UUI can contain various formats. This is a simplified extraction.
        // In practice, you'd parse the specific UUI format being used.
        Ok(uui_content
            .chars()
            .filter(|c| c.is_ascii_digit() || "ABCD*#".contains(*c))
            .collect())
    }

    /// Start digit collection transaction
    pub async fn start_digit_collection(&self, protocol: SigtranProtocol, cic: Option<u32>) -> u32 {
        let transaction_id = self.generate_transaction_id().await;
        let transaction = SigtranTransaction {
            transaction_id,
            protocol,
            cic,
            start_time: Instant::now(),
            collected_digits: String::new(),
            state: TransactionState::DigitCollectionActive,
        };

        let mut transactions = self.active_transactions.write().await;
        transactions.insert(transaction_id, transaction);

        info!(
            "Started Sigtran digit collection transaction: {} (CIC: {:?})",
            transaction_id, cic
        );

        transaction_id
    }

    /// Generate unique transaction ID
    async fn generate_transaction_id(&self) -> u32 {
        use std::sync::atomic::{AtomicU32, Ordering};
        static TRANSACTION_COUNTER: AtomicU32 = AtomicU32::new(1);
        TRANSACTION_COUNTER.fetch_add(1, Ordering::SeqCst)
    }

    /// Clean up expired transactions
    pub async fn cleanup_expired_transactions(&self) {
        let mut transactions = self.active_transactions.write().await;
        let now = Instant::now();
        let timeout = Duration::from_secs(self.config.collection_timeout.into());
        let mut to_remove = Vec::new();

        for (transaction_id, transaction) in transactions.iter() {
            if now.duration_since(transaction.start_time) > timeout {
                to_remove.push(*transaction_id);
            }
        }

        for transaction_id in to_remove {
            transactions.remove(&transaction_id);
            warn!("Cleaned up expired Sigtran transaction: {}", transaction_id);
        }
    }

    /// Get transaction statistics
    pub async fn get_transaction_stats(&self) -> Vec<SigtranTransactionStats> {
        let transactions = self.active_transactions.read().await;
        transactions
            .values()
            .map(|tx| SigtranTransactionStats {
                transaction_id: tx.transaction_id,
                protocol: tx.protocol,
                cic: tx.cic,
                start_time: tx.start_time,
                collected_digits: tx.collected_digits.clone(),
                state: format!("{:?}", tx.state),
            })
            .collect()
    }
}

/// Sigtran transaction statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigtranTransactionStats {
    pub transaction_id: u32,
    pub protocol: SigtranProtocol,
    pub cic: Option<u32>,
    #[serde(skip, default = "std::time::Instant::now")]
    pub start_time: Instant,
    pub collected_digits: String,
    pub state: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_sigtran_dtmf_processor() {
        let (event_sender, mut event_receiver) = mpsc::unbounded_channel();
        let config = SigtranDtmfConfig::default();
        let processor = SigtranDtmfProcessor::new(event_sender, config);

        // Test ISUP Generic Digits
        let message = SigtranDtmfMessage {
            protocol: SigtranProtocol::M3ua,
            message_type: SigtranDtmfMessageType::IsupGenericDigits,
            digits: "12345".to_string(),
            encoding: GenericDigitsEncoding::Ia5Character,
            cic: Some(100),
            transaction_id: None,
            parameters: HashMap::new(),
        };

        processor.process_incoming_message(message).await.unwrap();

        // Should receive multiple DTMF events
        for expected_digit in "12345".chars() {
            let event = event_receiver.recv().await.unwrap();
            match event {
                DtmfEvent::DigitDetected { digit, source, .. } => {
                    assert_eq!(digit, expected_digit);
                    assert_eq!(source, DtmfSource::Sigtran);
                }
                _ => assert!(false, "Expected DigitDetected event, got: {:?}", event),
            }
        }

        // Should also receive sequence complete event
        let seq_event = event_receiver.recv().await.unwrap();
        match seq_event {
            DtmfEvent::SequenceComplete {
                sequence, source, ..
            } => {
                assert_eq!(sequence, "12345");
                assert_eq!(source, DtmfSource::Sigtran);
            }
            _ => assert!(false, "Expected SequenceComplete event, got: {:?}", seq_event),
        }
    }

    #[test]
    fn test_bcd_encoding() {
        let config = SigtranDtmfConfig::default();
        let (event_sender, _) = mpsc::unbounded_channel();
        let processor = SigtranDtmfProcessor::new(event_sender, config);

        // Test BCD encoding
        let encoded = processor.encode_bcd("123*#A", false).unwrap();
        assert_eq!(encoded.len(), 3); // 6 digits = 3 bytes

        // Test BCD decoding
        let decoded = processor.decode_bcd(&encoded).unwrap();
        assert_eq!(decoded, "123*#A");
    }

    #[test]
    fn test_isup_parameter_creation() {
        let config = SigtranDtmfConfig::default();
        let (event_sender, _) = mpsc::unbounded_channel();
        let processor = SigtranDtmfProcessor::new(event_sender, config);

        let param = processor
            .create_isup_generic_digits("123", GenericDigitsType::DtmfDigits)
            .unwrap();
        assert!(!param.is_empty());
        assert_eq!(param[0], IsupParameterType::GenericDigits as u8);
    }

    #[tokio::test]
    async fn test_transaction_management() {
        let (event_sender, _) = mpsc::unbounded_channel();
        let config = SigtranDtmfConfig::default();
        let processor = SigtranDtmfProcessor::new(event_sender, config);

        // Start transaction
        let tx_id = processor
            .start_digit_collection(SigtranProtocol::M3ua, Some(123))
            .await;
        assert!(tx_id > 0);

        // Check transaction exists
        let stats = processor.get_transaction_stats().await;
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].transaction_id, tx_id);
        assert_eq!(stats[0].cic, Some(123));
    }
}
