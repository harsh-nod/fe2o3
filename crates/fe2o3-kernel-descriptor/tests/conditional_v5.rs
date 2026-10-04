use fe2o3_kernel_descriptor::*;
use sha2::{Digest, Sha256};
#[path = "support/conditional_v5.rs"]
mod fixture;
use fixture::{contract_start, free, repair_identity};
#[test]
fn rank_access_ownership_and_alias_refusals_do_not_fall_back() {
    for mode in 0..4 {
        fixture::with_input("gfx942:xnack-", 1, 1, |input| {
            let old = &input.nominal.kernels[0];
            let mut args = old
                .arguments
                .iter()
                .map(|a| LogicalArgumentInputV3 {
                    source_index: a.source_index,
                    name: a.name,
                    source_type: a.source_type,
                    device_layout: a.device_layout,
                    ownership: a.ownership,
                    access: a.access,
                    alias: a.alias,
                    components: a.components,
                })
                .collect::<Vec<_>>();
            match mode {
                0 => args[0].access = AccessMode::ReadWrite,
                1 => args[0].ownership = OwnershipSemantics::UniqueBorrow,
                2 => args[0].alias = AliasSemantics::Exclusive,
                _ => {}
            }
            let wrong_rank = LaunchConstraintsV1::new(
                2,
                BlockSizeV1::Any,
                DimensionsV1::new(1024, 1, 1).unwrap(),
                256,
                0,
                0,
            )
            .unwrap();
            let kernels = [KernelDescriptorInputV3 {
                kernel_id: old.kernel_id,
                logical_name: old.logical_name,
                entry_name: old.entry_name,
                descriptor_symbol: old.descriptor_symbol,
                source_evidence: old.source_evidence,
                executable_ir_evidence: old.executable_ir_evidence,
                capabilities: old.capabilities,
                abi_layout: old.abi_layout,
                launch: if mode == 3 { &wrong_rank } else { old.launch },
                arguments: &args,
            }];
            let changed = DeviceDescriptorTableInputV5 {
                nominal: DeviceDescriptorTableInputV3 {
                    canonical_code_object_digest: input.nominal.canonical_code_object_digest,
                    code_object_version: input.nominal.code_object_version,
                    compiler: input.nominal.compiler,
                    producer: input.nominal.producer,
                    device_target: input.nominal.device_target,
                    type_records: input.nominal.type_records,
                    layout_records: input.nominal.layout_records,
                    kernels: &kernels,
                    requirements: input.nominal.requirements,
                },
                contracts: input.contracts,
            };
            assert!(encoded_device_descriptor_table_v5_len(&changed, &mut free).is_err());
            let mut output = vec![0xA5; 4096];
            assert!(encode_device_descriptor_table_v5(&changed, &mut output, &mut free).is_err());
            assert!(output.iter().all(|byte| *byte == 0xA5));
        });
    }
}

fn refused(bytes: &[u8]) {
    assert!(decode_device_descriptor_table_v5(bytes, &mut free).is_err());
}
fn set_len(bytes: &mut [u8]) {
    let n = bytes.len() as u32;
    bytes[12..16].copy_from_slice(&n.to_le_bytes());
}

