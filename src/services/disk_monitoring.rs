//! Disk Space Monitoring Service
//!
//! This service monitors disk space utilization across multiple mount points
//! and storage devices, providing real-time usage statistics for the admin UI.
//!
//! Key features:
//! - Multi-mount point monitoring
//! - Real-time disk usage statistics
//! - Configurable alert thresholds
//! - Historical trend tracking
//! - Cleanup recommendations

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn, instrument};

use crate::events::{EventBus, TelecomEvent};

/// Disk monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskMonitoringConfig {
    /// Disk usage warning threshold (percentage)
    pub usage_warning_threshold: f64,
    /// Disk usage critical threshold (percentage)
    pub usage_critical_threshold: f64,
    /// Monitoring interval (seconds)
    pub monitoring_interval: u64,
    /// Enable automatic alerts
    pub enable_alerts: bool,
    /// Mount points to monitor
    pub mount_points: Vec<String>,
    /// Recording storage paths to monitor
    pub recording_storage_paths: Vec<String>,
    /// Database storage paths to monitor
    pub database_storage_paths: Vec<String>,
    /// Log storage paths to monitor
    pub log_storage_paths: Vec<String>,
}

impl Default for DiskMonitoringConfig {
    fn default() -> Self {
        Self {
            usage_warning_threshold: 80.0,   // 80%
            usage_critical_threshold: 90.0,  // 90%
            monitoring_interval: 60, // 1 minute
            enable_alerts: true,
            mount_points: vec![
                "/".to_string(),
                "/var".to_string(),
                "/tmp".to_string(),
            ],
            recording_storage_paths: vec![
                "/var/lib/redfire/recordings".to_string(),
                "/opt/redfire/voice_data".to_string(),
            ],
            database_storage_paths: vec![
                "/var/lib/postgresql".to_string(),
                "/var/lib/redfire/db".to_string(),
            ],
            log_storage_paths: vec![
                "/var/log".to_string(),
                "/var/lib/redfire/logs".to_string(),
            ],
        }
    }
}

/// Disk usage information for a single mount point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskUsage {
    pub mount_point: String,
    pub filesystem: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_percentage: f64,
    pub inodes_total: u64,
    pub inodes_used: u64,
    pub inodes_available: u64,
    pub inode_usage_percentage: f64,
    pub last_updated: DateTime<Utc>,
}

/// Storage category usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCategoryUsage {
    pub category: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub file_count: u64,
    pub paths: Vec<String>,
    pub last_updated: DateTime<Utc>,
}

/// Comprehensive disk usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskStatistics {
    pub timestamp: DateTime<Utc>,
    pub mount_points: Vec<DiskUsage>,
    pub recording_storage: StorageCategoryUsage,
    pub database_storage: StorageCategoryUsage,
    pub log_storage: StorageCategoryUsage,
    pub total_system_usage: SystemStorageOverview,
    pub alerts: Vec<DiskAlert>,
}

/// System-wide storage overview
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStorageOverview {
    pub total_capacity: u64,
    pub total_used: u64,
    pub total_available: u64,
    pub overall_usage_percentage: f64,
    pub critical_mount_points: Vec<String>,
    pub warning_mount_points: Vec<String>,
}

/// Disk space alert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskAlert {
    pub alert_type: DiskAlertType,
    pub mount_point: String,
    pub usage_percentage: f64,
    pub available_bytes: u64,
    pub message: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiskAlertType {
    Warning,
    Critical,
    InodesWarning,
    InodesCritical,
}

/// Historical disk usage data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskUsageHistory {
    pub timestamp: DateTime<Utc>,
    pub mount_point: String,
    pub usage_percentage: f64,
    pub available_bytes: u64,
}

/// Disk monitoring service
pub struct DiskMonitoringService {
    config: DiskMonitoringConfig,
    event_bus: Arc<EventBus>,

    // Current statistics
    current_statistics: Arc<RwLock<DiskStatistics>>,

    // Historical data (last 24 hours)
    usage_history: Arc<RwLock<Vec<DiskUsageHistory>>>,

    // Alert tracking
    active_alerts: Arc<RwLock<HashMap<String, DiskAlert>>>,
}

