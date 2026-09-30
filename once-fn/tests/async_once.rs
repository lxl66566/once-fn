//! async once fns: result caching, concurrent first calls, reference and Arc
//! returns

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use once_fn::once;

struct Big([u8; 4096]); // deliberately not Clone

static VALUE_RUNS: AtomicUsize = AtomicUsize::new(0);
static JOINED_RUNS: AtomicUsize = AtomicUsize::new(0);
static SHARED_RUNS: AtomicUsize = AtomicUsize::new(0);
static BIG_RUNS: AtomicUsize = AtomicUsize::new(0);

#[once]
async fn value(x: u32) -> u32 {
    VALUE_RUNS.fetch_add(1, Ordering::SeqCst);
    tokio::task::yield_now().await;
    x * 2
}

#[once]
async fn joined(x: u32) -> u32 {
    JOINED_RUNS.fetch_add(1, Ordering::SeqCst);
    tokio::task::yield_now().await; // suspend so the other calls park as waiters
    x + 1
}

#[once]
async fn shared(b: &u32) -> &u32 {
    SHARED_RUNS.fetch_add(1, Ordering::SeqCst);
    tokio::task::yield_now().await;
    b
}

#[once]
async fn big() -> Arc<Big> {
    BIG_RUNS.fetch_add(1, Ordering::SeqCst);
    tokio::task::yield_now().await;
    Arc::new(Big([7; 4096]))
}

#[tokio::test]
async fn caches_the_awaited_result() {
    assert_eq!(value(1).await, 2);
    for _ in 0..5 {
        assert_eq!(value(100).await, 2); // cached from the first call
    }
    assert_eq!(VALUE_RUNS.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn concurrent_first_calls_share_one_run() {
    let (a, b, c) = tokio::join!(joined(1), joined(2), joined(3));
    assert_eq!((a, b, c), (2, 2, 2));
    assert_eq!(JOINED_RUNS.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn reference_return_shares_the_cached_pointee() {
    let x = 5;
    let y = 9;
    let a = shared(&x).await;
    let b = shared(&y).await; // cached: still the first call's value
    assert_eq!(a, &5);
    assert_eq!(b, &5);
    assert!(std::ptr::eq(a, b));
    assert_eq!(SHARED_RUNS.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn arc_return_shares_one_allocation() {
    let a = big().await;
    let b = big().await;
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(a.0[0], 7);
    assert_eq!(BIG_RUNS.load(Ordering::SeqCst), 1);
}
