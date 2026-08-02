//! Decoding functions and lookup tables.

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use crate::capacity::{decoded_len, CapacityError};

pub mod map;

pub use map::*;

/// Counts the bytes of `input` that carry a Crockford value.
#[inline]
fn count_symbols(input: &[u8]) -> usize {
    input
        .iter()
        .filter(|&&byte| DECODE_MAP[byte as usize] != INVALID)
        .count()
}

/// Writes the complete decoded bytes of `input` into `output`, stopping
/// when `output` is full, and returns the number of bytes written.
fn write_decoded(input: &[u8], output: &mut [u8]) -> usize {
    let mut index = 0;
    let mut buffer: u64 = 0;
    let mut bits: u32 = 0;

    for &byte in input {
        let value = DECODE_MAP[byte as usize];

        if value == INVALID {
            continue;
        }

        buffer = (buffer << 5) | u64::from(value);
        bits += 5;

        if bits >= 8 {
            bits -= 8;

            if index == output.len() {
                return index;
            }

            output[index] = ((buffer >> bits) & 0xff) as u8;
            index += 1;
        }
    }

    index
}

/// Decodes a Crockford Base32 encoded byte slice into binary data.
///
/// Characters outside the Crockford alphabet, including whitespace and
/// hyphens, are silently skipped. Decoding is case-insensitive and
/// accepts common misreadings: `O`/`o` as `0`, and `I`/`i`/`L`/`l` as `1`.
///
/// Because invalid bytes are skipped rather than rejected, unboundedly
/// many visually distinct inputs decode to the same output. Do not rely
/// on this function where input canonicity matters (for example, when
/// comparing encoded strings for equality or accepting untrusted
/// identifiers); use [`try_decode`], which accepts an input only if it is
/// the canonical encoding of the bytes it returns.
///
/// Trailing bits that do not form a complete byte are discarded. To
/// preserve them as a left-aligned partial byte (useful for round-tripping
/// [`sized_encode`](crate::sized_encode) output), use
/// [`sized_decode`] instead.
///
/// The returned vector is sized from the number of symbols in `input`
/// rather than from its length, so skipped bytes leave no excess capacity.
///
/// Requires the `alloc` feature.
///
/// # Examples
///
/// ```
/// use ps_crockford32::decode;
///
/// assert_eq!(decode(b"D1MG"), b"hi");
/// assert_eq!(decode(b"d1mg"), b"hi");
/// assert_eq!(decode(b"D1-MG"), b"hi");
/// ```
#[cfg(feature = "alloc")]
#[inline]
#[must_use]
pub fn decode(input: &[u8]) -> Vec<u8> {
    let mut output = alloc::vec![0u8; decoded_len(count_symbols(input))];

    let written = write_decoded(input, &mut output);

    debug_assert_eq!(written, output.len());

    output
}

/// Errors returned by [`try_decode`] and [`try_decode_to_slice`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodeError {
    /// A byte that is neither a Crockford symbol (or alias) nor a hyphen.
    InvalidByte {
        /// Byte offset of the rejected byte within the input.
        position: usize,
        /// The rejected byte.
        byte: u8,
    },
    /// The final symbol contributes bits that do not complete a byte, at
    /// least one of which is nonzero. The canonical encoding pads those
    /// bits with zeros, so `b"29"` is rejected while `b"28"` is accepted;
    /// both would otherwise denote `[0x12]`.
    NonZeroPadding {
        /// Byte offset of the offending symbol within the input.
        position: usize,
        /// The offending symbol.
        byte: u8,
    },
    /// The final symbol contributes no bits to the payload at all, so the
    /// input is longer than the canonical encoding of the bytes it
    /// denotes. A single symbol appended to a complete encoding, as in
    /// `b"280"` for `b"28"`, produces this error.
    ExcessSymbol {
        /// Byte offset of the offending symbol within the input.
        position: usize,
        /// The offending symbol.
        byte: u8,
    },
    /// The output slice cannot hold the decoded bytes. Only
    /// [`try_decode_to_slice`] returns this variant.
    Capacity(CapacityError),
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Self::InvalidByte { position, byte } => {
                write!(f, "invalid byte {byte:#04x} at position {position}")
            }
            Self::NonZeroPadding { position, byte } => write!(
                f,
                "byte {byte:#04x} at position {position} sets bits that decoding discards"
            ),
            Self::ExcessSymbol { position, byte } => write!(
                f,
                "byte {byte:#04x} at position {position} contributes no bits to the payload"
            ),
            Self::Capacity(error) => write!(f, "{error}"),
        }
    }
}

