use std::io;

use tokio::sync::mpsc;
use tokio_stream::Stream;

use super::{api::Name, request::RequestBuf};

#[must_use = "futures do nothing unless polled"]
pub struct Incoming<N: Name> {
    rx: mpsc::UnboundedReceiver<Result<RequestBuf<N>, io::Error>>,
}

impl<N: Name> Incoming<N> {
    pub fn new(rx: mpsc::UnboundedReceiver<Result<RequestBuf<N>, io::Error>>) -> Self {
        Self { rx }
    }
}

impl<N: Name> Stream for Incoming<N> {
    type Item = Result<RequestBuf<N>, io::Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.get_mut().rx.poll_recv(cx)
    }
}
