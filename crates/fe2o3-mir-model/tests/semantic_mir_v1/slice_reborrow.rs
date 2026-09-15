use super::*;

const ELEMENT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const INPUT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const OUTPUT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const USIZE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const OTHER_ELEMENT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const OTHER_SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);

#[derive(Clone, Copy, Debug)]
struct Options {
    kind: SemanticBorrowKindV1,
    input_kind: SemanticPointerKindV1,
    input_mutability: SemanticMutabilityV1,
    output_kind: SemanticPointerKindV1,
    output_mutability: SemanticMutabilityV1,
    input_space: u32,
    output_space: u32,
    wrong_element: bool,
    missing_dereference: bool,
    address_of: bool,
    wrong_metadata_layout: bool,
    wrong_abi: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            kind: SemanticBorrowKindV1::Shared,
            input_kind: SemanticPointerKindV1::Reference,
            input_mutability: SemanticMutabilityV1::Mutable,
            output_kind: SemanticPointerKindV1::Reference,
            output_mutability: SemanticMutabilityV1::Immutable,
            input_space: 0,
            output_space: 0,
            wrong_element: false,
            missing_dereference: false,
            address_of: false,
            wrong_metadata_layout: false,
            wrong_abi: false,
        }
    }
}

fn slice_type(tag: u8, element: SemanticTypeIdV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        type_identity(tag),
        layout_identity(tag),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            4,
            SemanticFieldsShapeV1::array(4, 0),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(false),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Slice { element },
    )
}

fn slice_reference(
    tag: u8,
    pointee: SemanticTypeIdV1,
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
    space: u32,
    wrong_metadata_layout: bool,
) -> SemanticTypeDeclV1 {
    let data = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(space, 8, 8),
        SemanticScalarValidityRangeV1::new(
            u128::from(kind == SemanticPointerKindV1::Reference),
            u64::MAX.into(),
        ),
    );
    let length_bits = if wrong_metadata_layout { 32 } else { 64 };
    let length = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, length_bits, u64::from(length_bits / 8)),
        SemanticScalarValidityRangeV1::new(0, (1_u128 << length_bits) - 1),
    );
    let pointee_kind = match (kind, mutability) {
        (SemanticPointerKindV1::Raw, _) => SemanticAbiPointeeKindV1::Raw,
        (_, SemanticMutabilityV1::Immutable) => {
            SemanticAbiPointeeKindV1::SharedReference { frozen: true }
        }
        (_, SemanticMutabilityV1::Mutable) => {
            SemanticAbiPointeeKindV1::MutableReference { unpin: true }
        }
    };
    SemanticTypeDeclV1::new(
        type_identity(tag),
        layout_identity(tag),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(data, length),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                kind,
                mutability,
                space,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    pointee_kind,
                    0,
                    if kind == SemanticPointerKindV1::Raw {
                        1
                    } else {
                        4
                    },
                )
                .unwrap(),
            ),
            None,
        ),
    )
}

fn reborrow_request(options: Options) -> InertSemanticMirRequestV1 {
    reborrow_request_with_element(options, u32_type(1))
}

fn reborrow_request_with_element(
    options: Options,
    element: SemanticTypeDeclV1,
) -> InertSemanticMirRequestV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let reference = options.input_kind == SemanticPointerKindV1::Reference;
    let shared = reference && options.input_mutability == SemanticMutabilityV1::Immutable;
    let data_attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            shared,
            shared.then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            reference && !options.wrong_abi,
            shared,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        0,
        reference.then_some(4),
    )
    .unwrap();
    let ownership = match (options.input_kind, options.input_mutability) {
        (SemanticPointerKindV1::Raw, _) => SemanticSourceArgumentOwnershipV1::RawPointer,
        (_, SemanticMutabilityV1::Immutable) => SemanticSourceArgumentOwnershipV1::SharedBorrow,
        (_, SemanticMutabilityV1::Mutable) => SemanticSourceArgumentOwnershipV1::UniqueBorrow,
    };
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(bytes(1)),
        layout_identity(1),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![SemanticAbiValueV1::new(
            INPUT,
            SemanticAbiPassModeV1::Pair {
                first: data_attributes,
                second: noundef_attributes(SemanticAbiExtensionV1::None),
            },
        )],
        SemanticAbiValueV1::new(
            USIZE,
            SemanticAbiPassModeV1::Direct(noundef_attributes(SemanticAbiExtensionV1::None)),
        ),
    )
    .unwrap()
    .with_source_argument_ownership(vec![ownership])
    .unwrap();
    let projections = if options.missing_dereference {
        vec![]
    } else {
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SLICE).unwrap()]
    };
    let borrowed =
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), projections, SLICE).unwrap();
    let rvalue = if options.address_of {
        SemanticRvalueKindV1::AddressOf {
            mutability: options.output_mutability,
            place: borrowed,
        }
    } else {
        SemanticRvalueKindV1::Borrow {
            kind: options.kind,
            place: borrowed,
        }
    };
    let assign = |local, ty, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, ty),
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let mut types = vec![
        element,
        slice_type(2, ELEMENT),
        slice_reference(
            3,
            SLICE,
            options.input_kind,
            options.input_mutability,
            options.input_space,
            false,
        ),
        slice_reference(
            4,
            if options.wrong_element {
                OTHER_SLICE
            } else {
                SLICE
            },
            options.output_kind,
            options.output_mutability,
            options.output_space,
            options.wrong_metadata_layout,
        ),
        u64_type(7),
    ];
    if options.wrong_element {
        types.extend([i32_type(8), slice_type(9, OTHER_ELEMENT)]);
    }
    request(
        types,
        vec![],
        vec![function(
            1,
            abi,
            vec![
                local(1, USIZE, SemanticLocalRoleV1::Return),
                local(2, INPUT, SemanticLocalRoleV1::Argument(0)),
                local(3, OUTPUT, SemanticLocalRoleV1::Temporary),
            ],
            vec![block(
                1,
                vec![
                    assign(2, OUTPUT, rvalue),
                    assign(
                        0,
                        USIZE,
                        SemanticRvalueKindV1::Unary {
                            operation: SemanticUnaryOpV1::PointerMetadata,
                            operand: SemanticOperandV1::Move(place(2, OUTPUT)),
                        },
                    ),
                ],
                SemanticTerminatorKindV1::Return,
            )],
        )],
    )
}

