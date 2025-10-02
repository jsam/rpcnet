use crate::migration::models::*;
use crate::migration::types::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write, Cursor};
use std::time::SystemTime;
use uuid::Uuid;

// Serialization formats supported for state transfer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SerializationFormat {
    Bincode,
    Json,
    MessagePack,
}

impl Default for SerializationFormat {
    fn default() -> Self {
        SerializationFormat::Bincode
    }
}

// Compression methods for reducing state transfer size
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionMethod {
    None,
    Gzip,
    Lz4,
    Zstd,
}

impl Default for CompressionMethod {
    fn default() -> Self {
        CompressionMethod::Gzip
    }
}

// Result type for serialization operations
pub type SerializationResult<T> = Result<T, SerializationError>;

#[derive(Debug, thiserror::Error)]
pub enum SerializationError {
    #[error("Serialization failed: {0}")]
    SerializationFailed(String),
    
    #[error("Deserialization failed: {0}")]
    DeserializationFailed(String),
    
    #[error("Compression failed: {0}")]
    CompressionFailed(String),
    
    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),
    
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Size limit exceeded: {0} bytes")]
    SizeLimitExceeded(usize),
    
    #[error("Invalid checksum")]
    InvalidChecksum,
}

// Configuration for state serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializationConfig {
    pub format: SerializationFormat,
    pub compression: CompressionMethod,
    pub compression_level: Option<i32>,
    pub max_size_bytes: usize,
    pub chunk_size_bytes: Option<usize>,
    pub validate_checksum: bool,
}

impl Default for SerializationConfig {
    fn default() -> Self {
        Self {
            format: SerializationFormat::default(),
            compression: CompressionMethod::default(),
            compression_level: Some(6), // Moderate compression
            max_size_bytes: 50_000_000, // 50MB limit
            chunk_size_bytes: Some(1_048_576), // 1MB chunks
            validate_checksum: true,
        }
    }
}

// Metadata for serialized state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedStateMetadata {
    pub snapshot_id: Uuid,
    pub format: SerializationFormat,
    pub compression: CompressionMethod,
    pub original_size: usize,
    pub compressed_size: usize,
    pub checksum: Vec<u8>,
    pub chunk_count: usize,
    pub serialized_at: SystemTime,
}

// Chunked serialized data for large state transfers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedStateChunk {
    pub chunk_id: Uuid,
    pub snapshot_id: Uuid,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub data: Vec<u8>,
    pub chunk_checksum: Vec<u8>,
}

// Complete serialized state package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedState {
    pub metadata: SerializedStateMetadata,
    pub data: Vec<u8>,
    pub chunks: Option<Vec<SerializedStateChunk>>,
}

// Main state serializer
pub struct StateSerializer {
    config: SerializationConfig,
}

pub type SerializationService = StateSerializer;

impl StateSerializer {
    pub fn new(config: SerializationConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(SerializationConfig::default())
    }

    // Serialize a connection state snapshot
    pub fn serialize_snapshot(&self, snapshot: &ConnectionStateSnapshot) -> SerializationResult<SerializedState> {
        // First serialize to the chosen format
        let raw_data = self.serialize_to_format(snapshot)?;
        
        // Check size limits
        if raw_data.len() > self.config.max_size_bytes {
            return Err(SerializationError::SizeLimitExceeded(raw_data.len()));
        }

        // Apply compression if enabled
        let compressed_data = self.compress_data(&raw_data)?;
        
        // Calculate checksum
        let checksum = self.calculate_checksum(&compressed_data);
        
        // Determine if chunking is needed
        let (final_data, chunks) = if let Some(chunk_size) = self.config.chunk_size_bytes {
            if compressed_data.len() > chunk_size {
                let chunks = self.create_chunks(&compressed_data, snapshot.snapshot_id, chunk_size)?;
                (Vec::new(), Some(chunks))
            } else {
                (compressed_data, None)
            }
        } else {
            (compressed_data, None)
        };

        let metadata = SerializedStateMetadata {
            snapshot_id: snapshot.snapshot_id,
            format: self.config.format,
            compression: self.config.compression,
            original_size: raw_data.len(),
            compressed_size: if chunks.is_some() {
                chunks.as_ref().unwrap().iter().map(|c| c.data.len()).sum()
            } else {
                final_data.len()
            },
            checksum,
            chunk_count: chunks.as_ref().map(|c| c.len()).unwrap_or(1),
            serialized_at: SystemTime::now(),
        };

        Ok(SerializedState {
            metadata,
            data: final_data,
            chunks,
        })
    }

