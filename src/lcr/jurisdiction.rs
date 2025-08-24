use crate::lcr::cache::LcrCache;
use crate::lcr::lrn_dip::LrnDipService;
use crate::lcr::types::{CallJurisdiction, NanpaStatic};
use anyhow::Result;
use sqlx::Row;
use std::sync::Arc;

pub struct JurisdictionCalculator;

impl JurisdictionCalculator {
    pub fn determine_jurisdiction(
        ani_info: Option<&NanpaStatic>,
        dnis_info: Option<&NanpaStatic>,
    ) -> CallJurisdiction {
        match (ani_info, dnis_info) {
            (Some(ani), Some(dnis)) => {
                // Check if same state (intrastate)
                if ani.state == dnis.state {
                    CallJurisdiction::Intra
                } else {
                    CallJurisdiction::Inter
                }
            }
            _ => {
                // If we can't determine, use indeterminate jurisdiction
                CallJurisdiction::Indeterminate
            }
        }
    }

    /// Enhanced jurisdiction determination with special number handling
    pub fn determine_jurisdiction_enhanced(
        ani: &str,
        dnis: &str,
        ani_info: Option<&NanpaStatic>,
        dnis_info: Option<&NanpaStatic>,
    ) -> CallJurisdiction {
        // First check for special numbers that are always Indeterminate
        if Self::is_indeterminate_number(ani) || Self::is_indeterminate_number(dnis) {
            return CallJurisdiction::Indeterminate;
        }

        // Use standard logic for regular NANPA numbers
        Self::determine_jurisdiction(ani_info, dnis_info)
    }

    /// Check if a number should be classified as Indeterminate jurisdiction (SYNC VERSION)
    /// This is a lightweight check for common special service codes
    pub fn is_indeterminate_number(number: &str) -> bool {
        let normalized = Self::normalize_nanpa_number(number);
        if normalized.len() < 4 {
            return false;
        }

        let npa = &normalized[0..4]; // 1NPA format

        // Common toll-free and special service codes
        matches!(
            npa,
            "1800"
                | "1888"
                | "1877"
                | "1866"
                | "1855"
                | "1844"
                | "1833"
                | "1822"
                | "1880"
                | "1881"
                | "1882"
                | "1883"
                | "1884"
                | "1885"
                | "1886"
                | "1887"
                | "1889"
        )
    }

    /// Check if a number should be classified as Indeterminate jurisdiction (DATABASE VERSION)
    /// This function now requires a database connection instead of using hardcoded values
    pub async fn is_indeterminate_number_db(
        pool: &sqlx::PgPool,
        number: &str,
    ) -> Result<bool, sqlx::Error> {
        let normalized = Self::normalize_nanpa_number(number);

        // Check length first
        if normalized.len() < 10 || normalized.len() > 15 {
            return Ok(true); // Invalid length
        }

        // Extract NPA in 1NPA format
        let npa = if normalized.starts_with("1") && normalized.len() >= 4 {
            format!("1{}", &normalized[1..4])
        } else if normalized.len() >= 3 {
            format!("1{}", &normalized[0..3])
        } else {
            return Ok(true); // Too short
        };

        // Check for international access codes (hardcoded as these are not in NANPA)
        if normalized.starts_with("011") || normalized.starts_with("01") {
            return Ok(true); // International
        }

        // Check if it's a special service code
        let special_service_row = sqlx::query(
            "SELECT COUNT(*) as count FROM special_service_codes WHERE npa = $1 AND active = true",
        )
        .bind(&npa)
        .fetch_one(pool)
        .await?;

        let special_count: i64 = special_service_row.get("count");
        if special_count > 0 {
            return Ok(true);
        }

        // Check if Canadian NPA
        let canadian_row = sqlx::query(
            "SELECT COUNT(*) as count FROM nanpa_npa_info WHERE npa = $1 AND country = 'CANADA'",
        )
        .bind(&npa)
        .fetch_one(pool)
        .await?;

        let canadian_count: i64 = canadian_row.get("count");
        if canadian_count > 0 {
            return Ok(true);
        }

        // Check for invalid NPAs (N11, N9X where N=0/1)
        if npa.len() == 4 {
            let npa_digits = &npa[1..]; // Remove leading 1
            if npa_digits.starts_with('0')
                || npa_digits.starts_with('1')
                || npa_digits.ends_with("11")
            {
                return Ok(true);
            }
        }

        // If we get here, it's likely a valid US NPA
        Ok(false)
    }

