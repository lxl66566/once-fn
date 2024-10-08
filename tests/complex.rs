//! test the most complex fn case

use fn_once::once;

#[derive(Clone, Debug)]
pub struct Foo(i32);
pub struct Bar {
    x: Foo,
    y: Foo,
}
pub trait BarTrait {
    fn get0(&self) -> &Foo;
    fn get1(&self) -> &Foo;
}

impl BarTrait for Bar {
    fn get0(&self) -> &Foo {
        &self.x
    }
    fn get1(&self) -> &Foo {
        &self.y
    }
}

#[once]
pub async unsafe fn foo<T>(f: Foo, b: T) -> Foo
where
    T: BarTrait,
{
    Foo(b.get0().0 + b.get1().0 + f.0)
}

#[tokio::test]
async fn complex() {
    unsafe {
        let x: Foo = foo(
            Foo(1),
            Bar {
                x: Foo(2),
                y: Foo(3),
            },
        )
        .await;
        assert_eq!(x.0, 6);
    }
}
