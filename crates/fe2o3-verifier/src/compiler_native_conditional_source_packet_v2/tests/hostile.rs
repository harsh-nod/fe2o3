use super::*;

fn u32_at(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}
fn skip_blob(bytes: &[u8], at: &mut usize) {
    *at += 4 + u32_at(bytes, *at);
}
fn set_count(bytes: &mut [u8], at: usize, count: u32) {
    bytes[at..at + 4].copy_from_slice(&count.to_le_bytes());
}
fn fix_length(bytes: &mut [u8]) {
    let n = bytes.len() as u32;
    set_count(bytes, 12, n);
}
struct Offsets {
    order: usize,
    roots: usize,
    option: usize,
    staging: usize,
    evidence: usize,
    name: usize,
}
fn offsets(bytes: &[u8]) -> Offsets {
    let mut at = 17;
    skip_blob(bytes, &mut at);
    skip_blob(bytes, &mut at);
    let order = at;
    at += 4 + 4 * u32_at(bytes, at);
    let roots = at;
    at += 4 + 4 + 1;
    let name = at + 4;
    skip_blob(bytes, &mut at);
    at += 32 + 1;
    let option = at;
    at += 1;
    if bytes[option] == 1 {
        at += 12;
    }
    at += 12;
    for _ in 0..5 {
        skip_blob(bytes, &mut at);
    }
    let staging = at;
    at += 4 + u32_at(bytes, at) * EFFECT_BYTES;
    Offsets {
        order,
        roots,
        option,
        staging,
        evidence: at,
        name,
    }
}

#[test]
fn every_truncation_is_rejected_even_with_repaired_total_length() {
    let bytes = encoded();
    for length in 0..bytes.len() {
        let mut cut = bytes[..length].to_vec();
        if length >= 16 {
            fix_length(&mut cut);
        }
        assert!(decode_only(&cut).is_err(), "accepted length {length}");
    }
}

#[test]
fn magic_versions_flags_route_options_and_utf8_are_closed() {
    let bytes = encoded();
    let p = offsets(&bytes);
    for (at, value) in [
        (0, 0),
        (7, 1),
        (8, 1),
        (9, 1),
        (10, 1),
        (11, 1),
        (16, 0),
        (16, 1),
        (16, 2),
        (16, 4),
        (p.option, 2),
        (p.option, 255),
        (p.evidence + 4, 1),
        (p.evidence + 5, 1),
        (p.evidence + 6, 1),
        (p.evidence + 7, 1),
        (p.name, 255),
    ] {
        let mut bad = bytes.clone();
        bad[at] = value;
        assert!(decode_only(&bad).is_err(), "accepted offset {at}");
    }
}

#[test]
fn counts_lengths_permutations_and_trailing_bytes_are_rejected() {
    let bytes = encoded();
    let p = offsets(&bytes);
    for (at, value) in [
        (12, 0),
        (12, u32::MAX),
        (17, u32::MAX),
        (p.order, 0),
        (p.order, 1),
        (p.order, u32::MAX),
        (p.order + 4, 2),
        (p.order + 4, 0),
        (p.roots, 0),
        (p.roots, 1),
        (p.roots, u32::MAX),
        (p.staging, 0),
        (p.staging, u32::MAX),
        (p.evidence, 0),
        (p.evidence, (EVIDENCE_BYTES - 1) as u32),
        (p.evidence, (EVIDENCE_BYTES + 1) as u32),
    ] {
        let mut bad = bytes.clone();
        set_count(&mut bad, at, value);
        assert!(
            decode_only(&bad).is_err(),
            "accepted offset {at} value {value}"
        );
    }
    let mut bad = bytes;
    bad.push(0);
    fix_length(&mut bad);
    assert!(decode_only(&bad).is_err());
}

