#![feature(generators, generator_trait, let_else)]

//! The following are the solution different phases of the "merge overlapping intervals"
//! interview question, using iterator items.

use futures::stream::{Stream, StreamExt};
use iterator_item::iterator_item;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
struct Interval {
    start: usize,
    end: usize,
}

impl Interval {
    fn new(start: usize, end: usize) -> Interval {
        Interval { start, end }
    }

    fn overlaps(&self, other: &Interval) -> bool {
        let (a, b) = match self.start < other.start {
            true => (self, other),
            false => (other, self),
        };

        a.end >= b.start
    }

    fn merge(&self, other: &Interval) -> Interval {
        Interval {
            start: std::cmp::min(self.start, other.start),
            end: std::cmp::max(self.end, other.end),
        }
    }
}

// A naïve implementation using `Vec` would look like
//
// /// Precondition: `input` must be sorted
// fn merge_overlapping_intervals(input: Vec<Interval>) -> Vec<Interval> {
//    let mut result = Vec::with_capacity(input.len());
//    if input.is_empty() {
//        return result;
//    }
//    let mut prev = input[0];
//     for i in input {
//         if prev.overlaps(&i) {
//             prev = prev.merge(&i);
//         } else {
//             result.push(prev);
//             prev = i;
//         }
//     }
//     result.push(prev);
//     result
// }

// Turning it into an O(1) space implementation becomes almost trivial:

iterator_item! {
    /// Precondition: `input` must be sorted
    fn* merge_overlapping_intervals(mut input: impl Iterator<Item = Interval>) yields Interval {
        let Some(mut prev) = input.next() else {
            return;
        };
        for i in input {
            if prev.overlaps(&i) {
                prev = prev.merge(&i);
            } else {
                yield prev;
                prev = i;
            }
        }
        yield prev;
    }
}

// Implementing this in stable today would look close to the following boilerplate:

struct MergeOverlappingIntervals<I: Iterator<Item = Interval>> {
    input: I,
    prev: Option<Interval>,
}

impl<I: Iterator<Item = Interval>> Iterator for MergeOverlappingIntervals<I> {
    type Item = Interval;

    fn next(&mut self) -> Option<Self::Item> {
        let mut prev = self.prev?;
        while let Some(i) = self.input.next() {
            if prev.overlaps(&i) {
                prev = prev.merge(&i);
                self.prev = Some(prev);
            } else {
                self.prev = Some(i);
                return Some(prev);
            }
        }
        self.prev = None;
        Some(prev)
    }
}

fn handmade_merge_overlapping_intervals(
    mut input: impl Iterator<Item = Interval>,
) -> impl Iterator<Item = Interval> {
    let prev = input.next();
    MergeOverlappingIntervals { input, prev }
}

iterator_item! {
    /// Precondition: each `Iterator` in `inputs` must be sorted
    fn* sorted_merge_k_intervals(mut inputs: Vec<impl Iterator<Item = Interval>>) yields Interval {
        if inputs.len() == 0 {
            return;
        }
        let mut last: Vec<Option<Interval>> = inputs.iter_mut().map(|input| input.next()).collect();
        let mut opt_smallest = last
            .iter()
            .enumerate()
            .find(|(_, l)| l.is_some())
            .map(|(i, l)| (i, l.unwrap()));
        while last.iter().any(|l| l.is_some()) {
            for (i, l) in last.iter().enumerate() {
                match (opt_smallest, l) {
                    (Some((_, smallest)), Some(l)) if l <= &smallest => {
                        opt_smallest = Some((i, *l));
                    }
                    (None, Some(l)) => {
                        opt_smallest = Some((i, *l));
                    }
                    _ => {}
                }
            }
            if let Some((pos, smallest)) = opt_smallest {
                yield smallest;
                opt_smallest = None;
                last[pos] = inputs[pos].next();
            }
        }
    }
}

