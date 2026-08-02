//! Tests restricted to the API that survives `--no-default-features`.
//!
//! Nothing here may touch `alloc`, so that this suite compiles and runs in
//! the same configuration a `no_std` dependent gets.

// The generator below deliberately keeps only the low bits of its state.
#![allow(clippy::cast_possible_truncation, clippy::expect_used)]

use core::fmt::Write;

use crate::{
    check_digit, check_symbol, decode_to_slice, decoded_len, encode_into, encode_to_slice,
    encoded_len, sized_decode, sized_encode, try_decode_to_slice, CapacityError, DecodeError,
    ALPHABET, CHECK_ALPHABET, CHECK_DECODE_MAP, DECODE_MAP, INVALID,
};

/// A [`Write`] sink over a fixed slice, so that [`encode_into`] can be
/// exercised without a [`String`](alloc::string::String).
struct SliceSink<'a> {
    buffer: &'a mut [u8],
    len: usize,
}

impl<'a> SliceSink<'a> {
    fn new(buffer: &'a mut [u8]) -> Self {
        Self { buffer, len: 0 }
    }

    fn written(&self) -> &[u8] {
        &self.buffer[..self.len]
    }
}

impl Write for SliceSink<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();

        if end > self.buffer.len() {
            return Err(core::fmt::Error);
        }

        self.buffer[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;

        Ok(())
    }
}

/// Deterministic pseudo-random bytes, so the sweeps below need no `rand`.
struct Rng(u64);

impl Rng {
    fn next_byte(&mut self) -> u8 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;

        (self.0 >> 24) as u8
    }

    fn fill(&mut self, buffer: &mut [u8]) {
        for slot in buffer {
            *slot = self.next_byte();
        }
    }
}

#[test]
fn encoded_len_matches_the_bytes_written() {
    let mut input = [0u8; 64];
    let mut output = [0u8; 128];
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);

    rng.fill(&mut input);

    for len in 0..input.len() {
        let written = encode_to_slice(&input[..len], &mut output).expect("output is large enough");

        assert_eq!(written, encoded_len(len), "len={len}");
    }
}

#[test]
fn decoded_len_matches_the_bytes_written() {
    let mut input = [0u8; 40];
    let mut encoded = [0u8; 64];
    let mut decoded = [0u8; 64];
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);

    rng.fill(&mut input);

    for len in 0..input.len() {
        let symbols = encode_to_slice(&input[..len], &mut encoded).expect("output is large enough");
        let written =
            decode_to_slice(&encoded[..symbols], &mut decoded).expect("output is large enough");

        assert_eq!(written, decoded_len(symbols), "len={len}");
        assert_eq!(&decoded[..written], &input[..written], "len={len}");
    }
}

#[test]
fn encoded_len_is_a_ceiling_division() {
    assert_eq!(encoded_len(0), 0);
    assert_eq!(encoded_len(1), 2);
    assert_eq!(encoded_len(2), 4);
    assert_eq!(encoded_len(3), 5);
    assert_eq!(encoded_len(4), 7);
    assert_eq!(encoded_len(5), 8);
    assert_eq!(encoded_len(6), 10);

    for bytes in 0..1000usize {
        assert_eq!(encoded_len(bytes), (bytes * 8).div_ceil(5), "bytes={bytes}");
    }
}

#[test]
fn decoded_len_is_a_floor_division() {
    assert_eq!(decoded_len(0), 0);
    assert_eq!(decoded_len(1), 0);
    assert_eq!(decoded_len(2), 1);
    assert_eq!(decoded_len(8), 5);

    for symbols in 0..1000usize {
        assert_eq!(decoded_len(symbols), symbols * 5 / 8, "symbols={symbols}");
    }
}

