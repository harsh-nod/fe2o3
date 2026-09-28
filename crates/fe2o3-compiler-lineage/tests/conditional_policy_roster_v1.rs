//! Public synthetic policies exercise inert framing, never production authority.
use fe2o3_compiler_lineage::*;
use sha2::{Digest, Sha256};
use std::panic::{AssertUnwindSafe, catch_unwind};

const LIMIT: usize = MAX_NATIVE_CONDITIONAL_STORAGE_V1;
const HEADER: usize = NATIVE_CONDITIONAL_POLICY_ROSTER_HEADER_BYTES_V1;
const ROW: usize = NATIVE_CONDITIONAL_POLICY_ROOT_HEADER_BYTES_V1;
type Error = NativeConditionalPolicyRosterErrorV1<u8>;

fn root(id: u32, signers: &[[u8; 32]]) -> NativeConditionalPolicyRootInputV1<'_> {
    NativeConditionalPolicyRootInputV1 {
        semantic_root: id,
        kernel_binding: [1; 32],
        effect_signers: signers,
        effect_toolchain: [[2; 32], [3; 32], [4; 32], [5; 32], [6; 32]],
        formula_verifying_key: [7; 32],
        formula_toolchain: [[8; 32], [9; 32], [10; 32], [11; 32], [12; 32]],
        formula_boundary: 3,
    }
}
fn input<'a>(
    roots: &'a [NativeConditionalPolicyRootInputV1<'a>],
) -> NativeConditionalPolicyRosterInputV1<'a> {
    NativeConditionalPolicyRosterInputV1 {
        source_packet: b"exact source packet",
        roots,
    }
}
fn encode(roots: &[NativeConditionalPolicyRootInputV1<'_>]) -> Result<Vec<u8>, Error> {
    encode_native_conditional_policy_roster_v1(input(roots), LIMIT, |_| Ok(()))
}
fn read(bytes: &[u8]) -> Result<NativeConditionalPolicyRosterRefV1<'_>, Error> {
    read_native_conditional_policy_roster_v1(bytes, LIMIT, |_| Ok(()))
}
fn reseal(bytes: &mut [u8]) {
    let end = bytes.len() - 32;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/NATIVE-CONDITIONAL-POLICY-ROSTER/V1\0");
    hash.update((end as u64).to_le_bytes());
    hash.update(&bytes[..end]);
    bytes[end..].copy_from_slice(&hash.finalize());
}

#[test]
fn source_order_raw_source_binding_borrowed_rows_and_determinism() {
    let roots = [root(9, &[[20; 32], [21; 32]]), root(4, &[[22; 32]])];
    let bytes = encode(&roots).unwrap();
    assert_eq!(bytes, encode(&roots).unwrap());
    assert_eq!(bytes.capacity(), bytes.len());
    assert_eq!(bytes.len(), HEADER + 2 * ROW + 3 * 32 + 32);
    assert_eq!(&bytes[..16], b"F2NPR1\0\0\x01\0\x01\0\x50\0\0\0");
    let frame = read(&bytes).unwrap();
    assert_eq!(frame.canonical_bytes().as_ptr(), bytes.as_ptr());
    assert_eq!(
        frame.source_packet_len(),
        input(&roots).source_packet.len() as u64
    );
    assert_eq!(
        *frame.source_packet_sha256(),
        <[u8; 32]>::from(Sha256::digest(input(&roots).source_packet))
    );
    assert_eq!(frame.root_count(), 2);
    assert_eq!(frame.identity().byte_len(), bytes.len() as u64);
    assert_eq!(frame.identity().sha256(), &bytes[bytes.len() - 32..]);
    assert!(!frame.grants_authority());
    let mut rows = frame.roots();
    assert_eq!(rows.len(), 2);
    for (actual, expected) in rows.by_ref().zip(roots) {
        assert_eq!(actual.semantic_root(), expected.semantic_root);
        assert_eq!(actual.kernel_binding(), &expected.kernel_binding);
        assert_eq!(actual.effect_signers(), expected.effect_signers);
        assert_eq!(actual.effect_signer_count(), expected.effect_signers.len());
        assert_eq!(actual.effect_toolchain(), &expected.effect_toolchain);
        assert_eq!(
            actual.formula_verifying_key(),
            &expected.formula_verifying_key
        );
        assert_eq!(actual.formula_toolchain(), &expected.formula_toolchain);
        assert_eq!(actual.formula_boundary(), 3);
    }
    assert_eq!(rows.len(), 0);
    let first = frame.roots().next().unwrap();
    assert_eq!(
        first.effect_signers().as_ptr().cast::<u8>(),
        bytes[HEADER + ROW..].as_ptr()
    );
    assert_eq!(
        first.effect_toolchain().as_ptr().cast::<u8>(),
        bytes[HEADER + 48..].as_ptr()
    );
}

