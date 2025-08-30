//! RFC-compliant ANI-II implementation integrating with the new SIP RFC compliance module
//! 
//! This module provides proper ANI-II/OLI handling that conforms to:
//! - RFC 3261 (SIP)
//! - RFC 3372 (SIP-T) 
//! - ITU-T Q.1912.5 (SIP-I)
//! - NANPA ANI-II standards

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::sip_rfc_compliance::{extract_oli_info, OriginatingLineInfo};


/// NANPA-compliant ANI-II code definitions
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AniIICode {
    RegularLine = 0,                // 00 - POTS (Plain Old Telephone Service)
    Multiparty = 1,                  // 01 - Multiparty line  
    OffHookEmergency = 2,            // 02 - ANI failure/Emergency
    ManualOperator = 3,              // 03 - Manual Operator ID
    // 04-05 are unassigned
    StationLevelRating = 6,          // 06 - Station Level Rating
    SpecialOperator = 7,             // 07 - Special Operator Handling Required
    // 08-09 are unassigned
    RegionalCenter = 10,             // 10 - Not assignable/Regional Center Test
    // 11 is unassigned
    ConferencingOperator = 12,       // 12 - Long Distance Operator
    // 13-16 are unassigned
    IndividualLine = 17,             // 17 - Individual line (unrestricted)
    AutonOperatorIdentified = 18,    // 18 - Auton Operator Identified
    OnHookCoinOrNonCoin = 19,        // 19 - Coin or Non-Coin
    IntercityRestrictedLine = 20,    // 20 - Intercity Restricted Line
    InmateService = 21,              // 21 - Inmate Service  
    InmateServiceRestricted = 22,    // 22 - Inmate Service with restrictions
    CoinNonCoinUncertainty = 23,     // 23 - Coin Interface (Uncertainty)
    TollFreeSpsAccess = 24,          // 24 - Toll Free Service (SPS/WPS)
    SemiRestrictedLine = 25,         // 25 - Semi-Restricted Line
    // 26 is unassigned
    PayStationNetworkCoin = 27,      // 27 - Pay Station with Network Control Signaling
    // 28 is unassigned
    PrisonInmateFts = 29,            // 29 - Prison/Inmate Service (FTS)
    // 30-31 are unassigned  
    SecondaryIndividual = 32,        // 32 - Secondary/Toll Billing
    // 33 is unassigned
    NetworkIdentifiedDeniedAnonymity = 34, // 34 - Network Identified with denied anonymity
    // 35-39 are unassigned
    UnrestrictedUseA = 40,           // 40 - Unrestricted Use A
    UnrestrictedUseB = 41,           // 41 - Unrestricted Use B  
    UnrestrictedUseC = 42,           // 42 - Unrestricted Use C
    UnrestrictedUseD = 43,           // 43 - Unrestricted Use D
    UnrestrictedUseE = 44,           // 44 - Unrestricted Use E
    UnrestrictedUseF = 45,           // 45 - Unrestricted Use F
    UnrestrictedUseG = 46,           // 46 - Unrestricted Use G
    UnrestrictedUseH = 47,           // 47 - Unrestricted Use H
    BusinessPbxCentrex = 48,          // 48 - Business PBX/Centrex
    // 49-51 are unassigned
    Roaming = 52,                    // 52 - Roaming/Cellular
    // 53-60 are unassigned  
    CellularRadio = 61,              // 61 - Cellular/Wireless PCS (Type 1)
    CellularRadioType2 = 62,         // 62 - Cellular/Wireless PCS (Type 2)  
    CellularRadioRoaming1 = 63,      // 63 - Cellular/Wireless Roaming (Type 1)
    CellularRadioRoaming2 = 64,      // 64 - Cellular/Wireless Roaming (Type 2)
    // 65-69 are unassigned
    PayStationNonNetworkCoin = 70,   // 70 - Pay Station, No Network Control
    // 71-79 are unassigned  
    AiodListed = 80,                 // 80 - AIOD Listed DN sent
    // 81-82 are unassigned
    OnHookCoinLine = 83,             // 83 - On-Hook Coin Control Line
    // 84-92 are unassigned
    VoipPhone = 93,                  // 93 - VoIP Phone
    // 94-99 are unassigned or reserved
    Unknown = 99,                    // Used when code is not recognized
}

