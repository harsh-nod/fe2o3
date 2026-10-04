use fe2o3_kernel_descriptor::mixed_conditional_v26::*;
use fe2o3_kernel_descriptor::mixed_conditional_v86::*;
use fe2o3_kernel_descriptor::*;
#[path = "support/mixed_v89.rs"]
mod support;
use support::*;

#[test]
fn mixed_v89_retains_both_guards_repeated_and_unused_arguments_for_both_targets() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        for count in [1, 3] {
            let (nominal, rows) = inputs(target, count);
            let bytes = wire(&nominal, &rows);
            let table = decode_mixed_descriptor_v89(&bytes, &mut free).unwrap();
            let old = decode_device_descriptor_table_v3(&nominal, &mut free).unwrap();
            assert_eq!(table.canonical_bytes(), bytes);
            assert_eq!(table.nominal_canonical_bytes(), nominal);
            assert_eq!(table.kernel_count(), count);
            assert_eq!(table.device_target(), old.device_target());
            assert_eq!(table.code_object_version(), old.code_object_version());
            assert!(!table.grants_authority());
            for (i, expected) in rows.iter().enumerate() {
                let view = table.contract(i, &mut free).unwrap();
                let before = decode_mixed_contract_v86(expected, &mut free).unwrap();
                assert_eq!(view.canonical_bytes(), expected);
                assert_eq!(view.identity(), before.identity());
                assert_eq!(view.argument_count(), 3);
                assert_eq!(view.occurrence_count(), 3);
                assert_eq!(view.argument(1, &mut free).unwrap(), argument(1, 0, 0));
                for n in 0..3 {
                    assert_eq!(
                        view.occurrence(n, &mut free).unwrap(),
                        before.occurrence(n, &mut free).unwrap()
                    );
                    assert_eq!(
                        view.occurrence_identity(n, &mut free).unwrap(),
                        before.occurrence_identity(n, &mut free).unwrap()
                    );
                    assert_eq!(
                        view.occurrence(n, &mut free)
                            .unwrap()
                            .output_guard
                            .edge()
                            .is_some(),
                        n == 0
                    );
                }
                assert_ne!(
                    view.occurrence_identity(1, &mut free).unwrap(),
                    view.occurrence_identity(2, &mut free).unwrap()
                );
                let kernel = table.kernel(i, &mut free).unwrap();
                assert_eq!(
                    kernel.abi_layout(),
                    old.kernel(i, &mut free).unwrap().abi_layout()
                );
                assert_eq!(
                    kernel.kernel_id(),
                    old.kernel(i, &mut free).unwrap().kernel_id()
                );
                assert_eq!(
                    table.requirement(i, &mut free).unwrap(),
                    old.requirement(i, &mut free).unwrap()
                );
            }
            assert!(table.contract(count, &mut free).is_err());
        }
    }
}

#[test]
fn mixed_v89_incomplete_foreign_and_reordered_rosters_refuse_before_output_mutation() {
    let (nominal, rows) = inputs("gfx942:xnack-", 3);
    for mode in 0..14 {
        let mut changed = rows.clone();
        match mode {
            0 => {
                changed.pop();
            }
            1 => changed[1] = changed[0].clone(),
            2 => changed.swap(0, 1),
            _ => {
                let mut s = *decode_mixed_contract_v86(&changed[1], &mut free)
                    .unwrap()
                    .subjects();
                match mode {
                    3 => s.original_root = 2,
                    4 => s.original_root = 3,
                    5 => s.output_function = 1,
                    6 => s.source_semantic_identity[0] ^= 1,
                    7 => s.original_graph_identity[0] ^= 1,
                    8 => s.output_graph_identity[0] ^= 1,
                    9 => s.descriptor_identity[0] ^= 1,
                    10 => s.explicit_argument_bytes += 8,
                    11 => s.kernel_id[0] ^= 1,
                    12 => s.source_argument_count += 1,
                    13 => s.generated_field_count += 1,
                    _ => unreachable!(),
                }
                changed[1] = contract(s);
            }
        }
        let view = views(&changed);
        assert!(
            encoded_mixed_descriptor_v89_len(&nominal, &view, &mut free).is_err(),
            "mode {mode}"
        );
        let mut output = vec![0xa5; wire(&nominal, &rows).len()];
        assert!(encode_mixed_descriptor_v89(&nominal, &view, &mut output, &mut free).is_err());
        assert!(output.iter().all(|b| *b == 0xa5));
        let mut hostile = wire(&nominal, &rows)[..20 + nominal.len()].to_vec();
        for row in &changed {
            hostile.extend_from_slice(&(row.len() as u32).to_le_bytes());
            hostile.extend_from_slice(row);
        }
        let len = hostile.len() as u32;
        hostile[10..12].copy_from_slice(&(changed.len() as u16).to_le_bytes());
        hostile[12..16].copy_from_slice(&len.to_le_bytes());
        assert!(
            decode_mixed_descriptor_v89(&hostile, &mut free).is_err(),
            "decoder mode {mode}"
        );
    }
    let (other, _) = inputs("gfx950:xnack-", 3);
    assert!(encoded_mixed_descriptor_v89_len(&other, &views(&rows), &mut free).is_err());
}

