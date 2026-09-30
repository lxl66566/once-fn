use once_fn::once;

#[once(by_ref)]
fn foo(b: &bool) -> &bool {
    b
}

fn main() {}
