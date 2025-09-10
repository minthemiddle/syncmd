#![allow(dead_code)]

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token: String,
    pub client_id: String,
    pub client_name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct AuthManager {
    tokens: std::collections::HashMap<String, AuthToken>, // token -> AuthToken
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            tokens: std::collections::HashMap::new(),
        }
    }

    pub fn generate_token(&mut self, client_id: String, client_name: String) -> String {
        use uuid::Uuid;
        let token = format!("syncmd_{}", Uuid::new_v4().to_string());
        
        let auth_token = AuthToken {
            token: token.clone(),
            client_id: client_id.clone(),
            client_name,
            created_at: chrono::Utc::now(),
        };
        
        self.tokens.insert(token.clone(), auth_token);
        token
    }

    pub fn validate_token(&self, token: &str) -> Option<&AuthToken> {
        self.tokens.get(token)
    }

    pub fn revoke_token(&mut self, token: &str) -> bool {
        self.tokens.remove(token).is_some()
    }

    pub fn list_tokens(&self) -> Vec<&AuthToken> {
        self.tokens.values().collect()
    }
}

pub fn generate_client_id() -> String {
    use uuid::Uuid;
    format!("client_{}", Uuid::new_v4().to_string())
}