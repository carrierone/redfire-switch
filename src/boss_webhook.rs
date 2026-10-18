//! Sends rated wholesale CDRs to redfire-boss (switch_to_boss.wholesale_cdr
//! contract -- see contracts/schemas/switch_to_boss.wholesale_cdr.schema.json).
//! boss trusts these values verbatim via its PASSTHROUGH rating mode; this
//! switch has already done the LRN dip + jurisdiction + rate-deck lookup.
//!
//! Same HMAC-over-"{timestamp}.{raw body}" scheme boss's own UCaaS edge uses
//! (see redfire-boss/src/modules/ucaas/ucaas.controller.ts's
//! verifyUcaasSignature), so boss can verify both producers identically.

use crate::class4_b2bua::CallDetailRecord;
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::Utc;
use ring::hmac;
use serde::Serialize;
use tracing::{error, warn};

#[derive(Debug, Clone, Serialize)]
struct WholesaleCdrPayload {
    external_id: String,
    switch_customer_id: String,
    call_id: String,
    ingress_trunk_id: Option<String>,
    egress_trunk_id: Option<String>,
    direction: &'static str,
    from: String,
    to: String,
    lrn: Option<String>,
    jurisdiction: &'static str,
    start_time: String,
    answer_time: Option<String>,
    end_time: String,
    duration_seconds: u64,
    disposition: &'static str,
    client_rate: f64,
    client_billed_seconds: u64,
    client_cost: f64,
    client_rate_deck_version: Option<String>,
    vendor_rate: Option<f64>,
    vendor_billed_seconds: Option<u64>,
    vendor_cost: Option<f64>,
    vendor_rate_deck_version: Option<String>,
    currency_code: &'static str,
    usage_sequence: u32,
}

fn jurisdiction_str(j: crate::lcr::types::CallJurisdiction) -> &'static str {
    use crate::lcr::types::CallJurisdiction;
    match j {
        CallJurisdiction::Local => "local",
        CallJurisdiction::Intrastate => "intrastate",
        CallJurisdiction::Interstate => "interstate",
        CallJurisdiction::International => "international",
        CallJurisdiction::Indeterminate => "indeterminate",
    }
}

fn disposition_str(cdr: &CallDetailRecord) -> &'static str {
    if cdr.answer_time.is_some() {
        return "answered";
    }
    match cdr.termination_cause {
        Some(486) | Some(600) => "busy",
        Some(480) | Some(408) => "no_answer",
        Some(487) => "canceled",
        _ => "failed",
    }
}

/// Reads BOSS_WEBHOOK_URL / SWITCH_SHARED_SECRET at call time (CDR completion
/// is an infrequent, non-hot-path event, so there's no need to thread a
/// config struct through every constructor just to cache two env vars).
/// Returns Ok(()) and logs a warning -- never an error -- when unconfigured,
/// matching boss's own "integration not configured" posture: a missing
/// webhook target must never fail or block the CDR pipeline that already
/// persisted successfully.
pub async fn send_wholesale_cdr(cdr: &CallDetailRecord) -> Result<()> {
    let Ok(url) = std::env::var("BOSS_WEBHOOK_URL") else {
        warn!("BOSS_WEBHOOK_URL not configured, skipping wholesale CDR sync for call {}", cdr.a_leg_call_id);
        return Ok(());
    };
    let Ok(secret) = std::env::var("SWITCH_SHARED_SECRET") else {
        warn!("SWITCH_SHARED_SECRET not configured, skipping wholesale CDR sync for call {}", cdr.a_leg_call_id);
        return Ok(());
    };

    let payload = build_payload(cdr)?;

    let body = serde_json::to_vec(&payload)
        .map_err(|e| anyhow!("failed to serialize wholesale CDR payload: {}", e))?;
    let timestamp = Utc::now().timestamp().to_string();

    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
    let mut signed_data = format!("{}.", timestamp).into_bytes();
    signed_data.extend_from_slice(&body);
    let tag = hmac::sign(&key, &signed_data);
    let signature = BASE64.encode(tag.as_ref());

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-switch-timestamp", timestamp)
        .header("x-switch-signature", signature)
        .body(body)
        .send()
        .await
        .map_err(|e| anyhow!("failed to send wholesale CDR webhook: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        error!(
            "Wholesale CDR sync rejected for call {}: {} {}",
            cdr.a_leg_call_id, status, text
        );
        return Err(anyhow!("boss rejected wholesale CDR: {} {}", status, text));
    }

    Ok(())
}

