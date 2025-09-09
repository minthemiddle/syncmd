mod types;
mod indexer;
mod sync;
mod network;
mod cli;
mod file_transfer;
mod security;

use clap::Parser;
use cli::{Cli, Commands, Config};
// use indexer::FileIndexer;
use network::{DeviceManager, NetworkManager, NetworkMessage};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug)]
struct ServerState {
    files: HashMap<String, Vec<u8>>,  // path -> content
    metadata: HashMap<String, types::FileMetadata>,  // path -> metadata
    clients: HashMap<String, String>,  // device_id -> address
}

impl ServerState {
    fn new() -> Self {
        Self {
            files: HashMap::new(),
            metadata: HashMap::new(),
            clients: HashMap::new(),
        }
    }

    fn add_file(&mut self, path: String, content: Vec<u8>, metadata: types::FileMetadata) {
        self.files.insert(path.clone(), content);
        self.metadata.insert(path, metadata);
    }

    fn get_file(&self, path: &str) -> Option<&Vec<u8>> {
        self.files.get(path)
    }

    fn get_metadata(&self, path: &str) -> Option<&types::FileMetadata> {
        self.metadata.get(path)
    }

    fn list_files(&self) -> Vec<&types::FileMetadata> {
        self.metadata.values().collect()
    }

    fn add_client(&mut self, device_id: String, address: String) {
        self.clients.insert(device_id, address);
    }

    fn remove_client(&mut self, device_id: &str) {
        self.clients.remove(device_id);
    }
}

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
    storage_path: std::path::PathBuf,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::load()?;
    let device_manager = Arc::new(DeviceManager::new(config.device_name.clone()));
    let state = Arc::new(RwLock::new(ServerState::new()));
    
    println!("Starting syncmd VPS server");
    println!("Device ID: {}", device_manager.device_id());
    println!("Device Name: {}", device_manager.device_name());
    println!("Storage path: {:?}", storage_path);
    println!("Port: {}", port);
    
    // Initialize storage directory
    if !storage_path.exists() {
        std::fs::create_dir_all(&storage_path)?;
    }
    
    // Load existing files from storage
    load_existing_files(&state, &storage_path).await?;
    
    let _network_manager = NetworkManager::new(device_manager.clone(), format!("0.0.0.0:{}", port));
    
    println!("VPS server listening on port {}", port);
    
    let listener = tokio::net::TcpListener::bind(&format!("0.0.0.0:{}", port)).await
        .map_err(|e| format!("Failed to bind to port {}: {}", port, e))?;
    
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                let state = state.clone();
                let device_manager = device_manager.clone();
                
                tokio::spawn(async move {
                    if let Err(e) = handle_client_connection(stream, state, device_manager, addr.to_string()).await {
                        eprintln!("Client connection error: {}", e);
                    }
                });
            }
            Err(e) => eprintln!("Accept error: {}", e),
        }
    }
}

async fn load_existing_files(
    state: &Arc<RwLock<ServerState>>,
    storage_path: &std::path::PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut state_guard = state.write().await;
    
    if storage_path.exists() {
        for entry in std::fs::read_dir(storage_path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                let relative_path = path.strip_prefix(storage_path)?;
                let content = std::fs::read(&path)?;
                let metadata = std::fs::metadata(&path)?;
                
                let hash = blake3::hash(&content).to_hex().to_string();
                
                let file_metadata = types::FileMetadata {
                    path: relative_path.to_path_buf(),
                    hash,
                    size: metadata.len(),
                    modified: metadata.modified()?,
                    created: metadata.created()?,
                    version: metadata.modified()?.duration_since(std::time::SystemTime::UNIX_EPOCH)?.as_secs(),
                    device_id: "vps-server".to_string(),
                };
                
                state_guard.add_file(
                    relative_path.to_string_lossy().to_string(),
                    content,
                    file_metadata,
                );
            }
        }
    }
    
    println!("Loaded {} files from storage", state_guard.files.len());
    Ok(())
}

