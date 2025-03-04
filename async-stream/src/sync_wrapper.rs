use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A wrapper around `T` that only allows mutable access.
///
/// This allows it to unconditionally implement `Sync`, since there is nothing
/// you can do with an `&SyncWrapper<T>`.
pub(crate) struct SyncWrapper<T> {
    inner: T,
}

impl<T> SyncWrapper<T> {
    pub(crate) fn new(value: T) -> Self {
        Self { inner: value }
    }

    pub(crate) fn get_pinned_mut(self: Pin<&mut Self>) -> Pin<&mut T> {
        // We can't use pin_project! for this because it generates a project_ref
        // method which would allow accessing the inner element
        //
        // SAFETY: this.inner is guaranteed not to move as long as this lives.
        unsafe { self.map_unchecked_mut(|this| &mut this.inner) }
    }
}

// SAFETY: It is not possible to do anything with an &SyncWrapper<T> so it is
//         safe for it to be shared between threads.
//
// See [0] for more details.
//
// [0]: https://internals.rust-lang.org/t/what-shall-sync-mean-across-an-await/12020/2
unsafe impl<T> Sync for SyncWrapper<T> {}

impl<T: Future> Future for SyncWrapper<T> {
    type Output = T::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_pinned_mut().poll(cx)
    }
}

impl<T> fmt::Debug for SyncWrapper<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // We can't format the inner value (since that would create an &T reference)
        // so we just print a placeholder string.

        f.write_str("<opaque future>")
    }
}
