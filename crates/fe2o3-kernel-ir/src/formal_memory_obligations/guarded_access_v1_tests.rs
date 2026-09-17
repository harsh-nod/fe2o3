use super::*;
use crate::{BasicBlock, IntrinsicOperation, Kernel, Signature, SwitchCase, ValueDef};

#[test]
fn alias_region_union_preserves_fixed_envelopes_and_whole_allocation() {
    let left = FormalAliasRegionV1::FixedBytes(FormalByteRange {
        start: 8,
        end_exclusive: 12,
    });
    let right = FormalAliasRegionV1::FixedBytes(FormalByteRange {
        start: 4,
        end_exclusive: 20,
    });
    let whole = FormalAliasRegionV1::WholeFormalAllocation;
    assert_eq!(left.union(left), left);
    assert_eq!(left.union(right), right);
    assert_eq!(right.union(left), right);
    for region in [left, right, whole] {
        assert_eq!(region.union(whole), whole);
        assert_eq!(whole.union(region), whole);
    }
}

#[test]
fn execution_lifecycle_with_a_proved_guarded_read_remains_incomplete() {
    use crate::verification_execution_lifecycle_v15::tests::fixture as execution_fixture;

    for with_execution in [false, true] {
        let mut module = execution_fixture(1);
        let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
        assert_eq!(block.id, BlockId(7));
        assert_eq!(block.operations.len(), 6);
        assert_eq!(
            block.operations[2].memory_effects(),
            vec![crate::MemoryEffect::Read(AddressSpace::Global)]
        );
        if !with_execution {
            block.operations.clear();
        }
        let first = block.operations.len();
        let scalar = Type::Scalar(ScalarType::U32);
        let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
        block.operations.extend([
            op(
                100,
                Type::INDEX,
                OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
            ),
            op(
                101,
                Type::INDEX,
                OperationKind::SliceLength { slice: ValueId(0) },
            ),
            op(
                102,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(100),
                    rhs: ValueId(101),
                },
            ),
            op(
                103,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(0)),
            ),
            op(
                104,
                Type::INDEX,
                OperationKind::Select {
                    condition: ValueId(102),
                    true_value: ValueId(100),
                    false_value: ValueId(103),
                },
            ),
            op(
                105,
                pointer.clone(),
                OperationKind::SliceData { slice: ValueId(0) },
            ),
            op(
                106,
                pointer,
                OperationKind::GetElementPointer {
                    base: ValueId(105),
                    offset: ValueId(104),
                },
            ),
            op(
                107,
                scalar.clone(),
                OperationKind::Constant(Constant::U32(0)),
            ),
            op(
                108,
                scalar,
                OperationKind::GuardedLoad {
                    pointer: ValueId(106),
                    predicate: ValueId(102),
                    fallback: ValueId(107),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ]);
        let verified = verify_module_ref(&module).expect("genuine lifecycle and guarded KIR");
        let analysis = derive_kernel_memory_obligations_from_verified(
            verified,
            &KernelId::new("entry"),
            ExplicitLaunchExtent1d::Exact(64),
            FormalIndexWidth::Bits64,
        )
        .unwrap();
        let expected_reasons: Vec<_> = (0..first)
            .map(
                |operation| FormalMemoryIncompleteReason::UnsupportedMemoryEffect {
                    location: FunctionOperationLocation::new(BlockId(7), operation),
                },
            )
            .collect();
        assert_eq!(analysis.incomplete_reasons(), expected_reasons);
        assert_eq!(analysis.is_complete(), !with_execution);
        let obligations = analysis.obligations();
        let [access] = obligations.accesses.as_slice() else {
            panic!("only the ordinary guarded access has a formal row");
        };
        assert_eq!(
            access.location,
            FunctionOperationLocation::new(BlockId(7), first + 8)
        );
        assert_eq!(access.kind, FormalMemoryAccessKind::Read);
        let FormalAccessDomainV1::SliceBounded(domain) = access.domain else {
            panic!("ordinary read must retain its exact proved guard");
        };
        assert_eq!(domain.pointer, ValueId(106));
        assert_eq!(domain.index, ValueId(100));
        assert_eq!(domain.selected_offset, ValueId(104));
        assert_eq!(domain.allocation.parameter_index, 0);
        assert_eq!(domain.path, FormalGuardedPathV1::ExplicitPredicate);
        assert_eq!(obligations.bounds_requirements.len(), 1);
        assert_eq!(
            obligations.bounds_requirements[0].kind(),
            FormalBoundsKindV1::SliceElementAtGuardedIndex(domain)
        );
        assert!(obligations.runtime_alias_requirements.is_empty());
        assert!(obligations.inter_invocation_conflicts.is_empty());
    }
}

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn fixture(switch: Option<Type>) -> Module {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(
            3,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
        op(5, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        op(
            6,
            Type::INDEX,
            OperationKind::Select {
                condition: ValueId(4),
                true_value: ValueId(2),
                false_value: ValueId(5),
            },
        ),
        op(
            7,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            8,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(7),
                offset: ValueId(6),
            },
        ),
    ];
    entry.terminator = Some(if let Some(to) = switch {
        entry.operations.push(op(
            9,
            to.clone(),
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(4),
                to,
            },
        ));
        Terminator::Switch {
            selector: ValueId(9),
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
        }
    } else {
        Terminator::ConditionalBranch {
            condition: ValueId(4),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        }
    });
    let mut yes = BasicBlock::new(BlockId(1));
    yes.operations.push(store(ValueId(8)));
    yes.terminator = Some(Terminator::Return { values: vec![] });
    let mut no = BasicBlock::new(BlockId(2));
    no.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("guarded-formal");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(
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

fn store(pointer: ValueId) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer,
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )
}

