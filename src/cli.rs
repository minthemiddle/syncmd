use clap::{Parser, Subcommand};
use std::path::PathBuf;
use crate::security::DeviceCredentials;

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
        
        /// Enable peer discovery mode
        #[arg(long)]
        discover: bool,
        
        /// Port to listen on (server mode)
        #[arg(long, default_value = "8080")]
        port: u16,
        
        /// Discovery port
        #[arg(long, default_value = "8081")]
        discovery_port: u16,
    },
    
    /// List connected devices
    ListDevices,
    
    /// Show current sync status
    Status,
    
    /// Discover other syncmd peers on the network
    Discover {
        /// Port for discovery (default: 8081)
        #[arg(long, default_value = "8081")]
        port: u16,
    },
    
    /// Initialize a new sync configuration
    Init {
        /// Path to initialize
        #[arg(short, long)]
        path: PathBuf,
        
        /// Device name
        #[arg(short, long, default_value = "syncmd-device")]
        name: String,
        
        /// Enable encryption (requires password)
        #[arg(long)]
        encryption: bool,
        
        /// Password for encryption
        #[arg(long)]
        password: Option<String>,
    },
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub device_id: String,
    pub device_name: String,
    pub sync_roots: Vec<SyncRoot>,
    pub encryption_enabled: bool,
    pub credentials: Option<DeviceCredentials>,
    pub trusted_devices: Vec<String>,
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
            device_name: "syncmd-device".to_string(),
            sync_roots: Vec::new(),
            encryption_enabled: false,
            credentials: None,
            trusted_devices: Vec::new(),
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