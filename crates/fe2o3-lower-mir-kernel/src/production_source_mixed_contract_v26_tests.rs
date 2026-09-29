fn mixed_source_contract_descriptor_v26(
    root: &ProductionKernelArgumentAbiRootV18<'_>,
    fault: u8,
) -> Result<Vec<u8>, fe2o3_kernel_descriptor::DescriptorWireErrorV3<ArgumentResourceV1>> {
    use fe2o3_kernel_descriptor::*;
    let mut free = |_: usize| Ok::<(), ArgumentResourceV1>(());
    let mut sources = Vec::new();
    let mut layouts = Vec::new();
    let mut components = Vec::new();
    let mut arguments = Vec::new();
    for input in root.arguments {
        if let ProductionKernelArgumentAbiKindV18::Descriptor { source, argument } = &input.kind {
            let source = if fault == 2 && arguments.is_empty() {
                match source {
                    SourceTypeDescriptorV3::SharedSlice(_) => {
                        SourceTypeDescriptorV3::SharedSlice(ScalarTypeV1::U64)
                    }
                    SourceTypeDescriptorV3::DisjointSlice(_) => {
                        SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U64)
                    }
                    other => *other,
                }
            } else {
                *source
            };
            let layout = match source {
                SourceTypeDescriptorV3::Scalar(s) => DeviceLayoutDescriptorV1::scalar(s),
                SourceTypeDescriptorV3::SharedSlice(s) => DeviceLayoutDescriptorV1::shared_slice(s),
                SourceTypeDescriptorV3::DisjointSlice(s) => {
                    DeviceLayoutDescriptorV1::disjoint_slice(s)
                }
                SourceTypeDescriptorV3::GlobalMutPointer(s) => {
                    DeviceLayoutDescriptorV1::global_mut_pointer(s)
                }
                SourceTypeDescriptorV3::Usize => {
                    DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U64)
                }
                SourceTypeDescriptorV3::Isize => {
                    DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::I64)
                }
            };
            sources.push(SourceTypeRecordV3::new(source, &mut free).unwrap());
            let layout = if fault == 7 && arguments.is_empty() {
                DeviceLayoutDescriptorV1::shared_slice(source.physical_scalar())
            } else {
                layout
            };
            layouts.push(device_layout_record_v3(layout, &mut free).unwrap());
            components.push(
                argument
                    .physical_components()
                    .map(|(kind, offset, size, alignment)| PhysicalComponentV3 {
                        kind,
                        offset,
                        size,
                        alignment,
                        access: if kind == PhysicalAbiComponentKind::GlobalPointer {
                            argument.access()
                        } else {
                            AccessMode::ByValue
                        },
                        alias: if kind == PhysicalAbiComponentKind::GlobalPointer {
                            argument.alias()
                        } else {
                            AliasSemantics::Value
                        },
                    })
                    .collect::<Vec<_>>(),
            );
            arguments.push(argument);
        }
    }
    if fault == 5 {
        components[0][0].access = AccessMode::ByValue;
    }
    if fault == 6 {
        components[0][0].alias = AliasSemantics::Value;
    }
    let fields: Vec<_> = arguments
        .iter()
        .enumerate()
        .map(|(index, argument)| LogicalArgumentInputV3 {
            source_index: index.try_into().unwrap(),
            name: argument.name().as_str(),
            source_type: sources[index].identity(),
            device_layout: layouts[index].identity(),
            ownership: argument.ownership(),
            access: argument.access(),
            alias: argument.alias(),
            components: &components[index],
        })
        .collect();
    sources.sort_by_key(|source| source.identity());
    sources.dedup_by_key(|source| source.identity());
    layouts.sort_by_key(|layout| layout.identity());
    layouts.dedup_by_key(|layout| layout.identity());
    let launch = LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
        DimensionsV1::new(if fault == 3 { 2 } else { 1 }, 1, 1).unwrap(),
        64,
        0,
        0,
    )
    .unwrap();
    let mut id = *root.kernel_binding;
    if fault == 1 {
        id[0] ^= 1;
    }
    let id = KernelId::from_bytes(id);
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([1; 32]),
        EvidenceDigest::from_sha256_bytes([2; 32]),
    );
    let kernels = [KernelDescriptorInputV3 {
        kernel_id: id,
        logical_name: root.export,
        entry_name: root.export,
        descriptor_symbol: "mixed_contract_test.kd",
        source_evidence: evidence,
        executable_ir_evidence: evidence,
        capabilities: &[CapabilityV1::AmdWave],
        abi_layout: KernelAbiLayoutV1::new(
            root.explicit_argument_bytes,
            root.explicit_argument_bytes + 256,
            root.kernarg_alignment_bytes,
        )
        .unwrap(),
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
    let compiler = CompilerIdentityV1::new(
        Text::new("rustc").unwrap(),
        Text::new("source-contract-test").unwrap(),
        [7; 20],
    );
    let producer = ProducerIdentityV1::new(
        Text::new("fe2o3").unwrap(),
        Text::new("source-contract-test").unwrap(),
    );
    let table = DeviceDescriptorTableInputV3 {
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
    let n = encoded_device_descriptor_table_v3_len(&table, &mut free)?;
    let mut bytes = vec![0; n];
    encode_device_descriptor_table_v3(&table, &mut bytes, &mut free)?;
    Ok(bytes)
}

fn run_mixed_source_contract_v26(
    fixture: u8,
    fault: u8,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<(), ProductionMixedSourceHandoffErrorV26>,
    usize,
    usize,
    bool,
) {
    mod wire {
        pub(super) use fe2o3_kernel_descriptor::mixed_conditional_v26::*;
        pub(super) use fe2o3_kernel_descriptor::*;
    }
    let owner = match fixture {
        0 => descriptor_source_owner(DescriptorCase::READ),
        1 => issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Arithmetic),
        2 => issued_metadata_two_root_owner_v18(),
        3 => global_expression_helper_owner_v23(true),
        _ => unreachable!(),
    };
    let abi = if fixture != 0 {
        issued_descriptor_role_abi_v18(&owner)
    } else {
        kernel_argument_abi_v18::tests::FixtureKernelAbiV18::new(&owner)
    };
    let semantic = owner.source_semantic();
    let roots = abi.roots();
    let selected_root = usize::from(fixture == 2);
    let descriptor = mixed_source_contract_descriptor_v26(&roots[selected_root], fault).unwrap();
    let table = wire::decode_device_descriptor_table_v3(&descriptor, &mut |_: usize| {
        Ok::<(), ArgumentResourceV1>(())
    })
    .unwrap();
    let launch_roots: Vec<_> = semantic
        .roots()
        .iter()
        .map(|root| {
            let entry = semantic.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    let launch = ProductionSourceLaunchRosterV1::try_new(semantic, &launch_roots).unwrap();
    let sha = *owner.source_semantic_sha256();
    let classes = vec![ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
    let input = ProductionExecutionSourceInputV29 {
        semantic_sha256: &sha,
        roots: &[],
        classes: &classes,
        events: &[],
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut completed = false;
    let result = (|| -> Result<(), ProductionMixedSourceHandoffErrorV26> {
        let prepared =
            ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                owner,
                launch,
                input,
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            )?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let launches = vec![
                fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                    rank: 1,
                    extents: [64, 1, 1]
                };
                source.root_count(budget)?
            ];
            let mut handoff = source.conditional_mixed_worklist_output_v26(
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                &launches,
                fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                budget,
            )?;
            if fault == 8 {
                handoff.premises.clear();
                handoff.occurrences.clear();
            }
            let floor = budget.storage();
            let mut output = vec![
                0xA5;
                if fault == 4 {
                    1
                } else {
                    wire::MAX_MIXED_CONTRACT_BYTES_V26
                }
            ];
            let emitted = handoff.emit_mixed_contract_v26(
                selected_root,
                ProductionKernelArgumentAbiInputV18 { roots: &roots },
                &table,
                0,
                &mut output,
                budget,
            );
            // Inspection queries can exhaust the budget after successful emission.
            // Dispose of the retained handoff before propagating either refusal.
            let inspected = (|| -> SourceOwnedResultV18<()> {
                if let Ok(n) = emitted {
                    assert_eq!(budget.storage(), floor);
                    let mut free = |_: usize| Ok::<(), ArgumentResourceV1>(());
                    let contract =
                        wire::decode_mixed_contract_v26(&output[..n], &mut free).unwrap();
                    let subjects = contract.subjects();
                    assert_eq!(subjects.source_semantic_identity, sha);
                    assert_eq!(
                        subjects.original_graph_identity,
                        *source.canonical(budget)?.identity().digest()
                    );
                    assert_eq!(
                        subjects.output_graph_identity,
                        *handoff.output(budget)?.owner().identity().digest()
                    );
                    assert_eq!(
                        subjects.source_argument_count as usize,
                        roots[selected_root].arguments.len()
                    );
                    assert_eq!(subjects.original_root as usize, selected_root);
                    let actual = handoff.runtime_occurrences(budget)?;
                    let premises = handoff.runtime_premises(budget)?;
                    let selected = actual
                        .iter()
                        .filter(|row| premises[row.premise_index()].root() == selected_root);
                    assert_eq!(contract.occurrence_count(), selected.clone().count());
                    let mut writes = 0;
                    let mut generic = 0;
                    for (i, row) in selected.enumerate() {
                        let encoded = contract.occurrence(i, &mut free).unwrap();
                        assert_eq!(
                            encoded.output_operation.operation,
                            row.output_operation().operation
                        );
                        assert_eq!(encoded.original_instance as usize, row.original_instance());
                        assert_eq!(encoded.index_value, row.domain().index().0);
                        assert_eq!(encoded.writing, row.domain().writing());
                        let expected_space = match row.memory_access().address_space {
                            AddressSpace::Global => wire::MixedMemorySpaceV26::Global,
                            AddressSpace::Generic => {
                                generic += 1;
                                wire::MixedMemorySpaceV26::Generic
                            }
                            other => panic!("unexpected completed memory space: {other:?}"),
                        };
                        assert_eq!(encoded.address_space, expected_space);
                        assert!(matches!(
                            encoded.access_envelope,
                            wire::MixedIndexEnvelopeV26::LogicalExtent { .. }
                        ));
                        writes += usize::from(encoded.writing);
                    }
                    if fixture < 2 {
                        assert_eq!(writes > 0, fixture == 1);
                    }
                    if fixture == 3 {
                        assert!(
                            generic > 0,
                            "nested helper contract retains actual Generic accesses"
                        );
                    }
                    assert!(!handoff.grants_artifact_or_launch_authority());
                    completed = true;
                } else {
                    assert!(output.iter().all(|byte| *byte == 0xA5));
                }
                Ok(())
            })();
            let cleanup = handoff.discard(budget);
            emitted?;
            inspected?;
            cleanup?;
            Ok::<_, ProductionMixedSourceHandoffErrorV26>(())
        })
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn mixed_source_contract_v26_projects_actual_shared_and_rmw_policy9_handoffs() {
    for fixture in [0, 1] {
        let (result, _, _, reached) = run_mixed_source_contract_v26(
            fixture,
            0,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        result.unwrap();
        assert!(reached);
    }
}

#[test]
fn mixed_source_contract_v26_refuses_same_count_kernel_type_launch_and_output_substitution() {
    for fault in [1, 2, 3, 4, 8] {
        let (result, _, _, reached) =
            run_mixed_source_contract_v26(1, fault, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
        assert!(result.is_err(), "fault {fault}");
        assert!(!reached);
    }
}

#[test]
fn mixed_source_contract_v26_preserves_exact_and_one_short_whole_transaction_limits() {
    let (result, work, peak, reached) =
        run_mixed_source_contract_v26(1, 0, OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert!(reached);
    let (result, used, high, reached) = run_mixed_source_contract_v26(1, 0, work, peak);
    result.unwrap();
    assert!(reached);
    assert_eq!((used, high), (work, peak));
    for (work, storage) in [(work - 1, peak), (work, peak - 1)] {
        let (result, _, _, _) = run_mixed_source_contract_v26(1, 0, work, storage);
        assert!(result.is_err());
    }
}

#[test]
fn mixed_source_contract_v26_keeps_selected_root_and_nested_helper_occurrences_distinct() {
    for fixture in [2, 3] {
        let (result, _, _, reached) = run_mixed_source_contract_v26(
            fixture,
            0,
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
        );
        result.unwrap();
        assert!(reached);
    }
}

#[test]
fn mixed_source_contract_v26_descriptor_admission_rejects_noncanonical_component_and_layout_substitution()
 {
    let owner = issued_descriptor_role_owner_v18(DescriptorRoleSourceV18::Arithmetic);
    let abi = issued_descriptor_role_abi_v18(&owner);
    let roots = abi.roots();
    for fault in [5, 6, 7] {
        assert!(
            matches!(
                mixed_source_contract_descriptor_v26(&roots[0], fault),
                Err(fe2o3_kernel_descriptor::DescriptorWireErrorV3::Decode(_))
            ),
            "fault {fault}"
        );
    }
}
