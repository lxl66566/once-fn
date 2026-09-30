use once_fn::once;

#[once(by_ref, resettable)]
fn foo() -> u32 {
    7
}

fn main() {}
