use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

fn location(operation_index: usize) -> FunctionOperationLocation {
    FunctionOperationLocation::new(BlockId(0), operation_index)
}

fn function(name: &str, parameters: Vec<Type>, operations: Vec<Operation>) -> Function {
    let values = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    Function::internal_helper(
        name,
        Signature::new(parameters, vec![]),
        values,
        vec![block],
    )
}

fn vector_module(lanes: u16) -> Module {
    let vector = FixedVectorTypeV12::new(ScalarType::F32, lanes, VectorLayoutV12::Contiguous);
    let converted = vector.with_layout(VectorLayoutV12::Interleaved { factor: 2 });
    let memory = MemoryAccess::new(AddressSpace::Global, 16);
    let mut module = Module::new("vector_effects");
    module.functions.push(function(
        "vector",
        vec![
            Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly),
            Type::pointer(Type::F32, AddressSpace::Global, AccessMode::WriteOnly),
        ],
        vec![
            Operation::new(
                vec![ValueDef::new(ValueId(2), Type::vector(vector))],
                OperationKind::VectorLoad(VectorLoadOperationV12::new(
                    ValueId(0),
                    VectorMemoryAccessV12::new(vector, memory),
                )),
            ),
            Operation::new(
                vec![ValueDef::new(ValueId(3), Type::vector(converted))],
                OperationKind::VectorLayoutConvert(VectorLayoutConversionV12::new(
                    ValueId(2),
                    converted.layout,
                )),
            ),
            Operation::new(
                vec![],
                OperationKind::VectorStore(VectorStoreOperationV12::new(
                    ValueId(1),
                    ValueId(3),
                    VectorMemoryAccessV12::new(converted, memory),
                )),
            ),
        ],
    ));
    module
}

fn vector_bindings(width: u64, length: u64) -> FunctionEffectBindings {
    let mut bindings = FunctionEffectBindings::new();
    for (pointer, ordinal) in [(0, 0), (1, 2)] {
        bindings.bind_pointer_region(
            ValueId(pointer),
            MemoryRegion::new(
                AllocationId::new(pointer + 1).into(),
                AddressSpace::Global,
                ByteExpression::invocation_affine(0, width),
                ByteExpression::constant(length),
            ),
        );
        bindings.bind_invocations(location(ordinal), InvocationRange1d::from_count(8).unwrap());
        bindings.bind_epoch(location(ordinal), SynchronizationEpoch::new(7));
    }
    bindings
}

#[test]
fn vector_regions_use_full_descriptor_width_and_layout_conversion_has_no_memory_effect() {
    for lanes in [4, 8] {
        let module = vector_module(lanes);
        verify_module(&module).unwrap();
        let width = u64::from(lanes) * 4;
        let report = extract_function_region_effects(
            &module,
            &FunctionId::new("vector"),
            &vector_bindings(width, width),
        )
        .unwrap();
        assert_eq!(report.effects().len(), 2);
        assert_eq!(report.effects()[0].location, location(0));
        assert_eq!(report.effects()[0].pointer, ValueId(0));
        assert_eq!(report.effects()[0].effect.kind, RegionEffectKind::Read);
        assert_eq!(report.effects()[1].location, location(2));
        assert_eq!(report.effects()[1].pointer, ValueId(1));
        assert_eq!(report.effects()[1].effect.kind, RegionEffectKind::Write);
        for effect in report.effects() {
            assert_eq!(effect.effect.access_width, width);
            assert_eq!(effect.effect.alignment, 16);
            assert_eq!(effect.effect.epoch, SynchronizationEpoch::new(7));
        }
        assert_eq!(report.bounds_obligations().len(), 2);
        assert!(report.bounds_obligations().iter().all(|obligation| {
            obligation.access_width == Some(width)
                && obligation.outcome == BoundsObligationOutcome::EstablishedUnderSuppliedBindings
        }));
        assert!(report.extraction_issues().is_empty());
        assert_eq!(
            report.completeness(),
            EffectExtractionCompleteness::CompleteUnderSuppliedBindings
        );
        assert!(report.race_obligations().iter().all(|obligation| matches!(
            obligation.outcome,
            RaceObligationOutcome::NoConflictUnderSuppliedBindings(_)
        )));
    }
}

