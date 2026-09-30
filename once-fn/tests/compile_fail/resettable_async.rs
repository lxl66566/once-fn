use once_fn::once;

#[once(resettable)]
async fn foo() -> u32 {
    1
}

fn main() {}
