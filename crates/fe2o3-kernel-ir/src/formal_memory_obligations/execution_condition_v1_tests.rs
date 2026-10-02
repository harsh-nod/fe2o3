use super::*;
use crate::{BasicBlock, IntrinsicOperation, Kernel, Signature, SwitchCase, ValueDef};

#[test]
fn ordinary_access_roster_is_payload_independent_and_keeps_empty_guarded_domains() {
    let mut module = fixture();
    module.functions[0].signature.parameters = vec![
        Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite),
        Type::F32,
    ];
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.push(op(
        5,
        Type::BOOL,
        OperationKind::Constant(Constant::Bool(false)),
    ));
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    body.blocks[1].operations.extend([
        op(
            6,
            Type::F32,
            OperationKind::Load {
                pointer: ValueId(0),
                access,
            },
        ),
        op(
            7,
            Type::F32,
            OperationKind::GuardedLoad {
                pointer: ValueId(0),
                predicate: ValueId(5),
                fallback: ValueId(1),
                access,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::GuardedStore {
                pointer: ValueId(0),
                predicate: ValueId(5),
                value: ValueId(1),
                access,
            },
        ),
        op(
            8,
            Type::F32,
            OperationKind::Atomic(crate::Atomic {
                kind: crate::AtomicKind::Add,
                pointer: ValueId(0),
                value: Some(ValueId(1)),
                compare: None,
                access,
                scope: crate::SynchronizationScope::Device,
                ordering: crate::MemoryOrdering::Relaxed,
                failure_ordering: None,
            }),
        ),
        op(
            9,
            Type::Scalar(ScalarType::U32),
            OperationKind::Constant(Constant::U32(1)),
        ),
    ]);
    let analysis = analyze(&module);
    assert_eq!(analysis.accesses().len(), 5);
    let verified = verify_module_ref(&module).unwrap();
    for ordinal in 0..5 {
        assert_eq!(
            analysis.accesses()[ordinal].location(),
            location(1, ordinal)
        );
        assert!(singleton_at(&analysis, location(1, ordinal)));
        assert!(analysis.excludes_distinct_invocations(
            verified,
            analysis.kernel(),
            location(1, 0),
            location(1, ordinal),
        ));
    }
    assert!(!analysis.excludes_distinct_invocations(
        verified,
        analysis.kernel(),
        location(1, 0),
        location(1, 5),
    ));
}

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::new(vec![ValueDef::new(ValueId(id), ty)], kind)
}

fn store() -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )
}

fn ret() -> Option<Terminator> {
    Some(Terminator::Return { values: vec![] })
}

fn branch(target: u32) -> Option<Terminator> {
    Some(Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    })
}

fn conditional(predicate: u32, yes: u32, no: u32) -> Option<Terminator> {
    Some(Terminator::ConditionalBranch {
        condition: ValueId(predicate),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    })
}

fn fixture() -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(3, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        op(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
    ];
    entry.terminator = conditional(4, 1, 2);
    let mut yes = BasicBlock::new(BlockId(1));
    yes.operations.push(store());
    yes.terminator = ret();
    let mut no = BasicBlock::new(BlockId(2));
    no.terminator = ret();
    let mut module = Module::new("execution-condition-component");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ),
                Type::Scalar(ScalarType::U32),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![entry, yes, no],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn launch() -> ExplicitLaunchExtent {
    ExplicitLaunchExtent1d::Exact(128).into()
}

fn analyze(module: &Module) -> FormalAccessExecutionAnalysisV1<'_> {
    derive_formal_access_execution_conditions_v1(
        verify_module_ref(module).expect("verified test graph"),
        &KernelId::new("kernel"),
        launch(),
        FormalIndexWidth::Bits64,
    )
    .unwrap()
}

fn location(block: u32, ordinal: usize) -> FunctionOperationLocation {
    FunctionOperationLocation::new(BlockId(block), ordinal)
}

