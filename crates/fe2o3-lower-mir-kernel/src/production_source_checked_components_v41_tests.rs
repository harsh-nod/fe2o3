use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateLayoutV1, SemanticAggregateTypeV1, SemanticCheckedBinaryRvalueV1,
    SemanticPaddingV1,
};

#[path = "production_source_retained_checked_objects_v44_tests.rs"]
mod retained_checked_objects_v44_tests;

fn checked_loop(operation: SemanticCheckedBinaryOpV1) -> ProductionSemanticSsaOwnerV1 {
    let base = comparison_loop();
    let source = base.source_semantic();
    let old = &source.functions()[0];
    let boolean = old.locals()[3].ty();
    let mut types = source.types().to_vec();
    let pair = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([244; 32]),
        SemanticLayoutIdentityV1::from_sha256([244; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![SemanticPaddingV1::new(5, 3).unwrap()])
                .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, boolean]).unwrap()),
    ));
    let mut locals = old.locals().to_vec();
    let tuple_local = locals.len() as u32;
    assert_eq!(tuple_local, 4);
    locals.push(local(211, pair, SemanticLocalRoleV1::Temporary));
    let field = |ordinal, ty| {
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(tuple_local),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(ordinal), ty)
                        .unwrap(),
                ],
                ty,
            )
            .unwrap(),
        )
    };
    let left = SemanticOperandV1::Copy(place(2, U32));
    let right = SemanticOperandV1::Copy(place(1, U32));
    let mut blocks = old.blocks().to_vec();
    let continuation = blocks.len() as u32;
    blocks[2] = block(
        192,
        vec![assign(
            place(tuple_local, pair),
            SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                operation,
                left.clone(),
                right.clone(),
            )),
        )],
        SemanticTerminatorKindV1::Assert {
            condition: field(1, boolean),
            expected: false,
            message: SemanticAssertMessageV1::Overflow {
                operation: match operation {
                    SemanticCheckedBinaryOpV1::Add => SemanticBinaryOpV1::Add,
                    SemanticCheckedBinaryOpV1::Subtract => SemanticBinaryOpV1::Subtract,
                    SemanticCheckedBinaryOpV1::Multiply => SemanticBinaryOpV1::Multiply,
                },
                left,
                right,
            },
            target: SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::AssertSuccess,
                SemanticBlockIdV1::from_index(continuation),
            ),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    );
    blocks.push(block(
        195,
        vec![assign(
            place(2, U32),
            SemanticRvalueKindV1::Use(field(0, U32)),
        )],
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(1),
        )),
    ));
    let mut functions = source.functions().to_vec();
    functions[0] = rebuild_root(old, locals, blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn add_loop() -> ProductionSemanticSsaOwnerV1 {
    checked_loop(SemanticCheckedBinaryOpV1::Add)
}
fn sub_loop() -> ProductionSemanticSsaOwnerV1 {
    checked_loop(SemanticCheckedBinaryOpV1::Subtract)
}
fn mul_loop() -> ProductionSemanticSsaOwnerV1 {
    checked_loop(SemanticCheckedBinaryOpV1::Multiply)
}

fn transported_loop(moved: bool) -> ProductionSemanticSsaOwnerV1 {
    let base = add_loop();
    let source = base.source_semantic();
    let old = &source.functions()[0];
    let pair = old.locals()[4].ty();
    let boolean = old.locals()[3].ty();
    let mut locals = old.locals().to_vec();
    assert_eq!(locals.len(), 5);
    locals.push(local(212, pair, SemanticLocalRoleV1::Temporary));
    let mut blocks = old.blocks().to_vec();
    let mut statements = blocks[2].statements().to_vec();
    statements.push(assign(
        place(5, pair),
        SemanticRvalueKindV1::Use(if moved {
            SemanticOperandV1::Move(place(4, pair))
        } else {
            SemanticOperandV1::Copy(place(4, pair))
        }),
    ));
    let projected = |field, ty| {
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(5),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty).unwrap(),
                ],
                ty,
            )
            .unwrap(),
        )
    };
    let mut assertion = blocks[2].terminator().kind().clone();
    let SemanticTerminatorKindV1::Assert { condition, .. } = &mut assertion else {
        unreachable!()
    };
    *condition = projected(1, boolean);
    blocks[2] = block(192, statements, assertion);
    let last = blocks.len() - 1;
    blocks[last] = block(
        195,
        vec![assign(
            place(2, U32),
            SemanticRvalueKindV1::Use(projected(0, U32)),
        )],
        blocks[last].terminator().kind().clone(),
    );
    let mut functions = source.functions().to_vec();
    functions[0] = rebuild_root(old, locals, blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn copied_loop() -> ProductionSemanticSsaOwnerV1 {
    transported_loop(false)
}
fn moved_loop() -> ProductionSemanticSsaOwnerV1 {
    transported_loop(true)
}

fn projected_failure_loop(moved: bool, retained: bool) -> ProductionSemanticSsaOwnerV1 {
    projected_failure_loop_with_constructor(moved, retained, false)
}

fn projected_failure_loop_with_constructor(
    moved: bool,
    retained: bool,
    constructor: bool,
) -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let base = add_loop();
    let source = base.source_semantic();
    let old = &source.functions()[0];
    let pair = old.locals()[4].ty();
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(4),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U32).unwrap()],
        U32,
    )
    .unwrap();
    let mut assertion = old.blocks()[2].terminator().kind().clone();
    let SemanticTerminatorKindV1::Assert { message, .. } = &mut assertion else {
        unreachable!()
    };
    *message = SemanticAssertMessageV1::DivisionByZero(if moved {
        SemanticOperandV1::Move(projected)
    } else {
        SemanticOperandV1::Copy(projected)
    });
    let mut types = source.types().to_vec();
    let mut locals = old.locals().to_vec();
    let mut statements = old.blocks()[2].statements().to_vec();
    if constructor {
        assert!(retained);
        // The exact scalar-field constructor is supported independently of a
        // retained CheckedBinary result. Keep that earlier boundary distinct.
        statements[0] = assign(
            place(4, pair),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![
                        SemanticOperandV1::Copy(place(2, U32)),
                        SemanticOperandV1::Copy(place(3, old.locals()[3].ty())),
                    ],
                )
                .unwrap(),
            ),
        );
    }
    if retained {
        let pointer = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([245; 32]),
            SemanticLayoutIdentityV1::from_sha256([245; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    pair,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
        locals.push(local(212, pointer, SemanticLocalRoleV1::Temporary));
        statements.push(assign(
            place(5, pointer),
            SemanticRvalueKindV1::AddressOf {
                place: place(4, pair),
                mutability: SemanticMutabilityV1::Immutable,
            },
        ));
    }
    let mut blocks = old.blocks().to_vec();
    blocks[2] = block(192, statements, assertion);
    let mut functions = source.functions().to_vec();
    functions[0] = rebuild_root(old, locals, blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn projected_copy_failure_loop() -> ProductionSemanticSsaOwnerV1 {
    projected_failure_loop(false, false)
}

fn projected_move_failure_loop() -> ProductionSemanticSsaOwnerV1 {
    projected_failure_loop(true, false)
}

fn retained_projected_failure_loop() -> ProductionSemanticSsaOwnerV1 {
    projected_failure_loop(true, true)
}

fn retained_constructed_failure_loop() -> ProductionSemanticSsaOwnerV1 {
    projected_failure_loop_with_constructor(true, true, true)
}

#[test]
fn promoted_failure_components_preserve_copy_and_failure_only_move_in_production() {
    for factory in [
        projected_copy_failure_loop as fn() -> _,
        projected_move_failure_loop,
    ] {
        let reached = std::cell::Cell::new(false);
        with_policy11(factory, |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    let semantic = original.source.source_semantic(budget)?;
                    let function = &semantic.functions()[0];
                    let SemanticTerminatorKindV1::Assert {
                        message: SemanticAssertMessageV1::DivisionByZero(operand),
                        ..
                    } = function.blocks()[2].terminator().kind()
                    else {
                        panic!("diagnostic");
                    };
                    let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand
                    else {
                        panic!("original projected operand");
                    };
                    assert_eq!(place.local().index(), 4);
                    assert_eq!(
                        place.projections()[0].kind(),
                        SemanticProjectionKindV1::Field(0)
                    );
                    // The success continuation still reads the same component;
                    // the diagnostic Move cannot kill it on that path.
                    assert!(
                        matches!(function.blocks().last().unwrap().statements()[0].kind(),
                        SemanticStatementKindV1::Assign(assignment)
                        if matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(
                            SemanticOperandV1::Copy(success)) if success == place))
                    );
                    leaves
                        .original
                        .leaves
                        .check_boundary_equations_v31(budget)?;
                    leaves
                        .original
                        .leaves
                        .check_boundary_actual_v31(Some(leaves), budget)?;
                    reached.set(true);
                    Ok(())
                },
            )
        })
        .unwrap();
        assert!(reached.get());
    }
}

