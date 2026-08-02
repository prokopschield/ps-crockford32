//! Buffer sizing helpers and the error reported when an output slice is
//! too small.

/// Error returned when an output slice cannot hold the full result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityError {
    /// Number of bytes the operation requires.
    pub required: usize,
    /// Number of bytes the caller provided.
    pub available: usize,
}

impl core::fmt::Display for CapacityError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let Self {
            required,
            available,
        } = self;

        write!(
            f,
            "output slice length {available} is below the required length {required}"
        )
    }
}

impl core::error::Error for CapacityError {}

/// Returns the number of symbols that the encoding of `bytes` input bytes
/// occupies.
///
/// Mathematically `(bytes * 8).div_ceil(5)`, computed per five-byte block
/// so that the intermediate product cannot overflow `usize`.
///
/// Usable in `const` context.
///
/// # Overflow
///
/// A result exceeding `usize::MAX` requires an input occupying more than
/// five eighths of the address space, and is therefore unreachable for a
/// slice that exists. Should it happen anyway, the arithmetic behaves as
/// everywhere else in Rust: `const` evaluation and builds with overflow
/// checks enabled reject it, and a release build wraps.
///
/// # Examples
///
/// ```
/// use ps_crockford32::encoded_len;
///
/// assert_eq!(encoded_len(0), 0);
/// assert_eq!(encoded_len(2), 4);
/// assert_eq!(encoded_len(5), 8);
/// ```
#[inline]
#[must_use]
pub const fn encoded_len(bytes: usize) -> usize {
    // `bytes % 5 * 8` is at most 32, so the ceiling division applies to a
    // product that cannot overflow, unlike `(bytes * 8).div_ceil(5)`.
    bytes / 5 * 8 + (bytes % 5 * 8).div_ceil(5)
}

/// Returns the number of whole bytes that `symbols` Crockford symbols
/// decode to.
///
/// Mathematically `symbols * 5 / 8`, computed per eight-symbol block so
/// that the intermediate product cannot overflow `usize`. Bytes that
/// decoding skips do not count as symbols, so `decoded_len(input.len())`
/// is an upper bound for any input of that length.
///
/// Usable in `const` context.
///
/// # Examples
///
/// ```
/// use ps_crockford32::decoded_len;
///
/// assert_eq!(decoded_len(0), 0);
/// assert_eq!(decoded_len(4), 2);
/// assert_eq!(decoded_len(8), 5);
/// ```
#[inline]
#[must_use]
pub const fn decoded_len(symbols: usize) -> usize {
    symbols / 8 * 5 + symbols % 8 * 5 / 8
}
