//! a recursive call during initialization panics with a clear message

use once_fn::once;

#[once]
fn fact(n: u64) -> u64 {
    if n <= 1 { 1 } else { n * fact(n - 1) }
}

#[test]
#[should_panic(expected = "re-entered while it is initializing")]
fn recursive_call_panics() {
    let _ = fact(3);
}
