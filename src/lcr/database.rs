use anyhow::Result;
use chrono::NaiveTime;
use rust_decimal::Decimal;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::net::IpAddr;
use std::str::FromStr;

use crate::lcr::types::*;

pub struct DatabasePool {
    pub pool: PgPool,
}

impl DatabasePool {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    pub async fn load_vendor_rate_decks(&self) -> Result<Vec<RateDeck>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                name,
                vendor_id as owner_id,
                rate_type,
                effective_date,
                expires_date,
                deck_version,
                parent_deck_id,
                effective_time,
                preload_minutes,
                loaded_at,
                is_staged,
                active
            FROM vendor_rate_decks
            WHERE active = true
            ORDER BY effective_date DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let decks = rows
            .into_iter()
            .map(|row| RateDeck {
                id: row.get("id"),
                name: row.get("name"),
                owner_id: row.get("owner_id"),
                rate_type: match row.get::<String, _>("rate_type").as_str() {
                    "LRN" => RateType::LRN,
                    "DNIS" => RateType::DNIS,
                    _ => RateType::DNIS,
                },
                effective_date: row.get("effective_date"),
                end_date: row.get("expires_date"),
                deck_version: row.get::<Option<i32>, _>("deck_version").unwrap_or(1),
                parent_deck_id: row.get("parent_deck_id"),
                effective_time: row
                    .get::<Option<NaiveTime>, _>("effective_time")
                    .unwrap_or_else(|| {
                        NaiveTime::from_hms_opt(0, 0, 0).expect("Invalid default time 00:00:00")
                    }),
                preload_minutes: row.get::<Option<i32>, _>("preload_minutes").unwrap_or(30),
                loaded_at: row.get("loaded_at"),
                is_staged: row.get::<Option<bool>, _>("is_staged").unwrap_or(false),
                active: row.get("active"),
            })
            .collect();

