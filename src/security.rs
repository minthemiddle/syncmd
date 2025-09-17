#![allow(dead_code)]

use serde::{Serialize, Deserialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token: String,
    pub client_id: String,
    pub client_name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub last_used: chrono::DateTime<chrono::Utc>,
    pub is_revoked: bool,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub client_id: String,
    pub client_name: String,
    pub ip_address: String,
    pub user_agent: String,
    pub login_time: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub is_active: bool,
}

pub struct AuthManager {
    tokens: std::collections::HashMap<String, AuthToken>, // token -> AuthToken
    sessions: std::collections::HashMap<String, SessionInfo>, // client_id -> SessionInfo
    token_lifetime: Duration,
    session_timeout: Duration,
    max_tokens_per_client: usize,
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            tokens: std::collections::HashMap::new(),
            sessions: std::collections::HashMap::new(),
            token_lifetime: Duration::from_secs(24 * 60 * 60), // 24 hours
            session_timeout: Duration::from_secs(30 * 60), // 30 minutes
            max_tokens_per_client: 5,
        }
    }

    pub fn with_config(token_lifetime: Duration, session_timeout: Duration, max_tokens_per_client: usize) -> Self {
        Self {
            tokens: std::collections::HashMap::new(),
            sessions: std::collections::HashMap::new(),
            token_lifetime,
            session_timeout,
            max_tokens_per_client,
        }
    }

    pub fn generate_token(&mut self, client_id: String, client_name: String) -> Result<String, SyncError> {
        use uuid::Uuid;
        
        // Check if client has too many tokens
        let tokens_to_revoke: Vec<String> = self.tokens.values()
            .filter(|t| t.client_id == client_id && !t.is_revoked)
            .map(|t| t.token.clone())
            .collect();
        
        if tokens_to_revoke.len() >= self.max_tokens_per_client {
            // Revoke oldest token
            if let Some(oldest_token) = tokens_to_revoke.first() {
                self.revoke_token(oldest_token);
            }
        }
        
        let token = format!("syncmd_{}", Uuid::new_v4().to_string());
        let now = chrono::Utc::now();
        
        let auth_token = AuthToken {
            token: token.clone(),
            client_id: client_id.clone(),
            client_name,
            created_at: now,
            expires_at: now + chrono::Duration::from_std(self.token_lifetime).unwrap(),
            last_used: now,
            is_revoked: false,
            permissions: vec!["read".to_string(), "write".to_string(), "sync".to_string()],
        };
        
        self.tokens.insert(token.clone(), auth_token);
        Ok(token)
    }

    pub fn validate_token(&mut self, token: &str) -> Option<&AuthToken> {
        let now = chrono::Utc::now();
        
        // Clean up expired tokens first
        self.cleanup_expired_tokens();
        
        if let Some(auth_token) = self.tokens.get_mut(token) {
            if auth_token.is_revoked {
                return None;
            }
            
            if now > auth_token.expires_at {
                auth_token.is_revoked = true;
                return None;
            }
            
            // Update last used time
            auth_token.last_used = now;
            
            // Update session activity
            if let Some(session) = self.sessions.get_mut(&auth_token.client_id) {
                session.last_activity = now;
            }
            
            Some(auth_token)
        } else {
            None
        }
    }

    pub fn revoke_token(&mut self, token: &str) -> bool {
        if let Some(auth_token) = self.tokens.get_mut(token) {
            auth_token.is_revoked = true;
            true
        } else {
            false
        }
    }

    pub fn revoke_all_tokens_for_client(&mut self, client_id: &str) -> usize {
        let mut revoked_count = 0;
        for auth_token in self.tokens.values_mut() {
            if auth_token.client_id == client_id && !auth_token.is_revoked {
                auth_token.is_revoked = true;
                revoked_count += 1;
            }
        }
        revoked_count
    }

    pub fn list_active_tokens(&self) -> Vec<&AuthToken> {
        let now = chrono::Utc::now();
        self.tokens.values()
            .filter(|t| !t.is_revoked && t.expires_at > now)
            .collect()
    }

    pub fn list_tokens(&self) -> Vec<&AuthToken> {
        self.tokens.values().collect()
    }

    pub fn cleanup_expired_tokens(&mut self) -> usize {
        let now = chrono::Utc::now();
        let initial_count = self.tokens.len();
        
        self.tokens.retain(|_, token| {
            !token.is_revoked && token.expires_at > now
        });
        
        initial_count - self.tokens.len()
    }

    pub fn create_session(&mut self, client_id: String, client_name: String, ip_address: String, user_agent: String) -> String {
        let now = chrono::Utc::now();
        let session = SessionInfo {
            client_id: client_id.clone(),
            client_name,
            ip_address,
            user_agent,
            login_time: now,
            last_activity: now,
            is_active: true,
        };
        
        self.sessions.insert(client_id.clone(), session);
        client_id
    }

    pub fn update_session_activity(&mut self, client_id: &str) -> bool {
        if let Some(session) = self.sessions.get_mut(client_id) {
            session.last_activity = chrono::Utc::now();
            session.is_active = true;
            true
        } else {
            false
        }
    }

    pub fn end_session(&mut self, client_id: &str) -> bool {
        if let Some(session) = self.sessions.get_mut(client_id) {
            session.is_active = false;
            true
        } else {
            false
        }
    }

    pub fn cleanup_inactive_sessions(&mut self) -> usize {
        let now = chrono::Utc::now();
        let initial_count = self.sessions.len();
        
        self.sessions.retain(|_, session| {
            session.is_active && 
            (now - session.last_activity).num_seconds() < self.session_timeout.as_secs() as i64
        });
        
        initial_count - self.sessions.len()
    }

    pub fn get_active_sessions(&self) -> Vec<&SessionInfo> {
        self.sessions.values()
            .filter(|s| s.is_active)
            .collect()
    }

    pub fn get_client_sessions(&self, client_id: &str) -> Vec<&SessionInfo> {
        self.sessions.values()
            .filter(|s| s.client_id == client_id)
            .collect()
    }

    pub fn validate_permissions(&self, token: &str, required_permissions: &[String]) -> bool {
        if let Some(auth_token) = self.tokens.get(token) {
            if auth_token.is_revoked {
                return false;
            }
            
            let now = chrono::Utc::now();
            if now > auth_token.expires_at {
                return false;
            }
            
            // Check if token has all required permissions
            required_permissions.iter().all(|perm| {
                auth_token.permissions.contains(perm)
            })
        } else {
            false
        }
    }

    pub fn add_permission(&mut self, token: &str, permission: String) -> bool {
        if let Some(auth_token) = self.tokens.get_mut(token) {
            if !auth_token.permissions.contains(&permission) {
                auth_token.permissions.push(permission);
            }
            true
        } else {
            false
        }
    }

    pub fn remove_permission(&mut self, token: &str, permission: &str) -> bool {
        if let Some(auth_token) = self.tokens.get_mut(token) {
            auth_token.permissions.retain(|p| p != permission);
            true
        } else {
            false
        }
    }

    pub fn get_token_info(&self, token: &str) -> Option<&AuthToken> {
        self.tokens.get(token)
    }

    pub fn is_token_expired(&self, token: &str) -> bool {
        if let Some(auth_token) = self.tokens.get(token) {
            let now = chrono::Utc::now();
            now > auth_token.expires_at || auth_token.is_revoked
        } else {
            true
        }
    }

    pub fn refresh_token(&mut self, token: &str) -> Result<String, SyncError> {
        if let Some(auth_token) = self.tokens.get(token) {
            if auth_token.is_revoked {
                return Err(SyncError::Network("Token is revoked".to_string()));
            }
            
            let now = chrono::Utc::now();
            if now > auth_token.expires_at {
                return Err(SyncError::Network("Token is expired".to_string()));
            }
            
            // Generate new token with same client info
            self.generate_token(auth_token.client_id.clone(), auth_token.client_name.clone())
        } else {
            Err(SyncError::Network("Token not found".to_string()))
        }
    }
}

pub fn generate_client_id() -> String {
    use uuid::Uuid;
    format!("client_{}", Uuid::new_v4().to_string())
}

pub fn generate_secure_random_token() -> String {
    use uuid::Uuid;
    format!("syncmd_{}", Uuid::new_v4().to_string())
}

use crate::types::SyncError;