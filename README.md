# ps-crockford32

[![CI](https://github.com/prokopschield/ps-crockford32/actions/workflows/ci.yml/badge.svg)](https://github.com/prokopschield/ps-crockford32/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/ps-crockford32.svg)](https://crates.io/crates/ps-crockford32)
[![docs.rs](https://img.shields.io/docsrs/ps-crockford32)](https://docs.rs/ps-crockford32)

[Crockford Base32](https://www.crockford.com/base32.html) encoding and
decoding for byte slices, `no_std` compatible.

The Crockford alphabet (`0123456789ABCDEFGHJKMNPQRSTVWXYZ`) excludes the
visually ambiguous characters `I`, `L`, `O`, and `U`. Decoding is
case-insensitive, accepts common misreadings (`O` as `0`; `I` or `L` as
`1`), and, in the lenient functions, silently skips whitespace, hyphens,
and other characters outside the alphabet.

Note that this crate encodes byte slices, not integers: the final symbol
of a partial block is padded with zero bits on the right, whereas
Crockford's specification zero-extends integers on the left. Strings and
check symbols produced by this crate are therefore not interchangeable
with integer-based Crockford implementations.

## Installation

```sh
cargo add ps-crockford32
```

On a target without an allocator, disable the default `alloc` feature:

```sh
cargo add ps-crockford32 --no-default-features
```

## Usage

```rust
use ps_crockford32::{decode, encode};

assert_eq!(encode(b"hi"), "D1MG");
assert_eq!(decode(b"D1MG"), b"hi");
assert_eq!(decode(b"d1-mg"), b"hi");
```

The lenient `decode` accepts arbitrarily polluted input, so use
`try_decode` when input must be validated. It accepts an input only if it
is the canonical encoding of the bytes it returns, so
`encode(&try_decode(input)?)` reproduces `input` up to hyphens, case, and
ambiguous-glyph aliases; no re-encode-and-compare step is needed to
establish canonicity:

```rust
use ps_crockford32::{try_decode, DecodeError};

assert_eq!(try_decode(b"D1-MG"), Ok(b"hi".to_vec()));
assert_eq!(
    try_decode(b"D1 MG"),
    Err(DecodeError::InvalidByte { position: 2, byte: b' ' })
);

// `b"29"` denotes the same byte as the canonical `b"28"`, so only the
// latter is accepted.
assert_eq!(try_decode(b"28"), Ok(vec![0x12]));
assert_eq!(
    try_decode(b"29"),
    Err(DecodeError::NonZeroPadding { position: 1, byte: b'9' })
);
```

The `sized_encode` and `sized_decode` functions are allocation-free and
usable in `const` context. They truncate to their const-generic size,
which the caller chooses at compile time:

```rust
use ps_crockford32::{sized_decode, sized_encode};

const ENCODED: [u8; 4] = sized_encode(b"hi");
const DECODED: [u8; 2] = sized_decode(&ENCODED);

assert_eq!(&ENCODED, b"D1MG");
assert_eq!(&DECODED, b"hi");
```

For runtime-length buffers without allocation, `encode_to_slice`,
`decode_to_slice`, and `try_decode_to_slice` write into a caller-provided
slice and return the number of bytes written. Each writes the whole result
or none of it, reporting a `CapacityError` when the slice is too short;
`encoded_len` and `decoded_len` give the required sizes:

```rust
use ps_crockford32::{
    decode_to_slice, decoded_len, encode_to_slice, encoded_len, CapacityError,
};

let mut encoded = [0u8; 16];
let mut decoded = [0u8; 16];

let symbols = encode_to_slice(b"hi", &mut encoded)?;
let bytes = decode_to_slice(&encoded[..symbols], &mut decoded)?;

assert_eq!(&encoded[..symbols], b"D1MG");
assert_eq!(&decoded[..bytes], b"hi");

assert_eq!(symbols, encoded_len(2));
assert_eq!(bytes, decoded_len(symbols));

assert_eq!(
    encode_to_slice(b"hi", &mut encoded[..3]),
    Err(CapacityError { required: 4, available: 3 })
);
# Ok::<(), CapacityError>(())
```

`try_decode_to_slice` is the allocation-free counterpart of `try_decode`,
applying the same canonicity rules:

```rust
use ps_crockford32::{try_decode_to_slice, DecodeError};

let mut decoded = [0u8; 16];

assert_eq!(try_decode_to_slice(b"D1-MG", &mut decoded), Ok(2));
assert_eq!(&decoded[..2], b"hi");
assert_eq!(
    try_decode_to_slice(b"D1 MG", &mut decoded),
    Err(DecodeError::InvalidByte { position: 2, byte: b' ' })
);
```

The `encode_with_check` and `decode_with_check` functions implement
Crockford's optional trailing check symbol, computed modulo 37 over the
payload interpreted as a big-endian integer. The body is decoded strictly,
so a non-canonical body is rejected even when the check digit happens to
agree:

```rust
use ps_crockford32::{decode_with_check, encode_with_check, CheckError};

let encoded = encode_with_check(b"\x12");

assert_eq!(encoded, "28J");
assert_eq!(decode_with_check(encoded.as_bytes()), Ok(vec![0x12]));
assert!(matches!(decode_with_check(b"28X"), Err(CheckError::Mismatch { .. })));
assert!(matches!(decode_with_check(b"29J"), Err(CheckError::Body(_))));
```

Every single-symbol substitution and every transposition of two adjacent
symbols is caught. Alterations that change the number of symbols, however,
are caught only with probability 36/37, as for any check digit modulo 37;
in particular, passing plain `encode` output to `decode_with_check` is not
reliably an error, so tell checked strings from unchecked ones by their
symbol count rather than by whether `decode_with_check` rejects them.

## Features

- `alloc` (default): enables the `encode`, `decode`, `try_decode`,
  `encode_with_check`, and `decode_with_check` functions, which return
  owned `String` and `Vec<u8>` values. With `default-features = false`,
  the allocation-free APIs (`sized_encode`, `sized_decode`,
  `encode_to_slice`, `decode_to_slice`, `try_decode_to_slice`,
  `encode_into`, `encoded_len`, `decoded_len`, `check_digit`,
  `check_symbol`, and the lookup tables) remain available.

## Minimum supported Rust version

Rust 1.81.

## License

GPL-3.0-or-later. See [COPYING](COPYING) for the full license text.
