use super::super::tests::plan;
use super::*;
use fe2o3_kernel_descriptor::mixed_conditional_v26::*;
use fe2o3_kernel_descriptor::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[path = "generated_kfd_mixed_preparation_v53_tests.rs"]
mod preparation_v53_tests;

fn free(_: usize) -> std::result::Result<(), Resource> {
    Ok(())
}
fn geometry() -> AqlDispatchGeometryV1 {
    AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap()
}
fn packed<'a>(
    input: &'a [i32],
    output: &'a mut [i32],
    budget: &mut Budget<'_>,
) -> GeneratedKfdPackedArguments<'a> {
    let plan = plan();
    let (value, retained) = GeneratedKfdArgumentBinding::from_compiler_generated_parts(
        vec![plan.scalar(2, 9_u32).unwrap()],
        vec![
            GeneratedKfdReadSlice::new(input)
                .bind_argument(&plan, 0)
                .unwrap(),
            GeneratedKfdReadWriteSlice::new(output)
                .bind_argument(&plan, 1)
                .unwrap(),
        ],
    )
    .pack_with_conditional_plan_v1(&plan, budget)
    .unwrap();
    budget.reserve_storage(retained).unwrap();
    value
}

fn descriptor() -> Vec<u8> {
    let sources = [
        SourceTypeDescriptorV3::SharedSlice(ScalarTypeV1::I32),
        SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::I32),
        SourceTypeDescriptorV3::Scalar(ScalarTypeV1::U32),
    ]
    .map(|row| SourceTypeRecordV3::new(row, &mut free).unwrap());
    let layouts = [
        DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::I32),
        DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::I32),
        DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U32),
    ]
    .map(|row| device_layout_record_v3(row, &mut free).unwrap());
    let pointer = |offset, access, alias| PhysicalComponentV3 {
        kind: PhysicalAbiComponentKind::GlobalPointer,
        offset,
        size: 8,
        alignment: 8,
        access,
        alias,
    };
    let length = |offset| PhysicalComponentV3 {
        kind: PhysicalAbiComponentKind::SliceLengthU64,
        offset,
        size: 8,
        alignment: 8,
        access: AccessMode::ByValue,
        alias: AliasSemantics::Value,
    };
    let input_components = [
        pointer(0, AccessMode::ReadOnly, AliasSemantics::SharedReadOnly),
        length(8),
    ];
    let output_components = [
        pointer(16, AccessMode::ReadWrite, AliasSemantics::Exclusive),
        length(24),
    ];
    let scalar_components = [PhysicalComponentV3 {
        kind: PhysicalAbiComponentKind::ScalarByValue(ScalarTypeV1::U32),
        offset: 32,
        size: 4,
        alignment: 4,
        access: AccessMode::ByValue,
        alias: AliasSemantics::Value,
    }];
    let fields = [
        LogicalArgumentInputV3 {
            source_index: 0,
            name: "input",
            source_type: sources[0].identity(),
            device_layout: layouts[0].identity(),
            ownership: OwnershipSemantics::SharedBorrow,
            access: AccessMode::ReadOnly,
            alias: AliasSemantics::SharedReadOnly,
            components: &input_components,
        },
        LogicalArgumentInputV3 {
            source_index: 1,
            name: "output",
            source_type: sources[1].identity(),
            device_layout: layouts[1].identity(),
            ownership: OwnershipSemantics::UniqueBorrow,
            access: AccessMode::ReadWrite,
            alias: AliasSemantics::Exclusive,
            components: &output_components,
        },
        LogicalArgumentInputV3 {
            source_index: 2,
            name: "count",
            source_type: sources[2].identity(),
            device_layout: layouts[2].identity(),
            ownership: OwnershipSemantics::ByValue,
            access: AccessMode::ByValue,
            alias: AliasSemantics::Value,
            components: &scalar_components,
        },
    ];
    let launch = LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Any,
        DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
        1024,
        0,
        0,
    )
    .unwrap();
    let id = KernelId::from_bytes([0x42; 32]);
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([1; 32]),
        EvidenceDigest::from_sha256_bytes([2; 32]),
    );
    let kernels = [KernelDescriptorInputV3 {
        kernel_id: id,
        logical_name: "mixed",
        entry_name: "mixed",
        descriptor_symbol: "mixed.kd",
        source_evidence: evidence,
        executable_ir_evidence: evidence,
        capabilities: &[CapabilityV1::AmdWave],
        abi_layout: KernelAbiLayoutV1::new(40, 296, 8).unwrap(),
        launch: &launch,
        arguments: &fields,
    }];
    let requirements = [KernelTargetRequirementsV2::new(
        id,
        LdsRequirementsV2::new(0, 0).unwrap(),
        RequiredWavefrontWidthV2::Wave64,
        false,
        SynchronizationRequirementsV2::from_bits(0).unwrap(),
        AtomicRequirementsV2::from_bits(0).unwrap(),
    )];
    let mut sources = sources.to_vec();
    let mut layouts = layouts.to_vec();
    sources.sort_by_key(|row| row.identity());
    layouts.sort_by_key(|row| row.identity());
    let compiler = CompilerIdentityV1::new(
        Text::new("rustc").unwrap(),
        Text::new("mixed-test").unwrap(),
        [7; 20],
    );
    let producer = ProducerIdentityV1::new(
        Text::new("fe2o3").unwrap(),
        Text::new("mixed-test").unwrap(),
    );
    let input = DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
        code_object_version: CodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        type_records: &sources,
        layout_records: &layouts,
        kernels: &kernels,
        requirements: &requirements,
    };
    let n = encoded_device_descriptor_table_v3_len(&input, &mut free).unwrap();
    let mut bytes = vec![0; n];
    encode_device_descriptor_table_v3(&input, &mut bytes, &mut free).unwrap();
    bytes
}