#[test]
fn length_helpers_survive_inputs_that_overflow_a_naive_product() {
    // `bytes * 8` and `symbols * 5` overflow `usize` here, so the
    // per-block formulas are checked against widened arithmetic.
    let bytes = usize::MAX / 8 * 5;
    let expected = (u128::try_from(bytes).expect("usize fits in u128") * 8).div_ceil(5);

    assert_eq!(
        u128::try_from(encoded_len(bytes)).expect("usize fits in u128"),
        expected
    );

    for symbols in [usize::MAX, usize::MAX - 1, usize::MAX / 5 * 4] {
        let expected = u128::try_from(symbols).expect("usize fits in u128") * 5 / 8;

        assert_eq!(
            u128::try_from(decoded_len(symbols)).expect("usize fits in u128"),
            expected
        );
    }
}

#[test]
fn encode_to_slice_reports_capacity_instead_of_truncating() {
    let mut output = [0xEEu8; 8];

    assert_eq!(
        encode_to_slice(b"hi", &mut output[..3]),
        Err(CapacityError {
            required: 4,
            available: 3
        })
    );
    assert_eq!(output, [0xEE; 8], "output must be left untouched");
}

#[test]
fn encode_to_slice_rejects_a_whole_block_short_of_the_requirement() {
    let input = [0u8; 6];
    let mut output = [0xEEu8; 16];

    // Ten symbols are required; eight is an exact block boundary, which the
    // block loop alone would fill without noticing the missing tail.
    assert_eq!(
        encode_to_slice(&input, &mut output[..8]),
        Err(CapacityError {
            required: 10,
            available: 8
        })
    );
    assert_eq!(output, [0xEE; 16], "output must be left untouched");
}

#[test]
fn encode_to_slice_zero_capacity_rejects_nonempty_input() {
    let mut output = [0u8; 0];

    assert_eq!(
        encode_to_slice(b"h", &mut output),
        Err(CapacityError {
            required: 2,
            available: 0
        })
    );
    assert_eq!(encode_to_slice(b"", &mut output), Ok(0));
}

#[test]
fn encode_to_slice_leaves_excess_capacity_untouched() {
    let mut output = [0xEEu8; 8];
    let written = encode_to_slice(b"hi", &mut output).expect("output is large enough");

    assert_eq!(&output[..written], b"D1MG");
    assert_eq!(&output[written..], &[0xEE; 4]);
}

#[test]
fn decode_to_slice_reports_capacity_instead_of_truncating() {
    let mut output = [0xEEu8; 8];

    assert_eq!(
        decode_to_slice(b"D1MG", &mut output[..1]),
        Err(CapacityError {
            required: 2,
            available: 1
        })
    );
    assert_eq!(output, [0xEE; 8], "output must be left untouched");
}

#[test]
fn decode_to_slice_sizes_from_symbols_not_from_input_length() {
    let mut output = [0u8; 2];

    // Sixteen bytes of input, but only four of them are symbols.
    let written = decode_to_slice(b"-D-1-M-G-", &mut output).expect("output is large enough");

    assert_eq!(written, 2);
    assert_eq!(&output, b"hi");
}

#[test]
fn decode_to_slice_ignores_trailing_partial_bits() {
    let mut output = [0xEEu8; 4];

    // Three symbols carry fifteen bits, of which only the first eight form
    // a whole byte.
    let written = decode_to_slice(b"D1M", &mut output).expect("output is large enough");

    assert_eq!(written, 1);
    assert_eq!(&output, &[b'h', 0xEE, 0xEE, 0xEE]);
}

#[test]
fn decode_to_slice_skips_invalid_bytes() {
    let mut output = [0u8; 2];
    let written = decode_to_slice(b" d1 mg\n", &mut output).expect("output is large enough");

    assert_eq!(written, 2);
    assert_eq!(&output, b"hi");
}

