use core::cell::{Cell, UnsafeCell};
use core::fmt::{self, Debug, Formatter};
use core::future::Future;
use core::marker::PhantomPinned;
use core::mem::{self, MaybeUninit};
use core::pin::Pin;
use core::task::{Context, Poll};
use futures_core::{FusedStream, Stream};

#[doc(hidden)]
pub struct AsyncStream<T, F, Fut> {
    state: State<T, F, Fut>,
}

impl<T: Debug, F, Fut: Debug> Debug for AsyncStream<T, F, Fut> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncStream")
            .field("state", &self.state)
            .finish()
    }
}

/// The state of the async stream.
enum State<T, F, Fut> {
    /// The async stream has not yet been initialized. This holds the function that is used to
    /// initialize it.
    ///
    /// This state is necessary to allow the async stream to be soundly moved before `poll_next` is
    /// called for the first time.
    Uninit(F),

    /// The async stream has been initialized.
    ///
    /// This is an `UnsafeCell` to force the immutable borrow of its contents even when we have a
    /// mutable reference to it so that our mutable reference to it doesn't alias with the inner
    /// generator's reference to `Init::yielded`.
    Init(UnsafeCell<Init<T, Fut>>),
}

impl<T: Debug, F, Fut: Debug> Debug for State<T, F, Fut> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Uninit(_) => f.pad("Uninit"),
            Self::Init(init) => f
                .debug_tuple("Init")
                .field(unsafe { &*init.get() })
                .finish(),
        }
    }
}

/// An initialized async stream.
struct Init<T, Fut> {
    /// The last yielded item. The generator holds a pointer to this.
    yielded: Cell<Option<T>>,

    /// The generator itself. This is a `MaybeUninit` so that this type can be constructed with
    /// partial initialization - through all regular usage this is initialized. It is an
    /// `UnsafeCell` so we can get a mutable reference to it through the immutable reference
    /// provided by the outer `UnsafeCell`.
    generator: UnsafeCell<MaybeUninit<Fut>>,

    /// Whether the generator is done.
    done: bool,

    /// As the generator holds a pointer to `yielded`, this type cannot move in memory.
    _pinned: PhantomPinned,
}

impl<T: Debug, Fut: Debug> Debug for Init<T, Fut> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Init")
            .field(
                "generator",
                // Safety: We only create a shared reference to the data, and the only time a
                // mutable reference to this data is created is inside `poll_next` where we have a
                // `&mut Self` anyway.
                unsafe { &*(*self.generator.get()).as_ptr() },
            )
            .field("done", &self.done)
            .finish()
    }
}

impl<T, F, Fut> AsyncStream<T, F, Fut>
where
    F: FnOnce(Sender<T>) -> Fut,
    Fut: Future<Output = ()>,
{
    #[doc(hidden)]
    pub fn new(f: F) -> AsyncStream<T, F, Fut> {
        AsyncStream {
            state: State::Uninit(f),
        }
    }
}

impl<T, F, Fut> FusedStream for AsyncStream<T, F, Fut>
where
    Self: Stream,
{
    fn is_terminated(&self) -> bool {
        match &self.state {
            State::Uninit(_) => false,
            State::Init(init) => {
                // Safety: We never borrow this cell mutably from an immutable reference.
                unsafe { &*init.get() }.done
            }
        }
    }
}

impl<T, F, Fut> Stream for AsyncStream<T, F, Fut>
where
    F: FnOnce(Sender<T>) -> Fut,
    Fut: Future<Output = ()>,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = unsafe { Pin::into_inner_unchecked(self) };

        let init_cell = match &mut me.state {
            State::Uninit(_) => {
                let old_state = mem::replace(
                    &mut me.state,
                    State::Init(UnsafeCell::new(Init {
                        yielded: Cell::new(None),
                        generator: UnsafeCell::new(MaybeUninit::uninit()),
                        done: false,
                        _pinned: PhantomPinned,
                    })),
                );
                let f = match old_state {
                    State::Uninit(f) => f,
                    _ => unreachable!(),
                };
                let init_cell = match &mut me.state {
                    State::Init(init) => init,
                    _ => unreachable!(),
                };
                let init = unsafe_cell_get_mut(init_cell);
                let sender = Sender {
                    ptr: &init.yielded as *const _,
                };
                init.generator = UnsafeCell::new(MaybeUninit::new(f(sender)));
                init_cell
            }
            State::Init(init) => {
                if unsafe_cell_get_mut(init).done {
                    return Poll::Ready(None);
                }
                init
            }
        };

        // Immutably borrow `init`. If we mutably borrowed `init` here it would cause UB as this
        // mutable reference to `init.yielded` would alias with the generator's.
        let init = unsafe { &*init_cell.get() };

        let generator = unsafe { &mut *(*init.generator.get()).as_mut_ptr() };

        // Miri sometimes does not like this because of
        // <https://github.com/rust-lang/rust/issues/63818>. However this is acceptable because the
        // same unsoundness can be triggered with safe code:
        // https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=43b4a4f3d7e9287c73821b27084fb179
        // And so we know that once the safe code stops being UB (which will happen), this code will
        // also stop being UB.
        let res = unsafe { Pin::new_unchecked(generator) }.poll(cx);

        // Now that the generator no longer will use its pointer to `init.yielded`, we can create a
        // mutable reference.
        let init = unsafe_cell_get_mut(init_cell);

        if res.is_ready() {
            init.done = true;
        }

        if let Some(yielded) = init.yielded.take() {
            return Poll::Ready(Some(yielded));
        }

        match res {
            Poll::Ready(()) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct Sender<T> {
    ptr: *const Cell<Option<T>>,
}

impl<T> Sender<T> {
    pub fn send(&mut self, value: T) -> impl Future<Output = ()> {
        unsafe { &*self.ptr }.set(Some(value));

        SendFut { yielded: false }
    }
}

impl<T> Debug for Sender<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.pad("Sender")
    }
}

// Sender alone wouldn't be Send, however since we know it is only ever inside the generator if it
// is sent to another thread the AsyncStream it is inside will be too.
unsafe impl<T> Send for Sender<T> {}

struct SendFut {
    yielded: bool,
}
impl Future for SendFut {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.yielded {
            Poll::Ready(())
        } else {
            self.yielded = true;
            Poll::Pending
        }
    }
}

/// A helper function to reduce usages of `unsafe`.
fn unsafe_cell_get_mut<T: ?Sized>(cell: &mut UnsafeCell<T>) -> &mut T {
    unsafe { &mut *cell.get() }
}
