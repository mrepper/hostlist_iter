use std::collections::{HashMap, HashSet};

use pest::Parser;

mod hostlist;
mod hostlistelem;
mod range;
mod simplerange;

pub mod error;
pub use crate::error::{Error, Result};
pub use crate::hostlist::Hostlist;

use crate::hostlist::{HostlistParser, Rule};

/// Expands a hostlist expression into a list of host names
///
/// # Errors
/// Will return `Err` if there are issues parsing the provided hostlist expression.
/// ```
/// use hostlist_iter::expand_hostlist;
///
/// fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
///   let hosts: Vec<String> = expand_hostlist("node[1-3,5]")?;
///   assert_eq!(hosts, vec!["node1", "node2", "node3", "node5"]);
///
///   Ok(())
/// }
/// ```
pub fn expand_hostlist(hostlist: &str) -> Result<Vec<String>> {
    let hostlist = Hostlist::new(hostlist)?;
    Ok(hostlist.into_iter().collect())
}

/// Collapses a list of host names into a hostlist expression
///
/// # Errors
/// Will return `Err` if any host name cannot be parsed.
/// ```
/// use hostlist_iter::collapse_hosts;
///
/// fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
///   let hosts = vec!["node2", "node1", "node3", "node5", "node6"];
///   let hostlist = collapse_hosts(hosts)?;
///   assert_eq!("node[1-3,5-6]", hostlist);
///
///   Ok(())
/// }
/// ```
pub fn collapse_hosts(hosts: impl IntoIterator<Item = impl AsRef<str>>) -> Result<String> {
    let mut hostlist_elems: Vec<String> = Vec::new();
    let mut prefix_map: HashMap<String, HashSet<u32>> = HashMap::new();

    for host in hosts {
        let host = host.as_ref();
        if host.is_empty() {
            return Err(Error::InvalidHostname(host.into()));
        }

        let mut prefix = None;
        let mut suffix = None;
        let pairs = HostlistParser::parse(Rule::simple_hostname, host)?;
        for pair in pairs {
            match pair.as_rule() {
                Rule::prefix => prefix = Some(pair.as_str()),
                Rule::numeric_suffix => suffix = Some(pair.as_str()),
                Rule::EOI => break,
                rule => return Err(Error::UnexpectedParserState(rule)),
            }
        }

        let prefix = prefix
            .ok_or_else(|| Error::InvalidHostname(host.to_string()))?
            .to_string();

        if let Some(suffix) = suffix {
            let suffix = suffix.parse::<u32>()?;
            prefix_map.entry(prefix).or_default().insert(suffix);
        } else {
            hostlist_elems.push(prefix);
        }
    }

    let mut sorted_prefixes: Vec<&String> = prefix_map.keys().collect();
    sorted_prefixes.sort_unstable();
    for prefix in sorted_prefixes {
        let nums: Vec<u32> = prefix_map[prefix].iter().copied().collect();
        let mut host = prefix.clone();
        host.push_str(collapse_range(nums).as_str());
        hostlist_elems.push(host);
    }

    Ok(hostlist_elems.join(","))
}

