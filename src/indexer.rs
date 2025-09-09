use crate::types::{FileMetadata, SyncError, SyncState};
use blake3::hash;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

pub struct FileIndexer {
    device_id: String,
    sync_root: PathBuf,
}

impl FileIndexer {
    pub fn new(device_id: String, sync_root: PathBuf) -> Self {
        Self {
            device_id,
            sync_root,
        }
    }

    pub fn sync_root(&self) -> &PathBuf {
        &self.sync_root
    }

    pub fn index_directory(&self) -> Result<SyncState, SyncError> {
        let mut local_files = std::collections::HashMap::new();
        
        for entry in WalkDir::new(&self.sync_root)
            .into_iter()
            .filter_entry(|e| !Self::is_hidden(e.path()))
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() && (path.extension().and_then(|s| s.to_str()) == Some("md") || 
                                 Self::is_image_file(path)) {
                if let Ok(metadata) = self.get_file_metadata(path) {
                    local_files.insert(path.strip_prefix(&self.sync_root)?.to_path_buf(), metadata);
                }
            }
        }

        Ok(SyncState {
            local_files,
            device_id: self.device_id.clone(),
            sync_root: self.sync_root.clone(),
        })
    }

    fn get_file_metadata(&self, path: &Path) -> Result<FileMetadata, SyncError> {
        let metadata = fs::metadata(path)?;
        let content = fs::read(path)?;
        let file_hash = hash(&content);
        let relative_path = path.strip_prefix(&self.sync_root)?.to_path_buf();

        Ok(FileMetadata {
            path: relative_path,
            hash: file_hash.to_hex().to_string(),
            size: metadata.len(),
            modified: metadata.modified()?,
            created: metadata.created()?,
            version: metadata.modified()?.duration_since(SystemTime::UNIX_EPOCH)?.as_secs(),
            device_id: self.device_id.clone(),
        })
    }

    fn is_hidden(path: &Path) -> bool {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with('.'))
            .unwrap_or(false)
    }

    fn is_image_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| matches!(ext.to_lowercase().as_str(), "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp"))
            .unwrap_or(false)
    }

    pub fn calculate_file_hash(path: &Path) -> Result<String, SyncError> {
        let content = fs::read(path)?;
        let hash = hash(&content);
        Ok(hash.to_hex().to_string())
    }

    pub fn read_file_content(&self, relative_path: &Path) -> Result<Vec<u8>, SyncError> {
        let full_path = self.sync_root.join(relative_path);
        Ok(fs::read(full_path)?)
    }

    pub fn write_file_content(&self, relative_path: &Path, content: &[u8]) -> Result<(), SyncError> {
        let full_path = self.sync_root.join(relative_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(fs::write(full_path, content)?)
    }

    pub fn delete_file(&self, relative_path: &Path) -> Result<(), SyncError> {
        let full_path = self.sync_root.join(relative_path);
        Ok(fs::remove_file(full_path)?)
    }
}