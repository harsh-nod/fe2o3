use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn id(i: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(i)
}
fn projection(kind: SemanticProjectionKindV1, ty: u32) -> SemanticProjectionV1 {
    SemanticProjectionV1::new(kind, id(ty)).unwrap()
}
fn deref(ty: u32) -> SemanticProjectionV1 {
    projection(SemanticProjectionKindV1::Dereference, ty)
}
fn field(i: u32, ty: u32) -> SemanticProjectionV1 {
    projection(SemanticProjectionKindV1::Field(i), ty)
}
fn place(path: Vec<SemanticProjectionV1>, ty: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), path, id(ty)).unwrap()
}
fn reference(pointee: u32) -> SemanticTypeShapeV1 {
    SemanticTypeShapeV1::Pointer(
        SemanticPointerTypeV1::new_with_kind(
            id(pointee),
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            0,
            64,
            SemanticPointerMetadataV1::None,
        )
        .unwrap(),
    )
}
fn types() -> Vec<SemanticTypeDeclV1> {
    [
        (
            0,
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        ),
        (8, reference(0)),
        (
            16,
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![id(1), id(5)]).unwrap(),
            ),
        ),
        (8, reference(2)),
        (8, reference(3)),
        (
            8,
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
        ),
        (8, reference(1)),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (size, shape))| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([index as u8 + 1; 32]),
            SemanticLayoutIdentityV1::from_sha256([index as u8 + 1; 32]),
            SemanticTypeLayoutV1::new(Some(size), if size == 0 { 1 } else { 8 }).unwrap(),
            shape,
        )
    })
    .collect()
}

#[test]
fn exact_shared_closure_field_is_transport_not_a_workgroup_issuer() {
    let types = types();
    let route = borrow_route(
        &types,
        id(2),
        &place(vec![], 2),
        id(3),
        &[deref(2), field(0, 1)],
    )
    .unwrap();
    assert_eq!(route.path, [field(0, 1)]);
    assert!(route.storage_loan);
    // The route ends at the retained field. It does not fabricate an issuer,
    // turn the closure into a Workgroup, or remove the captured referent loan.
    assert!(borrow_route(&types, id(2), &place(vec![], 2), id(1), &[]).is_err());
}

#[test]
fn reference_slots_and_owned_carriers_keep_storage_loans() {
    let types = types();
    let route = borrow_route(
        &types,
        id(3),
        &place(vec![], 3),
        id(4),
        &[deref(3), deref(2), field(0, 1)],
    )
    .unwrap();
    assert_eq!(route.path, [deref(2), field(0, 1)]);
    assert!(
        route.storage_loan,
        "borrowing a reference slot is not a pointee reborrow"
    );
    let route = borrow_route(
        &types,
        id(2),
        &place(vec![field(0, 1)], 1),
        id(6),
        &[deref(1)],
    )
    .unwrap();
    assert!(
        route.storage_loan,
        "a direct field borrow keeps the carrier storage loan"
    );
    let route = borrow_route(
        &types,
        id(3),
        &place(vec![deref(2), field(0, 1)], 1),
        id(6),
        &[deref(1)],
    )
    .unwrap();
    assert!(
        !route.storage_loan,
        "pointee reborrow must retain the earlier loans, not renew them"
    );
    assert_eq!(route.path, [deref(2), field(0, 1)]);
}

#[test]
fn direct_workgroup_shared_shape_is_unchanged() {
    let types = types();
    for tail in [vec![], vec![deref(0)]] {
        let route = borrow_route(&types, id(0), &place(vec![], 0), id(1), &tail).unwrap();
        assert!(route.path.is_empty());
        assert!(route.storage_loan);
    }
    let route = borrow_route(&types, id(1), &place(vec![deref(0)], 0), id(1), &[]).unwrap();
    assert_eq!(route.path, [deref(0)]);
    assert!(!route.storage_loan);
}

#[test]
fn mutable_raw_wrong_pointee_space_width_and_metadata_never_route() {
    for change in 0..6 {
        let mut types = types();
        let pointer = SemanticPointerTypeV1::new_with_kind(
            id(if change == 0 { 5 } else { 2 }),
            if change == 1 {
                SemanticPointerKindV1::Raw
            } else {
                SemanticPointerKindV1::Reference
            },
            if change == 2 {
                SemanticMutabilityV1::Mutable
            } else {
                SemanticMutabilityV1::Immutable
            },
            u32::from(change == 3),
            if change == 4 { 32 } else { 64 },
            if change == 5 {
                SemanticPointerMetadataV1::SliceLength
            } else {
                SemanticPointerMetadataV1::None
            },
        )
        .unwrap();
        let old = &types[3];
        types[3] = SemanticTypeDeclV1::new(
            old.identity(),
            old.layout_identity(),
            old.layout().clone(),
            SemanticTypeShapeV1::Pointer(pointer),
        );
        assert!(
            borrow_route(
                &types,
                id(2),
                &place(vec![], 2),
                id(3),
                &[deref(2), field(0, 1)]
            )
            .is_err(),
            "mutation {change}"
        );
    }
}

#[test]
fn every_nominal_edge_and_unsupported_projection_rejects() {
    let types = types();
    for tail in [
        vec![deref(5), field(0, 1)],
        vec![deref(2), field(0, 5)],
        vec![deref(2), field(2, 1)],
        vec![field(0, 1)],
        vec![
            deref(2),
            projection(SemanticProjectionKindV1::Downcast(0), 1),
        ],
        vec![
            deref(2),
            projection(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(1)),
                1,
            ),
        ],
        vec![
            deref(2),
            projection(SemanticProjectionKindV1::OpaqueCast, 1),
        ],
    ] {
        assert!(borrow_route(&types, id(2), &place(vec![], 2), id(3), &tail).is_err());
    }
    assert!(borrow_route(&types, id(5), &place(vec![], 2), id(3), &[]).is_err());
    assert!(borrow_route(&types, id(3), &place(vec![deref(5)], 5), id(1), &[]).is_err());
}

#[test]
fn route_work_preserves_the_existing_sixteen_projection_bound() {
    assert_eq!(route_work(8, 8).unwrap(), 76);
    for (a, b) in [(16, 1), (0, 17), (usize::MAX, 1)] {
        assert!(
            matches!(route_work(a, b), Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
            if detail == "borrowed Workgroup source path is cyclic or too deep")
        );
    }
}
