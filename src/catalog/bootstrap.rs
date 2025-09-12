use crate::catalog::types::{AuditSettings, AuthSettings, Grant, Role, User};
use crate::storage::types::KvEntry;
use crate::storage::StorageEngine;
use chrono::Utc;

pub async fn bootstrap_if_needed(
    engine: &StorageEngine,
) -> Result<bool, crate::catalog::error::CatalogError> {
    // Check if already bootstrapped
    if engine.exists("_sys.settings:auth").await {
        return Ok(false); // already bootstrapped
    }

    tracing::info!("Bootstrapping system catalog...");

    // Create default roles
    let roles = [
        Role::new(1, "admin".to_string(), vec!["*".to_string()]), // "*" = all permissions
        Role::new(
            2,
            "reader".to_string(),
            vec!["GET".to_string(), "SCAN".to_string(), "EXISTS".to_string()],
        ),
        Role::new(
            3,
            "writer".to_string(),
            vec![
                "SET".to_string(),
                "DEL".to_string(),
                "INCR".to_string(),
                "APPEND".to_string(),
            ],
        ),
    ];

    for role in &roles {
        let key = format!("_sys.roles:{}", role.name);
        let value = serde_json::to_vec(role)?;
        let entry = KvEntry::new(value, None);
        engine.set(&key, entry.value, None).await?;
    }

    // Create default admin user (password: "admin" — CHANGE IN PRODUCTION)
    let admin_password_hash = hash_password("admin")?;
    let admin_user = User::new(1, "admin".to_string(), admin_password_hash);
    let user_key = "_sys.users:admin".to_string();
    let user_value = serde_json::to_vec(&admin_user)?;
    let user_entry = KvEntry::new(user_value, None);
    engine.set(&user_key, user_entry.value, None).await?;

    // Grant admin user → admin role
    let grant = Grant::new(
        "admin".to_string(),
        vec!["admin".to_string()],
        "system".to_string(),
    );
    let grant_key = "_sys.grants:admin".to_string();
    let grant_value = serde_json::to_vec(&grant)?;
    let grant_entry = KvEntry::new(grant_value, None);
    engine.set(&grant_key, grant_entry.value, None).await?;

    // Create settings
    let auth_settings = AuthSettings::default();
    let auth_key = "_sys.settings:auth".to_string();
    let auth_value = serde_json::to_vec(&auth_settings)?;
    let auth_entry = KvEntry::new(auth_value, None);
    engine.set(&auth_key, auth_entry.value, None).await?;

    let audit_settings = AuditSettings::default();
    let audit_key = "_sys.settings:audit".to_string();
    let audit_value = serde_json::to_vec(&audit_settings)?;
    let audit_entry = KvEntry::new(audit_value, None);
    engine.set(&audit_key, audit_entry.value, None).await?;

    tracing::info!("System catalog bootstrapped with default admin user (password: 'admin')");

    Ok(true)
}

pub fn hash_password(password: &str) -> Result<String, crate::catalog::error::CatalogError> {
    use scrypt::password_hash::PasswordHasher;
    use scrypt::{password_hash::SaltString, Scrypt};

    let salt = SaltString::generate(&mut rand::thread_rng());
    let hash = Scrypt
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| crate::catalog::error::CatalogError::Password(e.to_string()))?;

    Ok(hash.to_string())
}
