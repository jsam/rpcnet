use std::fs::File;
use std::{env, thread};
use std::{path::Path, sync::Arc};

use tokio::io;
use tokio::net::{TcpStream, ToSocketAddrs};

use crate::file::FileHandler;
use crate::network::NetworkHandler;
use crate::rpc::api::Name;
use crate::rpc::incoming::Incoming;
use crate::rpc::multiplex;
use crate::rpc::outgoing::Outgoing;
use crate::rpc::server::Server;

#[derive(Clone, Debug)]
pub struct RpcNet {
    pub hostname: Arc<String>,
    // NOTE:
    pub file: FileHandler, // File upload and download
}

impl RpcNet {
    /// Creates a new default network handle.
    ///
    /// The `hostname` argument should be set to an externally reachable
    /// hostname of the current process. If it is `None`, this constructor
    /// will try to retrieve it by reading the `LAYER_HOSTNAME` environment
    /// variable. If both the argument and the environment variable are missing,
    /// it falls back to `"localhost"`.
    pub fn new<T: Into<Option<String>>>(hostname: T) -> io::Result<Self> {
        // try to guess external hostname
        let hostname = if let Some(hostname_arg) = hostname.into() {
            hostname_arg
        } else if let Ok(hostname_env) = env::var("LAYER_HOSTNAME") {
            hostname_env
        } else {
            // warn!("unable to retrieve external hostname of machine.");
            // warn!("falling back to 'localhost', set LAYER_HOSTNAME to override");

            String::from("localhost")
        };

        Ok(RpcNet {
            hostname: Arc::new(hostname),
            file: FileHandler::new(),
        })
    }

    /// Returns the configured externally reachable hostname of the current process.
    pub fn hostname(&self) -> String {
        (*self.hostname).clone()
    }

    pub async fn client<N: Name, E: ToSocketAddrs>(
        &self,
        endpoint: E,
    ) -> io::Result<(Outgoing, Incoming<N>)> {
        let socket = TcpStream::connect(endpoint).await?;
        let (outgoing, incoming) = multiplex::split(socket)?;
        Ok((outgoing, incoming))
    }

    pub async fn server<N: Name, P: Into<Option<u16>>>(&self, port: P) -> io::Result<Server<N>> {
        Server::start(self.hostname.clone(), port.into().unwrap_or(0)).await
    }
}
