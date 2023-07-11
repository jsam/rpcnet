use std::{net::SocketAddr, sync::Arc};

use log::{debug, info, warn};
use tokio::{
    io::AsyncWriteExt,
    net::{tcp::OwnedWriteHalf, TcpStream},
    runtime::Handle,
    sync::mpsc,
    task::JoinHandle,
};

use crate::transport::message::MessageBuf;

#[derive(Clone, Debug)]
pub struct Sender {
    tx: mpsc::UnboundedSender<MessageBuf>,

    // NOTE: Internal routine responsible for writing to the outstream channel.
    thr: Arc<Option<JoinHandle<()>>>,
}

impl Sender {
    pub(crate) fn start(mut outstream: OwnedWriteHalf, from: SocketAddr, to: SocketAddr) -> Self {
        println!("[Sender::start] Creating a sender {} -> {}", from, to);

        let (tx, mut rx) = mpsc::unbounded_channel::<MessageBuf>();

        let thr = tokio::spawn(async move {
            println!("[Sender::start] started {} -> {}", from, to);
            while let Some(msg) = rx.recv().await {
                println!("[Sender::start] message received in sender");
                if let Err(err) = msg.write_async(&mut outstream).await {
                    println!(
                        "[Sender::start] unexpected error while writing bytes {} -> {}: {:?}",
                        from, to, err
                    );
                    break;
                }
            }

            println!("[Sender::start] shutting down {} -> {}", from, to);
            drop(outstream.shutdown());
        });

        Sender {
            tx: tx,
            thr: Arc::new(Some(thr)),
        }
    }

    pub async fn send<T: Into<MessageBuf>>(&self, msg: T) {
        let send_result = self.tx.send(msg.into());
        println!("[Sender::send] message sent in sender");
        if let Err(_err) = send_result {
            println!(
                "[Sender::send] unexpected error while sending message: {:?}",
                _err
            );
        }
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        // make sure to drain the queue if the other side is still connected
        // drop(self.tx);
        println!("[Sender::drop] Dropping sender");
        if let Some(handle) = Arc::get_mut(&mut self.thr).and_then(Option::take) {
            drop(handle.abort());
        }
    }
}
