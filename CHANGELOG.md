# Changelog

## 0.1.0-0 - 2026-08-02

First release: byte-oriented Crockford Base32 encoding and decoding,
`no_std` compatible and free of dependencies.

- Allocating API behind the default `alloc` feature: `encode`, `decode`,
  `try_decode`, `encode_with_check`, and `decode_with_check`.
- Allocation-free API: `sized_encode` and `sized_decode` (usable in
  `const` context), `encode_to_slice`, `decode_to_slice`,
  `try_decode_to_slice`, `encode_into`, `encoded_len`, `decoded_len`,
  `check_digit`, `check_symbol`, and the lookup tables.
- The strict decoders accept only the canonical encoding of the bytes
  they return; the lenient ones skip every byte outside the alphabet.
- `DecodeError` and `CheckError` are `#[non_exhaustive]`, so later
  versions can add variants without a breaking change.