    /// Enhanced jurisdiction determination using database lookups
    pub async fn determine_jurisdiction_enhanced_db(
        pool: &sqlx::PgPool,
        ani: &str,
        dnis: &str,
        ani_info: Option<&crate::lcr::types::NanpaStatic>,
        dnis_info: Option<&crate::lcr::types::NanpaStatic>,
    ) -> Result<crate::lcr::types::CallJurisdiction, sqlx::Error> {
        // First check for special numbers that are always Indeterminate
        if Self::is_indeterminate_number_db(pool, ani).await?
            || Self::is_indeterminate_number_db(pool, dnis).await?
        {
            return Ok(crate::lcr::types::CallJurisdiction::Indeterminate);
        }

        // Use standard logic for regular NANPA numbers
        Ok(Self::determine_jurisdiction(ani_info, dnis_info))
    }

    pub fn determine_local_jurisdiction(
        _ani: &str,
        _dnis: &str,
        ani_info: Option<&NanpaStatic>,
        dnis_info: Option<&NanpaStatic>,
    ) -> bool {
        match (ani_info, dnis_info) {
            (Some(ani_data), Some(dnis_data)) => {
                // 1. Same rate center = definitely local
                if let (Some(ani_rc), Some(dnis_rc)) =
                    (&ani_data.rate_center, &dnis_data.rate_center)
                {
                    if ani_rc == dnis_rc {
                        return true;
                    }
                }

                // 2. Same LATA and same state might be local (carrier-specific)
                if ani_data.state == dnis_data.state {
                    if let (Some(ani_lata), Some(dnis_lata)) = (&ani_data.lata, &dnis_data.lata) {
                        if ani_lata == dnis_lata {
                            // In many areas, same LATA calls within a state are local
                            // This is carrier and geography specific
                            return Self::is_local_by_lata(&ani_data.state, ani_lata, dnis_lata);
                        }
                    }
                }

                // 3. Check for metropolitan calling areas (NYCs, LA, etc.)
                Self::is_local_by_metro_area(ani_data, dnis_data)
            }
            _ => false,
        }
    }

    /// Check if call is local based on LATA rules for specific states
    fn is_local_by_lata(state: &str, ani_lata: &str, dnis_lata: &str) -> bool {
        // Same LATA in these states is often local
        let local_lata_states = ["CA", "NY", "FL", "TX", "IL"];

        ani_lata == dnis_lata && local_lata_states.contains(&state)
    }

    /// Check for metropolitan local calling areas
    fn is_local_by_metro_area(ani_info: &NanpaStatic, dnis_info: &NanpaStatic) -> bool {
        // Define metro areas where calls between different rate centers are local
        let metro_areas = vec![
            // NYC Metro (multiple rate centers but local calling)
            vec!["MANHATTAN", "BROOKLYN", "QUEENS", "BRONX", "STATEN IS"],
            // LA Metro
            vec!["LOS ANGELES", "HOLLYWOOD", "BEVERLY HILLS", "SANTA MONICA"],
            // Bay Area
            vec!["SAN FRANCISCO", "OAKLAND", "SAN JOSE", "PALO ALTO"],
        ];

        if let (Some(ani_rc), Some(dnis_rc)) = (&ani_info.rate_center, &dnis_info.rate_center) {
            for metro in metro_areas {
                if metro.contains(&ani_rc.as_str()) && metro.contains(&dnis_rc.as_str()) {
                    return true;
                }
            }
        }

        false
    }

    pub fn normalize_nanpa_number(number: &str) -> String {
        // Remove any non-digit characters
        let digits: String = number.chars().filter(|c| c.is_digit(10)).collect();

        // Handle different formats
        if digits.starts_with("1") && digits.len() == 11 {
            // Already in 1NPANXXNNNN format
            digits
        } else if digits.len() == 10 {
            // Add leading 1 for NPANXXNNNN
            format!("1{}", digits)
        } else if digits.starts_with("011") {
            // International number, not NANPA
            digits
        } else {
            // Return as-is if not recognized
            digits
        }
    }

    pub fn extract_npanxx(number: &str) -> Option<String> {
        let normalized = Self::normalize_nanpa_number(number);

        // Check if it's a valid NANPA number (1NPANXXNNNN)
        if normalized.len() == 11 && normalized.starts_with("1") {
            // Extract NPANXX (positions 1-7, skipping the leading 1)
            Some(normalized[1..7].to_string())
        } else if normalized.len() == 10 {
            // NPANXXNNNN format
            Some(normalized[0..6].to_string())
        } else {
            None
        }
    }