fn contract(
    table: &DeviceDescriptorTableV3<'_>,
    unused_input: bool,
    mutate: impl FnOnce(
        &mut MixedContractSubjectsV26,
        &mut Vec<MixedArgumentV26>,
        &mut Vec<MixedOccurrenceV26>,
    ),
) -> Vec<u8> {
    let kernel = table
        .find_kernel(KernelId::from_bytes([0x42; 32]), &mut free)
        .unwrap();
    let mut cursor = kernel.arguments();
    let mut args = Vec::new();
    for i in 0..2 {
        let row = cursor.next(&mut free).unwrap().unwrap();
        args.push(MixedArgumentV26 {
            source_argument: i,
            generated_field: i as u16,
            physical_parameter: i + 10,
            semantic_type: i + 20,
            semantic_type_identity: [i as u8 + 10; 32],
            descriptor_type_identity: *row.source_type().as_bytes(),
            device_layout_identity: *row.device_layout().as_bytes(),
            scalar: MixedScalarV26::I32,
            pointer_offset: 16 * i,
            length_offset: 16 * i + 8,
            reads: if i == 0 && unused_input { 0 } else { 1 },
            writes: u32::from(i == 1),
            source_exclusive: i == 1,
        });
    }
    let op = |function, block, operation| MixedOperationV26 {
        function,
        block,
        operation,
    };
    let occurrence = |argument, ordinal, writing| MixedOccurrenceV26 {
        argument,
        original_instance: 3,
        original_operation: op(6, 4, ordinal),
        output_operation: op(2, 5, ordinal),
        original_formation: op(6, 1, ordinal),
        output_formation: op(2, 2, ordinal),
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
            operation: op(2, 4, 8),
            result: 0,
        },
        slice_value: 19,
        pointer_value: 29,
        index_value: 39,
        guard_index_value: 49,
        length_value: 59,
        predicate_value: 69,
        path: MixedGuardPathV26::ExplicitPredicate,
        element_bytes: 4,
        alignment: 4,
        address_space: MixedMemorySpaceV26::Global,
        writing,
        volatile: false,
        invocation_axis: 0,
        invocation_value: 39,
        access_envelope: MixedIndexEnvelopeV26::LogicalExtent { argument },
        formation_envelope: MixedIndexEnvelopeV26::InvocationAxis { axis: 0 },
    };
    let mut rows = vec![occurrence(1, 2, false), occurrence(1, 3, true)];
    if !unused_input {
        rows.insert(0, occurrence(0, 1, false));
    }
    let mut subjects = MixedContractSubjectsV26 {
        kernel_id: [0x42; 32],
        source_semantic_identity: [1; 32],
        original_graph_identity: [2; 32],
        output_graph_identity: [3; 32],
        descriptor_identity: mixed_descriptor_subject_v26(table, &mut free).unwrap(),
        original_root: 7,
        output_function: 2,
        source_rank: 1,
        index_width: 64,
        exact_grid: [64 * u64::from(u32::MAX), 1, 1],
        source_argument_count: 3,
        generated_field_count: 3,
        explicit_argument_bytes: 40,
        kernarg_alignment: 8,
    };
    mutate(&mut subjects, &mut args, &mut rows);
    let input = MixedContractInputV26 {
        subjects,
        arguments: &args,
        occurrences: &rows,
    };
    let n = encoded_mixed_contract_v26_len(&input, &mut free).unwrap();
    let mut bytes = vec![0; n];
    encode_mixed_contract_v26(&input, &mut bytes, &mut free).unwrap();
    bytes
}

