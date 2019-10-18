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

#[tokio::test]
async fn convert_err() {
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
}
