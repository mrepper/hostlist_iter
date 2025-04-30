use core::fmt;
use std::collections::HashMap;
use std::iter::FusedIterator;
use std::str::FromStr;

use pest::Parser;
use pest_derive::Parser;

use crate::error::{Error, Result};
use crate::hostlistelem::{Component, HostlistElem};

#[derive(Parser)]
#[grammar = "src/hostlist.pest"]
pub struct HostlistParser;

/// An iterable structure representing the hosts in a hostlist expression
/// ```
/// use hostlist_iter::Hostlist;
///
/// fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
///   // Example 1
///   let mut hosts_iter = Hostlist::new("node[1-3,5]")?;
///   assert_eq!(hosts_iter.next(), Some("node1".into()));
///   assert_eq!(hosts_iter.next(), Some("node2".into()));
///   assert_eq!(hosts_iter.next(), Some("node3".into()));
///   assert_eq!(hosts_iter.next(), Some("node5".into()));
///
///   // Example 2
///   let hosts: Vec<String> = Hostlist::new("node[1-3,5]")?.collect();
///   assert_eq!(hosts, vec!["node1", "node2", "node3", "node5"]);
///
///   Ok(())
/// }
/// ```
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Hostlist {
    hostlist_elems: Vec<HostlistElem>,
}

impl fmt::Display for Hostlist {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let joined = self
            .hostlist_elems
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");

        f.write_str(&joined)
    }
}

impl Hostlist {
    /// Constructs a new `Hostlist` from a hostlist expression
    ///
    /// # Errors
    /// Will return `Err` if there are issues parsing the provided expression.
    pub fn new(expr: &str) -> Result<Self> {
        let mut hostlist_elems_by_fingerprint = HashMap::new();
        let pairs = HostlistParser::parse(Rule::hostlist, expr)?;

        for hostlist in pairs {
            match hostlist.as_rule() {
                Rule::hostlist_elem => {
                    let elem = HostlistElem::new(hostlist)?;
                    let fingerprint = elem.fingerprint();
                    hostlist_elems_by_fingerprint
                        .entry(fingerprint)
                        .or_insert_with(Vec::new)
                        .push(elem);
                }
                Rule::EOI => break,
                rule => return Err(Error::UnexpectedParserState(rule)),
            }
        }

        // Combine any hostlists that:
        //   a) have the same fingerprint
        //   b) have only 1 range component (for simplicity)
        let mut hostlist_elems: Vec<HostlistElem> = Vec::new();
        for (fingerprint, elems) in hostlist_elems_by_fingerprint {
            if fingerprint.count_ranges() != 1 || elems.len() == 1 {
                // We don't (currently) support merging hostlist elements with multiple ranges.
                // So if there are, or if there's only one entry, just add them/it and move on.
                hostlist_elems.extend(elems);
                continue;
            }

            let mut elems_iter = elems.into_iter();
            let mut combined_elem = elems_iter.next().ok_or(Error::Internal(
                "no next value when combining ranges".to_string(),
            ))?;
            let position = combined_elem
                .components
                .iter()
                .position(|item| matches!(item, Component::Range(_)))
                .ok_or(Error::Internal("no range component found".to_string()))?;

            if let Component::Range(range) = &mut combined_elem.components[position] {
                for elem in elems_iter {
                    if let Component::Range(range_to_add) = &elem.components[position] {
                        range.merge(range_to_add)?;
                    }
                }

                // Since we may have modified the underlying SimpleRange contents, we need to
                // update the internal length of the HostlistElem.
                combined_elem.update_len()?;
            }

            hostlist_elems.push(combined_elem);
        }

        // Check for overflow
        let mut len: usize = 0;
        for elem in &hostlist_elems {
            len = len.checked_add(elem.len()).ok_or(Error::HostlistTooLarge)?;
        }

        hostlist_elems.sort_unstable();

        Ok(Self { hostlist_elems })
    }

    /// Returns whether the hostlist is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hostlist_elems.iter().all(|e| e.len() == 0)
    }

    /// Returns the number of hosts in the hostlist
    pub fn len(&self) -> usize {
        self.hostlist_elems.iter().map(HostlistElem::len).sum()
    }

    #[must_use]
    pub fn iter(&self) -> Self {
        Self {
            hostlist_elems: self.hostlist_elems.clone(),
        }
    }
}

impl FromStr for Hostlist {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::new(s)
    }
}

impl Iterator for Hostlist {
    type Item = String;

    /// Returns the next host in the hostlist
    fn next(&mut self) -> Option<Self::Item> {
        self.hostlist_elems.iter_mut().find_map(Iterator::next)
    }
}

// This enables:
//   `for e in &range { ... }`
//   `for e in range.into_iter() { ... }`
impl IntoIterator for &Hostlist {
    type Item = String;
    type IntoIter = Hostlist;

    fn into_iter(self) -> Self::IntoIter {
        Hostlist {
            hostlist_elems: self.hostlist_elems.clone(),
        }
    }
}

