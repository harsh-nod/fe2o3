use fe2o3_differential::{
    BinaryOp, CodecError, Expr, KernelCase, MAX_CANONICAL_BYTES, Program, decode_case_v1,
    encode_case_v1,
};

fn fixture() -> KernelCase {
    let expression = Expr::Binary {
        op: BinaryOp::Add,
        left: Box::new(Expr::Load { input: 0 }),
        right: Box::new(Expr::Const(3)),
    };
    KernelCase::new(
        0x0102_0304_0506_0708,
        Program::new(1, 2, expression).unwrap(),
        vec![vec![7, -1]],
    )
    .unwrap()
}

#[test]
fn v1_encoding_has_stable_golden_bytes() {
    let expected = vec![
        0x46, 0x32, 0x44, 0x46, 0x01, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x02, 0x00,
        0x01, 0x02, 0x00, 0x07, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0x04, 0x00, 0x02, 0x00,
        0x00, 0x03, 0x00, 0x00, 0x00,
    ];
    assert_eq!(encode_case_v1(&fixture()).unwrap(), expected);
    assert_eq!(decode_case_v1(&expected).unwrap(), fixture());
}

#[test]
fn every_truncation_and_trailing_data_is_rejected() {
    let bytes = encode_case_v1(&fixture()).unwrap();
    for end in 0..bytes.len() {
        assert!(
            decode_case_v1(&bytes[..end]).is_err(),
            "accepted prefix {end}"
        );
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(decode_case_v1(&trailing), Err(CodecError::TrailingBytes));
}

#[test]
fn malformed_headers_and_tags_fail_closed() {
    let bytes = encode_case_v1(&fixture()).unwrap();

    let mut bad_magic = bytes.clone();
    bad_magic[0] = 0;
    assert_eq!(decode_case_v1(&bad_magic), Err(CodecError::BadMagic));

    let mut bad_version = bytes.clone();
    bad_version[4] = 2;
    assert_eq!(
        decode_case_v1(&bad_version),
        Err(CodecError::UnsupportedVersion { actual: 2 })
    );

    let mut bad_binary = bytes;
    bad_binary[27] = 99;
    assert_eq!(
        decode_case_v1(&bad_binary),
        Err(CodecError::InvalidBinaryTag { actual: 99 })
    );
}

#[test]
fn excessive_depth_and_byte_size_are_rejected_before_unbounded_work() {
    let mut deep = Vec::new();
    deep.extend_from_slice(b"F2DF");
    deep.push(1);
    deep.extend_from_slice(&0_u64.to_le_bytes());
    deep.extend_from_slice(&1_u16.to_le_bytes());
    deep.push(0);
    for _ in 0..13 {
        deep.extend_from_slice(&[3, 0]);
    }
    deep.extend_from_slice(&[0, 0, 0, 0, 0]);
    assert_eq!(decode_case_v1(&deep), Err(CodecError::ExpressionTooDeep));

    assert_eq!(
        decode_case_v1(&vec![0; MAX_CANONICAL_BYTES + 1]),
        Err(CodecError::TooLarge)
    );
}

#[test]
fn arbitrary_bounded_bytes_never_panic() {
    let mut state = 7_u64;
    for length in 0..512 {
        let mut bytes = vec![0; length];
        for byte in &mut bytes {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            *byte = (state >> 32) as u8;
        }
        let _ = decode_case_v1(&bytes);
    }
}