// This could/should have been written as the following instead:
//
// fn merge_k_overlapping_intervals(
//    inputs: Vec<impl Iterator<Item = Interval>>
// ) -> impl Iterator<Item = Interval> {
//     merge_overlapping_intervals(sorted_merge_k_intervals(inputs))
// }
//
// This could be easily detected and implemented as an auto-applicable `rustc` suggestion.
iterator_item! {
    fn* merge_k_overlapping_intervals(inputs: Vec<impl Iterator<Item = Interval>>) yields Interval {
        for i in merge_overlapping_intervals(sorted_merge_k_intervals(inputs)) {
            yield i;
        }
    }
}

#[test]
fn test_merge_overlapping_intervals() {
    let intervals = vec![
        Interval::new(1, 4),
        Interval::new(3, 6),
        Interval::new(8, 10),
        Interval::new(9, 11),
    ];
    let handmade_result: Vec<_> =
        handmade_merge_overlapping_intervals(intervals.iter().cloned()).collect();
    let result: Vec<_> = merge_overlapping_intervals(intervals.into_iter()).collect();
    assert_eq!(
        &[Interval::new(1, 6), Interval::new(8, 11)][..],
        &result[..]
    );
    assert_eq!(&handmade_result[..], &result[..]);

    let intervals = vec![Interval::new(1, 4)];
    let handmade_result: Vec<_> =
        handmade_merge_overlapping_intervals(intervals.iter().cloned()).collect();
    let result: Vec<_> = merge_overlapping_intervals(intervals.into_iter()).collect();
    assert_eq!(&[Interval::new(1, 4)][..], &result[..]);
    assert_eq!(&handmade_result[..], &result[..]);

    let intervals = vec![
        Interval::new(1, 12),
        Interval::new(3, 6),
        Interval::new(8, 10),
        Interval::new(9, 11),
    ];
    let handmade_result: Vec<_> =
        handmade_merge_overlapping_intervals(intervals.iter().cloned()).collect();
    let result: Vec<_> = merge_overlapping_intervals(intervals.into_iter()).collect();
    assert_eq!(&[Interval::new(1, 12)][..], &result[..]);
    assert_eq!(&handmade_result[..], &result[..]);

    let intervals = vec![
        Interval::new(1, 2),
        Interval::new(3, 6),
        Interval::new(8, 9),
        Interval::new(10, 11),
    ];
    let handmade_result: Vec<_> =
        handmade_merge_overlapping_intervals(intervals.iter().cloned()).collect();
    let result: Vec<_> = merge_overlapping_intervals(intervals.into_iter()).collect();
    assert_eq!(
        &[
            Interval::new(1, 2),
            Interval::new(3, 6),
            Interval::new(8, 9),
            Interval::new(10, 11)
        ][..],
        &result[..]
    );
    assert_eq!(&handmade_result[..], &result[..]);
}

#[test]
fn test_sorted_merge_k_intervals() {
    let intervals1 = vec![Interval::new(1, 2), Interval::new(8, 10)];
    let intervals2 = vec![
        Interval::new(1, 4),
        Interval::new(2, 4),
        Interval::new(9, 11),
    ];
    let intervals3 = vec![Interval::new(5, 6), Interval::new(12, 14)];
    let k_intervals = vec![
        intervals1.into_iter(),
        intervals2.into_iter(),
        intervals3.into_iter(),
    ];
    let result: Vec<_> = sorted_merge_k_intervals(k_intervals).collect();
    let expected = vec![
        Interval::new(1, 2),
        Interval::new(1, 4),
        Interval::new(2, 4),
        Interval::new(5, 6),
        Interval::new(8, 10),
        Interval::new(9, 11),
        Interval::new(12, 14),
    ];
    assert_eq!(&expected[..], &result[..]);
}