        Ok(decks)
    }

    pub async fn load_client_rate_decks(&self) -> Result<Vec<RateDeck>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                name,
                client_id as owner_id,
                rate_type,
                effective_date,
                expires_date,
                deck_version,
                parent_deck_id,
                effective_time,
                preload_minutes,
                loaded_at,
                is_staged,
                active
            FROM client_rate_decks
            WHERE active = true
            ORDER BY effective_date DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let decks = rows
            .into_iter()
            .map(|row| RateDeck {
                id: row.get("id"),
                name: row.get("name"),
                owner_id: row.get("owner_id"),
                rate_type: match row.get::<String, _>("rate_type").as_str() {
                    "LRN" => RateType::LRN,
                    "DNIS" => RateType::DNIS,
                    _ => RateType::DNIS,
                },
                effective_date: row.get("effective_date"),
                end_date: row.get("expires_date"),
                deck_version: row.get::<Option<i32>, _>("deck_version").unwrap_or(1),
                parent_deck_id: row.get("parent_deck_id"),
                effective_time: row
                    .get::<Option<NaiveTime>, _>("effective_time")
                    .unwrap_or_else(|| {
                        NaiveTime::from_hms_opt(0, 0, 0).expect("Invalid default time 00:00:00")
                    }),
                preload_minutes: row.get::<Option<i32>, _>("preload_minutes").unwrap_or(30),
                loaded_at: row.get("loaded_at"),
                is_staged: row.get::<Option<bool>, _>("is_staged").unwrap_or(false),
                active: row.get("active"),
            })
            .collect();

        Ok(decks)
    }

    pub async fn load_vendor_nanpa_rates(&self, deck_id: i32) -> Result<Vec<NanpaRate>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                deck_id,
                code,
                inter_rate,
                intra_rate,
                ij_rate,
                local_rate,
                min_increment,
                interval,
                setup_fee
            FROM vendor_nanpa_rates
            WHERE deck_id = $1
            ORDER BY code
            "#,
        )
        .bind(deck_id)
        .fetch_all(&self.pool)
        .await?;

        let rates = rows
            .into_iter()
            .map(|r| NanpaRate {
                id: r.get("id"),
                deck_id: r.get("deck_id"),
                code: r.get("code"),
                inter_rate: r.get("inter_rate"),
                intra_rate: r.get("intra_rate"),
                ij_rate: r.get("ij_rate"),
                local_rate: r.get("local_rate"),
                min_increment: r.get("min_increment"),
                interval: r.get("interval"),
                setup_fee: r.get("setup_fee"),
            })
            .collect();

        Ok(rates)
    }

    pub async fn load_client_nanpa_rates(&self, deck_id: i32) -> Result<Vec<NanpaRate>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                deck_id,
                code,
                inter_rate,
                intra_rate,
                ij_rate,
                local_rate,
                min_increment,
                interval,
                setup_fee
            FROM client_nanpa_rates
            WHERE deck_id = $1
            ORDER BY code
            "#,
        )
        .bind(deck_id)
        .fetch_all(&self.pool)
        .await?;

        let rates = rows
            .into_iter()
            .map(|r| NanpaRate {
                id: r.get("id"),
                deck_id: r.get("deck_id"),
                code: r.get("code"),
                inter_rate: r.get("inter_rate"),
                intra_rate: r.get("intra_rate"),
                ij_rate: r.get("ij_rate"),
                local_rate: r.get("local_rate"),
                min_increment: r.get("min_increment"),
                interval: r.get("interval"),
                setup_fee: r.get("setup_fee"),
            })
            .collect();

        Ok(rates)
    }

    pub async fn load_egress_trunks(&self) -> Result<Vec<EgressTrunk>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                name,
                vendor_id,
                host,
                port,
                transport,
                capacity_limit,
                cps_limit,
                active,
                priority,
                weight,
                tech_prefix,
                supports_international
            FROM egress_trunks
            WHERE active = true
            ORDER BY priority, name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let trunks = rows
            .into_iter()
            .map(|t| {
                let transport_str: Option<String> = t.get("transport");
                EgressTrunk {
                    id: t.get("id"),
                    name: t.get("name"),
                    vendor_id: t.get("vendor_id"),
                    host: t.get("host"),
                    port: t.get::<i32, _>("port") as u16,
                    transport: match transport_str.as_deref() {
                        Some("TCP") => TransportProtocol::Tcp,
                        Some("TLS") => TransportProtocol::Tls,
                        _ => TransportProtocol::Udp,
                    },
                    capacity_limit: t.get::<Option<i32>, _>("capacity_limit").unwrap_or(1000),
                    cps_limit: t
                        .get::<Option<Decimal>, _>("cps_limit")
                        .unwrap_or(Decimal::from(100)),
                    active: t.get::<Option<bool>, _>("active").unwrap_or(true),
                    priority: t.get::<Option<i32>, _>("priority").unwrap_or(100),
                    weight: t.get::<Option<i32>, _>("weight").unwrap_or(1),
                    tech_prefix: t.get("tech_prefix"),
                    supports_international: t
                        .get::<Option<bool>, _>("supports_international")
                        .unwrap_or(false),
                }
            })
            .collect();

        Ok(trunks)
    }

    pub async fn load_ingress_trunks(&self) -> Result<Vec<IngressTrunk>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                name,
                client_id,
                ip_address,
                capacity_limit,
                cps_limit,
                profit_protection,
                min_profit_margin,
                active,
                auth_username,
                auth_password,
                supports_international
            FROM ingress_trunks
            WHERE active = true
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let trunks = rows
            .into_iter()
            .map(|t| {
                let ip_str: String = t.get("ip_address");
                Ok(IngressTrunk {
                    id: t.get("id"),
                    name: t.get("name"),
                    client_id: t.get("client_id"),
                    ip_address: IpAddr::from_str(&ip_str)?,
                    capacity_limit: t.get::<Option<i32>, _>("capacity_limit").unwrap_or(100),
                    cps_limit: t
                        .get::<Option<Decimal>, _>("cps_limit")
                        .unwrap_or(Decimal::from(10)),
                    profit_protection: t
                        .get::<Option<bool>, _>("profit_protection")
                        .unwrap_or(true),
                    min_profit_margin: t
                        .get::<Option<Decimal>, _>("min_profit_margin")
                        .unwrap_or(Decimal::from_str("0.0001")?),
                    active: t.get::<Option<bool>, _>("active").unwrap_or(true),
                    auth_username: t.get("auth_username"),
                    auth_password: t.get("auth_password"),
                    supports_international: t
                        .get::<Option<bool>, _>("supports_international")
                        .unwrap_or(false),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(trunks)
    }

    pub async fn load_lcr_routes(&self) -> Result<Vec<LcrRoute>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                name,
                route_type,
                description,
                active,
                priority
            FROM lcr_routes
            WHERE active = true
            ORDER BY priority, name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let routes = rows
            .into_iter()
            .map(|r| {
                let route_type_str: String = r.get("route_type");
                LcrRoute {
                    id: r.get("id"),
                    name: r.get("name"),
                    route_type: match route_type_str.as_str() {
                        "NANPA" => RouteType::NANPA,
                        "A-Z" => RouteType::AZ,
                        _ => RouteType::OTHER,
                    },
                    description: r.get("description"),
                    active: r.get::<Option<bool>, _>("active").unwrap_or(true),
                    priority: r.get::<Option<i32>, _>("priority").unwrap_or(100),
                }
            })
            .collect();

        Ok(routes)
    }

    pub async fn load_static_routes(&self) -> Result<Vec<StaticRoute>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                ingress_trunk_id,
                egress_trunk_id,
                pattern,
                priority,
                position,
                description,
                active
            FROM static_routes
            WHERE active = true
            ORDER BY priority
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let routes = rows
            .into_iter()
            .map(|r| {
                let position_str: Option<String> = r.get("position");
                StaticRoute {
                    id: r.get("id"),
                    ingress_trunk_id: r.get("ingress_trunk_id"),
                    egress_trunk_id: r.get("egress_trunk_id"),
                    pattern: r.get("pattern"),
                    priority: r.get::<Option<i32>, _>("priority").unwrap_or(100),
                    position: match position_str.as_deref() {
                        Some("AFTER") => RoutePosition::After,
                        _ => RoutePosition::Before,
                    },
                    description: r.get("description"),
                    active: r.get::<Option<bool>, _>("active").unwrap_or(true),
                }
            })
            .collect();

        Ok(routes)
    }

    pub async fn load_route_advance_configs(&self) -> Result<Vec<RouteAdvanceConfig>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                scope,
                scope_id,
                advance_on_codes,
                stop_on_codes
            FROM route_advance_configs
            ORDER BY scope, scope_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let configs = rows
            .into_iter()
            .map(|c| {
                let scope_str: String = c.get("scope");
                RouteAdvanceConfig {
                    id: c.get("id"),
                    scope: match scope_str.as_str() {
                        "INGRESS_TRUNK" => ConfigScope::IngressTrunk,
                        "EGRESS_TRUNK" => ConfigScope::EgressTrunk,
                        _ => ConfigScope::Global,
                    },
                    scope_id: c.get("scope_id"),
                    advance_on_codes: c
                        .get::<Option<Vec<String>>, _>("advance_on_codes")
                        .unwrap_or_default(),
                    stop_on_codes: c
                        .get::<Option<Vec<String>>, _>("stop_on_codes")
                        .unwrap_or_default(),
                }
            })
            .collect();

        Ok(configs)
    }

    pub async fn load_timer_configs(&self) -> Result<Vec<TimerConfig>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                scope,
                scope_id,
                timer_100_to_183_ms,
                timer_max_call_duration_sec,
                timer_post_dial_delay_ms,
                timer_ringing_timeout_sec,
                timer_transaction_timeout_ms
            FROM timer_configs
            ORDER BY scope, scope_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let configs = rows
            .into_iter()
            .map(|c| {
                let scope_str: String = c.get("scope");
                TimerConfig {
                    id: c.get("id"),
                    scope: match scope_str.as_str() {
                        "INGRESS_TRUNK" => ConfigScope::IngressTrunk,
                        "EGRESS_TRUNK" => ConfigScope::EgressTrunk,
                        _ => ConfigScope::Global,
                    },
                    scope_id: c.get("scope_id"),
                    timer_100_to_183_ms: c
                        .get::<Option<i32>, _>("timer_100_to_183_ms")
                        .unwrap_or(30000),
                    timer_max_call_duration_sec: c
                        .get::<Option<i32>, _>("timer_max_call_duration_sec")
                        .unwrap_or(10800),
                    timer_post_dial_delay_ms: c
                        .get::<Option<i32>, _>("timer_post_dial_delay_ms")
                        .unwrap_or(5000),
                    timer_ringing_timeout_sec: c
                        .get::<Option<i32>, _>("timer_ringing_timeout_sec")
                        .unwrap_or(120),
                    timer_transaction_timeout_ms: c
                        .get::<Option<i32>, _>("timer_transaction_timeout_ms")
                        .unwrap_or(32000),
                }
            })
            .collect();

        Ok(configs)
    }

    pub async fn load_nanpa_static(&self) -> Result<Vec<NanpaStatic>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                npa,
                nxx,
                state,
                country,
                lata,
                ocn,
                rate_center,
                switch_clli
            FROM nanpa_static
            ORDER BY npa, nxx
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let entries = rows
            .into_iter()
            .map(|e| NanpaStatic {
                npa: e.get("npa"),
                nxx: e.get("nxx"),
                state: e.get("state"),
                country: e.get("country"),
                lata: e.get("lata"),
                ocn: e.get("ocn"),
                rate_center: e.get("rate_center"),
                switch_clli: e.get("switch_clli"),
            })
            .collect();

        Ok(entries)
    }

    pub async fn get_lrn_cache(&self, tn: &str) -> Result<Option<LrnCacheEntry>> {
        let row = sqlx::query(
            r#"
            SELECT 
                tn,
                lrn,
                spid,
                ocn,
                lata,
                state,
                jurisdiction,
                cached_at,
                expires_at
            FROM lrn_cache
            WHERE tn = $1 AND expires_at > NOW()
            "#,
        )
        .bind(tn)
        .fetch_optional(&self.pool)
        .await?;

        let entry = row.map(|e| {
            let tn: String = e.get("tn");
            let lrn: String = e.get("lrn");
            let jurisdiction_str: Option<String> = e.get("jurisdiction");

            LrnCacheEntry {
                tn: tn.clone(),
                lrn: lrn.clone(),
                spid: e.get("spid"),
                ocn: e.get("ocn"),
                lata: e.get("lata"),
                state: e.get("state"),
                jurisdiction: jurisdiction_str.map(|j| match j.as_str() {
                    "inter" => CallJurisdiction::Interstate,
                    "intra" => CallJurisdiction::Intrastate,
                    "local" => CallJurisdiction::Local,
                    _ => CallJurisdiction::Indeterminate,
                }),
                cached_at: e.get("cached_at"),
                expires_at: e.get("expires_at"),
                ported: lrn != tn, // Assume ported if LRN differs from original TN
                dip_response_time_ms: None,
            }
        });

        Ok(entry)
    }

    pub async fn update_lrn_cache(&self, entry: &LrnCacheEntry) -> Result<()> {
        let jurisdiction_str = entry.jurisdiction.map(|j| match j {
            CallJurisdiction::Interstate => "inter",
            CallJurisdiction::Intrastate => "intra",
            CallJurisdiction::Local => "local",
            CallJurisdiction::Indeterminate => "indeterminate",
        });

        sqlx::query(
            r#"
            INSERT INTO lrn_cache (tn, lrn, spid, ocn, lata, state, jurisdiction, cached_at, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (tn) 
            DO UPDATE SET 
                lrn = EXCLUDED.lrn,
                spid = EXCLUDED.spid,
                ocn = EXCLUDED.ocn,
                lata = EXCLUDED.lata,
                state = EXCLUDED.state,
                jurisdiction = EXCLUDED.jurisdiction,
                cached_at = EXCLUDED.cached_at,
                expires_at = EXCLUDED.expires_at
            "#
        )
        .bind(&entry.tn)
        .bind(&entry.lrn)
        .bind(&entry.spid)
        .bind(&entry.ocn)
        .bind(&entry.lata)
        .bind(&entry.state)
        .bind(jurisdiction_str)
        .bind(entry.cached_at)
        .bind(entry.expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn load_trunk_rate_associations(&self) -> Result<Vec<TrunkRateAssociation>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                egress_trunk_id,
                ingress_trunk_id,
                vendor_deck_id,
                client_deck_id,
                priority
            FROM trunk_rate_associations
            ORDER BY priority
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let associations = rows
            .into_iter()
            .map(|a| TrunkRateAssociation {
                id: a.get("id"),
                egress_trunk_id: a.get("egress_trunk_id"),
                ingress_trunk_id: a.get("ingress_trunk_id"),
                vendor_deck_id: a.get("vendor_deck_id"),
                client_deck_id: a.get("client_deck_id"),
                priority: a.get::<Option<i32>, _>("priority").unwrap_or(100),
            })
            .collect();

        Ok(associations)
    }

    pub async fn load_lcr_route_trunks(&self) -> Result<Vec<LcrRouteTrunk>> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                lcr_route_id,
                egress_trunk_id,
                vendor_deck_id,
                priority,
                weight
            FROM lcr_route_trunks
            ORDER BY lcr_route_id, priority
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let route_trunks = rows
            .into_iter()
            .map(|rt| LcrRouteTrunk {
                id: rt.get("id"),
                lcr_route_id: rt.get("lcr_route_id"),
                egress_trunk_id: rt.get("egress_trunk_id"),
                vendor_deck_id: rt.get("vendor_deck_id"),
                priority: rt.get::<Option<i32>, _>("priority").unwrap_or(100),
                weight: rt.get::<Option<i32>, _>("weight").unwrap_or(1),
            })
            .collect();

        Ok(route_trunks)
    }

    pub async fn update_trunk_usage(
        &self,
        trunk_id: i32,
        trunk_type: TrunkType,
        delta_calls: i32,
    ) -> Result<()> {
        let trunk_type_str = match trunk_type {
            TrunkType::Ingress => "INGRESS",
            TrunkType::Egress => "EGRESS",
        };

        sqlx::query(
            r#"
            INSERT INTO trunk_usage_stats (trunk_id, trunk_type, current_calls, last_call_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT (trunk_id, trunk_type)
            DO UPDATE SET 
                current_calls = trunk_usage_stats.current_calls + $3,
                total_calls = trunk_usage_stats.total_calls + 1,
                last_call_at = NOW(),
                updated_at = NOW()
            "#,
        )
        .bind(trunk_id)
        .bind(trunk_type_str)
        .bind(delta_calls)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Create default international routing plans if they don't exist
    pub async fn ensure_default_routing_plans(&self) -> Result<()> {
        // Check if any routing plans exist
        let count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM international_routing_plans")
                .fetch_one(&self.pool)
                .await?;

        if count == 0 {
            // Create default EEA routing plan
            let eea_plan_id = sqlx::query_scalar::<_, i32>(
                r#"
                INSERT INTO international_routing_plans (
                    name, description,
                    phone_validation_enabled, phone_validation_strict, 
                    phone_validation_default_region, phone_validation_use_country_detection,
                    eea_routing_enabled, eea_priority_routing, eea_reduced_rates, eea_rate_reduction,
                    default_jurisdiction, allow_unknown_destinations, max_rate_unknown_destinations,
                    require_strict_validation_unknown, active
                ) VALUES (
                    'Default EEA Routing', 
                    'Default routing plan with phone validation enabled and EEA optimization',
                    true, false, 'US', true,
                    true, true, true, 0.1000,
                    'ROW', true, 1.0000,
                    false, true
                ) RETURNING id
                "#
            )
            .fetch_one(&self.pool)
            .await?;

            // Create default ROW routing plan
            let row_plan_id = sqlx::query_scalar::<_, i32>(
                r#"
                INSERT INTO international_routing_plans (
                    name, description,
                    phone_validation_enabled, phone_validation_strict, 
                    phone_validation_default_region, phone_validation_use_country_detection,
                    eea_routing_enabled, eea_priority_routing, eea_reduced_rates, eea_rate_reduction,
                    default_jurisdiction, allow_unknown_destinations, max_rate_unknown_destinations,
                    require_strict_validation_unknown, active
                ) VALUES (
                    'Default ROW Routing', 
                    'Default routing plan for Rest of World destinations with basic validation',
                    true, false, 'US', true,
                    false, false, false, 0.0000,
                    'ROW', true, 2.0000,
                    true, true
                ) RETURNING id
                "#
            )
            .fetch_one(&self.pool)
            .await?;

            // Create strict validation routing plan
            let _strict_plan_id = sqlx::query_scalar::<_, i32>(
                r#"
                INSERT INTO international_routing_plans (
                    name, description,
                    phone_validation_enabled, phone_validation_strict, 
                    phone_validation_default_region, phone_validation_use_country_detection,
                    eea_routing_enabled, eea_priority_routing, eea_reduced_rates, eea_rate_reduction,
                    default_jurisdiction, allow_unknown_destinations, max_rate_unknown_destinations,
                    require_strict_validation_unknown, active
                ) VALUES (
                    'Strict Validation Plan', 
                    'High-security routing plan with strict phone number validation',
                    true, true, 'US', true,
                    true, true, true, 0.0500,
                    'ROW', false, 0.5000,
                    true, true
                ) RETURNING id
                "#
            )
            .fetch_one(&self.pool)
            .await?;

            // Add EEA country preferences for the EEA routing plan
            let eea_countries = vec![
                ("AT", "Austria"),
                ("BE", "Belgium"),
                ("BG", "Bulgaria"),
                ("CY", "Cyprus"),
                ("CZ", "Czech Republic"),
                ("DE", "Germany"),
                ("DK", "Denmark"),
                ("EE", "Estonia"),
                ("ES", "Spain"),
                ("FI", "Finland"),
                ("FR", "France"),
                ("GR", "Greece"),
                ("HR", "Croatia"),
                ("HU", "Hungary"),
                ("IE", "Ireland"),
                ("IS", "Iceland"),
                ("IT", "Italy"),
                ("LI", "Liechtenstein"),
                ("LT", "Lithuania"),
                ("LU", "Luxembourg"),
                ("LV", "Latvia"),
                ("MT", "Malta"),
                ("NL", "Netherlands"),
                ("NO", "Norway"),
                ("PL", "Poland"),
                ("PT", "Portugal"),
                ("RO", "Romania"),
                ("SE", "Sweden"),
                ("SI", "Slovenia"),
                ("SK", "Slovakia"),
            ];

            for (code, name) in &eea_countries {
                sqlx::query(
                    r#"
                    INSERT INTO country_routing_preferences (
                        routing_plan_id, country_code, country_name,
                        jurisdiction, quality_score, cost_multiplier,
                        require_validation, max_duration_minutes
                    ) VALUES ($1, $2, $3, 'EEA', 95, 0.9, true, 0)
                    "#,
                )
                .bind(eea_plan_id)
                .bind(code)
                .bind(name)
                .execute(&self.pool)
                .await?;
            }

            // Add some common ROW countries for the ROW routing plan
            let row_countries = vec![
                ("US", "United States"),
                ("CA", "Canada"),
                ("MX", "Mexico"),
                ("AU", "Australia"),
                ("NZ", "New Zealand"),
                ("JP", "Japan"),
                ("KR", "South Korea"),
                ("CN", "China"),
                ("IN", "India"),
                ("BR", "Brazil"),
                ("AR", "Argentina"),
                ("CL", "Chile"),
                ("ZA", "South Africa"),
                ("RU", "Russia"),
                ("TR", "Turkey"),
                ("AE", "United Arab Emirates"),
                ("SA", "Saudi Arabia"),
            ];

            for (code, name) in &row_countries {
                sqlx::query(
                    r#"
                    INSERT INTO country_routing_preferences (
                        routing_plan_id, country_code, country_name,
                        jurisdiction, quality_score, cost_multiplier,
                        require_validation, max_duration_minutes
                    ) VALUES ($1, $2, $3, 'ROW', 85, 1.0, false, 0)
                    "#,
                )
                .bind(row_plan_id)
                .bind(code)
                .bind(name)
                .execute(&self.pool)
                .await?;
            }

            println!("Created default international routing plans:");
            println!("  - Default EEA Routing (ID: {})", eea_plan_id);
            println!("  - Default ROW Routing (ID: {})", row_plan_id);
            println!("  - Added {} EEA country preferences", eea_countries.len());
            println!("  - Added {} ROW country preferences", row_countries.len());
        }

        Ok(())
    }
}
