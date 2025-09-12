use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,           // username
    pub exp: usize,            // expiration (Unix timestamp)
    pub perms: Vec<String>,    // permissions (cached at login)
    pub session_id: String,    // for revocation later
}

pub struct JwtManager {
    secret: String,
}

impl JwtManager {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }

    pub fn generate(&self, username: &str, permissions: Vec<String>, expires_in: u64) -> Result<String, jsonwebtoken::errors::Error> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let exp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize + expires_in as usize;

        let claims = Claims {
            sub: username.to_string(),
            exp,
            perms: permissions,
            session_id,
        };

        encode(&Header::default(), &claims, &EncodingKey::from_secret(self.secret.as_ref()))
    }

    pub fn validate(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let validation = Validation::default();
        let token_data = decode::<Claims>(token, &DecodingKey::from_secret(self.secret.as_ref()), &validation)?;
        Ok(token_data.claims)
    }
}