//! Crockford Base32 check-symbol support.
//!
//! The check symbol is computed as `n mod 37`, where `n` is the payload
//! interpreted as a big-endian integer. See the "Check symbols" section
//! of the crate-level documentation for the full discussion, including
//! interoperability caveats.

#[cfg(feature = "alloc")]
use alloc::{string::String, vec::Vec};

use crate::decoder::{DecodeError, DECODE_MAP, INVALID};

/// The 37-symbol check alphabet: the 32-symbol
/// [`ALPHABET`](crate::ALPHABET) followed by the five check-only symbols
/// `*`, `~`, `$`, `=`, and `U` at indices 32-36.
pub const CHECK_ALPHABET: [u8; 37] = {
    let mut arr = [0u8; 37];
    let mut i = 0;

    while i < 32 {
        arr[i] = crate::encoder::ALPHABET[i];
        i += 1;
    }

    arr[32] = b'*';
    arr[33] = b'~';
    arr[34] = b'$';
    arr[35] = b'=';
    arr[36] = b'U';

    arr
};

/// Lookup table mapping each byte to its check-alphabet value (`0..37`),
/// or to [`INVALID`] for unrecognized bytes.
///
/// Extends [`DECODE_MAP`] with values 32-36 for `*`, `~`, `$`, `=`, and
/// `U`/`u`. Because the base table is inherited, the ambiguous-glyph
/// aliases apply to check symbols as well: `O`/`o` are accepted as check
/// digit 0, and `I`/`i`/`L`/`l` as check digit 1. The one divergence is
/// `U`/`u`: aliases for `V` (value 27) in [`DECODE_MAP`], they are
/// overridden here to check value 36.
///
/// This table is for the single trailing check symbol only. Five of its
/// entries exceed the five bits a payload symbol carries, so decoding a
/// whole string through it yields values the Crockford alphabet cannot
/// represent; use [`DECODE_MAP`] for payload bytes.
pub const CHECK_DECODE_MAP: [u8; 256] = build_check_decode_map();

const fn build_check_decode_map() -> [u8; 256] {
    let mut map = DECODE_MAP;

    map[b'*' as usize] = 32;
    map[b'~' as usize] = 33;
    map[b'$' as usize] = 34;
    map[b'=' as usize] = 35;
    map[b'U' as usize] = 36;
    map[b'u' as usize] = 36;

    map
}

/// Computes the Crockford check digit for `input`.
///
/// The result is `n mod 37`, where `n` is the big-endian integer
/// represented by `input`. The returned value is in `0..37`.
///
/// # Examples
///
/// ```
/// use ps_crockford32::check_digit;
///
/// assert_eq!(check_digit(b"\x12"), 18);
/// assert_eq!(check_digit(&[]), 0);
/// ```
#[must_use]
pub const fn check_digit(input: &[u8]) -> u8 {
    let mut remainder: u16 = 0;
    let mut i = 0;

    while i < input.len() {
        remainder = (remainder * 256 + input[i] as u16) % 37;
        i += 1;
    }

    // `remainder` is reduced modulo 37 on every step, so it is always
    // in `0..37` and fits in a `u8`.
    #[allow(clippy::cast_possible_truncation)]
    {
        remainder as u8
    }
}

/// Returns the check symbol for `input`.
///
/// # Examples
///
/// ```
/// use ps_crockford32::check_symbol;
///
/// assert_eq!(check_symbol(b"\x12"), b'J');
/// ```
#[must_use]
pub const fn check_symbol(input: &[u8]) -> u8 {
    CHECK_ALPHABET[check_digit(input) as usize]
}

/// Encodes `input` into a Crockford Base32 string and appends the check
/// symbol.
///
/// Requires the `alloc` feature.
///
/// # Examples
///
/// ```
/// use ps_crockford32::encode_with_check;
///
/// assert_eq!(encode_with_check(b"\x12"), "28J");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
pub fn encode_with_check(input: &[u8]) -> String {
    let mut output = crate::encode(input);

    output.push(char::from(check_symbol(input)));

    output
}

/// Errors returned by [`decode_with_check`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CheckError {
    /// `input` contains no non-skipped character to serve as the check
    /// symbol.
    Missing,
    /// The trailing symbol is not part of the 37-symbol check alphabet.
    InvalidSymbol,
    /// The body preceding the check symbol is not a canonical Crockford
    /// encoding.
    Body(DecodeError),
    /// The check digit derived from the decoded payload does not match
    /// the digit supplied by the input's trailing check symbol.
    Mismatch {
        /// The check digit computed from the decoded payload.
        expected: u8,
        /// The check digit read from the input's trailing symbol.
        actual: u8,
    },
}

impl core::fmt::Display for CheckError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Missing => f.write_str("input contains no check symbol"),
            Self::InvalidSymbol => f.write_str("trailing symbol is not part of the check alphabet"),
            Self::Body(error) => write!(f, "invalid body: {error}"),
            Self::Mismatch { expected, actual } => write!(
                f,
                "check digit mismatch: payload requires {expected}, input supplied {actual}"
            ),
        }
    }
}