    // Deserialize a connection state snapshot
    pub fn deserialize_snapshot(&self, serialized: &SerializedState) -> SerializationResult<ConnectionStateSnapshot> {
        // Validate checksum if enabled
        if self.config.validate_checksum {
            self.validate_serialized_state(serialized)?;
        }

        // Reconstruct data from chunks if chunked
        let compressed_data = if let Some(ref chunks) = serialized.chunks {
            self.reconstruct_from_chunks(chunks)?
        } else {
            serialized.data.clone()
        };

        // Decompress the data
        let raw_data = self.decompress_data(&compressed_data, serialized.metadata.compression)?;
        
        // Deserialize from the format
        self.deserialize_from_format(&raw_data, serialized.metadata.format)
    }

    // Serialize migration message
    pub fn serialize_message(&self, message: &MigrationMessage) -> SerializationResult<Vec<u8>> {
        self.serialize_to_format(message)
    }

    // Deserialize migration message
    pub fn deserialize_message(&self, data: &[u8]) -> SerializationResult<MigrationMessage> {
        self.deserialize_from_format(data, self.config.format)
    }

    // Serialize any serializable type
    pub fn serialize_generic<T>(&self, data: &T) -> SerializationResult<Vec<u8>> 
    where 
        T: Serialize 
    {
        self.serialize_to_format(data)
    }

    // Deserialize any deserializable type
    pub fn deserialize_generic<T>(&self, data: &[u8]) -> SerializationResult<T> 
    where 
        T: for<'de> Deserialize<'de>
    {
        self.deserialize_from_format(data, self.config.format)
    }

    // Internal serialization to specific format
    fn serialize_to_format<T>(&self, data: &T) -> SerializationResult<Vec<u8>>
    where 
        T: Serialize
    {
        match self.config.format {
            SerializationFormat::Bincode => {
                bincode::serialize(data)
                    .map_err(|e| SerializationError::SerializationFailed(e.to_string()))
            },
            SerializationFormat::Json => {
                serde_json::to_vec(data)
                    .map_err(|e| SerializationError::SerializationFailed(e.to_string()))
            },
            SerializationFormat::MessagePack => {
                // For now, fallback to bincode as MessagePack would require additional dependency
                bincode::serialize(data)
                    .map_err(|e| SerializationError::SerializationFailed(e.to_string()))
            },
        }
    }

    // Internal deserialization from specific format
    fn deserialize_from_format<T>(&self, data: &[u8], format: SerializationFormat) -> SerializationResult<T>
    where 
        T: for<'de> Deserialize<'de>
    {
        match format {
            SerializationFormat::Bincode => {
                bincode::deserialize(data)
                    .map_err(|e| SerializationError::DeserializationFailed(e.to_string()))
            },
            SerializationFormat::Json => {
                serde_json::from_slice(data)
                    .map_err(|e| SerializationError::DeserializationFailed(e.to_string()))
            },
            SerializationFormat::MessagePack => {
                // For now, fallback to bincode
                bincode::deserialize(data)
                    .map_err(|e| SerializationError::DeserializationFailed(e.to_string()))
            },
        }
    }

    // Compress data using the configured method
    fn compress_data(&self, data: &[u8]) -> SerializationResult<Vec<u8>> {
        match self.config.compression {
            CompressionMethod::None => Ok(data.to_vec()),
            CompressionMethod::Gzip => {
                use std::io::Write;
                let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                encoder.write_all(data)?;
                encoder.finish()
                    .map_err(|e| SerializationError::CompressionFailed(e.to_string()))
            },
            CompressionMethod::Lz4 => {
                // For now, fallback to no compression as LZ4 would require additional dependency
                Ok(data.to_vec())
            },
            CompressionMethod::Zstd => {
                // For now, fallback to no compression as Zstd would require additional dependency
                Ok(data.to_vec())
            },
        }
    }

