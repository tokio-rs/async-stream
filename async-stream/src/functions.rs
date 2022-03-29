use pin_project_lite::pin_project;
use std::cell::Cell;
use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::future::Future;
use std::marker::PhantomData;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::ptr::NonNull;
use std::task;
use std::task::Poll;

/// Create an asynchronous stream.
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
///     let s = stream(|mut stream| async move {
///         for i in 0..3 {
///             stream.yield_item(i).await;
///         }
///     });
///
///     pin_mut!(s); // needed for iteration
///
///     while let Some(value) = s.next().await {
///         println!("got {}", value);
///     }
/// }
/// ```
pub fn stream<T, F, Fut>(function: F) -> impl futures_core::FusedStream<Item = T>
where
    F: FnOnce(Stream<T>) -> Fut,
    Fut: Future<Output = ()>,
{
    AsyncStream::Initial { function }
}

/// Create a fallible asynchronous stream.
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
///     try_stream(move |mut stream| async move {
///         let mut listener = TcpListener::bind(addr).await?;
///
///         loop {
///             let (tcp_stream, addr) = listener.accept().await?;
///             println!("received on {:?}", addr);
///             stream.yield_item(tcp_stream).await;
///         }
///     })
/// }
/// ```
pub fn try_stream<T, E, F, Fut>(function: F) -> impl futures_core::FusedStream<Item = Result<T, E>>
where
    F: FnOnce(Stream<T>) -> Fut,
    Fut: Future<Output = Result<(), E>>,
{
    AsyncStream::Initial { function }
}

/// Either `()` or `Result<(), E>`. This is used to share the code behind `stream` and `try_stream`.
/// The `T` generic parameter represents the type yielded by the inner future (the type parameter
/// `T` of `Stream<T>`).
trait FutureOutput<T> {
    /// The item type that should be yielded by the containing stream.
    type Item;

    /// Convert each `T` into an item type for yielding.
    fn yield_item(item: T) -> Self::Item;

    /// Obtain the final item that may or may not be yielded by the stream after the future has
    /// completed.
    fn into_item(self) -> Option<Self::Item>;
}

impl<T> FutureOutput<T> for () {
    type Item = T;

    fn yield_item(item: T) -> Self::Item {
        item
    }

    fn into_item(self) -> Option<Self::Item> {
        None
    }
}

impl<T, E> FutureOutput<T> for Result<(), E> {
    type Item = Result<T, E>;

    fn yield_item(item: T) -> Self::Item {
        Ok(item)
    }

