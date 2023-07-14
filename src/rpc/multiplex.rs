use std::{
    collections::HashMap,
    sync::{atomic::AtomicUsize, Arc},
};
use tokio::{
    io,
    net::TcpStream,
    sync::{mpsc::unbounded_channel, Mutex},
};
use tokio_stream::StreamExt;

use crate::queues::{receiver::Receiver, sender::Sender};

use super::{
    api::{Pending, RequestEnum, RequestId},
    incoming::Incoming,
    outgoing::Outgoing,
    request::RequestBuf,
};

pub fn make_rpc_connection<N: RequestEnum>(
    stream: TcpStream,
) -> io::Result<(Outgoing, Incoming<N>)> {
    let local = stream.local_addr()?;
    let remote = stream.peer_addr()?;

    let (instream, outstream) = stream.into_split();

    let (incoming_tx, incoming_rx) = unbounded_channel();
    let pending: Arc<Mutex<HashMap<RequestId, Pending>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let sender = Sender::start(outstream, local, remote);
    let outgoing = Outgoing::new(
        Arc::new(AtomicUsize::new(0)),
        pending.clone(),
        sender.clone(),
    );

    // TODO: Sender here should not be cloned, instead reference should be taken
    let mut receiver: Receiver<RequestBuf<N>> =
        Receiver::start(instream, remote, local, sender.clone(), move |msg| {
            use crate::transport::message::FramedMessage;
            let req = RequestBuf::from_message(sender.clone(), msg);
            if req.is_err() {
                // TODO:
            };
            req
        });

    tokio::spawn(async move {
        let mut is_ok = true;
        while is_ok {
            let msg = receiver.next().await;
            if msg.is_none() {
                // TODO: handle case where receiver is closed
                continue;
            }

            let msg = msg.unwrap();
            if msg.is_err() {
                continue;
            }

            let request = msg.unwrap();

            // TODO: handle error here

            match request.buf_type {
                super::api::Type::Request => {
                    incoming_tx.send(Ok(request));
                    // TODO: handle error
                }
                super::api::Type::Response => {
                    let mut pending = pending.try_lock().unwrap();
                    let completed = pending
                        .remove(&request.id)
                        .and_then(move |tx| tx.send(request.msg).ok())
                        .is_some();

                    if !completed {
                        //info!("dropping canceled response for {:?}", id);
                    }
                }
            }
        }
    });

    let incoming = Incoming::new(incoming_rx);
    Ok((outgoing, incoming))
}