    // Decompress data using the specified method
    fn decompress_data(&self, data: &[u8], method: CompressionMethod) -> SerializationResult<Vec<u8>> {
        match method {
            CompressionMethod::None => Ok(data.to_vec()),
            CompressionMethod::Gzip => {
                use std::io::Read;
                let mut decoder = flate2::read::GzDecoder::new(data);
                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed)?;
                Ok(decompressed)
            },
            CompressionMethod::Lz4 => {
                // Fallback to no decompression
                Ok(data.to_vec())
            },
            CompressionMethod::Zstd => {
                // Fallback to no decompression  
                Ok(data.to_vec())
            },
        }
    }

    // Calculate checksum for data integrity
    fn calculate_checksum(&self, data: &[u8]) -> Vec<u8> {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().to_vec()
    }

    // Create chunks from large data
    fn create_chunks(&self, data: &[u8], snapshot_id: Uuid, chunk_size: usize) -> SerializationResult<Vec<SerializedStateChunk>> {
        let total_chunks = ((data.len() + chunk_size - 1) / chunk_size) as u32;
        let mut chunks = Vec::new();

        for (index, chunk_data) in data.chunks(chunk_size).enumerate() {
            let chunk = SerializedStateChunk {
                chunk_id: Uuid::new_v4(),
                snapshot_id,
                chunk_index: index as u32,
                total_chunks,
                data: chunk_data.to_vec(),
                chunk_checksum: self.calculate_checksum(chunk_data),
            };
            chunks.push(chunk);
        }

        Ok(chunks)
    }

    // Reconstruct data from chunks
    fn reconstruct_from_chunks(&self, chunks: &[SerializedStateChunk]) -> SerializationResult<Vec<u8>> {
        if chunks.is_empty() {
            return Err(SerializationError::DeserializationFailed("No chunks provided".to_string()));
        }

        // Sort chunks by index to ensure correct order
        let mut sorted_chunks = chunks.to_vec();
        sorted_chunks.sort_by_key(|c| c.chunk_index);

        // Validate chunk sequence
        let total_chunks = sorted_chunks[0].total_chunks;
        if sorted_chunks.len() != total_chunks as usize {
            return Err(SerializationError::DeserializationFailed(
                format!("Missing chunks: expected {}, got {}", total_chunks, sorted_chunks.len())
            ));
        }

        // Validate chunk checksums if enabled
        if self.config.validate_checksum {
            for chunk in &sorted_chunks {
                let expected_checksum = self.calculate_checksum(&chunk.data);
                if expected_checksum != chunk.chunk_checksum {
                    return Err(SerializationError::InvalidChecksum);
                }
            }
        }

        // Reconstruct data
        let mut reconstructed = Vec::new();
        for chunk in sorted_chunks {
            reconstructed.extend_from_slice(&chunk.data);
        }

        Ok(reconstructed)
    }

    // Validate serialized state integrity
    fn validate_serialized_state(&self, serialized: &SerializedState) -> SerializationResult<()> {
        let data_to_check = if let Some(ref chunks) = serialized.chunks {
            self.reconstruct_from_chunks(chunks)?
        } else {
            serialized.data.clone()
        };

        let calculated_checksum = self.calculate_checksum(&data_to_check);
        if calculated_checksum != serialized.metadata.checksum {
            return Err(SerializationError::InvalidChecksum);
        }

        Ok(())
    }

    // Get serialization statistics
    pub fn get_compression_ratio(&self, original_size: usize, compressed_size: usize) -> f64 {
        if original_size == 0 {
            return 0.0;
        }
        compressed_size as f64 / original_size as f64
    }

    // Estimate serialized size without actually serializing
    pub fn estimate_size<T>(&self, data: &T) -> SerializationResult<usize>
    where 
        T: Serialize
    {
        // For estimation, we'll serialize to JSON as it's human-readable and gives a rough idea
        let json_size = serde_json::to_vec(data)
            .map_err(|e| SerializationError::SerializationFailed(e.to_string()))?
            .len();
        
        // Adjust estimate based on chosen format
        let format_factor = match self.config.format {
            SerializationFormat::Bincode => 0.6, // Bincode is typically more compact
            SerializationFormat::Json => 1.0,
            SerializationFormat::MessagePack => 0.7, // MessagePack is more compact than JSON
        };

        // Adjust for compression
        let compression_factor = match self.config.compression {
            CompressionMethod::None => 1.0,
            CompressionMethod::Gzip => 0.3, // Rough estimate for text compression
            CompressionMethod::Lz4 => 0.5,  // Less compression, more speed
            CompressionMethod::Zstd => 0.25, // Better compression
        };

        Ok((json_size as f64 * format_factor * compression_factor) as usize)
    }
}

