//! concurrent first calls run the body exactly once

use std::{
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Duration,
};

use once_fn::once;

static RUNS: AtomicUsize = AtomicUsize::new(0);

#[once]
fn slow() -> usize {
    RUNS.fetch_add(1, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(50));
    7
}

#[test]
fn body_runs_once_under_contention() {
    let handles: Vec<_> = (0..8).map(|_| thread::spawn(slow)).collect();
    for handle in handles {
        assert_eq!(handle.join().unwrap(), 7);
    }
    assert_eq!(RUNS.load(Ordering::SeqCst), 1);

    assert_eq!(slow(), 7);
    assert_eq!(RUNS.load(Ordering::SeqCst), 1);
}
