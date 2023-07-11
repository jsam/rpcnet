use std::{future::Future, marker::PhantomData, process::Output, sync::Arc};

use tokio::{
    io::{self},
    net::{TcpListener, TcpStream},
    sync::mpsc::UnboundedReceiver,
};
use tokio_stream::Stream;

use crate::transport::message::FramedMessage;

use super::{
    multiplex::{accept_connections, make_channel},
    receiver::Receiver,
    sender::Sender,
};

pub trait ConnectionSetup<R> {
    type Item: Send + 'static;

    fn setup(stream: TcpStream) -> Self::Item;
}

pub struct NetworkConnection;

impl<R> ConnectionSetup<R> for NetworkConnection
where
    R: FramedMessage<R> + Send + 'static,
{
    type Item = io::Result<(Sender, Receiver<R>)>;

    fn setup(stream: TcpStream) -> Self::Item {
        make_channel(stream)
    }
}

pub struct Listener<R, S>
where
    R: FramedMessage<R> + Send + 'static,
    S: ConnectionSetup<R> + Unpin + Send + 'static,
{
    external: Arc<String>,
    port: u16,
    rx: UnboundedReceiver<S::Item>,

    _PhantomData: PhantomData<S>,
}

impl<R, S> Listener<R, S>
where
    R: FramedMessage<R> + Send + 'static,
    S: ConnectionSetup<R> + Unpin + Send + 'static,
{
    pub async fn start(hostname: String, port: u16) -> io::Result<Self> {
        let sockaddr = ("0.0.0.0", port);
        let listener = TcpListener::bind(&sockaddr).await?;
        let external = Arc::new(hostname);
        let port = listener.local_addr()?.port();
        let rx = accept_connections(listener, S::setup).await;

        Ok(Listener {
            external: external,
            port: port,
            rx: rx,
            _PhantomData: PhantomData,
        })
    }

    pub fn external_addr(&self) -> (&str, u16) {
        (&*self.external, self.port)
    }
}

impl<R, S> Stream for Listener<R, S>
where
    R: FramedMessage<R> + Send + 'static,
    S: ConnectionSetup<R> + Unpin + Send + 'static,
{
    type Item = S::Item;

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
