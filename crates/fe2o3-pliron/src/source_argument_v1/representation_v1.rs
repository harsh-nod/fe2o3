pub fn shared_slice_helper_parameter_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    argument: u32,
    ty: SemanticTypeIdV1,
) -> bool {
    let abi = function.abi();
    let argument = argument as usize;
    let Some(value) = abi.adjusted_arguments().get(argument) else {
        return false;
    };
    if abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1::Rust
        || abi.source_input_types().get(argument) != Some(&ty)
        || abi.source_argument_ownership().get(argument)
            != Some(&SemanticSourceArgumentOwnershipV1::SharedBorrow)
        || value.role() != SemanticAbiArgumentRoleV1::Source
        || value.ty() != ty
        || value.value().adjusted().is_some()
        || value.value().pointee_override().is_some()
        || !matches!(value.mode(), SemanticAbiPassModeV1::Pair { .. })
    {
        return false;
    }
    shared_slice_leaf_v1(types, ty)
}

pub fn shared_slice_leaf_v1(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    let Some(declaration) = types.get(ty.index() as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
        return false;
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Immutable
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || pointer.metadata() != SemanticPointerMetadataV1::SliceLength
    {
        return false;
    }
    let Some(SemanticTypeShapeV1::Slice { element }) = types
        .get(pointer.pointee().index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return false;
    };
    let SemanticBackendReprV1::ScalarPair { first, second } = declaration.layout().backend_repr()
    else {
        return false;
    };
    if declaration.layout().size_bytes() != Some(16)
        || declaration.layout().alignment_bytes() != 8
        || !matches!(
            first.primitive(),
            SemanticBackendPrimitiveV1::Pointer {
                address_space: 0,
                size_bytes: 8,
                alignment_bytes: 8
            }
        )
        || !matches!(
            second.primitive(),
            SemanticBackendPrimitiveV1::Integer {
                bits: 64,
                signed: false,
                alignment_bytes: 8
            }
        )
    {
        return false;
    }
    // Full source/type/ABI admission precedes this representation selection.
    matches!(
        types
            .get(element.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
    )
}

pub fn lower_parameter_type(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Type, ProductionSourceArgumentErrorV1> {
    let declaration = types
        .get(usize::try_from(ty.index()).unwrap_or(usize::MAX))
        .ok_or_else(|| unsupported(0, None, None, "kernel argument type is missing"))?;
    require_ordinary_execution_representation_v29(declaration)?;
    let shape = declaration.shape();
    if let Some((element, _, access)) = disjoint_slice_descriptor(callables, ty) {
        return Ok(Type::slice(
            lower_scalar_type(types, element)?,
            AddressSpace::Global,
            access,
        ));
    }
    match shape {
        SemanticTypeShapeV1::Pointer(pointer) => {
            let access = match pointer.mutability() {
                SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
                SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
            };
            let address_space = lower_address_space(pointer.address_space())?;
            match pointer.metadata() {
                SemanticPointerMetadataV1::None => Ok(Type::pointer(
                    lower_parameter_memory_element_v1(types, pointer.pointee())?,
                    address_space,
                    access,
                )),
                SemanticPointerMetadataV1::SliceLength => {
                    let pointee =
                        types
                            .get(pointer.pointee().index() as usize)
                            .ok_or_else(|| {
                                unsupported(0, None, None, "slice pointee type is missing")
                            })?;
                    let SemanticTypeShapeV1::Slice { element } = pointee.shape() else {
                        return Err(unsupported(
                            0,
                            None,
                            None,
                            "slice-length pointer metadata has a non-slice pointee",
                        ));
                    };
                    Ok(Type::slice(
                        lower_parameter_scalar_v1(types, *element)?,
                        address_space,
                        access,
                    ))
                }
                SemanticPointerMetadataV1::VTable => Err(unsupported(
                    0,
                    None,
                    None,
                    "vtable-bearing kernel arguments are unsupported",
                )),
            }
        }
        SemanticTypeShapeV1::Scalar(_) => Ok(lower_scalar_type(types, ty)?),
        _ => Err(unsupported(
            0,
            None,
            None,
            "kernel argument type has no authenticated Kernel IR representation",
        )),
    }
}

pub fn memory_element_type_v1(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> Option<Type> {
    if matches!(
        types
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
    ) {
        return lower_scalar_type(types, ty).ok();
    }
    transparent_scalar_storage_type(types, ty)
}

pub fn transparent_scalar_storage_type(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<Type> {
    let mut current = ty;
    let mut visited = BTreeSet::new();
    loop {
        if !visited.insert(current) || visited.len() > MAX_SSA_VALUE_COMPONENTS_V1 {
            return None;
        }
        if matches!(
            types
                .get(current.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
        ) {
            return lower_scalar_type(types, current).ok();
        }
        let declaration = types.get(current.index() as usize)?;
        let SemanticTypeShapeV1::Aggregate(aggregate) = declaration.shape() else {
            return None;
        };
        let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
            return None;
        };
        if declaration.layout().is_uninhabited()
            || aggregate.fields().len() != 1
            || layout.field_offsets() != [0]
            || !layout.padding().is_empty()
        {
            return None;
        }
        let field_ty = aggregate.fields()[0];
        let field = types.get(field_ty.index() as usize)?;
        if field.layout().is_uninhabited()
            || declaration.layout().size_bytes() != field.layout().size_bytes()
            || declaration.layout().alignment_bytes() != field.layout().alignment_bytes()
        {
            return None;
        }
        current = field_ty;
    }
}

pub fn lower_kernel_parameter_type(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    argument: u32,
    ty: SemanticTypeIdV1,
) -> Result<Type, ProductionSourceArgumentErrorV1> {
    let declaration = types
        .get(usize::try_from(ty.index()).unwrap_or(usize::MAX))
        .ok_or_else(|| unsupported(0, None, None, "kernel argument type is missing"))?;
    if let Some(parameter) =
        authenticated_disjoint_slice_parameter(types, callables, function, argument, ty)
    {
        return Ok(parameter);
    }
    if matches!(declaration.shape(), SemanticTypeShapeV1::Aggregate(_))
        && let Some(parameter) =
            authenticated_global_mut_pointer_parameter(types, function, argument, ty)
    {
        return Ok(parameter);
    }
    if matches!(
        declaration.shape(),
        SemanticTypeShapeV1::Unit
            | SemanticTypeShapeV1::Array { .. }
            | SemanticTypeShapeV1::Tuple(_)
            | SemanticTypeShapeV1::Aggregate(_)
    ) {
        return Err(unsupported(
            0,
            None,
            None,
            "by-value kernel argument requires exact component lowering",
        ));
    }
    lower_parameter_type(types, callables, ty)
}

pub fn lower_by_value_kernel_parameter_components_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    argument: u32,
    ty: SemanticTypeIdV1,
) -> Result<Vec<ByValueKernelParameterComponentV1>, ProductionSourceArgumentErrorV1> {
    let argument = usize::try_from(argument).map_err(|_| {
        unsupported(
            0,
            None,
            None,
            "kernel argument index does not fit this host",
        )
    })?;
    if function.abi().source_argument_ownership().get(argument)
        != Some(&SemanticSourceArgumentOwnershipV1::ByValue)
        || function.abi().source_input_types().get(argument) != Some(&ty)
    {
        return Err(unsupported(
            0,
            None,
            None,
            "aggregate kernel argument lacks exact by-value ABI ownership",
        ));
    }
    let abi = function
        .abi()
        .adjusted_arguments()
        .get(argument)
        .ok_or_else(|| unsupported(0, None, None, "aggregate kernel argument ABI is absent"))?;
    if abi.ty() != ty {
        return Err(unsupported(
            0,
            None,
            None,
            "aggregate kernel argument ABI type changed",
        ));
    }
    lower_by_value_parameter_components_v1(types, function, abi)
}

pub fn lower_by_value_parameter_components_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    abi: &fe2o3_mir_model::semantic_mir_v1::SemanticAbiArgumentV1,
) -> Result<Vec<ByValueKernelParameterComponentV1>, ProductionSourceArgumentErrorV1> {
    lower_by_value_abi_components_v1(
        types,
        function,
        abi.value(),
        ParameterLeafPolicyV1::PointerFree,
    )
}

pub fn lower_by_value_abi_components_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    abi: &fe2o3_mir_model::semantic_mir_v1::SemanticAbiValueV1,
    policy: ParameterLeafPolicyV1,
) -> Result<Vec<ByValueKernelParameterComponentV1>, ProductionSourceArgumentErrorV1> {
    let ty = abi.ty();
    if abi.adjusted().is_some() {
        return Err(unsupported(
            0,
            None,
            None,
            "aggregate kernel argument has an adjusted ABI type",
        ));
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(MAX_SSA_VALUE_COMPONENTS_V1.min(8))
        .map_err(|_| ProductionSourceArgumentErrorV1::AllocationFailure {
            resource: ProductionSourceArgumentResourceV1::DebugBindings,
        })?;
    let mut path = Vec::new();
    let mut structural_nodes = 0;
    append_parameter_structure_v1(
        types,
        ty,
        policy,
        &mut path,
        &mut output,
        &mut structural_nodes,
        0,
        &mut |_| Ok(()),
    )?;
    let source_size = types[ty.index() as usize]
        .layout()
        .size_bytes()
        .ok_or_else(|| unsupported(0, None, None, "by-value argument layout is unsized"))?;
    let mut words = Vec::new();
    words
        .try_reserve_exact(output.len().saturating_mul(2))
        .map_err(|_| ProductionSourceArgumentErrorV1::AllocationFailure {
            resource: ProductionSourceArgumentResourceV1::DebugBindings,
        })?;
    for (_, _, _, offset, leaf) in &output {
        for (relative, scalar) in leaf.words() {
            let offset = offset
                .checked_add(relative)
                .ok_or_else(|| unsupported(0, None, None, "by-value ABI word offset overflows"))?;
            let width = scalar.primitive().size_bytes().ok_or_else(|| {
                unsupported(
                    0,
                    None,
                    None,
                    "by-value scalar component has no exact byte width",
                )
            })?;
            let end = offset.checked_add(width).ok_or_else(|| {
                unsupported(0, None, None, "by-value scalar component range overflows")
            })?;
            if end > source_size {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "by-value scalar component exceeds its source layout",
                ));
            }
            words.push((offset, end, scalar));
        }
    }
    words.sort_unstable_by_key(|word| word.0);
    if words.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(unsupported(
            0,
            None,
            None,
            "by-value scalar component ranges overlap",
        ));
    }
    let exact_mode = match abi.mode() {
        SemanticAbiPassModeV1::Ignore => {
            output.is_empty()
                && types[ty.index() as usize].layout().size_bytes() == Some(0)
                && matches!(
                    types[ty.index() as usize].layout().backend_repr(),
                    SemanticBackendReprV1::Memory { sized: true }
                )
        }
        SemanticAbiPassModeV1::Direct(attributes) => {
            match types[ty.index() as usize].layout().backend_repr() {
                SemanticBackendReprV1::Scalar(root_scalar) => matches!(
                    words.as_slice(),
                    [(0, _, leaf_scalar)] if leaf_scalar == root_scalar
                ),
                SemanticBackendReprV1::Memory { sized: true } => {
                    function.abi().spec_abi_unadjusted()
                        && *attributes
                            == fe2o3_mir_model::semantic_mir_v1::SemanticAbiValueAttributesV1::plain(
                            )
                        && !output.is_empty()
                }
                _ => false,
            }
        }
        SemanticAbiPassModeV1::Pair { .. } => {
            let SemanticBackendReprV1::ScalarPair { first, second } =
                types[ty.index() as usize].layout().backend_repr()
            else {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "pair by-value aggregate lacks scalar-pair ABI representation",
                ));
            };
            let first_size = first.primitive().size_bytes().ok_or_else(|| {
                unsupported(
                    0,
                    None,
                    None,
                    "first aggregate ABI scalar has invalid width",
                )
            })?;
            let alignment = second.primitive().alignment_bytes();
            let second_offset = first_size
                .checked_add(alignment.saturating_sub(1))
                .map(|value| value & !alignment.saturating_sub(1))
                .ok_or_else(|| {
                    unsupported(0, None, None, "aggregate ABI scalar offset overflows")
                })?;
            match words.as_slice() {
                [(0, _, a), (b_offset, _, b)] => {
                    a == first && *b_offset == second_offset && b == second
                }
                _ => false,
            }
        }
        SemanticAbiPassModeV1::Cast { pad_i32, cast } => {
            let layout = types[ty.index() as usize].layout();
            let regular = cast.attributes().regular();
            let exact = matches!(
                function.abi().canon_abi(),
                SemanticCanonAbiV1::Rust
                    | SemanticCanonAbiV1::RustCold
                    | SemanticCanonAbiV1::RustPreserveNone
            ) && matches!(
                layout.backend_repr(),
                SemanticBackendReprV1::Memory { sized: true }
            ) && layout.size_bytes().is_some_and(|size| size != 0)
                && !pad_i32
                && cast.prefix().iter().all(Option::is_none)
                && cast.rest_offset_bytes().is_none()
                && cast.rest().unit().kind() == SemanticAbiRegisterKindV1::Integer
                && layout.size_bytes() == Some(cast.rest().unit().size_bytes())
                && layout.size_bytes() == Some(cast.rest_total_bytes())
                && !cast.rest_consecutive()
                && !regular.no_alias()
                && regular.pointer_capture().is_none()
                && !regular.non_null()
                && !regular.read_only()
                && !regular.in_register()
                && cast.attributes().extension() == SemanticAbiExtensionV1::None
                && cast.attributes().pointee_size_bytes() == 0
                && cast.attributes().pointee_alignment_bytes().is_none()
                && !output.is_empty();
            if !exact {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "aggregate cast ABI is not an exact simple Rust integer transport",
                ));
            }
            true
        }
        SemanticAbiPassModeV1::Indirect {
            attributes,
            metadata_attributes,
            on_stack,
        } => {
            let layout = types[ty.index() as usize].layout();
            let regular = attributes.regular();
            let exact = matches!(
                layout.backend_repr(),
                SemanticBackendReprV1::Memory { sized: true }
            ) && layout.size_bytes().is_some_and(|size| size != 0)
                && metadata_attributes.is_none()
                && !on_stack
                && regular.no_alias()
                && matches!(
                    regular.pointer_capture(),
                    Some(
                        SemanticAbiPointerCaptureV1::CapturesAddress
                            | SemanticAbiPointerCaptureV1::CapturesNone
                    )
                )
                && regular.non_null()
                && regular.no_undef()
                && attributes.extension() == SemanticAbiExtensionV1::None
                && attributes.pointee_size_bytes() == layout.rustc_size_bytes()
                && attributes.pointee_alignment_bytes() == Some(layout.alignment_bytes())
                && !output.is_empty();
            if !exact {
                let reason = if metadata_attributes.is_some() {
                    "metadata-bearing indirect aggregate ABI is unsupported"
                } else if *on_stack {
                    "on-stack indirect aggregate ABI is unsupported"
                } else {
                    "indirect aggregate carrier does not exactly match its sized source layout"
                };
                return Err(unsupported(0, None, None, reason));
            }
            true
        }
    };
    if !exact_mode {
        return Err(unsupported(
            0,
            None,
            None,
            "aggregate kernel argument ABI mode does not match its scalar components",
        ));
    }
    Ok(output)
}