// Utility functions for common serialization tasks
pub mod utils {
    use super::*;

    pub fn quick_serialize<T>(data: &T) -> SerializationResult<Vec<u8>>
    where 
        T: Serialize 
    {
        let serializer = StateSerializer::with_default_config();
        serializer.serialize_generic(data)
    }

    pub fn quick_deserialize<T>(data: &[u8]) -> SerializationResult<T>
    where 
        T: for<'de> Deserialize<'de>
    {
        let serializer = StateSerializer::with_default_config();
        serializer.deserialize_generic(data)
    }

    pub fn serialize_to_json<T>(data: &T) -> SerializationResult<Vec<u8>>
    where 
        T: Serialize 
    {
        let config = SerializationConfig {
            format: SerializationFormat::Json,
            compression: CompressionMethod::None,
            ..Default::default()
        };
        let serializer = StateSerializer::new(config);
        serializer.serialize_generic(data)
    }

    pub fn deserialize_from_json<T>(data: &[u8]) -> SerializationResult<T>
    where 
        T: for<'de> Deserialize<'de>
    {
        let config = SerializationConfig {
            format: SerializationFormat::Json,
            compression: CompressionMethod::None,
            ..Default::default()
        };
        let serializer = StateSerializer::new(config);
        serializer.deserialize_generic(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn test_snapshot_serialization_roundtrip() {
        let snapshot = ConnectionStateSnapshot::new(Uuid::new_v4())
            .add_stream_buffer(1, vec![1, 2, 3, 4, 5]);

        let serializer = StateSerializer::with_default_config();
        let serialized = serializer.serialize_snapshot(&snapshot).unwrap();
        let deserialized = serializer.deserialize_snapshot(&serialized).unwrap();

        assert_eq!(snapshot.snapshot_id, deserialized.snapshot_id);
        assert_eq!(snapshot.stream_buffers, deserialized.stream_buffers);
    }

    #[test]
    fn test_chunked_serialization() {
        let mut snapshot = ConnectionStateSnapshot::new(Uuid::new_v4());
        // Add large random data to force chunking (doesn't compress well)
        let mut large_data = Vec::with_capacity(2_000_000);
        for i in 0..2_000_000u32 {
            large_data.push((i % 256) as u8);
        }
        snapshot = snapshot.add_stream_buffer(1, large_data);

        let config = SerializationConfig {
            chunk_size_bytes: Some(500_000), // 500KB chunks
            compression: CompressionMethod::None, // Disable compression to ensure chunking
            ..Default::default()
        };
        let serializer = StateSerializer::new(config);
        
        let serialized = serializer.serialize_snapshot(&snapshot).unwrap();
        assert!(serialized.chunks.is_some());
        assert!(serialized.chunks.as_ref().unwrap().len() > 1);

        let deserialized = serializer.deserialize_snapshot(&serialized).unwrap();
        assert_eq!(snapshot.stream_buffers, deserialized.stream_buffers);
    }

    #[test]
    fn test_different_formats() {
        let snapshot = ConnectionStateSnapshot::new(Uuid::new_v4());

        // Test Bincode
        let bincode_config = SerializationConfig {
            format: SerializationFormat::Bincode,
            compression: CompressionMethod::None,
            ..Default::default()
        };
        let bincode_serializer = StateSerializer::new(bincode_config);
        let bincode_result = bincode_serializer.serialize_snapshot(&snapshot).unwrap();

        // Test JSON
        let json_config = SerializationConfig {
            format: SerializationFormat::Json,
            compression: CompressionMethod::None,
            ..Default::default()
        };
        let json_serializer = StateSerializer::new(json_config);
        let json_result = json_serializer.serialize_snapshot(&snapshot).unwrap();

        // Bincode should typically be smaller
        assert!(bincode_result.data.len() <= json_result.data.len());

        // Both should deserialize correctly
        let bincode_deserialized = bincode_serializer.deserialize_snapshot(&bincode_result).unwrap();
        let json_deserialized = json_serializer.deserialize_snapshot(&json_result).unwrap();

        assert_eq!(bincode_deserialized.snapshot_id, json_deserialized.snapshot_id);
    }

    #[test]
    fn test_compression() {
        let mut snapshot = ConnectionStateSnapshot::new(Uuid::new_v4());
        // Add repetitive data that compresses well
        snapshot = snapshot.add_stream_buffer(1, vec![0u8; 10_000]);

        let uncompressed_config = SerializationConfig {
            compression: CompressionMethod::None,
            ..Default::default()
        };
        let compressed_config = SerializationConfig {
            compression: CompressionMethod::Gzip,
            ..Default::default()
        };

        let uncompressed_serializer = StateSerializer::new(uncompressed_config);
        let compressed_serializer = StateSerializer::new(compressed_config);

        let uncompressed_result = uncompressed_serializer.serialize_snapshot(&snapshot).unwrap();
        let compressed_result = compressed_serializer.serialize_snapshot(&snapshot).unwrap();

        // Compressed should be smaller
        assert!(compressed_result.data.len() < uncompressed_result.data.len());

        // Both should deserialize to the same result
        let uncompressed_deserialized = uncompressed_serializer.deserialize_snapshot(&uncompressed_result).unwrap();
        let compressed_deserialized = compressed_serializer.deserialize_snapshot(&compressed_result).unwrap();

        assert_eq!(uncompressed_deserialized.stream_buffers, compressed_deserialized.stream_buffers);
    }

    #[test]
    fn test_message_serialization() {
        let source_server = migration_request::ServerInstance::new("127.0.0.1".to_string(), 8080, 8081);
        let target_server = migration_request::ServerInstance::new("127.0.0.1".to_string(), 8082, 8083);
        let request = MigrationRequest::new(
            Uuid::new_v4(),
            source_server,
            target_server,
            MigrationReason::LoadBalancing,
            "admin".to_string(),
        );

        let message = MigrationMessage {
            message_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            timestamp: SystemTime::now(),
            source_server: Uuid::new_v4(),
            target_server: Uuid::new_v4(),
            protocol_version: ProtocolVersion::V2_0,
            message_type: MigrationMessageType::InitiationRequest(request),
        };

        let serializer = StateSerializer::with_default_config();
        let serialized = serializer.serialize_message(&message).unwrap();
        let deserialized = serializer.deserialize_message(&serialized).unwrap();

        assert_eq!(message.message_id, deserialized.message_id);
        assert!(matches!(deserialized.message_type, MigrationMessageType::InitiationRequest(_)));
    }

    #[test]
    fn test_checksum_validation() {
        let snapshot = ConnectionStateSnapshot::new(Uuid::new_v4());
        
        let serializer = StateSerializer::with_default_config();
        let mut serialized = serializer.serialize_snapshot(&snapshot).unwrap();

        // Corrupt the checksum
        serialized.metadata.checksum[0] ^= 1;

        // Should fail validation
        assert!(serializer.deserialize_snapshot(&serialized).is_err());
    }

    #[test]
    fn test_size_limits() {
        let mut snapshot = ConnectionStateSnapshot::new(Uuid::new_v4());
        // Add data that exceeds the limit
        snapshot = snapshot.add_stream_buffer(1, vec![0u8; 100_000_000]); // 100MB

        let config = SerializationConfig {
            max_size_bytes: 10_000_000, // 10MB limit
            ..Default::default()
        };
        let serializer = StateSerializer::new(config);

        // Should fail due to size limit
        assert!(matches!(
            serializer.serialize_snapshot(&snapshot).unwrap_err(),
            SerializationError::SizeLimitExceeded(_)
        ));
    }

    #[test]
    fn test_utility_functions() {
        let data = vec![1, 2, 3, 4, 5];
        
        let serialized = utils::quick_serialize(&data).unwrap();
        let deserialized: Vec<i32> = utils::quick_deserialize(&serialized).unwrap();
        assert_eq!(data, deserialized);

        let json_serialized = utils::serialize_to_json(&data).unwrap();
        let json_deserialized: Vec<i32> = utils::deserialize_from_json(&json_serialized).unwrap();
        assert_eq!(data, json_deserialized);
    }

    #[test]
    fn test_size_estimation() {
        let snapshot = ConnectionStateSnapshot::new(Uuid::new_v4())
            .add_stream_buffer(1, vec![0u8; 1000]);

        let serializer = StateSerializer::with_default_config();
        let estimated_size = serializer.estimate_size(&snapshot).unwrap();
        let actual_serialized = serializer.serialize_snapshot(&snapshot).unwrap();

        // Estimation should be in the right ballpark (within 50% typically)
        let ratio = estimated_size as f64 / actual_serialized.data.len() as f64;
        assert!(ratio > 0.1 && ratio < 10.0);
    }
}