#[test]
fn independent_exact_root_signer_and_source_bounds() {
    assert_eq!(MAX_NATIVE_CONDITIONAL_POLICY_ROSTER_ROOTS_V1, 128);
    assert_eq!(
        MAX_NATIVE_CONDITIONAL_POLICY_ROSTER_ROOTS_V1,
        MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3
    );
    assert_eq!(MAX_NATIVE_CONDITIONAL_POLICY_SIGNERS_PER_ROOT_V1, 4096);
    assert_eq!(encode(&[]), Err(Error::RootCount));
    assert_eq!(encode(&[root(9, &[])]), Err(Error::SignerCount));
    let mut roots: Vec<_> = (0..128).rev().map(|id| root(id, &[[20; 32]])).collect();
    assert_eq!(read(&encode(&roots).unwrap()).unwrap().root_count(), 128);
    roots.push(root(128, &[[20; 32]]));
    assert_eq!(encode(&roots), Err(Error::RootCount));
    let signers: Vec<_> = (1_u32..=4096)
        .map(|id| {
            let mut signer = [0; 32];
            signer[..4].copy_from_slice(&id.to_be_bytes());
            signer
        })
        .collect();
    let full = [root(u32::MAX, &signers)];
    let bytes = encode(&full).unwrap();
    assert_eq!(
        read(&bytes)
            .unwrap()
            .roots()
            .next()
            .unwrap()
            .effect_signer_count(),
        4096
    );
    let all_max: Vec<_> = (0..128).map(|id| root(id, &signers)).collect();
    let layout =
        NativeConditionalPolicyRosterLayoutV1::new(input(&all_max), |_| Ok::<_, u8>(())).unwrap();
    assert_eq!(
        layout.encoded_len(),
        MAX_NATIVE_CONDITIONAL_POLICY_ROSTER_BYTES_V1
    );
    assert_eq!(layout.encoded_len(), 16_828_528);
    assert!(layout.encoded_len() < MAX_NATIVE_CONDITIONAL_STORAGE_V1);
    let mut maximum = encode(&all_max).unwrap();
    assert_eq!(maximum.len(), layout.encoded_len());
    assert_eq!(read(&maximum).unwrap().root_count(), 128);
    maximum.push(0);
    let mut charged = false;
    assert!(matches!(
        read_native_conditional_policy_roster_v1(&maximum, LIMIT, |_| {
            charged = true;
            Ok::<_, u8>(())
        }),
        Err(Error::Length)
    ));
    assert!(!charged);
    let too_many = vec![[1; 32]; 4097];
    assert_eq!(encode(&[root(1, &too_many)]), Err(Error::SignerCount));
    let source = vec![1; MAX_NATIVE_REFINED_FORWARDING_SOURCE_BYTES_V1 + 1];
    for (len, valid) in [
        (0, false),
        (1, true),
        (MAX_NATIVE_REFINED_FORWARDING_SOURCE_BYTES_V1, true),
        (source.len(), false),
    ] {
        let result = encode_native_conditional_policy_roster_v1(
            NativeConditionalPolicyRosterInputV1 {
                source_packet: &source[..len],
                roots: &full,
            },
            LIMIT,
            |_| Ok::<_, u8>(()),
        );
        assert_eq!(result.is_ok(), valid);
        if valid {
            assert_eq!(
                read(&result.unwrap()).unwrap().source_packet_len(),
                len as u64
            );
        }
    }
    assert!(matches!(
        read_native_conditional_policy_roster_v1(&bytes, LIMIT + 1, |_| Ok::<_, u8>(())),
        Err(Error::StorageLimit)
    ));
}