impl core::error::Error for DecodeError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Capacity(error) => Some(error),
            Self::InvalidByte { .. } | Self::NonZeroPadding { .. } | Self::ExcessSymbol { .. } => {
                None
            }
        }
    }
}

impl From<CapacityError> for DecodeError {
    fn from(error: CapacityError) -> Self {
        Self::Capacity(error)
    }
}

/// Number of bits that a partial block of `count` symbols contributes
/// without completing a byte.
///
/// Full eight-symbol blocks contribute exactly forty bits, so only the
/// final partial block matters.
#[inline]
const fn discarded_bits(count: u32) -> u32 {
    count * 5 % 8
}

/// Rejects a partial block whose bits are not those of a canonical
/// encoding.
///
/// `count` is the number of symbols in the final partial block, `buffer`
/// holds their accumulated bits, and `last` identifies the most recent
/// symbol, which is the one at fault in either failure mode.
const fn check_tail(buffer: u64, count: u32, last: Option<(usize, u8)>) -> Result<(), DecodeError> {
    let Some((position, byte)) = last else {
        return Ok(());
    };

    let discarded = discarded_bits(count);

    // A canonical encoding discards fewer than five bits, because a
    // symbol that contributes no whole bit of payload is never emitted.
    if discarded >= 5 {
        return Err(DecodeError::ExcessSymbol { position, byte });
    }

    if buffer & ((1u64 << discarded) - 1) != 0 {
        return Err(DecodeError::NonZeroPadding { position, byte });
    }

    Ok(())
}

/// Rejects an input that is not a canonical encoding, and returns the
/// number of symbols it carries.
///
/// Shared by [`try_decode`] and [`try_decode_to_slice`], so that the two
/// necessarily reach the same verdict and size their output identically.
fn validate(input: &[u8]) -> Result<usize, DecodeError> {
    let mut buffer: u64 = 0;
    let mut count: u32 = 0;
    let mut symbols: usize = 0;
    let mut last: Option<(usize, u8)> = None;

    for (position, &byte) in input.iter().enumerate() {
        if byte == b'-' {
            continue;
        }

        let value = DECODE_MAP[byte as usize];

        if value == INVALID {
            return Err(DecodeError::InvalidByte { position, byte });
        }

        buffer = (buffer << 5) | u64::from(value);
        symbols += 1;
        count += 1;
        last = Some((position, byte));

        if count == 8 {
            buffer = 0;
            count = 0;
            last = None;
        }
    }

    check_tail(buffer, count, last)?;

    Ok(symbols)
}

