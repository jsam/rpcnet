use std::{
    collections::HashMap,
    future::Future,
    io::{self, Write},
    marker::PhantomData,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use tokio::sync::{
    oneshot::{self, error::RecvError},
    Mutex,
};

use crate::{queues::sender::Sender, transport::message::MessageBuf};

use super::api::{Name, Pending, Request, RequestId, Type};

// pub struct Responder<N: Name, R: Request<N>> {
//     id: RequestId,
//     origin: Sender,
//     marker: PhantomData<(N, R)>,
// }

// impl<N: Name, R: Request<N>> Responder<N, R> {
//     pub fn new(id: RequestId, origin: Sender) -> Self {
//         Self {
//             id,
//             origin,
//             marker: PhantomData,
//         }
//     }

//     /// Sends back the response to the client which submitted the request.
//     pub async fn respond(self, res: Result<R::Success, R::Error>) {
//         let mut msg = MessageBuf::empty();
//         msg.push(Type::Response as u8).unwrap();
//         msg.push(self.id).unwrap();
//         msg.push(res).unwrap();
//         self.origin.send(msg).await
//     }
// }

#[must_use = "futures do nothing unless polled"]
pub struct Response<N: Name, R: Request<N>> {
    rx: oneshot::Receiver<MessageBuf>,
    pending: Arc<Mutex<HashMap<RequestId, Pending>>>,
    id: RequestId,
    _request: PhantomData<(N, R)>,
}

impl<N: Name, R: Request<N>> Response<N, R> {
    pub fn new(
        rx: oneshot::Receiver<MessageBuf>,
        pending: Arc<Mutex<HashMap<RequestId, Pending>>>,
        id: RequestId,
    ) -> Self {
        Self {
            rx,
            pending,
            id,
            _request: PhantomData,
        }
    }
}

impl<N: Name, R: Request<N>> Future for Response<N, R> {
    type Output = Result<MessageBuf, RecvError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.rx).poll(cx) {
            Poll::Ready(Ok(message)) => Poll::Ready(Ok(message)),
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<N: Name, R: Request<N>> Drop for Response<N, R> {
    fn drop(&mut self) {
        // Remove pending response if it exists
        if let Ok(mut pending) = self.pending.try_lock() {
            if let Some(tx) = pending.remove(&self.id) {
                let _ = tx.send(MessageBuf::empty());
            }
        }
    }
}
