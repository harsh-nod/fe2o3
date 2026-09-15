use super::super::super::tests as source_fixtures;
use super::*;

#[test]
fn reused_index_still_rejects_duplicate_slots_before_publication() {
    let (input, plan) = fixture(2);
    let index = ValueOriginsV1::build(&input, &plan).unwrap();
    for id in 0..index.definitions.len() {
        let mut changed = index.clone();
        assert_eq!(
            changed.define(
                SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(id as u32)),
                DefinitionOrigin::Entry { argument: 0 }
            ),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        );
        assert_eq!(changed, index);
    }
}

#[test]
fn reused_index_missing_or_duplicate_unused_definition_rejects_owner_replay() {
    for duplicate in [false, true] {
        let mut owner = source_fixtures::owner(false);
        owner.verify_replay().unwrap();
        let plan = &mut owner.source_plans[0];
        // This final local2 Define has no Use; replay must cover it anyway.
        let (id, value) = plan
            .plan
            .reverse_postorder()
            .iter()
            .flat_map(|block| plan.plan.resolved_events(*block).unwrap())
            .filter_map(|(_, event)| {
                if let SsaResolvedEventV1::Define {
                    variable,
                    value: SsaValueV1::Definition(id),
                } = event
                    && variable.get() == 2
                {
                    Some((*id, SsaValueV1::Definition(*id)))
                } else {
                    None
                }
            })
            .last()
            .unwrap();
        assert!(!plan.plan.reverse_postorder().iter().flat_map(|block|
            plan.plan.resolved_events(*block).unwrap()).any(|(_, event)|
            matches!(event, SsaResolvedEventV1::Use { value: actual, .. } if *actual == value)));
        let old = plan.value_origins.definitions[id.get() as usize];
        let replacement = if duplicate {
            plan.value_origins
                .definitions
                .iter()
                .copied()
                .find(|row| *row != old)
                .unwrap()
        } else {
            DefinitionRow::MISSING
        };
        plan.value_origins.definitions[id.get() as usize] = replacement;
        assert_eq!(
            owner.verify_replay(),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        );
        let root = SemanticFunctionIdV1::from_index(0);
        let view = owner.execution_view_for_root(root).unwrap();
        let query = owner.source_query_for_root(root, view.body()).unwrap();
        assert!(matches!(
            query.definition_origin(id, &mut || true),
            Err(QueryError::MissingDefinition)
        ));
    }
}

#[test]
fn reused_index_unreachable_definitions_remain_outside_the_origin_inventory() {
    let (input, plan) = fixture(0);
    let baseline = ValueOriginsV1::build(&input, &plan).unwrap();
    let (input, plan) = fixture(64);
    let extra = ValueOriginsV1::build(&input, &plan).unwrap();
    assert_eq!(baseline.definitions, extra.definitions);
    assert_eq!(baseline.incoming, extra.incoming);
    assert_eq!(&extra.offsets[..baseline.offsets.len()], baseline.offsets);
    assert_eq!(plan.reverse_postorder().len(), 3);
}
