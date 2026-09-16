/// Frozen V4/V5 parameter rows describe whole locals, not ignored or projected
/// components. This support check alone authenticates no source or execution.
pub fn legacy_correspondence_helper_parameters_supported_v4(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    abi: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionAbiV1,
) -> bool {
    use fe2o3_mir_model::semantic_mir_v1::*;
    if abi.extern_abi() == SemanticExternAbiV1::RustCall
        || abi.source_input_types().len() != abi.adjusted_arguments().len()
    {
        return false;
    }
    abi.adjusted_arguments()
        .iter()
        .enumerate()
        .all(|(index, argument)| {
            if argument.role() != SemanticAbiArgumentRoleV1::Source
                || abi.source_input_types().get(index) != Some(&argument.ty())
                || argument.value().adjusted().is_some()
                || argument.value().pointee_override().is_some()
            {
                return false;
            }
            let Some(declaration) = types.get(argument.ty().index() as usize) else {
                return false;
            };
            match declaration.shape() {
                SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
                    matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_))
                }
                SemanticTypeShapeV1::Pointer(pointer) => {
                    abi.canon_abi() == SemanticCanonAbiV1::Rust
                        && abi.extern_abi() == SemanticExternAbiV1::Rust
                        && abi.source_argument_ownership().get(index)
                            == Some(&SemanticSourceArgumentOwnershipV1::SharedBorrow)
                        && matches!(argument.mode(), SemanticAbiPassModeV1::Pair { .. })
                        && pointer.kind() == SemanticPointerKindV1::Reference
                        && pointer.mutability() == SemanticMutabilityV1::Immutable
                        && pointer.address_space() == 0
                        && pointer.pointer_width_bits() == 64
                        && pointer.metadata() == SemanticPointerMetadataV1::SliceLength
                        && types
                            .get(pointer.pointee().index() as usize)
                            .is_some_and(|pointee| {
                                let SemanticTypeShapeV1::Slice { element } = pointee.shape() else {
                                    return false;
                                };
                                types.get(element.index() as usize).is_some_and(|element| {
                                    matches!(
                                        element.shape(),
                                        SemanticTypeShapeV1::Scalar(_)
                                            | SemanticTypeShapeV1::ValidityScalar(_)
                                    )
                                })
                            })
                }
                _ => false,
            }
        })
}

/// Checks retained helpers independently of supplied receipt rows. Source-selected
/// roots/bodies keep their entry handling; helper reuse does not inherit it.
pub fn legacy_correspondence_source_parameters_supported_v4(
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
) -> bool {
    legacy_source_function_envelope_v4(semantic, true, |function| {
        legacy_correspondence_helper_parameters_supported_v4(semantic.types(), function.abi())
    })
}