impl AniIICode {
    /// Convert from numeric digit to enum
    pub fn from_digit(digit: u8) -> Option<Self> {
        match digit {
            0 => Some(Self::RegularLine),
            1 => Some(Self::Multiparty),
            2 => Some(Self::OffHookEmergency),
            3 => Some(Self::ManualOperator),
            6 => Some(Self::StationLevelRating),
            7 => Some(Self::SpecialOperator),
            10 => Some(Self::RegionalCenter),
            12 => Some(Self::ConferencingOperator),
            17 => Some(Self::IndividualLine),
            18 => Some(Self::AutonOperatorIdentified),
            19 => Some(Self::OnHookCoinOrNonCoin),
            20 => Some(Self::IntercityRestrictedLine),
            21 => Some(Self::InmateService),
            22 => Some(Self::InmateServiceRestricted),
            23 => Some(Self::CoinNonCoinUncertainty),
            24 => Some(Self::TollFreeSpsAccess),
            25 => Some(Self::SemiRestrictedLine),
            27 => Some(Self::PayStationNetworkCoin),
            29 => Some(Self::PrisonInmateFts),
            32 => Some(Self::SecondaryIndividual),
            34 => Some(Self::NetworkIdentifiedDeniedAnonymity),
            40 => Some(Self::UnrestrictedUseA),
            41 => Some(Self::UnrestrictedUseB),
            42 => Some(Self::UnrestrictedUseC),
            43 => Some(Self::UnrestrictedUseD),
            44 => Some(Self::UnrestrictedUseE),
            45 => Some(Self::UnrestrictedUseF),
            46 => Some(Self::UnrestrictedUseG),
            47 => Some(Self::UnrestrictedUseH),
            48 => Some(Self::BusinessPbxCentrex),
            52 => Some(Self::Roaming),
            61 => Some(Self::CellularRadio),
            62 => Some(Self::CellularRadioType2),
            63 => Some(Self::CellularRadioRoaming1),
            64 => Some(Self::CellularRadioRoaming2),
            70 => Some(Self::PayStationNonNetworkCoin),
            80 => Some(Self::AiodListed),
            83 => Some(Self::OnHookCoinLine),
            93 => Some(Self::VoipPhone),
            _ => None
        }
    }
    
    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::RegularLine => "Regular Line (POTS)",
            Self::Multiparty => "Multiparty Line",
            Self::OffHookEmergency => "ANI Failure/Emergency",
            Self::ManualOperator => "Manual Operator",
            Self::StationLevelRating => "Station Level Rating",
            Self::SpecialOperator => "Special Operator Required",
            Self::RegionalCenter => "Regional Center Test",
            Self::ConferencingOperator => "Long Distance Operator",
            Self::IndividualLine => "Individual Line",
            Self::AutonOperatorIdentified => "Auton Operator",
            Self::OnHookCoinOrNonCoin => "Coin or Non-Coin",
            Self::IntercityRestrictedLine => "Intercity Restricted",
            Self::InmateService => "Inmate Service",
            Self::InmateServiceRestricted => "Inmate Service Restricted",
            Self::CoinNonCoinUncertainty => "Payphone (Coin Uncertainty)",
            Self::TollFreeSpsAccess => "Toll Free Service",
            Self::SemiRestrictedLine => "Semi-Restricted Line",
            Self::PayStationNetworkCoin => "Payphone (Network Coin)",
            Self::PrisonInmateFts => "Prison/Inmate FTS",
            Self::SecondaryIndividual => "Secondary/Toll Billing",
            Self::NetworkIdentifiedDeniedAnonymity => "Network ID No Anonymity",
            Self::UnrestrictedUseA => "Unrestricted Use",
            Self::UnrestrictedUseB => "Unrestricted Use B",
            Self::UnrestrictedUseC => "Unrestricted Use C",
            Self::UnrestrictedUseD => "Unrestricted Use D",
            Self::UnrestrictedUseE => "Unrestricted Use E",
            Self::UnrestrictedUseF => "Unrestricted Use F",
            Self::UnrestrictedUseG => "Unrestricted Use G",
            Self::UnrestrictedUseH => "Unrestricted Use H",
            Self::BusinessPbxCentrex => "Business PBX/Centrex",
            Self::Roaming => "Roaming/Cellular",
            Self::CellularRadio => "Cellular/Wireless Type 1",
            Self::CellularRadioType2 => "Cellular/Wireless Type 2",
            Self::CellularRadioRoaming1 => "Cellular Roaming Type 1",
            Self::CellularRadioRoaming2 => "Cellular Roaming Type 2",
            Self::PayStationNonNetworkCoin => "Payphone (No Network Control)",
            Self::AiodListed => "AIOD Listed DN",
            Self::OnHookCoinLine => "On-Hook Coin Line",
            Self::VoipPhone => "VoIP Phone",
            Self::Unknown => "Unknown/Reserved",
        }
    }
}

