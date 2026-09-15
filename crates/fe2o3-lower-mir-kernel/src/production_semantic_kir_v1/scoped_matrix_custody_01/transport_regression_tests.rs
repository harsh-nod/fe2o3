// These are exact transport-edge tests, not source issuers or layout receipts.
fn id(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

fn declaration(tag: u8, shape: SemanticTypeShapeV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        shape,
    )
}

#[test]
fn scoped_context_delegates_only_exact_permitted_reference_edges() {
    for (kind, mutability, target, space, width, expected) in [
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            id(0),
            0,
            64,
            Some(SemanticBorrowKindV1::Mutable),
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
            id(0),
            0,
            64,
            Some(SemanticBorrowKindV1::Shared),
        ),
        (
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
            id(0),
            0,
            64,
            None,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            id(1),
            0,
            64,
            None,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            id(0),
            1,
            64,
            None,
        ),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
            id(0),
            0,
            32,
            None,
        ),
    ] {
        let types = vec![
            declaration(1, SemanticTypeShapeV1::Unit),
            declaration(
                2,
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        target,
                        kind,
                        mutability,
                        space,
                        width,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            ),
        ];
        assert_eq!(
            context_reference(&types, id(1), id(0), Role::Context),
            expected
        );
        for role in [
            Role::Matrix,
            Role::Policy,
            Role::Subgroup,
            Role::Lane,
            Role::Bound,
            Role::Narrow,
        ] {
            assert_eq!(context_reference(&types, id(1), id(0), role), None);
        }
        assert_eq!(context_reference(&types, id(0), id(0), Role::Context), None);
        assert_eq!(
            context_reference(&types, id(99), id(0), Role::Context),
            None
        );
        if mutability == SemanticMutabilityV1::Mutable {
            assert_eq!(
                shared_pointee(&types, id(1)),
                None,
                "other scoped roles remain shared-only"
            );
        }
    }
}

fn tuple_types() -> Vec<SemanticTypeDeclV1> {
    vec![
        declaration(1, SemanticTypeShapeV1::Unit),
        declaration(
            2,
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        ),
        declaration(
            3,
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![id(0), id(1)]).unwrap()),
        ),
        declaration(
            4,
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    id(2),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
    ]
}

#[test]
fn scoped_tuple_paths_preserve_exact_declared_field_bounds_and_types() {
    let types = tuple_types();
    assert_eq!(aggregate_fields(&types, id(2)), Some(&[id(0), id(1)][..]));
    for (field, target) in [(0, id(0)), (1, id(1))] {
        let projection =
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), target).unwrap();
        check_path(&types, id(2), &[projection], target).unwrap();
        check_path(
            &types,
            id(3),
            &[
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, id(2)).unwrap(),
                projection,
            ],
            target,
        )
        .unwrap();
    }
    for (field, declared, target) in [
        (2, id(0), id(0)),
        (u32::MAX, id(0), id(0)),
        (0, id(1), id(1)),
        (0, id(0), id(1)),
    ] {
        assert!(matches!(
            check_path(
                &types,
                id(2),
                &[
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), declared)
                        .unwrap(),
                ],
                target
            ),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
    }
    assert!(
        check_path(
            &types,
            id(0),
            &[SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), id(0)).unwrap(),],
            id(0)
        )
        .is_err()
    );
    assert_eq!(aggregate_fields(&types, id(3)), None);
    assert_eq!(aggregate_fields(&types, id(99)), None);
}
