use once_fn::once_impl;

struct Boxed<T>(T);

#[once_impl]
impl<T> Boxed<T> {
    #[once]
    fn make() -> u32 {
        1
    }
}

fn main() {}
