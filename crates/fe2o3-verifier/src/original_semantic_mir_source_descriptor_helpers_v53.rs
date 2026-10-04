//! Exact original helper-boundary descriptor loans. Physical Slice values alone
//! cannot select this route or manufacture a nominal source reference.
use super::descriptor_loans::Recipe;
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticFunctionIdV1 as FunctionId, SemanticLocalIdV1 as LocalId,
    SemanticLocalRoleV1 as LocalRole,
};
use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Role;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct DescriptorOperand {
    pub local: usize,
    pub recipe: Recipe,
    pub moved: bool,
}

pub(in super::super) fn nominal_reference(
    slots: &SourceSlots<'_, '_>,
    ty: TypeId,
    out: &mut Writer<'_, '_>,
) -> Result<bool> {
    out.budget.charge_work(3)?;
    let semantic = slots
        .correspondence(out)?
        .source(out.budget)?
        .source_semantic(out.budget)?;
    let Some(declaration) = semantic.types().get(ty.index() as usize) else {
        return Err(mismatch());
    };
    let Shape::Pointer(pointer) = declaration.shape() else {
        return Ok(false);
    };
    Ok(pointer.kind() == PointerKind::Reference
        && pointer.metadata() == PointerMetadata::None
        && slots
            .descriptor_slice_class(pointer.pointee(), out)?
            .is_some())
}

fn recipe_for_value(
    slots: &SourceSlots<'_, '_>,
    plan: &InvocationPlan<'_, '_>,
    root: usize,
    instance: usize,
    function: FunctionId,
    local: LocalId,
    ty: TypeId,
    value: fe2o3_mir_model::SsaValueV1,
    out: &mut Writer<'_, '_>,
) -> Result<Recipe> {
    out.budget.charge_work(3)?;
    if !std::ptr::eq(
        slots.correspondence(out)?.source(out.budget)?,
        plan.source(out)?,
    ) {
        return Err(mismatch());
    }
    let endpoint = slots
        .correspondence(out)?
        .ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
    if endpoint.source_function(out.budget)? != function
        || endpoint.source_local(out.budget)? != local
        || endpoint.source_type(out.budget)? != ty
    {
        return Err(mismatch());
    }
    Recipe::derive(slots, plan, root, &endpoint, out)
}

