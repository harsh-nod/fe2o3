fn lower_shared_atomic_slice_parameter_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<Type> {
    let declaration = types.get(ty.index() as usize)?;
    let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
        return None;
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Immutable
        || pointer.metadata() != SemanticPointerMetadataV1::SliceLength
        || pointer.pointer_width_bits() != 64
        || lower_address_space(pointer.address_space()).ok()? != AddressSpace::Global
    {
        return None;
    }
    let SemanticTypeShapeV1::Slice { element } =
        types.get(pointer.pointee().index() as usize)?.shape()
    else {
        return None;
    };
    let element_type = types.get(element.index() as usize)?;
    if element_type.rust_type_kind()
        != fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1::CoreAtomicU32
        || element_type.layout().size_bytes() != Some(4)
        || element_type.layout().alignment_bytes() != 4
        || element_type.layout().is_uninhabited()
        || transparent_scalar_storage_type(types, *element) != Some(Type::Scalar(ScalarType::U32))
    {
        return None;
    }
    // Production source admission authenticates the nominal core type. Its
    // shared reference permits atomic mutation, not readonly or noalias semantics;
    // a constructed inert semantic marker is not source or execution authority.
    Some(Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ))
}

#[cfg(test)]
mod tests {
    include!("atomic_slice_parameter_v1_tests.rs");
}