#[test]
fn generated_mixed_pack_preserves_multiple_accesses_real_borrows_and_runtime_family() {
    let descriptor = descriptor();
    let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
    let bytes = contract(&table, false, |_, _, _| {});
    let contract = decode_mixed_contract_v26(&bytes, &mut free).unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let input = [1, 2, 3];
    let mut output = [0; 3];
    let value = packed(&input, &mut output, &mut budget);
    let plan_storage = value.source_plan_storage;
    let bound = value
        .bind_mixed_conditional_premises_v26(&table, &contract, geometry(), &mut budget)
        .unwrap();
    let retained = bound.retained_storage();
    budget.reserve_storage(retained).unwrap();
    let (value, premises) = bound.into_parts();
    assert_eq!(premises.accesses().len(), 3);
    assert!(premises.unused_slices().is_empty());
    assert_eq!(premises.accesses()[1].slice, premises.accesses()[2].slice);
    assert!(!premises.accesses()[1].writing);
    assert!(premises.accesses()[2].writing);
    assert_ne!(
        premises.accesses()[0].access_domain,
        premises.accesses()[0].address_domain
    );
    let bound = GeneratedMixedKfdArgumentsV26 {
        packed: value,
        premises,
        retained_storage: retained,
    };
    let (runtime, completion) = bound.into_runtime_inputs(geometry(), 0, 1000).unwrap();
    assert!(matches!(runtime.invocation_binding(),
        fe2o3_runtime::Gfx942RuntimeInvocationBindingV1::ConditionalMixedV26 { contract_identity, .. }
        if contract_identity == *contract.identity()
    ));
    assert_eq!(completion.buffers.len(), 2);
    drop((runtime, completion));
    budget.release_storage(plan_storage + retained).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn generated_mixed_generic_representation_preserves_distinct_conditional_occurrence_binding() {
    let descriptor = descriptor();
    let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
    let global_bytes = contract(&table, false, |_, _, _| {});
    let generic_bytes = contract(&table, false, |_, _, rows| {
        for row in rows {
            row.address_space = MixedMemorySpaceV26::Generic;
        }
    });
    let global = decode_mixed_contract_v26(&global_bytes, &mut free).unwrap();
    let generic = decode_mixed_contract_v26(&generic_bytes, &mut free).unwrap();
    assert_ne!(global.identity(), generic.identity());
    assert!(!generic.grants_artifact_or_launch_authority());
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let input = [1, 2, 3];
    let mut output = [0; 3];
    let bound = packed(&input, &mut output, &mut budget)
        .bind_mixed_conditional_premises_v26(&table, &generic, geometry(), &mut budget)
        .unwrap();
    let (value, premises) = bound.into_parts();
    assert_eq!(premises.accesses().len(), 3);
    for (i, access) in premises.accesses().iter().enumerate() {
        assert_eq!(
            access.occurrence_identity,
            generic.occurrence_identity(i, &mut free).unwrap()
        );
        assert_ne!(
            access.occurrence_identity,
            global.occurrence_identity(i, &mut free).unwrap()
        );
        assert_eq!(
            access.writing,
            generic.occurrence(i, &mut free).unwrap().writing
        );
    }
    assert_eq!(value.pointer_fixups.len(), 2);
    let plan_storage = value.source_plan_storage;
    drop((value, premises));
    budget.release_storage(plan_storage).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn generated_mixed_unused_slice_keeps_complete_actual_mapping_without_fake_read() {
    let descriptor = descriptor();
    let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
    let bytes = contract(&table, true, |_, _, _| {});
    let contract = decode_mixed_contract_v26(&bytes, &mut free).unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let input = [1, 2, 3];
    let mut output = [0; 3];
    let bound = packed(&input, &mut output, &mut budget)
        .bind_mixed_conditional_premises_v26(&table, &contract, geometry(), &mut budget)
        .unwrap();
    let (value, premises) = bound.into_parts();
    assert_eq!(premises.slices().len(), 2);
    assert_eq!(premises.accesses().len(), 2);
    assert!(premises.accesses().iter().all(|row| row.slice == 1));
    assert_eq!(
        premises.unused_slices(),
        &[Unused {
            slice: 0,
            source_argument_identity: contract.argument_identity(0, &mut free).unwrap(),
        }]
    );
    assert_eq!(premises.slices()[0].buffer_index, Some(0));
    assert_eq!(value.pointer_fixups.len(), 2);
    assert_eq!(value.completion.buffers.len(), 2);
    let plan_storage = value.source_plan_storage;
    drop((value, premises));
    budget.release_storage(plan_storage).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn generated_mixed_complete_join_refuses_same_count_abi_and_packing_substitution() {
    for fault in 0..10 {
        let descriptor = descriptor();
        let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
        let bytes = contract(&table, false, |subjects, args, _| match fault {
            0 => subjects.descriptor_identity[0] ^= 1,
            1 => args[0].descriptor_type_identity[0] ^= 1,
            2 => args[0].device_layout_identity[0] ^= 1,
            3 => {
                args[0].generated_field = 2;
            }
            4 => args[0].source_exclusive = true,
            5 => {
                args[0].generated_field = 1;
                args[1].generated_field = 0;
            }
            _ => {}
        });
        let contract = decode_mixed_contract_v26(&bytes, &mut free).unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let input = [1, 2, 3];
        let mut output = [0; 3];
        let mut value = packed(&input, &mut output, &mut budget);
        let floor = budget.storage();
        match fault {
            6 => value.explicit_kernarg[32] ^= 1,
            7 => value.packing_observation.buffers.swap(0, 1),
            8 => value.packing_observation.buffers[0].argument_index = 2,
            9 => value.alignment = 16,
            _ => {}
        }
        let result =
            value.bind_mixed_conditional_premises_v26(&table, &contract, geometry(), &mut budget);
        assert!(result.is_err(), "fault {fault}");
        drop(result);
        assert_eq!(budget.storage(), floor);
        budget.release_storage(floor).unwrap();
    }
}

#[test]
fn generated_mixed_exact_and_one_short_join_limits_preserve_inherited_storage() {
    fn run(work_limit: usize, storage_limit: usize) -> (bool, usize, usize, usize) {
        let descriptor = descriptor();
        let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
        let bytes = contract(&table, false, |_, _, _| {});
        let contract = decode_mixed_contract_v26(&bytes, &mut free).unwrap();
        let mut packing_work = Work::new(usize::MAX);
        let mut packing_budget = Budget::new(&mut packing_work, usize::MAX);
        let input = [1, 2, 3];
        let mut output = [0; 3];
        let value = packed(&input, &mut output, &mut packing_budget);
        let floor = value.source_plan_storage;
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result =
            value.bind_mixed_conditional_premises_v26(&table, &contract, geometry(), &mut budget);
        let ok = result.is_ok();
        drop(result);
        assert_eq!(budget.storage(), floor);
        let counters = (ok, budget.work(), budget.storage(), budget.peak_storage());
        budget.release_storage(floor).unwrap();
        packing_budget.release_storage(floor).unwrap();
        counters
    }
    let (ok, work, retained, peak) = run(usize::MAX, usize::MAX);
    assert!(ok);
    assert_eq!(run(work, peak), (true, work, retained, peak));
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
}

#[test]
fn generated_mixed_launch_access_domain_and_empty_bindings_remain_distinct() {
    for fault in 0..3 {
        let descriptor = descriptor();
        let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
        let bytes = contract(&table, false, |_, _, rows| {
            if fault == 1 {
                rows[0].access_envelope = MixedIndexEnvelopeV26::InvocationAxis { axis: 0 };
            }
        });
        let contract = decode_mixed_contract_v26(&bytes, &mut free).unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let input = [1, 2, 3];
        let mut output = [0; 3];
        let value = if fault == 2 {
            packed(&input[..0], &mut output[..0], &mut budget)
        } else {
            packed(&input, &mut output, &mut budget)
        };
        let floor = budget.storage();
        let launch = if fault == 0 {
            AqlDispatchGeometryV1::new([32, 1, 1], [32, 1, 1]).unwrap()
        } else {
            geometry()
        };
        let result =
            value.bind_mixed_conditional_premises_v26(&table, &contract, launch, &mut budget);
        if fault == 2 {
            let (value, premises) = result.unwrap().into_parts();
            assert!(
                premises
                    .slices()
                    .iter()
                    .all(|slice| slice.buffer_index.is_none())
            );
            assert!(value.pointer_fixups.is_empty());
            drop((value, premises));
        } else {
            assert!(result.is_err());
        }
        assert_eq!(budget.storage(), floor);
        budget.release_storage(floor).unwrap();
    }
}

#[test]
fn generated_mixed_physical_envelope_binds_workgroup_and_each_actual_dispatch() {
    let descriptor = descriptor();
    let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
    for (grid, group, wrong_envelope, admitted) in [
        (64, 64, false, true),
        (65, 64, false, true),
        (128, 64, false, true),
        (u32::MAX, 64, false, true),
        (64, 32, false, false),
        (128, 128, false, false),
        (64, 64, true, false),
    ] {
        let bytes = contract(&table, false, |subjects, _, _| {
            if wrong_envelope {
                subjects.exact_grid[0] -= 1;
            }
        });
        let contract = decode_mixed_contract_v26(&bytes, &mut free).unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let input = [1, 2, 3];
        let mut output = [0; 3];
        let value = packed(&input, &mut output, &mut budget);
        let floor = budget.storage();
        let geometry = AqlDispatchGeometryV1::new([grid, 1, 1], [group, 1, 1]).unwrap();
        let result =
            value.bind_mixed_conditional_premises_v26(&table, &contract, geometry, &mut budget);
        assert_eq!(result.is_ok(), admitted, "grid={grid}, workgroup={group}");
        if let Ok(bound) = result {
            let (runtime, completion) = bound.into_runtime_inputs(geometry, 0, 1000).unwrap();
            assert!(matches!(runtime.invocation_binding(),
                fe2o3_runtime::Gfx942RuntimeInvocationBindingV1::ConditionalMixedV26 { contract_identity, .. }
                if contract_identity == *contract.identity()
            ));
            drop((runtime, completion));
        }
        assert_eq!(budget.storage(), floor);
        budget.release_storage(floor).unwrap();
    }
}
