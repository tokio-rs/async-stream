#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub
)]
#![doc(test(no_crate_inject, attr(deny(rust_2018_idioms))))]
#![cfg_attr(doc_nightly, feature(doc_cfg))]

//! Asynchronous stream of elements.
//!
//! Provides two functions, `stream` and `try_stream`, allowing the caller to
//! define asynchronous streams of elements. These are implemented using `async`
//! & `await` notation. This crate works without unstable features.
//!
//! The `stream` function returns an anonymous type implementing the [`Stream`]
//! trait. The `Item` associated type is the type of the values yielded from the
//! stream. The `try_stream` also returns an anonymous type implementing the
//! [`Stream`] trait, but the `Item` associated type is `Result<T, Error>`. The
//! `try_stream` function supports using `?` notation as part of the
//! implementation.
//!
//! # Function Usage
//!
//! A basic stream yielding numbers. Values are yielded using the `yield`
//! keyword. The stream block must return `()`.
//!
//! ```rust
//! use async_stream::stream;
//!
//! use futures_util::pin_mut;
//! use futures_util::stream::StreamExt;
//!
//! #[tokio::main]
//! async fn main() {
//!     let s = stream(|mut stream| async move {
//!         for i in 0..3 {
//!             stream.yield_item(i).await;
//!         }
//!     });
//!
//!     pin_mut!(s); // needed for iteration
//!
//!     while let Some(value) = s.next().await {
//!         println!("got {}", value);
//!     }
//! }
//! ```
//!
//! Streams may be returned by using `impl Stream<Item = T>`:
//!
//! ```rust
//! use async_stream::stream;
//!
//! use futures_core::stream::Stream;
//! use futures_util::pin_mut;
//! use futures_util::stream::StreamExt;
//!
//! fn zero_to_three() -> impl Stream<Item = u32> {
//!     stream(|mut stream| async move {
//!         for i in 0..3 {
//!             stream.yield_item(i).await;
//!         }
//!     })
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let s = zero_to_three();
//!     pin_mut!(s); // needed for iteration
//!
//!     while let Some(value) = s.next().await {
//!         println!("got {}", value);
//!     }
//! }
//! ```
//!
//! Rust try notation (`?`) can be used with the `try_stream` function. The
//! `Item` of the returned stream is `Result` with `Ok` being the value yielded
//! and `Err` the error type returned by `?`.
//!
//! ```rust
//! use tokio::net::{TcpListener, TcpStream};
//!
//! use async_stream::try_stream;
//! use futures_core::stream::Stream;
//!
//! use std::io;
//! use std::net::SocketAddr;
//!
//! fn bind_and_accept(addr: SocketAddr)
//!     -> impl Stream<Item = io::Result<TcpStream>>
//! {
//!     try_stream(move |mut stream| async move {
//!         let mut listener = TcpListener::bind(addr).await?;
//!
//!         loop {
//!             let (tcp_stream, addr) = listener.accept().await?;
//!             println!("received on {:?}", addr);
//!             stream.yield_item(tcp_stream).await;
//!         }
//!     })
//! }
//! ```
//!
//! # Macro usage
//!
//! When the `macro` Cargo feature flag is enabled, this crate additionally
//! provides two macros, `stream!` and `try_stream!`, that are identical to
//! their equivalent functions but have a slightly nicer syntax where you can
//! write `yield x` instead of `stream.yield_item(x).await`. For example:
//!
//! ```
//! use async_stream::stream;
//!
//! use futures_core::stream::Stream;
//!
//! fn zero_to_three() -> impl Stream<Item = u32> {
//!     stream! {
//!         for i in 0..3 {
//!             yield i;
//!         }
//!     }
//! }
//! ```
//!
//! Streams may be implemented in terms of other streams - the macros provide
//! `for await` syntax to assist with this:
//!
//! ```rust
//! use async_stream::stream;
//!
//! use futures_core::stream::Stream;
//! use futures_util::pin_mut;
//! use futures_util::stream::StreamExt;
//!
//! fn zero_to_three() -> impl Stream<Item = u32> {
//!     stream! {
//!         for i in 0..3 {
//!             yield i;
//!         }
//!     }
//! }
//!
//! fn double<S: Stream<Item = u32>>(input: S)
//!     -> impl Stream<Item = u32>
//! {
//!     stream! {
//!         for await value in input {
//!             yield value * 2;
//!         }
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let s = double(zero_to_three());
//!     pin_mut!(s); // needed for iteration
//!
//!     while let Some(value) = s.next().await {
//!         println!("got {}", value);
//!     }
//! }
//! ```
//!
//! # Implementation
//!
//! The streams use a lightweight sender to send values from the stream
//! implementation to the caller. When entering the stream, an `Option<T>` is
//! stored on the stack. A pointer to the slot is stored in a thread local and
//! `poll` is called on the future. When `yield` is called, the value is stored
//! in there and the stream yields back to the caller.
//!
//! The `stream!` and `try_stream!` macros are implemented using proc macros.
//! The macro searches the syntax tree for instances of `yield $expr` and
//! transforms them into `stream.yield_item($expr).await`.
//!
//! # Supported Rust Versions
//!
//! `async-stream` is built against the latest stable release. The minimum
//! supported version is 1.45 due to [function-like procedural macros in
//! expression, pattern, and statement positions][1.45].
//!
//! [`Stream`]: https://docs.rs/futures-core/*/futures_core/stream/trait.Stream.html
//! [1.45]: https://blog.rust-lang.org/2020/07/16/Rust-1.45.0.html#stabilizing-function-like-procedural-macros-in-expressions-patterns-and-statements

#[cfg(feature = "macro")]
mod macros;

// Used by the macro, but not intended to be accessed publicly.
#[cfg(feature = "macro")]
#[doc(hidden)]
pub use {::async_stream_impl, macros::reexport};

mod functions;
pub use functions::{stream, try_stream, Stream};
