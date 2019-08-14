#![feature(async_await)]

//! Asynchronous stream of elements.
//!
//! Provides two macros, `stream!` and `try_stream!`, allowing the caller to
//! define asynchronous streams of elements. These are implemented using `async`
//! & `await` notation. The `stream!` macro works using only
//! `#[feature(async_await)]`.
//!
//! The `stream!` macro returns an anonymous type implementing the [`Stream`]
//! trait. The `Item` associated type is the type of the values yielded from the
//! stream. The `try_stream!` also returns an anonymous type implementing the
//! [`Stream`] trait, but the `Item` associated type is `Result<T, Error>`. The
//! `try_stream!` macro supports using `?` notiation as part of the
//! implementation.
//!
//! # Usage
//!
//! A basic stream yielding numbers. Values are yielded using the `yield`
//! keyword. The stream block must return `()`.
//!
//! ```rust
//! #![feature(async_await)]
//!
//! use tokio::prelude::*;
//!
//! use async_stream::stream;
//! use futures_util::pin_mut;
//!
//! #[tokio::main]
//! async fn main() {
//!     let s = stream! {
//!         for i in 0..3 {
//!             yield i;
//!         }
//!     };
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
//! #![feature(async_await)]
//!
//! use tokio::prelude::*;
//!
//! use async_stream::stream;
//! use futures_util::pin_mut;
//!
//! fn zero_to_three() -> impl Stream<Item = u32> {
//!     stream! {
//!         for i in 0..3 {
//!             yield i;
//!         }
//!     }
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
//! Streams may be implemented in terms of other streams:
//!
//! ```rust
//! #![feature(async_await)]
//!
//! use tokio::prelude::*;
//!
//! use async_stream::stream;
//! use futures_util::pin_mut;
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
//!         pin_mut!(input);
//!         while let Some(value) = input.next().await {
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
//! Rust try notation (`?`) can be used with the `try_stream!` macro. The `Item`
//! of the returned stream is `Result` with `Ok` being the value yielded and
//! `Err` the error type returned by `?`.
//!
//! ```rust
//! #![feature(async_await)]
//!
//! use tokio::net::{TcpListener, TcpStream};
//! use tokio::prelude::*;
//!
//! use async_stream::try_stream;
//! use std::io;
//! use std::net::SocketAddr;
//!
//! fn bind_and_accept(addr: SocketAddr)
//!     -> impl Stream<Item = io::Result<TcpStream>>
//! {
//!     try_stream! {
//!         let mut listener = TcpListener::bind(&addr)?;
//!
//!         loop {
//!             let (stream, addr) = listener.accept().await?;
//!             println!("received on {:?}", addr);
//!             yield stream;
//!         }
//!     }
//! }
//! ```
//!
//! # Implementation
//!
//! The `stream!` and `try_stream!` macros are implemented using proc macros.
//! Given that proc macros in expression position are not supported on stable
//! rust, a hack similar to the one provided by the [`proc-macro-hack`] crate is
//! used. The macro searches the syntax tree for instances of `sender.send($expr)` and
//! transforms them into `sender.send($expr).await`.
//!
//! The stream uses a lightweight sender to send values from the stream
//! implementation to the caller. When entering the stream, an `Option<T>` is
//! stored on the stack. A pointer to the cell is stored in a thread local and
//! `poll` is called on the async block. When `poll` returns.
//! `sender.send(value)` stores the value that cell and yields back to the
//! caller.
//!
//! # Limitations
//!
//! `async-stream` suffers from the same limitations as the [`proc-macro-hack`]
//! crate. Primarily, nesting support must be implemented using a `TT-muncher`.
//! If large `stream!` blocks are used, the caller will be required to add
//! `#![recursion_limit = "..."]` to their crate.
//!
//! A `stream!` macro may only contain up to 64 macro invocations.
//!
//! [`Stream`]: https://docs.rs/futures-core-preview/*/futures_core/stream/trait.Stream.html
//! [`proc-macro-hack`]: https://github.com/dtolnay/proc-macro-hack/

mod async_stream;
mod next;
#[doc(hidden)]
pub mod yielder;

// Used by the macro, but not intended to be accessed publically.
#[doc(hidden)]
pub use crate::{
    async_stream::AsyncStream,
};

#[doc(hidden)]
pub use async_stream_impl::{AsyncStreamHack, AsyncTryStreamHack};

#[doc(hidden)]
pub mod reexport {
    #[doc(hidden)]
    pub use crate::next::next;
    #[doc(hidden)]
    pub use std::pin::Pin;
    #[doc(hidden)]
    pub use std::option::Option::{Some, None};
}

