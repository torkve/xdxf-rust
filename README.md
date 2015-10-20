# xdxf-rust

This is the Rust implementation of the "XDXF" dictionary format parser.

It doesn't support real XDXF format, because the dictionaries I have don't comply with it,
so it should be considered as an example how such a parser could be implemented.

## Dependencies

* [sxd-document](https://crates.io/crates/sxd-document) XML DOM library
* [radix_trie](https://crates.io/crates/radix_trie) Prefix tree to store dictionary keys
* [regex](https://crates.io/crates/regex) Regular expressions library to format corrupted articles
* [regex_macros](https://crates.io/crates/regex_macros) Macros to build regexes in compile-time
