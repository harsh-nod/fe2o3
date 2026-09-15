use super::*;
use crate::production::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaSourceSiteV1,
};
use fe2o3_mir_model::SsaResolvedEventV1;
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "shared_leaf_exclusion_tests.rs"]
mod shared_leaf_exclusions;
#[path = "math_bound_barrier_tests117.rs"]
mod math_bound_barriers;
#[path = "mixed_math_tests117.rs"]
mod mixed_math;
#[path = "secondary_policy_tests119.rs"]
mod secondary_policy;

mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-lower-mir-kernel/src/production_semantic_kir_v1/numerical_policy_math_01/ssa_fixture.rs"
    ));
}

fn budget(limit: usize) -> Budget {
    Budget {
        remaining: limit,
        limit,
        profile: FlowWorkProfile::default(),
    }
}

fn bound_borrow(body: &SemanticFunctionDeclV1) -> (SemanticTransparentBorrowSiteV1, u32) {
    body.blocks()
        .iter()
        .enumerate()
        .find_map(|(block, body)| {
            body.statements()
                .iter()
                .enumerate()
                .find_map(|(statement, item)| {
                    let SemanticStatementKindV1::Assign(a) = item.kind() else {
                        return None;
                    };
                    let SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place,
                    } = a.value().kind()
                    else {
                        return None;
                    };
                    (place.ty() == ty(9) && place.projections().is_empty()).then_some((
                        SemanticTransparentBorrowSiteV1 {
                            block: block as u32,
                            statement: statement as u32,
                        },
                        place.local().index(),
                    ))
                })
        })
        .expect("fixture has an original shared borrow of the retained Bind result")
}

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}
fn place(local: u32, ty_index: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty(ty_index)).unwrap()
}

fn assign(destination: SemanticPlaceV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination.clone(),
            SemanticRvalueV1::new(destination.ty(), value),
        )),
    )
}

