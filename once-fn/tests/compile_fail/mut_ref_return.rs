use once_fn::once;

#[once]
fn foo(a: &mut u32) -> &mut u32 {
    a
}

fn main() {}
