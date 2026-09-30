use once_fn::once;

struct Foo;

#[once]
impl Foo {
    fn foo() -> u32 {
        1
    }
}

fn main() {}