fn rebuilt(
    body: &SemanticFunctionDeclV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let result = SemanticFunctionDeclV1::new(
        body.identity(),
        body.role(),
        body.item_definition_identity(),
        body.monomorphization_identity(),
        body.generic_type_arguments_identity(),
        body.const_generic_arguments_identity(),
        body.source(),
        body.abi().clone(),
        body.locals().to_vec(),
        body.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = body.kernel_entry() {
        result.with_kernel_entry(entry.clone())
    } else {
        result
    }
}

fn changed_block(
    body: &SemanticFunctionDeclV1,
    index: usize,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticFunctionDeclV1 {
    let mut blocks = body.blocks().to_vec();
    let old = &blocks[index];
    blocks[index] = SemanticBasicBlockV1::new(
        old.identity(),
        old.source(),
        statements,
        SemanticTerminatorV1::new(old.terminator().source(), terminator),
    )
    .unwrap();
    rebuilt(body, blocks)
}

fn receiver_block(body: &SemanticFunctionDeclV1) -> usize {
    body.blocks()
        .iter()
        .position(|block| {
            block.statements().iter().any(|statement| {
                matches!(statement.kind(), SemanticStatementKindV1::Assign(a)
            if a.destination().local().index() == 14)
            })
        })
        .unwrap()
}

fn classify(
    source: &AdmittedInertSemanticMirV1,
    expansion: &SemanticCallExpansionV1,
    body: &SemanticFunctionDeclV1,
) -> BTreeSet<SemanticTransparentBorrowSiteV1> {
    let view = expansion.root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(source).unwrap();
    let math = MathBorrowSitesV1::new(source, view, &bindings, MAX_FLOW_WORK).unwrap();
    sites_with_math(
        body,
        source.callables(),
        &[],
        MAX_FLOW_WORK,
        Some(source.types()),
        &math,
    )
    .unwrap()
}

#[test]
fn captured_bound_borrow_has_real_ssa_use_without_changing_source_events() {
    for reborrow in [false, true] {
        // Synthetic component source only. No production kernel root is invented.
        let semantic = fixture::captured_source(reborrow, false, false, false);
        let root = semantic.roots()[0];
        let mir = ProductionSemanticMirOwnerV1::try_new(
            semantic,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let owner =
            ProductionSemanticSsaOwnerV1::try_new(mir, ProductionSemanticSsaLimitsV1::default())
                .unwrap();
        owner.verify_replay().unwrap();
        assert!(!owner.grants_proof_or_artifact_authority());
        let view = owner.execution_view_for_root(root).unwrap();
        let (site, local) = bound_borrow(view.body());
        let sites = execution_sites(
            owner.source_semantic(),
            owner.execution_expansion(),
            view,
            MAX_FLOW_WORK,
        )
        .unwrap();
        assert!(sites.contains(&site));
        let (classified, _, _) = semantic_function_ssa_input_v1(
            view.body(),
            Some(owner.source_semantic().types()),
            owner.source_semantic().callables(),
            &sites,
        );
        let (unclassified, _, _) = semantic_function_ssa_input_v1(
            view.body(),
            Some(owner.source_semantic().types()),
            owner.source_semantic().callables(),
            &BTreeSet::new(),
        );
        assert!(classified.promotable()[local as usize]);
        assert!(!unclassified.promotable()[local as usize]);
        assert_eq!(
            classified.blocks(),
            unclassified.blocks(),
            "Copy/Move/Kill events are unchanged"
        );
        let plan = owner.execution_plan_for_root(root).unwrap().plan();
        assert!(
            plan.promoted_variables()
                .contains(&SsaVariableIdV1::new(local))
        );
        assert!(plan.resolved_events(SsaBlockIdV1::new(site.block)).unwrap().iter().any(|(_, event)|
            matches!(event, SsaResolvedEventV1::Use { variable, .. } if variable.get() == local)));
    }
}

#[test]
fn capture_escapes_and_duplicate_assignments_poison_the_original_borrow() {
    let semantic = fixture::captured_source(false, false, false, false);
    let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    let body = expansion.root(semantic.roots()[0]).unwrap().body();
    let (site, _) = bound_borrow(body);
    assert!(classify(&semantic, &expansion, body).contains(&site));
    let block = receiver_block(body);
    let original = &body.blocks()[block];
    for duplicate in [false, true] {
        let mut statements = original.statements().to_vec();
        if duplicate {
            let carrier = statements
                .iter()
                .find(|statement| {
                    matches!(statement.kind(),
                SemanticStatementKindV1::Assign(a) if a.destination().local().index() == 13)
                })
                .unwrap()
                .clone();
            statements.push(carrier);
        } else {
            // This private-classifier mutation tests an unmodeled operand use;
            // it is not presented as an admitted boolean expression.
            statements.push(SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Assume(SemanticOperandV1::Copy(place(13, 12))),
            ));
        }
        let changed = changed_block(
            body,
            block,
            statements,
            original.terminator().kind().clone(),
        );
        assert!(
            !classify(&semantic, &expansion, &changed).contains(&site),
            "duplicate={duplicate}"
        );
    }
    let changed = changed_block(
        body,
        block,
        original.statements().to_vec(),
        SemanticTerminatorKindV1::TailCall(
            SemanticDirectTailCallV1::new_callable(
                SemanticCallableIdV1::from_index(7),
                vec![SemanticOperandV1::Copy(place(13, 12))],
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    );
    assert!(!classify(&semantic, &expansion, &changed).contains(&site));
}

#[test]
fn captured_field_access_rejects_wrong_field_type_for_copy_and_move() {
    for projected_move in [false, true] {
        let semantic = fixture::captured_source(false, false, false, projected_move);
        let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
        let body = expansion.root(semantic.roots()[0]).unwrap().body();
        let (site, _) = bound_borrow(body);
        let block = receiver_block(body);
        for (field, result) in [(1, 10), (0, 11)] {
            let mut statements = body.blocks()[block].statements().to_vec();
            let index = statements
                .iter()
                .position(|statement| {
                    matches!(statement.kind(),
                SemanticStatementKindV1::Assign(a) if a.destination().local().index() == 14)
                })
                .unwrap();
            let projected = SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(13),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty(result))
                        .unwrap(),
                ],
                ty(result),
            )
            .unwrap();
            statements[index] = assign(
                place(14, 10),
                SemanticRvalueKindV1::Use(if projected_move {
                    SemanticOperandV1::Move(projected)
                } else {
                    SemanticOperandV1::Copy(projected)
                }),
            );
            let changed = changed_block(
                body,
                block,
                statements,
                body.blocks()[block].terminator().kind().clone(),
            );
            assert!(
                !classify(&semantic, &expansion, &changed).contains(&site),
                "field={field} result={result}"
            );
        }
    }
}

#[test]
fn exact_shared_math_leaf_move_retains_real_owner_and_partial_move_checks() {
    let source = fixture::captured_source(false, false, false, true);
    let root = source.roots()[0];
    let mir = ProductionSemanticMirOwnerV1::try_new(source, Default::default()).unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(mir, Default::default()).unwrap();
    owner.verify_replay().unwrap();
    assert!(!owner.grants_proof_or_artifact_authority());
    let view = owner.execution_view_for_root(root).unwrap();
    let (site, local) = bound_borrow(view.body());
    let plan = owner.execution_plan_for_root(root).unwrap();
    assert!(plan.partial_move_certificate().projected_moves() > 0);
    assert!(
        plan.plan()
            .promoted_variables()
            .contains(&SsaVariableIdV1::new(local))
    );
    let SemanticStatementKindV1::Assign(a) =
        view.body().blocks()[site.block as usize].statements()[site.statement as usize].kind()
    else {
        panic!("original assignment")
    };
    let SemanticRvalueKindV1::Borrow { place, .. } = a.value().kind() else {
        panic!("original borrow")
    };
    owner
        .source_query_for_root(root, view.body())
        .unwrap()
        .borrow_place_use(
            ProductionSemanticSsaSourceSiteV1::new(
                SemanticBlockIdV1::from_index(site.block),
                Some(site.statement),
            ),
            place,
            &mut || true,
        )
        .unwrap();
}

#[test]
fn shared_leaf_move_recognition_is_charged_and_keeps_exact_work_boundary() {
    let source = fixture::captured_source(false, false, false, true);
    let expansion = SemanticCallExpansionV1::try_new(&source, Default::default()).unwrap();
    let body = expansion.root(source.roots()[0]).unwrap().body();
    let routes = Routes::new(body, Some(source.types()), source.callables(), &mut budget(MAX_FLOW_WORK)).unwrap();
    let assignment = body.blocks()[receiver_block(body)].statements().iter().find_map(|s| {
        match s.kind() {
            SemanticStatementKindV1::Assign(a) if a.destination().local().index() == 14 => Some(a),
            _ => None,
        }
    }).unwrap();
    let mut measured = budget(MAX_FLOW_WORK);
    assert!(routes.source(assignment, &mut measured).unwrap().is_some());
    let required = MAX_FLOW_WORK - measured.remaining;
    assert!(required > 0);
    assert!(routes.source(assignment, &mut budget(required)).unwrap().is_some());
    assert!(matches!(routes.source(assignment, &mut budget(required - 1)),
        Err(ProductionSemanticSsaErrorV1::BorrowFlowWork { .. })));
}

#[test]
fn disjoint_scalar_copy_does_not_escape_or_move_the_math_field() {
    let semantic = fixture::captured_source(false, false, false, false);
    let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    let body = expansion.root(semantic.roots()[0]).unwrap().body();
    let (site, _) = bound_borrow(body);
    let block = receiver_block(body);
    let mut statements = body.blocks()[block].statements().to_vec();
    let scalar = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(13),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), ty(11)).unwrap()],
        ty(11),
    )
    .unwrap();
    statements.push(assign(
        place(9, 11),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(scalar)),
    ));
    let changed = changed_block(
        body,
        block,
        statements,
        body.blocks()[block].terminator().kind().clone(),
    );
    assert!(classify(&semantic, &expansion, &changed).contains(&site));
}

