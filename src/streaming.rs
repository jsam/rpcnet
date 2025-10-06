use futures::{Future, Stream};
use pin_project::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Sleep};
use tokio_stream::wrappers::ReceiverStream;

use crate::RpcError;

#[derive(Debug)]
pub enum StreamError<T> {
    Timeout,
    Transport(RpcError),
    Item(T),
}

#[pin_project]
pub struct TimeoutStream<S>
where
    S: Stream,
{
    #[pin]
    inner: S,
    timeout: Duration,
    #[pin]
    timer: Option<Sleep>,
}

impl<S> TimeoutStream<S>
where
    S: Stream,
{
    pub fn new(inner: S, timeout: Duration) -> Self {
        Self {
            inner,
            timeout,
            timer: None,
        }
    }
}

impl<S, T> Stream for TimeoutStream<S>
where
    S: Stream<Item = Result<T, RpcError>>,
{
    type Item = Result<T, StreamError<RpcError>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        if this.timer.is_none() {
            this.timer.set(Some(sleep(*this.timeout)));
        }

        if let Some(timer) = this.timer.as_mut().as_pin_mut() {
            if timer.poll(cx).is_ready() {
                return Poll::Ready(Some(Err(StreamError::Timeout)));
            }
        }

        match this.inner.poll_next(cx) {
            Poll::Ready(Some(Ok(item))) => {
                this.timer.set(Some(sleep(*this.timeout)));
                Poll::Ready(Some(Ok(item)))
            }
            Poll::Ready(Some(Err(e))) => {
                Poll::Ready(Some(Err(StreamError::Transport(e))))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct BidirectionalStream<T>
where
    T: Send + 'static,
{
    pub sender: mpsc::Sender<T>,
    stream: Pin<Box<dyn Stream<Item = T> + Send>>,
    abort_handle: Option<JoinHandle<()>>,
}

impl<T> BidirectionalStream<T>
where
    T: Send + 'static,
{
    pub fn new(buffer: usize) -> Self {
        let (tx, rx) = mpsc::channel::<T>(buffer);
        let stream = ReceiverStream::new(rx);
        
        Self {
            sender: tx,
            stream: Box::pin(stream),
            abort_handle: None,
        }
    }

    pub fn with_task<F, Fut>(buffer: usize, task: F) -> Self
    where
        F: FnOnce(mpsc::Sender<T>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let (tx, rx) = mpsc::channel::<T>(buffer);
        let stream = ReceiverStream::new(rx);
        
        let sender_clone = tx.clone();
        let abort_handle = tokio::spawn(task(sender_clone));
        
        Self {
            sender: tx,
            stream: Box::pin(stream),
            abort_handle: Some(abort_handle),
        }
    }

    pub fn abort(&mut self) {
        if let Some(handle) = self.abort_handle.take() {
            handle.abort();
        }
    }

    pub fn into_stream(mut self) -> Pin<Box<dyn Stream<Item = T> + Send>> {
        let stream_ptr = &mut self.stream as *mut _;
        std::mem::forget(self);
        unsafe { std::ptr::read(stream_ptr) }
    }
}

impl<T> Drop for BidirectionalStream<T>
where
    T: Send + 'static,
{
    fn drop(&mut self) {
        self.abort();
    }
}
