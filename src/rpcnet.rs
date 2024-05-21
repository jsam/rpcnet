//use crate::file::FileHandler;
use crate::rpc::api::RequestEnum;
use crate::rpc::dispatcher::Dispatcher;
use crate::rpc::incoming::Incoming;
use crate::rpc::multiplex;
use crate::rpc::outgoing::Outgoing;
use crate::rpc::server::Server;
use std::env;
use std::sync::Arc;
use tokio::io;
use tokio::net::{TcpStream, ToSocketAddrs};

#[derive(Clone, Debug)]
pub struct RpcNet {
    pub hostname: Arc<String>,
    //pub file: FileHandler, // File upload and download
}

impl RpcNet {
    pub fn new<T: Into<Option<String>>>(hostname: T) -> io::Result<Self> {
        let hostname = if let Some(hostname_arg) = hostname.into() {
            hostname_arg
        } else if let Ok(hostname_env) = env::var("LAYERDB_HOSTNAME") {
            hostname_env
        } else {
            // warn!("unable to retrieve external hostname of machine.");
            // warn!("falling back to 'localhost', set LAYER_HOSTNAME to override");

            String::from("localhost")
        };

        Ok(RpcNet {
            hostname: Arc::new(hostname),
            //file: FileHandler::new(),
        })
    }

    pub fn hostname(&self) -> String {
        (*self.hostname).clone()
    }

    pub async fn client<N: RequestEnum, E: ToSocketAddrs>(
        &self,
        endpoint: E,
    ) -> io::Result<(Outgoing, Incoming<N>)> {
        let socket = TcpStream::connect(endpoint).await?;
        let (outgoing, incoming) = multiplex::make_rpc_connection(socket)?;
        Ok((outgoing, incoming))
    }

    pub async fn server<
        N: RequestEnum,
        D: Dispatcher<N>,
        P: Into<Option<u16>>,
    >(
        &self,
        port: P,
        dispatcher: D,
    ) -> io::Result<Server>
    where
        N: RequestEnum + Send + 'static,
        D: Dispatcher<N> + Copy + Send + 'static
    {
        Server::start::<N, D>(
            self.hostname.clone(),
            port.into().unwrap_or(0),
            dispatcher,
        )
        .await
    }
}