/// Decodes a Crockford Base32 encoded byte slice, rejecting invalid and
/// non-canonical input.
///
/// Unlike [`decode`], which silently skips every byte outside the
/// alphabet, this function accepts an input only if it is the canonical
/// encoding of the bytes it returns: `encode(&try_decode(input)?)` equals
/// `input` with hyphens removed, uppercased, and aliases resolved.
/// Establishing canonicity therefore needs no re-encode-and-compare step.
///
/// Three things are rejected:
///
/// - any byte that is neither a Crockford symbol (or alias) nor a hyphen,
///   including whitespace, with [`DecodeError::InvalidByte`];
/// - a final symbol whose discarded bits are not all zero, with
///   [`DecodeError::NonZeroPadding`];
/// - a final symbol that contributes no bits at all, with
///   [`DecodeError::ExcessSymbol`].
///
/// Hyphens are skipped wherever they appear, as sanctioned by Crockford's
/// specification, so they are the one difference this function tolerates.
/// Decoding remains case-insensitive and accepts the ambiguous aliases
/// `O`/`o` as `0` and `I`/`i`/`L`/`l` as `1`.
///
/// The returned vector is sized from the number of symbols in `input`
/// rather than from its length, so hyphens leave no excess capacity.
///
/// Requires the `alloc` feature.
///
/// # Errors
///
/// Returns the [`DecodeError`] variant described above for the first byte
/// at fault. Never returns [`DecodeError::Capacity`].
///
/// # Examples
///
/// ```
/// use ps_crockford32::{try_decode, DecodeError};
///
/// assert_eq!(try_decode(b"D1MG"), Ok(b"hi".to_vec()));
/// assert_eq!(try_decode(b"D1-MG"), Ok(b"hi".to_vec()));
/// assert_eq!(
///     try_decode(b"D1 MG"),
///     Err(DecodeError::InvalidByte { position: 2, byte: b' ' })
/// );
/// // `b"29"` would decode to the same byte as the canonical `b"28"`.
/// assert_eq!(
///     try_decode(b"29"),
///     Err(DecodeError::NonZeroPadding { position: 1, byte: b'9' })
/// );
/// ```
#[cfg(feature = "alloc")]
#[inline]
pub fn try_decode(input: &[u8]) -> Result<Vec<u8>, DecodeError> {
    let mut output = alloc::vec![0u8; decoded_len(validate(input)?)];

    let written = write_decoded(input, &mut output);

    debug_assert_eq!(written, output.len());

    Ok(output)
}

/// Decodes a Crockford Base32 encoded byte slice into `output`, rejecting
/// invalid and non-canonical input, and returns the number of bytes
/// written.
///
/// The allocation-free counterpart of [`try_decode`]: it applies the same
/// three rules and additionally reports an output slice too small to hold
/// the payload. The whole payload is written or none of it is; `output` is
/// left unmodified on every error.
///
/// # Errors
///
/// Returns the [`DecodeError`] variant that [`try_decode`] would return,
/// or [`DecodeError::Capacity`] if `output` cannot hold the decoded bytes.
///
/// # Examples
///
/// ```
/// use ps_crockford32::{try_decode_to_slice, CapacityError, DecodeError};
///
/// let mut buffer = [0u8; 8];
/// let written = try_decode_to_slice(b"D1-MG", &mut buffer)?;
///
/// assert_eq!(written, 2);
/// assert_eq!(&buffer[..written], b"hi");
///
/// assert_eq!(
///     try_decode_to_slice(b"D1MG", &mut buffer[..1]),
///     Err(DecodeError::Capacity(CapacityError { required: 2, available: 1 }))
/// );
/// # Ok::<(), DecodeError>(())
/// ```
#[inline]
pub fn try_decode_to_slice(input: &[u8], output: &mut [u8]) -> Result<usize, DecodeError> {
    // Validate first, so that a rejected input never leaves `output` partly
    // written. The symbol count from the same pass makes the required
    // capacity exact.
    let required = decoded_len(validate(input)?);
    let available = output.len();

    if required > available {
        return Err(CapacityError {
            required,
            available,
        }
        .into());
    }

    let written = write_decoded(input, output);

    debug_assert_eq!(written, required);

    Ok(written)
}