#[test]
fn promoted_failure_components_do_not_bypass_real_retained_aggregate_history() {
    for (factory, constructor, expected) in [
        (
            retained_projected_failure_loop as fn() -> ProductionSemanticSsaOwnerV1,
            false,
            "failure history requires an exact whole scalar diagnostic",
        ),
        (
            retained_constructed_failure_loop as fn() -> ProductionSemanticSsaOwnerV1,
            true,
            "failure history requires an exact whole scalar diagnostic",
        ),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let reached = std::cell::Cell::new(false);
        with_pending_api_owner_v18(
            ModuleFixture::Ordinary,
            false,
            &mut budget,
            factory,
            |owner, launch, input, _, budget| {
                let original = &owner.source_semantic().functions()[0];
                let SemanticStatementKindV1::Assign(assignment) =
                    original.blocks()[2].statements()[0].kind()
                else {
                    panic!("original tuple assignment")
                };
                assert_eq!(
                    matches!(
                        assignment.value().kind(),
                        SemanticRvalueKindV1::Aggregate(_)
                    ),
                    constructor
                );
                assert_eq!(
                    matches!(
                        assignment.value().kind(),
                        SemanticRvalueKindV1::CheckedBinary(_)
                    ),
                    !constructor
                );
                let SemanticTerminatorKindV1::Assert {
                    message:
                        SemanticAssertMessageV1::DivisionByZero(SemanticOperandV1::Move(diagnostic)),
                    ..
                } = original.blocks()[2].terminator().kind()
                else {
                    panic!("failure-only projected Move")
                };
                assert_eq!(diagnostic.local().index(), 4);
                assert_eq!(
                    diagnostic.projections()[0].kind(),
                    SemanticProjectionKindV1::Field(0)
                );
                assert!(
                    matches!(original.blocks().last().unwrap().statements()[0].kind(),
                SemanticStatementKindV1::Assign(success)
                if matches!(success.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) if place == diagnostic))
                );
                assert!(owner.source_semantic().functions()[0].blocks()[2].statements().iter()
                .any(|row| matches!(row.kind(), SemanticStatementKindV1::Assign(assignment)
                    if matches!(assignment.value().kind(), SemanticRvalueKindV1::AddressOf { place, .. }
                        if place.local().index() == 4 && place.projections().is_empty()))));
                assert!(
                    !owner.plans()[0]
                        .plan()
                        .promoted_variables()
                        .iter()
                        .any(|local| local.get() == 4)
                );
                let fixture = OriginalKernelAbiFixtureV18::ordinary(&owner);
                let roots = fixture.roots();
                let prepared =
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
                    owner,
                    launch,
                    input,
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
                .unwrap();
                let consumed = std::cell::Cell::new(false);
                let error = prepared
                    .with_source_consumer_v18(budget, |_, _| -> SourceOwnedResultV18<()> {
                        consumed.set(true);
                        Ok(())
                    })
                    .err()
                    .expect("memory-backed diagnostics need every exact preceding source contract");
                assert!(!consumed.get());
                assert!(
                    matches!(&error, ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported {
                        function: 0, block: None, statement: None, detail,
                    })) if *detail == expected),
                    "{error:?}"
                );
                reached.set(true);
            },
        );
        assert!(reached.get());
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn promoted_failure_scalar_paths_reject_bad_fields_types_bounds_and_pointer_crossings() {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let owner = add_loop();
    let source = owner.source_semantic();
    let old = &source.functions()[0];
    let boolean = old.locals()[3].ty();
    let mut types = source.types().to_vec();
    let array = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([246; 32]),
        SemanticLayoutIdentityV1::from_sha256([246; 32]),
        SemanticTypeLayoutV1::new(Some(16), 4).unwrap(),
        SemanticTypeShapeV1::Array {
            element: old.locals()[4].ty(),
            length: 2,
        },
    ));
    let mut locals = old.locals().to_vec();
    locals.push(local(212, array, SemanticLocalRoleV1::Temporary));
    let function = rebuild_root(old, locals, old.blocks().to_vec());
    let make = |path: Vec<(SemanticProjectionKindV1, SemanticTypeIdV1)>| {
        let ty = path.last().map_or(array, |(_, ty)| *ty);
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(5),
            path.into_iter()
                .map(|(kind, ty)| SemanticProjectionV1::new(kind, ty).unwrap())
                .collect(),
            ty,
        )
        .unwrap()
    };
    let index = |offset, minimum_length, from_end| {
        (
            SemanticProjectionKindV1::ConstantIndex {
                offset,
                minimum_length,
                from_end,
            },
            old.locals()[4].ty(),
        )
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
    budget.reserve_storage(17).unwrap();
    for from_end in [false, true] {
        let place = make(vec![
            index(1, 2, from_end),
            (SemanticProjectionKindV1::Field(0), U32),
        ]);
        assert!(source_failure_scalar_path_v43(&function, &types, &place, &mut budget).unwrap());
        assert_eq!(budget.storage(), 17);
    }
    for path in [
        vec![
            index(1, 2, false),
            (SemanticProjectionKindV1::Field(2), U32),
        ],
        vec![
            index(1, 2, false),
            (SemanticProjectionKindV1::Field(0), boolean),
        ],
        vec![
            index(2, 3, false),
            (SemanticProjectionKindV1::Field(0), U32),
        ],
        vec![index(3, 3, true), (SemanticProjectionKindV1::Field(0), U32)],
        vec![(
            SemanticProjectionKindV1::ConstantIndex {
                offset: 1,
                minimum_length: 2,
                from_end: false,
            },
            U32,
        )],
    ] {
        assert!(
            source_failure_scalar_path_v43(&function, &types, &make(path), &mut budget).is_err()
        );
        assert_eq!(budget.storage(), 17);
    }
    for path in [
        vec![(SemanticProjectionKindV1::Dereference, U32)],
        vec![(
            SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(1)),
            U32,
        )],
        vec![index(1, 2, false)],
    ] {
        assert!(
            !source_failure_scalar_path_v43(&function, &types, &make(path), &mut budget).unwrap()
        );
        assert_eq!(budget.storage(), 17);
    }
    // These malformed offsets cannot reach the owner-backed path walker:
    // the original projection constructor rejects them first.
    for (offset, minimum_length, from_end) in [(2, 2, false), (3, 2, true), (0, 2, true)] {
        assert!(
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end
                },
                old.locals()[4].ty(),
            )
            .is_err()
        );
    }
    assert!(source_static_constant_index_v29(2, 2, 2, false).is_err());
    assert!(source_static_constant_index_v29(2, 3, 2, true).is_err());
    assert!(source_static_constant_index_v29(2, 0, 2, true).is_err());
    types[array.index() as usize] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([246; 32]),
        SemanticLayoutIdentityV1::from_sha256([246; 32]),
        SemanticTypeLayoutV1::new(Some(32), 4).unwrap(),
        SemanticTypeShapeV1::Array {
            element: old.locals()[4].ty(),
            length: 4,
        },
    );
    for (from_end, expected_index) in [(false, 1), (true, 3)] {
        assert_eq!(
            source_static_constant_index_v29(4, 1, 4, from_end).unwrap(),
            expected_index
        );
        let place = make(vec![
            index(1, 4, from_end),
            (SemanticProjectionKindV1::Field(0), U32),
        ]);
        assert!(source_failure_scalar_path_v43(&function, &types, &place, &mut budget).unwrap());
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn promoted_failure_scalar_path_has_exact_paid_work_and_scratch_boundaries() {
    let owner = add_loop();
    let source = owner.source_semantic();
    let function = &source.functions()[0];
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(4),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U32).unwrap()],
        U32,
    )
    .unwrap();
    type Fields<'a> = (
        &'a SemanticFunctionDeclV1,
        &'a [SemanticTypeDeclV1],
        &'a SemanticPlaceV1,
        SemanticTypeIdV1,
        std::slice::Iter<'a, SemanticProjectionV1>,
        &'a SemanticProjectionV1,
        &'a SemanticTypeDeclV1,
        Option<SemanticTypeIdV1>,
    );
    let header =
        size_of::<Fields<'_>>() + 2 * size_of::<Result<Fields<'_>, ProductionSemanticKirErrorV1>>();
    assert_eq!(
        header,
        source_reference_emission_headers_v29::<SourceFailureScalarPathFrameV43<'_>>().unwrap()
    );
    for (work_limit, storage_limit, expected) in [
        (12, 17 + header, None),
        (11, 17 + header, Some(true)),
        (12, 16 + header, Some(false)),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(17).unwrap();
        let result =
            source_failure_scalar_path_v43(function, source.types(), &projected, &mut budget);
        match expected {
            None => {
                assert!(result.unwrap());
                assert_eq!(budget.work(), 12);
                assert_eq!(budget.peak_storage(), 17 + header);
            }
            Some(work) => {
                let error = result.unwrap_err();
                match (work, error) {
                    (
                        true,
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(limit),
                        ),
                    ) => {
                        assert_eq!(limit.limit(), 11);
                        assert_eq!(limit.actual(), 12);
                    }
                    (
                        false,
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(limit),
                        ),
                    ) => {
                        assert_eq!(limit.limit(), 16 + header);
                        assert_eq!(limit.actual(), 17 + header);
                    }
                    (_, error) => panic!("exact resource refusal: {error:?}"),
                }
            }
        }
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn source_checked_tuple_components_bind_original_and_optimized_assert_and_backedge() {
    for factory in [
        add_loop as fn() -> _,
        sub_loop,
        mul_loop,
        copied_loop,
        moved_loop,
    ] {
        let reached = std::cell::Cell::new(false);
        with_policy11(factory, |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(optimized, 0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22, budget, |leaves, budget| {
                    let input = leaves.original.leaves;
                    assert!(!input.boundaries.rows.is_empty());
                    let semantic = original.source.source_semantic(budget)?;
                    let function = &semantic.functions()[0];
                    assert!(matches!(function.blocks()[2].statements()[0].kind(),
                        SemanticStatementKindV1::Assign(assignment) if matches!(assignment.value().kind(), SemanticRvalueKindV1::CheckedBinary(_))));
                    assert!(matches!(function.blocks()[2].terminator().kind(),
                        SemanticTerminatorKindV1::Assert { condition: SemanticOperandV1::Copy(place), expected: false, .. }
                        if matches!(place.projections(), [projection] if projection.kind() == SemanticProjectionKindV1::Field(1))));
                    input.check_boundary_equations_v31(budget)?;
                    input.check_boundary_actual_v31(Some(leaves), budget)?;
                    reached.set(true);
                    Ok(())
                })
        }).unwrap();
        assert!(reached.get());
    }
}