fn singleton_at(
    analysis: &FormalAccessExecutionAnalysisV1<'_>,
    location: FunctionOperationLocation,
) -> bool {
    analysis
        .accesses()
        .iter()
        .find(|row| row.location() == location)
        .is_some_and(|row| row.singleton().is_some())
}

#[test]
fn store_only_positive_keeps_full_bounds_conflicts_and_legacy_selection() {
    let module = fixture();
    let verified = verify_module_ref(&module).unwrap();
    let function = &module.functions[0];
    let flow = analyze_control_flow(function).unwrap();
    assert!(
        GuardedControlV1::collect(function, &flow)
            .unwrap()
            .is_none()
    );
    let analysis = analyze(&module);
    assert_eq!(
        analysis.invocations(),
        Some(InvocationRange1d::from_count(128).unwrap())
    );
    assert_eq!(analysis.launch(), launch());
    assert_eq!(analysis.index_width(), FormalIndexWidth::Bits64);
    assert_eq!(analysis.entry(), &FunctionId::new("entry"));
    assert_eq!(analysis.accesses().len(), 1);
    let condition = analysis.accesses()[0].singleton().unwrap();
    assert_eq!(condition.invocation(), 0);
    assert_eq!(condition.index(), ValueId(2));
    assert_eq!(condition.threshold(), ValueId(3));
    assert_eq!(condition.predicate(), ValueId(4));
    assert_eq!(
        condition.path(),
        FormalGuardedPathV1::TrueEdge {
            source: BlockId(0),
            ordinal: 0,
            target: BlockId(1),
        }
    );
    assert!(!analysis.grants_authority());
    assert!(analysis.excludes_distinct_invocations(
        verified,
        analysis.kernel(),
        location(1, 0),
        location(1, 0),
    ));
    let legacy = derive_kernel_memory_obligations_from_verified_for_launch(
        verified,
        analysis.kernel(),
        launch(),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(legacy.is_complete());
    assert_eq!(
        legacy.obligations().accesses()[0].invocations(),
        analysis.invocations().unwrap()
    );
    assert_eq!(legacy.obligations().inter_invocation_conflicts().len(), 1);
    assert_eq!(
        legacy.obligations().accesses()[0].domain(),
        FormalAccessDomainV1::LaunchEnvelope
    );
}

#[test]
fn false_edge_early_access_and_unreachable_access_do_not_gain_conditions() {
    let mut module = fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.push(store());
    body.blocks[2].operations.push(store());
    let mut unreachable = BasicBlock::new(BlockId(3));
    unreachable.operations.push(store());
    unreachable.terminator = ret();
    body.blocks.push(unreachable);
    let analysis = analyze(&module);
    assert_eq!(analysis.accesses().len(), 3);
    assert!(!singleton_at(&analysis, location(0, 3)));
    assert!(singleton_at(&analysis, location(1, 0)));
    assert!(!singleton_at(&analysis, location(2, 0)));
    let verified = verify_module_ref(&module).unwrap();
    for other in [
        location(0, 3),
        location(2, 0),
        location(3, 0),
        location(1, 9),
    ] {
        assert!(!analysis.excludes_distinct_invocations(
            verified,
            analysis.kernel(),
            location(1, 0),
            other
        ));
        assert!(!analysis.excludes_distinct_invocations(
            verified,
            analysis.kernel(),
            other,
            location(1, 0)
        ));
    }
}

#[test]
fn diamond_bypass_and_duplicate_edges_are_not_true_edge_dominance() {
    for duplicate in [false, true] {
        let mut module = fixture();
        let body = module.functions[0].body.as_mut().unwrap();
        if duplicate {
            body.blocks[0].terminator = conditional(4, 1, 1);
        } else {
            body.blocks[2].terminator = branch(1);
        }
        let analysis = analyze(&module);
        assert!(!singleton_at(&analysis, location(1, 0)));
    }
}

#[test]
fn reused_predicate_at_two_true_edges_is_conservatively_ambiguous() {
    let mut module = fixture();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[1].terminator = conditional(4, 3, 4);
    for id in [3, 4] {
        let mut block = BasicBlock::new(BlockId(id));
        block.operations.push(store());
        block.terminator = ret();
        body.blocks.push(block);
    }
    let analysis = analyze(&module);
    assert!(
        analysis
            .accesses()
            .iter()
            .all(|row| row.singleton().is_none())
    );
}

#[test]
fn dominating_true_region_supports_later_accesses_and_existing_bool_switch() {
    for switch in [false, true] {
        let mut module = fixture();
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks[1].terminator = branch(3);
        let mut later = BasicBlock::new(BlockId(3));
        later.operations.push(store());
        later.terminator = ret();
        body.blocks.push(later);
        if switch {
            body.blocks[0].operations.push(op(
                5,
                Type::Scalar(ScalarType::U32),
                OperationKind::Cast {
                    kind: CastKind::ZeroExtend,
                    value: ValueId(4),
                    to: Type::Scalar(ScalarType::U32),
                },
            ));
            body.blocks[0].terminator = Some(Terminator::Switch {
                selector: ValueId(5),
                cases: vec![
                    SwitchCase {
                        value: 0,
                        target: BlockId(2),
                        arguments: vec![],
                    },
                    SwitchCase {
                        value: 1,
                        target: BlockId(1),
                        arguments: vec![],
                    },
                ],
                default_target: BlockId(2),
                default_arguments: vec![],
            });
        }
        let analysis = analyze(&module);
        assert!(singleton_at(&analysis, location(1, 0)));
        assert!(singleton_at(&analysis, location(3, 0)));
        assert!(analysis.excludes_distinct_invocations(
            verify_module_ref(&module).unwrap(),
            analysis.kernel(),
            location(1, 0),
            location(3, 0),
        ));
    }
}

#[test]
fn wrong_threshold_predicate_index_origin_and_numeric_type_stay_unrestricted() {
    for mutation in 0..6 {
        let mut module = fixture();
        let entry = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
        match mutation {
            0 => entry.operations[1].kind = OperationKind::Constant(Constant::Index(2)),
            1 => {
                entry.operations[2].kind = OperationKind::Compare {
                    predicate: ComparePredicate::LessThanOrEqual,
                    lhs: ValueId(2),
                    rhs: ValueId(3),
                }
            }
            2 => {
                let OperationKind::Intrinsic(intrinsic) = &mut entry.operations[0].kind else {
                    unreachable!()
                };
                intrinsic.kind = IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                };
            }
            3 => {
                let OperationKind::Intrinsic(intrinsic) = &mut entry.operations[0].kind else {
                    unreachable!()
                };
                intrinsic.kind = IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Workgroup,
                    axis: Axis::X,
                };
            }
            4 => {
                entry.operations.insert(
                    2,
                    op(
                        5,
                        Type::Scalar(ScalarType::U64),
                        OperationKind::Cast {
                            kind: CastKind::Bitcast,
                            value: ValueId(2),
                            to: Type::Scalar(ScalarType::U64),
                        },
                    ),
                );
                entry.operations[1] = op(
                    3,
                    Type::Scalar(ScalarType::U64),
                    OperationKind::Constant(Constant::U64(1)),
                );
                entry.operations[3].kind = OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(5),
                    rhs: ValueId(3),
                };
            }
            _ => {
                entry.operations[0] =
                    op(2, Type::INDEX, OperationKind::Constant(Constant::Index(0)));
            }
        }
        let analysis = analyze(&module);
        assert!(
            !singleton_at(&analysis, location(1, 0)),
            "mutation {mutation}"
        );
    }
}

