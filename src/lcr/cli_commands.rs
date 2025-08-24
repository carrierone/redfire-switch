use anyhow::Result;
use chrono::{DateTime, NaiveTime, Utc};
use clap::{Parser, Subcommand};
use sqlx::Row;
use crate::lcr::deck_loader::DeckLoader;
use crate::lcr::types::{RateType, DeckLoadRequest};

#[derive(Parser)]
#[command(name = "lcr-deck")]
#[command(about = "LCR Deck Management Commands")]
pub struct DeckCli {
    #[arg(long)]
    database_url: String,

    #[command(subcommand)]
    command: DeckCommands,
}

#[derive(Subcommand)]
pub enum DeckCommands {
    /// Load a new vendor rate deck
    LoadVendor {
        /// Deck name
        #[arg(short, long)]
        name: String,

        /// Vendor ID
        #[arg(short, long)]
        vendor_id: i32,

        /// Rate type (LRN or DNIS)
        #[arg(short, long, default_value = "DNIS")]
        rate_type: String,

        /// Effective date (YYYY-MM-DD or YYYY-MM-DD HH:MM:SS)
        #[arg(short, long)]
        effective_date: String,

        /// Effective time (HH:MM:SS, default 00:00:00)
        #[arg(short = 't', long)]
        effective_time: Option<String>,

        /// Preload minutes before cutover (default 30)
        #[arg(short, long, default_value = "30")]
        preload_minutes: i32,

        /// CSV file path with rates
        #[arg(short, long)]
        csv_file: Option<String>,

        /// Force load even if a deck is already active
        #[arg(short = 'f', long)]
        force: bool,
    },

    /// Load a new client rate deck
    LoadClient {
        /// Deck name
        #[arg(short, long)]
        name: String,

        /// Client ID
        #[arg(short, long)]
        client_id: i32,

        /// Rate type (LRN or DNIS)
        #[arg(short, long, default_value = "DNIS")]
        rate_type: String,

        /// Effective date (YYYY-MM-DD or YYYY-MM-DD HH:MM:SS)
        #[arg(short, long)]
        effective_date: String,

        /// Effective time (HH:MM:SS, default 00:00:00)
        #[arg(short = 't', long)]
        effective_time: Option<String>,

        /// Preload minutes before cutover (default 30)
        #[arg(short, long, default_value = "30")]
        preload_minutes: i32,

        /// CSV file path with rates
        #[arg(short, long)]
        csv_file: Option<String>,

        /// Force load even if a deck is already active
        #[arg(short = 'f', long)]
        force: bool,
    },

    /// List all decks with versions
    ListDecks {
        /// Deck type (vendor, client, all)
        #[arg(short = 't', long, default_value = "all")]
        deck_type: String,

        /// Show only active decks
        #[arg(short, long)]
        active_only: bool,

        /// Show upcoming decks
        #[arg(short, long)]
        upcoming: bool,
    },

    /// Show deck versions and history
    ShowVersions {
        /// Deck name
        #[arg(short, long)]
        name: String,

        /// Owner ID (vendor or client ID)
        #[arg(short, long)]
        owner_id: i32,

        /// Deck type (vendor or client)
        #[arg(short = 't', long)]
        deck_type: String,
    },

    /// Show upcoming cutovers
    ShowCutovers {
        /// Hours to look ahead (default 24)
        #[arg(short, long, default_value = "24")]
        hours: i32,
    },

    /// Force preload a deck
    PreloadDeck {
        /// Deck ID
        #[arg(short, long)]
        deck_id: i32,

        /// Deck type (vendor or client)
        #[arg(short = 't', long)]
        deck_type: String,
    },

    /// Cancel a scheduled cutover
    CancelCutover {
        /// Schedule ID
        #[arg(short, long)]
        schedule_id: i32,
    },

    /// Test routing at a specific time
    TestRouting {
        /// ANI (calling number)
        #[arg(short, long)]
        ani: String,

        /// DNIS (called number)
        #[arg(short, long)]
        dnis: String,

        /// Ingress trunk ID
        #[arg(short = 't', long)]
        trunk_id: i32,

        /// Test at date/time (YYYY-MM-DD HH:MM:SS)
        #[arg(short, long)]
        at_time: String,

        /// Compare with current routing
        #[arg(short, long)]
        compare: bool,
    },
}

