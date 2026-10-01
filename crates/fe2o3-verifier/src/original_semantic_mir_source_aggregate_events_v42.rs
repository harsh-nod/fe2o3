//! Original aggregate value events. No canonical definition supplies a value.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticAssignmentV1, SemanticCheckedBinaryOpV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AggregatePlace {
    local: usize,
    root_type: TypeId,
    result_type: TypeId,
    first_leaf: usize,
    depth: usize,
}

impl AggregatePlace {
    pub(super) fn derive(
        context: &Context<'_, '_, '_>,
        place: &Place,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(3)?;
        let local = place.local().index();
        let root_type = context
            .function
            .locals()
            .get(local as usize)
            .ok_or_else(mismatch)?
            .ty();
        if !matches!(
            context
                .types
                .get(root_type.index() as usize)
                .map(Type::shape),
            Some(Shape::Tuple(_) | Shape::Aggregate(_) | Shape::Array { .. })
        ) || context
            .slots
            .has_original_object(context.root, context.instance, local, out)?
            || context.descriptor(local, out)?.is_some()
        {
            return Ok(None);
        }
        let Some((range, result_type)) =
            context
                .slots
                .aggregate_component_range(root_type, place.projections(), out)?
        else {
            return Ok(None);
        };
        if range.is_empty() || result_type != place.ty() {
            return Err(mismatch());
        }
        let path = context
            .slots
            .aggregate_leaf(root_type, range.start, out)?
            .path(out)?;
        if place.projections().len() > path.len() {
            return Err(mismatch());
        }
        Ok(Some(Self {
            local: context.local(local)?,
            root_type,
            result_type,
            first_leaf: range.start,
            depth: place.projections().len(),
        }))
    }

    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(out, "InvocationSourceAggregatePlaceV42 {{ local: {}int, root_type: {}int, result_type: {}int, first_leaf: {}int, depth: {}int }}",
            self.local, self.root_type.index(), self.result_type.index(), self.first_leaf, self.depth)
            .map_err(|_| out.error())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Transfer {
    destination: AggregatePlace,
    source: AggregatePlace,
    moved: bool,
}

impl Transfer {
    pub(super) fn derive(
        context: &Context<'_, '_, '_>,
        assignment: &SemanticAssignmentV1,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(2)?;
        let Rvalue::Use(operand) = assignment.value().kind() else {
            return Ok(None);
        };
        let ty = assignment.value().result_type();
        if !matches!(
            context.types.get(ty.index() as usize).map(Type::shape),
            Some(Shape::Tuple(_) | Shape::Aggregate(_) | Shape::Array { .. })
        ) {
            return Ok(None);
        }
        if assignment.destination().ty() != ty || operand.ty() != ty {
            return Err(mismatch());
        }
        let (Operand::Copy(place) | Operand::Move(place)) = operand else {
            return Err(unsupported());
        };
        let source = AggregatePlace::derive(context, place, out)?.ok_or_else(unsupported)?;
        let destination = AggregatePlace::derive(context, assignment.destination(), out)?
            .ok_or_else(unsupported)?;
        Ok(Some(Self {
            destination,
            source,
            moved: matches!(operand, Operand::Move(_)),
        }))
    }

    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(
            out,
            "InvocationSourceByteEventV36::AggregateTransfer {{ destination: "
        )
        .map_err(|_| out.error())?;
        self.destination.emit(out)?;
        write!(out, ", source: ").map_err(|_| out.error())?;
        self.source.emit(out)?;
        write!(out, ", moved: {} }}", self.moved).map_err(|_| out.error())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Checked {
    destination: usize,
    source_type: TypeId,
    left: Value,
    right: Value,
    scalar: ScalarV30,
    operation: SemanticCheckedBinaryOpV1,
}

impl Checked {
    pub(super) fn derive(
        context: &Context<'_, '_, '_>,
        assignment: &SemanticAssignmentV1,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(3)?;
        let Rvalue::CheckedBinary(checked) = assignment.value().kind() else {
            return Ok(None);
        };
        let ty = assignment.value().result_type();
        if assignment.destination().ty() != ty || !assignment.destination().projections().is_empty()
        {
            return Err(unsupported());
        }
        let Some(Shape::Tuple(tuple)) = context.types.get(ty.index() as usize).map(Type::shape)
        else {
            return Err(mismatch());
        };
        let [integer, boolean] = tuple.fields() else {
            return Err(mismatch());
        };
        let scalar = context.scalar(*integer, out)?;
        if !matches!(scalar, ScalarV30::Integer { .. })
            || context.scalar(*boolean, out)? != ScalarV30::Bool
            || checked.left().ty() != *integer
            || checked.right().ty() != *integer
        {
            return Err(mismatch());
        }
        if context.slots.aggregate_leaf_count(ty, out)? != Some(2) {
            return Err(unsupported());
        }
        for (ordinal, expected_ty, expected_scalar) in
            [(0, *integer, scalar), (1, *boolean, ScalarV30::Bool)]
        {
            let leaf = context.slots.aggregate_leaf(ty, ordinal, out)?;
            if leaf.path(out)? != [ordinal as u32]
                || leaf.source_type(out)? != expected_ty
                || leaf.scalar(out)? != expected_scalar
            {
                return Err(mismatch());
            }
        }
        let Destination::Local(destination) = context.destination(assignment.destination(), out)?
        else {
            return Err(unsupported());
        };
        let left = context.value(checked.left(), out)?;
        let right = context.value(checked.right(), out)?;
        if matches!(left, Value::Read { .. }) || matches!(right, Value::Read { .. }) {
            return Err(Error::Statement(
                "checked aggregate memory operands need ordered source read effects",
            ));
        }
        Ok(Some(Self {
            destination,
            source_type: ty,
            left,
            right,
            scalar,
            operation: checked.operation(),
        }))
    }

    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        let ScalarV30::Integer { signed, width } = self.scalar else {
            return Err(mismatch());
        };
        let operation = match self.operation {
            SemanticCheckedBinaryOpV1::Add => 0,
            SemanticCheckedBinaryOpV1::Subtract => 1,
            SemanticCheckedBinaryOpV1::Multiply => 2,
        };
        write!(out, "InvocationSourceByteEventV36::Checked {{ destination: {}int, source_type: {}int, operation: {operation}int, bits: {width}int, signed: {signed}, left: ", self.destination, self.source_type.index()).map_err(|_| out.error())?;
        emit_value(self.left, out)?;
        write!(out, ", right: ").map_err(|_| out.error())?;
        emit_value(self.right, out)?;
        write!(out, " }}").map_err(|_| out.error())
    }
}

pub(super) fn headers() -> usize {
    size_of::<Checked>()
        + 2 * size_of::<Result<Option<Checked>>>()
        + size_of::<AggregatePlace>()
        + 2 * size_of::<Result<Option<AggregatePlace>>>()
        + size_of::<Transfer>()
        + 2 * size_of::<Result<Option<Transfer>>>()
        + 2 * size_of::<Value>()
        + 4 * size_of::<TypeId>()
        + 2 * size_of::<super::super::slots::SourceAggregateLeafV42<'_, '_, '_>>()
        + 8 * size_of::<usize>()
        + 6 * size_of::<&()>()
}
