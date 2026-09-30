//! non-once members of an `#[once_impl]` block are preserved verbatim

use once_fn::once_impl;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Foo {
    base: u32,
}

impl Foo {
    fn outside_impl(&self, x: u32) -> u32 {
        self.base + x
    }
}

#[once_impl]
impl Foo {
    const ID: u32 = 7;

    fn plain(&self, x: u32) -> u32 {
        self.base + x
    }

    #[once]
    fn cached(x: u32) -> u32 {
        x * 2
    }

    #[once]
    fn singleton() -> Self {
        Foo { base: 99 }
    }

    #[once]
    fn pair(x: u32) -> Vec<Self> {
        vec![Foo { base: x }, Foo { base: x + 1 }]
    }
}

trait HasPair {
    type Pair;

    fn pair(&self) -> Self::Pair;

    fn cached_trait(x: u32) -> u32;
}

#[once_impl]
impl HasPair for Foo {
    type Pair = (u32, u32);

    fn pair(&self) -> Self::Pair {
        (self.base, Self::ID)
    }

    #[once]
    fn cached_trait(x: u32) -> u32 {
        x * 3
    }
}

#[test]
fn members_preserved() {
    let foo = Foo { base: 10 };
    assert_eq!(foo.outside_impl(1), 11);
    assert_eq!(Foo::ID, 7);
    assert_eq!(foo.plain(5), 15);
    assert_eq!(Foo::cached(21), 42);
    assert_eq!(Foo::cached(1), 42); // returns the cached value

    // `Self` in the return type resolves to the impl's self type
    assert_eq!(Foo::singleton(), Foo { base: 99 });
    assert_eq!(Foo::pair(7), vec![Foo { base: 7 }, Foo { base: 8 }]);
    assert_eq!(Foo::pair(1), vec![Foo { base: 7 }, Foo { base: 8 }]); // cached

    let pair: <Foo as HasPair>::Pair = foo.pair();
    assert_eq!(pair, (10, 7));
    assert_eq!(<Foo as HasPair>::cached_trait(7), 21);
    assert_eq!(<Foo as HasPair>::cached_trait(1), 21); // returns the cached value
}