pub async fn handle_deck_command(cli: DeckCli) -> Result<()> {
    let pool = sqlx::PgPool::connect(&cli.database_url).await?;
    let loader = DeckLoader::new(pool.clone());

    match cli.command {
        DeckCommands::LoadVendor {
            name,
            vendor_id,
            rate_type,
            effective_date,
            effective_time,
            preload_minutes,
            csv_file,
            force: _,
        } => {
            let rate_type = parse_rate_type(&rate_type)?;
            let effective_dt = parse_datetime(&effective_date)?;
            let effective_tm = effective_time
                .map(|t| parse_time(&t))
                .transpose()?;

            let request = DeckLoadRequest {
                deck_name: name.clone(),
                owner_id: vendor_id,
                rate_type,
                effective_date: effective_dt,
                effective_time: effective_tm,
                preload_minutes: Some(preload_minutes),
                rates_csv: csv_file,
                rates_data: None,
            };

            let deck_id = loader.load_vendor_deck(request).await?;
            println!("✓ Loaded vendor deck '{}' with ID {}", name, deck_id);
            println!("  Effective at: {}", effective_dt);
            
            if effective_dt > Utc::now() {
                let preload_at = effective_dt - chrono::Duration::minutes(preload_minutes as i64);
                println!("  Will preload at: {}", preload_at);
            }
        }

        DeckCommands::LoadClient {
            name,
            client_id,
            rate_type,
            effective_date,
            effective_time,
            preload_minutes,
            csv_file,
            force: _,
        } => {
            let rate_type = parse_rate_type(&rate_type)?;
            let effective_dt = parse_datetime(&effective_date)?;
            let effective_tm = effective_time
                .map(|t| parse_time(&t))
                .transpose()?;

            let request = DeckLoadRequest {
                deck_name: name.clone(),
                owner_id: client_id,
                rate_type,
                effective_date: effective_dt,
                effective_time: effective_tm,
                preload_minutes: Some(preload_minutes),
                rates_csv: csv_file,
                rates_data: None,
            };

            let deck_id = loader.load_client_deck(request).await?;
            println!("✓ Loaded client deck '{}' with ID {}", name, deck_id);
            println!("  Effective at: {}", effective_dt);
            
            if effective_dt > Utc::now() {
                let preload_at = effective_dt - chrono::Duration::minutes(preload_minutes as i64);
                println!("  Will preload at: {}", preload_at);
            }
        }

        DeckCommands::ListDecks {
            deck_type,
            active_only,
            upcoming,
        } => {
            list_decks(&pool, &deck_type, active_only, upcoming).await?;
        }

        DeckCommands::ShowVersions {
            name,
            owner_id,
            deck_type,
        } => {
            show_versions(&pool, &name, owner_id, &deck_type).await?;
        }

        DeckCommands::ShowCutovers { hours } => {
            show_cutovers(&pool, hours).await?;
        }

        DeckCommands::PreloadDeck { deck_id, deck_type } => {
            // Find the schedule for this deck
            let schedule_id = sqlx::query_scalar::<_, i32>(
                r#"
                SELECT id FROM deck_cutover_schedule
                WHERE new_deck_id = $1 AND deck_type = $2
                  AND status IN ('scheduled', 'preloading')
                LIMIT 1
                "#
            )
            .bind(deck_id)
            .bind(&deck_type)
            .fetch_optional(&pool)
            .await?;

            if let Some(schedule_id) = schedule_id {
                loader.preload_deck(schedule_id).await?;
                println!("✓ Preloaded {} deck ID {}", deck_type, deck_id);
            } else {
                println!("No pending cutover found for {} deck ID {}", deck_type, deck_id);
            }
        }

        DeckCommands::CancelCutover { schedule_id } => {
            sqlx::query(
                "UPDATE deck_cutover_schedule SET status = 'cancelled' WHERE id = $1"
            )
            .bind(schedule_id)
            .execute(&pool)
            .await?;
            
            println!("✓ Cancelled cutover schedule ID {}", schedule_id);
        }

        DeckCommands::TestRouting {
            ani,
            dnis,
            trunk_id,
            at_time,
            compare,
        } => {
            let test_time = parse_datetime(&at_time)?;
            test_routing(&pool, &ani, &dnis, trunk_id, test_time, compare).await?;
        }
    }

    Ok(())
}

fn parse_rate_type(s: &str) -> Result<RateType> {
    match s.to_uppercase().as_str() {
        "LRN" => Ok(RateType::LRN),
        "DNIS" => Ok(RateType::DNIS),
        _ => Err(anyhow::anyhow!("Invalid rate type: {}", s)),
    }
}

fn parse_datetime(s: &str) -> Result<DateTime<Utc>> {
    // Try parsing with time first
    if let Ok(dt) = DateTime::parse_from_str(&format!("{} +0000", s), "%Y-%m-%d %H:%M:%S %z") {
        return Ok(dt.with_timezone(&Utc));
    }
    
    // Try parsing date only (assume midnight)
    if let Ok(dt) = DateTime::parse_from_str(&format!("{} 00:00:00 +0000", s), "%Y-%m-%d %H:%M:%S %z") {
        return Ok(dt.with_timezone(&Utc));
    }
    
    Err(anyhow::anyhow!("Invalid date format: {}", s))
}

fn parse_time(s: &str) -> Result<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
        .map_err(|e| anyhow::anyhow!("Invalid time format: {}", e))
}

