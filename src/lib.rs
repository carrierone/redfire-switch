// redfire-switch library modules - CLEANED UP

// Re-export codec engine and SIP stack libraries
pub use redfire_codec_engine as codec_engine;
pub use redfire_sip_stack as sip_stack;

// Core B2BUA functionality  
pub mod sipi_b2bua;
pub mod sipi_compliance_tester;

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