use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use crate::ProductionSemanticMirLimitsV1;

mod value_origins;
mod definition_lookup;
mod resolved_events;
#[path = "operand_source_tests.rs"]
mod operand_source_tests;

mod borrow_place_boundaries {
    use super::*;
    include!("borrow_place_tests.rs");
}

const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);

fn place(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], U32).unwrap()
}
fn copy(local: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local))
}
fn constant(bits: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 4).unwrap()),
    ))
}
fn assignment(local: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local),
            SemanticRvalueV1::new(U32, value),
        )),
    )
}
fn assign(local: u32, value: SemanticOperandV1) -> SemanticStatementV1 {
    assignment(local, SemanticRvalueKindV1::Use(value))
}
fn block(statements: Vec<SemanticStatementV1>) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([210; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Return,
        ),
    )
    .unwrap()
}
fn source(expanded: bool) -> AdmittedInertSemanticMirV1 {
    source_with_statements(
        expanded,
        vec![
            assign(1, constant(7)),
            assign(2, copy(1)),
            assign(1, constant(9)),
            assign(2, SemanticOperandV1::Move(place(1))),
            assign(1, constant(11)),
            assignment(
                2,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::BitAnd,
                    left: copy(1),
                    right: copy(1),
                },
            ),
            SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Nop,
            ),
        ],
    )
}
fn source_with_statements(
    expanded: bool,
    statements: Vec<SemanticStatementV1>,
) -> AdmittedInertSemanticMirV1 {
    let seed = super::super::tests::admitted_helper_semantic();
    let mut types = seed.types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([211; 32]),
        SemanticLayoutIdentityV1::from_sha256([212; 32]),
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
    let original = &seed.functions()[usize::from(expanded)];
    let mut locals = original.locals().to_vec();
    for tag in [213, 214] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            U32,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    let mut function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![block(statements)],
    )
    .unwrap();
    let functions = if expanded {
        vec![seed.functions()[0].clone(), function]
    } else {
        function = function.with_kernel_entry(original.kernel_entry().unwrap().clone());
        vec![function]
    };
    InertSemanticMirRequestV1::new(
        seed.target().clone(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}
pub(super) fn owner(expanded: bool) -> ProductionSemanticSsaOwnerV1 {
    make_owner(source(expanded), ProductionSemanticSsaLimitsV1::default()).unwrap()
}
fn make_owner(
    source: AdmittedInertSemanticMirV1,
    limits: ProductionSemanticSsaLimitsV1,
) -> Result<ProductionSemanticSsaOwnerV1, ProductionSemanticSsaErrorV1> {
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        limits,
    )
}
fn charger(remaining: &mut usize) -> impl FnMut() -> bool + '_ {
    || match remaining.checked_sub(1) {
        Some(next) => {
            *remaining = next;
            true
        }
        None => false,
    }
}
fn source_site(view: &SemanticExpandedRootV1, source_function: u32, statement: u32) -> Site {
    for (block, origin) in view.block_origins().iter().enumerate() {
        if origin.function().index() != source_function {
            continue;
        }
        for (index, source) in origin.statements().iter().enumerate() {
            if *source == (SemanticExpandedStatementOriginV1::Source { statement }) {
                return Site::new(
                    SemanticBlockIdV1::from_index(block as u32),
                    Some(index as u32),
                );
            }
        }
    }
    panic!("source statement missing from checked expansion")
}
fn operand(function: &SemanticFunctionDeclV1, site: Site) -> &SemanticOperandV1 {
    let SemanticStatementKindV1::Assign(assign) = function.blocks()[site.block.index() as usize]
        .statements()[site.statement.unwrap() as usize]
        .kind()
    else {
        panic!("assignment")
    };
    match assign.value().kind() {
        SemanticRvalueKindV1::Use(operand) => operand,
        SemanticRvalueKindV1::Binary { left, .. } => left,
        _ => panic!("operand"),
    }
}

#[test]
fn retained_source_use_source_fallback_and_expanded_definition_versions_are_exact() {
    for expanded in [false, true] {
        let owner = owner(expanded);
        owner.verify_replay().unwrap();
        let view = owner.execution_view_for_root(ROOT).unwrap();
        assert_eq!(view.has_expanded_calls(), expanded);
        let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
        let mut budget = 4096;
        let mut values = Vec::new();
        for statement in [1, 3, 5] {
            let site = source_site(view, u32::from(expanded), statement);
            let use_ = query
                .operand_use(site, operand(view.body(), site), &mut charger(&mut budget))
                .unwrap();
            assert!(use_.belongs_to(&query));
            assert_eq!(use_.site(), site);
            assert_eq!(use_.agreeing_uses(), if statement == 5 { 2 } else { 1 });
            for (event, _) in query
                .plan()
                .plan()
                .resolved_events(SsaBlockIdV1::new(site.block.index()))
                .unwrap()
            {
                if use_.event_range().contains(&(*event as usize)) {
                    assert_eq!(
                        query
                            .event_site(
                                SsaBlockIdV1::new(site.block.index()),
                                *event,
                                &mut charger(&mut budget)
                            )
                            .unwrap(),
                        site
                    );
                }
            }
            values.push(use_.value());
        }
        assert_ne!(values[0], values[1]);
        assert_ne!(values[1], values[2]);
        assert!(query.plan().event_origins.block_ends(0).is_some());
        if !expanded {
            assert!(std::ptr::eq(
                query.plan(),
                owner.plan_for_function(ROOT).unwrap()
            ));
        }
    }
}

