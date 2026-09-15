use crate::production_ranked_projection_v1::tests::private_capture_tests::{
    AggregateChange, aggregate_capture_owner,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateLayoutV1, SemanticAggregateTypeV1, SemanticLayoutIdentityV1,
    SemanticPointerTypeV1, SemanticProjectionV1, SemanticTypeIdentityV1, SemanticTypeLayoutV1,
};

fn before_field_read<'a>(analysis: &mut Analysis<'a>) -> (&'a SemanticPlaceV1, Flow) {
    let body = analysis.function();
    let block = body.entry().index() as usize;
    let mut flow = Flow::default();
    for (statement, value) in body.blocks()[block].statements().iter().enumerate() {
        let site = Site { block, statement };
        assert!(analysis.checked_site(site));
        if let Some(place) = analysis.aggregate_candidate(value) {
            assert!(
                analysis
                    .aggregate_read_reference(place, &flow)
                    .unwrap()
                    .is_some()
            );
            return (place, flow);
        }
        let SemanticStatementKindV1::Assign(assignment) = value.kind() else {
            panic!("fixture prefix must consist of retained source assignments");
        };
        analysis.assignment(site, assignment, &mut flow).unwrap();
    }
    panic!("fixture must contain the actual private field read")
}

fn target(flow: &Flow, place: &SemanticPlaceV1) -> u32 {
    let Value::Reference(reference) = &flow.values[&place.local().index()] else {
        panic!("fixture requires an exact shared reference")
    };
    reference.target
}

