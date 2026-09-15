use super::*;
use fixtures::{ROOT, Shape, owner, use_site};

fn incoming<'a>(
    query: &ProductionSemanticSsaSourceQueryV1<'a>,
) -> ProductionSemanticSsaIncomingValuesV1<'a> {
    let (site, operand) = use_site(query.function());
    let value = query
        .operand_use(site, operand, &mut || true)
        .unwrap()
        .retained_value();
    let ProductionSemanticSsaValueOriginV1::BlockArgument(incoming) =
        query.value_origin(&value, &mut || true).unwrap()
    else {
        panic!("merge value")
    };
    assert_ne!(
        incoming.block().get(),
        site.block().index(),
        "actual merge is not the source use block"
    );
    incoming
}

#[test]
fn scalar_source_selection_uses_both_exact_incoming_values_and_switch_polarity() {
    for shape in [
        Shape::Diamond,
        Shape::Reversed,
        Shape::MoveCondition,
        Shape::ManyNops,
    ] {
        let owner = owner(shape);
        let view = owner.execution_view_for_root(ROOT).unwrap();
        let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
        let incoming = incoming(&query);
        let selected = diamond::two_way(&query, &incoming, &mut || true).unwrap();
        assert!(selected.when_true.belongs_to(&query));
        assert!(selected.when_false.belongs_to(&query));
        assert_ne!(selected.when_true.value(), selected.when_false.value());
        let SemanticTerminatorKindV1::SwitchInt { discriminant, .. } = view.body().blocks()
            [selected.site.block().index() as usize]
            .terminator()
            .kind()
        else {
            panic!("switch")
        };
        assert!(std::ptr::eq(discriminant, selected.condition));
        let origin = |value| match query.value_origin(&value, &mut || true).unwrap() {
            ProductionSemanticSsaValueOriginV1::Event { block, .. } => block.get(),
            _ => panic!("branch-local actual definition"),
        };
        let expected = if matches!(shape, Shape::Reversed) {
            (1, 2)
        } else {
            (2, 1)
        };
        assert_eq!(
            (origin(selected.when_true), origin(selected.when_false)),
            expected
        );
    }
}

#[test]
fn scalar_source_selection_rejects_bypass_backedge_and_complete_false_edge_inventory() {
    for shape in [Shape::Bypass, Shape::Backedge, Shape::FalseEdge] {
        let owner = owner(shape);
        let view = owner.execution_view_for_root(ROOT).unwrap();
        let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
        let incoming = incoming(&query);
        assert!(
            matches!(
                diamond::two_way(&query, &incoming, &mut || true),
                Err(CLOSED)
            ),
            "{shape:?}"
        );
    }
}

#[test]
fn scalar_source_selection_owner_and_all_shared_budget_boundaries_are_exact() {
    let owner = owner(Shape::Diamond);
    let other = fixtures::owner(Shape::Diamond);
    let view = owner.execution_view_for_root(ROOT).unwrap();
    let query = owner.source_query_for_root(ROOT, view.body()).unwrap();
    let foreign = other
        .source_query_for_root(ROOT, other.execution_view_for_root(ROOT).unwrap().body())
        .unwrap();
    let incoming = incoming(&query);
    assert!(matches!(
        diamond::two_way(&foreign, &incoming, &mut || true),
        Err(CLOSED)
    ));
    let mut cost = 0;
    diamond::two_way(&query, &incoming, &mut || {
        cost += 1;
        true
    })
    .unwrap();
    for limit in 0..cost {
        let mut remaining = limit;
        let result = diamond::two_way(&query, &incoming, &mut || {
            if remaining == 0 {
                false
            } else {
                remaining -= 1;
                true
            }
        });
        assert!(matches!(
            result,
            Err(CUSTODY) | Err("scalar SSA branch correspondence exhausted its shared budget")
        ));
        assert_eq!(remaining, 0);
    }
    let mut remaining = cost;
    diamond::two_way(&query, &incoming, &mut || {
        if remaining == 0 {
            false
        } else {
            remaining -= 1;
            true
        }
    })
    .unwrap();
    assert_eq!(remaining, 0);
}