// This trait guarantees that once the iterator returns None, it will always return None.
// No additional methods needed, it's a marker trait.
impl FusedIterator for Hostlist {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hostlist_len1() {
        let hostlist = Hostlist::new("node[1-1000000000]").unwrap();
        assert_eq!(hostlist.len(), 1_000_000_000);
    }

    #[test]
    fn test_hostlist_len2() {
        let hostlist = Hostlist::new("blah2,node[1-3,2-5],n[11-20],blah").unwrap();
        assert_eq!(hostlist.len(), 17);
    }

    #[test]
    fn test_hostlist_len_overflow() {
        let inputs =
            ["n[1-1000000][1-1000000][1-1000000][1-10],o[1-1000000][1-1000000][1-1000000][1-10]"];

        for input in inputs {
            let result = Hostlist::new(input);
            assert!(matches!(result, Err(Error::HostlistTooLarge)));
        }
    }

    #[test]
    fn test_hostlist_combine_like_prefixes() {
        let mut hostlist = Hostlist::new("node[1-3,2-5],node[2-7]").unwrap();
        let expected = vec![
            "node1", "node2", "node3", "node4", "node5", "node6", "node7",
        ];

        assert_eq!(hostlist.len(), 7);
        for e in expected {
            assert_eq!(hostlist.next(), Some(e.into()));
        }
        assert_eq!(hostlist.len(), 0);
    }

