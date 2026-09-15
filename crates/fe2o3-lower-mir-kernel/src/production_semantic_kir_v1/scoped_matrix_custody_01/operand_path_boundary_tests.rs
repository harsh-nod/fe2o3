use fe2o3_mir_model::{SemanticExpandedStatementOriginV1, SsaVariableIdV1};
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1,
    ProductionSemanticSsaSourceQueryErrorV1 as QueryError,
    ProductionSemanticSsaSourceSiteV1 as QuerySite,
};

// Real admitted/expanded/planned scalar source, not a Matrix/Context issuer.
// These exercise the exact query boundary used by operand_path and its Borrow
// recursion. They do not construct a fake Resolver or supply an issuer record.
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}
fn p(local: u32, type_id: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(type_id)).unwrap()
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}
fn assignment(local: u32, type_id: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        p(local, type_id),
        SemanticRvalueV1::new(ty(type_id), value),
    )))
}
fn constant() -> SemanticRvalueKindV1 {
    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty(1),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 4).unwrap()),
    )))
}
fn copy(local: u32, type_id: u32) -> SemanticRvalueKindV1 {
    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p(local, type_id)))
}

fn fixture(borrow: bool, dead_reference: bool) -> ProductionSemanticMirOwnerV1 {
    fixture_with_reborrow(borrow, dead_reference, false)
}

fn fixture_with_reborrow(
    borrow: bool,
    dead_reference: bool,
    reborrow: bool,
) -> ProductionSemanticMirOwnerV1 {
    fixture_initialized(borrow, dead_reference, reborrow, true)
}

fn fixture_initialized(
    borrow: bool,
    dead_reference: bool,
    reborrow: bool,
    initialized: bool,
) -> ProductionSemanticMirOwnerV1 {
    fixture_with_observation(borrow, dead_reference, reborrow, initialized, false)
}

fn fixture_with_observation(
    borrow: bool,
    dead_reference: bool,
    reborrow: bool,
    initialized: bool,
    observe_reference: bool,
) -> ProductionSemanticMirOwnerV1 {
    let seed = super::super::super::resource_tests::helper_closure_semantic_owner_with_calls(2);
    let source = seed.semantic();
    let original = &source.functions()[1];
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let mut types = source.types().to_vec();
    assert_eq!(types.len(), 1);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([71; 32]),
        SemanticLayoutIdentityV1::from_sha256([72; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX as u128),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    ));
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([73; 32]),
        SemanticLayoutIdentityV1::from_sha256([74; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ty(1),
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let mut locals = original.locals().to_vec();
    assert_eq!(locals.len(), 1);
    assert_eq!(locals[0].identity(), SemanticLocalIdentityV1::from_sha256([217; 32]));
    for (offset, type_id) in [1, 2, 2, 1, 1].into_iter().enumerate() {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([218 + offset as u8; 32]),
            ty(type_id),
            SemanticLocalRoleV1::Temporary,
            provenance,
        ));
    }
    let mut statements = if initialized {
        vec![assignment(1, 1, constant())]
    } else {
        Vec::new()
    };
    if borrow {
        statements.push(assignment(
            2,
            2,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: p(1, 1),
            },
        ));
        if dead_reference {
            statements.push(statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(2),
            )));
        }
        statements.push(assignment(
            3,
            2,
            if reborrow {
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(2),
                        vec![
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(1))
                                .unwrap(),
                        ],
                        ty(1),
                    )
                    .unwrap(),
                }
            } else {
                copy(2, 2)
            },
        ));
    }
    statements.push(assignment(4, 1, copy(1, 1)));
    statements.push(assignment(5, 1, copy(4, 1)));
    if observe_reference {
        assert!(borrow && initialized && !dead_reference);
        // An unused reference chain is address-transparent. This fixture needs
        // a real pointee read to exercise retained storage without owner SSA.
        statements.push(assignment(5, 1, SemanticRvalueKindV1::Use(
            SemanticOperandV1::Copy(SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(3),
                vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(1)).unwrap()],
                ty(1),
            ).unwrap()),
        )));
    }
    let helper = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        vec![
            SemanticBasicBlockV1::new(
                original.blocks()[0].identity(),
                provenance,
                statements,
                SemanticTerminatorV1::new(provenance, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let admitted = InertSemanticMirRequestV1::new(
        source.target().clone(),
        types,
        vec![],
        vec![],
        vec![],
        vec![source.functions()[0].clone(), helper],
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn owner(borrow: bool) -> ProductionSemanticSsaOwnerV1 {
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        fixture_with_observation(borrow, false, false, true, borrow),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    owner
}

fn sites(view: &SemanticExpandedRootV1, source_statement: u32) -> Vec<QuerySite> {
    let mut result = Vec::new();
    for (block, origin) in view.block_origins().iter().enumerate() {
        if origin.function().index() != 1 {
            continue;
        }
        for (statement, marker) in origin.statements().iter().enumerate() {
            if *marker
                == (SemanticExpandedStatementOriginV1::Source {
                    statement: source_statement,
                })
            {
                result.push(QuerySite::new(
                    SemanticBlockIdV1::from_index(block as u32),
                    Some(statement as u32),
                ));
            }
        }
    }
    assert_eq!(
        result.len(),
        2,
        "both original helper instances must remain"
    );
    result
}
fn assign_at(view: &SemanticExpandedRootV1, site: QuerySite) -> &SemanticAssignmentV1 {
    let SemanticStatementKindV1::Assign(a) = view.body().blocks()[site.block().index() as usize]
        .statements()[site.statement().unwrap() as usize]
        .kind()
    else {
        panic!("expected actual source assignment");
    };
    a
}
fn operand_at(view: &SemanticExpandedRootV1, site: QuerySite) -> &SemanticOperandV1 {
    let SemanticRvalueKindV1::Use(operand) = assign_at(view, site).value().kind() else {
        panic!("expected original source Use");
    };
    operand
}
fn local(operand: &SemanticOperandV1) -> u32 {
    match operand {
        SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) => p.local().index(),
        _ => panic!("non-place"),
    }
}
fn missing(result: Result<SsaValueV1>) {
    assert!(
        matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 0,
                block: None,
                statement: None,
                detail: "capability reference has no exact SSA use",
            })
        ),
        "must reject the missing use, not an unrelated source/work boundary: {result:?}"
    );
}

