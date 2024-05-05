use std::cell::Cell;
use std::future::Future;
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::pin::Pin;
use std::ptr;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

#[derive(Debug)]
pub struct Sender<T> {
    _p: PhantomData<T>,
}

#[derive(Debug)]
pub struct Receiver<T> {
    _p: PhantomData<T>,
}

// Note: It is considered unsound for anyone other than our macros to call
// this function. This is a private API intended only for calls from our
// macros, and users should never call it, but some people tend to
// misinterpret it as fine to call unless it is marked unsafe.
#[doc(hidden)]
pub unsafe fn pair<T>() -> (Sender<T>, Receiver<T>) {
    let tx = Sender { _p: PhantomData };
    let rx = Receiver { _p: PhantomData };
    (tx, rx)
}

// Tracks the pointer from `&'a Cell<Option<T>>`.
struct WakerWrapper<'a> {
    waker: &'a Waker,
    out_ref: *const (),
}

static STREAM_VTABLE: RawWakerVTable =
    RawWakerVTable::new(vtable_clone, vtable_wake, vtable_wake_by_ref, vtable_drop);

unsafe fn vtable_clone(p: *const ()) -> RawWaker {
    // clone the inner waker
    let waker = ManuallyDrop::new((*p.cast::<WakerWrapper<'_>>()).waker.clone());
    let raw = waker.as_raw();
    RawWaker::new(raw.data(), raw.vtable())
}

unsafe fn vtable_wake(_p: *const ()) {
    unreachable!("Futures can't obtain this internal waker by value")
}

unsafe fn vtable_wake_by_ref(p: *const ()) {
    (*p.cast::<WakerWrapper<'_>>()).waker.wake_by_ref();
}

unsafe fn vtable_drop(_p: *const ()) {
    unreachable!("Futures can't obtain this internal waker by value")
}

// ===== impl Sender =====

impl<T> Sender<T> {
    pub fn send(&mut self, value: T) -> impl Future<Output = ()> {
        Send { value: Some(value) }
    }
}

struct Send<T> {
    value: Option<T>,
}

impl<T> Unpin for Send<T> {}

impl<T> Future for Send<T> {
    type Output = ();

    fn poll<'a>(mut self: Pin<&mut Self>, cx: &mut Context<'a>) -> Poll<()> {
        if self.value.is_none() {
            return Poll::Ready(());
        }

        let waker = cx.waker().as_raw();
        assert!(
            ptr::eq(waker.vtable(), &STREAM_VTABLE),
            "internal context wrapper is altered"
        );

        let out_ref = unsafe {
            let wrapper = &*waker.data().cast::<WakerWrapper<'a>>();
            &*wrapper.out_ref.cast::<Cell<Option<T>>>()
        };

        let prev = out_ref.take();

        if prev.is_none() {
            out_ref.set(self.value.take())
        } else {
            out_ref.set(prev)
        }

        Poll::Pending
    }
}

// ===== impl Receiver =====

impl<T> Receiver<T> {
    pub(crate) fn with_context<'a, U>(
        &'a mut self,
        waker: &'a Waker,
        dst: &'a mut Option<T>,
        f: impl FnOnce(&mut Context<'_>) -> U,
    ) -> U {
        let wrapper = WakerWrapper {
            waker,
            out_ref: Cell::from_mut(dst) as *const Cell<Option<T>> as *const (),
        };
        let raw = RawWaker::new(
            &wrapper as *const WakerWrapper<'a> as *const (),
            &STREAM_VTABLE,
        );
        let waker = ManuallyDrop::new(unsafe { Waker::from_raw(raw) });
        f(&mut Context::from_waker(&waker))
    }
}
