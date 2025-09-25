use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::connection::metrics;
use crate::connection::types::{CloseReason, ConnectionInfo};

use super::config::ConnectionConfig;

type ConnectionMap = DashMap<uuid::Uuid, Arc<RwLock<ConnectionInfo>>>;

#[derive(Debug, Clone)]
pub struct ConnectionManager {
    config: Arc<ConnectionConfig>,
    connections: ConnectionMap,
}

impl ConnectionManager {
    pub fn new(config: ConnectionConfig) -> Self {
        Self {
            config: Arc::new(config),
            connections: ConnectionMap::new(),
        }
    }

    pub async fn accept(
        &self,
        addr: std::net::SocketAddr,
        is_websocket: bool,
    ) -> Result<ConnectionGuard, ConnectionError> {
        if self.connections.len() >= self.config.max_connections {
            if let Some(to_evict) = self.find_connection_to_evict().await {
                self.close_connection(to_evict, CloseReason::MaxConnectionsReached)
                    .await;
            } else {
                return Err(ConnectionError::MaxConnectionsExceeded);
            }
        }

        let conn = Arc::new(RwLock::new(ConnectionInfo::new(addr, is_websocket)));
        let id = conn.read().await.id;
        self.connections.insert(id, conn.clone());

        debug!(conn_id = %id, addr = %addr, "Connection accepted");
        metrics::inc_accepted("unknown");

        Ok(ConnectionGuard {
            id,
            manager: Arc::new(self.clone()),
            conn,
        })
    }

    pub async fn authenticate(
        &self,
        conn_id: uuid::Uuid,
        user: String,
        role: String,
        priority: u8,
    ) -> Result<(), ConnectionError> {
        if let Some(conn) = self.connections.get(&conn_id) {
            let mut conn_mut = conn.write().await;
            conn_mut.set_user(user.clone(), role.clone(), priority);
            metrics::inc_accepted(&role);
            metrics::inc_active(&role);
            debug!(conn_id = %conn_id, user = %user, role = %role, "Connection authenticated");
            Ok(())
        } else {
            Err(ConnectionError::NotFound)
        }
    }

    pub async fn touch(&self, conn_id: uuid::Uuid) {
        if let Some(conn) = self.connections.get(&conn_id) {
            conn.read().await.touch();
        }
    }

    pub async fn close_connection(&self, conn_id: uuid::Uuid, reason: CloseReason) {
        if let Some((_, conn)) = self.connections.remove(&conn_id) {
            let guard = conn.read().await;
            let role = guard.role.as_deref().unwrap_or("unknown");

            metrics::dec_active(role);
            metrics::inc_evicted(match reason {
                CloseReason::IdleTimeout => "idle_timeout",
                CloseReason::MaxConnectionsReached => "max_reached",
                CloseReason::ClientClosed => "client_closed",
                CloseReason::ServerShutdown => "server_shutdown",
                CloseReason::AuthFailed => "auth_failed",
            });
            debug!(conn_id = %conn_id, reason = ?reason, "Connection closed");
        }
    }

    async fn find_connection_to_evict(&self) -> Option<uuid::Uuid> {
        match self.config.evict_policy.as_str() {
            "idle_then_priority" => self.evict_by_idle_then_priority().await,
            "fifo" => self.evict_oldest().await,
            "priority_then_idle" => self.evict_by_priority_then_idle().await,
            _ => self.evict_oldest().await,
        }
    }

    async fn evict_by_idle_then_priority(&self) -> Option<uuid::Uuid> {
        let mut candidates = Vec::new();

        for entry in self.connections.iter() {
            let guard = entry.value().read().await;
            candidates.push((guard.id, guard.idle_time(), guard.priority));
        }

        candidates.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.2.cmp(&b.2)));

        candidates.first().map(|(id, _, _)| *id)
    }

    async fn evict_oldest(&self) -> Option<uuid::Uuid> {
        let mut oldest: Option<(uuid::Uuid, std::time::Instant)> = None;

        for entry in self.connections.iter() {
            let conn = entry.value().read().await;
            match &oldest {
                Some((_, time)) if conn.connected_at < *time => {
                    oldest = Some((conn.id, conn.connected_at));
                }
                None => {
                    oldest = Some((conn.id, conn.connected_at));
                }
                _ => {}
            }
        }

        oldest.map(|(id, _)| id)
    }

    async fn evict_by_priority_then_idle(&self) -> Option<uuid::Uuid> {
        let mut candidates = Vec::new();

        for entry in self.connections.iter() {
            let guard = entry.value().read().await;
            candidates.push((guard.id, guard.idle_time(), guard.priority));
        }

        candidates.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| b.2.cmp(&a.2)));

        candidates.first().map(|(id, _, _)| *id)
    }
}

#[derive(Debug)]
pub struct ConnectionGuard {
    id: uuid::Uuid,
    manager: Arc<ConnectionManager>,
    conn: Arc<RwLock<ConnectionInfo>>,
}

impl ConnectionGuard {
    pub fn id(&self) -> uuid::Uuid {
        self.id
    }

    pub async fn touch(&self) {
        self.conn.read().await.touch();
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        let manager = self.manager.clone();
        let id = self.id;
        tokio::spawn(async move {
            manager
                .close_connection(id, CloseReason::ClientClosed)
                .await;
        });
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    #[error("Max connections exceeded")]
    MaxConnectionsExceeded,
    #[error("Connection not found")]
    NotFound,
}
