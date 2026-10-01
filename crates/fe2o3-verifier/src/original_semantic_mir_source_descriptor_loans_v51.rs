//! Nominal descriptor loans retain the original borrow independently of Slice bits.

use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionSourceReferenceCarrierV38 as Carrier, ProductionSourceSsaEndpointV36 as Endpoint,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBorrowKindV1 as BorrowKind, SemanticFunctionIdV1 as FunctionId,
    SemanticMutabilityV1 as Mutability,
};
use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Role;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct Recipe {
    pub origin: usize,
    pub source_type: u32,
    pub reference_type: u32,
    pub generation: u32,
    pub instance: usize,
    pub block: u32,
    pub statement: usize,
    pub mutable: bool,
    pub metadata_bits: u32,
    pub width: u16,
}

impl Recipe {
    pub(in super::super) fn derive(
        slots: &SourceSlots<'_, '_>,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        endpoint: &Endpoint<'_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.charge_work(16)?;
        let reference = endpoint.reference(out.budget)?.ok_or_else(mismatch)?;
        let ty = endpoint.source_type(out.budget)?;
        let original = reference.origin_type(out.budget)?;
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
        let mutable = match (reference.borrow_kind(out.budget)?, pointer.mutability()) {
            (BorrowKind::Shared, Mutability::Immutable) => false,
            (BorrowKind::Mutable, Mutability::Mutable) => true,
            _ => return Err(mismatch()),
        };
        if reference.carrier(out.budget)? != Carrier::DescriptorSlice
            || pointer.kind() != PointerKind::Reference
            || pointer.metadata() != PointerMetadata::None
            || pointer.pointee() != original
        {
            return Err(mismatch());
        }
        let class = slots
            .descriptor_slice_class(original, out)?
            .ok_or_else(unsupported)?;
        let bits = class.element.bit_width().ok_or_else(unsupported)?;
        if !matches!(bits, 8 | 16 | 32 | 64 | 128) || !matches!(class.metadata_bits, 32 | 64) {
            return Err(unsupported());
        }
        let origin = plan.instance(root, reference.origin_instance(out.budget)?, out)?;
        let local = reference.origin_local(out.budget)?.index() as usize;
        if origin.function != reference.origin_function(out.budget)? || local >= origin.locals.len()
        {
            return Err(mismatch());
        }
        let (instance, block, statement) = reference.borrow_site(out.budget)?;
        Ok(Self {
            origin: origin
                .locals
                .start
                .checked_add(local)
                .ok_or(Resource::Arithmetic)?,
            source_type: original.index(),
            reference_type: ty.index(),
            generation: reference.origin_generation(out.budget)?,
            instance,
            block: block.index(),
            statement: statement.ok_or_else(mismatch)?,
            mutable,
            metadata_bits: class.metadata_bits,
            width: bits / 8,
        })
    }

