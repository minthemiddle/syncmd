use crate::types::{SyncError, SyncOperation, FileMetadata};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct SyncEngine {
    device_id: String,
}

impl SyncEngine {
    pub fn new(device_id: String) -> Self {
        Self { device_id }
    }

    pub fn calculate_sync_operations(
        &self,
        local_files: &HashMap<PathBuf, FileMetadata>,
        remote_files: &HashMap<PathBuf, FileMetadata>,
    ) -> Vec<SyncOperation> {
        let mut operations = Vec::new();

        // Files that exist locally but not remotely (additions)
        for (path, local_meta) in local_files {
            if let Some(remote_meta) = remote_files.get(path) {
                // File exists on both sides, check if update is needed
                if local_meta.hash != remote_meta.hash {
                    // Conflict resolution: newer version wins
                    if local_meta.version > remote_meta.version {
                        operations.push(SyncOperation::Update(local_meta.clone()));
                    }
                }
            } else {
                // File only exists locally
                operations.push(SyncOperation::Add(local_meta.clone()));
            }
        }

        // Files that exist remotely but not locally (additions from remote)
        for (path, remote_meta) in remote_files {
            if !local_files.contains_key(path) {
                operations.push(SyncOperation::Add(remote_meta.clone()));
            }
        }

        // Files that were deleted remotely
        for path in local_files.keys() {
            if !remote_files.contains_key(path) {
                operations.push(SyncOperation::Delete(path.clone()));
            }
        }

        operations
    }

    pub async fn apply_sync_operation(
        &self,
        operation: SyncOperation,
        local_files: &mut HashMap<PathBuf, FileMetadata>,
        remote_content: Option<Vec<u8>>,
    ) -> Result<(), SyncError> {
        match operation {
            SyncOperation::Add(metadata) | SyncOperation::Update(metadata) => {
                if metadata.device_id != self.device_id {
                    // This is a remote operation, we need content
                    if let Some(_content) = remote_content {
                        // For now, we'll just update the metadata
                        // Actual file I/O will be handled by the indexer
                        local_files.insert(metadata.path.clone(), metadata);
                    }
                } else {
                    // This is a local operation
                    local_files.insert(metadata.path.clone(), metadata);
                }
            }
            SyncOperation::Delete(path) => {
                local_files.remove(&path);
            }
        }
        Ok(())
    }

    pub fn merge_markdown_content(
        local_content: &str,
        remote_content: &str,
        base_content: &str,
    ) -> Result<String, SyncError> {
        // Simple 3-way merge for markdown files
        // Extract YAML frontmatter if present
        let (local_frontmatter, local_body) = Self::extract_frontmatter(local_content);
        let (remote_frontmatter, remote_body) = Self::extract_frontmatter(remote_content);
        let (base_frontmatter, base_body) = Self::extract_frontmatter(base_content);

        // Merge frontmatter (simple strategy: remote wins for now)
        let merged_frontmatter = if remote_frontmatter.is_empty() {
            local_frontmatter
        } else {
            remote_frontmatter
        };

        // Merge body content using diff3
        let merged_body = Self::merge_text_content(&local_body, &remote_body, &base_body)?;

        // Reconstruct the file
        let mut result = String::new();
        if !merged_frontmatter.is_empty() {
            result.push_str(&merged_frontmatter);
            result.push_str("---\n\n");
        }
        result.push_str(&merged_body);

        Ok(result)
    }

    fn extract_frontmatter(content: &str) -> (String, String) {
        if content.starts_with("---") {
            if let Some(end_offset) = content[3..].find("---") {
                let frontmatter_end = end_offset + 3;
                let frontmatter = content[..frontmatter_end + 3].to_string();
                let body = content[frontmatter_end + 3..].trim_start().to_string();
                return (frontmatter, body);
            }
        }
        (String::new(), content.to_string())
    }

    fn merge_text_content(local: &str, remote: &str, base: &str) -> Result<String, SyncError> {
        // For MVP, use a simple strategy: if base is empty, concatenate
        // Otherwise, use remote content (simple conflict resolution)
        if base.is_empty() {
            return Ok(format!("{}\n\n{}", local, remote));
        }

        // For now, prefer remote content (can be improved later)
        Ok(remote.to_string())
    }
}