#[test]
fn encoder_rejects_incomplete_permutation_and_staging_rosters() {
    fixture(|input| {
        for order in [&[][..], &[0][..], &[0, 0][..], &[0, 2][..]] {
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            assert!(
                encode_native_conditional_source_packet_v2(
                    NativeConditionalSourcePacketInputV2 {
                        canonical_kernel_order: order,
                        ..input
                    },
                    &mut budget
                )
                .is_err()
            );
            assert_eq!(budget.storage(), 0);
        }
        for omit_commitments in [false, true] {
            let mut roots = input.roots.to_vec();
            if omit_commitments {
                roots[0].staging_commitments = &[];
            } else {
                let receipts = roots[0].effect_receipts;
                roots[0].effect_receipts = &receipts[..1];
            }
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            assert!(
                encode_native_conditional_source_packet_v2(
                    NativeConditionalSourcePacketInputV2 {
                        roots: &roots,
                        ..input
                    },
                    &mut budget
                )
                .is_err()
            );
            assert_eq!(budget.storage(), 0);
        }
    });
}

#[test]
fn complete_frame_limit_includes_nested_payloads_and_repeated_metadata() {
    assert_eq!(MAX_BYTES, 4 * 1024 * 1024);
    let base = encoded().len();
    fixture(|input| {
        for extra in [0, 1] {
            let native = vec![0; MAX_BYTES - base + input.native_module.len() + extra];
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            let result = encode_native_conditional_source_packet_v2(
                NativeConditionalSourcePacketInputV2 {
                    native_module: &native,
                    ..input
                },
                &mut budget,
            );
            if extra == 0 {
                let bytes = result.unwrap().0;
                assert_eq!(bytes.len(), MAX_BYTES);
                decode_only(&bytes).unwrap();
            } else {
                assert!(matches!(result, Err(E::Invalid("aggregate packet limit"))));
            }
            assert_eq!(budget.storage(), 0);
        }
    });
    assert!(matches!(
        decode_only(&vec![0; MAX_BYTES + 1]),
        Err(E::Invalid("aggregate packet limit"))
    ));
}

#[test]
fn root_limit_is_checked_before_allocation() {
    fixture(|input| {
        let roots = vec![input.roots[0]; MAX_ROOTS + 1];
        let order = (0..roots.len() as u32).collect::<Vec<_>>();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        assert!(
            encode_native_conditional_source_packet_v2(
                NativeConditionalSourcePacketInputV2 {
                    roots: &roots,
                    canonical_kernel_order: &order,
                    ..input
                },
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 0);
    });
}

#[test]
fn v1_direct_and_unit_local_remain_separate_from_v2() {
    use crate::{
        NativeCompilerRankedRecipeSourceProofInputsV1, NativeCompilerSourceProofInputsV1,
        encode_native_compiler_source_packet_v1, validate_native_compiler_ranked_source_packet_v1,
        validate_native_compiler_unit_local_erased_source_packet_v1,
    };
    for erased in [None, Some(&b"E"[..])] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let (bytes, _) = encode_native_compiler_source_packet_v1(
            NativeCompilerRankedRecipeSourceProofInputsV1 {
                source: NativeCompilerSourceProofInputsV1 {
                    semantic_mir: b"mir",
                    native_module: b"N",
                    middle_end_roster: b"middle",
                    correspondence_roster: b"correspondence",
                    verus_roster: b"verus",
                    launch_inputs: &[],
                    staging_roots: &[],
                },
                ranked_roots: &[],
            },
            erased,
            &mut budget,
        )
        .unwrap();
        assert_eq!(&bytes[..12], b"F2NSRC1\0\x01\0\0\0");
        assert_eq!(bytes[16], if erased.is_some() { 2 } else { 1 });
        assert!(decode_only(&bytes).is_err());
    }
    let bytes = encoded();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    assert!(validate_native_compiler_ranked_source_packet_v1(&bytes, &mut budget).is_err());
    assert!(
        validate_native_compiler_unit_local_erased_source_packet_v1(&bytes, &mut budget).is_err()
    );
}
