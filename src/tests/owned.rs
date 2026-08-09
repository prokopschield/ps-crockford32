#![allow(clippy::expect_used)]

use alloc::{format, string::String, vec::Vec};

use crate::{decode, encode, sized_decode, sized_encode};

#[test]
fn empty_round_trip() {
    let input: &[u8] = &[];

    let encoded = encode(input);

    assert_eq!(encoded, "");
    assert_eq!(decode(encoded.as_bytes()), input);
}

#[test]
fn single_byte_round_trip() {
    for byte in 0..=255u8 {
        let input = [byte];

        let encoded = encode(&input);
        let decoded = decode(encoded.as_bytes());

        assert_eq!(decoded, input, "round trip failed for byte {byte:#04x}");
    }
}

#[test]
fn five_byte_boundary() {
    let input = [0x12, 0x34, 0x56, 0x78, 0x9a];

    let encoded = encode(&input);

    assert_eq!(encoded.len(), 8);
    assert_eq!(decode(encoded.as_bytes()), input);
}

#[test]
fn thirty_eight_bytes_yields_sixty_one_chars() {
    let input = [0xabu8; 38];

    let encoded = encode(&input);

    assert_eq!(encoded.len(), 61);
}

#[test]
fn sized_encode_truncates_to_capacity() {
    let input = [0xabu8; 38];

    let encoded: [u8; 60] = sized_encode(&input);

    assert_eq!(encoded.len(), 60);
}

#[test]
fn sized_encode_zero_capacity_returns_empty() {
    let encoded: [u8; 0] = sized_encode(&[0xab; 1024]);

    assert_eq!(encoded, []);
}

#[test]
fn sized_encode_ignores_suffix_after_output_is_full() {
    let prefix = [0x12, 0x34, 0x56, 0x78];
    let mut input = prefix.to_vec();
    input.extend_from_slice(&[0xff; 4096]);

    let encoded_prefix: [u8; 3] = sized_encode(&prefix);
    let encoded_full: [u8; 3] = sized_encode(&input);

    assert_eq!(encoded_full, encoded_prefix);
}

#[test]
fn sized_decode_pads_trailing_bits() {
    let input = [0xabu8; 38];

    let encoded: [u8; 60] = sized_encode(&input);
    let decoded: [u8; 38] = sized_decode(&encoded);

    assert_eq!(&decoded[..37], &input[..37]);
    assert_eq!(decoded[37] & 0xf0, input[37] & 0xf0);
    assert_eq!(decoded[37] & 0x0f, 0);
}

#[test]
fn sized_decode_zero_capacity_returns_empty() {
    let input = b"91JPRV3F".repeat(512);
    let decoded: [u8; 0] = sized_decode(&input);

    assert_eq!(decoded, []);
}

#[test]
fn sized_decode_ignores_suffix_after_output_is_full() {
    let encoded = encode(&[0x12, 0x34, 0x56, 0x78]);
    let mut with_suffix = encoded.clone();
    with_suffix.push_str("ZZZZ-ZZZZ invalid tail");

    let decoded_prefix: [u8; 2] = sized_decode(encoded.as_bytes());
    let decoded_full: [u8; 2] = sized_decode(with_suffix.as_bytes());

    assert_eq!(decoded_full, decoded_prefix);
}

#[test]
fn alphabet_uses_expected_characters() {
    let input: Vec<u8> = (0..32).collect();
    let encoded = encode(&input);

    for byte in encoded.as_bytes() {
        assert!(
            b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(byte),
            "unexpected character {:?} in output",
            *byte as char
        );
    }
}

#[test]
fn decode_is_case_insensitive() {
    let input = [0x12, 0x34, 0x56, 0x78, 0x9a];

    let upper = encode(&input);
    let lower = upper.to_lowercase();

    assert_eq!(decode(upper.as_bytes()), input);
    assert_eq!(decode(lower.as_bytes()), input);
}

#[test]
fn decode_accepts_ambiguous_aliases() {
    let canonical = decode(b"01");
    let via_o = decode(b"O1");
    let via_i = decode(b"0I");
    let via_l = decode(b"0L");

    assert_eq!(canonical, via_o);
    assert_eq!(canonical, via_i);
    assert_eq!(canonical, via_l);
    assert_eq!(decode(b"UB"), decode(b"VB"));
}

#[test]
fn decode_skips_whitespace_and_separators() {
    let input = [0x12, 0x34, 0x56, 0x78, 0x9a];

    let encoded = encode(&input);
    let with_separators = format!("{}-{} {}", &encoded[..3], &encoded[3..5], &encoded[5..]);

    assert_eq!(decode(with_separators.as_bytes()), input);
}

#[test]
fn encode_into_matches_encode() {
    let input: Vec<u8> = (0..50u8).collect();

    let via_string = encode(&input);

    let mut via_fmt = String::new();
    crate::encode_into(&input, &mut via_fmt).expect("write to String should not fail");

    assert_eq!(via_string, via_fmt);
}

#[test]
fn all_alphabet_letters_decode() {
    for letter in b"0123456789ABCDEFGHJKMNPQRSTVWXYZ" {
        let value = crate::DECODE_MAP[*letter as usize];

        assert_ne!(
            value,
            crate::INVALID,
            "alphabet letter {:?} should decode to a valid value",
            *letter as char
        );
    }
}

#[test]
fn u_aliases_to_v() {
    assert_eq!(crate::DECODE_MAP[b'U' as usize], 27);
    assert_eq!(crate::DECODE_MAP[b'u' as usize], 27);
    assert_eq!(
        crate::DECODE_MAP[b'U' as usize],
        crate::DECODE_MAP[b'V' as usize]
    );
}

#[test]
fn every_ascii_letter_decodes() {
    for upper in b'A'..=b'Z' {
        let value = crate::DECODE_MAP[upper as usize];

        let expected = match upper {
            b'O' => 0,
            b'I' | b'L' => 1,
            b'U' => 27,
            _ => {
                let index = crate::encoder::ALPHABET
                    .iter()
                    .position(|&symbol| symbol == upper)
                    .expect("every non-excluded letter is in the alphabet");

                u8::try_from(index).expect("alphabet indices fit in a u8")
            }
        };

        assert_eq!(
            value, expected,
            "letter {:?} must decode to {expected}",
            upper as char,
        );
    }
}

#[test]
fn decode_skips_arbitrary_invalid_bytes() {
    let input = [0x12, 0x34, 0x56, 0x78, 0x9a];

    let encoded = encode(&input);
    let mut polluted = String::new();

    for (i, ch) in encoded.chars().enumerate() {
        polluted.push(ch);
        polluted.push(match i % 4 {
            0 => '@',
            1 => '!',
            2 => '\0',
            _ => '\u{ff}',
        });
    }

    assert_eq!(decode(polluted.as_bytes()), input);
}

#[test]
fn sized_encode_pads_with_zero_character_when_short() {
    let input = b"\x12";
    let dynamic = encode(input);
    let fixed: [u8; 16] = sized_encode(input);

    assert_eq!(&fixed[..dynamic.len()], dynamic.as_bytes());
    assert!(fixed[dynamic.len()..].iter().all(|&b| b == b'0'));
}

