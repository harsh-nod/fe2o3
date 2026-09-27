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

#[test]
fn shared_value_reads_exact_original_direct_and_static_holder() {
    for mode in [Mode::Direct, Mode::Tuple, Mode::CompilerTemporary] {
        let owner = owner(mode);
        assert!(
            owner
                .plan_for_function(fid())
                .unwrap()
                .plan()
                .promoted_variables()
                .contains(&SsaVariableIdV1::new(1))
        );
        let mut work = Work::new(10_000_000);
        let mut budget = Budget::new(&mut work, 10_000_000);
        budget.reserve_storage(17).unwrap();
        let reads =
            crate::ProductionSemanticSharedReadsV1::try_new(&owner, fid(), &mut budget).unwrap();
        assert_eq!(reads.rows.len(), 1);
        let fixed = budget.storage();
        for _ in 0..3 {
            assert!(query(&reads, &owner, mode, &mut budget).unwrap());
            assert_eq!(budget.storage(), fixed);
        }
        reads.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 17);
        owner.verify_replay().unwrap();
    }
}

#[test]
fn shared_value_reads_unscoped_tuple_temporary_preserves_future_use_refusal() {
    let owner = owner(Mode::CompilerTemporaryFuture);
    assert!(
        !owner
            .plan_for_function(fid())
            .unwrap()
            .plan()
            .promoted_variables()
            .contains(&SsaVariableIdV1::new(1))
    );
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    budget.reserve_storage(17).unwrap();
    let reads =
        crate::ProductionSemanticSharedReadsV1::try_new(&owner, fid(), &mut budget).unwrap();
    assert!(!query(&reads, &owner, Mode::CompilerTemporaryFuture, &mut budget).unwrap());
    reads.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 17);
}

