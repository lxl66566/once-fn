//! cancelling the first call mid-initialization lets the next call re-run the
//! body and finish it

use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Duration,
};

use once_fn::once;

static TIMEOUT_RUNS: AtomicUsize = AtomicUsize::new(0);
static ABORT_RUNS: AtomicUsize = AtomicUsize::new(0);
static ABORT_STARTED: AtomicBool = AtomicBool::new(false);

#[once]
async fn slow_timeout() -> u32 {
    TIMEOUT_RUNS.fetch_add(1, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(50)).await;
    7
}

#[once]
async fn slow_abort() -> u32 {
    ABORT_RUNS.fetch_add(1, Ordering::SeqCst);
    ABORT_STARTED.store(true, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(50)).await;
    8
}

#[tokio::test]
async fn timeout_cancels_init_then_next_call_completes() {
    let first = tokio::time::timeout(Duration::from_millis(5), slow_timeout());
    assert!(first.await.is_err()); // dropped while the body is suspended
    assert_eq!(TIMEOUT_RUNS.load(Ordering::SeqCst), 1);

    let second = tokio::time::timeout(Duration::from_secs(1), slow_timeout()).await;
    assert_eq!(second.unwrap(), 7); // a fresh run completes the cache
    assert_eq!(TIMEOUT_RUNS.load(Ordering::SeqCst), 2);

    assert_eq!(slow_timeout().await, 7); // cached now
    assert_eq!(TIMEOUT_RUNS.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn abort_cancels_init_then_next_call_completes() {
    let task = tokio::spawn(slow_abort());
    // wait until the body actually started, so the abort lands mid-init
    while !ABORT_STARTED.load(Ordering::SeqCst) {
        tokio::task::yield_now().await;
    }
    task.abort();
    let _ = task.await;
    assert_eq!(ABORT_RUNS.load(Ordering::SeqCst), 1);

    let second = tokio::time::timeout(Duration::from_secs(1), slow_abort()).await;
    assert_eq!(second.unwrap(), 8);
    assert_eq!(ABORT_RUNS.load(Ordering::SeqCst), 2);
}
