use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ================
// USER
// ================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub oid: u32,
    pub username: String,
    pub password_hash: String, // scrypt format
    pub created_at: DateTime<Utc>,
    pub is_superuser: bool,
    pub is_active: bool,
    pub valid_until: Option<DateTime<Utc>>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl User {
    pub fn new(oid: u32, username: String, password_hash: String) -> Self {
        Self {
            oid,
            username,
            password_hash,
            created_at: Utc::now(),
            is_superuser: false,
            is_active: true,
            valid_until: None,
            metadata: HashMap::new(),
        }
    }
}

// ================
// ROLE
// ================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub oid: u32,
    pub name: String,
    pub permissions: Vec<String>, // e.g., ["GET", "SET", "DEL", "SCAN"]
    pub inherits: Vec<String>,    // future: role inheritance
    pub created_at: DateTime<Utc>,
}

impl Role {
    pub fn new(oid: u32, name: String, permissions: Vec<String>) -> Self {
        Self {
            oid,
            name,
            permissions,
            inherits: Vec::new(),
            created_at: Utc::now(),
        }
    }
}

// ================
// GRANT (user → roles)
// ================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grant {
    pub username: String,
    pub roles: Vec<String>,
    pub granted_by: String,
    pub granted_at: DateTime<Utc>,
}

impl Grant {
    pub fn new(username: String, roles: Vec<String>, granted_by: String) -> Self {
        Self {
            username,
            roles,
            granted_by,
            granted_at: Utc::now(),
        }
    }
}

// ================
// SETTINGS
// ================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSettings {
    pub password_encryption: String, // "scrypt" or "argon2id" later
    pub min_password_length: u8,
    pub login_attempt_limit: u8,
    pub lockout_duration_sec: u32,
    pub session_timeout_sec: u32,
}

impl Default for AuthSettings {
    fn default() -> Self {
        Self {
            password_encryption: "scrypt".to_string(),
            min_password_length: 8,
            login_attempt_limit: 5,
            lockout_duration_sec: 300,
            session_timeout_sec: 3600,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSettings {
    pub log_successful_logins: bool,
    pub log_failed_logins: bool,
    pub retain_logs_days: u32,
}

impl Default for AuditSettings {
    fn default() -> Self {
        Self {
            log_successful_logins: true,
            log_failed_logins: true,
            retain_logs_days: 90,
        }
    }
}