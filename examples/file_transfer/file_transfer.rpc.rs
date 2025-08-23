//! File transfer service definition.

use serde::{Serialize, Deserialize};

/// Request to upload a file chunk.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UploadChunkRequest {
    pub file_id: String,
    pub chunk_number: u32,
    pub total_chunks: u32,
    pub data: Vec<u8>,
}

/// Response from uploading a file chunk.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UploadChunkResponse {
    pub success: bool,
    pub bytes_received: usize,
}

/// Request to download a file chunk.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DownloadChunkRequest {
    pub file_id: String,
    pub chunk_number: u32,
}

/// Response from downloading a file chunk.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DownloadChunkResponse {
    pub data: Vec<u8>,
    pub chunk_number: u32,
    pub is_last_chunk: bool,
}

/// Request to get file information.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileInfoRequest {
    pub file_id: String,
}

/// Response with file information.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileInfoResponse {
    pub file_id: String,
    pub total_size: u64,
    pub total_chunks: u32,
    pub chunk_size: u32,
}

/// Errors that can occur in file transfer operations.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FileTransferError {
    /// File not found.
    FileNotFound,
    /// Chunk out of range.
    ChunkOutOfRange,
    /// Invalid file ID.
    InvalidFileId,
    /// Chunk too large.
    ChunkTooLarge,
    /// Storage error.
    StorageError(String),
}

/// File transfer service for handling chunked file uploads and downloads.
#[rpcnet::service]
pub trait FileTransfer {
    async fn upload_chunk(&self, request: UploadChunkRequest) -> Result<UploadChunkResponse, FileTransferError>;
    async fn download_chunk(&self, request: DownloadChunkRequest) -> Result<DownloadChunkResponse, FileTransferError>;
    async fn get_file_info(&self, request: FileInfoRequest) -> Result<FileInfoResponse, FileTransferError>;
}