use once_fn::once;

#[once]
fn foo(x: impl std::fmt::Display) -> String {
    x.to_string()
}

fn main() {}
