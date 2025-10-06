#![allow(dead_code)]
#![allow(unused_imports)]
//! File transfer client using generated code.

#[path = "generated/filetransfer/mod.rs"]
mod filetransfer;

use filetransfer::client::FileTransferClient;
use filetransfer::{DownloadChunkRequest, FileInfoRequest, UploadChunkRequest};
use rpcnet::RpcConfig;
use std::net::SocketAddr;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== File Transfer Client (Generated Code) ===");

    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:0")
        .with_server_name("localhost")
        .with_keep_alive_interval(Duration::from_secs(30));

    let server_addr: SocketAddr = "127.0.0.1:8082".parse()?;

    println!("Connecting to file transfer service at {}", server_addr);
    let client = FileTransferClient::connect(server_addr, config).await?;
    println!("Connected successfully!");

    // Test file upload in chunks
    println!("\n--- Testing File Upload ---");
    let file_id = "test_file_001";
    let test_data = b"This is a test file content that will be split into chunks for transfer.";
    let chunk_size = 20; // Small chunks for testing
    let total_chunks = test_data.len().div_ceil(chunk_size);

    println!(
        "Uploading file '{}' in {} chunks of {} bytes each",
        file_id, total_chunks, chunk_size
    );

    for chunk_num in 0..total_chunks {
        let start = chunk_num * chunk_size;
        let end = std::cmp::min(start + chunk_size, test_data.len());
        let chunk_data = test_data[start..end].to_vec();

        let upload_req = UploadChunkRequest {
            file_id: file_id.to_string(),
            chunk_number: chunk_num as u32,
            total_chunks: total_chunks as u32,
            data: chunk_data.clone(),
        };

        match client.upload_chunk(upload_req).await {
            Ok(response) => {
                println!(
                    "  Chunk {} uploaded: {} bytes",
                    chunk_num, response.bytes_received
                );
            }
            Err(e) => {
                println!("  Chunk {} upload failed: {}", chunk_num, e);
                return Ok(());
            }
        }
    }

    // Test file info retrieval
    println!("\n--- Testing File Info ---");
    let info_req = FileInfoRequest {
        file_id: file_id.to_string(),
    };
    match client.get_file_info(info_req).await {
        Ok(info) => {
            println!("File info:");
            println!("  ID: {}", info.file_id);
            println!("  Total size: {} bytes", info.total_size);
            println!("  Total chunks: {}", info.total_chunks);
            println!("  Chunk size: {} bytes", info.chunk_size);
        }
        Err(e) => println!("Failed to get file info: {}", e),
    }

    // Test file download
    println!("\n--- Testing File Download ---");
    let mut downloaded_data = Vec::new();
    let mut chunk_num = 0;

    loop {
        let download_req = DownloadChunkRequest {
            file_id: file_id.to_string(),
            chunk_number: chunk_num,
        };

        match client.download_chunk(download_req).await {
            Ok(response) => {
                downloaded_data.extend_from_slice(&response.data);
                println!(
                    "  Downloaded chunk {}: {} bytes",
                    response.chunk_number,
                    response.data.len()
                );

                if response.is_last_chunk {
                    break;
                }
                chunk_num += 1;
            }
            Err(e) => {
                println!("  Download failed at chunk {}: {}", chunk_num, e);
                break;
            }
        }
    }

    // Verify download
    if downloaded_data == test_data {
        println!(
            "✅ File download successful! {} bytes received",
            downloaded_data.len()
        );
        println!("✅ Data integrity verified");
    } else {
        println!("❌ File download verification failed");
        println!("   Original: {} bytes", test_data.len());
        println!("   Downloaded: {} bytes", downloaded_data.len());
    }

    println!("\nAll tests completed!");
    Ok(())
}
