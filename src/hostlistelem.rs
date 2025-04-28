use core::fmt;
use std::iter::FusedIterator;
use std::num::ParseIntError;

use derive_more::Display;

use crate::Rule;
use crate::error::{Error, Result};
use crate::range::Range;
use crate::simplerange::SimpleRange;

/// A component of a hostlist expression, `static_elem` or `range` from the pest grammar
#[derive(Debug, Display, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Component {
    #[display("{_0}")]
    Static(String),

    #[display("{_0}")]
    Range(Range),
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum FingerprintComponent {
    Static(String),
    RangePlaceholder,
}

// A type that uniquely identifies the structure of a hostlist element. Used to combine hostlist
// elements that are identical other than their range values.
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Fingerprint {
    pub components: Vec<FingerprintComponent>,
}

impl Fingerprint {
    pub fn count_ranges(&self) -> usize {
        self.components
            .iter()
            .filter(|e| matches!(e, FingerprintComponent::RangePlaceholder))
            .count()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct HostlistElem {
    pub components: Vec<Component>,
    latest: Option<String>,
    len: usize,
}

impl fmt::Display for HostlistElem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let joined = self
            .components
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<String>();

        f.write_str(&joined)
    }
}

impl HostlistElem {
    pub fn new(hostlist: pest::iterators::Pair<Rule>) -> Result<Self> {
        let mut components: Vec<Component> = Vec::new();
        for hostlist_elem in hostlist.into_inner() {
            match hostlist_elem.as_rule() {
                Rule::static_elem => {
                    let s = hostlist_elem.as_span().as_str().to_string();
                    components.push(Component::Static(s));
                }
                Rule::range => {
                    let mut range = Range::new();
                    for range_inner in hostlist_elem.into_inner() {
                        match range_inner.as_rule() {
                            r @ Rule::simple_range => {
                                let mut range_parts = range_inner.into_inner();
                                let start = get_value(
                                    &range_parts.next().ok_or(Error::UnexpectedParserState(r))?,
                                )?;
                                let end = get_value(
                                    &range_parts.next().ok_or(Error::UnexpectedParserState(r))?,
                                )?;
                                range.add_range(&SimpleRange::new(start, end)?)?;
                            }
                            Rule::number => {
                                let val = get_value(&range_inner)?;
                                range.add_range(&SimpleRange::new(val, val)?)?;
                            }
                            rule => return Err(Error::UnexpectedParserState(rule)),
                        }
                    }

                    components.push(Component::Range(range));
                }
                rule => return Err(Error::UnexpectedParserState(rule)),
            }
        }

        let mut elem = Self {
            components,
            latest: None,
            len: 0,
        };
        elem.update_len()?;

        Ok(elem)
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    // Recalculate the length of this container as the Cartesian product of all `Range`s
    // contained within.
    pub fn update_len(&mut self) -> Result<()> {
        if self.latest.is_some() {
            return Err(Error::Internal(
                "update_len called after iteration started".to_string(),
            ));
        }

        let mut len: usize = 1;
        for component in &self.components {
            if let Component::Range(r) = component {
                len = len.checked_mul(r.len()).ok_or(Error::HostlistTooLarge)?;
            }
        }
        self.len = len;

        Ok(())
    }

    fn construct_next(&mut self) -> Option<String> {
        if self.len == 0 {
            return None;
        }

        // Move the last non-empty iterator forward. If all iterators are empty, we're done.
        // Build the hostname parts (in reverse) as we go.
        let mut host_parts = Vec::new();
        let mut found_next = false;
        for elem in self.components.iter_mut().rev() {
            let hostname_component = match elem {
                Component::Static(s) => s.clone(),
                Component::Range(r) => {
                    if found_next {
                        r.latest()
                            .or_else(|| r.next())
                            .unwrap_or_else(|| {
                                panic!("internal error: no latest or next element in range: {r:?} with len {}", r.len())
                            })
                            .to_string()
                    } else if let Some(num) = r.next() {
                        found_next = true;
                        num.to_string()
                    } else {
                        r.reset();
                        r.next()
                            .unwrap_or_else(|| {
                                panic!(
                                    "internal error: no next element in range: {r:?} with len {}",
                                    r.len()
                                )
                            })
                            .to_string()
                    }
                }
            };

            host_parts.push(hostname_component);
        }

        let host: String = host_parts.into_iter().rev().collect();
        self.len -= 1;
        Some(host)
    }

    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint {
            components: self
                .components
                .iter()
                .map(|c| match c {
                    Component::Static(s) => FingerprintComponent::Static(s.clone()),
                    Component::Range(_) => FingerprintComponent::RangePlaceholder,
                })
                .collect(),
        }
    }
}