#[test]
fn canonical_identity_signer_and_boundary_checks_do_not_sort_roots() {
    assert_eq!(
        encode(&[root(9, &[[20; 32]]), root(9, &[[20; 32]])]),
        Err(Error::DuplicateRoot)
    );
    for signers in [&[[2; 32], [1; 32]][..], &[[1; 32], [1; 32]][..]] {
        assert_eq!(encode(&[root(1, signers)]), Err(Error::SignerOrder));
    }
    assert_eq!(encode(&[root(1, &[[0; 32]])]), Err(Error::ZeroIdentity));
    for boundary in 0..=4 {
        let mut row = root(9, &[[20; 32]]);
        row.formula_boundary = boundary;
        assert_eq!(encode(&[row]).is_ok(), (1..=3).contains(&boundary));
    }
    // Nonzero weak keys remain inert bytes; the verifier performs key admission.
    let mut weak = root(9, &[[20; 32]]);
    weak.formula_verifying_key = [0; 32];
    weak.formula_verifying_key[0] = 1;
    assert!(read(&encode(&[weak]).unwrap()).is_ok());
}

#[test]
fn truncations_and_resealed_noncanonical_rows_and_headers_are_rejected() {
    let roots = [root(9, &[[20; 32], [21; 32]]), root(4, &[[22; 32]])];
    let original = encode(&roots).unwrap();
    for end in 0..original.len() {
        assert!(read(&original[..end]).is_err());
    }
    let mut extended = original.clone();
    extended.push(0);
    assert!(read(&extended).is_err());
    for at in [0, 8, 10, 12, 16, 68, 79, HEADER + 9, HEADER + 15] {
        let mut bytes = original.clone();
        bytes[at] ^= 1;
        reseal(&mut bytes);
        assert!(read(&bytes).is_err(), "offset {at}");
    }
    for count in [0_u32, 1, 3, 129, u32::MAX] {
        let mut bytes = original.clone();
        bytes[64..68].copy_from_slice(&count.to_le_bytes());
        reseal(&mut bytes);
        assert!(read(&bytes).is_err());
    }
    for source_len in [
        0_u64,
        MAX_NATIVE_REFINED_FORWARDING_SOURCE_BYTES_V1 as u64 + 1,
        u64::MAX,
    ] {
        let mut bytes = original.clone();
        bytes[24..32].copy_from_slice(&source_len.to_le_bytes());
        reseal(&mut bytes);
        assert!(read(&bytes).is_err());
    }
    for at in (HEADER + 16..HEADER + ROW)
        .step_by(32)
        .chain([HEADER + ROW])
    {
        let mut bytes = original.clone();
        bytes[at..at + 32].fill(0);
        reseal(&mut bytes);
        assert!(matches!(read(&bytes), Err(Error::ZeroIdentity)));
    }
    for boundary in [0, 4, 255] {
        let mut bytes = original.clone();
        bytes[HEADER + 8] = boundary;
        reseal(&mut bytes);
        assert!(matches!(read(&bytes), Err(Error::FormulaBoundary)));
    }
    for signer_count in [0_u32, 1, 3, 4097, u32::MAX] {
        let mut bytes = original.clone();
        bytes[HEADER + 4..HEADER + 8].copy_from_slice(&signer_count.to_le_bytes());
        reseal(&mut bytes);
        assert!(read(&bytes).is_err());
    }
    let mut duplicate = original.clone();
    let second = HEADER + ROW + 64;
    duplicate[second..second + 4].copy_from_slice(&9_u32.to_le_bytes());
    reseal(&mut duplicate);
    assert!(matches!(read(&duplicate), Err(Error::DuplicateRoot)));
    let mut unordered = original.clone();
    unordered[HEADER + ROW..HEADER + ROW + 32].fill(22);
    reseal(&mut unordered);
    assert!(matches!(read(&unordered), Err(Error::SignerOrder)));
    let mut digest = original;
    *digest.last_mut().unwrap() ^= 1;
    assert!(matches!(read(&digest), Err(Error::Identity)));
}

