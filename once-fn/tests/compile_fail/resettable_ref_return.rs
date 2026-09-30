use once_fn::once;

#[once(resettable)]
fn foo(b: &bool) -> &bool {
    b
}

fn main() {}