#[test]
fn mixed_v89_never_downgrades_headers_or_nested_contracts() {
    let (nominal, rows) = inputs("gfx942:xnack-", 1);
    let bytes = wire(&nominal, &rows);
    let subject = *views(&rows)[0].subjects();
    let input = MixedContractInputV26 {
        subjects: subject,
        arguments: &[],
        occurrences: &[],
    };
    let mut legacy = vec![0; encoded_mixed_contract_v26_len(&input, &mut free).unwrap()];
    encode_mixed_contract_v26(&input, &mut legacy, &mut free).unwrap();
    let legacy_contract = decode_mixed_contract_v26(&legacy, &mut free).unwrap();
    let mut legacy_table =
        vec![0; encoded_mixed_descriptor_v53_len(&nominal, &[legacy_contract], &mut free).unwrap()];
    encode_mixed_descriptor_v53(
        &nominal,
        &[decode_mixed_contract_v26(&legacy, &mut free).unwrap()],
        &mut legacy_table,
        &mut free,
    )
    .unwrap();
    assert!(decode_mixed_descriptor_v53(&bytes, &mut free).is_err());
    assert!(decode_mixed_descriptor_v89(&legacy_table, &mut free).is_err());
    assert!(decode_device_descriptor_table_v3(&bytes, &mut free).is_err());
    assert!(decode_device_descriptor_table_v5(&bytes, &mut free).is_err());
    assert!(decode_mixed_descriptor_v89(&nominal, &mut free).is_err());
    let mut changed = bytes.clone();
    changed[..8].copy_from_slice(b"FE2O3D53");
    changed[8..10].copy_from_slice(&53u16.to_le_bytes());
    assert!(decode_mixed_descriptor_v53(&changed, &mut free).is_err());
    legacy_table[..8].copy_from_slice(b"FE2O3D89");
    legacy_table[8..10].copy_from_slice(&89u16.to_le_bytes());
    assert!(decode_mixed_descriptor_v89(&legacy_table, &mut free).is_err());
    assert_ne!(
        COMPILER_MIXED_DESCRIPTOR_SECTION_V53,
        COMPILER_MIXED_DESCRIPTOR_SECTION_V89
    );
    assert_ne!(MIXED_CONTRACT_DOMAIN_V26, MIXED_CONTRACT_DOMAIN_V86);
}