#[test]
fn encode_into_propagates_sink_errors() {
    struct FailingSink;

    impl core::fmt::Write for FailingSink {
        fn write_str(&mut self, _: &str) -> core::fmt::Result {
            Err(core::fmt::Error)
        }
    }

    let result = crate::encode_into(b"\x12\x34", FailingSink);

    assert!(result.is_err());
}

#[test]
fn sized_encode_is_const_evaluable() {
    const ENCODED: [u8; 8] = crate::sized_encode(b"\x12\x34\x56\x78\x9a");

    assert_eq!(&ENCODED, encode(b"\x12\x34\x56\x78\x9a").as_bytes());
}

#[test]
fn sized_decode_is_const_evaluable() {
    const DECODED: [u8; 5] = crate::sized_decode(b"28T8DRWK");

    assert_eq!(decode(b"28T8DRWK"), DECODED);
}

#[test]
fn many_round_trips() {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    for length in 0..=64 {
        let mut input = Vec::with_capacity(length);

        for _ in 0..length {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            input.push((state >> 56) as u8);
        }

        let encoded = encode(&input);
        let decoded = decode(encoded.as_bytes());

        assert_eq!(decoded, input, "round trip failed for length {length}");
    }
}

#[test]
fn known_encoding_vectors() {
    let cases: &[(&[u8], &str)] = &[
        (&[], ""),
        (&[0x00], "00"),
        (&[0xff], "ZW"),
        (&[0x00, 0x00], "0000"),
        (&[0xff, 0xff], "ZZZG"),
        (&[0x00, 0x00, 0x00, 0x00, 0x00], "00000000"),
        (&[0xff, 0xff, 0xff, 0xff, 0xff], "ZZZZZZZZ"),
        (b"hi", "D1MG"),
        (b"hello", "D1JPRV3F"),
    ];

    for (input, expected) in cases {
        assert_eq!(encode(input), *expected, "encode({input:?})");
        assert_eq!(decode(expected.as_bytes()), *input, "decode({expected:?})");
    }
}

#[test]
fn encode_is_injective_for_same_length_inputs() {
    let inputs: [&[u8]; 4] = [b"abc", b"abd", b"aac", b"zzz"];
    let mut seen = alloc::collections::BTreeSet::new();

    for input in inputs {
        assert!(seen.insert(encode(input)), "collision for {input:?}");
    }
}

#[test]
fn round_trip_via_sized_variants() {
    let mut state: u64 = 0xCAFE_F00D_DEAD_BEEF;

    for _ in 0..32 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        let input = state.to_be_bytes();

        let encoded: [u8; 13] = sized_encode(&input);
        let decoded: [u8; 8] = sized_decode(&encoded);

        assert_eq!(decoded, input);
    }
}

#[test]
fn large_input_round_trip() {
    let input: Vec<u8> = (0u16..4096)
        .map(|i| (i ^ (i >> 3)).to_be_bytes()[1])
        .collect();

    let encoded = encode(&input);
    let decoded = decode(encoded.as_bytes());

    assert_eq!(decoded, input);
    assert_eq!(encoded.len(), (input.len() * 8).div_ceil(5));
}

#[test]
fn decoder_skips_all_ascii_whitespace_variants() {
    let payload = b"\x12\x34\x56\x78\x9a";
    let encoded = encode(payload);

    let mut polluted = String::new();
    let whitespaces = [' ', '\t', '\n', '\r', '\x0c', '\x0b'];

    for (i, ch) in encoded.chars().enumerate() {
        polluted.push(ch);
        polluted.push(whitespaces[i % whitespaces.len()]);
    }

    assert_eq!(decode(polluted.as_bytes()), payload);
}

#[test]
fn decoder_handles_pure_skip_input() {
    assert_eq!(decode(b"   "), b"");
    assert_eq!(decode(b"---"), b"");
    assert_eq!(decode(b" \n-\t\r "), b"");
    assert_eq!(decode(b""), b"");
}

#[test]
fn sized_encode_with_empty_input_yields_all_padding() {
    let encoded: [u8; 5] = sized_encode(&[]);

    assert_eq!(&encoded, b"00000");
}

#[test]
fn sized_decode_with_empty_input_yields_all_zero() {
    let decoded: [u8; 5] = sized_decode(&[]);

    assert_eq!(&decoded, &[0u8; 5]);
}

#[test]
fn sized_encode_and_encode_agree_on_prefix() {
    let inputs: [&[u8]; 5] = [&[], &[0x12], b"abc", b"hello world", &[0xff; 20]];

    for input in inputs {
        let dynamic = encode(input);
        let fixed: [u8; 64] = sized_encode(input);

        assert_eq!(
            &fixed[..dynamic.len()],
            dynamic.as_bytes(),
            "mismatch for {input:?}"
        );
    }
}

#[test]
fn decode_map_rejects_check_only_symbols() {
    for byte in *b"*~$=" {
        assert_eq!(
            crate::DECODE_MAP[byte as usize],
            crate::INVALID,
            "check-only symbol {:?} must be invalid in DECODE_MAP",
            byte as char,
        );
    }
}

#[test]
fn alphabet_excludes_all_ambiguous_letters() {
    let alphabet = crate::encoder::ALPHABET;

    for excluded in *b"ILOUilou" {
        assert!(
            !alphabet.contains(&excluded),
            "alphabet must not contain {:?}",
            excluded as char,
        );
    }
}

#[test]
fn single_byte_round_trip_via_sized_variants() {
    for byte in 0..=255u8 {
        let input = [byte];

        let encoded: [u8; 2] = sized_encode(&input);
        let decoded: [u8; 1] = sized_decode(&encoded);

        assert_eq!(
            decoded, input,
            "sized round trip failed for byte {byte:#04x}"
        );
    }
}

#[test]
fn encoded_output_never_contains_excluded_letters() {
    let mut state: u64 = 0xF00D_DEAD_BEEF_CAFE;
    let excluded = b"ILOUilou";

    for length in 0..=32usize {
        let mut input = Vec::with_capacity(length);

        for _ in 0..length {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            input.push((state >> 56) as u8);
        }

        let encoded = encode(&input);

        for byte in encoded.as_bytes() {
            assert!(
                !excluded.contains(byte),
                "encoded output contains excluded letter {:?}: {encoded:?}",
                *byte as char,
            );
        }
    }
}

#[test]
fn decode_map_every_entry_is_valid_or_invalid() {
    for byte in 0..=255u8 {
        let value = crate::DECODE_MAP[byte as usize];

        assert!(
            value == crate::INVALID || value < 32,
            "DECODE_MAP[{byte:#04x}] = {value}, outside valid range",
        );
    }
}

#[test]
fn encode_into_empty_input_writes_nothing() {
    let mut buffer = String::new();

    crate::encode_into(&[], &mut buffer).expect("write to String never fails");

    assert!(buffer.is_empty());
}