#[test]
fn operand_path_borrowed_place_storage_is_distinct_from_promoted_reference_use() {
    let owner = owner(true);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let plan = owner.execution_plan_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let mut graph = Graph::new(view.body(), plan.plan(), 100_000).unwrap();
    for (borrow_site, reference_site) in sites(view, 1).into_iter().zip(sites(view, 2)) {
        let SemanticRvalueKindV1::Borrow { place, .. } =
            assign_at(view, borrow_site).value().kind()
        else {
            panic!("Borrow");
        };
        let reference = operand_at(view, reference_site);
        assert!(
            !plan
                .plan()
                .promoted_variables()
                .contains(&SsaVariableIdV1::new(place.local().index()))
        );
        assert!(
            plan.plan()
                .promoted_variables()
                .contains(&SsaVariableIdV1::new(local(reference)))
        );
        let use_ = query
            .operand_use(reference_site, reference, &mut || graph.charge(1).is_ok())
            .unwrap();
        assert_eq!(
            graph
                .use_value(reference_site.block().index(), local(reference))
                .unwrap(),
            use_.value()
        );
        let definition = graph.definition(use_.value()).unwrap();
        assert_eq!(
            (definition.block, definition.statement),
            (borrow_site.block().index(), borrow_site.statement())
        );
        missing(graph.use_value(borrow_site.block().index(), place.local().index()));
        assert!(matches!(
            query.borrow_place_use(borrow_site, place, &mut || graph.charge(1).is_ok()),
            Err(QueryError::NoPromotedUse)
        ));
        // Equal-looking Copy is not an original operand of the Borrow node.
        let invented = SemanticOperandV1::Copy(place.clone());
        assert!(matches!(
            query.operand_use(borrow_site, &invented, &mut || graph.charge(1).is_ok()),
            Err(QueryError::OperandOutsideSite)
        ));
    }
}

#[test]
fn operand_path_original_reborrow_place_selects_only_its_existing_root_use() {
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        fixture_with_reborrow(true, false, true),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let mut graph = Graph::new(
        view.body(),
        owner.execution_plan_for_root(ROOT).unwrap().plan(),
        100_000,
    )
    .unwrap();
    let reborrows = sites(view, 2);
    for &site in &reborrows {
        let SemanticRvalueKindV1::Borrow { place, .. } = assign_at(view, site).value().kind()
        else {
            panic!("original Borrow");
        };
        let value = query
            .borrow_place_use(site, place, &mut || graph.charge(1).is_ok())
            .unwrap();
        assert_eq!(
            value,
            graph
                .use_value(site.block().index(), place.local().index())
                .unwrap()
        );
        let clone = place.clone();
        assert!(matches!(
            query.borrow_place_use(site, &clone, &mut || graph.charge(1).is_ok()),
            Err(QueryError::OperandOutsideSite)
        ));
    }
    let SemanticRvalueKindV1::Borrow { place, .. } = assign_at(view, reborrows[0]).value().kind()
    else {
        unreachable!()
    };
    assert!(matches!(
        query.borrow_place_use(reborrows[1], place, &mut || graph.charge(1).is_ok()),
        Err(QueryError::OperandOutsideSite)
    ));
}