#[test]
fn mixed_v89_rejects_every_truncation_trailing_bytes_and_length_overflow() {
    let (nominal, rows) = inputs("gfx950:xnack-", 1);
    let bytes = wire(&nominal, &rows);
    for end in 0..bytes.len() {
        assert!(decode_mixed_descriptor_v89(&bytes[..end], &mut free).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    let len = trailing.len() as u32;
    trailing[12..16].copy_from_slice(&len.to_le_bytes());
    assert!(decode_mixed_descriptor_v89(&trailing, &mut free).is_err());
    for at in [12, 16, 20 + nominal.len()] {
        let mut changed = bytes.clone();
        changed[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_mixed_descriptor_v89(&changed, &mut free).is_err());
    }
}

#[test]
fn mixed_v89_digest_normalization_preserves_exact_occurrence_and_guard_identity() {
    let (nominal, rows) = inputs("gfx942:xnack-", 1);
    let mut bytes = wire(&nominal, &rows);
    bytes[CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V89..CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V89 + 32]
        .fill(0x71);
    let table = decode_mixed_descriptor_v89(&bytes, &mut free).unwrap();
    assert_eq!(table.canonical_code_object_digest().as_bytes(), &[0x71; 32]);
    let view = table.contract(0, &mut free).unwrap();
    assert_eq!(view.canonical_bytes(), rows[0]);
    let arguments = (0..3)
        .map(|i| view.argument(i, &mut free).unwrap())
        .collect::<Vec<_>>();
    let occurrences = (0..3)
        .map(|i| view.occurrence(i, &mut free).unwrap())
        .collect::<Vec<_>>();
    for mode in 0..3 {
        let mut changed = occurrences.clone();
        match mode {
            0 => {
                changed[1].output_guard = MixedAccessGuardV86::ExplicitPredicate {
                    condition: definition(view.subjects().output_function, 5),
                    bound_comparison: definition(view.subjects().output_function, 4),
                }
            }
            1 => changed[1].formation_envelope = MixedIndexEnvelopeV26::UnsignedWidth { bits: 64 },
            2 => changed.swap(1, 2),
            _ => unreachable!(),
        }
        let changed_bytes = encode_contract(*view.subjects(), &arguments, &changed);
        let changed = decode_mixed_contract_v86(&changed_bytes, &mut free).unwrap();
        assert_ne!(changed.identity(), view.identity());
        assert_ne!(
            changed.occurrence_identity(1, &mut free).unwrap(),
            view.occurrence_identity(1, &mut free).unwrap()
        );
    }
    assert!(!table.grants_authority());
}

#[test]
fn mixed_v89_encoder_exact_short_and_panicking_work_preserves_output() {
    let (nominal, rows) = inputs("gfx950:xnack-", 3);
    let rows = views(&rows);
    let length = encoded_mixed_descriptor_v89_len(&nominal, &rows, &mut free).unwrap();
    let mut expected = vec![0; length];
    let mut work = 0usize;
    encode_mixed_descriptor_v89(&nominal, &rows, &mut expected, &mut |n| {
        work += n;
        Ok::<_, ()>(())
    })
    .unwrap();
    for remaining in [work, work - 1] {
        let mut left = remaining;
        let mut output = vec![0xa5; length];
        let result = encode_mixed_descriptor_v89(&nominal, &rows, &mut output, &mut |n| {
            left = left.checked_sub(n).ok_or(())?;
            Ok::<(), ()>(())
        });
        if remaining == work {
            result.unwrap();
            assert_eq!(left, 0);
            assert_eq!(output, expected);
        } else {
            assert!(result.is_err());
            assert!(output.iter().all(|b| *b == 0xa5));
        }
    }
    let mut output = vec![0xa5; length];
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = encode_mixed_descriptor_v89(&nominal, &rows, &mut output, &mut |_| -> Result<
                (),
                (),
            > {
                panic!("funding panic")
            });
        }))
        .is_err()
    );
    assert!(output.iter().all(|b| *b == 0xa5));
    let mut short = vec![0xa5; length - 1];
    assert!(matches!(
        encode_mixed_descriptor_v89(&nominal, &rows, &mut short, &mut free),
        Err(MixedDescriptorErrorV89::OutputLength { .. })
    ));
    assert!(short.iter().all(|b| *b == 0xa5));
}

#[test]
fn mixed_v89_decoder_exact_and_one_short_work_have_no_fallback() {
    let (nominal, rows) = inputs("gfx950:xnack-", 3);
    let bytes = wire(&nominal, &rows);
    let mut work = 0usize;
    decode_mixed_descriptor_v89(&bytes, &mut |n| {
        work += n;
        Ok::<_, ()>(())
    })
    .unwrap();
    for remaining in [work, work - 1] {
        let mut left = remaining;
        let result = decode_mixed_descriptor_v89(&bytes, &mut |n| {
            left = left.checked_sub(n).ok_or(())?;
            Ok::<(), ()>(())
        });
        assert_eq!(result.is_ok(), remaining == work);
        if remaining == work {
            assert_eq!(left, 0);
        }
    }
    assert!(
        MIXED_DESCRIPTOR_READER_STORAGE_V89
            > MIXED_CONTRACT_CODEC_STORAGE_V86 + size_of::<MixedDescriptorTableV89<'static>>()
    );
}
