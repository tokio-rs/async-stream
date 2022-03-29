mod next;

/// Asynchronous stream
///
/// See [crate](index.html) documentation for more details.
///
/// # Examples
///
/// ```
/// use async_stream::stream;
///
/// use futures_util::pin_mut;
/// use futures_util::stream::StreamExt;
///
/// #[tokio::main]
/// async fn main() {
///     let s = stream! {
///         for i in 0..3 {
///             yield i;
///         }
///     };
///
///     pin_mut!(s); // needed for iteration
///
///     while let Some(value) = s.next().await {
///         println!("got {}", value);
///     }
/// }
/// ```
#[cfg_attr(doc_nightly, doc(cfg(feature = "macro")))]
#[macro_export]
macro_rules! stream {
    ($($tt:tt)*) => {
        $crate::async_stream_impl::stream_inner!(($crate) $($tt)*)
    }
}

/// Asynchronous fallible stream
///
/// See [crate](index.html) documentation for more details.
///
/// # Examples
///
/// ```
/// use tokio::net::{TcpListener, TcpStream};
///
/// use async_stream::try_stream;
/// use futures_core::stream::Stream;
///
/// use std::io;
/// use std::net::SocketAddr;
///
/// fn bind_and_accept(addr: SocketAddr)
///     -> impl Stream<Item = io::Result<TcpStream>>
/// {
///     try_stream! {
///         let mut listener = TcpListener::bind(addr).await?;
///
///         loop {
///             let (stream, addr) = listener.accept().await?;
///             println!("received on {:?}", addr);
///             yield stream;
///         }
///     }
/// }
/// ```
#[cfg_attr(doc_nightly, doc(cfg(feature = "macro")))]
#[macro_export]
macro_rules! try_stream {
    ($($tt:tt)*) => {
        $crate::async_stream_impl::try_stream_inner!(($crate) $($tt)*)
    }
}

#[doc(hidden)]
pub mod reexport {
    pub use super::next::next;
}
