mod types;
mod indexer;
mod sync;
mod network;
mod cli;
mod watcher;
mod file_transfer;
mod security;

use clap::Parser;
use cli::{Cli, Commands, Config};
use indexer::FileIndexer;
use network::{ClientManager, NetworkManager, NetworkMessage};
use sync::SyncEngine;
use std::sync::Arc;
use tokio::signal;
use watcher::{FileWatcher, WatchEvent};
use file_transfer::FileTransferManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    
    let cli = Cli::parse();
    
        
    match cli.command {
        Commands::Sync { path, connect, server, port } => {
            sync_folder(path, connect, server, port).await?;
        }
        Commands::ListClients => {
            list_clients().await?;
        }
        Commands::Status => {
            show_status().await?;
        }
        Commands::Init { path, name, auth_token } => {
            init_config(path, name, auth_token).await?;
        }
    }
    
    Ok(())
}

async fn sync_folder(
    path: std::path::PathBuf,
    connect: Option<String>,
    server_mode: bool,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load()?;
    let client_manager = Arc::new(ClientManager::new());
    
    println!("Starting sync for folder: {:?}", path);
    println!("Server ID: {}", client_manager.server_id());
    println!("Client Name: {}", config.device_name);
    
    let indexer = FileIndexer::new(client_manager.server_id().to_string(), path.clone());
    let sync_engine = SyncEngine::new(client_manager.server_id().to_string());
    
    // Initial indexing
    let sync_state = indexer.index_directory()?;
    println!("Indexed {} files", sync_state.local_files.len());
    
    let network_manager = NetworkManager::new(
        client_manager.clone(),
        format!("0.0.0.0:{}", port),
    );
    
    if server_mode {
        println!("Starting server on port {}", port);
        tokio::spawn(async move {
            if let Err(e) = network_manager.start_server().await {
                eprintln!("Server error: {}", e);
            }
        });
        
        // Wait for Ctrl+C
        signal::ctrl_c().await?;
        println!("Shutting down server...");
    } else if let Some(server_addr) = connect {
        println!("Connecting to server: {}", server_addr);
        
        let mut stream = network_manager.connect_to_server(&server_addr).await?;
        
        // Calculate root hash for handshake
        let _root_hash = calculate_root_hash(&sync_state)?;
        
        // Send authentication
        let auth_token = config.auth_token.clone()
            .ok_or("Authentication token required. Please run 'syncmd init' with --auth-token.")?;
        network_manager.send_authentication(&mut stream, auth_token, config.device_name.clone()).await?;
        println!("Connected to server successfully");
        
        // Start file watcher for real-time sync
        let mut file_watcher = FileWatcher::new(path.clone())?;
        println!("Started file watcher for: {:?}", path);
        
        // Start periodic sync
        let sync_stream = Arc::new(tokio::sync::Mutex::new(stream));
        let sync_indexer = Arc::new(indexer);
        let sync_engine_clone = Arc::new(sync_engine);
        
        // File watching task
        let watcher_sync_stream = sync_stream.clone();
        let watcher_indexer = sync_indexer.clone();
        let watcher_engine = sync_engine_clone.clone();
        let watcher_path = path.clone();
        
        tokio::spawn(async move {
            let mut last_sync = std::time::Instant::now();
            
            loop {
                if let Some(event) = file_watcher.next_event_debounced().await {
                    // Check if this event should trigger a sync
                    if file_watcher.should_sync_event(&event) {
                        println!("File event: {:?}", event);
                        
                        // Get relative path for logging
                        if let Some(relative_path) = file_watcher.get_relative_path(
                            match &event {
                                WatchEvent::Created(p) | WatchEvent::Modified(p) | WatchEvent::Deleted(p) => p,
                                WatchEvent::Renamed(_, new) => new,
                            },
                            &watcher_path
                        ) {
                            println!("Relative path: {:?}", relative_path);
                        }
                        
                        // Debounce rapid changes
                        if last_sync.elapsed() > std::time::Duration::from_secs(1) {
                            if let Ok(mut stream) = watcher_sync_stream.try_lock() {
                                if let Err(e) = perform_sync(&watcher_indexer, &watcher_engine, &mut stream).await {
                                    eprintln!("Real-time sync error: {}", e);
                                }
                                last_sync = std::time::Instant::now();
                            }
                        }
                    }
                }
            }
        });
        
        // Periodic sync task
        let periodic_sync_stream = sync_stream.clone();
        let periodic_indexer = sync_indexer.clone();
        let periodic_engine = sync_engine_clone.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                if let Ok(mut stream) = periodic_sync_stream.try_lock() {
                    if let Err(e) = perform_sync(&periodic_indexer, &periodic_engine, &mut stream).await {
                        eprintln!("Periodic sync error: {}", e);
                    }
                }
            }
        });
        
        // Wait for Ctrl+C
        signal::ctrl_c().await?;
        println!("Shutting down client...");
    } else {
        println!("Either --connect or --server must be specified");
    }
    
    Ok(())
}

