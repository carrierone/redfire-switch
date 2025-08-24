/*
 * Redfire Switch - SIP-T and SIP-I Protocol Support
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

use anyhow::{anyhow, Result};
use bitflags::bitflags;
use nom::{
    number::complete::{be_u16, be_u8},
    sequence::tuple,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use tracing::{debug, info, warn};

/// SIP-T (SIP for Telephones) and SIP-I (SIP with encapsulated ISUP) support
///
/// SIP-T: RFC 3372 - Session Initiation Protocol for Telephones (SIP-T):
///        Context and Architectures
/// SIP-I: RFC 3398 - Integrated Services Digital Network (ISDN) User Part (ISUP)
///        to Session Initiation Protocol (SIP) Interworking

/// ISUP message types as defined in ITU-T Q.763
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum IsupMessageType {
    /// Initial Address Message
    IAM = 0x01,
    /// Subsequent Address Message  
    SAM = 0x02,
    /// Information Request
    INR = 0x03,
    /// Information
    INF = 0x04,
    /// Continuity
    COT = 0x05,
    /// Address Complete Message
    ACM = 0x06,
    /// Connect
    CON = 0x07,
    /// Forward Transfer
    FOT = 0x08,
    /// Answer
    ANM = 0x09,
    /// Release
    REL = 0x0C,
    /// Suspend
    SUS = 0x0D,
    /// Resume
    RES = 0x0E,
    /// Release Complete
    RLC = 0x10,
    /// Continuity Check Request
    CCR = 0x11,
    /// Reset Circuit
    RSC = 0x12,
    /// Blocking
    BLO = 0x13,
    /// Unblocking
    UBL = 0x14,
    /// Blocking Acknowledgement
    BLA = 0x15,
    /// Unblocking Acknowledgement
    UBA = 0x16,
    /// Circuit Group Reset
    GRS = 0x17,
    /// Circuit Group Reset Acknowledgement
    GRA = 0x29,
    /// Circuit Group Blocking
    CGB = 0x18,
    /// Circuit Group Unblocking
    CGU = 0x19,
    /// Circuit Group Blocking Acknowledgement
    CGBA = 0x1A,
    /// Circuit Group Unblocking Acknowledgement
    CGUA = 0x1B,
    /// Call Progress
    CPG = 0x2C,
    /// User-to-User Information
    USR = 0x2D,
    /// Unequipped Circuit Identification Code
    UCIC = 0x2E,
    /// Confusion
    CFN = 0x2F,
    /// Overload
    OLM = 0x30,
    /// Charge Information
    CRG = 0x31,
    /// Network Resource Management
    NRM = 0x32,
    /// Facility
    FAC = 0x33,
    /// User Part Test
    UPT = 0x34,
    /// User Part Available
    UPA = 0x35,
    /// Identification Request
    IDR = 0x36,
    /// Identification Response
    IRS = 0x37,
    /// Segmentation
    SGM = 0x38,
    /// Loop Prevention
    LPR = 0x40,
    /// Application Transport
    APT = 0x41,
    /// Pre-release Information
    PRI = 0x42,
    /// Subsequent Directory Number
    SDN = 0x43,
}

impl From<u8> for IsupMessageType {
    fn from(value: u8) -> Self {
        match value {
            0x01 => IsupMessageType::IAM,
            0x02 => IsupMessageType::SAM,
            0x03 => IsupMessageType::INR,
            0x04 => IsupMessageType::INF,
            0x05 => IsupMessageType::COT,
            0x06 => IsupMessageType::ACM,
            0x07 => IsupMessageType::CON,
            0x08 => IsupMessageType::FOT,
            0x09 => IsupMessageType::ANM,
            0x0C => IsupMessageType::REL,
            0x0D => IsupMessageType::SUS,
            0x0E => IsupMessageType::RES,
            0x10 => IsupMessageType::RLC,
            0x11 => IsupMessageType::CCR,
            0x12 => IsupMessageType::RSC,
            0x13 => IsupMessageType::BLO,
            0x14 => IsupMessageType::UBL,
            0x15 => IsupMessageType::BLA,
            0x16 => IsupMessageType::UBA,
            0x17 => IsupMessageType::GRS,
            0x29 => IsupMessageType::GRA,
            0x18 => IsupMessageType::CGB,
            0x19 => IsupMessageType::CGU,
            0x1A => IsupMessageType::CGBA,
            0x1B => IsupMessageType::CGUA,
            0x2C => IsupMessageType::CPG,
            0x2D => IsupMessageType::USR,
            0x2E => IsupMessageType::UCIC,
            0x2F => IsupMessageType::CFN,
            0x30 => IsupMessageType::OLM,
            0x31 => IsupMessageType::CRG,
            0x32 => IsupMessageType::NRM,
            0x33 => IsupMessageType::FAC,
            0x34 => IsupMessageType::UPT,
            0x35 => IsupMessageType::UPA,
            0x36 => IsupMessageType::IDR,
            0x37 => IsupMessageType::IRS,
            0x38 => IsupMessageType::SGM,
            0x40 => IsupMessageType::LPR,
            0x41 => IsupMessageType::APT,
            0x42 => IsupMessageType::PRI,
            0x43 => IsupMessageType::SDN,
            _ => IsupMessageType::IAM, // Default fallback
        }
    }
}

impl fmt::Display for IsupMessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            IsupMessageType::IAM => "Initial Address Message",
            IsupMessageType::SAM => "Subsequent Address Message",
            IsupMessageType::INR => "Information Request",
            IsupMessageType::INF => "Information",
            IsupMessageType::COT => "Continuity",
            IsupMessageType::ACM => "Address Complete Message",
            IsupMessageType::CON => "Connect",
            IsupMessageType::FOT => "Forward Transfer",
            IsupMessageType::ANM => "Answer",
            IsupMessageType::REL => "Release",
            IsupMessageType::SUS => "Suspend",
            IsupMessageType::RES => "Resume",
            IsupMessageType::RLC => "Release Complete",
            IsupMessageType::CCR => "Continuity Check Request",
            IsupMessageType::RSC => "Reset Circuit",
            IsupMessageType::BLO => "Blocking",
            IsupMessageType::UBL => "Unblocking",
            IsupMessageType::BLA => "Blocking Acknowledgement",
            IsupMessageType::UBA => "Unblocking Acknowledgement",
            IsupMessageType::GRS => "Circuit Group Reset",
            IsupMessageType::GRA => "Circuit Group Reset Acknowledgement",
            IsupMessageType::CGB => "Circuit Group Blocking",
            IsupMessageType::CGU => "Circuit Group Unblocking",
            IsupMessageType::CGBA => "Circuit Group Blocking Acknowledgement",
            IsupMessageType::CGUA => "Circuit Group Unblocking Acknowledgement",
            IsupMessageType::CPG => "Call Progress",
            IsupMessageType::USR => "User-to-User Information",
            IsupMessageType::UCIC => "Unequipped Circuit Identification Code",
            IsupMessageType::CFN => "Confusion",
            IsupMessageType::OLM => "Overload",
            IsupMessageType::CRG => "Charge Information",
            IsupMessageType::NRM => "Network Resource Management",
            IsupMessageType::FAC => "Facility",
            IsupMessageType::UPT => "User Part Test",
            IsupMessageType::UPA => "User Part Available",
            IsupMessageType::IDR => "Identification Request",
            IsupMessageType::IRS => "Identification Response",
            IsupMessageType::SGM => "Segmentation",
            IsupMessageType::LPR => "Loop Prevention",
            IsupMessageType::APT => "Application Transport",
            IsupMessageType::PRI => "Pre-release Information",
            IsupMessageType::SDN => "Subsequent Directory Number",
        };
        write!(f, "{}", name)
    }
}

/// ISUP parameter types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum IsupParameterType {
    /// End of Optional Parameters
    EndOfOptionalParameters = 0x00,
    /// Call Reference
    CallReference = 0x01,
    /// Transmission Medium Requirement
    TransmissionMediumRequirement = 0x02,
    /// Access Transport
    AccessTransport = 0x03,
    /// Called Party Number
    CalledPartyNumber = 0x04,
    /// Subsequent Number
    SubsequentNumber = 0x05,
    /// Nature of Connection Indicators
    NatureOfConnectionIndicators = 0x06,
    /// Forward Call Indicators
    ForwardCallIndicators = 0x07,
    /// Optional Forward Call Indicators
    OptionalForwardCallIndicators = 0x08,
    /// Calling Party Category
    CallingPartyCategory = 0x09,
    /// Calling Party Number
    CallingPartyNumber = 0x0A,
    /// Redirecting Number
    RedirectingNumber = 0x0B,
    /// Redirection Number
    RedirectionNumber = 0x0C,
    /// Connection Request
    ConnectionRequest = 0x0D,
    /// Information Request Indicators
    InformationRequestIndicators = 0x0E,
    /// Information Indicators
    InformationIndicators = 0x0F,
    /// Continuity Indicators
    ContinuityIndicators = 0x10,
    /// Backward Call Indicators
    BackwardCallIndicators = 0x11,
    /// Cause Indicators
    CauseIndicators = 0x12,
    /// Redirection Information
    RedirectionInformation = 0x13,
    /// Circuit Group Supervision Message Type Indicator
    CircuitGroupSupervisionMessageTypeIndicator = 0x15,
    /// Range and Status
    RangeAndStatus = 0x16,
    /// Facility Indicator
    FacilityIndicator = 0x18,
    /// User Service Information
    UserServiceInformation = 0x1D,
    /// Signalling Point Code
    SignallingPointCode = 0x1E,
    /// User to User Information
    UserToUserInformation = 0x20,
    /// Connected Number
    ConnectedNumber = 0x21,
    /// Suspend Resume Indicators
    SuspendResumeIndicators = 0x22,
    /// Transit Network Selection
    TransitNetworkSelection = 0x23,
    /// Event Information
    EventInformation = 0x24,
    /// Circuit Assignment Map
    CircuitAssignmentMap = 0x25,
    /// Circuit State Indicator
    CircuitStateIndicator = 0x26,
    /// Automatic Congestion Level
    AutomaticCongestionLevel = 0x27,
    /// Optional Backward Call Indicators
    OptionalBackwardCallIndicators = 0x29,
    /// User to User Indicators
    UserToUserIndicators = 0x2A,
    /// Origination ISC Point Code
    OriginationIscPointCode = 0x2B,
    /// Generic Notification Indicator
    GenericNotificationIndicator = 0x2C,
    /// Call History Information
    CallHistoryInformation = 0x2D,
    /// Access Delivery Information
    AccessDeliveryInformation = 0x2E,
    /// Network Specific Facility
    NetworkSpecificFacility = 0x2F,
    /// User Service Information Prime
    UserServiceInformationPrime = 0x30,
    /// Propagation Delay Counter
    PropagationDelayCounter = 0x31,
    /// Remote Operations
    RemoteOperations = 0x32,
    /// Service Activation
    ServiceActivation = 0x33,
    /// User Teleservice Information
    UserTeleserviceInformation = 0x34,
    /// Transmission Medium Used
    TransmissionMediumUsed = 0x35,
    /// Call Diversion Information
    CallDiversionInformation = 0x36,
    /// Echo Control Information
    EchoControlInformation = 0x37,
    /// Message Compatibility Information
    MessageCompatibilityInformation = 0x38,
    /// Parameter Compatibility Information
    ParameterCompatibilityInformation = 0x39,
    /// MLPP Precedence
    MlppPrecedence = 0x3A,
    /// MCID Request Indicators
    McidRequestIndicators = 0x3B,
    /// MCID Response Indicators
    McidResponseIndicators = 0x3C,
    /// Hop Counter
    HopCounter = 0x3D,
    /// Transmission Medium Requirement Prime
    TransmissionMediumRequirementPrime = 0x3E,
    /// Location Number
    LocationNumber = 0x3F,
    /// Redirection Number Restriction
    RedirectionNumberRestriction = 0x40,
    /// Call Transfer Reference
    CallTransferReference = 0x43,
    /// Loop Prevention Indicators
    LoopPreventionIndicators = 0x44,
    /// Call Transfer Number
    CallTransferNumber = 0x45,
    /// CCSS
    Ccss = 0x4B,
    /// Forward GVNS
    ForwardGvns = 0x4C,
    /// Backward GVNS
    BackwardGvns = 0x4D,
    /// Redirect Capability
    RedirectCapability = 0x4E,
    /// Network Management Controls
    NetworkManagementControls = 0x5B,
    /// Circuit Identification Code
    CircuitIdentificationCode = 0x5C,
    /// SCCP Method
    SccpMethod = 0x83,
}

impl From<u8> for IsupParameterType {
    fn from(value: u8) -> Self {
        match value {
            0x00 => IsupParameterType::EndOfOptionalParameters,
            0x01 => IsupParameterType::CallReference,
            0x02 => IsupParameterType::TransmissionMediumRequirement,
            0x03 => IsupParameterType::AccessTransport,
            0x04 => IsupParameterType::CalledPartyNumber,
            0x05 => IsupParameterType::SubsequentNumber,
            0x06 => IsupParameterType::NatureOfConnectionIndicators,
            0x07 => IsupParameterType::ForwardCallIndicators,
            0x08 => IsupParameterType::OptionalForwardCallIndicators,
            0x09 => IsupParameterType::CallingPartyCategory,
            0x0A => IsupParameterType::CallingPartyNumber,
            0x0B => IsupParameterType::RedirectingNumber,
            0x0C => IsupParameterType::RedirectionNumber,
            0x0D => IsupParameterType::ConnectionRequest,
            0x0E => IsupParameterType::InformationRequestIndicators,
            0x0F => IsupParameterType::InformationIndicators,
            0x10 => IsupParameterType::ContinuityIndicators,
            0x11 => IsupParameterType::BackwardCallIndicators,
            0x12 => IsupParameterType::CauseIndicators,
            0x13 => IsupParameterType::RedirectionInformation,
            0x15 => IsupParameterType::CircuitGroupSupervisionMessageTypeIndicator,
            0x16 => IsupParameterType::RangeAndStatus,
            0x18 => IsupParameterType::FacilityIndicator,
            0x1D => IsupParameterType::UserServiceInformation,
            0x1E => IsupParameterType::SignallingPointCode,
            0x20 => IsupParameterType::UserToUserInformation,
            0x21 => IsupParameterType::ConnectedNumber,
            0x22 => IsupParameterType::SuspendResumeIndicators,
            0x23 => IsupParameterType::TransitNetworkSelection,
            0x24 => IsupParameterType::EventInformation,
            0x25 => IsupParameterType::CircuitAssignmentMap,
            0x26 => IsupParameterType::CircuitStateIndicator,
            0x27 => IsupParameterType::AutomaticCongestionLevel,
            0x29 => IsupParameterType::OptionalBackwardCallIndicators,
            0x2A => IsupParameterType::UserToUserIndicators,
            0x2B => IsupParameterType::OriginationIscPointCode,
            0x2C => IsupParameterType::GenericNotificationIndicator,
            0x2D => IsupParameterType::CallHistoryInformation,
            0x2E => IsupParameterType::AccessDeliveryInformation,
            0x2F => IsupParameterType::NetworkSpecificFacility,
            0x30 => IsupParameterType::UserServiceInformationPrime,
            0x31 => IsupParameterType::PropagationDelayCounter,
            0x32 => IsupParameterType::RemoteOperations,
            0x33 => IsupParameterType::ServiceActivation,
            0x34 => IsupParameterType::UserTeleserviceInformation,
            0x35 => IsupParameterType::TransmissionMediumUsed,
            0x36 => IsupParameterType::CallDiversionInformation,
            0x37 => IsupParameterType::EchoControlInformation,
            0x38 => IsupParameterType::MessageCompatibilityInformation,
            0x39 => IsupParameterType::ParameterCompatibilityInformation,
            0x3A => IsupParameterType::MlppPrecedence,
            0x3B => IsupParameterType::McidRequestIndicators,
            0x3C => IsupParameterType::McidResponseIndicators,
            0x3D => IsupParameterType::HopCounter,
            0x3E => IsupParameterType::TransmissionMediumRequirementPrime,
            0x3F => IsupParameterType::LocationNumber,
            0x40 => IsupParameterType::RedirectionNumberRestriction,
            0x43 => IsupParameterType::CallTransferReference,
            0x44 => IsupParameterType::LoopPreventionIndicators,
            0x45 => IsupParameterType::CallTransferNumber,
            0x4B => IsupParameterType::Ccss,
            0x4C => IsupParameterType::ForwardGvns,
            0x4D => IsupParameterType::BackwardGvns,
            0x4E => IsupParameterType::RedirectCapability,
            0x5B => IsupParameterType::NetworkManagementControls,
            0x5C => IsupParameterType::CircuitIdentificationCode,
            0x83 => IsupParameterType::SccpMethod,
            _ => IsupParameterType::EndOfOptionalParameters, // Default fallback
        }
    }
}

/// ISUP parameter with raw data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsupParameter {
    pub param_type: IsupParameterType,
    pub length: u8,
    pub data: Vec<u8>,
}

/// ISUP message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsupMessage {
    /// Circuit Identification Code
    pub cic: u16,
    /// Message type
    pub message_type: IsupMessageType,
    /// Mandatory fixed parameters
    pub mandatory_fixed: Vec<u8>,
    /// Mandatory variable parameters
    pub mandatory_variable: Vec<IsupParameter>,
    /// Optional parameters
    pub optional: Vec<IsupParameter>,
    /// Raw message data for debugging
    pub raw_data: Vec<u8>,
}

bitflags! {
    /// Forward Call Indicators as defined in ITU-T Q.763
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ForwardCallIndicators: u16 {
        /// National/international call indicator
        const NATIONAL_CALL = 0x0001;
        /// End-to-end method indicator
        const END_TO_END_METHOD_AVAILABLE = 0x0002;
        /// Interworking indicator
        const INTERWORKING_ENCOUNTERED = 0x0004;
        /// End-to-end information indicator
        const END_TO_END_INFO_AVAILABLE = 0x0008;
        /// ISDN User Part indicator
        const ISDN_USER_PART_ALL_THE_WAY = 0x0010;
        /// ISDN User Part preference indicator
        const ISDN_USER_PART_PREFERRED = 0x0020;
        /// ISDN access indicator
        const ISDN_ACCESS = 0x0040;
        /// SCCP method indicator
        const SCCP_METHOD_USED = 0x0080;
    }
}

bitflags! {
    /// Backward Call Indicators as defined in ITU-T Q.763
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BackwardCallIndicators: u16 {
        /// Charge indicator
        const CHARGE_TO_BE_APPLIED = 0x0001;
        /// Called party status indicator
        const CALLED_PARTY_SUBSCRIBER = 0x0002;
        /// Called party category indicator
        const CALLED_PARTY_ORDINARY = 0x0004;
        /// End-to-end method indicator
        const END_TO_END_METHOD_AVAILABLE = 0x0008;
        /// Interworking indicator
        const INTERWORKING_ENCOUNTERED = 0x0010;
        /// End-to-end information indicator
        const END_TO_END_INFO_AVAILABLE = 0x0020;
        /// ISDN User Part indicator
        const ISDN_USER_PART_ALL_THE_WAY = 0x0040;
        /// Holding indicator
        const HOLDING_REQUESTED = 0x0080;
        /// ISDN access indicator
        const ISDN_ACCESS = 0x0100;
        /// Echo control device indicator
        const ECHO_CONTROL_INCLUDED = 0x0200;
        /// SCCP method indicator
        const SCCP_METHOD_USED = 0x0400;
    }
}

/// SIP-T/SIP-I configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SipTSipIConfig {
    /// Enable SIP-T support
    pub sipt_enabled: bool,
    /// Enable SIP-I support  
    pub sipi_enabled: bool,
    /// ISUP variant (ITU-T, ANSI, etc.)
    pub isup_variant: IsupVariant,
    /// Default originating point code
    pub originating_point_code: u32,
    /// Default destination point code
    pub destination_point_code: u32,
    /// Circuit identification code range start
    pub cic_range_start: u16,
    /// Circuit identification code range end
    pub cic_range_end: u16,
    /// Enable ISUP message validation
    pub validate_isup: bool,
    /// Enable multipart MIME support for SIP-T
    pub multipart_support: bool,
    /// Maximum ISUP message size
    pub max_isup_size: usize,
}

impl Default for SipTSipIConfig {
    fn default() -> Self {
        SipTSipIConfig {
            sipt_enabled: false,
            sipi_enabled: false,
            isup_variant: IsupVariant::Itu,
            originating_point_code: 0x001234,
            destination_point_code: 0x005678,
            cic_range_start: 1,
            cic_range_end: 4095,
            validate_isup: true,
            multipart_support: true,
            max_isup_size: 65535,
        }
    }
}

/// ISUP variant types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IsupVariant {
    /// ITU-T Q.763 (International)
    Itu,
    /// ANSI T1.113 (North American)
    Ansi,
    /// ETSI ETS 300 356 (European)
    Etsi,
    /// China Communications Standard
    China,
}

impl fmt::Display for IsupVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IsupVariant::Itu => write!(f, "ITU-T"),
            IsupVariant::Ansi => write!(f, "ANSI"),
            IsupVariant::Etsi => write!(f, "ETSI"),
            IsupVariant::China => write!(f, "China"),
        }
    }
}

/// SIP-T/SIP-I service for handling ISUP encapsulation
pub struct SipTSipIService {
    config: SipTSipIConfig,
}

impl SipTSipIService {
    /// Create a new SIP-T/SIP-I service
    pub fn new(config: SipTSipIConfig) -> Self {
        info!(
            "Initializing SIP-T/SIP-I service - SIP-T: {}, SIP-I: {}, Variant: {}",
            config.sipt_enabled, config.sipi_enabled, config.isup_variant
        );

        Self { config }
    }

    /// Parse ISUP message from binary data
    pub fn parse_isup_message(&self, data: &[u8]) -> Result<IsupMessage> {
        if data.len() < 3 {
            return Err(anyhow!("ISUP message too short"));
        }

        let (remaining, (cic, message_type_byte)) =
            tuple((be_u16, be_u8))(data).map_err(|e: nom::Err<nom::error::Error<_>>| {
                anyhow!("Failed to parse ISUP header: {}", e)
            })?;

        let message_type = IsupMessageType::from(message_type_byte);

        debug!(
            "Parsing ISUP message: CIC={}, Type={} (0x{:02X})",
            cic, message_type, message_type_byte
        );

        // Parse mandatory fixed parameters based on message type
        let (remaining, mandatory_fixed) = self.parse_mandatory_fixed(&message_type, remaining)?;

        // Parse mandatory variable parameters
        let (remaining, mandatory_variable) =
            self.parse_mandatory_variable(&message_type, &remaining)?;

        // Parse optional parameters
        let optional = self.parse_optional_parameters(&remaining)?;

        Ok(IsupMessage {
            cic,
            message_type,
            mandatory_fixed,
            mandatory_variable,
            optional,
            raw_data: data.to_vec(),
        })
    }

    /// Parse mandatory fixed parameters
    fn parse_mandatory_fixed(
        &self,
        message_type: &IsupMessageType,
        data: &[u8],
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        let fixed_length = self.get_mandatory_fixed_length(message_type);

        if data.len() < fixed_length {
            return Err(anyhow!("Not enough data for mandatory fixed parameters"));
        }

        let fixed_params = data[..fixed_length].to_vec();
        let remaining = data[fixed_length..].to_vec();

        Ok((remaining, fixed_params))
    }

    /// Get mandatory fixed parameter length for message type
    fn get_mandatory_fixed_length(&self, message_type: &IsupMessageType) -> usize {
        match message_type {
            IsupMessageType::IAM => 4, // Nature of Connection + Forward Call Indicators + Calling Party Category + Transmission Medium Requirement
            IsupMessageType::ACM => 2, // Backward Call Indicators
            IsupMessageType::ANM => 1, // Backward Call Indicators (optional in some variants)
            IsupMessageType::REL => 1, // Cause Indicators
            IsupMessageType::RLC => 0, // No mandatory fixed parameters
            IsupMessageType::CPG => 1, // Event Information
            IsupMessageType::COT => 1, // Continuity Indicators
            IsupMessageType::CCR => 0, // No mandatory fixed parameters
            IsupMessageType::RSC => 0, // No mandatory fixed parameters
            IsupMessageType::BLO => 0, // No mandatory fixed parameters
            IsupMessageType::UBL => 0, // No mandatory fixed parameters
            IsupMessageType::BLA => 0, // No mandatory fixed parameters
            IsupMessageType::UBA => 0, // No mandatory fixed parameters
            IsupMessageType::GRS => 1, // Range and Status
            IsupMessageType::GRA => 1, // Range and Status
            _ => 0,                    // Default to no fixed parameters
        }
    }

    /// Parse mandatory variable parameters
    fn parse_mandatory_variable(
        &self,
        message_type: &IsupMessageType,
        data: &[u8],
    ) -> Result<(Vec<u8>, Vec<IsupParameter>)> {
        let mut parameters = Vec::new();
        let mut current_data = data;

        let variable_count = self.get_mandatory_variable_count(message_type);

        if variable_count == 0 {
            return Ok((data.to_vec(), parameters));
        }

        // Read pointer table
        if current_data.len() < variable_count {
            return Err(anyhow!("Not enough data for variable parameter pointers"));
        }

        let mut pointers = Vec::new();
        for &byte in &current_data[..variable_count] {
            pointers.push(byte);
        }

        current_data = &current_data[variable_count..];

        // Parse each variable parameter
        for &pointer in &pointers {
            if pointer == 0 {
                continue; // Skip empty parameters
            }

            let offset = (pointer as usize).saturating_sub(1);
            if offset >= current_data.len() {
                return Err(anyhow!("Invalid variable parameter pointer"));
            }

            let param_data = &current_data[offset..];
            if param_data.is_empty() {
                continue;
            }

            let length = param_data[0] as usize;
            if param_data.len() < length + 1 {
                return Err(anyhow!("Invalid variable parameter length"));
            }

            let param_value = param_data[1..length + 1].to_vec();

            // Determine parameter type based on message type and position
            let param_type = self.get_variable_parameter_type(message_type, parameters.len());

            parameters.push(IsupParameter {
                param_type,
                length: length as u8,
                data: param_value,
            });
        }

        Ok((current_data.to_vec(), parameters))
    }

    /// Get number of mandatory variable parameters for message type
    fn get_mandatory_variable_count(&self, message_type: &IsupMessageType) -> usize {
        match message_type {
            IsupMessageType::IAM => 1, // Called Party Number
            IsupMessageType::SAM => 1, // Subsequent Number
            IsupMessageType::REL => 0, // No mandatory variable parameters
            IsupMessageType::ACM => 0, // No mandatory variable parameters
            IsupMessageType::ANM => 0, // No mandatory variable parameters
            IsupMessageType::CPG => 0, // No mandatory variable parameters
            IsupMessageType::GRS => 0, // No mandatory variable parameters (Range and Status is fixed)
            IsupMessageType::GRA => 0, // No mandatory variable parameters
            _ => 0,                    // Default to no variable parameters
        }
    }

    /// Get parameter type for mandatory variable parameter by position
    fn get_variable_parameter_type(
        &self,
        message_type: &IsupMessageType,
        position: usize,
    ) -> IsupParameterType {
        match (message_type, position) {
            (IsupMessageType::IAM, 0) => IsupParameterType::CalledPartyNumber,
            (IsupMessageType::SAM, 0) => IsupParameterType::SubsequentNumber,
            _ => IsupParameterType::EndOfOptionalParameters,
        }
    }

    /// Parse optional parameters
    fn parse_optional_parameters(&self, data: &[u8]) -> Result<Vec<IsupParameter>> {
        let mut parameters = Vec::new();
        let mut current_data = data;

        while !current_data.is_empty() {
            let param_type_byte = current_data[0];

            // End of optional parameters marker
            if param_type_byte == 0x00 {
                break;
            }

            if current_data.len() < 2 {
                break;
            }

            let length = current_data[1] as usize;

            if current_data.len() < length + 2 {
                return Err(anyhow!("Invalid optional parameter length"));
            }

            let param_data = current_data[2..length + 2].to_vec();
            let param_type = IsupParameterType::from(param_type_byte);

            parameters.push(IsupParameter {
                param_type,
                length: length as u8,
                data: param_data,
            });

            current_data = &current_data[length + 2..];
        }

        Ok(parameters)
    }

    /// Create ISUP message binary data
    pub fn create_isup_message(&self, message: &IsupMessage) -> Result<Vec<u8>> {
        let mut data = Vec::new();

        // Add CIC and message type
        data.extend_from_slice(&message.cic.to_be_bytes());
        data.push(message.message_type as u8);

        // Add mandatory fixed parameters
        data.extend_from_slice(&message.mandatory_fixed);

        // Add mandatory variable parameters
        if !message.mandatory_variable.is_empty() {
            // Create pointer table
            let mut pointers = Vec::new();
            let mut var_data = Vec::new();
            let pointer_base = message.mandatory_variable.len();

            for param in &message.mandatory_variable {
                if param.data.is_empty() {
                    pointers.push(0u8);
                } else {
                    pointers.push((pointer_base + var_data.len() + 1) as u8);
                    var_data.push(param.length);
                    var_data.extend_from_slice(&param.data);
                }
            }

            data.extend_from_slice(&pointers);
            data.extend_from_slice(&var_data);
        }

        // Add optional parameters
        for param in &message.optional {
            data.push(param.param_type as u8);
            data.push(param.length);
            data.extend_from_slice(&param.data);
        }

        // Add end of optional parameters marker
        if !message.optional.is_empty() {
            data.push(0x00);
        }

        Ok(data)
    }

    /// Convert ISUP message to SIP-T MIME multipart body
    pub fn create_sipt_body(&self, isup_data: &[u8], sdp_body: Option<&str>) -> Result<String> {
        if !self.config.sipt_enabled || !self.config.multipart_support {
            return Err(anyhow!("SIP-T multipart support not enabled"));
        }

        let uuid_str = uuid::Uuid::new_v4().to_string();
        let boundary = format!("redfire-sipt-{}", &uuid_str[..8]);
        let mut body = String::new();

        // Create multipart header
        body.push_str(&format!(
            "Content-Type: multipart/mixed; boundary={boundary}\r\n\r\n"
        ));

        // Add ISUP part
        body.push_str(&format!("--{boundary}\r\n"));
        body.push_str("Content-Type: application/ISUP; version=itu-t92+\r\n");
        body.push_str("Content-Disposition: signal; handling=required\r\n\r\n");
        body.push_str(&hex::encode(isup_data));
        body.push_str("\r\n");

        // Add SDP part if provided
        if let Some(sdp) = sdp_body {
            body.push_str(&format!("--{boundary}\r\n"));
            body.push_str("Content-Type: application/sdp\r\n\r\n");
            body.push_str(sdp);
            body.push_str("\r\n");
        }

        // Close multipart
        body.push_str(&format!("--{boundary}--\r\n"));

        Ok(body)
    }

    /// Parse SIP-T MIME multipart body to extract ISUP data
    pub fn parse_sipt_body(&self, body: &str) -> Result<(Vec<u8>, Option<String>)> {
        if !self.config.sipt_enabled || !self.config.multipart_support {
            return Err(anyhow!("SIP-T multipart support not enabled"));
        }

        // Find boundary
        let content_type_start = body
            .find("Content-Type:")
            .ok_or_else(|| anyhow!("No Content-Type found"))?;
        let content_type_line = body[content_type_start..].lines().next().unwrap_or("");

        let boundary = if let Some(boundary_start) = content_type_line.find("boundary=") {
            let boundary_value = &content_type_line[boundary_start + 9..];
            boundary_value.trim_matches(&['"', ' ', '\r', '\n'][..])
        } else {
            return Err(anyhow!("No boundary found in Content-Type"));
        };

        let boundary_marker = format!("--{}", boundary);
        let parts: Vec<&str> = body.split(&boundary_marker).collect();

        let mut isup_data = None;
        let mut sdp_data = None;

        for part in parts.iter().skip(1) {
            // Skip everything before first boundary
            if part.trim().is_empty() || part.starts_with("--") {
                continue;
            }

            let part_lines: Vec<&str> = part.lines().collect();
            let mut content_type = "";
            let mut content_start = 0;

            // Find content type and start of content
            for (i, line) in part_lines.iter().enumerate() {
                if line.to_lowercase().starts_with("content-type:") {
                    content_type = line;
                }
                if line.trim().is_empty() {
                    content_start = i + 1;
                    break;
                }
            }

            let content = part_lines[content_start..].join("\n").trim().to_string();

            if content_type.to_lowercase().contains("application/isup") {
                // Parse hex-encoded ISUP data
                match hex::decode(&content.replace(&[' ', '\n', '\r'][..], "")) {
                    Ok(data) => isup_data = Some(data),
                    Err(e) => warn!("Failed to decode ISUP hex data: {}", e),
                }
            } else if content_type.to_lowercase().contains("application/sdp") {
                sdp_data = Some(content);
            }
        }

        let isup = isup_data.ok_or_else(|| anyhow!("No ISUP data found in multipart body"))?;
        Ok((isup, sdp_data))
    }

    /// Create SIP-I encapsulated message (ISUP in SIP body)
    pub fn create_sipi_body(&self, isup_data: &[u8]) -> Result<String> {
        if !self.config.sipi_enabled {
            return Err(anyhow!("SIP-I support not enabled"));
        }

        // SIP-I uses direct ISUP encapsulation
        Ok(hex::encode(isup_data))
    }

    /// Parse SIP-I body to extract ISUP data
    pub fn parse_sipi_body(&self, body: &str) -> Result<Vec<u8>> {
        if !self.config.sipi_enabled {
            return Err(anyhow!("SIP-I support not enabled"));
        }

        hex::decode(body.trim().replace(&[' ', '\n', '\r'][..], ""))
            .map_err(|e| anyhow!("Failed to decode SIP-I hex data: {}", e))
    }

    /// Extract phone numbers from ISUP parameters
    pub fn extract_calling_number(&self, message: &IsupMessage) -> Option<String> {
        for param in &message.optional {
            if param.param_type == IsupParameterType::CallingPartyNumber {
                return self.decode_phone_number(&param.data);
            }
        }
        None
    }

    /// Extract called number from ISUP message
    pub fn extract_called_number(&self, message: &IsupMessage) -> Option<String> {
        for param in &message.mandatory_variable {
            if param.param_type == IsupParameterType::CalledPartyNumber {
                return self.decode_phone_number(&param.data);
            }
        }
        None
    }

    /// Decode phone number from ISUP parameter data
    fn decode_phone_number(&self, data: &[u8]) -> Option<String> {
        if data.len() < 2 {
            return None;
        }

        // Skip nature of address indicator and numbering plan
        let digits_data = &data[2..];
        let mut number = String::new();

        for &byte in digits_data {
            let first_digit = byte & 0x0F;
            let second_digit = (byte & 0xF0) >> 4;

            if first_digit <= 9 {
                number.push((b'0' + first_digit) as char);
            }

            if second_digit <= 9 {
                number.push((b'0' + second_digit) as char);
            } else if second_digit == 0x0F {
                break; // End of number marker
            }
        }

        if number.is_empty() {
            None
        } else {
            Some(number)
        }
    }

    /// Convert SIP INVITE to IAM message
    pub fn sip_to_iam(&self, from: &str, to: &str, cic: u16) -> Result<IsupMessage> {
        let mut message = IsupMessage {
            cic,
            message_type: IsupMessageType::IAM,
            mandatory_fixed: vec![
                0x00, // Nature of Connection Indicators
                0x60, 0x01, // Forward Call Indicators (National call, ISDN all the way)
                0x0A, // Calling Party Category (Ordinary subscriber)
                0x03, // Transmission Medium Requirement (3.1 kHz audio)
            ],
            mandatory_variable: Vec::new(),
            optional: Vec::new(),
            raw_data: Vec::new(),
        };

        // Add Called Party Number (mandatory variable)
        let called_number_data = self.encode_phone_number(to)?;
        message.mandatory_variable.push(IsupParameter {
            param_type: IsupParameterType::CalledPartyNumber,
            length: called_number_data.len() as u8,
            data: called_number_data,
        });

        // Add Calling Party Number (optional)
        let calling_number_data = self.encode_phone_number(from)?;
        message.optional.push(IsupParameter {
            param_type: IsupParameterType::CallingPartyNumber,
            length: calling_number_data.len() as u8,
            data: calling_number_data,
        });

        Ok(message)
    }

    /// Encode phone number for ISUP parameter
    fn encode_phone_number(&self, number: &str) -> Result<Vec<u8>> {
        let cleaned_number = number
            .trim_start_matches('+')
            .replace(&[' ', '-', '(', ')'][..], "");

        if cleaned_number.is_empty() {
            return Err(anyhow!("Empty phone number"));
        }

        let mut data = vec![
            0x03, // Nature of Address: National (significant) number
            0x10, // Numbering Plan: ISDN/telephony numbering plan
        ];

        let digits: Vec<u8> = cleaned_number
            .chars()
            .filter_map(|c| c.to_digit(10).map(|d| d as u8))
            .collect();

        // Encode digits in pairs (BCD)
        for chunk in digits.chunks(2) {
            let first = chunk[0];
            let second = chunk.get(1).copied().unwrap_or(0x0F); // 0x0F = filler
            data.push(first | (second << 4));
        }

        Ok(data)
    }

    /// Check if service is enabled
    pub fn is_sipt_enabled(&self) -> bool {
        self.config.sipt_enabled
    }

    /// Check if SIP-I is enabled
    pub fn is_sipi_enabled(&self) -> bool {
        self.config.sipi_enabled
    }

    /// Get configuration
    pub fn get_config(&self) -> &SipTSipIConfig {
        &self.config
    }
}

/// SIP-T/SIP-I utilities
pub mod utils {
    use super::*;

    /// Convert ISUP message type to SIP method
    pub fn isup_to_sip_method(msg_type: &IsupMessageType) -> &'static str {
        match msg_type {
            IsupMessageType::IAM => "INVITE",
            IsupMessageType::REL => "BYE",
            IsupMessageType::ACM => "183", // Session Progress
            IsupMessageType::ANM => "200", // OK
            IsupMessageType::CPG => "183", // Session Progress
            _ => "INFO",                   // Default to INFO for other messages
        }
    }

    /// Convert SIP method to ISUP message type
    pub fn sip_to_isup_type(method: &str) -> Option<IsupMessageType> {
        match method.to_uppercase().as_str() {
            "INVITE" => Some(IsupMessageType::IAM),
            "BYE" => Some(IsupMessageType::REL),
            "CANCEL" => Some(IsupMessageType::REL),
            _ => None,
        }
    }

    /// Generate next available CIC
    pub fn get_next_cic(config: &SipTSipIConfig, used_cics: &[u16]) -> Option<u16> {
        for cic in config.cic_range_start..=config.cic_range_end {
            if !used_cics.contains(&cic) {
                return Some(cic);
            }
        }
        None
    }

    /// Validate ISUP message structure
    pub fn validate_isup_message(message: &IsupMessage) -> Result<()> {
        if message.cic == 0 {
            return Err(anyhow!("Invalid CIC: 0"));
        }

        // Validate message type specific requirements
        match message.message_type {
            IsupMessageType::IAM => {
                if message.mandatory_variable.is_empty() {
                    return Err(anyhow!("IAM must have Called Party Number"));
                }
                if message.mandatory_fixed.len() < 4 {
                    return Err(anyhow!("IAM mandatory fixed parameters incomplete"));
                }
            }
            IsupMessageType::REL => {
                if message.mandatory_fixed.is_empty() {
                    return Err(anyhow!("REL must have Cause Indicators"));
                }
            }
            _ => {} // Other messages have different requirements
        }

        Ok(())
    }

    /// Format ISUP message for debugging
    pub fn format_isup_debug(message: &IsupMessage) -> String {
        let mut output = String::new();
        output.push_str(&format!(
            "ISUP Message: {} (CIC: {})\n",
            message.message_type, message.cic
        ));
        output.push_str(&format!(
            "  Mandatory Fixed: {} bytes\n",
            message.mandatory_fixed.len()
        ));
        output.push_str(&format!(
            "  Mandatory Variable: {} parameters\n",
            message.mandatory_variable.len()
        ));
        output.push_str(&format!(
            "  Optional: {} parameters\n",
            message.optional.len()
        ));

        for (i, param) in message.optional.iter().enumerate() {
            output.push_str(&format!(
                "    Param {}: {:?} ({} bytes)\n",
                i, param.param_type, param.length
            ));
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isup_message_type_conversion() {
        assert_eq!(IsupMessageType::from(0x01), IsupMessageType::IAM);
        assert_eq!(IsupMessageType::from(0x06), IsupMessageType::ACM);
        assert_eq!(IsupMessageType::from(0x09), IsupMessageType::ANM);
        assert_eq!(IsupMessageType::from(0x0C), IsupMessageType::REL);
    }

    #[test]
    fn test_phone_number_encoding() {
        let config = SipTSipIConfig::default();
        let service = SipTSipIService::new(config);

        let encoded = service.encode_phone_number("+15551234567").unwrap();
        assert_eq!(encoded[0], 0x03); // Nature of address
        assert_eq!(encoded[1], 0x10); // Numbering plan
        assert!(encoded.len() > 2);
    }

    #[test]
    fn test_sip_to_isup_conversion() {
        assert_eq!(
            utils::sip_to_isup_type("INVITE"),
            Some(IsupMessageType::IAM)
        );
        assert_eq!(utils::sip_to_isup_type("BYE"), Some(IsupMessageType::REL));
        assert_eq!(utils::sip_to_isup_type("OPTIONS"), None);
    }

    #[test]
    fn test_isup_to_sip_conversion() {
        assert_eq!(utils::isup_to_sip_method(&IsupMessageType::IAM), "INVITE");
        assert_eq!(utils::isup_to_sip_method(&IsupMessageType::REL), "BYE");
        assert_eq!(utils::isup_to_sip_method(&IsupMessageType::ACM), "183");
    }

    #[test]
    fn test_sipt_body_creation() {
        let config = SipTSipIConfig {
            sipt_enabled: true,
            multipart_support: true,
            ..Default::default()
        };
        let service = SipTSipIService::new(config);

        let isup_data = vec![0x01, 0x02, 0x03, 0x04];
        let body = service
            .create_sipt_body(&isup_data, Some("v=0\r\n"))
            .unwrap();

        assert!(body.contains("multipart/mixed"));
        assert!(body.contains("application/ISUP"));
        assert!(body.contains("application/sdp"));
        assert!(body.contains("01020304"));
    }

    #[test]
    fn test_cic_generation() {
        let config = SipTSipIConfig {
            cic_range_start: 1,
            cic_range_end: 10,
            ..Default::default()
        };

        let used_cics = vec![1, 3, 5];
        let next_cic = utils::get_next_cic(&config, &used_cics);
        assert_eq!(next_cic, Some(2));
    }
}
