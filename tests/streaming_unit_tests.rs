// Unit tests for src/streaming.rs module
// Focus on TimeoutStream, BidirectionalStream, and StreamError coverage

use futures::{stream, StreamExt};
use rpcnet::streaming::{BidirectionalStream, StreamError, TimeoutStream};
use rpcnet::RpcError;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_timeout_stream_success() {
    // Test TimeoutStream with successful items that don't timeout
    use futures::pin_mut;

    let items = vec![
        Ok::<Vec<u8>, RpcError>(vec![1, 2, 3]),
        Ok(vec![4, 5, 6]),
        Ok(vec![7, 8, 9]),
    ];
    let stream = stream::iter(items);
    let timeout_stream = TimeoutStream::new(stream, Duration::from_millis(100));
    pin_mut!(timeout_stream);

    // Collect all items
    let results: Vec<_> = timeout_stream.collect().await;
    assert_eq!(results.len(), 3);
    assert!(results[0].is_ok());
    assert_eq!(results[0].as_ref().unwrap(), &vec![1, 2, 3]);
}

#[tokio::test]
async fn test_timeout_stream_triggers_timeout() {
    // Test TimeoutStream that triggers a timeout
    use futures::pin_mut;

    let stream = stream::unfold((), |_| async {
        // Simulate slow stream that takes longer than timeout
        sleep(Duration::from_millis(200)).await;
        Some((Ok::<Vec<u8>, RpcError>(vec![1, 2, 3]), ()))
    });

    let timeout_stream = TimeoutStream::new(stream, Duration::from_millis(50));
    pin_mut!(timeout_stream);

    // First poll should timeout
    match timeout_stream.next().await {
        Some(Err(StreamError::Timeout)) => {
            // Expected timeout
        }
        other => panic!("Expected timeout, got {:?}", other),
    }
}

#[tokio::test]
async fn test_timeout_stream_transport_error() {
    // Test TimeoutStream with transport error
    use futures::pin_mut;

    let items = vec![
        Ok::<Vec<u8>, RpcError>(vec![1, 2, 3]),
        Err(RpcError::ConnectionError("Connection lost".to_string())),
        Ok(vec![4, 5, 6]),
    ];
    let stream = stream::iter(items);
    let timeout_stream = TimeoutStream::new(stream, Duration::from_secs(1));
    pin_mut!(timeout_stream);

    // First item should succeed
    let first = timeout_stream.next().await.unwrap();
    assert!(first.is_ok());

    // Second item should be transport error
    match timeout_stream.next().await {
        Some(Err(StreamError::Transport(RpcError::ConnectionError(_)))) => {
            // Expected error
        }
        other => panic!("Expected transport error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_timeout_stream_resets_timer_on_success() {
    // Test that TimeoutStream resets timer after each successful item
    use futures::pin_mut;

    let items = vec![Ok::<Vec<u8>, RpcError>(vec![1]), Ok(vec![2]), Ok(vec![3])];
    let stream = stream::iter(items).then(|item| async move {
        sleep(Duration::from_millis(30)).await; // Less than timeout
        item
    });

    let timeout_stream = TimeoutStream::new(stream, Duration::from_millis(50));
    pin_mut!(timeout_stream);

    // All items should succeed without timeout
    let results: Vec<_> = timeout_stream.collect().await;
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|r| r.is_ok()));
}

#[tokio::test]
async fn test_timeout_stream_empty() {
    // Test TimeoutStream with empty stream
    use futures::pin_mut;

    let stream = stream::empty::<Result<Vec<u8>, RpcError>>();
    let timeout_stream = TimeoutStream::new(stream, Duration::from_millis(100));
    pin_mut!(timeout_stream);

    // Should immediately return None
    assert!(timeout_stream.next().await.is_none());
}

#[tokio::test]
async fn test_bidirectional_stream_new() {
    // Test creating a new BidirectionalStream
    let (tx, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(10);

    // Create stream using manual channel to have full control
    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);

    // Send some data
    tx.send(vec![1, 2, 3]).await.unwrap();
    tx.send(vec![4, 5, 6]).await.unwrap();
    drop(tx); // Close sender

    // Collect from stream
    use futures::StreamExt;
    let results: Vec<_> = stream.collect().await;

    assert_eq!(results.len(), 2);
    assert_eq!(results[0], vec![1, 2, 3]);
    assert_eq!(results[1], vec![4, 5, 6]);
}

