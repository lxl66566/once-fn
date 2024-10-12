# once-fn

This crate focuses on one simple thing: **make a function runs only once**. All subsequent calls will return the result of the first call.

## Limitations

- Return type must satisfy:
  - Implements `Clone`, or a reference points to a type that implements `Clone`.
  - could not be generics type or `impl` clause.

## Example

```rust
use once_fn::once;

#[once]
pub fn foo(b: bool) -> bool {
    b
}
assert!(foo(true));  // set the return value to `true`
assert!(foo(false)); // will not run this function twice; directly return `true`.

// allows ref:
#[once]
pub fn foo2(b: &bool) -> &bool {
    b
}
```

for impl block:

```rust
use once_fn::once_impl;
struct Foo;

#[once_impl]
impl Foo {
    #[once]
    pub fn foo(b: bool) -> bool {
        b
    }
}
```

see [tests](./tests/) for more examples.

## Why not

- `cached::proc_macro::once`
  - does not support async fn
  - does not support generics (in input)
  - does not support reference (in return type)
  - does not support use in impl block
- `fn-once`
  - Almost no docs; I don't know what it actually do.
  - It can't even compile its example

## MSRV

1.61.0 (nightly), 1.70.0 (stable)

## todo

- [x] support impl block