#[test]
fn crockford32_namespace_re_exports_match_root() {
    use crate::crockford32;

    assert_eq!(crockford32::encode(b"hi"), crate::encode(b"hi"));
    assert_eq!(crockford32::decode(b"D1MG"), crate::decode(b"D1MG"));
    assert_eq!(crockford32::try_decode(b"D1MG"), crate::try_decode(b"D1MG"));

    let a: [u8; 4] = crockford32::sized_encode(b"hi");
    let b: [u8; 4] = crate::sized_encode(b"hi");
    assert_eq!(a, b);

    let c: [u8; 2] = crockford32::sized_decode(b"D1MG");
    let d: [u8; 2] = crate::sized_decode(b"D1MG");
    assert_eq!(c, d);

    assert_eq!(crockford32::encoded_len(2), crate::encoded_len(2));
    assert_eq!(crockford32::decoded_len(4), crate::decoded_len(4));

    let mut namespaced = String::new();
    let mut root = String::new();

    crockford32::encode_into(b"hi", &mut namespaced).expect("write to String never fails");
    crate::encode_into(b"hi", &mut root).expect("write to String never fails");

    assert_eq!(namespaced, root);

    let mut namespaced = [0u8; 4];
    let mut root = [0u8; 4];

    assert_eq!(
        crockford32::encode_to_slice(b"hi", &mut namespaced),
        crate::encode_to_slice(b"hi", &mut root)
    );
    assert_eq!(namespaced, root);

    let mut namespaced = [0u8; 2];
    let mut root = [0u8; 2];

    assert_eq!(
        crockford32::decode_to_slice(b"D1MG", &mut namespaced),
        crate::decode_to_slice(b"D1MG", &mut root)
    );
    assert_eq!(namespaced, root);

    let mut namespaced = [0u8; 2];
    let mut root = [0u8; 2];

    assert_eq!(
        crockford32::try_decode_to_slice(b"D1MG", &mut namespaced),
        crate::try_decode_to_slice(b"D1MG", &mut root)
    );
    assert_eq!(namespaced, root);

    // The namespaced error types are re-exports rather than separate
    // definitions, so these coercions compile only if both paths name the
    // same item.
    let capacity: crockford32::CapacityError = crate::CapacityError {
        required: 5,
        available: 2,
    };
    let _: crate::CapacityError = capacity;

    let decode: crockford32::DecodeError = crate::DecodeError::InvalidByte {
        position: 0,
        byte: b'!',
    };
    let _: crate::DecodeError = decode;
}

#[test]
fn decode_map_specific_canonical_values() {
    assert_eq!(crate::DECODE_MAP[b'0' as usize], 0);
    assert_eq!(crate::DECODE_MAP[b'9' as usize], 9);
    assert_eq!(crate::DECODE_MAP[b'A' as usize], 10);
    assert_eq!(crate::DECODE_MAP[b'Z' as usize], 31);
}

#[test]
fn decode_of_single_character_returns_empty() {
    // A single 5-bit symbol is not enough to emit a byte; decode returns
    // empty rather than padding or panicking.
    assert!(decode(b"5").is_empty());
    assert!(decode(b"Z").is_empty());
}

#[test]
fn decode_discards_trailing_partial_bits_explicitly() {
    // 8 chars = 40 bits = 5 bytes, no leftover.
    assert_eq!(decode(b"91JPRV3F").len(), 5);
    // 9 chars = 45 bits = 5 bytes with 5 leftover bits that must be
    // discarded by `decode` (use `sized_decode` to preserve).
    assert_eq!(decode(b"91JPRV3F0").len(), 5);
}

#[test]
fn round_trip_covers_every_bits_residue_after_loop() {
    // Encoder leftover bits after the last byte are `(8 * len) mod 5`:
    // len=1 -> 3, len=2 -> 1, len=3 -> 4, len=4 -> 2, len=5 -> 0.
    // Each residue must round trip.
    for len in 1u8..=5 {
        let input: Vec<u8> = (0..len).map(|i| 0xa5u8.wrapping_add(i)).collect();
        let encoded = encode(&input);
        let decoded = decode(encoded.as_bytes());

        assert_eq!(decoded, input, "residue case len={len}");
    }
}

#[test]
fn encode_of_all_ones_at_five_byte_boundary_is_all_z() {
    let encoded = encode(&[0xffu8; 5]);

    assert_eq!(encoded, "ZZZZZZZZ");
}

#[test]
fn sized_encode_capacity_one_writes_first_symbol_only() {
    let encoded: [u8; 1] = sized_encode(b"\xff\xff");

    assert_eq!(&encoded, b"Z");
}

#[test]
fn sized_encode_capacity_exceeds_exact_pads_with_zero() {
    let input = [0x12, 0x34, 0x56, 0x78, 0x9a];

    // encode(input) is exactly 8 chars; ask for 12 to force four '0' pads.
    let encoded: [u8; 12] = sized_encode(&input);
    let dynamic = encode(&input);

    assert_eq!(&encoded[..8], dynamic.as_bytes());
    assert_eq!(&encoded[8..], &[b'0'; 4]);
}

#[test]
fn sized_encode_three_byte_input_writes_trailing_partial_symbol() {
    // 3 bytes = 24 bits = 4 full 5-bit symbols + 4 leftover bits, so
    // sized_encode<5> must exercise the trailing-bits branch.
    let input = [0xab, 0xcd, 0xef];

    let encoded: [u8; 5] = sized_encode(&input);
    let dynamic = encode(&input);

    assert_eq!(&encoded, dynamic.as_bytes());
}

#[test]
fn sized_decode_capacity_one_only_skip_chars_leaves_zero() {
    let decoded: [u8; 1] = sized_decode(b"   ---  ");

    assert_eq!(&decoded, &[0u8]);
}

#[test]
fn sized_decode_with_arbitrary_invalid_byte_is_skipped() {
    let encoded = encode(b"\x12\x34");
    let mut polluted = encoded.into_bytes();
    polluted.insert(1, b'@');

    let decoded: [u8; 2] = sized_decode(&polluted);

    assert_eq!(&decoded, b"\x12\x34");
}

#[test]
fn sized_decode_capacity_well_above_payload_zeroes_unused_slots() {
    let encoded = encode(b"\xab");
    let decoded: [u8; 8] = sized_decode(encoded.as_bytes());

    // The first byte holds the high bits of the partial encoding; the
    // remaining seven slots stay at their initialized zero value.
    assert_eq!(decoded[0] & 0xf0, 0xa0);
    assert_eq!(decoded[1..], [0u8; 7]);
}

#[test]
fn sized_decode_when_bits_zero_skips_trailing_flush() {
    // Exactly 5 bytes = 8 chars with no leftover bits, so the trailing
    // `bits > 0` flush must NOT fire and the next slot must stay zero.
    let encoded = encode(&[0x12, 0x34, 0x56, 0x78, 0x9a]);
    let decoded: [u8; 6] = sized_decode(encoded.as_bytes());

    assert_eq!(&decoded[..5], &[0x12, 0x34, 0x56, 0x78, 0x9a]);
    assert_eq!(decoded[5], 0);
}

#[test]
fn decode_rejects_check_only_symbols_in_payload() {
    // Each of the four check-only symbols without an alias must be
    // silently skipped, not mistaken for data, in the plain decoder.
    for symbol in *b"*~$=" {
        let mut input = b"91JPRV3F".to_vec();
        input.insert(4, symbol);

        assert_eq!(
            decode(&input),
            b"Hello",
            "check-only symbol {:?} should be skipped in payload",
            symbol as char,
        );
    }
}

#[test]
fn decode_treats_u_as_v_in_payload() {
    for symbol in *b"Uu" {
        let mut input = b"91JPRV3F".to_vec();
        input[5] = symbol;

        assert_eq!(
            decode(&input),
            b"Hello",
            "{:?} should decode as `V` in payload",
            symbol as char,
        );
    }
}

