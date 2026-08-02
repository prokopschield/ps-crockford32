//! Encoding functions and the Crockford alphabet.

#[cfg(feature = "alloc")]
use alloc::string::String;

use crate::capacity::{encoded_len, CapacityError};

/// The Crockford Base32 alphabet: digits 0-9 followed by the twenty-two
/// letters A-Z excluding I, L, O, and U.
pub const ALPHABET: [u8; 32] = *b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Maps the low five bits of `value` to a Crockford alphabet byte.
#[inline]
const fn symbol(value: u64) -> u8 {
    ALPHABET[(value & 0x1f) as usize]
}

/// Encodes a full five-byte block into eight symbols.
#[inline]
fn encode_block(chunk: &[u8]) -> [u8; 8] {
    let mut buffer: u64 = 0;

    for &byte in chunk {
        buffer = (buffer << 8) | u64::from(byte);
    }

    let mut symbols = [0u8; 8];
    let mut shift = 40;

    for slot in &mut symbols {
        shift -= 5;
        *slot = symbol(buffer >> shift);
    }

    symbols
}

/// Writes the complete encoding of `input` into `output` and returns the
/// number of bytes written.
///
/// `output` must hold at least [`encoded_len(input.len())`](encoded_len)
/// bytes, which every caller checks before calling; the `debug_assert`
/// below catches a caller that stops doing so.
fn write_encoded(input: &[u8], output: &mut [u8]) -> usize {
    debug_assert!(output.len() >= encoded_len(input.len()));

    let blocks = input.len() / 5;

    for (chunk, slots) in input.chunks_exact(5).zip(output.chunks_exact_mut(8)) {
        slots.copy_from_slice(&encode_block(chunk));
    }

    let mut index = blocks * 8;
    let mut buffer: u64 = 0;
    let mut bits: u32 = 0;

    for &byte in &input[blocks * 5..] {
        buffer = (buffer << 8) | u64::from(byte);
        bits += 8;

        while bits >= 5 {
            bits -= 5;
            output[index] = symbol(buffer >> bits);
            index += 1;
        }
    }

    if bits > 0 {
        output[index] = symbol(buffer << (5 - bits));
        index += 1;
    }

    index
}

/// Encodes `input` into a fixed-size Crockford Base32 byte array.
///
/// If the full encoding does not fit, it is truncated to `S` bytes; if it
/// is shorter than `S` bytes, the remaining bytes are filled with `'0'`.
/// Unlike [`encode_to_slice`], an output too small to hold the whole
/// encoding is not an error, because the fixed size is chosen by the
/// caller at compile time.
///
/// When the bit length of `input` is not a multiple of five, the final
/// symbol is padded with zero bits on the right (see [`encode`]).
///
/// Usable in `const` context.
///
/// # Examples
///
/// ```
/// use ps_crockford32::sized_encode;
///
/// const ENCODED: [u8; 4] = sized_encode(b"hi");
/// assert_eq!(&ENCODED, b"D1MG");
/// ```
#[inline]
#[must_use]
pub const fn sized_encode<const S: usize>(input: &[u8]) -> [u8; S] {
    let mut output = [b'0'; S];

    if S == 0 {
        return output;
    }

    let mut index = 0;
    let mut buffer: u64 = 0;
    let mut bits: u32 = 0;
    let mut i = 0;

    while i < input.len() {
        buffer = (buffer << 8) | input[i] as u64;
        bits += 8;

        while bits >= 5 {
            bits -= 5;
            output[index] = symbol(buffer >> bits);
            index += 1;

            if index == S {
                return output;
            }
        }

        i += 1;
    }

    if bits > 0 {
        output[index] = symbol(buffer << (5 - bits));
    }

    output
}

