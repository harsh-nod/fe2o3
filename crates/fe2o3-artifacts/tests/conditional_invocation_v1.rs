use fe2o3_artifacts::*;
use sha2::{Digest, Sha256};
#[path = "../../fe2o3-kernel-descriptor/tests/support/conditional_invocation.rs"]
mod support;
use support::{Fixture, free};

// Independently specified wire extents. Changes require explicit schema review.
const FIXED: usize = 28 + 292 + 256 + 48;
const ARGUMENT: usize = 88;
const READ: usize = 80;
const PREMISE: usize = 32;

fn refused(bytes: &[u8]) {
    assert!(decode_conditional_invocation_contract_v1(bytes, &mut free).is_err());
}
fn refused_input(f: &Fixture) {
    assert!(encoded_conditional_invocation_contract_v1_len(&f.input(), &mut free).is_err());
    let mut output = vec![0xA5; 4096];
    assert!(encode_conditional_invocation_contract_v1(&f.input(), &mut output, &mut free).is_err());
    assert!(output.iter().all(|byte| *byte == 0xA5));
}

#[test]
fn roundtrips_preserve_exact_rosters_and_domains() {
    for (inputs, reads) in [(0, 0), (1, 1), (2, 2), (2, 4), (63, 128)] {
        let mut f = Fixture::new(inputs, reads);
        if reads == 128 {
            f.roots = vec![[1, 2, 3, 4]; 256];
        }
        // Repeated roots and repeated parameters are retained, not deduplicated.
        let bytes = f.wire();
        assert_eq!(
            bytes.len(),
            FIXED
                + 32 * f.roots.len()
                + ARGUMENT * f.arguments.len()
                + READ * reads
                + PREMISE * f.premises.len()
        );
        let view = decode_conditional_invocation_contract_v1(&bytes, &mut free).unwrap();
        assert_eq!(
            view.numerical_domain(),
            ConditionalNumericalDomainV1::LittleEndianSharedIeeeV1
        );
        assert_eq!(view.subjects(), &f.subjects);
        assert_eq!(view.theorem(), &f.theorem);
        assert_eq!(view.output(), f.output);
        let mut roots = view.typed_roots();
        let mut arguments = view.arguments();
        let mut occurrences = view.reads();
        let mut premises = view.premises();
        for root in &f.roots {
            assert_eq!(roots.next(&mut free).unwrap(), Some(*root));
        }
        for (i, a) in f.arguments.iter().enumerate() {
            assert_eq!(arguments.next(&mut free).unwrap(), Some(*a));
            assert_eq!(view.argument(i, &mut free).unwrap(), *a);
        }
        for read in &f.reads {
            assert_eq!(occurrences.next(&mut free).unwrap(), Some(*read));
        }
        for p in &f.premises {
            assert_eq!(premises.next(&mut free).unwrap(), Some(*p));
        }
        assert_eq!(roots.next(&mut free).unwrap(), None);
        assert_eq!(arguments.next(&mut free).unwrap(), None);
        assert_eq!(occurrences.next(&mut free).unwrap(), None);
        assert_eq!(premises.next(&mut free).unwrap(), None);
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/CONDITIONAL-INVOCATION/V1\0");
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(&bytes);
        let expected: [u8; 32] = hash.finalize().into();
        assert_eq!(view.identity().as_bytes(), &expected);
        view.require_identity(
            ConditionalInvocationIdentityV1::from_untrusted_bytes(expected),
            &mut free,
        )
        .unwrap();
        assert!(
            view.require_identity(
                ConditionalInvocationIdentityV1::from_untrusted_bytes([0; 32]),
                &mut free
            )
            .is_err()
        );
    }
}

#[test]
fn repeated_reads_keep_per_occurrence_alignment_and_consistent_element_width() {
    let mut fixture = Fixture::new(1, 2);
    fixture.reads[1].alignment = 1;
    fixture.refresh_premises();
    let bytes = fixture.wire();
    let contract = decode_conditional_invocation_contract_v1(&bytes, &mut free).unwrap();
    let mut reads = contract.reads();
    for expected in &fixture.reads {
        assert_eq!(reads.next(&mut free).unwrap(), Some(*expected));
    }
    assert_eq!(reads.next(&mut free).unwrap(), None);
    fixture.reads[1].element_bytes = 8;
    fixture.refresh_premises();
    refused_input(&fixture);
    fixture.reads[1].element_bytes = 4;
    for alignment in [0, 3] {
        fixture.reads[1].alignment = alignment;
        fixture.refresh_premises();
        refused_input(&fixture);
    }
}

