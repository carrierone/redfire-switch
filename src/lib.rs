// redfire-switch library modules - CLEANED UP

// Re-export codec engine and SIP stack libraries
pub use redfire_codec_engine as codec_engine;
pub use redfire_sip_stack as sip_stack;

// Core configuration and services
pub mod billing;
pub mod config;
pub mod simple_b2bua;

// REST API and web interface
pub mod api;
pub mod monitor;
pub mod rest_api;

// Core routing modules
pub mod call_control;
pub mod routing;

// Database-dependent modules
#[cfg(feature = "database")]
pub mod cdr;

// Optional service modules
#[cfg(feature = "services")]
pub mod cnam;
#[cfg(feature = "services")]
pub mod rtp_proxy;
#[cfg(feature = "services")]
pub mod sms;
#[cfg(feature = "services")]
pub mod stir_shaken;
#[cfg(feature = "services")]
pub mod twilio_api;

// String parsing utilities
pub mod string_parser;

// Memory safety and concurrency utilities
pub mod memory_safety;

// High-performance optimizations for carrier-grade operations
#[cfg(feature = "database")]
pub mod performance;

// Performance optimization utilities
pub mod buffer_pool;
pub mod codec_optimized;

// Core B2BUA functionality
pub mod sipi_b2bua;
pub mod sipi_compliance_tester;
pub mod sipt_sipi;

// Codec modules
pub mod codec;

// SIP Stack and Codec Engine Integration
pub mod integrated_sip_codec;
pub mod sip_codec_integration;

// ISDN PRI Stack - Q.931 Network Layer and Q.921 Data Link
pub mod pri_timers;
pub mod q921_lapd;
pub mod q931_messages;
// pub mod isdn_cli; // Temporarily disabled due to syntax issues
pub mod isdn_stack_manager;

// TDMoE NI-2 Signaling and CESoPSN
pub mod cesopsn;
pub mod cesopsn_ni2_integration;
pub mod tdmoe_dtmf;
pub mod tdmoe_ni2_signaling;

// Media infrastructure (legacy modules, gradually being moved to codec_engine)
pub mod rtp;
pub mod rtp_proxy_impl;
pub mod sdp;

// Security framework
pub mod ddos_protection;
pub mod security;
pub mod security_monitor;
pub mod security_utils;

// Database and data management
pub mod database;

// Monitoring and observability
pub mod health_monitoring;
pub mod snmp_monitoring;

// Audit and compliance
pub mod audit_logging;

// Anti-fraud voice monitoring services are included in the main services module

// Compliance and regulatory framework
pub mod calea_sip_bridge;
pub mod compliance_framework;
pub mod etsi_li;
pub mod j_std_025;

// Enterprise features
pub mod ai_analytics_engine;
pub mod cluster_management;
pub mod enterprise_b2bua;
pub mod ml_threat_detection;
pub mod operational_dashboard;
pub mod secure_sipi_b2bua;

// Interactive CLI
pub mod cli;

// DTMF and signaling modules
pub mod dtmf_processor;
pub mod rfc2833_events;
pub mod sigtran_dtmf;
pub mod sip_info_dtmf;
pub mod stir_shaken_tdm;

// G.729 codec implementations
pub mod g729_annex_gpu;
pub mod g729_codec;
pub mod g729_external_asm;
pub mod g729_optimized;
pub mod g729_simple_test;
pub mod g729_test_standalone;

// GPU codec acceleration
pub mod gpu_codec_accel;

// Least Cost Routing (LCR) engine
pub mod lcr;

// Routing engines
pub mod origination_routing;
pub mod route_advancement;
pub mod termination_routing;

// Enhanced database connectivity patterns
pub mod database_connections;

// Class 4 B2BUA implementation
pub mod class4_b2bua;
pub mod class4_integration;

// Event-driven architecture
pub mod events;

// Microservices architecture
pub mod services;

// Plugin architecture for B2BUA
pub mod plugins;

// ANI-II (Automatic Number Identification Information Indicator) handling
pub mod ani_ii;

// RFC-compliant SIP and SIP-I implementation
pub mod sip_rfc_compliance;

// RFC-compliant ANI-II implementation
pub mod ani_ii_rfc_compliant;
