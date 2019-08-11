use crate::yielder::Receiver;

use futures_core::Stream;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

pub struct AsyncStream<T, U> {
    rx: Receiver<T>,
    done: bool,
    generator: U,
}

impl<T, U> AsyncStream<T, U> {
    pub fn new(rx: Receiver<T>, generator: U) -> AsyncStream<T, U> {
        AsyncStream { rx, done: false, generator }
    }
}

impl<T, U> Stream for AsyncStream<T, U>
where U: Future<Output = ()>
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        unsafe {
            let me = Pin::get_unchecked_mut(self);

            if me.done {
                panic!("poll after done");
            }

            let mut dst = None;
            let res = {
                let _enter = me.rx.enter(&mut dst);
                Pin::new_unchecked(&mut me.generator).poll(cx)
            };

            me.done = res.is_ready();

            if dst.is_some() {
                return Poll::Ready(dst.take());
            }

            if me.done {
                Poll::Ready(None)
            } else {
                Poll::Pending
            }
        }
    }
}
