use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub hash: String,
    pub size: u64,
    pub modified: SystemTime,
    pub created: SystemTime,
    pub version: u64,
    pub device_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncState {
    pub local_files: std::collections::HashMap<PathBuf, FileMetadata>,
    pub device_id: String,
    pub sync_root: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SyncOperation {
    Add(FileMetadata),
    Update(FileMetadata),
    Delete(PathBuf),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub address: String,
    pub last_seen: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("File not found: {0}")]
    NotFound(PathBuf),
    
    #[error("Conflict detected: {0}")]
    Conflict(String),
    
    #[error("Path strip error: {0}")]
    StripPrefix(#[from] std::path::StripPrefixError),
    
    #[error("System time error: {0}")]
    SystemTime(#[from] std::time::SystemTimeError),
    
    #[error("File watcher error: {0}")]
    Watcher(#[from] notify::Error),
}