#[test]
fn test_merge_k_overlapping_intervals() {
    let intervals1 = vec![Interval::new(1, 2), Interval::new(8, 10)];
    let intervals2 = vec![
        Interval::new(1, 4),
        Interval::new(2, 4),
        Interval::new(9, 11),
    ];
    let intervals3 = vec![Interval::new(5, 6), Interval::new(11, 14)];
    let k_intervals = vec![
        intervals1.into_iter(),
        intervals2.into_iter(),
        intervals3.into_iter(),
    ];
    let result: Vec<_> = merge_k_overlapping_intervals(k_intervals).collect();
    let expected = vec![
        Interval::new(1, 4),
        Interval::new(5, 6),
        Interval::new(8, 14),
    ];
    assert_eq!(&expected[..], &result[..]);
}

// Implementing the `async` version of this requires barely changing the signature of the
// iterators and some translation to be able to consume the `Stream`s.

iterator_item! {
    /// Precondition: `input` must be sorted
    async fn* async_merge_overlapping_intervals(input: impl Stream<Item = Interval>) yields Interval {
        let mut input = Box::pin(input);
        let mut prev = if let Some(prev) = input.next().await {
            // FIXME: why din't `let else` work here?
            prev
        } else {
            return;
        };
        // We had to change the `for i in input` with an `.await` appropriate `while let` loop.
        while let Some(i) = input.next().await {
            if prev.overlaps(&i) {
                prev = prev.merge(&i);
            } else {
                yield prev;
                prev = i;
            }
        }
        yield prev;
    }
}

iterator_item! {
    /// Precondition: each `Iterator` in `inputs` must be sorted
    async fn* async_sorted_merge_k_intervals(inputs: Vec<impl Stream<Item = Interval>>) yields Interval {
        if inputs.len() == 0 {
            return;
        }
        // We need to `Pin` all the incoming `Stream`s. Should this be part of the desugaring?
        let mut inputs: Vec<_> = inputs.into_iter().map(|i| Box::pin(i)).collect();
        // We needed to change this because we can't use `.await` inside of a closure passed into
        // `.map`. I expect this is something that will trip people up, we should handle that in
        // `rustc`.
        let mut last: Vec<Option<Interval>> = Vec::with_capacity(inputs.len());
        for input in inputs.iter_mut() {
            last.push(input.next().await);
        };
        let mut opt_smallest = last
            .iter()
            .enumerate()
            .find(|(_, l)| l.is_some())
            .map(|(i, l)| (i, l.unwrap()));
        while last.iter().any(|l| l.is_some()) {
            for (i, l) in last.iter().enumerate() {
                match (opt_smallest, l) {
                    (Some((_, smallest)), Some(l)) if l <= &smallest => {
                        opt_smallest = Some((i, *l));
                    }
                    (None, Some(l)) => {
                        opt_smallest = Some((i, *l));
                    }
                    _ => {}
                }
            }
            if let Some((pos, smallest)) = opt_smallest {
                yield smallest;
                opt_smallest = None;
                last[pos] = inputs[pos].next().await;
            }
        }
    }
}

// We don't need as it exists but I think it's neat that we can write it this easily.
iterator_item! {
    async fn* into_stream(input: impl Iterator<Item = Interval>) yields Interval {
        for i in input {
            yield i;
        }
    }
}

#[tokio::test]
async fn test_async_merge_k_overlapping_intervals() {
    let intervals1 = vec![Interval::new(1, 2), Interval::new(8, 10)];
    let intervals2 = vec![
        Interval::new(1, 4),
        Interval::new(2, 4),
        Interval::new(9, 11),
    ];
    let intervals3 = vec![Interval::new(5, 6), Interval::new(11, 14)];
    let k_intervals = vec![
        into_stream(intervals1.into_iter()),
        into_stream(intervals2.into_iter()),
        into_stream(intervals3.into_iter()),
    ];
    let result = async_merge_overlapping_intervals(async_sorted_merge_k_intervals(k_intervals));
    let mut result = Box::pin(result);
    let expected = vec![
        Interval::new(1, 4),
        Interval::new(5, 6),
        Interval::new(8, 14),
    ];
    let mut x = 0;
    while let Some(i) = result.next().await {
        assert_eq!(expected[x], i);
        x += 1;
    }
}
