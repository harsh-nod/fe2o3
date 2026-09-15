use super::*;
use crate::production::{
    ProductionSemanticMirLimitsV1, ProductionSemanticSsaSourceQueryErrorV1,
    ProductionSemanticSsaSourceSiteV1, SemanticPartialMoveViolationV1,
};

#[path = "shared_carrier_fixture.rs"]
mod fixture;
use fixture::{CARRIER, CARRIER_LOCAL, CARRIER_REF};

#[path = "shared_carrier_tests/shared_budget_tests.rs"]
mod shared_budget_tests;

#[path = "shared_carrier_observation_tests.rs"]
mod observation_tests;

fn budget(limit: usize) -> Budget {
    Budget {
        remaining: limit,
        limit,
        profile: FlowWorkProfile::default(),
    }
}

fn owner(
    source: AdmittedInertSemanticMirV1,
) -> Result<ProductionSemanticSsaOwnerV1, ProductionSemanticSsaErrorV1> {
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
}

fn readmit(
    source: &AdmittedInertSemanticMirV1,
    root: SemanticFunctionDeclV1,
) -> AdmittedInertSemanticMirV1 {
    let mut functions = source.functions().to_vec();
    functions[0] = root;
    InertSemanticMirRequestV1::new_with_callables(
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
    .admit_exact_v20(Default::default())
    .unwrap()
}

fn changed_statements(
    body: &SemanticFunctionDeclV1,
    index: usize,
    statements: Vec<SemanticStatementV1>,
) -> SemanticFunctionDeclV1 {
    let mut blocks = body.blocks().to_vec();
    let old = &blocks[index];
    blocks[index] = SemanticBasicBlockV1::new(
        old.identity(),
        old.source(),
        statements,
        old.terminator().clone(),
    )
    .unwrap();
    fixture::rebuild(body, body.locals().to_vec(), blocks)
}

#[test]
fn repeated_shared_workgroup_carrier_has_original_borrow_uses_on_the_replayed_owner() {
    let source = fixture::admitted();
    let source_identity = *source.semantic_sha256().as_bytes();
    let root = source.roots()[0];
    let original = source.functions()[0].clone();
    let owner = owner(source).unwrap();
    owner.verify_replay().unwrap();
    assert_eq!(owner.source_semantic_sha256(), &source_identity);
    assert_eq!(owner.source_semantic().functions()[0], original);
    assert!(!owner.grants_proof_or_artifact_authority());
    let view = owner.execution_view_for_root(root).unwrap();
    assert!(view.has_expanded_calls());
    let borrows = fixture::original_borrows(view.body());
    assert_eq!(borrows.len(), 2);
    let query = owner.source_query_for_root(root, view.body()).unwrap();
    let mut values = Vec::new();
    for (site, place) in borrows {
        assert!(
            owner
                .execution_plan_for_root(root)
                .unwrap()
                .plan()
                .promoted_variables()
                .contains(&SsaVariableIdV1::new(place.local().index()))
        );
        let site = ProductionSemanticSsaSourceSiteV1::new(
            SemanticBlockIdV1::from_index(site.block),
            Some(site.statement),
        );
        values.push(query.borrow_place_use(site, place, &mut || true).unwrap());
        // Equality of a copied place is deliberately insufficient for a token.
        assert!(matches!(
            query.borrow_place_use(site, &place.clone(), &mut || true),
            Err(ProductionSemanticSsaSourceQueryErrorV1::OperandOutsideSite)
        ));
        assert!(query.borrow_place_use(site, place, &mut || false).is_err());
    }
    assert_eq!(
        values[0], values[1],
        "both borrows use the same unchanged initialized carrier"
    );
}

#[test]
fn missing_carrier_initialization_never_becomes_a_promoted_borrow_use() {
    let source = fixture::admitted();
    let root = &source.functions()[0];
    let mut statements = root.blocks()[0].statements().to_vec();
    statements.retain(|s| {
        !matches!(s.kind(), SemanticStatementKindV1::Assign(a)
        if a.destination().local().index() == CARRIER_LOCAL)
    });
    let source = readmit(&source, changed_statements(root, 0, statements));
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    let view = expansion.root(source.roots()[0]).unwrap();
    let selected = execution_sites(&source, &expansion, view, MAX_FLOW_WORK).unwrap();
    let borrows = fixture::original_borrows(view.body());
    assert_eq!(borrows.len(), 2);
    for (site, _) in borrows {
        assert!(!selected.contains(&site));
    }
}

#[test]
fn original_carrier_storage_death_and_move_still_reject_at_ssa_construction() {
    for moved in [false, true] {
        let source = fixture::admitted();
        let root = &source.functions()[0];
        let mut statements = root.blocks()[0].statements().to_vec();
        let index = statements.len() - 1;
        statements.insert(
            index,
            if moved {
                assign(
                    11,
                    CARRIER,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(
                        CARRIER_LOCAL,
                        CARRIER,
                    ))),
                )
            } else {
                statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(CARRIER_LOCAL),
                ))
            },
        );
        let error = owner(readmit(&source, changed_statements(root, 0, statements))).unwrap_err();
        let cause = match &error {
            ProductionSemanticSsaErrorV1::ExpandedExecution {
                root,
                source_local: Some(origin),
                error: cause,
                ..
            } => {
                assert_eq!(*root, source.roots()[0]);
                assert_eq!(origin.instance().index(), 0);
                assert_eq!(origin.function().index(), 0);
                assert_eq!(origin.local().index(), CARRIER_LOCAL);
                cause.as_ref()
            }
            cause => cause,
        };
        assert!(
            match cause {
                ProductionSemanticSsaErrorV1::PartialMove {
                    local: CARRIER_LOCAL,
                    violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                    ..
                } => true,
                ProductionSemanticSsaErrorV1::Planner {
                    error: SsaPlannerErrorV1::UndefinedAtUse { variable, .. },
                    ..
                } => variable.get() == CARRIER_LOCAL,
                _ => false,
            },
            "moved={moved}: {error:?}"
        );
    }
}

