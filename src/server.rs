mod types;
mod indexer;
mod sync;
mod network;
mod cli;
mod security;

use clap::Parser;
use cli::{Cli, Commands, Config};
use indexer::FileIndexer;
use network::{DeviceManager, NetworkManager};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Sync { path, port, .. } => {
            start_server(path, port).await?;
        }
        _ => {
            println!("Server mode only supports sync command");
        }
    }
    
    Ok(())
}

async fn start_server(
    path: std::path::PathBuf,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load()?;
    let device_manager = Arc::new(DeviceManager::new(config.device_name.clone()));
    
    println!("Starting syncmd server");
    println!("Device ID: {}", device_manager.device_id());
    println!("Device Name: {}", device_manager.device_name());
    println!("Sync path: {:?}", path);
    println!("Port: {}", port);
    
    let indexer = FileIndexer::new(device_manager.device_id().to_string(), path.clone());
    
    // Initial indexing
    let sync_state = indexer.index_directory()?;
    println!("Indexed {} files", sync_state.local_files.len());
    
    let network_manager = NetworkManager::new(
        device_manager.clone(),
        format!("0.0.0.0:{}", port),
    );
    
    println!("Server listening on port {}", port);
    network_manager.start_server().await?;
    
    Ok(())
}