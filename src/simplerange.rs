use std::iter::FusedIterator;

use crate::error::{Error, Result};

/// A simple a-b range, where a <= b
#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct SimpleRange {
    pub start: u32,
    pub end: u32,
    current: Option<u32>,
}

impl SimpleRange {
    pub const fn new(start: u32, end: u32) -> Result<Self> {
        if start > end {
            return Err(Error::InvalidRangeReversed { start, end });
        }

        // We want the range to be inclusive, and in the iterator we keep track of 'current' by
        // letting it go one higher than 'end', so we don't support 'end' being the max value for
        // the type.
        if end == u32::MAX {
            return Err(Error::TooLarge(end));
        }

        Ok(Self {
            start,
            end,
            current: Some(start),
        })
    }

    /// Resets the range iterator back to the start
    pub fn reset(&mut self) {
        self.current = Some(self.start);
    }

    /// Number of values represented by the range
    pub const fn len(&self) -> usize {
        if let Some(current) = self.current {
            (self.end - current + 1) as usize
        } else {
            0
        }
    }

    // Returns an iterator over our range of values
    pub const fn iter(&self) -> SimpleRangeIter {
        SimpleRangeIter {
            current: self.start,
            end: self.end,
        }
    }
}

// This enables `for e in range { ... }`
impl Iterator for SimpleRange {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current) = self.current {
            self.current = if current == self.end {
                None
            } else {
                Some(current + 1)
            };
            Some(current)
        } else {
            None
        }
    }
}

// This enables:
//   `for e in &range { ... }`
//   `for e in range.into_iter() { ... }`
impl IntoIterator for &SimpleRange {
    type Item = u32;
    type IntoIter = SimpleRangeIter;

    fn into_iter(self) -> Self::IntoIter {
        SimpleRangeIter {
            current: self.start,
            end: self.end,
        }
    }
}

#[derive(Debug)]
pub struct SimpleRangeIter {
    current: u32,
    end: u32,
}

impl Iterator for SimpleRangeIter {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current <= self.end {
            let result = self.current;
            self.current += 1; // Guaranteed to not overflow since we don't allow max value
            Some(result)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = if self.current <= self.end {
            (self.end - self.current + 1) as usize
        } else {
            0
        };
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for SimpleRangeIter {
    fn len(&self) -> usize {
        if self.current <= self.end {
            (self.end - self.current + 1) as usize
        } else {
            0
        }
    }
}

// This trait guarantees that once the iterator returns None, it will always return None.
// No additional methods needed, it's a marker trait.
impl FusedIterator for SimpleRangeIter {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simplerange_len() {
        let range = SimpleRange::new(1, 1_000_000_000).unwrap();
        assert_eq!(range.len(), 1_000_000_000);
    }

    #[test]
    fn test_simplerange_len_limits() -> Result<()> {
        let toolarge = SimpleRange::new(0, u32::MAX);
        assert!(matches!(toolarge, Err(Error::TooLarge(_))));

        let range = SimpleRange::new(0, u32::MAX - 1)?;
        assert_eq!(range.len(), u32::MAX as usize);

        Ok(())
    }

    #[test]
    fn test_simplerange_iterator() {
        let mut expected = vec![1, 2, 3, 4, 5];

        let simplerange = SimpleRange::new(1, 5).unwrap();
        for elem in simplerange {
            let e = expected.remove(0);
            assert_eq!(e, elem);
        }

        let mut simplerange = SimpleRange::new(1, 5).unwrap();
        assert_eq!(simplerange.next(), Some(1));
        assert_eq!(simplerange.next(), Some(2));
        assert_eq!(simplerange.next(), Some(3));
        assert_eq!(simplerange.next(), Some(4));
        assert_eq!(simplerange.next(), Some(5));
        assert_eq!(simplerange.next(), None);
        assert_eq!(simplerange.len(), 0);
        simplerange.reset();
        assert_eq!(simplerange.len(), 5);
        assert_eq!(simplerange.next(), Some(1));
        assert_eq!(simplerange.len(), 4);
    }

    #[test]
    fn test_simplerange_iter() {
        let expected_orig = vec![1, 2, 3, 4, 5];
        let simplerange = SimpleRange::new(1, 5).unwrap();

        let mut expected = expected_orig.clone();
        for elem in &simplerange {
            let e = expected.remove(0);
            assert_eq!(e, elem);
        }
        assert!(expected.is_empty());

        let mut expected = expected_orig.clone();
        for elem in &simplerange {
            let e = expected.remove(0);
            assert_eq!(e, elem);
        }
        assert!(expected.is_empty());

        let expected = expected_orig;
        let elems: Vec<u32> = simplerange.iter().collect();
        assert_eq!(expected, elems);
    }

    #[test]
    fn test_simplerange_intoiter() {
        let expected_orig = vec![1, 2, 3, 4, 5];
        let simplerange = SimpleRange::new(1, 5).unwrap();

        let mut expected = expected_orig.clone();
        for elem in &simplerange {
            let e = expected.remove(0);
            assert_eq!(e, elem);
        }
        assert!(expected.is_empty());

        let mut expected = expected_orig.clone();
        for elem in &simplerange {
            let e = expected.remove(0);
            assert_eq!(e, elem);
        }
        assert!(expected.is_empty());

        let mut expected = expected_orig;
        for elem in simplerange {
            let e = expected.remove(0);
            assert_eq!(e, elem);
        }
        assert!(expected.is_empty());
    }

    #[test]
    fn test_simplerange_exactsizeiterator() {
        fn inner(_: impl ExactSizeIterator) {
            // Do nothing, just need to ensure this test compiles successfully to
            // indicate 'iter' implements ExactSizeIterator.
        }

        let simplerange = SimpleRange::new(1, 1).unwrap();

        inner(simplerange.iter());
    }
}
