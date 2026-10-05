//! Inert descriptor candidates exercised only against genuine source owners.
use super::*;
use fe2o3_kernel_descriptor::*;
use fe2o3_lower_mir_kernel::{
    ProductionKernelArgumentAbiKindV18 as Kind, ProductionKernelArgumentAbiRootV18 as Root,
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Mode {
    Success,
    WrongTarget,
    WrongBinding,
    WrongType,
    IncompleteAbi,
    ForeignLedger,
    ExactStorage,
    ShortStorage,
    WorkRefusal,
    RestoredFloor,
}

#[test]
fn mixed_worker_input_frames_have_an_independent_header_oracle() {
    type Owner<'a> = PreparedMixedWorkerInputV26<'a, 'a, 'a, 'a, 'a, 'a>;
    type Frame<'a> = (
        &'a ConditionalMixedTargetLlvmV26<'a, 'a, 'a>,
        &'a DeviceDescriptorTableV3<'a>,
        Abi<'a>,
        &'a mut Budget<'a>,
        Cell<usize>,
        &'a Cell<usize>,
        Vec<Vec<u8>>,
        Vec<u8>,
        Vec<u8>,
        [usize; 12],
        [u8; 32],
        Result<Vec<Vec<u8>>>,
        std::thread::Result<Result<Vec<Vec<u8>>>>,
        KernelDescriptorRefV3<'a, 'a>,
        std::slice::Iter<'a, Root<'a>>,
    );
    let expected = size_of::<Owner<'_>>()
        + align_of::<Owner<'_>>()
        + size_of::<Frame<'_>>()
        + align_of::<Frame<'_>>()
        + size_of::<AssertUnwindSafe<Frame<'_>>>()
        + size_of::<Result<Owner<'_>>>();
    assert_eq!(headers().unwrap(), expected);
}