#[test]
fn decode_accepts_arbitrary_hyphen_placement() {
    let payload = b"\x12\x34\x56\x78\x9a";
    let encoded = encode(payload);

    let leading = alloc::format!("-{encoded}");
    let trailing = alloc::format!("{encoded}-");
    let consecutive = alloc::format!("{}---{}", &encoded[..4], &encoded[4..]);

    assert_eq!(decode(leading.as_bytes()), payload);
    assert_eq!(decode(trailing.as_bytes()), payload);
    assert_eq!(decode(consecutive.as_bytes()), payload);
}

#[test]
fn sized_encode_exact_capacity_fits_without_padding() {
    let input = [0x12, 0x34, 0x56, 0x78, 0x9a];

    let encoded: [u8; 8] = sized_encode(&input);
    let dynamic = encode(&input);

    assert_eq!(&encoded, dynamic.as_bytes());
}

#[test]
fn lowercase_aliases_match_uppercase() {
    for upper in b'A'..=b'Z' {
        let lower = upper + 32;
        let upper_val = crate::DECODE_MAP[upper as usize];
        let lower_val = crate::DECODE_MAP[lower as usize];

        assert_eq!(
            upper_val, lower_val,
            "uppercase {:?} and lowercase {:?} must decode identically",
            upper as char, lower as char,
        );
    }
}

mod check {
    use alloc::{vec, vec::Vec};

    use crate::check::{
        check_digit, check_symbol, decode_with_check, encode_with_check, CheckError,
        CHECK_ALPHABET, CHECK_DECODE_MAP,
    };
    use crate::DecodeError;

    #[test]
    fn check_alphabet_layout() {
        assert_eq!(&CHECK_ALPHABET[..32], &crate::encoder::ALPHABET);
        assert_eq!(&CHECK_ALPHABET[32..], b"*~$=U");
    }

    #[test]
    fn check_decode_map_recognizes_check_symbols() {
        assert_eq!(CHECK_DECODE_MAP[b'*' as usize], 32);
        assert_eq!(CHECK_DECODE_MAP[b'~' as usize], 33);
        assert_eq!(CHECK_DECODE_MAP[b'$' as usize], 34);
        assert_eq!(CHECK_DECODE_MAP[b'=' as usize], 35);
        assert_eq!(CHECK_DECODE_MAP[b'U' as usize], 36);
        assert_eq!(CHECK_DECODE_MAP[b'u' as usize], 36);
    }

    #[test]
    fn check_digit_matches_big_endian_modulo() {
        assert_eq!(check_digit(&[]), 0);
        assert_eq!(check_digit(&[0]), 0);
        assert_eq!(check_digit(&[0x12]), 18);
        assert_eq!(check_digit(&[0xff, 0xff]), 8);
        assert_eq!(check_digit(&[0x00, 0x12]), 18);
    }

    #[test]
    fn check_digit_is_const_evaluable() {
        const DIGIT: u8 = check_digit(&[0x12]);

        assert_eq!(DIGIT, 18);
    }

    #[test]
    fn check_symbol_uses_extended_alphabet() {
        assert_eq!(check_symbol(&[0x12]), b'J');
        assert_eq!(check_symbol(&[]), b'0');
    }

    #[test]
    fn encode_with_check_appends_one_symbol() {
        let payload = b"\x12\x34\x56\x78\x9a";

        let plain = crate::encode(payload);
        let checked = encode_with_check(payload);

        assert!(checked.starts_with(plain.as_str()));
        assert_eq!(checked.len(), plain.len() + 1);
    }

    #[test]
    fn check_round_trips_through_encode_and_decode() {
        for length in 0u8..=16 {
            let payload: Vec<u8> = (0..length).map(|i| i.wrapping_mul(31)).collect();
            let encoded = encode_with_check(&payload);

            assert_eq!(decode_with_check(encoded.as_bytes()), Ok(payload));
        }
    }

    #[test]
    fn decode_with_check_rejects_empty_input() {
        assert_eq!(decode_with_check(b""), Err(CheckError::Missing));
        assert_eq!(decode_with_check(b"   --   "), Err(CheckError::Missing));
    }

    #[test]
    fn decode_with_check_rejects_invalid_trailing_symbol() {
        // '@' is not in the 37-symbol check alphabet.
        assert_eq!(decode_with_check(b"28@"), Err(CheckError::InvalidSymbol));
    }

    #[test]
    fn decode_with_check_detects_mismatch() {
        let payload = b"\x12";
        let mut encoded = encode_with_check(payload).into_bytes();

        // Flip the last character (the check symbol) to something else.
        let last = encoded.last_mut().expect("non-empty encoding");
        *last = if *last == b'X' { b'Y' } else { b'X' };

        assert!(matches!(
            decode_with_check(&encoded),
            Err(CheckError::Mismatch { .. })
        ));
    }

    #[test]
    fn decode_with_check_ignores_skip_chars_around_check_symbol() {
        let checked = encode_with_check(b"\x12\x34");
        let with_skips = alloc::format!("{checked}\n -");

        assert_eq!(
            decode_with_check(with_skips.as_bytes()),
            Ok(vec![0x12, 0x34])
        );
    }

    #[test]
    fn decode_with_check_recognizes_u_as_check_only() {
        // 36 = 'U' as check digit; precompute an input whose check is 36.
        let mut bytes = [0u8; 1];
        for byte in 0..=255u8 {
            bytes[0] = byte;
            if check_digit(&bytes) == 36 {
                let encoded = encode_with_check(&bytes);

                assert!(encoded.ends_with('U'));
                assert_eq!(decode_with_check(encoded.as_bytes()), Ok(vec![byte]));

                return;
            }
        }

        panic!("expected to find some byte whose check digit is 36");
    }

    #[test]
    fn all_check_only_symbols_validate_as_check_digits() {
        let targets: [(u8, char); 5] = [(32, '*'), (33, '~'), (34, '$'), (35, '='), (36, 'U')];

        for (target_digit, symbol) in targets {
            let mut bytes = [0u8; 1];

            for byte in 0..=255u8 {
                bytes[0] = byte;
                if check_digit(&bytes) == target_digit {
                    let encoded = encode_with_check(&bytes);

                    assert!(
                        encoded.ends_with(symbol),
                        "expected encoding to end with {symbol:?}, got {encoded:?}",
                    );
                    assert_eq!(decode_with_check(encoded.as_bytes()), Ok(vec![byte]));
                    break;
                }

                assert!(
                    byte != 255,
                    "no single-byte input found with check digit {target_digit}",
                );
            }
        }
    }

    #[test]
    fn every_check_digit_value_is_reachable() {
        let mut reached = [false; 37];

        for byte in 0..=255u8 {
            reached[check_digit(&[byte]) as usize] = true;
        }

        for (digit, &was_reached) in reached.iter().enumerate() {
            assert!(
                was_reached,
                "check digit {digit} was not reached by any single byte"
            );
        }
    }

    #[test]
    fn check_digit_matches_naive_big_endian_modulo() {
        // For inputs that fit in a u64, the loop result must agree with
        // the obvious `n % 37` computation.
        let inputs: [&[u8]; 6] = [
            &[],
            &[0x01],
            &[0x12, 0x34],
            &[0x12, 0x34, 0x56],
            &[0xde, 0xad, 0xbe, 0xef],
            &[0xff; 8],
        ];

        for input in inputs {
            let mut naive: u64 = 0;
            for &byte in input {
                naive = naive.wrapping_mul(256).wrapping_add(u64::from(byte));
            }

            #[allow(clippy::cast_possible_truncation)]
            let expected = (naive % 37) as u8;

            assert_eq!(check_digit(input), expected, "check_digit({input:?})");
        }
    }

