# Redfire Switch: Stabilization Audit (2026-07-04)

Baseline re-evaluation of a year-old codebase. Source of truth for the stabilization effort.
Toolchain: rustc 1.91.0. Workspace: 5 crates, ~140k LOC Rust.

## Build health (baseline)

| Target set | Status |
|---|---|
| `cargo check --workspace --lib --bins` | **PASS** (0 errors) |
| Examples | 4 broken |
| Benches | 2 broken |
| Integration tests | 9 broken |
| Lib unit tests | 320 pass / 28 fail / 1+ hang / 3 ignored |
| `redfire-codec-engine` tests | 30 pass |
| `redfire-sip-stack` tests | 25 pass, **doctest broken** |
| `redfire-mcp-server` | builds |

## Decision policy (from user)
- Delete dead demos. Get it green first. Automate test calling. Defer GPU.
- Keep the custom SIP stack (exists for SIP-T / SIP-I support).

## Broken examples -> DELETE (dead demos, API drift)
- `examples/sip_demo.rs`
- `examples/integrated_sip_codec_demo.rs`
- `examples/sip_codec_integration_demo.rs`
- `examples/dtmf_showcase.rs` (referenced by docs/DTMF_IMPLEMENTATION_SUMMARY.md -> update doc)

Other examples currently compile; leave unless they break.

## Broken benches -> DELETE for now (GPU/perf deferred)
- `benches/performance_validation.rs` (referenced by scripts/run_performance_validation.sh -> remove script ref)
- `benches/dtmf_benchmarks.rs`

Keep `benches/g729_benchmark.rs` if it compiles.

## Broken integration tests -> REPAIR (real features, API drift only)
All failures are API drift (renamed methods, changed signatures, missing derives, async fn used as non-async).
| Test | Root cause category |
|---|---|
| `tests/integration_tests.rs` | E0308 mismatched types (6) |
| `tests/routing_engine_v2_tests.rs` | test fns are `async` but not `#[tokio::test]` (E0277) |
| `tests/g729_annex_tests.rs` | `G729FrameType` missing `Eq`/`Hash` derives (fix in lib) |
| `tests/dtmf_integration_tests.rs` | renamed methods: `generate_sequence`, `get_statistics`; arg-count drift |
| `tests/security_integration_tests.rs` | drift |
| `tests/lrn_dip_tests.rs` | drift (20 errors) |
| `tests/compliance_performance_tests.rs` | drift |
| `tests/anti_fraud_monitoring_tests.rs` | drift |
| `tests/etsi_li_compliance_tests.rs` | drift (36 errors) |

## Failing lib unit tests -> REPAIR (real bugs)
Categories:
- **Runtime-context bugs**: constructors call `tokio::spawn` eagerly, so non-async tests panic
  with "no reactor running" (e.g. `billing.rs:359`, `services::routing`, `services::registry`).
  `services::tests::test_service_registry_core_functionality` HANGS >60s.
  Fix: don't spawn in constructor, or gate background task startup behind an explicit `start()`.
- **Logic/assertion drift**: `ani_ii` toll-free/OLI, `sip_rfc_compliance` URI + multipart parsing,
  `security::config` JWT validation, `billing` credit/funds checks, `plugins::loader`,
  `route_advancement`, `termination_routing`, `api::tests` app-state/auth, `lcr` intl phone validation.

Full failing set (28): ani_ii::test_toll_free_detection, ani_ii_rfc_compliant::test_rfc_compliant_oli_parsing,
api::test_app_state_builders, api::test_protected_endpoint_without_auth, billing::test_emergency_number_detection,
billing::test_postpaid_credit_check, billing::test_prepaid_funds_check,
lcr::routing_integration_tests::test_phone_validation_international, plugins::loader::(4 tests),
route_advancement::(8 tests), security::config::test_jwt_config_validation, services::routing::(2 tests),
services::test_service_health_checking, sip_rfc_compliance::(3 tests), termination_routing::test_termination_routing_no_routes.

## Known signal-processing defects -> REPAIR (remove #[ignore])
- mu-law round-trip: `src/codec_optimized.rs:272`, `src/cesopsn_ni2_integration.rs:649`.
- DTMF detector timing: `src/dtmf_processor.rs` (`test_dtmf_detector`, currently ignored per github-issues).

## SIP stack doctest -> REPAIR
- `redfire-sip-stack/src/lib.rs:44-47` doc example uses stale API (arg counts, `.method` field).

## Debt markers (for later, not this pass)
- 141 TODO/FIXME/unimplemented, 549 stub/"in a real"/mock strings, 623 `.unwrap()`, 76 panic/expect.
- `rsipstack` disabled for security vulns (using `rsip 0.4`); `cargo audit` ignores RUSTSEC-2023-0071. Keep + document.

## main.rs
- Top-level binary is a launcher stub that prints available binaries; not a real switch. Document as launcher.

## Test-calling automation (new deliverable)
- Build an automated SIP call harness that drives real INVITE/BYE flows against a running switch bin
  (e.g. `lcr_sip_server` / a b2bua bin) and asserts on outcomes. SIPp scenarios exist under `tests/sipp`
  and scripts; wire them into a repeatable `cargo`/script target.
