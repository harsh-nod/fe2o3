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
        out.budget.charge_work(2)?;
        if matches!(operand, Operand::Copy(_))
            && matches!(
                context
                    .types
                    .get(ty.index() as usize)
                    .ok_or_else(mismatch)?
                    .rust_type_kind(),
                SemanticRustTypeKindV1::Execution(_)
            )
            && !context.slots.product_type_copyable_v282(ty, out)?
        {
            return Err(unsupported());
        }
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
    destination: CheckedDestination,
    source_type: TypeId,
    left: Value,
    right: Value,
    scalar: ScalarV30,
    operation: SemanticCheckedBinaryOpV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CheckedDestination {
    Local(usize),
    Object { access: Access, offsets: [u64; 2] },
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
        let destination = if context.slots.has_original_object(
            context.root,
            context.instance,
            assignment.destination().local().index(),
            out,
        )? {
            let access = context.access(assignment.destination(), out)?;
            if !matches!(access.address, Address::Object { offset: 0, .. }) {
                return Err(unsupported());
            }
            let original = context
                .slots
                .checked_object_type_v47(ty, out)?
                .ok_or_else(unsupported)?;
            if original.scalar != scalar
                || original.bytes != access.bytes
                || original.alignment != access.alignment
            {
                return Err(mismatch());
            }
            CheckedDestination::Object {
                access,
                offsets: original.offsets,
            }
        } else {
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
            let Destination::Local(destination) =
                context.destination(assignment.destination(), out)?
            else {
                return Err(unsupported());
            };
            CheckedDestination::Local(destination)
        };
        let left = context.value(checked.left(), out)?;
        let right = context.value(checked.right(), out)?;
        if matches!(destination, CheckedDestination::Local(_))
            && (matches!(left, Value::Read { .. }) || matches!(right, Value::Read { .. }))
        {
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
        match self.destination {
            CheckedDestination::Local(destination) => write!(out,
                "InvocationSourceByteEventV36::Checked {{ destination: {destination}int, source_type: {}int, operation: {operation}int, bits: {width}int, signed: {signed}, left: ",
                self.source_type.index()).map_err(|_| out.error())?,
            CheckedDestination::Object { access, offsets } => {
                write!(out, "InvocationSourceByteEventV36::CheckedObject(InvocationSourceCheckedObjectV44 {{ access: ").map_err(|_| out.error())?;
                emit_access(access, out)?;
                write!(out, ", source_type: {}int, value_offset: {}int, overflow_offset: {}int, operation: {operation}int, bits: {width}int, signed: {signed}, left: ",
                    self.source_type.index(), offsets[0], offsets[1]).map_err(|_| out.error())?;
            }
        }
        emit_value(self.left, out)?;
        write!(out, ", right: ").map_err(|_| out.error())?;
        emit_value(self.right, out)?;
        write!(
            out,
            " }}{}",
            if matches!(self.destination, CheckedDestination::Object { .. }) {
                ")"
            } else {
                ""
            }
        )
        .map_err(|_| out.error())
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
        + size_of::<CheckedDestination>()
        + size_of::<[u64; 2]>()
        + size_of::<&[u64]>()
        + 4 * size_of::<TypeId>()
        + 2 * size_of::<super::super::slots::SourceAggregateLeafV42<'_, '_, '_>>()
        + 8 * size_of::<usize>()
        + 6 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_source_checked_object_emission_v44_tests.rs"]
mod checked_object_emission_tests;
