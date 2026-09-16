#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArgumentHelperResultShape {
    Zero,
    Singleton,
    Pair,
    Nominal,
    Nested,
    Array(u64),
}

fn argument_result_owner(
    expanded: bool,
    shape: ArgumentHelperResultShape,
) -> ProductionSemanticSsaOwnerV1 {
    argument_call_owner_with_result(
        expanded,
        ArgumentTupleShape::Mixed,
        true,
        true,
        true,
        ArgumentCallResult::Scalar,
        true,
        Some(shape),
    )
}

fn argument_result_declaration(
    shape: ArgumentHelperResultShape,
    types: &mut Vec<SemanticTypeDeclV1>,
) -> SemanticTypeIdV1 {
    let length = match shape {
        ArgumentHelperResultShape::Zero => return ZERO,
        ArgumentHelperResultShape::Pair => return PAIR,
        ArgumentHelperResultShape::Nested => return TUPLE,
        ArgumentHelperResultShape::Nominal => {
            let id = SemanticTypeIdV1::from_index(types.len() as u32);
            let pair = &types[PAIR.index() as usize];
            let SemanticTypeShapeV1::Tuple(fields) = pair.shape() else {
                unreachable!()
            };
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([181; 32]),
                SemanticLayoutIdentityV1::from_sha256([181; 32]),
                pair.layout().clone(),
                SemanticTypeShapeV1::Aggregate(fields.clone()),
            ));
            return id;
        }
        ArgumentHelperResultShape::Singleton => 1,
        ArgumentHelperResultShape::Array(length) => length,
    };
    let id = SemanticTypeIdV1::from_index(types.len() as u32);
    let (fields, backend, declaration) = if shape == ArgumentHelperResultShape::Singleton {
        (
            SemanticFieldsShapeV1::arbitrary(vec![0, 4], vec![0, 1]).unwrap(),
            *types[U32.index() as usize].layout().backend_repr(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, UNIT]).unwrap()),
        )
    } else {
        (
            SemanticFieldsShapeV1::array(4, length),
            SemanticBackendReprV1::memory(true),
            SemanticTypeShapeV1::Array {
                element: U32,
                length,
            },
        )
    };
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([180; 32]),
        SemanticLayoutIdentityV1::from_sha256([180; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            4 * length,
            4,
            fields,
            SemanticRustcVariantsV1::Single { index: 0 },
            backend,
            None,
            false,
            None,
            4,
            4 * length,
            if shape == ArgumentHelperResultShape::Singleton {
                SemanticTypeLayoutDetailsV1::Aggregate(
                    SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
                )
            } else {
                SemanticTypeLayoutDetailsV1::None
            },
        )
        .unwrap(),
        declaration,
    ));
    id
}

fn argument_result_abi(
    shape: ArgumentHelperResultShape,
    ty: SemanticTypeIdV1,
    attrs: SemanticAbiValueAttributesV1,
) -> SemanticAbiValueV1 {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiCastV1, SemanticAbiRegisterV1, SemanticAbiUniformV1,
    };
    let mode = match shape {
        ArgumentHelperResultShape::Zero | ArgumentHelperResultShape::Array(0) => {
            SemanticAbiPassModeV1::Ignore
        }
        ArgumentHelperResultShape::Singleton => SemanticAbiPassModeV1::Direct(attrs),
        ArgumentHelperResultShape::Pair | ArgumentHelperResultShape::Nominal => {
            SemanticAbiPassModeV1::Pair {
                first: attrs,
                second: attrs,
            }
        }
        ArgumentHelperResultShape::Array(length @ 1..=2) => SemanticAbiPassModeV1::Cast {
            pad_i32: false,
            cast: SemanticAbiCastV1::new(
                [None; 8],
                None,
                SemanticAbiUniformV1::new(
                    SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 4 * length)
                        .unwrap(),
                    4 * length,
                )
                .unwrap(),
                SemanticAbiValueAttributesV1::plain(),
            ),
        },
        ArgumentHelperResultShape::Nested | ArgumentHelperResultShape::Array(_) => {
            let size = match shape {
                ArgumentHelperResultShape::Array(length) => length * 4,
                _ => 12,
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
                    Some(4),
                )
                .unwrap(),
                metadata_attributes: None,
                on_stack: false,
            }
        }
    };
    SemanticAbiValueV1::new(ty, mode)
}

fn argument_result_value(
    shape: ArgumentHelperResultShape,
    ty: SemanticTypeIdV1,
    scalar: SemanticPlaceV1,
    pair: SemanticPlaceV1,
) -> SemanticRvalueV1 {
    let zero = |ty| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty,
            SemanticConstantValueV1::ZeroSized,
        ))
    };
    let scalar = SemanticOperandV1::Copy(scalar);
    let (kind, operands) = match shape {
        ArgumentHelperResultShape::Zero => {
            return SemanticRvalueV1::new(ty, SemanticRvalueKindV1::Use(zero(ty)));
        }
        ArgumentHelperResultShape::Singleton => {
            (SemanticAggregateKindV1::Tuple, vec![scalar, zero(UNIT)])
        }
        ArgumentHelperResultShape::Pair | ArgumentHelperResultShape::Nominal => (
            if shape == ArgumentHelperResultShape::Nominal {
                SemanticAggregateKindV1::Aggregate
            } else {
                SemanticAggregateKindV1::Tuple
            },
            vec![scalar.clone(), zero(UNIT), scalar],
        ),
        ArgumentHelperResultShape::Nested => (
            SemanticAggregateKindV1::Tuple,
            vec![SemanticOperandV1::Copy(pair), zero(ZERO), scalar],
        ),
        ArgumentHelperResultShape::Array(length) => (
            SemanticAggregateKindV1::Array,
            vec![scalar; length as usize],
        ),
    };
    SemanticRvalueV1::new(
        ty,
        SemanticRvalueKindV1::Aggregate(SemanticAggregateRvalueV1::new(kind, operands).unwrap()),
    )
}
