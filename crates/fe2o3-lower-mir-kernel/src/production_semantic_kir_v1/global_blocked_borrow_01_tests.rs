#[test]
fn blocked_store_requires_exact_shared_reference_not_owned_witness_type() {
    use fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1;

    let owned = SemanticTypeIdV1::from_index(0);
    let reference = SemanticTypeIdV1::from_index(1);
    let other = SemanticTypeIdV1::from_index(2);
    let declaration = |tag, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            shape,
        )
    };
    for mutation in 0..7 {
        let types = vec![
            declaration(
                1,
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
            ),
            declaration(
                2,
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        if mutation == 1 { other } else { owned },
                        if mutation == 2 {
                            SemanticPointerKindV1::Raw
                        } else {
                            SemanticPointerKindV1::Reference
                        },
                        if mutation == 3 {
                            SemanticMutabilityV1::Mutable
                        } else {
                            SemanticMutabilityV1::Immutable
                        },
                        u32::from(mutation == 4),
                        if mutation == 5 { 32 } else { 64 },
                        if mutation == 6 {
                            SemanticPointerMetadataV1::SliceLength
                        } else {
                            SemanticPointerMetadataV1::None
                        },
                    )
                    .unwrap(),
                ),
            ),
            declaration(
                3,
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
            ),
        ];
        assert_eq!(
            semantic_shared_reference_to_v1(&types, reference, owned),
            mutation == 0,
            "borrow shape mutation {mutation}"
        );
        assert!(!semantic_shared_reference_to_v1(&types, owned, owned));
    }
}
