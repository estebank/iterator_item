//! This library provides a macro to define iterator items, A.K.A. generators.
//!
//! It is intended to explore the design space of the syntax for generators. More
//! documentation can be found in the description of the macro.
#![feature(generator_trait, try_trait_v2)]
#![cfg_attr(feature = "std_async_iter", async_stream)]
#![no_std]

/// This macro can be used to make functions that function as generators.
///
/// Functions annotated with this macro can use the `yield` keyword to give the next element in a
/// sequence. They yield an item and then continue. When you call a generator function, you get an
/// iterator of the type it yields, rather than just one value of that type.
///
/// You can still use the `return` keyword to terminate the generator early, but the `return`
/// keyword cannot take a value; it only terminates the function.
///
/// The behavior of `?` is also modified in these functions. In the event of an error, the
/// generator yields the error value, and then the next time it is resumed it returns `None`.
///
/// ## Forbidding self-references
///
/// Unlike async functions, generators cannot contain self-references: a reference into their stack
/// space that exists across a yield point. Instead, anything you wish to have by reference you
/// should move out of the state of the generator, taking it as an argument, or else not holding it
/// by reference across a point that you yield.
///
/// ## Unstable features
///
/// In order to use this attribute, you must turn on all of these features:
/// - `generators`
/// - `generator_trait`
/// - `async_stream`, if enabling feature `std_async_iter` (WIP)
///
/// ## Example
///
/// ```rust
/// #![feature(generators, generator_trait)]
/// # use iterator_item::iterator_item;
///
/// iterator_item! {
///     gen fn fizz_buzz() -> String {
///        for x in 1..101 {
///           match (x % 3 == 0, x % 5 == 0) {
///               (true, true)  => yield String::from("FizzBuzz"),
///               (true, false) => yield String::from("Fizz"),
///               (false, true) => yield String::from("Buzz"),
///               (..)          => yield x.to_string(),
///           }
///        }
///     }
/// }
///
/// fn main() {
///     let mut fizz_buzz = fizz_buzz();
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "1");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "2");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "Fizz");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "4");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "Buzz");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "Fizz");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "7");
///
///     // yada yada yada
///     let mut fizz_buzz = fizz_buzz.skip(90);
///
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "98");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "Fizz");
///     assert_eq!(&fizz_buzz.next().unwrap()[..], "Buzz");
///     assert!(fizz_buzz.next().is_none());
/// }
/// ```
///
/// The intention of this crate is for people to fork it and submit alternative syntax for this
/// feature that they believe would make for a better user experience.
pub use iterator_item_macros::iterator_item;

#[doc(hidden)]
pub mod __internal {
    use core::marker::Unpin;
    use core::ops::{Generator, GeneratorState};
    use core::pin::Pin;
    use core::task::{Context, Poll};
    #[cfg(not(feature = "std_async_iter"))]
    pub use futures::stream::{Stream, StreamExt};

    /// New-type wrapper around the unstable `Generator` opaque type.
    ///
    /// The final version of this type in `std`, if needed, would *also* not be be either
    /// perma-unstable to use directly, or another opaque type. This is used to both give us a way
    /// to `impl Iterator` and somewhere to hold the computed `size_hint` value.
    pub struct IteratorItem<G: Generator<Return = ()> + Unpin> {
        pub gen: G,
        pub size_hint: (usize, Option<usize>),
    }

    impl<G: Generator<Return = ()> + Unpin> Iterator for IteratorItem<G> {
        type Item = G::Yield;

        fn next(&mut self) -> Option<Self::Item> {
            match Pin::new(&mut self.gen).resume(()) {
                GeneratorState::Yielded(item) => Some(item),
                GeneratorState::Complete(()) => None,
            }
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.size_hint
        }
    }

    /// New-type wrapper around the unstable `Generator` opaque type.
    ///
    /// The final version of this type in `std`, if needed, would *also* not be be either
    /// perma-unstable to use directly, or another opaque type. This is used to both give us a way
    /// to `impl Stream` and somewhere to hold the computed `size_hint` value.
    ///
    /// I refer to it as `AsyncIteratorItem` instead of `StreamItem` in anticipation of the trait
    /// potentially being renamed.
    pub struct AsyncIteratorItem<G: Generator<*mut (), Return = ()>> {
        pub gen: G,
        pub size_hint: (usize, Option<usize>),
    }

    /// This implementation is functional, but [`Stream` is currently in flux][1]:
    ///
    /// [1]: https://rust-lang.github.io/wg-async-foundations/vision/roadmap/async_iter/traits.html
    #[cfg(feature = "std_async_iter")]
    impl<G: Generator<*mut (), Yield = Poll<T>, Return = ()>, T> core::stream::Stream
        for AsyncIteratorItem<G>
    {
        type Item = T;

        fn poll_next(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let ctx: *mut () = ctx as *mut Context<'_> as *mut ();

            let gen: Pin<&mut G> = unsafe { Pin::map_unchecked_mut(self, |this| &mut this.gen) };
            match gen.resume(ctx) {
                GeneratorState::Yielded(Poll::Ready(item)) => Poll::Ready(Some(item)),
                GeneratorState::Yielded(Poll::Pending) => Poll::Pending,
                GeneratorState::Complete(()) => Poll::Ready(None),
            }
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.size_hint
        }
    }

    #[cfg(not(feature = "std_async_iter"))]
    impl<G: Generator<*mut (), Yield = Poll<T>, Return = ()>, T> Stream for AsyncIteratorItem<G> {
        type Item = T;

        fn poll_next(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            let ctx: *mut () = ctx as *mut Context<'_> as *mut ();

            let gen: Pin<&mut G> = unsafe { Pin::map_unchecked_mut(self, |this| &mut this.gen) };
            match gen.resume(ctx) {
                GeneratorState::Yielded(Poll::Ready(item)) => Poll::Ready(Some(item)),
                GeneratorState::Yielded(Poll::Pending) => Poll::Pending,
                GeneratorState::Complete(()) => Poll::Ready(None),
            }
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.size_hint
        }
    }

    #[doc(hidden)]
    #[macro_export]
    macro_rules! gen_try {
        ($e:expr) => {{
            use core::ops::{ControlFlow, FromResidual, Try};
            match Try::branch($e) {
                ControlFlow::Continue(ok) => ok,
                ControlFlow::Break(err) => {
                    yield FromResidual::from_residual(err);
                    return;
                }
            }
        }};
    }

    #[doc(hidden)]
    #[macro_export]
    macro_rules! async_gen_try {
        ($e:expr) => {{
            use core::ops::{ControlFlow, FromResidual, Try};
            match Try::branch($e) {
                ControlFlow::Continue(ok) => ok,
                ControlFlow::Break(err) => {
                    yield core::task::Poll::Ready(FromResidual::from_residual(err));
                    return;
                }
            }
        }};
    }

    #[doc(hidden)]
    #[macro_export]
    macro_rules! async_gen_yield {
        ($e:expr) => {{
            yield core::task::Poll::Ready($e)
        }};
    }

    #[doc(hidden)]
    #[macro_export]
    macro_rules! async_gen_await {
        ($e:expr, $ctx:expr) => {{
            unsafe {
                use core::pin::Pin;
                use core::task::{Context, Poll};
                let ctx = &mut *($ctx as *mut Context<'_>);
                let mut e = $e;
                let mut future = Pin::new_unchecked(&mut e);
                loop {
                    match core::future::Future::poll(Pin::as_mut(&mut future), ctx) {
                        Poll::Ready(x) => break x,
                        Poll::Pending => $ctx = yield Poll::Pending,
                    }
                }
            }
        }};
    }
}