#[test]
fn operand_path_plain_copy_uses_original_callee_site_not_the_callers_frame() {
    let owner = owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let plan = owner.execution_plan_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let mut graph = Graph::new(view.body(), plan.plan(), 100_000).unwrap();
    let mut values = Vec::new();
    for site in sites(view, 1) {
        let operand = operand_at(view, site);
        let instance = view.block_origins()[site.block().index() as usize].instance();
        let caller = view
            .block_origins()
            .iter()
            .enumerate()
            .find_map(|(block, origin)| {
                (origin.terminator()
                    == (SemanticExpandedTerminatorOriginV1::CallEntry { callee: instance }))
                .then_some(block as u32)
            })
            .unwrap();
        assert_ne!(caller, site.block().index());
        let correct = query
            .operand_use(site, operand, &mut || graph.charge(1).is_ok())
            .unwrap();
        assert_eq!(
            graph
                .use_value(site.block().index(), local(operand))
                .unwrap(),
            correct.value()
        );
        missing(graph.use_value(caller, local(operand)));
        assert!(matches!(view.body().blocks()[caller as usize].terminator().kind(),
            SemanticTerminatorKindV1::Goto(_)), "the original call was expanded to an operand-free edge");
        assert!(matches!(
            query.operand_use(
                QuerySite::new(SemanticBlockIdV1::from_index(caller), None),
                operand,
                &mut || graph.charge(1).is_ok()
            ),
            Err(QueryError::UnsupportedOperand)
        ));
        let cloned = operand.clone();
        assert!(matches!(
            query.operand_use(site, &cloned, &mut || graph.charge(1).is_ok()),
            Err(QueryError::OperandOutsideSite)
        ));
        values.push(correct.value());
    }
    assert_ne!(
        values[0], values[1],
        "repeated helpers must not collapse current SSA identity"
    );
}

#[test]
fn operand_path_source_use_cannot_cross_call_instances_or_owner_roots() {
    let owner = owner(false);
    let foreign = self::owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    assert!(matches!(
        foreign.source_query_for_root(ROOT, view.body()),
        Err(QueryError::WrongOwner)
    ));
    assert!(matches!(
        owner.source_query_for_root(SemanticFunctionIdV1::from_index(1), view.body()),
        Err(QueryError::WrongOwner)
    ));
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let mut graph = Graph::new(
        view.body(),
        owner.execution_plan_for_root(ROOT).unwrap().plan(),
        100_000,
    )
    .unwrap();
    let sites = sites(view, 1);
    assert!(matches!(
        query.operand_use(sites[1], operand_at(view, sites[0]), &mut || graph
            .charge(1)
            .is_ok()),
        Err(QueryError::OperandOutsideSite)
    ));
    missing(graph.use_value(sites[1].block().index(), local(operand_at(view, sites[0]))));
}

#[test]
fn operand_path_dead_reference_does_not_get_reconstructed_from_its_borrow() {
    let error = ProductionSemanticSsaOwnerV1::try_new(
        fixture(true, true),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap_err();
    assert!(
        matches!(error, fe2o3_pliron::ProductionSemanticSsaErrorV1::Planner {
        error: fe2o3_mir_model::SsaPlannerErrorV1::UndefinedAtUse { variable, .. }, ..
    } if variable.get() == 2),
        "reference death must reject in the real source plan: {error:?}"
    );
}

#[test]
fn operand_path_uninitialized_storage_does_not_acquire_a_borrow_source_value() {
    let source = fixture_initialized(true, false, false, false);
    let expansion = fe2o3_mir_model::SemanticCallExpansionV1::try_new(
        source.semantic(),
        fe2o3_mir_model::SemanticCallExpansionLimitsV1::default(),
    )
    .unwrap();
    let view = expansion.root(ROOT).unwrap();
    let site = sites(view, 0)[0];
    let SemanticRvalueKindV1::Borrow { place, .. } = assign_at(view, site).value().kind() else {
        panic!("original uninitialized owner Borrow");
    };
    let block_origin = &view.block_origins()[site.block().index() as usize];
    let local_origin = view.local_origins()[place.local().index() as usize];
    let error = ProductionSemanticSsaOwnerV1::try_new(
        source,
        ProductionSemanticSsaLimitsV1::default(),
    ).unwrap_err();
    let fe2o3_pliron::ProductionSemanticSsaErrorV1::ExpandedExecution {
        root, source_block, source_statement, source_local, error, ..
    } = error else {
        panic!("expected rejection at the expanded original Borrow: {error:?}");
    };
    assert_eq!(root, ROOT);
    assert_eq!(source_block, Some((block_origin.instance(), block_origin.function(), block_origin.block())));
    assert_eq!(source_statement, Some(SemanticExpandedStatementOriginV1::Source { statement: 0 }));
    assert_eq!(source_local, Some(local_origin));
    assert!(matches!(*error, fe2o3_pliron::ProductionSemanticSsaErrorV1::Planner {
        function, error: fe2o3_mir_model::SsaPlannerErrorV1::UndefinedAtUse { block, variable, .. },
    } if function == ROOT && block.get() == site.block().index() && variable.get() == place.local().index()),
        "uninitialized storage must reject before a source query can acquire a value: {error:?}");
}
