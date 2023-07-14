use std::net::SocketAddr;

use crate::transport::message::FramedMessage;

use super::receiver::Receiver;
use super::sender::Sender;
use tokio::{
    io::{self, Interest},
    net::{TcpListener, TcpStream},
    sync::mpsc::{unbounded_channel, UnboundedReceiver},
};

pub fn make_connection<R: FramedMessage<R> + Send + 'static>(
    stream: TcpStream,
) -> io::Result<(Sender, Receiver<R>)> {
    let local = stream.local_addr()?;
    let remote = stream.peer_addr()?;

    let (instream, outstream) = stream.into_split();

    let sender = Sender::start(outstream, local, remote);
    let moved_sender = sender.clone();
    let receiver: Receiver<R> =
        Receiver::start(instream, remote, local, sender.clone(), move |msg| {
            R::from_message(moved_sender.clone(), msg)
        });

    Ok((sender, receiver))
}

pub(crate) async fn accept_connections<T, F>(
    listener: TcpListener,
    mut f: F,
) -> UnboundedReceiver<T>
where
    F: FnMut(TcpStream) -> T,
    F: Send + 'static,
    // Fut: Send + Future<Output = T>,
    T: Send + 'static,
{
    let (tx, rx) = unbounded_channel::<T>();

    tokio::spawn(async move {
        let mut is_ok = true;
        while is_ok {
            let stream: Result<(TcpStream, SocketAddr), io::Error> =
                listener.accept().await;
            is_ok = stream.is_ok();

            match stream {
                Ok((stream, addr)) => {
                    let ready = stream
                        .ready(Interest::READABLE | Interest::WRITABLE)
                        .await;
                    if !ready.is_ok() {
                        break;
                    }

                    let ready = ready.unwrap();
                    let started = f(stream);
                    if let Err(_) = tx.send(started) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    rx
}