#[test]
fn try_decode_to_slice_matches_decode_to_slice_on_canonical_input() {
    let mut input = [0u8; 40];
    let mut encoded = [0u8; 64];
    let mut strict = [0u8; 64];
    let mut lenient = [0u8; 64];
    let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);

    rng.fill(&mut input);

    for len in 0..input.len() {
        let symbols = encode_to_slice(&input[..len], &mut encoded).expect("output is large enough");

        let written = try_decode_to_slice(&encoded[..symbols], &mut strict)
            .unwrap_or_else(|error| panic!("len={len} rejected: {error}"));

        assert_eq!(
            Ok(written),
            decode_to_slice(&encoded[..symbols], &mut lenient)
        );
        assert_eq!(strict[..written], lenient[..written], "len={len}");
        assert_eq!(&strict[..len], &input[..len], "len={len}");
    }
}

#[test]
fn try_decode_to_slice_skips_hyphens_only() {
    let mut output = [0u8; 2];

    assert_eq!(try_decode_to_slice(b"-D1--MG-", &mut output), Ok(2));
    assert_eq!(&output, b"hi");
    assert_eq!(
        try_decode_to_slice(b"D1 MG", &mut output),
        Err(DecodeError::InvalidByte {
            position: 2,
            byte: b' '
        })
    );
}

#[test]
fn try_decode_to_slice_rejects_nonzero_discarded_bits() {
    let mut output = [0xEEu8; 8];

    assert_eq!(
        try_decode_to_slice(b"29", &mut output),
        Err(DecodeError::NonZeroPadding {
            position: 1,
            byte: b'9'
        })
    );
    assert_eq!(output, [0xEE; 8], "output must be left untouched");

    assert_eq!(try_decode_to_slice(b"28", &mut output), Ok(1));
    assert_eq!(output[0], 0x12);
}

#[test]
fn try_decode_to_slice_rejects_a_symbol_carrying_no_payload() {
    let mut output = [0u8; 8];

    for input in [&b"0"[..], b"D1M", b"D1MGR0"] {
        assert!(
            matches!(
                try_decode_to_slice(input, &mut output),
                Err(DecodeError::ExcessSymbol { .. })
            ),
            "input={input:?}"
        );
    }
}

#[test]
fn try_decode_to_slice_reports_capacity_last() {
    let mut output = [0xEEu8; 8];

    assert_eq!(
        try_decode_to_slice(b"D1MG", &mut output[..1]),
        Err(DecodeError::Capacity(CapacityError {
            required: 2,
            available: 1
        }))
    );
    assert_eq!(output, [0xEE; 8], "output must be left untouched");

    // A malformed input is rejected on its own merits even when the output
    // is also too small.
    assert_eq!(
        try_decode_to_slice(b"D1 MG", &mut output[..0]),
        Err(DecodeError::InvalidByte {
            position: 2,
            byte: b' '
        })
    );
}

#[test]
fn try_decode_to_slice_sizes_from_symbols_not_from_input_length() {
    let mut output = [0u8; 2];

    assert_eq!(try_decode_to_slice(b"-D-1-M-G-", &mut output), Ok(2));
    assert_eq!(&output, b"hi");
}

#[test]
fn try_decode_to_slice_empty_input_writes_nothing() {
    let mut output = [0xEEu8; 4];

    assert_eq!(try_decode_to_slice(b"", &mut output), Ok(0));
    assert_eq!(try_decode_to_slice(b"----", &mut output), Ok(0));
    assert_eq!(output, [0xEE; 4]);
}

#[test]
fn every_accepted_input_is_the_encoding_of_what_it_decodes_to() {
    let mut input = [0u8; 32];
    let mut payload = [0u8; 32];
    let mut re_encoded = [0u8; 64];
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);

    // Arbitrary symbol strings, so the accepted ones are whatever the
    // canonicity rules let through rather than encodings by construction.
    for len in 0..input.len() {
        for _ in 0..64 {
            for slot in &mut input[..len] {
                *slot = ALPHABET[(rng.next_byte() & 0x1f) as usize];
            }

            let Ok(written) = try_decode_to_slice(&input[..len], &mut payload) else {
                continue;
            };

            let symbols =
                encode_to_slice(&payload[..written], &mut re_encoded).expect("buffer is large");

            assert_eq!(&re_encoded[..symbols], &input[..len], "len={len}");
        }
    }
}