#[test]
fn retained_source_use_rejects_other_root_owner_body_and_cloned_occurrence() {
    let owner = owner(false);
    let other = self::owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let cloned = view.body().clone();
    assert!(matches!(
        owner.source_query_for_root(ROOT, &cloned),
        Err(QueryError::WrongOwner)
    ));
    assert!(matches!(
        owner.source_query_for_root(SemanticFunctionIdV1::from_index(1), view.body()),
        Err(QueryError::WrongOwner)
    ));
    assert!(matches!(
        other.source_query_for_root(ROOT, view.body()),
        Err(QueryError::WrongOwner)
    ));
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let other_view = other.execution_view_for_root(ROOT).unwrap();
    let other_query = other
        .source_query_for_root(ROOT, other_view.body())
        .unwrap();
    let site = source_site(view, 0, 1);
    let mut budget = 4096;
    let use_ = query
        .operand_use(site, operand(view.body(), site), &mut charger(&mut budget))
        .unwrap();
    assert!(!use_.belongs_to(&other_query));
    assert!(matches!(
        query.operand_use(site, operand(&cloned, site), &mut charger(&mut budget)),
        Err(QueryError::OperandOutsideSite)
    ));
    let wrong_site = source_site(view, 0, 3);
    assert!(matches!(
        query.operand_use(
            wrong_site,
            operand(view.body(), site),
            &mut charger(&mut budget)
        ),
        Err(QueryError::OperandOutsideSite)
    ));
}

#[test]
fn retained_source_use_missing_empty_and_foreign_events_do_not_guess_origins() {
    let owner = owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let mut budget = 4096;
    let empty = source_site(view, 0, 6);
    assert_eq!(
        query.site_range(empty, &mut charger(&mut budget)),
        Err(QueryError::NoPromotedUse)
    );
    assert_eq!(
        query.event_site(SsaBlockIdV1::new(999), 0, &mut charger(&mut budget)),
        Err(QueryError::InvalidSite)
    );
    assert_eq!(
        query.event_site(SsaBlockIdV1::new(0), u32::MAX, &mut charger(&mut budget)),
        Err(QueryError::MissingEventOrigin)
    );
    assert!(matches!(
        query.operand_use(
            Site::new(SemanticBlockIdV1::from_index(0), Some(999)),
            operand(view.body(), source_site(view, 0, 1)),
            &mut charger(&mut budget)
        ),
        Err(QueryError::InvalidSite)
    ));
}

#[test]
fn retained_source_use_shared_caller_budget_is_not_reset_or_refunded() {
    let owner = owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let site = source_site(view, 0, 5);
    let mut broad = 4096;
    query
        .operand_use(site, operand(view.body(), site), &mut charger(&mut broad))
        .unwrap();
    let cost = 4096 - broad;
    for limit in [0, 1, cost - 1] {
        let mut budget = limit;
        assert!(matches!(
            query.operand_use(site, operand(view.body(), site), &mut charger(&mut budget)),
            Err(QueryError::WorkLimit)
        ));
        assert_eq!(budget, 0);
        assert!(matches!(
            query.operand_use(site, operand(view.body(), site), &mut charger(&mut budget)),
            Err(QueryError::WorkLimit)
        ));
    }
    let mut budget = cost;
    query
        .operand_use(site, operand(view.body(), site), &mut charger(&mut budget))
        .unwrap();
    assert_eq!(budget, 0);
    assert!(matches!(
        query.operand_use(site, operand(view.body(), site), &mut charger(&mut budget)),
        Err(QueryError::WorkLimit)
    ));
}

#[test]
fn retained_source_use_source_roster_mutation_changes_identity_and_replay() {
    let mut owner = owner(false);
    let before = owner.source_identity;
    owner.source_plans[0]
        .event_origins
        .record(0, Some(99), 0..99);
    let after = derive_semantic_ssa_identity_v1(
        &owner.source_semantic_sha256,
        &owner.source_plans,
        owner.source_summary,
    );
    assert_ne!(before, after);
    assert_eq!(
        owner.verify_replay(),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    );
}