#[test]
fn both_targets_multi_kernel_queries_retain_exact_v2_contracts() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        for (entries, inputs) in [(1, 0), (1, 2), (3, 2)] {
            let bytes = fixture::wire(target, entries, inputs);
            let view = decode_device_descriptor_table_v5(&bytes, &mut free).unwrap();
            assert_eq!(view.canonical_bytes(), bytes);
            assert_eq!(view.kernel_count(), entries);
            assert_eq!(view.device_target(), DeviceTargetV1::parse(target).unwrap());
            assert_eq!(view.code_object_version(), CodeObjectVersion::V6);
            assert_eq!(view.compiler_name(), "rustc");
            assert_eq!(view.producer_version(), "inert-test");
            let mut hash = Sha256::new();
            hash.update(b"FE2O3/DEVICE-DESCRIPTOR-TABLE/V5\0");
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(&bytes);
            let digest: [u8; 32] = hash.finalize().into();
            assert_eq!(view.table_digest(&mut free).unwrap().as_bytes(), &digest);
            for i in 0..entries {
                let kernel = view.kernel(i, &mut free).unwrap();
                let contract = kernel.conditional_contract(&mut free).unwrap();
                assert_eq!(contract.theorem().cpu_input_commitment, [17; 32]);
                assert_eq!(
                    &contract.subjects().kernel_id,
                    kernel.kernel_id().as_bytes()
                );
                assert_eq!(contract.read_count(), inputs);
                assert_eq!(contract.premise_count(), 4 + 3 * inputs);
                assert_eq!(
                    view.find_kernel(kernel.kernel_id(), &mut free)
                        .unwrap()
                        .entry_name(),
                    kernel.entry_name()
                );
                let mut arguments = kernel.arguments();
                while let Some(a) = arguments.next(&mut free).unwrap() {
                    assert_eq!(a.component_count(), 2);
                    view.source_type(a.source_type(), &mut free).unwrap();
                    view.device_layout(a.device_layout(), &mut free).unwrap();
                }
                view.requirement(i, &mut free).unwrap();
            }
            assert!(view.kernel(entries, &mut free).is_err());
            assert!(
                view.find_kernel(KernelId::from_bytes([255; 32]), &mut free)
                    .is_err()
            );
        }
    }
}

#[test]
fn repeated_reads_and_independent_ordinal_spaces_are_not_restricted() {
    for reorder in [false, true] {
        let bytes = fixture::wire_custom("gfx950:xnack-", 1, 1, |f| {
            fixture::mixed_read_alignments(f);
            if reorder {
                let first = f.arguments[0].canonical_parameter;
                f.arguments[0].canonical_parameter = f.arguments[1].canonical_parameter;
                f.arguments[1].canonical_parameter = first;
                f.arguments.sort_by_key(|a| a.canonical_parameter);
                f.output.argument = 0;
                for read in &mut f.reads {
                    read.argument = 1;
                }
                f.refresh_premises();
            }
        });
        let table = decode_device_descriptor_table_v5(&bytes, &mut free).unwrap();
        let kernel = table.kernel(0, &mut free).unwrap();
        let contract = kernel.conditional_contract(&mut free).unwrap();
        assert_eq!(contract.premise_count(), 10);
        assert_eq!(contract.output().argument, if reorder { 0 } else { 1 });
        let mut reads = contract.reads();
        for alignment in [4, 1] {
            let r = reads.next(&mut free).unwrap().unwrap();
            assert_eq!(
                (r.argument, r.element_bytes, r.alignment),
                (u16::from(reorder), 4, alignment)
            );
        }
        assert_eq!(reads.next(&mut free).unwrap(), None);
    }
}

#[test]
fn old_schemas_and_cross_version_contracts_never_fall_back() {
    let bytes = fixture::wire("gfx942:xnack-", 1, 1);
    assert!(decode_device_descriptor_table_v1(&bytes).is_err());
    assert!(decode_device_descriptor_table_v2(&bytes).is_err());
    assert!(decode_device_descriptor_table_v3(&bytes, &mut free).is_err());
    assert!(decode_device_descriptor_table_v4(&bytes, &mut free).is_err());
    let mut relabeled = bytes.clone();
    relabeled[8..10].copy_from_slice(&4u16.to_le_bytes());
    assert!(matches!(
        decode_device_descriptor_table_v4(&relabeled, &mut free),
        Err(DescriptorWireErrorV4::Contract(_))
    ));
    let mut old = fixture::v4::wire("gfx942:xnack-", 1, 1);
    refused(&old);
    old[8..10].copy_from_slice(&5u16.to_le_bytes());
    assert!(matches!(
        decode_device_descriptor_table_v5(&old, &mut free),
        Err(DescriptorWireErrorV5::Contract(_))
    ));
    for version in [0u16, 1, 2, 3, 4, 6, 0x0105, u16::MAX] {
        let mut changed = bytes.clone();
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        refused(&changed);
    }
    fixture::with_input("gfx942:xnack-", 1, 1, |input| {
        let mut old =
            vec![0; encoded_device_descriptor_table_v3_len(&input.nominal, &mut free).unwrap()];
        encode_device_descriptor_table_v3(&input.nominal, &mut old, &mut free).unwrap();
        old[8..10].copy_from_slice(&5u16.to_le_bytes());
        refused(&old);
    });
}

