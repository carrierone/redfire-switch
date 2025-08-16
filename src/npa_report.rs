use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};
use std::sync::Arc;

/// NPA (Numbering Plan Area) Report Data Loader and Country Detection
/// Supports loading NANPA and international numbering plan data from CSV files

/// NPA report entry from CSV file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpaReportEntry {
    /// NPA (Area Code) - 3 digits
    pub npa: String,
    /// NXX (Exchange Code) - 3 digits  
    pub nxx: Option<String>,
    /// XXXX (Line Number) range start
    pub xxxx_start: Option<String>,
    /// XXXX (Line Number) range end
    pub xxxx_end: Option<String>,
    /// Country code (ISO 3166-1 alpha-2)
    pub country_code: String,
    /// Country name
    pub country_name: String,
    /// Region/State/Province
    pub region: Option<String>,
    /// City or area description
    pub city: Option<String>,
    /// Time zone(s) 
    pub timezone: Option<String>,
    /// Rate center
    pub rate_center: Option<String>,
    /// LATA (Local Access and Transport Area)
    pub lata: Option<String>,
    /// OCN (Operating Company Number)
    pub ocn: Option<String>,
    /// Service provider/carrier name
    pub carrier: Option<String>,
    /// DID type (geographic, non-geographic, premium, etc.)
    pub number_type: Option<String>,
    /// Is mobile/wireless number
    pub is_mobile: Option<bool>,
    /// Is toll-free number
    pub is_toll_free: Option<bool>,
    /// Effective date
    pub effective_date: Option<NaiveDate>,
    /// Last updated
    pub last_updated: Option<DateTime<Utc>>,
    /// Additional notes
    pub notes: Option<String>,
}

/// Country detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountryDetectionResult {
    /// Detected country code
    pub country_code: String,
    /// Country name
    pub country_name: String,
    /// Region/state/province
    pub region: Option<String>,
    /// City
    pub city: Option<String>,
    /// Time zone
    pub timezone: Option<String>,
    /// Is mobile number
    pub is_mobile: bool,
    /// Is toll-free number
    pub is_toll_free: bool,
    /// Service provider
    pub carrier: Option<String>,
    /// Number type
    pub number_type: Option<String>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
}

/// NPA lookup key for efficient searching
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct NpaLookupKey {
    /// Country calling code (+1, +44, etc.)
    pub country_calling_code: String,
    /// NPA (area code)
    pub npa: String,
    /// NXX (exchange) - optional for broader matching
    pub nxx: Option<String>,
}

/// NPA report database service
pub struct NpaReportService {
    /// Main NPA database indexed by lookup key
    npa_database: Arc<RwLock<HashMap<NpaLookupKey, Vec<NpaReportEntry>>>>,
    /// Country calling code mappings
    country_calling_codes: Arc<RwLock<HashMap<String, CountryInfo>>>,
    /// Cache for recent lookups
    lookup_cache: Arc<RwLock<HashMap<String, CountryDetectionResult>>>,
    /// Statistics
    stats: Arc<RwLock<NpaStats>>,
}

/// Country calling code information
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CountryInfo {
    pub country_code: String,
    pub country_name: String,
    pub calling_code: String,
    pub number_length_min: u8,
    pub number_length_max: u8,
}

/// NPA database statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpaStats {
    pub total_entries: usize,
    pub countries_covered: usize,
    pub npas_covered: usize,
    pub last_updated: Option<DateTime<Utc>>,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub lookups_performed: u64,
}

impl NpaReportService {
    /// Create new NPA report service
    pub fn new() -> Self {
        let service = Self {
            npa_database: Arc::new(RwLock::new(HashMap::new())),
            country_calling_codes: Arc::new(RwLock::new(HashMap::new())),
            lookup_cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(NpaStats {
                total_entries: 0,
                countries_covered: 0,
                npas_covered: 0,
                last_updated: None,
                cache_hits: 0,
                cache_misses: 0,
                lookups_performed: 0,
            })),
        };
        
        // Initialize default country calling codes
        tokio::spawn({
            let service = service.clone();
            async move {
                if let Err(e) = service.initialize_default_country_codes().await {
                    error!("Failed to initialize default country codes: {}", e);
                }
            }
        });
        
