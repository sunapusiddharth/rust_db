use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    IdleTimeout,
    MaxConnectionsReached,
    ClientClosed,
    ServerShutdown,
    AuthFailed,
}

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub id: Uuid,
    pub addr: SocketAddr,
    pub user: Option<String>,
    pub role: Option<String>,
    pub priority: u8, // 0 = lowest, 255 = highest (admin)
    pub connected_at: Instant,
    pub last_active: Arc<AtomicU64>, // Unix timestamp in nanos
    pub is_websocket: bool,
}

impl ConnectionInfo {
    pub fn new(addr: SocketAddr, is_websocket: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            addr,
            user: None,
            role: None,
            priority: 0, // default lowest
            connected_at: Instant::now(),
            last_active: Arc::new(AtomicU64::new(0)),
            is_websocket,
        }
    }

    pub fn set_user(&mut self, user: String, role: String, priority: u8) {
        self.user = Some(user);
        self.role = Some(role);
        self.priority = priority;
    }

    pub fn touch(&self) {
        self.last_active
            .store(Instant::now().elapsed().as_nanos(), Ordering::Relaxed);
    }

    pub fn idle_time(&self) -> Duration {
        let now_nanos = Instant::now().elapsed().as_nanos();
        let last_nanos = self.last_active.load(Ordering::Relaxed);
        if last_nanos == 0 {
            return Duration::from_secs(0);
        }
        Duration::from_nanos((now_nanos - last_nanos) as u64)
    }
}