use core::fmt;
use std::collections::HashSet;

use crate::error::Result;
use crate::simplerange::SimpleRange;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Range {
    pub ranges: Vec<SimpleRange>,
    latest: Option<u32>, // The most recent value returned by next()
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let joined = self
            .ranges
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");

        write!(f, "[{joined}]")?;
        Ok(())
    }
}

impl Range {
    pub const fn new() -> Self {
        Self {
            ranges: Vec::new(),
            latest: None,
        }
    }

    pub const fn latest(&self) -> Option<u32> {
        self.latest
    }

    pub fn add_range(&mut self, range: &SimpleRange) -> Result<()> {
        // Shrink this new range until it does not overlap with any existing range
        let mut rangeset = HashSet::new();
        rangeset.insert((range.start, range.end));
        while !rangeset.is_empty() {
            while let Some(&(mut lo, mut hi)) = rangeset.iter().next() {
                rangeset.remove(&(lo, hi));

                // Whittle down the (lo,hi) range until we're left with either:
                //  1. a range that doesn't overlap with any existing range, or
                //  2. nothing
                let mut keep = true;
                for r in &self.ranges {
                    let (a, b) = (r.start, r.end);

                    if lo >= a && hi <= b {
                        //    l--h         l---h
                        // a--------b      a---b
                        // Redundant range
                        keep = false;
                        break;
                    }

                    if lo < a && hi > b {
                        // l--------h
                        //    a--b
                        // Both sides overlap. Save the right side for later, keep checking the left side.
                        rangeset.insert((b + 1, hi));
                        hi = a - 1;
                    } else if hi >= a && hi <= b {
                        // l-----h
                        //    a------b
                        // Left overlap
                        hi = a - 1;
                    } else if lo >= a && lo <= b {
                        //    l-----h     l------h
                        // a------b       a---b
                        // Right overlap
                        lo = b + 1;
                    }

                    // l--h                   l--h
                    //       a---b     a---b
                    // No overlap: continue checking
                }

                if keep {
                    if let Ok(range) = SimpleRange::new(lo, hi) {
                        self.ranges.push(range);
                    }
                }
            }
        }

        self.condense_ranges()?;

        Ok(())
    }

    /// Combine contiguous sub-ranges into larger ranges until the minimum remain.
    /// Assumes ranges are non-overlapping.
    fn condense_ranges(&mut self) -> Result<()> {
        let mut new_ranges: Vec<SimpleRange> = Vec::new();
        let mut lo = 0;
        let mut hi = None;
        self.ranges.sort_unstable();
        for r in &self.ranges {
            match hi {
                None => {
                    lo = r.start;
                }
                Some(h) => {
                    if h != r.start - 1 {
                        // non-contiguous case: add the previous range to our vec and start a new one
                        let range = SimpleRange::new(lo, h)?;
                        new_ranges.push(range);
                        lo = r.start;
                    }
                }
            }
            hi = Some(r.end);
        }

        // Add the last range if we ended on a non-contiguous case
        if let Some(h) = hi {
            let range = SimpleRange::new(lo, h)?;
            new_ranges.push(range);
        }

        self.ranges = new_ranges;

        Ok(())
    }

    pub fn len(&self) -> usize {
        self.ranges.iter().map(SimpleRange::len).sum()
    }

    pub fn reset(&mut self) {
        for r in &mut self.ranges {
            r.reset();
        }
    }

    pub fn merge(&mut self, other: &Range) -> Result<()> {
        for range in &other.ranges {
            self.add_range(range)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn iter(&self) -> Self {
        Self {
            ranges: self.ranges.clone(),
            latest: None,
        }
    }
}

impl Iterator for Range {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        for r in &mut self.ranges {
            if let Some(rnext) = r.next() {
                self.latest = Some(rnext);
                return Some(rnext);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_range_1() -> Result<()> {
        let mut range = Range::new();
        assert_eq!(range.len(), 0);

        let _ = range.add_range(&SimpleRange::new(1, 5)?);
        let _ = range.add_range(&SimpleRange::new(2, 4)?);
        let _ = range.add_range(&SimpleRange::new(3, 7)?);
        assert_eq!(range.len(), 7);

        for i in 1..=7 {
            assert_eq!(range.next(), Some(i));
        }
        assert_eq!(range.next(), None);
        assert_eq!(range.len(), 0);

        Ok(())
    }

    #[test]
    fn test_range_iter() -> Result<()> {
        let mut range = Range::new();
        assert_eq!(range.len(), 0);

        let _ = range.add_range(&SimpleRange::new(1, 5)?);
        let _ = range.add_range(&SimpleRange::new(2, 4)?);
        let _ = range.add_range(&SimpleRange::new(3, 7)?);
        assert_eq!(range.len(), 7);

        let expected = 1..=7;
        let expected: Vec<u32> = expected.collect();
        for (i, elem) in range.iter().enumerate() {
            assert_eq!(expected[i], elem);
        }
        for (i, elem) in range.iter().enumerate() {
            assert_eq!(expected[i], elem);
        }

        assert_eq!(range.len(), 7);

        Ok(())
    }

    #[test]
    fn test_range_len_limits() -> Result<()> {
        let mut range = Range::new();

        range.add_range(&SimpleRange::new(0, 0)?).unwrap();
        assert_eq!(range.len(), 1);
        range.add_range(&SimpleRange::new(1, 1)?).unwrap();
        assert_eq!(range.len(), 2);
        range
            .add_range(&SimpleRange::new(2, u32::MAX - 1)?)
            .unwrap();

        assert_eq!(range.len(), u32::MAX as usize);

        let mut range = Range::new();

        range
            .add_range(&SimpleRange::new(0, u32::MAX - 1)?)
            .unwrap();
        range
            .add_range(&SimpleRange::new(0, u32::MAX - 1)?)
            .unwrap();

        assert_eq!(range.len(), u32::MAX as usize);

        Ok(())
    }
}
