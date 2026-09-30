use once_fn::once;

#[once(something)]
fn foo() -> u32 {
    1
}

fn main() {}
