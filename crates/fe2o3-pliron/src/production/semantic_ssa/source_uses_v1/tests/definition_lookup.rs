use super::*;
use fe2o3_mir_model::SsaDefinitionIdV1;

fn same_origin(
    a: ProductionSemanticSsaValueOriginV1<'_>,
    b: ProductionSemanticSsaValueOriginV1<'_>,
) {
    use ProductionSemanticSsaValueOriginV1 as Origin;
    match (a, b) {
        (Origin::Entry { argument: a }, Origin::Entry { argument: b }) => assert_eq!(a, b),
        (
            Origin::Event {
                block: a,
                event: ae,
                site: as_,
            },
            Origin::Event {
                block: b,
                event: be,
                site: bs,
            },
        ) => assert_eq!((a, ae, as_), (b, be, bs)),
        (
            Origin::Edge {
                edge: a,
                definition: ad,
            },
            Origin::Edge {
                edge: b,
                definition: bd,
            },
        ) => assert_eq!((a, ad), (b, bd)),
        _ => panic!("definition origin kind changed"),
    }
}

#[test]
fn inert_definition_lookup_reuses_retained_token_validation_and_work() {
    for expanded in [false, true] {
        let owner = owner(expanded);
        owner.verify_replay().unwrap();
        let view = owner.execution_view_for_root(ROOT).unwrap();
        let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
        for statement in [1, 3, 5] {
            let site = source_site(view, u32::from(expanded), statement);
            let token = query
                .operand_use(site, operand(view.body(), site), &mut || true)
                .unwrap()
                .retained_value();
            let SsaValueV1::Definition(id) = token.value() else {
                panic!("definition");
            };
            let mut inert_work = 4096;
            let (variable, inert) = query
                .definition_origin(id, &mut charger(&mut inert_work))
                .unwrap();
            let mut token_work = 4096;
            let retained = query
                .value_origin(&token, &mut charger(&mut token_work))
                .unwrap();
            assert_eq!(variable, token.variable());
            assert_eq!(inert_work, token_work);
            same_origin(inert, retained);
        }
    }
}

#[test]
fn inert_definition_lookup_preserves_exact_budget_and_out_of_range_rejection() {
    let owner = owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let site = source_site(view, 0, 1);
    let token = query
        .operand_use(site, operand(view.body(), site), &mut || true)
        .unwrap();
    let SsaValueV1::Definition(id) = token.value() else {
        panic!("definition");
    };
    let mut remaining = 4096;
    query
        .definition_origin(id, &mut charger(&mut remaining))
        .unwrap();
    let cost = 4096 - remaining;
    for limit in 0..cost {
        let mut work = limit;
        assert!(matches!(
            query.definition_origin(id, &mut charger(&mut work)),
            Err(QueryError::WorkLimit)
        ));
        assert_eq!(work, 0);
    }
    let mut work = cost;
    query
        .definition_origin(id, &mut charger(&mut work))
        .unwrap();
    assert_eq!(work, 0);
    assert!(matches!(
        query.definition_origin(SsaDefinitionIdV1::new(u32::MAX), &mut || true),
        Err(QueryError::MissingDefinition)
    ));
    assert!(matches!(
        query.definition_origin(SsaDefinitionIdV1::new(u32::MAX), &mut || false),
        Err(QueryError::WorkLimit)
    ));
}

#[test]
fn inert_lookup_does_not_relax_foreign_value_or_cloned_body_token_boundary() {
    let owner = owner(false);
    let other = self::owner(false);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let foreign = other
        .source_query_for_root(ROOT, other.execution_view_for_root(ROOT).unwrap().body())
        .unwrap();
    let site = source_site(view, 0, 1);
    let token = query
        .operand_use(site, operand(view.body(), site), &mut || true)
        .unwrap()
        .retained_value();
    assert!(matches!(
        foreign.value_origin(&token, &mut || true),
        Err(QueryError::WrongOwner)
    ));
    assert!(matches!(
        owner.source_query_for_root(ROOT, &view.body().clone()),
        Err(QueryError::WrongOwner)
    ));
    let SsaValueV1::Definition(id) = token.value() else {
        panic!("definition");
    };
    // An inert row in the foreign plan does not turn the original token into
    // a member of that plan, even when the numeric definition ID is equal.
    foreign.definition_origin(id, &mut || true).unwrap();
    assert!(!token.belongs_to(&foreign));
    assert!(token.belongs_to(&query));
}

#[test]
fn inert_entry_and_normal_call_return_origins_match_retained_tokens_and_charges() {
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        vec![SemanticOperandV1::Move(place(1))],
        Some(SemanticCallDestinationV1::new(
            place(2),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let owner = make_owner(
        value_origins::parameter_source(
            vec![
                cfg_block(220, vec![], SemanticTerminatorKindV1::Call(call)),
                cfg_block(
                    221,
                    vec![assign(1, copy(2))],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
            true,
            true,
        ),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let SemanticTerminatorKindV1::Call(call) = view.body().blocks()[0].terminator().kind() else {
        panic!("call");
    };
    let entry = query
        .operand_use(
            Site::new(SemanticBlockIdV1::from_index(0), None),
            &call.arguments()[0],
            &mut || true,
        )
        .unwrap()
        .retained_value();
    let site = Site::new(SemanticBlockIdV1::from_index(1), Some(0));
    let returned = query
        .operand_use(site, operand(view.body(), site), &mut || true)
        .unwrap()
        .retained_value();
    assert!(matches!(
        query.value_origin(&entry, &mut || true).unwrap(),
        ProductionSemanticSsaValueOriginV1::Entry { argument: 0 }
    ));
    assert!(
        matches!(query.value_origin(&returned, &mut || true).unwrap(), ProductionSemanticSsaValueOriginV1::Edge { edge, definition: 0 } if edge.id().ordinal() == 0)
    );
    for token in [entry, returned] {
        let SsaValueV1::Definition(id) = token.value() else {
            panic!("definition");
        };
        let mut a = 4096;
        let mut b = 4096;
        let (variable, inert) = query.definition_origin(id, &mut charger(&mut a)).unwrap();
        let retained = query.value_origin(&token, &mut charger(&mut b)).unwrap();
        assert_eq!(variable, token.variable());
        assert_eq!(a, b);
        same_origin(inert, retained);
    }
}