async fn perform_sync(
    indexer: &FileIndexer,
    _sync_engine: &SyncEngine,
    stream: &mut tokio::net::TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    
    // Get current state
    let sync_state = indexer.index_directory()?;
    
    // Send sync request
    let sync_request = NetworkMessage::SyncRequest {
        client_id: sync_state.device_id.clone(),
        files: sync_state.local_files.values().cloned().collect(),
    };
    
    let request_data = serde_json::to_vec(&sync_request)?;
    stream.write_all(&request_data).await?;
    
    // Read response
    let mut response_buffer = vec![0u8; 8192];
    let n = stream.read(&mut response_buffer).await?;
    
    if n > 0 {
        let response: NetworkMessage = serde_json::from_slice(&response_buffer[..n])?;
        
        if let NetworkMessage::SyncResponse { operations } = response {
            println!("Received {} sync operations", operations.len());
            
            // Apply operations
            for operation in operations {
                match operation {
                    crate::types::SyncOperation::Add(metadata) => {
                        println!("Add operation for: {:?}", metadata.path);
                        
                        // Use new file transfer system
                        let mut transfer_manager = FileTransferManager::new();
                        if let Err(e) = transfer_manager.receive_file(stream, indexer.sync_root()).await {
                            eprintln!("File transfer error: {}", e);
                        }
                    }
                    crate::types::SyncOperation::Update(metadata) => {
                        println!("Update operation for: {:?}", metadata.path);
                        
                        // Use new file transfer system for updates too
                        let mut transfer_manager = FileTransferManager::new();
                        if let Err(e) = transfer_manager.receive_file(stream, indexer.sync_root()).await {
                            eprintln!("File transfer error: {}", e);
                        }
                    }
                    crate::types::SyncOperation::Delete(path) => {
                        println!("Delete operation for: {:?}", path);
                        indexer.delete_file(&path)?;
                    }
                }
            }
        }
    }
    
    Ok(())
}

async fn list_clients() -> Result<(), Box<dyn std::error::Error>> {
    let _config = Config::load()?;
    let client_manager = ClientManager::new();
    
    let clients = client_manager.list_clients().await;
    
    if clients.is_empty() {
        println!("No connected clients");
    } else {
        println!("Connected clients:");
        for client in clients {
            println!("  - {} ({}) at {}", client.name, client.id, client.address);
        }
    }
    
    Ok(())
}

async fn show_status() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load()?;
    
    println!("Device ID: {}", config.device_id);
    println!("Device Name: {}", config.device_name);
    println!("Sync Roots:");
    
    for root in &config.sync_roots {
        let status = if root.enabled { "enabled" } else { "disabled" };
        let last_sync = root.last_sync
            .map(|t| t.to_rfc2822())
            .unwrap_or_else(|| "never".to_string());
        println!("  - {:?} ({}) - last sync: {}", root.path, status, last_sync);
    }
    
    Ok(())
}


async fn init_config(
    path: std::path::PathBuf,
    name: String,
    auth_token: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::load()?;
    config.device_name = name;
    
    if let Some(token) = auth_token {
        println!("Setting up authentication...");
        config.auth_token = Some(token);
        println!("Authentication token configured");
    }
    
    config.add_sync_root(path);
    config.save()?;
    
    println!("Configuration initialized successfully");
    println!("Client Name: {}", config.device_name);
    
    Ok(())
}

fn calculate_root_hash(sync_state: &types::SyncState) -> Result<String, Box<dyn std::error::Error>> {
    use md5::Digest;
    
    let mut hasher = md5::Md5::new();
    for (path, metadata) in &sync_state.local_files {
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update(metadata.hash.as_bytes());
    }
    
    Ok(format!("{:x}", hasher.finalize()))
}