/// Asynchronous stream
///
/// See [crate](index.html) documentation for more details.
///
/// # Examples
///
/// ```rust
/// #![feature(async_await)]
///
/// use tokio::prelude::*;
///
/// use async_stream::stream;
/// use futures_util::pin_mut;
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
#[macro_export]
macro_rules! stream {
    ($($body:tt)*) => {{
        let (mut __yield_tx, __yield_rx) = $crate::yielder::pair();
        $crate::AsyncStream::new(__yield_rx, async move {
            #[derive($crate::AsyncStreamHack)]
            enum Dummy {
                Value = $crate::scrub! { $($body)* }
            }

            $crate::dispatch!(($($body)*))
        })
    }}
}

/// Asynchronous fallible stream
///
/// See [crate](index.html) documentation for more details.
///
/// # Examples
///
/// ```rust
/// #![feature(async_await)]
///
/// use tokio::net::{TcpListener, TcpStream};
/// use tokio::prelude::*;
///
/// use async_stream::try_stream;
/// use std::io;
/// use std::net::SocketAddr;
///
/// fn bind_and_accept(addr: SocketAddr)
///     -> impl Stream<Item = io::Result<TcpStream>>
/// {
///     try_stream! {
///         let mut listener = TcpListener::bind(&addr)?;
///
///         loop {
///             let (stream, addr) = listener.accept().await?;
///             println!("received on {:?}", addr);
///             yield stream;
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! try_stream {
    ($($body:tt)*) => {{
        let (mut __yield_tx, __yield_rx) = $crate::yielder::pair();
        $crate::AsyncStream::new(__yield_rx, async move {
            #[derive($crate::AsyncTryStreamHack)]
            enum Dummy {
                Value = $crate::scrub! { $($body)* }
            }

            $crate::dispatch!(($($body)*))
        })
    }}
}

#[doc(hidden)]
#[macro_export]
macro_rules! scrub {
    ($($body:tt)*) => {{
        0
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! dispatch {
    (() $($bang:tt)*) => {
        $crate::count!($($bang)*)
    };
    ((($($first:tt)*) $($rest:tt)*) $($bang:tt)*) => {
        $crate::dispatch!(($($first)* $($rest)*) $($bang)*)
    };
    (([$($first:tt)*] $($rest:tt)*) $($bang:tt)*) => {
        $crate::dispatch!(($($first)* $($rest)*) $($bang)*)
    };
    (({$($first:tt)*} $($rest:tt)*) $($bang:tt)*) => {
        $crate::dispatch!(($($first)* $($rest)*) $($bang)*)
    };
    ((! $($rest:tt)*) $($bang:tt)*) => {
        $crate::dispatch!(($($rest)*) $($bang)* !)
    };
    ((!= $($rest:tt)*) $($bang:tt)*) => {
        $crate::dispatch!(($($rest)*) $($bang)* !)
    };
    (($first:tt $($rest:tt)*) $($bang:tt)*) => {
        $crate::dispatch!(($($rest)*) $($bang)*)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! count {
    () => { stream_0!() };
    (!) => { stream_1!() };
    (!!) => { stream_2!() };
    (!!!) => { stream_3!() };
    (!!!!) => { stream_4!() };
    (!!!!!) => { stream_5!() };
    (!!!!!!) => { stream_6!() };
    (!!!!!!!) => { stream_7!() };
    (!!!!!!!!) => { stream_8!() };
    (!!!!!!!!!) => { stream_9!() };
    (!!!!!!!!!!) => { stream_10!() };
    (!!!!!!!!!!!) => { stream_11!() };
    (!!!!!!!!!!!!) => { stream_12!() };
    (!!!!!!!!!!!!!) => { stream_13!() };
    (!!!!!!!!!!!!!!) => { stream_14!() };
    (!!!!!!!!!!!!!!!) => { stream_15!() };
    (!!!!!!!!!!!!!!!!) => { stream_16!() };
    (!!!!!!!!!!!!!!!!!) => { stream_17!() };
    (!!!!!!!!!!!!!!!!!!) => { stream_18!() };
    (!!!!!!!!!!!!!!!!!!!) => { stream_19!() };
    (!!!!!!!!!!!!!!!!!!!!) => { stream_20!() };
    (!!!!!!!!!!!!!!!!!!!!!) => { stream_21!() };
    (!!!!!!!!!!!!!!!!!!!!!!) => { stream_22!() };
    (!!!!!!!!!!!!!!!!!!!!!!!) => { stream_23!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_24!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_25!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_26!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_27!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_28!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_29!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_30!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_31!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_32!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_33!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_34!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_35!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_36!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_37!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_38!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_39!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_40!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_41!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_42!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_43!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_44!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_45!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_46!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_47!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_48!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_49!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_50!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_51!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_52!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_53!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_54!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_55!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_56!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_57!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_58!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_59!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_60!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_61!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_62!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_63!() };
    (!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!) => { stream_64!() };
}
