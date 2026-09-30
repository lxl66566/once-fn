//! resettable mode: resetting drops the cached value and the next call re-runs

use std::sync::atomic::{AtomicUsize, Ordering};

use once_fn::once;

static RUNS: AtomicUsize = AtomicUsize::new(0);
static DROPS: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone)]
struct Tracked(u32);

impl Drop for Tracked {
    fn drop(&mut self) {
        DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

mod inner {
    use super::*;

    #[once(resettable)]
    pub fn make() -> Tracked {
        Tracked(RUNS.fetch_add(1, Ordering::SeqCst) as u32 + 1)
    }
}

#[test]
fn reset_reruns_and_drops() {
    let first = inner::make();
    assert_eq!(RUNS.load(Ordering::SeqCst), 1);
    assert_eq!(first.0, 1);

    let again = inner::make(); // cached, no new run
    assert_eq!(RUNS.load(Ordering::SeqCst), 1);
    assert_eq!(again.0, 1);

    inner::make_reset(); // public like `make`, drops the cached value
    assert_eq!(DROPS.load(Ordering::SeqCst), 1);

    let third = inner::make(); // runs again
    assert_eq!(RUNS.load(Ordering::SeqCst), 2);
    assert_eq!(DROPS.load(Ordering::SeqCst), 1);
    assert_eq!(third.0, 2);
}
