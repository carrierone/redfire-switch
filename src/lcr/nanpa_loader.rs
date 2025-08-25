use anyhow::{anyhow, Result};
use csv::Reader;
use std::path::Path;
use tracing::{info, warn};

use crate::lcr::database::DatabasePool;
use crate::lcr::types::NanpaStatic;

/// Load NANPA data from CSV files into the database
pub struct NanpaDataLoader;

impl NanpaDataLoader {
    /// Load NANPA data from the npa-nxx-companytype-ocn.csv file
    pub async fn load_from_csv(db: &DatabasePool, csv_path: &Path) -> Result<usize> {
        if !csv_path.exists() {
            return Err(anyhow!("NANPA CSV file not found: {:?}", csv_path));
        }

        info!("Loading NANPA data from {:?}", csv_path);

        let mut reader = Reader::from_path(csv_path)?;
        let mut count = 0;
        let mut batch = Vec::new();

        for result in reader.records() {
            let record = result?;

            // Expected CSV format:
            // NPA,NXX,CompanyType,OCN,CompanyName,RateCenter,State
            if record.len() < 7 {
                warn!("Skipping malformed record: {:?}", record);
                continue;
            }

            let npa = record.get(0).unwrap_or("").to_string();
            let nxx = record.get(1).unwrap_or("").to_string();
            let ocn = record.get(3).map(|s| s.to_string());
            let rate_center = record.get(5).map(|s| s.to_string());
            let state = record.get(6).unwrap_or("").to_string();

            // Skip if essential fields are missing
            if npa.is_empty() || state.is_empty() {
                continue;
            }

            let entry = NanpaStatic {
                npa: npa.clone(),
                nxx: if nxx.is_empty() { None } else { Some(nxx) },
                state,
                country: "US".to_string(),
                lata: None, // Could be parsed from extended data
                ocn,
                rate_center,
                switch_clli: None,
            };

            batch.push(entry);
            count += 1;

            // Insert in batches of 1000
            if batch.len() >= 1000 {
                Self::insert_batch(db, &batch).await?;
                batch.clear();
            }
        }

        // Insert remaining records
        if !batch.is_empty() {
            Self::insert_batch(db, &batch).await?;
        }

        info!("Loaded {} NANPA records", count);
        Ok(count)
    }

    /// Load NPA report data for additional information
    pub async fn load_npa_report(db: &DatabasePool, csv_path: &Path) -> Result<usize> {
        if !csv_path.exists() {
            return Err(anyhow!("NPA report file not found: {:?}", csv_path));
        }

        info!("Loading NPA report from {:?}", csv_path);

        let mut reader = Reader::from_path(csv_path)?;
        let mut count = 0;

        for result in reader.records() {
            let record = result?;

            // Expected format: NPA,Type,Assignable,InService,Location,Country,TimeZone,etc.
            if record.len() < 7 {
                continue;
            }

            let npa = record.get(0).unwrap_or("").to_string();
            let location = record.get(4).unwrap_or("").to_string();
            let country = record.get(5).unwrap_or("US").to_string();

            // Extract state from location if possible
            let state = Self::extract_state_from_location(&location);

            if npa.is_empty() || state.is_empty() {
                continue;
            }

            // Update existing NPA entries with additional info
            Self::update_npa_info(db, &npa, &state, &country).await?;
            count += 1;
        }

        info!("Updated {} NPA records from report", count);
        Ok(count)
    }

    async fn insert_batch(db: &DatabasePool, batch: &[NanpaStatic]) -> Result<()> {
        // Build bulk insert query
        let mut query = String::from(
            "INSERT INTO nanpa_static (npa, nxx, state, country, ocn, rate_center) VALUES ",
        );

        let mut values = Vec::new();
        for (i, entry) in batch.iter().enumerate() {
            if i > 0 {
                query.push_str(", ");
            }
            query.push_str(&format!(
                "(${},${},${},${},${},${})",
                i * 6 + 1,
                i * 6 + 2,
                i * 6 + 3,
                i * 6 + 4,
                i * 6 + 5,
                i * 6 + 6
            ));

            values.push(entry.npa.clone());
            values.push(entry.nxx.clone().unwrap_or_default());
            values.push(entry.state.clone());
            values.push(entry.country.clone());
            values.push(entry.ocn.clone().unwrap_or_default());
            values.push(entry.rate_center.clone().unwrap_or_default());
        }

        query.push_str(
            " ON CONFLICT (npa, nxx) DO UPDATE SET 
            state = EXCLUDED.state,
            country = EXCLUDED.country,
            ocn = EXCLUDED.ocn,
            rate_center = EXCLUDED.rate_center",
        );

        // This is a placeholder - need to implement proper parameterized query
        // For now, insert one by one to avoid SQL injection
        for entry in batch {
            sqlx::query(
                "INSERT INTO nanpa_static (npa, nxx, state, country, ocn, rate_center) 
                 VALUES ($1, $2, $3, $4, $5, $6)
                 ON CONFLICT (npa, nxx) DO UPDATE SET 
                     state = EXCLUDED.state,
                     country = EXCLUDED.country,
                     ocn = EXCLUDED.ocn,
                     rate_center = EXCLUDED.rate_center"
            )
            .bind(&entry.npa)
            .bind(&entry.nxx)
            .bind(&entry.state)
            .bind(&entry.country)
            .bind(&entry.ocn)
            .bind(&entry.rate_center)
            .execute(&db.pool)
            .await?;
        }

        Ok(())
    }