#[test]
fn source_checked_tuple_components_reject_copied_occurrences_wrong_field_types_and_sites() {
    for fault in 0..3 {
        let entered = std::cell::Cell::new(false);
        let result = with_policy11(add_loop, |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    with_check(leaves, budget, |check, budget| {
                        let semantic = original.source.source_semantic(budget)?;
                        let function = &semantic.functions()[0];
                        let SemanticTerminatorKindV1::Assert {
                            condition: SemanticOperandV1::Copy(place),
                            ..
                        } = function.blocks()[2].terminator().kind()
                        else {
                            unreachable!()
                        };
                        let copied = place.clone();
                        let mut remaining =
                            fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
                        let input = ProductionSourceScalarLeavesV18 {
                            leaves: leaves.original.leaves,
                        };
                        entered.set(true);
                        check
                            .index
                            .checked_component_expression_v41(
                                &input,
                                0,
                                if fault == 1 { U32 } else { place.ty() },
                                ProductionSemanticScalarTypeV2::Bool,
                                EntrySiteV20::Terminator {
                                    block: fe2o3_mir_model::SsaBlockIdV1::new(if fault == 2 {
                                        1
                                    } else {
                                        2
                                    }),
                                },
                                EntryOperandV20::AssertCondition,
                                if fault == 0 { &copied } else { place },
                                0,
                                &mut remaining,
                                budget,
                            )
                            .map(drop)
                    })
                },
            )
        });
        assert!(entered.get());
        assert_binding(
            result,
            if fault == 1 {
                "checked source component result type differs"
            } else {
                "checked source component original holder or occurrence differs"
            },
        );
    }
}

