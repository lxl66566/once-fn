//! an async once fn caches the awaited result; later calls skip the body

use std::sync::atomic::{AtomicUsize, Ordering};

use once_fn::once;

static RUNS: AtomicUsize = AtomicUsize::new(0);

struct Foo(bool);

impl Foo {
    fn new() -> Foo {
        Foo(true)
    }
    fn next(&mut self) -> bool {
        self.0 = !self.0;
        self.0
    }
}

#[once]
async fn foo(f: &mut Foo) -> bool {
    RUNS.fetch_add(1, Ordering::SeqCst);
    tokio::task::yield_now().await;
    f.next()
}

#[tokio::test]
async fn caches_the_awaited_result() {
    let mut f = Foo::new();
    for i in 0..10 {
        // the first call flips `f` and caches `false`; later calls return the
        // cached value without touching `f` or re-running the body
        assert!(!foo(&mut f).await, "call {i}");
        assert_eq!(RUNS.load(Ordering::SeqCst), 1, "body ran again at call {i}");
    }
    assert!(!f.0, "later calls must not touch the argument");
}
