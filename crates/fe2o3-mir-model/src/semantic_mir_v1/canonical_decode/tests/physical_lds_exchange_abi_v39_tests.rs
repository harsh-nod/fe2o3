//! Structural marker-ABI tests only; these inert records never mint source custody.
use super::*;

fn marker() -> (InertSemanticMirRequestV1, SemanticFunctionAbiV1) {
    let mut request = minimal_request();
    let scalar = |bits, size| {
        SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, bits, size),
            SemanticScalarValidityRangeV1::new(
                0,
                if bits == 32 {
                    u32::MAX as u128
                } else {
                    u64::MAX as u128
                },
            ),
        )
    };
    let ptr = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX as u128),
    );
    let pair = SemanticBackendReprV1::ScalarPair {
        first: ptr,
        second: scalar(64, 8),
    };
    let ty = |index: u8, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1([20 + index; 32]),
            SemanticLayoutIdentityV1([40 + index; 32]),
            layout,
            shape,
        )
    };
    let layout = |size, align, repr| {
        SemanticTypeLayoutV1::new_with_backend_repr(size, align, repr, false).unwrap()
    };
    let mut types = request.types.to_vec();
    types.push(ty(
        1,
        layout(Some(0), 1, SemanticBackendReprV1::memory(true)),
        SemanticTypeShapeV1::Unit,
    ));
    types.push(ty(
        2,
        layout(Some(8), 8, SemanticBackendReprV1::scalar(scalar(64, 8))),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    ));
    types.push(ty(
        3,
        layout(Some(8), 8, SemanticBackendReprV1::scalar(ptr)),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1(0),
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    types.push(ty(
        4,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            16,
            8,
            SemanticFieldsShapeV1::arbitrary(vec![0, 8, 16], vec![0, 1, 2]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            pair,
            None,
            false,
            None,
            8,
            0,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![
                SemanticTypeIdV1(3),
                SemanticTypeIdV1(2),
                SemanticTypeIdV1(1),
            ])
            .unwrap(),
        ),
    ));
    types.push(ty(
        5,
        layout(None, 4, SemanticBackendReprV1::memory(false)),
        SemanticTypeShapeV1::Slice {
            element: SemanticTypeIdV1(0),
        },
    ));
    types.push(ty(
        6,
        layout(Some(16), 8, pair),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1(5),
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
    ));
    request.types = types.into_boxed_slice();
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let value = |id| {
        SemanticAbiValueV1::new(
            SemanticTypeIdV1(id),
            SemanticAbiPassModeV1::Pair {
                first: attributes,
                second: attributes,
            },
        )
    };
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1([60; 32]),
        SemanticLayoutIdentityV1([61; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![value(6), value(4)],
        SemanticAbiValueV1::new(SemanticTypeIdV1(1), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
    ])
    .unwrap();
    (request, abi)
}

#[test]
fn physical_lds_exchange_v39_structural_marker_joins_two_pairs_and_actual_u32_pointees() {
    let (request, abi) = marker();
    assert!(physical::signature_matches(
        &request,
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalLdsExchangeBegin(frame()),
        &abi
    ));
}

#[test]
fn physical_lds_exchange_v39_marker_rejects_ownership_mutability_shape_and_pointee_substitution() {
    let (request, abi) = marker();
    let begin = SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalLdsExchangeBegin(frame());
    for owners in [
        vec![
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
        ],
        vec![
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::ByValue,
        ],
        vec![
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        ],
    ] {
        assert!(!physical::signature_matches(
            &request,
            begin,
            &abi.clone().with_source_argument_ownership(owners).unwrap()
        ));
    }
    for case in 0..6 {
        let mut changed = request.clone();
        match case {
            0 => {
                changed.types[6].shape = SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        SemanticTypeIdV1(5),
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Mutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::SliceLength,
                    )
                    .unwrap(),
                )
            }
            1 => {
                changed.types[6].shape = SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        SemanticTypeIdV1(5),
                        SemanticPointerKindV1::Raw,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::SliceLength,
                    )
                    .unwrap(),
                )
            }
            2 => {
                changed.types[5].shape = SemanticTypeShapeV1::Slice {
                    element: SemanticTypeIdV1(2),
                }
            }
            3 => {
                changed.types[3].shape = SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        SemanticTypeIdV1(2),
                        SemanticPointerKindV1::Raw,
                        SemanticMutabilityV1::Mutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                )
            }
            4 => {
                changed.types[4].shape = SemanticTypeShapeV1::Tuple(
                    SemanticAggregateTypeV1::new(vec![
                        SemanticTypeIdV1(3),
                        SemanticTypeIdV1(2),
                        SemanticTypeIdV1(1),
                    ])
                    .unwrap(),
                )
            }
            5 => {
                changed.types[4].shape = SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![
                        SemanticTypeIdV1(2),
                        SemanticTypeIdV1(3),
                        SemanticTypeIdV1(1),
                    ])
                    .unwrap(),
                )
            }
            _ => unreachable!(),
        }
        assert!(
            !physical::signature_matches(&changed, begin, &abi),
            "mutation {case}"
        );
    }
}

#[test]
fn physical_lds_exchange_v39_tagged_source_storage_remains_one_bounded_payload() {
    use std::mem::{align_of, size_of};
    let before = size_of::<Option<SemanticPhysicalEntrySourceV37>>();
    let after = size_of::<Option<PhysicalSourceTailV1>>();
    assert!(after <= before + align_of::<SemanticPhysicalEntrySourceV37>());
    println!(
        "physical_source_tail_layout old={before} tagged={after} direct_call={}",
        size_of::<SemanticDirectCallV1>()
    );
}
