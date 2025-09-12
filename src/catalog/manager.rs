use std::sync::Arc;

use crate::catalog::types::{AuthSettings, AuditSettings, Grant, Role, User};
use crate::storage::StorageEngine;

pub struct CatalogManager {
    engine: Arc<StorageEngine>,
}

impl CatalogManager {
    pub fn new(engine: Arc<StorageEngine>) -> Self {
        Self { engine }
    }

    // ================
    // USERS
    // ================
    pub async fn get_user(&self, username: &str) -> Result<User, crate::catalog::error::CatalogError> {
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
    pub async fn get_role(&self, role_name: &str) -> Result<Role, crate::catalog::error::CatalogError> {
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
    pub async fn get_grant(&self, username: &str) -> Result<Grant, crate::catalog::error::CatalogError> {
        let key = format!("_sys.grants:{}", username);
        let entry = self.engine.get(&key).await?;
        let grant: Grant = serde_json::from_slice(&entry.value)?;
        Ok(grant)
    }

    pub async fn set_grant(&self, grant: &Grant) -> Result<(), crate::catalog::error::CatalogError> {
        let key = format!("_sys.grants:{}", grant.username);
        let value = serde_json::to_vec(grant)?;
        self.engine.set(&key, value, None).await?;
        Ok(())
    }

    // ================
    // SETTINGS
    // ================
    pub async fn get_auth_settings(&self) -> Result<AuthSettings, crate::catalog::error::CatalogError> {
        let key = "_sys.settings:auth".to_string();
        let entry = self.engine.get(&key).await?;
        let settings: AuthSettings = serde_json::from_slice(&entry.value)?;
        Ok(settings)
    }

    pub async fn get_audit_settings(&self) -> Result<AuditSettings, crate::catalog::error::CatalogError> {
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

    pub fn hash_password(&self, password: &str) -> Result<String, crate::catalog::error::CatalogError> {
        crate::catalog::bootstrap::hash_password(password)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::StorageConfig;

    #[tokio::test]
    async fn test_catalog_bootstrap_and_user_crud() {
        let config = StorageConfig::default();
        let engine = StorageEngine::new(config);
        let catalog = CatalogManager::new(engine.clone());

        // Bootstrap
        let bootstrapped = bootstrap_if_needed(&engine).await.unwrap();
        assert!(bootstrapped);

        // Get admin user
        let user = catalog.get_user("admin").await.unwrap();
        assert_eq!(user.username, "admin");
        assert!(catalog.verify_password("admin", &user.password_hash));

        // Get admin role
        let role = catalog.get_role("admin").await.unwrap();
        assert_eq!(role.name, "admin");
        assert!(role.permissions.contains(&"*".to_string()));

        // Get admin grant
        let grant = catalog.get_grant("admin").await.unwrap();
        assert_eq!(grant.username, "admin");
        assert!(grant.roles.contains(&"admin".to_string()));
    }

    #[tokio::test]
    async fn test_catalog_password_hashing() {
        let config = StorageConfig::default();
        let engine = StorageEngine::new(config);
        let catalog = CatalogManager::new(engine);

        let password = "my_secret_password";
        let hash = catalog.hash_password(password).unwrap();
        assert_ne!(hash, password);
        assert!(catalog.verify_password(password, &hash));
        assert!(!catalog.verify_password("wrong_password", &hash));
    }
}