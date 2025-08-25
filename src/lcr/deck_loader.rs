use super::types::*;
use super::cache::LcrCache;
use super::database::DatabasePool;
use chrono::{DateTime, Utc, NaiveTime};
use rust_decimal::Decimal;
use sqlx::{PgPool, postgres::PgRow, Row};
use anyhow::{Result, anyhow};
use tracing::{info, error, warn};
use std::sync::Arc;

pub struct DeckLoader {
    pool: PgPool,
    cache: Option<Arc<LcrCache>>,
    db_pool: Option<Arc<DatabasePool>>,
}

impl DeckLoader {
    pub fn new(pool: PgPool) -> Self {
        Self { 
            pool,
            cache: None,
            db_pool: None,
        }
    }
    
    pub fn with_cache_and_db(pool: PgPool, cache: Arc<LcrCache>, db_pool: Arc<DatabasePool>) -> Self {
        Self { 
            pool,
            cache: Some(cache),
            db_pool: Some(db_pool),
        }
    }

    /// Load a new vendor rate deck with automatic versioning
    pub async fn load_vendor_deck(&self, request: DeckLoadRequest) -> Result<i32> {
        let mut tx = self.pool.begin().await?;
        
        // Get the current active deck version
        let current_version = self.get_current_vendor_version(&request.deck_name, request.owner_id).await?;
        let new_version = current_version.map_or(1, |v| v + 1);
        
        // Get parent deck ID if this is not the first version
        let parent_deck_id = if new_version > 1 {
            self.get_current_vendor_deck_id(&request.deck_name, request.owner_id).await?
        } else {
            None
        };
        
        // Set effective time (default to midnight GMT)
        let effective_time = request.effective_time.unwrap_or(NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        let preload_minutes = request.preload_minutes.unwrap_or(30);
        
        // Insert new deck version
        let deck_id: i32 = sqlx::query_scalar(
            r#"
            INSERT INTO vendor_rate_decks 
            (name, vendor_id, rate_type, effective_date, deck_version, 
             parent_deck_id, effective_time, preload_minutes, is_staged, active)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, true)
            RETURNING id
            "#
        )
        .bind(&request.deck_name)
        .bind(request.owner_id)
        .bind(format!("{:?}", request.rate_type))
        .bind(request.effective_date)
        .bind(new_version)
        .bind(parent_deck_id)
        .bind(effective_time)
        .bind(preload_minutes)
        .bind(request.effective_date > Utc::now())
        .fetch_one(&mut *tx)
        .await?;
        
        // Load rates if provided
        if let Some(rates) = &request.rates_data {
            self.load_vendor_rates(deck_id, rates.clone(), &mut tx).await?;
        } else if let Some(csv_path) = &request.rates_csv {
            let rates = self.parse_rates_csv(csv_path).await?;
            self.load_vendor_rates(deck_id, rates, &mut tx).await?;
        }
        
        // Handle deck activation based on effective date
        if request.effective_date > Utc::now() {
            // Future deck - schedule cutover
            if let Some(parent_id) = parent_deck_id {
                self.schedule_cutover("vendor", parent_id, deck_id, request.effective_date, preload_minutes, &mut tx).await?;
            }
        } else {
            // Past or current deck - activate immediately
            if let Some(parent_id) = parent_deck_id {
                self.activate_deck_immediately("vendor", parent_id, deck_id, request.effective_date, &mut tx).await?;
            }
        }
        
        // Record in history
        sqlx::query(
            r#"
            INSERT INTO deck_load_history 
            (deck_type, deck_id, deck_version, effective_date, rate_count)
            VALUES ('vendor', $1, $2, $3, $4)
            "#
        )
        .bind(deck_id)
        .bind(new_version)
        .bind(request.effective_date)
        .bind(request.rates_data.as_ref().map(|r| r.len() as i32).unwrap_or(0))
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;
        
        // If this was an immediate activation (past effective_date), reload cache
        if request.effective_date <= Utc::now() {
            if let Some(cache) = &self.cache {
                if let Some(db_pool) = &self.db_pool {
                    if let Err(e) = cache.load_from_database(db_pool).await {
                        warn!("Failed to reload cache after immediate deck activation: {}", e);
                    }
                }
            }
        }
        
        info!(
            "Loaded vendor deck '{}' version {} (ID: {}) effective at {}",
            request.deck_name, new_version, deck_id, request.effective_date
        );
        
        Ok(deck_id)
    }
    
