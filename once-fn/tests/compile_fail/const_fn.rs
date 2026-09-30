use once_fn::once;

#[once]
const fn foo() -> u32 {
    1
}

fn main() {}