#[test]
fn malformed_missing_reordered_duplicate_and_mixed_contracts_are_rejected() {
    let bytes = fixture::wire("gfx942:xnack-", 2, 1);
    for end in 0..bytes.len() {
        refused(&bytes[..end]);
    }
    let start = contract_start(&bytes);
    for offset in [
        0,
        10,
        12,
        start - 40,
        start - 38,
        start - 36,
        start - 32,
        start,
    ] {
        let mut changed = bytes.clone();
        changed[offset] ^= 0xff;
        refused(&changed);
    }
    let n = u32::from_le_bytes(bytes[start - 36..start - 32].try_into().unwrap()) as usize;
    let first = bytes[start - 36..start + n].to_vec();
    let second = bytes[start + n..].to_vec();
    let mut swapped = bytes.clone();
    swapped[start - 36..start + n].copy_from_slice(&second);
    swapped[start + n..].copy_from_slice(&first);
    refused(&swapped);
    let mut duplicate = bytes.clone();
    duplicate[start + n..].copy_from_slice(&first);
    refused(&duplicate);
    let old = fixture::v4::wire("gfx942:xnack-", 2, 1);
    let old_start = old
        .windows(8)
        .position(|w| w == CONDITIONAL_INVOCATION_MAGIC_V1)
        .unwrap();
    let old_n =
        u32::from_le_bytes(old[old_start - 36..old_start - 32].try_into().unwrap()) as usize;
    let mut mixed = bytes[..start + n].to_vec();
    mixed.extend_from_slice(&old[old_start + old_n..]);
    set_len(&mut mixed);
    refused(&mixed);
    let mut trailing = bytes.clone();
    trailing.push(0);
    set_len(&mut trailing);
    refused(&trailing);
    let mut missing = bytes[..start - 40].to_vec();
    set_len(&mut missing);
    refused(&missing);
    let mut oversized = bytes.clone();
    oversized[start - 36..start - 32]
        .copy_from_slice(&((MAX_CONDITIONAL_INVOCATION_BYTES_V2 + 1) as u32).to_le_bytes());
    refused(&oversized);
    refused(&vec![0; MAX_DESCRIPTOR_TABLE_BYTES + 1]);
    fixture::with_input("gfx942:xnack-", 1, 1, |mut input| {
        input.contracts = &[];
        assert!(matches!(
            encoded_device_descriptor_table_v5_len(&input, &mut free),
            Err(DescriptorWireErrorV5::Invalid("mandatory contract roster"))
        ));
    });
}

#[test]
fn complete_nominal_joins_and_seventh_field_are_checked_without_authenticating_them() {
    let bytes = fixture::wire("gfx942:xnack-", 1, 2);
    let start = contract_start(&bytes);
    let argument = start + 656 + 32;
    for offset in [
        start + 28,
        start + 16,
        start + 320,
        start + 576,
        argument + 4,
        argument + 20,
        argument + 24,
        argument + 56,
        start + 608 + 32,
        argument + 3 * 88 + 68,
    ] {
        let mut changed = bytes.clone();
        changed[offset] ^= 0x40;
        repair_identity(&mut changed);
        refused(&changed);
    }
    // Coherent inert CPU substitution is valid content, not an imported receipt.
    let mut changed = bytes.clone();
    changed[start + 576] ^= 0x40;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/CONDITIONAL-MEMORY-TRANSITION/V2/LE/SHARED-IEEE/CPU-SOURCE\0");
    for offset in [92, 352, 448, 480, 512, 544, 576] {
        hash.update(&changed[start + offset..start + offset + 32]);
    }
    changed[start + 320..start + 352].copy_from_slice(&hash.finalize());
    repair_identity(&mut changed);
    let a = decode_device_descriptor_table_v5(&bytes, &mut free).unwrap();
    let b = decode_device_descriptor_table_v5(&changed, &mut free).unwrap();
    assert_ne!(
        a.table_digest(&mut free).unwrap(),
        b.table_digest(&mut free).unwrap()
    );
}