#[test]
fn charges_precede_walks_and_sealing_never_mutates_on_failure() {
    let roots = [root(9, &[[20; 32], [21; 32]]), root(4, &[[22; 32]])];
    let original = encode(&roots).unwrap();
    let layout =
        NativeConditionalPolicyRosterLayoutV1::new(input(&roots), |_| Ok::<_, u8>(())).unwrap();
    let mut costs = Vec::new();
    read_native_conditional_policy_roster_v1(&original, LIMIT, |n| {
        costs.push(n);
        Ok::<_, u8>(())
    })
    .unwrap();
    for fail in 0..costs.len() {
        let mut call = 0;
        let result = read_native_conditional_policy_roster_v1(&original, LIMIT, |_| {
            let refused = call == fail;
            call += 1;
            if refused { Err(37_u8) } else { Ok(()) }
        });
        assert!(matches!(result, Err(Error::Charge(37))));
        assert_eq!(call, fail + 1);
    }
    let total: usize = costs.iter().sum();
    for limit in [total - 1, total] {
        let mut remaining = limit;
        let result = read_native_conditional_policy_roster_v1(&original, LIMIT, |n| {
            remaining = remaining.checked_sub(n).ok_or(37_u8)?;
            Ok::<_, u8>(())
        });
        assert_eq!(result.is_ok(), limit == total);
    }
    let mut paid = 0;
    let mut bytes = original.clone();
    seal_native_conditional_policy_roster_v1(
        layout,
        input(&roots).source_packet,
        &mut bytes,
        LIMIT,
        |n| {
            paid += n;
            Ok::<_, u8>(())
        },
    )
    .unwrap();
    for limit in [paid - 1, paid] {
        let mut candidate = original.clone();
        candidate[..HEADER].fill(19);
        let before = candidate.clone();
        let result = seal_native_conditional_policy_roster_v1(
            layout,
            input(&roots).source_packet,
            &mut candidate,
            LIMIT,
            |n| {
                if n > limit { Err(37_u8) } else { Ok(()) }
            },
        );
        if limit < paid {
            assert_eq!(result, Err(Error::Charge(37)));
            assert_eq!(candidate, before);
        } else {
            assert!(result.is_ok());
            assert_eq!(candidate, original);
        }
    }
    let mut bad = original.clone();
    bad[HEADER + 8] = 0;
    let before = bad.clone();
    let mut call = 0;
    assert!(matches!(
        read_native_conditional_policy_roster_v1(&bad, LIMIT, |_| {
            call += 1;
            if call == 2 { Err(37_u8) } else { Ok(()) }
        }),
        Err(Error::Charge(37))
    ));
    assert_eq!(call, 2);
    assert!(matches!(
        read_native_conditional_policy_roster_v1(&bad, LIMIT, |_| Err(37_u8)),
        Err(Error::Charge(37))
    ));
    assert_eq!(
        seal_native_conditional_policy_roster_v1(
            layout,
            input(&roots).source_packet,
            &mut bad,
            LIMIT,
            |_| Ok::<_, u8>(())
        ),
        Err(Error::FormulaBoundary)
    );
    assert_eq!(bad, before);
    assert_eq!(
        seal_native_conditional_policy_roster_v1(
            layout,
            b"wrong length",
            &mut bad,
            LIMIT,
            |_| Ok::<_, u8>(())
        ),
        Err(Error::SourceLength)
    );
    assert_eq!(bad, before);
    let mut encoded_work = 0;
    encode_native_conditional_policy_roster_v1(input(&roots), LIMIT, |n| {
        encoded_work += n;
        Ok::<_, u8>(())
    })
    .unwrap();
    for limit in [encoded_work - 1, encoded_work] {
        let mut remaining = limit;
        let result = encode_native_conditional_policy_roster_v1(input(&roots), LIMIT, |n| {
            remaining = remaining.checked_sub(n).ok_or(37_u8)?;
            Ok::<_, u8>(())
        });
        assert_eq!(result.is_ok(), limit == encoded_work);
    }
    assert_eq!(encode(&roots).unwrap(), original);
}

#[test]
fn callback_panics_and_callback_drop_precede_destination_mutation() {
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic!("callback destructor");
        }
    }
    let roots = [root(9, &[[20; 32]])];
    let original = encode(&roots).unwrap();
    let layout =
        NativeConditionalPolicyRosterLayoutV1::new(input(&roots), |_| Ok::<_, u8>(())).unwrap();
    for drop_panic in [false, true] {
        let mut bytes = original.clone();
        bytes[..HEADER].fill(33);
        let before = bytes.clone();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            if drop_panic {
                let bomb = Bomb;
                seal_native_conditional_policy_roster_v1(
                    layout,
                    input(&roots).source_packet,
                    &mut bytes,
                    LIMIT,
                    move |_| {
                        let _capture = &bomb;
                        Ok::<_, u8>(())
                    },
                )
                .unwrap();
            } else {
                seal_native_conditional_policy_roster_v1(
                    layout,
                    input(&roots).source_packet,
                    &mut bytes,
                    LIMIT,
                    |_| -> Result<(), u8> { panic!("charge panic") },
                )
                .unwrap();
            }
        }));
        assert!(outcome.is_err());
        assert_eq!(bytes, before);
    }
}
