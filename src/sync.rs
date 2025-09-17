#![allow(dead_code)]

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
                    // Conflict resolution: newer version wins based on timestamp
                    if local_meta.modified > remote_meta.modified {
                        operations.push(SyncOperation::Update(local_meta.clone()));
                    } else if remote_meta.modified > local_meta.modified {
                        operations.push(SyncOperation::Update(remote_meta.clone()));
                    } else {
                        // Same timestamp, prefer local version
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
        let (_base_frontmatter, base_body) = Self::extract_frontmatter(base_content);

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
        // Use simple merge for now (similar crate not available)
        if base.is_empty() {
            return Ok(format!("{}\n\n{}", local, remote));
        }

        // Simple conflict detection and resolution
        let mut result = String::new();
        
        // Check if there are significant changes in both versions
        let local_changes = local.lines().count() != base.lines().count();
        let remote_changes = remote.lines().count() != base.lines().count();
        
        if local_changes && remote_changes {
            // Both sides have changes, use conflict markers
            result.push_str("<<<<<<< LOCAL\n");
            result.push_str(local);
            result.push_str("\n=======\n");
            result.push_str(remote);
            result.push_str("\n>>>>>>> REMOTE\n");
        } else if remote_changes {
            // Only remote has changes
            result.push_str(remote);
        } else {
            // Only local has changes or no changes
            result.push_str(local);
        }
        
        Ok(result)
    }

    pub fn calculate_bidirectional_sync(
        &self,
        local_files: &HashMap<PathBuf, FileMetadata>,
        remote_files: &HashMap<PathBuf, FileMetadata>,
    ) -> (Vec<SyncOperation>, Vec<SyncOperation>) {
        let mut local_operations = Vec::new();
        let mut remote_operations = Vec::new();

        for (path, local_meta) in local_files {
            if let Some(remote_meta) = remote_files.get(path) {
                // File exists on both sides
                if local_meta.hash != remote_meta.hash {
                    // Conflict resolution based on timestamps
                    if local_meta.modified > remote_meta.modified {
                        // Local is newer, push to remote
                        remote_operations.push(SyncOperation::Update(local_meta.clone()));
                    } else if remote_meta.modified > local_meta.modified {
                        // Remote is newer, pull to local
                        local_operations.push(SyncOperation::Update(remote_meta.clone()));
                    } else {
                        // Same timestamp, prefer local version
                        remote_operations.push(SyncOperation::Update(local_meta.clone()));
                    }
                }
            } else {
                // File only exists locally, push to remote
                remote_operations.push(SyncOperation::Add(local_meta.clone()));
            }
        }

        for (path, remote_meta) in remote_files {
            if !local_files.contains_key(path) {
                // File only exists remotely, pull to local
                local_operations.push(SyncOperation::Add(remote_meta.clone()));
            }
        }

        // Handle deletions
        for path in local_files.keys() {
            if !remote_files.contains_key(path) {
                // File was deleted locally, delete from remote
                remote_operations.push(SyncOperation::Delete(path.clone()));
            }
        }

        for path in remote_files.keys() {
            if !local_files.contains_key(path) {
                // File was deleted remotely, delete from local
                local_operations.push(SyncOperation::Delete(path.clone()));
            }
        }

        (local_operations, remote_operations)
    }

    pub fn merge_markdown_files_with_conflict_resolution(
        &self,
        local_content: &str,
        remote_content: &str,
        base_content: &str,
        local_meta: &FileMetadata,
        remote_meta: &FileMetadata,
    ) -> Result<String, SyncError> {
        // Enhanced conflict resolution with multiple strategies
        if local_meta.modified > remote_meta.modified {
            // Local is newer, but still try to merge if there are conflicts
            if Self::has_significant_changes(local_content, base_content) && 
               Self::has_significant_changes(remote_content, base_content) {
                // Both sides have significant changes, attempt merge
                match Self::merge_markdown_content(local_content, remote_content, base_content) {
                    Ok(merged) => Ok(merged),
                    Err(_) => Ok(local_content.to_string()), // Fall back to local
                }
            } else {
                Ok(local_content.to_string())
            }
        } else if remote_meta.modified > local_meta.modified {
            // Remote is newer, but still try to merge if there are conflicts
            if Self::has_significant_changes(local_content, base_content) && 
               Self::has_significant_changes(remote_content, base_content) {
                // Both sides have significant changes, attempt merge
                match Self::merge_markdown_content(local_content, remote_content, base_content) {
                    Ok(merged) => Ok(merged),
                    Err(_) => Ok(remote_content.to_string()), // Fall back to remote
                }
            } else {
                Ok(remote_content.to_string())
            }
        } else {
            // Same timestamp, try to merge
            Self::merge_markdown_content(local_content, remote_content, base_content)
        }
    }

    fn has_significant_changes(content: &str, base: &str) -> bool {
        // Simple change detection (similar crate not available)
        if base.is_empty() {
            return !content.is_empty();
        }
        
        // Count line differences
        let base_lines = base.lines().count();
        let content_lines = content.lines().count();
        
        // Consider significant if more than 2 lines different
        (base_lines as i32 - content_lines as i32).abs() > 2
    }

    pub fn create_sync_report(
        &self,
        local_operations: &[SyncOperation],
        remote_operations: &[SyncOperation],
    ) -> String {
        let mut report = String::new();
        report.push_str("=== Sync Report ===\n\n");
        
        report.push_str("Local Operations:\n");
        for op in local_operations {
            match op {
                SyncOperation::Add(meta) => report.push_str(&format!("  + Add: {:?}\n", meta.path)),
                SyncOperation::Update(meta) => report.push_str(&format!("  * Update: {:?}\n", meta.path)),
                SyncOperation::Delete(path) => report.push_str(&format!("  - Delete: {:?}\n", path)),
            }
        }
        
        report.push_str("\nRemote Operations:\n");
        for op in remote_operations {
            match op {
                SyncOperation::Add(meta) => report.push_str(&format!("  + Add: {:?}\n", meta.path)),
                SyncOperation::Update(meta) => report.push_str(&format!("  * Update: {:?}\n", meta.path)),
                SyncOperation::Delete(path) => report.push_str(&format!("  - Delete: {:?}\n", path)),
            }
        }
        
        report
    }
}