use fe2o3_kernel_descriptor::mixed_conditional_v26::*;
use fe2o3_kernel_descriptor::*;
#[path = "support/conditional_v5.rs"]
mod fixture;
use fixture::free;

// These are inert codec subjects, not an executed compiler or proof fixture.
fn inputs(target: &str, count: usize) -> (Vec<u8>, Vec<Vec<u8>>) {
    fixture::with_input(target, count, 1, |input| {
        let mut nominal =
            vec![0; encoded_device_descriptor_table_v3_len(&input.nominal, &mut free).unwrap()];
        encode_device_descriptor_table_v3(&input.nominal, &mut nominal, &mut free).unwrap();
        let table = decode_device_descriptor_table_v3(&nominal, &mut free).unwrap();
        let descriptor = mixed_descriptor_subject_v26(&table, &mut free).unwrap();
        let mut contracts = Vec::new();
        for ordinal in 0..count {
            let kernel = table.kernel(ordinal, &mut free).unwrap();
            contracts.push(contract(MixedContractSubjectsV26 {
                kernel_id: *kernel.kernel_id().as_bytes(),
                source_semantic_identity: [2; 32],
                original_graph_identity: [3; 32],
                output_graph_identity: [4; 32],
                descriptor_identity: descriptor,
                original_root: (count - 1 - ordinal) as u32,
                output_function: (ordinal * 2 + 1) as u32,
                source_rank: kernel.launch().rank(),
                index_width: 64,
                exact_grid: [32, 1, 1],
                source_argument_count: kernel.argument_count() as u32,
                generated_field_count: kernel.component_count() as u32,
                explicit_argument_bytes: kernel.abi_layout().explicit_argument_size(),
                kernarg_alignment: kernel.abi_layout().kernarg_segment_alignment(),
            }));
        }
        (nominal, contracts)
    })
}
fn contract(subjects: MixedContractSubjectsV26) -> Vec<u8> {
    let input = MixedContractInputV26 {
        subjects,
        arguments: &[],
        occurrences: &[],
    };
    let mut bytes = vec![0; encoded_mixed_contract_v26_len(&input, &mut free).unwrap()];
    encode_mixed_contract_v26(&input, &mut bytes, &mut free).unwrap();
    bytes
}
fn views(bytes: &[Vec<u8>]) -> Vec<MixedContractV26<'_>> {
    bytes
        .iter()
        .map(|b| decode_mixed_contract_v26(b, &mut free).unwrap())
        .collect()
}
fn wire(nominal: &[u8], rows: &[Vec<u8>]) -> Vec<u8> {
    let rows = views(rows);
    let mut output = vec![0; encoded_mixed_descriptor_v53_len(nominal, &rows, &mut free).unwrap()];
    encode_mixed_descriptor_v53(nominal, &rows, &mut output, &mut free).unwrap();
    output
}

#[test]
fn mixed_v53_retains_exact_nominal_and_contract_rosters_for_both_targets() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        for count in [1, 3] {
            let (nominal, rows) = inputs(target, count);
            let bytes = wire(&nominal, &rows);
            let table = decode_mixed_descriptor_v53(&bytes, &mut free).unwrap();
            let old = decode_device_descriptor_table_v3(&nominal, &mut free).unwrap();
            assert_eq!(table.canonical_bytes(), bytes);
            assert_eq!(table.kernel_count(), count);
            assert_eq!(table.device_target(), old.device_target());
            assert_eq!(table.code_object_version(), old.code_object_version());
            assert!(!table.grants_authority());
            for (i, expected) in rows.iter().enumerate() {
                assert_eq!(
                    table.contract(i, &mut free).unwrap().canonical_bytes(),
                    expected
                );
                let a = table.kernel(i, &mut free).unwrap();
                let b = old.kernel(i, &mut free).unwrap();
                assert_eq!(a.kernel_id(), b.kernel_id());
                assert_eq!(a.entry_name(), b.entry_name());
                assert_eq!(a.argument_count(), b.argument_count());
                assert_eq!(a.component_count(), b.component_count());
                assert_eq!(a.abi_layout(), b.abi_layout());
            }
            assert!(table.contract(count, &mut free).is_err());
        }
    }
}