#[test]
fn repeated_definition_and_unknown_observation_poison_the_whole_carrier_component() {
    for duplicate in [false, true] {
        let source = fixture::admitted();
        let root = &source.functions()[0];
        let mut statements = root.blocks()[0].statements().to_vec();
        let mut locals = root.locals().to_vec();
        locals.push(local(12, 5, SemanticLocalRoleV1::Temporary));
        let projected = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(7),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(CARRIER))
                    .unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty(5)).unwrap(),
            ],
            ty(5),
        )
        .unwrap();
        statements.push(assign(
            12,
            5,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected)),
        ));
        let term = call(
            4,
            vec![SemanticOperandV1::Copy(place(12, 5))],
            place(8, 9),
            1,
        );
        let leaf_body = |statements| {
            fixture::rebuild(
                root,
                locals.clone(),
                vec![
                    block(0, statements, term.clone()),
                    block(1, vec![], SemanticTerminatorKindV1::Return),
                ],
            )
        };
        let baseline = leaf_body(statements.clone());
        let selected = sites(
            &baseline,
            source.callables(),
            &[],
            MAX_FLOW_WORK,
            Some(source.types()),
        )
        .unwrap();
        let original = fixture::original_borrows(&baseline)[0].0;
        assert!(
            selected.contains(&original),
            "positive complete-use classifier control"
        );
        if duplicate {
            let duplicate = statements[4].clone();
            statements.insert(5, duplicate);
        } else {
            let deref = SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(7),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(CARRIER))
                        .unwrap(),
                ],
                ty(CARRIER),
            )
            .unwrap();
            statements.push(statement(SemanticStatementKindV1::Assign(
                SemanticAssignmentV1::new(
                    place(11, CARRIER),
                    SemanticRvalueV1::new(
                        ty(CARRIER),
                        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                            deref,
                            SemanticVolatilityV1::NonVolatile,
                            None,
                        )),
                    ),
                ),
            )));
        }
        // The unknown Load mutation is a classifier input, not admitted MIR or
        // invented memory evidence. Every unrecognized carrier use must poison.
        let changed = leaf_body(statements);
        let routes = math_capture_flow_v1::Routes::new(
            &changed,
            Some(source.types()),
            source.callables(),
            &mut budget(MAX_FLOW_WORK),
        )
        .unwrap();
        assert_eq!(
            routes
                .shared_owned(ty(CARRIER_REF), &mut budget(MAX_FLOW_WORK))
                .unwrap(),
            Some(ty(4))
        );
        let selected = sites(
            &changed,
            source.callables(),
            &[],
            MAX_FLOW_WORK,
            Some(source.types()),
        )
        .unwrap();
        for (site, _) in fixture::original_borrows(&changed) {
            assert!(!selected.contains(&site));
        }
    }
}

