use async_stream::stream;
use futures_util::stream::StreamExt;
use tokio::pin;

#[tokio::test]
async fn test() {
    let stream_1 = stream(|mut stream_1| async move {
        let mut stream_1_clone = stream_1.clone();
        let stream_2 = stream(|mut stream_2| async move {
            stream_1_clone.yield_item(1).await;
            stream_2.yield_item(2).await;
        });
        pin!(stream_2);
        assert_eq!(stream_2.next().await, Some(2));
        stream_1.yield_item(11).await;
        assert_eq!(stream_2.next().await, None);
    });
    pin!(stream_1);
    assert_eq!(stream_1.next().await, Some(1));
    assert_eq!(stream_1.next().await, Some(11));
    assert_eq!(stream_1.next().await, None);
}
