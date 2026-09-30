//! A runtime-agnostic asynchronous once cell.

use std::{
    cell::Cell,
    future::Future,
    mem,
    pin::Pin,
    sync::{Mutex, MutexGuard},
    task::{Context, Poll, Waker},
};

/// An asynchronous once cell: the first future returned by
/// [`get_or_init`](AsyncOnceCell::get_or_init) to be polled runs its
/// initializer to completion and caches the output; every other poll of any
/// future created for the cell waits for that value and resolves to a shared
/// reference to it.
///
/// Unlike [`std::sync::OnceLock`], initialization happens inside the polling
/// task: the initializer future is polled in the context of the first caller,
/// so no thread blocks, no task is spawned and no runtime is required.
///
/// # Cancellation and panics
///
/// If the initializing future is dropped before it finishes (the awaiting task
/// was cancelled) or its poll panics, the cell rolls back to empty and the
/// next call runs the initializer again, so a body passed to `get_or_init` may
/// run more than once. This matches the retry semantics of
/// [`std::sync::OnceLock`].
///
/// # Reentrancy
///
/// A future that re-enters the same cell's `get_or_init` while that cell is
/// initializing would wait for itself; such a call panics with a clear message
/// instead of deadlocking. Initializing a different cell from inside an
/// initializer is supported.
///
/// # Examples
///
/// ```
/// use once_fn::AsyncOnceCell;
///
/// static CELL: AsyncOnceCell<u32> = AsyncOnceCell::new();
///
/// #[tokio::main]
/// async fn main() {
///     let a = CELL.get_or_init(|| async { 7 }).await;
///     let b = CELL.get_or_init(|| async { unreachable!() }).await;
///     assert_eq!((a, b), (&7, &7));
/// }
/// ```
pub struct AsyncOnceCell<T> {
    state: Mutex<State<T>>,
}

enum State<T> {
    /// No initializer has run; the next poll claims initialization.
    New,
    /// An initializer future exists and is being driven to completion.
    Init { wakers: Vec<Waker> },
    /// Terminal state: the value is cached and never written again.
    Ready(T),
}

// SAFETY: `Send`: a cell may move to another thread, carrying a stored `T`.
// `Sync`: `get_or_init` hands `&T` out through `&self` to arbitrary threads,
// which requires `T: Sync`, and initialization moves a `T` produced on one
// thread into state shared by all of them, which requires `T: Send`.
unsafe impl<T: Send> Send for AsyncOnceCell<T> {}
unsafe impl<T: Send + Sync> Sync for AsyncOnceCell<T> {}

thread_local! {
    /// Address of the cell whose user future is being polled on this thread
    /// (`0` = none). Addresses are only compared, never dereferenced. A stored
    /// address is observable only while the initializer's poll is on this
    /// thread's stack, and two live cells never share an address, so distinct
    /// cells cannot be confused here.
    static POLLING: Cell<usize> = const { Cell::new(0) };
}

/// Restores [`POLLING`] on scope exit, including on unwind: a panic escaping
/// a user future's poll must not leave a stale address behind.
struct RestorePolling(usize);

impl Drop for RestorePolling {
    fn drop(&mut self) {
        POLLING.with(|p| p.set(self.0));
    }
}

impl<T> AsyncOnceCell<T> {
    /// Create an empty cell.
    pub const fn new() -> Self {
        Self {
            state: Mutex::new(State::New),
        }
    }

    /// Return a future that resolves to a reference to the cached value,
    /// initializing the cell with `f` if needed.
    ///
    /// The first future to be polled calls `f` and polls the returned future
    /// in the caller's task context; futures polled while initialization is in
    /// flight park as waiters and are woken once the value is stored.
    ///
    /// # Panics
    ///
    /// Panics if the cell is re-entered from inside its own initializer, which
    /// would otherwise deadlock waiting for itself.
    pub fn get_or_init<F, Fut>(&self, f: F) -> GetOrInit<'_, T, F, Fut>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = T>,
    {
        GetOrInit {
            cell: self,
            f: Some(f),
            fut: None,
        }
    }

    /// Waiter-side reentrancy check: panics if this cell's initializer is on
    /// the current thread's stack, meaning the caller would wait for itself.
    fn assert_not_reentrant(&self) {
        let addr = self as *const Self as usize;
        if POLLING.with(|p| p.get()) == addr {
            panic!("an async once fn was re-entered while it is initializing; this would deadlock");
        }
    }

    /// # Safety
    ///
    /// `ptr` must point to the `Ready` payload stored in this cell. `Ready` is
    /// terminal: this type exposes no take or reset, so once stored the value
    /// is never moved, mutated or dropped while the cell lives, and the lock
    /// pair that stored it happens-before any later lock, so readers observe a
    /// fully initialized value. This is the only way a reference may outlive
    /// the state lock it was taken under.
    unsafe fn ready_ref(&self, ptr: *const T) -> &T {
        unsafe { &*ptr }
    }

