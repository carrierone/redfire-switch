use anyhow::Result;
use chrono::NaiveDate;
use sqlx::{PgPool, Row};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use tracing::{debug, info, warn};

/// Data loader for NANPA and LERG files into PostgreSQL
pub struct NanpaLergDataLoader {
    pool: PgPool,
}

impl NanpaLergDataLoader {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Load NANPA NPA report from files/npa_report.csv
    pub async fn load_nanpa_npa_report<P: AsRef<Path>>(&self, file_path: P) -> Result<usize> {
        info!("Loading NANPA NPA report from {:?}", file_path.as_ref());

        let file = File::open(&file_path)?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // Skip header lines
        let _file_date = lines.next().transpose()?;
        let _header = lines.next().transpose()?;

        let mut loaded_count = 0;
        let mut transaction = self.pool.begin().await?;

        // Clear existing data
        sqlx::query("DELETE FROM nanpa_npa_info")
            .execute(&mut *transaction)
            .await?;

        for line in lines {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split(',').collect();
            if fields.len() < 20 {
                warn!("Skipping malformed line: {}", line);
                continue;
            }

            let npa_raw = fields[0];
            if npa_raw.len() != 3 || !npa_raw.chars().all(char::is_numeric) {
                continue; // Skip non-NPA entries
            }
            let npa = format!("1{}", npa_raw); // Convert to 1NPA format

            let type_of_code = if fields[1].is_empty() {
                None
            } else {
                Some(fields[1])
            };
            let assignable = parse_bool_field(fields[2]);
            let reserved = parse_bool_field(fields[4]);
            let assigned = parse_bool_field(fields[5]);
            let assignment_date = parse_date_field(fields[6]);
            let use_type = if fields[7].is_empty() {
                None
            } else {
                Some(fields[7].chars().next().unwrap().to_string())
            };
            let location = if fields[8].is_empty() {
                None
            } else {
                Some(fields[8])
            };
            let country = if fields[9].is_empty() {
                None
            } else {
                Some(fields[9])
            };
            let in_service = parse_bool_field(fields[10]);
            let in_service_date = parse_date_field(fields[11]);
            let status = if fields[12].is_empty() {
                None
            } else {
                Some(fields[12])
            };
            let overlay = parse_bool_field(fields[15]);
            let service_type = if fields[18].is_empty() {
                None
            } else {
                Some(fields[18])
            };
            let time_zone = if fields[19].is_empty() {
                None
            } else {
                Some(fields[19])
            };
            let area_served = if fields[20].is_empty() {
                None
            } else {
                Some(fields[20])
            };

            sqlx::query(
                r#"
                INSERT INTO nanpa_npa_info (
                    npa, type_of_code, assignable, reserved, assigned, assignment_date,
                    use_type, location, country, in_service, in_service_date, status,
                    overlay, service_type, time_zone, area_served
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
                ON CONFLICT (npa) DO UPDATE SET
                    type_of_code = EXCLUDED.type_of_code,
                    assignable = EXCLUDED.assignable,
                    reserved = EXCLUDED.reserved,
                    assigned = EXCLUDED.assigned,
                    assignment_date = EXCLUDED.assignment_date,
                    use_type = EXCLUDED.use_type,
                    location = EXCLUDED.location,
                    country = EXCLUDED.country,
                    in_service = EXCLUDED.in_service,
                    in_service_date = EXCLUDED.in_service_date,
                    status = EXCLUDED.status,
                    overlay = EXCLUDED.overlay,
                    service_type = EXCLUDED.service_type,
                    time_zone = EXCLUDED.time_zone,
                    area_served = EXCLUDED.area_served,
                    updated_at = NOW()
                "#,
            )
            .bind(npa)
            .bind(type_of_code)
            .bind(assignable)
            .bind(reserved)
            .bind(assigned)
            .bind(assignment_date)
            .bind(use_type)
            .bind(location)
            .bind(country)
            .bind(in_service)
            .bind(in_service_date)
            .bind(status)
            .bind(overlay)
            .bind(service_type)
            .bind(time_zone)
            .bind(area_served)
            .execute(&mut *transaction)
            .await?;

            loaded_count += 1;
        }

        transaction.commit().await?;
        info!("Loaded {} NANPA NPA records", loaded_count);
        Ok(loaded_count)
    }

    /// Load LERG NPA-NXX data from files/npa-nxx-companytype-ocn.csv
    pub async fn load_lerg_npanxx_data<P: AsRef<Path>>(&self, file_path: P) -> Result<usize> {
        info!("Loading LERG NPA-NXX data from {:?}", file_path.as_ref());

        let file = File::open(&file_path)?;
        let reader = BufReader::new(file);

        let mut loaded_count = 0;
        let mut transaction = self.pool.begin().await?;

        // Clear existing data
        sqlx::query("DELETE FROM lerg_npanxx_info")
            .execute(&mut *transaction)
            .await?;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split(',').collect();
            if fields.len() < 8 {
                warn!("Skipping malformed line: {}", line);
                continue;
            }

            let npa_raw = fields[0];
            let nxx = fields[1];

            // Validate NPA and NXX
            if npa_raw.len() != 3
                || nxx.len() != 3
                || !npa_raw.chars().all(char::is_numeric)
                || !nxx.chars().all(char::is_numeric)
            {
                continue;
            }

            let npa = format!("1{}", npa_raw); // Convert to 1NPA format
            let npanxx = format!("1{}{}", npa_raw, nxx); // Create 1NPANXX

            let company_type = if fields[2].is_empty() {
                None
            } else {
                Some(fields[2].trim_matches('"'))
            };
            let ocn: Option<i32> = fields[3].parse().ok();
            let company_name = if fields[4].is_empty() {
                None
            } else {
                Some(fields[4].trim_matches('"'))
            };
            let lata: Option<i32> = fields[5].parse().ok();
            let rate_center = if fields[6].is_empty() {
                None
            } else {
                Some(fields[6].trim_matches('"'))
            };
            let state = if fields[7].is_empty() {
                None
            } else {
                Some(fields[7].trim_matches('"'))
            };

            sqlx::query(
                r#"
                INSERT INTO lerg_npanxx_info (
                    npanxx, npa, nxx, company_type, ocn, company_name, lata, rate_center, state
                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT (npanxx) DO UPDATE SET
                    company_type = EXCLUDED.company_type,
                    ocn = EXCLUDED.ocn,
                    company_name = EXCLUDED.company_name,
                    lata = EXCLUDED.lata,
                    rate_center = EXCLUDED.rate_center,
                    state = EXCLUDED.state,
                    updated_at = NOW()
                "#,
            )
            .bind(npanxx)
            .bind(npa)
            .bind(nxx)
            .bind(company_type)
            .bind(ocn)
            .bind(company_name)
            .bind(lata)
            .bind(rate_center)
            .bind(state)
            .execute(&mut *transaction)
            .await?;

            loaded_count += 1;
        }

        transaction.commit().await?;
        info!("Loaded {} LERG NPA-NXX records", loaded_count);
        Ok(loaded_count)
    }

    /// Update the nanpa_static table with LERG data for jurisdiction determination
    pub async fn update_nanpa_static_from_lerg(&self) -> Result<usize> {
        info!("Updating nanpa_static table from LERG data");

        let mut transaction = self.pool.begin().await?;

        // Clear existing nanpa_static data
        sqlx::query("DELETE FROM nanpa_static")
            .execute(&mut *transaction)
            .await?;

        // Insert LERG data into nanpa_static format
        let result = sqlx::query(
            r#"
            INSERT INTO nanpa_static (npa, nxx, state, country, lata, ocn, rate_center)
            SELECT 
                l.npa,
                l.nxx,
                COALESCE(l.state, 'XX') as state,
                CASE 
                    WHEN n.country = 'CANADA' THEN 'CA'
                    WHEN n.country = 'US' THEN 'US' 
                    ELSE 'US'
                END as country,
                l.lata::text as lata,
                l.ocn::text as ocn,
                l.rate_center
            FROM lerg_npanxx_info l
            LEFT JOIN nanpa_npa_info n ON l.npa = n.npa
            WHERE l.npa IS NOT NULL AND l.nxx IS NOT NULL
            "#,
        )
        .execute(&mut *transaction)
        .await?;

        let updated_count = result.rows_affected() as usize;
        transaction.commit().await?;

        info!(
            "Updated {} records in nanpa_static from LERG data",
            updated_count
        );
        Ok(updated_count)
    }

    /// Get jurisdiction for a number using database lookups instead of hardcoded values
    pub async fn get_jurisdiction_from_db(&self, ani: &str, dnis: &str) -> Result<String> {
        let ani_npa = extract_npa(ani);
        let dnis_npa = extract_npa(dnis);

        match (ani_npa, dnis_npa) {
            (Some(ani_area), Some(dnis_area)) => {
                // Check for special service codes first
                if let Some(jurisdiction) = self.check_special_service_code(&ani_area).await? {
                    return Ok(jurisdiction);
                }
                if let Some(jurisdiction) = self.check_special_service_code(&dnis_area).await? {
                    return Ok(jurisdiction);
                }

                // Check if Canadian NPAs
                if self.is_canadian_npa(&ani_area).await?
                    || self.is_canadian_npa(&dnis_area).await?
                {
                    return Ok("indeterminate".to_string());
                }

                // For US numbers, determine jurisdiction by state
                let ani_state = self.get_npa_state(&ani_area).await?;
                let dnis_state = self.get_npa_state(&dnis_area).await?;

                match (ani_state, dnis_state) {
                    (Some(ani_st), Some(dnis_st)) if ani_st == dnis_st => Ok("intra".to_string()),
                    (Some(_), Some(_)) => Ok("inter".to_string()),
                    _ => Ok("indeterminate".to_string()),
                }
            }
            _ => Ok("indeterminate".to_string()),
        }
    }

    async fn check_special_service_code(&self, npa: &str) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT jurisdiction_override FROM special_service_codes WHERE npa = $1 AND active = true"
        )
        .bind(npa)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let jurisdiction: String = row.get("jurisdiction_override");
            Ok(Some(format!(
                "{}Jurisdiction",
                jurisdiction.replace("IJ", "Indeterminate")
            )))
        } else {
            Ok(None)
        }
    }

    async fn is_canadian_npa(&self, npa: &str) -> Result<bool> {
        let row = sqlx::query(
            "SELECT COUNT(*) as count FROM nanpa_npa_info WHERE npa = $1 AND country = 'CANADA'",
        )
        .bind(npa)
        .fetch_one(&self.pool)
        .await?;

        let count: i64 = row.get("count");
        Ok(count > 0)
    }

    async fn get_npa_state(&self, npa: &str) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT location FROM nanpa_npa_info WHERE npa = $1 AND country = 'US' LIMIT 1",
        )
        .bind(npa)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let location: Option<String> = row.get("location");
            Ok(location)
        } else {
            Ok(None)
        }
    }

    /// Load all NANPA/LERG data
    pub async fn load_all_data(&self, data_dir: &str) -> Result<()> {
        let npa_report_path = format!("{}/npa_report.csv", data_dir);
        let lerg_path = format!("{}/npa-nxx-companytype-ocn.csv", data_dir);

        info!("Loading all NANPA/LERG data from {}", data_dir);

        // Load NANPA NPA report
        if Path::new(&npa_report_path).exists() {
            self.load_nanpa_npa_report(&npa_report_path).await?;
        } else {
            warn!("NANPA NPA report not found at {}", npa_report_path);
        }

        // Load LERG NPA-NXX data
        if Path::new(&lerg_path).exists() {
            self.load_lerg_npanxx_data(&lerg_path).await?;
        } else {
            warn!("LERG NPA-NXX data not found at {}", lerg_path);
        }

        // Update nanpa_static from LERG data
        self.update_nanpa_static_from_lerg().await?;

        info!("Completed loading all NANPA/LERG data");
        Ok(())
    }
}

