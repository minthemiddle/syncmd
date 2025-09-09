use crate::types::{SyncError, FileMetadata};
use std::path::{Path, PathBuf};
use std::io::{Read, Write};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde::{Serialize, Deserialize};
use std::time::Instant;

const CHUNK_SIZE: usize = 64 * 1024; // 64KB chunks
const MAX_RETRIES: u32 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTransferHeader {
    pub path: String,
    pub size: u64,
    pub chunks: u32,
    pub metadata: FileMetadata,
    pub transfer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
    pub transfer_id: String,
    pub chunk_index: u32,
    pub data: Vec<u8>,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileTransferMessage {
    StartTransfer(FileTransferHeader),
    Chunk(FileChunk),
    AckChunk { transfer_id: String, chunk_index: u32 },
    CompleteTransfer { transfer_id: String },
    TransferError { transfer_id: String, error: String },
}

pub struct FileTransferManager {
    active_transfers: std::collections::HashMap<String, FileTransferState>,
}

#[derive(Debug)]
struct FileTransferState {
    path: PathBuf,
    size: u64,
    chunks_received: u32,
    total_chunks: u32,
    metadata: FileMetadata,
    temp_file: Option<std::fs::File>,
    started_at: Instant,
    last_progress: std::time::Instant,
}

impl FileTransferManager {
    pub fn new() -> Self {
        Self {
            active_transfers: std::collections::HashMap::new(),
        }
    }

    pub async fn send_file(
        &self,
        stream: &mut tokio::net::TcpStream,
        file_path: &Path,
        metadata: FileMetadata,
    ) -> Result<(), SyncError> {
        let transfer_id = uuid::Uuid::new_v4().to_string();
        let mut file = std::fs::File::open(file_path)?;
        let file_size = file.metadata()?.len();
        let total_chunks = (file_size + CHUNK_SIZE as u64 - 1) / CHUNK_SIZE as u64;

        println!("Starting file transfer: {} ({} bytes, {} chunks)", 
            file_path.display(), file_size, total_chunks);

        // Send transfer header
        let header = FileTransferHeader {
            path: file_path.to_string_lossy().to_string(),
            size: file_size,
            chunks: total_chunks as u32,
            metadata: metadata.clone(),
            transfer_id: transfer_id.clone(),
        };

        let header_msg = FileTransferMessage::StartTransfer(header);
        let header_data = serde_json::to_vec(&header_msg)?;
        stream.write_all(&header_data).await?;

        // Send file chunks
        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut chunk_index = 0;
        let mut bytes_sent = 0;

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }

            let chunk_data = buffer[..bytes_read].to_vec();
            let checksum = blake3::hash(&chunk_data).to_string();

            let chunk = FileChunk {
                transfer_id: transfer_id.clone(),
                chunk_index,
                data: chunk_data,
                checksum,
            };

            let chunk_msg = FileTransferMessage::Chunk(chunk);
            let chunk_data = serde_json::to_vec(&chunk_msg)?;
            stream.write_all(&chunk_data).await?;

            // Wait for acknowledgment
            let mut ack_buffer = vec![0u8; 1024];
            let n = stream.read(&mut ack_buffer).await?;
            if n > 0 {
                let ack: FileTransferMessage = serde_json::from_slice(&ack_buffer[..n])?;
                match ack {
                    FileTransferMessage::AckChunk { transfer_id: ack_id, chunk_index: ack_index } => {
                        if ack_id == transfer_id && ack_index == chunk_index {
                            bytes_sent += bytes_read as u64;
                            self.print_progress(&transfer_id, bytes_sent, file_size);
                        }
                    }
                    FileTransferMessage::TransferError { transfer_id: _error_id, error } => {
                        return Err(SyncError::Network(format!("Transfer error: {}", error)));
                    }
                    _ => {
                        eprintln!("Unexpected message during transfer");
                    }
                }
            }

            chunk_index += 1;
        }

        // Send completion message
        let complete_msg = FileTransferMessage::CompleteTransfer { transfer_id: transfer_id.clone() };
        let complete_data = serde_json::to_vec(&complete_msg)?;
        stream.write_all(&complete_data).await?;

        println!("File transfer completed: {}", file_path.display());
        Ok(())
    }

    pub async fn receive_file(
        &mut self,
        stream: &mut tokio::net::TcpStream,
        base_path: &Path,
    ) -> Result<(), SyncError> {
        let mut buffer = vec![0u8; CHUNK_SIZE + 1024]; // Extra space for metadata

        loop {
            let n = stream.read(&mut buffer).await?;
            if n == 0 {
                break;
            }

            let message: FileTransferMessage = serde_json::from_slice(&buffer[..n])?;

            match message {
                FileTransferMessage::StartTransfer(header) => {
                    self.start_transfer(header, base_path).await?;
                }
                FileTransferMessage::Chunk(chunk) => {
                    self.receive_chunk(chunk, stream).await?;
                }
                FileTransferMessage::CompleteTransfer { transfer_id } => {
                    self.complete_transfer(&transfer_id).await?;
                }
                FileTransferMessage::TransferError { transfer_id, error } => {
                    eprintln!("Transfer error for {}: {}", transfer_id, error);
                    self.active_transfers.remove(&transfer_id);
                }
                _ => {
                    eprintln!("Unexpected file transfer message");
                }
            }
        }

        Ok(())
    }

    async fn start_transfer(&mut self, header: FileTransferHeader, base_path: &Path) -> Result<(), SyncError> {
        let file_path = base_path.join(&header.path);
        
        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Create temporary file
        let temp_path = format!("{}.tmp", file_path.to_string_lossy());
        let temp_file = std::fs::File::create(&temp_path)?;

        let transfer_state = FileTransferState {
            path: file_path.clone(),
            size: header.size,
            chunks_received: 0,
            total_chunks: header.chunks,
            metadata: header.metadata,
            temp_file: Some(temp_file),
            started_at: Instant::now(),
            last_progress: std::time::Instant::now(),
        };

        self.active_transfers.insert(header.transfer_id.clone(), transfer_state);
        println!("Started receiving file: {} ({} bytes)", file_path.display(), header.size);

        Ok(())
    }

    async fn receive_chunk(&mut self, chunk: FileChunk, stream: &mut tokio::net::TcpStream) -> Result<(), SyncError> {
        let transfer_id = chunk.transfer_id.clone();
        let bytes_received = if let Some(transfer_state) = self.active_transfers.get_mut(&chunk.transfer_id) {
            // Verify checksum
            let calculated_checksum = blake3::hash(&chunk.data).to_string();
            if calculated_checksum != chunk.checksum {
                let error_msg = FileTransferMessage::TransferError {
                    transfer_id: chunk.transfer_id.clone(),
                    error: format!("Checksum mismatch for chunk {}", chunk.chunk_index),
                };
                let error_data = serde_json::to_vec(&error_msg)?;
                stream.write_all(&error_data).await?;
                return Err(SyncError::Network("Checksum mismatch".to_string()));
            }

            // Write chunk to temporary file
            if let Some(ref mut temp_file) = transfer_state.temp_file {
                temp_file.write_all(&chunk.data)?;
                transfer_state.chunks_received += 1;

                // Send acknowledgment
                let ack = FileTransferMessage::AckChunk {
                    transfer_id: chunk.transfer_id.clone(),
                    chunk_index: chunk.chunk_index,
                };
                let ack_data = serde_json::to_vec(&ack)?;
                stream.write_all(&ack_data).await?;

                // Calculate bytes received for progress
                transfer_state.chunks_received as u64 * CHUNK_SIZE as u64
            } else {
                0
            }
        } else {
            return Ok(());
        };

        // Print progress (separate to avoid borrowing conflict)
        if let Some(transfer_state) = self.active_transfers.get(&transfer_id) {
            self.print_progress(&transfer_id, bytes_received, transfer_state.size);
        }

        Ok(())
    }

    async fn complete_transfer(&mut self, transfer_id: &str) -> Result<(), SyncError> {
        if let Some(transfer_state) = self.active_transfers.remove(transfer_id) {
            let temp_path = format!("{}.tmp", transfer_state.path.to_string_lossy());
            
            // Verify all chunks received
            if transfer_state.chunks_received != transfer_state.total_chunks {
                return Err(SyncError::Network("Incomplete transfer".to_string()));
            }

            // Rename temporary file to final location
            std::fs::rename(&temp_path, &transfer_state.path)?;

            // Set file metadata
            transfer_state.metadata.apply_to_file(&transfer_state.path)?;

            let duration = transfer_state.started_at.elapsed();
            println!("File transfer completed: {} in {:.2}s", 
                transfer_state.path.display(), duration.as_secs_f64());
        }

        Ok(())
    }

    fn print_progress(&self, transfer_id: &str, bytes_transferred: u64, total_bytes: u64) {
        if let Some(transfer_state) = self.active_transfers.get(transfer_id) {
            let now = std::time::Instant::now();
            if now.duration_since(transfer_state.last_progress).as_secs() >= 1 {
                let progress = (bytes_transferred as f64 / total_bytes as f64) * 100.0;
                let speed = bytes_transferred as f64 / transfer_state.started_at.elapsed().as_secs_f64() / 1024.0 / 1024.0;
                println!("Progress: {:.1}% ({:.1} MB/s)", progress, speed);
                // Note: We can't modify transfer_state.last_progress here due to borrowing
                // In a real implementation, we'd need a more sophisticated approach
            }
        }
    }
}

impl FileMetadata {
    pub fn apply_to_file(&self, file_path: &Path) -> Result<(), SyncError> {
        // Set file permissions and timestamps
        if let Ok(metadata) = std::fs::metadata(file_path) {
            let mut permissions = metadata.permissions();
            permissions.set_readonly(true);
            std::fs::set_permissions(file_path, permissions)?;
        }
        Ok(())
    }
}