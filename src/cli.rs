#![allow(dead_code)]

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "syncmd")]
#[command(about = "Efficient markdown file synchronization tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    
    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start syncing a folder
    Sync {
        /// Path to the folder to sync
        #[arg(short, long)]
        path: PathBuf,
        
        /// Connect to a remote server
        #[arg(short, long)]
        connect: Option<String>,
        
        /// Start in server mode
        #[arg(long)]
        server: bool,
        
        /// Port to listen on (server mode)
        #[arg(long, default_value = "8080")]
        port: u16,
    },
    
    /// List connected clients
    ListClients,
    
    /// Show current sync status
    Status,
    
    /// Initialize a new sync configuration
    Init {
        /// Path to initialize
        #[arg(short, long)]
        path: PathBuf,
        
        /// Client name
        #[arg(short, long, default_value = "syncmd-client")]
        name: String,
        
        /// Authentication token for server access
        #[arg(long)]
        auth_token: Option<String>,
    },
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub device_id: String,
    pub device_name: String,
    pub sync_roots: Vec<SyncRoot>,
    pub auth_token: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SyncRoot {
    pub path: PathBuf,
    pub enabled: bool,
    pub last_sync: Option<chrono::DateTime<chrono::Utc>>,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = Self::config_path()?;
        if config_path.exists() {
            let content = std::fs::read_to_string(config_path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::config_path()?;
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    fn config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Could not find config directory")?
            .join("syncmd");
        Ok(config_dir.join("config.json"))
    }

    fn default() -> Self {
        Self {
            device_id: uuid::Uuid::new_v4().to_string(),
            device_name: "syncmd-client".to_string(),
            sync_roots: Vec::new(),
            auth_token: None,
        }
    }

    pub fn add_sync_root(&mut self, path: PathBuf) {
        self.sync_roots.push(SyncRoot {
            path,
            enabled: true,
            last_sync: None,
        });
    }

    pub fn get_sync_root(&self, path: &PathBuf) -> Option<&SyncRoot> {
        self.sync_roots.iter().find(|root| root.path == *path)
    }
}