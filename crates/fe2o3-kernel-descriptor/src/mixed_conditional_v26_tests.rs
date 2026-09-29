use super::*;

fn pay(_: usize) -> Result<(), ()> {
    Ok(())
}

fn subjects() -> MixedContractSubjectsV26 {
    MixedContractSubjectsV26 {
        kernel_id: [1; 32],
        source_semantic_identity: [2; 32],
        original_graph_identity: [3; 32],
        output_graph_identity: [4; 32],
        descriptor_identity: [5; 32],
        original_root: 7,
        output_function: 2,
        source_rank: 3,
        index_width: 32,
        exact_grid: [64, 1, 1],
        source_argument_count: 5,
        generated_field_count: 5,
        explicit_argument_bytes: 80,
        kernarg_alignment: 8,
    }
}
fn argument(source: u32, reads: u32, writes: u32) -> MixedArgumentV26 {
    MixedArgumentV26 {
        source_argument: source,
        generated_field: source as u16,
        physical_parameter: source + 9,
        semantic_type: source + 20,
        semantic_type_identity: [6; 32],
        descriptor_type_identity: [7; 32],
        device_layout_identity: [8; 32],
        scalar: MixedScalarV26::U32,
        pointer_offset: source * 16,
        length_offset: source * 16 + 8,
        reads,
        writes,
        source_exclusive: writes != 0,
    }
}
fn operation(function: u32, block: u32, operation: u32) -> MixedOperationV26 {
    MixedOperationV26 {
        function,
        block,
        operation,
    }
}
fn occurrence(argument: u16, ordinal: u32, writing: bool) -> MixedOccurrenceV26 {
    MixedOccurrenceV26 {
        argument,
        original_instance: 3,
        original_operation: operation(6, 4, ordinal),
        output_operation: operation(2, 5, ordinal),
        original_formation: operation(6, 1, ordinal),
        output_formation: operation(2, 2, ordinal),
        output_address_index: MixedDefinitionV26::FunctionArgument {
            function: 2,
            argument: 0,
        },
        output_guard_edge: MixedEdgeV26 {
            function: 2,
            block: 4,
            successor: 0,
        },
        output_guard_condition: MixedDefinitionV26::Result {
            operation: operation(2, 4, 8),
            result: 1,
        },
        slice_value: 19,
        pointer_value: 29,
        index_value: 39,
        guard_index_value: 49,
        length_value: 59,
        predicate_value: 69,
        path: MixedGuardPathV26::TrueEdge {
            source: 87,
            successor: 0,
            target: 42,
        },
        element_bytes: 4,
        alignment: 4,
        address_space: MixedMemorySpaceV26::Global,
        writing,
        volatile: false,
        invocation_axis: 0,
        invocation_value: 39,
        access_envelope: MixedIndexEnvelopeV26::LogicalExtent { argument },
        formation_envelope: MixedIndexEnvelopeV26::InvocationAxis { axis: 0 },
    }
}
fn encoded(
    subjects: MixedContractSubjectsV26,
    arguments: &[MixedArgumentV26],
    occurrences: &[MixedOccurrenceV26],
) -> Vec<u8> {
    let input = MixedContractInputV26 {
        subjects,
        arguments,
        occurrences,
    };
    let len = encoded_mixed_contract_v26_len(&input, &mut pay).unwrap();
    let mut bytes = vec![0; len];
    encode_mixed_contract_v26(&input, &mut bytes, &mut pay).unwrap();
    bytes
}

#[test]
fn mixed_contract_retains_complete_multiaccess_rows_and_distinct_coordinate_spaces() {
    let args = [argument(0, 1, 1), argument(2, 0, 1), argument(4, 1, 0)];
    let rows = [
        occurrence(0, 1, false),
        occurrence(0, 2, true),
        occurrence(1, 3, true),
        occurrence(2, 4, false),
    ];
    let bytes = encoded(subjects(), &args, &rows);
    let view = decode_mixed_contract_v26(&bytes, &mut pay).unwrap();
    assert_eq!(view.subjects(), &subjects());
    assert_eq!((view.argument_count(), view.occurrence_count()), (3, 4));
    for (i, expected) in args.iter().enumerate() {
        assert_eq!(view.argument(i, &mut pay).unwrap(), *expected);
        assert_ne!(
            expected.semantic_type_identity,
            expected.descriptor_type_identity
        );
    }
    for (i, expected) in rows.iter().enumerate() {
        assert_eq!(view.occurrence(i, &mut pay).unwrap(), *expected);
        assert_ne!(expected.original_operation, expected.output_operation);
        assert_ne!(
            expected.output_formation.block,
            expected.output_operation.block
        );
        assert_ne!(expected.access_envelope, expected.formation_envelope);
    }
    assert!(!view.grants_artifact_or_launch_authority());
    assert_eq!(view.canonical_bytes(), bytes);
    assert!(view.argument(3, &mut pay).is_err());
    assert!(view.occurrence(4, &mut pay).is_err());
    assert!(view.occurrence_identity(4, &mut pay).is_err());
}

