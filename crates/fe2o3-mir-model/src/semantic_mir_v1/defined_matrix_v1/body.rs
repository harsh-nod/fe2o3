use super::*;

pub(super) fn distinct_ids(ids: &[SemanticTypeIdV1]) -> bool {
    ids.iter()
        .enumerate()
        .all(|(index, id)| u64::from(id.index()) < HARD_MAX_TYPES_V1 && !ids[..index].contains(id))
}

fn nominal(
    declarations: &[SemanticTypeDeclV1],
    ids: &[SemanticTypeIdV1],
    identity: SemanticDefinedMatrixIdentityV1,
) -> bool {
    let markers = [
        identity.policy,
        identity.kernel_brand,
        identity.matrix_brand,
        identity.epoch,
        identity.execution_brand(),
    ];
    distinct_ids(ids)
        && ids.iter().enumerate().all(|(index, id)| {
            declarations.get(id.index() as usize).is_some_and(|ty| {
                !ty.layout().is_uninhabited()
                    && ty.identity().as_bytes() != &[0; 32]
                    && !markers.contains(&ty.identity())
                    && ids[..index].iter().all(|prior| {
                        declarations[prior.index() as usize].identity() != ty.identity()
                    })
            })
        })
}

fn zst(declarations: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    declarations.get(ty.index() as usize).is_some_and(|ty| {
        matches!(ty.shape(), SemanticTypeShapeV1::Aggregate(_))
            && ty.layout().size_bytes() == Some(0)
            && ty.layout().alignment_bytes() == 1
            && !ty.layout().is_uninhabited()
    })
}

fn shared(
    declarations: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
) -> bool {
    declarations
        .get(reference.index() as usize)
        .is_some_and(|ty| {
            matches!(ty.shape(), SemanticTypeShapeV1::Pointer(pointer)
            if pointer.pointee() == pointee
                && pointer.kind() == SemanticPointerKindV1::Reference
                && pointer.mutability() == SemanticMutabilityV1::Immutable
                && pointer.address_space() == 0 && pointer.pointer_width_bits() == 64
                && pointer.metadata() == SemanticPointerMetadataV1::None)
        })
}

fn aggregate(
    declarations: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    size: u64,
) -> Option<&[SemanticTypeIdV1]> {
    let ty = declarations.get(ty.index() as usize)?;
    let SemanticTypeShapeV1::Aggregate(fields) = ty.shape() else {
        return None;
    };
    (ty.layout().size_bytes() == Some(size)
        && ty.layout().alignment_bytes() == 8
        && !ty.layout().is_uninhabited())
    .then_some(fields.fields())
}

fn bind_marker_types(
    declarations: &[SemanticTypeDeclV1],
    types: SemanticPolicyMatrixBindTypesV1,
) -> Option<[SemanticTypeIdV1; 2]> {
    let [matrix, policy, brands, private] = aggregate(declarations, types.bound, 16)? else {
        return None;
    };
    (*matrix == types.matrix_reference
        && *policy == types.policy_reference
        && zst(declarations, *brands)
        && zst(declarations, *private))
    .then_some([*brands, *private])
}

pub(super) fn bind_types(
    declarations: &[SemanticTypeDeclV1],
    types: SemanticPolicyMatrixBindTypesV1,
    identity: SemanticDefinedMatrixIdentityV1,
) -> bool {
    nominal(declarations, &types.all(), identity)
        && shared(declarations, types.matrix_reference, types.matrix)
        && shared(declarations, types.policy_reference, types.capability)
        && zst(declarations, types.matrix)
        && zst(declarations, types.capability)
        && bind_marker_types(declarations, types).is_some()
}

pub(super) fn narrow_types(
    declarations: &[SemanticTypeDeclV1],
    types: SemanticPolicyGfx950NarrowTypesV1,
    identity: SemanticDefinedMatrixIdentityV1,
) -> bool {
    let Some([reference, marker]) = aggregate(declarations, types.narrowed, 8) else {
        return false;
    };
    nominal(declarations, &types.all(), identity)
        && bind_types(declarations, types.bind, identity)
        && shared(declarations, types.bound_reference, types.bind.bound)
        && *reference == types.bound_reference
        && zst(declarations, *marker)
}

