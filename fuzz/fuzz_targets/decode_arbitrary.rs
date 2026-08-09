//! Treats the input as an untrusted encoded string: nothing may panic, and
//! whatever the strict decoders accept must be canonical.

#![no_main]

use libfuzzer_sys::fuzz_target;
use ps_crockford32::{
    check_digit, check_symbol, decode, decode_to_slice, decode_with_check, decoded_len, encode,
    encode_into, encode_to_slice, sized_decode, sized_encode, try_decode, try_decode_to_slice,
    CapacityError, DecodeError, ALPHABET, DECODE_MAP, INVALID,
};

/// The input with hyphens removed and every remaining symbol folded to its
/// canonical uppercase spelling, which is what a strict decoder may
/// legitimately differ from the input by.
fn normalize(input: &[u8]) -> Option<Vec<u8>> {
    input
        .iter()
        .filter(|&&byte| byte != b'-')
        .map(|&byte| match DECODE_MAP[byte as usize] {
            INVALID => None,
            value => Some(ALPHABET[value as usize]),
        })
        .collect()
}

fuzz_target!(|data: &[u8]| {
    // Nothing below may panic, whatever the input.
    let lenient = decode(data);
    let strict = try_decode(data);

    let _: [u8; 33] = sized_decode(data);
    let _: [u8; 33] = sized_encode(data);
    let _ = check_digit(data);
    let _ = check_symbol(data);
    let _ = decode_with_check(data);

    let mut buffer = [0u8; 37];

    let _ = decode_to_slice(data, &mut buffer);
    let _ = try_decode_to_slice(data, &mut buffer);
    let _ = encode_to_slice(data, &mut buffer);
    let _ = encode_into(data, &mut String::new());

    // Whatever the strict decoder accepts must re-encode to the input,
    // modulo hyphens and the ambiguous-glyph aliases.
    match &strict {
        Ok(payload) => {
            assert_eq!(*payload, lenient, "strict and lenient decoders disagree");
            assert_eq!(
                encode(payload).as_bytes(),
                normalize(data)
                    .expect("an accepted input contains only symbols and hyphens")
                    .as_slice(),
                "accepted a non-canonical input"
            );
        }
        Err(DecodeError::InvalidByte { position, byte }) => {
            assert_eq!(data[*position], *byte);
            assert!(*byte != b'-' && DECODE_MAP[*byte as usize] == INVALID);
        }
        Err(DecodeError::NonZeroPadding { position, byte } | DecodeError::ExcessSymbol {
            position,
            byte,
        }) => {
            assert_eq!(data[*position], *byte);
            assert_ne!(
                normalize(data).as_deref(),
                Some(encode(&lenient).as_bytes()),
                "rejected a canonical input"
            );
        }
        Err(DecodeError::Capacity(_)) => panic!("try_decode never reports a capacity error"),
        Err(error) => panic!("try_decode reported an unexpected error: {error:?}"),
    }

    // The two strict decoders must reach the same verdict, and the two
    // lenient ones must write the same bytes.
    let mut output = vec![0u8; lenient.len()];

    assert_eq!(
        try_decode_to_slice(data, &mut output).map(|written| written == lenient.len()),
        strict.as_ref().map(|_| true).map_err(|error| *error),
    );

    if strict.is_ok() {
        assert_eq!(output, lenient);
    }

    output.fill(0);

    assert_eq!(decode_to_slice(data, &mut output), Ok(lenient.len()));
    assert_eq!(output, lenient);
    assert!(decoded_len(data.len()) >= lenient.len());

    if let Some(available) = lenient.len().checked_sub(1) {
        assert_eq!(
            decode_to_slice(data, &mut output[..available]),
            Err(CapacityError {
                required: lenient.len(),
                available,
            })
        );
    }
});
