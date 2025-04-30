# hostlist_iter

A "hostlist" handling library with low memory footprint.

[![Crates.io](https://img.shields.io/crates/v/hostlist_iter.svg)](https://crates.io/crates/hostlist_iter)
[![Documentation](https://docs.rs/hostlist_iter/badge.svg)](https://docs.rs/hostlist_iter)

## Overview

A hostlist is an expression representing a set of host names, commonly used in
High Performance Computing (HPC) system applications. This library provides
efficient parsing and manipulation of hostlist expressions with a minimal
memory footprint.

Examples of hostlists and their equivalent host names:
- `node[1-3]` == `node1`, `node2`, `node3`
- `n[1-2]m[5-6]` == `n1m5`, `n1m6`, `n2m5`, `n2m6`

## Features
1. **Memory Efficient**: Memory footprint scales with the number of sections in the hostlist expression, not with the number of hosts represented by it.
2. **Bidirectional Conversion**: Provides both hostlist->hosts conversion and hosts->hostlist compression.
3. **Clear grammar**: Uses the [Pest](https://pest.rs/) parser library with a separate file clearly defining [our grammar](src/hostlist.pest).

## Installation
To add this crate to your project:
```bash
cargo add hostlist_iter
```

To install an optional CLI tool `hostlist_iter`:
```bash
cargo install hostlist_iter --features cli
```

## Usage
### Converting a hostlist to hosts
Use the `Hostlist` type and iterate over it:
```rust
use hostlist_iter::Hostlist;

fn example() -> Result<(), hostlist_iter::Error> {
    let hostlist: Hostlist = "node[1-3,5]".parse()?;
    // or
    let hostlist = Hostlist::new("node[1-3,5]")?;

    for host in &hostlist {
        println!("{host}");
    }

    Ok(())
}
```
output:
```
node1
node2
node3
node5
```

If you just want a Vec of host names, you can either collect() them from a
`Hostlist`, or use the `expand_hostlist` convenience function:
```rust
use hostlist_iter::expand_hostlist;

fn example() -> Result<(), hostlist_iter::Error> {
    let hosts = expand_hostlist("node[1-3,5]")?;

    assert_eq!(hosts, vec!["node1", "node2", "node3", "node5"]);
    Ok(())
}
```
Note: Using `expand_hostlist` or collecting hosts into a Vec will not achieve
the memory footprint feature mentioned above.

### Converting hosts to a hostlist
Use the `collapse_hosts` function:
```rust
use hostlist_iter::collapse_hosts;

fn example() -> Result<(), hostlist_iter::Error> {
    let hosts = vec!["n6", "n2", "n3", "n1", "abc1"];
    let hostlist = collapse_hosts(hosts)?;

    assert_eq!(hostlist, "abc1,n[1-3,6]");
    Ok(())
}
```

## Error handling
This crate provides custom `Error` and `Result` types. The most common error
variant is likely to be `ParseError` which contains a Box'd
`pest::error::Error<Rule>` with more details. All errors implement the Display
trait for user-friendly output.

```rust
use hostlist_iter::Hostlist;

fn example() {
    match "bad[hostlist".parse::<Hostlist>() {
        Ok(hostlist) => println!("Valid hostlist: {}", hostlist),
        Err(e) => println!("Error parsing hostlist: {}", e),
    }
}
```

## Memory footprint
The following `Hostlist`s use the same amount of memory due to the internal
representation not expanding the entire range up front:
```rust
let hostlist1: Hostlist = "node[1-10]".parse()?;
let hostlist2: Hostlist = "node[1-1000000000]".parse()?;
```

Iterating over each hostlist will generate values on demand rather than storing
them all in memory.

## Grammar exploration
Pest's website provides a convenient exploration tool for grammar rules. You
can enter your own hostlist expressions and see how they will be parsed.

[link to Pest Editor for our grammar](https://pest.rs/?g=N4Ig5gTghgtjURALhAdQBIEkAqBRAygAoCCAwrgAQC8FA%2BsBQDohMsA%2BrjjALsxRwDlcqADKYhFAL6MAdrIAWAewDO3ADYBLVRR26a9CvgDymCgD8KACiWrNq2gFM1DmOavMANHws31W7o7OMACUAFRhbrgmUgoqfvZOLtQ6DKpQ3BoAxoFJFpbQMmAObmkZ2YkwAPwR0nIypVk5rjQAAqnc6Y2Z8ggA1DH1HWW03QjJBsT4pJiYtMQihOjEAgCqALK4AEqYpPystHwczAC0h6wAdHy1sgVFuvfJFAzMANreFLcOTW6Wnu%2BfTQiFmYAF0rjcoIUvhU9HRUhoYAAHZy0T57GQAVxgACMHBABsoEcivmiaE8KJicXi3Cd3pTcfjavTqQ9km0KJNprMACKYADiOH613qRJRvhksGK%2BgYxlMeUREAcADMNAAPNyUvGNZQYpUq9UcTUQbW6-V7BXKtXBSLRWoWs2tBiWACERpNerVJSGXR6EBqsjd2R1HvVjo5UxmtF5Auw-Qszoa2VGjNkIA8IA0MkRGO4yBAMkUABMHC8AIzHUsABg8ACZK8cAMyVkEeJWKRRl441jwAVhBaZAykSmW4DkLmwxzjzvjsuckQA#editor)

(click "Wide Mode" for a better experience)

## API Reference

### Core Types

- **`Hostlist`** - Main type representing a parsed hostlist expression
- **`Error`** - Error type for all operations in this crate
- **`Result<T>`** - Specialized result type for this crate

### Key Functions

- **`Hostlist::new(expr: &str) -> Result<Hostlist>`** - Parse a hostlist expression
- **`expand_hostlist(expr: &str) -> Result<Vec<String>>`** - Convert a hostlist to a vector of host names
- **`collapse_hosts(hosts: impl IntoIterator<Item = impl AsRef<str>>) -> Result<String>`** - Convert host names to a compact(-ish) hostlist expression

## Limitations

- `collapse_hosts` only collapses along a single numeric suffix
- zero-padding in ranges is ignored since the bounds are converted to integers (`"n[001-002]"` == `["n1", "n2"]`)

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   https://opensource.org/licenses/MIT)

at your option.