/// Helper functions
fn parse_bool_field(field: &str) -> Option<bool> {
    match field.trim().to_uppercase().as_str() {
        "YES" | "Y" | "TRUE" | "1" => Some(true),
        "NO" | "N" | "FALSE" | "0" => Some(false),
        _ => None,
    }
}

fn parse_date_field(field: &str) -> Option<NaiveDate> {
    if field.trim().is_empty() {
        return None;
    }

    // Try different date formats
    if let Ok(date) = NaiveDate::parse_from_str(field, "%d-%b-%Y") {
        Some(date)
    } else if let Ok(date) = NaiveDate::parse_from_str(field, "%Y-%m-%d") {
        Some(date)
    } else if let Ok(date) = NaiveDate::parse_from_str(field, "%m/%d/%Y") {
        Some(date)
    } else {
        debug!("Could not parse date: {}", field);
        None
    }
}

fn extract_npa(number: &str) -> Option<String> {
    let digits: String = number.chars().filter(|c| c.is_ascii_digit()).collect();

    if digits.starts_with('1') && digits.len() >= 4 {
        Some(format!("1{}", &digits[1..4])) // Return 1NPA format
    } else if digits.len() >= 3 {
        Some(format!("1{}", &digits[0..3])) // Add leading 1 for NPA format
    } else {
        None
    }
}
