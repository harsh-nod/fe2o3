use fe2o3_kernel_descriptor::*;
use sha2::{Digest, Sha256};
#[path = "support/conditional_invocation_v2.rs"]
mod support;
use support::*;

#[path = "conditional_invocation_v2/golden.rs"]
mod golden;
#[path = "conditional_invocation_v2/resources.rs"]
mod resources;

#[test]
fn v2_roundtrip_preserves_complete_rosters_and_maximal_limits() {
    for (inputs, reads) in [(0, 0), (1, 1), (2, 4), (63, 128)] {
        let mut f = Fixture::new(inputs, reads);
        if reads == 128 {
            f.roots.resize(256, [1, 2, 3, 4]);
        }
        let bytes = wire(&f);
        let v = decode_conditional_invocation_contract_v2(&bytes, &mut free).unwrap();
        assert_eq!(bytes.len(), f.wire().len() + 32);
        assert_eq!(v.canonical_bytes(), bytes);
        assert_eq!(v.subjects(), &f.subjects);
        assert_eq!(v.theorem(), &input(&f, [17; 32]).theorem);
        assert_eq!(v.output(), f.output);
        assert_eq!(
            v.numerical_domain(),
            ConditionalNumericalDomainV1::LittleEndianSharedIeeeV1
        );
        assert_eq!(v.typed_root_count(), f.roots.len());
        assert_eq!(v.argument_count(), f.arguments.len());
        assert_eq!(v.read_count(), f.reads.len());
        assert_eq!(v.premise_count(), f.premises.len());
        let mut roots = v.typed_roots();
        for expected in &f.roots {
            assert_eq!(roots.next(&mut free).unwrap(), Some(*expected));
        }
        assert_eq!(roots.next(&mut free).unwrap(), None);
        let mut args = v.arguments();
        for (i, expected) in f.arguments.iter().enumerate() {
            assert_eq!(args.next(&mut free).unwrap(), Some(*expected));
            assert_eq!(v.argument(i, &mut free).unwrap(), *expected);
        }
        assert_eq!(args.next(&mut free).unwrap(), None);
        let mut occurrences = v.reads();
        for expected in &f.reads {
            assert_eq!(occurrences.next(&mut free).unwrap(), Some(*expected));
        }
        assert_eq!(occurrences.next(&mut free).unwrap(), None);
        let mut premises = v.premises();
        for expected in &f.premises {
            assert_eq!(premises.next(&mut free).unwrap(), Some(*expected));
        }
        assert_eq!(premises.next(&mut free).unwrap(), None);
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/CONDITIONAL-INVOCATION/V2\0");
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(&bytes);
        let expected: [u8; 32] = hash.finalize().into();
        assert_eq!(v.identity().as_bytes(), &expected);
        v.require_identity(
            ConditionalInvocationIdentityV2::from_untrusted_bytes(expected),
            &mut free,
        )
        .unwrap();
        assert!(
            v.require_identity(
                ConditionalInvocationIdentityV2::from_untrusted_bytes([0; 32]),
                &mut free
            )
            .is_err()
        );
    }
    assert_eq!(MAX_CONDITIONAL_INVOCATION_BYTES_V2, 65536);
    assert_eq!(MAX_CONDITIONAL_ARGUMENTS_V2, 64);
    assert_eq!(MAX_CONDITIONAL_READS_V2, 128);
    assert_eq!(MAX_CONDITIONAL_ROOTS_V2, 256);
    assert_eq!(MAX_CONDITIONAL_PREMISES_V2, 388);
}

#[test]
fn v2_never_falls_back_to_v1_or_accepts_header_only_relabeling() {
    let f = Fixture::new(1, 1);
    let v1 = f.wire();
    let v2 = wire(&f);
    assert!(decode_conditional_invocation_contract_v1(&v2, &mut free).is_err());
    invalid(&v1, "magic");
    let mut retagged = v1.clone();
    retagged[..8].copy_from_slice(b"FE2O3C2\0");
    invalid(&retagged, "version");
    retagged[8..10].copy_from_slice(&2u16.to_le_bytes());
    invalid(&retagged, "record length/trailing bytes");
    drop(retagged.splice(CPU..CPU, [17; 32]));
    set_len(&mut retagged);
    invalid(&retagged, "theorem preimage");
    // Removing V2's seventh input and changing framing cannot become a V1 theorem.
    let mut stripped = v2.clone();
    drop(stripped.drain(CPU..CPU + 32));
    stripped[..8].copy_from_slice(b"FE2O3CI\0");
    stripped[8..10].copy_from_slice(&1u16.to_le_bytes());
    set_len(&mut stripped);
    assert!(matches!(
        decode_conditional_invocation_contract_v1(&stripped, &mut free),
        Err(ConditionalInvocationWireErrorV1::Contract(
            ConditionalInvocationContractErrorV1::Invalid("theorem preimage")
        ))
    ));
}

