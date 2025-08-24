use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    Router,
    routing::{get, post, delete},
};
use chrono::{DateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::error;

use crate::lcr::deck_loader::DeckLoader;
use crate::lcr::types::{RateType, NanpaRate, DeckLoadRequest};

#[derive(Clone)]
pub struct DeckApiState {
    pub pool: PgPool,
    pub loader: Arc<DeckLoader>,
}

pub fn deck_routes() -> Router<DeckApiState> {
    Router::new()
        .route("/vendor", post(load_vendor_deck))
        .route("/client", post(load_client_deck))
        .route("/vendor/:id", get(get_vendor_deck))
        .route("/client/:id", get(get_client_deck))
        .route("/vendor/:id/versions", get(get_vendor_versions))
        .route("/client/:id/versions", get(get_client_versions))
        .route("/cutovers", get(get_cutovers))
        .route("/cutovers/:id", delete(cancel_cutover))
        .route("/preload/:id", post(preload_deck))
        .route("/test-routing", post(test_routing_at_time))
}

#[derive(Deserialize)]
struct LoadDeckRequest {
    name: String,
    owner_id: i32,
    rate_type: String,
    effective_date: String,
    effective_time: Option<String>,
    preload_minutes: Option<i32>,
    rates: Option<Vec<RateInput>>,
    csv_url: Option<String>,
}

#[derive(Deserialize)]
struct RateInput {
    code: String,
    inter_rate: f64,
    intra_rate: f64,
    ij_rate: f64,
    local_rate: Option<f64>,
    min_increment: Option<i32>,
    interval: Option<i32>,
    setup_fee: Option<f64>,
}

#[derive(Serialize)]
struct DeckResponse {
    id: i32,
    name: String,
    owner_id: i32,
    deck_version: i32,
    effective_date: DateTime<Utc>,
    end_date: Option<DateTime<Utc>>,
    status: String,
    rate_count: Option<i32>,
}

#[derive(Serialize)]
struct DeckVersionResponse {
    versions: Vec<DeckVersion>,
}

#[derive(Serialize)]
struct DeckVersion {
    id: i32,
    version: i32,
    effective_date: DateTime<Utc>,
    end_date: Option<DateTime<Utc>>,
    status: String,
    rate_count: i32,
}

#[derive(Deserialize)]
struct CutoverQuery {
    hours: Option<i32>,
}

#[derive(Serialize)]
struct CutoverResponse {
    schedules: Vec<CutoverSchedule>,
}

#[derive(Serialize)]
struct CutoverSchedule {
    id: i32,
    deck_type: String,
    current_deck_id: i32,
    new_deck_id: i32,
    cutover_date: DateTime<Utc>,
    preload_at: DateTime<Utc>,
    status: String,
}

#[derive(Deserialize)]
struct TestRoutingRequest {
    ani: String,
    dnis: String,
    trunk_id: i32,
    test_time: String,
    compare_with_current: Option<bool>,
}

#[derive(Serialize)]
struct TestRoutingResponse {
    test_time: DateTime<Utc>,
    routes: Vec<RouteInfo>,
    current_routes: Option<Vec<RouteInfo>>,
}

#[derive(Serialize)]
struct RouteInfo {
    egress_trunk: String,
    vendor: String,
    cost: f64,
    sell: f64,
    profit: f64,
    deck_version: i32,
}

async fn load_vendor_deck(
    State(state): State<DeckApiState>,
    Json(req): Json<LoadDeckRequest>,
) -> Result<Json<DeckResponse>, StatusCode> {
    let rate_type = parse_rate_type(&req.rate_type)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let effective_date = parse_datetime(&req.effective_date)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let effective_time = req.effective_time
        .map(|t| parse_time(&t))
        .transpose()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let rates_data = req.rates.map(|rates| {
        rates.into_iter().map(|r| NanpaRate {
            id: 0,
            deck_id: 0,
            code: r.code,
            inter_rate: Decimal::try_from(r.inter_rate).unwrap_or_default(),
            intra_rate: Decimal::try_from(r.intra_rate).unwrap_or_default(),
            ij_rate: Decimal::try_from(r.ij_rate).unwrap_or_default(),
            local_rate: r.local_rate.map(|f| Decimal::try_from(f).unwrap_or_default()),
            min_increment: r.min_increment.unwrap_or(6),
            interval: r.interval.unwrap_or(6),
            setup_fee: r.setup_fee.map(|f| Decimal::try_from(f).unwrap_or_default()),
        }).collect()
    });
    
    let load_request = DeckLoadRequest {
        deck_name: req.name.clone(),
        owner_id: req.owner_id,
        rate_type,
        effective_date,
        effective_time,
        preload_minutes: req.preload_minutes,
        rates_csv: req.csv_url,
        rates_data,
    };
    
    match state.loader.load_vendor_deck(load_request).await {
        Ok(deck_id) => {
            // Get deck info
            let deck = sqlx::query_as::<_, (i32, String, i32, i32, DateTime<Utc>, Option<DateTime<Utc>>)>(
                r#"
                SELECT id, name, vendor_id, deck_version, effective_date, end_date
                FROM vendor_rate_decks
                WHERE id = $1
                "#
            )
            .bind(deck_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            let rate_count = sqlx::query_scalar::<_, i32>(
                "SELECT COUNT(*) FROM vendor_nanpa_rates WHERE deck_id = $1"
            )
            .bind(deck_id)
            .fetch_one(&state.pool)
            .await
            .ok();
            
            let status = if deck.4 > Utc::now() {
                "future"
            } else if deck.5.is_none() || deck.5.unwrap() > Utc::now() {
                "active"
            } else {
                "expired"
            };
            
            Ok(Json(DeckResponse {
                id: deck.0,
                name: deck.1,
                owner_id: deck.2,
                deck_version: deck.3,
                effective_date: deck.4,
                end_date: deck.5,
                status: status.to_string(),
                rate_count,
            }))
        }
        Err(e) => {
            error!("Failed to load vendor deck: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn load_client_deck(
    State(state): State<DeckApiState>,
    Json(req): Json<LoadDeckRequest>,
) -> Result<Json<DeckResponse>, StatusCode> {
    let rate_type = parse_rate_type(&req.rate_type)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let effective_date = parse_datetime(&req.effective_date)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let effective_time = req.effective_time
        .map(|t| parse_time(&t))
        .transpose()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    let rates_data = req.rates.map(|rates| {
        rates.into_iter().map(|r| NanpaRate {
            id: 0,
            deck_id: 0,
            code: r.code,
            inter_rate: Decimal::try_from(r.inter_rate).unwrap_or_default(),
            intra_rate: Decimal::try_from(r.intra_rate).unwrap_or_default(),
            ij_rate: Decimal::try_from(r.ij_rate).unwrap_or_default(),
            local_rate: r.local_rate.map(|f| Decimal::try_from(f).unwrap_or_default()),
            min_increment: r.min_increment.unwrap_or(6),
            interval: r.interval.unwrap_or(6),
            setup_fee: r.setup_fee.map(|f| Decimal::try_from(f).unwrap_or_default()),
        }).collect()
    });
    
    let load_request = DeckLoadRequest {
        deck_name: req.name.clone(),
        owner_id: req.owner_id,
        rate_type,
        effective_date,
        effective_time,
        preload_minutes: req.preload_minutes,
        rates_csv: req.csv_url,
        rates_data,
    };
    
    match state.loader.load_client_deck(load_request).await {
        Ok(deck_id) => {
            // Get deck info
            let deck = sqlx::query_as::<_, (i32, String, i32, i32, DateTime<Utc>, Option<DateTime<Utc>>)>(
                r#"
                SELECT id, name, client_id, deck_version, effective_date, end_date
                FROM client_rate_decks
                WHERE id = $1
                "#
            )
            .bind(deck_id)
            .fetch_one(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            let rate_count = sqlx::query_scalar::<_, i32>(
                "SELECT COUNT(*) FROM client_nanpa_rates WHERE deck_id = $1"
            )
            .bind(deck_id)
            .fetch_one(&state.pool)
            .await
            .ok();
            
            let status = if deck.4 > Utc::now() {
                "future"
            } else if deck.5.is_none() || deck.5.unwrap() > Utc::now() {
                "active"
            } else {
                "expired"
            };
            
            Ok(Json(DeckResponse {
                id: deck.0,
                name: deck.1,
                owner_id: deck.2,
                deck_version: deck.3,
                effective_date: deck.4,
                end_date: deck.5,
                status: status.to_string(),
                rate_count,
            }))
        }
        Err(e) => {
            error!("Failed to load client deck: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_vendor_deck(
    State(state): State<DeckApiState>,
    Path(id): Path<i32>,
) -> Result<Json<DeckResponse>, StatusCode> {
    let deck = sqlx::query_as::<_, (i32, String, i32, i32, DateTime<Utc>, Option<DateTime<Utc>>)>(
        r#"
        SELECT id, name, vendor_id, deck_version, effective_date, end_date
        FROM vendor_rate_decks
        WHERE id = $1
        "#
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    match deck {
        Some(deck) => {
            let rate_count = sqlx::query_scalar::<_, i32>(
                "SELECT COUNT(*) FROM vendor_nanpa_rates WHERE deck_id = $1"
            )
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .ok();
            
            let status = if deck.4 > Utc::now() {
                "future"
            } else if deck.5.is_none() || deck.5.unwrap() > Utc::now() {
                "active"
            } else {
                "expired"
            };
            
            Ok(Json(DeckResponse {
                id: deck.0,
                name: deck.1,
                owner_id: deck.2,
                deck_version: deck.3,
                effective_date: deck.4,
                end_date: deck.5,
                status: status.to_string(),
                rate_count,
            }))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn get_client_deck(
    State(state): State<DeckApiState>,
    Path(id): Path<i32>,
) -> Result<Json<DeckResponse>, StatusCode> {
    let deck = sqlx::query_as::<_, (i32, String, i32, i32, DateTime<Utc>, Option<DateTime<Utc>>)>(
        r#"
        SELECT id, name, client_id, deck_version, effective_date, end_date
        FROM client_rate_decks
        WHERE id = $1
        "#
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    match deck {
        Some(deck) => {
            let rate_count = sqlx::query_scalar::<_, i32>(
                "SELECT COUNT(*) FROM client_nanpa_rates WHERE deck_id = $1"
            )
            .bind(id)
            .fetch_one(&state.pool)
            .await
            .ok();
            
            let status = if deck.4 > Utc::now() {
                "future"
            } else if deck.5.is_none() || deck.5.unwrap() > Utc::now() {
                "active"
            } else {
                "expired"
            };
            
            Ok(Json(DeckResponse {
                id: deck.0,
                name: deck.1,
                owner_id: deck.2,
                deck_version: deck.3,
                effective_date: deck.4,
                end_date: deck.5,
                status: status.to_string(),
                rate_count,
            }))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn get_vendor_versions(
    State(state): State<DeckApiState>,
    Path(id): Path<i32>,
) -> Result<Json<DeckVersionResponse>, StatusCode> {
    // Get the deck name and vendor_id from the provided deck id
    let deck_info = sqlx::query_as::<_, (String, i32)>(
        "SELECT name, vendor_id FROM vendor_rate_decks WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    match deck_info {
        Some((name, vendor_id)) => {
            let versions = sqlx::query_as::<_, (i32, i32, DateTime<Utc>, Option<DateTime<Utc>>, i32)>(
                r#"
                SELECT vrd.id, vrd.deck_version, vrd.effective_date, vrd.end_date,
                       COUNT(vnr.id)::INTEGER as rate_count
                FROM vendor_rate_decks vrd
                LEFT JOIN vendor_nanpa_rates vnr ON vnr.deck_id = vrd.id
                WHERE vrd.name = $1 AND vrd.vendor_id = $2
                GROUP BY vrd.id
                ORDER BY vrd.deck_version DESC
                "#
            )
            .bind(&name)
            .bind(vendor_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            let versions = versions.into_iter().map(|v| {
                let status = if v.2 > Utc::now() {
                    "future"
                } else if v.3.is_none() || v.3.unwrap() > Utc::now() {
                    "active"
                } else {
                    "expired"
                };
                
                DeckVersion {
                    id: v.0,
                    version: v.1,
                    effective_date: v.2,
                    end_date: v.3,
                    status: status.to_string(),
                    rate_count: v.4,
                }
            }).collect();
            
            Ok(Json(DeckVersionResponse { versions }))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn get_client_versions(
    State(state): State<DeckApiState>,
    Path(id): Path<i32>,
) -> Result<Json<DeckVersionResponse>, StatusCode> {
    // Get the deck name and client_id from the provided deck id
    let deck_info = sqlx::query_as::<_, (String, i32)>(
        "SELECT name, client_id FROM client_rate_decks WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    match deck_info {
        Some((name, client_id)) => {
            let versions = sqlx::query_as::<_, (i32, i32, DateTime<Utc>, Option<DateTime<Utc>>, i32)>(
                r#"
                SELECT crd.id, crd.deck_version, crd.effective_date, crd.end_date,
                       COUNT(cnr.id)::INTEGER as rate_count
                FROM client_rate_decks crd
                LEFT JOIN client_nanpa_rates cnr ON cnr.deck_id = crd.id
                WHERE crd.name = $1 AND crd.client_id = $2
                GROUP BY crd.id
                ORDER BY crd.deck_version DESC
                "#
            )
            .bind(&name)
            .bind(client_id)
            .fetch_all(&state.pool)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            let versions = versions.into_iter().map(|v| {
                let status = if v.2 > Utc::now() {
                    "future"
                } else if v.3.is_none() || v.3.unwrap() > Utc::now() {
                    "active"
                } else {
                    "expired"
                };
                
                DeckVersion {
                    id: v.0,
                    version: v.1,
                    effective_date: v.2,
                    end_date: v.3,
                    status: status.to_string(),
                    rate_count: v.4,
                }
            }).collect();
            
            Ok(Json(DeckVersionResponse { versions }))
        }
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn get_cutovers(
    State(state): State<DeckApiState>,
    Query(query): Query<CutoverQuery>,
) -> Result<Json<CutoverResponse>, StatusCode> {
    let hours = query.hours.unwrap_or(24);
    
    let schedules = sqlx::query_as::<_, (i32, String, i32, i32, DateTime<Utc>, DateTime<Utc>, String)>(
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
    .fetch_all(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let schedules = schedules.into_iter().map(|s| CutoverSchedule {
        id: s.0,
        deck_type: s.1,
        current_deck_id: s.2,
        new_deck_id: s.3,
        cutover_date: s.4,
        preload_at: s.5,
        status: s.6,
    }).collect();
    
    Ok(Json(CutoverResponse { schedules }))
}

async fn cancel_cutover(
    State(state): State<DeckApiState>,
    Path(id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    let result = sqlx::query(
        "UPDATE deck_cutover_schedule SET status = 'cancelled' WHERE id = $1"
    )
    .bind(id)
    .execute(&state.pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    if result.rows_affected() > 0 {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn preload_deck(
    State(state): State<DeckApiState>,
    Path(id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    match state.loader.preload_deck(id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            error!("Failed to preload deck: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn test_routing_at_time(
    State(state): State<DeckApiState>,
    Json(req): Json<TestRoutingRequest>,
) -> Result<Json<TestRoutingResponse>, StatusCode> {
    let test_time = parse_datetime(&req.test_time)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    
    // TODO: Implement actual routing test using RoutingEngineV2
    // This is a placeholder response
    Ok(Json(TestRoutingResponse {
        test_time,
        routes: vec![],
        current_routes: if req.compare_with_current.unwrap_or(false) {
            Some(vec![])
        } else {
            None
        },
    }))
}

// Helper functions

fn parse_rate_type(s: &str) -> Result<RateType, ()> {
    match s.to_uppercase().as_str() {
        "LRN" => Ok(RateType::LRN),
        "DNIS" => Ok(RateType::DNIS),
        _ => Err(()),
    }
}

fn parse_datetime(s: &str) -> Result<DateTime<Utc>, ()> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            // Try parsing as YYYY-MM-DD HH:MM:SS
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| ndt.and_utc())
                .or_else(|_| {
                    // Try parsing as YYYY-MM-DD
                    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                        .map(|nd| nd.and_hms_opt(0, 0, 0).unwrap())
                        .map(|ndt| ndt.and_utc())
                })
        })
        .map_err(|_| ())
}

fn parse_time(s: &str) -> Result<NaiveTime, ()> {
    NaiveTime::parse_from_str(s, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M"))
        .map_err(|_| ())
}