#[test]
fn round_trip_index_cast_and_block_parameter_transport_are_not_inferred() {
    let mut cast = fixture();
    let entry = &mut cast.functions[0].body.as_mut().unwrap().blocks[0];
    entry.operations.insert(
        2,
        op(
            5,
            Type::Scalar(ScalarType::U64),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(2),
                to: Type::Scalar(ScalarType::U64),
            },
        ),
    );
    entry.operations.insert(
        3,
        op(
            6,
            Type::INDEX,
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(5),
                to: Type::INDEX,
            },
        ),
    );
    entry.operations[4].kind = OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs: ValueId(6),
        rhs: ValueId(3),
    };
    assert!(!singleton_at(&analyze(&cast), location(1, 0)));

    let mut phi = fixture();
    let body = phi.functions[0].body.as_mut().unwrap();
    let compare = body.blocks[0].operations.pop().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(2)],
    });
    let mut forwarded = BasicBlock::new(BlockId(3));
    forwarded
        .parameters
        .push(ValueDef::new(ValueId(5), Type::INDEX));
    forwarded.operations.push(compare);
    forwarded.operations[0].kind = OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs: ValueId(5),
        rhs: ValueId(3),
    };
    forwarded.terminator = conditional(4, 1, 2);
    body.blocks.push(forwarded);
    assert!(!singleton_at(&analyze(&phi), location(1, 0)));
}

