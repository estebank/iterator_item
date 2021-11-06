# Rust Iterator Items: a syntax exploration

This crate is a thin wrapper around the unstable generator feature, allowing users to create new 
items that act as generators. It follows the general semantics of the [Propane crate][propane], but my
interest for this crate is for interested people to fork it and come up with their own syntax for
these.

[propane]: https://github.com/withoutboats/propane

The initial syntax looks like this and needs to be surrounded by an invocation of the
`iterator_item` macro:

```rust
fn* foo() yields i32 {
    for n in 0i32..10 {
        yield n;
    }
}
```

Because it is a macro, it does not work as well as a native language feature would, and has worse
error messages, but some effort has been made to make them usable.

## Design decisions of propane

Because the semantics are heavily leaning on Propane, the following considerations also apply to
this crate.

Propane is designed to allow users to write generators for the purpose of implementing iterators.
For that reason, its generators are restricted in some important ways. These are the intentional
design restrictions of propane (that is, these are not limitations because of bugs, they are not
intended to be lifted):

1. A propane generator becomes a function that returns an `impl Iterator`; the iterator interface is
   the only interface users can use with the generator's return type.
2. A propane generator can only return `()`, it cannot yield one type and then return another
   interesting type. The `?` operator yields the error and then, on the next resumption, returns.
3. A propane generator implements Unpin, and cannot be self-referential (unlike async functions).

## Notes on the Unpin requirement

Because of the signature of `Iterator::next`, it is always safe to move iterators between calls to
`next`. This makes unboxed, self-referential iterators unsound. We did not have `Pin` when we
designed the Iterator API.

However, in general, users can push unowned data outside of the iterator in a way they can't with
futures. Futures, usually, ultimately have to be `'static`, so they can spawned, but iterators
usually are consumed in a way that does not require them to own all of their data.

Therefore, it is potentially the case that generators restricted to not contain self-references are
sufficient for this use case. Propane intends to explore that possibility.
