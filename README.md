# once-fn

This crate focuses on one simple thing: **make a function runs only once**. All subsequent calls will return the result of the first call.

## Limitations

- Return type must satisfy:
  - Implements `Clone`, or a reference points to a type that implements `Clone`.
  - could not be generics type or `impl` clause.
- Could not use in impl block.

## Why not

- `cached::proc_macro::once`
  - does not support async fn
  - does not support generics (in input)
  - does not support reference (in return type)
- `fn-once`
  - almost no docs
  - It can't even compile its example

## MSRV

1.61.0 (nightly), 1.70.0 (stable)

## todo

- [ ] support impl block
