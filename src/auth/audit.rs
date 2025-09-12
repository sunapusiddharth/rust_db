use std::fs::OpenOptions;
use std::io::Write;
use std::time::SystemTime;

use serde::Serialize;

#[derive(Serialize)]
pub struct AuditEvent {
    pub timestamp: u64,
    pub event: String,           // "login_success", "login_failed", "permission_denied"
    pub user: Option<String>,
    pub source_ip: String,
    pub auth_method: String,     // "api_key", "jwt", "password"
    pub key_id: Option<String>,  // if API key
    pub op: Option<String>,      // if permission denied
    pub key: Option<String>,     // if permission denied
    pub success: bool,
    pub details: Option<String>,
}

pub struct AuditLogger {
    file: std::fs::File,
}

impl AuditLogger {
    pub fn new(log_path: &str) -> Result<Self, std::io::Error> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;

        Ok(Self { file })
    }

    pub fn log(&mut self, event: AuditEvent) -> Result<(), std::io::Error> {
        let line = serde_json::to_string(&event)?;
        writeln!(self.file, "{}", line)?;
        self.file.flush()?;
        Ok(())
    }
}