fn analyze(module: &Module, extent: u64) -> FormalMemoryObligationAnalysis {
    let verified = verify_module_ref(module).unwrap();
    derive_kernel_memory_obligations_from_verified(
        verified,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(extent),
        FormalIndexWidth::Bits64,
    )
    .unwrap()
}

#[test]
fn true_regions_retain_symbolic_bounds_without_launch_minimum() {
    for switch in [
        None,
        Some(Type::Scalar(ScalarType::I64)),
        Some(Type::Scalar(ScalarType::U64)),
    ] {
        let module = fixture(switch);
        for launch in [1, 64, u64::MAX] {
            let analysis = analyze(&module, launch);
            assert!(analysis.is_complete(), "{analysis:?}");
            let obligations = analysis.obligations();
            assert_eq!(obligations.accesses.len(), 1);
            assert!(obligations.inter_invocation_conflicts.is_empty());
            let access = &obligations.accesses[0];
            let FormalAccessDomainV1::SliceBounded(domain) = access.domain else {
                panic!("guarded domain")
            };
            assert_eq!(domain.pointer, ValueId(8));
            assert_eq!(domain.index, ValueId(2));
            assert_eq!(domain.selected_offset, ValueId(6));
            assert_eq!(domain.allocation.parameter_index, 0);
            assert!(matches!(
                domain.path,
                FormalGuardedPathV1::TrueEdge {
                    target: BlockId(1),
                    ..
                }
            ));
            assert_eq!(obligations.bounds_requirements[0].minimum_byte_len(), None);
            assert_eq!(
                obligations.bounds_requirements[0].kind(),
                FormalBoundsKindV1::SliceElementAtGuardedIndex(domain)
            );
            assert!(!domain.may_access_untrusted_index(0, 0, launch));
            assert!(domain.may_access_untrusted_index(0, 1, launch));
            assert!(!domain.may_access_untrusted_index(1, 1, launch));
            assert!(!domain.may_access_untrusted_index(u64::MAX, u64::MAX, launch));
            assert_eq!(
                InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(obligations),
                Err(FormalMemoryReceiptErrorV1::UnsupportedGuardedRepresentation)
            );
        }
    }
}

#[test]
fn same_pointer_true_then_false_is_checked_at_each_access() {
    let mut module = fixture(None);
    module.functions[0].body.as_mut().unwrap().blocks[2]
        .operations
        .push(store(ValueId(8)));
    let analysis = analyze(&module, 64);
    assert!(!analysis.is_complete());
    assert_eq!(analysis.obligations().accesses.len(), 1);
    assert!(
        matches!(analysis, FormalMemoryObligationAnalysis::Incomplete { ref reasons, .. } if reasons.contains(&FormalMemoryIncompleteReason::GuardedAccessPathUnavailable { location: FunctionOperationLocation::new(BlockId(2), 0), predicate: ValueId(4) }))
    );
}

