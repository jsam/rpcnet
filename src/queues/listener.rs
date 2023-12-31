use std::sync::Arc;

use tokio::{
    io::{self},
    net::TcpListener,
    sync::mpsc::UnboundedReceiver,
};
use tokio_stream::Stream;

use crate::transport::message::MessageBuf;

use super::{
    multiplex::{accept_connections, make_connection},
    receiver::Receiver,
    sender::Sender,
};

pub struct Listener {
    external: Arc<String>,
    port: u16,
    rx: UnboundedReceiver<io::Result<(Sender, Receiver<MessageBuf>)>>,
}

impl Listener {
    pub async fn start(hostname: String, port: u16) -> io::Result<Self> {
        let sockaddr = ("0.0.0.0", port);
        let listener = TcpListener::bind(&sockaddr).await?;
        let external = Arc::new(hostname);
        let port = listener.local_addr()?.port();
        let rx = accept_connections(listener, make_connection).await;

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

impl Stream for Listener {
    type Item = io::Result<(Sender, Receiver<MessageBuf>)>;

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
