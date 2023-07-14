use std::io;

use tokio::sync::mpsc;
use tokio_stream::Stream;

use super::{api::RequestEnum, request::RequestBuf};

#[must_use = "futures do nothing unless polled"]
pub struct Incoming<N: RequestEnum> {
    rx: mpsc::UnboundedReceiver<Result<RequestBuf<N>, io::Error>>,
}

impl<N: RequestEnum> Incoming<N> {
    pub fn new(
        rx: mpsc::UnboundedReceiver<Result<RequestBuf<N>, io::Error>>,
    ) -> Self {
        Self { rx }
    }
}

impl<N: RequestEnum> Stream for Incoming<N> {
    type Item = Result<RequestBuf<N>, io::Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.get_mut().rx.poll_recv(cx)
    }
}
