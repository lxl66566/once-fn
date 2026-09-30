use once_fn::once_impl;

struct Foo;

#[once_impl(foo)]
impl Foo {
    fn foo() -> u32 {
        1
    }
}

fn main() {}
