use std::{net::SocketAddr, sync::Arc};

use tokio::{
    io::AsyncWriteExt,
    net::tcp::OwnedWriteHalf,
    sync::mpsc::{unbounded_channel, UnboundedSender},
    task::JoinHandle,
};

use crate::transport::message::MessageBuf;

#[derive(Clone, Debug)]
pub struct Sender {
    tx: UnboundedSender<MessageBuf>,

    // NOTE: Internal routine responsible for writing to the outstream channel.
    thr: Arc<Option<JoinHandle<()>>>,
}

impl Sender {
    pub(crate) fn start(
        mut outstream: OwnedWriteHalf,
        from: SocketAddr,
        to: SocketAddr,
    ) -> Self {
        let (tx, mut rx) = unbounded_channel::<MessageBuf>();

        let thr = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(err) = msg.write_async(&mut outstream).await {
                    break;
                }
            }

            drop(outstream.shutdown());
        });

        Sender {
            tx: tx,
            thr: Arc::new(Some(thr)),
        }
    }

    pub fn send<T: Into<MessageBuf>>(&self, msg: T) {
        let send_result = self.tx.send(msg.into());
        if let Err(_err) = send_result {
            // TODO:
        }
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        // drop(self.tx);  // drain + drop
        if let Some(handle) = Arc::get_mut(&mut self.thr).and_then(Option::take) {
            drop(handle.abort());
        }
    }
}
