#![feature(async_await)]

use async_stream::try_stream;

use tokio::prelude::*;

#[tokio::test]
async fn single_err() {
    let s = try_stream! {
        if true {
            Err("hello")?;
        } else {
            yield "world";
        }

        unreachable!();
    };

    let values: Vec<_> = s.collect().await;
    assert_eq!(1, values.len());
    assert_eq!(Err("hello"), values[0]);
}

#[tokio::test]
async fn yield_then_err() {
    let s = try_stream! {
        yield "hello";
        Err("world")?;
        unreachable!();
    };

    let values: Vec<_> = s.collect().await;
    assert_eq!(2, values.len());
    assert_eq!(Ok("hello"), values[0]);
    assert_eq!(Err("world"), values[1]);
}
