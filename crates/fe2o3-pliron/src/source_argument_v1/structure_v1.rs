struct ParameterStructureNodeV1<'a> {
    ty: SemanticTypeIdV1,
    path: &'a [ProductionArgumentProjectionV1],
    physical: std::ops::Range<usize>,
    represented_leaf: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterLeafPolicyV1 {
    PointerFree,
    SharedSliceLeaves,
    // Validation only: nominal pointer words must not become physical arguments.
    ExecutionAbiWords,
}

#[derive(Clone, Copy)]
pub enum ParameterAbiLeafV1 {
    Scalar(SemanticBackendScalarV1),
    SharedSlicePair {
        first: SemanticBackendScalarV1,
        second: SemanticBackendScalarV1,
    },
}

impl ParameterAbiLeafV1 {
    pub fn words(self) -> impl Iterator<Item = (u64, SemanticBackendScalarV1)> {
        match self {
            Self::Scalar(scalar) => [Some((0, scalar)), None],
            Self::SharedSlicePair { first, second } => [Some((0, first)), Some((8, second))],
        }
        .into_iter()
        .flatten()
    }
}

#[allow(clippy::too_many_arguments)]
fn append_parameter_structure_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    policy: ParameterLeafPolicyV1,
    path: &mut Vec<ProductionArgumentProjectionV1>,
    output: &mut Vec<ByValueKernelParameterComponentV1>,
    structural_nodes: &mut usize,
    offset: u64,
    visit: &mut impl FnMut(ParameterStructureNodeV1<'_>) -> Result<(), ProductionSourceArgumentErrorV1>,
) -> Result<(), ProductionSourceArgumentErrorV1> {
    *structural_nodes = structural_nodes
        .checked_add(1)
        .ok_or_else(|| unsupported(0, None, None, "by-value argument structure overflows"))?;
    if *structural_nodes > MAX_SSA_VALUE_COMPONENTS_V1 || output.len() > MAX_SSA_VALUE_COMPONENTS_V1
    {
        return Err(unsupported(
            0,
            None,
            None,
            "by-value argument exceeds the component limit",
        ));
    }
    let declaration = types
        .get(ty.index() as usize)
        .ok_or_else(|| unsupported(0, None, None, "by-value argument type is missing"))?;
    if declaration.layout().is_uninhabited() || declaration.layout().size_bytes().is_none() {
        return Err(unsupported(
            0,
            None,
            None,
            "by-value argument has an uninhabited or unsized layout",
        ));
    }
    let first = output.len();
    match declaration.shape() {
        SemanticTypeShapeV1::Unit => Ok(()),
        SemanticTypeShapeV1::Scalar(_)
        | SemanticTypeShapeV1::ValidityScalar(_)
        | SemanticTypeShapeV1::Pointer(_) => {
            let (kir, leaf) = match declaration.shape() {
                SemanticTypeShapeV1::Pointer(_)
                    if policy == ParameterLeafPolicyV1::ExecutionAbiWords
                        && execution_cfg_nominal_kind_v29(types, ty)? == Some(true) =>
                {
                    let scalar = execution_reference_abi_scalar_v29(types, ty)?;
                    (
                        Type::Scalar(ScalarType::U64),
                        ParameterAbiLeafV1::Scalar(scalar),
                    )
                }
                SemanticTypeShapeV1::Pointer(_)
                    if matches!(
                        policy,
                        ParameterLeafPolicyV1::SharedSliceLeaves
                            | ParameterLeafPolicyV1::ExecutionAbiWords
                    ) && shared_slice_leaf_v1(types, ty) =>
                {
                    let SemanticBackendReprV1::ScalarPair { first, second } =
                        declaration.layout().backend_repr()
                    else {
                        return Err(unsupported(
                            0,
                            None,
                            None,
                            "shared slice leaf lacks a scalar-pair carrier",
                        ));
                    };
                    (
                        lower_parameter_type(types, &[], ty)?,
                        ParameterAbiLeafV1::SharedSlicePair {
                            first: *first,
                            second: *second,
                        },
                    )
                }
                SemanticTypeShapeV1::Pointer(_) => {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "embedded pointer kernel arguments have no owned region binding",
                    ));
                }
                _ => {
                    let SemanticBackendReprV1::Scalar(scalar) = declaration.layout().backend_repr()
                    else {
                        return Err(unsupported(
                            0,
                            None,
                            None,
                            "by-value scalar leaf lacks exact scalar backend representation",
                        ));
                    };
                    (
                        lower_scalar_type(types, ty)?,
                        ParameterAbiLeafV1::Scalar(*scalar),
                    )
                }
            };
            let mut retained_path = Vec::new();
            retained_path.try_reserve_exact(path.len()).map_err(|_| {
                ProductionSourceArgumentErrorV1::AllocationFailure {
                    resource: ProductionSourceArgumentResourceV1::DebugBindings,
                }
            })?;
            for part in path.iter().copied() {
                retained_path.push(match part {
                    ProductionArgumentProjectionV1::Field(field) => {
                        SemanticKirParameterProjectionV1::Field(field)
                    }
                    ProductionArgumentProjectionV1::ArrayIndex(index) => {
                        SemanticKirParameterProjectionV1::ArrayIndex(u32::try_from(index).map_err(
                            |_| {
                                unsupported(
                                    0,
                                    None,
                                    None,
                                    "array index does not fit the emission wire",
                                )
                            },
                        )?)
                    }
                });
            }
            output.try_reserve(1).map_err(|_| {
                ProductionSourceArgumentErrorV1::AllocationFailure {
                    resource: ProductionSourceArgumentResourceV1::DebugBindings,
                }
            })?;
            output.push((retained_path, ty, kir, offset, leaf));
            Ok(())
        }
        SemanticTypeShapeV1::Array { element, length } => {
            let length = usize::try_from(*length).map_err(|_| {
                unsupported(
                    0,
                    None,
                    None,
                    "by-value array length does not fit this host",
                )
            })?;
            if length > MAX_SSA_VALUE_COMPONENTS_V1 {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "by-value array exceeds the component limit",
                ));
            }
            let SemanticFieldsShapeV1::Array {
                stride_bytes,
                count,
            } = declaration.layout().fields()
            else {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "by-value array lacks exact rustc field-stride evidence",
                ));
            };
            if usize::try_from(*count) != Ok(length) {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "by-value array length disagrees with rustc layout",
                ));
            }
            for index in 0..length {
                path.try_reserve(1).map_err(|_| {
                    ProductionSourceArgumentErrorV1::AllocationFailure {
                        resource: ProductionSourceArgumentResourceV1::DebugBindings,
                    }
                })?;
                path.push(ProductionArgumentProjectionV1::ArrayIndex(index as u64));
                let element_offset = stride_bytes
                    .checked_mul(index as u64)
                    .and_then(|relative| offset.checked_add(relative))
                    .ok_or_else(|| unsupported(0, None, None, "by-value array offset overflows"))?;
                append_parameter_structure_v1(
                    types,
                    *element,
                    policy,
                    path,
                    output,
                    structural_nodes,
                    element_offset,
                    visit,
                )?;
                path.pop();
            }
            Ok(())
        }
        SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
            let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details()
            else {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "by-value aggregate lacks exact rustc field-offset evidence",
                ));
            };
            if layout.field_offsets().len() != fields.fields().len() {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "by-value aggregate field count disagrees with rustc layout",
                ));
            }
            for (index, field) in fields.fields().iter().copied().enumerate() {
                path.try_reserve(1).map_err(|_| {
                    ProductionSourceArgumentErrorV1::AllocationFailure {
                        resource: ProductionSourceArgumentResourceV1::DebugBindings,
                    }
                })?;
                path.push(ProductionArgumentProjectionV1::Field(
                    u32::try_from(index).map_err(|_| {
                        unsupported(0, None, None, "aggregate field does not fit the wire")
                    })?,
                ));
                let field_offset = offset
                    .checked_add(layout.field_offsets()[index])
                    .ok_or_else(|| unsupported(0, None, None, "by-value field offset overflows"))?;
                append_parameter_structure_v1(
                    types,
                    field,
                    policy,
                    path,
                    output,
                    structural_nodes,
                    field_offset,
                    visit,
                )?;
                path.pop();
            }
            Ok(())
        }
        SemanticTypeShapeV1::Enum { .. } => Err(unsupported(
            0,
            None,
            None,
            "by-value enum kernel arguments require variant-aware packing evidence",
        )),
        _ => Err(unsupported(
            0,
            None,
            None,
            "kernel argument has no pointer-free aggregate component representation",
        )),
    }?;
    visit(ParameterStructureNodeV1 {
        ty,
        path,
        physical: first..output.len(),
        represented_leaf: matches!(
            declaration.shape(),
            SemanticTypeShapeV1::Scalar(_)
                | SemanticTypeShapeV1::ValidityScalar(_)
                | SemanticTypeShapeV1::Pointer(_)
        ),
    })
}
