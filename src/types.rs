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
pub struct ClientInfo {
    pub id: String,
    pub name: String,
    pub address: String,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub auth_token: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FileCategory {
    Text,
    Code,
    Image,
    Document,
    Data,
    Config,
    Other,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileAnalysis {
    pub line_count: usize,
    pub word_count: usize,
    pub character_count: usize,
    pub is_binary: bool,
    pub encoding: String,
    pub file_type: FileCategory,
}

impl Default for FileAnalysis {
    fn default() -> Self {
        Self {
            line_count: 0,
            word_count: 0,
            character_count: 0,
            is_binary: false,
            encoding: "utf-8".to_string(),
            file_type: FileCategory::Other,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileChange {
    pub path: PathBuf,
    pub old_metadata: Option<FileMetadata>,
    pub new_metadata: Option<FileMetadata>,
    pub analysis: FileAnalysis,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DetailedFileChanges {
    pub added: Vec<FileChange>,
    pub modified: Vec<FileChange>,
    pub deleted: Vec<FileChange>,
    pub renamed: Vec<FileChange>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransferProgress {
    pub transfer_id: String,
    pub bytes_transferred: u64,
    pub total_bytes: u64,
    pub progress: f64,
    pub speed_mbps: f64,
    pub elapsed_seconds: f64,
    pub estimated_remaining_seconds: f64,
    pub chunks_received: u32,
    pub total_chunks: u32,
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
    #[allow(dead_code)]
    Network(String),
    
    #[error("File not found: {0}")]
    #[allow(dead_code)]
    NotFound(PathBuf),
    
    #[error("Conflict detected: {0}")]
    #[allow(dead_code)]
    Conflict(String),
    
    #[error("Path strip error: {0}")]
    StripPrefix(#[from] std::path::StripPrefixError),
    
    #[error("System time error: {0}")]
    SystemTime(#[from] std::time::SystemTimeError),
    
    #[error("File watcher error: {0}")]
    Watcher(#[from] notify::Error),
    
    #[error("Authentication error: {0}")]
    Auth(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Token revoked")]
    TokenRevoked,
    
    #[error("Invalid token")]
    InvalidToken,
    
    #[error("Session expired")]
    SessionExpired,
}