pub fn authenticated_disjoint_slice_parameter(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    argument: u32,
    ty: SemanticTypeIdV1,
) -> Option<Type> {
    let (element, raw_index, access) = disjoint_slice_descriptor(callables, ty)
        .or_else(|| {
            complete_body_parameter_vnext::complete_body_slice_parameter_vnext(
                types, callables, function, argument, ty,
            )
        })
        .or_else(|| {
            physical_entry_parameter_v20::physical_entry_slice_parameter_v20(
                types, callables, function, argument, ty,
            )
        })
        .or_else(|| {
            physical_global_copy_parameter_v21::physical_global_copy_slice_parameter_v21(
                types, callables, function, argument, ty,
            )
        })
        .or_else(|| {
            physical_lds_exchange_parameter_v22::physical_lds_exchange_slice_parameter_v22(
                types, callables, function, argument, ty,
            )
        })?;
    let argument = usize::try_from(argument).ok()?;
    let abi = function.abi();
    if abi.source_input_types().get(argument) != Some(&ty)
        || abi.source_argument_ownership().get(argument)
            != Some(&SemanticSourceArgumentOwnershipV1::ExclusiveOwner)
    {
        return None;
    }
    let abi_argument = abi.adjusted_arguments().get(argument)?;
    if abi_argument.ty() != ty
        || abi_argument.value().adjusted().is_some()
        || !matches!(abi_argument.mode(), SemanticAbiPassModeV1::Pair { .. })
    {
        return None;
    }

    let declaration = types.get(ty.index() as usize)?;
    let SemanticTypeShapeV1::Aggregate(aggregate) = declaration.shape() else {
        return None;
    };
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
        return None;
    };
    let SemanticBackendReprV1::ScalarPair { first, second } = declaration.layout().backend_repr()
    else {
        return None;
    };
    if aggregate.fields().len() != layout.field_offsets().len() {
        return None;
    }

    let mut pointer_field = None;
    let mut length_field = None;
    for (index, (&field_ty, &offset)) in aggregate
        .fields()
        .iter()
        .zip(layout.field_offsets())
        .enumerate()
    {
        let field = types.get(field_ty.index() as usize)?;
        if let SemanticTypeShapeV1::Pointer(pointer) = field.shape()
            && pointer.pointee() == element
            && pointer.kind() == SemanticPointerKindV1::Raw
            && pointer.mutability() == SemanticMutabilityV1::Mutable
            && pointer.address_space() == 0
            && pointer.pointer_width_bits() == 64
            && pointer.metadata() == SemanticPointerMetadataV1::None
        {
            let SemanticBackendReprV1::Scalar(pointer_scalar) = field.layout().backend_repr()
            else {
                return None;
            };
            if pointer_field.replace(index).is_some() || offset != 0 || pointer_scalar != first {
                return None;
            }
        } else if field_ty == raw_index {
            let SemanticBackendReprV1::Scalar(length_scalar) = field.layout().backend_repr() else {
                return None;
            };
            if length_field.replace(index).is_some()
                || offset != 8
                || length_scalar != second
                || !matches!(
                    field.shape(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 64
                    })
                )
            {
                return None;
            }
        } else if field.layout().size_bytes() != Some(0) || field.layout().is_uninhabited() {
            return None;
        }
    }
    pointer_field?;
    length_field?;
    if declaration.layout().size_bytes() != Some(16) || declaration.layout().alignment_bytes() != 8
    {
        return None;
    }

    Some(Type::slice(
        lower_parameter_scalar_v1(types, element).ok()?,
        AddressSpace::Global,
        access,
    ))
}

