//! test if a reference is in the return type

use once_fn::once;

#[once]
fn foo(f: &bool) -> &bool {
    f
}

#[once]
fn foo_with_lifetime(f: &'static bool) -> &'static bool {
    f
}

#[test]
fn test() {
    let f = true;
    let f = Box::leak(Box::new(f));
    assert_eq!(foo(f), f);
    assert_eq!(foo_with_lifetime(f), f);
}
