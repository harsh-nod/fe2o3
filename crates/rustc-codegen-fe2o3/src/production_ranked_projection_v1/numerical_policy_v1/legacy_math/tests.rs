use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn id(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

fn ty(index: u8, shape: SemanticTypeShapeV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([index + 1; 32]),
        SemanticLayoutIdentityV1::from_sha256([index + 1; 32]),
        SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
        shape,
    )
}

fn aggregate(index: u8, fields: &[u32]) -> SemanticTypeDeclV1 {
    ty(
        index,
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(fields.iter().copied().map(id).collect()).unwrap(),
        ),
    )
}

fn pointer(index: u8, pointee: u32) -> SemanticTypeDeclV1 {
    ty(
        index,
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
        ),
    )
}

#[test]
fn retained_policy_cannot_be_erased_by_legacy_math_projection() {
    let types = vec![
        aggregate(0, &[]),
        aggregate(1, &[]),
        pointer(2, 0),
        pointer(3, 1),
        aggregate(4, &[2, 3]),
        pointer(5, 4),
        aggregate(6, &[5]),
    ];
    let retained = types.clone();
    assert!(policy_free_context(&types, id(0), &[id(1)]));
    assert!(policy_free_context(&types, id(2), &[id(1)]));
    for index in [1, 3, 4, 5, 6] {
        assert!(!policy_free_context(&types, id(index), &[id(1)]));
    }
    assert_eq!(types, retained);
}

#[test]
fn checks_all_issuances_and_terminates_on_recursive_or_missing_types() {
    let types = vec![
        aggregate(0, &[]),
        aggregate(1, &[2]),
        pointer(2, 1),
        aggregate(3, &[2, 0]),
    ];
    assert!(policy_free_context(&types, id(1), &[id(0)]));
    assert!(!policy_free_context(&types, id(3), &[id(99), id(0)]));
    assert!(!policy_free_context(&types, id(99), &[]));
    let types = vec![aggregate(0, &[99])];
    assert!(!policy_free_context(&types, id(0), &[]));
}

#[test]
fn checks_policy_references_inside_each_aggregate_shape() {
    for shape in [
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![id(1)]).unwrap()),
        SemanticTypeShapeV1::Union(SemanticAggregateTypeV1::new(vec![id(1)]).unwrap()),
        SemanticTypeShapeV1::Array {
            element: id(1),
            length: 1,
        },
        SemanticTypeShapeV1::Slice { element: id(1) },
        SemanticTypeShapeV1::Enum {
            discriminant: id(3),
            variants: vec![SemanticEnumVariantV1::new(
                0,
                SemanticAggregateTypeV1::new(vec![id(1)]).unwrap(),
            )]
            .into_boxed_slice(),
        },
    ] {
        let types = vec![
            ty(0, shape),
            pointer(1, 2),
            aggregate(2, &[]),
            aggregate(3, &[]),
        ];
        assert!(!policy_free_context(&types, id(0), &[id(2)]));
    }
}
