use async_stream::stream;
use async_stream::try_stream;
use std::cell::Cell;

fn assert_send_sync<T: Send + Sync>(_: T) {}

#[test]
fn functions() {
    let stream = stream(|mut stream| async move {
        stream.yield_item(Cell::new(0)).await;
    });
    assert_send_sync(stream);

    let stream = try_stream(|mut stream| async move {
        stream.yield_item(Cell::new(0)).await;
        Err(Cell::new(0))
    });
    assert_send_sync(stream);
}

#[cfg(feature = "macro")]
#[test]
fn macros() {
    let stream = stream! {
        yield Cell::new(0);
    };
    assert_send_sync(stream);

    let stream = try_stream! {
        yield Cell::new(0);
        return Err(Cell::new(0));
    };
    assert_send_sync(stream);
}