impl core::error::Error for CheckError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Body(error) => Some(error),
            Self::Missing | Self::InvalidSymbol | Self::Mismatch { .. } => None,
        }
    }
}

impl From<DecodeError> for CheckError {
    fn from(error: DecodeError) -> Self {
        Self::Body(error)
    }
}

/// Decodes a Crockford Base32 input that ends with a check symbol.
///
/// The check symbol is the last character of `input` that is neither
/// ASCII whitespace nor a hyphen, so trailing line endings and separators
/// do not obscure it. Everything before it is the body, decoded with
/// [`try_decode`](crate::try_decode) and then validated against the check
/// digit.
///
/// # Limits of the check
///
/// Alterations that preserve the number of symbols are caught in full:
/// because the body must be a canonical encoding, every substitution of a
/// single symbol and every transposition of two adjacent symbols is
/// rejected, either as a non-canonical body or as a check mismatch.
///
/// Alterations that change the number of symbols are caught only with
/// probability 36/37, as for any check digit modulo 37. In particular,
/// passing plain [`encode`](crate::encode) output to this function is not
/// reliably an error: the final payload symbol is taken as the check
/// symbol, and roughly one such string in thirty-seven is accepted,
/// returning a silently truncated payload. Distinguish checked strings
/// from unchecked ones by their symbol count, not by whether this function
/// rejects them.
///
/// Hyphens remain invisible to the check wherever they appear, because
/// Crockford's specification designates them insignificant separators and
/// [`try_decode`](crate::try_decode) accordingly skips them, and trailing
/// ASCII whitespace is ignored when locating the check symbol.
///
/// A `U` in the body is decoded as the alias for `V`, but a trailing `U`
/// is always read as the check symbol for value 36, never as that alias.
///
/// Note the asymmetry at the end of the input: within the body, a byte
/// outside the alphabet is reported as [`CheckError::Body`], but a
/// non-alphabet byte in the trailing position is taken as the check symbol
/// and rejected with [`CheckError::InvalidSymbol`].
///
/// Requires the `alloc` feature.
///
/// # Errors
///
/// - [`CheckError::Missing`] if `input` is empty or contains only skipped
///   characters.
/// - [`CheckError::InvalidSymbol`] if the trailing symbol is not part of
///   the check alphabet.
/// - [`CheckError::Body`] if the body is not a canonical Crockford
///   encoding.
/// - [`CheckError::Mismatch`] if the check digit does not match the
///   decoded payload.
///
/// # Examples
///
/// ```
/// use ps_crockford32::{decode_with_check, encode, CheckError, DecodeError};
///
/// assert_eq!(decode_with_check(b"28J"), Ok(vec![0x12]));
/// assert!(matches!(decode_with_check(b"28X"), Err(CheckError::Mismatch { .. })));
/// assert_eq!(decode_with_check(b""), Err(CheckError::Missing));
///
/// // A mistyped final body symbol changes bits the payload discards, so
/// // the body is no longer canonical and the substitution is caught.
/// assert_eq!(
///     decode_with_check(b"29J"),
///     Err(CheckError::Body(DecodeError::NonZeroPadding { position: 1, byte: b'9' }))
/// );
///
/// // A string that carries no check symbol at all, however, is only
/// // rejected when dropping its last symbol leaves a body that is not
/// // canonical or whose check disagrees. Here neither holds, so the
/// // trailing `0` passes as the check digit of the body `"0000"`.
/// assert_eq!(encode(&[0, 0, 0]), "00000");
/// assert_eq!(decode_with_check(b"00000"), Ok(vec![0, 0]));
/// ```
#[cfg(feature = "alloc")]
pub fn decode_with_check(input: &[u8]) -> Result<Vec<u8>, CheckError> {
    let check_pos = input
        .iter()
        .rposition(|&b| !is_skip(b))
        .ok_or(CheckError::Missing)?;

    let check_val = CHECK_DECODE_MAP[input[check_pos] as usize];

    if check_val > 36 {
        return Err(CheckError::InvalidSymbol);
    }

    let decoded = crate::try_decode(&input[..check_pos])?;
    let expected = check_digit(&decoded);

    if check_val == expected {
        Ok(decoded)
    } else {
        Err(CheckError::Mismatch {
            expected,
            actual: check_val,
        })
    }
}

#[cfg(feature = "alloc")]
#[inline]
const fn is_skip(byte: u8) -> bool {
    byte.is_ascii_whitespace() || byte == b'-'
}

const _: () = {
    // `INVALID` exceeds the valid check range, so the `check_val > 36`
    // rejection covers unrecognized bytes as well.
    assert!(INVALID > 36);

    assert!(CHECK_ALPHABET[36] == b'U');
    assert!(CHECK_DECODE_MAP[b'*' as usize] == 32);
    assert!(CHECK_DECODE_MAP[b'u' as usize] == 36);
    assert!(check_digit(b"\x12") == 18);
    assert!(check_symbol(b"\x12") == b'J');
};