#[test]
fn exact_slice_reborrows_preserve_the_pair_and_source_borrow_ownership() {
    let f32_type = SemanticTypeDeclV1::new(
        type_identity(1),
        layout_identity(1),
        scalar_layout(
            4,
            4,
            SemanticBackendPrimitiveV1::float(32, 4),
            SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
        ),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
    );
    for element in [u32_type(1), i32_type(1), f32_type] {
        check_slice_reborrow_ownership(element);
    }
}

fn check_slice_reborrow_ownership(element: SemanticTypeDeclV1) {
    for (input, output, kind) in [
        (
            SemanticMutabilityV1::Immutable,
            SemanticMutabilityV1::Immutable,
            SemanticBorrowKindV1::Shared,
        ),
        (
            SemanticMutabilityV1::Mutable,
            SemanticMutabilityV1::Immutable,
            SemanticBorrowKindV1::Shared,
        ),
        (
            SemanticMutabilityV1::Mutable,
            SemanticMutabilityV1::Mutable,
            SemanticBorrowKindV1::Mutable,
        ),
    ] {
        let request = reborrow_request_with_element(
            Options {
                input_mutability: input,
                output_mutability: output,
                kind,
                ..Options::default()
            },
            element.clone(),
        );
        let ownership = vec![if input == SemanticMutabilityV1::Mutable {
            SemanticSourceArgumentOwnershipV1::UniqueBorrow
        } else {
            SemanticSourceArgumentOwnershipV1::SharedBorrow
        }];
        assert!(!ownership.contains(&SemanticSourceArgumentOwnershipV1::ExclusiveOwner));
        let admitted = request
            .admit_exact_v12(SemanticMirLimitsV1::default())
            .unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_exact_v12_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
        assert_eq!(
            decoded.functions()[0].abi().source_argument_ownership(),
            ownership
        );
    }
}

#[test]
fn slice_reborrow_rejects_raw_uniqueness_element_and_address_space_substitutions() {
    for options in [
        Options {
            input_kind: SemanticPointerKindV1::Raw,
            ..Options::default()
        },
        Options {
            output_kind: SemanticPointerKindV1::Raw,
            ..Options::default()
        },
        Options {
            input_mutability: SemanticMutabilityV1::Immutable,
            output_mutability: SemanticMutabilityV1::Mutable,
            kind: SemanticBorrowKindV1::Mutable,
            ..Options::default()
        },
        Options {
            output_mutability: SemanticMutabilityV1::Mutable,
            ..Options::default()
        },
        Options {
            kind: SemanticBorrowKindV1::Fake,
            ..Options::default()
        },
        Options {
            wrong_element: true,
            ..Options::default()
        },
        Options {
            input_space: 1,
            ..Options::default()
        },
        Options {
            output_space: 1,
            ..Options::default()
        },
        Options {
            address_of: true,
            output_kind: SemanticPointerKindV1::Raw,
            ..Options::default()
        },
    ] {
        let result = reborrow_request(options).admit_exact_v12(SemanticMirLimitsV1::default());
        assert!(
            matches!(
                result,
                Err(SemanticMirErrorV1::InvalidTypeOperation {
                    operation: SemanticTypeOperationV1::Borrow,
                    ..
                })
            ),
            "{options:?}: {result:?}"
        );
    }
}

#[test]
fn slice_reborrow_does_not_bypass_place_layout_or_abi_validation() {
    for options in [
        Options {
            missing_dereference: true,
            ..Options::default()
        },
        Options {
            wrong_metadata_layout: true,
            ..Options::default()
        },
        Options {
            wrong_abi: true,
            ..Options::default()
        },
    ] {
        assert!(
            reborrow_request(options)
                .admit_exact_v12(SemanticMirLimitsV1::default())
                .is_err(),
            "{options:?}"
        );
    }
}