fn function_shape(body: &SemanticFunctionDeclV1) -> bool {
    body.role() == SemanticFunctionRoleV1::InternalHelper
        && body.export().is_none()
        && body.blocks().get(body.entry().index() as usize).is_some()
}

pub(super) fn defined<'a>(
    function: SemanticFunctionIdV1,
    functions: &'a [SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
) -> Result<&'a SemanticFunctionDeclV1, SemanticMirErrorV1> {
    require(
        callables.get(function.index() as usize)
            == Some(&SemanticCallableDeclV1::defined(function)),
    )?;
    let body = functions
        .get(function.index() as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    require(function_shape(body))?;
    Ok(body)
}

fn abi(
    abi: &SemanticFunctionAbiV1,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    pair: bool,
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
            .all(|o| *o == SemanticSourceArgumentOwnershipV1::SharedBorrow)
        && abi.arguments().iter().zip(inputs).all(|(arg, ty)| {
            arg.role() == SemanticAbiArgumentRoleV1::Source
                && arg.ty() == *ty
                && matches!(arg.value().mode(), SemanticAbiPassModeV1::Direct(_))
                && arg.value().adjusted().is_none()
                && arg.value().pointee_override().is_none()
        })
        && abi.return_value().adjusted().is_none()
        && abi.return_value().pointee_override().is_none()
        && if pair {
            matches!(
                abi.return_value().mode(),
                SemanticAbiPassModeV1::Pair { .. }
            )
        } else {
            matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Direct(_))
        }
}

fn local(
    body: &SemanticFunctionDeclV1,
    ty: SemanticTypeIdV1,
    role: SemanticLocalRoleV1,
) -> Option<SemanticLocalIdV1> {
    let mut matching = body
        .locals()
        .iter()
        .enumerate()
        .filter(|(_, local)| local.role() == role);
    let (index, declaration) = matching.next()?;
    (declaration.ty() == ty && matching.next().is_none())
        .then_some(SemanticLocalIdV1::from_index(u32::try_from(index).ok()?))
}

fn plain(place: &SemanticPlaceV1, local: SemanticLocalIdV1, ty: SemanticTypeIdV1) -> bool {
    place.local() == local && place.ty() == ty && place.projections().is_empty()
}

fn copied(operand: &SemanticOperandV1, local: SemanticLocalIdV1, ty: SemanticTypeIdV1) -> bool {
    matches!(operand, SemanticOperandV1::Copy(place) if plain(place, local, ty))
}

fn returned_aggregate<'a>(
    block: &'a SemanticBasicBlockV1,
    output: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
) -> Option<&'a [SemanticOperandV1]> {
    if !matches!(block.terminator().kind(), SemanticTerminatorKindV1::Return) {
        return None;
    }
    let [statement] = block.statements() else {
        return None;
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return None;
    };
    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
        return None;
    };
    (plain(assignment.destination(), output, ty)
        && assignment.value().result_type() == ty
        && *aggregate.kind() == SemanticAggregateKindV1::Aggregate)
        .then_some(aggregate.operands())
}

fn marker(operand: &SemanticOperandV1) -> Option<SemanticTypeIdV1> {
    match operand {
        SemanticOperandV1::Constant(value)
            if matches!(value.value(), SemanticConstantValueV1::ZeroSized) =>
        {
            Some(value.ty())
        }
        _ => None,
    }
}

pub(super) fn bind_markers(
    body: &SemanticFunctionDeclV1,
    types: SemanticPolicyMatrixBindTypesV1,
) -> Option<[SemanticTypeIdV1; 2]> {
    if !function_shape(body)
        || body.blocks().len() != 1
        || body.locals().len() != 3
        || !abi(
            body.abi(),
            &[types.matrix_reference, types.policy_reference],
            types.bound,
            true,
        )
    {
        return None;
    }
    let output = local(body, types.bound, SemanticLocalRoleV1::Return)?;
    let matrix = local(
        body,
        types.matrix_reference,
        SemanticLocalRoleV1::Argument(0),
    )?;
    let policy = local(
        body,
        types.policy_reference,
        SemanticLocalRoleV1::Argument(1),
    )?;
    let [matrix_value, policy_value, brands, private] = returned_aggregate(
        &body.blocks()[body.entry().index() as usize],
        output,
        types.bound,
    )?
    else {
        return None;
    };
    if !copied(matrix_value, matrix, types.matrix_reference)
        || !copied(policy_value, policy, types.policy_reference)
    {
        return None;
    }
    Some([marker(brands)?, marker(private)?])
}

