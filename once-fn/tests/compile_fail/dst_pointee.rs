use once_fn::once;

#[once]
fn foo(s: &str) -> &str {
    s
}

fn main() {}