#[test]
fn mixed_contract_same_count_guard_address_rhs_subject_and_identity_substitutions_change_commitment()
 {
    let args = [argument(0, 0, 1)];
    let row = occurrence(0, 1, true);
    let bytes = encoded(subjects(), &args, &[row]);
    let base = decode_mixed_contract_v26(&bytes, &mut pay).unwrap();
    let base_occurrence = base.occurrence_identity(0, &mut pay).unwrap();
    let mutations: [fn(&mut MixedOccurrenceV26); 14] = [
        |r| r.original_instance += 1,
        |r| r.original_operation.operation += 1,
        |r| r.output_operation.operation += 1,
        |r| r.original_formation.block += 1,
        |r| r.output_formation.block += 1,
        |r| r.output_guard_edge.successor += 1,
        |r| {
            r.output_guard_condition = MixedDefinitionV26::BlockArgument {
                function: 2,
                block: 7,
                argument: 2,
            }
        },
        |r| {
            r.output_address_index = MixedDefinitionV26::Result {
                operation: operation(2, 1, 2),
                result: 3,
            }
        },
        |r| r.predicate_value += 1,
        |r| r.guard_index_value += 1,
        |r| r.index_value += 1,
        |r| r.pointer_value += 1,
        |r| r.path = MixedGuardPathV26::ExplicitPredicate,
        |r| r.formation_envelope = MixedIndexEnvelopeV26::UnsignedWidth { bits: 32 },
    ];
    for mutate in mutations {
        let mut changed = row;
        mutate(&mut changed);
        let bytes = encoded(subjects(), &args, &[changed]);
        let view = decode_mixed_contract_v26(&bytes, &mut pay).unwrap();
        assert_ne!(view.identity(), base.identity());
        assert_ne!(
            view.occurrence_identity(0, &mut pay).unwrap(),
            base_occurrence
        );
        assert_eq!((view.argument_count(), view.occurrence_count()), (1, 1));
    }
    // The complete actual graph commitment includes Store RHS and all control
    // outside the occurrence rows. A selected row digest cannot replace it.
    for field in 0..5 {
        let mut s = subjects();
        match field {
            0 => s.output_graph_identity[0] ^= 1,
            1 => s.original_graph_identity[0] ^= 1,
            2 => s.source_semantic_identity[0] ^= 1,
            3 => s.descriptor_identity[0] ^= 1,
            _ => s.kernel_id[0] ^= 1,
        }
        let bytes = encoded(s, &args, &[row]);
        let view = decode_mixed_contract_v26(&bytes, &mut pay).unwrap();
        assert_ne!(
            view.occurrence_identity(0, &mut pay).unwrap(),
            base_occurrence
        );
    }
    for semantic in [false, true] {
        let mut changed = args;
        if semantic {
            changed[0].semantic_type_identity[0] ^= 1;
        } else {
            changed[0].descriptor_type_identity[0] ^= 1;
        }
        let bytes = encoded(subjects(), &changed, &[row]);
        assert_ne!(
            decode_mixed_contract_v26(&bytes, &mut pay)
                .unwrap()
                .identity(),
            base.identity()
        );
    }
}

