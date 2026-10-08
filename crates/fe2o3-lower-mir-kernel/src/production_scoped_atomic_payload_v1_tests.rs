use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn atomic_source(
    op: SemanticAtomicRmwOpV1,
    ordering: SemanticAtomicOrderingV1,
    scope: SemanticAtomicScopeV1,
) -> SemanticAtomicRmwV1 {
    let ty = SemanticTypeIdV1::from_index(1);
    let place = |n| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(n), vec![], ty).unwrap();
    SemanticAtomicRmwV1::new(
        place(3),
        place(1),
        SemanticOperandV1::Copy(place(2)),
        op,
        SemanticAtomicAccessV1::new(ordering, scope),
    )
}
fn physical() -> Atomic {
    Atomic {
        kind: AtomicKind::Add,
        pointer: ValueId(1),
        value: Some(ValueId(2)),
        compare: None,
        access: MemoryAccess::new(AddressSpace::Global, 4),
        scope: SynchronizationScope::System,
        ordering: MemoryOrdering::Relaxed,
        failure_ordering: None,
    }
}
fn operation() -> Operation {
    Operation::new(
        vec![ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32))],
        OperationKind::Atomic(physical()),
    )
}
fn check_operation(
    op: &Operation,
    pointer: ValueId,
    value: ValueId,
    result: ValueId,
    effect: ScopedAtomicEffectV1,
    work_limit: usize,
) -> bool {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, 4096);
    let before = budget.storage();
    let result = tile_atomic_payload_operand_v1(op, pointer, value, result, effect, &mut budget);
    assert_eq!(
        budget.storage(),
        before,
        "operand walk allocates no retained storage"
    );
    matches!(result, Ok(1))
}

#[test]
fn atomic_payload_supported_integer_operations_keep_exact_kind() {
    for (source, expected_kind, scalar) in [
        (
            SemanticAtomicRmwOpV1::Exchange,
            AtomicKind::Exchange,
            ScalarType::U32,
        ),
        (SemanticAtomicRmwOpV1::Add, AtomicKind::Add, ScalarType::U32),
        (
            SemanticAtomicRmwOpV1::Subtract,
            AtomicKind::Subtract,
            ScalarType::U32,
        ),
        (
            SemanticAtomicRmwOpV1::BitAnd,
            AtomicKind::BitAnd,
            ScalarType::U32,
        ),
        (
            SemanticAtomicRmwOpV1::BitOr,
            AtomicKind::BitOr,
            ScalarType::U32,
        ),
        (
            SemanticAtomicRmwOpV1::BitXor,
            AtomicKind::BitXor,
            ScalarType::U32,
        ),
        (
            SemanticAtomicRmwOpV1::SignedMinimum,
            AtomicKind::Min,
            ScalarType::I32,
        ),
        (
            SemanticAtomicRmwOpV1::SignedMaximum,
            AtomicKind::Max,
            ScalarType::I32,
        ),
        (
            SemanticAtomicRmwOpV1::UnsignedMinimum,
            AtomicKind::Min,
            ScalarType::U32,
        ),
        (
            SemanticAtomicRmwOpV1::UnsignedMaximum,
            AtomicKind::Max,
            ScalarType::U32,
        ),
    ] {
        let original = atomic_source(
            source,
            SemanticAtomicOrderingV1::Relaxed,
            SemanticAtomicScopeV1::System,
        );
        let mut effect = ScopedAtomicEffectV1::from_operation(&physical()).unwrap();
        effect.kind = expected_kind;
        assert!(effect.matches_source(&original, scalar, effect.access));
        effect.kind = if expected_kind == AtomicKind::Add {
            AtomicKind::Subtract
        } else {
            AtomicKind::Add
        };
        assert!(!effect.matches_source(&original, scalar, effect.access));
    }
}

