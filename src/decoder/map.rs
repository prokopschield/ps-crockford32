//! Lookup table for decoding Crockford Base32 characters.

/// Sentinel marking bytes that are not part of the Crockford alphabet.
pub const INVALID: u8 = 0xff;

/// Lookup table mapping each byte to its five-bit Crockford value, or to
/// [`INVALID`] for unrecognized bytes.
///
/// The canonical alphabet is `0`-`9`, `A`-`H`, `J`, `K`, `M`, `N`, `P`-`T`,
/// and `V`-`Z`. Lowercase letters are accepted as aliases for their
/// uppercase counterparts, as are the visually ambiguous glyphs `O`/`o`
/// (treated as `0`) and `I`/`i`/`L`/`l` (treated as `1`).
pub const DECODE_MAP: [u8; 256] = build_decode_map();

const fn build_decode_map() -> [u8; 256] {
    let mut map = [INVALID; 256];

    let mut i: u8 = 0;

    while i < 32 {
        let upper = crate::encoder::ALPHABET[i as usize];
        map[upper as usize] = i;

        // Indices 10..32 are letters; the ASCII offset from uppercase to
        // lowercase is 32, so `upper + 32` is the lowercase counterpart.
        if i >= 10 {
            map[(upper + 32) as usize] = i;
        }

        i += 1;
    }

    map[b'O' as usize] = 0;
    map[b'o' as usize] = 0;
    map[b'I' as usize] = 1;
    map[b'i' as usize] = 1;
    map[b'L' as usize] = 1;
    map[b'l' as usize] = 1;

    map
}
