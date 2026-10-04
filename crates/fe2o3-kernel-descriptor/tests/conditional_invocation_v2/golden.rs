use super::*;

fn u16_at(bytes: &mut [u8], at: usize, n: u16) {
    bytes[at..at + 2].copy_from_slice(&n.to_le_bytes());
}
fn u32_at(bytes: &mut [u8], at: usize, n: u32) {
    bytes[at..at + 4].copy_from_slice(&n.to_le_bytes());
}
fn u64_at(bytes: &mut [u8], at: usize, n: u64) {
    bytes[at..at + 8].copy_from_slice(&n.to_le_bytes());
}

// Independent literal one-input/one-read grammar, not an encoder round-trip
// oracle. All reserved bytes and zero fields start at zero.
fn golden(version: u16) -> Vec<u8> {
    let fixed = if version == 1 { FIXED_V1 } else { FIXED_V2 };
    let mut b = vec![0; fixed + 32 + 176 + 80 + 224];
    b[..8].copy_from_slice(if version == 1 {
        b"FE2O3CI\0"
    } else {
        b"FE2O3C2\0"
    });
    u16_at(&mut b, 8, version);
    set_len(&mut b);
    for (at, value) in [(16, 1), (18, 1), (20, 2), (22, 1), (24, 7)] {
        u16_at(&mut b, at, value);
    }
    for (at, value) in [
        (28, 13),
        (60, 2),
        (92, 3),
        (124, 4),
        (160, 5),
        (224, 6),
        (256, 7),
        (288, 8),
    ] {
        b[at..at + 32].fill(value);
    }
    u32_at(&mut b, 156, 2);
    let statement_v1 = [
        0xba, 0x32, 0xfb, 0x0b, 0x89, 0x42, 0x36, 0xf4, 0x70, 0x30, 0x55, 0xaa, 0x93, 0x6b, 0xb2,
        0x73, 0x76, 0x50, 0xb4, 0x6d, 0xe6, 0x3b, 0x51, 0x23, 0xbf, 0x32, 0x46, 0xca, 0x1c, 0xbc,
        0x80, 0xb9,
    ];
    let statement = if version == 1 {
        statement_v1
    } else {
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/CONDITIONAL-MEMORY-TRANSITION/V2/LE/SHARED-IEEE/CPU-SOURCE\0");
        for value in [3u8, 9, 12, 14, 15, 16, 17] {
            hash.update([value; 32]);
        }
        hash.finalize().into()
    };
    b[320..352].copy_from_slice(&statement);
    for (at, value) in [
        (352, 9),
        (384, 10),
        (416, 11),
        (448, 12),
        (480, 14),
        (512, 15),
        (544, 16),
    ] {
        b[at..at + 32].fill(value);
    }
    if version == 2 {
        b[576..608].fill(17);
    }
    let output = fixed - 48;
    u16_at(&mut b, output, 1);
    u32_at(&mut b, output + 4, 42);
    u64_at(&mut b, output + 8, 10000);
    u32_at(&mut b, output + 20, 10000);
    u32_at(&mut b, output + 28, 10001);
    u64_at(&mut b, output + 32, 4);
    u32_at(&mut b, output + 40, 4);
    for (i, value) in [11, 22, 33, 44].into_iter().enumerate() {
        u64_at(&mut b, fixed + i * 8, value);
    }
    let args = fixed + 32;
    for i in 0..2 {
        let at = args + i * 88;
        for (offset, value) in [
            (0, 10 + 3 * i),
            (4, i),
            (8, 100 + i),
            (12, 1000 + i),
            (16, 7),
        ] {
            u32_at(&mut b, at + offset, value as u32);
        }
        u16_at(&mut b, at + 20, i as u16);
        b[at + 22] = i as u8;
        b[at + 24..at + 56].fill(21);
        b[at + 56..at + 88].fill(22);
    }
    let read = args + 176;
    for (offset, value) in [
        (4, 42),
        (16, 1),
        (20, 2),
        (24, 3),
        (28, 4),
        (52, 2),
        (56, 99),
        (76, 4),
    ] {
        u32_at(&mut b, read + offset, value);
    }
    b[read + 65] = 1;
    u64_at(&mut b, read + 68, 4);
    let premises = read + 80;
    for (i, (tag, domain, a, other, width, align)) in [
        (0, 0, 0, 0, 0, 0),
        (1, 0, 13, 0, 0, 0),
        (2, 0, 13, 0, 0, 0),
        (5, 0, 13, 0, 4, 4),
        (3, 0, 10, 0, 0, 0),
        (4, 0, 10, 13, 0, 0),
        (5, 1, 10, 0, 4, 4),
    ]
    .into_iter()
    .enumerate()
    {
        let at = premises + i * 32;
        b[at] = tag;
        b[at + 1] = domain;
        u32_at(&mut b, at + 4, a);
        u32_at(&mut b, at + 8, other);
        u64_at(&mut b, at + 12, width);
        u32_at(&mut b, at + 20, align);
    }
    b
}