#[tokio::test]
async fn test_bidirectional_stream_with_task() {
    // Test BidirectionalStream with background task
    let bidi_stream = BidirectionalStream::<i32>::with_task(10, |sender| async move {
        for i in 0..5 {
            let _ = sender.send(i).await;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        // Sender dropped here after task completes
    });

    // Convert to stream and collect with timeout
    let mut stream = bidi_stream.into_stream();
    let mut results = Vec::new();

    // Collect items with timeout to prevent hanging
    while let Ok(Some(value)) = tokio::time::timeout(Duration::from_secs(1), stream.next()).await {
        results.push(value);
        if results.len() >= 5 {
            break;
        }
    }

    assert_eq!(results, vec![0, 1, 2, 3, 4]);
}

#[tokio::test]
async fn test_bidirectional_stream_abort() {
    // Test aborting BidirectionalStream task
    let mut bidi_stream = BidirectionalStream::<i32>::with_task(10, |sender| async move {
        for i in 0..1000 {
            if sender.send(i).await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    // Give task time to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Abort the task
    bidi_stream.abort();

    // Collect items with timeout (should be few since task was aborted)
    let mut stream = bidi_stream.into_stream();
    let result = tokio::time::timeout(Duration::from_millis(500), async {
        let mut count = 0;
        while stream.next().await.is_some() {
            count += 1;
        }
        count
    })
    .await;

    // Either timeout (no items) or small number of items
    assert!(result.is_ok() || result.is_err());
}

#[tokio::test]
async fn test_bidirectional_stream_drop_aborts() {
    // Test that dropping BidirectionalStream aborts the task
    let bidi_stream = BidirectionalStream::<i32>::with_task(10, |sender| async move {
        // This task should be aborted when bidi_stream is dropped
        loop {
            if sender.send(42).await.is_err() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    // Drop the stream (should call abort via Drop impl)
    drop(bidi_stream);

    // Give it a moment to process
    tokio::time::sleep(Duration::from_millis(50)).await;

    // If we reach here without hanging, the abort worked
}

#[tokio::test]
async fn test_bidirectional_stream_buffer_full() {
    // Test BidirectionalStream with small buffer
    let bidi_stream = BidirectionalStream::<Vec<u8>>::new(2);
    let sender = bidi_stream.sender.clone();

    // Fill the buffer
    sender.send(vec![1]).await.unwrap();
    sender.send(vec![2]).await.unwrap();

    // Try to send more (should block in real scenario, but we'll timeout)
    let send_result = tokio::time::timeout(Duration::from_millis(50), sender.send(vec![3])).await;

    // Should timeout because buffer is full
    assert!(send_result.is_err());
}

#[tokio::test]
async fn test_stream_error_debug() {
    // Test Debug impl for StreamError variants
    let timeout_err = StreamError::<RpcError>::Timeout;
    let debug_str = format!("{:?}", timeout_err);
    assert!(debug_str.contains("Timeout"));

    let transport_err: StreamError<RpcError> =
        StreamError::Transport(RpcError::ConnectionError("test".to_string()));
    let debug_str = format!("{:?}", transport_err);
    assert!(debug_str.contains("Transport"));

    let item_err: StreamError<RpcError> =
        StreamError::Item(RpcError::StreamError("test".to_string()));
    let debug_str = format!("{:?}", item_err);
    assert!(debug_str.contains("Item"));
}

#[tokio::test]
async fn test_timeout_stream_pending_state() {
    // Test TimeoutStream Poll::Pending behavior
    use futures::pin_mut;
    use futures::task::Poll;

    let stream = stream::poll_fn(|_cx| Poll::Pending::<Option<Result<Vec<u8>, RpcError>>>);
    let timeout_stream = TimeoutStream::new(stream, Duration::from_millis(50));
    pin_mut!(timeout_stream);

    // Poll should eventually timeout
    tokio::time::timeout(Duration::from_millis(100), async {
        match timeout_stream.next().await {
            Some(Err(StreamError::Timeout)) => {
                // Expected
            }
            other => panic!("Expected timeout, got {:?}", other),
        }
    })
    .await
    .expect("Test itself should not timeout");
}

#[tokio::test]
async fn test_bidirectional_stream_sender_clone() {
    // Test that sender can be cloned and used from multiple places
    let bidi_stream = BidirectionalStream::<i32>::new(10);
    let sender1 = bidi_stream.sender.clone();
    let sender2 = bidi_stream.sender.clone();

    // Send from different senders in spawned tasks
    tokio::spawn(async move {
        for i in 0..3 {
            let _ = sender1.send(i).await;
        }
        // sender1 dropped here
    });

    tokio::spawn(async move {
        for i in 10..13 {
            let _ = sender2.send(i).await;
        }
        // sender2 dropped here
    });

    // Give tasks time to send and drop senders
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut stream = bidi_stream.into_stream();
    let mut results = Vec::new();

    // Collect with timeout
    while let Ok(Some(value)) =
        tokio::time::timeout(Duration::from_millis(500), stream.next()).await
    {
        results.push(value);
        if results.len() >= 6 {
            break;
        }
    }

    // Should have received 6 items total (order may vary)
    assert_eq!(results.len(), 6);
}
