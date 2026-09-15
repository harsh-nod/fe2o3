//! Inert complete-body correspondence. Never supplies a leader or source seal.
use fe2o3_mir_model::semantic_mir_v1::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct IndexTypes {
    pub leader: SemanticTypeIdV1,
    pub reference: SemanticTypeIdV1,
    pub raw: SemanticTypeIdV1,
    pub witness: SemanticTypeIdV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct IndexBody {
    pub receiver: SemanticLocalIdV1,
    pub raw: SemanticLocalIdV1,
    pub returned: SemanticLocalIdV1,
    pub constructor: SemanticFunctionIdV1,
    pub constructor_raw: SemanticLocalIdV1,
    pub constructor_return: SemanticLocalIdV1,
    pub return_block: SemanticBlockIdV1,
}

fn role(
    function: &SemanticFunctionDeclV1,
    role: SemanticLocalRoleV1,
    ty: SemanticTypeIdV1,
) -> Option<SemanticLocalIdV1> {
    let mut matches = function
        .locals()
        .iter()
        .enumerate()
        .filter(|(_, l)| l.role() == role);
    let (id, local) = matches.next()?;
    (local.ty() == ty && matches.next().is_none())
        .then_some(SemanticLocalIdV1::from_index(u32::try_from(id).ok()?))
}

fn is_copy(operand: &SemanticOperandV1, local: SemanticLocalIdV1, ty: SemanticTypeIdV1) -> bool {
    matches!(operand, SemanticOperandV1::Copy(place)
        if place.local() == local && place.ty() == ty && place.projections().is_empty())
}

fn abi(
    body: &SemanticFunctionDeclV1,
    inputs: &[SemanticTypeIdV1],
    ownership: &[SemanticSourceArgumentOwnershipV1],
    output: SemanticTypeIdV1,
) -> bool {
    let abi = body.abi();
    body.role() == SemanticFunctionRoleV1::InternalHelper
        && body.export().is_none()
        && body.defined_capability_contract().is_none()
        && abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.can_unwind()
        && !abi.c_variadic()
        && abi.hidden_arguments().is_empty()
        && abi.fixed_count() as usize == inputs.len()
        && abi.source_input_types() == inputs
        && abi.source_argument_ownership() == ownership
        && abi.source_output_type() == output
        && abi.arguments().len() == inputs.len()
        && abi.arguments().iter().zip(inputs).all(|(arg, ty)| {
            arg.role() == SemanticAbiArgumentRoleV1::Source
                && arg.ty() == *ty
                && arg.value().adjusted().is_none()
                && arg.value().pointee_override().is_none()
                && matches!(arg.value().mode(), SemanticAbiPassModeV1::Direct(_))
        })
        && abi.return_value().ty() == output
        && abi.return_value().adjusted().is_none()
        && abi.return_value().pointee_override().is_none()
        && matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Direct(_))
}

fn zero_sized(declarations: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    declarations.get(ty.index() as usize).is_some_and(|ty| {
        ty.layout().size_bytes() == Some(0)
            && ty.layout().alignment_bytes() == 1
            && !ty.layout().is_uninhabited()
    })
}

fn types_match(declarations: &[SemanticTypeDeclV1], t: IndexTypes) -> bool {
    let ids = [t.leader, t.reference, t.raw, t.witness];
    if !ids.iter().enumerate().all(|(i, id)| {
        declarations.get(id.index() as usize).is_some_and(|ty| {
            ty.identity().as_bytes() != &[0; 32]
                && !ty.layout().is_uninhabited()
                && ids[..i].iter().all(|previous| {
                    previous != id
                        && declarations[previous.index() as usize].identity() != ty.identity()
                })
        })
    }) {
        return false;
    }
    let reference = &declarations[t.reference.index() as usize];
    let raw = &declarations[t.raw.index() as usize];
    let witness = &declarations[t.witness.index() as usize];
    zero_sized(declarations, t.leader)
        && matches!(declarations[t.leader.index() as usize].shape(), SemanticTypeShapeV1::Aggregate(a)
            if a.fields().len() == 3 && a.fields().iter().all(|ty| zero_sized(declarations, *ty)))
        && reference.layout().size_bytes() == Some(8)
        && reference.layout().alignment_bytes() == 8
        && matches!(reference.shape(), SemanticTypeShapeV1::Pointer(p)
            if p.kind() == SemanticPointerKindV1::Reference && p.pointee() == t.leader
                && p.mutability() == SemanticMutabilityV1::Immutable
                && p.address_space() == 0 && p.pointer_width_bits() == 64
                && p.metadata() == SemanticPointerMetadataV1::None)
        && raw.layout().size_bytes() == Some(8)
        && raw.layout().alignment_bytes() == 8
        && matches!(
            raw.shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64
            })
        )
        && witness.layout().size_bytes() == Some(8)
        && witness.layout().alignment_bytes() == 8
        && matches!(witness.shape(), SemanticTypeShapeV1::Aggregate(a)
            if a.fields().len() == 4 && a.fields()[0] == t.raw
                && a.fields()[1..].iter().all(|ty| zero_sized(declarations, *ty)))
}