    #[test]
    fn test_hostlist_fromstr() {
        let hostlist: Hostlist = "node[1-3]".parse().unwrap();
        let expected = vec!["node1", "node2", "node3"];

        let result: Vec<String> = hostlist.into_iter().collect();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_hostlist_iter1() {
        let mut hostlist: Hostlist = "n[1-5]".parse().unwrap();
        let expected = ["n1", "n2", "n3", "n4", "n5"];

        for e in expected {
            assert_eq!(hostlist.next(), Some(e.into()));
        }
        assert_eq!(hostlist.next(), None);
        assert_eq!(hostlist.len(), 0);
    }

    #[test]
    fn test_hostlist_iter2() {
        let mut hostlist: Hostlist = "n[1-3]s".parse().unwrap();
        let expected = ["n1s", "n2s", "n3s"];

        for e in expected {
            assert_eq!(hostlist.next(), Some(e.into()));
        }
        assert_eq!(hostlist.next(), None);
        assert_eq!(hostlist.len(), 0);
    }

    #[test]
    fn test_hostlist_iter3() {
        let mut hostlist: Hostlist = "n[1-3]s[1-2]".parse().unwrap();
        let expected = ["n1s1", "n1s2", "n2s1", "n2s2", "n3s1", "n3s2"];

        for e in expected {
            assert_eq!(hostlist.next(), Some(e.into()));
        }
        assert_eq!(hostlist.next(), None);
        assert_eq!(hostlist.len(), 0);
    }

    #[test]
    fn test_hostlist_iter4() -> Result<()> {
        let mut hostlist: Hostlist = "n[1-5]".parse()?;
        let expected_orig = vec!["n2", "n3", "n4", "n5"];

        assert_eq!(hostlist.len(), 5);
        assert_eq!(hostlist.next(), Some("n1".to_string()));
        assert_eq!(hostlist.len(), 4);

        let mut expected = expected_orig.clone();
        for host in &hostlist {
            let e = expected.remove(0);
            assert_eq!(e, host);
        }
        assert!(expected.is_empty());
        assert_eq!(hostlist.len(), 4);

        Ok(())
    }

    #[test]
    fn test_hostlist_iter5() -> Result<()> {
        let mut hostlist: Hostlist = "n[1-5]".parse()?;

        assert_eq!(hostlist.len(), 5);
        assert_eq!(hostlist.next(), Some("n1".to_string()));
        assert_eq!(hostlist.len(), 4);
        assert_eq!(hostlist.next(), Some("n2".to_string()));
        assert_eq!(hostlist.len(), 3);
        assert_eq!(hostlist.next(), Some("n3".to_string()));
        assert_eq!(hostlist.len(), 2);
        assert_eq!(hostlist.next(), Some("n4".to_string()));
        assert_eq!(hostlist.len(), 1);
        assert_eq!(hostlist.next(), Some("n5".to_string()));
        assert_eq!(hostlist.len(), 0);
        assert_eq!(hostlist.next(), None);
        assert_eq!(hostlist.len(), 0);

        #[allow(clippy::useless_conversion)]
        let mut hostlist_iter = hostlist.into_iter();
        assert!(hostlist_iter.next().is_none());

        Ok(())
    }

    #[test]
    fn test_hostlist_iter6() -> Result<()> {
        let mut hostlist: Hostlist = "n[1-5]".parse()?;

        assert_eq!(hostlist.len(), 5);
        assert_eq!(hostlist.next(), Some("n1".to_string()));
        assert_eq!(hostlist.len(), 4);
        assert_eq!(hostlist.next(), Some("n2".to_string()));
        assert_eq!(hostlist.len(), 3);
        assert_eq!(hostlist.next(), Some("n3".to_string()));
        assert_eq!(hostlist.len(), 2);
        assert_eq!(hostlist.next(), Some("n4".to_string()));
        assert_eq!(hostlist.len(), 1);

        let expected_orig = vec!["n5"];

        let mut expected = expected_orig.clone();
        for host in &hostlist {
            let e = expected.remove(0);
            assert_eq!(e, host);
        }
        assert!(expected.is_empty());

        // hostlist is unchanged
        assert_eq!(hostlist.len(), 1);

        // and can still be modified
        assert_eq!(hostlist.next(), Some("n5".to_string()));
        assert_eq!(hostlist.len(), 0);
        assert_eq!(hostlist.next(), None);
        assert_eq!(hostlist.len(), 0);

        Ok(())
    }

    #[test]
    fn test_hostlist_invalid() {
        let inputs = [
            ("[1-3]", "no prefix"),
            ("node[1-", "unclosed bracket range"),
            ("node]1-3[", "inverted brackets"),
            ("node[3-1]", "reversed range"),
            ("node[1,]", "trailing comma"),
            ("node[,1]", "leading comma"),
            ("node[1,,3]", "double comma"),
            ("node[1-3", "missing closing bracket"),
            ("node1-3]", "missing opening bracket"),
            ("node[a-3]", "non-numeric character in range"),
            ("node[-1-3]", "negative number in range"),
            ("node[1.5-3]", "non-integer in range"),
            ("node[1--3]", "double hyphen in range"),
            ("node[[1-3]]", "nested brackets"),
            ("node[1:2]", "using colon instead of hyphen for range"),
        ];

        for (input, description) in inputs {
            let result = input.parse::<Hostlist>();
            if let Ok(hostlist) = result {
                panic!("Failure on '{input}' ({description}). Hostlist parsed to: '{hostlist}'.");
            }
        }
    }

    #[test]
    fn test_hostlist_valid() -> Result<()> {
        let inputs = [
            ("node[1-3]", vec!["node1", "node2", "node3"]),
            ("node[01-03]", vec!["node1", "node2", "node3"]),
            ("node[04-06]", vec!["node4", "node5", "node6"]),
            ("compute[1,3,5]", vec!["compute1", "compute3", "compute5"]),
            (
                "server[1-3,5,7-9]",
                vec![
                    "server1", "server2", "server3", "server5", "server7", "server8", "server9",
                ],
            ),
            (
                "host[1-3]-rack[1-2]",
                vec![
                    "host1-rack1",
                    "host1-rack2",
                    "host2-rack1",
                    "host2-rack2",
                    "host3-rack1",
                    "host3-rack2",
                ],
            ),
            (
                "node[4-6,8,10-12]",
                vec![
                    "node4", "node5", "node6", "node8", "node10", "node11", "node12",
                ],
            ),
            (
                "prefix[1-3]suffix",
                vec!["prefix1suffix", "prefix2suffix", "prefix3suffix"],
            ),
            (
                "node[1-3],server[1-2]",
                vec!["node1", "node2", "node3", "server1", "server2"],
            ),
            ("", vec![]),
            ("singlenode", vec!["singlenode"]),
            ("node[0-0]", vec!["node0"]),
            ("node[42]", vec!["node42"]),
        ];

        for (input, expected) in inputs {
            eprintln!("input: \"{input}\"");
            let hosts = input.parse::<Hostlist>()?.collect::<Vec<_>>();
            assert_eq!(hosts, expected);
        }

        Ok(())
    }

    #[test]
    fn test_hostlist_display() -> Result<()> {
        let inputs = [
            ("node[1-3]", "node[1-3]"),
            ("node[01-03]", "node[1-3]"),
            ("node[04-06]", "node[4-6]"),
            ("node[04-04]", "node[4]"),
            ("compute[1,3,5]", "compute[1,3,5]"),
            ("server[1-3,5,7-9]", "server[1-3,5,7-9]"),
            ("host[1-3]-rack[1-2]", "host[1-3]-rack[1-2]"),
            ("node[4-6,8,10-12]", "node[4-6,8,10-12]"),
            ("prefix[1-3]suffix", "prefix[1-3]suffix"),
            ("node[1-3],server[1-2]", "node[1-3],server[1-2]"),
            ("", ""),
            ("singlenode", "singlenode"),
            ("node[0]", "node[0]"),
            ("node[0-0]", "node[0]"),
            ("node[42]", "node[42]"),
        ];

        for (input, expected) in inputs {
            eprintln!("input: \"{input}\"");
            let hostlist = input.parse::<Hostlist>()?;
            assert_eq!(hostlist.to_string(), expected);
        }

        Ok(())
    }
}