pub fn authenticated_global_mut_pointer_parameter(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    argument: u32,
    ty: SemanticTypeIdV1,
) -> Option<Type> {
    let argument = usize::try_from(argument).ok()?;
    let abi = function.abi();
    if abi.source_input_types().get(argument) != Some(&ty)
        || abi.source_argument_ownership().get(argument)
            != Some(&SemanticSourceArgumentOwnershipV1::ExclusiveOwner)
    {
        return None;
    }
    let abi_argument = abi.adjusted_arguments().get(argument)?;
    if abi_argument.ty() != ty || !matches!(abi_argument.mode(), SemanticAbiPassModeV1::Direct(_)) {
        return None;
    }

    let declaration = types.get(ty.index() as usize)?;
    let pointee = abi_argument
        .value()
        .pointee_override()
        .or(declaration.abi_properties().first_pointee())?;
    if pointee.kind() != SemanticAbiPointeeKindV1::Raw {
        return None;
    }
    let outer_pointer = scalar_backend_pointer(declaration)?;
    let SemanticTypeShapeV1::Aggregate(aggregate) = declaration.shape() else {
        return None;
    };
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
        return None;
    };
    if aggregate.fields().len() != layout.field_offsets().len() {
        return None;
    }

    let mut physical_pointer = None;
    for (&field_ty, &field_offset) in aggregate.fields().iter().zip(layout.field_offsets()) {
        let field = types.get(field_ty.index() as usize)?;
        if field.layout().size_bytes() == Some(0) {
            if field.layout().is_uninhabited() {
                return None;
            }
            continue;
        }
        if physical_pointer.is_some() || field_offset != 0 {
            return None;
        }
        let SemanticTypeShapeV1::Pointer(pointer) = field.shape() else {
            return None;
        };
        if pointer.kind() != SemanticPointerKindV1::Raw
            || pointer.mutability() != SemanticMutabilityV1::Mutable
            || pointer.metadata() != SemanticPointerMetadataV1::None
            || scalar_backend_pointer(field)? != outer_pointer
            || declaration.layout().size_bytes() != field.layout().size_bytes()
            || declaration.layout().alignment_bytes() != field.layout().alignment_bytes()
        {
            return None;
        }
        physical_pointer = Some(pointer);
    }

    let pointer = physical_pointer?;
    Some(Type::pointer(
        lower_parameter_scalar_v1(types, pointer.pointee()).ok()?,
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ))
}