#[test]
fn mixed_contract_complete_census_order_and_layout_checks_are_fail_closed() {
    let mut args = [argument(0, 1, 1), argument(2, 0, 1)];
    let rows = [
        occurrence(0, 1, false),
        occurrence(0, 2, true),
        occurrence(1, 3, true),
    ];
    let invalid = |a: &[MixedArgumentV26], r: &[MixedOccurrenceV26], s| {
        assert!(
            encoded_mixed_contract_v26_len(
                &MixedContractInputV26 {
                    subjects: s,
                    arguments: a,
                    occurrences: r
                },
                &mut pay
            )
            .is_err()
        );
    };
    invalid(&args, &rows[..2], subjects());
    args.swap(0, 1);
    invalid(&args, &rows, subjects());
    args.swap(0, 1);
    for mutate in [
        (|a: &mut MixedArgumentV26| a.source_exclusive = false) as fn(&mut MixedArgumentV26),
        |a| a.pointer_offset = a.length_offset,
        |a| a.pointer_offset = u32::MAX,
        |a| a.length_offset = 3,
        |a| a.generated_field = 5,
        |a| a.source_argument = 5,
        |a| a.writes = u32::MAX,
    ] {
        let mut changed = args;
        mutate(&mut changed[0]);
        invalid(&changed, &rows, subjects());
    }
    for mutate in [
        (|r: &mut MixedOccurrenceV26| r.argument = 9) as fn(&mut MixedOccurrenceV26),
        |r| r.volatile = true,
        |r| r.element_bytes = 8,
        |r| r.alignment = 3,
        |r| r.invocation_axis = 255,
        |r| r.invocation_axis = 3,
        |r| r.output_formation.function = 3,
        |r| r.formation_envelope = MixedIndexEnvelopeV26::UnsignedWidth { bits: 64 },
        |r| r.access_envelope = MixedIndexEnvelopeV26::LogicalExtent { argument: 2 },
    ] {
        let mut changed = rows;
        mutate(&mut changed[1]);
        invalid(&args, &changed, subjects());
    }
    let mut s = subjects();
    s.source_rank = 0;
    invalid(&args, &rows, s);
    s = subjects();
    s.source_rank = 1;
    s.exact_grid[1] = 2;
    invalid(&args, &rows, s);
    s = subjects();
    s.index_width = 16;
    invalid(&args, &rows, s);
    s = subjects();
    s.exact_grid[0] = u64::from(u32::MAX) + 1;
    invalid(&args, &rows, s);
    s = subjects();
    s.kernarg_alignment = 3;
    invalid(&args, &rows, s);
}