pub(super) fn bind(
    body: &SemanticFunctionDeclV1,
    types: SemanticPolicyMatrixBindTypesV1,
    declarations: &[SemanticTypeDeclV1],
) -> bool {
    bind_markers(body, types)
        .is_some_and(|markers| Some(markers) == bind_marker_types(declarations, types))
}

// The observed source calls the original field getter, then retains that same
// receiver in field zero. The getter is not a fresh or ambient matrix issuer.
pub(super) fn narrow_recipe(
    body: &SemanticFunctionDeclV1,
    types: SemanticPolicyGfx950NarrowTypesV1,
) -> Option<(SemanticTypeIdV1, SemanticCallableIdV1)> {
    if !function_shape(body)
        || body.locals().len() != 3
        || body.blocks().len() != 2
        || !abi(body.abi(), &[types.bound_reference], types.narrowed, false)
    {
        return None;
    }
    let output = local(body, types.narrowed, SemanticLocalRoleV1::Return)?;
    let receiver = local(
        body,
        types.bound_reference,
        SemanticLocalRoleV1::Argument(0),
    )?;
    let entry = &body.blocks()[body.entry().index() as usize];
    let temporary = local(
        body,
        types.bind.matrix_reference,
        SemanticLocalRoleV1::Temporary,
    )?;
    let SemanticTerminatorKindV1::Call(call) = entry.terminator().kind() else {
        return None;
    };
    let destination = call.destination()?;
    if !entry.statements().is_empty()
        || call.arguments().len() != 1
        || !copied(&call.arguments()[0], receiver, types.bound_reference)
        || !call.variadic_argument_abis().is_empty()
        || !plain(destination.place(), temporary, types.bind.matrix_reference)
        || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
        || destination.edge().target() == body.entry()
        || !matches!(
            call.unwind(),
            SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
        )
    {
        return None;
    }
    let exit = body
        .blocks()
        .get(destination.edge().target().index() as usize)?;
    let [reference, private] = returned_aggregate(exit, output, types.narrowed)? else {
        return None;
    };
    copied(reference, receiver, types.bound_reference).then_some((marker(private)?, call.callee()))
}

pub(super) fn narrow(
    body: &SemanticFunctionDeclV1,
    types: SemanticPolicyGfx950NarrowTypesV1,
    declarations: &[SemanticTypeDeclV1],
) -> Option<SemanticCallableIdV1> {
    let (marker, projection) = narrow_recipe(body, types)?;
    let [reference, expected_marker] = aggregate(declarations, types.narrowed, 8)? else {
        return None;
    };
    (*reference == types.bound_reference && *expected_marker == marker && zst(declarations, marker))
        .then_some(projection)
}

pub(super) fn projection(
    body: &SemanticFunctionDeclV1,
    types: SemanticPolicyGfx950NarrowTypesV1,
) -> bool {
    if !function_shape(body)
        || body.locals().len() != 2
        || body.blocks().len() != 1
        || !abi(
            body.abi(),
            &[types.bound_reference],
            types.bind.matrix_reference,
            false,
        )
    {
        return false;
    }
    let Some(output) = local(
        body,
        types.bind.matrix_reference,
        SemanticLocalRoleV1::Return,
    ) else {
        return false;
    };
    let Some(receiver) = local(
        body,
        types.bound_reference,
        SemanticLocalRoleV1::Argument(0),
    ) else {
        return false;
    };
    let block = &body.blocks()[body.entry().index() as usize];
    let [statement] = block.statements() else {
        return false;
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return false;
    };
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
    else {
        return false;
    };
    let [dereference, field] = place.projections() else {
        return false;
    };
    matches!(block.terminator().kind(), SemanticTerminatorKindV1::Return)
        && plain(
            assignment.destination(),
            output,
            types.bind.matrix_reference,
        )
        && assignment.value().result_type() == types.bind.matrix_reference
        && place.local() == receiver
        && place.ty() == types.bind.matrix_reference
        && dereference.kind() == SemanticProjectionKindV1::Dereference
        && dereference.result_type() == types.bind.bound
        && field.kind() == SemanticProjectionKindV1::Field(0)
        && field.result_type() == types.bind.matrix_reference
}