pub fn scalar_backend_pointer(
    declaration: &SemanticTypeDeclV1,
) -> Option<SemanticBackendPrimitiveV1> {
    let SemanticBackendReprV1::Scalar(scalar) = declaration.layout().backend_repr() else {
        return None;
    };
    let primitive = scalar.primitive();
    matches!(primitive, SemanticBackendPrimitiveV1::Pointer { .. }).then_some(primitive)
}

pub fn require_ordinary_execution_representation_v29(
    declaration: &SemanticTypeDeclV1,
) -> Result<(), ProductionSourceArgumentErrorV1> {
    if matches!(
        declaration.rust_type_kind(),
        fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1::Execution(_)
    ) {
        return Err(unsupported(
            0,
            None,
            None,
            "execution roles require occurrence-bound transport, not an ordinary Rust representation",
        ));
    }
    Ok(())
}

pub fn lower_scalar_type(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Type, ProductionSourceArgumentErrorV1> {
    let shape = types
        .get(usize::try_from(ty.index()).unwrap_or(usize::MAX))
        .ok_or_else(|| unsupported(0, None, None, "scalar type is missing"))?
        .shape();
    let scalar = match shape {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool) => ScalarType::Bool,
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }) => {
            match (*signed, *bits) {
                (true, 8) => ScalarType::I8,
                (true, 16) => ScalarType::I16,
                (true, 32) => ScalarType::I32,
                (true, 64) => ScalarType::I64,
                (true, 128) => ScalarType::I128,
                (false, 8) => ScalarType::U8,
                (false, 16) => ScalarType::U16,
                (false, 32) => ScalarType::U32,
                (false, 64) => ScalarType::U64,
                (false, 128) => ScalarType::U128,
                _ => {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "integer argument width is unsupported",
                    ));
                }
            }
        }
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits }) => match bits {
            16 => ScalarType::F16,
            32 => ScalarType::F32,
            64 => ScalarType::F64,
            _ => {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "floating argument width is unsupported",
                ));
            }
        },
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Char) => ScalarType::U32,
        SemanticTypeShapeV1::ValidityScalar(validity) => {
            return lower_scalar_kind(validity.scalar());
        }
        _ => {
            return Err(ProductionSourceArgumentErrorV1::ScalarTypeUnavailable {
                semantic_type: ty.index(),
                shape: types
                    .get(ty.index() as usize)
                    .map(|declaration| format!("{declaration:?}"))
                    .unwrap_or_else(|| "<missing>".to_owned()),
            });
        }
    };
    Ok(Type::Scalar(scalar))
}