#[test]
fn mixed_contract_canonical_frames_reject_other_families_padding_and_truncation() {
    let args = [argument(0, 0, 1)];
    let rows = [occurrence(0, 1, true)];
    let bytes = encoded(subjects(), &args, &rows);
    for end in 0..bytes.len() {
        assert!(
            decode_mixed_contract_v26(&bytes[..end], &mut pay).is_err(),
            "prefix {end}"
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(decode_mixed_contract_v26(&trailing, &mut pay).is_err());
    for at in [0, 8, 14, 16] {
        let mut bad = bytes.clone();
        bad[at] ^= 1;
        assert!(decode_mixed_contract_v26(&bad, &mut pay).is_err());
    }
    assert!(crate::decode_conditional_invocation_contract_v1(&bytes, &mut pay).is_err());
    assert!(crate::decode_conditional_invocation_contract_v2(&bytes, &mut pay).is_err());
    // FunctionArgument's unused coordinate words have canonical zero padding.
    let mut bad = bytes.clone();
    let definition = PREFIX
        + MixedContractSubjectsV26::BYTES
        + MixedArgumentV26::BYTES
        + 2
        + 4
        + 4 * MixedOperationV26::BYTES;
    bad[definition + 9] = 1;
    assert!(decode_mixed_contract_v26(&bad, &mut pay).is_err());
}

#[test]
fn mixed_contract_exact_and_one_short_work_keep_output_transactional() {
    let args = [argument(0, 0, 1)];
    let rows = [occurrence(0, 1, true)];
    let input = MixedContractInputV26 {
        subjects: subjects(),
        arguments: &args,
        occurrences: &rows,
    };
    let len = PREFIX
        + MixedContractSubjectsV26::BYTES
        + MixedArgumentV26::BYTES
        + MixedOccurrenceV26::BYTES;
    // Independent equation, not measured by rerunning production with a cap.
    let work = 4096 + 32 * len;
    for (limit, succeeds) in [(work, true), (work - 1, false)] {
        let mut remaining = limit;
        let mut charge = |n| {
            remaining = remaining.checked_sub(n).ok_or("work")?;
            Ok(())
        };
        let mut output = vec![0xa5; len];
        let result = encode_mixed_contract_v26(&input, &mut output, &mut charge);
        assert_eq!(result.is_ok(), succeeds);
        if succeeds {
            assert_eq!(remaining, 0);
            let mut left = work;
            assert!(
                decode_mixed_contract_v26(&output, &mut |n| {
                    left = left.checked_sub(n).ok_or("work")?;
                    Ok::<(), &str>(())
                })
                .is_ok()
            );
            assert_eq!(left, 0);
            assert_eq!(
                decode_mixed_contract_v26(&output, &mut |_| Err("work")).err(),
                Some(MixedContractErrorV26::Resource("work"))
            );
        } else {
            assert_eq!(result, Err(MixedContractErrorV26::Resource("work")));
            assert!(output.iter().all(|byte| *byte == 0xa5));
        }
    }
    let mut bad = rows;
    bad[0].argument = 1;
    let mut output = vec![0xa5; len];
    assert!(
        encode_mixed_contract_v26(
            &MixedContractInputV26 {
                subjects: subjects(),
                arguments: &args,
                occurrences: &bad
            },
            &mut output,
            &mut pay
        )
        .is_err()
    );
    assert!(output.iter().all(|byte| *byte == 0xa5));
}

#[test]
fn mixed_contract_empty_private_root_maximum_rosters_and_index_storage_are_explicit() {
    let empty = encoded(subjects(), &[], &[]);
    let view = decode_mixed_contract_v26(&empty, &mut pay).unwrap();
    assert_eq!((view.argument_count(), view.occurrence_count()), (0, 0));
    let mut s = subjects();
    s.source_argument_count = 64;
    s.generated_field_count = 64;
    s.explicit_argument_bytes = 1024;
    let args: Vec<_> = (0..64).map(|i| argument(i, 2, 2)).collect();
    let rows: Vec<_> = (0..256)
        .map(|i| occurrence((i / 4) as u16, i, i % 4 >= 2))
        .collect();
    let bytes = encoded(s, &args, &rows);
    assert!(bytes.len() <= MAX_MIXED_CONTRACT_BYTES_V26);
    let view = decode_mixed_contract_v26(&bytes, &mut pay).unwrap();
    assert_eq!((view.argument_count(), view.occurrence_count()), (64, 256));
    let mut extra = rows;
    extra.push(extra[0]);
    assert!(
        encoded_mixed_contract_v26_len(
            &MixedContractInputV26 {
                subjects: s,
                arguments: &args,
                occurrences: &extra
            },
            &mut pay
        )
        .is_err()
    );
    let mut a = argument(0, 0, 1);
    a.scalar = MixedScalarV26::Index;
    let mut row = occurrence(0, 1, true);
    row.element_bytes = 8;
    row.alignment = 8;
    // A formal 32-bit arithmetic envelope never narrows the 64-bit Index ABI.
    let bytes = encoded(subjects(), &[a], &[row]);
    assert_eq!(
        decode_mixed_contract_v26(&bytes, &mut pay)
            .unwrap()
            .occurrence(0, &mut pay)
            .unwrap()
            .element_bytes,
        8
    );
}

#[test]
fn mixed_contract_source_argument_and_generated_field_ordinals_are_independent() {
    let mut s = subjects();
    s.source_argument_count = 9;
    s.generated_field_count = 3;
    let mut args = [argument(1, 1, 0), argument(4, 0, 1), argument(8, 1, 0)];
    for (index, arg) in args.iter_mut().enumerate() {
        arg.generated_field = index as u16;
        arg.pointer_offset = index as u32 * 16;
        arg.length_offset = arg.pointer_offset + 8;
    }
    let rows = [
        occurrence(0, 1, false),
        occurrence(1, 2, true),
        occurrence(2, 3, false),
    ];
    let bytes = encoded(s, &args, &rows);
    let view = decode_mixed_contract_v26(&bytes, &mut pay).unwrap();
    assert_eq!(view.argument(2, &mut pay).unwrap().source_argument, 8);
    assert_eq!(view.argument(2, &mut pay).unwrap().generated_field, 2);
    for source in [false, true] {
        let mut bad = args;
        if source {
            bad[2].source_argument = 9;
        } else {
            bad[2].generated_field = 3;
        }
        assert!(
            encoded_mixed_contract_v26_len(
                &MixedContractInputV26 {
                    subjects: s,
                    arguments: &bad,
                    occurrences: &rows
                },
                &mut pay
            )
            .is_err()
        );
    }
}

fn nominal_descriptor(final_digest: u8, name: &str) -> Vec<u8> {
    use crate::*;
    let compiler = CompilerIdentityV1::new(
        Text::new("rustc").unwrap(),
        Text::new("test").unwrap(),
        [1; 20],
    );
    let producer = ProducerIdentityV1::new(Text::new("fe2o3").unwrap(), Text::new("test").unwrap());
    let source =
        SourceTypeRecordV3::new(SourceTypeDescriptorV3::Scalar(ScalarTypeV1::U32), &mut pay)
            .unwrap();
    let layout = device_layout_record_v3(
        DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U32),
        &mut pay,
    )
    .unwrap();
    let components = [PhysicalComponentV3 {
        kind: PhysicalAbiComponentKind::ScalarByValue(ScalarTypeV1::U32),
        offset: 0,
        size: 4,
        alignment: 4,
        access: AccessMode::ByValue,
        alias: AliasSemantics::Value,
    }];
    let arguments = [LogicalArgumentInputV3 {
        source_index: 0,
        name,
        source_type: source.identity(),
        device_layout: layout.identity(),
        ownership: OwnershipSemantics::ByValue,
        access: AccessMode::ByValue,
        alias: AliasSemantics::Value,
        components: &components,
    }];
    let launch = LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Any,
        DimensionsV1::new(64, 1, 1).unwrap(),
        64,
        0,
        0,
    )
    .unwrap();
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([2; 32]),
        EvidenceDigest::from_sha256_bytes([3; 32]),
    );
    let id = KernelId::from_bytes([4; 32]);
    let kernels = [KernelDescriptorInputV3 {
        kernel_id: id,
        logical_name: "kernel",
        entry_name: "kernel",
        descriptor_symbol: "kernel.kd",
        source_evidence: evidence,
        executable_ir_evidence: evidence,
        capabilities: &[CapabilityV1::AmdWave],
        abi_layout: KernelAbiLayoutV1::new(4, 4, 8).unwrap(),
        launch: &launch,
        arguments: &arguments,
    }];
    let requirements = [KernelTargetRequirementsV2::new(
        id,
        LdsRequirementsV2::new(0, 0).unwrap(),
        RequiredWavefrontWidthV2::Wave64,
        false,
        SynchronizationRequirementsV2::empty(),
        AtomicRequirementsV2::empty(),
    )];
    let input = DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([final_digest; 32]),
        code_object_version: CodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        type_records: &[source],
        layout_records: &[layout],
        kernels: &kernels,
        requirements: &requirements,
    };
    let mut bytes = vec![0; encoded_device_descriptor_table_v3_len(&input, &mut pay).unwrap()];
    encode_device_descriptor_table_v3(&input, &mut bytes, &mut pay).unwrap();
    bytes
}

