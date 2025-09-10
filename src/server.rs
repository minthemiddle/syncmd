mod types;
mod indexer;
mod sync;
mod network;
mod cli;
mod security;

use clap::Parser;
use cli::{Cli, Commands, Config};
use indexer::FileIndexer;
use network::{ClientManager, NetworkManager};
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
    let client_manager = Arc::new(ClientManager::new());
    
    println!("Starting syncmd server");
    println!("Server ID: {}", client_manager.server_id());
    println!("Server Name: {}", config.device_name);
    println!("Sync path: {:?}", path);
    println!("Port: {}", port);
    
    let indexer = FileIndexer::new(client_manager.server_id().to_string(), path.clone());
    
    // Initial indexing
    let sync_state = indexer.index_directory()?;
    println!("Indexed {} files", sync_state.local_files.len());
    
    let network_manager = NetworkManager::new(
        client_manager.clone(),
        format!("0.0.0.0:{}", port),
    );
    
    println!("Server listening on port {}", port);
    network_manager.start_server().await?;
    
    Ok(())
}