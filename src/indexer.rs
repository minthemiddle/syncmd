#![allow(dead_code)]

use crate::types::{FileMetadata, SyncError, SyncState, FileCategory, FileAnalysis, FileChange, DetailedFileChanges};
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
                "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" | "bmp" | "ico" | "tiff" | "tif" => true,
                // Code files
                "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "html" | "css" | "scss" | "json" | "yaml" | "yml" | "toml" | "xml" => true,
                // Configuration files
                "ini" | "cfg" | "conf" | "config" | "env" | "env.example" => true,
                // Documentation files
                "txt" | "rtf" | "doc" | "docx" | "pdf" => true,
                // Data files
                "csv" | "tsv" | "jsonl" => true,
                _ => false,
            }
        } else {
            // Check if it's a dotfile that should be synced
            self.should_sync_dotfile(path)
        }
    }

    fn should_sync_dotfile(&self, path: &Path) -> bool {
        let file_name = path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        
        match file_name {
            // Common dotfiles to sync
            ".gitignore" | ".gitattributes" | ".editorconfig" | ".env.example" => true,
            // Project configuration files
            ".eslintrc" | ".eslintrc.json" | ".eslintrc.js" | ".eslintrc.yml" => true,
            ".prettierrc" | ".prettierrc.json" | ".prettierrc.js" | ".prettierrc.yml" => true,
            ".babelrc" | ".babelrc.json" | ".babelrc.js" => true,
            ".vscode" | ".vscodeignore" => true, // Directory
            // Package manager files
            "package.json" | "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" => true,
            // Build configuration
            "Cargo.toml" | "Cargo.lock" | "go.mod" | "go.sum" | "pom.xml" | "build.gradle" => true,
            // Python project files
            "requirements.txt" | "pyproject.toml" | "setup.py" | "Pipfile" | "poetry.lock" => true,
            // Node.js project files
            "tsconfig.json" | "webpack.config.js" | "vite.config.js" | "next.config.js" => true,
            // Other common project files
            "README" | "README.md" | "LICENSE" | "CHANGELOG.md" | "CONTRIBUTING.md" => true,
            _ => false,
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

    pub fn get_file_category(&self, path: &Path) -> FileCategory {
        if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
            match ext.to_lowercase().as_str() {
                // Text files
                "md" | "markdown" | "txt" | "rtf" => FileCategory::Text,
                // Code files
                "rs" | "py" | "js" | "ts" | "jsx" | "tsx" | "html" | "css" | "scss" | "json" | "yaml" | "yml" | "toml" | "xml" => FileCategory::Code,
                // Image files
                "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" | "bmp" | "ico" | "tiff" | "tif" => FileCategory::Image,
                // Document files
                "doc" | "docx" | "pdf" => FileCategory::Document,
                // Data files
                "csv" | "tsv" | "jsonl" => FileCategory::Data,
                // Configuration files
                "ini" | "cfg" | "conf" | "config" | "env" => FileCategory::Config,
                _ => FileCategory::Other,
            }
        } else {
            // Check for specific dotfiles or directories
            let file_name = path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            
            match file_name {
                ".gitignore" | ".gitattributes" | ".editorconfig" => FileCategory::Config,
                "package.json" | "Cargo.toml" | "requirements.txt" => FileCategory::Config,
                "README" | "LICENSE" | "CHANGELOG.md" => FileCategory::Text,
                _ => FileCategory::Other,
            }
        }
    }

    pub fn analyze_file_content(&self, path: &Path) -> Result<FileAnalysis, SyncError> {
        let content = fs::read(path)?;
        let mut analysis = FileAnalysis {
            line_count: 0,
            word_count: 0,
            character_count: content.len(),
            is_binary: false,
            encoding: "utf-8".to_string(),
            file_type: self.get_file_category(path),
        };

        // Try to detect if file is binary
        if content.iter().any(|&b| b == 0) {
            analysis.is_binary = true;
            return Ok(analysis);
        }

        // Try to decode as UTF-8 for text analysis
        if let Ok(text_content) = std::str::from_utf8(&content) {
            analysis.line_count = text_content.lines().count();
            analysis.word_count = text_content.split_whitespace().count();
            analysis.character_count = text_content.len();
        } else {
            analysis.is_binary = true;
        }

        Ok(analysis)
    }

    pub fn get_file_changes_detailed(&self, old_state: &SyncState) -> DetailedFileChanges {
        let mut changes = DetailedFileChanges {
            added: Vec::new(),
            modified: Vec::new(),
            deleted: Vec::new(),
            renamed: Vec::new(),
        };
        
        // Get current state
        let current_state = match self.index_directory() {
            Ok(state) => state,
            Err(_) => return changes,
        };
        
        // Find new or modified files
        for (path, current_meta) in &current_state.local_files {
            match old_state.local_files.get(path) {
                Some(old_meta) => {
                    if current_meta.hash != old_meta.hash {
                        let analysis = self.analyze_file_content(&self.sync_root.join(path)).unwrap_or_default();
                        changes.modified.push(FileChange {
                            path: path.clone(),
                            old_metadata: Some(old_meta.clone()),
                            new_metadata: Some(current_meta.clone()),
                            analysis,
                        });
                    }
                }
                None => {
                    let analysis = self.analyze_file_content(&self.sync_root.join(path)).unwrap_or_default();
                    changes.added.push(FileChange {
                        path: path.clone(),
                        old_metadata: None,
                        new_metadata: Some(current_meta.clone()),
                        analysis,
                    });
                }
            }
        }
        
        // Find deleted files
        for path in old_state.local_files.keys() {
            if !current_state.local_files.contains_key(path) {
                changes.deleted.push(FileChange {
                    path: path.clone(),
                    old_metadata: old_state.local_files.get(path).cloned(),
                    new_metadata: None,
                    analysis: FileAnalysis::default(),
                });
            }
        }
        
        changes
    }
}