    #[test]
    fn decode_with_check_round_trips_random_payloads() {
        let mut state: u64 = 0xDEAD_BEEF_F00D_BABE;

        for length in 0u8..=32 {
            let mut payload = Vec::with_capacity(length as usize);
            for _ in 0..length {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                payload.push((state >> 56) as u8);
            }

            let encoded = encode_with_check(&payload);

            assert_eq!(
                decode_with_check(encoded.as_bytes()),
                Ok(payload),
                "round trip failed for length {length}",
            );
        }
    }

    #[test]
    fn decode_with_check_accepts_lowercase_u_check() {
        // Find an input whose check digit is 36, encode it, swap the
        // trailing 'U' for 'u', and verify the decoder still accepts it.
        for byte in 0..=255u8 {
            if check_digit(&[byte]) == 36 {
                let mut encoded = encode_with_check(&[byte]).into_bytes();
                let last = encoded.last_mut().expect("non-empty encoding");
                *last = b'u';

                assert_eq!(decode_with_check(&encoded), Ok(vec![byte]));
                return;
            }
        }
    }

    #[test]
    fn check_decode_map_every_entry_is_valid_or_invalid() {
        for byte in 0..=255u8 {
            let value = CHECK_DECODE_MAP[byte as usize];

            assert!(
                value == crate::INVALID || value < 37,
                "CHECK_DECODE_MAP[{byte:#04x}] = {value}, outside valid range",
            );
        }
    }

    #[test]
    fn check_decode_map_extends_decode_map() {
        // Every byte that's valid in DECODE_MAP must decode to the same
        // value in CHECK_DECODE_MAP, except 'U'/'u': aliases for 'V' in
        // payloads, they are overridden to check value 36 here. New
        // mappings only add the four remaining check-only symbols.
        for byte in 0..=255u8 {
            if byte == b'U' || byte == b'u' {
                continue;
            }

            let plain = crate::DECODE_MAP[byte as usize];

            if plain != crate::INVALID {
                assert_eq!(
                    CHECK_DECODE_MAP[byte as usize], plain,
                    "CHECK_DECODE_MAP diverges from DECODE_MAP at {byte:#04x}",
                );
            }
        }

        assert_eq!(crate::DECODE_MAP[b'U' as usize], 27);
        assert_eq!(crate::DECODE_MAP[b'u' as usize], 27);
        assert_eq!(CHECK_DECODE_MAP[b'U' as usize], 36);
        assert_eq!(CHECK_DECODE_MAP[b'u' as usize], 36);
    }

