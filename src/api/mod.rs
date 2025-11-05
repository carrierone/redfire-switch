/*
 * Redfire Switch - API Module
 * Copyright (C) 2025 Carrier One Inc and contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Sponsored by Carrier One Inc (https://www.carrierone.com)
 */

pub mod anti_fraud_endpoints;
pub mod auth;
pub mod config;
pub mod endpoints;
pub mod metrics_endpoints;
pub mod server;
pub mod simplified_server;

#[cfg(test)]
pub mod tests;
