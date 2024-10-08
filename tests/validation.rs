//! test if the output is correct

use once_fn::once;

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
    return f.next();
}

#[tokio::test]
async fn test() {
    let mut f = Foo::new();
    for _ in 0..10 {
        assert!(!foo(&mut f).await);
    }
}