    pub fn extract_npa(number: &str) -> Option<String> {
        let normalized = Self::normalize_nanpa_number(number);

        // Check if it's a valid NANPA number
        if normalized.len() == 11 && normalized.starts_with("1") {
            // Extract NPA (positions 1-4, skipping the leading 1)
            Some(normalized[1..4].to_string())
        } else if normalized.len() == 10 {
            // NPANXXNNNN format
            Some(normalized[0..3].to_string())
        } else {
            None
        }
    }

    pub async fn get_jurisdiction_with_lrn(
        cache: &LcrCache,
        ani: &str,
        dnis: &str,
        use_lrn: bool,
    ) -> (CallJurisdiction, Option<String>) {
        // Get the DNIS to use for rating (LRN or original)
        let (rating_dnis, lrn_number) = if use_lrn {
            // Check LRN cache first
            if let Some(lrn_entry) = cache.get_lrn_cached(dnis) {
                let lrn = lrn_entry.lrn.clone();
                (lrn.clone(), Some(lrn))
            } else {
                // Use original DNIS if no LRN found in cache
                // LRN dipping would be done at routing level
                (dnis.to_string(), None)
            }
        } else {
            (dnis.to_string(), None)
        };

        // Get NANPA info for ANI and DNIS
        let ani_npanxx = Self::extract_npanxx(ani);
        let dnis_npanxx = Self::extract_npanxx(&rating_dnis);

        let ani_info = ani_npanxx.and_then(|npanxx| cache.get_nanpa_info(&npanxx));
        let dnis_info = dnis_npanxx.and_then(|npanxx| cache.get_nanpa_info(&npanxx));

        // Determine jurisdiction using enhanced logic
        let jurisdiction = Self::determine_jurisdiction_enhanced(
            ani,
            &rating_dnis,
            ani_info.as_ref(),
            dnis_info.as_ref(),
        );

        // Check for local calling
        if Self::determine_local_jurisdiction(
            ani,
            &rating_dnis,
            ani_info.as_ref(),
            dnis_info.as_ref(),
        ) {
            return (CallJurisdiction::Local, lrn_number);
        }

        (jurisdiction, lrn_number)
    }

    /// Get jurisdiction with LRN dipping support
    pub async fn get_jurisdiction_with_lrn_dip(
        cache: &LcrCache,
        lrn_dip_service: Option<Arc<LrnDipService>>,
        ani: &str,
        dnis: &str,
        use_lrn: bool,
    ) -> (CallJurisdiction, Option<String>) {
        // Get the DNIS to use for rating (LRN or original)
        let (rating_dnis, lrn_number) = if use_lrn {
            // Check LRN cache first
            if let Some(lrn_entry) = cache.get_lrn_cached(dnis) {
                let lrn = lrn_entry.lrn.clone();
                (lrn.clone(), Some(lrn))
            } else if let Some(lrn_service) = lrn_dip_service {
                // Perform LRN dip if service is available and enabled
                match lrn_service.dip_lrn(dnis, Some(ani)).await {
                    Ok(lrn_response) if lrn_response.lrn.is_some() => {
                        let lrn = lrn_response.lrn.unwrap();
                        (lrn.clone(), Some(lrn))
                    }
                    Ok(_) => {
                        // No LRN found, use original DNIS
                        (dnis.to_string(), None)
                    }
                    Err(e) => {
                        tracing::warn!("LRN dip failed for {}: {}", dnis, e);
                        // Fall back to original DNIS on error
                        (dnis.to_string(), None)
                    }
                }
            } else {
                // Use original DNIS if no LRN service
                (dnis.to_string(), None)
            }
        } else {
            (dnis.to_string(), None)
        };

        // Get NANPA info for ANI and DNIS
        let ani_npanxx = Self::extract_npanxx(ani);
        let dnis_npanxx = Self::extract_npanxx(&rating_dnis);

        let ani_info = ani_npanxx.and_then(|npanxx| cache.get_nanpa_info(&npanxx));
        let dnis_info = dnis_npanxx.and_then(|npanxx| cache.get_nanpa_info(&npanxx));

        // Determine jurisdiction using enhanced logic
        let jurisdiction = Self::determine_jurisdiction_enhanced(
            ani,
            &rating_dnis,
            ani_info.as_ref(),
            dnis_info.as_ref(),
        );

        // Check for local calling
        if Self::determine_local_jurisdiction(
            ani,
            &rating_dnis,
            ani_info.as_ref(),
            dnis_info.as_ref(),
        ) {
            return (CallJurisdiction::Local, lrn_number);
        }

        (jurisdiction, lrn_number)
    }
}