#[test]
fn mixed_descriptor_subject_normalizes_only_final_digest_and_charges_before_hashing() {
    let first = nominal_descriptor(0, "count");
    let finalized = nominal_descriptor(9, "count");
    let other = nominal_descriptor(9, "other");
    let first = crate::decode_device_descriptor_table_v3(&first, &mut pay).unwrap();
    let finalized = crate::decode_device_descriptor_table_v3(&finalized, &mut pay).unwrap();
    let other = crate::decode_device_descriptor_table_v3(&other, &mut pay).unwrap();
    let expected = mixed_descriptor_subject_v26(&first, &mut pay).unwrap();
    assert_eq!(
        mixed_descriptor_subject_v26(&finalized, &mut pay).unwrap(),
        expected
    );
    assert_ne!(
        mixed_descriptor_subject_v26(&other, &mut pay).unwrap(),
        expected
    );
    assert_ne!(
        first.table_digest(&mut pay).unwrap(),
        finalized.table_digest(&mut pay).unwrap()
    );
    let work = first.canonical_bytes().len() + 256;
    let mut remaining = work;
    assert_eq!(
        mixed_descriptor_subject_v26(&first, &mut |n| {
            remaining = remaining.checked_sub(n).ok_or("work")?;
            Ok::<(), &str>(())
        })
        .unwrap(),
        expected
    );
    assert_eq!(remaining, 0);
    let mut remaining = work - 1;
    assert_eq!(
        mixed_descriptor_subject_v26(&first, &mut |n| {
            remaining = remaining.checked_sub(n).ok_or("work")?;
            Ok::<(), &str>(())
        }),
        Err(MixedContractErrorV26::Resource("work"))
    );
}