#[test]
fn atomic_payload_ordering_and_scope_are_not_weakened() {
    for (source_scope, target_scope) in [
        (
            SemanticAtomicScopeV1::Workgroup,
            SynchronizationScope::Workgroup,
        ),
        (SemanticAtomicScopeV1::Agent, SynchronizationScope::Device),
        (SemanticAtomicScopeV1::System, SynchronizationScope::System),
    ] {
        for (source_order, target_order) in [
            (SemanticAtomicOrderingV1::Relaxed, MemoryOrdering::Relaxed),
            (SemanticAtomicOrderingV1::Acquire, MemoryOrdering::Acquire),
            (SemanticAtomicOrderingV1::Release, MemoryOrdering::Release),
            (
                SemanticAtomicOrderingV1::AcquireRelease,
                MemoryOrdering::AcquireRelease,
            ),
            (
                SemanticAtomicOrderingV1::SequentiallyConsistent,
                MemoryOrdering::SequentiallyConsistent,
            ),
        ] {
            let original = atomic_source(SemanticAtomicRmwOpV1::Add, source_order, source_scope);
            let mut effect = ScopedAtomicEffectV1::from_operation(&physical()).unwrap();
            effect.scope = target_scope;
            effect.ordering = target_order;
            assert!(effect.matches_source(&original, ScalarType::U32, effect.access));
            effect.ordering = if target_order == MemoryOrdering::Relaxed {
                MemoryOrdering::Acquire
            } else {
                MemoryOrdering::Relaxed
            };
            assert!(!effect.matches_source(&original, ScalarType::U32, effect.access));
            effect.ordering = target_order;
            effect.scope = if target_scope == SynchronizationScope::System {
                SynchronizationScope::Workgroup
            } else {
                SynchronizationScope::System
            };
            assert!(!effect.matches_source(&original, ScalarType::U32, effect.access));
        }
    }
}

#[test]
fn atomic_payload_unsupported_source_scope_signedness_and_nand_refuse() {
    let effect = ScopedAtomicEffectV1::from_operation(&physical()).unwrap();
    for scope in [
        SemanticAtomicScopeV1::SingleThread,
        SemanticAtomicScopeV1::Device,
    ] {
        assert!(!effect.matches_source(
            &atomic_source(
                SemanticAtomicRmwOpV1::Add,
                SemanticAtomicOrderingV1::Relaxed,
                scope
            ),
            ScalarType::U32,
            effect.access
        ));
    }
    for (kind, scalar) in [
        (SemanticAtomicRmwOpV1::BitNand, ScalarType::U32),
        (SemanticAtomicRmwOpV1::SignedMinimum, ScalarType::U32),
        (SemanticAtomicRmwOpV1::UnsignedMinimum, ScalarType::I32),
        (SemanticAtomicRmwOpV1::Add, ScalarType::F32),
    ] {
        let mut effect = effect;
        if matches!(
            kind,
            SemanticAtomicRmwOpV1::SignedMinimum | SemanticAtomicRmwOpV1::UnsignedMinimum
        ) {
            effect.kind = AtomicKind::Min;
        }
        assert!(!effect.matches_source(
            &atomic_source(
                kind,
                SemanticAtomicOrderingV1::Relaxed,
                SemanticAtomicScopeV1::System
            ),
            scalar,
            effect.access
        ));
    }
}

#[test]
fn atomic_payload_rmw_shape_excludes_load_store_compare_and_volatile() {
    for change in 0..7 {
        let mut atomic = physical();
        match change {
            0 => atomic.kind = AtomicKind::Load,
            1 => atomic.kind = AtomicKind::Store,
            2 => atomic.kind = AtomicKind::CompareExchange,
            3 => atomic.value = None,
            4 => atomic.compare = Some(ValueId(4)),
            5 => atomic.failure_ordering = Some(MemoryOrdering::Relaxed),
            6 => atomic.access.volatile = true,
            _ => unreachable!(),
        }
        assert!(
            ScopedAtomicEffectV1::from_operation(&atomic).is_none(),
            "mutation {change}"
        );
    }
}

#[test]
fn atomic_payload_access_alignment_space_and_volatility_are_exact() {
    let original = atomic_source(
        SemanticAtomicRmwOpV1::Add,
        SemanticAtomicOrderingV1::Relaxed,
        SemanticAtomicScopeV1::System,
    );
    let effect = ScopedAtomicEffectV1::from_operation(&physical()).unwrap();
    for change in 0..3 {
        let mut expected = effect.access;
        match change {
            0 => expected.alignment = 8,
            1 => expected.address_space = AddressSpace::Workgroup,
            2 => expected.volatile = true,
            _ => unreachable!(),
        }
        assert!(!effect.matches_source(&original, ScalarType::U32, expected));
    }
}

