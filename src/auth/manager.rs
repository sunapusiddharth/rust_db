use crate::auth::audit::AuditLogger;
use crate::auth::jwt::JwtManager;
use crate::auth::AuthError;
use crate::catalog::CatalogManager;
use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::Arc;

pub struct AuthManager {
    catalog: Arc<CatalogManager>,
    jwt_manager: JwtManager,
    audit_logger: AuditLogger,
}

impl AuthManager {
    pub fn new(
        catalog: Arc<CatalogManager>,
        jwt_secret: String,
        audit_log_path: String,
    ) -> Result<Self, std::io::Error> {
        let jwt_manager = JwtManager::new(jwt_secret);
        let audit_logger = AuditLogger::new(&audit_log_path)?;

        Ok(Self {
            catalog,
            jwt_manager,
            audit_logger,
        })
    }

    // ================
    // AUTHENTICATE
    // ================
    pub async fn authenticate_api_key(
        &self,
        key_id: &str,
        source_ip: IpAddr,
    ) -> Result<crate::auth::types::AuthContext, crate::auth::types::AuthError> {
        match self.catalog.api_key_validator().validate(key_id).await {
            Ok((user, direct_permissions)) => {
                // For MVP: permissions from API key override roles
                // Later: merge with role permissions

                let ctx = crate::auth::types::AuthContext {
                    user: user.clone(),
                    roles: Vec::new(), // not used in MVP for API keys
                    permissions: direct_permissions.clone(),
                    source_ip,
                    auth_method: crate::auth::types::AuthMethod::ApiKey(key_id.to_string()),
                    session_id: uuid::Uuid::new_v4().to_string(),
                };

                // Log success
                self.audit_logger
                    .log(crate::auth::audit::AuditEvent {
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        event: "login_success".to_string(),
                        user: Some(user),
                        source_ip: source_ip.to_string(),
                        auth_method: "api_key".to_string(),
                        key_id: Some(key_id.to_string()),
                        op: None,
                        key: None,
                        success: true,
                        details: None,
                    })
                    .ok(); // best effort

                Ok(ctx)
            }
            Err(e) => {
                // Log failure
                self.audit_logger
                    .log(crate::auth::audit::AuditEvent {
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        event: "login_failed".to_string(),
                        user: None,
                        source_ip: source_ip.to_string(),
                        auth_method: "api_key".to_string(),
                        key_id: Some(key_id.to_string()),
                        op: None,
                        key: None,
                        success: false,
                        details: Some(e.to_string()),
                    })
                    .ok();

                Err(e)
            }
        }
    }

    pub async fn authenticate_jwt(
        &self,
        token: &str,
        source_ip: IpAddr,
    ) -> Result<crate::auth::types::AuthContext, crate::auth::types::AuthError> {
        match self.jwt_manager.validate(token) {
            Ok(claims) => {
                // Later: verify user still exists + active
                // For now: trust the token

                let ctx = crate::auth::types::AuthContext {
                    user: claims.sub.clone(),
                    roles: Vec::new(),
                    permissions: claims.perms.clone(),
                    source_ip,
                    auth_method: crate::auth::types::AuthMethod::Jwt(token.to_string()),
                    session_id: claims.session_id,
                };

                self.audit_logger
                    .log(crate::auth::audit::AuditEvent {
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        event: "login_success".to_string(),
                        user: Some(claims.sub),
                        source_ip: source_ip.to_string(),
                        auth_method: "jwt".to_string(),
                        key_id: None,
                        op: None,
                        key: None,
                        success: true,
                        details: None,
                    })
                    .ok();

                Ok(ctx)
            }
            Err(e) => {
                self.audit_logger
                    .log(crate::auth::audit::AuditEvent {
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                        event: "login_failed".to_string(),
                        user: None,
                        source_ip: source_ip.to_string(),
                        auth_method: "jwt".to_string(),
                        key_id: None,
                        op: None,
                        key: None,
                        success: false,
                        details: Some(e.to_string()),
                    })
                    .ok();

                Err(crate::auth::types::AuthError::InvalidCredentials)
            }
        }
    }

    // ================
    // AUTHORIZE
    // ================
    pub fn authorize(
        &self,
        ctx: &crate::auth::types::AuthContext,
        op: &str,
        key: &str,
    ) -> Result<(), crate::auth::types::AuthError> {
        // Check if user has permission
        let has_permission = ctx.permissions.contains(&"*".to_string()) || // superuser
                             ctx.permissions.contains(&op.to_string());

        if has_permission {
            Ok(())
        } else {
            // Log denial
            self.audit_logger
                .log(crate::auth::audit::AuditEvent {
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    event: "permission_denied".to_string(),
                    user: Some(ctx.user.clone()),
                    source_ip: ctx.source_ip.to_string(),
                    auth_method: match &ctx.auth_method {
                        crate::auth::types::AuthMethod::ApiKey(_) => "api_key".to_string(),
                        crate::auth::types::AuthMethod::Jwt(_) => "jwt".to_string(),
                        crate::auth::types::AuthMethod::Password => "password".to_string(),
                    },
                    key_id: None,
                    op: Some(op.to_string()),
                    key: Some(key.to_string()),
                    success: false,
                    details: Some(format!("required permission: {}", op)),
                })
                .ok();

            Err(crate::auth::types::AuthError::PermissionDenied(
                op.to_string(),
                ctx.user.clone(),
            ))
        }
    }
}
