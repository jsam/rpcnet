use std::{net::SocketAddr, time::Duration};

use tokio::{
    io::{self, BufReader, Interest},
    net::tcp::OwnedReadHalf,
    sync::mpsc::{self, unbounded_channel},
    time::sleep,
};
use tokio_stream::Stream;

use crate::transport::message::{FramedMessage, MessageBuf};

use super::sender::Sender;

pub struct Receiver<R>
where
    R: Send + 'static,
{
    rx: mpsc::UnboundedReceiver<io::Result<R>>,
    from: SocketAddr,
    to: SocketAddr,
    // TODO: short-circuit responder
    pub responder: Sender,
}

impl<R> Receiver<R>
where
    R: Send + 'static,
    R: FramedMessage<R>,
{
    pub(crate) fn start(
        mut instream: OwnedReadHalf,
        from: SocketAddr,
        to: SocketAddr,
        responder: Sender,
        mut handle: impl FnMut(MessageBuf) -> io::Result<R> + Send + 'static,
    ) -> Self {
        let (tx, rx) = unbounded_channel::<io::Result<R>>();

        tokio::spawn(async move {
            loop {
                let result = instream.ready(Interest::READABLE).await.unwrap();
                if !result.is_readable() {
                    sleep(Duration::from_millis(1)).await;
                    continue;
                }
                break;
            }

            let mut instream = BufReader::new(instream);
            let mut stop = false;

            while !stop {
                let message = match MessageBuf::read_async(&mut instream).await
                {
                    Ok(Some(msg)) => handle(msg),
                    Ok(None) => {
                        sleep(Duration::from_millis(1)).await;
                        break;
                    }
                    Err(err) => {
                        stop = true;
                        Err(err)
                    }
                };

                match tx.send(message) {
                    Ok(tx) => tx,
                    Err(err) => {
                        break;
                    }
                };
            }
            //drop(instream.get_mut().poll_shutdown());
        });

        Receiver {
            rx: rx,
            from: from,
            to: to,
            responder: responder,
        }
    }
}

impl<R> Stream for Receiver<R>
where
    R: Send + 'static,
{
    type Item = io::Result<R>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.get_mut().rx.poll_recv(cx)
    }
}

impl<R> Drop for Receiver<R>
where
    R: Send + 'static,
{
    fn drop(&mut self) {
    }
}