    #[test]
    fn check_digit_independent_of_leading_zero_padding() {
        let mut state: u64 = 0xABCD_EF01_2345_6789;

        for _ in 0..8 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let bytes = state.to_be_bytes();
            let trimmed: &[u8] = match bytes.iter().position(|&b| b != 0) {
                Some(start) => &bytes[start..],
                None => &[],
            };

            assert_eq!(check_digit(&bytes), check_digit(trimmed));
        }
    }

    #[test]
    fn decode_with_check_accepts_ambiguous_aliases_as_check_symbol() {
        // The check map inherits ambiguous-glyph aliases from DECODE_MAP:
        // O/o -> 0, I/i/L/l -> 1.
        // check_digit(&[]) == 0, so a single 'O' is a valid check.
        assert_eq!(decode_with_check(b"O"), Ok(vec![]));
        assert_eq!(decode_with_check(b"o"), Ok(vec![]));

        // A single 'I' or 'L' encodes value 1; find a payload whose
        // check is 1.
        for byte in 0..=255u8 {
            if check_digit(&[byte]) == 1 {
                let body = encode_with_check(&[byte]);
                let body_only = &body.as_bytes()[..body.len() - 1];

                let mut with_i = body_only.to_vec();
                with_i.push(b'I');
                let mut with_l = body_only.to_vec();
                with_l.push(b'L');

                assert_eq!(decode_with_check(&with_i), Ok(vec![byte]));
                assert_eq!(decode_with_check(&with_l), Ok(vec![byte]));
                break;
            }
        }
    }

    #[test]
    fn decode_with_check_accepts_u_as_v_in_body() {
        // [0xDA] encodes to "V8"; a body 'U' aliases to 'V', while the
        // trailing check symbol is untouched.
        let checked = encode_with_check(&[0xDA]);

        assert_eq!(checked.as_bytes()[0], b'V');

        let mut with_u = checked.into_bytes();
        with_u[0] = b'U';

        assert_eq!(decode_with_check(&with_u), Ok(vec![0xDA]));
    }

    #[test]
    fn decode_with_check_rejects_check_only_symbols_inside_body() {
        // A mid-body '*' is outside the payload alphabet, so the strict
        // body decoder rejects it instead of dropping it and reconstructing
        // the original payload from what remains.
        let payload = b"\x12\x34\x56";
        let encoded = encode_with_check(payload);

        let mut polluted = encoded.into_bytes();
        polluted.insert(2, b'*');

        assert_eq!(
            decode_with_check(&polluted),
            Err(CheckError::Body(DecodeError::InvalidByte {
                position: 2,
                byte: b'*',
            }))
        );
    }

    #[test]
    fn decode_with_check_rejects_invalid_byte_in_body() {
        let payload = b"\xde\xad\xbe\xef";
        let encoded = encode_with_check(payload);

        let mut polluted = encoded.into_bytes();
        polluted.insert(3, b'@');

        assert_eq!(
            decode_with_check(&polluted),
            Err(CheckError::Body(DecodeError::InvalidByte {
                position: 3,
                byte: b'@',
            }))
        );
    }

    #[test]
    fn decode_with_check_rejects_a_non_canonical_body() {
        // Mistyping the final body symbol perturbs bits the payload
        // discards, which the check digit alone cannot see.
        assert_eq!(
            decode_with_check(b"29J"),
            Err(CheckError::Body(DecodeError::NonZeroPadding {
                position: 1,
                byte: b'9',
            }))
        );
        assert_eq!(decode_with_check(b"28J"), Ok(vec![0x12]));
    }

    #[test]
    fn decode_with_check_rejects_whitespace_inside_the_body() {
        let checked = encode_with_check(b"\x12\x34");
        let mut polluted = checked.into_bytes();
        polluted.insert(1, b' ');

        assert_eq!(
            decode_with_check(&polluted),
            Err(CheckError::Body(DecodeError::InvalidByte {
                position: 1,
                byte: b' ',
            }))
        );
    }

    #[test]
    fn decode_with_check_accepts_hyphens_inside_the_body() {
        let checked = encode_with_check(b"\x12\x34");
        let mut hyphenated = checked.into_bytes();
        hyphenated.insert(1, b'-');

        assert_eq!(decode_with_check(&hyphenated), Ok(vec![0x12, 0x34]));
    }

    /// Deterministic payloads of the given length, so the sweeps below need
    /// no `rand`.
    fn payload(length: usize, seed: u64) -> Vec<u8> {
        let mut state = seed;
        let mut output = Vec::with_capacity(length);

        for _ in 0..length {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            output.push((state >> 56) as u8);
        }

        output
    }

    #[test]
    fn every_single_symbol_substitution_is_detected() {
        // The guarantee documented on `decode_with_check`: a canonical body
        // plus a check digit modulo 37 leaves no single-symbol substitution
        // undetected, because 2 is a primitive root modulo 37 and a symbol
        // differs from another by less than 37.
        for length in 0..=8usize {
            let payload = payload(length, 0x1234_5678_9ABC_DEF0);
            let checked = encode_with_check(&payload).into_bytes();

            for position in 0..checked.len() {
                for &symbol in &CHECK_ALPHABET {
                    if symbol == checked[position] {
                        continue;
                    }

                    let mut mutated = checked.clone();
                    mutated[position] = symbol;

                    assert_ne!(
                        decode_with_check(&mutated).as_deref(),
                        Ok(payload.as_slice()),
                        "substituting {:?} at position {position} of {:?} went undetected",
                        symbol as char,
                        core::str::from_utf8(&checked).expect("check symbols are ASCII"),
                    );
                }
            }
        }
    }

    #[test]
    fn every_adjacent_transposition_is_detected() {
        // Transposing adjacent symbols perturbs the payload by a multiple of
        // 31, which is likewise nonzero modulo 37.
        for length in 0..=8usize {
            let payload = payload(length, 0x0F1E_2D3C_4B5A_6978);
            let checked = encode_with_check(&payload).into_bytes();

            for position in 0..checked.len().saturating_sub(1) {
                if checked[position] == checked[position + 1] {
                    continue;
                }

                let mut mutated = checked.clone();
                mutated.swap(position, position + 1);

                assert_ne!(
                    decode_with_check(&mutated).as_deref(),
                    Ok(payload.as_slice()),
                    "transposing positions {position} and {} of {:?} went undetected",
                    position + 1,
                    core::str::from_utf8(&checked).expect("check symbols are ASCII"),
                );
            }
        }
    }

    #[test]
    fn decode_with_check_does_not_reliably_reject_an_unchecked_string() {
        // Four symbols leave a three-symbol body, which is never canonical,
        // so this unchecked string is caught.
        assert!(matches!(
            decode_with_check(crate::encode(b"hi").as_bytes()),
            Err(CheckError::Body(DecodeError::ExcessSymbol { .. }))
        ));

        // Five symbols leave a canonical four-symbol body, and here the
        // check digit agrees, so a truncated payload comes back instead.
        assert_eq!(crate::encode(&[0, 0, 0]), "00000");
        assert_eq!(decode_with_check(b"00000"), Ok(vec![0, 0]));
    }

    #[test]
    fn check_error_implements_debug_and_eq() {
        let missing = CheckError::Missing;
        let mismatch = CheckError::Mismatch {
            expected: 1,
            actual: 2,
        };
        let body = CheckError::Body(DecodeError::InvalidByte {
            position: 0,
            byte: b' ',
        });

        assert_eq!(missing, CheckError::Missing);
        assert_ne!(missing, CheckError::InvalidSymbol);
        assert_ne!(missing, body);
        assert_ne!(
            mismatch,
            CheckError::Mismatch {
                expected: 1,
                actual: 3
            }
        );
        assert_ne!(
            body,
            CheckError::Body(DecodeError::InvalidByte {
                position: 1,
                byte: b' ',
            })
        );

        let _ = alloc::format!("{missing:?}");
        let _ = alloc::format!("{mismatch:?}");
        let _ = alloc::format!("{body:?}");
    }

    #[test]
    fn check_error_implements_display_and_error() {
        let mismatch = CheckError::Mismatch {
            expected: 1,
            actual: 2,
        };
        let body = CheckError::Body(DecodeError::InvalidByte {
            position: 3,
            byte: 0x40,
        });

        assert_eq!(
            alloc::format!("{}", CheckError::Missing),
            "input contains no check symbol"
        );
        assert_eq!(
            alloc::format!("{}", CheckError::InvalidSymbol),
            "trailing symbol is not part of the check alphabet"
        );
        assert_eq!(
            alloc::format!("{mismatch}"),
            "check digit mismatch: payload requires 1, input supplied 2"
        );
        assert_eq!(
            alloc::format!("{body}"),
            "invalid body: invalid byte 0x40 at position 3"
        );

        let error: &dyn core::error::Error = &mismatch;

        assert!(error.source().is_none());

        let error: &dyn core::error::Error = &body;

        assert!(error.source().is_some());
    }

    #[test]
    fn check_error_converts_from_decode_error() {
        let error = DecodeError::ExcessSymbol {
            position: 0,
            byte: b'0',
        };

        assert_eq!(CheckError::from(error), CheckError::Body(error));
    }
}

mod strict {
    use alloc::{vec, vec::Vec};

    use crate::{decode, encode, try_decode, CapacityError, DecodeError};

    #[test]
    fn try_decode_matches_decode_on_clean_input() {
        let mut state: u64 = 0x1357_9BDF_2468_ACE0;

        for length in 0..=64 {
            let mut input = Vec::with_capacity(length);

            for _ in 0..length {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                input.push((state >> 56) as u8);
            }

            let encoded = encode(&input);

            assert_eq!(
                try_decode(encoded.as_bytes()),
                Ok(decode(encoded.as_bytes())),
                "mismatch for length {length}",
            );
            assert_eq!(try_decode(encoded.as_bytes()), Ok(input));
        }
    }

    #[test]
    fn try_decode_skips_hyphens_only() {
        assert_eq!(try_decode(b"D1-MG"), Ok(b"hi".to_vec()));
        assert_eq!(try_decode(b"-D1MG-"), Ok(b"hi".to_vec()));
        assert_eq!(try_decode(b"---"), Ok(Vec::new()));
    }

    #[test]
    fn try_decode_rejects_whitespace() {
        assert_eq!(
            try_decode(b"D1 MG"),
            Err(DecodeError::InvalidByte {
                position: 2,
                byte: b' ',
            })
        );
        assert_eq!(
            try_decode(b"\tD1MG"),
            Err(DecodeError::InvalidByte {
                position: 0,
                byte: b'\t',
            })
        );
    }

    #[test]
    fn try_decode_reports_first_invalid_byte() {
        assert_eq!(
            try_decode(b"D1@M!G"),
            Err(DecodeError::InvalidByte {
                position: 2,
                byte: b'@',
            })
        );
    }

    #[test]
    fn try_decode_rejects_check_only_symbols() {
        for byte in *b"*~$=" {
            let mut input = b"D1MG".to_vec();
            input.insert(2, byte);

            assert_eq!(
                try_decode(&input),
                Err(DecodeError::InvalidByte { position: 2, byte })
            );
        }
    }

    #[test]
    fn try_decode_accepts_aliases_and_lowercase() {
        assert_eq!(try_decode(b"d1mg"), Ok(b"hi".to_vec()));
        assert_eq!(try_decode(b"DIMG"), Ok(b"hi".to_vec()));
        assert_eq!(try_decode(b"dlmg"), Ok(b"hi".to_vec()));
        assert_eq!(try_decode(b"OO"), Ok(decode(b"00")));
        assert_eq!(try_decode(b"I0"), Ok(decode(b"10")));
        assert_eq!(try_decode(b"L0"), Ok(decode(b"10")));
        assert_eq!(try_decode(b"U8"), Ok(decode(b"V8")));
        assert_eq!(try_decode(b"u8"), Ok(decode(b"V8")));
    }

