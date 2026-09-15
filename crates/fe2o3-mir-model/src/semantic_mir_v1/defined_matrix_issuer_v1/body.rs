//! Shared closed body checks; these facts do not authenticate a provider or root.
use super::*;

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Recipe<'a> {
    pub getter: &'a SemanticFunctionDeclV1,
    pub bridge_function: SemanticFunctionIdV1,
    pub bridge: &'a SemanticFunctionDeclV1,
    pub current_callable: SemanticCallableIdV1,
    pub current_binding: &'a SemanticNonBodyCallableBindingV1,
    pub types: SemanticKernelMatrixDeriveTypesV1,
    pub provenance: SemanticKernelCapabilityProvenanceV1,
    pub kernel_brand: SemanticTypeIdentityV1,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn observe_recipe<'a>(
    function: SemanticFunctionIdV1,
    functions: &'a [SemanticFunctionDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    declarations: &[SemanticTypeDeclV1],
    types: SemanticKernelMatrixDeriveTypesV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    kernel_brand: SemanticTypeIdentityV1,
) -> Result<Recipe<'a>, SemanticMirErrorV1> {
    let body = defined_body(function, functions, callables)?;
    require(derive_types_match(declarations, types, kernel_brand))?;
    let bridge_callable = getter_body(body, types).ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    let Some(SemanticCallableDeclV1::Defined {
        function: bridge_function,
    }) = callables.get(bridge_callable.index() as usize)
    else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    require(*bridge_function != function)?;
    let bridge_body = defined_body(*bridge_function, functions, callables)?;
    let current_callable =
        bridge_body_call(bridge_body, types).ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
        binding, operation, ..
    }) = callables.get(current_callable.index() as usize)
    else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    require(
        *operation
            == (SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent {
                context: types.unbranded_matrix,
            })
            && abi_matches(
                binding.abi(),
                &[],
                types.unbranded_matrix,
                ReturnMode::Ignore,
            ),
    )?;

    Ok(Recipe {
        getter: body,
        bridge_function: *bridge_function,
        bridge: bridge_body,
        current_callable,
        current_binding: binding,
        types,
        provenance,
        kernel_brand,
    })
}

pub(super) fn require(condition: bool) -> Result<(), SemanticMirErrorV1> {
    if condition {
        Ok(())
    } else {
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    }
}

pub(super) fn distinct_ids(ids: &[SemanticTypeIdV1]) -> bool {
    ids.iter()
        .enumerate()
        .all(|(index, id)| u64::from(id.index()) < HARD_MAX_TYPES_V1 && !ids[..index].contains(id))
}

pub(super) fn nominal_types(
    declarations: &[SemanticTypeDeclV1],
    ids: &[SemanticTypeIdV1],
    markers: &[SemanticTypeIdentityV1],
) -> bool {
    distinct_ids(ids)
        && ids.iter().enumerate().all(|(index, id)| {
            declarations.get(id.index() as usize).is_some_and(|ty| {
                ty.identity().as_bytes() != &[0; 32]
                    && !ty.layout().is_uninhabited()
                    && !markers.contains(&ty.identity())
                    && ids[..index].iter().all(|previous| {
                        declarations
                            .get(previous.index() as usize)
                            .is_some_and(|previous| previous.identity() != ty.identity())
                    })
            })
        })
}

pub(super) fn aggregate_zst(declarations: &[SemanticTypeDeclV1], id: SemanticTypeIdV1) -> bool {
    declarations.get(id.index() as usize).is_some_and(|ty| {
        matches!(ty.shape(), SemanticTypeShapeV1::Aggregate(_))
            && ty.layout().size_bytes() == Some(0)
            && ty.layout().alignment_bytes() == 1
            && !ty.layout().is_uninhabited()
    })
}

pub(super) fn shared_type(
    declarations: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
) -> bool {
    matches!(declarations.get(reference.index() as usize).map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Pointer(pointer)) if pointer.pointee() == pointee
            && pointer.kind() == SemanticPointerKindV1::Reference
            && pointer.mutability() == SemanticMutabilityV1::Immutable
            && pointer.address_space() == 0 && pointer.pointer_width_bits() == 64
            && pointer.metadata() == SemanticPointerMetadataV1::None)
}

pub(super) fn derive_types_match(
    declarations: &[SemanticTypeDeclV1],
    types: SemanticKernelMatrixDeriveTypesV1,
    brand: SemanticTypeIdentityV1,
) -> bool {
    nominal_types(declarations, &types.all(), &[brand])
        && shared_type(declarations, types.context_reference, types.context)
        && matches!(declarations[types.matrix.index() as usize].shape(),
            SemanticTypeShapeV1::Aggregate(aggregate) if aggregate.fields()
                == [types.unbranded_matrix, types.brand_marker, types.thread_marker])
        && [types.brand_marker, types.thread_marker].iter().all(|id| {
            matches!(declarations[id.index() as usize].shape(),
                SemanticTypeShapeV1::Aggregate(aggregate) if aggregate.fields().is_empty())
        })
        && [
            types.context,
            types.matrix,
            types.unbranded_matrix,
            types.brand_marker,
            types.thread_marker,
        ]
        .iter()
        .all(|id| aggregate_zst(declarations, *id))
}

