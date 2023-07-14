use std::{
    collections::HashMap,
    future::Future,
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

use super::api::{Pending, Request, RequestEnum, RequestId, Type};

pub struct Responder<N: RequestEnum, R: Request<N>> {
    id: RequestId,
    origin: Sender,
    marker: PhantomData<(N, R)>,
}

impl<N: RequestEnum, R: Request<N>> Responder<N, R> {
    pub fn new(id: RequestId, origin: Sender) -> Self {
        Self {
            id,
            origin,
            marker: PhantomData,
        }
    }

    pub fn respond(self, res: Result<R::Success, R::Error>) {
        let mut msg = MessageBuf::empty();
        msg.push(Type::Response as u8).unwrap();
        msg.push(self.id).unwrap();
        msg.push(R::NAME.to_byte()).unwrap();
        msg.push(res).unwrap();
        self.origin.send(msg)
    }
}

#[must_use = "futures do nothing unless polled"]
pub struct Response<N: RequestEnum, R: Request<N>> {
    rx: oneshot::Receiver<MessageBuf>,
    // TODO: implement resolution of pending responses
    pending: Arc<Mutex<HashMap<RequestId, Pending>>>, // NOTE: Pending channel is `tx` part of oneshot::channel()
    pub id: RequestId,
    _request: PhantomData<(N, R)>,
}

impl<N: RequestEnum, R: Request<N>> Response<N, R> {
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

impl<N: RequestEnum, R: Request<N>> Future for Response<N, R> {
    type Output = Result<Result<R::Success, R::Error>, RecvError>;

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        match Pin::new(&mut self.rx).poll(cx) {
            Poll::Ready(Ok(mut message)) => {
                match message.pop::<Result<R::Success, R::Error>>() {
                    Ok(Ok(payload)) => Poll::Ready(Ok(Ok(payload))),
                    Ok(Err(err)) => Poll::Ready(Ok(Err(err))),
                    Err(_) => todo!(),
                }
            }
            Poll::Ready(Err(err)) => Poll::Ready(Err(err)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<N: RequestEnum, R: Request<N>> Drop for Response<N, R> {
    fn drop(&mut self) {
        if let Ok(mut pending) = self.pending.try_lock() {
            if let Some(tx) = pending.remove(&self.id) {
                let _ = tx.send(MessageBuf::empty());
            }
        }
    }
}
