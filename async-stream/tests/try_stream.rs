use async_stream::try_stream;

use futures_core::stream::Stream;
use futures_util::stream::StreamExt;

#[test]
fn single_err() {
    futures_executor::block_on(async {
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
    });
}

#[test]
fn yield_then_err() {
    futures_executor::block_on(async {
        let s = try_stream! {
            yield "hello";
            Err("world")?;
            unreachable!();
        };

        let values: Vec<_> = s.collect().await;
        assert_eq!(2, values.len());
        assert_eq!(Ok("hello"), values[0]);
        assert_eq!(Err("world"), values[1]);
    });
}

#[test]
fn convert_err() {
    futures_executor::block_on(async {
        struct ErrorA(u8);
        #[derive(PartialEq, Debug)]
        struct ErrorB(u8);
        impl From<ErrorA> for ErrorB {
            fn from(a: ErrorA) -> ErrorB {
                ErrorB(a.0)
            }
        }

        fn test() -> impl Stream<Item = Result<&'static str, ErrorB>> {
            try_stream! {
                if true {
                    Err(ErrorA(1))?;
                } else {
                    Err(ErrorB(2))?;
                }
                yield "unreachable";
            }
        }

        let values: Vec<_> = test().collect().await;
        assert_eq!(1, values.len());
        assert_eq!(Err(ErrorB(1)), values[0]);
    });
}

#[test]
fn multi_try() {
    fn test() -> impl Stream<Item = Result<i32, String>> {
        try_stream! {
            let a = Ok::<_,  String>(Ok::<_,  String>(123))??;
            for _ in (1..10) {
                yield a;
            }
        }
    }
    futures_executor::block_on(async {
        let values: Vec<_> = test().collect().await;
        assert_eq!(9, values.len());
        assert_eq!(
            std::iter::repeat(123).take(9).map(Ok).collect::<Vec<_>>(),
            values
        );
    });
}