#[test]
fn appending_a_symbol_is_accepted_only_when_it_lengthens_the_payload() {
    let mut input = [0u8; 16];
    let mut encoded = [0u8; 32];
    let mut output = [0u8; 32];
    let mut rng = Rng(0x0F1E_2D3C_4B5A_6978);

    rng.fill(&mut input);

    for len in 0..input.len() {
        let symbols = encode_to_slice(&input[..len], &mut encoded).expect("output is large enough");

        assert_eq!(
            try_decode_to_slice(&encoded[..symbols], &mut output),
            Ok(len),
            "len={len}"
        );

        // A canonical encoding of `len` bytes discards `symbols * 5 -
        // len * 8` bits, and one extra symbol adds five more. The sum is
        // what the appended symbol must leave zero, so it exceeds a byte
        // (and lengthens the payload) only for the last two residues.
        let discarded = symbols * 5 - len * 8;

        for &symbol in &ALPHABET {
            encoded[symbols] = symbol;

            let value = DECODE_MAP[symbol as usize];

            let expected = match discarded {
                3 => Ok(len + 1),
                4 if value % 2 == 0 => Ok(len + 1),
                4 => Err(DecodeError::NonZeroPadding {
                    position: symbols,
                    byte: symbol,
                }),
                _ => Err(DecodeError::ExcessSymbol {
                    position: symbols,
                    byte: symbol,
                }),
            };

            assert_eq!(
                try_decode_to_slice(&encoded[..=symbols], &mut output),
                expected,
                "len={len} symbol={symbol}"
            );
        }
    }
}

#[test]
fn strict_decoding_rejects_every_byte_outside_the_alphabet() {
    let mut output = [0u8; 8];

    for byte in 0..=u8::MAX {
        let input = [b'D', b'1', byte, b'M', b'G'];
        let accepted = byte == b'-' || DECODE_MAP[byte as usize] != INVALID;

        match try_decode_to_slice(&input, &mut output) {
            Err(DecodeError::InvalidByte {
                position,
                byte: got,
            }) => {
                assert!(!accepted, "byte={byte:#04x} wrongly rejected");
                assert_eq!(position, 2);
                assert_eq!(got, byte);
            }
            other => assert!(accepted, "byte={byte:#04x} -> {other:?}"),
        }
    }
}

#[test]
fn decode_errors_implement_display_and_error() {
    use core::error::Error;

    let invalid = DecodeError::InvalidByte {
        position: 2,
        byte: b' ',
    };
    let capacity = DecodeError::Capacity(CapacityError {
        required: 2,
        available: 1,
    });

    assert!(invalid.source().is_none());
    assert!(capacity.source().is_some());

    let mut buffer = [0u8; 128];

    for error in [
        invalid,
        DecodeError::NonZeroPadding {
            position: 1,
            byte: b'9',
        },
        DecodeError::ExcessSymbol {
            position: 0,
            byte: b'0',
        },
        capacity,
    ] {
        let mut sink = SliceSink::new(&mut buffer);

        write!(sink, "{error}").expect("the sink is large enough");

        assert!(!sink.written().is_empty());
    }
}

#[test]
fn capacity_error_implements_display_and_error() {
    use core::error::Error;

    let error = CapacityError {
        required: 4,
        available: 3,
    };

    assert!(error.source().is_none());

    let mut buffer = [0u8; 128];
    let mut sink = SliceSink::new(&mut buffer);

    write!(sink, "{error}").expect("the sink is large enough");

    assert_eq!(
        sink.written(),
        b"output slice length 3 is below the required length 4"
    );
}

#[test]
fn sized_encode_and_encode_to_slice_agree_where_both_fit() {
    let input = b"hello";
    let fixed: [u8; 8] = sized_encode(input);
    let mut output = [0u8; 8];

    let written = encode_to_slice(input, &mut output).expect("output is large enough");

    assert_eq!(written, 8);
    assert_eq!(fixed, output);
}