#[test]
fn atomic_payload_actual_use_is_rhs_even_when_pointer_and_value_alias() {
    let effect = ScopedAtomicEffectV1::from_operation(&physical()).unwrap();
    assert!(check_operation(
        &operation(),
        ValueId(1),
        ValueId(2),
        ValueId(3),
        effect,
        100
    ));
    let mut same = operation();
    let OperationKind::Atomic(ref mut atomic) = same.kind else {
        unreachable!();
    };
    atomic.value = Some(atomic.pointer);
    assert!(check_operation(
        &same,
        ValueId(1),
        ValueId(1),
        ValueId(3),
        effect,
        100
    ));
    assert!(!check_operation(
        &same,
        ValueId(1),
        ValueId(2),
        ValueId(3),
        effect,
        100
    ));
}

#[test]
fn atomic_payload_pointer_rhs_result_and_result_census_mutations_refuse() {
    let effect = ScopedAtomicEffectV1::from_operation(&physical()).unwrap();
    for change in 0..5 {
        let mut op = operation();
        match change {
            0 => {
                let OperationKind::Atomic(a) = &mut op.kind else {
                    unreachable!();
                };
                a.pointer = ValueId(4);
            }
            1 => {
                let OperationKind::Atomic(a) = &mut op.kind else {
                    unreachable!();
                };
                a.value = Some(ValueId(4));
            }
            2 => op.results[0].id = ValueId(4),
            3 => op.results.clear(),
            4 => op
                .results
                .push(ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32))),
            _ => unreachable!(),
        }
        assert!(!check_operation(
            &op,
            ValueId(1),
            ValueId(2),
            ValueId(3),
            effect,
            100
        ));
    }
}

#[test]
fn atomic_payload_physical_effect_substitution_refuses() {
    let effect = ScopedAtomicEffectV1::from_operation(&physical()).unwrap();
    for change in 0..5 {
        let mut op = operation();
        let OperationKind::Atomic(a) = &mut op.kind else {
            unreachable!();
        };
        match change {
            0 => a.kind = AtomicKind::Subtract,
            1 => a.scope = SynchronizationScope::Workgroup,
            2 => a.ordering = MemoryOrdering::SequentiallyConsistent,
            3 => a.access.alignment = 8,
            4 => a.access.address_space = AddressSpace::Workgroup,
            _ => unreachable!(),
        }
        assert!(!check_operation(
            &op,
            ValueId(1),
            ValueId(2),
            ValueId(3),
            effect,
            100
        ));
    }
}

#[test]
fn atomic_payload_operand_walk_cannot_run_without_original_work_budget() {
    let effect = ScopedAtomicEffectV1::from_operation(&physical()).unwrap();
    assert!(!check_operation(
        &operation(),
        ValueId(1),
        ValueId(2),
        ValueId(3),
        effect,
        0
    ));
    assert!(!check_operation(
        &operation(),
        ValueId(1),
        ValueId(2),
        ValueId(3),
        effect,
        13
    ));
    assert!(check_operation(
        &operation(),
        ValueId(1),
        ValueId(2),
        ValueId(3),
        effect,
        14
    ));
}

