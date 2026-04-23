# pct

[![Crate](https://img.shields.io/crates/v/pct.svg?style=flat-square)](https://crates.io/crates/pct)
[![Docs](https://img.shields.io/docsrs/pct?style=flat-square)](https://docs.rs/pct)
[![MSRV](https://img.shields.io/crates/msrv/pct?style=flat-square)](https://crates.io/crates/pct)
[![License](https://img.shields.io/crates/l/pct.svg?style=flat-square)](#license)

A small, allocation-conscious Rust crate for percent-encoded strings used in URLs, URIs, IRIs, etc. — parse, validate, encode, decode, compare.

```rust
use pct::{PctStr, PctString, UriReserved};

let s = PctStr::new("Hello%20World%21")?;
assert_eq!(s, "Hello World!");
assert_eq!(s.decode(), "Hello World!");

let encoded = PctString::encode("Hello World!".chars(), UriReserved::Any);
assert_eq!(encoded.as_str(), "Hello%20World%21");
```

Pick an `Encoder` impl (`UriReserved`, `IriReserved`) or write your own:

```rust
use pct::{Encoder, PctString, UriReserved};

struct Upper;
impl Encoder for Upper {
    fn encode(&self, c: char) -> bool {
        UriReserved::Any.encode(c) || c.is_uppercase()
    }
}

let s = PctString::encode("Hello World!".chars(), Upper);
assert_eq!(s.as_str(), "%48ello%20%57orld%21");
```

---

## Why this fork

Fork of [`pct-str`](https://crates.io/crates/pct-str) by [Timothée Haudebourg](https://github.com/timothee-haudebourg/pct-str). Public API and RFC behavior unchanged; this fork adds:

- SWAR plain-run scanner and portable-SIMD path (nightly, gated on `simd`) for validate / decode / encode.
- Hex-decode lookup tables replacing per-nibble branches.
- Byte-level fast paths for `new`, `decode`, `encode_bytes`, `eq`, `ord`, `hash`, `len`.
- `memchr`-accelerated `%` scan (default-on).
- Criterion bench suite under `benches/`.
- Rust 2024 edition, MSRV 1.85.
- Renamed crate to `pct`.

Credit and history preserved — see [Attribution](#attribution).

## Install

```sh
cargo add pct
```

## Feature flags

| Flag     | Default | Enables                                                           |
| -------- | :-----: | ----------------------------------------------------------------- |
| `std`    |   yes   | `std::error::Error` impls, owned `PctString`, `String` APIs       |
| `memchr` |   yes   | `memchr`-accelerated `%` scan in validate / decode / encode       |
| `simd`   |         | Portable-SIMD plain-run scanner. **Requires nightly rustc.**      |

For `no_std`, disable default features. You get `PctStr` (borrowed, zero-alloc) and the `Encoder` trait. Re-enable `std` for `PctString` and `String`-returning APIs.

## Streaming decode

`PctStr::chars()` and `PctStr::bytes()` are lazy iterators over the decoded form. No intermediate `String`, works under `no_std`:

```rust
let s = pct::PctStr::new("caf%C3%A9")?;
for ch in s.chars() { /* 'c', 'a', 'f', 'é' */ }
```

For encoding from a known `&str`, prefer `PctString::encode_bytes` — it skips UTF-8 re-iteration and hits the SWAR/SIMD scanner directly.

## Equivalence

Equality, ordering, and hashing compare the **decoded** bytes — `PctStr::new("%41") == "A"`, and hex case (`%2f` vs `%2F`) is irrelevant. Keep this in mind when using `PctStr` / `PctString` as map keys: two values with different `as_str()` can hash equal.

## Examples

See `examples/` for runnable end-to-end usage:

```sh
cargo run --example encode
cargo run --example str
cargo run --example string
```

## Benchmarks

```sh
cargo bench
cargo +nightly bench --features simd
```

Criterion output: `target/criterion/`.

## MSRV

Rust 1.85 (edition 2024). The `simd` feature additionally requires nightly.

## Attribution

Original crate: [`pct-str`](https://crates.io/crates/pct-str) by [Timothée Haudebourg](https://github.com/timothee-haudebourg/pct-str). Upstream commits are preserved in this repo's history under their original authorship. This fork is a thin layer of performance and ergonomics work on top of their design.

## License

Dual-licensed, same as upstream. Pick whichever fits:

- [Apache-2.0](https://github.com/mskvarc/pct/blob/master/LICENSE-APACHE.md)
- [MIT](https://github.com/mskvarc/pct/blob/master/LICENSE-MIT.md)
