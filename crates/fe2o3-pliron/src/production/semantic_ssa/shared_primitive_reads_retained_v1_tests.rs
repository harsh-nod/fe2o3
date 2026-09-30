use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;

#[derive(Clone, Copy, Debug)]
enum Mode {
    Direct,
    Tuple,
    Load,
    Mutable,
    Invalidated,
    AddressObserved,
    CompilerTemporary,
    CompilerTemporaryFuture,
}
fn ty(id: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(id)
}
fn fid() -> SemanticFunctionIdV1 {
    SemanticFunctionIdV1::from_index(0)
}
fn place(local: u32, kind: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(kind)).unwrap()
}
fn assign(local: u32, kind: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, kind),
            SemanticRvalueV1::new(ty(kind), value),
        )),
    )
}
fn constant(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty(1),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 8).unwrap()),
    ))
}

// Same exact scalar/reference/ScalarPair layouts as the admitted main lowerer
// controls, constructed here independently before actual SSA owner planning.
fn owner(mode: Mode) -> ProductionSemanticSsaOwnerV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let integer = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let pointer_primitive = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
    let range = SemanticScalarValidityRangeV1::new(1, u64::MAX.into());
    let pointer = SemanticBackendScalarV1::initialized(pointer_primitive, range);
    let declaration = |id: u8, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([id; 32]),
            SemanticLayoutIdentityV1::from_sha256([id + 1; 32]),
            layout,
            shape,
        )
    };
    let mut types = vec![
        declaration(
            20,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        declaration(
            22,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(integer),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
        ),
        declaration(
            24,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(pointer),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(1),
                    SemanticPointerKindV1::Reference,
                    if matches!(mode, Mode::Mutable) {
                        SemanticMutabilityV1::Mutable
                    } else {
                        SemanticMutabilityV1::Immutable
                    },
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
    ];
    let mut locals = vec![0, 1, 2, 1];
    if matches!(mode, Mode::AddressObserved) {
        types.push(declaration(
            28,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    pointer_primitive,
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(1),
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
        locals.push(3);
    }
    let mut statements = vec![
        assign(1, 1, SemanticRvalueKindV1::Use(constant(17))),
        assign(
            2,
            2,
            SemanticRvalueKindV1::Borrow {
                kind: if matches!(mode, Mode::Mutable) {
                    SemanticBorrowKindV1::Mutable
                } else {
                    SemanticBorrowKindV1::Shared
                },
                place: place(1, 1),
            },
        ),
    ];
    let compiler_temporary = matches!(
        mode,
        Mode::CompilerTemporary | Mode::CompilerTemporaryFuture
    );
    let (holder, mut projections) = if matches!(mode, Mode::Tuple) || compiler_temporary {
        types.push(declaration(
            26,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                16,
                8,
                SemanticFieldsShapeV1::arbitrary(vec![0, 8], vec![0, 1]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::ScalarPair {
                    first: integer,
                    second: pointer,
                },
                Some(SemanticLayoutNicheV1::new(8, pointer_primitive, range).unwrap()),
                false,
                None,
                8,
                0,
                SemanticTypeLayoutDetailsV1::Aggregate(
                    SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
                ),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![ty(1), ty(2)]).unwrap()),
        ));
        locals.push(3);
        if compiler_temporary {
            locals.extend([2, 2]);
            statements.push(assign(
                5,
                2,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, 2))),
            ));
        }
        statements.push(assign(
            4,
            3,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Tuple,
                vec![
                    constant(23),
                    SemanticOperandV1::Move(place(if compiler_temporary { 5 } else { 2 }, 2)),
                ],
            )
            .unwrap(),
        ));
        if compiler_temporary {
            statements.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(5)),
            ));
            statements.push(assign(
                6,
                2,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(4),
                        vec![
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), ty(2))
                                .unwrap(),
                        ],
                        ty(2),
                    )
                    .unwrap(),
                )),
            ));
            (6, vec![])
        } else {
            (
                4,
                vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), ty(2)).unwrap()],
            )
        }
    } else {
        (2, vec![])
    };
    projections
        .push(SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(1)).unwrap());
    let access =
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(holder), projections, ty(1)).unwrap();
    statements.push(assign(
        3,
        1,
        if matches!(mode, Mode::Load) {
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                access,
                SemanticVolatilityV1::NonVolatile,
                None,
            ))
        } else {
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(access))
        },
    ));
    if matches!(mode, Mode::Invalidated) {
        statements.push(assign(1, 1, SemanticRvalueKindV1::Use(constant(29))));
    }
    if compiler_temporary {
        for local in [4, 2, 1] {
            statements.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(local)),
            ));
        }
        if matches!(mode, Mode::CompilerTemporaryFuture) {
            statements.push(assign(
                3,
                1,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(6),
                        vec![
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(1))
                                .unwrap(),
                        ],
                        ty(1),
                    )
                    .unwrap(),
                )),
            ));
        }
    } else {
        statements.push(SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(holder)),
        ));
    }
    if matches!(mode, Mode::AddressObserved) {
        statements.push(assign(
            4,
            3,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(1, 1),
            },
        ));
    }
    statements.push(assign(
        0,
        0,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(0),
            SemanticConstantValueV1::ZeroSized,
        ))),
    ));
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([30; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(ty(0), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([32; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([33; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([34; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([35; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([36; 32]),
        source,
        abi,
        locals
            .into_iter()
            .enumerate()
            .map(|(id, kind)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([60 + id as u8; 32]),
                    ty(kind),
                    if id == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([40; 32]),
                source,
                statements,
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"shared_value_reads".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([41; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let semantic = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![fid()],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let source = ProductionSemanticMirOwnerV1::try_new(
        semantic,
        crate::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let owner =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    owner.verify_replay().unwrap();
    owner
}

fn access(owner: &ProductionSemanticSsaOwnerV1, mode: Mode) -> (usize, &SemanticPlaceV1) {
    let statement = match mode {
        Mode::Tuple => 3,
        Mode::CompilerTemporary | Mode::CompilerTemporaryFuture => 6,
        _ => 2,
    };
    let SemanticStatementKindV1::Assign(assignment) =
        owner.source_semantic().functions()[0].blocks()[0].statements()[statement].kind()
    else {
        panic!()
    };
    let place = match assignment.value().kind() {
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) => place,
        SemanticRvalueKindV1::Load(load) => load.source(),
        _ => panic!(),
    };
    (statement, place)
}
fn query(
    reads: &ProductionSemanticSharedReadsV1<'_>,
    owner: &ProductionSemanticSsaOwnerV1,
    mode: Mode,
    budget: &mut Budget<'_>,
) -> R<bool> {
    let (statement, place) = access(owner, mode);
    reads.contains(
        owner,
        fid(),
        &owner.source_semantic().functions()[0],
        0,
        Some(statement),
        place,
        budget,
    )
}

const LIMIT: usize = 16 * 1024 * 1024;
const FLOOR: usize = 31;
fn retained_rows(
    view: &ProductionSemanticSharedReadsV1<'_>,
) -> Vec<(u32, u32, u32, u32, u32, usize)> {
    view.rows
        .iter()
        .map(|row| {
            let (b, s) = row.borrow.test_coordinates();
            (b, s, row.source, row.block, row.statement, row.place)
        })
        .collect()
}
fn state(p: &ProductionSemanticSharedReadsPreparationV1<'_>) -> Vec<usize> {
    vec![
        p.phase as usize,
        p.engine.is_some() as usize,
        p.promoted.as_ptr() as usize,
        p.promoted.len(),
        p.promoted.capacity(),
        p.rows.rows.as_ptr() as usize,
        p.rows.rows.len(),
        p.rows.rows.capacity(),
        p.rows.transferred as usize,
        p.view.is_some() as usize,
        p.view.as_ref().map_or(0, |v| v.rows.as_ptr() as usize),
        p.view.as_ref().map_or(0, |v| v.rows.len()),
        p.view.as_ref().map_or(0, |v| v.rows.capacity()),
        p.scratch,
        p.engine_start,
        p.refunded,
    ]
}
fn measured(owner: &ProductionSemanticSsaOwnerV1) -> (usize, usize) {
    let mut pending = ProductionSemanticSharedReadsPreparationV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    pending
        .prepare_into(owner, fid(), &mut budget, &mut owned)
        .unwrap();
    pending
        .completed_for(owner, fid(), &budget, &owned)
        .unwrap();
    let result = (budget.work(), budget.peak_storage());
    assert_eq!(budget.storage(), FLOOR + owned);
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    result
}
#[test]
fn retained_shared_actual_owner_matches_original_complete_rows_queries_and_work() {
    for mode in [
        Mode::Direct,
        Mode::Tuple,
        Mode::Load,
        Mode::Mutable,
        Mode::Invalidated,
        Mode::AddressObserved,
        Mode::CompilerTemporary,
        Mode::CompilerTemporaryFuture,
    ] {
        let owner = owner(mode);
        let mut old_work = Work::new(LIMIT);
        let mut old_budget = Budget::new(&mut old_work, LIMIT);
        old_budget.reserve_storage(FLOOR).unwrap();
        let original =
            ProductionSemanticSharedReadsV1::try_new(&owner, fid(), &mut old_budget).unwrap();
        let rows = retained_rows(&original);
        let original_work = old_budget.work();
        let expected = query(&original, &owner, mode, &mut old_budget).unwrap();
        let mut pending = ProductionSemanticSharedReadsPreparationV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        pending
            .prepare_into(&owner, fid(), &mut budget, &mut owned)
            .unwrap();
        let prefix = if pending.dimensions.unwrap().is_some() {
            64
        } else {
            0
        };
        assert_eq!(budget.work(), original_work + prefix, "{mode:?}");
        let view = pending
            .completed_for(&owner, fid(), &budget, &owned)
            .unwrap();
        assert_eq!(retained_rows(view), rows, "{mode:?}");
        assert_eq!(
            query(view, &owner, mode, &mut budget).unwrap(),
            expected,
            "{mode:?}"
        );
        assert_eq!(
            owned,
            headers().unwrap() + frame().unwrap() + view.rows.capacity() * size_of::<Read>()
        );
        assert!(pending.engine.is_none());
        assert_eq!(pending.promoted.capacity(), 0);
        assert_eq!(budget.storage(), FLOOR + owned);
        drop(pending);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        original.release(&mut old_budget).unwrap();
        assert_eq!(old_budget.storage(), FLOOR);
        owner.verify_replay().unwrap();
    }
}
#[test]
fn retained_shared_no_candidate_is_real_empty_view_not_outer_none() {
    let owner = owner(Mode::Mutable);
    let mut pending = ProductionSemanticSharedReadsPreparationV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    pending
        .prepare_into(&owner, fid(), &mut budget, &mut owned)
        .unwrap();
    assert_eq!(pending.dimensions, Some(None));
    assert!(pending.view().is_some());
    assert!(pending.engine.is_none());
    assert!(!pending.rows.transferred);
    assert_eq!(pending.refunded, 0);
    let view = pending
        .completed_for(&owner, fid(), &budget, &owned)
        .unwrap();
    assert_eq!(view.rows.len(), 0);
    assert_eq!(view.rows.capacity(), 0);
    assert!(!query(view, &owner, Mode::Mutable, &mut budget).unwrap());
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
#[test]
fn retained_shared_filtered_empty_keeps_actual_row_capacity_and_real_view() {
    for mode in [
        Mode::Invalidated,
        Mode::AddressObserved,
        Mode::CompilerTemporaryFuture,
    ] {
        let owner = owner(mode);
        let mut pending = ProductionSemanticSharedReadsPreparationV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        pending
            .prepare_into(&owner, fid(), &mut budget, &mut owned)
            .unwrap();
        assert!(pending.dimensions.unwrap().is_some());
        assert!(pending.rows.transferred);
        assert!(pending.refunded >= pending.scratch);
        assert!(pending.engine.is_none());
        let view = pending
            .completed_for(&owner, fid(), &budget, &owned)
            .unwrap();
        assert_eq!(view.rows.len(), 0);
        assert!(view.rows.capacity() > 0);
        assert!(!query(view, &owner, mode, &mut budget).unwrap());
        drop(pending);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn retained_shared_every_work_cut_retains_partial_owners_first_error_and_terminal_state() {
    let owner = owner(Mode::Tuple);
    let needed = measured(&owner).0;
    let mut saw_engine = false;
    let mut saw_promoted = false;
    for limit in 0..needed {
        let mut pending = ProductionSemanticSharedReadsPreparationV1::new();
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let Err(Failure::Resource(first @ Resource::Work(_))) =
            pending.prepare_into(&owner, fid(), &mut budget, &mut owned)
        else {
            panic!("{limit}")
        };
        assert_eq!(pending.failure, Some(first));
        assert!(pending.view().is_none());
        saw_engine |= pending.engine.is_some();
        saw_promoted |= pending.promoted.capacity() != 0;
        assert_eq!(budget.storage(), FLOOR + owned);
        let before = (state(&pending), owned, budget.storage(), budget.work());
        assert!(
            matches!(pending.prepare_into(&owner,fid(),&mut budget,&mut owned),
            Err(Failure::Resource(error)) if error==first)
        );
        assert!(matches!(pending.completed_for(&owner,fid(),&budget,&owned),
            Err(Failure::Resource(error)) if error==first));
        assert_eq!(
            before,
            (state(&pending), owned, budget.storage(), budget.work())
        );
        drop(pending);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
    assert!(saw_engine);
    assert!(saw_promoted);
}
#[test]
fn retained_shared_every_storage_cut_preserves_exact_coupled_credits_until_drop() {
    let owner = owner(Mode::Tuple);
    let needed = measured(&owner).1;
    let mut saw_engine = false;
    for limit in FLOOR..needed {
        let mut pending = ProductionSemanticSharedReadsPreparationV1::new();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        let Err(Failure::Resource(first @ Resource::Storage(_))) =
            pending.prepare_into(&owner, fid(), &mut budget, &mut owned)
        else {
            panic!("{limit}")
        };
        assert_eq!(pending.failure, Some(first));
        saw_engine |= pending.engine.is_some();
        let before = (state(&pending), owned, budget.storage(), budget.work());
        assert_eq!(budget.storage(), FLOOR + owned);
        assert!(
            matches!(pending.prepare_into(&owner,fid(),&mut budget,&mut owned),
            Err(Failure::Resource(error)) if error==first)
        );
        assert_eq!(
            before,
            (state(&pending), owned, budget.storage(), budget.work())
        );
        drop(pending);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
    assert!(saw_engine);
}
#[test]
fn retained_shared_exact_and_one_short_limits_include_success_scratch_refund() {
    let owner = owner(Mode::Tuple);
    let (needed_work, needed_storage) = measured(&owner);
    for (w, s, expected) in [
        (needed_work, needed_storage, 0),
        (needed_work - 1, needed_storage, 1),
        (needed_work, needed_storage - 1, 2),
    ] {
        let mut pending = ProductionSemanticSharedReadsPreparationV1::new();
        let mut work = Work::new(w);
        let mut budget = Budget::new(&mut work, s);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owned = 0;
        match pending.prepare_into(&owner, fid(), &mut budget, &mut owned) {
            Ok(()) => {
                assert_eq!(expected, 0);
                assert!(pending.refunded > 0);
                assert!(budget.storage() < budget.peak_storage());
            }
            Err(Failure::Resource(Resource::Work(_))) => assert_eq!(expected, 1),
            Err(Failure::Resource(Resource::Storage(_))) => assert_eq!(expected, 2),
            other => panic!("{other:?}"),
        }
        assert_eq!(budget.storage(), FLOOR + owned);
        drop(pending);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}
#[test]
fn retained_shared_real_owner_plan_binding_precedes_discovery_and_refuses_reentry() {
    let owner = owner(Mode::Direct);
    let mut pending = ProductionSemanticSharedReadsPreparationV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    assert!(matches!(
        pending.prepare_into(
            &owner,
            SemanticFunctionIdV1::from_index(99),
            &mut budget,
            &mut owned
        ),
        Err(Failure::Binding)
    ));
    assert_eq!(budget.work(), 8);
    assert!(pending.semantic.is_some());
    assert!(pending.declaration.is_none());
    assert!(pending.dimensions.is_none());
    assert!(pending.engine.is_none());
    let before = (state(&pending), owned, budget.work(), budget.storage());
    assert!(
        pending
            .prepare_into(&owner, fid(), &mut budget, &mut owned)
            .is_err()
    );
    assert_eq!(
        before,
        (state(&pending), owned, budget.work(), budget.storage())
    );
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
#[test]
fn retained_shared_completion_binds_original_budget_counter_owner_and_held_floor() {
    let other = owner(Mode::Direct);
    let owner = owner(Mode::Direct);
    let mut pending = ProductionSemanticSharedReadsPreparationV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    pending
        .prepare_into(&owner, fid(), &mut budget, &mut owned)
        .unwrap();
    let before = budget.work();
    let copied = owned;
    assert!(
        pending
            .completed_for(&owner, fid(), &budget, &copied)
            .is_err()
    );
    assert!(
        pending
            .completed_for(&other, fid(), &budget, &owned)
            .is_err()
    );
    let mut foreign_work = Work::new(LIMIT);
    let mut foreign = Budget::new(&mut foreign_work, LIMIT);
    foreign.reserve_storage(budget.storage()).unwrap();
    assert!(
        pending
            .completed_for(&owner, fid(), &foreign, &owned)
            .is_err()
    );
    assert_eq!(budget.work(), before);
    assert_eq!(foreign.work(), 0);
    budget.release_storage(1).unwrap();
    owned -= 1;
    assert!(
        pending
            .completed_for(&owner, fid(), &budget, &owned)
            .is_err()
    );
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
#[test]
fn retained_shared_queries_keep_original_pointer_binding_and_swallowed_first_denial() {
    let owner = owner(Mode::Tuple);
    let mut pending = ProductionSemanticSharedReadsPreparationV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    pending
        .prepare_into(&owner, fid(), &mut budget, &mut owned)
        .unwrap();
    let view = pending.view().unwrap();
    let (statement, place) = access(&owner, Mode::Tuple);
    let copied = place.clone();
    assert!(
        !view
            .contains(
                &owner,
                fid(),
                &owner.source_semantic().functions()[0],
                0,
                Some(statement),
                &copied,
                &mut budget
            )
            .unwrap()
    );
    assert!(query(view, &owner, Mode::Tuple, &mut budget).unwrap());
    budget.charge_work(LIMIT - budget.work() - 13).unwrap();
    let Err(Failure::Resource(first @ Resource::Work(_))) =
        query(view, &owner, Mode::Tuple, &mut budget)
    else {
        panic!()
    };
    let before = budget.work();
    assert!(
        matches!(pending.completed_for(&owner,fid(),&budget,&owned),Err(Failure::Resource(e)) if e==first)
    );
    assert_eq!(budget.work(), before);
    assert_eq!(budget.storage(), owned);
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
#[test]
fn retained_shared_caller_unwind_keeps_completed_rows_until_outer_drop_and_refund() {
    let owner = owner(Mode::Tuple);
    let mut pending = ProductionSemanticSharedReadsPreparationV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pending
            .prepare_into(&owner, fid(), &mut budget, &mut owned)
            .unwrap();
        panic!("controlled downstream unwind");
    }));
    assert!(caught.is_err());
    assert!(!pending.view().unwrap().rows.is_empty());
    pending
        .completed_for(&owner, fid(), &budget, &owned)
        .unwrap();
    assert_eq!(budget.storage(), FLOOR + owned);
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
#[test]
fn retained_shared_source_order_keeps_real_discovery_promotion_handoff_and_drop_before_refund() {
    fn compact(s: &str) -> String {
        s.chars().filter(|c| !c.is_whitespace()).collect()
    }
    let source = compact(include_str!("shared_primitive_reads_retained_v1.rs"));
    let order = [
        "budget.charge_work(8)?;",
        "letsemantic=owner.source_semantic();",
        "letplan=owner.plan_for_function(function)",
        "has_candidates(",
        "scan_size(",
        "self.engine=Some(",
        ".prepare_observer_into(",
        ".analyze_into(",
        "plan.promoted_variables().len()",
        "self.promoted.try_reserve_exact(",
        "forvariableinplan.promoted_variables()",
        ".finish_rows_into(",
        "drop(std::mem::take(&mutself.promoted));",
        "drop(self.engine.take());",
        "budget.release_storage(refund)?;",
        "*owned=next_owned;",
        "self.view=Some(",
    ];
    let mut offset = 0;
    for marker in order {
        let marker = compact(marker);
        let next = source[offset..].find(&marker).expect(&marker);
        offset += next + marker.len();
    }
    assert!(!source.contains("analyze_observed("));
    assert!(!source.contains("vec![false;"));
}

#[test]
fn retained_shared_preserves_preexisting_coupled_credits_and_accepts_later_owner_growth() {
    let owner = owner(Mode::Tuple);
    let mut pending = ProductionSemanticSharedReadsPreparationV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let prefix = 47usize;
    budget.reserve_storage(FLOOR + prefix).unwrap();
    let mut owned = prefix;
    pending
        .prepare_into(&owner, fid(), &mut budget, &mut owned)
        .unwrap();
    let view = pending
        .completed_for(&owner, fid(), &budget, &owned)
        .unwrap();
    assert_eq!(view.floor, FLOOR + prefix);
    assert_eq!(view.owned, owned - prefix);
    assert_eq!(budget.storage(), FLOOR + owned);
    budget.reserve_storage(19).unwrap();
    owned += 19;
    pending
        .completed_for(&owner, fid(), &budget, &owned)
        .unwrap();
    assert!(query(pending.view().unwrap(), &owner, Mode::Tuple, &mut budget).unwrap());
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn retained_shared_completed_scope_preserves_untracked_consumer_storage() {
    let owner = owner(Mode::Tuple);
    let mut pending = ProductionSemanticSharedReadsPreparationV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    pending
        .prepare_into(&owner, fid(), &mut budget, &mut owned)
        .unwrap();
    let held = pending.held;
    let receipt = owned;
    let before = state(&pending);
    budget.reserve_storage(29).unwrap();
    // The existing outer scope permits the consumer's separate receipt.
    let view = pending
        .completed_for(&owner, fid(), &budget, &owned)
        .unwrap();
    assert!(query(view, &owner, Mode::Tuple, &mut budget).unwrap());
    assert_eq!(owned, receipt);
    assert!(pending.held == held);
    assert_eq!(before, state(&pending));
    // Construction's exact coupling did not change into a lower-bound check.
    assert!(matches!(
        pending.check_pair(&owner, fid(), &budget, &owned, false),
        Err(Failure::Resource(Resource::Accounting))
    ));
    pending
        .completed_for(&owner, fid(), &budget, &owned)
        .unwrap();
    drop(pending);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR + 29);
    budget.release_storage(29).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn retained_shared_external_surplus_does_not_cover_counter_identity_or_held_floor_loss() {
    let other = owner(Mode::Tuple);
    let owner = owner(Mode::Tuple);
    let mut pending = ProductionSemanticSharedReadsPreparationV1::new();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    pending
        .prepare_into(&owner, fid(), &mut budget, &mut owned)
        .unwrap();
    budget.reserve_storage(29).unwrap();
    let receipt = owned;
    let before = budget.work();
    let copied = owned;
    assert!(
        pending
            .completed_for(&owner, fid(), &budget, &copied)
            .is_err()
    );
    assert!(
        pending
            .completed_for(&other, fid(), &budget, &owned)
            .is_err()
    );
    let mut other_work = Work::new(LIMIT);
    let mut foreign = Budget::new(&mut other_work, LIMIT);
    foreign.reserve_storage(budget.storage()).unwrap();
    assert!(
        pending
            .completed_for(&owner, fid(), &foreign, &owned)
            .is_err()
    );
    owned -= 1;
    // There are still29 surplus bytes, but this owner's held receipt fell.
    assert!(budget.storage() > FLOOR + receipt);
    assert!(
        pending
            .completed_for(&owner, fid(), &budget, &owned)
            .is_err()
    );
    owned = receipt;
    budget.release_storage(30).unwrap();
    assert!(
        pending
            .completed_for(&owner, fid(), &budget, &owned)
            .is_err()
    );
    assert_eq!(budget.work(), before);
    assert_eq!(foreign.work(), 0);
    drop(pending);
    // The deliberate negative storage mutation removed one tracked credit.
    budget.release_storage(receipt - 1).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