#[test]
fn v2_exact_seven_field_preimage_rejects_each_input_and_legacy_domain() {
    let f = Fixture::new(1, 1);
    let bytes = wire(&f);
    for offset in [
        28 + 64,
        THEOREM,
        THEOREM + 32,
        THEOREM + 128,
        THEOREM + 160,
        THEOREM + 192,
        THEOREM + 224,
        CPU,
    ] {
        let mut changed = bytes.clone();
        changed[offset] ^= 1;
        invalid(&changed, "theorem preimage");
    }
    for domain in [
        b"FE2O3/CONDITIONAL-MEMORY-TRANSITION/V1/LE/SHARED-IEEE\0".as_slice(),
        V2_DOMAIN,
    ] {
        let mut hash = Sha256::new();
        hash.update(domain);
        for value in [3u8, 9, 12, 14, 15, 16] {
            hash.update([value; 32]);
        }
        // Even the correct V2 domain is invalid without the seventh field.
        let mut changed = bytes.clone();
        changed[THEOREM..THEOREM + 32].copy_from_slice(&hash.finalize());
        invalid(&changed, "theorem preimage");
    }
    let mut i = input(&f, [17; 32]);
    i.theorem.statement_identity = f.theorem.statement_identity;
    let mut untouched = vec![0xa5; bytes.len()];
    assert!(encode_conditional_invocation_contract_v2(&i, &mut untouched, &mut free).is_err());
    assert!(untouched.iter().all(|b| *b == 0xa5));
}

#[test]
fn v2_coherent_cpu_substitution_changes_content_without_claiming_authenticity() {
    let f = Fixture::new(1, 1);
    let original = wire(&f);
    let identity = decode_conditional_invocation_contract_v2(&original, &mut free)
        .unwrap()
        .identity();
    for cpu in [[18; 32], [0; 32]] {
        let i = input(&f, cpu);
        let mut changed = vec![0; original.len()];
        encode_conditional_invocation_contract_v2(&i, &mut changed, &mut free).unwrap();
        let v = decode_conditional_invocation_contract_v2(&changed, &mut free).unwrap();
        assert_eq!(v.theorem().cpu_input_commitment, cpu);
        assert_ne!(v.identity(), identity);
        assert!(v.require_identity(identity, &mut free).is_err());
    }
    // The codec does not invent checks on claims outside the statement preimage.
    for offset in [28, 28 + 32, 28 + 96, THEOREM + 64, THEOREM + 96, FIXED_V2] {
        let mut changed = original.clone();
        changed[offset] ^= 1;
        let v = decode_conditional_invocation_contract_v2(&changed, &mut free).unwrap();
        assert_ne!(v.identity(), identity);
    }
}

#[test]
fn v2_rejects_extra_or_reordered_statement_fields() {
    let bytes = wire(&Fixture::new(1, 1));
    for values in [
        vec![3u8, 9, 12, 14, 15, 16, 17, 18],
        vec![3, 9, 12, 14, 15, 17, 16],
        vec![17, 3, 9, 12, 14, 15, 16],
    ] {
        let mut hash = Sha256::new();
        hash.update(V2_DOMAIN);
        for value in values {
            hash.update([value; 32]);
        }
        let mut changed = bytes.clone();
        changed[THEOREM..THEOREM + 32].copy_from_slice(&hash.finalize());
        invalid(&changed, "theorem preimage");
    }
    // A V1 domain cannot carry seven fields under the V2 framing either.
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/CONDITIONAL-MEMORY-TRANSITION/V1/LE/SHARED-IEEE\0");
    for value in [3u8, 9, 12, 14, 15, 16, 17] {
        hash.update([value; 32]);
    }
    let mut changed = bytes;
    changed[THEOREM..THEOREM + 32].copy_from_slice(&hash.finalize());
    invalid(&changed, "theorem preimage");
}

