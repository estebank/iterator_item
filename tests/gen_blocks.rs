#![feature(generators, generator_trait, let_else, try_trait_v2)]
use iterator_item::iterator_item;
use std::cell::Cell;

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
struct Foo(Cell<Option<i32>>);

impl Foo {
    iterator_item! { #
        fn method(&self) -> impl Iterator<Item=i32> + '_ {
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
    let foo = Foo(Cell::new(Some(0)));
    let mut iter = foo.method();
    assert_eq!(iter.next(), Some(0));
    foo.0.set(Some(1));
    assert_eq!(iter.next(), Some(1));
    assert!(iter.next().is_none());
}

iterator_item! { #
    fn replace_pairs(buffer: &mut Vec<i32>) {
        let count = buffer.drain(..).sum();
        buffer.extend(gen {
            yield 0;
            for i in 1..=count {
                yield -i;
                yield i;
            }
        });
    }
}

#[test]
fn test_replace_pairs() {
    let mut pairs = vec![1, 1, 1];
    replace_pairs(&mut pairs);
    assert_eq!(&pairs, &[0, -1, 1, -2, 2, -3, 3]);
}
