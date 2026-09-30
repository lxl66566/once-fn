//! `Arc<T>` returns get cheap clones without requiring `T: Clone`

use std::sync::Arc;

use once_fn::once;

pub struct Big([u8; 4096]); // does not implement Clone

#[once]
fn big() -> Arc<Big> {
    Arc::new(Big([0; 4096]))
}

#[test]
fn arc_cache_is_shared() {
    let a = big();
    let b = big();
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(a.0.len(), 4096);
    assert_eq!(b.0.len(), 4096);
}