fn resource_cut(cut: Option<(bool, usize)>) -> (usize, usize, bool) {
    let measured = std::cell::Cell::new(None);
    let result = with_policy11(mul_loop, |original, optimized, budget| {
        let floor = budget.storage();
        let result =
            source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
                let available = if let Some((false, limit)) = cut {
                    limit
                } else {
                    MODULE_LIMIT / 2
                };
                budget.reserve_storage(MODULE_LIMIT - budget.storage() - available)?;
                if let Some((true, limit)) = cut {
                    budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - limit)?;
                }
                let before = (budget.work(), budget.storage());
                let entered = std::cell::Cell::new(false);
                let result = original.with_optimized_scalar_leaf_namespace_v18(
                    optimized,
                    0,
                    &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                    budget,
                    |_, _| {
                        entered.set(true);
                        Err::<(), _>(ProductionSourceOwnedViewErrorV18::Binding(
                            "checked component resource stop",
                        ))
                    },
                );
                measured.set(Some((
                    budget.work() - before.0,
                    budget.peak_storage() - before.1,
                    entered.get(),
                )));
                match (&result, entered.get(), cut) {
                    (
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "checked component resource stop",
                        )),
                        true,
                        _,
                    ) => {}
                    (
                        Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(
                            error,
                        ))),
                        false,
                        Some((true, _)),
                    ) => {
                        assert_eq!(error.limit(), OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                        assert_eq!(error.actual(), error.limit() + 1);
                    }
                    (
                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Storage(error),
                        )),
                        false,
                        Some((false, _)),
                    ) => {
                        assert_eq!(error.limit(), MODULE_LIMIT);
                        assert!(error.actual() > error.limit());
                    }
                    _ => panic!("unexpected checked component resource result: {result:?}"),
                }
                result
            });
        assert_eq!(budget.storage(), floor);
        result
    });
    assert!(result.is_err());
    measured.get().unwrap()
}

