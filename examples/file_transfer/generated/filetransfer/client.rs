use super::types::*;
use rpcnet::{RpcClient, RpcConfig, RpcError};
use std::net::SocketAddr;
/// Generated client for calling service methods.
pub struct FileTransferClient {
    inner: RpcClient,
}
impl FileTransferClient {
    /// Connects to the service at the given address.
    pub async fn connect(addr: SocketAddr, config: RpcConfig) -> Result<Self, RpcError> {
        let inner = RpcClient::connect(addr, config).await?;
        Ok(Self { inner })
    }
    pub async fn upload_chunk(
        &self,
        request: UploadChunkRequest,
    ) -> Result<UploadChunkResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self.inner.call("FileTransfer.upload_chunk", params).await?;
        bincode::deserialize::<UploadChunkResponse>(&response_data)
            .map_err(RpcError::SerializationError)
    }
    pub async fn download_chunk(
        &self,
        request: DownloadChunkRequest,
    ) -> Result<DownloadChunkResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self
            .inner
            .call("FileTransfer.download_chunk", params)
            .await?;
        bincode::deserialize::<DownloadChunkResponse>(&response_data)
            .map_err(RpcError::SerializationError)
    }
    pub async fn get_file_info(
        &self,
        request: FileInfoRequest,
    ) -> Result<FileInfoResponse, RpcError> {
        let params = bincode::serialize(&request).map_err(RpcError::SerializationError)?;
        let response_data = self
            .inner
            .call("FileTransfer.get_file_info", params)
            .await?;
        bincode::deserialize::<FileInfoResponse>(&response_data)
            .map_err(RpcError::SerializationError)
    }
}