/// Convert an iterator of numbers into a range expression
fn collapse_range<I>(nums: I) -> String
where
    I: IntoIterator<Item = u32>,
{
    let mut nums: Vec<u32> = nums.into_iter().collect();
    nums.sort_unstable();
    nums.dedup();

    let mut collapsed = String::new();
    let mut in_range = false;
    let mut needs_brackets = false;
    let mut prev_num = 0;
    for (i, num) in nums.into_iter().enumerate() {
        if i == 0 {
            collapsed += &num.to_string();
        } else if num == prev_num + 1 {
            if !in_range {
                // saw the second number in a range
                collapsed.push('-');
                in_range = true;
                needs_brackets = true;
            }
        } else {
            if in_range {
                // previous number was the end of a range
                collapsed += &prev_num.to_string();
                in_range = false;
            }
            // current number starts a new range
            collapsed.push(',');
            needs_brackets = true;
            collapsed += &num.to_string();
        }
        prev_num = num;
    }
    if in_range {
        collapsed += &prev_num.to_string();
    }

    if needs_brackets {
        format!("[{collapsed}]")
    } else {
        collapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collapse_range() {
        let tests = [
            (vec![], ""),
            (vec![7], "7"),
            (vec![1, 2], "[1-2]"),
            (vec![1, 2, 3], "[1-3]"),
            (vec![1, 2, 3, 5], "[1-3,5]"),
            (vec![1, 2, 3, 5, 6, 7], "[1-3,5-7]"),
            (vec![1, 3, 5], "[1,3,5]"),
            (vec![1, 3, 4, 5], "[1,3-5]"),
            (vec![1, 1, 3, 4, 5], "[1,3-5]"),
        ];

        for (input, expected) in tests {
            assert_eq!(collapse_range(input), expected);
        }
    }

    #[test]
    fn test_expand_simple1() {
        let hostlist = "n1";
        let expected = vec!["n1"];

        let result = expand_hostlist(hostlist).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_expand_simple2() {
        let hostlist = "abc,cba1";
        let expected = vec!["abc", "cba1"];

        let result = expand_hostlist(hostlist).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_expand_simple3() {
        let hostlist = "n[1-3,9]";
        let expected = vec!["n1", "n2", "n3", "n9"];

        let result = expand_hostlist(hostlist).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_expand_valid() {
        let hostlist = "node[1-5]";
        let expected = vec!["node1", "node2", "node3", "node4", "node5"];

        let result = expand_hostlist(hostlist).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_expand_invalid_format() {
        let hostlist = "node[1-5"; // Missing closing bracket
        let result = expand_hostlist(hostlist);
        assert!(matches!(result, Err(Error::ParseError(_))));
    }

    #[test]
    fn test_expand_invalid_reversed_range() {
        let hostlist = "node[5-1]";
        let result = expand_hostlist(hostlist);
        assert!(matches!(
            result,
            Err(Error::InvalidRangeReversed { start: 5, end: 1 })
        ));
    }

    #[test]
    fn test_expand_range_integer_overflow() {
        let hostlist = "n[4294967295]";
        let result = expand_hostlist(hostlist);
        assert!(matches!(result, Err(Error::TooLarge(4_294_967_295))));
    }

    #[test]
    fn test_expand_single() {
        let hostlist = "node[7-7]";
        let expected = vec!["node7"];
        let result = expand_hostlist(hostlist).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_expand_space_separation() {
        let hostlist = "n[5-7,60]-01, an[1-3,9],server";
        let expected = vec![
            "an1", "an2", "an3", "an9", "n5-01", "n6-01", "n7-01", "n60-01", "server",
        ];
        let result = expand_hostlist(hostlist).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_expand_multi_range() {
        let hostlist = "n[1-2]m[1-3]o[1-2]";
        let expected = vec![
            "n1m1o1", "n1m1o2", "n1m2o1", "n1m2o2", "n1m3o1", "n1m3o2", "n2m1o1", "n2m1o2",
            "n2m2o1", "n2m2o2", "n2m3o1", "n2m3o2",
        ];
        let result = expand_hostlist(hostlist).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_expand_multi_range_adjacent() {
        let hostlist = "n[1-2][1-3][1-2]";
        let expected = vec![
            "n111", "n112", "n121", "n122", "n131", "n132", "n211", "n212", "n221", "n222", "n231",
            "n232",
        ];
        let result = expand_hostlist(hostlist).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_expand_multi_range_list() {
        let hostlist = "n[1-2]m[1-3]o[1-2],compute[1-2]x[5-6]";
        let expected = vec![
            "compute1x5",
            "compute1x6",
            "compute2x5",
            "compute2x6",
            "n1m1o1",
            "n1m1o2",
            "n1m2o1",
            "n1m2o2",
            "n1m3o1",
            "n1m3o2",
            "n2m1o1",
            "n2m1o2",
            "n2m2o1",
            "n2m2o2",
            "n2m3o1",
            "n2m3o2",
        ];
        let result = expand_hostlist(hostlist).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_expand_range_overlap1() {
        let tests = [
            ("n[1]", vec!["n1"]),
            ("n[1-3]", vec!["n1", "n2", "n3"]),
            ("n[5,4-6]", vec!["n4", "n5", "n6"]),
            ("n[4-6,5]", vec!["n4", "n5", "n6"]),
            ("n[4,4-6]", vec!["n4", "n5", "n6"]),
            ("n[4-6,4]", vec!["n4", "n5", "n6"]),
            ("n[0,1-2,3-4]", vec!["n0", "n1", "n2", "n3", "n4"]),
            ("n[1,1-2,1-3]", vec!["n1", "n2", "n3"]),
            ("n[1-4,2-6]", vec!["n1", "n2", "n3", "n4", "n5", "n6"]),
            ("n[2-6,1-4]", vec!["n1", "n2", "n3", "n4", "n5", "n6"]),
            ("n[1-5,2-4]", vec!["n1", "n2", "n3", "n4", "n5"]),
            ("n[2-4,1-5]", vec!["n1", "n2", "n3", "n4", "n5"]),
        ];
        for (input, expected) in tests {
            assert_eq!(expected, expand_hostlist(input).unwrap());
        }
    }

    #[test]
    fn test_expand_range_overlap2() {
        let hostlist = "n[3-8,1-6,5-10]";
        let expected = vec!["n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9", "n10"];
        let result = expand_hostlist(hostlist).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_expand_range_overlap3() {
        let hostlist = "n[1-4,2]";
        let expected = vec!["n1", "n2", "n3", "n4"];
        let result = expand_hostlist(hostlist).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_expand_range_overlap4() {
        let hostlist = "n[5,3-6,1-3,7,7-10]";
        let expected = vec!["n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9", "n10"];
        let result = expand_hostlist(hostlist).unwrap();
        assert_eq!(expected, result);
    }

    #[test]
    fn test_collapse_hosts() -> Result<()> {
        let tests = [
            (vec!["1"], "1"),
            (vec!["1", "2"], "[1-2]"),
            (vec!["1", "2", "4"], "[1-2,4]"),
            (vec!["n0", "1", "2", "4", "n1"], "[1-2,4],n[0-1]"),
            (vec!["n1"], "n1"),
            (vec!["n1", "n2"], "n[1-2]"),
            (vec!["n1", "n2", "n3"], "n[1-3]"),
            (vec!["some.host"], "some.host"),
            (vec!["n1", "n2", "n3", "n5"], "n[1-3,5]"),
            (vec!["n1", "n2", "n3", "n5", "n6"], "n[1-3,5-6]"),
            (vec!["n1", "n2", "n5", "n6", "foo1"], "foo1,n[1-2,5-6]"),
            (
                vec!["n1", "n2", "n3", "n5", "n6", "foo1"],
                "foo1,n[1-3,5-6]",
            ),
        ];
        for (input, expected) in tests {
            assert_eq!(expected, collapse_hosts(input)?);
        }

        Ok(())
    }

    #[test]
    fn test_collapse_hosts_invalid() {
        let invalid_inputs = [
            vec!["foo[1-2]"],
            vec!["node1", "node2?"],
            vec![""],
            vec!["!#*&^!*&#$"],
            vec!["😀"],
        ];

        for input in &invalid_inputs {
            match collapse_hosts(input) {
                Err(Error::InvalidHostname(_) | Error::ParseError(_)) => (),
                _ => panic!("unexpected result from collapse_hosts for input {input:?}"),
            }
        }
    }
}
