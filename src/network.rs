#![allow(dead_code)]

use crate::types::{ClientInfo, SyncError};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct ClientManager {
    clients: Arc<RwLock<HashMap<String, ClientInfo>>>,
    server_id: String,
    auth_tokens: HashMap<String, String>, // token -> client_id
}

impl ClientManager {
    pub fn new() -> Self {
        let server_id = Uuid::new_v4().to_string();
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            server_id,
            auth_tokens: HashMap::new(),
        }
    }

    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    pub fn generate_auth_token(&mut self, client_id: String, _client_name: String) -> String {
        let token = format!("syncmd_{}", Uuid::new_v4().to_string());
        self.auth_tokens.insert(token.clone(), client_id.clone());
        token
    }

    pub fn validate_token(&self, token: &str) -> Option<String> {
        self.auth_tokens.get(token).cloned()
    }

    pub async fn register_client(&self, client_info: ClientInfo) -> Result<(), SyncError> {
        let mut clients = self.clients.write().await;
        clients.insert(client_info.id.clone(), client_info);
        Ok(())
    }

    pub async fn get_client(&self, client_id: &str) -> Option<ClientInfo> {
        let clients = self.clients.read().await;
        clients.get(client_id).cloned()
    }

    pub async fn list_clients(&self) -> Vec<ClientInfo> {
        let clients = self.clients.read().await;
        clients.values().cloned().collect()
    }

    pub async fn remove_client(&self, client_id: &str) -> Result<(), SyncError> {
        let mut clients = self.clients.write().await;
        clients.remove(client_id);
        Ok(())
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum NetworkMessage {
    Authenticate {
        token: String,
        client_name: String,
    },
    AuthResponse {
        success: bool,
        client_id: Option<String>,
        message: String,
    },
    SyncRequest {
        client_id: String,
        files: Vec<crate::types::FileMetadata>,
    },
    SyncResponse {
        operations: Vec<crate::types::SyncOperation>,
    },
    FileTransfer {
        path: String,
        content: Vec<u8>,
        metadata: crate::types::FileMetadata,
    },
    FileRequest {
        path: String,
    },
    FileResponse {
        path: String,
        found: bool,
        content: Option<Vec<u8>>,
        metadata: Option<crate::types::FileMetadata>,
    },
    Heartbeat,
}

#[derive(Clone)]
pub struct NetworkManager {
    client_manager: Arc<ClientManager>,
    server_address: String,
}

impl NetworkManager {
    pub fn new(client_manager: Arc<ClientManager>, server_address: String) -> Self {
        Self {
            client_manager,
            server_address,
        }
    }

    pub async fn start_server(&self) -> Result<(), SyncError> {
        // Simple TCP server for MVP
        let listener = tokio::net::TcpListener::bind(&self.server_address).await
            .map_err(|e| SyncError::Network(e.to_string()))?;

        println!("Server listening on {}", self.server_address);

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    let client_manager = self.client_manager.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, client_manager, addr.to_string()).await {
                            eprintln!("Connection error: {}", e);
                        }
                    });
                }
                Err(e) => eprintln!("Accept error: {}", e),
            }
        }
    }

    async fn handle_connection(
        mut stream: tokio::net::TcpStream,
        client_manager: Arc<ClientManager>,
        client_addr: String,
    ) -> Result<(), SyncError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let mut buffer = vec![0u8; 1024];
        let n = stream.read(&mut buffer).await?;
        
        if n == 0 {
            return Ok(());
        }

        let message: NetworkMessage = serde_json::from_slice(&buffer[..n])?;
        
        match message {
            NetworkMessage::Authenticate { token, client_name } => {
                println!("Authentication request from: {}", client_name);
                
                if let Some(client_id) = client_manager.validate_token(&token) {
                    println!("Authentication successful for client: {}", client_id);
                    
                    let client_info = ClientInfo {
                        id: client_id.clone(),
                        name: client_name,
                        address: client_addr,
                        last_seen: chrono::Utc::now(),
                        auth_token: token,
                    };
                    
                    client_manager.register_client(client_info).await?;
                    
                    let response = NetworkMessage::AuthResponse {
                        success: true,
                        client_id: Some(client_id),
                        message: "Authentication successful".to_string(),
                    };
                    
                    let response_data = serde_json::to_vec(&response)?;
                    stream.write_all(&response_data).await?;
                } else {
                    println!("Authentication failed for client: {}", client_name);
                    
                    let response = NetworkMessage::AuthResponse {
                        success: false,
                        client_id: None,
                        message: "Invalid authentication token".to_string(),
                    };
                    
                    let response_data = serde_json::to_vec(&response)?;
                    stream.write_all(&response_data).await?;
                }
            }
            NetworkMessage::SyncRequest { client_id, files } => {
                println!("Sync request from {} with {} files", client_id, files.len());
                
                // Get server's current file state
                // For now, we'll just acknowledge the sync request
                let response = NetworkMessage::SyncResponse {
                    operations: vec![],
                };
                
                let response_data = serde_json::to_vec(&response)?;
                stream.write_all(&response_data).await?;
            }
            NetworkMessage::FileRequest { path } => {
                println!("File request for: {}", path);
                // Handle file requests (for VPS server, this would be from storage)
                let response = NetworkMessage::FileResponse {
                    path: path.clone(),
                    found: false,
                    content: None,
                    metadata: None,
                };
                
                let response_data = serde_json::to_vec(&response)?;
                stream.write_all(&response_data).await?;
            }
            NetworkMessage::FileTransfer { path, content, metadata: _ } => {
                println!("File transfer: {} ({} bytes)", path, content.len());
                // Handle incoming file transfer (client to server)
            }
            NetworkMessage::Heartbeat => {
                // Handle heartbeat
            }
            _ => {
                eprintln!("Unexpected message type");
            }
        }

        Ok(())
    }

    pub async fn connect_to_server(
        &self,
        server_addr: &str,
    ) -> Result<tokio::net::TcpStream, SyncError> {
        let stream = tokio::net::TcpStream::connect(server_addr).await
            .map_err(|e| SyncError::Network(e.to_string()))?;
        Ok(stream)
    }

    pub async fn send_authentication(
        &self,
        stream: &mut tokio::net::TcpStream,
        auth_token: String,
        client_name: String,
    ) -> Result<(), SyncError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let auth_request = NetworkMessage::Authenticate {
            token: auth_token,
            client_name,
        };

        let data = serde_json::to_vec(&auth_request)?;
        stream.write_all(&data).await?;

        let mut response_buffer = vec![0u8; 1024];
        let n = stream.read(&mut response_buffer).await?;
        
        let response: NetworkMessage = serde_json::from_slice(&response_buffer[..n])?;
        
        if let NetworkMessage::AuthResponse { success, client_id: _, message } = response {
            if success {
                println!("{}", message);
                Ok(())
            } else {
                Err(SyncError::Network(message))
            }
        } else {
            Err(SyncError::Network("Invalid authentication response".to_string()))
        }
    }
}