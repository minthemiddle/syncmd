use crate::types::{DeviceInfo, SyncError};
use std::collections::HashMap;
use std::sync::Arc;
use std::net::{SocketAddr, IpAddr, Ipv4Addr, UdpSocket};
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct DeviceManager {
    devices: Arc<RwLock<HashMap<String, DeviceInfo>>>,
    device_id: String,
    device_name: String,
}

impl DeviceManager {
    pub fn new(device_name: String) -> Self {
        let device_id = Uuid::new_v4().to_string();
        Self {
            devices: Arc::new(RwLock::new(HashMap::new())),
            device_id,
            device_name,
        }
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub async fn register_device(&self, device_info: DeviceInfo) -> Result<(), SyncError> {
        let mut devices = self.devices.write().await;
        devices.insert(device_info.id.clone(), device_info);
        Ok(())
    }

    pub async fn get_device(&self, device_id: &str) -> Option<DeviceInfo> {
        let devices = self.devices.read().await;
        devices.get(device_id).cloned()
    }

    pub async fn list_devices(&self) -> Vec<DeviceInfo> {
        let devices = self.devices.read().await;
        devices.values().cloned().collect()
    }

    pub async fn remove_device(&self, device_id: &str) -> Result<(), SyncError> {
        let mut devices = self.devices.write().await;
        devices.remove(device_id);
        Ok(())
    }

    pub fn create_device_info(&self, address: String) -> DeviceInfo {
        DeviceInfo {
            id: self.device_id.clone(),
            name: self.device_name.clone(),
            address,
            last_seen: chrono::Utc::now(),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum NetworkMessage {
    Handshake {
        device_id: String,
        device_name: String,
        sync_root_hash: String,
    },
    HandshakeResponse {
        accepted: bool,
        device_info: DeviceInfo,
    },
    SyncRequest {
        device_id: String,
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
    device_manager: Arc<DeviceManager>,
    server_address: String,
}

impl NetworkManager {
    pub fn new(device_manager: Arc<DeviceManager>, server_address: String) -> Self {
        Self {
            device_manager,
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
                    let device_manager = self.device_manager.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(stream, device_manager, addr.to_string()).await {
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
        device_manager: Arc<DeviceManager>,
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
            NetworkMessage::Handshake { device_id, device_name, sync_root_hash: _ } => {
                println!("Handshake from {} ({})", device_name, device_id);
                
                let device_info = DeviceInfo {
                    id: device_id.clone(),
                    name: device_name,
                    address: client_addr,
                    last_seen: chrono::Utc::now(),
                };
                
                device_manager.register_device(device_info).await?;
                
                let response = NetworkMessage::HandshakeResponse {
                    accepted: true,
                    device_info: device_manager.create_device_info("server".to_string()),
                };
                
                let response_data = serde_json::to_vec(&response)?;
                stream.write_all(&response_data).await?;
            }
            NetworkMessage::SyncRequest { device_id, files } => {
                println!("Sync request from {} with {} files", device_id, files.len());
                
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

    pub async fn send_handshake(
        &self,
        stream: &mut tokio::net::TcpStream,
        sync_root_hash: String,
    ) -> Result<(), SyncError> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let handshake = NetworkMessage::Handshake {
            device_id: self.device_manager.device_id().to_string(),
            device_name: self.device_manager.device_name().to_string(),
            sync_root_hash,
        };

        let data = serde_json::to_vec(&handshake)?;
        stream.write_all(&data).await?;

        let mut response_buffer = vec![0u8; 1024];
        let n = stream.read(&mut response_buffer).await?;
        
        let response: NetworkMessage = serde_json::from_slice(&response_buffer[..n])?;
        
        if let NetworkMessage::HandshakeResponse { accepted, device_info } = response {
            if accepted {
                self.device_manager.register_device(device_info).await?;
                Ok(())
            } else {
                Err(SyncError::Network("Handshake rejected".to_string()))
            }
        } else {
            Err(SyncError::Network("Invalid handshake response".to_string()))
        }
    }

    pub async fn discover_peers(&self, port: u16) -> Result<Vec<SocketAddr>, SyncError> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_broadcast(true)?;
        
        let broadcast_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), port);
        let discovery_msg = format!("SYNCMD_DISCOVER:{}", self.device_manager.device_id());
        
        socket.send_to(discovery_msg.as_bytes(), broadcast_addr)?;
        
        let mut peers = Vec::new();
        let mut buffer = [0u8; 1024];
        
        // Wait for responses
        socket.set_read_timeout(Some(std::time::Duration::from_secs(2)))?;
        
        while let Ok((size, addr)) = socket.recv_from(&mut buffer) {
            if size > 0 {
                let response = String::from_utf8_lossy(&buffer[..size]);
                if response.starts_with("SYNCMD_RESPONSE:") {
                    peers.push(addr);
                }
            }
        }
        
        Ok(peers)
    }

    pub async fn start_discovery_server(&self, port: u16) -> Result<(), SyncError> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))?;
        let device_manager = self.device_manager.clone();
        
        println!("Peer discovery listening on port {}", port);
        
        tokio::spawn(async move {
            let mut buffer = [0u8; 1024];
            loop {
                match socket.recv_from(&mut buffer) {
                    Ok((size, addr)) => {
                        if size > 0 {
                            let msg = String::from_utf8_lossy(&buffer[..size]);
                            if let Some(peer_id) = msg.strip_prefix("SYNCMD_DISCOVER:") {
                                if peer_id != device_manager.device_id() {
                                    println!("Discovered peer {} from {}", peer_id, addr);
                                    let response = format!("SYNCMD_RESPONSE:{}", device_manager.device_id());
                                    let _ = socket.send_to(response.as_bytes(), addr);
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        
        Ok(())
    }
}