#![feature(generators, generator_trait, let_else, try_trait_v2)]
use iterator_item::iterator_item;

iterator_item! { #
    /// Basic smoke test
    fn foo() -> impl Iterator<Item=i32> {
        #[size_hint((10, Some(10)))]
        gen {
            for n in 0..10 {
                yield n;
            }
        }
    }
}

#[test]
fn test_foo() {
    let mut foo = foo();
    assert_eq!(foo.size_hint(), (10, Some(10)));
    for n in 0..10 {
        assert_eq!(foo.next(), Some(n));
    }
    assert!(foo.next().is_none());
}

iterator_item! { #
    /// Show off the way you can write a custom `size_hint` impl.
    fn bar() -> impl Iterator<Item = i32> {
        let inner = vec![1, 2, 3].into_iter();

        #[size_hint({
            let (x, y) = inner.size_hint();
            (x + 2, y.map(|y| y + 2))
        })]
        gen {
            yield 42;
            for n in inner {
                yield n;
            }
            yield 42;
        }
    }
}

#[test]
fn test_bar() {
    let bar = bar();
    assert_eq!(bar.size_hint(), (5, Some(5)));
    assert_eq!(&[42, 1, 2, 3, 42][..], &bar.collect::<Vec<_>>()[..]);
}

iterator_item! { #
    fn early_return() -> impl Iterator<Item=i32> {
        gen {
            let mut x = Some(3);
            let y = x.take()?;
            yield y;
            let y = x.take()?;
            yield y;
        }
    }
}

#[test]
fn test_early_return() {
    let mut result = early_return();

    assert_eq!(result.next(), Some(3));
    assert!(result.next().is_none())
}

struct Foo(Option<i32>);

impl Foo {
    iterator_item! { #
        /// You can also have "associated iterator items"
        fn method(&mut self) -> impl Iterator<Item=i32> + '_ {
            gen {
                while let Some(n) = self.0.take() {
                    yield n;
                }
            }
        }
    }
}

#[test]
fn test_foo_method() {
    let mut foo = Foo(Some(0));
    let mut iter = foo.method();
    assert_eq!(iter.next(), Some(0));
    assert!(iter.next().is_none());
}
