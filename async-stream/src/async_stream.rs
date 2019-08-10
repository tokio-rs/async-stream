use crate::yielder::Receiver;

use futures_core::Stream;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct AsyncStream<T, U> {
    rx: Receiver<T>,
    generator: U,
}

impl<T, U> AsyncStream<T, U> {
    pub fn new(rx: Receiver<T>, generator: U) -> AsyncStream<T, U> {
        AsyncStream { rx, generator }
    }
}

impl<T, U> Stream for AsyncStream<T, U>
where U: Future<Output = ()>
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        unimplemented!();
    }
}