#[test]
fn private_aggregate_capture_missing_partial_and_opaque_nodes_reject() {
    let owner = aggregate_capture_owner(AggregateChange::None, 1);
    let root = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut analysis = Analysis::new(owner.source_semantic().types(), root);
    let (place, initial) = before_field_read(&mut analysis);
    let target = target(&initial, place);
    for change in 0..5 {
        let mut flow = initial.clone();
        if change == 0 {
            flow.values.insert(target, Value::Opaque);
        } else {
            flow.values.edit(target, |value| {
                let Value::Fields(fields) = value else {
                    panic!()
                };
                match change {
                    1 => fields[1] = None,
                    2 => fields[1] = Some(Value::Opaque),
                    3 => {
                        let Some(Value::Fields(leaves)) = &mut fields[1] else {
                            panic!()
                        };
                        leaves[0] = None;
                    }
                    4 => {
                        fields.pop();
                    }
                    _ => unreachable!(),
                }
            });
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
fn private_aggregate_capture_unknown_or_missing_predecessor_cannot_initialize_fields() {
    let owner = aggregate_capture_owner(AggregateChange::None, 1);
    let root = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut analysis = Analysis::new(owner.source_semantic().types(), root);
    let (place, initial) = before_field_read(&mut analysis);
    let target = target(&initial, place);
    for missing in [false, true] {
        let mut other = initial.clone();
        if missing {
            other.values.remove(&target);
        } else {
            other.values.insert(target, Value::Opaque);
        }
        let joined = initial.join(&other, &mut analysis.budget).unwrap();
        assert!(
            analysis
                .aggregate_read_reference(place, &joined)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn private_aggregate_capture_dead_rebound_and_escaped_referents_reject() {
    let owner = aggregate_capture_owner(AggregateChange::None, 1);
    let root = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut analysis = Analysis::new(owner.source_semantic().types(), root);
    let (place, initial) = before_field_read(&mut analysis);
    let target = target(&initial, place);
    for change in 0..5 {
        let mut flow = initial.clone();
        match change {
            0 => {
                flow.kill(target, &mut analysis.budget).unwrap();
                flow.dead.insert(target);
            }
            1 => {
                let value = flow.values[&target].clone();
                flow.assign(target, Some(value), &mut analysis.budget)
                    .unwrap();
            }
            2 => flow.escape(target, &mut analysis.budget).unwrap(),
            3 => {
                let nested = Value::Fields(vec![Some(flow.values[&place.local().index()].clone())]);
                flow.escape_value(&nested, &mut analysis.budget).unwrap();
            }
            4 => flow.forget_references(&mut analysis.budget).unwrap(),
            _ => unreachable!(),
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
fn private_aggregate_capture_exact_field_types_and_reference_shape_are_required() {
    let owner = aggregate_capture_owner(AggregateChange::None, 1);
    let root = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let types = owner.source_semantic().types();
    let mut analysis = Analysis::new(types, root);
    let (place, initial) = before_field_read(&mut analysis);
    let pointee = place.projections()[0].result_type();
    let reference = root.body().locals()[place.local().index() as usize].ty();
    for (field, result) in [(6, place.ty()), (1, pointee), (4, place.ty())] {
        let changed = SemanticPlaceV1::new(
            place.local(),
            vec![
                place.projections()[0].clone(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), result).unwrap(),
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
    let mut foreign_types = types.to_vec();
    let foreign = SemanticTypeIdV1::from_index(foreign_types.len() as u32);
    let original = &types[pointee.index() as usize];
    foreign_types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([253; 32]),
        original.layout_identity(),
        original.layout().clone(),
        original.shape().clone(),
    ));
    let changed = SemanticPlaceV1::new(
        place.local(),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, foreign).unwrap(),
            place.projections()[1].clone(),
        ],
        place.ty(),
    )
    .unwrap();
    assert!(
        Analysis::new(&foreign_types, root)
            .aggregate_read_reference(&changed, &initial)
            .unwrap()
            .is_none()
    );
    for (kind, mutability, space, width, metadata) in [
        (
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            0,
            64,
            SemanticPointerMetadataV1::None,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            0,
            64,
            SemanticPointerMetadataV1::None,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            1,
            64,
            SemanticPointerMetadataV1::None,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
            32,
            SemanticPointerMetadataV1::None,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
            64,
            SemanticPointerMetadataV1::SliceLength,
        ),
    ] {
        let mut changed_types = types.to_vec();
        let original = &types[reference.index() as usize];
        changed_types[reference.index() as usize] = SemanticTypeDeclV1::new(
            original.identity(),
            original.layout_identity(),
            original.layout().clone(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    pointee, kind, mutability, space, width, metadata,
                )
                .unwrap(),
            ),
        );
        assert!(
            Analysis::new(&changed_types, root)
                .aggregate_read_reference(place, &initial)
                .unwrap()
                .is_none()
        );
    }
}

#[test]
fn private_aggregate_capture_exact_work_budget_and_publication_failure() {
    let owner = aggregate_capture_owner(AggregateChange::None, 1);
    let root = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let types = owner.source_semantic().types();
    let mut full = Analysis::new(types, root);
    assert_eq!(full.run().unwrap().len(), 2);
    let used = MAX_WORK - full.budget.remaining;
    let mut exact = Analysis::new(types, root);
    exact.budget = Budget::new(used);
    assert_eq!(exact.run().unwrap().len(), 2);
    let mut short = Analysis::new(types, root);
    short.budget = Budget::new(used - 1);
    assert!(short.run().is_err());
    let mut zero = Analysis::new(types, root);
    zero.budget = Budget::new(0);
    assert!(zero.run().unwrap_or_default().is_empty());
    let mut occupied = Analysis::new(types, root);
    let held = occupied.budget.reserve(MAX_STORAGE).unwrap();
    assert!(occupied.run().is_err());
    drop(held);
}

#[test]
fn private_aggregate_capture_recursive_data_depth_and_field_limits() {
    let owner = aggregate_capture_owner(AggregateChange::None, 1);
    let root = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut types = owner.source_semantic().types().to_vec();
    let integer = types
        .iter()
        .position(|decl| {
            matches!(
                decl.shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32
                })
            )
        })
        .unwrap();
    let mut ty = SemanticTypeIdV1::from_index(integer as u32);
    let mut value = Value::Opaque;
    for depth in 1..=MAX_DEPTH + 1 {
        let next = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([depth as u8; 32]),
            SemanticLayoutIdentityV1::from_sha256([depth as u8; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(4),
                4,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![ty]).unwrap()),
        ));
        ty = next;
        value = Value::Fields(vec![Some(value)]);
        assert_eq!(
            Analysis::new(&types, root)
                .initialized_scalar_data(ty, &value, 0)
                .unwrap(),
            depth <= MAX_DEPTH
        );
    }
    for count in [MAX_FIELDS, MAX_FIELDS + 1] {
        let ty = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([count as u8; 32]),
            SemanticLayoutIdentityV1::from_sha256([count as u8; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(count as u64 * 4),
                4,
                SemanticAggregateLayoutV1::new((0..count).map(|i| i as u64 * 4).collect(), vec![])
                    .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![
                    SemanticTypeIdV1::from_index(integer as u32);
                    count
                ])
                .unwrap(),
            ),
        ));
        let value = Value::Fields(vec![Some(Value::Opaque); count]);
        assert_eq!(
            Analysis::new(&types, root)
                .initialized_scalar_data(ty, &value, 0)
                .unwrap(),
            count <= MAX_FIELDS
        );
    }
}