// This builder is deliberately a test-data producer, not a signed compiler
// descriptor. The real constructor must independently replay every source row.
fn descriptor_fixture(
    native: &ConditionalMixedTargetLlvmV26<'_, '_, '_>,
    roots: &[Root<'_>],
    budget: &mut Budget<'_>,
    mode: Mode,
) -> Result<Vec<u8>> {
    let source_launch = native.source.source_launch(budget)?;
    let mut free = |_: usize| Ok::<(), Resource>(());
    let mut sources = Vec::new();
    let mut layouts = Vec::new();
    let mut components = Vec::new();
    let mut arguments = Vec::new();
    let mut ranges = Vec::new();
    for root in roots {
        let start = arguments.len();
        for input in root.arguments {
            let Kind::Descriptor { source, argument } = &input.kind else {
                continue;
            };
            let source = if matches!(mode, Mode::WrongType) && arguments.is_empty() {
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
        ranges.push(start..arguments.len());
    }
    let fields: Vec<Vec<_>> = ranges
        .iter()
        .map(|range| {
            range
                .clone()
                .enumerate()
                .map(|(index, flat)| {
                    let argument = arguments[flat];
                    LogicalArgumentInputV3 {
                        source_index: index.try_into().unwrap(),
                        name: argument.name().as_str(),
                        source_type: sources[flat].identity(),
                        device_layout: layouts[flat].identity(),
                        ownership: argument.ownership(),
                        access: argument.access(),
                        alias: argument.alias(),
                        components: &components[flat],
                    }
                })
                .collect()
        })
        .collect();
    sources.sort_by_key(|row| row.identity());
    sources.dedup_by_key(|row| row.identity());
    layouts.sort_by_key(|row| row.identity());
    layouts.dedup_by_key(|row| row.identity());
    let launches: Vec<_> = source_launch
        .roots()
        .iter()
        .map(|root| {
            let launch = root.source_launch();
            let [x, y, z] = launch
                .exact_workgroup()
                .expect("actual mixed fixtures have exact workgroup");
            let [gx, gy, gz] = launch.max_grid();
            LaunchConstraintsV1::new(
                launch.rank(),
                BlockSizeV1::Exact(DimensionsV1::new(x, y, z).unwrap()),
                DimensionsV1::new(gx, gy, gz).unwrap(),
                x * y * z,
                0,
                0,
            )
            .unwrap()
        })
        .collect();
    let symbols: Vec<_> = roots
        .iter()
        .map(|root| format!("{}.kd", root.export))
        .collect();
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([1; 32]),
        EvidenceDigest::from_sha256_bytes([2; 32]),
    );
    let mut kernels: Vec<_> = roots
        .iter()
        .enumerate()
        .map(|(index, root)| {
            let mut id = *root.kernel_binding;
            if matches!(mode, Mode::WrongBinding) && index == 0 {
                id[0] ^= 1;
            }
            KernelDescriptorInputV3 {
                kernel_id: KernelId::from_bytes(id),
                logical_name: root.export,
                entry_name: root.export,
                descriptor_symbol: &symbols[index],
                source_evidence: evidence,
                executable_ir_evidence: evidence,
                capabilities: &[CapabilityV1::AmdWave],
                abi_layout: KernelAbiLayoutV1::new(
                    root.explicit_argument_bytes,
                    root.explicit_argument_bytes + 256,
                    root.kernarg_alignment_bytes,
                )
                .unwrap(),
                launch: &launches[index],
                arguments: &fields[index],
            }
        })
        .collect();
    kernels.sort_by_key(|row| row.kernel_id);
    let requirements: Vec<_> = kernels
        .iter()
        .map(|row| {
            KernelTargetRequirementsV2::new(
                row.kernel_id,
                LdsRequirementsV2::new(0, 0).unwrap(),
                RequiredWavefrontWidthV2::Wave64,
                false,
                SynchronizationRequirementsV2::from_bits(0).unwrap(),
                AtomicRequirementsV2::from_bits(0).unwrap(),
            )
        })
        .collect();
    let compiler = CompilerIdentityV1::new(
        Text::new("rustc").unwrap(),
        Text::new("candidate-test").unwrap(),
        [7; 20],
    );
    let producer = ProducerIdentityV1::new(
        Text::new("fe2o3").unwrap(),
        Text::new("candidate-test").unwrap(),
    );
    let target = native.target(budget)?.device_target();
    let target = if matches!(mode, Mode::WrongTarget) {
        if target.starts_with("gfx942") {
            "gfx950:xnack-"
        } else {
            "gfx942:xnack-"
        }
    } else {
        target
    };
    let table = DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
        code_object_version: CodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: DeviceTargetV1::parse(target).unwrap(),
        type_records: &sources,
        layout_records: &layouts,
        kernels: &kernels,
        requirements: &requirements,
    };
    let length = encoded_device_descriptor_table_v3_len(&table, &mut free)
        .map_err(MixedWorkerInputErrorV26::Descriptor)?;
    let mut bytes = vec![0; length];
    encode_device_descriptor_table_v3(&table, &mut bytes, &mut free)
        .map_err(MixedWorkerInputErrorV26::Descriptor)?;
    Ok(bytes)
}

/// Every call requires a fresh real source transaction because negatives can
/// intentionally latch its sticky refusal. No authentic owner is fabricated.
pub(crate) fn genuine_worker_input_case(
    source: &super::super::super::Source<'_>,
    handoff: &super::super::MixedHandoff<'_, '_>,
    roots: &[Root<'_>],
    target: super::super::super::TargetProfile,
    budget: &mut Budget<'_>,
    mode: Mode,
) -> Result<()> {
    let native =
        super::super::check_and_lower_mixed_target_llvm_v26(source, handoff, target, budget)?;
    let baseline_fill = if matches!(mode, Mode::ExactStorage | Mode::ShortStorage) {
        // Raise the live floor to the previous cumulative peak, so the measured
        // new peak necessarily belongs to this constructor, not source import.
        budget.peak_storage() - budget.storage()
    } else {
        0
    };
    budget.reserve_storage(baseline_fill)?;
    let selected = (|| -> Result<()> {
        let bytes = descriptor_fixture(&native, roots, budget, mode)?;
        let table =
            decode_device_descriptor_table_v3(&bytes, &mut |_: usize| Ok::<(), Resource>(()))
                .map_err(MixedWorkerInputErrorV26::Descriptor)?;
        let floor = budget.storage();
        if matches!(mode, Mode::ForeignLedger) {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
            let mut foreign = Budget::new(&mut work, budget.storage_limit());
            foreign.reserve_storage(floor)?;
            let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
            let error =
                prepare_mixed_worker_input_v26(&native, Abi { roots }, &table, &mut foreign)
                    .err()
                    .unwrap();
            assert!(matches!(
                error,
                MixedWorkerInputErrorV26::Source(SourceError::Resource(Resource::Accounting))
            ));
            assert_eq!(
                (foreign.work(), foreign.storage(), foreign.peak_storage()),
                before
            );
            return Err(error);
        }
        if matches!(mode, Mode::IncompleteAbi) {
            let changed = Root {
                kernel_binding: roots[0].kernel_binding,
                export: roots[0].export,
                arguments: &roots[0].arguments[..roots[0].arguments.len() - 1],
                explicit_argument_bytes: roots[0].explicit_argument_bytes,
                kernarg_alignment_bytes: roots[0].kernarg_alignment_bytes,
            };
            let error =
                prepare_mixed_worker_input_v26(&native, Abi { roots: &[changed] }, &table, budget)
                    .err()
                    .unwrap();
            assert!(matches!(error, MixedWorkerInputErrorV26::Source(_)));
            assert_eq!(budget.storage(), floor);
            return Err(error);
        }
        let work_before = budget.work();
        let prepared = prepare_mixed_worker_input_v26(&native, Abi { roots }, &table, budget);
        let required_work = budget.work() - work_before;
        if matches!(
            mode,
            Mode::WrongTarget | Mode::WrongBinding | Mode::WrongType
        ) {
            let error = prepared
                .err()
                .expect("same-count candidate substitution was admitted");
            assert!(
                match (&mode, &error) {
                    (
                        Mode::WrongTarget | Mode::WrongBinding,
                        MixedWorkerInputErrorV26::Mismatch(_),
                    ) => true,
                    (
                        Mode::WrongType,
                        MixedWorkerInputErrorV26::Source(SourceError::Binding(_)),
                    ) => true,
                    _ => false,
                },
                "{mode:?}: {error:?}"
            );
            assert_eq!(budget.storage(), floor);
            return Err(error);
        }
        let prepared = prepared?;
        let retained = prepared.retained_storage(budget)?;
        assert_eq!(budget.storage(), floor + retained);
        assert_eq!(prepared.root_count(budget)?, roots.len());
        assert!(std::ptr::eq(prepared.descriptor(budget)?, &table));
        assert_eq!(prepared.llvm_ir(budget)?, native.llvm_ir(budget)?);
        assert_eq!(
            prepared.open_gates(),
            &[
                MixedWorkerInputOpenGateV26::SignedV18SourceRefinementReceipt,
                MixedWorkerInputOpenGateV26::ProtectedCompilerExecutionJoin,
                MixedWorkerInputOpenGateV26::VersionedWorkerAndFinalizerReplay,
                MixedWorkerInputOpenGateV26::ConcreteRuntimePremiseDischarge,
            ]
        );
        assert!(!prepared.grants_worker_or_artifact_authority());
        for (root, abi) in roots.iter().enumerate() {
            let contract =
                fe2o3_kernel_descriptor::mixed_conditional_v26::decode_mixed_contract_v26(
                    prepared.contract(root, budget)?,
                    &mut |_: usize| Ok::<(), Resource>(()),
                )
                .unwrap();
            assert_eq!(contract.subjects().kernel_id, *abi.kernel_binding);
            assert_eq!(contract.subjects().original_root, root as u32);
            assert_eq!(
                contract.subjects().output_graph_identity,
                *handoff.owner(budget)?.identity().digest()
            );
            let premises = handoff.runtime_premises(budget)?;
            let expected_arguments: Vec<_> =
                premises.iter().filter(|row| row.root() == root).collect();
            assert_eq!(contract.argument_count(), expected_arguments.len());
            for expected in expected_arguments {
                let mut matched = false;
                for index in 0..contract.argument_count() {
                    let actual = contract
                        .argument(index, &mut |_: usize| Ok::<(), Resource>(()))
                        .unwrap();
                    if actual.source_argument == expected.original_argument() {
                        assert!(!matched);
                        assert_eq!(
                            [actual.reads as usize, actual.writes as usize],
                            expected.access_counts()
                        );
                        matched = true;
                    }
                }
                assert!(
                    matched,
                    "unused and accessed source arguments must both survive"
                );
            }
            let expected: Vec<_> = handoff
                .runtime_occurrences(budget)?
                .iter()
                .filter(|row| premises[row.premise_index()].root() == root)
                .collect();
            assert_eq!(contract.occurrence_count(), expected.len());
            let operation = |row: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1| {
                mixed_conditional_v26::MixedOperationV26 {
                    function: row.block.function.0,
                    block: row.block.block,
                    operation: row.operation,
                }
            };
            let definition = |row: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1| {
                use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as D;
                use mixed_conditional_v26::MixedDefinitionV26 as W;
                match row {
                    D::FunctionArgument { function, argument } => W::FunctionArgument {
                        function: function.0,
                        argument,
                    },
                    D::BlockArgument { block, argument } => W::BlockArgument {
                        function: block.function.0,
                        block: block.block,
                        argument,
                    },
                    D::Result {
                        operation: at,
                        result,
                    } => W::Result {
                        operation: operation(at),
                        result,
                    },
                }
            };
            for (index, expected) in expected.iter().enumerate() {
                let actual = contract
                    .occurrence(index, &mut |_: usize| Ok::<(), Resource>(()))
                    .unwrap();
                assert_eq!(
                    actual.original_instance as usize,
                    expected.original_instance()
                );
                assert_eq!(
                    actual.original_operation,
                    operation(expected.original_operation())
                );
                assert_eq!(
                    actual.output_operation,
                    operation(expected.output_operation())
                );
                assert_eq!(
                    actual.original_formation,
                    operation(expected.original_address_formation())
                );
                assert_eq!(
                    actual.output_formation,
                    operation(expected.output_address_formation())
                );
                assert_eq!(
                    actual.output_address_index,
                    definition(expected.output_address_index())
                );
                assert_eq!(
                    actual.output_guard_condition,
                    definition(expected.output_guard_condition())
                );
            }
            assert!(!contract.grants_artifact_or_launch_authority());
        }
        if matches!(mode, Mode::RestoredFloor) {
            budget.release_storage(1)?;
            assert!(prepared.contract(0, budget).is_err());
            budget.reserve_storage(1)?;
            let paid = budget.storage();
            let error = prepared.discard(budget).unwrap_err();
            assert_eq!(
                budget.storage(),
                paid,
                "restored floor must not restore custody"
            );
            return Err(error);
        }
        prepared.discard(budget)?;
        assert_eq!(budget.storage(), floor);
        if matches!(mode, Mode::WorkRefusal) {
            let limit = crate::production_pipeline::source_owned_v29::WORK_LIMIT;
            budget.charge_work(limit - budget.work() - (required_work - 1))?;
            let error = prepare_mixed_worker_input_v26(&native, Abi { roots }, &table, budget)
                .err()
                .expect("one-work-short mixed Worker input was admitted");
            assert!(matches!(
                error,
                MixedWorkerInputErrorV26::Source(SourceError::Resource(Resource::Work(_)))
                    | MixedWorkerInputErrorV26::Descriptor(DescriptorWireErrorV3::Work(
                        Resource::Work(_)
                    ))
            ));
            assert_eq!(budget.storage(), floor);
            return Err(error);
        }
        if matches!(mode, Mode::ExactStorage | Mode::ShortStorage) {
            // The first attempt began at the previous cumulative peak, so this
            // is its exact whole-attempt peak, including source-emitter scratch.
            let peak = budget.peak_storage();
            let short = usize::from(matches!(mode, Mode::ShortStorage));
            let fill = budget.storage_limit() - peak + short;
            budget.reserve_storage(fill)?;
            let prepared = prepare_mixed_worker_input_v26(&native, Abi { roots }, &table, budget);
            if short == 0 {
                assert_eq!(budget.peak_storage(), budget.storage_limit());
                prepared?.discard(budget)?;
                budget.release_storage(fill)?;
            } else {
                let error = prepared
                    .err()
                    .expect("one-short Worker-input peak was admitted");
                assert!(matches!(
                    error,
                    MixedWorkerInputErrorV26::Source(SourceError::Resource(Resource::Storage(_)))
                ));
                assert_eq!(budget.storage(), floor + fill);
                budget.release_storage(fill)?;
                return Err(error);
            }
        }
        Ok(())
    })();
    let filler_settled = budget
        .release_storage(baseline_fill)
        .map_err(MixedWorkerInputErrorV26::from);
    let settled = native
        .discard(budget)
        .map_err(MixedWorkerInputErrorV26::Source);
    selected.and(filler_settled).and(settled)
}