#[derive(Clone, Copy)]
pub(super) enum ReturnMode {
    Ignore,
    Pair,
}

pub(super) fn abi_matches(
    abi: &SemanticFunctionAbiV1,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    return_mode: ReturnMode,
) -> bool {
    abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.can_unwind()
        && !abi.c_variadic()
        && abi.fixed_count() as usize == inputs.len()
        && abi.hidden_arguments().is_empty()
        && abi.source_input_types() == inputs
        && abi.source_output_type() == output
        && abi.arguments().len() == inputs.len()
        && abi.source_argument_ownership().len() == inputs.len()
        && abi
            .source_argument_ownership()
            .iter()
            .all(|ownership| *ownership == SemanticSourceArgumentOwnershipV1::SharedBorrow)
        && abi.arguments().iter().zip(inputs).all(|(argument, ty)| {
            argument.role() == SemanticAbiArgumentRoleV1::Source
                && argument.ty() == *ty
                && matches!(argument.value().mode(), SemanticAbiPassModeV1::Direct(_))
                && argument.value().adjusted().is_none()
                && argument.value().pointee_override().is_none()
        })
        && abi.return_value().ty() == output
        && abi.return_value().adjusted().is_none()
        && abi.return_value().pointee_override().is_none()
        && match return_mode {
            ReturnMode::Ignore => {
                matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
            }
            ReturnMode::Pair => matches!(
                abi.return_value().mode(),
                SemanticAbiPassModeV1::Pair { .. }
            ),
        }
}

pub(super) fn defined_body<'a>(
    function: SemanticFunctionIdV1,
    functions: &'a [SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
) -> Result<&'a SemanticFunctionDeclV1, SemanticMirErrorV1> {
    require(
        callables.get(function.index() as usize)
            == Some(&SemanticCallableDeclV1::Defined { function }),
    )?;
    let body = functions
        .get(function.index() as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    require(
        body.role() == SemanticFunctionRoleV1::InternalHelper
            && body.export().is_none()
            && body.blocks().get(body.entry().index() as usize).is_some(),
    )?;
    Ok(body)
}

pub(super) fn local(
    body: &SemanticFunctionDeclV1,
    ty: SemanticTypeIdV1,
    role: SemanticLocalRoleV1,
) -> Option<SemanticLocalIdV1> {
    let mut matching = body
        .locals()
        .iter()
        .enumerate()
        .filter(|(_, local)| local.role() == role);
    let (index, local) = matching.next()?;
    if local.ty() != ty || matching.next().is_some() {
        return None;
    }
    Some(SemanticLocalIdV1::from_index(u32::try_from(index).ok()?))
}

pub(super) fn forwarding_call(
    body: &SemanticFunctionDeclV1,
    output_local: SemanticLocalIdV1,
    output: SemanticTypeIdV1,
) -> Option<SemanticCallableIdV1> {
    if body.blocks().len() != 2 {
        return None;
    }
    // Canonical order follows identity, not the original rustc block ordinal.
    let entry = body.blocks().get(body.entry().index() as usize)?;
    let SemanticTerminatorKindV1::Call(call) = entry.terminator().kind() else {
        return None;
    };
    let destination = call.destination()?;
    if destination.edge().target() == body.entry() {
        return None;
    }
    let exit = body
        .blocks()
        .get(destination.edge().target().index() as usize)?;
    if !entry.statements().is_empty()
        || !exit.statements().is_empty()
        || !matches!(exit.terminator().kind(), SemanticTerminatorKindV1::Return)
    {
        return None;
    }
    (call.arguments().is_empty()
        && destination.place().local() == output_local
        && call.variadic_argument_abis().is_empty()
        && destination.place().projections().is_empty()
        && destination.place().ty() == output
        && destination.edge().role() == SemanticEdgeRoleV1::CallReturn
        && matches!(call.unwind(), SemanticUnwindActionV1::Unreachable))
    .then_some(call.callee())
}

pub(super) fn getter_body(
    body: &SemanticFunctionDeclV1,
    types: SemanticKernelMatrixDeriveTypesV1,
) -> Option<SemanticCallableIdV1> {
    if body.locals().len() != 2
        || !abi_matches(
            body.abi(),
            &[types.context_reference],
            types.matrix,
            ReturnMode::Ignore,
        )
    {
        return None;
    }
    let output = local(body, types.matrix, SemanticLocalRoleV1::Return)?;
    local(
        body,
        types.context_reference,
        SemanticLocalRoleV1::Argument(0),
    )?;
    forwarding_call(body, output, types.matrix)
}

pub(super) fn bridge_body_call(
    body: &SemanticFunctionDeclV1,
    types: SemanticKernelMatrixDeriveTypesV1,
) -> Option<SemanticCallableIdV1> {
    if body.locals().len() != 2 || !abi_matches(body.abi(), &[], types.matrix, ReturnMode::Ignore) {
        return None;
    }
    local(body, types.matrix, SemanticLocalRoleV1::Return)?;
    let output = local(body, types.unbranded_matrix, SemanticLocalRoleV1::Temporary)?;
    forwarding_call(body, output, types.unbranded_matrix)
}