    /// Load a new client rate deck with automatic versioning
    pub async fn load_client_deck(&self, request: DeckLoadRequest) -> Result<i32> {
        let mut tx = self.pool.begin().await?;
        
        // Get the current active deck version
        let current_version = self.get_current_client_version(&request.deck_name, request.owner_id).await?;
        let new_version = current_version.map_or(1, |v| v + 1);
        
        // Get parent deck ID if this is not the first version
        let parent_deck_id = if new_version > 1 {
            self.get_current_client_deck_id(&request.deck_name, request.owner_id).await?
        } else {
            None
        };
        
        // Set effective time (default to midnight GMT)
        let effective_time = request.effective_time.unwrap_or(NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        let preload_minutes = request.preload_minutes.unwrap_or(30);
        
        // Insert new deck version
        let deck_id: i32 = sqlx::query_scalar(
            r#"
            INSERT INTO client_rate_decks 
            (name, client_id, rate_type, effective_date, deck_version, 
             parent_deck_id, effective_time, preload_minutes, is_staged, active)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, true)
            RETURNING id
            "#
        )
        .bind(&request.deck_name)
        .bind(request.owner_id)
        .bind(format!("{:?}", request.rate_type))
        .bind(request.effective_date)
        .bind(new_version)
        .bind(parent_deck_id)
        .bind(effective_time)
        .bind(preload_minutes)
        .bind(request.effective_date > Utc::now())
        .fetch_one(&mut *tx)
        .await?;
        
        // Load rates if provided
        if let Some(rates) = &request.rates_data {
            self.load_client_rates(deck_id, rates.clone(), &mut tx).await?;
        } else if let Some(csv_path) = &request.rates_csv {
            let rates = self.parse_rates_csv(csv_path).await?;
            self.load_client_rates(deck_id, rates, &mut tx).await?;
        }
        
        // Handle deck activation based on effective date
        if request.effective_date > Utc::now() {
            // Future deck - schedule cutover
            if let Some(parent_id) = parent_deck_id {
                self.schedule_cutover("client", parent_id, deck_id, request.effective_date, preload_minutes, &mut tx).await?;
            }
        } else {
            // Past or current deck - activate immediately
            if let Some(parent_id) = parent_deck_id {
                self.activate_deck_immediately("client", parent_id, deck_id, request.effective_date, &mut tx).await?;
            }
        }
        
        tx.commit().await?;
        
        // If this was an immediate activation (past effective_date), reload cache
        if request.effective_date <= Utc::now() {
            if let Some(cache) = &self.cache {
                if let Some(db_pool) = &self.db_pool {
                    if let Err(e) = cache.load_from_database(db_pool).await {
                        warn!("Failed to reload cache after immediate deck activation: {}", e);
                    }
                }
            }
        }
        
        info!(
            "Loaded client deck '{}' version {} (ID: {}) effective at {}",
            request.deck_name, new_version, deck_id, request.effective_date
        );
        
        Ok(deck_id)
    }
    
    /// Get decks that need to be preloaded
    pub async fn get_decks_to_preload(&self) -> Result<Vec<DeckCutoverSchedule>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id, deck_type, current_deck_id, new_deck_id,
                cutover_date, preload_at, status,
                preloaded_at, activated_at
            FROM deck_cutover_schedule
            WHERE status IN ('scheduled', 'preloading')
              AND preload_at <= NOW() + INTERVAL '60 minutes'
              AND preload_at > NOW()
            ORDER BY preload_at
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        let schedules = rows.into_iter().map(|row: PgRow| {
            DeckCutoverSchedule {
                id: row.get("id"),
                deck_type: row.get("deck_type"),
                current_deck_id: row.get("current_deck_id"),
                new_deck_id: row.get("new_deck_id"),
                cutover_date: row.get("cutover_date"),
                preload_at: row.get("preload_at"),
                status: row.get("status"),
                preloaded_at: row.get("preloaded_at"),
                activated_at: row.get("activated_at"),
            }
        }).collect();
        