#[test]
fn sized_encode_truncates_where_encode_to_slice_refuses() {
    let fixed: [u8; 3] = sized_encode(b"hi");
    let mut output = [0u8; 3];

    assert_eq!(&fixed, b"D1M");
    assert!(encode_to_slice(b"hi", &mut output).is_err());
}

#[test]
fn sized_decode_keeps_the_trailing_partial_byte() {
    // Three symbols carry fifteen bits; `sized_decode` left-aligns the
    // remaining seven, while `decode_to_slice` drops them.
    let fixed: [u8; 2] = sized_decode(b"D1M");
    let mut output = [0xEEu8; 2];

    let written = decode_to_slice(b"D1M", &mut output).expect("output is large enough");

    assert_eq!(fixed[0], b'h');
    assert_ne!(fixed[1], 0);
    assert_eq!(written, 1);
    assert_eq!(output[1], 0xEE);
}

#[test]
fn encode_into_matches_encode_to_slice() {
    let mut input = [0u8; 40];
    let mut expected = [0u8; 64];
    let mut buffer = [0u8; 64];
    let mut rng = Rng(0x0BAD_C0DE_0BAD_C0DE);

    rng.fill(&mut input);

    for len in 0..input.len() {
        let written =
            encode_to_slice(&input[..len], &mut expected).expect("output is large enough");

        let mut sink = SliceSink::new(&mut buffer);

        encode_into(&input[..len], &mut sink).expect("the sink is large enough");

        assert_eq!(sink.written(), &expected[..written], "len={len}");
    }
}

#[test]
fn encode_into_propagates_sink_errors() {
    let mut buffer = [0u8; 1];
    let mut sink = SliceSink::new(&mut buffer);

    assert!(encode_into(b"hello", &mut sink).is_err());
}

#[test]
fn round_trip_through_the_slice_api_at_every_residue() {
    let mut input = [0u8; 41];
    let mut encoded = [0u8; 80];
    let mut decoded = [0u8; 64];
    let mut rng = Rng(0xF00D_F00D_F00D_F00D);

    rng.fill(&mut input);

    for len in 0..input.len() {
        let symbols = encode_to_slice(&input[..len], &mut encoded).expect("output is large enough");
        let written =
            try_decode_to_slice(&encoded[..symbols], &mut decoded).expect("input is canonical");

        assert_eq!(written, len, "len={len}");
        assert_eq!(&decoded[..written], &input[..len], "len={len}");
    }
}

#[test]
fn check_digit_and_symbol_need_no_allocation() {
    let mut input = [0u8; 24];
    let mut rng = Rng(0xABCD_1234_ABCD_1234);

    rng.fill(&mut input);

    for len in 0..input.len() {
        let mut expected: u64 = 0;

        for &byte in &input[..len] {
            expected = (expected * 256 + u64::from(byte)) % 37;
        }

        let digit = check_digit(&input[..len]);

        assert_eq!(u64::from(digit), expected, "len={len}");
        assert_eq!(check_symbol(&input[..len]), CHECK_ALPHABET[digit as usize]);
        assert_eq!(
            CHECK_DECODE_MAP[check_symbol(&input[..len]) as usize],
            digit
        );
    }
}

#[test]
fn no_input_panics_the_allocation_free_api() {
    let mut raw = [0u8; 64];
    let mut output = [0u8; 13];
    let mut sink_buffer = [0u8; 200];
    let mut rng = Rng(0x5EED_5EED_5EED_5EED);

    for _ in 0..256 {
        rng.fill(&mut raw);

        for len in 0..raw.len() {
            let input = &raw[..len];

            let _ = encode_to_slice(input, &mut output);
            let _ = decode_to_slice(input, &mut output);
            let _ = try_decode_to_slice(input, &mut output);
            let _: [u8; 7] = sized_encode(input);
            let _: [u8; 7] = sized_decode(input);
            let _ = check_digit(input);
            let _ = check_symbol(input);

            let mut sink = SliceSink::new(&mut sink_buffer);

            let _ = encode_into(input, &mut sink);
        }
    }
}
