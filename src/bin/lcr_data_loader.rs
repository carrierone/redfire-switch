use anyhow::Result;
use clap::{Arg, Command};
use redfire_switch::lcr::data_loader::NanpaLergDataLoader;
use sqlx::postgres::PgPoolOptions;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::init();

    let matches = Command::new("LCR Data Loader")
        .version("1.0")
        .about("Loads NANPA and LERG data into PostgreSQL database")
        .arg(
            Arg::new("database-url")
                .long("database-url")
                .env("DATABASE_URL")
                .default_value("postgresql://postgres:postgres@localhost:5432/lcr")
                .help("PostgreSQL database URL"),
        )
        .arg(
            Arg::new("data-dir")
                .long("data-dir")
                .default_value("./files")
                .help("Directory containing NANPA/LERG CSV files"),
        )
        .arg(
            Arg::new("npa-report")
                .long("npa-report-only")
                .action(clap::ArgAction::SetTrue)
                .help("Load only NANPA NPA report"),
        )
        .arg(
            Arg::new("lerg-only")
                .long("lerg-only")
                .action(clap::ArgAction::SetTrue)
                .help("Load only LERG NPA-NXX data"),
        )
        .get_matches();

    let database_url = matches.get_one::<String>("database-url").unwrap();
    let data_dir = matches.get_one::<String>("data-dir").unwrap();
    let npa_report_only = matches.get_flag("npa-report");
    let lerg_only = matches.get_flag("lerg-only");

    info!("Connecting to database: {}", database_url);
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    let loader = NanpaLergDataLoader::new(pool);

    if npa_report_only {
        let npa_report_path = format!("{}/npa_report.csv", data_dir);
        info!("Loading NANPA NPA report from: {}", npa_report_path);
        match loader.load_nanpa_npa_report(&npa_report_path).await {
            Ok(count) => info!("Successfully loaded {} NANPA NPA records", count),
            Err(e) => warn!("Failed to load NANPA NPA report: {}", e),
        }
    } else if lerg_only {
        let lerg_path = format!("{}/npa-nxx-companytype-ocn.csv", data_dir);
        info!("Loading LERG NPA-NXX data from: {}", lerg_path);
        match loader.load_lerg_npanxx_data(&lerg_path).await {
            Ok(count) => info!("Successfully loaded {} LERG NPA-NXX records", count),
            Err(e) => warn!("Failed to load LERG NPA-NXX data: {}", e),
        }
    } else {
        info!("Loading all NANPA/LERG data from: {}", data_dir);
        match loader.load_all_data(data_dir).await {
            Ok(()) => info!("Successfully loaded all NANPA/LERG data"),
            Err(e) => warn!("Failed to load all data: {}", e),
        }
    }

    // Test jurisdiction determination
    info!("Testing database-driven jurisdiction determination...");

    let test_cases = vec![
        (
            "12125551234",
            "18005551234",
            "Regular to toll-free should be Indeterminate",
        ),
        (
            "14165551234",
            "12125551234",
            "Canadian to US should be Indeterminate",
        ),
        (
            "12125551234",
            "13105551234",
            "US to US should be Interstate",
        ),
        (
            "19005551234",
            "12125551234",
            "Premium to regular should be Indeterminate",
        ),
    ];

    for (ani, dnis, description) in test_cases {
        match loader.get_jurisdiction_from_db(ani, dnis).await {
            Ok(jurisdiction) => info!("✅ {}: {} → {} = {}", description, ani, dnis, jurisdiction),
            Err(e) => warn!("❌ {}: {} → {} failed: {}", description, ani, dnis, e),
        }
    }

    info!("LCR Data Loader completed successfully!");
    Ok(())
}
