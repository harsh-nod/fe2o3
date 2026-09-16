use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "shared_slice_aggregate_parameter_tests.rs"]
mod aggregate_leaves;

// Descriptor-selection component fixtures only. No source admission or owner
// is claimed for hostile combinations of shape, layout and ABI below.
#[allow(clippy::too_many_arguments)]
fn descriptors(
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
    address_space: u32,
    width: u16,
    metadata: SemanticPointerMetadataV1,
    pointee: u32,
    scalar_element: bool,
    pair: bool,
    ownership: SemanticSourceArgumentOwnershipV1,
) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let scalar = SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
        signed: false,
        bits: 32,
    });
    let shapes = [
        if scalar_element {
            scalar
        } else {
            SemanticTypeShapeV1::Unit
        },
        SemanticTypeShapeV1::Slice {
            element: SemanticTypeIdV1::from_index(0),
        },
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(pointee),
                kind,
                mutability,
                address_space,
                width,
                metadata,
            )
            .unwrap(),
        ),
    ];
    let types = shapes
        .into_iter()
        .enumerate()
        .map(|(index, shape)| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([index as u8 + 1; 32]),
                SemanticLayoutIdentityV1::from_sha256([index as u8 + 1; 32]),
                if index == 2 {
                    SemanticTypeLayoutV1::new_with_backend_repr(
                        Some(16),
                        8,
                        SemanticBackendReprV1::scalar_pair(
                            SemanticBackendScalarV1::initialized(
                                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                            ),
                            SemanticBackendScalarV1::initialized(
                                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                            ),
                        ),
                        false,
                    )
                    .unwrap()
                } else {
                    SemanticTypeLayoutV1::new_with_backend_repr(
                        Some(4),
                        4,
                        SemanticBackendReprV1::memory(true),
                        false,
                    )
                    .unwrap()
                },
                shape,
            )
        })
        .collect();
    let reference = SemanticTypeIdV1::from_index(2);
    let mode = if pair {
        SemanticAbiPassModeV1::Pair {
            first: SemanticAbiValueAttributesV1::plain(),
            second: SemanticAbiValueAttributesV1::plain(),
        }
    } else {
        SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain())
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([4; 32]),
        SemanticLayoutIdentityV1::from_sha256([5; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            reference, mode,
        ))],
        SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(0),
            SemanticAbiPassModeV1::Ignore,
        ),
    )
    .unwrap()
    .with_source_argument_ownership(vec![ownership])
    .unwrap();
    let source = SemanticSourceProvenanceV1::unavailable();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([6; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([7; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([8; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([9; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([10; 32]),
        source,
        abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([11; 32]),
                SemanticTypeIdV1::from_index(0),
                SemanticLocalRoleV1::Return,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([12; 32]),
                reference,
                SemanticLocalRoleV1::Argument(0),
                source,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([13; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    (types, function)
}

#[test]
fn shared_slice_helper_descriptor_selection_rejects_nonmatching_axes() {
    let exact = (
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
    let cases = [
        exact,
        (
            SemanticPointerKindV1::Raw,
            exact.1,
            exact.2,
            exact.3,
            exact.4,
            exact.5,
            exact.6,
            exact.7,
            exact.8,
        ),
        (
            exact.0,
            SemanticMutabilityV1::Mutable,
            exact.2,
            exact.3,
            exact.4,
            exact.5,
            exact.6,
            exact.7,
            exact.8,
        ),
        (
            exact.0, exact.1, 3, exact.3, exact.4, exact.5, exact.6, exact.7, exact.8,
        ),
        (
            exact.0, exact.1, exact.2, 32, exact.4, exact.5, exact.6, exact.7, exact.8,
        ),
        (
            exact.0,
            exact.1,
            exact.2,
            exact.3,
            SemanticPointerMetadataV1::None,
            exact.5,
            exact.6,
            exact.7,
            exact.8,
        ),
        (
            exact.0,
            exact.1,
            exact.2,
            exact.3,
            SemanticPointerMetadataV1::VTable,
            exact.5,
            exact.6,
            exact.7,
            exact.8,
        ),
        (
            exact.0, exact.1, exact.2, exact.3, exact.4, 0, exact.6, exact.7, exact.8,
        ),
        (
            exact.0, exact.1, exact.2, exact.3, exact.4, 99, exact.6, exact.7, exact.8,
        ),
        (
            exact.0, exact.1, exact.2, exact.3, exact.4, exact.5, false, exact.7, exact.8,
        ),
        (
            exact.0, exact.1, exact.2, exact.3, exact.4, exact.5, exact.6, false, exact.8,
        ),
        (
            exact.0,
            exact.1,
            exact.2,
            exact.3,
            exact.4,
            exact.5,
            exact.6,
            exact.7,
            SemanticSourceArgumentOwnershipV1::ByValue,
        ),
    ];
    for (index, (kind, mutability, space, width, metadata, pointee, scalar, pair, ownership)) in
        cases.into_iter().enumerate()
    {
        let (types, function) = descriptors(
            kind, mutability, space, width, metadata, pointee, scalar, pair, ownership,
        );
        assert_eq!(
            shared_slice_helper_parameter_v1(&types, &function, 0, SemanticTypeIdV1::from_index(2)),
            index == 0,
            "case {index}"
        );
        assert!(!shared_slice_helper_parameter_v1(
            &types,
            &function,
            1,
            SemanticTypeIdV1::from_index(2)
        ));
        assert!(!shared_slice_helper_parameter_v1(
            &types,
            &function,
            0,
            SemanticTypeIdV1::from_index(0)
        ));
    }
}