#[test]
fn source_checked_tuple_components_use_exact_and_one_short_cumulative_budgets() {
    let (work, storage, entered) = resource_cut(None);
    assert!(entered && work > 0 && storage > 0);
    assert_eq!(resource_cut(Some((true, work))), (work, storage, true));
    assert!(!resource_cut(Some((true, work - 1))).2);
    assert_eq!(resource_cut(Some((false, storage))), (work, storage, true));
    assert!(!resource_cut(Some((false, storage - 1))).2);
}

struct ArithmeticTree(Box<CheckedArithmeticNodeV41<ArithmeticTree>>);

fn arithmetic_tree(
    operation: CheckedBinaryOperator,
    scalar: ProductionSemanticScalarTypeV2,
) -> ArithmeticTree {
    checked_overflow_expression_v41(
        operation,
        scalar,
        &mut |node| Ok::<_, ()>(ArithmeticTree(Box::new(node))),
        || (),
    )
    .unwrap()
}

fn signed_value(bits: u64, width: u16) -> i128 {
    let value = i128::from(bits);
    if bits & (1u64 << (width - 1)) != 0 {
        value - (1i128 << width)
    } else {
        value
    }
}

fn evaluate(tree: &ArithmeticTree, values: [u64; 2]) -> u64 {
    use CheckedArithmeticNodeV41 as N;
    match tree.0.as_ref() {
        N::Operand(index) => values[*index],
        N::Constant { bits, .. } => *bits,
        N::Select {
            condition,
            when_true,
            when_false,
        } => {
            let condition = evaluate(condition, values);
            assert!(condition <= 1);
            // Deliberately lazy: division by zero and MIN/-1 are invalid even
            // when their wrapping expression sits in an unselected branch.
            evaluate(
                if condition == 1 {
                    when_true
                } else {
                    when_false
                },
                values,
            )
        }
        N::Binary {
            operation,
            scalar,
            lhs,
            rhs,
        } => {
            let (signed, width) = checked_arithmetic_scalar_v41(*scalar).unwrap();
            let lhs = evaluate(lhs, values);
            let rhs = evaluate(rhs, values);
            let mask = u128::from(u64::MAX >> (64 - width));
            let result = match operation {
                ProductionSemanticBinaryOpV2::Add => u128::from(lhs) + u128::from(rhs),
                ProductionSemanticBinaryOpV2::Subtract => {
                    u128::from(lhs).wrapping_sub(u128::from(rhs))
                }
                ProductionSemanticBinaryOpV2::Multiply => u128::from(lhs) * u128::from(rhs),
                ProductionSemanticBinaryOpV2::Divide if signed => {
                    let lhs = signed_value(lhs, width);
                    let rhs = signed_value(rhs, width);
                    assert!(rhs != 0 && !(lhs == -(1i128 << (width - 1)) && rhs == -1));
                    (lhs / rhs) as u128
                }
                ProductionSemanticBinaryOpV2::Divide => {
                    assert_ne!(rhs, 0);
                    u128::from(lhs / rhs)
                }
                _ => panic!("unexpected arithmetic recipe operator"),
            };
            (result & mask) as u64
        }
        N::Compare {
            operation,
            scalar,
            lhs,
            rhs,
        } => {
            let lhs = evaluate(lhs, values);
            let rhs = evaluate(rhs, values);
            let (lhs, rhs) = match *scalar {
                ProductionSemanticScalarTypeV2::Integer { signed: true, bits } => {
                    (signed_value(lhs, bits), signed_value(rhs, bits))
                }
                _ => (i128::from(lhs), i128::from(rhs)),
            };
            u64::from(match operation {
                ProductionSemanticComparisonV2::Equal => lhs == rhs,
                ProductionSemanticComparisonV2::NotEqual => lhs != rhs,
                ProductionSemanticComparisonV2::LessThan => lhs < rhs,
                _ => panic!("unexpected arithmetic recipe comparison"),
            })
        }
    }
}