#[test]
fn mixed_v53_missing_duplicate_reordered_and_foreign_contracts_refuse_before_mutation() {
    let (nominal, rows) = inputs("gfx942:xnack-", 3);
    for mode in 0..10 {
        let mut changed = rows.clone();
        match mode {
            0 => {
                changed.pop();
            }
            1 => changed[1] = changed[0].clone(),
            2 => changed.swap(0, 1),
            _ => {
                let mut s = *decode_mixed_contract_v26(&changed[1], &mut free)
                    .unwrap()
                    .subjects();
                match mode {
                    3 => s.original_root = 2,
                    4 => s.original_root = 3,
                    5 => s.output_function = 1,
                    6 => s.source_semantic_identity[0] ^= 1,
                    7 => s.output_graph_identity[0] ^= 1,
                    8 => s.descriptor_identity[0] ^= 1,
                    9 => s.explicit_argument_bytes += 8,
                    _ => unreachable!(),
                }
                changed[1] = contract(s);
            }
        }
        let view = views(&changed);
        assert!(
            encoded_mixed_descriptor_v53_len(&nominal, &view, &mut free).is_err(),
            "mode {mode}"
        );
        let mut output = vec![0xa5; wire(&nominal, &rows).len()];
        assert!(encode_mixed_descriptor_v53(&nominal, &view, &mut output, &mut free).is_err());
        assert!(output.iter().all(|b| *b == 0xa5));
        // Bypass the encoder with individually canonical changed contracts.
        let mut hostile = wire(&nominal, &rows)[..20 + nominal.len()].to_vec();
        for row in &changed {
            hostile.extend_from_slice(&(row.len() as u32).to_le_bytes());
            hostile.extend_from_slice(row);
        }
        let len = hostile.len() as u32;
        hostile[10..12].copy_from_slice(&(changed.len() as u16).to_le_bytes());
        hostile[12..16].copy_from_slice(&len.to_le_bytes());
        assert!(
            decode_mixed_descriptor_v53(&hostile, &mut free).is_err(),
            "decoder mode {mode}"
        );
    }
    let (other, _) = inputs("gfx950:xnack-", 3);
    assert!(encoded_mixed_descriptor_v53_len(&other, &views(&rows), &mut free).is_err());
}

#[test]
fn mixed_v53_reader_checks_every_byte_extent_and_rejects_old_schema_substitution() {
    let (nominal, rows) = inputs("gfx942:xnack-", 1);
    let bytes = wire(&nominal, &rows);
    for end in 0..bytes.len() {
        assert!(decode_mixed_descriptor_v53(&bytes[..end], &mut free).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    let len = trailing.len() as u32;
    trailing[12..16].copy_from_slice(&len.to_le_bytes());
    assert!(decode_mixed_descriptor_v53(&trailing, &mut free).is_err());
    assert!(decode_device_descriptor_table_v1(&bytes).is_err());
    assert!(decode_device_descriptor_table_v3(&bytes, &mut free).is_err());
    assert!(decode_device_descriptor_table_v5(&bytes, &mut free).is_err());
    assert!(decode_mixed_descriptor_v53(&nominal, &mut free).is_err());
    let old = fixture::wire("gfx942:xnack-", 1, 1);
    assert!(decode_mixed_descriptor_v53(&old, &mut free).is_err());
    for version in [1u16, 3, 5, 50, 52, 54] {
        let mut changed = bytes.clone();
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        assert!(decode_mixed_descriptor_v53(&changed, &mut free).is_err());
    }
}

#[test]
fn mixed_v53_only_digest_slot_is_normalized_not_target_or_contract_identity() {
    let (nominal, rows) = inputs("gfx942:xnack-", 1);
    let mut bytes = wire(&nominal, &rows);
    bytes[CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V53..CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V53 + 32]
        .fill(0x71);
    let table = decode_mixed_descriptor_v53(&bytes, &mut free).unwrap();
    assert_eq!(table.canonical_code_object_digest().as_bytes(), &[0x71; 32]);
    assert_eq!(
        table.contract(0, &mut free).unwrap().canonical_bytes(),
        rows[0]
    );
    // The artifact finalizer, not this inert codec, authenticates this digest.
    assert!(!table.grants_authority());
    let contract_start = 20 + nominal.len() + 4;
    bytes[contract_start + rows[0].len() - 1] ^= 1;
    assert!(decode_mixed_descriptor_v53(&bytes, &mut free).is_err());
}

#[test]
fn mixed_v53_encoder_exact_work_and_panic_leave_output_untouched() {
    let (nominal, rows) = inputs("gfx950:xnack-", 3);
    let rows = views(&rows);
    let length = encoded_mixed_descriptor_v53_len(&nominal, &rows, &mut free).unwrap();
    let mut expected = vec![0; length];
    let mut work = 0usize;
    encode_mixed_descriptor_v53(&nominal, &rows, &mut expected, &mut |n| {
        work += n;
        Ok::<_, ()>(())
    })
    .unwrap();
    for remaining in [work, work - 1] {
        let mut left = remaining;
        let mut output = vec![0xa5; length];
        let result = encode_mixed_descriptor_v53(&nominal, &rows, &mut output, &mut |n| {
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
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ =
            encode_mixed_descriptor_v53(&nominal, &rows, &mut output, &mut |_| -> Result<(), ()> {
                panic!("funding panic")
            });
    }));
    assert!(panic.is_err());
    assert!(output.iter().all(|b| *b == 0xa5));
}
