use once_fn::once;

#[once(by_ref)]
fn foo(s: &str) -> std::borrow::Cow<'_, str> {
    std::borrow::Cow::Borrowed(s)
}

fn main() {}
