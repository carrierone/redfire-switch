/*
 * Redfire Switch - Unified SIP Module  
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

//! # Unified SIP System
//! 
//! This module contains all SIP-related functionality:
//! - SIP message parsing and validation
//! - SIP state machine and transaction handling
//! - SIP authentication mechanisms
//! - SIP transport layer (UDP/TCP/TLS)
//! - Core SIP processing engine
//! - SIP debugging and diagnostics
//! - SIP interoperability with different stacks
//! - RFC compliance checking

pub mod parser;
pub mod state;
pub mod authentication;
pub mod transport;
pub mod core;
pub mod debug_cli;
pub mod interop;
pub mod compliance;

// Re-export commonly used types and functions
pub use parser::*;
pub use state::*;
pub use authentication::*;
pub use transport::*;
pub use core::*;
pub use debug_cli::*;
pub use interop::*;
pub use compliance::*;