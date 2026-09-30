//! `#[once(by_ref)]`: the owned return value is cached without `Clone` and
//! every call borrows the same cached slot

use std::sync::atomic::{AtomicUsize, Ordering};

use once_fn::{once, once_impl};

struct Big([u8; 4096]); // deliberately not Clone

static BIG_RUNS: AtomicUsize = AtomicUsize::new(0);
static METHOD_RUNS: AtomicUsize = AtomicUsize::new(0);
static ASYNC_RUNS: AtomicUsize = AtomicUsize::new(0);

#[once(by_ref)]
fn big(flag: u8) -> Big {
    BIG_RUNS.fetch_add(1, Ordering::SeqCst);
    Big([flag; 4096])
}

struct Config {
    value: u32,
}

#[once_impl]
impl Config {
    #[once(by_ref)]
    fn shared(x: u32) -> Self {
        METHOD_RUNS.fetch_add(1, Ordering::SeqCst);
        Config { value: x }
    }
}

#[once(by_ref)]
async fn answer(flag: u8) -> Big {
    ASYNC_RUNS.fetch_add(1, Ordering::SeqCst);
    tokio::task::yield_now().await;
    Big([flag; 4096])
}

#[test]
fn lends_the_cached_value_without_clone() {
    let a: &'static Big = big(7);
    let b = big(9); // cached: arguments of later calls have no effect
    assert!(std::ptr::eq(a, b));
    assert_eq!(a.0[0], 7);
    assert_eq!(BIG_RUNS.load(Ordering::SeqCst), 1);
}

#[test]
fn method_lends_self() {
    let a: &'static Config = Config::shared(1);
    let b = Config::shared(2);
    assert!(std::ptr::eq(a, b));
    assert_eq!(a.value, 1);
    assert_eq!(METHOD_RUNS.load(Ordering::SeqCst), 1);
}

#[once(by_ref)]
fn borrowed_args<'a, 'b>(a: &'a str, b: &'b str) -> String {
    // lifetime-only generics are fine: the cached value itself is 'static
    format!("{a}{b}")
}

#[once(by_ref)]
fn no_return() {
    // `fn f()` desugars to `-> ()`; lent as `&'static ()`
}

#[test]
fn lifetime_only_generics_and_unit_return_are_accepted() {
    assert_eq!(borrowed_args("a", "b"), "ab");
    assert_eq!(borrowed_args("x", "y"), "ab"); // cached
    let () = *no_return();
}

#[tokio::test]
async fn async_lends_the_awaited_value() {
    let a: &'static Big = answer(3).await;
    let b = answer(4).await; // cached
    assert!(std::ptr::eq(a, b));
    assert_eq!(a.0[0], 3);
    assert_eq!(ASYNC_RUNS.load(Ordering::SeqCst), 1);
}