/// Encodes `input` and writes the symbols into `output`, returning the
/// number of bytes written.
///
/// The whole encoding is written or none of it is: if `output` cannot hold
/// [`encoded_len(input.len())`](encoded_len) bytes, `output` is left
/// unmodified and the required and available lengths are reported. On
/// success the return value is always `encoded_len(input.len())`. Bytes of
/// `output` beyond the encoding are left untouched rather than filled with
/// `'0'` as [`sized_encode`] does.
///
/// When the bit length of `input` is not a multiple of five, the final
/// symbol is padded with zero bits on the right (see [`encode`]).
///
/// # Errors
///
/// Returns a [`CapacityError`] if `output` is shorter than
/// `encoded_len(input.len())`.
///
/// # Examples
///
/// ```
/// use ps_crockford32::{encode_to_slice, CapacityError};
///
/// let mut buffer = [0u8; 8];
/// let written = encode_to_slice(b"hi", &mut buffer)?;
///
/// assert_eq!(written, 4);
/// assert_eq!(&buffer[..written], b"D1MG");
///
/// assert_eq!(
///     encode_to_slice(b"hi", &mut buffer[..3]),
///     Err(CapacityError { required: 4, available: 3 })
/// );
/// # Ok::<(), CapacityError>(())
/// ```
#[inline]
pub fn encode_to_slice(input: &[u8], output: &mut [u8]) -> Result<usize, CapacityError> {
    let required = encoded_len(input.len());
    let available = output.len();

    if required > available {
        return Err(CapacityError {
            required,
            available,
        });
    }

    let written = write_encoded(input, output);

    debug_assert_eq!(written, required);

    Ok(written)
}

/// Encodes `input` into a Crockford Base32 string.
///
/// This is a byte-oriented encoding: when the bit length of `input` is
/// not a multiple of five, the final symbol is padded with zero bits on
/// the right. Crockford's specification instead encodes integers,
/// zero-extending on the left; the two schemes produce different strings
/// (`encode(&[0x12])` is `"28"`, while the integer 18 encodes to `"J"`
/// under the specification), so output of this crate is not
/// interchangeable with integer-based Crockford implementations.
///
/// Requires the `alloc` feature.
///
/// # Examples
///
/// ```
/// use ps_crockford32::encode;
///
/// assert_eq!(encode(b"hi"), "D1MG");
/// ```
#[cfg(feature = "alloc")]
#[must_use]
// Every symbol comes from the ASCII `ALPHABET`, so the buffer is always
// valid UTF-8 and the `expect` cannot panic.
#[allow(clippy::expect_used, clippy::missing_panics_doc)]
pub fn encode(input: &[u8]) -> String {
    let mut output = alloc::vec![0u8; encoded_len(input.len())];

    let written = write_encoded(input, &mut output);

    debug_assert_eq!(written, output.len());

    String::from_utf8(output).expect("Crockford symbols are ASCII")
}

const _: () = {
    assert!(ALPHABET[0] == b'0');
    assert!(symbol(0) == b'0');
    assert!(symbol(31) == b'Z');

    let encoded: [u8; 4] = sized_encode(b"hi");

    assert!(encoded[0] == b'D');
    assert!(encoded[1] == b'1');
    assert!(encoded[2] == b'M');
    assert!(encoded[3] == b'G');
};

/// Encodes `input` and writes the result into `sink`.
///
/// When the bit length of `input` is not a multiple of five, the final
/// symbol is padded with zero bits on the right (see [`encode`]).
///
/// # Errors
///
/// Propagates any error returned by the sink's
/// [`write_str`](core::fmt::Write::write_str) implementation.
///
/// # Examples
///
/// ```
/// use ps_crockford32::encode_into;
///
/// let mut buffer = String::new();
/// encode_into(b"hi", &mut buffer).unwrap();
/// assert_eq!(buffer, "D1MG");
/// ```
#[inline]
// The `expect` below converts a block of ASCII `ALPHABET` symbols, so it
// cannot panic.
#[allow(clippy::missing_panics_doc)]
pub fn encode_into<W>(input: &[u8], mut sink: W) -> core::fmt::Result
where
    W: core::fmt::Write,
{
    let mut chunks = input.chunks_exact(5);

    for chunk in &mut chunks {
        let symbols = encode_block(chunk);

        // Every symbol comes from the ASCII `ALPHABET`, so the block is
        // always valid UTF-8 and this cannot panic.
        #[allow(clippy::expect_used)]
        sink.write_str(core::str::from_utf8(&symbols).expect("Crockford symbols are ASCII"))?;
    }

    let mut buffer: u64 = 0;
    let mut bits: u32 = 0;

    for &byte in chunks.remainder() {
        buffer = (buffer << 8) | u64::from(byte);
        bits += 8;

        while bits >= 5 {
            bits -= 5;
            sink.write_char(char::from(symbol(buffer >> bits)))?;
        }
    }

    if bits > 0 {
        sink.write_char(char::from(symbol(buffer << (5 - bits))))?;
    }

    Ok(())
}
