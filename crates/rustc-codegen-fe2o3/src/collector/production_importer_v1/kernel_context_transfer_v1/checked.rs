use fe2o3_mir_model::semantic_mir_v1::*;

/// Created only by the collector-backed importer, never from a context type.
pub(super) struct SourceBoundary {
    pub(super) root: SemanticFunctionIdentityV1,
    pub(super) helper: SemanticFunctionIdentityV1,
    pub(super) issuance_block: SemanticBlockIdentityV1,
    pub(super) issuance_terminal: SemanticFunctionIdentityV1,
    pub(super) physical_arguments: u32,
    pub(super) logical_arguments: u32,
}

/// Recover the logical move erased by rustc's ignored-ABI ZST propagation at
/// the authenticated generated wrapper boundary. No other constant is an issuer.
pub(super) fn restore(
    boundary: &SourceBoundary,
    root: &SemanticFunctionDeclV1,
    helper_id: SemanticFunctionIdV1,
    helper: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
) -> Result<Option<SemanticFunctionDeclV1>, &'static str> {
    if root.identity() != boundary.root
        || helper.identity() != boundary.helper
        || root.role() != SemanticFunctionRoleV1::KernelRoot
        || helper.role() != SemanticFunctionRoleV1::InternalHelper
        || root.defined_capability_contract().is_some()
        || boundary
            .logical_arguments
            .checked_sub(boundary.physical_arguments)
            != Some(1)
        || root.abi().source_input_types().len() != boundary.physical_arguments as usize
        || helper.abi().source_input_types().len() != boundary.logical_arguments as usize
    {
        return Err("context transfer does not match the authenticated root/helper ABI boundary");
    }
    let mut helper_call = None;
    let mut issuance = None;
    for (index, block) in root.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        match callables.get(call.callee().index() as usize) {
            Some(SemanticCallableDeclV1::Defined { function }) if *function == helper_id => {
                if helper_call.replace((index, call)).is_some() {
                    return Err("context transfer has more than one logical-helper call");
                }
            }
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context },
                ..
            }) => {
                if issuance.replace((index, call, binding, *context)).is_some() {
                    return Err("context transfer has more than one issuer");
                }
            }
            _ => {}
        }
    }
    let (helper_block, call) =
        helper_call.ok_or("context transfer lost its logical-helper call")?;
    let Some(SemanticOperandV1::Constant(constant)) = call.arguments().first() else {
        // Retained source operands keep their bytes and ordinary move checks.
        return Ok(None);
    };
    if constant.value() != &SemanticConstantValueV1::ZeroSized {
        return Err("context transfer argument is not the exact erased ZST");
    }
    let (issue_block, issue, binding, context) =
        issuance.ok_or("erased context transfer has no retained issuer")?;
    let destination = issue
        .destination()
        .ok_or("context issuer has no normal result")?;
    let issued = destination.place();
    if root.blocks()[issue_block].identity() != boundary.issuance_block
        || binding.identity() != boundary.issuance_terminal
        || root.entry().index() as usize != issue_block
        || !issue.arguments().is_empty()
        || !issue.variadic_argument_abis().is_empty()
        || issue.unwind() != SemanticUnwindActionV1::Unreachable
        || !binding.abi().source_input_types().is_empty()
        || binding.abi().source_output_type() != context
        || binding.abi().return_value().mode() != &SemanticAbiPassModeV1::Ignore
        || issued.ty() != context
        || !issued.projections().is_empty()
        || destination.edge().target().index() as usize != helper_block
        || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
    {
        return Err("erased context transfer does not follow its exact retained issuer");
    }
    let issued_local = issued.local();
    if !root
        .locals()
        .get(issued_local.index() as usize)
        .is_some_and(|local| {
            local.ty() == context && local.role() == SemanticLocalRoleV1::Temporary
        })
        || constant.ty() != context
        || helper.abi().source_input_types().first() != Some(&context)
        || !helper.abi().arguments().first().is_some_and(|argument| {
            argument.value().ty() == context
                && argument.value().mode() == &SemanticAbiPassModeV1::Ignore
        })
        || call.arguments().len() != boundary.logical_arguments as usize
        || !call.variadic_argument_abis().is_empty()
        || call.unwind() != SemanticUnwindActionV1::Unreachable
        || call.destination().is_none()
    {
        return Err("erased context transfer changed the exact owned context type or ABI");
    }

    // The generated wrapper forwards each physical argument once. In particular
    // the erased operand cannot duplicate or alias another context operand.
    for (ordinal, argument) in call.arguments().iter().skip(1).enumerate() {
        let place = match argument {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
            SemanticOperandV1::Constant(_) => {
                return Err("context wrapper does not forward its physical arguments exactly");
            }
        };
        let declaration = root.locals().get(place.local().index() as usize);
        if !place.projections().is_empty()
            || place.ty() != root.abi().source_input_types()[ordinal]
            || place.ty() != helper.abi().source_input_types()[ordinal + 1]
            || !declaration.is_some_and(|local| {
                local.ty() == place.ty()
                    && local.role() == SemanticLocalRoleV1::Argument(ordinal as u32)
            })
        {
            return Err("context wrapper does not forward its physical arguments exactly");
        }
    }

    // This source recipe is deliberately one normal edge, not a second CFG
    // origin solver. A bypass, backedge, cleanup, or lifetime transition fails.
    let mut incoming = 0_usize;
    for (index, block) in root.blocks().iter().enumerate() {
        block.terminator().kind().try_for_each_edge(|edge| {
            if edge.target().index() as usize == helper_block {
                incoming += 1;
                if index != issue_block || edge.role() != SemanticEdgeRoleV1::CallReturn {
                    return Err("erased context transfer has a bypass or repeated entry");
                }
            }
            if edge.target() == root.entry() {
                return Err("erased context issuer can be entered again");
            }
            Ok(())
        })?;
    }
    if incoming != 1 {
        return Err("erased context transfer does not have one issuer predecessor");
    }
    for statement in root.blocks()[helper_block].statements() {
        match statement.kind() {
            SemanticStatementKindV1::Nop => {}
            SemanticStatementKindV1::StorageLive(local)
            | SemanticStatementKindV1::StorageDead(local)
                if *local != issued_local => {}
            _ => return Err("erased context result is touched before its logical move"),
        }
    }

    let mut arguments = call.arguments().to_vec();
    arguments[0] = SemanticOperandV1::Move(issued.clone());
    let restored = SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
        call.callee(),
        arguments,
        call.variadic_argument_abis().to_vec(),
        call.destination().cloned(),
        call.unwind(),
    )
    .map_err(|_| "context transfer exceeds semantic call bounds")?;
    let mut blocks = root.blocks().to_vec();
    let old = &blocks[helper_block];
    blocks[helper_block] = SemanticBasicBlockV1::new(
        old.identity(),
        old.source(),
        old.statements().to_vec(),
        SemanticTerminatorV1::new(
            old.terminator().source(),
            SemanticTerminatorKindV1::Call(restored),
        ),
    )
    .map_err(|_| "context transfer exceeds semantic block bounds")?;
    let mut result = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        root.locals().to_vec(),
        root.entry(),
        blocks,
    )
    .map_err(|_| "context transfer exceeds semantic body bounds")?;
    match root.export() {
        Some(SemanticFunctionExportV1::Kernel(entry)) => {
            result = result.with_kernel_entry(entry.clone())
        }
        _ => return Err("context transfer root lost its kernel export"),
    }
    Ok(Some(result))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
