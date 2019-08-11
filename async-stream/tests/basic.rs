#![feature(async_await)]

use async_stream::stream;

use tokio::prelude::*;
use futures_util::pin_mut;

#[tokio::test]
async fn noop_stream() {
    let s = stream! {};
    pin_mut!(s);

    while let Some(_) = s.next().await {
        unreachable!();
    }
}

#[tokio::test]
async fn empty_stream() {
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
}

#[tokio::test]
async fn yield_single_value() {
    let s = stream! {
        yield "hello";
    };

    let values: Vec<_> = s.collect().await;

    assert_eq!(1, values.len());
    assert_eq!("hello", values[0]);
}

#[tokio::test]
async fn yield_multi_value() {
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
}

#[tokio::test]
async fn return_stream() {
    fn build_stream() -> impl Stream<Item = u32> {
        stream! {
            yield 1;
            yield 2;
            yield 3;
        }
    }

    let s = build_stream();

    let values: Vec<_> = s.collect().await;
    assert_eq!(3, values.len());
    assert_eq!(1, values[0]);
    assert_eq!(2, values[1]);
    assert_eq!(3, values[2]);
}