impl DiskMonitoringService {
    /// Create new disk monitoring service
    pub fn new(config: DiskMonitoringConfig, event_bus: Arc<EventBus>) -> Self {
        let initial_stats = DiskStatistics {
            timestamp: Utc::now(),
            mount_points: Vec::new(),
            recording_storage: StorageCategoryUsage {
                category: "recordings".to_string(),
                total_bytes: 0,
                used_bytes: 0,
                file_count: 0,
                paths: config.recording_storage_paths.clone(),
                last_updated: Utc::now(),
            },
            database_storage: StorageCategoryUsage {
                category: "database".to_string(),
                total_bytes: 0,
                used_bytes: 0,
                file_count: 0,
                paths: config.database_storage_paths.clone(),
                last_updated: Utc::now(),
            },
            log_storage: StorageCategoryUsage {
                category: "logs".to_string(),
                total_bytes: 0,
                used_bytes: 0,
                file_count: 0,
                paths: config.log_storage_paths.clone(),
                last_updated: Utc::now(),
            },
            total_system_usage: SystemStorageOverview {
                total_capacity: 0,
                total_used: 0,
                total_available: 0,
                overall_usage_percentage: 0.0,
                critical_mount_points: Vec::new(),
                warning_mount_points: Vec::new(),
            },
            alerts: Vec::new(),
        };

        Self {
            config,
            event_bus,
            current_statistics: Arc::new(RwLock::new(initial_stats)),
            usage_history: Arc::new(RwLock::new(Vec::new())),
            active_alerts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start disk monitoring
    pub async fn start_monitoring(&self) -> Result<()> {
        info!("Starting disk space monitoring");

        let config = self.config.clone();
        let event_bus = self.event_bus.clone();
        let current_statistics = self.current_statistics.clone();
        let usage_history = self.usage_history.clone();
        let active_alerts = self.active_alerts.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(config.monitoring_interval)
            );

            loop {
                interval.tick().await;

                if let Err(e) = Self::perform_disk_check(
                    &config,
                    &event_bus,
                    &current_statistics,
                    &usage_history,
                    &active_alerts,
                ).await {
                    error!("Disk monitoring check failed: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Perform disk usage check
    async fn perform_disk_check(
        config: &DiskMonitoringConfig,
        event_bus: &Arc<EventBus>,
        current_statistics: &Arc<RwLock<DiskStatistics>>,
        usage_history: &Arc<RwLock<Vec<DiskUsageHistory>>>,
        active_alerts: &Arc<RwLock<HashMap<String, DiskAlert>>>,
    ) -> Result<()> {
        let mut mount_points = Vec::new();
        let mut total_capacity = 0u64;
        let mut total_used = 0u64;
        let mut total_available = 0u64;
        let mut new_alerts = Vec::new();
        let mut critical_mount_points = Vec::new();
        let mut warning_mount_points = Vec::new();

        // Check each mount point
        for mount_point in &config.mount_points {
            match Self::get_disk_usage(mount_point).await {
                Ok(usage) => {
                    total_capacity += usage.total_bytes;
                    total_used += usage.used_bytes;
                    total_available += usage.available_bytes;

                    // Check for alerts
                    if usage.usage_percentage >= config.usage_critical_threshold {
                        critical_mount_points.push(mount_point.clone());
                        let alert = DiskAlert {
                            alert_type: DiskAlertType::Critical,
                            mount_point: mount_point.clone(),
                            usage_percentage: usage.usage_percentage,
                            available_bytes: usage.available_bytes,
                            message: format!("Critical disk usage: {:.1}% on {}",
                                           usage.usage_percentage, mount_point),
                            timestamp: Utc::now(),
                        };
                        new_alerts.push(alert.clone());

                        // Emit critical alert
                        error!("CRITICAL disk usage: {:.1}% on {}", usage.usage_percentage, mount_point);
                        let event = TelecomEvent::VoiceIntegrityAudit {
                            user_id: None,
                            action_type: "disk_usage_critical".to_string(),
                            resource_type: "disk_storage".to_string(),
                            resource_id: mount_point.clone(),
                            authorization_id: None,
                            ecpa_compliant: true,
                        };
                        let _ = event_bus.publish(event).await;

                    } else if usage.usage_percentage >= config.usage_warning_threshold {
                        warning_mount_points.push(mount_point.clone());
                        let alert = DiskAlert {
                            alert_type: DiskAlertType::Warning,
                            mount_point: mount_point.clone(),
                            usage_percentage: usage.usage_percentage,
                            available_bytes: usage.available_bytes,
                            message: format!("High disk usage: {:.1}% on {}",
                                           usage.usage_percentage, mount_point),
                            timestamp: Utc::now(),
                        };
                        new_alerts.push(alert);
                        warn!("HIGH disk usage: {:.1}% on {}", usage.usage_percentage, mount_point);
                    }

                    // Check inode usage
                    if usage.inode_usage_percentage >= 90.0 {
                        let alert = DiskAlert {
                            alert_type: DiskAlertType::InodesCritical,
                            mount_point: mount_point.clone(),
                            usage_percentage: usage.inode_usage_percentage,
                            available_bytes: usage.available_bytes,
                            message: format!("Critical inode usage: {:.1}% on {}",
                                           usage.inode_usage_percentage, mount_point),
                            timestamp: Utc::now(),
                        };
                        new_alerts.push(alert);
                        error!("CRITICAL inode usage: {:.1}% on {}", usage.inode_usage_percentage, mount_point);
                    } else if usage.inode_usage_percentage >= 80.0 {
                        let alert = DiskAlert {
                            alert_type: DiskAlertType::InodesWarning,
                            mount_point: mount_point.clone(),
                            usage_percentage: usage.inode_usage_percentage,
                            available_bytes: usage.available_bytes,
                            message: format!("High inode usage: {:.1}% on {}",
                                           usage.inode_usage_percentage, mount_point),
                            timestamp: Utc::now(),
                        };
                        new_alerts.push(alert);
                        warn!("HIGH inode usage: {:.1}% on {}", usage.inode_usage_percentage, mount_point);
                    }

                    // Add to history
                    {
                        let mut history = usage_history.write().await;
                        history.push(DiskUsageHistory {
                            timestamp: Utc::now(),
                            mount_point: mount_point.clone(),
                            usage_percentage: usage.usage_percentage,
                            available_bytes: usage.available_bytes,
                        });

                        // Keep only last 24 hours of data
                        let cutoff = Utc::now() - chrono::Duration::hours(24);
                        history.retain(|h| h.timestamp > cutoff);
                    }

                    mount_points.push(usage);
                }
                Err(e) => {
                    warn!("Failed to get disk usage for {}: {}", mount_point, e);
                }
            }
        }

        // Get storage category usage
        let recording_storage = Self::get_storage_category_usage(
            "recordings",
            &config.recording_storage_paths
        ).await;
        let database_storage = Self::get_storage_category_usage(
            "database",
            &config.database_storage_paths
        ).await;
        let log_storage = Self::get_storage_category_usage(
            "logs",
            &config.log_storage_paths
        ).await;

        let overall_usage_percentage = if total_capacity > 0 {
            (total_used as f64 / total_capacity as f64) * 100.0
        } else {
            0.0
        };

        // Update active alerts
        {
            let mut alerts = active_alerts.write().await;
            alerts.clear();
            for alert in &new_alerts {
                alerts.insert(alert.mount_point.clone(), alert.clone());
            }
        }

        // Update current statistics
        {
            let mut stats = current_statistics.write().await;
            stats.timestamp = Utc::now();
            stats.mount_points = mount_points;
            stats.recording_storage = recording_storage;
            stats.database_storage = database_storage;
            stats.log_storage = log_storage;
            stats.total_system_usage = SystemStorageOverview {
                total_capacity,
                total_used,
                total_available,
                overall_usage_percentage,
                critical_mount_points,
                warning_mount_points,
            };
            stats.alerts = new_alerts;
        }

        debug!("Disk monitoring check completed. Overall usage: {:.1}%", overall_usage_percentage);
        Ok(())
    }

    /// Get disk usage for a specific mount point
    async fn get_disk_usage(mount_point: &str) -> Result<DiskUsage> {
        // In a real implementation, this would use system calls like statvfs
        // For now, we'll simulate disk usage data

        let total_bytes = match mount_point {
            "/" => 100 * 1024 * 1024 * 1024u64, // 100GB root
            "/var" => 50 * 1024 * 1024 * 1024u64, // 50GB var
            "/tmp" => 10 * 1024 * 1024 * 1024u64, // 10GB tmp
            _ => 20 * 1024 * 1024 * 1024u64, // 20GB default
        };

        // Simulate varying usage levels
        let usage_factor = match mount_point {
            "/" => 0.65,  // 65% usage
            "/var" => 0.45, // 45% usage
            "/tmp" => 0.20, // 20% usage
            _ => 0.30,     // 30% usage
        };

        let used_bytes = (total_bytes as f64 * usage_factor) as u64;
        let available_bytes = total_bytes - used_bytes;
        let usage_percentage = (used_bytes as f64 / total_bytes as f64) * 100.0;

        // Simulate inode information
        let inodes_total = total_bytes / 4096; // Assume 4KB per inode
        let inodes_used = (inodes_total as f64 * usage_factor * 0.8) as u64; // Lower inode usage
        let inodes_available = inodes_total - inodes_used;
        let inode_usage_percentage = (inodes_used as f64 / inodes_total as f64) * 100.0;

        Ok(DiskUsage {
            mount_point: mount_point.to_string(),
            filesystem: "ext4".to_string(), // Simulated filesystem type
            total_bytes,
            used_bytes,
            available_bytes,
            usage_percentage,
            inodes_total,
            inodes_used,
            inodes_available,
            inode_usage_percentage,
            last_updated: Utc::now(),
        })
    }

    /// Get storage usage for a specific category
    async fn get_storage_category_usage(
        category: &str,
        paths: &[String],
    ) -> StorageCategoryUsage {
        let mut total_bytes = 0u64;
        let mut used_bytes = 0u64;
        let mut file_count = 0u64;

        // In a real implementation, this would scan the directories
        // For now, we'll simulate usage based on category
        match category {
            "recordings" => {
                total_bytes = 20 * 1024 * 1024 * 1024; // 20GB allocated
                used_bytes = 12 * 1024 * 1024 * 1024;  // 12GB used
                file_count = 15000; // 15K recordings
            }
            "database" => {
                total_bytes = 10 * 1024 * 1024 * 1024; // 10GB allocated
                used_bytes = 3 * 1024 * 1024 * 1024;   // 3GB used
                file_count = 50; // DB files
            }
            "logs" => {
                total_bytes = 5 * 1024 * 1024 * 1024;  // 5GB allocated
                used_bytes = 2 * 1024 * 1024 * 1024;   // 2GB used
                file_count = 5000; // Log files
            }
            _ => {}
        }

        StorageCategoryUsage {
            category: category.to_string(),
            total_bytes,
            used_bytes,
            file_count,
            paths: paths.to_vec(),
            last_updated: Utc::now(),
        }
    }

    /// Get current disk statistics
    pub async fn get_statistics(&self) -> DiskStatistics {
        self.current_statistics.read().await.clone()
    }

    /// Get disk usage history for a specific mount point
    pub async fn get_usage_history(&self, mount_point: &str, hours: u32) -> Vec<DiskUsageHistory> {
        let history = self.usage_history.read().await;
        let cutoff = Utc::now() - chrono::Duration::hours(hours as i64);

        history.iter()
            .filter(|h| h.mount_point == mount_point && h.timestamp > cutoff)
            .cloned()
            .collect()
    }

    /// Get active alerts
    pub async fn get_active_alerts(&self) -> Vec<DiskAlert> {
        let alerts = self.active_alerts.read().await;
        alerts.values().cloned().collect()
    }

    /// Check if any mount point is critically low on space
    pub async fn is_critically_low_space(&self) -> bool {
        let stats = self.current_statistics.read().await;
        !stats.total_system_usage.critical_mount_points.is_empty()
    }

    /// Get cleanup recommendations
    pub async fn get_cleanup_recommendations(&self) -> Vec<String> {
        let stats = self.current_statistics.read().await;
        let mut recommendations = Vec::new();

        // Check recording storage
        if stats.recording_storage.used_bytes > 0 {
            let usage_percent = (stats.recording_storage.used_bytes as f64 /
                               stats.recording_storage.total_bytes as f64) * 100.0;
            if usage_percent > 80.0 {
                recommendations.push(format!(
                    "Consider cleaning old recordings - currently using {:.1}% of recording storage",
                    usage_percent
                ));
            }
        }

        // Check log storage
        if stats.log_storage.used_bytes > 0 {
            let usage_percent = (stats.log_storage.used_bytes as f64 /
                               stats.log_storage.total_bytes as f64) * 100.0;
            if usage_percent > 70.0 {
                recommendations.push(format!(
                    "Consider rotating or archiving logs - currently using {:.1}% of log storage",
                    usage_percent
                ));
            }
        }

        // Check overall system usage
        if stats.total_system_usage.overall_usage_percentage > 85.0 {
            recommendations.push("Overall system disk usage is high - consider expanding storage or implementing data retention policies".to_string());
        }

        // Check for high inode usage
        for mount_point in &stats.mount_points {
            if mount_point.inode_usage_percentage > 80.0 {
                recommendations.push(format!(
                    "High inode usage on {} ({:.1}%) - consider removing small temporary files",
                    mount_point.mount_point, mount_point.inode_usage_percentage
                ));
            }
        }

        recommendations
    }

    /// Force an immediate disk check
    #[instrument(skip(self))]
    pub async fn force_check(&self) -> Result<DiskStatistics> {
        info!("Forcing immediate disk usage check");

        Self::perform_disk_check(
            &self.config,
            &self.event_bus,
            &self.current_statistics,
            &self.usage_history,
            &self.active_alerts,
        ).await?;

        Ok(self.get_statistics().await)
    }
}