    fn lock(&self) -> MutexGuard<'_, State<T>> {
        // a poisoned lock means an initializer panicked; the state is still
        // consistent, so keep going like `OnceLock` does
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl<T> Default for AsyncOnceCell<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Future returned by [`AsyncOnceCell::get_or_init`]; resolves to a shared
/// reference to the cached value.
pub struct GetOrInit<'a, T, F, Fut> {
    cell: &'a AsyncOnceCell<T>,
    /// The initializer; taken when this future claims initialization.
    f: Option<F>,
    /// The user future; `Some` once this future is the initializer.
    fut: Option<Pin<Box<Fut>>>,
}

// SAFETY: no field is structurally pinned. The user future is polled through
// `Pin<Box<Fut>>`, and moving this wrapper only moves the box, not the pinned
// allocation; `f` is only ever taken by value, never pinned. `poll` therefore
// does not rely on the caller keeping this future at a stable address.
impl<T, F, Fut> Unpin for GetOrInit<'_, T, F, Fut> {}

impl<'a, T, F, Fut> Future for GetOrInit<'a, T, F, Fut>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = T>,
{
    type Output = &'a T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // `Self: Unpin`, so no pin projection is needed.
        let this = self.get_mut();

        {
            let mut state = this.cell.lock();
            match &mut *state {
                State::Ready(v) => {
                    let ptr: *const T = v;
                    return Poll::Ready(unsafe { this.cell.ready_ref(ptr) });
                }
                State::New => *state = State::Init { wakers: Vec::new() },
                State::Init { wakers } if this.fut.is_none() => {
                    this.cell.assert_not_reentrant();
                    // the last waiter may already hold an equivalent waker
                    if !wakers.last().is_some_and(|w| w.will_wake(cx.waker())) {
                        wakers.push(cx.waker().clone());
                    }
                    return Poll::Pending;
                }
                State::Init { .. } => {}
            }
        }
        // the lock is released before any user code runs

        if this.fut.is_none() {
            let f = this
                .f
                .take()
                .expect("the initializer keeps its closure until the future is built");
            this.fut = Some(Box::pin(f()));
        }

        // Poll the user future in the caller's context: the waker it registers
        // drives this future to completion, so a `Pending` result needs no
        // bookkeeping of our own.
        let saved = POLLING.with(|p| p.replace(this.cell as *const _ as usize));
        let _restore = RestorePolling(saved);
        let out = this
            .fut
            .as_mut()
            .expect("initializer future")
            .as_mut()
            .poll(cx);
        match out {
            Poll::Pending => Poll::Pending,
            Poll::Ready(v) => {
                // Store the value and collect the waiters in one critical
                // section, then wake only after releasing the lock: a woken
                // task re-enters `poll` immediately and must not block on it.
                let (ptr, wakers): (*const T, Vec<Waker>) = {
                    let mut state = this.cell.lock();
                    let old = mem::replace(&mut *state, State::Ready(v));
                    let ptr = match &*state {
                        State::Ready(v) => v,
                        _ => unreachable!("`Ready` was just stored"),
                    };
                    match old {
                        State::Init { wakers } => (ptr, wakers),
                        _ => unreachable!("only the initializer stores `Ready`"),
                    }
                };
                for waker in wakers {
                    waker.wake();
                }
                Poll::Ready(unsafe { this.cell.ready_ref(ptr) })
            }
        }
    }
}