/// Enhanced ANI-II information with RFC compliance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AniIIInfo {
    pub code: AniIICode,
    pub raw_digit: u8,
    pub source: String,  // Where it was found (P-ISUP-OLI, From URI, etc)
    pub calling_number: Option<String>,
    pub is_payphone: bool,
    pub restricted: bool,
}

impl AniIIInfo {
    /// Create from OLI info extracted via RFC-compliant parser
    pub fn from_oli_info(oli: OriginatingLineInfo) -> Option<Self> {
        let code = AniIICode::from_digit(oli.oli_value)?;
        
        // Determine if this triggers payphone surcharge
        let is_payphone = matches!(oli.oli_value, 23 | 27 | 70 | 19 | 83);
        
        // Determine if this is a restricted line
        let is_restricted = matches!(
            code,
            AniIICode::InmateService | 
            AniIICode::InmateServiceRestricted | 
            AniIICode::PrisonInmateFts |
            AniIICode::IntercityRestrictedLine |
            AniIICode::SemiRestrictedLine
        );
        
        Some(AniIIInfo {
            code,
            raw_digit: oli.oli_value,
            source: format!("{:?}", oli.source),
            calling_number: oli.calling_number,
            is_payphone: matches!(code, AniIICode::PayStationNetworkCoin | AniIICode::PayStationNonNetworkCoin | AniIICode::CoinNonCoinUncertainty),
            restricted: is_restricted,
        })
    }
    
    /// Convert from legacy ANI-II format for compatibility
    pub fn from_legacy(legacy: crate::ani_ii::AniIIInfo) -> Result<Self> {
        let code = AniIICode::from_digit(legacy.raw_digit)
            .ok_or_else(|| anyhow!("Invalid ANI-II code: {}", legacy.raw_digit))?;
            
        Ok(AniIIInfo {
            code,
            raw_digit: legacy.raw_digit,
            source: format!("{:?}", legacy.source),
            calling_number: None, // Legacy struct doesn't have calling number
            is_payphone: matches!(code, AniIICode::PayStationNetworkCoin | AniIICode::PayStationNonNetworkCoin | AniIICode::CoinNonCoinUncertainty),
            restricted: legacy.triggers_surcharge, // Use the actual field name
        })
    }
}

/// RFC-compliant ANI-II parser for telecommunications
pub struct RfcCompliantAniIIParser;

impl RfcCompliantAniIIParser {
    /// Parse ANI-II/OLI from SIP message using RFC-compliant methods
    pub fn parse_from_sip_message(
        headers: &HashMap<String, String>,
        body: Option<&str>
    ) -> Option<AniIIInfo> {
        // Use the RFC-compliant OLI extraction
        let oli_info = extract_oli_info(headers, body)?;
        AniIIInfo::from_oli_info(oli_info)
    }
    