#[test]
fn captured_routes_require_closed_consumer_and_bound_shape_work() {
    let semantic = fixture::captured_source(false, false, false, false);
    let body = &semantic.functions()[0];
    let mut measured = budget(MAX_FLOW_WORK);
    let routes = Routes::new(
        body,
        Some(semantic.types()),
        semantic.callables(),
        &mut measured,
    )
    .unwrap();
    assert_eq!(routes.owned(ty(12)), Some(&ty(9)));
    let required = MAX_FLOW_WORK - measured.remaining;
    assert!(
        Routes::new(
            body,
            Some(semantic.types()),
            semantic.callables(),
            &mut budget(required)
        )
        .is_ok()
    );
    let limit = required - 1;
    let Err(ProductionSemanticSsaErrorV1::BorrowFlowWork {
        remaining_work_units,
        requested_work_units,
        phase_work_units,
        error,
        ..
    }) = Routes::new(
        body,
        Some(semantic.types()),
        semantic.callables(),
        &mut budget(limit),
    ) else {
        panic!("one-below budget must retain its original flow failure")
    };
    assert!(requested_work_units > remaining_work_units);
    assert_eq!(phase_work_units.iter().sum::<usize>(), limit - remaining_work_units);
    assert_eq!(
        *error,
        ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            required: limit + 1,
            limit,
        }
    );
    assert!(
        Routes::new(
            body,
            Some(semantic.types()),
            &semantic.callables()[..7],
            &mut budget(MAX_FLOW_WORK)
        )
        .unwrap()
        .routes
        .is_empty()
    );
    assert!(
        Routes::new(body, None, semantic.callables(), &mut budget(MAX_FLOW_WORK))
            .unwrap()
            .routes
            .is_empty()
    );
    let mut pairs = BTreeMap::new();
    pairs.insert(ty(10), ty(9));
    let mut types = semantic.types().to_vec();
    // Private shape test: a second capability leaf cannot silently become an
    // untracked sibling. These declarations are not used as admitted MIR.
    types[12] = SemanticTypeDeclV1::new(
        types[12].identity(),
        types[12].layout_identity(),
        types[12].layout().clone(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![ty(10), ty(10)]).unwrap()),
    );
    let mut found = None;
    let mut nodes = 0;
    assert!(
        !walk(
            &types,
            &pairs,
            ty(12),
            &mut [0; MAX_FIELDS],
            0,
            &mut nodes,
            &mut found,
            &mut budget(MAX_FLOW_WORK)
        )
        .unwrap()
    );
    let mut found = None;
    let mut nodes = MAX_SHAPE_NODES;
    assert!(
        !walk(
            semantic.types(),
            &pairs,
            ty(12),
            &mut [0; MAX_FIELDS],
            0,
            &mut nodes,
            &mut found,
            &mut budget(MAX_FLOW_WORK)
        )
        .unwrap()
    );
}

