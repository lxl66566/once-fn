use once_fn::once_impl;
struct Foo;

#[once_impl]
impl Foo {
    /// comment will be kept
    #[once]
    pub fn foo(b: bool) -> bool {
        b
    }
}

trait Bar {
    fn foo(b: bool) -> bool;
}

#[once_impl]
impl Bar for Foo {
    #[once]
    fn foo(b: bool) -> bool {
        b
    }
}

#[test]
fn test() {
    assert!(Foo::foo(true));
    assert!(Foo::foo(false));

    // They are actually different functions, so they do not share the same cache

    assert!(<Foo as Bar>::foo(true));
    assert!(<Foo as Bar>::foo(false));
}