/// Builds the switch_to_boss.wholesale_cdr payload from a persisted CDR.
/// Pure/no I/O so it's unit-testable against the contract schema directly.
fn build_payload(cdr: &CallDetailRecord) -> Result<WholesaleCdrPayload> {
    let Some(duration) = cdr.duration_seconds else {
        return Err(anyhow!("CDR {} has no duration_seconds, cannot sync", cdr.a_leg_call_id));
    };
    let Some(end_time) = cdr.end_time else {
        return Err(anyhow!("CDR {} has no end_time, cannot sync", cdr.a_leg_call_id));
    };
    let Some(client_billed_seconds) = cdr.client_billed_seconds else {
        return Err(anyhow!(
            "CDR {} was never rated via apply_lcr_route_costs (no client_billed_seconds) -- not an LCR-routed call, nothing to sync",
            cdr.a_leg_call_id
        ));
    };
    let Some(client_cost) = cdr.client_cost else {
        return Err(anyhow!("CDR {} missing client_cost", cdr.a_leg_call_id));
    };
    let Some(client_rate) = cdr.client_rate else {
        return Err(anyhow!("CDR {} missing client_rate", cdr.a_leg_call_id));
    };

    Ok(WholesaleCdrPayload {
        external_id: format!("switch_{}", cdr.a_leg_call_id),
        // IpAuthConfig.customer_id resolution is itself unimplemented in this
        // codebase today (load_ip_auth_configs returns hardcoded example
        // data) -- ingress_trunk_name is the real, persisted identifier
        // available instead. Swap this for the real customer_id lookup once
        // that's built; boss's voice_accounts.switchAccountId provisioning
        // must match whichever one is actually sent.
        switch_customer_id: cdr.ingress_trunk_name.clone(),
        call_id: cdr.a_leg_call_id.clone(),
        ingress_trunk_id: Some(cdr.ingress_trunk_id.to_string()),
        egress_trunk_id: cdr.egress_trunk_id.map(|id| id.to_string()),
        direction: "termination",
        from: cdr.ani.clone(),
        to: cdr.dnis.clone(),
        lrn: cdr.lrn.clone(),
        jurisdiction: cdr.jurisdiction.map(jurisdiction_str).unwrap_or("unknown"),
        start_time: cdr.start_time.to_rfc3339(),
        answer_time: cdr.answer_time.map(|t| t.to_rfc3339()),
        end_time: end_time.to_rfc3339(),
        duration_seconds: duration,
        disposition: disposition_str(cdr),
        client_rate,
        client_billed_seconds,
        client_cost,
        client_rate_deck_version: cdr.client_rate_deck_version.clone(),
        vendor_rate: cdr.vendor_rate,
        vendor_billed_seconds: cdr.vendor_billed_seconds,
        vendor_cost: cdr.vendor_cost,
        vendor_rate_deck_version: cdr.vendor_rate_deck_version.clone(),
        currency_code: "USD",
        // No real per-customer monotonic sequence counter exists in this
        // codebase yet -- boss stores this passively (gap detection is not
        // currently enforced on ingest), so a constant doesn't break
        // anything functional, only loses the informational gap-signal.
        usage_sequence: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lcr::types::CallJurisdiction;
    use chrono::Utc;

    fn minimal_cdr(duration_seconds: u64) -> CallDetailRecord {
        CallDetailRecord {
            session_id: "sess-1".to_string(),
            a_leg_call_id: "a-leg-1".to_string(),
            b_leg_call_id: None,
            ani: "+15551234567".to_string(),
            dnis: "+14155550100".to_string(),
            start_time: Utc::now(),
            answer_time: Some(Utc::now()),
            end_time: Some(Utc::now()),
            duration_seconds: Some(duration_seconds),
            ingress_trunk_id: 1,
            ingress_trunk_name: "ingress-1".to_string(),
            ingress_ip: "127.0.0.1".parse().unwrap(),
            ingress_rate_per_minute: 0.0,
            ingress_cost: 0.0,
            ingress_revenue: None,
            egress_trunk_id: Some(2),
            egress_trunk_name: Some("egress-1".to_string()),
            egress_ip: None,
            egress_rate_per_minute: 0.0,
            egress_cost: 0.0,
            egress_revenue: None,
            total_cost: 0.0,
            total_revenue: 0.0,
            net_margin: 0.0,
            profit_margin_percent: 0.0,
            codec_negotiated: None,
            transcoding_used: false,
            termination_cause: None,
            termination_reason: None,
            route_attempts: 0,
            final_route: None,
            ani_ii_ingress: None,
            ani_ii_egress: None,
            ani_ii_source: None,
            is_toll_free: false,
            payphone_surcharge_amount: None,
            payphone_surcharge_reason: None,
            lrn: Some("4155550000".to_string()),
            jurisdiction: Some(CallJurisdiction::Interstate),
            client_rate: Some(0.02),
            client_billed_seconds: Some(36),
            client_cost: Some(0.012),
            client_rate_deck_version: Some("14".to_string()),
            vendor_rate: Some(0.008),
            vendor_billed_seconds: Some(60),
            vendor_cost: Some(0.008),
            vendor_rate_deck_version: Some("9".to_string()),
        }
    }

    /// Temporarily swaps the Rust-generated payload in for the real
    /// switch_to_boss.wholesale_cdr.valid.json fixture (restored afterward
    /// either way) and runs the actual Python reference validator against
    /// it -- proves this struct really produces a payload the contract
    /// accepts, not just "it compiles".
    #[test]
    fn build_payload_validates_against_the_real_contract_schema() {
        let cdr = minimal_cdr(31);
        let payload = build_payload(&cdr).expect("build_payload failed");
        let json = serde_json::to_string_pretty(&payload).expect("serialize failed");

        let contracts_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/contracts");
        let fixture_path = format!("{}/fixtures/switch_to_boss.wholesale_cdr.valid.json", contracts_dir);
        let original = std::fs::read_to_string(&fixture_path).expect("failed to read real fixture");

        std::fs::write(&fixture_path, &json).expect("failed to write generated fixture");

        let output = std::process::Command::new("python3")
            .arg(format!("{}/tools/contract_check.py", contracts_dir))
            .arg("--contracts")
            .arg(contracts_dir)
            .output();

        std::fs::write(&fixture_path, &original).expect("failed to restore real fixture");

        let output = output.expect("failed to run contract_check.py -- is python3 installed?");
        assert!(
            output.status.success(),
            "contract_check.py failed against the Rust-generated payload:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Real HTTP round-trip against a local axum server standing in for
    /// boss: proves send_wholesale_cdr's HMAC signature is independently
    /// verifiable with the same "{timestamp}.{raw body}" formula boss's own
    /// verifyUcaasSignature uses, not just that signing code runs.
    #[tokio::test]
    async fn send_wholesale_cdr_produces_a_verifiable_signature() {
        use axum::{extract::State, http::HeaderMap, routing::post, Router};
        use std::sync::{Arc, Mutex};

        #[derive(Clone, Default)]
        struct Captured {
            inner: Arc<Mutex<Option<(HeaderMap, Vec<u8>)>>>,
        }

        async fn capture(State(state): State<Captured>, headers: HeaderMap, body: axum::body::Bytes) -> &'static str {
            *state.inner.lock().unwrap() = Some((headers, body.to_vec()));
            "ok"
        }

        let captured = Captured::default();
        let app = Router::new().route("/webhook", post(capture)).with_state(captured.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let secret = "test-shared-secret";
        std::env::set_var("BOSS_WEBHOOK_URL", format!("http://{}/webhook", addr));
        std::env::set_var("SWITCH_SHARED_SECRET", secret);

        let cdr = minimal_cdr(31);
        send_wholesale_cdr(&cdr).await.expect("send_wholesale_cdr failed");

        std::env::remove_var("BOSS_WEBHOOK_URL");
        std::env::remove_var("SWITCH_SHARED_SECRET");

        let (headers, body) = captured.inner.lock().unwrap().take().expect("server never received a request");
        let timestamp = headers.get("x-switch-timestamp").unwrap().to_str().unwrap().to_string();
        let signature = headers.get("x-switch-signature").unwrap().to_str().unwrap().to_string();

        // Independently recompute the signature exactly as boss's
        // verifyUcaasSignature would, and confirm it matches.
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        let mut signed_data = format!("{}.", timestamp).into_bytes();
        signed_data.extend_from_slice(&body);
        let expected_tag = hmac::sign(&key, &signed_data);
        let expected_signature = BASE64.encode(expected_tag.as_ref());

        assert_eq!(signature, expected_signature, "signature does not match independent recomputation");

        let parsed: serde_json::Value = serde_json::from_slice(&body).expect("body was not valid JSON");
        assert_eq!(parsed["call_id"], "a-leg-1");
        assert_eq!(parsed["client_billed_seconds"], 36);
    }
}
