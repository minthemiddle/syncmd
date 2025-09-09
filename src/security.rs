use crate::types::SyncError;
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use base64::{Engine as _, engine::general_purpose};

const SALT_LEN: usize = 32;
const KEY_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCredentials {
    pub device_id: String,
    pub password_hash: String,
    pub salt: Vec<u8>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct SecurityManager {
    device_credentials: DeviceCredentials,
}

impl SecurityManager {
    pub fn new(password: &str, device_id: String) -> Result<Self, SyncError> {
        // Generate salt
        let salt: [u8; SALT_LEN] = rand::random();
        
        // Hash password
        let password_hash = Self::hash_password(password, &salt)?;
        
        let device_credentials = DeviceCredentials {
            device_id: device_id.clone(),
            password_hash,
            salt: salt.to_vec(),
            created_at: chrono::Utc::now(),
        };
        
        Ok(Self {
            device_credentials,
        })
    }
    
    pub fn load_credentials(password: &str, credentials: DeviceCredentials) -> Result<Self, SyncError> {
        // Verify password
        let password_hash = Self::hash_password(password, &credentials.salt)?;
        if password_hash != credentials.password_hash {
            return Err(SyncError::Network("Invalid password".to_string()));
        }
        
        Ok(Self {
            device_credentials: credentials,
        })
    }
    
    pub fn get_credentials(&self) -> &DeviceCredentials {
        &self.device_credentials
    }
    
    pub fn verify_password(&self, password: &str) -> Result<bool, SyncError> {
        let password_hash = Self::hash_password(password, &self.device_credentials.salt)?;
        Ok(password_hash == self.device_credentials.password_hash)
    }
    
    fn hash_password(password: &str, salt: &[u8]) -> Result<String, SyncError> {
        let mut hasher = Sha256::new();
        hasher.update(password.as_bytes());
        hasher.update(salt);
        Ok(format!("{:x}", hasher.finalize()))
    }
}

pub fn generate_device_id() -> String {
    let device_id: [u8; 16] = rand::random();
    format!("syncmd_{}", general_purpose::STANDARD.encode(&device_id))
}

pub fn hash_password(password: &str, salt: &[u8]) -> Result<String, SyncError> {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(salt);
    Ok(format!("{:x}", hasher.finalize()))
}