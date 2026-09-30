use once_fn::once;

#[once]
fn foo<T: Clone>(a: T) -> T {
    a
}

fn main() {}