    fn into_item(self) -> Option<Self::Item> {
        match self {
            Ok(()) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

pin_project! {
    /// The actual implementation behind `stream` and `try_stream`.
    #[project = Proj]
    #[project_replace = ProjReplace]
    enum AsyncStream<T, F, Fut> {
        /// The stream hasn't started yet (before first poll).
        Initial {
            function: F,
        },
        /// The stream is currently running the inner future.
        Running {
            #[pin]
            future: Fut,
        },
        /// The stream has completed and will only yield `Poll::Ready(None)` from now on.
        Complete,
        /// This variant is never created and only exists for various phantom things.
        Phantoms {
            // The address of the `AsyncStream` needs to remain stable so that the yielder can
            // assert that it is sending the item to the correct stream.
            #[pin]
            _pinned: PhantomPinned,

            // Covariance because we contain a `FnOnce(Yielder<T>)` which is semantically a
            // `fn(fn(T))` that is covariant (as the contravariances of `fn(T)` cancel out).
            _covariant_input: Option<fn() -> T>,
        },
    }
}

// SAFETY: You cannot do anything with an `&AsyncStream`.
unsafe impl<T, F, Fut> Sync for AsyncStream<T, F, Fut> {}

impl<T, F, Fut> futures_core::Stream for AsyncStream<T, F, Fut>
where
    F: FnOnce(Stream<T>) -> Fut,
    Fut: Future,
    Fut::Output: FutureOutput<T>,
{
    type Item = <Fut::Output as FutureOutput<T>>::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        // Strict provenance-wise, this is identical to a call to `.addr()` since it is only used
        // for equality checking from this point onward (and never converted back into a pointer).
        let addr = self.as_ref().get_ref() as *const Self as usize;

        if let Proj::Initial { .. } = self.as_mut().project() {
            let function = match self.as_mut().project_replace(AsyncStream::Complete) {
                ProjReplace::Initial { function } => function,
                _ => unreachable!(),
            };
            let future = function(Stream {
                addr,
                _contravariant: PhantomData,
            });
            self.set(AsyncStream::Running { future });
        }

        let future = match self.as_mut().project() {
            Proj::Running { future } => future,
            Proj::Complete => return Poll::Ready(None),
            // Not possible as the above `if` just checked for it and replaced it.
            Proj::Initial { .. } => unreachable!(),
            // We never create this variant so this can't happen.
            Proj::Phantoms { .. } => unreachable!(),
        };

        let mut slot: Option<T> = None;

        let poll = CURRENT_CONTAINING_STREAM.with(|containing_stream| {
            struct Guard<'tls> {
                containing_stream: &'tls Cell<Option<ContainingStream>>,
                old: Option<ContainingStream>,
            }
            impl<'tls> Drop for Guard<'tls> {
                fn drop(&mut self) {
                    self.containing_stream.set(self.old.take());
                }
            }

            let mut guard = Guard {
                containing_stream,
                old: containing_stream.take(),
            };
            containing_stream.set(Some(ContainingStream {
                addr,
                slot: NonNull::from(&mut slot).cast::<()>(),
                next: NonNull::from(&mut guard.old),
            }));

            future.poll(cx)
        });

        match poll {
            Poll::Ready(output) => {
                self.set(AsyncStream::Complete);
                assert!(slot.is_none(), "yielded item as stream completed");
                Poll::Ready(output.into_item())
            }
            Poll::Pending => {
                if let Some(item) = slot {
                    Poll::Ready(Some(<Fut::Output as FutureOutput<T>>::yield_item(item)))
                } else {
                    Poll::Pending
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Complete => (0, Some(0)),
            _ => (0, None),
        }
    }
}

impl<T, F, Fut> futures_core::FusedStream for AsyncStream<T, F, Fut>
where
    F: FnOnce(Stream<T>) -> Fut,
    Fut: Future,
    Fut::Output: FutureOutput<T>,
{
    fn is_terminated(&self) -> bool {
        matches!(self, Self::Complete)
    }
}

/// Pointer to a stream that is currently being polled.
#[derive(Clone, Copy)]
struct ContainingStream {
    /// The address of the stream, used to verify that the stream is the correct one. This is never
    /// turned back into a pointer.
    addr: usize,

    /// A type-erased pointer to the `Option<T>` that holds the item about to be yielded.
    slot: NonNull<()>,

    /// A pointer to the containing `ContainingStream` in case multiple streams are being polled
    /// recursively. This forms a temporary stack-allocated linked list.
    next: NonNull<Option<ContainingStream>>,
}

thread_local! {
    static CURRENT_CONTAINING_STREAM: Cell<Option<ContainingStream>> = Cell::new(None);
}

/// The internal interface of an async stream.
pub struct Stream<T> {
    /// The address of the containing `AsyncStream`, used to ensure that we don't ever yield to the
    /// wrong stream.
    addr: usize,

    /// Contravariance because we are semantically a `fn(T)`.
    _contravariant: PhantomData<fn(T)>,
}

impl<T> Stream<T> {
    /// Yield an item to the containing stream.
    ///
    /// # Panics
    ///
    /// - Panics if this is called outside a stream's context.
    /// - Panics if an item has already been yielded in the stream's context.
    /// - Will result in further panics if the containing future completes before this has finished
    /// `.await`ing.
    pub async fn yield_item(&mut self, item: T) {
        const OUTSIDE_CONTEXT_PANIC: &str = "called `yield_item` outside of stream context";
        const DOUBLE_YIELD_PANIC: &str = "called `yield_item` twice in the same context";

        // Loop through the stack of streams that are currently being polled to try and find our
        // one. Although this is `O(n)` other than in pathological cases `n` will be very small.
        let mut option_stream = CURRENT_CONTAINING_STREAM.with(Cell::get);
        let mut slot = loop {
            let stream = option_stream.expect(OUTSIDE_CONTEXT_PANIC);
            if stream.addr == self.addr {
                break stream.slot.cast::<Option<T>>();
            }
            option_stream = unsafe { *stream.next.as_ref() };
        };

        let slot = unsafe { slot.as_mut() };
        assert!(slot.is_none(), "{}", DOUBLE_YIELD_PANIC);
        *slot = Some(item);

        PendOnce::default().await;
    }
}

// Intentionally `Clone` but not `Copy` (even though we could be) to prevent misuse (e.g. calling
// `yield_item` on one `Stream` in multiple arms of a `join!`).
impl<T> Clone for Stream<T> {
    fn clone(&self) -> Self {
        Self {
            addr: self.addr,
            _contravariant: PhantomData,
        }
    }
}

impl<T> Debug for Stream<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad("Stream")
    }
}

#[derive(Default)]
struct PendOnce(bool);

impl Future for PendOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, _cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        match self.0 {
            false => {
                self.0 = true;
                Poll::Pending
            }
            true => Poll::Ready(()),
        }
    }
}
