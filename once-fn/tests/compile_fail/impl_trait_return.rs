use once_fn::once;

#[once]
fn foo(x: &u32) -> impl std::fmt::Display + use<'_> {
    *x
}

fn main() {}