pub fn lower_scalar_kind(
    scalar: SemanticScalarTypeV1,
) -> Result<Type, ProductionSourceArgumentErrorV1> {
    let scalar = match scalar {
        SemanticScalarTypeV1::Bool => ScalarType::Bool,
        SemanticScalarTypeV1::Integer { signed, bits } => match (signed, bits) {
            (true, 8) => ScalarType::I8,
            (true, 16) => ScalarType::I16,
            (true, 32) => ScalarType::I32,
            (true, 64) => ScalarType::I64,
            (true, 128) => ScalarType::I128,
            (false, 8) => ScalarType::U8,
            (false, 16) => ScalarType::U16,
            (false, 32) => ScalarType::U32,
            (false, 64) => ScalarType::U64,
            (false, 128) => ScalarType::U128,
            _ => {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "integer argument width is unsupported",
                ));
            }
        },
        SemanticScalarTypeV1::Float { bits } => match bits {
            16 => ScalarType::F16,
            32 => ScalarType::F32,
            64 => ScalarType::F64,
            _ => {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "floating argument width is unsupported",
                ));
            }
        },
        SemanticScalarTypeV1::Char => ScalarType::U32,
    };
    Ok(Type::Scalar(scalar))
}

pub fn disjoint_slice_descriptor(
    callables: &[SemanticCallableDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<(SemanticTypeIdV1, SemanticTypeIdV1, AccessMode)> {
    let mut descriptor = None;
    for callable in callables {
        let candidate = match callable {
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation:
                    SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                        disjoint_slice,
                        element,
                        raw_index,
                        ..
                    }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
                        disjoint_slice,
                        element,
                        raw_index,
                        ..
                    }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
                        disjoint_slice,
                        element,
                        raw_index,
                        ..
                    }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
                        disjoint_slice,
                        element,
                        raw_index,
                        ..
                    }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut {
                        disjoint_slice,
                        element,
                        raw_index,
                        ..
                    }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut {
                        disjoint_slice,
                        element,
                        raw_index,
                        ..
                    }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
                        disjoint_slice,
                        element,
                        raw_index,
                        ..
                    },
                ..
            } if *disjoint_slice == ty => Some((*element, *raw_index, AccessMode::ReadWrite)),
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation:
                    SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceLen {
                        disjoint_slice,
                        element,
                        raw_index,
                        ..
                    },
                ..
            } if *disjoint_slice == ty => Some((*element, *raw_index, AccessMode::WriteOnly)),
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation:
                    SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                        disjoint_slice,
                        element,
                        raw_index,
                        ..
                    },
                ..
            } if *disjoint_slice == ty => Some((*element, *raw_index, AccessMode::WriteOnly)),
            SemanticCallableDeclV1::Defined { .. }
            | SemanticCallableDeclV1::DeviceFfiImport { .. }
            | SemanticCallableDeclV1::CompilerIntrinsic { .. } => None,
        };
        if let Some(candidate) = candidate {
            if descriptor.is_some_and(|previous| previous != candidate) {
                return None;
            }
            descriptor = Some(candidate);
        }
    }
    descriptor
}

