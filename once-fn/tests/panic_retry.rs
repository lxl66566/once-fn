//! a panicking initializer is retried on the next call

use std::{
    panic::catch_unwind,
    sync::atomic::{AtomicUsize, Ordering},
};

use once_fn::once;

static CALLS: AtomicUsize = AtomicUsize::new(0);

#[once]
fn flaky() -> u32 {
    let calls = CALLS.fetch_add(1, Ordering::SeqCst) + 1;
    assert!(calls != 1, "first call must fail");
    42
}

#[test]
fn panic_then_retry() {
    assert!(catch_unwind(flaky).is_err());
    assert_eq!(CALLS.load(Ordering::SeqCst), 1);

    assert_eq!(flaky(), 42); // the initializer runs again and succeeds
    assert_eq!(CALLS.load(Ordering::SeqCst), 2);

    assert_eq!(flaky(), 42); // cached now
    assert_eq!(CALLS.load(Ordering::SeqCst), 2);
}
