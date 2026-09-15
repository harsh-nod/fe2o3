use super::*;
use fe2o3_mir_model::semantic_direct_call_expansion_v1::{
    SemanticCallInstanceIdV1, SemanticExpandedTerminatorOriginV1,
};

impl<'a> ScopedCaptures<'a> {
    // A Narrow record authenticates both the original getter and the normal
    // return aggregate. Neither occurrence is selected by shape alone.
    pub(super) fn narrow(
        &mut self,
        source: &AdmittedInertSemanticMirV1,
        view: &'a SemanticExpandedRootV1,
        binding: &SemanticExpandedDefinedCapabilityV1,
        record: SemanticPolicyGfx950NarrowV1,
        limit: usize,
    ) -> Result<u32, ProductionSemanticSsaErrorV1> {
        self.charge(128, limit)?;
        let ids = record.types();
        let original = source
            .functions()
            .get(record.function().index() as usize)
            .ok_or_else(mismatch)?;
        if original.blocks().len() != 2 || original.locals().len() != 3 {
            return Err(mismatch());
        }
        let entry = &original.blocks()[original.entry().index() as usize];
        let SemanticTerminatorKindV1::Call(call) = entry.terminator().kind() else {
            return Err(mismatch());
        };
        let destination = call.destination().ok_or_else(mismatch)?;
        if !entry.statements().is_empty()
            || source.callables().get(call.callee().index() as usize)
                != Some(&SemanticCallableDeclV1::Defined {
                    function: record.projection().function(),
                })
            || call.arguments().len() != 1
            || !call.variadic_argument_abis().is_empty()
            || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
            || destination.edge().target() == original.entry()
            || !destination.place().projections().is_empty()
            || destination.place().ty() != ids.bind.matrix_reference
        {
            return Err(mismatch());
        }
        let entry_block = expanded_block(view, binding.callee_instance(), original.entry())?;
        let exit_block =
            expanded_block(view, binding.callee_instance(), destination.edge().target())?;
        if entry_block != binding.expanded_entry_block().index() {
            return Err(mismatch());
        }
        let SemanticExpandedTerminatorOriginV1::CallEntry {
            callee: getter_instance,
        } = view.block_origins()[entry_block as usize].terminator()
        else {
            return Err(mismatch());
        };
        let frame = view
            .instances()
            .get(getter_instance.index() as usize)
            .ok_or_else(mismatch)?;
        let getter = source
            .functions()
            .get(record.projection().function().index() as usize)
            .ok_or_else(mismatch)?;
        if frame.parent() != Some(binding.callee_instance())
            || frame.function() != record.projection().function()
            || frame.function_identity() != getter.identity()
            || frame.call_block() != Some(original.entry())
            || getter.locals().len() != 2
            || getter.blocks().len() != 1
            || frame.local_count() as usize != getter.locals().len()
            || frame.block_count() as usize != getter.blocks().len()
        {
            return Err(mismatch());
        }
        let getter_block = expanded_block(view, getter_instance, getter.entry())?;
        let expected_argument = unique_local(
            getter,
            SemanticLocalRoleV1::Argument(0),
            ids.bound_reference,
        )?;
        let expected_return = unique_local(
            getter,
            SemanticLocalRoleV1::Return,
            ids.bind.matrix_reference,
        )?;
        let argument = expanded_local(
            view,
            getter_instance,
            expected_argument,
            ids.bound_reference,
        )?;
        let returned = expanded_local(
            view,
            getter_instance,
            expected_return,
            ids.bind.matrix_reference,
        )?;
        let temporary = expanded_local(
            view,
            binding.callee_instance(),
            destination.place().local(),
            ids.bind.matrix_reference,
        )?;
        let [receiver] = binding.callee_arguments() else {
            return Err(mismatch());
        };
        let site = SemanticTransparentBorrowSiteV1 {
            block: getter_block,
            statement: 0,
        };
        let assignment = original_assignment(view, site)?;
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
        else {
            return Err(mismatch());
        };
        let [deref, field] = place.projections() else {
            return Err(mismatch());
        };
        if place.local() != argument
            || place.ty() != ids.bind.matrix_reference
            || deref.kind() != SemanticProjectionKindV1::Dereference
            || deref.result_type() != ids.bind.bound
            || field.kind() != SemanticProjectionKindV1::Field(0)
            || field.result_type() != ids.bind.matrix_reference
            || !plain(
                assignment.destination(),
                returned,
                ids.bind.matrix_reference,
            )
            || assignment.value().result_type() != ids.bind.matrix_reference
            || view.block_origins()[getter_block as usize].terminator()
                != (SemanticExpandedTerminatorOriginV1::CallReturn {
                    callee: getter_instance,
                })
            || !goto(view, entry_block, getter_block)
            || !goto(view, getter_block, exit_block)
        {
            return Err(mismatch());
        }
        self.transfer(
            view,
            entry_block,
            SemanticExpandedStatementOriginV1::ParameterTransfer {
                callee: getter_instance,
                argument: 0,
            },
            argument,
            *receiver,
            ids.bound_reference,
            false,
            limit,
        )?;
        self.transfer(
            view,
            getter_block,
            SemanticExpandedStatementOriginV1::ReturnTransfer {
                callee: getter_instance,
            },
            temporary,
            returned,
            ids.bind.matrix_reference,
            true,
            limit,
        )?;
        // This is a closed read-only use of the bound reference. The getter's
        // exact result is returned only to the unused Narrow temporary. Other
        // uses, duplicate assignments and escapes still visit the common flow.
        self.insert(site, assignment, [argument.index(), 0], 1, limit)?;
        Ok(exit_block)
    }

