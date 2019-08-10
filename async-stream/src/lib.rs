#![feature(async_await)]

mod async_stream;
#[doc(hidden)]
pub mod yielder;

// Used by the macro, but not intended to be accessed publically.
#[doc(hidden)]
pub use crate::{
    async_stream::AsyncStream,
};

use proc_macro_hack::proc_macro_hack;

#[doc(hidden)]
#[proc_macro_hack]
pub use async_stream_impl::async_stream_impl;

/// Asynchronous stream
#[macro_export]
macro_rules! stream {
    ($($body:tt)*) => {{
        let (mut __yield_tx, __yield_rx) = $crate::yielder::pair();
        $crate::AsyncStream::new(__yield_rx, async move {
            $crate::async_stream_impl!(__yield_tx, $($body)*)
        })
    }}
}
