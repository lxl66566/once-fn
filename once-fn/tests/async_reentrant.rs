//! awaiting an async once fn from inside its own initializing body panics with
//! a clear message instead of deadlocking

use once_fn::once;

#[once]
async fn reenter() -> u32 {
    reenter().await
}

#[tokio::test]
#[should_panic(expected = "re-entered while it is initializing")]
async fn reentrant_call_panics() {
    reenter().await;
}
