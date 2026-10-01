//! Capacity controls for actual planner results, without measuring admission estimates.
use super::*;
use crate::{
    SsaArgumentV1, SsaBlockIdV1, SsaBlockInputV1, SsaConstructionInputV1, SsaEdgeInputV1,
    SsaEdgeRoleV1, SsaEventV1, SsaResolvedEventV1, SsaVariableIdV1, plan_ssa_v1,
};

fn input() -> SsaConstructionInputV1 {
    let a = SsaVariableIdV1::new(0);
    let b = SsaVariableIdV1::new(1);
    SsaConstructionInputV1::new(
        SsaBlockIdV1::new(0),
        2,
        vec![true, true],
        vec![a],
        vec![
            SsaBlockInputV1::new(
                vec![SsaEventV1::Use(a), SsaEventV1::Define(b)],
                vec![SsaEdgeInputV1::new(
                    SsaEdgeRoleV1::new(1),
                    SsaBlockIdV1::new(1),
                    vec![],
                )],
            ),
            SsaBlockInputV1::new(vec![SsaEventV1::Use(b)], vec![]),
        ],
    )
}

fn spare<T>(values: &mut Vec<T>) {
    values.reserve_exact(7);
}
fn spare_rows<T>(values: &mut Vec<Vec<T>>) {
    spare(values);
    for row in values {
        spare(row);
    }
}
fn spare_edges<T>(values: &mut Vec<Vec<Vec<T>>>) {
    spare(values);
    for rows in values {
        spare_rows(rows);
    }
}
fn plan() -> SsaConstructionPlanV1 {
    let mut plan = plan_ssa_v1(&input()).unwrap();
    spare(&mut plan.reachable);
    spare(&mut plan.reverse_postorder);
    spare(&mut plan.promoted_variables);
    spare_rows(&mut plan.live_in);
    spare_rows(&mut plan.merge_variables);
    spare_rows(&mut plan.transport_variables);
    spare(&mut plan.entry_definitions);
    spare(&mut plan.entry_arguments);
    spare_rows(&mut plan.resolved_events);
    spare_edges(&mut plan.edge_definitions);
    spare_edges(&mut plan.edge_arguments);
    plan
}
fn expected(plan: &SsaConstructionPlanV1) -> (usize, usize) {
    // Independent direct-field formula, not visitor/helper output.
    macro_rules! capacity {
        ($field:ident, $ty:ty) => {
            plan.$field.capacity() * size_of::<$ty>()
        };
    }
    macro_rules! rows {
        ($field:ident, $ty:ty) => {
            capacity!($field, Vec<$ty>)
                + plan
                    .$field
                    .iter()
                    .map(|row| row.capacity() * size_of::<$ty>())
                    .sum::<usize>()
        };
    }
    macro_rules! edges {
        ($field:ident) => {
            capacity!($field, Vec<Vec<SsaArgumentV1>>)
                + plan
                    .$field
                    .iter()
                    .map(|block| {
                        block.capacity() * size_of::<Vec<SsaArgumentV1>>()
                            + block
                                .iter()
                                .map(|row| row.capacity() * size_of::<SsaArgumentV1>())
                                .sum::<usize>()
                    })
                    .sum::<usize>()
        };
    }
    let bytes = capacity!(reachable, bool)
        + capacity!(reverse_postorder, SsaBlockIdV1)
        + capacity!(promoted_variables, SsaVariableIdV1)
        + rows!(live_in, SsaVariableIdV1)
        + rows!(merge_variables, SsaVariableIdV1)
        + rows!(transport_variables, SsaVariableIdV1)
        + capacity!(entry_definitions, SsaArgumentV1)
        + capacity!(entry_arguments, SsaArgumentV1)
        + rows!(resolved_events, (u32, SsaResolvedEventV1))
        + edges!(edge_definitions)
        + edges!(edge_arguments);
    let items = 1
        + 11
        + plan.live_in.len()
        + plan.merge_variables.len()
        + plan.transport_variables.len()
        + plan.resolved_events.len()
        + plan.edge_definitions.len()
        + plan.edge_arguments.len()
        + plan.edge_definitions.iter().map(Vec::len).sum::<usize>()
        + plan.edge_arguments.iter().map(Vec::len).sum::<usize>();
    (bytes, items)
}
#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    Arithmetic,
    Bytes,
    Items,
}
fn charge(
    state: &mut (usize, usize),
    count: usize,
    width: usize,
    max_bytes: usize,
    max_items: usize,
) -> Result<(), Refusal> {
    let extent = count.checked_mul(width).ok_or(Refusal::Arithmetic)?;
    let bytes = state.0.checked_add(extent).ok_or(Refusal::Arithmetic)?;
    let items = state.1.checked_add(1).ok_or(Refusal::Arithmetic)?;
    if bytes > max_bytes {
        return Err(Refusal::Bytes);
    }
    if items > max_items {
        return Err(Refusal::Items);
    }
    *state = (bytes, items);
    Ok(())
}