    fn transfer(
        &mut self,
        view: &SemanticExpandedRootV1,
        block: u32,
        marker: SemanticExpandedStatementOriginV1,
        destination: SemanticLocalIdV1,
        source: SemanticLocalIdV1,
        ty: SemanticTypeIdV1,
        moved: bool,
        limit: usize,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        let body = &view.body().blocks()[block as usize];
        let origins = view.block_origins()[block as usize].statements();
        if body.statements().len() != origins.len() {
            return Err(mismatch());
        }
        let mut found = false;
        for (statement, origin) in body.statements().iter().zip(origins) {
            self.charge(1, limit)?;
            if *origin != marker {
                continue;
            }
            if found {
                return Err(mismatch());
            }
            found = true;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                return Err(mismatch());
            };
            let original = match (assignment.value().kind(), moved) {
                (SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)), false)
                | (SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place)), true) => place,
                _ => return Err(mismatch()),
            };
            if !plain(assignment.destination(), destination, ty)
                || !plain(original, source, ty)
                || assignment.value().result_type() != ty
            {
                return Err(mismatch());
            }
        }
        if !found {
            return Err(mismatch());
        }
        Ok(())
    }
}

fn expanded_block(
    view: &SemanticExpandedRootV1,
    instance: SemanticCallInstanceIdV1,
    original: SemanticBlockIdV1,
) -> Result<u32, ProductionSemanticSsaErrorV1> {
    let frame = view
        .instances()
        .get(instance.index() as usize)
        .ok_or_else(mismatch)?;
    if original.index() >= frame.block_count() {
        return Err(mismatch());
    }
    let index = frame
        .block_start()
        .checked_add(original.index())
        .ok_or_else(mismatch)?;
    let origin = view
        .block_origins()
        .get(index as usize)
        .ok_or_else(mismatch)?;
    if origin.instance() != instance
        || origin.function() != frame.function()
        || origin.block() != original
        || view.body().blocks().get(index as usize).is_none()
    {
        return Err(mismatch());
    }
    Ok(index)
}

fn expanded_local(
    view: &SemanticExpandedRootV1,
    instance: SemanticCallInstanceIdV1,
    original: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
) -> Result<SemanticLocalIdV1, ProductionSemanticSsaErrorV1> {
    let frame = view
        .instances()
        .get(instance.index() as usize)
        .ok_or_else(mismatch)?;
    if original.index() >= frame.local_count() {
        return Err(mismatch());
    }
    let index = frame
        .local_start()
        .checked_add(original.index())
        .ok_or_else(mismatch)?;
    let origin = view
        .local_origins()
        .get(index as usize)
        .ok_or_else(mismatch)?;
    if origin.instance() != instance
        || origin.function() != frame.function()
        || origin.local() != original
        || view
            .body()
            .locals()
            .get(index as usize)
            .is_none_or(|local| local.ty() != ty)
    {
        return Err(mismatch());
    }
    Ok(SemanticLocalIdV1::from_index(index))
}

fn unique_local(
    body: &SemanticFunctionDeclV1,
    role: SemanticLocalRoleV1,
    ty: SemanticTypeIdV1,
) -> Result<SemanticLocalIdV1, ProductionSemanticSsaErrorV1> {
    let mut found = None;
    for (index, local) in body.locals().iter().enumerate() {
        if local.role() != role {
            continue;
        }
        if local.ty() != ty
            || found
                .replace(SemanticLocalIdV1::from_index(index as u32))
                .is_some()
        {
            return Err(mismatch());
        }
    }
    found.ok_or_else(mismatch)
}

fn plain(place: &SemanticPlaceV1, local: SemanticLocalIdV1, ty: SemanticTypeIdV1) -> bool {
    place.local() == local && place.ty() == ty && place.projections().is_empty()
}

fn goto(view: &SemanticExpandedRootV1, from: u32, to: u32) -> bool {
    matches!(view.body().blocks()[from as usize].terminator().kind(), SemanticTerminatorKindV1::Goto(edge)
        if edge.role() == SemanticEdgeRoleV1::Goto && edge.target().index() == to)
}
