use crate::production_ranked_projection_v1::tests::private_capture_tests::{
    ExclusiveAggregateChange, exclusive_aggregate_owner,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1;

mod cfg_tests {
    use super::*;
    include!("exclusive_aggregate_cfg_tests.rs");
}

fn before_exclusive_read<'a>(analysis: &mut Analysis<'a>) -> (&'a SemanticPlaceV1, Flow) {
    let body = analysis.function();
    let block = body.entry().index() as usize;
    let mut flow = Flow::default();
    for (statement, value) in body.blocks()[block].statements().iter().enumerate() {
        let site = Site { block, statement };
        assert!(analysis.checked_site(site));
        if let Some(place) = analysis.aggregate_candidate(value) {
            let reference = analysis
                .aggregate_read_reference(place, &flow)
                .unwrap()
                .unwrap();
            assert!(reference.mutable);
            return (place, flow);
        }
        let SemanticStatementKindV1::Assign(assignment) = value.kind() else {
            panic!("retained fixture prefix must consist of source assignments");
        };
        analysis.assignment(site, assignment, &mut flow).unwrap();
    }
    panic!("missing original exclusive private aggregate read")
}

fn reference(flow: &Flow, place: &SemanticPlaceV1) -> Reference {
    let Value::Reference(reference) = flow.values[&place.local().index()] else {
        panic!()
    };
    reference
}

