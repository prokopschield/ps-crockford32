//! Crockford Base32 encoding and decoding.
//!
//! The Crockford alphabet excludes the visually ambiguous characters `I`,
//! `L`, `O`, and `U`. Decoding is case-insensitive and accepts each of
//! them as an alias for its lookalike (`O` as `0`; `I` or `L` as `1`;
//! `U` as `V`).
//!
//! # Byte-oriented encoding
//!
//! This crate encodes byte slices, not integers. When the bit length of
//! the input is not a multiple of five, the final symbol is padded with
//! zero bits on the right. Crockford's specification instead encodes
//! integers, zero-extending on the left, so strings produced by this
//! crate are not interchangeable with integer-based Crockford
//! implementations (see [`encode`] for details).
//!
//! # Lenient and strict decoding
//!
//! [`decode`], [`sized_decode`], and [`decode_to_slice`] are lenient:
//! they silently skip every byte outside the alphabet. [`try_decode`] and
//! [`try_decode_to_slice`] are strict: they accept an input only if it is
//! the canonical encoding of the bytes it denotes, so they reject bytes
//! outside the alphabet as well as a final symbol whose discarded bits are
//! not all zero or that contributes no bits at all.
//!
//! # Output slices
//!
//! [`encode_to_slice`], [`decode_to_slice`], and [`try_decode_to_slice`]
//! write the whole result or none of it, reporting a [`CapacityError`] when
//! the output slice is too short; [`encoded_len`] and [`decoded_len`] give
//! the sizes they require. [`sized_encode`] and [`sized_decode`] instead
//! truncate to their const-generic size, which the caller chooses at
//! compile time.
//!
//! # Check symbols
//!
//! Crockford's specification defines an optional trailing check symbol
//! computed as `n mod 37`, where `n` is the integer being encoded. Five
//! additional symbols (`*`, `~`, `$`, `=`, `U`) extend the alphabet to 37
//! values to accommodate check-digit values 32-36. Because this crate
//! encodes byte slices rather than abstract integers, the check digit is
//! computed over the byte slice interpreted as a big-endian integer;
//! leading zero bytes do not affect the result.
//!
//! Specification-conformant implementations compute the check over the
//! integer denoted by the symbol string, which generally differs from the
//! big-endian value of the decoded bytes. For the payload `[0x12]`,
//! [`encode_with_check`] produces `"28J"` (0x12 mod 37 = 18 = `J`),
//! whereas an integer-based implementation reads the body `"28"` as the
//! number 72 and expects 72 mod 37 = 35 = `=`. Check symbols produced by
//! this crate are therefore not interchangeable with those of
//! integer-based Crockford implementations.
//!
//! # `no_std` support
//!
//! This crate is `no_std` compatible. The `alloc` feature (enabled by
//! default) adds the convenience functions that return owned
//! [`String`](alloc::string::String) and [`Vec`](alloc::vec::Vec) values:
//! [`encode`], [`decode`], [`try_decode`], and the [`encode_with_check`]
//! and [`decode_with_check`] pair. With the feature disabled, the
//! allocation-free APIs remain available: [`sized_encode`],
//! [`sized_decode`], [`encode_to_slice`], [`decode_to_slice`],
//! [`try_decode_to_slice`], [`encode_into`], [`encoded_len`],
//! [`decoded_len`], [`check_digit`], [`check_symbol`], the [`ALPHABET`],
//! [`DECODE_MAP`], [`CHECK_ALPHABET`], and [`CHECK_DECODE_MAP`] lookup
//! tables, and the [`INVALID`] sentinel.

#![no_std]
// The documentation links the allocating API, which the `alloc` feature
// gates out of existence; with the feature disabled those links have no
// target. The documented configuration is the default one, where they all
// resolve and the lint stays active.
#![cfg_attr(not(feature = "alloc"), allow(rustdoc::broken_intra_doc_links))]

#[cfg(feature = "alloc")]
extern crate alloc;

// Every module documents itself with an inner `//!` block, so the
// declarations below carry no doc comment of their own.
mod capacity;
mod check;
mod decoder;
mod encoder;

#[cfg(test)]
mod tests;

/// Compile-tests the README examples alongside the regular doctests.
#[cfg(all(doctest, feature = "alloc"))]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;

pub use capacity::{decoded_len, encoded_len, CapacityError};
pub use check::{check_digit, check_symbol, CheckError, CHECK_ALPHABET, CHECK_DECODE_MAP};
#[cfg(feature = "alloc")]
pub use check::{decode_with_check, encode_with_check};
#[cfg(feature = "alloc")]
pub use decoder::{decode, try_decode};
pub use decoder::{
    decode_to_slice, sized_decode, try_decode_to_slice, DecodeError, DECODE_MAP, INVALID,
};
#[cfg(feature = "alloc")]
pub use encoder::encode;
pub use encoder::{encode_into, encode_to_slice, sized_encode, ALPHABET};

/// Namespaced re-exports of the core encoding and decoding functions, the
/// buffer-sizing helpers they need, and their error types.
///
/// Useful when glob-importing alongside another base32 crate; importing
/// `use ps_crockford32::crockford32::*;` brings in only the encode/decode
/// surface, not the alphabet or lookup-table constants.
pub mod crockford32 {
    pub use crate::capacity::{decoded_len, encoded_len, CapacityError};
    #[cfg(feature = "alloc")]
    pub use crate::decoder::{decode, try_decode};
    pub use crate::decoder::{decode_to_slice, sized_decode, try_decode_to_slice, DecodeError};
    #[cfg(feature = "alloc")]
    pub use crate::encoder::encode;
    pub use crate::encoder::{encode_into, encode_to_slice, sized_encode};
}
