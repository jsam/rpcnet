use std::borrow::BorrowMut;
use std::marker::PhantomData;
use std::{future::Future, net::SocketAddr, pin::Pin, time::Duration};

use tokio::io::AsyncWriteExt;
use tokio::{
    io::{self, BufReader, Interest},
    net::{tcp::OwnedReadHalf, TcpListener, TcpStream},
    sync::mpsc::{self, unbounded_channel, UnboundedReceiver},
    time::{sleep, Sleep},
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
    responder: Sender,
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
                    println!("[Receiver::start] STREAM IS NOT READABLE");
                    sleep(Duration::from_millis(1)).await;
                    continue;
                }
                break;
            }

            let mut instream = BufReader::new(instream);
            let mut stop = false;

            while !stop {
                println!("[Receiver::start] tick");

                let message = match MessageBuf::read_async(&mut instream).await {
                    Ok(Some(msg)) => handle(msg),
                    Ok(None) => {
                        sleep(Duration::from_millis(1)).await;
                        break;
                    }
                    Err(err) => {
                        println!(
                            "[Receiver::start] unexpected error while reading bytes {} -> {}: {:?}",
                            from, to, err
                        );
                        stop = true;
                        Err(err)
                    }
                };

                match tx.send(message) {
                    Ok(tx) => tx,
                    Err(err) => {
                        println!(
                            "[Receiver::start] unexpected error while sending message {} -> {}: {:?}",
                            from, to, err
                        );
                        break;
                    }
                };
            }

            println!("[Receiver::start] shutting down {} -> {}", from, to);
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
        println!(
            "[Receiver::drop] dropping receiver {} -> {}",
            self.from, self.to
        );
    }
}