#[test]
fn vector_regions_do_not_turn_short_or_missing_bindings_into_bounds_proofs() {
    let module = vector_module(4);
    let report = extract_function_region_effects(
        &module,
        &FunctionId::new("vector"),
        &vector_bindings(16, 15),
    )
    .unwrap();
    assert!(
        report
            .bounds_obligations()
            .iter()
            .all(|obligation| matches!(
                obligation.outcome,
                BoundsObligationOutcome::Violated(BoundsViolation::Region(
                    RegionValidationError::AccessExceedsRegion {
                        access_width: 16,
                        byte_length: 15,
                        ..
                    }
                ))
            ))
    );
    let report = extract_function_region_effects(
        &module,
        &FunctionId::new("vector"),
        &FunctionEffectBindings::new(),
    )
    .unwrap();
    assert_eq!(report.effects().len(), 2);
    assert!(
        report
            .bounds_obligations()
            .iter()
            .all(|obligation| matches!(
                obligation.outcome,
                BoundsObligationOutcome::Indeterminate(
                    BoundsIndeterminateReason::MissingPointerRegion { .. }
                )
            ))
    );
    assert!(
        report.race_obligations().iter().any(|obligation| matches!(
            obligation.outcome,
            RaceObligationOutcome::Indeterminate(_)
        ))
    );
}

fn marker_parameters() -> Vec<Type> {
    vec![
        Type::pointer(
            Type::Scalar(ScalarType::I32),
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        ),
        Type::INDEX,
    ]
}

fn marker() -> Operation {
    Operation::new(
        vec![],
        OperationKind::VerificationContract(
            VerificationContractOperationV12::WorkgroupPipelineEvent {
                contract: VerificationContractKeyV12::new(0),
                kind: WorkgroupPipelineEventKindV12::Stage,
                storage: ValueId(0),
                epoch: ValueId(1),
            },
        ),
    )
}

fn call(callee: &str) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new(callee),
            arguments: vec![ValueId(0), ValueId(1)],
        },
    )
}

fn marker_module(indirect: bool) -> Module {
    let mut module = Module::new("marker_effects");
    let mut entry = function(
        "entry",
        marker_parameters(),
        if indirect {
            vec![call("middle")]
        } else {
            vec![marker()]
        },
    );
    entry.role = FunctionRole::KernelEntry;
    module.functions.push(entry);
    if indirect {
        module
            .functions
            .push(function("middle", marker_parameters(), vec![call("leaf")]));
        module
            .functions
            .push(function("leaf", marker_parameters(), vec![marker()]));
    }
    let mut kernel = Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    module.kernels.push(kernel);
    module
}

#[test]
fn marker_only_helpers_are_memory_pure_but_not_compiler_pure_through_two_calls() {
    let module = marker_module(true);
    let analysis = analyze_interprocedural_effects_v1(&module).unwrap();
    for name in ["entry", "middle", "leaf"] {
        let decision = analysis.function(&FunctionId::new(name)).unwrap();
        assert!(decision.is_complete());
        assert!(!decision.is_complete_and_pure());
        assert!(decision.summary().memory().is_pure());
        assert!(decision.summary().effects().is_empty());
        assert!(
            decision
                .summary()
                .compiler_ordering()
                .has_ordered_verification_contract()
        );
        assert!(!decision.summary().is_pure());
    }

    let mut pure = module;
    pure.functions[2].body.as_mut().unwrap().blocks[0]
        .operations
        .clear();
    let analysis = analyze_interprocedural_effects_v1(&pure).unwrap();
    for name in ["entry", "middle", "leaf"] {
        assert!(
            analysis
                .function(&FunctionId::new(name))
                .unwrap()
                .is_complete_and_pure()
        );
    }
}

#[test]
fn partial_marker_summaries_retain_existing_declaration_and_cycle_reasons() {
    for recursive in [false, true] {
        let mut module = marker_module(true);
        module.functions[2].body.as_mut().unwrap().blocks[0]
            .operations
            .push(call(if recursive { "middle" } else { "external" }));
        if !recursive {
            module.functions.push(Function::external_import(
                "external",
                Signature::new(marker_parameters(), vec![]),
            ));
        }
        let analysis = analyze_interprocedural_effects_v1(&module).unwrap();
        for name in ["entry", "middle", "leaf"] {
            let decision = analysis.function(&FunctionId::new(name)).unwrap();
            assert!(!decision.is_complete());
            assert!(!decision.is_complete_and_pure());
            assert!(
                decision
                    .summary()
                    .compiler_ordering()
                    .has_ordered_verification_contract()
            );
            assert!(decision.incomplete_reasons().iter().any(|reason| {
                if recursive {
                    matches!(
                        reason,
                        InterproceduralEffectIncompleteReasonV1::RecursiveCallCycle { .. }
                    )
                } else {
                    matches!(
                        reason,
                        InterproceduralEffectIncompleteReasonV1::FunctionDeclaration { .. }
                    )
                }
            }));
        }
    }
}