#[test]
fn shared_carrier_requires_exact_dereference_then_exact_field_type() {
    let source = fixture::admitted();
    let helper = &source.functions()[3];
    // Its carrier value type is also retained in the root. Add an inert local to
    // this leaf-only classifier fixture so both ends of the route are present.
    let mut locals = helper.locals().to_vec();
    locals.push(local(3, CARRIER, SemanticLocalRoleV1::Temporary));
    let helper = fixture::rebuild(helper, locals, helper.blocks().to_vec());
    let routes = math_capture_flow_v1::Routes::new(
        &helper,
        Some(source.types()),
        source.callables(),
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    for (projections, result, accepted) in [
        (
            vec![
                (SemanticProjectionKindV1::Dereference, CARRIER),
                (SemanticProjectionKindV1::Field(0), 5),
            ],
            5,
            true,
        ),
        (vec![(SemanticProjectionKindV1::Field(0), 5)], 5, false),
        (
            vec![
                (SemanticProjectionKindV1::Dereference, 4),
                (SemanticProjectionKindV1::Field(0), 5),
            ],
            5,
            false,
        ),
        (
            vec![
                (SemanticProjectionKindV1::Dereference, CARRIER),
                (SemanticProjectionKindV1::Field(1), 5),
            ],
            5,
            false,
        ),
        (
            vec![
                (SemanticProjectionKindV1::Dereference, CARRIER),
                (SemanticProjectionKindV1::Field(0), 1),
            ],
            1,
            false,
        ),
    ] {
        let place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            projections
                .into_iter()
                .map(|(kind, id)| SemanticProjectionV1::new(kind, ty(id)).unwrap())
                .collect(),
            ty(result),
        )
        .unwrap();
        let assignment = SemanticAssignmentV1::new(
            super::place(2, 5),
            SemanticRvalueV1::new(
                ty(5),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)),
            ),
        );
        assert_eq!(
            routes
                .source(&assignment, &mut budget(MAX_FLOW_WORK))
                .unwrap()
                .is_some(),
            accepted
        );
    }
}

