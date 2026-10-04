use fe2o3_kernel_descriptor::*;
use sha2::{Digest, Sha256};
#[path = "support/conditional_v4.rs"]
mod fixture;
fn free(_: usize) -> Result<(), ()> {
    Ok(())
}
fn refused(bytes: &[u8]) {
    assert!(decode_device_descriptor_table_v4(bytes, &mut free).is_err());
}
fn contract_start(bytes: &[u8]) -> usize {
    bytes
        .windows(8)
        .position(|w| w == CONDITIONAL_INVOCATION_MAGIC_V1)
        .unwrap()
}
fn repair_contract_identity(bytes: &mut [u8]) {
    let start = contract_start(bytes);
    let n = u32::from_le_bytes(bytes[start - 36..start - 32].try_into().unwrap()) as usize;
    let mut hash = Sha256::new();
    hash.update(CONDITIONAL_INVOCATION_DOMAIN_V1);
    hash.update((n as u64).to_le_bytes());
    hash.update(&bytes[start..start + n]);
    bytes[start - 32..start].copy_from_slice(&hash.finalize());
}

#[test]
fn nominal_multi_entry_roundtrip_and_mandatory_contract_queries() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        for (entries, inputs) in [(1, 0), (1, 2), (3, 2)] {
            let bytes = fixture::wire(target, entries, inputs);
            let view = decode_device_descriptor_table_v4(&bytes, &mut free).unwrap();
            assert_eq!(view.kernel_count(), entries);
            assert_eq!(view.device_target(), DeviceTargetV1::parse(target).unwrap());
            assert_eq!(view.compiler_name(), "rustc");
            assert_eq!(view.producer_version(), "inert-test");
            assert_eq!(view.canonical_bytes(), bytes);
            let mut hash = Sha256::new();
            hash.update(b"FE2O3/DEVICE-DESCRIPTOR-TABLE/V4\0");
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(&bytes);
            let expected: [u8; 32] = hash.finalize().into();
            assert_eq!(view.table_digest(&mut free).unwrap().as_bytes(), &expected);
            for i in 0..entries {
                let kernel = view.kernel(i, &mut free).unwrap();
                assert_eq!(kernel.kernel_id(), KernelId::from_bytes([13 + i as u8; 32]));
                assert_eq!(
                    view.find_kernel(kernel.kernel_id(), &mut free)
                        .unwrap()
                        .entry_name(),
                    kernel.entry_name()
                );
                let contract = kernel.conditional_contract(&mut free).unwrap();
                assert_eq!(
                    &contract.subjects().kernel_id,
                    kernel.kernel_id().as_bytes()
                );
                assert_eq!(contract.read_count(), inputs);
                assert_eq!(contract.argument_count(), inputs + 1);
                assert_eq!(contract.premise_count(), 4 + 3 * inputs);
                assert_eq!(kernel.argument_count(), inputs + 1);
                assert_eq!(
                    contract.numerical_domain(),
                    ConditionalNumericalDomainV1::LittleEndianSharedIeeeV1
                );
                let mut args = kernel.arguments();
                while let Some(arg) = args.next(&mut free).unwrap() {
                    assert_eq!(arg.component_count(), 2);
                    view.source_type(arg.source_type(), &mut free).unwrap();
                    view.device_layout(arg.device_layout(), &mut free).unwrap();
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
fn repeated_input_reads_preserve_independent_alignment_premises() {
    let bytes = fixture::wire_custom("gfx942:xnack-", 1, 1, fixture::mixed_read_alignments);
    let table = decode_device_descriptor_table_v4(&bytes, &mut free).unwrap();
    let kernel = table.kernel(0, &mut free).unwrap();
    let contract = kernel.conditional_contract(&mut free).unwrap();
    assert_eq!(contract.argument_count(), 2);
    assert_eq!(contract.premise_count(), 10);
    let mut reads = contract.reads();
    for alignment in [4, 1] {
        let read = reads.next(&mut free).unwrap().unwrap();
        assert_eq!(
            (read.argument, read.element_bytes, read.alignment),
            (0, 4, alignment)
        );
    }
    assert_eq!(reads.next(&mut free).unwrap(), None);
}

#[test]
fn canonical_argument_order_does_not_determine_nominal_field_or_output_order() {
    let bytes = fixture::wire_custom("gfx950:xnack-", 1, 2, |contract| {
        let first = contract.arguments[0].canonical_parameter;
        contract.arguments[0].canonical_parameter = contract.arguments[2].canonical_parameter;
        contract.arguments[2].canonical_parameter = first;
        contract
            .arguments
            .sort_by_key(|argument| argument.canonical_parameter);
        contract.output.argument = 0;
        for read in &mut contract.reads {
            read.argument = contract
                .arguments
                .iter()
                .position(|argument| argument.generated_field == read.argument)
                .unwrap() as u16;
        }
        contract.refresh_premises();
    });
    let table = decode_device_descriptor_table_v4(&bytes, &mut free).unwrap();
    let kernel = table.kernel(0, &mut free).unwrap();
    let contract = kernel.conditional_contract(&mut free).unwrap();
    assert_eq!(contract.output().argument, 0);
    for (index, field) in [2, 1, 0].into_iter().enumerate() {
        assert_eq!(
            contract.argument(index, &mut free).unwrap().generated_field,
            field
        );
    }
    let mut reads = contract.reads();
    assert_eq!(reads.next(&mut free).unwrap().unwrap().argument, 2);
    assert_eq!(reads.next(&mut free).unwrap().unwrap().argument, 1);
    assert_eq!(reads.next(&mut free).unwrap(), None);
}

#[test]
fn older_decoders_and_relabeling_cannot_drop_contracts() {
    let bytes = fixture::wire("gfx942:xnack-", 1, 2);
    assert!(decode_device_descriptor_table_v1(&bytes).is_err());
    assert!(decode_device_descriptor_table_v2(&bytes).is_err());
    assert!(decode_device_descriptor_table_v3(&bytes, &mut free).is_err());
    for version in [1_u16, 2, 3, 5] {
        let mut changed = bytes.clone();
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        refused(&changed);
        if version == 3 {
            assert!(decode_device_descriptor_table_v3(&changed, &mut free).is_err());
        }
    }
    fixture::with_input("gfx942:xnack-", 1, 2, |input| {
        let mut old =
            vec![0; encoded_device_descriptor_table_v3_len(&input.nominal, &mut free).unwrap()];
        encode_device_descriptor_table_v3(&input.nominal, &mut old, &mut free).unwrap();
        assert!(decode_device_descriptor_table_v3(&old, &mut free).is_ok());
        refused(&old);
        old[8..10].copy_from_slice(&4_u16.to_le_bytes());
        refused(&old);
    });
}

#[test]
fn malformed_transport_and_missing_contracts_are_rejected() {
    let bytes = fixture::wire("gfx942:xnack-", 1, 2);
    for end in 0..bytes.len() {
        refused(&bytes[..end]);
    }
    let start = contract_start(&bytes);
    for offset in [0, 10, 12, start - 40, start - 38, start - 36, start - 32] {
        let mut bad = bytes.clone();
        bad[offset] ^= 0xFF;
        refused(&bad);
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    let n = trailing.len() as u32;
    trailing[12..16].copy_from_slice(&n.to_le_bytes());
    refused(&trailing);
    let mut missing = bytes[..start - 40].to_vec();
    let n = missing.len() as u32;
    missing[12..16].copy_from_slice(&n.to_le_bytes());
    refused(&missing);
    refused(&vec![0; MAX_DESCRIPTOR_TABLE_BYTES + 1]);
    fixture::with_input("gfx942:xnack-", 1, 2, |mut input| {
        input.contracts = &[];
        assert!(encoded_device_descriptor_table_v4_len(&input, &mut free).is_err());
    });
}

#[test]
fn independent_decode_checks_field_source_type_and_layout_joins() {
    let bytes = fixture::wire("gfx942:xnack-", 1, 2);
    let start = contract_start(&bytes);
    let first_argument = start + 624 + 32;
    // Repair the content digest, so refusal must come from structure/theorem or
    // the descriptor join, not merely the enclosing expected contract digest.
    for offset in [
        start + 28,
        start + 16,
        start + 320,
        first_argument + 4,
        first_argument + 20,
        first_argument + 24,
        first_argument + 56,
        start + 576 + 32,
        first_argument + 3 * 88 + 68,
    ] {
        let mut bad = bytes.clone();
        bad[offset] ^= 0x40;
        repair_contract_identity(&mut bad);
        refused(&bad);
    }
    // A canonical alternate input binding is still not a source proof: changing
    // adjusted/source-semantic coordinates changes identity, but cannot be
    // checked against an authenticated source owner by this inert decoder.
    let mut changed = bytes.clone();
    changed[first_argument + 8] ^= 0x40;
    repair_contract_identity(&mut changed);
    let a = decode_device_descriptor_table_v4(&bytes, &mut free).unwrap();
    let b = decode_device_descriptor_table_v4(&changed, &mut free).unwrap();
    assert_ne!(
        a.table_digest(&mut free).unwrap(),
        b.table_digest(&mut free).unwrap()
    );
}

#[test]
fn contracts_cannot_move_between_entries() {
    let mut bytes = fixture::wire("gfx942:xnack-", 2, 2);
    let start = contract_start(&bytes);
    let n = u32::from_le_bytes(bytes[start - 36..start - 32].try_into().unwrap()) as usize;
    let first = bytes[start - 36..start + n].to_vec();
    let second = bytes[start + n..start + n + 36 + n].to_vec();
    bytes[start - 36..start + n].copy_from_slice(&second);
    bytes[start + n..start + n + 36 + n].copy_from_slice(&first);
    refused(&bytes);
}

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
            let changed = DeviceDescriptorTableInputV4 {
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
            assert!(encoded_device_descriptor_table_v4_len(&changed, &mut free).is_err());
            let mut output = vec![0xA5; 4096];
            assert!(encode_device_descriptor_table_v4(&changed, &mut output, &mut free).is_err());
            assert!(output.iter().all(|byte| *byte == 0xA5));
        });
    }
}

#[test]
fn decoder_denial_cannot_return_a_partial_conditional_table() {
    let bytes = fixture::wire("gfx942:xnack-", 1, 1);
    let mut calls = 0;
    decode_device_descriptor_table_v4(&bytes, &mut |_| {
        calls += 1;
        Ok::<(), ()>(())
    })
    .unwrap();
    for deny in 0..calls {
        let mut seen = 0;
        assert!(
            decode_device_descriptor_table_v4(&bytes, &mut |_| {
                seen += 1;
                if seen == deny + 1 { Err(()) } else { Ok(()) }
            })
            .is_err()
        );
    }
}

#[test]
fn every_encoder_denial_precedes_mutation() {
    fixture::with_input("gfx942:xnack-", 1, 1, |input| {
        let n = encoded_device_descriptor_table_v4_len(&input, &mut free).unwrap();
        let mut output = vec![0; n];
        let mut calls = 0;
        encode_device_descriptor_table_v4(&input, &mut output, &mut |_| {
            calls += 1;
            Ok::<(), ()>(())
        })
        .unwrap();
        for deny in 0..calls {
            let mut seen = 0;
            output.fill(0xA5);
            assert!(
                encode_device_descriptor_table_v4(&input, &mut output, &mut |_| {
                    seen += 1;
                    if seen == deny + 1 { Err(()) } else { Ok(()) }
                })
                .is_err()
            );
            assert!(output.iter().all(|b| *b == 0xA5));
        }
        let mut short = vec![0xA5; n - 1];
        assert!(encode_device_descriptor_table_v4(&input, &mut short, &mut free).is_err());
        assert!(short.iter().all(|b| *b == 0xA5));
    });
}