    #[test]
    fn try_decode_rejects_nonzero_discarded_bits() {
        // `b"29"` and the canonical `b"28"` both denote `[0x12]`.
        assert_eq!(
            try_decode(b"29"),
            Err(DecodeError::NonZeroPadding {
                position: 1,
                byte: b'9',
            })
        );
        assert_eq!(try_decode(b"28"), Ok(vec![0x12]));

        // Hyphens do not shift the reported position.
        assert_eq!(
            try_decode(b"2-9"),
            Err(DecodeError::NonZeroPadding {
                position: 2,
                byte: b'9',
            })
        );
    }

    #[test]
    fn try_decode_rejects_a_symbol_that_carries_no_payload() {
        assert_eq!(
            try_decode(b"0"),
            Err(DecodeError::ExcessSymbol {
                position: 0,
                byte: b'0',
            })
        );
        assert_eq!(
            try_decode(b"280"),
            Err(DecodeError::ExcessSymbol {
                position: 2,
                byte: b'0',
            })
        );
    }

    #[test]
    fn try_decode_accepts_exactly_the_canonical_encodings() {
        let mut state: u64 = 0x2468_ACE0_1357_9BDF;

        for length in 0..=64usize {
            let mut input = Vec::with_capacity(length);

            for _ in 0..length {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                input.push((state >> 56) as u8);
            }

            let encoded = encode(&input);

            // Canonicity means the round trip closes in both directions,
            // so no re-encode-and-compare step is needed at a call site.
            let decoded = try_decode(encoded.as_bytes()).expect("encode output is canonical");

            assert_eq!(decoded, input, "length {length}");
            assert_eq!(encode(&decoded), encoded, "length {length}");
        }
    }

    #[test]
    fn try_decode_rejects_every_non_canonical_symbol_string() {
        let mut state: u64 = 0x0F1E_2D3C_4B5A_6978;

        // Sweep arbitrary symbol strings; each accepted one must re-encode
        // to itself, and no rejected one may round-trip.
        for length in 0..=24usize {
            for _ in 0..64 {
                let mut input = Vec::with_capacity(length);

                for _ in 0..length {
                    state = state
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1);
                    input.push(crate::ALPHABET[((state >> 56) & 0x1f) as usize]);
                }

                match try_decode(&input) {
                    Ok(payload) => assert_eq!(
                        encode(&payload).as_bytes(),
                        input.as_slice(),
                        "accepted a non-canonical input"
                    ),
                    Err(_) => assert_ne!(
                        encode(&decode(&input)).as_bytes(),
                        input.as_slice(),
                        "rejected a canonical input"
                    ),
                }
            }
        }
    }

    #[test]
    fn try_decode_empty_input_yields_empty() {
        assert_eq!(try_decode(b""), Ok(Vec::new()));
    }

    #[test]
    fn decode_error_implements_display_and_error() {
        let error = DecodeError::InvalidByte {
            position: 3,
            byte: 0x40,
        };

        assert_eq!(alloc::format!("{error}"), "invalid byte 0x40 at position 3");
        assert_eq!(
            alloc::format!(
                "{}",
                DecodeError::NonZeroPadding {
                    position: 1,
                    byte: b'9'
                }
            ),
            "byte 0x39 at position 1 sets bits that decoding discards"
        );
        assert_eq!(
            alloc::format!(
                "{}",
                DecodeError::ExcessSymbol {
                    position: 0,
                    byte: b'0'
                }
            ),
            "byte 0x30 at position 0 contributes no bits to the payload"
        );
        assert_eq!(
            alloc::format!(
                "{}",
                DecodeError::Capacity(CapacityError {
                    required: 2,
                    available: 1
                })
            ),
            "output slice length 1 is below the required length 2"
        );

        let dynamic: &dyn core::error::Error = &error;

        assert!(dynamic.source().is_none());

        let capacity = DecodeError::Capacity(CapacityError {
            required: 2,
            available: 1,
        });
        let dynamic: &dyn core::error::Error = &capacity;

        assert!(dynamic.source().is_some());
    }

    #[test]
    fn decode_error_converts_from_capacity_error() {
        let error = CapacityError {
            required: 4,
            available: 3,
        };

        assert_eq!(DecodeError::from(error), DecodeError::Capacity(error));
    }
}

mod slices {
    use alloc::vec::Vec;

    use crate::{
        decode_to_slice, decoded_len, encode, encode_to_slice, encoded_len, sized_decode,
        sized_encode, try_decode_to_slice, CapacityError, DecodeError,
    };

    #[test]
    fn encode_to_slice_matches_encode_at_exact_capacity() {
        let mut state: u64 = 0xFEED_FACE_CAFE_BEEF;

        for length in 0..=64usize {
            let mut input = Vec::with_capacity(length);

            for _ in 0..length {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                input.push((state >> 56) as u8);
            }

            let expected = encode(&input);

            let mut output = alloc::vec![0u8; expected.len()];
            let written = encode_to_slice(&input, &mut output);

            assert_eq!(written, Ok(expected.len()), "length {length}");
            assert_eq!(encoded_len(length), expected.len(), "length {length}");
            assert_eq!(output, expected.as_bytes(), "length {length}");
        }
    }

    #[test]
    fn encode_to_slice_leaves_excess_capacity_untouched() {
        let mut output = [0xEEu8; 16];

        let written = encode_to_slice(b"hi", &mut output);

        assert_eq!(written, Ok(4));
        assert_eq!(&output[..4], b"D1MG");
        assert!(output[4..].iter().all(|&b| b == 0xEE));
    }

    #[test]
    fn encode_to_slice_rejects_a_capacity_that_sized_encode_would_truncate_to() {
        let input = [0xabu8; 38];

        let mut output = [0xEEu8; 60];

        assert_eq!(
            encode_to_slice(&input, &mut output),
            Err(CapacityError {
                required: 61,
                available: 60,
            })
        );
        assert!(output.iter().all(|&b| b == 0xEE));

        // `sized_encode` truncates to the same capacity instead, because
        // the size is a compile-time choice by the caller.
        let fixed: [u8; 60] = sized_encode(&input);

        assert_eq!(&fixed, &encode(&input).as_bytes()[..60]);
    }

    #[test]
    fn encode_to_slice_rejects_a_capacity_short_inside_the_block_tail() {
        // 10 input bytes want 16 symbols; capacity 11 would fill one full
        // 8-symbol block and then run out mid-tail.
        let input = [0x5au8; 10];

        let mut output = [0xEEu8; 11];

        assert_eq!(
            encode_to_slice(&input, &mut output),
            Err(CapacityError {
                required: 16,
                available: 11,
            })
        );
        assert!(output.iter().all(|&b| b == 0xEE));
    }

    #[test]
    fn encode_to_slice_rejects_a_capacity_short_at_a_block_boundary() {
        // 6 input bytes want 10 symbols; capacity 8 is an exact block
        // boundary, which the block loop alone would fill happily.
        let input = [0x5au8; 6];

        let mut output = [0xEEu8; 8];

        assert_eq!(
            encode_to_slice(&input, &mut output),
            Err(CapacityError {
                required: 10,
                available: 8,
            })
        );
        assert!(output.iter().all(|&b| b == 0xEE));
    }