    /// Check if this is a toll-free number
    pub fn is_toll_free(number: &str) -> bool {
        // Clean number
        let cleaned = number.chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>();
        
        // Check NANPA toll-free prefixes
        if cleaned.len() >= 10 {
            let npa = if cleaned.starts_with("1") && cleaned.len() >= 11 {
                &cleaned[1..4]
            } else {
                &cleaned[0..3]
            };
            
            matches!(npa, "800" | "833" | "844" | "855" | "866" | "877" | "888")
        } else {
            false
        }
    }
    
    /// Calculate payphone surcharge for toll-free calls
    pub fn calculate_surcharge(
        ani_ii: Option<&AniIIInfo>,
        is_toll_free: bool,
        trunk_config: Option<&HashMap<String, f64>>
    ) -> (bool, f64, String) {
        if !is_toll_free {
            return (false, 0.0, "Not a toll-free call".to_string());
        }
        
        if let Some(info) = ani_ii {
            if !info.is_payphone {
                return (false, 0.0, "ANI-II does not trigger surcharge".to_string());
            }
            
            // Check trunk-specific surcharge overrides
            let surcharge = if let Some(config) = trunk_config {
                config.get(&format!("surcharge_{}", info.raw_digit))
                    .copied()
                    .unwrap_or(0.49) // Default FCC surcharge
            } else {
                0.49 // Standard payphone surcharge
            };
            
            (true, surcharge, format!("Payphone surcharge for ANI-II {}", info.raw_digit))
        } else {
            (false, 0.0, "No ANI-II information".to_string())
        }
    }
}

/// Backward compatibility wrapper for existing code
pub mod legacy_compatibility {
    use super::*;
    
    /// Parse using legacy method names for backward compatibility
    pub fn parse_ani_ii_from_headers(headers: &HashMap<String, String>) -> Option<AniIIInfo> {
        RfcCompliantAniIIParser::parse_from_sip_message(headers, None)
    }
    
    /// Parse from SIP-I using legacy method name
    pub fn parse_ani_ii_from_sip_i(
        headers: &HashMap<String, String>, 
        body: &str
    ) -> Option<AniIIInfo> {
        RfcCompliantAniIIParser::parse_from_sip_message(headers, Some(body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc_compliant_oli_parsing() {
        let mut headers = HashMap::new();
        // Correct RFC-compliant format with ;oli= in URI
        headers.insert("From".to_string(), 
            "<sip:+15551234567@carrier.com;oli=70>;tag=abc123".to_string());
        
        let ani_ii = RfcCompliantAniIIParser::parse_from_sip_message(&headers, None).unwrap();
        assert_eq!(ani_ii.raw_digit, 70);
        assert_eq!(ani_ii.code, AniIICode::PayStationNonNetworkCoin);
        assert!(ani_ii.is_payphone);
    }

    #[test]
    fn test_p_isup_oli_header() {
        let mut headers = HashMap::new();
        headers.insert("P-ISUP-OLI".to_string(), "23".to_string());
        
        let ani_ii = RfcCompliantAniIIParser::parse_from_sip_message(&headers, None).unwrap();
        assert_eq!(ani_ii.raw_digit, 23);
        assert!(ani_ii.is_payphone);
    }

    #[test]
    fn test_toll_free_detection() {
        assert!(RfcCompliantAniIIParser::is_toll_free("18005551234"));
        assert!(RfcCompliantAniIIParser::is_toll_free("+18885551234"));
        assert!(RfcCompliantAniIIParser::is_toll_free("8665551234"));
        assert!(!RfcCompliantAniIIParser::is_toll_free("12125551234"));
    }

    #[test]
    fn test_surcharge_calculation() {
        let ani_ii = AniIIInfo {
            code: AniIICode::PayStationNonNetworkCoin,
            raw_digit: 70,
            source: "P-ISUP-OLI".to_string(),
            calling_number: Some("+15551234567".to_string()),
            is_payphone: true,
            restricted: false,
        };
        
        let (applies, amount, reason) = RfcCompliantAniIIParser::calculate_surcharge(
            Some(&ani_ii),
            true,
            None
        );
        
        assert!(applies);
        assert_eq!(amount, 0.49);
        assert!(reason.contains("Payphone surcharge"));
    }
}