async fn handle_client_connection(
    mut stream: tokio::net::TcpStream,
    state: Arc<RwLock<ServerState>>,
    device_manager: Arc<DeviceManager>,
    client_addr: String,
) -> Result<(), Box<dyn std::error::Error>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    
    let mut buffer = vec![0u8; 8192];
    
    loop {
        let n = stream.read(&mut buffer).await?;
        
        if n == 0 {
            println!("Client disconnected: {}", client_addr);
            break;
        }
        
        let message: NetworkMessage = serde_json::from_slice(&buffer[..n])?;
        
        match message {
            NetworkMessage::Handshake { device_id, device_name, sync_root_hash: _ } => {
                println!("Handshake from {} ({})", device_name, device_id);
                
                let device_info = types::DeviceInfo {
                    id: device_id.clone(),
                    name: device_name,
                    address: client_addr.clone(),
                    last_seen: chrono::Utc::now(),
                };
                
                device_manager.register_device(device_info).await?;
                
                // Add client to state
                state.write().await.add_client(device_id.clone(), client_addr.clone());
                
                let response = NetworkMessage::HandshakeResponse {
                    accepted: true,
                    device_info: device_manager.create_device_info("vps-server".to_string()),
                };
                
                let response_data = serde_json::to_vec(&response)?;
                stream.write_all(&response_data).await?;
            }
            
            NetworkMessage::SyncRequest { device_id, files } => {
                println!("Sync request from {} with {} files", device_id, files.len());
                
                let state_guard = state.read().await;
                let server_files = state_guard.list_files();
                
                // Calculate sync operations
                let operations = calculate_sync_operations_for_client(&files, &server_files);
                
                let response = NetworkMessage::SyncResponse { operations };
                let response_data = serde_json::to_vec(&response)?;
                stream.write_all(&response_data).await?;
            }
            
            NetworkMessage::FileRequest { path } => {
                println!("File request for: {}", path);
                
                let state_guard = state.read().await;
                let response = if let Some(content) = state_guard.get_file(&path) {
                    let metadata = state_guard.get_metadata(&path).cloned();
                    NetworkMessage::FileResponse {
                        path: path.clone(),
                        found: true,
                        content: Some(content.clone()),
                        metadata,
                    }
                } else {
                    NetworkMessage::FileResponse {
                        path: path.clone(),
                        found: false,
                        content: None,
                        metadata: None,
                    }
                };
                
                let response_data = serde_json::to_vec(&response)?;
                stream.write_all(&response_data).await?;
            }
            
            NetworkMessage::FileTransfer { path, content, metadata } => {
                println!("Legacy file transfer: {} ({} bytes)", path, content.len());
                
                // Handle legacy file transfer (for backwards compatibility)
                let mut state_guard = state.write().await;
                state_guard.add_file(path.clone(), content, metadata);
                
                // Persist to disk
                let storage_path = std::path::PathBuf::from("./vps_storage");
                if !storage_path.exists() {
                    std::fs::create_dir_all(&storage_path)?;
                }
                
                let file_path = storage_path.join(&path);
                if let Some(parent) = file_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                
                if let Some(file_content) = state_guard.get_file(&path) {
                    std::fs::write(&file_path, file_content)?;
                }
                
                println!("File stored on VPS: {}", path);
            }
            
            NetworkMessage::Heartbeat => {
                // Respond to heartbeat
                let response = NetworkMessage::Heartbeat;
                let response_data = serde_json::to_vec(&response)?;
                stream.write_all(&response_data).await?;
            }
            
            _ => {
                eprintln!("Unexpected message type from client: {}", client_addr);
            }
        }
    }
    
    Ok(())
}

fn calculate_sync_operations_for_client(
    client_files: &[types::FileMetadata],
    server_files: &[&types::FileMetadata],
) -> Vec<types::SyncOperation> {
    let mut operations = Vec::new();
    
    // Create a map of server files by path
    let server_file_map: HashMap<std::path::PathBuf, &types::FileMetadata> = server_files
        .iter()
        .map(|f| (f.path.clone(), *f))
        .collect();
    
    // Check each client file
    for client_file in client_files {
        if let Some(server_file) = server_file_map.get(&client_file.path) {
            // File exists on both sides
            if client_file.hash != server_file.hash {
                // Server version is newer
                if server_file.version > client_file.version {
                    operations.push(types::SyncOperation::Update((*server_file).clone()));
                }
            }
        } else {
            // File exists on server but not on client
            if let Some(server_file) = server_file_map.get(&client_file.path) {
                operations.push(types::SyncOperation::Add((*server_file).clone()));
            }
        }
    }
    
    // Find files that exist on server but not on client
    for server_file in server_files {
        let file_exists_on_client = client_files.iter()
            .any(|f| f.path == server_file.path);
        
        if !file_exists_on_client {
            operations.push(types::SyncOperation::Add((*server_file).clone()));
        }
    }
    
    operations
}