impl Iterator for HostlistElem {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.construct_next();
        self.latest.clone_from(&next);
        next
    }
}

// This trait guarantees that once the iterator returns None, it will always return None.
// No additional methods needed, it's a marker trait.
impl FusedIterator for HostlistElem {}

fn get_value(number: &pest::iterators::Pair<Rule>) -> std::result::Result<u32, ParseIntError> {
    number.as_str().parse::<u32>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HostlistParser, Rule};
    use pest::Parser;

    #[test]
    fn test_hostlistelem_1() -> Result<()> {
        let mut pairs = HostlistParser::parse(Rule::hostlist, "n[1-5]")?;
        let mut elems = HostlistElem::new(pairs.next().unwrap())?;

        assert_eq!(elems.len(), 5);
        for i in 1..=5_u32 {
            assert_eq!(elems.next(), Some(format!("n{i}")));
        }
        assert_eq!(elems.next(), None);
        assert_eq!(elems.len(), 0);

        Ok(())
    }

    #[test]
    fn test_hostlistelem_2() -> Result<()> {
        let mut pairs = HostlistParser::parse(Rule::hostlist, "n[1-5]m[1-3]")?;
        let mut elems = HostlistElem::new(pairs.next().unwrap())?;
        assert_eq!(pairs.next().unwrap().as_rule(), Rule::EOI);

        assert_eq!(elems.len(), 15);
        for n in 1..=5_u32 {
            for m in 1..=3_u32 {
                let elem = elems.next();
                assert_eq!(elem, Some(format!("n{n}m{m}")));
            }
        }
        assert_eq!(elems.next(), None);
        assert_eq!(elems.len(), 0);

        Ok(())
    }

    #[test]
    fn test_hostlistelem_3() -> Result<()> {
        let mut pairs = HostlistParser::parse(Rule::hostlist, "n[1-3],foo[5-7]")?;
        let mut hostlist_elem = HostlistElem::new(pairs.next().unwrap())?;

        assert_eq!(hostlist_elem.len(), 3);
        for n in 1..=3_u32 {
            let elem = hostlist_elem.next();
            assert_eq!(elem, Some(format!("n{n}")));
        }
        assert_eq!(hostlist_elem.len(), 0);

        let mut hostlist_elem = HostlistElem::new(pairs.next().unwrap())?;
        assert_eq!(hostlist_elem.len(), 3);
        for n in 5..=7_u32 {
            let elem = hostlist_elem.next();
            assert_eq!(elem, Some(format!("foo{n}")));
        }
        assert_eq!(hostlist_elem.len(), 0);

        assert_eq!(hostlist_elem.next(), None);
        assert_eq!(hostlist_elem.len(), 0);

        assert_eq!(pairs.next().unwrap().as_rule(), Rule::EOI);

        Ok(())
    }

    #[test]
    fn test_hostlistelem_len_overflow() -> Result<()> {
        let inputs = ["n[1-1000][1-1000][1-1000][1-1000][1-1000][1-1000][1-1000]"];

        for input in inputs {
            let mut pairs = HostlistParser::parse(Rule::hostlist, input)?;
            let result = HostlistElem::new(pairs.next().unwrap());
            assert!(matches!(result, Err(Error::HostlistTooLarge)));
        }

        Ok(())
    }
}