#[test]
fn retained_source_use_retained_and_replay_rows_are_both_accounted() {
    let owner = owner(true);
    for plan in owner.plans().iter().chain(std::iter::once(
        owner.execution_plan_for_root(ROOT).unwrap(),
    )) {
        let blocks = plan.plan().resources().input_blocks();
        let sites: usize = (0..blocks).map(|block| {
            plan.event_origins.block_ends(block as u32).unwrap().len()
        }).sum();
        let resources = plan.event_origins.resources().unwrap();
        let word = std::mem::size_of::<usize>();
        let headers = 2 * std::mem::size_of::<execution::ExecutionEventOriginsV1>().div_ceil(word);
        assert_eq!(
            resources.storage_words,
            2 * (((blocks + 1) * 4).div_ceil(word) + (sites * 4).div_ceil(word)) + headers
        );
        assert_eq!(resources.work_units, 12 * (blocks + sites + 1));
        assert!(plan.auxiliary_resources.storage_words >= resources.storage_words);
        assert!(plan.auxiliary_resources.work_units >= resources.work_units);
    }
    let resources = owner.summary();
    let broad = ProductionSemanticSsaModuleLimitsV1::production();
    for (limit, accepted) in [
        (resources.storage_words(), true),
        (resources.storage_words() - 1, false),
    ] {
        let module = ProductionSemanticSsaModuleLimitsV1::try_new(
            broad.max_variables(),
            broad.max_blocks(),
            broad.max_edges(),
            broad.max_events(),
            broad.max_edge_definitions(),
            broad.max_output_items(),
            limit,
            broad.max_work_units(),
        )
        .unwrap();
        let result = make_owner(
            source(true),
            ProductionSemanticSsaLimitsV1::with_module_limits(
                SsaPlannerLimitsV1::default(),
                module,
            ),
        );
        if accepted {
            result.unwrap().verify_replay().unwrap();
        } else {
            assert!(
                matches!(result,Err(ProductionSemanticSsaErrorV1::ExpandedExecution { error,.. }) if matches!(*error,ProductionSemanticSsaErrorV1::AggregateResourceLimit {resource:SsaPlannerResourceV1::StorageWords,required,limit:observed} if required==resources.storage_words() && observed==limit))
            );
        }
    }
}

fn with_blocks(blocks: Vec<SemanticBasicBlockV1>) -> AdmittedInertSemanticMirV1 {
    let seed = source(false);
    let original = &seed.functions()[0];
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    InertSemanticMirRequestV1::new(
        seed.target().clone(),
        seed.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}
fn cfg_block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    term: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), term),
    )
    .unwrap()
}
fn go(block: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::Goto,
        SemanticBlockIdV1::from_index(block),
    ))
}
fn switch(first: u32, second: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: constant(0),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    SemanticBlockIdV1::from_index(first),
                ),
            )],
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::SwitchOtherwise,
                SemanticBlockIdV1::from_index(second),
            ),
        )
        .unwrap(),
    }
}