        service
    }

    /// Load NPA report from CSV file
    pub async fn load_npa_report_csv<P: AsRef<Path>>(&self, file_path: P) -> Result<usize> {
        let file_path = file_path.as_ref();
        info!("Loading NPA report from CSV file: {:?}", file_path);
        
        let content = tokio::fs::read_to_string(file_path).await?;
        let mut reader = csv::Reader::from_reader(content.as_bytes());
        
        let mut entries = Vec::new();
        let mut row_count = 0;
        let mut error_count = 0;
        
        for result in reader.deserialize() {
            row_count += 1;
            
            match result {
                Ok(entry) => {
                    let npa_entry: NpaReportEntry = entry;
                    // Validate entry
                    if self.validate_npa_entry(&npa_entry).is_ok() {
                        entries.push(npa_entry);
                    } else {
                        error_count += 1;
                        if error_count < 10 { // Log first 10 errors
                            warn!("Invalid NPA entry at row {}: {:?}", row_count, npa_entry);
                        }
                    }
                }
                Err(e) => {
                    error_count += 1;
                    if error_count < 10 { // Log first 10 errors
                        warn!("Error parsing CSV row {}: {}", row_count, e);
                    }
                }
            }
        }
        
        if error_count > 0 {
            warn!("Encountered {} errors while loading CSV file", error_count);
        }
        
        // Process and index entries
        let indexed_count = self.index_npa_entries(entries).await?;
        
        // Update statistics
        let mut stats = self.stats.write().await;
        stats.total_entries = indexed_count;
        stats.last_updated = Some(Utc::now());
        
        // Count unique countries and NPAs
        let database = self.npa_database.read().await;
        let mut countries = std::collections::HashSet::new();
        let mut npas = std::collections::HashSet::new();
        
        for (key, entries) in database.iter() {
            npas.insert(key.npa.clone());
            for entry in entries {
                countries.insert(entry.country_code.clone());
            }
        }
        
        stats.countries_covered = countries.len();
        stats.npas_covered = npas.len();
        
        info!("Successfully loaded {} NPA entries covering {} countries and {} NPAs", 
              indexed_count, countries.len(), npas.len());
        
        Ok(indexed_count)
    }

    /// Load multiple CSV files (supports wildcards)
    pub async fn load_npa_reports_bulk<P: AsRef<Path>>(&self, directory_path: P) -> Result<usize> {
        let dir_path = directory_path.as_ref();
        info!("Loading NPA reports from directory: {:?}", dir_path);
        
        let mut total_loaded = 0;
        let mut entries = tokio::fs::read_dir(dir_path).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            // Only process CSV files
            if let Some(extension) = path.extension() {
                if extension == "csv" {
                    match self.load_npa_report_csv(&path).await {
                        Ok(count) => {
                            total_loaded += count;
                            info!("Loaded {} entries from {:?}", count, path);
                        }
                        Err(e) => {
                            error!("Failed to load {:?}: {}", path, e);
                        }
                    }
                }
            }
        }
        
        info!("Bulk loading completed: {} total entries loaded", total_loaded);
        Ok(total_loaded)
    }

    /// Detect country and region information for a phone number
    pub async fn detect_country(&self, phone_number: &str) -> Result<CountryDetectionResult> {
        let mut stats = self.stats.write().await;
        stats.lookups_performed += 1;
        drop(stats);
        
        // Check cache first
        let cache_key = phone_number.to_string();
        {
            let cache = self.lookup_cache.read().await;
            if let Some(cached_result) = cache.get(&cache_key) {
                let mut stats = self.stats.write().await;
                stats.cache_hits += 1;
                return Ok(cached_result.clone());
            }
        }
        
        let mut stats = self.stats.write().await;
        stats.cache_misses += 1;
        drop(stats);
        
        // Parse phone number
        let (country_calling_code, npa, nxx, _subscriber) = self.parse_phone_number(phone_number)?;
        
        // Look up in database
        let result = self.lookup_number_info(&country_calling_code, &npa, nxx.as_deref()).await?;
        
        // Cache result
        let mut cache = self.lookup_cache.write().await;
        cache.insert(cache_key, result.clone());
        
        // Limit cache size
        if cache.len() > 10000 {
            // Remove oldest 1000 entries (simple LRU approximation)
            let keys_to_remove: Vec<String> = cache.keys().take(1000).cloned().collect();
            for key in keys_to_remove {
                cache.remove(&key);
            }
        }
        
        Ok(result)
    }

    /// Get ANI (Automatic Number Identification) country information
    pub async fn get_ani_country(&self, ani: &str) -> Result<String> {
        let result = self.detect_country(ani).await?;
        Ok(result.country_code)
    }

    /// Get DNIS (Dialed Number Identification Service) country information
    pub async fn get_dnis_country(&self, dnis: &str) -> Result<String> {
        let result = self.detect_country(dnis).await?;
        Ok(result.country_code)
    }

    /// Get DID (Direct Inward Dialing) country information
    pub async fn get_did_country(&self, did: &str) -> Result<String> {
        let result = self.detect_country(did).await?;
        Ok(result.country_code)
    }

    /// Bulk country detection for multiple numbers
    pub async fn detect_countries_bulk(&self, phone_numbers: &[String]) -> Vec<(String, Result<CountryDetectionResult>)> {
        let mut results = Vec::new();
        
        for phone_number in phone_numbers {
            let result = self.detect_country(phone_number).await;
            results.push((phone_number.clone(), result));
        }
        
        results
    }

    /// Parse phone number into components
    /// Accepts numbers with or without + prefix, removes all spaces and formatting
    fn parse_phone_number(&self, phone_number: &str) -> Result<(String, String, Option<String>, Option<String>)> {
        // Clean phone number: remove +, spaces, dashes, parentheses, dots
        let cleaned = phone_number
            .trim_start_matches('+')
            .replace(['-', ' ', '(', ')', '.'], "")
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect::<String>();
        
        if cleaned.len() < 7 {
            return Err(anyhow!("Phone number too short: {}", phone_number));
        }
        
        // Handle NANP numbers (1 + 10 digits)
        if cleaned.starts_with('1') && cleaned.len() == 11 {
            let npa = cleaned[1..4].to_string();
            let nxx = cleaned[4..7].to_string();
            let subscriber = cleaned[7..11].to_string();
            return Ok(("1".to_string(), npa, Some(nxx), Some(subscriber)));
        }
        
        // Handle NANP numbers without country code (10 digits)
        if cleaned.len() == 10 && !cleaned.starts_with('0') {
            let npa = cleaned[0..3].to_string();
            let nxx = cleaned[3..6].to_string();
            let subscriber = cleaned[6..10].to_string();
            return Ok(("1".to_string(), npa, Some(nxx), Some(subscriber)));
        }
        
        // Handle other international numbers
        let country_calling_code = self.extract_country_calling_code(&cleaned)?;
        let remaining = &cleaned[country_calling_code.len()..];
        
        if remaining.len() >= 3 {
            let npa = if remaining.len() >= 3 {
                remaining[0..3].to_string()
            } else {
                remaining.to_string()
            };
            let nxx = if remaining.len() >= 6 {
                Some(remaining[3..6].to_string())
            } else if remaining.len() > 3 {
                Some(remaining[3..].to_string())
            } else {
                None
            };
            let subscriber = if remaining.len() > 6 {
                Some(remaining[6..].to_string())
            } else {
                None
            };
            
            return Ok((country_calling_code, npa, nxx, subscriber));
        }
        
        Err(anyhow!("Unable to parse phone number: {}", phone_number))
    }

    /// Extract country calling code from phone number
    fn extract_country_calling_code(&self, cleaned_number: &str) -> Result<String> {
        // Common country calling codes (longest first for proper matching)
        let country_codes = [
            "44", "49", "33", "39", "34", "46", "41", "31", "32", "43", "45", "47", "48", "30", "36", "420", "421",
            "61", "64", "65", "60", "62", "66", "84", "86", "82", "81", "852", "853", "886", "65", "91", "92", "93", "94", "95", "98",
            "7", "380", "375", "372", "371", "370", "373", "374", "994", "995", "996", "998", "992", "993", "997",
            "52", "54", "55", "56", "57", "58", "51", "53", "506", "507", "508", "509", "504", "503", "502", "501",
            "212", "213", "216", "218", "220", "221", "222", "223", "224", "225", "226", "227", "228", "229", "230",
            "231", "232", "233", "234", "235", "236", "237", "238", "239", "240", "241", "242", "243", "244", "245",
            "246", "248", "249", "250", "251", "252", "253", "254", "255", "256", "257", "258", "260", "261", "262",
            "263", "264", "265", "266", "267", "268", "269", "290", "291", "297", "298", "299",
        ];
        
        for code in &country_codes {
            if cleaned_number.starts_with(code) {
                return Ok(code.to_string());
            }
        }
        
        // Default to single digit
        if !cleaned_number.is_empty() {
            return Ok(cleaned_number[0..1].to_string());
        }
        
        Err(anyhow!("Unable to extract country calling code"))
    }

    /// Look up number information in database
    async fn lookup_number_info(&self, country_calling_code: &str, npa: &str, nxx: Option<&str>) -> Result<CountryDetectionResult> {
        let database = self.npa_database.read().await;
        
        // Normalize country calling code (remove + if present)
        let normalized_country_code = country_calling_code.trim_start_matches('+');
        
        // Try exact match first (with NXX)
        if let Some(nxx_val) = nxx {
            let exact_key = NpaLookupKey {
                country_calling_code: normalized_country_code.to_string(),
                npa: npa.to_string(),
                nxx: Some(nxx_val.to_string()),
            };
            
            if let Some(entries) = database.get(&exact_key) {
                if let Some(entry) = entries.first() {
                    return Ok(self.entry_to_detection_result(entry, 1.0));
                }
            }
        }
        
        // Try NPA-only match
        let npa_key = NpaLookupKey {
            country_calling_code: normalized_country_code.to_string(),
            npa: npa.to_string(),
            nxx: None,
        };
        
        if let Some(entries) = database.get(&npa_key) {
            if let Some(entry) = entries.first() {
                let confidence = if nxx.is_some() { 0.8 } else { 0.9 };
                return Ok(self.entry_to_detection_result(entry, confidence));
            }
        }
        
        // Try broader country-level matching
        for (key, entries) in database.iter() {
            if key.country_calling_code == normalized_country_code {
                if let Some(entry) = entries.first() {
                    return Ok(self.entry_to_detection_result(entry, 0.5));
                }
            }
        }
        
        // Fallback to country calling code information
        let country_codes = self.country_calling_codes.read().await;
        if let Some(country_info) = country_codes.get(normalized_country_code) {
            return Ok(CountryDetectionResult {
                country_code: country_info.country_code.clone(),
                country_name: country_info.country_name.clone(),
                region: None,
                city: None,
                timezone: None,
                is_mobile: false,
                is_toll_free: false,
                carrier: None,
                number_type: None,
                confidence: 0.3,
            });
        }
        
        Err(anyhow!("Country information not found for calling code: {}", normalized_country_code))
    }

    /// Convert NPA entry to detection result
    fn entry_to_detection_result(&self, entry: &NpaReportEntry, confidence: f32) -> CountryDetectionResult {
        CountryDetectionResult {
            country_code: entry.country_code.clone(),
            country_name: entry.country_name.clone(),
            region: entry.region.clone(),
            city: entry.city.clone(),
            timezone: entry.timezone.clone(),
            is_mobile: entry.is_mobile.unwrap_or(false),
            is_toll_free: entry.is_toll_free.unwrap_or(false),
            carrier: entry.carrier.clone(),
            number_type: entry.number_type.clone(),
            confidence,
        }
    }

    /// Validate NPA entry
    fn validate_npa_entry(&self, entry: &NpaReportEntry) -> Result<()> {
        if entry.npa.len() != 3 || !entry.npa.chars().all(|c| c.is_ascii_digit()) {
            return Err(anyhow!("Invalid NPA: {}", entry.npa));
        }
        
        if let Some(ref nxx) = entry.nxx {
            if nxx.len() != 3 || !nxx.chars().all(|c| c.is_ascii_digit()) {
                return Err(anyhow!("Invalid NXX: {}", nxx));
            }
        }
        
        if entry.country_code.len() != 2 {
            return Err(anyhow!("Invalid country code: {}", entry.country_code));
        }
        
        Ok(())
    }

    /// Index NPA entries for efficient lookup
    async fn index_npa_entries(&self, entries: Vec<NpaReportEntry>) -> Result<usize> {
        let mut database = self.npa_database.write().await;
        let mut count = 0;
        
        for entry in entries {
            // Determine country calling code from country code
            let country_calling_code = self.country_code_to_calling_code(&entry.country_code);
            
            // Create lookup keys
            let mut keys = Vec::new();
            
            // Add exact key (with NXX if available)
            if let Some(ref nxx) = entry.nxx {
                keys.push(NpaLookupKey {
                    country_calling_code: country_calling_code.clone(),
                    npa: entry.npa.clone(),
                    nxx: Some(nxx.clone()),
                });
            }
            
            // Add NPA-only key
            keys.push(NpaLookupKey {
                country_calling_code: country_calling_code,
                npa: entry.npa.clone(),
                nxx: None,
            });
            
            // Add entry to database under all keys
            for key in keys {
                database.entry(key).or_insert_with(Vec::new).push(entry.clone());
                count += 1;
            }
        }
        
        Ok(count)
    }

    /// Convert country code to calling code (without + prefix)
    fn country_code_to_calling_code(&self, country_code: &str) -> String {
        match country_code {
            "US" | "CA" => "1".to_string(),
            "GB" => "44".to_string(),
            "DE" => "49".to_string(),
            "FR" => "33".to_string(),
            "IT" => "39".to_string(),
            "ES" => "34".to_string(),
            "AU" => "61".to_string(),
            "JP" => "81".to_string(),
            "KR" => "82".to_string(),
            "IN" => "91".to_string(),
            "CN" => "86".to_string(),
            "BR" => "55".to_string(),
            "MX" => "52".to_string(),
            "RU" => "7".to_string(),
            _ => "1".to_string(), // Default fallback
        }
    }

    /// Initialize default country calling codes
    async fn initialize_default_country_codes(&self) -> Result<()> {
        let mut country_codes = self.country_calling_codes.write().await;
        
        let default_codes = [
            ("1", "US", "United States", 10, 11),
            ("1", "CA", "Canada", 10, 11),
            ("44", "GB", "United Kingdom", 10, 11),
            ("49", "DE", "Germany", 11, 12),
            ("33", "FR", "France", 10, 10),
            ("39", "IT", "Italy", 10, 11),
            ("34", "ES", "Spain", 9, 9),
            ("46", "SE", "Sweden", 9, 9),
            ("47", "NO", "Norway", 8, 8),
            ("45", "DK", "Denmark", 8, 8),
            ("31", "NL", "Netherlands", 9, 9),
            ("32", "BE", "Belgium", 9, 9),
            ("41", "CH", "Switzerland", 9, 9),
            ("43", "AT", "Austria", 11, 12),
            ("61", "AU", "Australia", 9, 9),
            ("64", "NZ", "New Zealand", 8, 9),
            ("81", "JP", "Japan", 10, 11),
            ("82", "KR", "South Korea", 10, 11),
            ("86", "CN", "China", 11, 11),
            ("91", "IN", "India", 10, 10),
            ("55", "BR", "Brazil", 10, 11),
            ("52", "MX", "Mexico", 10, 10),
            ("7", "RU", "Russia", 10, 10),
        ];
        
        for (calling_code, country_code, country_name, min_len, max_len) in &default_codes {
            country_codes.insert(calling_code.to_string(), CountryInfo {
                country_code: country_code.to_string(),
                country_name: country_name.to_string(),
                calling_code: calling_code.to_string(),
                number_length_min: *min_len,
                number_length_max: *max_len,
            });
        }
        
        info!("Initialized {} default country calling codes", default_codes.len());
        Ok(())
    }

    /// Get service statistics
    pub async fn get_stats(&self) -> NpaStats {
        self.stats.read().await.clone()
    }

    /// Clear cache
    pub async fn clear_cache(&self) {
        let mut cache = self.lookup_cache.write().await;
        cache.clear();
        info!("Cleared NPA lookup cache");
    }

    /// Export database as CSV
    pub async fn export_to_csv<P: AsRef<Path>>(&self, file_path: P) -> Result<usize> {
        let database = self.npa_database.read().await;
        let file_path = file_path.as_ref();
        
        let mut writer = csv::Writer::from_path(file_path)?;
        let mut count = 0;
        
        // Write all unique entries
        let mut written_entries = std::collections::HashSet::new();
        
        for entries in database.values() {
            for entry in entries {
                let key = format!("{}-{}-{:?}", entry.country_code, entry.npa, entry.nxx);
                if !written_entries.contains(&key) {
                    writer.serialize(entry)?;
                    written_entries.insert(key);
                    count += 1;
                }
            }
        }
        
        writer.flush()?;
        info!("Exported {} NPA entries to {:?}", count, file_path);
        Ok(count)
    }
}

