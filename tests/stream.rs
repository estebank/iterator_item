#![feature(generators, generator_trait, try_trait_v2)]

pub use futures::stream::StreamExt;
use std::future::Future;
use std::pin::Pin;

iterator_item::iterator_item! { *
    async fn* foo<F: Future<Output = i32>>(fut: F) yields i32 {
        yield 0; // `yield 0;` gets desugared to `yield Poll::Ready(0);`
        yield fut.await; // `fut.await` gets desugared to a `poll(fut, cxt)` call
        yield 2;
    }
}

iterator_item::iterator_item! { *
    async fn* stream<T, F: Future<Output = T>>(futures: Vec<F>) yields T {
        for future in futures {
            yield future.await;
        }
    }
}

#[tokio::test]
async fn test_foo() {
    let mut foo = Box::pin(foo(async { 1 }));
    let mut x = 0;
    while let Some(i) = foo.next().await {
        assert_eq!(x, i);
        x += 1;
    }
    assert_eq!(x, 3);
}

#[tokio::test]
async fn test_stream() {
    let mut stream = Box::pin(stream(vec![
        Box::pin(async { 1 }) as Pin<Box<dyn Future<Output = i32>>>,
        Box::pin(async { 2 }),
        Box::pin(async { 3 }),
    ]));
    let mut x = 0;
    while let Some(i) = stream.next().await {
        x += 1;
        assert_eq!(x, i);
    }
    assert_eq!(x, 3);
}

iterator_item::iterator_item! { *
    async fn* result() yields Result<i32, ()> {
        fn bar() -> Result<(), ()> {
            Err(())
        }

        for n in 0..5 {
            yield Ok(n);
        }

        bar()?;

        yield Ok(10); // will not be evaluated
    }
}

#[tokio::test]
async fn test_result() {
    let mut result = Box::pin(result());
    for n in 0..5 {
        assert_eq!(result.next().await, Some(Ok(n)));
    }

    assert_eq!(result.next().await, Some(Err(())));
    assert!(result.next().await.is_none())
}