/// Fixed three-block/six-local recipe. Charge its fixed visits before examining
/// the source; variable rosters are length-checked before any iteration.
pub(super) fn observe(
    body: &SemanticFunctionDeclV1,
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    declarations: &[SemanticTypeDeclV1],
    t: IndexTypes,
    charge: &mut impl FnMut(usize) -> bool,
) -> Result<Option<IndexBody>, ()> {
    if !charge(128) {
        return Err(());
    }
    if body.blocks().len() != 2
        || body.locals().len() != 3
        || !types_match(declarations, t)
        || !abi(
            body,
            &[t.reference, t.raw],
            &[
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ],
            t.witness,
        )
    {
        return Ok(None);
    }
    let Some(receiver) = role(body, SemanticLocalRoleV1::Argument(0), t.reference) else {
        return Ok(None);
    };
    let Some(raw) = role(body, SemanticLocalRoleV1::Argument(1), t.raw) else {
        return Ok(None);
    };
    let Some(returned) = role(body, SemanticLocalRoleV1::Return, t.witness) else {
        return Ok(None);
    };
    let Some(entry) = body.blocks().get(body.entry().index() as usize) else {
        return Ok(None);
    };
    let SemanticTerminatorKindV1::Call(call) = entry.terminator().kind() else {
        return Ok(None);
    };
    let Some(destination) = call.destination() else {
        return Ok(None);
    };
    let [argument] = call.arguments() else {
        return Ok(None);
    };
    if !entry.statements().is_empty()
        || !is_copy(argument, raw, t.raw)
        || call.unwind() != SemanticUnwindActionV1::Unreachable
        || !call.variadic_argument_abis().is_empty()
        || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
        || destination.edge().target() == body.entry()
        || destination.place().local() != returned
        || destination.place().ty() != t.witness
        || !destination.place().projections().is_empty()
    {
        return Ok(None);
    }
    let return_block = destination.edge().target();
    let Some(exit) = body.blocks().get(return_block.index() as usize) else {
        return Ok(None);
    };
    if !exit.statements().is_empty()
        || !matches!(exit.terminator().kind(), SemanticTerminatorKindV1::Return)
    {
        return Ok(None);
    }
    let Some(SemanticCallableDeclV1::Defined {
        function: constructor,
    }) = callables.get(call.callee().index() as usize)
    else {
        return Ok(None);
    };
    let Some(constructor_body) = functions.get(constructor.index() as usize) else {
        return Ok(None);
    };
    if constructor_body.blocks().len() != 1
        || constructor_body.locals().len() != 2
        || !abi(
            constructor_body,
            &[t.raw],
            &[SemanticSourceArgumentOwnershipV1::ByValue],
            t.witness,
        )
    {
        return Ok(None);
    }
    let Some(constructor_raw) = role(constructor_body, SemanticLocalRoleV1::Argument(0), t.raw)
    else {
        return Ok(None);
    };
    let Some(constructor_return) = role(constructor_body, SemanticLocalRoleV1::Return, t.witness)
    else {
        return Ok(None);
    };
    let Some(block) = constructor_body
        .blocks()
        .get(constructor_body.entry().index() as usize)
    else {
        return Ok(None);
    };
    let [statement] = block.statements() else {
        return Ok(None);
    };
    let SemanticStatementKindV1::Assign(a) = statement.kind() else {
        return Ok(None);
    };
    let SemanticRvalueKindV1::Aggregate(value) = a.value().kind() else {
        return Ok(None);
    };
    let SemanticTypeShapeV1::Aggregate(fields) = declarations[t.witness.index() as usize].shape()
    else {
        return Ok(None);
    };
    let [value_raw, markers @ ..] = value.operands() else {
        return Ok(None);
    };
    if !matches!(block.terminator().kind(), SemanticTerminatorKindV1::Return)
        || a.destination().local() != constructor_return
        || a.destination().ty() != t.witness
        || !a.destination().projections().is_empty()
        || a.value().result_type() != t.witness
        || *value.kind() != SemanticAggregateKindV1::Aggregate
        || markers.len() != 3
        || !is_copy(value_raw, constructor_raw, t.raw)
        || !markers
            .iter()
            .zip(&fields.fields()[1..])
            .all(|(value, ty)| {
                matches!(value, SemanticOperandV1::Constant(c)
                if c.ty() == *ty && matches!(c.value(), SemanticConstantValueV1::ZeroSized))
            })
    {
        return Ok(None);
    }
    Ok(Some(IndexBody {
        receiver,
        raw,
        returned,
        constructor: *constructor,
        constructor_raw,
        constructor_return,
        return_block,
    }))
}

#[cfg(test)]
#[path = "index_body_tests.rs"]
mod tests;
