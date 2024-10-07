use fn_once::once;

struct Foo(bool);

impl Foo {
    fn new() -> Foo {
        Foo(true)
    }

    fn get(&mut self) -> bool {
        self.0 = !self.0;
        self.0
    }
}

/// Test macro
#[once]
async fn foo(f: &mut Foo) -> bool {
    return f.get();
}

#[tokio::test]
async fn test() {
    let mut f = Foo::new();
    for _ in 0..10 {
        println!("{}", foo(&mut f));
    }
}