#[test]
fn private_exclusive_aggregate_flow_requires_selected_initialized_fields_not_opaque() {
    let owner = exclusive_aggregate_owner(ExclusiveAggregateChange::None);
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut analysis = Analysis::new(owner.source_semantic().types(), view);
    let (place, initial) = before_exclusive_read(&mut analysis);
    let reference = reference(&initial, place);
    for change in 0..5 {
        let mut flow = initial.clone();
        match change {
            0 => {
                flow.values.insert(reference.target, Value::Opaque);
            }
            1 => {
                flow.values.remove(&reference.target);
            }
            _ => {
                flow.values.edit(reference.target, |value| {
                    let Value::Fields(fields) = value else {
                        panic!()
                    };
                    match change {
                        2 => fields[0] = None,
                        3 => fields[0] = Some(Value::Reference(reference)),
                        4 => {
                            fields.pop();
                        }
                        _ => unreachable!(),
                    }
                });
            }
        }
        assert!(
            analysis
                .aggregate_read_reference(place, &flow)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn private_exclusive_aggregate_flow_requires_original_borrow_mutability_target_and_site() {
    let owner = exclusive_aggregate_owner(ExclusiveAggregateChange::None);
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut analysis = Analysis::new(owner.source_semantic().types(), view);
    let (place, initial) = before_exclusive_read(&mut analysis);
    let original = reference(&initial, place);
    for change in 0..5 {
        let mut flow = initial.clone();
        let mut changed = original;
        match change {
            0 => changed.mutable = false,
            1 => changed.borrow.statement += 1,
            2 => changed.borrow.block = view.body().blocks().len(),
            3 => changed.target = place.local().index(),
            4 => {
                // Same type and initialized value are insufficient without the
                // matching original borrower of that particular local.
                let target = view
                    .body()
                    .locals()
                    .iter()
                    .enumerate()
                    .find(|(index, local)| {
                        *index != original.target as usize
                            && local.ty() == place.projections()[0].result_type()
                    })
                    .unwrap()
                    .0 as u32;
                flow.values
                    .insert(target, initial.values[&original.target].clone());
                changed.target = target;
            }
            _ => unreachable!(),
        }
        flow.values
            .insert(place.local().index(), Value::Reference(changed));
        assert!(
            analysis
                .aggregate_read_reference(place, &flow)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn private_exclusive_aggregate_flow_exact_projection_and_full_type_path() {
    let owner = exclusive_aggregate_owner(ExclusiveAggregateChange::None);
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut analysis = Analysis::new(owner.source_semantic().types(), view);
    let (place, initial) = before_exclusive_read(&mut analysis);
    let root = place.projections()[0].result_type();
    for (kind, result) in [
        (SemanticProjectionKindV1::Field(4), place.ty()),
        (SemanticProjectionKindV1::Field(0), root),
        (SemanticProjectionKindV1::Dereference, place.ty()),
        (SemanticProjectionKindV1::Downcast(0), place.ty()),
    ] {
        let changed = SemanticPlaceV1::new(
            place.local(),
            vec![
                place.projections()[0].clone(),
                SemanticProjectionV1::new(kind, result).unwrap(),
            ],
            result,
        )
        .unwrap();
        assert!(
            analysis
                .aggregate_read_reference(&changed, &initial)
                .unwrap()
                .is_none()
        );
    }
    let mut nested = place.projections().to_vec();
    nested.push(SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), place.ty()).unwrap());
    let changed = SemanticPlaceV1::new(place.local(), nested, place.ty()).unwrap();
    assert!(
        analysis
            .aggregate_read_reference(&changed, &initial)
            .unwrap()
            .is_none()
    );
}

#[test]
fn private_exclusive_aggregate_flow_forwarded_copy_move_keeps_original_borrow() {
    let owner = exclusive_aggregate_owner(ExclusiveAggregateChange::None);
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut analysis = Analysis::new(owner.source_semantic().types(), view);
    let (place, initial) = before_exclusive_read(&mut analysis);
    let base_type = view.body().locals()[place.local().index() as usize].ty();
    let alias = view
        .body()
        .locals()
        .iter()
        .enumerate()
        .find(|(index, local)| *index != place.local().index() as usize && local.ty() == base_type)
        .unwrap()
        .0 as u32;
    let base = SemanticPlaceV1::new(place.local(), vec![], base_type).unwrap();
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(alias),
        place.projections().to_vec(),
        place.ty(),
    )
    .unwrap();
    for moved in [false, true] {
        let mut flow = initial.clone();
        let operand = if moved {
            SemanticOperandV1::Move(base.clone())
        } else {
            SemanticOperandV1::Copy(base.clone())
        };
        let value = analysis
            .operand(&operand, &mut flow, view.body().entry().index() as usize)
            .unwrap();
        flow.assign(alias, value, &mut analysis.budget).unwrap();
        assert_eq!(
            analysis
                .aggregate_read_reference(&projected, &flow)
                .unwrap(),
            Some(reference(&initial, place))
        );
        if moved {
            assert!(!flow.values.contains_key(&place.local().index()));
        }
        flow.invalidate(reference(&initial, place).target, &mut analysis.budget)
            .unwrap();
        assert!(
            analysis
                .aggregate_read_reference(&projected, &flow)
                .unwrap()
                .is_none()
        );
        assert!(
            analysis
                .aggregate_read_reference(place, &flow)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn private_exclusive_aggregate_flow_competing_borrow_reborrow_kill_escape_and_write_reject() {
    let owner = exclusive_aggregate_owner(ExclusiveAggregateChange::None);
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut analysis = Analysis::new(owner.source_semantic().types(), view);
    let (place, initial) = before_exclusive_read(&mut analysis);
    let original = reference(&initial, place);
    let pointer_ty = view.body().locals()[place.local().index() as usize].ty();
    for change in 0..9 {
        let mut flow = initial.clone();
        match change {
            0 => flow.kill(original.target, &mut analysis.budget).unwrap(),
            1 => {
                let value = flow.values[&original.target].clone();
                flow.assign(original.target, Some(value), &mut analysis.budget)
                    .unwrap();
            }
            2 => flow.escape(original.target, &mut analysis.budget).unwrap(),
            // Store/atomic and unknown-call argument escape use these exact
            // operations; the transfer and terminator arms remain unchanged.
            3 => flow.forget_references(&mut analysis.budget).unwrap(),
            4 => flow
                .escape_value(&Value::Reference(original), &mut analysis.budget)
                .unwrap(),
            5 => analysis.kill_place(place, &mut flow).unwrap(),
            6 => {
                flow.kill(original.target, &mut analysis.budget).unwrap();
                flow.dead.insert(original.target);
            }
            7 | 8 => {
                let source = if change == 7 {
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(original.target),
                        vec![],
                        place.projections()[0].result_type(),
                    )
                    .unwrap()
                } else {
                    SemanticPlaceV1::new(
                        place.local(),
                        vec![place.projections()[0].clone()],
                        place.projections()[0].result_type(),
                    )
                    .unwrap()
                };
                let assignment = SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(place.local(), vec![], pointer_ty).unwrap(),
                    SemanticRvalueV1::new(
                        pointer_ty,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Mutable,
                            place: source,
                        },
                    ),
                );
                analysis
                    .assignment(original.borrow, &assignment, &mut flow)
                    .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(
            analysis
                .aggregate_read_reference(place, &flow)
                .unwrap()
                .is_none(),
            "change {change}"
        );
    }
}

#[test]
fn private_exclusive_aggregate_flow_missing_unknown_and_escaped_incoming_reject() {
    let owner = exclusive_aggregate_owner(ExclusiveAggregateChange::None);
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut analysis = Analysis::new(owner.source_semantic().types(), view);
    let (place, initial) = before_exclusive_read(&mut analysis);
    let original = reference(&initial, place);
    let equal_join = initial.join(&initial, &mut analysis.budget).unwrap();
    assert_eq!(
        analysis
            .aggregate_read_reference(place, &equal_join)
            .unwrap(),
        Some(original)
    );
    for change in 0..4 {
        let mut incoming = initial.clone();
        match change {
            0 => {
                incoming.values.remove(&original.target);
            }
            1 => {
                incoming.values.insert(original.target, Value::Opaque);
            }
            2 => incoming
                .escape(original.target, &mut analysis.budget)
                .unwrap(),
            3 => {
                incoming.values.remove(&place.local().index());
            }
            _ => unreachable!(),
        }
        for joined in [
            initial.join(&incoming, &mut analysis.budget).unwrap(),
            incoming.join(&initial, &mut analysis.budget).unwrap(),
        ] {
            assert!(
                analysis
                    .aggregate_read_reference(place, &joined)
                    .unwrap()
                    .is_none()
            );
        }
    }
}

#[test]
fn private_exclusive_aggregate_work_exact_one_short_and_shared_storage_stay_closed() {
    let owner = exclusive_aggregate_owner(ExclusiveAggregateChange::None);
    let view = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let types = owner.source_semantic().types();
    let mut analysis = Analysis::new(types, view);
    assert_eq!(analysis.run().unwrap().len(), 3);
    let spent = MAX_WORK - analysis.budget.remaining;
    let mut exact = Analysis::new(types, view);
    exact.budget = Budget::new(spent);
    assert_eq!(exact.run().unwrap().len(), 3);
    let mut short = Analysis::new(types, view);
    short.budget = Budget::new(spent - 1);
    assert!(short.run().is_err());
    assert!(short.budget.work_exhausted);
    assert_eq!(short.observation.admitted_reads, 0);
    let mut occupied = Analysis::new(types, view);
    let held = occupied.budget.reserve(MAX_STORAGE).unwrap();
    assert!(occupied.run().is_err());
    assert_eq!(occupied.observation.admitted_reads, 0);
    drop(held);
    let mut leaf = Analysis::new(types, view);
    let (place, initial) = before_exclusive_read(&mut leaf);
    let before = leaf.budget.remaining;
    leaf.aggregate_read_reference(place, &initial)
        .unwrap()
        .unwrap();
    let work = before - leaf.budget.remaining;
    leaf.budget = Budget::new(work);
    assert!(
        leaf.aggregate_read_reference(place, &initial)
            .unwrap()
            .is_some()
    );
    leaf.budget = Budget::new(work - 1);
    assert!(leaf.aggregate_read_reference(place, &initial).is_err());
}
