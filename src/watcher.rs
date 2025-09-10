#![allow(dead_code)]

use crate::types::SyncError;
use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event, EventKind};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub struct FileWatcher {
    watcher: RecommendedWatcher,
    event_rx: mpsc::Receiver<WatchEvent>,
    debouncer: std::collections::HashMap<PathBuf, Instant>,
    debounce_duration: Duration,
}

#[derive(Debug, Clone)]
pub enum WatchEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
    Renamed(PathBuf, PathBuf), // old_path, new_path
}

impl FileWatcher {
    pub fn new(watch_path: PathBuf) -> Result<Self, SyncError> {
        Self::with_debounce(watch_path, Duration::from_millis(500))
    }

    pub fn with_debounce(watch_path: PathBuf, debounce_duration: Duration) -> Result<Self, SyncError> {
        let (event_tx, event_rx) = mpsc::channel(100);
        
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    if let Some(path) = event.paths.first() {
                        match event.kind {
                            EventKind::Create(_) => {
                                let _ = event_tx.blocking_send(WatchEvent::Created(path.clone()));
                            }
                            EventKind::Modify(_) => {
                                let _ = event_tx.blocking_send(WatchEvent::Modified(path.clone()));
                            }
                            EventKind::Remove(_) => {
                                let _ = event_tx.blocking_send(WatchEvent::Deleted(path.clone()));
                            }
                            _ => {
                                // Handle other events
                                if event.paths.len() > 1 {
                                    let _ = event_tx.blocking_send(WatchEvent::Renamed(path.clone(), event.paths[1].clone()));
                                }
                            }
                        }
                    }
                }
            },
            notify::Config::default(),
        )?;
        
        watcher.watch(&watch_path, RecursiveMode::Recursive)?;
        
        Ok(Self {
            watcher,
            event_rx,
            debouncer: std::collections::HashMap::new(),
            debounce_duration,
        })
    }
    
    pub async fn next_event(&mut self) -> Option<WatchEvent> {
        self.event_rx.recv().await
    }

    pub async fn next_event_debounced(&mut self) -> Option<WatchEvent> {
        loop {
            match self.event_rx.recv().await {
                Some(event) => {
                    let path = match &event {
                        WatchEvent::Created(p) | WatchEvent::Modified(p) | WatchEvent::Deleted(p) => p.clone(),
                        WatchEvent::Renamed(_old, new) => new.clone(),
                    };

                    // Check if we should debounce this event
                    if let Some(last_time) = self.debouncer.get(&path) {
                        if last_time.elapsed() < self.debounce_duration {
                            continue; // Skip this event, it's too soon after the last one
                        }
                    }

                    // Update the debouncer
                    self.debouncer.insert(path, Instant::now());
                    
                    // Clean up old entries from the debouncer
                    self.debouncer.retain(|_, time| time.elapsed() < self.debounce_duration * 2);
                    
                    return Some(event);
                }
                None => return None,
            }
        }
    }
    
    pub fn watch_path(&mut self, path: &PathBuf) -> Result<(), SyncError> {
        self.watcher.watch(path, RecursiveMode::Recursive)?;
        Ok(())
    }
    
    pub fn unwatch_path(&mut self, path: &PathBuf) -> Result<(), SyncError> {
        self.watcher.unwatch(path)?;
        Ok(())
    }

    pub fn should_sync_event(&self, event: &WatchEvent) -> bool {
        let path = match event {
            WatchEvent::Created(p) | WatchEvent::Modified(p) | WatchEvent::Deleted(p) => p,
            WatchEvent::Renamed(_, new) => new,
        };

        // Skip hidden files and directories
        if path.file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.starts_with('.'))
            .unwrap_or(false)
        {
            return false;
        }

        // Check file extension
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

    pub fn get_relative_path(&self, path: &PathBuf, base_path: &PathBuf) -> Option<PathBuf> {
        path.strip_prefix(base_path).ok().map(|p| p.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_file_watcher() {
        let temp_dir = TempDir::new().unwrap();
        let watch_path = temp_dir.path().to_path_buf();
        
        let mut watcher = FileWatcher::new(watch_path.clone()).unwrap();
        
        // Test file creation
        let test_file = watch_path.join("test.md");
        std::fs::write(&test_file, "# Test").unwrap();
        
        // Wait for and verify the created event
        let mut created_received = false;
        for _ in 0..10 {
            if let Some(event) = watcher.next_event().await {
                match event {
                    WatchEvent::Created(path) => {
                        if path.file_name() == test_file.file_name() {
                            created_received = true;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        assert!(created_received, "Expected Created event for test file");
        
        // Test file modification
        std::fs::write(&test_file, "# Updated Test").unwrap();
        
        // Wait for and verify the modified event
        let mut modified_received = false;
        for _ in 0..10 {
            if let Some(event) = watcher.next_event().await {
                match event {
                    WatchEvent::Modified(path) => {
                        if path.file_name() == test_file.file_name() {
                            modified_received = true;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        assert!(modified_received, "Expected Modified event for test file");
        
        // Test file deletion
        std::fs::remove_file(&test_file).unwrap();
        
        // Wait for and verify the deleted event
        let mut deleted_received = false;
        for _ in 0..10 {
            if let Some(event) = watcher.next_event().await {
                match event {
                    WatchEvent::Deleted(path) => {
                        if path.file_name() == test_file.file_name() {
                            deleted_received = true;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
        assert!(deleted_received, "Expected Deleted event for test file");
    }
}