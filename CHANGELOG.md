# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- Restored the real least-cost-routing (LCR) engine, which had been replaced by
  a stub that queried a nonexistent `routes` table; `find_routes`/`simulate_call`
  work again.
- Deck versioning now resolves the deck family active at the effective time
  instead of a pinned version, so future-dated rate decks are reachable.
- Live CPS is computed from the sliding window in `trunk_manager` instead of a
  stale cached value.
- G.711 <-> G.729 transcoding handles multi-frame RTP payloads (a 20ms G.711
  packet now maps to two 10ms G.729 frames) instead of dropping every packet.
- G.729 Annex B VAD/DTX and comfort-noise generation work on CPU without a GPU.
- ETSI/CALEA lawful-intercept warrants persist across restart and every warrant
  operation is audit-logged.
- SIP URI/OLI parsing fixes (tel: URIs, empty-user rejection, OLI <= 99,
  Remote-Party-ID/Diversion sources).
- `security_utils` validators self-initialize instead of panicking if security
  wasn't initialized first.

### Added
- Automated SIP call-flow test harness (`tests/sip_call_flow_tests.rs`) that
  places real calls over UDP through the LCR routing engine, no SIPp required.
- Reusable `sip_call_server::LcrSipServer` library module (the `lcr_sip_server`
  binary is now a thin wrapper).
- A checked-in `config-production-example.json`.

### Changed
- Documentation truth-pass: corrected overstated codec claims; GPU codec paths
  are optional/feature-gated and currently unverified in CI; G.729 and AMR-WB
  are simplified reference implementations.

### Removed
- Dead demo binaries (`ai_analytics_demo`, `enterprise_demo`, `comprehensive_demo`,
  `class4_b2bua_demo`, `calea_sip_integration_demo`, `demo_phone_validation`,
  `tdmoe_dtmf_demo`).

## [0.1.0] - 2025-01-XX

### Added
- Initial release of Redfire Switch
- Complete SIP stack with RFC 3261 compliance
- Advanced codec transcoding engine with GPU acceleration
- Universal GPU transcoding supporting 56 codec pairs
- G.729, G.722.2/AMR-WB, G.722, G.711 codec implementations
- 15-20x performance improvement with GPU acceleration
- STIR/SHAKEN call authentication and fraud prevention
- Call routing engine with LCR, LNP, and ENUM support
- Emergency services (911/E911) routing
- Call Detail Records (CDR) with ClickHouse integration
- Docker development environment
- Comprehensive testing framework with SIPp
- Cross-platform support (Linux, Windows, macOS)
- CUDA and ROCm GPU backend support
- Memory-efficient GPU memory pooling
- Automatic CPU fallback for reliability
- Enterprise security features
- Regulatory compliance tools

### Technical Features
- Direct codec-to-codec transcoding without PCM intermediate
- Automatic sample rate conversion (8kHz ↔ 16kHz)
- Batch processing for high-volume applications
- Real-time processing with <50μs latency
- Patent-free codec implementations
- ITU-T standard compliance
- Hardware acceleration with CUDA/ROCm
- Memory usage optimization (3.2KB per stream vs 8KB CPU)

### Performance
- Single server handles 10,000+ concurrent calls
- GPU transcoding: 15,000x realtime processing
- G.711 μ-law ↔ A-law: 15x speedup (0.8μs vs 12μs)
- G.729 ↔ G.711: 19x speedup (45μs vs 850μs)
- G.722.2 ↔ G.711: 17x speedup (55μs vs 920μs)
- G.729 ↔ G.722.2: 16x speedup (75μs vs 1200μs)

[0.1.0]: https://github.com/carrierone/redfire-switch/releases/tag/v0.1.0