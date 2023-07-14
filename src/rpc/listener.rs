use std::sync::Arc;

use tokio::{
    io::{self},
    net::TcpListener,
    sync::mpsc::UnboundedReceiver,
};
use tokio_stream::Stream;

use crate::queues::multiplex::accept_connections;

use super::{
    api::RequestEnum, incoming::Incoming, multiplex::make_rpc_connection,
    outgoing::Outgoing,
};

pub struct Listener<N>
where
    N: RequestEnum,
{
    external: Arc<String>,
    port: u16,
    rx: UnboundedReceiver<io::Result<(Outgoing, Incoming<N>)>>,
}

impl<N> Listener<N>
where
    N: RequestEnum,
{
    pub async fn start(hostname: String, port: u16) -> io::Result<Self> {
        let sockaddr = ("0.0.0.0", port);
        let listener = TcpListener::bind(&sockaddr).await?;
        let external = Arc::new(hostname);
        let port = listener.local_addr()?.port();
        let rx = accept_connections(listener, make_rpc_connection).await;

        Ok(Listener {
            external: external,
            port: port,
            rx: rx,
        })
    }

    pub fn external_addr(&self) -> (&str, u16) {
        (&*self.external, self.port)
    }
}

impl<N> Stream for Listener<N>
where
    N: RequestEnum,
{
    type Item = io::Result<(Outgoing, Incoming<N>)>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.get_mut().rx.poll_recv(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}