#[test]
fn launch_rank_shape_width_and_overflow_are_not_implicitly_authenticated() {
    let module = fixture();
    let verified = verify_module_ref(&module).unwrap();
    for (launch, width) in [
        (launch(), FormalIndexWidth::Bits32),
        (launch(), FormalIndexWidth::Unknown),
        (ExplicitLaunchExtent::Unknown, FormalIndexWidth::Bits64),
        (
            ExplicitLaunchExtent1d::Exact(0).into(),
            FormalIndexWidth::Bits64,
        ),
        (
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [128, 2, 1],
            },
            FormalIndexWidth::Bits64,
        ),
        (
            ExplicitLaunchExtent1d::Exact(u64::from(u32::MAX) + 2).into(),
            FormalIndexWidth::Bits32,
        ),
    ] {
        let analysis = derive_formal_access_execution_conditions_v1(
            verified,
            &KernelId::new("kernel"),
            launch,
            width,
        )
        .unwrap();
        assert!(
            analysis
                .accesses()
                .iter()
                .all(|row| row.singleton().is_none())
        );
    }
    for y in [1, 2] {
        let mut ranked = fixture();
        ranked.kernels[0].domain = LaunchDomain::D2 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Dynamic,
        };
        let analysis = derive_formal_access_execution_conditions_v1(
            verify_module_ref(&ranked).unwrap(),
            &KernelId::new("kernel"),
            ExplicitLaunchExtent::Exact {
                rank: 2,
                extents: [128, y, 1],
            },
            FormalIndexWidth::Bits64,
        )
        .unwrap();
        assert!(!singleton_at(&analysis, location(1, 0)));
    }
    let mut overflow = fixture();
    overflow.kernels[0].domain = LaunchDomain::D2 {
        x: LaunchExtent::Dynamic,
        y: LaunchExtent::Dynamic,
    };
    let analysis = derive_formal_access_execution_conditions_v1(
        verify_module_ref(&overflow).unwrap(),
        &KernelId::new("kernel"),
        ExplicitLaunchExtent::Exact {
            rank: 2,
            extents: [u64::MAX, 2, 1],
        },
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert_eq!(analysis.invocations(), None);
    assert!(!singleton_at(&analysis, location(1, 0)));
    let mut fixed = fixture();
    fixed.kernels[0].domain = LaunchDomain::D1 {
        x: LaunchExtent::Static(64),
    };
    assert!(!singleton_at(&analyze(&fixed), location(1, 0)));
}

#[test]
fn owner_kernel_and_actual_access_roster_cannot_be_substituted() {
    let module = fixture();
    let analysis = analyze(&module);
    let copied = module.clone();
    assert!(!analysis.excludes_distinct_invocations(
        verify_module_ref(&copied).unwrap(),
        analysis.kernel(),
        location(1, 0),
        location(1, 0),
    ));
    assert!(!analysis.excludes_distinct_invocations(
        verify_module_ref(&module).unwrap(),
        &KernelId::new("other"),
        location(1, 0),
        location(1, 0),
    ));
    assert!(!analysis.excludes_distinct_invocations(
        verify_module_ref(&module).unwrap(),
        analysis.kernel(),
        location(0, 2),
        location(1, 0),
    ));
    assert!(matches!(
        derive_formal_access_execution_conditions_v1(
            verify_module_ref(&module).unwrap(),
            &KernelId::new("missing"),
            launch(),
            FormalIndexWidth::Bits64,
        ),
        Err(FormalMemoryObligationError::MissingKernel { .. })
    ));
}

