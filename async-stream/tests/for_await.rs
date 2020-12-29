use async_stream::stream;

use futures_util::stream::StreamExt;

#[test]
// We can remove this once <https://github.com/rust-lang/rust/issues/63818> is fixed.
#[cfg(not(miri))]
fn test() {
    futures_executor::block_on(async {
        let s = stream! {
            yield "hello";
            yield "world";
        };

        let s = stream! {
            for await x in s {
                yield x.to_owned() + "!";
            }
        };

        tokio::pin!(s);
        let values: Vec<_> = s.collect().await;

        assert_eq!(2, values.len());
        assert_eq!("hello!", values[0]);
        assert_eq!("world!", values[1]);
    });
}
