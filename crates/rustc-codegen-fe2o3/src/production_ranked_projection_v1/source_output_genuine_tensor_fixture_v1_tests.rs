// Included below the genuine pipeline fixture helpers. These construct source
// declarations only; all executable owners come from normal admission/lowering.
const T_U16: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const T_F32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const T_SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const T_INPUT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const T_OUTPUT_SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const T_OUTPUT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
const T_LANE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
const T_LANE_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(11);
const T_CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(12);
const T_CONTEXT_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(13);
const T_VIEW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(14);
const T_VIEW_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(15);
const T_ERROR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(16);
const T_RESULT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(17);
const T_A: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(18);
const T_B: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(19);
const T_ACC: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(20);
const T_VALUES: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(21);

fn tensor_type(
    ty: SemanticTypeIdV1,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    let tag = 30 + ty.index() as u8;
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        layout,
        shape,
    )
}

fn tensor_array_type(
    ty: SemanticTypeIdV1,
    element: SemanticTypeIdV1,
    width: u64,
    length: u64,
) -> SemanticTypeDeclV1 {
    tensor_type(
        ty,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            width * length,
            width,
            SemanticFieldsShapeV1::array(width, length),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            width,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array { element, length },
    )
}

fn tensor_reference_type(
    ty: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
    mutable: bool,
    slice: bool,
    size: u64,
    align: u64,
) -> SemanticTypeDeclV1 {
    let pointer = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    );
    let backend = if slice {
        SemanticBackendReprV1::scalar_pair(
            pointer,
            SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            ),
        )
    } else {
        SemanticBackendReprV1::scalar(pointer)
    };
    tensor_type(
        ty,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(if slice { 16 } else { 8 }),
            8,
            backend,
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                SemanticPointerKindV1::Reference,
                if mutable {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
                0,
                64,
                if slice {
                    SemanticPointerMetadataV1::SliceLength
                } else {
                    SemanticPointerMetadataV1::None
                },
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    if mutable {
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                    } else {
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                    },
                    size,
                    align,
                )
                .unwrap(),
            ),
            None,
        ),
    )
}

fn tensor_enum_type(
    ty: SemanticTypeIdV1,
    size: u64,
    fields: Vec<Vec<SemanticTypeIdV1>>,
    offsets: Vec<Vec<u64>>,
) -> SemanticTypeDeclV1 {
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, (fields.len() - 1) as u128),
    );
    let variants = offsets
        .into_iter()
        .enumerate()
        .map(|(index, offsets)| {
            SemanticEnumVariantLayoutV1::from_rustc(
                index as u32,
                size,
                8,
                SemanticFieldsShapeV1::arbitrary(
                    offsets.clone(),
                    (0..offsets.len() as u32).collect(),
                )
                .unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                0,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap()
        })
        .collect();
    tensor_type(
        ty,
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            size,
            8,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            U32,
            fields
                .into_iter()
                .enumerate()
                .map(|(index, fields)| {
                    SemanticEnumVariantV1::new(
                        index as u128,
                        SemanticAggregateTypeV1::new(fields).unwrap(),
                    )
                })
                .collect(),
        )
        .unwrap(),
    )
}

fn tensor_types() -> Vec<SemanticTypeDeclV1> {
    let mut result = types();
    result.truncate(4);
    result.push(tensor_type(
        T_U16,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(2),
            2,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 16, 2),
                SemanticScalarValidityRangeV1::new(0, u16::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 16,
        }),
    ));
    result.push(tensor_type(
        T_F32,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::float(32, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
    ));
    result.push(tensor_type(
        T_SLICE,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            2,
            SemanticFieldsShapeV1::array(2, 0),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(false),
            None,
            false,
            None,
            2,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Slice { element: T_U16 },
    ));
    result.push(tensor_reference_type(T_INPUT, T_SLICE, false, true, 0, 2));
    result.push(tensor_type(
        T_OUTPUT_SLICE,
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
        SemanticTypeShapeV1::Slice { element: T_F32 },
    ));
    result.push(tensor_reference_type(
        T_OUTPUT,
        T_OUTPUT_SLICE,
        true,
        true,
        0,
        4,
    ));
    result.push(tensor_type(
        T_LANE,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    ));
    result.push(tensor_reference_type(
        T_LANE_REF, T_LANE, false, false, 4, 4,
    ));
    result.push(tensor_type(
        T_CONTEXT,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ));
    result.push(tensor_reference_type(
        T_CONTEXT_REF,
        T_CONTEXT,
        false,
        false,
        0,
        1,
    ));
    result.push(tensor_type(
        T_VIEW,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(48),
            8,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 16, 24, 32, 40], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![T_INPUT, U64, U64, U64, U64]).unwrap(),
        ),
    ));
    result.push(tensor_reference_type(
        T_VIEW_REF, T_VIEW, false, false, 48, 8,
    ));
    result.push(tensor_enum_type(
        T_ERROR,
        24,
        vec![vec![], vec![], vec![U64, U64]],
        vec![vec![], vec![], vec![8, 16]],
    ));
    result.push(tensor_enum_type(
        T_RESULT,
        56,
        vec![vec![T_VIEW], vec![T_ERROR]],
        vec![vec![8], vec![8]],
    ));
    result.push(tensor_array_type(T_A, T_U16, 2, 4));
    result.push(tensor_array_type(T_B, T_U16, 2, 4));
    result.push(tensor_array_type(T_ACC, T_F32, 4, 4));
    result.push(tensor_array_type(T_VALUES, T_F32, 4, 4));
    result
}