pub(in super::super) fn call_argument(
    slots: &SourceSlots<'_, '_>,
    plan: &InvocationPlan<'_, '_>,
    root: usize,
    instance: usize,
    block: usize,
    argument: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Option<DescriptorOperand>> {
    let row = plan.instance(root, instance, out)?;
    let semantic = slots
        .correspondence(out)?
        .source(out.budget)?
        .source_semantic(out.budget)?;
    let function = semantic
        .functions()
        .get(row.function.index() as usize)
        .ok_or_else(mismatch)?;
    out.budget.charge_work(6)?;
    let Some(Terminator::Call(call)) = function.blocks().get(block).map(|b| b.terminator().kind())
    else {
        return Err(mismatch());
    };
    let operand = call.arguments().get(argument).ok_or_else(mismatch)?;
    if !nominal_reference(slots, operand.ty(), out)? {
        return Ok(None);
    }
    let (place, moved) = match operand {
        Operand::Copy(place) => (place, false),
        Operand::Move(place) => (place, true),
        _ => return Err(unsupported()),
    };
    if !place.projections().is_empty()
        || function
            .locals()
            .get(place.local().index() as usize)
            .map(|local| local.ty())
            != Some(place.ty())
        || slots.has_original_object(root, instance, place.local().index(), out)?
    {
        return Err(unsupported());
    }
    let value = witness_events::original_value(
        slots,
        row.function,
        block,
        None,
        Role::CallArgument(u32::try_from(argument).map_err(|_| Resource::Arithmetic)?),
        place.local().index(),
        false,
        out,
    )?;
    let recipe = recipe_for_value(
        slots,
        plan,
        root,
        instance,
        row.function,
        place.local(),
        place.ty(),
        value,
        out,
    )?;
    if recipe.mutable && !moved {
        return Err(mismatch());
    }
    Ok(Some(DescriptorOperand {
        local: row
            .locals
            .start
            .checked_add(place.local().index() as usize)
            .ok_or(Resource::Arithmetic)?,
        recipe,
        moved,
    }))
}

pub(in super::super) fn entry_recipe(
    slots: &SourceSlots<'_, '_>,
    plan: &InvocationPlan<'_, '_>,
    root: usize,
    instance: usize,
    local: LocalId,
    out: &mut Writer<'_, '_>,
) -> Result<Option<Recipe>> {
    let row = plan.instance(root, instance, out)?;
    let owner = slots
        .correspondence(out)?
        .source(out.budget)?
        .source_ssa(out.budget)?;
    let function = owner
        .source_semantic()
        .functions()
        .get(row.function.index() as usize)
        .ok_or_else(mismatch)?;
    let declaration = function
        .locals()
        .get(local.index() as usize)
        .ok_or_else(mismatch)?;
    if !nominal_reference(slots, declaration.ty(), out)? {
        return Ok(None);
    }
    out.budget.charge_work(5)?;
    let LocalRole::Argument(argument) = declaration.role() else {
        return Err(mismatch());
    };
    if instance == 0
        || row.incoming.is_none()
        || function.abi().source_input_types().get(argument as usize) != Some(&declaration.ty())
    {
        return Err(unsupported());
    }
    let occurrences = owner
        .occurrences_v1()
        .and_then(|all| all.function(row.function))
        .ok_or_else(mismatch)?;
    let mut found = None;
    for entry in occurrences.entry_definitions() {
        out.budget.charge_work(3)?;
        if entry.variable().get() == local.index() {
            if entry.origin()
                != fe2o3_pliron::ProductionSemanticSsaEntryOriginV1::Argument(argument)
            {
                return Err(mismatch());
            }
            if found.replace(entry.value().ok_or_else(mismatch)?).is_some() {
                return Err(mismatch());
            }
        }
    }
    recipe_for_value(
        slots,
        plan,
        root,
        instance,
        row.function,
        local,
        declaration.ty(),
        found.ok_or_else(mismatch)?,
        out,
    )
    .map(Some)
}

pub(in super::super) fn destination_recipe(
    slots: &SourceSlots<'_, '_>,
    plan: &InvocationPlan<'_, '_>,
    root: usize,
    instance: usize,
    block: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Recipe> {
    let row = plan.instance(root, instance, out)?;
    let owner = slots
        .correspondence(out)?
        .source(out.budget)?
        .source_ssa(out.budget)?;
    let function = owner
        .source_semantic()
        .functions()
        .get(row.function.index() as usize)
        .ok_or_else(mismatch)?;
    out.budget.charge_work(8)?;
    let Some(Terminator::Call(call)) = function.blocks().get(block).map(|b| b.terminator().kind())
    else {
        return Err(mismatch());
    };
    let destination = call.destination().ok_or_else(mismatch)?;
    let place = destination.place();
    if destination.edge().role() != fe2o3_mir_model::semantic_mir_v1::SemanticEdgeRoleV1::CallReturn
        || !place.projections().is_empty()
        || !nominal_reference(slots, place.ty(), out)?
        || function
            .locals()
            .get(place.local().index() as usize)
            .map(|l| l.ty())
            != Some(place.ty())
        || slots.has_original_object(root, instance, place.local().index(), out)?
    {
        return Err(mismatch());
    }
    let definitions = owner
        .plan_for_function(row.function)
        .ok_or_else(mismatch)?
        .plan()
        .edge_definitions(fe2o3_mir_model::SsaEdgeIdV1::new(
            fe2o3_mir_model::SsaBlockIdV1::new(
                u32::try_from(block).map_err(|_| Resource::Arithmetic)?,
            ),
            0,
        ))
        .ok_or_else(mismatch)?;
    out.budget.charge_work(definitions.len())?;
    let [definition] = definitions else {
        return Err(mismatch());
    };
    if definition.variable().get() != place.local().index() {
        return Err(mismatch());
    }
    recipe_for_value(
        slots,
        plan,
        root,
        instance,
        row.function,
        place.local(),
        place.ty(),
        definition.value(),
        out,
    )
}

pub(in super::super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<DescriptorOperand>()
        + h::<Option<DescriptorOperand>>()
        + h::<Recipe>()
        + h::<Option<Recipe>>()
        + h::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + h::<Option<fe2o3_mir_model::SsaValueV1>>()
        + 24 * size_of::<usize>()
        + 24 * size_of::<&()>()
}

#[cfg(test)]
pub(in super::super) fn header_oracle() -> usize {
    #[allow(dead_code)]
    struct RecipeFields {
        origin: usize,
        source_type: u32,
        reference_type: u32,
        generation: u32,
        instance: usize,
        block: u32,
        statement: usize,
        mutable: bool,
        metadata_bits: u32,
        width: u16,
    }
    #[allow(dead_code)]
    struct OperandFields {
        local: usize,
        recipe: RecipeFields,
        moved: bool,
    }
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    assert_eq!(size_of::<Recipe>(), size_of::<RecipeFields>());
    assert_eq!(
        std::mem::align_of::<Recipe>(),
        std::mem::align_of::<RecipeFields>()
    );
    assert_eq!(size_of::<DescriptorOperand>(), size_of::<OperandFields>());
    assert_eq!(
        std::mem::align_of::<DescriptorOperand>(),
        std::mem::align_of::<OperandFields>()
    );
    h::<OperandFields>()
        + h::<Option<OperandFields>>()
        + h::<RecipeFields>()
        + h::<Option<RecipeFields>>()
        + h::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + h::<Option<fe2o3_mir_model::SsaValueV1>>()
        + 24 * size_of::<usize>()
        + 24 * size_of::<&()>()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn descriptor_helper_headers_have_independent_operand_and_recipe_fields() {
        assert_eq!(headers(), header_oracle());
    }
}

pub(in super::super) fn return_recipe(
    slots: &SourceSlots<'_, '_>,
    plan: &InvocationPlan<'_, '_>,
    root: usize,
    instance: usize,
    block: usize,
    local: LocalId,
    out: &mut Writer<'_, '_>,
) -> Result<Option<Recipe>> {
    let row = plan.instance(root, instance, out)?;
    let semantic = slots
        .correspondence(out)?
        .source(out.budget)?
        .source_semantic(out.budget)?;
    let function = semantic
        .functions()
        .get(row.function.index() as usize)
        .ok_or_else(mismatch)?;
    let declaration = function
        .locals()
        .get(local.index() as usize)
        .ok_or_else(mismatch)?;
    if !nominal_reference(slots, declaration.ty(), out)? {
        return Ok(None);
    }
    out.budget.charge_work(5)?;
    if instance == 0
        || row.incoming.is_none()
        || declaration.role() != LocalRole::Return
        || function.abi().return_type() != declaration.ty()
        || !matches!(
            function.blocks().get(block).map(|b| b.terminator().kind()),
            Some(Terminator::Return)
        )
        || slots.has_original_object(root, instance, local.index(), out)?
    {
        return Err(unsupported());
    }
    let value = witness_events::original_value(
        slots,
        row.function,
        block,
        None,
        Role::ReturnValue,
        local.index(),
        false,
        out,
    )?;
    recipe_for_value(
        slots,
        plan,
        root,
        instance,
        row.function,
        local,
        declaration.ty(),
        value,
        out,
    )
    .map(Some)
}
