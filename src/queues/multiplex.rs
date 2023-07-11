use std::{future::Future, net::SocketAddr};

use crate::transport::message::{FramedMessage, MessageBuf};

use super::sender::Sender;
use super::{listener::ConnectionSetup, receiver::Receiver};
use tokio::{
    io::{self, Interest},
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, unbounded_channel, UnboundedReceiver},
};

pub fn make_channel<R: FramedMessage<R> + Send + 'static>(
    stream: TcpStream,
) -> io::Result<(Sender, Receiver<R>)> {
    let local = stream.local_addr()?;
    let remote = stream.peer_addr()?;

    let (instream, outstream) = stream.into_split();

    let sender = Sender::start(outstream, local, remote);
    let receiver: Receiver<R> = Receiver::start(instream, remote, local, sender.clone(), |msg| {
        R::from_message(msg)
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
        println!("[Receiver::accept] accepting connections");
        let mut is_ok = true;
        while is_ok {
            let stream: Result<(TcpStream, SocketAddr), io::Error> = listener.accept().await;
            is_ok = stream.is_ok();

            match stream {
                Ok((stream, addr)) => {
                    println!("[Receiver::accept] accepted connection from {}", addr);
                    let ready = stream.ready(Interest::READABLE | Interest::WRITABLE).await;
                    println!("[Receiver::accept] ready object available");
                    if !ready.is_ok() {
                        println!("[Receiver::accept] STREAM IS NOT READABLE NOR WRITABLE");
                        break;
                    }

                    let ready = ready.unwrap();
                    println!("[Receiver::accept] STREAM IS READABLE");
                    let started = f(stream);
                    if let Err(_) = tx.send(started) {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        println!("[Receiver::accept] accepting connections - ENDED");
    });

    rx
}