pub fn lower_address_space(
    address_space: u32,
) -> Result<AddressSpace, ProductionSourceArgumentErrorV1> {
    match address_space {
        0 | 1 => Ok(AddressSpace::Global),
        3 => Ok(AddressSpace::Workgroup),
        4 => Ok(AddressSpace::Constant),
        5 => Ok(AddressSpace::Private),
        _ => Err(unsupported(
            0,
            None,
            None,
            "semantic pointer address space is unsupported",
        )),
    }
}

pub fn execution_cfg_error_v29() -> ProductionSourceArgumentErrorV1 {
    unsupported(
        0,
        None,
        None,
        "execution CFG transport differs from its captured SSA state",
    )
}

pub fn execution_cfg_nominal_kind_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<Option<bool>, ProductionSourceArgumentErrorV1> {
    let declaration = types
        .get(ty.index() as usize)
        .ok_or_else(execution_cfg_error_v29)?;
    if matches!(
        declaration.rust_type_kind(),
        fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1::Execution(_)
    ) {
        return Ok(Some(false));
    }
    if let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape()
        && matches!(
            types
                .get(pointer.pointee().index() as usize)
                .map(|ty| ty.rust_type_kind()),
            Some(fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1::Execution(_))
        )
    {
        if pointer.kind() != SemanticPointerKindV1::Reference {
            return Err(execution_cfg_error_v29());
        }
        return Ok(Some(true));
    }
    Ok(None)
}

pub fn execution_reference_abi_scalar_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<SemanticBackendScalarV1, ProductionSourceArgumentErrorV1> {
    if execution_cfg_nominal_kind_v29(types, ty)? != Some(true) {
        return Err(execution_call_error_v29());
    }
    let declaration = &types[ty.index() as usize];
    let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
        return Err(execution_call_error_v29());
    };
    let SemanticBackendReprV1::Scalar(scalar) = declaration.layout().backend_repr() else {
        return Err(execution_call_error_v29());
    };
    if pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || pointer.metadata() != SemanticPointerMetadataV1::None
        || declaration.layout().size_bytes() != Some(8)
        || declaration.layout().alignment_bytes() != 8
        || declaration.layout().is_uninhabited()
        || scalar.primitive() != SemanticBackendPrimitiveV1::pointer(0, 8, 8)
    {
        return Err(execution_call_error_v29());
    }
    Ok(*scalar)
}

pub fn execution_call_error_v29() -> ProductionSourceArgumentErrorV1 {
    unsupported(
        0,
        None,
        None,
        "execution call parameters differ from their source instance",
    )
}