#[test]
fn nonempty_plan_spare_capacities_match_independent_formula_without_mutation() {
    let plan = plan();
    let before = plan.clone();
    let expected = expected(&plan);
    assert!(plan.reachable.capacity() > plan.reachable.len());
    assert!(plan.edge_arguments[0].capacity() > plan.edge_arguments[0].len());
    assert!(plan.edge_arguments[0][0].capacity() > plan.edge_arguments[0][0].len());
    let mut state = (0, 0);
    plan.visit_logical_retained_heap_v1(&mut |n, w| {
        charge(&mut state, n, w, expected.0, expected.1)
    })
    .unwrap();
    assert_eq!(state, expected);
    assert_eq!(plan, before);
    plan.verify_replay(&input(), crate::SsaPlannerLimitsV1::default())
        .unwrap();
}

#[test]
fn visitor_stops_at_every_first_refusal_without_later_collection_visits() {
    let plan = plan();
    for refused in 1..=expected(&plan).1 {
        let mut calls = 0;
        let result = plan.visit_logical_retained_heap_v1(&mut |_, _| {
            calls += 1;
            if calls == refused {
                Err(refused)
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Err(refused));
        assert_eq!(calls, refused);
    }
}

#[test]
fn exact_and_short_limits_distinguish_complete_from_partial_observations() {
    let plan = plan();
    let (bytes, items) = expected(&plan);
    for (max_bytes, max_items, refusal) in [
        (bytes - 1, items, Refusal::Bytes),
        (bytes, items - 1, Refusal::Items),
        (bytes, 0, Refusal::Items),
    ] {
        let mut state = (0, 0);
        assert_eq!(
            plan.visit_logical_retained_heap_v1(&mut |n, w| {
                charge(&mut state, n, w, max_bytes, max_items)
            }),
            Err(refusal)
        );
        assert_ne!(state, (bytes, items));
    }
}

#[test]
fn empty_allocations_still_visit_all_eleven_collections_and_no_header() {
    let mut plan = plan_ssa_v1(&input()).unwrap();
    // Private storage-only mutation, not a valid reconstructed planner result.
    plan.reachable = Vec::new();
    plan.reverse_postorder = Vec::new();
    plan.promoted_variables = Vec::new();
    plan.live_in = Vec::new();
    plan.merge_variables = Vec::new();
    plan.transport_variables = Vec::new();
    plan.entry_definitions = Vec::new();
    plan.entry_arguments = Vec::new();
    plan.resolved_events = Vec::new();
    plan.edge_definitions = Vec::new();
    plan.edge_arguments = Vec::new();
    let mut state = (0, 0);
    plan.visit_logical_retained_heap_v1(&mut |n, w| charge(&mut state, n, w, 0, 12))
        .unwrap();
    assert_eq!(state, (0, 12));
}

#[test]
fn caller_checked_product_addition_and_item_overflow_do_not_commit() {
    let mut state = (0, 0);
    assert_eq!(
        charge(&mut state, usize::MAX, 2, usize::MAX, usize::MAX),
        Err(Refusal::Arithmetic)
    );
    assert_eq!(state, (0, 0));
    state = (usize::MAX, 0);
    assert_eq!(
        charge(&mut state, 1, 1, usize::MAX, usize::MAX),
        Err(Refusal::Arithmetic)
    );
    assert_eq!(state, (usize::MAX, 0));
    state = (0, usize::MAX);
    let plan = plan();
    assert_eq!(
        plan.visit_logical_retained_heap_v1(&mut |n, w| {
            charge(&mut state, n, w, usize::MAX, usize::MAX)
        }),
        Err(Refusal::Arithmetic)
    );
    assert_eq!(state, (0, usize::MAX));
}