#[test]
fn v2_wire_premise_omission_reorder_and_occurrence_substitution_are_checked() {
    let bytes = wire(&Fixture::new(1, 1));
    let start = FIXED_V2 + 32 + 176 + 80;
    for i in 0..7 {
        let mut bad = bytes.clone();
        drop(bad.drain(start + 32 * i..start + 32 * (i + 1)));
        bad[24..26].copy_from_slice(&6u16.to_le_bytes());
        set_len(&mut bad);
        invalid(&bad, "complete premise roster");
    }
    let mut bad = bytes.clone();
    let first: [u8; 32] = bad[start + 32..start + 64].try_into().unwrap();
    bad.copy_within(start + 64..start + 96, start + 32);
    bad[start + 64..start + 96].copy_from_slice(&first);
    invalid(&bad, "output premise order/binding");
    let mut bad = bytes;
    bad[start + 5 * 32 + 4..start + 5 * 32 + 8].copy_from_slice(&11u32.to_le_bytes());
    invalid(&bad, "read premise order/binding");
    let bytes = wire(&Fixture::new(1, 2));
    let read = FIXED_V2 + 32 + 176;
    for range in [4..16, 32..40] {
        let mut bad = bytes.clone();
        bad.copy_within(
            read + range.start..read + range.end,
            read + 80 + range.start,
        );
        invalid(&bad, "duplicate read occurrence");
    }
}

#[test]
fn v2_framing_truncation_reserved_tags_and_trailing_bytes_fail_closed() {
    let bytes = wire(&Fixture::new(1, 1));
    for end in 0..bytes.len() {
        assert!(decode_conditional_invocation_contract_v2(&bytes[..end], &mut free).is_err());
    }
    let mut extra = bytes.clone();
    extra.push(0);
    invalid(&extra, "declared length");
    set_len(&mut extra);
    invalid(&extra, "record length/trailing bytes");
    for (offset, reason) in [
        (0, "magic"),
        (8, "version"),
        (10, "flags"),
        (12, "declared length"),
        (16, "numerical domain/version"),
        (26, "header reserved"),
        (28 + 128, "reference kind"),
    ] {
        let mut bad = bytes.clone();
        bad[offset] ^= 0x80;
        invalid(&bad, reason);
    }
    for offset in [18, 20, 22, 24] {
        let mut bad = bytes.clone();
        bad[offset..offset + 2].copy_from_slice(&u16::MAX.to_le_bytes());
        invalid(&bad, "record count");
    }
    let args = FIXED_V2 + 32;
    let reads = args + 176;
    let premises = reads + 80;
    for offset in [
        608 + 2,
        608 + 44,
        608 + 45,
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
        bad[offset] = 0xff;
        assert!(
            decode_conditional_invocation_contract_v2(&bad, &mut free).is_err(),
            "{offset}"
        );
    }
    invalid(
        &vec![0; MAX_CONDITIONAL_INVOCATION_BYTES_V2 + 1],
        "byte limit",
    );
}

#[test]
fn v2_uses_unchanged_complete_shape_and_occurrence_validation() {
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
        refused(&f);
    }
    refused(&Fixture::new(64, 64));
    refused(&Fixture::new(1, 129));
    for i in 0..7 {
        let mut f = Fixture::new(1, 1);
        f.premises.remove(i);
        refused(&f);
    }
    let mut f = Fixture::new(1, 1);
    f.premises.swap(1, 2);
    refused(&f);
    let mut f = Fixture::new(1, 1);
    f.premises.push(ConditionalRuntimePremiseV1::D1Launch);
    refused(&f);
    let mut f = Fixture::new(1, 1);
    f.premises[5] = ConditionalRuntimePremiseV1::SeparateInputOutput {
        input: 0,
        output: 1,
    };
    refused(&f);
    let mut f = Fixture::new(1, 2);
    f.reads[1].alignment = 1;
    f.refresh_premises();
    assert!(decode_conditional_invocation_contract_v2(&wire(&f), &mut free).is_ok());
    f.reads[1].element_bytes = 8;
    f.refresh_premises();
    refused(&f);
}
