//! Dashboard management system
//!
//! This module provides real-time dashboard capabilities for monitoring
//! system status and metrics visualization.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Dashboard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    /// Enable dashboard
    pub enabled: bool,
    /// Dashboard update interval (seconds)
    pub update_interval_seconds: u64,
    /// Dashboard port
    pub port: u16,
    /// Enable real-time updates via WebSocket
    pub enable_websocket: bool,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            update_interval_seconds: 5,
            port: 8081,
            enable_websocket: true,
        }
    }
}

/// Dashboard widget types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WidgetType {
    /// Metric gauge (single value)
    Gauge,
    /// Time series graph
    Graph,
    /// Status indicator
    Status,
    /// Table of values
    Table,
    /// Heatmap
    Heatmap,
    /// Counter
    Counter,
}

/// Dashboard widget
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardWidget {
    /// Widget ID
    pub id: String,
    /// Widget title
    pub title: String,
    /// Widget type
    pub widget_type: WidgetType,
    /// Metric path to display
    pub metric_path: String,
    /// Widget position (grid row)
    pub row: u32,
    /// Widget position (grid column)
    pub col: u32,
    /// Widget width
    pub width: u32,
    /// Widget height
    pub height: u32,
}

/// Dashboard layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardLayout {
    /// Layout ID
    pub id: String,
    /// Layout name
    pub name: String,
    /// Widgets in this layout
    pub widgets: Vec<DashboardWidget>,
}

/// Dashboard manager
pub struct DashboardManager {
    /// Configuration
    config: DashboardConfig,
    /// Available layouts
    layouts: Arc<RwLock<Vec<DashboardLayout>>>,
    /// Active layout ID
    active_layout: Arc<RwLock<Option<String>>>,
    /// Dashboard running state
    running: Arc<RwLock<bool>>,
}

impl DashboardManager {
    /// Create new dashboard manager
    pub fn new(enabled: bool) -> Result<Self> {
        let mut config = DashboardConfig::default();
        config.enabled = enabled;

        let default_layouts = Self::create_default_layouts();

        Ok(Self {
            config,
            layouts: Arc::new(RwLock::new(default_layouts)),
            active_layout: Arc::new(RwLock::new(Some("default".to_string()))),
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Create default dashboard layouts
    fn create_default_layouts() -> Vec<DashboardLayout> {
        vec![
            DashboardLayout {
                id: "default".to_string(),
                name: "System Overview".to_string(),
                widgets: vec![
                    DashboardWidget {
                        id: "cpu_gauge".to_string(),
                        title: "CPU Usage".to_string(),
                        widget_type: WidgetType::Gauge,
                        metric_path: "system.cpu_usage_percent".to_string(),
                        row: 0,
                        col: 0,
                        width: 1,
                        height: 1,
                    },
                    DashboardWidget {
                        id: "memory_gauge".to_string(),
                        title: "Memory Usage".to_string(),
                        widget_type: WidgetType::Gauge,
                        metric_path: "system.memory_usage_mb".to_string(),
                        row: 0,
                        col: 1,
                        width: 1,
                        height: 1,
                    },
                    DashboardWidget {
                        id: "active_calls".to_string(),
                        title: "Active Calls".to_string(),
                        widget_type: WidgetType::Counter,
                        metric_path: "business.active_calls".to_string(),
                        row: 0,
                        col: 2,
                        width: 1,
                        height: 1,
                    },
                    DashboardWidget {
                        id: "call_success_rate".to_string(),
                        title: "Call Success Rate".to_string(),
                        widget_type: WidgetType::Graph,
                        metric_path: "business.call_success_rate".to_string(),
                        row: 1,
                        col: 0,
                        width: 2,
                        height: 2,
                    },
                    DashboardWidget {
                        id: "sip_messages".to_string(),
                        title: "SIP Messages/sec".to_string(),
                        widget_type: WidgetType::Graph,
                        metric_path: "sip.messages_per_second".to_string(),
                        row: 1,
                        col: 2,
                        width: 2,
                        height: 2,
                    },
                ],
            },
            DashboardLayout {
                id: "sip".to_string(),
                name: "SIP Processing".to_string(),
                widgets: vec![
                    DashboardWidget {
                        id: "sip_total_messages".to_string(),
                        title: "Total SIP Messages".to_string(),
                        widget_type: WidgetType::Counter,
                        metric_path: "sip.total_messages_processed".to_string(),
                        row: 0,
                        col: 0,
                        width: 1,
                        height: 1,
                    },
                    DashboardWidget {
                        id: "sip_active_transactions".to_string(),
                        title: "Active Transactions".to_string(),
                        widget_type: WidgetType::Gauge,
                        metric_path: "sip.active_transactions".to_string(),
                        row: 0,
                        col: 1,
                        width: 1,
                        height: 1,
                    },
                    DashboardWidget {
                        id: "sip_latency".to_string(),
                        title: "Processing Latency".to_string(),
                        widget_type: WidgetType::Graph,
                        metric_path: "sip.avg_processing_latency_ms".to_string(),
                        row: 1,
                        col: 0,
                        width: 2,
                        height: 2,
                    },
                ],
            },
        ]
    }

    /// Start dashboard
    pub async fn start(&self) -> Result<()> {
        if !self.config.enabled {
            debug!("Dashboard disabled, skipping start");
            return Ok(());
        }

        info!("Starting dashboard manager...");

        *self.running.write().await = true;

        // In a full implementation, this would start:
        // - HTTP server for dashboard UI
        // - WebSocket server for real-time updates
        // - Dashboard update loop

        info!("Dashboard manager started on port {}", self.config.port);
        Ok(())
    }

    /// Stop dashboard
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping dashboard manager...");

        *self.running.write().await = false;

        info!("Dashboard manager stopped");
        Ok(())
    }

    /// Get available layouts
    pub async fn get_layouts(&self) -> Vec<DashboardLayout> {
        let layouts = self.layouts.read().await;
        layouts.clone()
    }

    /// Get active layout
    pub async fn get_active_layout(&self) -> Option<DashboardLayout> {
        let active_id = self.active_layout.read().await;
        let layouts = self.layouts.read().await;

        active_id.as_ref().and_then(|id| {
            layouts.iter().find(|l| l.id == *id).cloned()
        })
    }

    /// Set active layout
    pub async fn set_active_layout(&self, layout_id: String) -> Result<()> {
        let layouts = self.layouts.read().await;

        if layouts.iter().any(|l| l.id == layout_id) {
            *self.active_layout.write().await = Some(layout_id);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Layout not found: {}", layout_id))
        }
    }

    /// Add custom layout
    pub async fn add_layout(&self, layout: DashboardLayout) -> Result<()> {
        let mut layouts = self.layouts.write().await;
        layouts.push(layout);
        Ok(())
    }

    /// Check if dashboard is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dashboard_manager_creation() {
        let manager = DashboardManager::new(true).unwrap();

        let layouts = manager.get_layouts().await;
        assert!(layouts.len() > 0);

        assert!(!manager.is_running().await);
    }

    #[tokio::test]
    async fn test_dashboard_start_stop() {
        let manager = DashboardManager::new(true).unwrap();

        manager.start().await.unwrap();
        assert!(manager.is_running().await);

        manager.stop().await.unwrap();
        assert!(!manager.is_running().await);
    }
}
