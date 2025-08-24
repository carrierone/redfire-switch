use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::{postgres::PgPoolOptions, PgPool};
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
        let decks = sqlx::query_as!(
            RateDeck,
            r#"
            SELECT 
                id,
                name,
                vendor_id as owner_id,
                rate_type as "rate_type: RateType",
                effective_date,
                expires_date,
                active
            FROM vendor_rate_decks
            WHERE active = true
            ORDER BY effective_date DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(decks)
    }

    pub async fn load_client_rate_decks(&self) -> Result<Vec<RateDeck>> {
        let decks = sqlx::query_as!(
            RateDeck,
            r#"
            SELECT 
                id,
                name,
                client_id as owner_id,
                rate_type as "rate_type: RateType",
                effective_date,
                expires_date,
                active
            FROM client_rate_decks
            WHERE active = true
            ORDER BY effective_date DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(decks)
    }

    pub async fn load_vendor_nanpa_rates(&self, deck_id: i32) -> Result<Vec<NanpaRate>> {
        let rates = sqlx::query!(
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
            deck_id
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| NanpaRate {
            id: r.id,
            deck_id: r.deck_id,
            code: r.code,
            inter_rate: r.inter_rate,
            intra_rate: r.intra_rate,
            ij_rate: r.ij_rate,
            local_rate: r.local_rate,
            min_increment: r.min_increment,
            interval: r.interval,
            setup_fee: r.setup_fee,
        })
        .collect();

        Ok(rates)
    }

    pub async fn load_client_nanpa_rates(&self, deck_id: i32) -> Result<Vec<NanpaRate>> {
        let rates = sqlx::query!(
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
            deck_id
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| NanpaRate {
            id: r.id,
            deck_id: r.deck_id,
            code: r.code,
            inter_rate: r.inter_rate,
            intra_rate: r.intra_rate,
            ij_rate: r.ij_rate,
            local_rate: r.local_rate,
            min_increment: r.min_increment,
            interval: r.interval,
            setup_fee: r.setup_fee,
        })
        .collect();

        Ok(rates)
    }

    pub async fn load_egress_trunks(&self) -> Result<Vec<EgressTrunk>> {
        let trunks = sqlx::query!(
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
                tech_prefix
            FROM egress_trunks
            WHERE active = true
            ORDER BY priority, name
            "#
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|t| EgressTrunk {
            id: t.id,
            name: t.name,
            vendor_id: t.vendor_id,
            host: t.host,
            port: t.port as u16,
            transport: match t.transport.as_deref() {
                Some("TCP") => TransportProtocol::TCP,
                Some("TLS") => TransportProtocol::TLS,
                _ => TransportProtocol::UDP,
            },
            capacity_limit: t.capacity_limit.unwrap_or(1000),
            cps_limit: t.cps_limit.unwrap_or(Decimal::from(100)),
            active: t.active.unwrap_or(true),
            priority: t.priority.unwrap_or(100),
            weight: t.weight.unwrap_or(1),
            tech_prefix: t.tech_prefix,
        })
        .collect();

        Ok(trunks)
    }

    pub async fn load_ingress_trunks(&self) -> Result<Vec<IngressTrunk>> {
        let trunks = sqlx::query!(
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
                auth_password
            FROM ingress_trunks
            WHERE active = true
            ORDER BY name
            "#
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|t| {
            Ok(IngressTrunk {
                id: t.id,
                name: t.name,
                client_id: t.client_id,
                ip_address: IpAddr::from_str(&t.ip_address.to_string())?,
                capacity_limit: t.capacity_limit.unwrap_or(100),
                cps_limit: t.cps_limit.unwrap_or(Decimal::from(10)),
                profit_protection: t.profit_protection.unwrap_or(true),
                min_profit_margin: t.min_profit_margin.unwrap_or(Decimal::from_str("0.0001")?),
                active: t.active.unwrap_or(true),
                auth_username: t.auth_username,
                auth_password: t.auth_password,
            })
        })
        .collect::<Result<Vec<_>>>()?;

        Ok(trunks)
    }

    pub async fn load_lcr_routes(&self) -> Result<Vec<LcrRoute>> {
        let routes = sqlx::query!(
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
            "#
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| LcrRoute {
            id: r.id,
            name: r.name,
            route_type: match r.route_type.as_str() {
                "NANPA" => RouteType::NANPA,
                "A-Z" => RouteType::AZ,
                _ => RouteType::OTHER,
            },
            description: r.description,
            active: r.active.unwrap_or(true),
            priority: r.priority.unwrap_or(100),
        })
        .collect();

        Ok(routes)
    }

    pub async fn load_static_routes(&self) -> Result<Vec<StaticRoute>> {
        let routes = sqlx::query!(
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
            "#
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|r| StaticRoute {
            id: r.id,
            ingress_trunk_id: r.ingress_trunk_id,
            egress_trunk_id: r.egress_trunk_id,
            pattern: r.pattern,
            priority: r.priority.unwrap_or(100),
            position: match r.position.as_deref() {
                Some("AFTER") => RoutePosition::After,
                _ => RoutePosition::Before,
            },
            description: r.description,
            active: r.active.unwrap_or(true),
        })
        .collect();

        Ok(routes)
    }

    pub async fn load_route_advance_configs(&self) -> Result<Vec<RouteAdvanceConfig>> {
        let configs = sqlx::query!(
            r#"
            SELECT 
                id,
                scope,
                scope_id,
                advance_on_codes,
                stop_on_codes
            FROM route_advance_configs
            ORDER BY scope, scope_id
            "#
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|c| RouteAdvanceConfig {
            id: c.id,
            scope: match c.scope.as_str() {
                "INGRESS_TRUNK" => ConfigScope::IngressTrunk,
                "EGRESS_TRUNK" => ConfigScope::EgressTrunk,
                _ => ConfigScope::Global,
            },
            scope_id: c.scope_id,
            advance_on_codes: c.advance_on_codes.unwrap_or_default(),
            stop_on_codes: c.stop_on_codes.unwrap_or_default(),
        })
        .collect();

        Ok(configs)
    }

    pub async fn load_timer_configs(&self) -> Result<Vec<TimerConfig>> {
        let configs = sqlx::query!(
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
            "#
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|c| TimerConfig {
            id: c.id,
            scope: match c.scope.as_str() {
                "INGRESS_TRUNK" => ConfigScope::IngressTrunk,
                "EGRESS_TRUNK" => ConfigScope::EgressTrunk,
                _ => ConfigScope::Global,
            },
            scope_id: c.scope_id,
            timer_100_to_183_ms: c.timer_100_to_183_ms.unwrap_or(30000),
            timer_max_call_duration_sec: c.timer_max_call_duration_sec.unwrap_or(10800),
            timer_post_dial_delay_ms: c.timer_post_dial_delay_ms.unwrap_or(5000),
            timer_ringing_timeout_sec: c.timer_ringing_timeout_sec.unwrap_or(120),
            timer_transaction_timeout_ms: c.timer_transaction_timeout_ms.unwrap_or(32000),
        })
        .collect();

        Ok(configs)
    }

    pub async fn load_nanpa_static(&self) -> Result<Vec<NanpaStatic>> {
        let entries = sqlx::query!(
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
            "#
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|e| NanpaStatic {
            npa: e.npa,
            nxx: e.nxx,
            state: e.state,
            country: e.country,
            lata: e.lata,
            ocn: e.ocn,
            rate_center: e.rate_center,
            switch_clli: e.switch_clli,
        })
        .collect();

        Ok(entries)
    }

    pub async fn get_lrn_cache(&self, tn: &str) -> Result<Option<LrnCacheEntry>> {
        let entry = sqlx::query!(
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
            tn
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|e| LrnCacheEntry {
            tn: e.tn,
            lrn: e.lrn,
            spid: e.spid,
            ocn: e.ocn,
            lata: e.lata,
            state: e.state,
            jurisdiction: e.jurisdiction.map(|j| match j.as_str() {
                "INTER" => CallJurisdiction::Interstate,
                "INTRA" => CallJurisdiction::Intrastate,
                "LOCAL" => CallJurisdiction::Local,
                _ => CallJurisdiction::IndeterminateJurisdiction,
            }),
            cached_at: e.cached_at,
            expires_at: e.expires_at,
        });

        Ok(entry)
    }

    pub async fn update_lrn_cache(&self, entry: &LrnCacheEntry) -> Result<()> {
        let jurisdiction_str = entry.jurisdiction.map(|j| match j {
            CallJurisdiction::Interstate => "INTER",
            CallJurisdiction::Intrastate => "INTRA",
            CallJurisdiction::Local => "LOCAL",
            CallJurisdiction::IndeterminateJurisdiction => "IJ",
        });

        sqlx::query!(
            r#"
            INSERT INTO lrn_cache (tn, lrn, spid, ocn, lata, state, jurisdiction, cached_at, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7::call_jurisdiction, $8, $9)
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
            "#,
            entry.tn,
            entry.lrn,
            entry.spid,
            entry.ocn,
            entry.lata,
            entry.state,
            jurisdiction_str,
            entry.cached_at,
            entry.expires_at
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn load_trunk_rate_associations(&self) -> Result<Vec<TrunkRateAssociation>> {
        let associations = sqlx::query!(
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
            "#
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|a| TrunkRateAssociation {
            id: a.id,
            egress_trunk_id: a.egress_trunk_id,
            ingress_trunk_id: a.ingress_trunk_id,
            vendor_deck_id: a.vendor_deck_id,
            client_deck_id: a.client_deck_id,
            priority: a.priority.unwrap_or(100),
        })
        .collect();

        Ok(associations)
    }

    pub async fn load_lcr_route_trunks(&self) -> Result<Vec<LcrRouteTrunk>> {
        let route_trunks = sqlx::query!(
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
            "#
        )
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|rt| LcrRouteTrunk {
            id: rt.id,
            lcr_route_id: rt.lcr_route_id,
            egress_trunk_id: rt.egress_trunk_id,
            vendor_deck_id: rt.vendor_deck_id,
            priority: rt.priority.unwrap_or(100),
            weight: rt.weight.unwrap_or(1),
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

        sqlx::query!(
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
            trunk_id,
            trunk_type_str,
            delta_calls
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