#[path = "disjoint_field_use_tests.rs"]
mod disjoint_field_use_tests;

#[test]
fn capture_without_original_borrow_cannot_create_a_component_root() {
    let semantic = fixture::captured_source(false, false, false, false);
    let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    let body = expansion.root(semantic.roots()[0]).unwrap().body();
    let (site, _) = bound_borrow(body);
    let block = &body.blocks()[site.block as usize];
    let mut statements = block.statements().to_vec();
    statements[site.statement as usize] = SemanticStatementV1::new(
        statements[site.statement as usize].source(),
        SemanticStatementKindV1::Nop,
    );
    let changed = changed_block(
        body,
        site.block as usize,
        statements,
        block.terminator().kind().clone(),
    );
    let sites = classify(&semantic, &expansion, &changed);
    assert!(!sites.contains(&site));
    assert!(sites.iter().all(|site| !matches!(
        changed.blocks()[site.block as usize].statements()[site.statement as usize].kind(),
        SemanticStatementKindV1::Assign(a) if a.destination().ty() == ty(10)
    )));
}

#[test]
fn capture_route_depth_and_total_structural_nodes_are_bounded() {
    let semantic = fixture::captured_source(false, false, false, false);
    let mut types = semantic.types().to_vec();
    let pairs = BTreeMap::from([(ty(10), ty(9))]);
    let append = |types: &mut Vec<SemanticTypeDeclV1>, fields: Vec<SemanticTypeIdV1>| {
        let id = types.len() as u32;
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([id as u8; 32]),
            SemanticLayoutIdentityV1::from_sha256([id as u8; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                8,
                SemanticAggregateLayoutV1::new(vec![0; fields.len()], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
        ));
        ty(id)
    };
    let mut nested = ty(10);
    for _ in 0..MAX_FIELDS {
        nested = append(&mut types, vec![nested]);
    }
    let mut found = None;
    let mut nodes = 0;
    assert!(
        walk(
            &types,
            &pairs,
            nested,
            &mut [0; MAX_FIELDS],
            0,
            &mut nodes,
            &mut found,
            &mut budget(MAX_FLOW_WORK)
        )
        .unwrap()
    );
    assert_eq!(found.unwrap().len, MAX_FIELDS);
    nested = append(&mut types, vec![nested]);
    let mut found = None;
    let mut nodes = 0;
    assert!(
        !walk(
            &types,
            &pairs,
            nested,
            &mut [0; MAX_FIELDS],
            0,
            &mut nodes,
            &mut found,
            &mut budget(MAX_FLOW_WORK)
        )
        .unwrap()
    );
    for extra in [0, 1] {
        let mut fields = vec![ty(10)];
        fields.extend(std::iter::repeat_n(ty(0), MAX_SHAPE_NODES - 2 + extra));
        let wide = append(&mut types, fields);
        let mut found = None;
        let mut nodes = 0;
        let accepted = walk(
            &types,
            &pairs,
            wide,
            &mut [0; MAX_FIELDS],
            0,
            &mut nodes,
            &mut found,
            &mut budget(MAX_FLOW_WORK),
        )
        .unwrap();
        assert_eq!(accepted, extra == 0);
        assert_eq!(nodes, MAX_SHAPE_NODES);
    }
}
