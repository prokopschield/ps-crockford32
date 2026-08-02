//! Treats the input as a payload and checks that every encoder agrees and
//! that every decoder recovers it.

#![no_main]

use libfuzzer_sys::fuzz_target;
use ps_crockford32::{
    check_digit, check_symbol, decode, decode_to_slice, decode_with_check, decoded_len, encode,
    encode_into, encode_to_slice, encode_with_check, encoded_len, sized_decode, sized_encode,
    try_decode, try_decode_to_slice, CapacityError,
};

fuzz_target!(|payload: &[u8]| {
    let encoded = encode(payload);

    assert_eq!(encoded.len(), encoded_len(payload.len()));
    assert!(encoded.bytes().all(|b| ps_crockford32::ALPHABET.contains(&b)));

    // `encode_into` must produce the same string.
    let mut sink = String::new();

    encode_into(payload, &mut sink).expect("a String sink never fails");

    assert_eq!(sink, encoded);

    // `encode_to_slice` must fill an exact-size buffer identically, and
    // refuse every smaller one.
    let mut buffer = vec![0u8; encoded.len()];

    assert_eq!(encode_to_slice(payload, &mut buffer), Ok(encoded.len()));
    assert_eq!(buffer, encoded.as_bytes());

    if let Some(available) = encoded.len().checked_sub(1) {
        assert_eq!(
            encode_to_slice(payload, &mut buffer[..available]),
            Err(CapacityError {
                required: encoded.len(),
                available,
            })
        );
    }

    // `sized_encode` at the exact length must agree too.
    let fixed: [u8; 64] = sized_encode(payload);
    let shared = fixed.len().min(encoded.len());

    assert_eq!(&fixed[..shared], &encoded.as_bytes()[..shared]);

    // Every decoder must recover the payload.
    assert_eq!(decode(encoded.as_bytes()), payload);
    assert_eq!(try_decode(encoded.as_bytes()).as_deref(), Ok(payload));

    let mut decoded = vec![0u8; payload.len()];

    assert_eq!(
        decode_to_slice(encoded.as_bytes(), &mut decoded),
        Ok(payload.len())
    );
    assert_eq!(decoded, payload);
    assert_eq!(decoded_len(encoded.len()), payload.len());

    decoded.fill(0);

    assert_eq!(
        try_decode_to_slice(encoded.as_bytes(), &mut decoded),
        Ok(payload.len())
    );
    assert_eq!(decoded, payload);

    let fixed: [u8; 64] = sized_decode(encoded.as_bytes());
    let shared = fixed.len().min(payload.len());

    assert_eq!(&fixed[..shared], &payload[..shared]);

    // Hyphens are insignificant separators, so scattering them must not
    // change what any decoder returns.
    let mut hyphenated = Vec::with_capacity(encoded.len() * 2 + 1);

    for (index, byte) in encoded.bytes().enumerate() {
        if index % 3 == 0 {
            hyphenated.push(b'-');
        }

        hyphenated.push(byte);
    }

    hyphenated.push(b'-');

    assert_eq!(decode(&hyphenated), payload);
    assert_eq!(try_decode(&hyphenated).as_deref(), Ok(payload));

    // The check symbol must append exactly one character and verify.
    let checked = encode_with_check(payload);

    assert_eq!(checked.len(), encoded.len() + 1);
    assert!(checked.starts_with(encoded.as_str()));
    assert_eq!(checked.as_bytes()[encoded.len()], check_symbol(payload));
    assert!(check_digit(payload) < 37);
    assert_eq!(decode_with_check(checked.as_bytes()).as_deref(), Ok(payload));

    // Trailing whitespace must not obscure the check symbol.
    let mut padded = checked.into_bytes();
    padded.extend_from_slice(b" \r\n\t-");

    assert_eq!(decode_with_check(&padded).as_deref(), Ok(payload));
});
