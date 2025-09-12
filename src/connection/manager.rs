use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::connection::metrics;
use crate::connection::types::{CloseReason, ConnectionInfo};

use super::config::ConnectionConfig;

type ConnectionMap = DashMap<uuid::Uuid, Arc<ConnectionInfo>>;

pub struct ConnectionManager {
    config: Arc<ConnectionConfig>,
    connections: ConnectionMap,
    // We'll add metrics, eviction timers, etc. later
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
        // Check global limit
        if self.connections.len() >= self.config.max_connections {
            if let Some(to_evict) = self.find_connection_to_evict().await {
                self.close_connection(to_evict, CloseReason::MaxConnectionsReached)
                    .await;
            } else {
                return Err(ConnectionError::MaxConnectionsExceeded);
            }
        }

        let conn = Arc::new(ConnectionInfo::new(addr, is_websocket));
        let id = conn.id;

        self.connections.insert(id, conn.clone());

        debug!(conn_id = %id, addr = %addr, "Connection accepted");
        metrics::inc_accepted("unknown"); // will update after auth

        Ok(ConnectionGuard {
            id,
            manager: Arc::new(self.clone()),
            conn: conn.clone(),
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
            let mut conn_mut = (*conn).clone();
            conn_mut.set_user(user.clone(), role.clone(), priority);
            self.connections.insert(conn_id, conn_mut);

            // Update metrics label from "unknown" to actual role
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
            conn.touch();
        }
    }

    pub async fn close_connection(&self, conn_id: uuid::Uuid, reason: CloseReason) {
        if let Some(conn) = self.connections.remove(&conn_id) {
            let role = conn.role.as_deref().unwrap_or("unknown");
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
            _ => self.evict_oldest().await, // default
        }
    }

    async fn evict_by_idle_then_priority(&self) -> Option<uuid::Uuid> {
        let mut candidates: Vec<_> = self
            .connections
            .iter()
            .map(|r| (r.id, r.idle_time(), r.priority))
            .collect();

        // Sort: most idle first, then lowest priority
        candidates.sort_by(|a, b| {
            b.1.cmp(&a.1) // reverse idle time (most idle first)
                .then_with(|| a.2.cmp(&b.2)) // then lowest priority first
        });

        candidates.first().map(|(id, _, _)| *id)
    }

    async fn evict_oldest(&self) -> Option<uuid::Uuid> {
        self.connections
            .iter()
            .min_by_key(|r| r.connected_at)
            .map(|r| r.id)
    }

    async fn evict_by_priority_then_idle(&self) -> Option<uuid::Uuid> {
        let mut candidates: Vec<_> = self
            .connections
            .iter()
            .map(|r| (r.id, r.priority, r.idle_time()))
            .collect();

        // Sort: lowest priority first, then most idle
        candidates.sort_by(|a, b| {
            a.1.cmp(&b.1) // lowest priority first
                .then_with(|| b.2.cmp(&a.2)) // then most idle
        });

        candidates.first().map(|(id, _, _)| *id)
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionGuard {
    id: uuid::Uuid,
    manager: Arc<ConnectionManager>,
    conn: Arc<ConnectionInfo>,
}

impl ConnectionGuard {
    pub fn id(&self) -> uuid::Uuid {
        self.id
    }

    pub fn touch(&self) {
        self.conn.touch();
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        // Spawn a task to avoid blocking in Drop
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    #[tokio::test]
    async fn test_connection_eviction_idle_then_priority() {
        let config = ConnectionConfig {
            max_connections: 2,
            idle_timeout_sec: 300,
            evict_policy: "idle_then_priority".to_string(),
            per_role: std::collections::HashMap::new(),
        };

        let manager = ConnectionManager::new(config);

        // Add low-priority connection (reader)
        let guard1 = manager
            .accept("127.0.0.1:8081".parse().unwrap(), false)
            .await
            .unwrap();
        manager
            .authenticate(guard1.id(), "reader".to_string(), "reader".to_string(), 50)
            .await
            .unwrap();

        // Add high-priority connection (admin)
        let guard2 = manager
            .accept("127.0.0.1:8082".parse().unwrap(), false)
            .await
            .unwrap();
        manager
            .authenticate(guard2.id(), "admin".to_string(), "admin".to_string(), 200)
            .await
            .unwrap();

        // Try to add third connection
        let result = manager
            .accept("127.0.0.1:8083".parse().unwrap(), false)
            .await;

        // Should evict the reader (lower priority)
        assert!(result.is_ok());

        // Check that reader is evicted
        assert!(manager.connections.get(&guard1.id()).is_none());
        assert!(manager.connections.get(&guard2.id()).is_some());
    }

    #[tokio::test]
    async fn test_connection_eviction_oldest() {
        let mut config = ConnectionConfig {
            max_connections: 2,
            idle_timeout_sec: 300,
            evict_policy: "fifo".to_string(),
            per_role: std::collections::HashMap::new(),
        };

        let manager = ConnectionManager::new(config);

        // Add first connection
        let guard1 = manager
            .accept("127.0.0.1:8081".parse().unwrap(), false)
            .await
            .unwrap();
        manager
            .authenticate(guard1.id(), "user1".to_string(), "reader".to_string(), 100)
            .await
            .unwrap();

        // Add second connection
        let guard2 = manager
            .accept("127.0.0.1:8082".parse().unwrap(), false)
            .await
            .unwrap();
        manager
            .authenticate(guard2.id(), "user2".to_string(), "reader".to_string(), 100)
            .await
            .unwrap();

        // Try to add third connection
        let _guard3 = manager
            .accept("127.0.0.1:8083".parse().unwrap(), false)
            .await
            .unwrap();

        // Should evict oldest (user1)
        assert!(manager.connections.get(&guard1.id()).is_none());
        assert!(manager.connections.get(&guard2.id()).is_some());
    }
}