impl Clone for NpaReportService {
    fn clone(&self) -> Self {
        Self {
            npa_database: Arc::clone(&self.npa_database),
            country_calling_codes: Arc::clone(&self.country_calling_codes),
            lookup_cache: Arc::clone(&self.lookup_cache),
            stats: Arc::clone(&self.stats),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_phone_number_parsing() {
        let service = NpaReportService::new();
        
        // Test NANP number with formatting
        let result = service.parse_phone_number("+1-212-555-1234");
        assert!(result.is_ok());
        let (country_code, npa, nxx, subscriber) = result.unwrap();
        assert_eq!(country_code, "1");
        assert_eq!(npa, "212");
        assert_eq!(nxx, Some("555".to_string()));
        assert_eq!(subscriber, Some("1234".to_string()));
        
        // Test NANP number without + and formatting
        let result = service.parse_phone_number("12125551234");
        assert!(result.is_ok());
        let (country_code, npa, nxx, subscriber) = result.unwrap();
        assert_eq!(country_code, "1");
        assert_eq!(npa, "212");
        assert_eq!(nxx, Some("555".to_string()));
        assert_eq!(subscriber, Some("1234".to_string()));
        
        // Test 10-digit NANP number (assumes US/Canada)
        let result = service.parse_phone_number("2125551234");
        assert!(result.is_ok());
        let (country_code, npa, nxx, subscriber) = result.unwrap();
        assert_eq!(country_code, "1");
        assert_eq!(npa, "212");
        assert_eq!(nxx, Some("555".to_string()));
        assert_eq!(subscriber, Some("1234".to_string()));
        
        // Test UK number with spaces and formatting
        let result = service.parse_phone_number("44 20 7946 0958");
        assert!(result.is_ok());
        let (country_code, npa, _, _) = result.unwrap();
        assert_eq!(country_code, "44");
        assert_eq!(npa, "207");
        
        // Test number with mixed formatting
        let result = service.parse_phone_number("(212) 555-1234");
        assert!(result.is_ok());
        let (country_code, npa, nxx, subscriber) = result.unwrap();
        assert_eq!(country_code, "1");
        assert_eq!(npa, "212");
        assert_eq!(nxx, Some("555".to_string()));
        assert_eq!(subscriber, Some("1234".to_string()));
    }

    #[tokio::test]
    async fn test_country_detection() {
        let service = NpaReportService::new();
        
        // Add test data
        let test_entries = vec![
            NpaReportEntry {
                npa: "212".to_string(),
                nxx: Some("555".to_string()),
                xxxx_start: None,
                xxxx_end: None,
                country_code: "US".to_string(),
                country_name: "United States".to_string(),
                region: Some("New York".to_string()),
                city: Some("New York City".to_string()),
                timezone: Some("America/New_York".to_string()),
                rate_center: None,
                lata: None,
                ocn: None,
                carrier: Some("Verizon".to_string()),
                number_type: Some("Geographic".to_string()),
                is_mobile: Some(false),
                is_toll_free: Some(false),
                effective_date: None,
                last_updated: None,
                notes: None,
            }
        ];
        
        service.index_npa_entries(test_entries).await.unwrap();
        
        // Test detection
        let result = service.detect_country("12125551234").await.unwrap();
        assert_eq!(result.country_code, "US");
        assert_eq!(result.country_name, "United States");
        assert_eq!(result.region, Some("New York".to_string()));
        assert_eq!(result.carrier, Some("Verizon".to_string()));
        assert_eq!(result.confidence, 1.0);
    }

    #[tokio::test] 
    async fn test_ani_dnis_did_country_detection() {
        let service = NpaReportService::new();
        
        // Test with default country codes (should fall back to basic detection)
        let ani_country = service.get_ani_country("15551234567").await;
        assert!(ani_country.is_ok());
        
        let dnis_country = service.get_dnis_country("442079460958").await;
        assert!(dnis_country.is_ok());
        
        let did_country = service.get_did_country("493012345678").await;
        assert!(did_country.is_ok());
    }

    #[tokio::test]
    async fn test_bulk_detection() {
        let service = NpaReportService::new();
        
        let numbers = vec![
            "12125551234".to_string(),
            "442079460958".to_string(),
            "493012345678".to_string(),
        ];
        
        let results = service.detect_countries_bulk(&numbers).await;
        assert_eq!(results.len(), 3);
        
        // All should have results (even if low confidence)
        for (_, result) in results {
            assert!(result.is_ok());
        }
    }
}