use quiche;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use crate::errors::RpcError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    max_idle_timeout: Duration,
    max_packet_size: usize,
    max_concurrent_streams: u64,
    application_protos: Vec<Vec<u8>>,
    verify_peer: bool,
    cert_path: Option<PathBuf>,
    key_path: Option<PathBuf>,
    ca_path: Option<PathBuf>,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            max_idle_timeout: Duration::from_secs(30),
            max_packet_size: 1350,
            max_concurrent_streams: 1000,
            application_protos: vec![b"rpc-1".to_vec()],
            verify_peer: false,
            cert_path: None,
            key_path: None,
            ca_path: None,
        }
    }
}

impl RpcConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_idle_timeout(mut self, timeout: Duration) -> Self {
        self.max_idle_timeout = timeout;
        self
    }

    pub fn with_max_packet_size(mut self, size: usize) -> Self {
        self.max_packet_size = size;
        self
    }

    pub fn with_max_concurrent_streams(mut self, streams: u64) -> Self {
        self.max_concurrent_streams = streams;
        self
    }

    pub fn with_application_protos(mut self, protos: Vec<Vec<u8>>) -> Self {
        self.application_protos = protos;
        self
    }

    pub fn with_verify_peer(mut self, verify: bool) -> Self {
        self.verify_peer = verify;
        self
    }

    pub fn with_cert_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.cert_path = Some(path.into());
        self
    }

    pub fn with_key_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.key_path = Some(path.into());
        self
    }

    pub fn with_ca_path<P: Into<PathBuf>>(mut self, path: P) -> Self {
        self.ca_path = Some(path.into());
        self
    }

    pub fn build(&self) -> Result<quiche::Config, RpcError> {
        let mut config = quiche::Config::new(quiche::PROTOCOL_VERSION)?;

        config.set_max_idle_timeout(self.max_idle_timeout.as_millis() as u64);
        config.set_max_send_udp_payload_size(self.max_packet_size);
        config.set_initial_max_data(10_000_000);
        config.set_initial_max_stream_data_bidi_local(1_000_000);
        config.set_initial_max_stream_data_bidi_remote(1_000_000);
        config.set_initial_max_streams_bidi(self.max_concurrent_streams);

        let _protos = self.application_protos.iter().map(|v| v.as_slice()).collect::<Vec<&[u8]>>();
        config.set_application_protos(&_protos)?;

        if self.verify_peer {
            // Load CA certificates for peer verification
            if let Some(ca_path) = &self.ca_path {
                config.load_verify_locations_from_file(ca_path.to_str().expect("ca_path not specified"))?;
            }

            // Load server certificates and private key if provided
            if let (Some(cert_path), Some(key_path)) = (&self.cert_path, &self.key_path) {
                config.load_cert_chain_from_pem_file(cert_path.to_str().expect("cert_path no specified"))?;
                config.load_priv_key_from_pem_file(key_path.to_str().expect("key_path not specified"))?;
            }
        }

        Ok(config)
    }
}