// These constructors exercise the real source/payload checkers. They are not
// a rustc extraction, admitted owner/loan witness, or ordinary-route activation.
fn source_fixture(
    kind: SemanticAtomicRmwOpV1,
) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let ty = SemanticTypeIdV1::from_index(1);
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let word = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    );
    let direct = SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    );
    let locals = (0..4)
        .map(|i| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([10 + i as u8; 32]),
                ty,
                if i == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                provenance,
            )
        })
        .collect();
    let atomic = atomic_source(
        kind,
        SemanticAtomicOrderingV1::Relaxed,
        SemanticAtomicScopeV1::System,
    );
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([20; 32]),
        provenance,
        vec![SemanticStatementV1::new(
            provenance,
            SemanticStatementKindV1::AtomicRmw(atomic),
        )],
        SemanticTerminatorV1::new(provenance, SemanticTerminatorKindV1::Return),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([30; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([31; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([32; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([33; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([34; 32]),
        provenance,
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([35; 32]),
            SemanticLayoutIdentityV1::from_sha256([36; 32]),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![],
            direct,
        )
        .unwrap(),
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap();
    (vec![word.clone(), word], function)
}

fn source_row() -> ScopedMemoryAnchorV29 {
    let site = execution_site_v29(SemanticBlockIdV1::from_index(0), Some(0));
    ScopedMemoryAnchorV29 {
        block: BlockId(0),
        position: 0,
        source: Some(ScopedMemoryFrameV29::operand(
            site,
            Some(ExecutionOperandV29::AtomicAddress),
        )),
        kind: ScopedMemoryAnchorKindV29::Access {
            pointer: ValueId(1),
            payload: Some(ScopedMemoryPayloadV29::AtomicRmw {
                result: ValueId(3),
                value: ValueId(2),
                source: ScopedMemoryStoreSourceV29::Operand {
                    site,
                    role: ExecutionOperandV29::AtomicValue,
                    ty: SemanticTypeIdV1::from_index(1),
                    // The shape checker has no occurrence authority. Full
                    // check_scoped_payload_v29 still requires the real archive.
                    source: ScopedMemoryOperandSourceV29::Constant,
                },
                effect: ScopedAtomicEffectV1::from_operation(&physical()).unwrap(),
            }),
        },
    }
}

#[test]
fn atomic_payload_source_site_frame_and_type_are_bound() {
    let (types, function) = source_fixture(SemanticAtomicRmwOpV1::Add);
    let row = source_row();
    let ScopedMemoryAnchorKindV29::Access {
        payload: Some(ScopedMemoryPayloadV29::AtomicRmw { source, effect, .. }),
        ..
    } = row.kind
    else {
        unreachable!();
    };
    for mutation in 0..7 {
        let mut source = source;
        let mut frame = row.source;
        let mut result = operation().results.remove(0);
        match mutation {
            0 => {}
            1 => frame = None,
            2 => {
                frame.as_mut().unwrap().role = Some(ScopedMemoryRoleV29::Operand(
                    ExecutionOperandV29::StoreDestination,
                ))
            }
            3 => {
                let ScopedMemoryStoreSourceV29::Operand { site, .. } = &mut source else {
                    unreachable!();
                };
                *site = execution_site_v29(SemanticBlockIdV1::from_index(0), Some(1));
            }
            4 => {
                let ScopedMemoryStoreSourceV29::Operand { role, .. } = &mut source else {
                    unreachable!();
                };
                *role = ExecutionOperandV29::StoreValue;
            }
            5 => {
                let ScopedMemoryStoreSourceV29::Operand { ty, .. } = &mut source else {
                    unreachable!();
                };
                *ty = SemanticTypeIdV1::from_index(0);
            }
            6 => result.ty = Type::Scalar(ScalarType::I32),
            _ => unreachable!(),
        }
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 4096);
        let before = budget.storage();
        assert_eq!(
            scoped_atomic_source_v1(
                &types,
                &function,
                frame,
                source,
                effect,
                &result,
                &mut budget
            )
            .is_ok(),
            mutation == 0,
            "mutation {mutation}"
        );
        assert_eq!(budget.storage(), before);
    }
}

#[test]
fn atomic_payload_effect_replay_rechecks_original_statement_and_refunds_only_header() {
    let (_, original) = source_fixture(SemanticAtomicRmwOpV1::Add);
    let (_, wrong) = source_fixture(SemanticAtomicRmwOpV1::Subtract);
    let row = source_row();
    for function in [&original, &wrong] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 4096);
        budget.reserve_storage(37).unwrap();
        assert_eq!(
            check_scoped_payload_effect_v29(function, &row, &operation(), &mut budget).is_ok(),
            std::ptr::eq(function, &original)
        );
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn atomic_payload_effect_replay_does_not_accept_ordinary_memory_or_missing_payload() {
    let (_, function) = source_fixture(SemanticAtomicRmwOpV1::Add);
    for mutation in 0..3 {
        let mut row = source_row();
        let mut op = operation();
        match mutation {
            0 => {
                op.kind = OperationKind::Load {
                    pointer: ValueId(1),
                    access: physical().access,
                }
            }
            1 => {
                op.results.clear();
                op.kind = OperationKind::Store {
                    pointer: ValueId(1),
                    value: ValueId(2),
                    access: physical().access,
                };
            }
            2 => {
                row.kind = ScopedMemoryAnchorKindV29::Access {
                    pointer: ValueId(1),
                    payload: None,
                }
            }
            _ => unreachable!(),
        }
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 4096);
        assert!(check_scoped_payload_effect_v29(&function, &row, &op, &mut budget).is_err());
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn atomic_payload_effect_replay_preserves_work_and_storage_refusals() {
    let (_, function) = source_fixture(SemanticAtomicRmwOpV1::Add);
    for (work_limit, storage_limit) in [(0, 4096), (1000, 0)] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        assert!(
            check_scoped_payload_effect_v29(&function, &source_row(), &operation(), &mut budget)
                .is_err()
        );
        assert_eq!(budget.storage(), 0);
    }
}