#[test]
fn direct_and_indirect_marker_reports_do_not_silently_claim_complete_region_extraction() {
    let module = marker_module(true);
    for (name, expected) in [
        (
            "leaf",
            EffectExtractionIssue::CompilerOrderingEffectsUnavailable {
                location: location(0),
            },
        ),
        (
            "middle",
            EffectExtractionIssue::CallEffectsUnavailable {
                location: location(0),
                callee: FunctionId::new("leaf"),
            },
        ),
        (
            "entry",
            EffectExtractionIssue::CallEffectsUnavailable {
                location: location(0),
                callee: FunctionId::new("middle"),
            },
        ),
    ] {
        let report = extract_function_region_effects(
            &module,
            &FunctionId::new(name),
            &FunctionEffectBindings::new(),
        )
        .unwrap();
        assert!(report.effects().is_empty());
        assert!(report.bounds_obligations().is_empty());
        assert_eq!(report.extraction_issues(), &[expected]);
        assert_eq!(
            report.completeness(),
            EffectExtractionCompleteness::Incomplete
        );
    }
}

#[test]
fn direct_and_indirect_markers_remain_incomplete_for_formal_memory() {
    for indirect in [false, true] {
        let module = marker_module(indirect);
        verify_module(&module).unwrap();
        let analysis = derive_kernel_memory_obligations(
            &module,
            &KernelId::new("kernel"),
            ExplicitLaunchExtent1d::Exact(1),
            FormalIndexWidth::Bits64,
        )
        .unwrap();
        assert!(!analysis.is_complete());
        assert!(analysis.obligations().accesses().is_empty());
        let expected = if indirect {
            FormalMemoryIncompleteReason::CallEffectsUnavailable {
                location: location(0),
                callee: FunctionId::new("middle"),
            }
        } else {
            FormalMemoryIncompleteReason::UnsupportedMemoryEffect {
                location: location(0),
            }
        };
        assert!(analysis.incomplete_reasons().contains(&expected));
    }
}

#[test]
fn compiler_order_propagation_preserves_one_multispace_sync_event_and_distinct_fence() {
    let mut module = marker_module(true);
    let spaces = BTreeSet::from([AddressSpace::Global, AddressSpace::Workgroup]);
    module.functions[2].body.as_mut().unwrap().blocks[0]
        .operations
        .extend([
            Operation::new(
                vec![],
                OperationKind::Barrier(Barrier {
                    execution_scope: SynchronizationScope::Workgroup,
                    memory_scope: SynchronizationScope::Workgroup,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::AcquireRelease,
                        spaces.clone(),
                    ),
                }),
            ),
            Operation::new(
                vec![],
                OperationKind::Fence(Fence {
                    memory_scope: SynchronizationScope::Workgroup,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::AcquireRelease,
                        spaces.clone(),
                    ),
                }),
            ),
        ]);
    module.required_capabilities.extend([
        TargetCapability::WorkgroupMemory,
        TargetCapability::WorkgroupBarrier,
    ]);
    let expected = BTreeSet::from([
        MemoryEffect::Synchronize {
            execution_scope: SynchronizationScope::Workgroup,
            memory_scope: SynchronizationScope::Workgroup,
            address_spaces: spaces.clone(),
        },
        MemoryEffect::Fence {
            memory_scope: SynchronizationScope::Workgroup,
            ordering: MemoryOrdering::AcquireRelease,
            address_spaces: spaces,
        },
    ]);
    let analysis = analyze_interprocedural_effects_v1(&module).unwrap();
    for name in ["entry", "middle", "leaf"] {
        let decision = analysis.function(&FunctionId::new(name)).unwrap();
        assert!(decision.is_complete());
        assert_eq!(decision.summary().effects(), &expected);
        assert!(
            decision
                .summary()
                .compiler_ordering()
                .has_ordered_verification_contract()
        );
        assert!(!decision.is_complete_and_pure());
    }
}