async fn list_decks(pool: &sqlx::PgPool, deck_type: &str, active_only: bool, upcoming: bool) -> Result<()> {
    let mut where_clause = String::new();
    
    if active_only {
        where_clause.push_str(" AND active = true AND effective_date <= NOW() AND (end_date IS NULL OR end_date > NOW())");
    }
    
    if upcoming {
        where_clause.push_str(" AND effective_date > NOW()");
    }

    if deck_type != "all" {
        println!("\n{} Rate Decks:", deck_type.to_uppercase());
        println!("{:-<80}", "");
        
        let query = format!(
            r#"
            SELECT id, name, {}_id as owner_id, rate_type, deck_version, 
                   effective_date, end_date, is_staged, loaded_at
            FROM {}_rate_decks
            WHERE 1=1 {}
            ORDER BY name, deck_version DESC
            "#,
            if deck_type == "vendor" { "vendor" } else { "client" },
            deck_type,
            where_clause
        );
        
        let rows = sqlx::query(&query).fetch_all(pool).await?;
        
        for row in rows {
            let id: i32 = row.get("id");
            let name: String = row.get("name");
            let version: i32 = row.get("deck_version");
            let effective: DateTime<Utc> = row.get("effective_date");
            let end: Option<DateTime<Utc>> = row.get("end_date");
            let staged: bool = row.get("is_staged");
            
            let status = if effective > Utc::now() {
                "FUTURE"
            } else if end.is_none() || end.unwrap() > Utc::now() {
                "ACTIVE"
            } else {
                "EXPIRED"
            };
            
            println!(
                "ID: {:4} | {} v{} | {} | Status: {} {}",
                id, name, version, 
                effective.format("%Y-%m-%d %H:%M"),
                status,
                if staged { "[STAGED]" } else { "" }
            );
        }
    }
    
    Ok(())
}

async fn show_versions(pool: &sqlx::PgPool, name: &str, owner_id: i32, deck_type: &str) -> Result<()> {
    let table = if deck_type == "vendor" {
        "vendor_rate_decks"
    } else {
        "client_rate_decks"
    };
    
    let owner_col = if deck_type == "vendor" {
        "vendor_id"
    } else {
        "client_id"
    };
    
    let query = format!(
        r#"
        SELECT id, deck_version, effective_date, end_date, active, loaded_at
        FROM {}
        WHERE name = $1 AND {} = $2
        ORDER BY deck_version DESC
        "#,
        table, owner_col
    );
    
    let rows = sqlx::query(&query)
        .bind(name)
        .bind(owner_id)
        .fetch_all(pool)
        .await?;
    
    println!("\nDeck Version History: {}", name);
    println!("{:-<80}", "");
    println!("Version | Effective Date        | End Date             | Status");
    println!("{:-<80}", "");
    
    for row in rows {
        let version: i32 = row.get("deck_version");
        let effective: DateTime<Utc> = row.get("effective_date");
        let end: Option<DateTime<Utc>> = row.get("end_date");
        let active: bool = row.get("active");
        
        let status = if !active {
            "INACTIVE"
        } else if effective > Utc::now() {
            "FUTURE"
        } else if end.is_none() || end.unwrap() > Utc::now() {
            "ACTIVE"
        } else {
            "EXPIRED"
        };
        
        println!(
            "v{:3}    | {} | {} | {}",
            version,
            effective.format("%Y-%m-%d %H:%M:%S"),
            end.map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "Never".to_string()),
            status
        );
    }
    
    Ok(())
}

async fn show_cutovers(pool: &sqlx::PgPool, hours: i32) -> Result<()> {
    let rows = sqlx::query(
        r#"
        SELECT id, deck_type, current_deck_id, new_deck_id, 
               cutover_date, preload_at, status
        FROM deck_cutover_schedule
        WHERE cutover_date <= NOW() + $1::interval
          AND status NOT IN ('completed', 'cancelled')
        ORDER BY cutover_date
        "#
    )
    .bind(format!("{} hours", hours))
    .fetch_all(pool)
    .await?;
    
    println!("\nUpcoming Deck Cutovers (next {} hours):", hours);
    println!("{:-<80}", "");
    
    for row in rows {
        let id: i32 = row.get("id");
        let deck_type: String = row.get("deck_type");
        let current_id: i32 = row.get("current_deck_id");
        let new_id: i32 = row.get("new_deck_id");
        let cutover: DateTime<Utc> = row.get("cutover_date");
        let preload: DateTime<Utc> = row.get("preload_at");
        let status: String = row.get("status");
        
        println!(
            "Schedule #{}: {} deck {} -> {}",
            id, deck_type, current_id, new_id
        );
        println!("  Cutover: {}", cutover.format("%Y-%m-%d %H:%M:%S"));
        println!("  Preload: {}", preload.format("%Y-%m-%d %H:%M:%S"));
        println!("  Status:  {}", status);
        println!();
    }
    
    Ok(())
}

async fn test_routing(
    pool: &sqlx::PgPool,
    ani: &str,
    dnis: &str,
    trunk_id: i32,
    test_time: DateTime<Utc>,
    compare: bool,
) -> Result<()> {
    // This would use the RoutingEngineV2 to test routing at specific time
    println!("\nTesting routing at {}", test_time.format("%Y-%m-%d %H:%M:%S"));
    println!("ANI: {} -> DNIS: {}", ani, dnis);
    println!("Ingress Trunk: {}", trunk_id);
    
    // TODO: Implement actual routing test
    
    Ok(())
}