#[test]
fn framing_counts_versions_domains_padding_and_truncation_are_independent() {
    let bytes = Fixture::new(1, 1).wire();
    for end in 0..bytes.len() {
        refused(&bytes[..end]);
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    refused(&trailing);
    let n = trailing.len() as u32;
    trailing[12..16].copy_from_slice(&n.to_le_bytes());
    refused(&trailing);
    for offset in [0, 8, 10, 12, 16, 26, 28 + 128] {
        let mut bad = bytes.clone();
        bad[offset] ^= 0x80;
        refused(&bad);
    }
    for offset in [18, 20, 22, 24] {
        let mut bad = bytes.clone();
        bad[offset..offset + 2].copy_from_slice(&u16::MAX.to_le_bytes());
        refused(&bad);
    }
    let args = FIXED + 32;
    let reads = args + 2 * ARGUMENT;
    let premises = reads + READ;
    for offset in [
        args + 22,
        args + 23,
        reads + 2,
        reads + 40,
        reads + 48,
        reads + 64,
        reads + 66,
        premises,
        premises + 1,
        premises + 2,
        premises + 4,
        premises + 24,
    ] {
        let mut bad = bytes.clone();
        bad[offset] = 0xFF;
        refused(&bad);
    }
    refused(&vec![0; MAX_CONDITIONAL_INVOCATION_BYTES_V1 + 1]);
}

#[test]
fn each_theorem_preimage_commitment_is_checked_but_other_claims_remain_inert() {
    let fixture = Fixture::new(1, 1);
    // Independently calculated SHA-256 for the literal six-digest preimage.
    assert_eq!(
        fixture.theorem.statement_identity,
        [
            0xba, 0x32, 0xfb, 0x0b, 0x89, 0x42, 0x36, 0xf4, 0x70, 0x30, 0x55, 0xaa, 0x93, 0x6b,
            0xb2, 0x73, 0x76, 0x50, 0xb4, 0x6d, 0xe6, 0x3b, 0x51, 0x23, 0xbf, 0x32, 0x46, 0xca,
            0x1c, 0xbc, 0x80, 0xb9,
        ]
    );
    let bytes = fixture.wire();
    let theorem = 28 + 292;
    for offset in [
        28 + 64,
        theorem,
        theorem + 32,
        theorem + 128,
        theorem + 160,
        theorem + 192,
        theorem + 224,
    ] {
        let mut bad = bytes.clone();
        bad[offset] ^= 1;
        refused(&bad);
    }
    // Neither a digest nor a self-consistent preimage authenticates these claims.
    // Their changed content identity must be rejected by the consuming owner.
    let original = decode_conditional_invocation_contract_v1(&bytes, &mut free)
        .unwrap()
        .identity();
    for offset in [28, 28 + 32, 28 + 96, theorem + 64, theorem + 96, FIXED] {
        let mut changed = bytes.clone();
        changed[offset] ^= 1;
        let view = decode_conditional_invocation_contract_v1(&changed, &mut free).unwrap();
        assert_ne!(view.identity(), original);
        assert!(view.require_identity(original, &mut free).is_err());
    }
}

#[test]
fn missing_extra_reordered_and_rebound_premises_fail_both_directions() {
    for i in 0..7 {
        let mut f = Fixture::new(1, 1);
        f.premises.remove(i);
        refused_input(&f);
        let f = Fixture::new(1, 1);
        let mut bytes = f.wire();
        let start = FIXED + 32 + 2 * ARGUMENT + READ;
        bytes[start + i * PREMISE..start + (i + 1) * PREMISE].fill(0);
        if i != 0 {
            refused(&bytes);
        }
    }
    let mut f = Fixture::new(1, 1);
    f.premises.push(ConditionalRuntimePremiseV1::D1Launch);
    refused_input(&f);
    let mut f = Fixture::new(1, 1);
    f.premises.swap(1, 2);
    refused_input(&f);
    let mut f = Fixture::new(1, 1);
    f.premises[5] = ConditionalRuntimePremiseV1::SeparateInputOutput {
        input: 0,
        output: 1,
    };
    refused_input(&f);
    let mut f = Fixture::new(1, 1);
    f.output.address_domain = ConditionalAddressDomainV1::GlobalLaunch;
    refused_input(&f);
    f.refresh_premises();
    assert!(encoded_conditional_invocation_contract_v1_len(&f.input(), &mut free).is_ok());
}

#[test]
fn mappings_occurrences_layouts_and_closed_limits_are_checked() {
    for mutate in [
        |f: &mut Fixture| f.arguments[1].canonical_parameter = f.arguments[0].canonical_parameter,
        |f: &mut Fixture| f.arguments[1].generated_field = f.arguments[0].generated_field,
        |f: &mut Fixture| f.arguments[1].source_argument = f.arguments[0].source_argument,
        |f: &mut Fixture| f.arguments[1].adjusted_argument = f.arguments[0].adjusted_argument,
        |f: &mut Fixture| f.arguments[1].semantic_local = f.arguments[0].semantic_local,
        |f: &mut Fixture| f.arguments[0].generated_field = 64,
        |f: &mut Fixture| f.output.argument = u16::MAX,
        |f: &mut Fixture| f.arguments[0].role = ConditionalArgumentRoleV1::Output,
        |f: &mut Fixture| f.reads[0].argument = u16::MAX,
        |f: &mut Fixture| f.reads[0].argument = f.output.argument,
        |f: &mut Fixture| f.reads[1].canonical = f.reads[0].canonical,
        |f: &mut Fixture| f.reads[1].ranked = f.reads[0].ranked,
        |f: &mut Fixture| f.reads[0].canonical = f.output.canonical_store,
        |f: &mut Fixture| f.reads[0].ranked = f.output.ranked_effect,
        |f: &mut Fixture| f.output.ranked_effect = f.output.ranked_store,
        |f: &mut Fixture| f.reads[0].element_bytes = 3,
        |f: &mut Fixture| f.reads[0].alignment = 0,
        |f: &mut Fixture| f.reads[0].alignment = 8,
        |f: &mut Fixture| f.reads[1].access_domain = ConditionalAddressDomainV1::GlobalLaunch,
        |f: &mut Fixture| f.roots.clear(),
        |f: &mut Fixture| f.roots.resize(257, [0; 4]),
        |f: &mut Fixture| f.subjects.safe_reference_source_hash = [1; 32],
        |f: &mut Fixture| f.subjects.safe_reference_identity = [0; 32],
        |f: &mut Fixture| f.subjects.safe_reference_mir_hash = [0; 32],
        |f: &mut Fixture| f.subjects.kernel_subject_identity = [0; 32],
        |f: &mut Fixture| f.subjects.kernel_mir_hash = [0; 32],
    ] {
        let mut f = Fixture::new(2, 2);
        mutate(&mut f);
        refused_input(&f);
    }
    refused_input(&Fixture::new(64, 64));
    refused_input(&Fixture::new(1, 129));
    let f = Fixture::new(1, 1);
    let bytes = f.wire();
    let mut noncanonical_mir = bytes.clone();
    noncanonical_mir[192] = 1;
    refused(&noncanonical_mir);
    let args = FIXED + 32;
    let reads = args + 2 * ARGUMENT;
    for (offset, data) in [
        (args + ARGUMENT, vec![0; 4]),
        (reads, vec![255; 2]),
        (reads + 76, vec![0; 4]),
    ] {
        let mut bad = bytes.clone();
        bad[offset..offset + data.len()].copy_from_slice(&data);
        refused(&bad);
    }
}

#[test]
fn all_work_refusals_preserve_output_and_cursors() {
    let f = Fixture::new(1, 1);
    let bytes = f.wire();
    let mut charges = vec![];
    let mut output = vec![0; bytes.len()];
    encode_conditional_invocation_contract_v1(&f.input(), &mut output, &mut |n| {
        charges.push(n);
        Ok::<(), ()>(())
    })
    .unwrap();
    for fail in 0..charges.len() {
        let mut i = 0;
        output.fill(0xA5);
        assert!(
            encode_conditional_invocation_contract_v1(&f.input(), &mut output, &mut |_| {
                i += 1;
                if i == fail + 1 { Err(()) } else { Ok(()) }
            })
            .is_err()
        );
        assert!(output.iter().all(|b| *b == 0xA5));
    }
    for fail in 0..2 {
        let mut i = 0;
        assert!(
            decode_conditional_invocation_contract_v1(&bytes, &mut |_| {
                i += 1;
                if i == fail + 1 { Err(()) } else { Ok(()) }
            })
            .is_err()
        );
    }
    let view = decode_conditional_invocation_contract_v1(&bytes, &mut free).unwrap();
    let mut cursor = view.arguments();
    assert!(cursor.next(&mut |_| Err(())).is_err());
    assert_eq!(cursor.next(&mut free).unwrap(), Some(f.arguments[0]));
    assert!(view.argument(usize::MAX, &mut free).is_err());
    let mut short = vec![0xA5; bytes.len() - 1];
    assert!(encode_conditional_invocation_contract_v1(&f.input(), &mut short, &mut free).is_err());
    assert!(short.iter().all(|b| *b == 0xA5));
}