#[test]
fn shared_value_reads_foreign_owner_and_equal_locator_are_not_authority() {
    let other = owner(Mode::Tuple);
    let owner = owner(Mode::Tuple);
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let reads = ProductionSemanticSharedReadsV1::try_new(&owner, fid(), &mut budget).unwrap();
    let before = budget.work();
    assert!(matches!(
        query(&reads, &other, Mode::Tuple, &mut budget),
        Err(Failure::Binding)
    ));
    assert_eq!(budget.work(), before);
    let (statement, place) = access(&owner, Mode::Tuple);
    let copied = place.clone();
    let declaration = &owner.source_semantic().functions()[0];
    let before = budget.work();
    assert!(matches!(
        reads.contains(
            &owner,
            SemanticFunctionIdV1::from_index(1),
            declaration,
            0,
            Some(statement),
            place,
            &mut budget
        ),
        Err(Failure::Binding)
    ));
    assert_eq!(budget.work(), before);
    let sibling = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(4),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty(1)).unwrap()],
        ty(1),
    )
    .unwrap();
    assert!(
        !reads
            .contains(
                &owner,
                fid(),
                declaration,
                0,
                Some(statement),
                &sibling,
                &mut budget
            )
            .unwrap()
    );
    assert!(
        !reads
            .contains(
                &owner,
                fid(),
                declaration,
                0,
                Some(statement),
                &copied,
                &mut budget
            )
            .unwrap()
    );
    assert!(
        !reads
            .contains(
                &owner,
                fid(),
                declaration,
                0,
                Some(statement + 1),
                place,
                &mut budget
            )
            .unwrap()
    );
    assert!(
        !reads
            .contains(&owner, fid(), declaration, 0, None, place, &mut budget)
            .unwrap()
    );
    assert!(query(&reads, &owner, Mode::Tuple, &mut budget).unwrap());
    reads.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn shared_value_reads_later_invalidation_and_explicit_memory_are_excluded() {
    for mode in [
        Mode::Invalidated,
        Mode::Load,
        Mode::Mutable,
        Mode::AddressObserved,
    ] {
        let owner = owner(mode);
        assert!(
            !owner
                .plan_for_function(fid())
                .unwrap()
                .plan()
                .promoted_variables()
                .contains(&SsaVariableIdV1::new(1))
        );
        let mut work = Work::new(10_000_000);
        let mut budget = Budget::new(&mut work, 10_000_000);
        let reads = ProductionSemanticSharedReadsV1::try_new(&owner, fid(), &mut budget).unwrap();
        assert_eq!(reads.rows.len(), 0, "{mode:?}");
        assert!(!query(&reads, &owner, mode, &mut budget).unwrap());
        reads.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn shared_value_reads_constructor_exact_and_one_short_resource_cuts() {
    let owner = owner(Mode::Tuple);
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    budget.reserve_storage(11).unwrap();
    let reads = ProductionSemanticSharedReadsV1::try_new(&owner, fid(), &mut budget).unwrap();
    let exact_work = budget.work();
    let exact_storage = budget.peak_storage();
    assert_eq!(
        reads.owned,
        headers().unwrap() + reads.rows.capacity() * size_of::<Read>()
    );
    reads.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 11);
    for (work_limit, storage_limit, kind) in [
        (exact_work, exact_storage, 0),
        (exact_work - 1, exact_storage, 1),
        (exact_work, exact_storage - 1, 2),
        (exact_work, 11 + headers().unwrap() - 1, 2),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(11).unwrap();
        match ProductionSemanticSharedReadsV1::try_new(&owner, fid(), &mut budget) {
            Ok(reads) => {
                assert_eq!(kind, 0);
                reads.release(&mut budget).unwrap();
            }
            Err(Failure::Resource(Resource::Work(_))) => assert_eq!(kind, 1),
            Err(Failure::Resource(Resource::Storage(_))) => assert_eq!(kind, 2),
            Err(error) => panic!("{error:?}"),
        }
        assert_eq!(budget.storage(), 11);
    }
}

#[test]
fn shared_value_reads_swallowed_query_denial_replays_first_error_and_refunds() {
    let owner = owner(Mode::Direct);
    let limit = 10_000_000;
    let mut work = Work::new(limit);
    let mut budget = Budget::new(&mut work, 10_000_000);
    budget.reserve_storage(13).unwrap();
    let reads = ProductionSemanticSharedReadsV1::try_new(&owner, fid(), &mut budget).unwrap();
    assert!(query(&reads, &owner, Mode::Direct, &mut budget).unwrap());
    // One read row: fixed12 + independently two ordered-search height units.
    budget.charge_work(limit - budget.work() - 13).unwrap();
    let before = budget.work();
    let Err(Failure::Resource(first @ Resource::Work(_))) =
        query(&reads, &owner, Mode::Direct, &mut budget)
    else {
        panic!()
    };
    assert_eq!(budget.work(), before);
    assert!(
        matches!(query(&reads,&owner,Mode::Direct,&mut budget),Err(Failure::Resource(e)) if e==first)
    );
    assert_eq!(budget.work(), before);
    assert!(matches!(reads.release(&mut budget),Err(Failure::Resource(e)) if e==first));
    assert_eq!(budget.storage(), 13);
}

#[test]
fn shared_value_reads_foreign_ledger_no_debit_then_exact_accounting_replay() {
    let owner = owner(Mode::Direct);
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let mut foreign_work = Work::new(10_000_000);
    let mut foreign = Budget::new(&mut foreign_work, 10_000_000);
    budget.reserve_storage(19).unwrap();
    let reads = ProductionSemanticSharedReadsV1::try_new(&owner, fid(), &mut budget).unwrap();
    assert!(query(&reads, &owner, Mode::Direct, &mut budget).unwrap());
    foreign.reserve_storage(budget.storage()).unwrap();
    assert!(matches!(
        query(&reads, &owner, Mode::Direct, &mut foreign),
        Err(Failure::Resource(Resource::Accounting))
    ));
    assert_eq!(foreign.work(), 0);
    let accepted = budget.work();
    assert!(matches!(
        query(&reads, &owner, Mode::Direct, &mut budget),
        Err(Failure::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.work(), accepted);
    assert!(matches!(
        reads.release(&mut budget),
        Err(Failure::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), 19);
}

#[test]
fn shared_value_reads_caller_floor_loss_does_not_refund_unowned_storage() {
    let owner = owner(Mode::Direct);
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    budget.reserve_storage(23).unwrap();
    let reads = ProductionSemanticSharedReadsV1::try_new(&owner, fid(), &mut budget).unwrap();
    budget.release_storage(1).unwrap();
    let floor = budget.storage();
    let work = budget.work();
    assert!(matches!(
        query(&reads, &owner, Mode::Direct, &mut budget),
        Err(Failure::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.work(), work);
    assert!(matches!(
        reads.release(&mut budget),
        Err(Failure::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), floor);
}