/// Decodes a Crockford Base32 encoded byte slice into a fixed-size array.
///
/// Characters outside the Crockford alphabet, including whitespace and
/// hyphens, are silently skipped. If the input decodes to more than `S`
/// bytes, the output is truncated; if to fewer, the remaining bytes are
/// zero. Unlike [`decode_to_slice`], an output too small to hold the whole
/// payload is not an error, because the fixed size is chosen by the caller
/// at compile time.
///
/// Unlike [`decode`], trailing bits that do not form a complete byte are
/// flushed as a left-aligned partial byte (high bits from the input, low
/// bits zero), so that [`sized_encode`](crate::sized_encode)
/// followed by [`sized_decode`] preserves the leading bits of the input.
///
/// Usable in `const` context.
///
/// # Examples
///
/// ```
/// use ps_crockford32::sized_decode;
///
/// const DECODED: [u8; 2] = sized_decode(b"D1MG");
/// assert_eq!(&DECODED, b"hi");
/// ```
#[inline]
#[must_use]
pub const fn sized_decode<const S: usize>(input: &[u8]) -> [u8; S] {
    let mut output = [0u8; S];

    if S == 0 {
        return output;
    }

    let mut index = 0;
    let mut buffer: u64 = 0;
    let mut bits: u32 = 0;
    let mut i = 0;

    while i < input.len() {
        let value = DECODE_MAP[input[i] as usize];

        if value != INVALID {
            buffer = (buffer << 5) | value as u64;
            bits += 5;

            if bits >= 8 {
                bits -= 8;
                output[index] = ((buffer >> bits) & 0xff) as u8;
                index += 1;

                if index == S {
                    return output;
                }
            }
        }

        i += 1;
    }

    if bits > 0 {
        output[index] = ((buffer << (8 - bits)) & 0xff) as u8;
    }

    output
}

/// Decodes `input` and writes the decoded bytes into `output`, returning
/// the number of bytes written.
///
/// Characters outside the Crockford alphabet, including whitespace and
/// hyphens, are silently skipped, as with [`decode`]. Use
/// [`try_decode_to_slice`] to reject them instead.
///
/// The whole payload is written or none of it is: if `output` cannot hold
/// every complete byte the input decodes to, `output` is left unmodified
/// and the required and available lengths are reported. Bytes of `output`
/// beyond the payload are left untouched rather than zeroed as
/// [`sized_decode`] does.
///
/// As with [`decode`], trailing bits that do not form a complete byte
/// are discarded. To preserve them as a left-aligned partial byte, use
/// [`sized_decode`] instead.
///
/// # Errors
///
/// Returns a [`CapacityError`] if `output` cannot hold the decoded bytes.
///
/// # Examples
///
/// ```
/// use ps_crockford32::{decode_to_slice, CapacityError};
///
/// let mut buffer = [0u8; 8];
/// let written = decode_to_slice(b"D1MG", &mut buffer)?;
///
/// assert_eq!(written, 2);
/// assert_eq!(&buffer[..written], b"hi");
///
/// assert_eq!(
///     decode_to_slice(b"D1MG", &mut buffer[..1]),
///     Err(CapacityError { required: 2, available: 1 })
/// );
/// # Ok::<(), CapacityError>(())
/// ```
#[inline]
pub fn decode_to_slice(input: &[u8], output: &mut [u8]) -> Result<usize, CapacityError> {
    let available = output.len();

    // `decoded_len(input.len())` bounds the payload from above, so the
    // symbols need counting only when that bound does not already fit.
    if decoded_len(input.len()) > available {
        let required = decoded_len(count_symbols(input));

        if required > available {
            return Err(CapacityError {
                required,
                available,
            });
        }
    }

    let written = write_decoded(input, output);

    debug_assert_eq!(written, decoded_len(count_symbols(input)));

    Ok(written)
}

const _: () = {
    assert!(DECODE_MAP[b'0' as usize] == 0);
    assert!(DECODE_MAP[b'Z' as usize] == 31);
    assert!(DECODE_MAP[b' ' as usize] == INVALID);

    // A canonical encoding never discards five or more bits, so exactly
    // the symbol counts below mark an excess trailing symbol.
    assert!(discarded_bits(1) == 5);
    assert!(discarded_bits(3) == 7);
    assert!(discarded_bits(6) == 6);
    assert!(discarded_bits(0) == 0);
    assert!(discarded_bits(2) == 2);
    assert!(discarded_bits(4) == 4);
    assert!(discarded_bits(5) == 1);
    assert!(discarded_bits(7) == 3);

    let decoded: [u8; 2] = sized_decode(b"D1MG");

    assert!(decoded[0] == b'h');
    assert!(decoded[1] == b'i');
};