fn tensor_abi_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    let plain = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let (pointee_size, pointee_align) = match ty {
        T_INPUT => (0, Some(2)),
        T_LANE_REF => (4, Some(4)),
        T_VIEW_REF => (48, Some(8)),
        T_CONTEXT_REF => (0, None),
        _ => (0, None),
    };
    let reference = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        pointee_size,
        pointee_align,
    )
    .unwrap();
    let mode = match ty {
        UNIT | T_CONTEXT => SemanticAbiPassModeV1::Ignore,
        T_INPUT => SemanticAbiPassModeV1::Pair {
            first: reference,
            second: plain,
        },
        T_OUTPUT => SemanticAbiPassModeV1::Pair {
            first: SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(true, None, true, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                Some(4),
            )
            .unwrap(),
            second: plain,
        },
        T_LANE_REF | T_CONTEXT_REF | T_VIEW_REF => SemanticAbiPassModeV1::Direct(reference),
        T_A | T_B => SemanticAbiPassModeV1::cast(
            false,
            SemanticAbiCastV1::new(
                [None; 8],
                None,
                SemanticAbiUniformV1::new(
                    SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8).unwrap(),
                    8,
                )
                .unwrap(),
                SemanticAbiValueAttributesV1::plain(),
            ),
        ),
        T_VIEW | T_ERROR | T_RESULT | T_ACC | T_VALUES => {
            let (size, align) = match ty {
                T_VIEW => (48, 8),
                T_ERROR => (24, 8),
                T_RESULT => (56, 8),
                T_ACC | T_VALUES => (16, 4),
                _ => unreachable!(),
            };
            SemanticAbiPassModeV1::Indirect {
                attributes: SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        true,
                        Some(SemanticAbiPointerCaptureV1::CapturesNone),
                        true,
                        false,
                        false,
                        true,
                    ),
                    SemanticAbiExtensionV1::None,
                    size,
                    Some(align),
                )
                .unwrap(),
                metadata_attributes: None,
                on_stack: false,
            }
        }
        _ => SemanticAbiPassModeV1::Direct(plain),
    };
    SemanticAbiValueV1::new(ty, mode)
}

#[test]
fn tensor_context_reference_abi_omits_unit_alignment_attribute() {
    let types = tensor_types();
    let context = &types[T_CONTEXT.index() as usize];
    assert_eq!(context.layout().size_bytes(), Some(0));
    assert_eq!(context.layout().alignment_bytes(), 1);
    let value = tensor_abi_value(T_CONTEXT_REF);
    let SemanticAbiPassModeV1::Direct(attributes) = value.mode() else {
        panic!("context reference retains direct scalar ABI")
    };
    assert_eq!(attributes.pointee_size_bytes(), 0);
    assert_eq!(attributes.pointee_alignment_bytes(), None);
    assert_eq!(
        attributes.regular(),
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        )
    );
}

fn tensor_borrow(
    destination: u32,
    ty: SemanticTypeIdV1,
    source: u32,
    pointee: SemanticTypeIdV1,
) -> SemanticStatementV1 {
    assignment(
        destination,
        ty,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: place(source, pointee),
        },
    )
}

