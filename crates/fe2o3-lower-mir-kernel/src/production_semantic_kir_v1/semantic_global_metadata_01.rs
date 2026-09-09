fn global_capability_physical_field_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    authenticated: Option<&RootKernelContextLoweringV1>,
    parent: SemanticTypeIdV1,
    physical: SemanticTypeIdV1,
    binding: &SemanticValueBindingV1,
) -> Option<SemanticValueBindingV1> {
    let SemanticValueBindingV1::GlobalCapability {
        value,
        semantic_view,
        element,
        contract,
        provenance,
        capability,
    } = binding
    else {
        return None;
    };
    let authenticated = authenticated?;
    if parent != *semantic_view
        || !global_capability_provenance_matches_v1(authenticated, *provenance)
        || lower_global_capability_type_v1(types, *element, *contract, &authenticated.context_type)
            .ok()?
            != *capability
        || global_capability_descriptor_v1(callables, parent)
            != Some((*element, *contract, *provenance))
    {
        return None;
    }
    let declaration = types.get(parent.index() as usize)?;
    let SemanticTypeShapeV1::Aggregate(fields) = declaration.shape() else {
        return None;
    };
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
        return None;
    };
    if fields.fields().first() != Some(&physical)
        || layout.field_offsets().first() != Some(&0)
        || !layout.padding().is_empty()
    {
        return None;
    }
    let authenticated_field = callables.iter().any(|callable| {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding, operation, ..
        } = callable
        else {
            return false;
        };
        let (SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindReadOnly {
            context,
            physical: payload,
            view,
            source_identity,
            ..
        }
        | SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindExclusiveReadWrite {
            context,
            physical: payload,
            view,
            source_identity,
            ..
        }
        | SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindDisjointWrite {
            context,
            physical: payload,
            view,
            source_identity,
            ..
        }) = operation
        else {
            return false;
        };
        *context == authenticated.semantic_type
            && *view == parent
            && *payload == physical
            && *source_identity == binding.identity()
    });
    // Admission authenticates the transparent layout and inert brand fields of
    // this exact bind. Keep its capability carrier, never recover the raw slice.
    authenticated_field.then(|| SemanticValueBindingV1::Value {
        id: *value,
        ty: Type::GlobalCapability(capability.clone()),
    })
}

fn slice_metadata_carrier_v1(
    types: &[SemanticTypeDeclV1],
    semantic_type: SemanticTypeIdV1,
    binding: &SemanticValueBindingV1,
    pointer_metadata: bool,
) -> Option<ValueId> {
    let (value, ty) = binding.value().ok()?;
    if matches!(ty, Type::Slice(_)) {
        return Some(value);
    }
    // Only the authenticated physical-field projection creates this ordinary
    // value carrier. A bound view itself is not a source-language slice.
    let SemanticValueBindingV1::Value {
        ty: Type::GlobalCapability(capability),
        ..
    } = binding
    else {
        return None;
    };
    let mut declaration = types.get(semantic_type.index() as usize)?;
    if pointer_metadata {
        let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
            return None;
        };
        if pointer.metadata() != SemanticPointerMetadataV1::SliceLength {
            return None;
        }
        declaration = types.get(pointer.pointee().index() as usize)?;
    }
    let SemanticTypeShapeV1::Slice { element } = declaration.shape() else {
        return None;
    };
    (lower_scalar_type(types, *element).ok()? == *capability.element()).then_some(value)
}