#[test]
fn alternate_reachable_predecessor_and_default_target_do_not_prove_true() {
    for alternate in [false, true] {
        let mut module = fixture(Some(Type::Scalar(ScalarType::U64)));
        let body = module.functions[0].body.as_mut().unwrap();
        if alternate {
            body.blocks[2].terminator = Some(Terminator::Branch {
                target: BlockId(1),
                arguments: vec![],
            });
        } else {
            let Some(Terminator::Switch { default_target, .. }) = &mut body.blocks[0].terminator
            else {
                unreachable!()
            };
            *default_target = BlockId(1);
        }
        let analysis = analyze(&module, 64);
        assert!(!analysis.is_complete(), "{analysis:?}");
        assert!(analysis.obligations().accesses.is_empty());
    }
}

#[test]
fn explicit_guard_uses_exact_predicate_and_retains_the_write() {
    for matching in [true, false] {
        let mut module = fixture(None);
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks[1].operations.clear();
        body.blocks[0].operations.push(op(
            10,
            Type::BOOL,
            OperationKind::Constant(Constant::Bool(true)),
        ));
        body.blocks[0].operations.push(Operation::new(
            vec![],
            OperationKind::GuardedStore {
                pointer: ValueId(8),
                value: ValueId(1),
                predicate: ValueId(if matching { 4 } else { 10 }),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        let analysis = analyze(&module, 64);
        assert_eq!(analysis.is_complete(), matching, "{analysis:?}");
        if matching {
            let [access] = analysis.obligations().accesses.as_slice() else {
                panic!("one actual write")
            };
            assert_eq!(access.kind, FormalMemoryAccessKind::Write);
            assert!(matches!(
                access.domain,
                FormalAccessDomainV1::SliceBounded(FormalSliceBoundedDomainV1 {
                    path: FormalGuardedPathV1::ExplicitPredicate,
                    ..
                })
            ));
        }
    }
}

#[test]
fn selected_recipe_requires_exact_zero_index_and_slice_subject() {
    for mutation in 0..3 {
        let mut module = fixture(None);
        let body = module.functions[0].body.as_mut().unwrap();
        match mutation {
            0 => body.blocks[0].operations[3].kind = OperationKind::Constant(Constant::Index(1)),
            1 => {
                body.blocks[0].operations[4].kind = OperationKind::Select {
                    condition: ValueId(4),
                    true_value: ValueId(5),
                    false_value: ValueId(2),
                }
            }
            _ => {
                body.blocks[0].operations[2].kind = OperationKind::Compare {
                    predicate: ComparePredicate::LessThanOrEqual,
                    lhs: ValueId(2),
                    rhs: ValueId(3),
                };
            }
        }
        let analysis = analyze(&module, 64);
        assert!(!analysis.is_complete(), "{analysis:?}");
        assert!(analysis.obligations().accesses.is_empty());
    }
}

#[test]
fn rank_two_cannot_reuse_global_x_as_injective_launch_index() {
    let mut module = fixture(None);
    module.kernels[0].domain = LaunchDomain::D2 {
        x: LaunchExtent::Dynamic,
        y: LaunchExtent::Dynamic,
    };
    let analysis = derive_kernel_memory_obligations_from_verified_for_launch(
        verify_module_ref(&module).unwrap(),
        &KernelId::new("kernel"),
        ExplicitLaunchExtent::Exact {
            rank: 2,
            extents: [4, 2, 1],
        },
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(!analysis.is_complete(), "{analysis:?}");
    assert!(analysis.obligations().accesses.is_empty());
}

#[test]
fn mixed_fixed_guarded_pairs_keep_conflicts_and_alias_regions() {
    let mut module = fixture(None);
    module.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .push(store(ValueId(7)));
    let analysis = analyze(&module, 64);
    assert!(analysis.is_complete(), "{analysis:?}");
    let obligations = analysis.obligations();
    assert_eq!(obligations.accesses.len(), 2);
    assert!(
        obligations
            .inter_invocation_conflicts
            .iter()
            .any(
                |pair| pair.left == FunctionOperationLocation::new(BlockId(1), 0)
                    && pair.right == FunctionOperationLocation::new(BlockId(1), 1)
            )
    );
    assert!(
        obligations
            .inter_invocation_conflicts
            .iter()
            .any(|pair| pair.left == pair.right && pair.left.operation_index == 1)
    );

    let mut module = fixture(None);
    module.functions[0].signature.parameters.push(Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    let body = module.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(20));
    body.blocks[1].operations.push(store(ValueId(20)));
    let analysis = analyze(&module, 64);
    assert!(analysis.is_complete(), "{analysis:?}");
    let [alias] = analysis.obligations().runtime_alias_requirements.as_slice() else {
        panic!("retained alias pair")
    };
    assert_eq!(
        alias.left_region(),
        FormalAliasRegionV1::WholeFormalAllocation
    );
    assert_eq!(
        alias.right_accessed_bytes().unwrap(),
        FormalByteRange {
            start: 0,
            end_exclusive: 4
        }
    );
    assert!(!analysis.obligations().inter_invocation_conflicts.is_empty());
}

#[test]
fn fixed_component_work_denial_and_storage_census_are_explicit() {
    for (charge, limit, success) in [
        (RECIPE_WORK, 128, true),
        (RECIPE_WORK, 127, false),
        (USE_WORK, 64, true),
        (USE_WORK, 63, false),
        (BOUNDS_WORK, 16, true),
        (BOUNDS_WORK, 15, false),
    ] {
        let mut ledger = GuardLedger {
            work: CanonicalKernelIrWorkBudgetV1::new(limit),
            bytes: 0,
            records: 0,
        };
        assert_eq!(ledger.charge(charge).is_ok(), success);
    }
    let mut ledger = GuardLedger::new(0).unwrap();
    ledger.bytes = MAX_NEW_BYTES;
    let mut rows = Vec::<ControlRow>::new();
    assert!(matches!(
        ledger.reserve(&mut rows, 1),
        Err(ResourceError::Storage { .. })
    ));
    assert_eq!(rows.capacity(), 0);
    let mut ledger = GuardLedger::new(0).unwrap();
    ledger.records = MAX_FORMAL_MEMORY_RECORDS_V1;
    assert!(matches!(
        ledger.reserve(&mut rows, 1),
        Err(ResourceError::Storage { .. })
    ));
    assert_eq!(rows.capacity(), 0);
}

#[test]
fn old_wire_refuses_each_unrepresentable_record_family_independently() {
    let module = fixture(None);
    let original = analyze(&module, 64).obligations().clone();
    let mut only_access = original.clone();
    only_access.bounds_requirements[0].kind = FormalBoundsKindV1::FixedMinimumBytes(256);
    assert_eq!(
        InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(&only_access),
        Err(FormalMemoryReceiptErrorV1::UnsupportedGuardedRepresentation)
    );
    let mut only_bound = original;
    only_bound.accesses[0].domain = FormalAccessDomainV1::LaunchEnvelope;
    assert_eq!(
        InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(&only_bound),
        Err(FormalMemoryReceiptErrorV1::UnsupportedGuardedRepresentation)
    );

    let mut module = fixture(None);
    module.functions[0].signature.parameters.push(Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    let body = module.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(20));
    body.blocks[1].operations.push(store(ValueId(20)));
    let mut only_alias = analyze(&module, 64).obligations().clone();
    for access in &mut only_alias.accesses {
        access.domain = FormalAccessDomainV1::LaunchEnvelope;
    }
    for bound in &mut only_alias.bounds_requirements {
        bound.kind = FormalBoundsKindV1::FixedMinimumBytes(256);
    }
    assert_eq!(
        only_alias.runtime_alias_requirements[0].left_region(),
        FormalAliasRegionV1::WholeFormalAllocation
    );
    assert_eq!(
        InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(&only_alias),
        Err(FormalMemoryReceiptErrorV1::UnsupportedGuardedRepresentation)
    );
}

#[test]
fn real_recipe_and_use_queries_have_derived_exact_and_one_short_work() {
    let module = fixture(None);
    let _verified = verify_module_ref(&module).unwrap();
    let function = &module.functions[0];
    let (definitions, control) = collect_definitions(function).unwrap();
    let mut guarded =
        GuardedAnalysisV1::new(control.unwrap(), &definitions, function, true).unwrap();
    let gep = &function.body.as_ref().unwrap().blocks[0].operations[6];
    // Seven sorted definitions: every selected recipe lookup visits three
    // comparisons plus final equality (5 work); formal 0 of two costs 4.
    for (limit, success) in [(128 + 6 * 5 + 4, true), (128 + 6 * 5 + 3, false)] {
        guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let result = guarded.recipe(ValueId(8), gep);
        if success {
            let recipe = result.unwrap().expect("actual recipe");
            assert_eq!(recipe.domain.pointer, ValueId(8));
            assert_eq!(guarded.ledger.work.work(), limit);
        } else {
            assert!(matches!(result, Err(ResourceError::Work(_))));
        }
    }
    // One recipe lookup costs 3, access block 1 of three costs 4, and
    // the one-row true-edge lookup costs 3. Explicit guards need no last lookup.
    for (predicate, exact) in [(None, 3 + 64 + 4 + 3), (Some(ValueId(4)), 3 + 64 + 4)] {
        for (limit, success) in [(exact, true), (exact - 1, false)] {
            guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let result = guarded.access(
                FunctionOperationLocation::new(BlockId(1), 0),
                ValueId(8),
                FormalMemoryAccessKind::Write,
                MemoryAccess::new(AddressSpace::Global, 4),
                InvocationRange1d::new(0, 64).unwrap(),
                predicate,
            );
            if success {
                let Ok(Some(access)) = result else {
                    panic!("exact paid actual use")
                };
                assert!(matches!(
                    access.domain,
                    FormalAccessDomainV1::SliceBounded(_)
                ));
                assert_eq!(guarded.ledger.work.work(), exact);
            } else {
                assert!(matches!(
                    result,
                    Err(AccessDerivationError::Resource(ResourceError::Work(_)))
                ));
            }
        }
    }
}

#[test]
fn raw_offset_keeps_legacy_envelope_and_foreign_slice_does_not_join() {
    let mut module = fixture(None);
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[6].kind =
        OperationKind::GetElementPointer {
            base: ValueId(7),
            offset: ValueId(2),
        };
    let analysis = analyze(&module, 64);
    assert!(analysis.is_complete(), "{analysis:?}");
    assert_eq!(
        analysis.obligations().accesses[0].domain,
        FormalAccessDomainV1::LaunchEnvelope
    );
    assert_eq!(
        analysis.obligations().bounds_requirements[0].minimum_byte_len(),
        Some(256)
    );
    InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(analysis.obligations())
        .unwrap();

    let mut module = fixture(None);
    module.functions[0].signature.parameters.push(Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    let body = module.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(20));
    body.blocks[0].operations[5].kind = OperationKind::SliceData { slice: ValueId(20) };
    let analysis = analyze(&module, 64);
    assert!(!analysis.is_complete(), "{analysis:?}");
    assert!(analysis.obligations().accesses.is_empty());
}

#[test]
fn explicit_guarded_load_retains_the_read_without_ranked_discharge() {
    let mut module = fixture(None);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[1].operations.clear();
    body.blocks[0].operations.push(op(
        10,
        Type::Scalar(ScalarType::U32),
        OperationKind::GuardedLoad {
            pointer: ValueId(8),
            predicate: ValueId(4),
            fallback: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    let analysis = analyze(&module, 64);
    assert!(analysis.is_complete(), "{analysis:?}");
    assert_eq!(analysis.obligations().accesses.len(), 1);
    assert_eq!(
        analysis.obligations().accesses[0].kind,
        FormalMemoryAccessKind::Read
    );
    assert!(matches!(
        analysis.obligations().accesses[0].domain,
        FormalAccessDomainV1::SliceBounded(_)
    ));
}

#[test]
fn two_guarded_allocations_keep_a_whole_allocation_alias_pair() {
    let mut module = fixture(None);
    module.functions[0].signature.parameters.push(Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    let body = module.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(20));
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    body.blocks[0].operations.extend([
        op(
            21,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(20) },
        ),
        op(
            22,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(21),
            },
        ),
        op(
            23,
            Type::INDEX,
            OperationKind::Select {
                condition: ValueId(22),
                true_value: ValueId(2),
                false_value: ValueId(5),
            },
        ),
        op(
            24,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(20) },
        ),
        op(
            25,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(24),
                offset: ValueId(23),
            },
        ),
    ]);
    body.blocks[1].operations.push(Operation::new(
        vec![],
        OperationKind::GuardedStore {
            pointer: ValueId(25),
            predicate: ValueId(22),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    let analysis = analyze(&module, 64);
    assert!(analysis.is_complete(), "{analysis:?}");
    let obligations = analysis.obligations();
    assert_eq!(obligations.accesses.len(), 2);
    assert_eq!(obligations.bounds_requirements.len(), 2);
    assert!(obligations.inter_invocation_conflicts.is_empty());
    let [alias] = obligations.runtime_alias_requirements.as_slice() else {
        panic!("distinct actual formal allocations retain alias obligation")
    };
    assert_eq!(
        (alias.left.parameter_index, alias.right.parameter_index),
        (0, 2)
    );
    assert_eq!(
        alias.left_region(),
        FormalAliasRegionV1::WholeFormalAllocation
    );
    assert_eq!(
        alias.right_region(),
        FormalAliasRegionV1::WholeFormalAllocation
    );
}

#[test]
fn wrong_guarded_load_predicate_retains_conservative_read_and_conflict() {
    let mut module = fixture(None);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.push(op(
        10,
        Type::BOOL,
        OperationKind::Constant(Constant::Bool(true)),
    ));
    body.blocks[0].operations.push(op(
        11,
        Type::Scalar(ScalarType::U32),
        OperationKind::GuardedLoad {
            pointer: ValueId(8),
            predicate: ValueId(10),
            fallback: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    let analysis = analyze(&module, 64);
    let location = FunctionOperationLocation::new(BlockId(0), 8);
    assert_eq!(
        analysis.incomplete_reasons(),
        &[FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { location }]
    );
    let obligations = analysis.obligations();
    assert_eq!(obligations.accesses.len(), 2);
    let read = &obligations.accesses[0];
    assert_eq!(read.location, location);
    assert_eq!(read.kind, FormalMemoryAccessKind::Read);
    assert_eq!(read.byte_offset, ByteExpression::Unbounded);
    assert_eq!(read.domain, FormalAccessDomainV1::LaunchEnvelope);
    assert_eq!(read.allocation.parameter_index, 0);
    assert_eq!(obligations.bounds_requirements.len(), 1);
    let [conflict] = obligations.inter_invocation_conflicts.as_slice() else {
        panic!("conservative read still conflicts with the Some write")
    };
    assert_eq!(conflict.left, location);
    assert_eq!(
        conflict.right,
        FunctionOperationLocation::new(BlockId(1), 0)
    );
}

#[test]
fn overwritten_private_index_cannot_recover_stale_global_x_recipe() {
    let mut module = fixture(None);
    let baseline = analyze(&module, 64);
    assert!(baseline.is_complete(), "{baseline:?}");
    assert_eq!(baseline.obligations().accesses.len(), 1);

    let private_access = MemoryAccess::new(AddressSpace::Private, 8);
    let entry = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    entry.operations[2].kind = OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs: ValueId(22),
        rhs: ValueId(3),
    };
    entry.operations[4].kind = OperationKind::Select {
        condition: ValueId(4),
        true_value: ValueId(22),
        false_value: ValueId(5),
    };
    drop(entry.operations.splice(
        2..2,
        [
            op(
                20,
                Type::pointer(Type::INDEX, AddressSpace::Private, AccessMode::ReadWrite),
                OperationKind::Alloca {
                    element: Type::INDEX,
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 8,
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(20),
                    value: ValueId(2),
                    access: private_access,
                },
            ),
            op(21, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(20),
                    value: ValueId(21),
                    access: private_access,
                },
            ),
            op(
                22,
                Type::INDEX,
                OperationKind::Load {
                    pointer: ValueId(20),
                    access: private_access,
                },
            ),
        ],
    ));

    let verified = verify_module_ref(&module).unwrap();
    let function = &module.functions[0];
    let (definitions, _) = collect_definitions(function).unwrap();
    let value_types = collect_types(function);
    let mut reasons = BTreeSet::new();
    let slots = classify_eligible_private_slots(function, &definitions, &value_types, &mut reasons);
    assert!(reasons.is_empty(), "{reasons:?}");
    assert!(slots.contains(&ValueId(20)));
    let sources = collect_private_load_sources(function, &definitions, &value_types, &slots);
    assert_eq!(sources.get(&ValueId(22)), Some(&ValueId(21)));
    assert_ne!(sources.get(&ValueId(22)), Some(&ValueId(2)));

    let analysis = derive_kernel_memory_obligations_from_verified(
        verified,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(!analysis.is_complete());
    assert_eq!(
        analysis.incomplete_reasons(),
        &[FormalMemoryIncompleteReason::UnsupportedIndexExpression {
            location: FunctionOperationLocation::new(BlockId(0), 11),
            index: ValueId(6),
            allocation: FormalAllocationIdentity { parameter_index: 0 },
        },]
    );
    assert!(analysis.obligations().accesses.is_empty());
}

#[test]
fn divergent_private_pointer_join_cannot_inherit_prior_direct_guarded_use() {
    let mut module = fixture(None);
    let baseline = analyze(&module, 64);
    assert!(baseline.is_complete(), "{baseline:?}");
    assert_eq!(baseline.obligations().accesses.len(), 1);

    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    );
    let private_access = MemoryAccess::new(AddressSpace::Private, 8);
    module.functions[0]
        .signature
        .parameters
        .push(pointer.clone());
    let body = module.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(20));
    body.blocks[0].operations.push(op(
        21,
        Type::pointer(
            pointer.clone(),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        ),
        OperationKind::Alloca {
            element: pointer.clone(),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 8,
        },
    ));
    body.blocks[0].operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(21),
            value: ValueId(8),
            access: private_access,
        },
    ));
    for (block, value) in [(1, ValueId(8)), (2, ValueId(20))] {
        body.blocks[block].operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(21),
                value,
                access: private_access,
            },
        ));
        body.blocks[block].terminator = Some(Terminator::Branch {
            target: BlockId(3),
            arguments: vec![],
        });
    }
    let mut join = BasicBlock::new(BlockId(3));
    join.operations.push(op(
        22,
        pointer,
        OperationKind::Load {
            pointer: ValueId(21),
            access: private_access,
        },
    ));
    join.operations.push(store(ValueId(22)));
    join.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(join);

    let verified = verify_module_ref(&module).unwrap();
    let function = &module.functions[0];
    let (definitions, _) = collect_definitions(function).unwrap();
    let value_types = collect_types(function);
    let mut reasons = BTreeSet::new();
    let slots = classify_eligible_private_slots(function, &definitions, &value_types, &mut reasons);
    assert!(reasons.is_empty(), "{reasons:?}");
    assert!(slots.contains(&ValueId(21)));
    let sources = collect_private_load_sources(function, &definitions, &value_types, &slots);
    assert!(!sources.contains_key(&ValueId(22)));

    let analysis = derive_kernel_memory_obligations_from_verified(
        verified,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(!analysis.is_complete());
    assert_eq!(
        analysis.incomplete_reasons(),
        &[FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
            location: FunctionOperationLocation::new(BlockId(3), 0),
            pointer: ValueId(22),
        },]
    );
    let [prior] = analysis.obligations().accesses.as_slice() else {
        panic!("the prior direct Some Store must remain in the partial report")
    };
    assert_eq!(
        prior.location,
        FunctionOperationLocation::new(BlockId(1), 0)
    );
    assert_eq!(prior.kind, FormalMemoryAccessKind::Write);
    let FormalAccessDomainV1::SliceBounded(domain) = prior.domain else {
        panic!("the direct Store retains its own guarded domain")
    };
    assert_eq!(domain.pointer, ValueId(8));
    assert_eq!(
        domain.path,
        FormalGuardedPathV1::TrueEdge {
            source: BlockId(0),
            ordinal: 0,
            target: BlockId(1),
        }
    );
    assert_eq!(analysis.obligations().bounds_requirements.len(), 1);
}
