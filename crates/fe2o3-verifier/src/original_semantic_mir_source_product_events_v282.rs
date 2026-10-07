//! Typed original aggregate construction and transport. All operand records are
//! joined to the retained statement before any executable model is emitted.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateKindV1 as AggregateKind, SemanticAssignmentV1 as Assignment,
    SemanticFunctionIdV1 as FunctionId, SemanticRustTypeKindV1 as RustType,
};
use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as OperandRole;

#[cfg(test)]
#[path = "original_semantic_mir_source_product_events_v282_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProductPlace {
    pub local: usize,
    pub root_type: TypeId,
    pub result_type: TypeId,
    first: usize,
    depth: usize,
}

impl ProductPlace {
    pub(super) fn derive(
        context: &Context<'_, '_, '_>,
        place: &Place,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(4)?;
        let local = place.local().index();
        let root_type = context
            .function
            .locals()
            .get(local as usize)
            .ok_or_else(mismatch)?
            .ty();
        if !context.slots.is_product_v282(root_type, out)? {
            return Ok(None);
        }
        if context
            .slots
            .has_original_object(context.root, context.instance, local, out)?
            || context.descriptor(local, out)?.is_some()
        {
            return Err(unsupported());
        }
        let (range, result_type) = context
            .slots
            .product_component_range_v282(root_type, place.projections(), out)?
            .ok_or_else(unsupported)?;
        if range.is_empty() || result_type != place.ty() {
            return Err(mismatch());
        }
        let first = context
            .slots
            .product_component_v282(root_type, range.start, out)?;
        if first.path(out)?.len() < place.projections().len() {
            return Err(mismatch());
        }
        Ok(Some(Self {
            local: context.local(local)?,
            root_type,
            result_type,
            first: range.start,
            depth: place.projections().len(),
        }))
    }

    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(out, "InvocationSourceProductPlaceV282 {{ local: {}int, root_type: {}int, result_type: {}int, first_component: {}int, depth: {}int }}",
            self.local, self.root_type.index(), self.result_type.index(), self.first, self.depth).map_err(|_| out.error())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Destination {
    Local { local: usize, ty: TypeId },
    Product(ProductPlace),
    Aggregate(aggregates::AggregatePlace),
}

impl Destination {
    fn derive(
        context: &Context<'_, '_, '_>,
        place: &Place,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        if let Some(product) = ProductPlace::derive(context, place, out)? {
            return Ok(Self::Product(product));
        }
        if place.projections().is_empty() {
            if context.slots.has_original_object(
                context.root,
                context.instance,
                place.local().index(),
                out,
            )? || context.descriptor(place.local().index(), out)?.is_some()
                || context
                    .function
                    .locals()
                    .get(place.local().index() as usize)
                    .map(|row| row.ty())
                    != Some(place.ty())
            {
                return Err(unsupported());
            }
            return Ok(Self::Local {
                local: context.local(place.local().index())?,
                ty: place.ty(),
            });
        }
        aggregates::AggregatePlace::derive(context, place, out)?
            .map(Self::Aggregate)
            .ok_or_else(unsupported)
    }

    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        match self {
            Self::Local { local, ty } => write!(out, "InvocationSourceProductDestinationV282::Local {{ local: {local}int, source_type: {}int }}", ty.index()).map_err(|_| out.error()),
            Self::Product(place) => {
                write!(out, "InvocationSourceProductDestinationV282::Product(").map_err(|_| out.error())?;
                place.emit(out)?;
                write!(out, ")").map_err(|_| out.error())
            }
            Self::Aggregate(place) => {
                write!(out, "InvocationSourceProductDestinationV282::Aggregate(").map_err(|_| out.error())?;
                place.emit(out)?;
                write!(out, ")").map_err(|_| out.error())
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Transfer {
    destination: Destination,
    input: TypedOperand,
}

impl Transfer {
    pub(super) fn derive(
        context: &Context<'_, '_, '_>,
        plan: &InvocationPlan<'_, '_>,
        function: FunctionId,
        block: usize,
        statement: usize,
        assignment: &Assignment,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        let Rvalue::Use(operand) = assignment.value().kind() else {
            return Ok(None);
        };
        let destination = ProductPlace::derive(context, assignment.destination(), out)?;
        let input = match operand {
            Operand::Copy(place) | Operand::Move(place) => {
                ProductPlace::derive(context, place, out)?
            }
            Operand::Constant(_) => None,
        };
        if destination.is_none() && input.is_none() {
            return Ok(None);
        }
        if operand.ty() != assignment.destination().ty()
            || operand.ty() != assignment.value().result_type()
        {
            return Err(mismatch());
        }
        original_assignment(context, plan, function, block, statement, assignment, out)?;
        if !context
            .slots
            .product_type_supported_v282(operand.ty(), out)?
        {
            return Err(unsupported());
        }
        let destination = Destination::derive(context, assignment.destination(), out)?;
        let input = original_operand(context, plan, function, block, statement, 0, operand, out)?;
        Ok(Some(Self { destination, input }))
    }

    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        write!(
            out,
            "InvocationSourceByteEventV36::ProductTransfer {{ destination: "
        )
        .map_err(|_| out.error())?;
        self.destination.emit(out)?;
        write!(out, ", input: ").map_err(|_| out.error())?;
        emit_input(self.input, out)?;
        write!(out, " }}").map_err(|_| out.error())
    }
}

fn original_assignment(
    context: &Context<'_, '_, '_>,
    plan: &InvocationPlan<'_, '_>,
    function: FunctionId,
    block: usize,
    statement: usize,
    assignment: &Assignment,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(10)?;
    let source = context.slots.correspondence(out)?.source(out.budget)?;
    if !std::ptr::eq(source, plan.source(out)?) {
        return Err(mismatch());
    }
    let semantic = source.source_semantic(out.budget)?;
    let row = plan.instance(context.root, context.instance, out)?;
    let original = semantic
        .functions()
        .get(function.index() as usize)
        .ok_or_else(mismatch)?;
    let actual = original
        .blocks()
        .get(block)
        .and_then(|row| row.statements().get(statement))
        .ok_or_else(mismatch)?;
    if row.function != function
        || !row.active
        || !std::ptr::eq(original, context.function)
        || !matches!(actual.kind(), Statement::Assign(value) if std::ptr::eq(value, assignment))
    {
        return Err(mismatch());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Construct {
    destination: Destination,
    ty: TypeId,
    first: usize,
    count: usize,
}

impl Construct {
    pub(super) fn derive(
        context: &Context<'_, '_, '_>,
        plan: &InvocationPlan<'_, '_>,
        function: FunctionId,
        block: usize,
        statement: usize,
        assignment: &Assignment,
        operands: &mut Vec<TypedOperand>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        let Rvalue::Aggregate(aggregate) = assignment.value().kind() else {
            return Ok(None);
        };
        if matches!(aggregate.kind(), AggregateKind::EnumVariant(_)) {
            return Ok(None);
        }
        original_assignment(context, plan, function, block, statement, assignment, out)?;
        let ty = assignment.value().result_type();
        if !context.slots.product_type_supported_v282(ty, out)? {
            return Err(unsupported());
        }
        let declaration = context
            .types
            .get(ty.index() as usize)
            .ok_or_else(mismatch)?;
        if declaration.rust_type_kind() != RustType::Ordinary
            || assignment.destination().ty() != ty
            || context.slots.descriptor_slice_class(ty, out)?.is_some()
            || context.slots.witness_class(ty, out)?.is_some()
        {
            return Err(unsupported());
        }
        let destination = Destination::derive(context, assignment.destination(), out)?;
        let fields = match (aggregate.kind(), declaration.shape()) {
            (AggregateKind::Tuple, Shape::Tuple(fields))
            | (AggregateKind::Aggregate, Shape::Aggregate(fields)) => Some(fields.fields()),
            (AggregateKind::Array, Shape::Array { length, .. })
                if usize::try_from(*length).map_err(|_| Resource::Arithmetic)?
                    == aggregate.operands().len() =>
            {
                None
            }
            _ => return Err(mismatch()),
        };
        if fields.is_some_and(|fields| fields.len() != aggregate.operands().len()) {
            return Err(mismatch());
        }
        let first = operands.len();
        for (ordinal, operand) in aggregate.operands().iter().enumerate() {
            out.budget.charge_work(4)?;
            let field = match fields {
                Some(fields) => *fields.get(ordinal).ok_or_else(mismatch)?,
                None => match declaration.shape() {
                    Shape::Array { element, .. } => *element,
                    _ => return Err(mismatch()),
                },
            };
            if operand.ty() != field {
                return Err(mismatch());
            }
            if !context.slots.product_type_supported_v282(field, out)? {
                return Err(unsupported());
            }
            let input = original_operand(
                context, plan, function, block, statement, ordinal, operand, out,
            )?;
            if operands.len() == operands.capacity() {
                return Err(Resource::Accounting.into());
            }
            operands.push(input);
        }
        Ok(Some(Self {
            destination,
            ty,
            first,
            count: operands.len() - first,
        }))
    }

    pub(super) fn emit(self, operands: &[TypedOperand], out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(3)?;
        let end = self
            .first
            .checked_add(self.count)
            .ok_or(Resource::Arithmetic)?;
        let operands = operands.get(self.first..end).ok_or_else(mismatch)?;
        write!(
            out,
            "InvocationSourceByteEventV36::ProductConstruct {{ destination: "
        )
        .map_err(|_| out.error())?;
        self.destination.emit(out)?;
        write!(out, ", source_type: {}int, fields: seq![", self.ty.index())
            .map_err(|_| out.error())?;
        for operand in operands {
            emit_input(*operand, out)?;
            write!(out, ",").map_err(|_| out.error())?;
        }
        write!(out, "] }}").map_err(|_| out.error())
    }
}

fn emit_input(operand: TypedOperand, out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.charge_work(2)?;
    write!(
        out,
        "InvocationSourceProductOperandV282 {{ source_type: {}int, operand: ",
        operand.ty.index()
    )
    .map_err(|_| out.error())?;
    operand.emit(out)?;
    write!(out, ", descriptor: ").map_err(|_| out.error())?;
    match operand.kind {
        OperandKind::Descriptor(value) => {
            write!(out, "Some(").map_err(|_| out.error())?;
            value.recipe.emit(out)?;
            write!(out, ")").map_err(|_| out.error())?;
        }
        _ => write!(out, "None").map_err(|_| out.error())?,
    }
    write!(out, " }}").map_err(|_| out.error())
}

fn original_operand(
    context: &Context<'_, '_, '_>,
    plan: &InvocationPlan<'_, '_>,
    function: FunctionId,
    block: usize,
    statement: usize,
    ordinal: usize,
    operand: &Operand,
    out: &mut Writer<'_, '_>,
) -> Result<TypedOperand> {
    out.budget.charge_work(8)?;
    if matches!(operand, Operand::Copy(_))
        && !context
            .slots
            .product_type_copyable_v282(operand.ty(), out)?
    {
        return Err(unsupported());
    }
    if matches!(operand, Operand::Copy(_))
        && matches!(
            context
                .types
                .get(operand.ty().index() as usize)
                .ok_or_else(mismatch)?
                .rust_type_kind(),
            RustType::Execution(_)
        )
    {
        return Err(unsupported());
    }
    let execution = execution_loans::nominal_reference(context.types, operand.ty())?.is_some();
    let descriptor = descriptor_helpers::nominal_reference(context.slots, operand.ty(), out)?;
    if !execution && !descriptor {
        return context.typed_operand(operand, out);
    }
    if execution && descriptor {
        return Err(mismatch());
    }
    let (place, moved) = match operand {
        Operand::Copy(place) => (place, false),
        Operand::Move(place) => (place, true),
        _ => return Err(unsupported()),
    };
    // A product projection already carries its exact retained atom. Bare loan
    // locals still require the original SSA borrow recipe at this occurrence.
    if let Some(product) = ProductPlace::derive(context, place, out)? {
        return Ok(TypedOperand {
            ty: operand.ty(),
            kind: OperandKind::Product {
                place: product,
                moved,
            },
        });
    }
    if !place.projections().is_empty()
        || context
            .function
            .locals()
            .get(place.local().index() as usize)
            .map(|row| row.ty())
            != Some(place.ty())
        || context.slots.has_original_object(
            context.root,
            context.instance,
            place.local().index(),
            out,
        )?
    {
        return Err(unsupported());
    }
    let value = witness_events::original_value(
        context.slots,
        function,
        block,
        Some(statement),
        OperandRole::RvalueOperand(u32::try_from(ordinal).map_err(|_| Resource::Arithmetic)?),
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
    if endpoint.source_function(out.budget)? != function
        || endpoint.source_local(out.budget)? != place.local()
        || endpoint.source_type(out.budget)? != place.ty()
    {
        return Err(mismatch());
    }
    let local = context.local(place.local().index())?;
    let kind = if execution {
        let (recipe, _) = execution_loans::Recipe::derive(context.slots, &endpoint, out)?;
        if recipe.mutable {
            return Err(unsupported());
        }
        OperandKind::Execution(execution_loans::ExecutionOperand {
            local,
            recipe,
            moved,
        })
    } else {
        let recipe =
            descriptor_loans::Recipe::derive(context.slots, plan, context.root, &endpoint, out)?;
        if recipe.mutable && !moved {
            return Err(mismatch());
        }
        OperandKind::Descriptor(descriptor_helpers::DescriptorOperand {
            local,
            recipe,
            moved,
        })
    };
    Ok(TypedOperand {
        ty: operand.ty(),
        kind,
    })
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<ProductPlace>()
        + h::<Destination>()
        + h::<Construct>()
        + h::<Transfer>()
        + h::<TypedOperand>()
        + 3 * size_of::<Vec<TypedOperand>>()
        + 32 * size_of::<usize>()
        + 16 * size_of::<&()>()
}
