/*
 * Redfire Switch - LERG and NANPA Data Management
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
use chrono::{DateTime, Utc, NaiveDate};
use csv::Reader;
use dashmap::DashMap;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tracing::{info, debug};

use crate::termination_routing::NanpaJurisdiction;

/// LERG data entry for NPA-NXX information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LergEntry {
    /// NPA (Area Code)
    pub npa: String,
    /// NXX (Exchange Code)
    pub nxx: String,
    /// Combined NPA-NXX as 1NPANXX format
    pub npa_nxx: String,
    /// Company type (e.g., "CLEC", "ILEC", "WIRELESS")
    pub company_type: String,
    /// Company/carrier name
    pub company_name: String,
    /// LATA (Local Access and Transport Area)
    pub lata: String,
    /// Rate center name
    pub rate_center: String,
    /// State/province
    pub state: String,
    /// OCN (Operating Company Number)
    pub ocn: Option<String>,
}

/// NANPA NPA table entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NanpaNpaEntry {
    /// NPA ID
    pub npa_id: String,
    /// NPA (Area Code)
    pub npa: String,
    /// Type of code (e.g., "GEO", "NON-GEO")
    pub type_of_code: String,
    /// Whether the NPA is assignable
    pub assignable: bool,
    /// Status (Y/N for in service)
    pub in_service: bool,
    /// In-service date
    pub in_service_date: Option<NaiveDate>,
    /// Location description
    pub location: String,
    /// Country
    pub country: String,
    /// Time zone
    pub time_zone: String,
    /// Area served description
    pub area_served: String,
    /// Whether it's an overlay
    pub overlay: bool,
    /// Parent NPA ID for overlays
    pub parent_npa_id: Option<String>,
}

/// LERG and NANPA data service
pub struct LergNanpaService {
    /// LERG data indexed by NPA-NXX
    lerg_data: Arc<DashMap<String, LergEntry>>,
    /// NANPA NPA data indexed by NPA
    nanpa_data: Arc<DashMap<String, NanpaNpaEntry>>,
    /// HTTP client for downloads
    client: Client,
    /// Service statistics
    stats: Arc<parking_lot::RwLock<LergNanpaStats>>,
}

/// Service statistics
#[derive(Debug, Clone, Default)]
pub struct LergNanpaStats {
    /// Total LERG entries loaded
    pub lerg_entries: u64,
    /// Total NANPA entries loaded
    pub nanpa_entries: u64,
    /// Last LERG file load time
    pub last_lerg_load: Option<DateTime<Utc>>,
    /// Last NANPA download time
    pub last_nanpa_download: Option<DateTime<Utc>>,
    /// Jurisdiction lookups performed
    pub jurisdiction_lookups: u64,
    /// Rate center lookups performed
    pub rate_center_lookups: u64,
}

impl LergNanpaService {
    /// Create a new LERG/NANPA service
    pub fn new() -> Self {
        Self {
            lerg_data: Arc::new(DashMap::new()),
            nanpa_data: Arc::new(DashMap::new()),
            client: Client::new(),
            stats: Arc::new(parking_lot::RwLock::new(LergNanpaStats::default())),
        }
    }

    /// Load LERG data from CSV file
    pub async fn load_lerg_file(&self, file_path: &str) -> Result<()> {
        info!("Loading LERG data from: {}", file_path);

        if !Path::new(file_path).exists() {
            return Err(anyhow!("LERG file not found: {}", file_path));
        }

        let content = fs::read_to_string(file_path).await?;
        let mut reader = Reader::from_reader(content.as_bytes());
        let mut loaded_count = 0;
        let mut error_count = 0;

        // Clear existing data
        self.lerg_data.clear();

        for result in reader.records() {
            match result {
                Ok(record) => {
                    if record.len() >= 7 {
                        let npa = record.get(0).unwrap_or("").to_string();
                        let nxx = record.get(1).unwrap_or("").to_string();
                        let company_type = record.get(2).unwrap_or("").to_string();
                        let company_name = record.get(3).unwrap_or("").to_string();
                        let lata = record.get(4).unwrap_or("").to_string();
                        let rate_center = record.get(5).unwrap_or("").to_string();
                        let state = record.get(6).unwrap_or("").to_string();

                        // Validate NPA and NXX
                        if self.validate_npa_nxx(&npa, &nxx) {
                            let npa_nxx = format!("1{}{}", npa, nxx);
                            
                            let entry = LergEntry {
                                npa: npa.clone(),
                                nxx: nxx.clone(),
                                npa_nxx: npa_nxx.clone(),
                                company_type,
                                company_name,
                                lata,
                                rate_center,
                                state,
                                ocn: None, // OCN not in this CSV format
                            };

                            self.lerg_data.insert(npa_nxx, entry);
                            loaded_count += 1;
                        } else {
                            error_count += 1;
                            debug!("Invalid NPA/NXX in LERG data: {}/{}", npa, nxx);
                        }
                    } else {
                        error_count += 1;
                        debug!("Invalid LERG record format: insufficient fields");
                    }
                }
                Err(e) => {
                    error_count += 1;
                    debug!("Error reading LERG record: {}", e);
                }
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.lerg_entries = loaded_count;
            stats.last_lerg_load = Some(Utc::now());
        }

        info!("LERG data loaded: {} entries ({} errors)", loaded_count, error_count);
        Ok(())
    }

    /// Download and load NANPA NPA table
    pub async fn download_nanpa_npa_table(&self) -> Result<()> {
        info!("Downloading NANPA NPA table from official source");

        let url = "https://reports.nanpa.com/public/npa_report.csv";
        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("Failed to download NANPA NPA table: HTTP {}", response.status()));
        }

        let content = response.text().await?;
        self.parse_nanpa_npa_data(&content).await?;

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.last_nanpa_download = Some(Utc::now());
        }

        info!("NANPA NPA table downloaded and processed successfully");
        Ok(())
    }

    /// Parse NANPA NPA CSV data
    async fn parse_nanpa_npa_data(&self, content: &str) -> Result<()> {
        let mut reader = Reader::from_reader(content.as_bytes());
        let mut loaded_count = 0;
        let mut error_count = 0;

        // Clear existing data
        self.nanpa_data.clear();

        // Skip the file header lines and find the actual data
        let mut found_header = false;
        for result in reader.records() {
            match result {
                Ok(record) => {
                    // Check if this is the header row
                    if !found_header {
                        if record.get(0).unwrap_or("") == "NPA_ID" {
                            found_header = true;
                        }
                        continue;
                    }

                    if record.len() >= 13 {
                        let npa_id = record.get(0).unwrap_or("").to_string();
                        let npa = record.get(1).map(|s| s.trim()).unwrap_or("");
                        let type_of_code = record.get(2).unwrap_or("").to_string();
                        let assignable = record.get(3).unwrap_or("").to_uppercase() == "Y";
                        let in_service = record.get(8).unwrap_or("").to_uppercase() == "Y";
                        let in_service_date_str = record.get(9).unwrap_or("");
                        let location = record.get(11).unwrap_or("").to_string();
                        let country = record.get(12).unwrap_or("").to_string();
                        let time_zone = record.get(17).unwrap_or("").to_string();
                        let area_served = record.get(18).unwrap_or("").to_string();
                        let overlay = record.get(19).unwrap_or("").to_uppercase() == "Y";
                        let parent_npa_id = record.get(21)
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string());

                        // Only load entries that are in service
                        if in_service && self.validate_npa(npa) {
                            let in_service_date = self.parse_date(in_service_date_str);

                            let entry = NanpaNpaEntry {
                                npa_id,
                                npa: npa.to_string(),
                                type_of_code,
                                assignable,
                                in_service,
                                in_service_date,
                                location,
                                country,
                                time_zone,
                                area_served,
                                overlay,
                                parent_npa_id,
                            };

                            self.nanpa_data.insert(npa.to_string(), entry);
                            loaded_count += 1;
                        }
                    } else {
                        error_count += 1;
                        debug!("Invalid NANPA record format: insufficient fields");
                    }
                }
                Err(e) => {
                    error_count += 1;
                    debug!("Error reading NANPA record: {}", e);
                }
            }
        }

        // Update statistics
        {
            let mut stats = self.stats.write();
            stats.nanpa_entries = loaded_count;
        }

        info!("NANPA NPA data processed: {} entries ({} errors)", loaded_count, error_count);
        Ok(())
    }

    /// Parse date from NANPA format
    fn parse_date(&self, date_str: &str) -> Option<NaiveDate> {
        if date_str.is_empty() {
            return None;
        }

        // Try common date formats
        let formats = [
            "%m/%d/%Y", // MM/DD/YYYY
            "%Y-%m-%d", // YYYY-MM-DD
            "%m-%d-%Y", // MM-DD-YYYY
        ];

        for format in &formats {
            if let Ok(date) = NaiveDate::parse_from_str(date_str, format) {
                return Some(date);
            }
        }

        None
    }

    /// Validate NPA (area code)
    fn validate_npa(&self, npa: &str) -> bool {
        npa.len() == 3 && npa.chars().all(|c| c.is_ascii_digit()) && !npa.starts_with('0')
    }

    /// Validate NPA and NXX
    fn validate_npa_nxx(&self, npa: &str, nxx: &str) -> bool {
        self.validate_npa(npa) && 
        nxx.len() == 3 && 
        nxx.chars().all(|c| c.is_ascii_digit()) &&
        !nxx.starts_with('0')
    }

    /// Determine jurisdiction between two NANPA numbers
    pub async fn determine_jurisdiction(&self, dest_number: &str, orig_number: &str) -> Result<NanpaJurisdiction> {
        self.stats.write().jurisdiction_lookups += 1;

        // Extract NPA-NXX from numbers
        let dest_npa_nxx = self.extract_npa_nxx(dest_number);
        let orig_npa_nxx = self.extract_npa_nxx(orig_number);

        match (dest_npa_nxx, orig_npa_nxx) {
            (Some(dest), Some(orig)) => {
                // Get LERG entries for both numbers
                let dest_entry = self.lerg_data.get(&dest);
                let orig_entry = self.lerg_data.get(&orig);

                match (dest_entry, orig_entry) {
                    (Some(dest_lerg), Some(orig_lerg)) => {
                        if dest_lerg.state == orig_lerg.state {
                            // Same state - check if same rate center for local determination
                            if dest_lerg.rate_center == orig_lerg.rate_center &&
                               dest_lerg.lata == orig_lerg.lata {
                                Ok(NanpaJurisdiction::Local)
                            } else {
                                Ok(NanpaJurisdiction::Intrastate)
                            }
                        } else {
                            Ok(NanpaJurisdiction::Interstate)
                        }
                    }
                    _ => {
                        // Missing LERG data, fallback to NPA-based state lookup
                        let dest_npa = &dest[1..4]; // Extract NPA from 1NPANXX
                        let orig_npa = &orig[1..4];
                        
                        if let (Some(dest_state), Some(orig_state)) = (
                            self.get_state_for_npa(dest_npa),
                            self.get_state_for_npa(orig_npa)
                        ) {
                            if dest_state == orig_state {
                                Ok(NanpaJurisdiction::Intrastate)
                            } else {
                                Ok(NanpaJurisdiction::Interstate)
                            }
                        } else {
                            Ok(NanpaJurisdiction::Indeterminate)
                        }
                    }
                }
            }
            _ => Ok(NanpaJurisdiction::Indeterminate)
        }
    }

    /// Extract NPA-NXX in 1NPANXX format from a phone number
    fn extract_npa_nxx(&self, number: &str) -> Option<String> {
        let cleaned = number.trim_start_matches('+')
            .replace(['(', ')', '-', ' ', '.'], "");

        if cleaned.len() >= 10 {
            let start = if cleaned.len() == 11 && cleaned.starts_with('1') { 1 } else { 0 };
            let npa = &cleaned[start..start + 3];
            let nxx = &cleaned[start + 3..start + 6];
            
            if self.validate_npa_nxx(npa, nxx) {
                Some(format!("1{}{}", npa, nxx))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get state for NPA from NANPA data
    fn get_state_for_npa(&self, npa: &str) -> Option<String> {
        if let Some(entry) = self.nanpa_data.get(npa) {
            // Extract state from location or area_served
            // NANPA data might have formats like "CALIFORNIA", "CA", or "California"
            let location = entry.location.to_uppercase();
            let area_served = entry.area_served.to_uppercase();
            
            // Try to map to standard state abbreviations
            self.map_to_state_abbreviation(&location)
                .or_else(|| self.map_to_state_abbreviation(&area_served))
        } else {
            None
        }
    }

    /// Map location string to standard state abbreviation
    fn map_to_state_abbreviation(&self, location: &str) -> Option<String> {
        let state_mapping = self.get_state_mapping();
        
        // Direct lookup
        if let Some(abbrev) = state_mapping.get(location) {
            return Some(abbrev.clone());
        }
        
        // Partial match
        for (name, abbrev) in &state_mapping {
            if location.contains(name) {
                return Some(abbrev.clone());
            }
        }
        
        None
    }

    /// Get state name to abbreviation mapping
    fn get_state_mapping(&self) -> HashMap<String, String> {
        let mut mapping = HashMap::new();
        
        // US States
        mapping.insert("ALABAMA".to_string(), "AL".to_string());
        mapping.insert("ALASKA".to_string(), "AK".to_string());
        mapping.insert("ARIZONA".to_string(), "AZ".to_string());
        mapping.insert("ARKANSAS".to_string(), "AR".to_string());
        mapping.insert("CALIFORNIA".to_string(), "CA".to_string());
        mapping.insert("COLORADO".to_string(), "CO".to_string());
        mapping.insert("CONNECTICUT".to_string(), "CT".to_string());
        mapping.insert("DELAWARE".to_string(), "DE".to_string());
        mapping.insert("FLORIDA".to_string(), "FL".to_string());
        mapping.insert("GEORGIA".to_string(), "GA".to_string());
        mapping.insert("HAWAII".to_string(), "HI".to_string());
        mapping.insert("IDAHO".to_string(), "ID".to_string());
        mapping.insert("ILLINOIS".to_string(), "IL".to_string());
        mapping.insert("INDIANA".to_string(), "IN".to_string());
        mapping.insert("IOWA".to_string(), "IA".to_string());
        mapping.insert("KANSAS".to_string(), "KS".to_string());
        mapping.insert("KENTUCKY".to_string(), "KY".to_string());
        mapping.insert("LOUISIANA".to_string(), "LA".to_string());
        mapping.insert("MAINE".to_string(), "ME".to_string());
        mapping.insert("MARYLAND".to_string(), "MD".to_string());
        mapping.insert("MASSACHUSETTS".to_string(), "MA".to_string());
        mapping.insert("MICHIGAN".to_string(), "MI".to_string());
        mapping.insert("MINNESOTA".to_string(), "MN".to_string());
        mapping.insert("MISSISSIPPI".to_string(), "MS".to_string());
        mapping.insert("MISSOURI".to_string(), "MO".to_string());
        mapping.insert("MONTANA".to_string(), "MT".to_string());
        mapping.insert("NEBRASKA".to_string(), "NE".to_string());
        mapping.insert("NEVADA".to_string(), "NV".to_string());
        mapping.insert("NEW HAMPSHIRE".to_string(), "NH".to_string());
        mapping.insert("NEW JERSEY".to_string(), "NJ".to_string());
        mapping.insert("NEW MEXICO".to_string(), "NM".to_string());
        mapping.insert("NEW YORK".to_string(), "NY".to_string());
        mapping.insert("NORTH CAROLINA".to_string(), "NC".to_string());
        mapping.insert("NORTH DAKOTA".to_string(), "ND".to_string());
        mapping.insert("OHIO".to_string(), "OH".to_string());
        mapping.insert("OKLAHOMA".to_string(), "OK".to_string());
        mapping.insert("OREGON".to_string(), "OR".to_string());
        mapping.insert("PENNSYLVANIA".to_string(), "PA".to_string());
        mapping.insert("RHODE ISLAND".to_string(), "RI".to_string());
        mapping.insert("SOUTH CAROLINA".to_string(), "SC".to_string());
        mapping.insert("SOUTH DAKOTA".to_string(), "SD".to_string());
        mapping.insert("TENNESSEE".to_string(), "TN".to_string());
        mapping.insert("TEXAS".to_string(), "TX".to_string());
        mapping.insert("UTAH".to_string(), "UT".to_string());
        mapping.insert("VERMONT".to_string(), "VT".to_string());
        mapping.insert("VIRGINIA".to_string(), "VA".to_string());
        mapping.insert("WASHINGTON".to_string(), "WA".to_string());
        mapping.insert("WEST VIRGINIA".to_string(), "WV".to_string());
        mapping.insert("WISCONSIN".to_string(), "WI".to_string());
        mapping.insert("WYOMING".to_string(), "WY".to_string());
        
        // Canadian provinces/territories
        mapping.insert("ALBERTA".to_string(), "AB".to_string());
        mapping.insert("BRITISH COLUMBIA".to_string(), "BC".to_string());
        mapping.insert("MANITOBA".to_string(), "MB".to_string());
        mapping.insert("NEW BRUNSWICK".to_string(), "NB".to_string());
        mapping.insert("NEWFOUNDLAND AND LABRADOR".to_string(), "NL".to_string());
        mapping.insert("NORTHWEST TERRITORIES".to_string(), "NT".to_string());
        mapping.insert("NOVA SCOTIA".to_string(), "NS".to_string());
        mapping.insert("NUNAVUT".to_string(), "NU".to_string());
        mapping.insert("ONTARIO".to_string(), "ON".to_string());
        mapping.insert("PRINCE EDWARD ISLAND".to_string(), "PE".to_string());
        mapping.insert("QUEBEC".to_string(), "QC".to_string());
        mapping.insert("SASKATCHEWAN".to_string(), "SK".to_string());
        mapping.insert("YUKON".to_string(), "YT".to_string());
        
        // Caribbean and other NANPA territories
        mapping.insert("PUERTO RICO".to_string(), "PR".to_string());
        mapping.insert("US VIRGIN ISLANDS".to_string(), "VI".to_string());
        mapping.insert("GUAM".to_string(), "GU".to_string());
        mapping.insert("NORTHERN MARIANA ISLANDS".to_string(), "MP".to_string());
        mapping.insert("AMERICAN SAMOA".to_string(), "AS".to_string());
        mapping.insert("BAHAMAS".to_string(), "BS".to_string());
        mapping.insert("BARBADOS".to_string(), "BB".to_string());
        mapping.insert("BERMUDA".to_string(), "BM".to_string());
        mapping.insert("JAMAICA".to_string(), "JM".to_string());
        
        mapping
    }

    /// Get company information for a phone number
    pub async fn get_company_info(&self, number: &str) -> Option<(String, String)> {
        self.stats.write().rate_center_lookups += 1;

        if let Some(npa_nxx) = self.extract_npa_nxx(number) {
            if let Some(entry) = self.lerg_data.get(&npa_nxx) {
                return Some((entry.company_name.clone(), entry.company_type.clone()));
            }
        }
        
        None
    }

    /// Get rate center for a phone number
    pub async fn get_rate_center(&self, number: &str) -> Option<String> {
        if let Some(npa_nxx) = self.extract_npa_nxx(number) {
            if let Some(entry) = self.lerg_data.get(&npa_nxx) {
                return Some(entry.rate_center.clone());
            }
        }
        
        None
    }

    /// Get LERG entry for a number
    pub fn get_lerg_entry(&self, number: &str) -> Option<LergEntry> {
        if let Some(npa_nxx) = self.extract_npa_nxx(number) {
            self.lerg_data.get(&npa_nxx).map(|entry| entry.clone())
        } else {
            None
        }
    }

    /// Get NANPA entry for an NPA
    pub fn get_nanpa_entry(&self, npa: &str) -> Option<NanpaNpaEntry> {
        self.nanpa_data.get(npa).map(|entry| entry.clone())
    }

    /// Get service statistics
    pub fn get_stats(&self) -> LergNanpaStats {
        self.stats.read().clone()
    }

    /// Get LERG data count
    pub fn get_lerg_count(&self) -> usize {
        self.lerg_data.len()
    }

    /// Get NANPA data count
    pub fn get_nanpa_count(&self) -> usize {
        self.nanpa_data.len()
    }

    /// Export LERG data to CSV
    pub async fn export_lerg_data(&self, output_path: &str) -> Result<()> {
        use tokio::fs::File;
        use tokio::io::AsyncWriteExt;

        let mut file = File::create(output_path).await?;
        
        // Write header
        file.write_all(b"npa,nxx,npa_nxx,company_type,company_name,lata,rate_center,state\n").await?;
        
        // Write data
        for entry in self.lerg_data.iter() {
            let line = format!("{},{},{},{},{},{},{},{}\n",
                entry.npa, entry.nxx, entry.npa_nxx, entry.company_type,
                entry.company_name, entry.lata, entry.rate_center, entry.state
            );
            file.write_all(line.as_bytes()).await?;
        }
        
        file.flush().await?;
        info!("Exported {} LERG entries to {}", self.lerg_data.len(), output_path);
        Ok(())
    }
}

/// LERG/NANPA utilities
pub mod utils {
    use super::*;

    /// Check if a number is likely to be NANPA
    pub fn is_nanpa_number(number: &str) -> bool {
        let cleaned = number.trim_start_matches('+')
            .replace(['(', ')', '-', ' ', '.'], "");

        (cleaned.len() == 10 && cleaned.chars().all(|c| c.is_ascii_digit())) ||
        (cleaned.len() == 11 && cleaned.starts_with('1') && cleaned[1..].chars().all(|c| c.is_ascii_digit()))
    }

    /// Extract NPA from phone number
    pub fn extract_npa(number: &str) -> Option<String> {
        let cleaned = number.trim_start_matches('+')
            .replace(['(', ')', '-', ' ', '.'], "");

        if cleaned.len() >= 10 {
            let start = if cleaned.len() == 11 && cleaned.starts_with('1') { 1 } else { 0 };
            let npa = &cleaned[start..start + 3];
            
            if npa.chars().all(|c| c.is_ascii_digit()) && !npa.starts_with('0') {
                Some(npa.to_string())
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Create sample LERG entries for testing
    pub fn create_sample_lerg_entries() -> Vec<LergEntry> {
        vec![
            LergEntry {
                npa: "212".to_string(),
                nxx: "555".to_string(),
                npa_nxx: "1212555".to_string(),
                company_type: "ILEC".to_string(),
                company_name: "Verizon New York Inc.".to_string(),
                lata: "132".to_string(),
                rate_center: "NEW YORK".to_string(),
                state: "NY".to_string(),
                ocn: Some("9740".to_string()),
            },
            LergEntry {
                npa: "310".to_string(),
                nxx: "555".to_string(),
                npa_nxx: "1310555".to_string(),
                company_type: "CLEC".to_string(),
                company_name: "Level 3 Communications".to_string(),
                lata: "730".to_string(),
                rate_center: "LOS ANGELES".to_string(),
                state: "CA".to_string(),
                ocn: Some("3794".to_string()),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_npa_nxx_extraction() {
        let service = LergNanpaService::new();
        
        assert_eq!(service.extract_npa_nxx("15551234567"), Some("1555123".to_string()));
        assert_eq!(service.extract_npa_nxx("+1-555-123-4567"), Some("1555123".to_string()));
        assert_eq!(service.extract_npa_nxx("(555) 123-4567"), Some("1555123".to_string()));
        assert_eq!(service.extract_npa_nxx("442071234567"), None); // Not NANPA
    }

    #[test]
    fn test_npa_validation() {
        let service = LergNanpaService::new();
        
        assert!(service.validate_npa("555"));
        assert!(!service.validate_npa("055")); // Starts with 0
        assert!(!service.validate_npa("55")); // Too short
        assert!(!service.validate_npa("5555")); // Too long
    }

    #[test]
    fn test_state_mapping() {
        let service = LergNanpaService::new();
        let mapping = service.get_state_mapping();
        
        assert_eq!(mapping.get("CALIFORNIA"), Some(&"CA".to_string()));
        assert_eq!(mapping.get("NEW YORK"), Some(&"NY".to_string()));
        assert_eq!(mapping.get("TEXAS"), Some(&"TX".to_string()));
    }

    #[test]
    fn test_utils_nanpa_check() {
        assert!(utils::is_nanpa_number("15551234567"));
        assert!(utils::is_nanpa_number("555-123-4567"));
        assert!(!utils::is_nanpa_number("442071234567"));
        assert!(!utils::is_nanpa_number("123"));
    }

    #[test]
    fn test_npa_extraction() {
        assert_eq!(utils::extract_npa("15551234567"), Some("555".to_string()));
        assert_eq!(utils::extract_npa("+1 (555) 123-4567"), Some("555".to_string()));
        assert_eq!(utils::extract_npa("442071234567"), Some("442".to_string())); // Invalid but extracts
    }
}