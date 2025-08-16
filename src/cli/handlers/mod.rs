/*
 * Redfire Switch - CLI Handlers Module
 * Copyright (C) 2025 Carrier One Inc and contributors
 * 
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 * 
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

//! # CLI Handlers Module
//! 
//! This module contains all CLI command handlers for the Redfire Switch:
//! - Call control commands
//! - Simulation commands
//! - CNAM (Caller Name) commands
//! - LERG/NANPA database commands
//! - Emergency routing commands

pub mod call_control;
pub mod simulation;
pub mod cnam;
pub mod lerg_nanpa;
pub mod emergency;
pub mod trunk_kpi;

// Re-export commonly used functions
pub use call_control::*;
pub use simulation::*;
pub use cnam::*;
pub use lerg_nanpa::*;
pub use emergency::*;
pub use trunk_kpi::*;