#[test]
fn pair_exclusion_requires_both_conditions_and_the_same_invocation() {
    let module = fixture();
    let analysis = analyze(&module);
    let zero = analysis.accesses()[0].singleton().unwrap();
    let one = FormalSingletonExecutionV1 {
        invocation: 1,
        ..zero
    };
    assert!(same_singleton(Some(zero), Some(zero)));
    for pair in [
        (Some(zero), None),
        (None, Some(zero)),
        (None, None),
        (Some(zero), Some(one)),
        (Some(one), Some(zero)),
    ] {
        assert!(!same_singleton(pair.0, pair.1));
    }
}

#[test]
fn execution_index_work_and_storage_exhaustion_are_errors() {
    let module = fixture();
    let verified = verify_module_ref(&module).unwrap();
    let analysis = analyze(&module);
    let exact = derive_with_limits(
        verified,
        analysis.kernel(),
        launch(),
        FormalIndexWidth::Bits64,
        analysis.work(),
        analysis.storage(),
    )
    .unwrap();
    assert_eq!(exact.work(), analysis.work());
    assert_eq!(exact.storage(), analysis.storage());
    for limit in [0, analysis.work() - 1] {
        assert!(matches!(
            derive_with_limits(
                verified,
                analysis.kernel(),
                launch(),
                FormalIndexWidth::Bits64,
                limit,
                MAX_NEW_BYTES,
            ),
            Err(FormalMemoryObligationError::GuardedResource(
                ResourceError::Work(_)
            ))
        ));
    }
    for limit in [0, analysis.storage() - 1] {
        assert!(matches!(
            derive_with_limits(
                verified,
                analysis.kernel(),
                launch(),
                FormalIndexWidth::Bits64,
                crate::MAX_CFG_ANALYSIS_WORK as usize,
                limit,
            ),
            Err(FormalMemoryObligationError::GuardedResource(
                ResourceError::Storage { .. }
            ))
        ));
    }
}

#[test]
fn cfg_control_stage_is_metered_before_allocation_at_exact_and_short_limits() {
    let mut module = fixture();
    // Nonordered physical block IDs also exercise metered radix scratch.
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks.swap(1, 2);
    verify_module_ref(&module).unwrap();
    let function = &module.functions[0];
    let run = |work, storage| {
        execution_control_index(
            function,
            GuardLedger {
                work: CanonicalKernelIrWorkBudgetV1::new(work),
                bytes: MAX_NEW_BYTES - storage,
                records: 0,
            },
        )
    };
    let (seed, peak) = run(crate::MAX_CFG_ANALYSIS_WORK as usize, MAX_NEW_BYTES).unwrap();
    let work = seed.ledger.work.work();
    let storage = peak.max(seed.ledger.bytes);
    assert!(work > 0 && storage > 0);
    let (exact, exact_peak) = run(work, storage).unwrap();
    assert_eq!(exact.ledger.work.work(), work);
    assert_eq!(
        exact_peak.max(exact.ledger.bytes) - (MAX_NEW_BYTES - storage),
        storage
    );
    assert!(matches!(
        run(work - 1, storage),
        Err(ResourceError::Work(_))
    ));
    assert!(matches!(
        run(work, storage - 1),
        Err(ResourceError::Storage { .. })
    ));
    assert!(matches!(run(0, MAX_NEW_BYTES), Err(ResourceError::Work(_))));
    // The owner-header charge fits; zero remaining CFG cells rejects its first
    // allocation through the shared resource budget rather than after a scan.
    assert!(matches!(
        run(work, size_of::<crate::MeteredIndexedControlFlowV1>()),
        Err(ResourceError::Storage { .. })
    ));
}