#[test]
fn inherited_kernel_argument_and_occurrence_limits_are_not_reduced() {
    for (entries, inputs) in [(MAX_KERNELS, 0), (1, MAX_ARGUMENTS_PER_KERNEL - 1)] {
        let bytes = fixture::wire("gfx942:xnack-", entries, inputs);
        let table = decode_device_descriptor_table_v5(&bytes, &mut free).unwrap();
        assert_eq!(table.kernel_count(), entries);
        assert_eq!(
            table.kernel(0, &mut free).unwrap().argument_count(),
            inputs + 1
        );
    }
    let bytes = fixture::wire_custom("gfx950:xnack-", 1, 1, |f| {
        f.reads = fixture::Fixture::new(1, MAX_CONDITIONAL_READS_V2).reads;
        f.refresh_premises();
    });
    let table = decode_device_descriptor_table_v5(&bytes, &mut free).unwrap();
    assert_eq!(
        table
            .kernel(0, &mut free)
            .unwrap()
            .conditional_contract(&mut free)
            .unwrap()
            .read_count(),
        MAX_CONDITIONAL_READS_V2
    );
    fixture::with_input("gfx942:xnack-", MAX_KERNELS + 1, 0, |input| {
        assert!(encoded_device_descriptor_table_v5_len(&input, &mut free).is_err());
    });
}

#[test]
fn exact_work_one_short_and_each_denial_preserve_output_and_original_callback() {
    fixture::with_input("gfx942:xnack-", 1, 0, |input| {
        let n = encoded_device_descriptor_table_v5_len(&input, &mut free).unwrap();
        let mut bytes = vec![0; n];
        let mut trace = Vec::new();
        encode_device_descriptor_table_v5(&input, &mut bytes, &mut |w| {
            trace.push(w);
            Ok::<_, ()>(())
        })
        .unwrap();
        for limit in [trace.iter().sum::<usize>(), trace.iter().sum::<usize>() - 1] {
            let mut remaining = limit;
            let mut denied = 0;
            let mut output = vec![0xa5; n];
            let result = encode_device_descriptor_table_v5(&input, &mut output, &mut |w| {
                if w > remaining {
                    denied += 1;
                    Err(())
                } else {
                    remaining -= w;
                    Ok(())
                }
            });
            if limit == trace.iter().sum::<usize>() {
                assert!(result.is_ok());
                assert_eq!(remaining, 0);
                assert_eq!(output, bytes);
            } else {
                assert!(result.is_err());
                assert_eq!(denied, 1);
                assert!(output.iter().all(|b| *b == 0xa5));
            }
        }
        for deny in 0..trace.len() {
            let mut seen = Vec::new();
            let mut output = vec![0xa5; n];
            assert!(
                encode_device_descriptor_table_v5(&input, &mut output, &mut |w| {
                    seen.push(w);
                    if seen.len() == deny + 1 {
                        Err(deny)
                    } else {
                        Ok(())
                    }
                })
                .is_err()
            );
            assert_eq!(seen, trace[..=deny]);
            assert!(output.iter().all(|b| *b == 0xa5));
        }
        let mut short = vec![0xa5; n - 1];
        assert!(matches!(
            encode_device_descriptor_table_v5(&input, &mut short, &mut free),
            Err(DescriptorWireErrorV5::OutputLength { .. })
        ));
        assert!(short.iter().all(|b| *b == 0xa5));
        let mut decode_trace = Vec::new();
        decode_device_descriptor_table_v5(&bytes, &mut |w| {
            decode_trace.push(w);
            Ok::<_, ()>(())
        })
        .unwrap();
        for deny in 0..decode_trace.len() {
            let mut seen = Vec::new();
            assert!(
                decode_device_descriptor_table_v5(&bytes, &mut |w| {
                    seen.push(w);
                    if seen.len() == deny + 1 {
                        Err(deny)
                    } else {
                        Ok(())
                    }
                })
                .is_err()
            );
            assert_eq!(seen, decode_trace[..=deny]);
        }
    });
}