    pub(in super::super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(out, "InvocationSourceDescriptorRecipeV51 {{ origin: {}, source_type: {}, reference_type: {}, generation: {}, instance: {}, block: {}, statement: {}, mutable: {}, metadata_bits: {}, width: {} }}",
            self.origin, self.source_type, self.reference_type, self.generation,
            self.instance, self.block, self.statement, self.mutable, self.metadata_bits, self.width)
            .map_err(|_| out.error())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DescriptorEvent {
    Borrow {
        destination: usize,
        recipe: Recipe,
        parent: Option<(usize, Recipe)>,
    },
    Transfer {
        destination: usize,
        input: usize,
        recipe: Recipe,
        moved: bool,
    },
}

fn reference_type(types: &[Type], ty: TypeId) -> Option<TypeId> {
    match types.get(ty.index() as usize)?.shape() {
        Shape::Pointer(pointer)
            if pointer.kind() == PointerKind::Reference
                && pointer.metadata() == PointerMetadata::None =>
        {
            Some(pointer.pointee())
        }
        _ => None,
    }
}

fn endpoint<'a, 'source>(
    context: &Context<'_, 'a, 'source>,
    function: FunctionId,
    block: usize,
    statement: usize,
    role: Role,
    place: &Place,
    define: bool,
    out: &mut Writer<'_, '_>,
) -> Result<Endpoint<'a, 'source>> {
    let value = witness_events::original_value(
        context.slots,
        function,
        block,
        Some(statement),
        role,
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
    if endpoint.source_function(out.budget)? != function
        || endpoint.source_local(out.budget)? != place.local()
        || endpoint.source_type(out.budget)? != place.ty()
    {
        return Err(mismatch());
    }
    Ok(endpoint)
}

pub(super) fn derive(
    context: &Context<'_, '_, '_>,
    plan: &InvocationPlan<'_, '_>,
    function: FunctionId,
    block: usize,
    statement: usize,
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    out: &mut Writer<'_, '_>,
) -> Result<Option<DescriptorEvent>> {
    out.budget.charge_work(8)?;
    let destination = assignment.destination();
    let Some(original) = reference_type(context.types, destination.ty()) else {
        return Ok(None);
    };
    if context
        .slots
        .descriptor_slice_class(original, out)?
        .is_none()
    {
        return Ok(None);
    }
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
    let output = endpoint(
        context,
        function,
        block,
        statement,
        Role::Destination,
        destination,
        true,
        out,
    )?;
    let output_recipe = Recipe::derive(context.slots, plan, context.root, &output, out)?;
    match assignment.value().kind() {
        Rvalue::Borrow { kind, place } => {
            let reborrow = matches!(place.projections(), [p] if p.kind() == Projection::Dereference
                && p.result_type() == original);
            if place.ty() != original
                || !place.projections().is_empty() && !reborrow
                || output_recipe.mutable != (*kind == BorrowKind::Mutable)
                || !matches!(kind, BorrowKind::Shared | BorrowKind::Mutable)
                || output_recipe.instance != context.instance
                || output_recipe.block as usize != block
                || output_recipe.statement != statement
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
                if reference_type(context.types, ty) != Some(original) {
                    return Err(mismatch());
                }
                let value = witness_events::original_value(
                    context.slots,
                    function,
                    block,
                    Some(statement),
                    Role::RvaluePlace,
                    place.local().index(),
                    false,
                    out,
                )?;
                let input = context.slots.correspondence(out)?.ssa_typed_endpoint_v36(
                    context.root,
                    context.instance,
                    value,
                    out.budget,
                )?;
                if input.source_type(out.budget)? != ty
                    || input.source_local(out.budget)? != place.local()
                    || input.source_function(out.budget)? != function
                {
                    return Err(mismatch());
                }
                let parent = Recipe::derive(context.slots, plan, context.root, &input, out)?;
                if parent.origin != output_recipe.origin
                    || parent.source_type != output_recipe.source_type
                    || parent.generation != output_recipe.generation
                    || output_recipe.mutable && !parent.mutable
                {
                    return Err(mismatch());
                }
                Some((context.local(place.local().index())?, parent))
            } else {
                if output_recipe.origin != context.local(place.local().index())? {
                    return Err(mismatch());
                }
                None
            };
            Ok(Some(DescriptorEvent::Borrow {
                destination: context.local(destination.local().index())?,
                recipe: output_recipe,
                parent,
            }))
        }
        Rvalue::Use(operand) => {
            let (place, moved) = match operand {
                Operand::Copy(p) => (p, false),
                Operand::Move(p) => (p, true),
                _ => return Err(unsupported()),
            };
            if !place.projections().is_empty() || place.ty() != destination.ty() {
                return Err(unsupported());
            }
            let input = endpoint(
                context,
                function,
                block,
                statement,
                Role::RvalueOperand(0),
                place,
                false,
                out,
            )?;
            let recipe = Recipe::derive(context.slots, plan, context.root, &input, out)?;
            if recipe != output_recipe || recipe.mutable && !moved {
                return Err(mismatch());
            }
            Ok(Some(DescriptorEvent::Transfer {
                destination: context.local(destination.local().index())?,
                input: context.local(place.local().index())?,
                recipe,
                moved,
            }))
        }
        _ => Err(unsupported()),
    }
}

impl DescriptorEvent {
    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(out, "InvocationSourceByteEventV36::Descriptor(").map_err(|_| out.error())?;
        match self {
            Self::Borrow {
                destination,
                recipe,
                parent,
            } => {
                write!(out, "InvocationSourceDescriptorEventV51::Borrow {{ destination: {destination}, recipe: ").map_err(|_| out.error())?;
                recipe.emit(out)?;
                write!(out, ", parent: ").map_err(|_| out.error())?;
                match parent {
                    None => write!(out, "None").map_err(|_| out.error())?,
                    Some((local, parent)) => {
                        write!(out, "Some(({local}, ").map_err(|_| out.error())?;
                        parent.emit(out)?;
                        write!(out, "))").map_err(|_| out.error())?;
                    }
                }
            }
            Self::Transfer {
                destination,
                input,
                recipe,
                moved,
            } => {
                write!(out, "InvocationSourceDescriptorEventV51::Transfer {{ destination: {destination}, input: {input}, moved: {moved}, recipe: ").map_err(|_| out.error())?;
                recipe.emit(out)?;
            }
        }
        write!(out, " }})").map_err(|_| out.error())
    }
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<Recipe>()
        + h::<DescriptorEvent>()
        + h::<Option<DescriptorEvent>>()
        + h::<Option<(usize, Recipe)>>()
        + 2 * h::<Endpoint<'_, '_>>()
        + h::<fe2o3_lower_mir_kernel::ProductionSourceReferenceEndpointV38<'_, '_>>()
        + h::<Role>()
        + h::<BorrowKind>()
        + h::<TypeId>()
        + h::<Option<TypeId>>()
        + 32 * size_of::<usize>()
        + 20 * size_of::<&()>()
}

#[cfg(test)]
mod header_tests {
    use super::*;
    use std::mem::align_of;

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
    enum EventFields {
        Borrow {
            destination: usize,
            recipe: RecipeFields,
            parent: Option<(usize, RecipeFields)>,
        },
        Transfer {
            destination: usize,
            input: usize,
            recipe: RecipeFields,
            moved: bool,
        },
    }

    #[test]
    fn descriptor_loan_headers_match_independent_recipe_and_event_fields() {
        fn envelope<T>() -> usize {
            size_of::<T>() + 2 * size_of::<Result<T>>()
        }
        assert_eq!(size_of::<Recipe>(), size_of::<RecipeFields>());
        assert_eq!(align_of::<Recipe>(), align_of::<RecipeFields>());
        assert_eq!(size_of::<DescriptorEvent>(), size_of::<EventFields>());
        assert_eq!(align_of::<DescriptorEvent>(), align_of::<EventFields>());
        let expected = envelope::<RecipeFields>()
            + envelope::<EventFields>()
            + envelope::<Option<EventFields>>()
            + envelope::<Option<(usize, RecipeFields)>>()
            + 2 * envelope::<Endpoint<'_, '_>>()
            + envelope::<fe2o3_lower_mir_kernel::ProductionSourceReferenceEndpointV38<'_, '_>>()
            + envelope::<Role>()
            + envelope::<BorrowKind>()
            + envelope::<TypeId>()
            + envelope::<Option<TypeId>>()
            + 32 * size_of::<usize>()
            + 20 * size_of::<&()>();
        assert_eq!(headers(), expected);
    }
}
