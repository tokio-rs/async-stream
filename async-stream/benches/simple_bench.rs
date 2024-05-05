use std::future::poll_fn;
use std::pin::pin;
use std::task::Poll;

use async_stream::stream;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use futures_util::{FutureExt, StreamExt};

const ITER: usize = 1000;
const NUM: usize = 42;

pub fn simple_bench(c: &mut Criterion) {
    c.bench_function("simple bench", |b| {
        b.iter(|| {
            let mut s = pin!(stream! {
                for _ in 0..ITER {
                    yield poll_fn(|_| black_box(Poll::Ready(NUM))).await;
                }
            });

            for _ in 0..ITER {
                assert_eq!(s.next().now_or_never(), Some(Some(NUM)));
            }

            assert_eq!(s.next().now_or_never(), Some(None));
        })
    });
}

criterion_group!(benches, simple_bench);
criterion_main!(benches);
