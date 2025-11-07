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
        let params = rmp_serde::to_vec(&request)?;
        let response_data = self.inner.call("FileTransfer.upload_chunk", params).await?;
        rmp_serde::from_slice::<UploadChunkResponse>(&response_data).map_err(Into::into)
    }
    pub async fn download_chunk(
        &self,
        request: DownloadChunkRequest,
    ) -> Result<DownloadChunkResponse, RpcError> {
        let params = rmp_serde::to_vec(&request)?;
        let response_data = self
            .inner
            .call("FileTransfer.download_chunk", params)
            .await?;
        rmp_serde::from_slice::<DownloadChunkResponse>(&response_data).map_err(Into::into)
    }
    pub async fn get_file_info(
        &self,
        request: FileInfoRequest,
    ) -> Result<FileInfoResponse, RpcError> {
        let params = rmp_serde::to_vec(&request)?;
        let response_data = self
            .inner
            .call("FileTransfer.get_file_info", params)
            .await?;
        rmp_serde::from_slice::<FileInfoResponse>(&response_data).map_err(Into::into)
    }
}