impl<T, F, Fut> Drop for GetOrInit<'_, T, F, Fut> {
    fn drop(&mut self) {
        // This future owns the `Init` state it claimed if it built the user
        // future, or if `f()` panicked after `f` was taken. A pure waiter
        // (`f` untouched, no future) owns nothing.
        if self.f.is_none() || self.fut.is_some() {
            let wakers = {
                let mut state = self.cell.lock();
                if let State::Init { wakers } = &mut *state {
                    let taken = mem::take(wakers);
                    *state = State::New;
                    taken
                } else {
                    // `Ready`: initialization completed through this future.
                    Vec::new()
                }
            };
            for waker in wakers {
                waker.wake();
            }
            // Drop the user future after the rollback and after waking: its
            // destructor may call back into this cell and must find `New`, not
            // an initializer that will never run again.
            drop(self.fut.take());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        future::Future,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll, Wake, Waker},
    };

    use super::*;

    /// A future that stays `Pending` until the shared flag is set. It ignores
    /// the passed waker: the tests decide when to poll again.
    struct Gate<T: Clone> {
        open: Rc<Cell<bool>>,
        value: T,
    }

    impl<T: Clone> Future for Gate<T> {
        type Output = T;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<T> {
            if self.open.get() {
                Poll::Ready(self.value.clone())
            } else {
                Poll::Pending
            }
        }
    }

    /// A future whose poll always panics.
    struct Boom;

    impl Future for Boom {
        type Output = u32;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<u32> {
            panic!("boom")
        }
    }

    /// A waker that counts how many times it was woken.
    struct CountWaker(Arc<AtomicUsize>);

    impl Wake for CountWaker {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
        payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .expect("panic payload is a string")
    }

    /// Poll an `Unpin` future in place.
    fn poll_unpin<F: Future + Unpin>(fut: &mut F, cx: &mut Context<'_>) -> Poll<F::Output> {
        Pin::new(fut).poll(cx)
    }

    #[test]
    fn initializer_and_waiter_share_one_run() {
        let runs = Rc::new(Cell::new(0u32));
        let open = Rc::new(Cell::new(false));
        let cell = AsyncOnceCell::new();
        let make_init = {
            let runs = runs.clone();
            let open = open.clone();
            move || {
                runs.set(runs.get() + 1);
                Gate {
                    open: open.clone(),
                    value: 42,
                }
            }
        };

        let mut init = cell.get_or_init(make_init);
        let mut waiter = cell.get_or_init(|| Gate {
            open: Rc::new(Cell::new(true)),
            value: 0,
        });

        let wakes = Arc::new(AtomicUsize::new(0));
        let waker = Waker::from(Arc::new(CountWaker(wakes.clone())));
        let mut waiter_cx = Context::from_waker(&waker);
        let mut init_cx = Context::from_waker(Waker::noop());

        assert_eq!(poll_unpin(&mut init, &mut init_cx), Poll::Pending); // body started, gate closed
        assert_eq!(runs.get(), 1);
        assert_eq!(poll_unpin(&mut waiter, &mut waiter_cx), Poll::Pending); // parked as waiter
        assert_eq!(wakes.load(Ordering::SeqCst), 0);

        open.set(true);
        let Poll::Ready(first) = poll_unpin(&mut init, &mut init_cx) else {
            panic!("initializer finished")
        };
        assert_eq!(*first, 42);
        assert_eq!(wakes.load(Ordering::SeqCst), 1); // waiter was woken

        let Poll::Ready(second) = poll_unpin(&mut waiter, &mut waiter_cx) else {
            panic!("waiter resolved")
        };
        assert!(std::ptr::eq(first, second)); // both share the cached value
        assert_eq!(runs.get(), 1);
    }

    #[test]
    fn dropping_the_initializer_rolls_back_and_wakes() {
        let runs = Rc::new(Cell::new(0u32));
        let open = Rc::new(Cell::new(false));
        let cell = AsyncOnceCell::new();
        let make = {
            let runs = runs.clone();
            let open = open.clone();
            move || {
                runs.set(runs.get() + 1);
                Gate {
                    open: open.clone(),
                    value: 42,
                }
            }
        };

        let wakes = Arc::new(AtomicUsize::new(0));
        let waker = Waker::from(Arc::new(CountWaker(wakes.clone())));
        let mut cx = Context::from_waker(&waker);

        let mut init = cell.get_or_init(make.clone());
        let mut waiter = cell.get_or_init(make.clone());
        let mut noop_cx = Context::from_waker(Waker::noop());
        assert_eq!(poll_unpin(&mut init, &mut noop_cx), Poll::Pending);
        assert_eq!(poll_unpin(&mut waiter, &mut cx), Poll::Pending);
        assert_eq!(runs.get(), 1);

        drop(init); // cancelled: the cell rolls back and wakes the waiter
        assert_eq!(wakes.load(Ordering::SeqCst), 1);

        // the waiter claims initialization and runs the body again
        assert_eq!(poll_unpin(&mut waiter, &mut cx), Poll::Pending);
        assert_eq!(runs.get(), 2);

        open.set(true);
        let Poll::Ready(v) = poll_unpin(&mut waiter, &mut cx) else {
            panic!("waiter finished its own initialization")
        };
        assert_eq!(*v, 42);

        // Ready is terminal: a later caller gets the value without a new run
        let mut late = cell.get_or_init(make);
        let Poll::Ready(late_v) = poll_unpin(&mut late, &mut cx) else {
            panic!("late caller resolved")
        };
        assert!(std::ptr::eq(v, late_v));
        assert_eq!(runs.get(), 2);
    }

    #[test]
    fn panic_in_the_initializer_is_retryable() {
        let runs = Rc::new(Cell::new(0u32));
        let cell = AsyncOnceCell::new();
        let mut first = {
            let runs = runs.clone();
            cell.get_or_init(move || {
                runs.set(runs.get() + 1);
                Boom
            })
        };
        let mut cx = Context::from_waker(Waker::noop());

        let panicked = catch_unwind(AssertUnwindSafe(|| poll_unpin(&mut first, &mut cx)));
        assert!(panicked.is_err());
        drop(first); // the aborted initializer rolls the cell back
        assert_eq!(runs.get(), 1);

        let mut second = {
            let runs = runs.clone();
            cell.get_or_init(move || {
                runs.set(runs.get() + 1);
                Gate {
                    open: Rc::new(Cell::new(true)),
                    value: 7,
                }
            })
        };
        let Poll::Ready(v) = poll_unpin(&mut second, &mut cx) else {
            panic!("second attempt succeeded")
        };
        assert_eq!(*v, 7);
        assert_eq!(runs.get(), 2);
    }

    #[test]
    fn panicking_initializer_closure_is_retryable() {
        // the closure itself panicking (between claiming `Init` and building
        // the future) must roll the cell back too
        let runs = Rc::new(Cell::new(0u32));
        let cell = AsyncOnceCell::new();
        let mut first = {
            let runs = runs.clone();
            cell.get_or_init(move || -> Gate<u32> {
                runs.set(runs.get() + 1);
                panic!("closure boom");
            })
        };
        let mut cx = Context::from_waker(Waker::noop());

        let panicked = catch_unwind(AssertUnwindSafe(|| poll_unpin(&mut first, &mut cx)));
        assert!(panicked.is_err());
        drop(first);
        assert_eq!(runs.get(), 1);

        let mut second = cell.get_or_init(|| Gate {
            open: Rc::new(Cell::new(true)),
            value: 5,
        });
        let Poll::Ready(v) = poll_unpin(&mut second, &mut cx) else {
            panic!("second attempt succeeded")
        };
        assert_eq!(*v, 5);
    }

    #[test]
    fn reentering_the_cell_panics() {
        struct Reenter<'a> {
            cell: &'a AsyncOnceCell<u32>,
        }

        impl Future for Reenter<'_> {
            type Output = u32;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
                // re-enter the same cell while its initializer is on the stack
                let mut inner = self.cell.get_or_init(|| Gate {
                    open: Rc::new(Cell::new(true)),
                    value: 1,
                });
                let _ = poll_unpin(&mut inner, cx);
                Poll::Ready(0)
            }
        }

        let cell = AsyncOnceCell::new();
        let mut init = cell.get_or_init(|| Reenter { cell: &cell });
        let mut cx = Context::from_waker(Waker::noop());

        let payload = catch_unwind(AssertUnwindSafe(|| poll_unpin(&mut init, &mut cx)))
            .expect_err("must panic");
        let message = panic_message(payload);
        assert!(
            message.contains("re-entered while it is initializing"),
            "unexpected message: {message}"
        );
        drop(init);
    }

    #[test]
    fn polling_distinct_cells_never_panics() {
        let a = AsyncOnceCell::new();
        let b = AsyncOnceCell::new();
        let open = Rc::new(Cell::new(false));
        let make = {
            let open = open.clone();
            move || Gate {
                open: open.clone(),
                value: 3,
            }
        };

        let mut fa = a.get_or_init(make.clone());
        let mut fb = b.get_or_init(make);
        let mut cx = Context::from_waker(Waker::noop());

        // interleaved polls of two initializing cells under a noop waker
        assert_eq!(poll_unpin(&mut fa, &mut cx), Poll::Pending);
        assert_eq!(poll_unpin(&mut fb, &mut cx), Poll::Pending);
        assert_eq!(poll_unpin(&mut fa, &mut cx), Poll::Pending);
        assert_eq!(poll_unpin(&mut fb, &mut cx), Poll::Pending);

        open.set(true);
        let Poll::Ready(va) = poll_unpin(&mut fa, &mut cx) else {
            panic!("a resolved")
        };
        let Poll::Ready(vb) = poll_unpin(&mut fb, &mut cx) else {
            panic!("b resolved")
        };
        assert_eq!((*va, *vb), (3, 3));
    }

    #[test]
    fn initializing_another_cell_from_an_initializer_is_allowed() {
        struct Nested<'b> {
            inner: &'b AsyncOnceCell<u32>,
        }

        impl Future for Nested<'_> {
            type Output = u32;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
                let mut inner = self.inner.get_or_init(|| Gate {
                    open: Rc::new(Cell::new(true)),
                    value: 9,
                });
                let Poll::Ready(v) = poll_unpin(&mut inner, cx) else {
                    return Poll::Pending;
                };
                Poll::Ready(v + 1)
            }
        }

        let outer = AsyncOnceCell::new();
        let inner = AsyncOnceCell::new();
        let mut fut = outer.get_or_init(|| Nested { inner: &inner });
        let mut cx = Context::from_waker(Waker::noop());

        let Poll::Ready(v) = poll_unpin(&mut fut, &mut cx) else {
            panic!("nested initialization completed inline")
        };
        assert_eq!(*v, 10);
        assert!(matches!(*inner.lock(), State::Ready(9)));
    }
}
