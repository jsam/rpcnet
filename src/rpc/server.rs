use std::sync::Arc;

use tokio::{io, task::JoinHandle};
use tokio_stream::StreamExt;

use super::{api::RequestEnum, dispatcher::Dispatcher, listener::Listener};

pub struct Server {
    listener: JoinHandle<()>,
    pub external: Arc<String>,
    pub port: u16,
}

impl Server {
    pub async fn start<N, D>(
        hostname: Arc<String>,
        port: u16,
        dispatcher: D,
    ) -> io::Result<Self>
    where
        N: RequestEnum + Send + 'static,
        D: Dispatcher<N> + Copy + Send + 'static,
        for<'a> D::Check<'a>: Send,
    {
        let listener =
            Listener::<N>::start(hostname.clone().to_string(), port).await?;

        let addr = listener.external_addr();
        let mut dispatcher_impl = dispatcher;

        let listener_task = tokio::spawn(async move {
            tokio::pin!(listener);

            while let Some(connection) = listener.next().await {
                match connection {
                    Ok((outgoing, incoming)) => {
                        tokio::spawn(async move {
                            dispatcher_impl
                                .dispatch_connection(outgoing, incoming)
                                .await;
                        });
                    }
                    Err(err) => panic!("Error: {}", err), // TODO: Handle error
                }
            }
        });

        Ok(Server {
            listener: listener_task,
            external: hostname,
            port: port,
        })
    }

    pub fn external_addr(&self) -> (&str, u16) {
        (&*self.external, self.port)
    }

    pub fn addr(&self) -> String {
        format!("{}:{}", self.external, self.port)
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpStream;

    use crate::rpc::{
        multiplex,
        status::{PingRequest, StatusDispatcher, StatusRPC},
    };

    use super::*;

    #[tokio::test]
    async fn test_server() {
        let server = Server::start::<StatusRPC, StatusDispatcher>(
            Arc::new("localhost".to_string()),
            0,
            StatusDispatcher {},
        )
        .await;
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_server_client() {
        let server = Server::start::<StatusRPC, StatusDispatcher>(
            Arc::new("localhost".to_string()),
            6789,
            StatusDispatcher {},
        )
        .await
        .unwrap();
        let socket = TcpStream::connect(server.addr()).await.unwrap();
        let (outgoing, incoming) =
            multiplex::make_rpc_connection::<StatusRPC>(socket).unwrap();

        let response = outgoing.request(&PingRequest {});
        assert!(response.id == 0);

        let response = outgoing.request(&PingRequest {});
        assert!(response.id == 1);

        let response = outgoing.request(&PingRequest {});
        assert!(response.id == 2);

        let response = response.await.unwrap();
        assert!(response.is_ok());
    }
}