    #[test]
    fn encode_to_slice_rejects_every_capacity_below_the_requirement() {
        let input = [0x5au8; 21];
        let required = encoded_len(input.len());

        for available in 0..required {
            let mut output = alloc::vec![0xEEu8; available];

            assert_eq!(
                encode_to_slice(&input, &mut output),
                Err(CapacityError {
                    required,
                    available,
                }),
                "available {available}"
            );
            assert!(output.iter().all(|&b| b == 0xEE), "available {available}");
        }
    }

    #[test]
    fn encode_to_slice_zero_capacity_rejects_nonempty_input() {
        let mut output = [0u8; 0];

        assert_eq!(
            encode_to_slice(b"hello", &mut output),
            Err(CapacityError {
                required: 8,
                available: 0,
            })
        );
    }

    #[test]
    fn encode_to_slice_empty_input_writes_nothing() {
        let mut output = [0xEEu8; 4];

        assert_eq!(encode_to_slice(b"", &mut output), Ok(0));
        assert_eq!(output, [0xEE; 4]);
    }

    #[test]
    fn decode_to_slice_matches_sized_decode() {
        let input = [0xabu8; 38];

        let encoded = encode(&input);

        let mut output = [0u8; 38];
        let written = decode_to_slice(encoded.as_bytes(), &mut output);

        let fixed: [u8; 38] = sized_decode(encoded.as_bytes());

        assert_eq!(written, Ok(38));
        assert_eq!(output, fixed);
    }

    #[test]
    fn decode_to_slice_discards_trailing_partial_bits() {
        // "D1MGG" is 25 bits: three complete bytes plus one leftover bit
        // that must be discarded, matching `decode`.
        let mut output = [0u8; 8];

        let written = decode_to_slice(b"D1MGG", &mut output);
        let fixed: [u8; 4] = sized_decode(b"D1MGG");

        assert_eq!(written, Ok(3));
        assert_eq!(decoded_len(5), 3);
        assert_eq!(&output[..3], &fixed[..3]);
        assert_eq!(output[3], 0);
    }

    #[test]
    fn decode_to_slice_rejects_a_capacity_below_the_payload() {
        let encoded = encode(&[0x12, 0x34, 0x56, 0x78, 0x9a]);

        let mut output = [0xEEu8; 8];

        assert_eq!(
            decode_to_slice(encoded.as_bytes(), &mut output[..3]),
            Err(CapacityError {
                required: 5,
                available: 3,
            })
        );
        assert!(output.iter().all(|&b| b == 0xEE));
    }

    #[test]
    fn decode_to_slice_sizes_from_symbols_not_from_input_length() {
        // Sixteen bytes of input carry only four symbols, so a two-byte
        // output is exactly enough.
        let mut output = [0u8; 2];

        assert_eq!(decode_to_slice(b"-D- 1 -M-\tG-\r\n", &mut output), Ok(2));
        assert_eq!(&output, b"hi");
    }

    #[test]
    fn decode_to_slice_skips_invalid_bytes() {
        let mut output = [0u8; 2];

        let written = decode_to_slice(b" D1@-MG\n", &mut output);

        assert_eq!(written, Ok(2));
        assert_eq!(&output, b"hi");
    }

    #[test]
    fn decode_to_slice_zero_capacity_rejects_a_nonempty_payload() {
        let mut output = [0u8; 0];

        assert_eq!(
            decode_to_slice(b"D1MG", &mut output),
            Err(CapacityError {
                required: 2,
                available: 0,
            })
        );
        assert_eq!(decode_to_slice(b"----", &mut output), Ok(0));
    }

    #[test]
    fn try_decode_to_slice_matches_try_decode() {
        let mut state: u64 = 0xC0FF_EE00_C0FF_EE00;

        for length in 0..=64usize {
            let mut input = Vec::with_capacity(length);

            for _ in 0..length {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                input.push((state >> 56) as u8);
            }

            let encoded = encode(&input);

            let mut output = alloc::vec![0u8; length];
            let written = try_decode_to_slice(encoded.as_bytes(), &mut output);

            assert_eq!(written, Ok(length), "length {length}");
            assert_eq!(
                Ok(&output),
                crate::try_decode(encoded.as_bytes()).as_ref(),
                "length {length}"
            );
        }
    }

    #[test]
    fn try_decode_to_slice_reports_the_same_errors_as_try_decode() {
        let mut output = [0u8; 16];

        for input in [&b"D1 MG"[..], b"29", b"280", b"D1@MG", b"0"] {
            assert_eq!(
                try_decode_to_slice(input, &mut output).map(|_| ()),
                crate::try_decode(input).map(|_| ()),
                "input {input:?}"
            );
        }
    }

    #[test]
    fn try_decode_to_slice_reports_capacity_after_validating() {
        let mut output = [0xEEu8; 8];

        assert_eq!(
            try_decode_to_slice(b"D1MG", &mut output[..1]),
            Err(DecodeError::Capacity(CapacityError {
                required: 2,
                available: 1,
            }))
        );
        assert!(output.iter().all(|&b| b == 0xEE));

        // Validation runs first, so a malformed input is reported on its
        // own terms even when the output is also too small.
        assert_eq!(
            try_decode_to_slice(b"D1 MG", &mut output[..0]),
            Err(DecodeError::InvalidByte {
                position: 2,
                byte: b' ',
            })
        );
    }
}

mod blocks {
    use alloc::{string::String, vec::Vec};

    use crate::{decode, encode, try_decode};

    #[test]
    fn decode_block_path_survives_misaligned_pollution() {
        // Invalid bytes every three characters break the 8-symbol block
        // alignment, forcing the accumulator to span pollution.
        let payload: Vec<u8> = (0u8..=99).collect();
        let encoded = encode(&payload);

        let mut polluted = String::new();

        for (i, ch) in encoded.chars().enumerate() {
            polluted.push(ch);

            if i % 3 == 2 {
                polluted.push('@');
            }
        }

        assert_eq!(decode(polluted.as_bytes()), payload);
    }

    #[test]
    fn try_decode_block_path_survives_misaligned_hyphens() {
        let payload: Vec<u8> = (0u8..=99).collect();
        let encoded = encode(&payload);

        let mut hyphenated = String::new();

        for (i, ch) in encoded.chars().enumerate() {
            hyphenated.push(ch);

            if i % 3 == 2 {
                hyphenated.push('-');
            }
        }

        assert_eq!(try_decode(hyphenated.as_bytes()), Ok(payload));
    }

    #[test]
    fn round_trip_around_block_boundaries() {
        // Input lengths straddling multiples of five exercise both the
        // block fast path and the scalar tail on the encode side, and
        // encoded lengths around multiples of eight do the same for
        // decode.
        for length in [1usize, 4, 5, 6, 9, 10, 11, 14, 15, 16, 39, 40, 41] {
            let input: Vec<u8> = (0..length)
                .map(|i| u8::try_from((i * 37 + 11) & 0xff).expect("masked to eight bits"))
                .collect();

            let encoded = encode(&input);

            assert_eq!(
                encoded.len(),
                (length * 8).div_ceil(5),
                "encoded length for input length {length}",
            );
            assert_eq!(decode(encoded.as_bytes()), input, "length {length}");
        }
    }
}
