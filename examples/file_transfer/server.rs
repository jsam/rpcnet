#![allow(dead_code)]
#![allow(unused_imports)]
//! File transfer server using generated code.

#[path = "generated/filetransfer/mod.rs"]
mod filetransfer;

use filetransfer::server::{FileTransferHandler, FileTransferServer};
use filetransfer::{
    DownloadChunkRequest, DownloadChunkResponse, FileInfoRequest, FileInfoResponse,
    FileTransferError, UploadChunkRequest, UploadChunkResponse,
};
use rpcnet::RpcConfig;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

// Simple in-memory file storage for demonstration
#[derive(Clone)]
struct FileData {
    chunks: Vec<Vec<u8>>,
    total_size: u64,
    chunk_size: u32,
}

struct MyFileTransferService {
    files: Arc<Mutex<HashMap<String, FileData>>>,
}

impl MyFileTransferService {
    fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl FileTransferHandler for MyFileTransferService {
    async fn upload_chunk(
        &self,
        request: UploadChunkRequest,
    ) -> Result<UploadChunkResponse, FileTransferError> {
        const MAX_CHUNK_SIZE: usize = 1024 * 1024; // 1MB

        if request.data.len() > MAX_CHUNK_SIZE {
            return Err(FileTransferError::ChunkTooLarge);
        }

        if request.file_id.is_empty() {
            return Err(FileTransferError::InvalidFileId);
        }

        let mut files = self.files.lock().await;
        let file_data = files
            .entry(request.file_id.clone())
            .or_insert_with(|| FileData {
                chunks: vec![Vec::new(); request.total_chunks as usize],
                total_size: 0,
                chunk_size: request.data.len() as u32,
            });

        if request.chunk_number >= request.total_chunks {
            return Err(FileTransferError::ChunkOutOfRange);
        }

        file_data.chunks[request.chunk_number as usize] = request.data.clone();

        // Update total size calculation
        file_data.total_size = file_data
            .chunks
            .iter()
            .map(|chunk| chunk.len() as u64)
            .sum();

        Ok(UploadChunkResponse {
            success: true,
            bytes_received: request.data.len(),
        })
    }

    async fn download_chunk(
        &self,
        request: DownloadChunkRequest,
    ) -> Result<DownloadChunkResponse, FileTransferError> {
        let files = self.files.lock().await;

        let file_data = files
            .get(&request.file_id)
            .ok_or(FileTransferError::FileNotFound)?;

        if request.chunk_number as usize >= file_data.chunks.len() {
            return Err(FileTransferError::ChunkOutOfRange);
        }

        let chunk = file_data.chunks[request.chunk_number as usize].clone();
        let is_last_chunk = request.chunk_number == (file_data.chunks.len() - 1) as u32;

        Ok(DownloadChunkResponse {
            data: chunk,
            chunk_number: request.chunk_number,
            is_last_chunk,
        })
    }

    async fn get_file_info(
        &self,
        request: FileInfoRequest,
    ) -> Result<FileInfoResponse, FileTransferError> {
        let files = self.files.lock().await;

        let file_data = files
            .get(&request.file_id)
            .ok_or(FileTransferError::FileNotFound)?;

        Ok(FileInfoResponse {
            file_id: request.file_id,
            total_size: file_data.total_size,
            total_chunks: file_data.chunks.len() as u32,
            chunk_size: file_data.chunk_size,
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== File Transfer Server (Generated Code) ===");

    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:8082")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_secs(30));

    let service = MyFileTransferService::new();
    let server = FileTransferServer::new(service, config);

    println!("Starting file transfer server on port 8082...");
    println!("Server supports chunked file upload/download");

    server.serve().await?;
    Ok(())
}