fn mathematical_overflow(
    operation: CheckedBinaryOperator,
    signed: bool,
    width: u16,
    [lhs, rhs]: [u64; 2],
) -> bool {
    if signed {
        let lhs = signed_value(lhs, width);
        let rhs = signed_value(rhs, width);
        let value = match operation {
            CheckedBinaryOperator::Add => lhs + rhs,
            CheckedBinaryOperator::Subtract => lhs - rhs,
            CheckedBinaryOperator::Multiply => lhs * rhs,
        };
        value < -(1i128 << (width - 1)) || value >= (1i128 << (width - 1))
    } else {
        let lhs = u128::from(lhs);
        let rhs = u128::from(rhs);
        let value = match operation {
            CheckedBinaryOperator::Add => Some(lhs + rhs),
            CheckedBinaryOperator::Subtract => lhs.checked_sub(rhs),
            CheckedBinaryOperator::Multiply => Some(lhs * rhs),
        };
        value.is_none_or(|v| v >= (1u128 << width))
    }
}

#[test]
fn checked_overflow_recipe_exhausts_all_signed_and_unsigned_eight_bit_inputs() {
    for operation in [
        CheckedBinaryOperator::Add,
        CheckedBinaryOperator::Subtract,
        CheckedBinaryOperator::Multiply,
    ] {
        for signed in [false, true] {
            let tree = arithmetic_tree(
                operation,
                ProductionSemanticScalarTypeV2::Integer { signed, bits: 8 },
            );
            for lhs in 0..=255 {
                for rhs in 0..=255 {
                    assert_eq!(
                        evaluate(&tree, [lhs, rhs]) == 1,
                        mathematical_overflow(operation, signed, 8, [lhs, rhs]),
                        "{operation:?} signed={signed} {lhs} {rhs}"
                    );
                }
            }
        }
    }
}