#[test]
fn outer_reference_shape_and_inner_uniqueness_are_required_without_shape_authority() {
    let source = fixture::admitted();
    let root = &source.functions()[0];
    for (kind, mutability, space, bits, metadata, pointee) in [
        (
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            0,
            64,
            SemanticPointerMetadataV1::None,
            CARRIER,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            0,
            64,
            SemanticPointerMetadataV1::None,
            CARRIER,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            1,
            64,
            SemanticPointerMetadataV1::None,
            CARRIER,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
            32,
            SemanticPointerMetadataV1::None,
            CARRIER,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
            64,
            SemanticPointerMetadataV1::SliceLength,
            CARRIER,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
            64,
            SemanticPointerMetadataV1::None,
            4,
        ),
    ] {
        let mut types = source.types().to_vec();
        let old = &types[CARRIER_REF as usize];
        types[CARRIER_REF as usize] = SemanticTypeDeclV1::new(
            old.identity(),
            old.layout_identity(),
            old.layout().clone(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(pointee),
                    kind,
                    mutability,
                    space,
                    bits,
                    metadata,
                )
                .unwrap(),
            ),
        );
        let routes = math_capture_flow_v1::Routes::new(
            root,
            Some(&types),
            source.callables(),
            &mut budget(MAX_FLOW_WORK),
        )
        .unwrap();
        assert_eq!(
            routes
                .shared_owned(ty(CARRIER_REF), &mut budget(MAX_FLOW_WORK))
                .unwrap(),
            None
        );
    }
    let mut types = source.types().to_vec();
    let old = &types[CARRIER as usize];
    types[CARRIER as usize] = SemanticTypeDeclV1::new(
        old.identity(),
        old.layout_identity(),
        old.layout().clone(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![ty(5), ty(5)]).unwrap()),
    );
    let routes = math_capture_flow_v1::Routes::new(
        root,
        Some(&types),
        source.callables(),
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    assert_eq!(
        routes
            .shared_owned(ty(CARRIER_REF), &mut budget(MAX_FLOW_WORK))
            .unwrap(),
        None
    );
    // Even the exact valid shape is inert without its bound source callable.
    let mut callables = source.callables().to_vec();
    callables[4] = borrowed_callable(5, false);
    let routes = math_capture_flow_v1::Routes::new(
        root,
        Some(source.types()),
        &callables,
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    assert_eq!(
        routes
            .shared_owned(ty(CARRIER_REF), &mut budget(MAX_FLOW_WORK))
            .unwrap(),
        None
    );
}

#[test]
fn whole_carrier_mutable_borrow_and_nested_dereference_remain_unsupported() {
    let source = fixture::admitted();
    let root = &source.functions()[0];
    let routes = math_capture_flow_v1::Routes::new(
        root,
        Some(source.types()),
        source.callables(),
        &mut budget(MAX_FLOW_WORK),
    )
    .unwrap();
    let reborrow = SemanticAssignmentV1::new(
        place(9, CARRIER_REF),
        SemanticRvalueV1::new(
            ty(CARRIER_REF),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(7),
                    vec![
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::Dereference,
                            ty(CARRIER),
                        )
                        .unwrap(),
                    ],
                    ty(CARRIER),
                )
                .unwrap(),
            },
        ),
    );
    assert!(
        routes
            .source(&reborrow, &mut budget(MAX_FLOW_WORK))
            .unwrap()
            .is_some()
    );
    for (kind, from) in [
        (SemanticBorrowKindV1::Mutable, place(CARRIER_LOCAL, CARRIER)),
        (
            SemanticBorrowKindV1::Shared,
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(7),
                vec![
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Dereference,
                        ty(CARRIER_REF),
                    )
                    .unwrap(),
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(CARRIER))
                        .unwrap(),
                ],
                ty(CARRIER),
            )
            .unwrap(),
        ),
    ] {
        let a = SemanticAssignmentV1::new(
            place(9, CARRIER_REF),
            SemanticRvalueV1::new(
                ty(CARRIER_REF),
                SemanticRvalueKindV1::Borrow { kind, place: from },
            ),
        );
        assert!(
            routes
                .source(&a, &mut budget(MAX_FLOW_WORK))
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn new_workgroup_route_and_shared_edge_have_exact_one_less_work_boundaries() {
    let source = fixture::admitted();
    let root = &source.functions()[0];
    let mut measured = budget(MAX_FLOW_WORK);
    let routes = math_capture_flow_v1::Routes::new(
        root,
        Some(source.types()),
        source.callables(),
        &mut measured,
    )
    .unwrap();
    let required = MAX_FLOW_WORK - measured.remaining;
    assert!(required > 0);
    math_capture_flow_v1::Routes::new(
        root,
        Some(source.types()),
        source.callables(),
        &mut budget(required),
    )
    .unwrap();
    assert!(matches!(
        math_capture_flow_v1::Routes::new(
            root,
            Some(source.types()),
            source.callables(),
            &mut budget(required - 1)
        ),
        Err(ProductionSemanticSsaErrorV1::BorrowFlowWork { .. })
    ));
    let SemanticStatementKindV1::Assign(a) = root.blocks()[0].statements().last().unwrap().kind()
    else {
        panic!("borrow")
    };
    let mut measured = budget(MAX_FLOW_WORK);
    assert!(routes.source(a, &mut measured).unwrap().is_some());
    let required = MAX_FLOW_WORK - measured.remaining;
    assert!(required > 0);
    assert!(routes.source(a, &mut budget(required)).unwrap().is_some());
    assert!(matches!(
        routes.source(a, &mut budget(required - 1)),
        Err(ProductionSemanticSsaErrorV1::BorrowFlowWork { .. })
    ));
}
