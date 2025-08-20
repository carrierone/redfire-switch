// redfire-switch library modules - CLEANED UP

// Re-export codec engine and SIP stack libraries
pub use redfire_codec_engine as codec_engine;
pub use redfire_sip_stack as sip_stack;

// String parsing utilities
pub mod string_parser;

// Memory safety and concurrency utilities
pub mod memory_safety;

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
pub mod sip_codec_integration;
pub mod integrated_sip_codec;

// ISDN PRI Stack - Q.931 Network Layer and Q.921 Data Link
pub mod q931_messages;
pub mod q921_lapd;
pub mod pri_timers;
// pub mod isdn_cli; // Temporarily disabled due to syntax issues
pub mod isdn_stack_manager;

// TDMoE NI-2 Signaling and CESoPSN
pub mod tdmoe_ni2_signaling;
pub mod cesopsn;
pub mod cesopsn_ni2_integration;

// Media infrastructure (legacy modules, gradually being moved to codec_engine)
pub mod rtp;
pub mod rtp_proxy_impl;
pub mod sdp;

// Security framework
pub mod security_utils;
pub mod security_monitor;

// Enterprise features
pub mod operational_dashboard;
pub mod cluster_management;
pub mod ml_threat_detection;
pub mod secure_sipi_b2bua;
pub mod enterprise_b2bua;
pub mod ai_analytics_engine;

// DTMF processing modules
pub mod dtmf_processor;
pub mod dtmf_freeswitch_integration;
pub mod rfc2833_events;
pub mod sip_info_dtmf;
pub mod sigtran_dtmf;
pub mod stir_shaken_tdm;
pub mod tdmoe_dtmf;

// G.729 codec implementations
pub mod g729_codec;
// pub mod g729_asm; // Removed - replaced by g729_external_asm with external assembler
pub mod g729_optimized;
pub mod g729_external_asm;
pub mod g729_simple_test;
pub mod g729_test_standalone;