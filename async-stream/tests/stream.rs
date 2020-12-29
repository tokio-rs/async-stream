use async_stream::stream;

use futures_core::stream::{FusedStream, Stream};
use futures_util::pin_mut;
use futures_util::stream::StreamExt;
use tokio::sync::mpsc;
use tokio_test::assert_ok;

#[test]
fn noop_stream() {
    futures_executor::block_on(async {
        let s = stream! {};
        pin_mut!(s);

        while let Some(_) = s.next().await {
            unreachable!();
        }
    });
}

#[test]
fn empty_stream() {
    futures_executor::block_on(async {
        let mut ran = false;

        {
            let r = &mut ran;
            let s = stream! {
                *r = true;
                println!("hello world!");
            };
            pin_mut!(s);

            while let Some(_) = s.next().await {
                unreachable!();
            }
        }

        assert!(ran);
    });
}

#[test]
fn yield_single_value() {
    futures_executor::block_on(async {
        let s = stream! {
            yield "hello";
        };

        let values: Vec<_> = s.collect().await;

        assert_eq!(1, values.len());
        assert_eq!("hello", values[0]);
    });
}

#[test]
fn fused() {
    futures_executor::block_on(async {
        let s = stream! {
            yield "hello";
        };
        pin_mut!(s);

        assert!(!s.is_terminated());
        assert_eq!(s.next().await, Some("hello"));
        assert_eq!(s.next().await, None);

        assert!(s.is_terminated());
        // This should return None from now on
        assert_eq!(s.next().await, None);
    });
}

#[test]
fn yield_multi_value() {
    futures_executor::block_on(async {
        let s = stream! {
            yield "hello";
            yield "world";
            yield "dizzy";
        };

        let values: Vec<_> = s.collect().await;

        assert_eq!(3, values.len());
        assert_eq!("hello", values[0]);
        assert_eq!("world", values[1]);
        assert_eq!("dizzy", values[2]);
    });
}

#[test]
fn return_stream() {
    fn build_stream() -> impl Stream<Item = u32> {
        stream! {
            yield 1;
            yield 2;
            yield 3;
        }
    }

    futures_executor::block_on(async {
        let s = build_stream();

        let values: Vec<_> = s.collect().await;
        assert_eq!(3, values.len());
        assert_eq!(1, values[0]);
        assert_eq!(2, values[1]);
        assert_eq!(3, values[2]);
    });
}

#[test]
fn consume_channel() {
    futures_executor::block_on(async {
        let (tx, mut rx) = mpsc::channel(10);

        let s = stream! {
            while let Some(v) = rx.recv().await {
                yield v;
            }
        };

        pin_mut!(s);

        for i in 0..3 {
            assert_ok!(tx.send(i).await);
            assert_eq!(Some(i), s.next().await);
        }

        drop(tx);
        assert_eq!(None, s.next().await);
    });
}

#[test]
fn borrow_self() {
    futures_executor::block_on(async {
        struct Data(String);

        impl Data {
            fn stream<'a>(&'a self) -> impl Stream<Item = &str> + 'a {
                stream! {
                    yield &self.0[..];
                }
            }
        }

        let data = Data("hello".to_string());
        let s = data.stream();
        pin_mut!(s);

        assert_eq!(Some("hello"), s.next().await);
    });
}

#[test]
// We can remove this once <https://github.com/rust-lang/rust/issues/63818> is fixed.
#[cfg(not(miri))]
fn stream_in_stream() {
    futures_executor::block_on(async {
        let s = stream! {
            let s = stream! {
                for i in 0..3 {
                    yield i;
                }
            };

            pin_mut!(s);
            while let Some(v) = s.next().await {
                yield v;
            }
        };

        let values: Vec<_> = s.collect().await;
        assert_eq!(3, values.len());
    });
}

#[test]
#[cfg(not(miri))]
fn test() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
