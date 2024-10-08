# fn-once

This crate focuses on one simple thing: make a function runs only once. All subsequent calls will return the result of the first call.

## Limitations

- Return type must impls `Clone` and could not be generics or `impl` clause.
- Could not use in impl block.

## Compare

### Compare to cached::proc_macro::once

This crate:

- support async
- support generics (in input)

## todo

- [ ] support impl block