fn tensor_payload(source: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Move(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(source),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(0), T_RESULT).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), T_VIEW).unwrap(),
            ],
            T_VIEW,
        )
        .unwrap(),
    )
}

#[derive(Clone, Copy, Debug)]
enum TensorSourceFaultV1 {
    None,
    ReusedLaneBorrow,
    UnissuedLane,
    UnissuedContext,
    MissingResultSuccess,
    WrongResultSuccess,
    SharedResultSuccessTarget,
    OverwrittenResultAfterSuccess,
    AliasedResultAfterSuccess,
    MissingRhsProducer,
}

fn genuine_tensor_source_v1(
    fault: TensorSourceFaultV1,
) -> Result<ProductionPreRankedKirOwnerV1, Box<dyn std::error::Error>> {
    let operand = |role| SemanticMfmaOperandContractV1 {
        role,
        profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
        register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
        wave_width: 64,
    };
    let accumulator = SemanticMfmaAccumulatorContractV1 {
        profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
        distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
        wave_width: 64,
    };
    let callable = |tag, inputs: &[SemanticTypeIdV1], output, operation| {
        neutral_compiler_intrinsic_callable_v1(
            tag,
            inputs.iter().copied().map(tensor_abi_value).collect(),
            tensor_abi_value(output),
            operation,
        )
    };
    let callables = vec![
        SemanticCallableDeclV1::defined(ROOT),
        callable(
            120,
            &[],
            T_CONTEXT,
            SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { context: T_CONTEXT },
        ),
        callable(
            121,
            &[],
            T_LANE,
            SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
                lane: T_LANE,
                wave_width: 64,
            },
        ),
        callable(
            122,
            &[T_INPUT, U64, U64, U64, U64],
            T_RESULT,
            SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewRowMajor {
                result: T_RESULT,
                view: T_VIEW,
                error: T_ERROR,
                role: SemanticMfmaOperandRoleV1::A,
                storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            },
        ),
        callable(
            123,
            &[T_VIEW_REF, T_LANE_REF, U64, U64],
            T_A,
            SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 {
                fragment: T_A,
                view: T_VIEW,
                lane: T_LANE,
                contract: operand(SemanticMfmaOperandRoleV1::A),
                storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            },
        ),
        callable(
            124,
            &[T_INPUT, U64, U64, U64, U64],
            T_RESULT,
            SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewRowMajor {
                result: T_RESULT,
                view: T_VIEW,
                error: T_ERROR,
                role: SemanticMfmaOperandRoleV1::B,
                storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            },
        ),
        callable(
            125,
            &[T_VIEW_REF, T_LANE_REF, U64, U64],
            T_B,
            SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 {
                fragment: T_B,
                view: T_VIEW,
                lane: T_LANE,
                contract: operand(SemanticMfmaOperandRoleV1::B),
                storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            },
        ),
        callable(
            126,
            &[T_LANE_REF],
            T_ACC,
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
                lane: T_LANE,
                fragment: T_ACC,
                contract: accumulator,
            },
        ),
        callable(
            127,
            &[T_CONTEXT_REF, T_A, T_B, T_ACC],
            T_ACC,
            SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                context: T_CONTEXT,
                lhs_fragment: T_A,
                rhs_fragment: T_B,
                accumulator_fragment: T_ACC,
                lhs: operand(SemanticMfmaOperandRoleV1::A),
                rhs: operand(SemanticMfmaOperandRoleV1::B),
                accumulator,
            },
        ),
        callable(
            128,
            &[T_ACC],
            T_VALUES,
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                fragment: T_ACC,
                values: T_VALUES,
            },
        ),
        callable(
            129,
            &[],
            U32,
            SemanticCompilerIntrinsicOperationV1::ThreadIndex(SemanticAxisV1::X),
        ),
    ];
    let view_arguments = |local| {
        vec![
            value(local, T_INPUT),
            constant(U64, 0, 8),
            constant(U64, 16, 8),
            constant(U64, 16, 8),
            constant(U64, 16, 8),
        ]
    };
    let discriminate = |carrier, destination| {
        assignment(
            destination,
            U32,
            SemanticRvalueKindV1::Discriminant(place(carrier, T_RESULT)),
        )
    };
    let success = |discriminant, yes| SemanticTerminatorKindV1::SwitchInt {
        discriminant: value(discriminant, U32),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, yes),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, 13),
        )
        .unwrap(),
    };
    let store = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(3),
                vec![
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Dereference,
                        T_OUTPUT_SLICE,
                    )
                    .unwrap(),
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(23)),
                        T_F32,
                    )
                    .unwrap(),
                ],
                T_F32,
            )
            .unwrap(),
            SemanticRvalueV1::new(
                T_F32,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(20),
                        vec![
                            SemanticProjectionV1::new(
                                SemanticProjectionKindV1::ConstantIndex {
                                    offset: 0,
                                    minimum_length: 4,
                                    from_end: false,
                                },
                                T_F32,
                            )
                            .unwrap(),
                        ],
                        T_F32,
                    )
                    .unwrap(),
                )),
            ),
        )),
    );
    let mut blocks = vec![
        block(200, vec![], pipeline_call(1, vec![], (4, T_CONTEXT), 1)),
        block(201, vec![], pipeline_call(2, vec![], (5, T_LANE), 2)),
        block(
            202,
            vec![],
            pipeline_call(3, view_arguments(1), (6, T_RESULT), 3),
        ),
        block(203, vec![discriminate(6, 7)], success(7, 4)),
        block(
            204,
            vec![
                assignment(8, T_VIEW, SemanticRvalueKindV1::Use(tensor_payload(6))),
                tensor_borrow(9, T_VIEW_REF, 8, T_VIEW),
                tensor_borrow(10, T_LANE_REF, 5, T_LANE),
            ],
            pipeline_call(
                4,
                vec![
                    value(9, T_VIEW_REF),
                    value(10, T_LANE_REF),
                    constant(U64, 0, 8),
                    constant(U64, 0, 8),
                ],
                (11, T_A),
                5,
            ),
        ),
        block(
            205,
            vec![],
            pipeline_call(5, view_arguments(2), (12, T_RESULT), 6),
        ),
        block(206, vec![discriminate(12, 13)], success(13, 7)),
        block(
            207,
            vec![
                assignment(14, T_VIEW, SemanticRvalueKindV1::Use(tensor_payload(12))),
                tensor_borrow(15, T_VIEW_REF, 14, T_VIEW),
                tensor_borrow(10, T_LANE_REF, 5, T_LANE),
            ],
            pipeline_call(
                6,
                vec![
                    value(15, T_VIEW_REF),
                    value(10, T_LANE_REF),
                    constant(U64, 0, 8),
                    constant(U64, 0, 8),
                ],
                (16, T_B),
                8,
            ),
        ),
        block(
            208,
            vec![tensor_borrow(10, T_LANE_REF, 5, T_LANE)],
            pipeline_call(7, vec![value(10, T_LANE_REF)], (17, T_ACC), 9),
        ),
        block(
            209,
            vec![tensor_borrow(18, T_CONTEXT_REF, 4, T_CONTEXT)],
            pipeline_call(
                8,
                vec![
                    value(18, T_CONTEXT_REF),
                    SemanticOperandV1::Move(place(11, T_A)),
                    SemanticOperandV1::Move(place(16, T_B)),
                    SemanticOperandV1::Move(place(17, T_ACC)),
                ],
                (19, T_ACC),
                10,
            ),
        ),
        block(
            210,
            vec![],
            pipeline_call(
                9,
                vec![SemanticOperandV1::Move(place(19, T_ACC))],
                (20, T_VALUES),
                11,
            ),
        ),
        block(211, vec![], pipeline_call(10, vec![], (21, U32), 12)),
        block(
            212,
            vec![
                assignment(
                    22,
                    U64,
                    SemanticRvalueKindV1::Unary {
                        operation: SemanticUnaryOpV1::PointerMetadata,
                        operand: value(3, T_OUTPUT),
                    },
                ),
                assignment(
                    23,
                    U64,
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand: value(21, U32),
                    },
                ),
                assignment(
                    24,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: value(23, U64),
                        right: value(22, U64),
                    },
                ),
            ],
            SemanticTerminatorKindV1::Assert {
                condition: value(24, BOOL),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: value(22, U64),
                    index: value(23, U64),
                },
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 14),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(213, vec![], SemanticTerminatorKindV1::Return),
        block(214, vec![store], SemanticTerminatorKindV1::Return),
    ];
    match fault {
        TensorSourceFaultV1::None => {}
        TensorSourceFaultV1::ReusedLaneBorrow => {
            // Recreate the rejected one-borrow/three-consumer source shape.
            for index in [7, 8] {
                let original = &blocks[index];
                blocks[index] = block(
                    200 + index as u8,
                    original.statements()[..original.statements().len() - 1].to_vec(),
                    original.terminator().kind().clone(),
                );
            }
        }
        TensorSourceFaultV1::UnissuedContext => {
            blocks[0] = block(
                200,
                vec![assignment(
                    4,
                    T_CONTEXT,
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Aggregate, vec![])
                            .unwrap(),
                    ),
                )],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
            );
        }
        TensorSourceFaultV1::UnissuedLane => {
            blocks[1] = block(
                201,
                vec![assignment(
                    5,
                    T_LANE,
                    SemanticRvalueKindV1::Use(constant(T_LANE, 0, 4)),
                )],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2)),
            );
        }
        TensorSourceFaultV1::MissingResultSuccess => {
            blocks[3] = block(
                203,
                vec![],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 4)),
            );
        }
        TensorSourceFaultV1::WrongResultSuccess
        | TensorSourceFaultV1::SharedResultSuccessTarget
        | TensorSourceFaultV1::OverwrittenResultAfterSuccess => {
            let (case, target, otherwise) = match fault {
                TensorSourceFaultV1::WrongResultSuccess => (1, 4, 13),
                TensorSourceFaultV1::SharedResultSuccessTarget => (0, 4, 4),
                TensorSourceFaultV1::OverwrittenResultAfterSuccess => {
                    blocks.push(block(
                        215,
                        vec![],
                        pipeline_call(3, view_arguments(1), (6, T_RESULT), 4),
                    ));
                    (0, 15, 13)
                }
                _ => unreachable!("matched Result branch faults"),
            };
            blocks[3] = block(
                203,
                vec![discriminate(6, 7)],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: value(7, U32),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            case,
                            edge(SemanticEdgeRoleV1::SwitchValue, target),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise),
                    )
                    .unwrap(),
                },
            );
        }
        TensorSourceFaultV1::AliasedResultAfterSuccess => {
            let mut statements = blocks[4].statements().to_vec();
            statements[0] = assignment(8, T_VIEW, SemanticRvalueKindV1::Use(tensor_payload(12)));
            statements.insert(
                0,
                assignment(12, T_RESULT, SemanticRvalueKindV1::Use(value(6, T_RESULT))),
            );
            blocks[4] = block(204, statements, blocks[4].terminator().kind().clone());
        }
        TensorSourceFaultV1::MissingRhsProducer => {
            blocks[7] = block(
                207,
                vec![],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 8)),
            );
        }
    }
    let local_types = [
        UNIT,
        T_INPUT,
        T_INPUT,
        T_OUTPUT,
        T_CONTEXT,
        T_LANE,
        T_RESULT,
        U32,
        T_VIEW,
        T_VIEW_REF,
        T_LANE_REF,
        T_A,
        T_RESULT,
        U32,
        T_VIEW,
        T_VIEW_REF,
        T_B,
        T_ACC,
        T_CONTEXT_REF,
        T_ACC,
        T_VALUES,
        U32,
        U64,
        U64,
        BOOL,
    ];
    let source = SemanticSourceProvenanceV1::unavailable();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([180; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        3,
        [T_INPUT, T_INPUT, T_OUTPUT]
            .into_iter()
            .map(tensor_abi_value)
            .map(SemanticAbiArgumentV1::source)
            .collect(),
        tensor_abi_value(UNIT),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
    ])
    .unwrap();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([181; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([182; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([183; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([184; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([185; 32]),
        source,
        abi,
        local_types
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([70 + index as u8; 32]),
                    ty,
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else if index <= 3 {
                        SemanticLocalRoleV1::Argument(index as u32 - 1)
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"native_tensor_contract_source".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([186; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        tensor_types(),
        vec![],
        vec![],
        vec![],
        vec![function],
        callables,
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())?;
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())?;
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())?;
    let launch = fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[
            fe2o3_lower_mir_kernel::ProductionSourceLaunchRootInputV1::new(
                "native_tensor_contract",
                [186; 32],
                fe2o3_lower_mir_kernel::ProductionSourceLaunchInputV1::new(
                    1,
                    Some([64, 1, 1]),
                    [1, 1, 1],
                ),
            ),
        ],
    )
    .unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
    Ok(ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )?)
}
