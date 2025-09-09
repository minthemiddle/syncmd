use crate::types::SyncError;
use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event, EventKind};
use std::path::PathBuf;
use tokio::sync::mpsc;

pub struct FileWatcher {
    watcher: RecommendedWatcher,
    event_rx: mpsc::Receiver<WatchEvent>,
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
        })
    }
    
    pub async fn next_event(&mut self) -> Option<WatchEvent> {
        self.event_rx.recv().await
    }
    
    pub fn watch_path(&mut self, path: &PathBuf) -> Result<(), SyncError> {
        self.watcher.watch(path, RecursiveMode::Recursive)?;
        Ok(())
    }
    
    pub fn unwatch_path(&mut self, path: &PathBuf) -> Result<(), SyncError> {
        self.watcher.unwatch(path)?;
        Ok(())
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
        
        if let Some(WatchEvent::Created(path)) = watcher.next_event().await {
            assert_eq!(path, test_file);
        } else {
            panic!("Expected Created event");
        }
        
        // Test file modification
        std::fs::write(&test_file, "# Updated Test").unwrap();
        
        if let Some(WatchEvent::Modified(path)) = watcher.next_event().await {
            assert_eq!(path, test_file);
        } else {
            panic!("Expected Modified event");
        }
        
        // Test file deletion
        std::fs::remove_file(&test_file).unwrap();
        
        if let Some(WatchEvent::Deleted(path)) = watcher.next_event().await {
            assert_eq!(path, test_file);
        } else {
            panic!("Expected Deleted event");
        }
    }
}