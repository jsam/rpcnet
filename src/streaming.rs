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

// Conversion from rmp_serde errors to StreamError<RpcError>
impl From<rmp_serde::encode::Error> for StreamError<RpcError> {
    fn from(err: rmp_serde::encode::Error) -> Self {
        StreamError::Transport(RpcError::from(err))
    }
}

impl From<rmp_serde::decode::Error> for StreamError<RpcError> {
    fn from(err: rmp_serde::decode::Error) -> Self {
        StreamError::Transport(RpcError::from(err))
    }
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
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(StreamError::Transport(e)))),
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use tokio;

    #[test]
    fn test_stream_error_display() {
        let timeout_err: StreamError<RpcError> = StreamError::Timeout;
        assert!(format!("{:?}", timeout_err).contains("Timeout"));

        let transport_err: StreamError<RpcError> =
            StreamError::Transport(RpcError::ConnectionError("test".to_string()));
        assert!(format!("{:?}", transport_err).contains("Transport"));
    }

    #[test]
    fn test_stream_error_from_msgpack_encode() {
        use rmp_serde::encode::Error as EncodeError;

        // Create a serialization error
        #[derive(serde::Serialize)]
        struct BadStruct {
            #[serde(serialize_with = "fail")]
            field: i32,
        }

        fn fail<S>(_: &i32, _: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("encode error"))
        }

        let result: Result<Vec<u8>, EncodeError> = rmp_serde::to_vec(&BadStruct { field: 1 });
        if let Err(encode_err) = result {
            let stream_err: StreamError<RpcError> = encode_err.into();
            match stream_err {
                StreamError::Transport(RpcError::SerializationError(_)) => {}
                _ => panic!("Expected Transport with SerializationError"),
            }
        }
    }

    #[test]
    fn test_stream_error_from_msgpack_decode() {
        use rmp_serde::decode::Error as DecodeError;

        // Invalid MessagePack data
        let invalid_data = vec![0xFF, 0xFE, 0xFD];
        let result: Result<i32, DecodeError> = rmp_serde::from_slice(&invalid_data);

        if let Err(decode_err) = result {
            let stream_err: StreamError<RpcError> = decode_err.into();
            match stream_err {
                StreamError::Transport(RpcError::SerializationError(_)) => {}
                _ => panic!("Expected Transport with SerializationError"),
            }
        }
    }

    #[tokio::test]
    async fn test_timeout_stream_creation() {
        let stream = futures::stream::iter(vec![Ok(1), Ok(2), Ok(3)]);
        let timeout_stream = TimeoutStream::new(stream, Duration::from_secs(1));

        let items: Vec<_> = timeout_stream.collect().await;
        assert_eq!(items.len(), 3);
        assert!(items[0].is_ok());
    }

    #[tokio::test]
    async fn test_timeout_stream_with_items() {
        let stream = futures::stream::iter(vec![Ok::<i32, RpcError>(1), Ok(2), Ok(3)]);

        let timeout_stream = TimeoutStream::new(stream, Duration::from_secs(10));
        let items: Vec<_> = timeout_stream.collect().await;

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].as_ref().unwrap(), &1);
        assert_eq!(items[1].as_ref().unwrap(), &2);
        assert_eq!(items[2].as_ref().unwrap(), &3);
    }

    #[tokio::test]
    async fn test_timeout_stream_with_error() {
        let stream = futures::stream::iter(vec![
            Ok::<i32, RpcError>(1),
            Err(RpcError::StreamError("test error".to_string())),
            Ok(3),
        ]);

        let timeout_stream = TimeoutStream::new(stream, Duration::from_secs(10));
        let items: Vec<_> = timeout_stream.collect().await;

        assert_eq!(items.len(), 3);
        assert!(items[0].is_ok());
        assert!(matches!(items[1], Err(StreamError::Transport(_))));
        assert!(items[2].is_ok());
    }

    #[tokio::test]
    async fn test_timeout_stream_actually_times_out() {
        let stream = futures::stream::unfold((), |_| async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            Some((Ok::<i32, RpcError>(1), ()))
        });

        let timeout_stream = TimeoutStream::new(stream, Duration::from_millis(5));
        let mut items = vec![];

        let mut stream = Box::pin(timeout_stream);
        while let Some(item) = stream.next().await {
            let is_timeout = matches!(item, Err(StreamError::Timeout));
            items.push(item);
            if is_timeout {
                break;
            }
        }

        // Should timeout before getting an item
        assert!(!items.is_empty());
        assert!(matches!(items.last().unwrap(), Err(StreamError::Timeout)));
    }

    #[tokio::test]
    async fn test_bidirectional_stream_new() {
        let mut stream: BidirectionalStream<i32> = BidirectionalStream::new(10);

        // Send some items
        stream.sender.send(1).await.unwrap();
        stream.sender.send(2).await.unwrap();
        stream.sender.send(3).await.unwrap();

        // Drop sender to close the channel
        drop(std::mem::replace(&mut stream.sender, mpsc::channel(1).0));

        let items: Vec<_> = stream.into_stream().collect().await;
        assert_eq!(items, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_bidirectional_stream_with_buffer() {
        let mut stream: BidirectionalStream<String> = BidirectionalStream::new(100);

        // Send many items
        for i in 0..50 {
            stream.sender.send(format!("item-{}", i)).await.unwrap();
        }

        // Drop sender to close the channel
        drop(std::mem::replace(&mut stream.sender, mpsc::channel(1).0));

        let items: Vec<_> = stream.into_stream().collect().await;
        assert_eq!(items.len(), 50);
        assert_eq!(items[0], "item-0");
        assert_eq!(items[49], "item-49");
    }

    #[tokio::test]
    async fn test_bidirectional_stream_with_task() {
        let mut stream = BidirectionalStream::with_task(10, |sender| async move {
            for i in 0..5 {
                sender.send(i).await.ok();
            }
            // Task completes and drops sender automatically
        });

        // Wait for task to complete and send items
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Drop the stream's sender (task already dropped its clone)
        drop(std::mem::replace(&mut stream.sender, mpsc::channel(1).0));

        let items: Vec<_> = stream.into_stream().collect().await;
        assert_eq!(items, vec![0, 1, 2, 3, 4]);
    }

    #[tokio::test]
    async fn test_bidirectional_stream_abort() {
        let mut stream = BidirectionalStream::with_task(10, |sender| async move {
            loop {
                sender.send(1).await.ok();
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });

        tokio::time::sleep(Duration::from_millis(5)).await;
        stream.abort();

        // Task should be aborted
        assert!(stream.abort_handle.is_none());
    }

    #[tokio::test]
    async fn test_bidirectional_stream_drop_aborts() {
        let stream = BidirectionalStream::with_task(10, |sender| async move {
            loop {
                sender.send(1).await.ok();
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });

        // Stream should abort task on drop
        drop(stream);

        tokio::time::sleep(Duration::from_millis(5)).await;
        // If we get here without hanging, drop worked correctly
    }

    #[tokio::test]
    async fn test_timeout_stream_resets_timer_on_item() {
        // Stream that emits items slowly but within timeout
        let stream = futures::stream::unfold(0, |count| async move {
            if count < 3 {
                tokio::time::sleep(Duration::from_millis(3)).await;
                Some((Ok::<i32, RpcError>(count), count + 1))
            } else {
                None
            }
        });

        let timeout_stream = TimeoutStream::new(stream, Duration::from_millis(10));
        let items: Vec<_> = timeout_stream.collect().await;

        // Should get all 3 items without timeout because timer resets
        assert_eq!(items.len(), 3);
        assert!(items.iter().all(|item| item.is_ok()));
    }

    #[tokio::test]
    async fn test_bidirectional_stream_sender_clone() {
        let mut stream: BidirectionalStream<i32> = BidirectionalStream::new(10);

        let sender1 = stream.sender.clone();
        let sender2 = stream.sender.clone();
        let sender3 = stream.sender.clone();

        // Multiple senders can send
        sender1.send(1).await.unwrap();
        sender2.send(2).await.unwrap();
        sender3.send(3).await.unwrap();

        drop(sender1);
        drop(sender2);
        drop(sender3);
        // Also drop the original sender to close the channel
        drop(std::mem::replace(&mut stream.sender, mpsc::channel(1).0));

        let items: Vec<_> = stream.into_stream().collect().await;
        assert_eq!(items.len(), 3);
    }
}
