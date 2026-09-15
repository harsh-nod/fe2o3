//! Retained forwarding shape, not provider authentication or checked extent.
use fe2o3_mir_model::semantic_mir_v1::*;

pub(super) fn exact_forwarder(
    body: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    checked: SemanticFunctionIdV1,
) -> Option<SemanticLocalIdV1> {
    let inputs = body.abi().source_input_types();
    if inputs.len() != 6
        || body.locals().len() != 7
        || body.blocks().len() != 2
        || body.abi().can_unwind()
        || body.abi().c_variadic()
        || body
            .blocks()
            .iter()
            .any(|block| !block.statements().is_empty())
        || body.abi().source_argument_ownership()
            != [
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ]
    {
        return None;
    }
    // Canonical local IDs may change; the complete source-role bijection cannot.
    let mut arguments = [None; 6];
    let mut return_local = None;
    for (index, local) in body.locals().iter().enumerate() {
        let id = SemanticLocalIdV1::from_index(index as u32);
        let slot = match local.role() {
            SemanticLocalRoleV1::Return if local.ty() == body.abi().source_output_type() => {
                &mut return_local
            }
            SemanticLocalRoleV1::Argument(argument)
                if inputs.get(argument as usize) == Some(&local.ty()) =>
            {
                arguments.get_mut(argument as usize)?
            }
            _ => return None,
        };
        if slot.replace(id).is_some() {
            return None;
        }
    }
    let return_local = return_local?;
    let receiver = arguments[0]?;
    let entry = body.blocks().get(body.entry().index() as usize)?;
    let SemanticTerminatorKindV1::Call(call) = entry.terminator().kind() else {
        return None;
    };
    let destination = call.destination()?;
    (callables.get(call.callee().index() as usize)
        == Some(&SemanticCallableDeclV1::Defined { function: checked })
        && call.arguments().len() == 5
        && call.variadic_argument_abis().is_empty()
        && call
            .arguments()
            .iter()
            .enumerate()
            .all(|(index, argument)| {
                matches!(argument, SemanticOperandV1::Copy(place)
                if Some(place.local()) == arguments[index + 1]
                    && place.projections().is_empty()
                    && place.ty() == inputs[index + 1])
            })
        && destination.place().local() == return_local
        && destination.place().projections().is_empty()
        && destination.place().ty() == body.abi().source_output_type()
        && destination.edge().target() != body.entry()
        && destination.edge().role() == SemanticEdgeRoleV1::CallReturn
        && matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
        && body
            .blocks()
            .get(destination.edge().target().index() as usize)
            .is_some_and(|block| {
                matches!(block.terminator().kind(), SemanticTerminatorKindV1::Return)
            }))
        .then_some(receiver)
}

/// Expansion retains canonical source block IDs, not rustc START_BLOCK numbers.
pub(super) fn exact_forwarder_at(
    body: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    checked: SemanticFunctionIdV1,
    call_block: Option<SemanticBlockIdV1>,
) -> Option<SemanticLocalIdV1> {
    if call_block != Some(body.entry()) {
        return None;
    }
    exact_forwarder(body, callables, checked)
}

#[cfg(test)]
#[path = "constructor_forwarder_tests.rs"]
mod tests;