#[test]
fn checked_overflow_recipe_covers_all_admitted_widths_and_lazy_invalid_quotients() {
    for width in [8, 16, 32, 64] {
        let maximum = u64::MAX >> (64 - width);
        let minimum = 1u64 << (width - 1);
        let inputs = [
            0,
            1,
            2,
            minimum - 1,
            minimum,
            minimum + 1,
            maximum - 1,
            maximum,
        ];
        for signed in [false, true] {
            for operation in [
                CheckedBinaryOperator::Add,
                CheckedBinaryOperator::Subtract,
                CheckedBinaryOperator::Multiply,
            ] {
                let tree = arithmetic_tree(
                    operation,
                    ProductionSemanticScalarTypeV2::Integer {
                        signed,
                        bits: width,
                    },
                );
                for lhs in inputs {
                    for rhs in inputs {
                        assert_eq!(
                            evaluate(&tree, [lhs, rhs]) == 1,
                            mathematical_overflow(operation, signed, width, [lhs, rhs]),
                            "{operation:?} signed={signed} width={width} {lhs} {rhs}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn checked_overflow_recipe_explicitly_refuses_noninteger_and_unmodeled_widths() {
    for scalar in [
        ProductionSemanticScalarTypeV2::Bool,
        ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 1,
        },
        ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 128,
        },
        ProductionSemanticScalarTypeV2::Integer {
            signed: true,
            bits: 128,
        },
    ] {
        let mut called = false;
        assert!(
            checked_overflow_expression_v41(
                CheckedBinaryOperator::Add,
                scalar,
                &mut |_| {
                    called = true;
                    Ok::<_, ()>(())
                },
                || ()
            )
            .is_err()
        );
        assert!(!called);
    }
}

fn independent_formula_headers<T>() -> usize {
    size_of::<[Option<T>; 8]>()
        + size_of::<CheckedArithmeticNodeV41<T>>()
        + 3 * size_of::<T>()
        + size_of::<Option<T>>()
        + size_of::<&[CheckedArithmeticInstructionV41]>()
        + size_of::<std::slice::Iter<'_, CheckedArithmeticInstructionV41>>()
        + size_of::<CheckedArithmeticInstructionV41>()
        + size_of::<(
            CheckedBinaryOperator,
            ProductionSemanticScalarTypeV2,
            usize,
            bool,
            u16,
            u64,
        )>()
}

#[test]
fn checked_component_headers_match_independent_formula_and_source_frames() {
    assert_eq!(CHECKED_ARITHMETIC_STACK_V41, 8);
    assert_eq!(
        checked_arithmetic_headers_v41::<ProductionSemanticExpressionV2>(),
        independent_formula_headers::<ProductionSemanticExpressionV2>()
    );
    assert_eq!(
        checked_arithmetic_headers_v41::<NormalizedScalarExpressionV1>(),
        independent_formula_headers::<NormalizedScalarExpressionV1>()
    );
    type SourceFrame<'a> = (
        &'a OriginalEntryIndexV20<'a, 'a>,
        &'a ProductionSourceScalarLeavesV18<'a>,
        &'a SemanticFunctionDeclV1,
        &'a SemanticPlaceV1,
        &'a fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
        [&'a SemanticOperandV1; 2],
        [&'a SemanticTypeDeclV1; 3],
        &'a [SemanticTypeIdV1],
        OriginalEntryDefinitionRowV20,
        EntryValueV20,
        EntrySiteV20,
        EntryOperandV20,
        [ProductionSourceSsaEndpointV36<'a, 'a>; 3],
        [Option<usize>; 2],
        [SourceOwnedResultV18<Option<usize>>; 2],
        [fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1; 2],
        [usize; 8],
        [SemanticTypeIdV1; 3],
        CheckedBinaryOperator,
        ProductionSemanticScalarTypeV2,
        ProductionSemanticScalarTypeV2,
        SourceOwnedResultV18<ProductionSemanticExpressionV2>,
        [&'a (); 6],
    );
    assert_eq!(
        source_checked_component_headers_v41().unwrap(),
        size_of::<SourceFrame<'_>>()
            + std::mem::align_of::<SourceFrame<'_>>()
            + independent_formula_headers::<ProductionSemanticExpressionV2>()
    );
}