        Ok(schedules)
    }
    
    /// Preload a deck into cache
    pub async fn preload_deck(&self, schedule_id: i32) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        
        // Update status to preloading
        sqlx::query(
            "UPDATE deck_cutover_schedule SET status = 'preloading', updated_at = NOW() WHERE id = $1"
        )
        .bind(schedule_id)
        .execute(&mut *tx)
        .await?;
        
        // Get the schedule details
        let schedule = sqlx::query(
            "SELECT deck_type, new_deck_id FROM deck_cutover_schedule WHERE id = $1"
        )
        .bind(schedule_id)
        .fetch_one(&mut *tx)
        .await?;
        
        // Mark deck as loaded
        let deck_type: String = schedule.get("deck_type");
        let new_deck_id: i32 = schedule.get("new_deck_id");
        
        match deck_type.as_str() {
            "vendor" => {
                sqlx::query(
                    "UPDATE vendor_rate_decks SET loaded_at = NOW(), is_staged = false WHERE id = $1"
                )
                .bind(new_deck_id)
                .execute(&mut *tx)
                .await?;
            },
            "client" => {
                sqlx::query(
                    "UPDATE client_rate_decks SET loaded_at = NOW(), is_staged = false WHERE id = $1"
                )
                .bind(new_deck_id)
                .execute(&mut *tx)
                .await?;
            },
            _ => {}
        }
        
        // Update schedule status
        sqlx::query(
            "UPDATE deck_cutover_schedule SET status = 'preloaded', preloaded_at = NOW() WHERE id = $1"
        )
        .bind(schedule_id)
        .execute(&mut *tx)
        .await?;
        
        tx.commit().await?;
        
        info!("Preloaded deck for schedule ID {}", schedule_id);
        Ok(())
    }
    
    /// Check and activate decks that have reached their effective date
    pub async fn activate_due_decks(&self) -> Result<Vec<i32>> {
        let mut activated = Vec::new();
        
        let schedules = sqlx::query(
            r#"
            SELECT id, new_deck_id, deck_type
            FROM deck_cutover_schedule
            WHERE status = 'preloaded'
              AND cutover_date <= NOW()
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        
        for row in schedules {
            let schedule_id: i32 = row.get("id");
            let deck_id: i32 = row.get("new_deck_id");
            let deck_type: String = row.get("deck_type");
            
            // Update schedule status
            sqlx::query(
                "UPDATE deck_cutover_schedule SET status = 'active', activated_at = NOW() WHERE id = $1"
            )
            .bind(schedule_id)
            .execute(&self.pool)
            .await?;
            
            activated.push(deck_id);
            
            info!("Activated {} deck ID {} via schedule {}", deck_type, deck_id, schedule_id);
        }
        
        Ok(activated)
    }
    
    // Helper methods
    
    async fn get_current_vendor_version(&self, name: &str, vendor_id: i32) -> Result<Option<i32>> {
        let version = sqlx::query_scalar(
            r#"
            SELECT MAX(deck_version) 
            FROM vendor_rate_decks 
            WHERE name = $1 AND vendor_id = $2 AND deleted = false
            "#
        )
        .bind(name)
        .bind(vendor_id)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(version)
    }
    
    async fn get_current_vendor_deck_id(&self, name: &str, vendor_id: i32) -> Result<Option<i32>> {
        let id = sqlx::query_scalar(
            r#"
            SELECT id 
            FROM vendor_rate_decks 
            WHERE name = $1 AND vendor_id = $2 
              AND (end_date IS NULL OR end_date > NOW())
              AND active = true 
              AND deleted = false
            ORDER BY deck_version DESC
            LIMIT 1
            "#
        )
        .bind(name)
        .bind(vendor_id)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(id)
    }
    
    async fn get_current_client_version(&self, name: &str, client_id: i32) -> Result<Option<i32>> {
        let version = sqlx::query_scalar(
            r#"
            SELECT MAX(deck_version) 
            FROM client_rate_decks 
            WHERE name = $1 AND client_id = $2 AND deleted = false
            "#
        )
        .bind(name)
        .bind(client_id)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(version)
    }
    
    async fn get_current_client_deck_id(&self, name: &str, client_id: i32) -> Result<Option<i32>> {
        let id = sqlx::query_scalar(
            r#"
            SELECT id 
            FROM client_rate_decks 
            WHERE name = $1 AND client_id = $2 
              AND (end_date IS NULL OR end_date > NOW())
              AND active = true 
              AND deleted = false
            ORDER BY deck_version DESC
            LIMIT 1
            "#
        )
        .bind(name)
        .bind(client_id)
        .fetch_optional(&self.pool)
        .await?;
        
        Ok(id)
    }
    
    async fn load_vendor_rates(&self, deck_id: i32, rates: Vec<NanpaRate>, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<()> {
        for rate in rates {
            sqlx::query(
                r#"
                INSERT INTO vendor_nanpa_rates 
                (deck_id, code, inter_rate, intra_rate, ij_rate, local_rate,
                 min_increment, interval, setup_fee)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#
            )
            .bind(deck_id)
            .bind(&rate.code)
            .bind(rate.inter_rate.to_string())
            .bind(rate.intra_rate.to_string())
            .bind(rate.ij_rate.to_string())
            .bind(rate.local_rate.map(|d| d.to_string()))
            .bind(rate.min_increment)
            .bind(rate.interval)
            .bind(rate.setup_fee.map(|d| d.to_string()))
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }
    
    async fn load_client_rates(&self, deck_id: i32, rates: Vec<NanpaRate>, tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<()> {
        for rate in rates {
            sqlx::query(
                r#"
                INSERT INTO client_nanpa_rates 
                (deck_id, code, inter_rate, intra_rate, ij_rate, local_rate,
                 min_increment, interval, setup_fee)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                "#
            )
            .bind(deck_id)
            .bind(&rate.code)
            .bind(rate.inter_rate.to_string())
            .bind(rate.intra_rate.to_string())
            .bind(rate.ij_rate.to_string())
            .bind(rate.local_rate.map(|d| d.to_string()))
            .bind(rate.min_increment)
            .bind(rate.interval)
            .bind(rate.setup_fee.map(|d| d.to_string()))
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }
    
    async fn schedule_cutover(&self, deck_type: &str, current_id: i32, new_id: i32, 
                             cutover_date: DateTime<Utc>, preload_minutes: i32,
                             tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO deck_cutover_schedule 
            (deck_type, current_deck_id, new_deck_id, cutover_date, preload_at, status)
            VALUES ($1, $2, $3, $4, $5, 'scheduled')
            "#
        )
        .bind(deck_type)
        .bind(current_id)
        .bind(new_id)
        .bind(cutover_date)
        .bind(cutover_date - chrono::Duration::minutes(preload_minutes as i64))
        .execute(&mut **tx)
        .await?;
        
        Ok(())
    }
    
    async fn activate_deck_immediately(&self, deck_type: &str, current_deck_id: i32, new_deck_id: i32,
                                     effective_date: DateTime<Utc>,
                                     tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> Result<()> {
        // Set the end_date of the current deck to 1 second before the new effective_date
        let end_date = effective_date - chrono::Duration::seconds(1);
        
        match deck_type {
            "vendor" => {
                // Update the current vendor deck's end_date
                sqlx::query(
                    "UPDATE vendor_rate_decks SET end_date = $1 WHERE id = $2"
                )
                .bind(end_date)
                .bind(current_deck_id)
                .execute(&mut **tx)
                .await?;
                
                // Ensure the new deck is active and not staged
                sqlx::query(
                    "UPDATE vendor_rate_decks SET is_staged = false, active = true, loaded_at = NOW() WHERE id = $1"
                )
                .bind(new_deck_id)
                .execute(&mut **tx)
                .await?;
            },
            "client" => {
                // Update the current client deck's end_date
                sqlx::query(
                    "UPDATE client_rate_decks SET end_date = $1 WHERE id = $2"
                )
                .bind(end_date)
                .bind(current_deck_id)
                .execute(&mut **tx)
                .await?;
                
                // Ensure the new deck is active and not staged
                sqlx::query(
                    "UPDATE client_rate_decks SET is_staged = false, active = true, loaded_at = NOW() WHERE id = $1"
                )
                .bind(new_deck_id)
                .execute(&mut **tx)
                .await?;
            },
            _ => {
                return Err(anyhow!("Unknown deck type: {}", deck_type));
            }
        }
        
        info!(
            "Immediately activated {} deck {} (replacing deck {}), effective date: {}",
            deck_type, new_deck_id, current_deck_id, effective_date
        );
        
        Ok(())
    }
    
    async fn parse_rates_csv(&self, csv_path: &str) -> Result<Vec<NanpaRate>> {
        // CSV parsing implementation
        // This would read the CSV and parse it into NanpaRate structs
        todo!("Implement CSV parsing")
    }
    
    /// Safely soft delete a vendor deck (prevents ID reuse)
    pub async fn soft_delete_vendor_deck(&self, deck_id: i32) -> Result<()> {
        let result = sqlx::query_scalar::<_, bool>(
            "SELECT soft_delete_vendor_deck($1)"
        )
        .bind(deck_id)
        .fetch_one(&self.pool)
        .await?;
        
        if !result {
            return Err(anyhow!("Failed to soft delete vendor deck {}", deck_id));
        }
        
        // Reload cache if available
        if let Some(cache) = &self.cache {
            if let Some(db_pool) = &self.db_pool {
                if let Err(e) = cache.load_from_database(db_pool).await {
                    warn!("Failed to reload cache after soft deletion: {}", e);
                }
            }
        }
        
        info!("Successfully soft deleted vendor deck {}", deck_id);
        Ok(())
    }
    
    /// Safely soft delete a client deck (prevents ID reuse)
    pub async fn soft_delete_client_deck(&self, deck_id: i32) -> Result<()> {
        let result = sqlx::query_scalar::<_, bool>(
            "SELECT soft_delete_client_deck($1)"
        )
        .bind(deck_id)
        .fetch_one(&self.pool)
        .await?;
        
        if !result {
            return Err(anyhow!("Failed to soft delete client deck {}", deck_id));
        }
        
        // Reload cache if available
        if let Some(cache) = &self.cache {
            if let Some(db_pool) = &self.db_pool {
                if let Err(e) = cache.load_from_database(db_pool).await {
                    warn!("Failed to reload cache after soft deletion: {}", e);
                }
            }
        }
        
        info!("Successfully soft deleted client deck {}", deck_id);
        Ok(())
    }
    
    /// Safely delete an entire deck version chain
    pub async fn soft_delete_deck_chain(&self, deck_name: &str, owner_id: i32, deck_type: &str) -> Result<i32> {
        let deleted_count = sqlx::query_scalar::<_, i32>(
            "SELECT soft_delete_deck_chain($1, $2, $3)"
        )
        .bind(deck_name)
        .bind(owner_id)
        .bind(deck_type)
        .fetch_one(&self.pool)
        .await?;
        
        // Reload cache if available
        if let Some(cache) = &self.cache {
            if let Some(db_pool) = &self.db_pool {
                if let Err(e) = cache.load_from_database(db_pool).await {
                    warn!("Failed to reload cache after chain deletion: {}", e);
                }
            }
        }
        
        info!("Successfully soft deleted {} versions of {} deck '{}'", 
              deleted_count, deck_type, deck_name);
        
        Ok(deleted_count)
    }
    
}

/// Background task to manage deck preloading and activation
pub async fn deck_manager_task(pool: PgPool) {
    let loader = DeckLoader::new(pool);
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
    
    loop {
        interval.tick().await;
        
        // Check for decks to preload
        match loader.get_decks_to_preload().await {
            Ok(schedules) => {
                for schedule in schedules {
                    if schedule.preload_at <= Utc::now() {
                        if let Err(e) = loader.preload_deck(schedule.id).await {
                            error!("Failed to preload deck {}: {}", schedule.id, e);
                        }
                    }
                }
            }
            Err(e) => error!("Failed to get decks to preload: {}", e),
        }
        
        // Activate due decks
        match loader.activate_due_decks().await {
            Ok(activated) => {
                if !activated.is_empty() {
                    info!("Activated {} decks", activated.len());
                }
            }
            Err(e) => error!("Failed to activate due decks: {}", e),
        }
    }
}