#[test]
fn retained_source_use_joins_and_backedges_remain_actual_block_arguments() {
    for backedge in [false, true] {
        let blocks = if backedge {
            vec![
                cfg_block(230, vec![assign(1, constant(7))], go(1)),
                cfg_block(
                    231,
                    vec![assign(2, copy(1)), assign(1, constant(9))],
                    switch(1, 2),
                ),
                cfg_block(232, vec![], SemanticTerminatorKindV1::Return),
            ]
        } else {
            vec![
                cfg_block(230, vec![], switch(1, 2)),
                cfg_block(231, vec![assign(1, constant(7))], go(3)),
                cfg_block(232, vec![assign(1, constant(9))], go(3)),
                cfg_block(
                    233,
                    vec![assign(2, copy(1))],
                    SemanticTerminatorKindV1::Return,
                ),
            ]
        };
        let owner = make_owner(
            with_blocks(blocks),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        owner.verify_replay().unwrap();
        let view = owner.execution_view_for_root(ROOT).unwrap();
        let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
        let (block,statement)=view.body().blocks().iter().enumerate().find_map(|(block,body)| {
            body.statements().iter().enumerate().find_map(|(statement,value)|
                matches!(value.kind(),SemanticStatementKindV1::Assign(value) if value.destination().local().index()==2)
                    .then_some((block,statement)))
        }).unwrap();
        let site = Site::new(
            SemanticBlockIdV1::from_index(block as u32),
            Some(statement as u32),
        );
        let mut budget = 4096;
        let value = query
            .operand_use(site, operand(view.body(), site), &mut charger(&mut budget))
            .unwrap();
        assert_eq!(
            value.value(),
            SsaValueV1::BlockArgument {
                block: SsaBlockIdV1::new(block as u32),
                variable: SsaVariableIdV1::new(1)
            }
        );
        assert!(!owner.grants_proof_or_artifact_authority());
    }
}

#[test]
fn retained_source_use_dominating_merge_owner_is_not_relabelled_as_the_use_block() {
    let source = with_blocks(vec![
        cfg_block(230, vec![], switch(1, 2)),
        cfg_block(231, vec![assign(1, constant(7))], go(3)),
        cfg_block(232, vec![assign(1, constant(9))], go(3)),
        cfg_block(233, vec![], go(4)),
        cfg_block(
            234,
            vec![assign(2, copy(1))],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    let owner = make_owner(source, ProductionSemanticSsaLimitsV1::default()).unwrap();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let (use_block,statement)=view.body().blocks().iter().enumerate().find_map(|(block,body)| {
        body.statements().iter().enumerate().find_map(|(statement,value)|
            matches!(value.kind(),SemanticStatementKindV1::Assign(value) if value.destination().local().index()==2)
                .then_some((block,statement)))
    }).unwrap();
    let site = Site::new(
        SemanticBlockIdV1::from_index(use_block as u32),
        Some(statement as u32),
    );
    let mut budget = 4096;
    let value = query
        .operand_use(site, operand(view.body(), site), &mut charger(&mut budget))
        .unwrap();
    let SsaValueV1::BlockArgument { block, variable } = value.value() else {
        panic!("actual merge value")
    };
    assert_ne!(block.get(), site.block.index());
    assert_eq!(variable, SsaVariableIdV1::new(1));
    assert!(
        query
            .plan()
            .plan()
            .merge_variables(block)
            .unwrap()
            .contains(&variable)
    );
    assert_eq!(value.site(), site);
}

#[test]
fn retained_source_use_source_kill_has_no_promoted_use_and_bypass_still_rejects() {
    let killed = source_with_statements(
        false,
        vec![
            assign(1, constant(7)),
            SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Deinitialize(place(1)),
            ),
            assign(2, copy(1)),
        ],
    );
    let bypass = with_blocks(vec![
        cfg_block(230, vec![], switch(1, 2)),
        cfg_block(231, vec![assign(1, constant(7))], go(3)),
        cfg_block(232, vec![], go(3)),
        cfg_block(
            233,
            vec![assign(2, copy(1))],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    let owner = make_owner(killed, ProductionSemanticSsaLimitsV1::default()).unwrap();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let site = source_site(view, 0, 2);
    let mut budget = 4096;
    assert!(
        !query
            .plan()
            .plan()
            .promoted_variables()
            .contains(&SsaVariableIdV1::new(1))
    );
    assert!(matches!(
        query.operand_use(site, operand(view.body(), site), &mut charger(&mut budget)),
        Err(QueryError::NoPromotedUse)
    ));
    let result = make_owner(bypass, ProductionSemanticSsaLimitsV1::default());
    assert!(
        matches!(result,Err(ProductionSemanticSsaErrorV1::Planner {
        error:SsaPlannerErrorV1::UndefinedAtUse {variable,..}
            | SsaPlannerErrorV1::UndefinedAtEdge {variable,..},..
    }) if variable.get()==1),
        "{result:?}"
    );
}

#[test]
fn retained_source_use_rejects_constants_mismatched_types_and_projected_places() {
    let owner = owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let site = source_site(view, 0, 1);
    let mut budget = 4096;
    let wrong_type = SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![],
            SemanticTypeIdV1::from_index(0),
        )
        .unwrap(),
    );
    let projected = SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Subtype, U32).unwrap()],
            U32,
        )
        .unwrap(),
    );
    for operand in [constant(7), wrong_type, projected] {
        assert!(matches!(
            query.operand_use(site, &operand, &mut charger(&mut budget)),
            Err(QueryError::UnsupportedOperand)
        ));
    }
    // A retained body's equal-looking operand from a different occurrence cannot
    // be converted into a use token even when local and declared type agree.
    let copy = copy(1);
    assert!(matches!(
        query.operand_use(site, &copy, &mut charger(&mut budget)),
        Err(QueryError::OperandOutsideSite)
    ));
}

#[test]
fn retained_source_use_promoted_storage_dead_and_move_kills_remain_undefined() {
    for kill in [
        SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
        ),
        assign(2, SemanticOperandV1::Move(place(1))),
    ] {
        let source = source_with_statements(
            false,
            vec![assign(1, constant(7)), kill, assign(2, copy(1))],
        );
        let result = make_owner(source, ProductionSemanticSsaLimitsV1::default());
        assert!(
            matches!(result,Err(ProductionSemanticSsaErrorV1::Planner {
            error:SsaPlannerErrorV1::UndefinedAtUse {variable,..},..
        }) if variable.get()==1),
            "{result:?}"
        );
    }
}
