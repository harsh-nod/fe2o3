use super::*;

// Representation checks on descriptors; admitted-source tests live separately.
#[test]
fn slice_aggregate_policy_keeps_root_and_result_walks_pointer_free() {
    let (mut types, function) = descriptors(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Immutable,
        0,
        64,
        SemanticPointerMetadataV1::SliceLength,
        1,
        true,
        true,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
    );
    let reference = SemanticTypeIdV1::from_index(2);
    let unit = SemanticTypeIdV1::from_index(3);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([20; 32]),
        SemanticLayoutIdentityV1::from_sha256([20; 32]),
        SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
        SemanticTypeShapeV1::Unit,
    ));
    let wrapper = SemanticTypeIdV1::from_index(4);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([21; 32]),
        SemanticLayoutIdentityV1::from_sha256([21; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(16),
            8,
            *types[2].layout().backend_repr(),
            false,
            SemanticAggregateLayoutV1::new(vec![16, 0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![unit, reference]).unwrap()),
    ));
    let abi = SemanticAbiValueV1::new(wrapper, function.abi().arguments()[0].mode().clone());
    assert!(
        lower_by_value_abi_components_v1(
            &types,
            &function,
            &abi,
            ParameterLeafPolicyV1::PointerFree
        )
        .is_err()
    );
    let components = lower_by_value_abi_components_v1(
        &types,
        &function,
        &abi,
        ParameterLeafPolicyV1::SharedSliceLeaves,
    )
    .unwrap();
    assert_eq!(components.len(), 1);
    assert_eq!(
        components[0].0,
        [SemanticKirParameterProjectionV1::Field(1)]
    );
    assert_eq!(components[0].1, reference);
    assert_eq!(components[0].4.words().count(), 2);
    assert_eq!(components[0].3, 0);
    let scalar_mode = SemanticAbiValueV1::new(
        wrapper,
        SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
    );
    assert!(
        lower_by_value_abi_components_v1(
            &types,
            &function,
            &scalar_mode,
            ParameterLeafPolicyV1::SharedSliceLeaves
        )
        .is_err()
    );
}
