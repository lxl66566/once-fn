//! the most complex async fn case: lifetimes, unsafe, traits, references and
//! an async method returning `Self`

use std::sync::atomic::{AtomicUsize, Ordering};

use once_fn::{once, once_impl};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Foo(i32);

pub struct Bar {
    x: Foo,
    y: Foo,
}

pub trait BarTrait {
    fn get0(&self) -> &Foo;
    fn get1(&self) -> &Foo;
}

impl BarTrait for Bar {
    fn get0(&self) -> &Foo {
        &self.x
    }

    fn get1(&self) -> &Foo {
        &self.y
    }
}

static FOO_RUNS: AtomicUsize = AtomicUsize::new(0);
static FIRST_RUNS: AtomicUsize = AtomicUsize::new(0);

#[once]
pub async unsafe fn foo<'a>(f: Foo, b: &'a Bar) -> Foo {
    FOO_RUNS.fetch_add(1, Ordering::SeqCst);
    tokio::task::yield_now().await;
    Foo(b.get0().0 + b.get1().0 + f.0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Config {
    value: u32,
}

#[once_impl]
impl Config {
    #[once]
    pub async fn first(x: u32) -> Self {
        FIRST_RUNS.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        Config { value: x }
    }
}

#[tokio::test]
async fn complex_free_fn() {
    unsafe {
        let x: Foo = foo(Foo(1), &Bar {
            x: Foo(2),
            y: Foo(3),
        })
        .await;
        assert_eq!(x, Foo(6));

        // cached: same result even with different arguments, body not re-run
        let y: Foo = foo(Foo(100), &Bar {
            x: Foo(0),
            y: Foo(0),
        })
        .await;
        assert_eq!(y, Foo(6));
        assert_eq!(FOO_RUNS.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn complex_impl_method() {
    assert_eq!(Config::first(1).await, Config { value: 1 });
    assert_eq!(Config::first(2).await, Config { value: 1 }); // cached from the first call
    assert_eq!(FIRST_RUNS.load(Ordering::SeqCst), 1);
}
