use super::*;

pub(super) fn abi_matches(
    request: &InertSemanticMirRequestV1,
    abi: &SemanticFunctionAbiV1,
    operation: SemanticExecutionCapabilityOperationV1,
) -> bool {
    let SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue {
        context,
        capability,
        policy,
    } = operation
    else {
        return true;
    };
    let Some(SemanticTypeShapeV1::Pointer(pointer)) =
        request.types.get(context.0 as usize).map(|ty| &ty.shape)
    else {
        return false;
    };
    abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.can_unwind()
        && !abi.c_variadic()
        && abi.fixed_count() == 1
        && abi.source_input_types() == [context]
        && abi.source_output_type() == capability
        && abi.hidden_arguments().is_empty()
        && abi.source_argument_ownership() == [SemanticSourceArgumentOwnershipV1::SharedBorrow]
        && matches!(abi.arguments(), [argument]
            if argument.role() == SemanticAbiArgumentRoleV1::Source
                && argument.ty() == context
                && matches!(argument.value().mode(), SemanticAbiPassModeV1::Direct(_))
                && argument.value().adjusted().is_none()
                && argument.value().pointee_override().is_none())
        && abi.return_value().ty() == capability
        && matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
        && abi.return_value().adjusted().is_none()
        && abi.return_value().pointee_override().is_none()
        && pointer.kind == SemanticPointerKindV1::Reference
        && pointer.mutability == SemanticMutabilityV1::Immutable
        && pointer.metadata == SemanticPointerMetadataV1::None
        && pointer.pointee != capability
        && exact_inhabited_aggregate_zst(request, pointer.pointee)
        && exact_inhabited_aggregate_zst(request, capability)
        && request.types[capability.0 as usize].layout.alignment_bytes == 1
        && [context, pointer.pointee, capability]
            .iter()
            .all(|ty| request.types[ty.0 as usize].identity != policy)
}