    async fn update_npa_info(
        db: &DatabasePool,
        npa: &str,
        state: &str,
        country: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE nanpa_static SET state = $2, country = $3 
             WHERE npa = $1 AND nxx IS NULL"
        )
        .bind(npa)
        .bind(state)
        .bind(country)
        .execute(&db.pool)
        .await?;

        Ok(())
    }

    fn extract_state_from_location(location: &str) -> String {
        // Extract state abbreviation from location strings like "California", "New York", etc.
        let state_map = vec![
            ("California", "CA"),
            ("New York", "NY"),
            ("Texas", "TX"),
            ("Florida", "FL"),
            ("Illinois", "IL"),
            ("Pennsylvania", "PA"),
            ("Ohio", "OH"),
            ("Georgia", "GA"),
            ("North Carolina", "NC"),
            ("Michigan", "MI"),
            ("New Jersey", "NJ"),
            ("Virginia", "VA"),
            ("Washington", "WA"),
            ("Arizona", "AZ"),
            ("Massachusetts", "MA"),
            ("Tennessee", "TN"),
            ("Indiana", "IN"),
            ("Missouri", "MO"),
            ("Maryland", "MD"),
            ("Wisconsin", "WI"),
            ("Colorado", "CO"),
            ("Minnesota", "MN"),
            ("South Carolina", "SC"),
            ("Alabama", "AL"),
            ("Louisiana", "LA"),
            ("Kentucky", "KY"),
            ("Oregon", "OR"),
            ("Oklahoma", "OK"),
            ("Connecticut", "CT"),
            ("Utah", "UT"),
            ("Iowa", "IA"),
            ("Nevada", "NV"),
            ("Arkansas", "AR"),
            ("Mississippi", "MS"),
            ("Kansas", "KS"),
            ("New Mexico", "NM"),
            ("Nebraska", "NE"),
            ("West Virginia", "WV"),
            ("Idaho", "ID"),
            ("Hawaii", "HI"),
            ("New Hampshire", "NH"),
            ("Maine", "ME"),
            ("Montana", "MT"),
            ("Rhode Island", "RI"),
            ("Delaware", "DE"),
            ("South Dakota", "SD"),
            ("North Dakota", "ND"),
            ("Alaska", "AK"),
            ("Vermont", "VT"),
            ("Wyoming", "WY"),
            ("District of Columbia", "DC"),
        ];

        for (full_name, abbr) in state_map {
            if location.contains(full_name) {
                return abbr.to_string();
            }
        }

        // Try to find state abbreviation at the end of location
        let parts: Vec<&str> = location.split(',').collect();
        if let Some(last) = parts.last() {
            let trimmed = last.trim();
            if trimmed.len() == 2 && trimmed.chars().all(|c| c.is_alphabetic()) {
                return trimmed.to_uppercase();
            }
        }

        "".to_string()
    }
}

/// CLI command to load NANPA data
pub async fn load_nanpa_command(database_url: &str) -> Result<()> {
    let db = DatabasePool::new(database_url).await?;

    // Load main NANPA data
    let nanpa_csv = Path::new("files/npa-nxx-companytype-ocn.csv");
    if nanpa_csv.exists() {
        let count = NanpaDataLoader::load_from_csv(&db, nanpa_csv).await?;
        info!("Loaded {} NANPA records from CSV", count);
    } else {
        warn!("NANPA CSV file not found at {:?}", nanpa_csv);
    }

    // Load NPA report for additional data
    let npa_report = Path::new("files/npa_report.csv");
    if npa_report.exists() {
        let count = NanpaDataLoader::load_npa_report(&db, npa_report).await?;
        info!("Updated {} NPA records from report", count);
    } else {
        warn!("NPA report file not found at {:?}", npa_report);
    }

    Ok(())
}
