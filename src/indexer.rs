#![allow(dead_code)]

use crate::types::{FileMetadata, SyncError, SyncState};
use blake3::hash;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

#[allow(dead_code)]
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
            if path.is_file() && self.should_sync_file(path) {
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

    fn should_sync_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
            match ext.to_lowercase().as_str() {
                // Markdown files
                "md" | "markdown" | "mdown" | "mkdn" | "mkd" | "mdwn" | "mdtxt" | "mdtext" | "text" => true,
                // Image files
                "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" | "bmp" | "ico" => true,
                _ => false,
            }
        } else {
            false
        }
    }

    fn is_image_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| matches!(ext.to_lowercase().as_str(), "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" | "bmp" | "ico"))
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

    pub fn get_file_changes(&self, old_state: &SyncState) -> Vec<crate::types::SyncOperation> {
        let mut operations = Vec::new();
        
        // Get current state
        let current_state = match self.index_directory() {
            Ok(state) => state,
            Err(_) => return operations,
        };
        
        // Find new or modified files
        for (path, current_meta) in &current_state.local_files {
            match old_state.local_files.get(path) {
                Some(old_meta) => {
                    if current_meta.hash != old_meta.hash {
                        operations.push(crate::types::SyncOperation::Update(current_meta.clone()));
                    }
                }
                None => {
                    operations.push(crate::types::SyncOperation::Add(current_meta.clone()));
                }
            }
        }
        
        // Find deleted files
        for path in old_state.local_files.keys() {
            if !current_state.local_files.contains_key(path) {
                operations.push(crate::types::SyncOperation::Delete(path.clone()));
            }
        }
        
        operations
    }

    pub fn is_text_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
            matches!(ext.to_lowercase().as_str(), 
                "md" | "markdown" | "mdown" | "mkdn" | "mkd" | "mdwn" | "mdtxt" | "mdtext" | "text")
        } else {
            false
        }
    }

    pub fn get_file_size(&self, relative_path: &Path) -> Result<u64, SyncError> {
        let full_path = self.sync_root.join(relative_path);
        Ok(fs::metadata(full_path)?.len())
    }
}