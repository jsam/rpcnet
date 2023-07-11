use std::{
    collections::HashMap,
    marker::PhantomData,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use tokio::sync::{oneshot, Mutex};

use crate::{queues::sender::Sender, transport::message::MessageBuf};

use super::{
    api::{Name, Pending, Request, RequestId, Type},
    response::Response,
};

#[derive(Clone)]
pub struct Outgoing {
    next_id: Arc<AtomicUsize>,
    pending: Arc<Mutex<HashMap<RequestId, Pending>>>,
    sender: Sender,
}

impl Outgoing {
    pub fn new(
        next_id: Arc<AtomicUsize>,
        pending: Arc<Mutex<HashMap<RequestId, Pending>>>,
        sender: Sender,
    ) -> Self {
        Self {
            next_id,
            pending,
            sender,
        }
    }

    fn next_id(&self) -> RequestId {
        self.next_id.fetch_add(1, Ordering::SeqCst) as u32
    }

    /// Asynchronously sends out a request to the remote peer.
    ///
    /// Returns a future for the pending response. The next request can be
    /// submitted without having to wait for the previous response to arrive.
    pub async fn request<N: Name, R: Request<N>>(&self, r: &R) -> Response<N, R> {
        let id = self.next_id();
        let (tx, rx) = oneshot::channel();

        // step 1: create request packet
        let mut msg = MessageBuf::empty();
        msg.push(Type::Request as u8).unwrap();
        msg.push(id).unwrap();
        msg.push(R::NAME.discriminant()).unwrap();
        msg.push::<&R>(r).unwrap();

        // step 2: add completion handle for pending responses
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, tx);
        }

        // step 3: send packet to network
        self.sender.send(msg).await;

        // step 4: prepare response decoder
        Response::new(rx, self.pending.clone(), id)
    }
}
