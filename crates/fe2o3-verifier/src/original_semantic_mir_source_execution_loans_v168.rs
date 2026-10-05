//! Original execution loans retain exact borrow occurrences, never pointer bits.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionSourceExecutionBorrowCoordinatesV163 as Coordinates,
    ProductionSourceSsaEndpointV36 as Endpoint,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBorrowKindV1 as BorrowKind, SemanticExecutionRoleV29 as ExecutionRole,
    SemanticFunctionIdV1 as FunctionId, SemanticMutabilityV1 as Mutability,
    SemanticRustTypeKindV1 as RustType,
};
use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as OperandRole;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum Role {
    Context,
    Workgroup,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct Recipe {
    pub reference_type: u32,
    pub source_type: u32,
    pub mutable: bool,
    pub role: Role,
    pub instance: usize,
    pub block: u32,
    pub statement: usize,
}

fn role(types: &[Type], ty: TypeId) -> Result<Option<Role>> {
    let declaration = types.get(ty.index() as usize).ok_or_else(mismatch)?;
    Ok(match declaration.rust_type_kind() {
        RustType::Execution(ExecutionRole::KernelContext) => Some(Role::Context),
        RustType::Execution(ExecutionRole::Workgroup) => Some(Role::Workgroup),
        RustType::Execution(_) => return Err(unsupported()),
        _ => None,
    })
}

pub(in super::super) fn nominal_reference(types: &[Type], ty: TypeId) -> Result<Option<TypeId>> {
    let declaration = types.get(ty.index() as usize).ok_or_else(mismatch)?;
    let Shape::Pointer(pointer) = declaration.shape() else {
        return Ok(None);
    };
    if pointer.kind() != PointerKind::Reference || pointer.metadata() != PointerMetadata::None {
        return Ok(None);
    }
    Ok(role(types, pointer.pointee())?.map(|_| pointer.pointee()))
}

impl Recipe {
    pub(in super::super) fn derive(
        slots: &SourceSlots<'_, '_>,
        endpoint: &Endpoint<'_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<(Self, Coordinates)> {
        out.budget.charge_work(12)?;
        let coordinates = endpoint
            .execution_borrow_v163(out.budget)?
            .ok_or_else(mismatch)?;
        let ty = endpoint.source_type(out.budget)?;
        let semantic = slots
            .correspondence(out)?
            .source(out.budget)?
            .source_semantic(out.budget)?;
        let Shape::Pointer(pointer) = semantic
            .types()
            .get(ty.index() as usize)
            .ok_or_else(mismatch)?
            .shape()
        else {
            return Err(mismatch());
        };
        let mutable = match (coordinates.kind, pointer.mutability()) {
            (BorrowKind::Shared, Mutability::Immutable) => false,
            (BorrowKind::Mutable, Mutability::Mutable) => true,
            _ => return Err(mismatch()),
        };
        if pointer.kind() != PointerKind::Reference
            || pointer.metadata() != PointerMetadata::None
            || pointer.pointee() != coordinates.referent_type
        {
            return Err(mismatch());
        }
        let role = role(semantic.types(), coordinates.referent_type)?.ok_or_else(mismatch)?;
        Ok((
            Self {
                reference_type: ty.index(),
                source_type: coordinates.referent_type.index(),
                mutable,
                role,
                instance: coordinates.site.instance,
                block: coordinates.site.block.index(),
                statement: coordinates.site.statement,
            },
            coordinates,
        ))
    }

    pub(in super::super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        let role = match self.role {
            Role::Context => "Context",
            Role::Workgroup => "Workgroup",
        };
        write!(out, "InvocationSourceExecutionRecipeV168 {{ reference_type: {}, source_type: {}, role: InvocationSourceExecutionRoleV168::{role}, mutable: {}, instance: {}, block: {}, statement: {} }}",
            self.reference_type, self.source_type, self.mutable, self.instance, self.block, self.statement)
            .map_err(|_| out.error())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExecutionEvent {
    Borrow {
        destination: usize,
        input: usize,
        recipe: Recipe,
        parent: Option<Recipe>,
    },
    Transfer {
        destination: usize,
        input: usize,
        recipe: Recipe,
        moved: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct ExecutionOperand {
    pub local: usize,
    pub recipe: Recipe,
    pub moved: bool,
}

impl ExecutionOperand {
    pub(in super::super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        write!(
            out,
            "InvocationSourceExecutionOperandV168 {{ local: {}, moved: {}, recipe: ",
            self.local, self.moved
        )
        .map_err(|_| out.error())?;
        self.recipe.emit(out)?;
        write!(out, " }}").map_err(|_| out.error())
    }
}

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn call_argument(
    slots: &SourceSlots<'_, '_>,
    plan: &InvocationPlan<'_, '_>,
    root: usize,
    instance: usize,
    block: usize,
    argument: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Option<ExecutionOperand>> {
    out.budget.charge_work(12)?;
    let source = slots.correspondence(out)?.source(out.budget)?;
    if !std::ptr::eq(source, plan.source(out)?) {
        return Err(mismatch());
    }
    let row = plan.instance(root, instance, out)?;
    let semantic = source.source_semantic(out.budget)?;
    let function = semantic
        .functions()
        .get(row.function.index() as usize)
        .ok_or_else(mismatch)?;
    let Some(Terminator::Call(call)) = function.blocks().get(block).map(|b| b.terminator().kind())
    else {
        return Err(mismatch());
    };
    let operand = call.arguments().get(argument).ok_or_else(mismatch)?;
    if nominal_reference(semantic.types(), operand.ty())?.is_none() {
        return Ok(None);
    }
    let (place, moved) = match operand {
        Operand::Copy(place) => (place, false),
        Operand::Move(place) => (place, true),
        _ => return Err(unsupported()),
    };
    if !row.active
        || row.locals.len() != function.locals().len()
        || !place.projections().is_empty()
        || function
            .locals()
            .get(place.local().index() as usize)
            .map(|l| l.ty())
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
        OperandRole::CallArgument(u32::try_from(argument).map_err(|_| Resource::Arithmetic)?),
        place.local().index(),
        false,
        out,
    )?;
    let endpoint = slots
        .correspondence(out)?
        .ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
    if endpoint.source_function(out.budget)? != row.function
        || endpoint.source_local(out.budget)? != place.local()
        || endpoint.source_type(out.budget)? != place.ty()
    {
        return Err(mismatch());
    }
    let (recipe, _) = Recipe::derive(slots, &endpoint, out)?;
    if recipe.mutable && !moved {
        return Err(mismatch());
    }
    Ok(Some(ExecutionOperand {
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
    local: fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1,
    out: &mut Writer<'_, '_>,
) -> Result<Option<Recipe>> {
    out.budget.charge_work(12)?;
    let row = plan.instance(root, instance, out)?;
    let relation = slots.correspondence(out)?;
    let source = relation.source(out.budget)?;
    if !std::ptr::eq(source, plan.source(out)?) {
        return Err(mismatch());
    }
    let owner = source.source_ssa(out.budget)?;
    let function = owner
        .source_semantic()
        .functions()
        .get(row.function.index() as usize)
        .ok_or_else(mismatch)?;
    let declaration = function
        .locals()
        .get(local.index() as usize)
        .ok_or_else(mismatch)?;
    if nominal_reference(owner.source_semantic().types(), declaration.ty())?.is_none() {
        return Ok(None);
    }
    let fe2o3_mir_model::semantic_mir_v1::SemanticLocalRoleV1::Argument(argument) =
        declaration.role()
    else {
        return Err(mismatch());
    };
    if instance == 0
        || !row.active
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
                || found.replace(entry.value().ok_or_else(mismatch)?).is_some()
            {
                return Err(mismatch());
            }
        }
    }
    let endpoint =
        relation.ssa_typed_endpoint_v36(root, instance, found.ok_or_else(mismatch)?, out.budget)?;
    if endpoint.source_function(out.budget)? != row.function
        || endpoint.source_local(out.budget)? != local
        || endpoint.source_type(out.budget)? != declaration.ty()
    {
        return Err(mismatch());
    }
    let (recipe, _) = Recipe::derive(slots, &endpoint, out)?;
    // Unique helper loans and reference returns remain explicit unsupported paths.
    if recipe.mutable {
        return Err(unsupported());
    }
    Ok(Some(recipe))
}

fn recipe_at(
    context: &Context<'_, '_, '_>,
    function: FunctionId,
    block: usize,
    statement: usize,
    operand: OperandRole,
    place: &Place,
    define: bool,
    out: &mut Writer<'_, '_>,
) -> Result<(Recipe, Coordinates)> {
    let value = witness_events::original_value(
        context.slots,
        function,
        block,
        Some(statement),
        operand,
        place.local().index(),
        define,
        out,
    )?;
    let endpoint = context.slots.correspondence(out)?.ssa_typed_endpoint_v36(
        context.root,
        context.instance,
        value,
        out.budget,
    )?;
    if endpoint.source_type(out.budget)? != place.ty()
        || endpoint.source_local(out.budget)? != place.local()
        || endpoint.source_function(out.budget)? != function
    {
        return Err(mismatch());
    }
    Recipe::derive(context.slots, &endpoint, out)
}

pub(super) fn derive(
    context: &Context<'_, '_, '_>,
    function: FunctionId,
    block: usize,
    statement: usize,
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    out: &mut Writer<'_, '_>,
) -> Result<Option<ExecutionEvent>> {
    out.budget.charge_work(10)?;
    let destination = assignment.destination();
    let Some(original) = nominal_reference(context.types, destination.ty())? else {
        return Ok(None);
    };
    if !destination.projections().is_empty()
        || destination.ty() != assignment.value().result_type()
        || context.slots.has_original_object(
            context.root,
            context.instance,
            destination.local().index(),
            out,
        )?
    {
        return Err(unsupported());
    }
    let (output, coordinates) = recipe_at(
        context,
        function,
        block,
        statement,
        OperandRole::Destination,
        destination,
        true,
        out,
    )?;
    match assignment.value().kind() {
        Rvalue::Borrow { kind, place } => {
            let reborrow = matches!(place.projections(), [p] if p.kind() == Projection::Dereference
                && p.result_type() == original);
            if place.ty() != original
                || !place.projections().is_empty() && !reborrow
                || !matches!(kind, BorrowKind::Shared | BorrowKind::Mutable)
                || coordinates.kind != *kind
                || coordinates.source_local != place.local()
                || coordinates.destination_local != destination.local()
                || output.instance != context.instance
                || output.block as usize != block
                || output.statement != statement
            {
                return Err(mismatch());
            }
            let parent = if reborrow {
                let ty = context
                    .function
                    .locals()
                    .get(place.local().index() as usize)
                    .ok_or_else(mismatch)?
                    .ty();
                if nominal_reference(context.types, ty)? != Some(original) {
                    return Err(mismatch());
                }
                let value = witness_events::original_value(
                    context.slots,
                    function,
                    block,
                    Some(statement),
                    OperandRole::RvaluePlace,
                    place.local().index(),
                    false,
                    out,
                )?;
                let endpoint = context.slots.correspondence(out)?.ssa_typed_endpoint_v36(
                    context.root,
                    context.instance,
                    value,
                    out.budget,
                )?;
                if endpoint.source_type(out.budget)? != ty
                    || endpoint.source_local(out.budget)? != place.local()
                    || endpoint.source_function(out.budget)? != function
                {
                    return Err(mismatch());
                }
                let (parent, parent_coordinates) = Recipe::derive(context.slots, &endpoint, out)?;
                if coordinates.parent != Some(parent_coordinates.site)
                    || parent.source_type != output.source_type
                    || parent.role != output.role
                    || output.mutable && !parent.mutable
                {
                    return Err(mismatch());
                }
                Some(parent)
            } else {
                if coordinates.parent.is_some() {
                    return Err(mismatch());
                }
                None
            };
            Ok(Some(ExecutionEvent::Borrow {
                destination: context.local(destination.local().index())?,
                input: context.local(place.local().index())?,
                recipe: output,
                parent,
            }))
        }
        Rvalue::Use(operand) => {
            let (place, moved) = match operand {
                Operand::Copy(place) => (place, false),
                Operand::Move(place) => (place, true),
                _ => return Err(unsupported()),
            };
            if !place.projections().is_empty() || place.ty() != destination.ty() {
                return Err(unsupported());
            }
            let (input, _) = recipe_at(
                context,
                function,
                block,
                statement,
                OperandRole::RvalueOperand(0),
                place,
                false,
                out,
            )?;
            if input != output || input.mutable && !moved {
                return Err(mismatch());
            }
            Ok(Some(ExecutionEvent::Transfer {
                destination: context.local(destination.local().index())?,
                input: context.local(place.local().index())?,
                recipe: input,
                moved,
            }))
        }
        _ => Err(unsupported()),
    }
}

impl ExecutionEvent {
    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(out, "InvocationSourceByteEventV36::ExecutionLoan(").map_err(|_| out.error())?;
        match self {
            Self::Borrow {
                destination,
                input,
                recipe,
                parent,
            } => {
                write!(out, "InvocationSourceExecutionEventV168::Borrow {{ destination: {destination}, input: {input}, recipe: ").map_err(|_| out.error())?;
                recipe.emit(out)?;
                write!(out, ", parent: ").map_err(|_| out.error())?;
                if let Some(parent) = parent {
                    write!(out, "Some(").map_err(|_| out.error())?;
                    parent.emit(out)?;
                    write!(out, ")").map_err(|_| out.error())?;
                } else {
                    write!(out, "None").map_err(|_| out.error())?;
                }
            }
            Self::Transfer {
                destination,
                input,
                recipe,
                moved,
            } => {
                write!(out, "InvocationSourceExecutionEventV168::Transfer {{ destination: {destination}, input: {input}, moved: {moved}, recipe: ").map_err(|_| out.error())?;
                recipe.emit(out)?;
            }
        }
        write!(out, " }})").map_err(|_| out.error())
    }
}

pub(in super::super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<Recipe>()
        + h::<Coordinates>()
        + h::<(Recipe, Coordinates)>()
        + h::<ExecutionEvent>()
        + h::<Option<ExecutionEvent>>()
        + h::<Option<Recipe>>()
        + h::<ExecutionOperand>()
        + h::<Option<ExecutionOperand>>()
        + h::<Endpoint<'_, '_>>()
        + h::<fe2o3_mir_model::SsaValueV1>()
        + 32 * size_of::<usize>()
        + 24 * size_of::<&()>()
}