#[test]
fn v1_golden_bytes_hashes_and_charge_trace_remain_exact() {
    let f = Fixture::new(1, 1);
    let expected = golden(1);
    assert_eq!(expected.len(), 1136);
    let mut output = vec![0xa5; 1136];
    let mut charges = Vec::new();
    encode_conditional_invocation_contract_v1(&f.input(), &mut output, &mut |n| {
        charges.push(n);
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(output, expected);
    assert_eq!(charges, [1, 294912, 2273]);
    charges.clear();
    let v = decode_conditional_invocation_contract_v1(&expected, &mut |n| {
        charges.push(n);
        Ok::<(), ()>(())
    })
    .unwrap();
    assert_eq!(charges, [1, 294912]);
    assert_eq!(v.theorem(), &f.theorem);
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/CONDITIONAL-INVOCATION/V1\0");
    hash.update(1136u64.to_le_bytes());
    hash.update(&expected);
    let digest: [u8; 32] = hash.finalize().into();
    assert_eq!(v.identity().as_bytes(), &digest);
}

#[test]
fn v2_golden_is_exactly_new_framing_statement_and_one_digest() {
    let f = Fixture::new(1, 1);
    let v1 = golden(1);
    let expected = golden(2);
    assert_eq!(expected.len(), 1168);
    assert_eq!(wire(&f), expected);
    assert_eq!(&expected[16..320], &v1[16..320]);
    assert_eq!(&expected[352..576], &v1[352..576]);
    assert_eq!(&expected[576..608], &[17; 32]);
    assert_eq!(&expected[608..], &v1[576..]);
    assert_ne!(&expected[320..352], &v1[320..352]);
    let v = decode_conditional_invocation_contract_v2(&expected, &mut free).unwrap();
    assert_eq!(v.theorem().statement_identity, statement(&f, [17; 32]));
}

#[test]
fn v1_view_and_resource_extents_match_the_original_public_shape() {
    // Exact pre-refactor fields. Rust layout is not a wire format; this guards
    // the caller's existing size_of-based retained-storage contract.
    #[allow(dead_code)]
    struct LegacyView<'a> {
        bytes: &'a [u8],
        identity: ConditionalInvocationIdentityV1,
        subjects: ConditionalSubjectsV1,
        theorem: ConditionalTheoremV1,
        output: ConditionalOutputV1,
        counts: [usize; 4],
        starts: [usize; 4],
    }
    assert_eq!(
        CONDITIONAL_INVOCATION_VIEW_STORAGE_V1,
        size_of::<LegacyView<'static>>()
    );
    assert_eq!(
        CONDITIONAL_INVOCATION_VIEW_STORAGE_V2,
        CONDITIONAL_INVOCATION_VIEW_STORAGE_V1 + 32
    );
    assert_eq!(
        CONDITIONAL_INVOCATION_QUERY_STORAGE_V2,
        CONDITIONAL_INVOCATION_QUERY_STORAGE_V1
    );
    assert_eq!(
        CONDITIONAL_INVOCATION_CODEC_STORAGE_V1,
        size_of::<LegacyView<'static>>()
            + CONDITIONAL_INVOCATION_QUERY_STORAGE_V1
            + size_of::<Sha256>()
            + size_of::<ConditionalInvocationContractInputV1<'static>>()
            + 512
    );
    assert_eq!(
        CONDITIONAL_INVOCATION_CODEC_STORAGE_V2,
        CONDITIONAL_INVOCATION_CODEC_STORAGE_V1 + 64
    );
}
