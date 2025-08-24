use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::lcr::LcrEngine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClusterMessage {
    CacheReload,
    TrunkUpdate { trunk_id: i32, active: bool },
    RateDeckUpdate { deck_id: i32 },
    ConfigUpdate { config_type: String },
}

pub struct ClusterSync {
    lcr_engine: Arc<LcrEngine>,
    tx: broadcast::Sender<ClusterMessage>,
    rx: broadcast::Receiver<ClusterMessage>,
}

impl ClusterSync {
    pub fn new(lcr_engine: Arc<LcrEngine>) -> Self {
        let (tx, rx) = broadcast::channel(100);

        Self { lcr_engine, tx, rx }
    }

    pub async fn broadcast_cache_reload(&self) -> Result<()> {
        self.tx.send(ClusterMessage::CacheReload)?;
        Ok(())
    }

    pub async fn broadcast_trunk_update(&self, trunk_id: i32, active: bool) -> Result<()> {
        self.tx
            .send(ClusterMessage::TrunkUpdate { trunk_id, active })?;
        Ok(())
    }

    pub async fn broadcast_rate_deck_update(&self, deck_id: i32) -> Result<()> {
        self.tx.send(ClusterMessage::RateDeckUpdate { deck_id })?;
        Ok(())
    }

    pub async fn start_listener(&mut self) -> Result<()> {
        loop {
            match self.rx.recv().await {
                Ok(msg) => {
                    self.handle_cluster_message(msg).await?;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Cluster sync lagged by {} messages", n);
                }
                Err(e) => {
                    tracing::error!("Cluster sync error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_cluster_message(&self, msg: ClusterMessage) -> Result<()> {
        match msg {
            ClusterMessage::CacheReload => {
                tracing::info!("Received cluster cache reload message");
                self.lcr_engine.reload_cache().await?;
            }
            ClusterMessage::TrunkUpdate { trunk_id, active } => {
                tracing::info!("Received trunk update: {} active={}", trunk_id, active);
                // TODO: Update specific trunk in cache
            }
            ClusterMessage::RateDeckUpdate { deck_id } => {
                tracing::info!("Received rate deck update: {}", deck_id);
                // TODO: Reload specific rate deck
            }
            ClusterMessage::ConfigUpdate { config_type } => {
                tracing::info!("Received config update: {}", config_type);
                // TODO: Reload specific configuration
            }
        }

        Ok(())
    }

    pub fn get_sender(&self) -> broadcast::Sender<ClusterMessage> {
        self.tx.clone()
    }
}
