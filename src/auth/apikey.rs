use crate::catalog::CatalogManager;
use crate::storage::StorageEngine;

pub struct ApiKeyValidator {
    catalog: CatalogManager,
}

impl ApiKeyValidator {
    pub fn new(catalog: CatalogManager) -> Self {
        Self { catalog }
    }

    pub async fn validate(&self, key_id: &str) -> Result<(String, Vec<String>), crate::auth::types::AuthError> {
        // In MVP: key_id is stored as `_sys.api_keys:<key_id>`
        // Value is JSON: { "owner_user": "...", "permissions": [...] }
        let key = format!("_sys.api_keys:{}", key_id);
        let entry = self.catalog.engine.get(&key).await.map_err(|_| crate::auth::types::AuthError::InvalidCredentials)?;

        let api_key: ApiKeyEntry = serde_json::from_slice(&entry.value)
            .map_err(|_| crate::auth::types::AuthError::InvalidCredentials)?;

        if api_key.revoked {
            return Err(crate::auth::types::AuthError::InvalidCredentials);
        }

        // Check expiry
        if let Some(expires_at) = api_key.expires_at {
            let now = chrono::Utc::now();
            if now > expires_at {
                return Err(crate::auth::types::AuthError::InvalidCredentials);
            }
        }

        // Return (user, permissions)
        Ok((api_key.owner_user, api_key.permissions))
    }
}

#[derive(Debug, serde::Deserialize)]
struct ApiKeyEntry {
    owner_user: String,
    permissions: Vec<String>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    revoked: bool,
}