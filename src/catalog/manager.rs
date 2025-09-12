use std::sync::Arc;

use crate::catalog::types::{AuditSettings, AuthSettings, Grant, Role, User};
use crate::storage::StorageEngine;

pub struct CatalogManager {
    pub engine: Arc<StorageEngine>,
}

impl CatalogManager {
    pub fn new(engine: Arc<StorageEngine>) -> Self {
        Self { engine }
    }

    // ================
    // USERS
    // ================
    pub async fn get_user(
        &self,
        username: &str,
    ) -> Result<User, crate::catalog::error::CatalogError> {
        let key = format!("_sys.users:{}", username);
        let entry = self.engine.get(&key).await?;
        let user: User = serde_json::from_slice(&entry.value)?;
        Ok(user)
    }

    pub async fn set_user(&self, user: &User) -> Result<(), crate::catalog::error::CatalogError> {
        let key = format!("_sys.users:{}", user.username);
        let value = serde_json::to_vec(user)?;
        self.engine.set(&key, value, None).await?;
        Ok(())
    }

    // ================
    // ROLES
    // ================
    pub async fn get_role(
        &self,
        role_name: &str,
    ) -> Result<Role, crate::catalog::error::CatalogError> {
        let key = format!("_sys.roles:{}", role_name);
        let entry = self.engine.get(&key).await?;
        let role: Role = serde_json::from_slice(&entry.value)?;
        Ok(role)
    }

    pub async fn set_role(&self, role: &Role) -> Result<(), crate::catalog::error::CatalogError> {
        let key = format!("_sys.roles:{}", role.name);
        let value = serde_json::to_vec(role)?;
        self.engine.set(&key, value, None).await?;
        Ok(())
    }

    // ================
    // GRANTS
    // ================
    pub async fn get_grant(
        &self,
        username: &str,
    ) -> Result<Grant, crate::catalog::error::CatalogError> {
        let key = format!("_sys.grants:{}", username);
        let entry = self.engine.get(&key).await?;
        let grant: Grant = serde_json::from_slice(&entry.value)?;
        Ok(grant)
    }

    pub async fn set_grant(
        &self,
        grant: &Grant,
    ) -> Result<(), crate::catalog::error::CatalogError> {
        let key = format!("_sys.grants:{}", grant.username);
        let value = serde_json::to_vec(grant)?;
        self.engine.set(&key, value, None).await?;
        Ok(())
    }

    // ================
    // SETTINGS
    // ================
    pub async fn get_auth_settings(
        &self,
    ) -> Result<AuthSettings, crate::catalog::error::CatalogError> {
        let key = "_sys.settings:auth".to_string();
        let entry = self.engine.get(&key).await?;
        let settings: AuthSettings = serde_json::from_slice(&entry.value)?;
        Ok(settings)
    }

    pub async fn get_audit_settings(
        &self,
    ) -> Result<AuditSettings, crate::catalog::error::CatalogError> {
        let key = "_sys.settings:audit".to_string();
        let entry = self.engine.get(&key).await?;
        let settings: AuditSettings = serde_json::from_slice(&entry.value)?;
        Ok(settings)
    }

    // ================
    // PASSWORD UTILS
    // ================
    pub fn verify_password(&self, password: &str, hash: &str) -> bool {
        use scrypt::password_hash::PasswordVerifier;
        use scrypt::Scrypt;

        Scrypt
            .verify_password(password.as_bytes(), &hash.parse().unwrap())
            .is_ok()
    }

    pub fn hash_password(
        &self,
        password: &str,
    ) -> Result<String, crate::catalog::error::CatalogError> {
        crate::catalog::bootstrap::hash_password(password)
    }
}
