//! Original statements, independently classified before any physical execution.
//! A missing object-generation join is a refusal, never a scalar fallback.

use super::super::{
    Constant, Function, Operand, Place, Rvalue, ScalarV30, Shape, Statement, Type, TypeId,
    invocations::InvocationPlan,
};
use super::{Error, Resource, Result, Writer, slots::SourceSlots, vector};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBackendPrimitiveV1 as BackendPrimitive, SemanticBackendReprV1 as BackendRepr,
    SemanticBackendScalarV1 as BackendScalar, SemanticFieldsShapeV1 as Fields,
    SemanticPointerKindV1 as PointerKind, SemanticPointerMetadataV1 as PointerMetadata,
    SemanticProjectionKindV1 as Projection, SemanticTerminatorKindV1 as Terminator,
    SemanticVolatilityV1 as Volatility,
};
use std::{fmt::Write as _, mem::size_of, ops::Range};

#[path = "original_semantic_mir_invocation_source_pointers_v36.rs"]
mod pointer_events;
pub(super) use pointer_events::SOURCE_POINTERS_V36;
#[path = "original_semantic_mir_source_witness_borrows_v38.rs"]
pub(super) mod witness_events;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Address {
    Slot { descriptor: usize, offset: u64 },
    Pointer { local: usize, offset: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Access {
    address: Address,
    ty: TypeId,
    bytes: u64,
    alignment: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Value {
    Constant(u128),
    Local { local: usize, moved: bool },
    Read { access: Access, moved: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OperandKind {
    Scalar {
        value: Value,
        scalar: ScalarV30,
    },
    Pointer {
        local: usize,
        moved: bool,
    },
    Slice {
        local: usize,
        moved: bool,
        metadata_bits: u32,
    },
}

// Only archived Call/Switch operands can construct this descriptor. Pointer
// carriers are distinct tagged locals, never scalar memory payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TypedOperand {
    ty: TypeId,
    kind: OperandKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Destination {
    Local(usize),
    Memory(Access),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Event {
    /// The existing scalar graph must still interpret and check this statement.
    Scalar,
    WitnessBorrow(witness_events::Borrow),
    Pointer(pointer_events::Event),
    Transfer {
        destination: Destination,
        value: Value,
        scalar: ScalarV30,
    },
    Address {
        destination: usize,
        access: Access,
    },
    Deinitialize(Access),
    StorageLive {
        descriptor: usize,
        local: usize,
    },
    StorageDead {
        descriptor: usize,
        local: usize,
    },
}

pub(super) struct SourceByteBody<'slots, 'view, 'source> {
    slots: &'slots SourceSlots<'view, 'source>,
    root: usize,
    instance: usize,
    function: usize,
    locals: Range<usize>,
    blocks: Vec<Range<usize>>,
    events: Vec<Event>,
    required: usize,
}

struct Context<'a, 'view, 'source> {
    slots: &'a SourceSlots<'view, 'source>,
    types: &'a [Type],
    function: &'a Function,
    root: usize,
    instance: usize,
    locals: Range<usize>,
}

fn unsupported() -> Error {
    Error::Statement("original MIR typed byte statement is not modeled")
}

fn mismatch() -> Error {
    Error::Statement("original MIR typed byte statement identity or layout differs")
}

pub(super) fn slice_metadata_bits_v36(declaration: &Type, out: &mut Writer<'_, '_>) -> Result<u32> {
    out.budget.charge_work(2)?;
    let BackendRepr::ScalarPair {
        second:
            BackendScalar::Initialized {
                primitive:
                    BackendPrimitive::Integer {
                        signed: false,
                        bits,
                        ..
                    },
                valid_range,
            },
        ..
    } = declaration.layout().backend_repr()
    else {
        return Err(unsupported());
    };
    if !matches!(*bits, 8 | 16 | 32 | 64)
        || valid_range.start() != 0
        || valid_range.end() != (1u128 << bits) - 1
    {
        return Err(unsupported());
    }
    Ok(u32::from(*bits))
}

impl<'slots, 'view, 'source> SourceByteBody<'slots, 'view, 'source> {
    pub(super) fn derive(
        plan: &InvocationPlan<'_, '_>,
        slots: &'slots SourceSlots<'view, 'source>,
        root: usize,
        instance: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        out.budget.reserve_storage(witness_events::headers())?;
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
        out.budget.charge_work(3)?;
        if !row.active
            || row.locals.len() != function.locals().len()
            || row.blocks.len() != function.blocks().len()
        {
            return Err(mismatch());
        }
        let mut count = 0usize;
        for block in function.blocks() {
            out.budget.charge_work(1)?;
            count = count
                .checked_add(block.statements().len())
                .ok_or(Resource::Arithmetic)?;
        }
        let mut blocks = vector(function.blocks().len(), out)?;
        let mut events = vector(count, out)?;
        let context = Context {
            slots,
            types: semantic.types(),
            function,
            root,
            instance,
            locals: row.locals.clone(),
        };
        for (block_ordinal, block) in function.blocks().iter().enumerate() {
            out.budget.charge_work(1)?;
            let start = events.len();
            for (statement_ordinal, statement) in block.statements().iter().enumerate() {
                out.budget.charge_work(1)?;
                let borrowed = match statement.kind() {
                    Statement::Assign(assignment) => witness_events::Borrow::derive(
                        &context,
                        row.function,
                        block_ordinal,
                        statement_ordinal,
                        assignment,
                        out,
                    )?,
                    _ => None,
                };
                let event = match borrowed {
                    Some(borrow) => Event::WitnessBorrow(borrow),
                    None => context.statement(statement.kind(), out)?,
                };
                if events.len() == events.capacity() {
                    return Err(Resource::Accounting.into());
                }
                events.push(event);
            }
            blocks.push(start..events.len());
        }
        if events.len() != count {
            return Err(mismatch());
        }
        Ok(Self {
            slots,
            root,
            instance,
            function: row.function.index() as usize,
            locals: row.locals.clone(),
            blocks,
            events,
            required: out.budget.storage(),
        })
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        let source = self.slots.correspondence(out)?.source(out.budget)?;
        if out.budget.storage() < self.required {
            return Err(source
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        Ok(())
    }

    fn context(&self, out: &mut Writer<'_, '_>) -> Result<Context<'_, 'view, 'source>> {
        self.check(out)?;
        let semantic = self
            .slots
            .correspondence(out)?
            .source(out.budget)?
            .source_semantic(out.budget)?;
        out.budget.charge_work(2)?;
        let function = semantic
            .functions()
            .get(self.function)
            .ok_or_else(mismatch)?;
        if function.locals().len() != self.locals.len()
            || function.blocks().len() != self.blocks.len()
        {
            return Err(mismatch());
        }
        Ok(Context {
            slots: self.slots,
            types: semantic.types(),
            function,
            root: self.root,
            instance: self.instance,
            locals: self.locals.clone(),
        })
    }

    /// The call-transfer consumer still checks the exact callee ABI and order.
    /// Evaluation of these descriptors must thread the current source state.
    pub(super) fn call_argument(
        &self,
        block: usize,
        argument: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<TypedOperand> {
        let context = self.context(out)?;
        out.budget.charge_work(3)?;
        let Some(Terminator::Call(call)) = context
            .function
            .blocks()
            .get(block)
            .map(|block| block.terminator().kind())
        else {
            return Err(mismatch());
        };
        let operand = call.arguments().get(argument).ok_or_else(mismatch)?;
        context.typed_operand(operand, out)
    }

    pub(super) fn switch_operand(
        &self,
        block: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<TypedOperand> {
        let context = self.context(out)?;
        out.budget.charge_work(3)?;
        let Some(Terminator::SwitchInt { discriminant, .. }) = context
            .function
            .blocks()
            .get(block)
            .map(|block| block.terminator().kind())
        else {
            return Err(mismatch());
        };
        let value = context.typed_operand(discriminant, out)?;
        if !matches!(
            value.kind,
            OperandKind::Scalar {
                scalar: ScalarV30::Bool | ScalarV30::Integer { .. },
                ..
            }
        ) {
            return Err(unsupported());
        }
        Ok(value)
    }

    pub(super) fn event_at(
        &self,
        block: usize,
        statement: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Event> {
        self.check(out)?;
        out.budget.charge_work(3)?;
        let range = self.blocks.get(block).ok_or_else(mismatch)?;
        if statement >= range.len() {
            return Err(mismatch());
        }
        self.events
            .get(
                range
                    .start
                    .checked_add(statement)
                    .ok_or(Resource::Arithmetic)?,
            )
            .copied()
            .ok_or_else(mismatch)
    }

    pub(super) fn emit(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(out)?;
        write!(out, "open spec fn invocation_source_byte_event_{}_{}_v36(block: int, statement: int) -> Option<InvocationSourceByteEventV36> {{\n", self.root, self.instance).map_err(|_| out.error())?;
        for (block, range) in self.blocks.iter().enumerate() {
            out.budget.charge_work(1)?;
            for (statement, event) in self.events[range.clone()].iter().enumerate() {
                out.budget.charge_work(1)?;
                write!(
                    out,
                    " if block == {block} && statement == {statement} {{ Some("
                )
                .map_err(|_| out.error())?;
                emit_event(*event, out)?;
                write!(out, ") }} else").map_err(|_| out.error())?;
            }
        }
        write!(out, " {{ None }}\n}}\n").map_err(|_| out.error())
    }
}

impl Context<'_, '_, '_> {
    fn local(&self, local: u32) -> Result<usize> {
        let at = self
            .locals
            .start
            .checked_add(local as usize)
            .ok_or(Resource::Arithmetic)?;
        if at >= self.locals.end {
            return Err(mismatch());
        }
        Ok(at)
    }

    fn scalar(&self, ty: TypeId, out: &mut Writer<'_, '_>) -> Result<ScalarV30> {
        out.budget.charge_work(2)?;
        let scalar = ScalarV30::from_source(self.types, ty)?;
        let declaration = self.types.get(ty.index() as usize).ok_or_else(mismatch)?;
        let width = match scalar {
            ScalarV30::Unit => 0,
            ScalarV30::Bool => 1,
            ScalarV30::Integer { width, .. } => u64::from(width / 8),
        };
        if declaration.layout().size_bytes() != Some(width) {
            return Err(mismatch());
        }
        Ok(scalar)
    }

    fn ordinary(&self, volatility: Volatility, atomic: bool) -> Result<()> {
        if volatility != Volatility::NonVolatile || atomic {
            return Err(unsupported());
        }
        Ok(())
    }

    fn descriptor(&self, local: u32, out: &mut Writer<'_, '_>) -> Result<Option<usize>> {
        Ok(self
            .slots
            .legacy_descriptor_by_source(self.root, self.instance, local, out)?
            .map(|(ordinal, _)| ordinal))
    }

    fn access(&self, place: &Place, out: &mut Writer<'_, '_>) -> Result<Access> {
        out.budget.charge_work(4)?;
        let declaration = self
            .function
            .locals()
            .get(place.local().index() as usize)
            .ok_or_else(mismatch)?;
        let mut ty = declaration.ty();
        let mut projections = place.projections();
        let base = if projections
            .first()
            .is_some_and(|p| p.kind() == Projection::Dereference)
        {
            let pointer = match self.types.get(ty.index() as usize).map(Type::shape) {
                Some(Shape::Pointer(pointer)) => pointer,
                _ => return Err(mismatch()),
            };
            if pointer.metadata() != PointerMetadata::None {
                return Err(unsupported());
            }
            if self.descriptor(place.local().index(), out)?.is_some() {
                // Pointer-valued memory is outside the stored-provenance model.
                return Err(unsupported());
            }
            ty = pointer.pointee();
            if projections[0].result_type() != ty {
                return Err(mismatch());
            }
            projections = &projections[1..];
            Address::Pointer {
                local: self.local(place.local().index())?,
                offset: 0,
            }
        } else {
            let (descriptor, frame) = self
                .slots
                .legacy_descriptor_by_source(self.root, self.instance, place.local().index(), out)?
                .ok_or_else(unsupported)?;
            let layout = self
                .types
                .get(ty.index() as usize)
                .ok_or_else(mismatch)?
                .layout();
            if frame.semantic_type() != ty
                || frame.source_generation().is_some()
                || layout.size_bytes() != Some(frame.bytes())
                || u64::from(frame.alignment()) % layout.alignment_bytes() != 0
            {
                return Err(mismatch());
            }
            Address::Slot {
                descriptor,
                offset: 0,
            }
        };
        let mut offset = 0u64;
        for projection in projections {
            out.budget.charge_work(6)?;
            let parent = self.types.get(ty.index() as usize).ok_or_else(mismatch)?;
            let (next, delta) = match (projection.kind(), parent.shape()) {
                (Projection::Field(field), Shape::Tuple(fields) | Shape::Aggregate(fields)) => {
                    let field = field as usize;
                    let next = *fields.fields().get(field).ok_or_else(mismatch)?;
                    let delta = *parent
                        .layout()
                        .fields()
                        .source_order_offsets_bytes()
                        .and_then(|offsets| offsets.get(field))
                        .ok_or_else(unsupported)?;
                    (next, delta)
                }
                (
                    Projection::ConstantIndex {
                        offset,
                        minimum_length,
                        from_end,
                    },
                    Shape::Array { element, length },
                ) => {
                    if minimum_length > *length
                        || (from_end && (offset == 0 || offset > *length))
                        || (!from_end && offset >= *length)
                    {
                        return Err(mismatch());
                    }
                    let ordinal = if from_end { length - offset } else { offset };
                    let stride = match parent.layout().fields() {
                        Fields::Array {
                            stride_bytes,
                            count,
                        } if count == length => *stride_bytes,
                        _ => return Err(unsupported()),
                    };
                    (
                        *element,
                        ordinal.checked_mul(stride).ok_or(Resource::Arithmetic)?,
                    )
                }
                _ => return Err(unsupported()),
            };
            let child = self.types.get(next.index() as usize).ok_or_else(mismatch)?;
            let extent = child.layout().size_bytes().ok_or_else(unsupported)?;
            if next != projection.result_type()
                || delta.checked_add(extent).ok_or(Resource::Arithmetic)?
                    > parent.layout().size_bytes().ok_or_else(unsupported)?
            {
                return Err(mismatch());
            }
            offset = offset.checked_add(delta).ok_or(Resource::Arithmetic)?;
            ty = next;
        }
        if ty != place.ty() {
            return Err(mismatch());
        }
        let layout = self
            .types
            .get(ty.index() as usize)
            .ok_or_else(mismatch)?
            .layout();
        let address = match base {
            Address::Slot { descriptor, .. } => Address::Slot { descriptor, offset },
            Address::Pointer { local, .. } => Address::Pointer { local, offset },
        };
        Ok(Access {
            address,
            ty,
            bytes: layout.size_bytes().ok_or_else(unsupported)?,
            alignment: layout.alignment_bytes(),
        })
    }

    fn destination(&self, place: &Place, out: &mut Writer<'_, '_>) -> Result<Destination> {
        out.budget.charge_work(2)?;
        let declaration = self
            .function
            .locals()
            .get(place.local().index() as usize)
            .ok_or_else(mismatch)?;
        if place.projections().is_empty() && self.descriptor(place.local().index(), out)?.is_none()
        {
            if declaration.ty() != place.ty() {
                return Err(mismatch());
            }
            Ok(Destination::Local(self.local(place.local().index())?))
        } else {
            Ok(Destination::Memory(self.access(place, out)?))
        }
    }

    fn value(&self, operand: &Operand, out: &mut Writer<'_, '_>) -> Result<Value> {
        out.budget.charge_work(3)?;
        let scalar = self.scalar(operand.ty(), out)?;
        match operand {
            Operand::Constant(constant) => {
                let bits = match (constant.value(), scalar) {
                    (Constant::ZeroSized, ScalarV30::Unit) => 0,
                    (Constant::Scalar(value), scalar) if scalar != ScalarV30::Unit => {
                        let declaration = self
                            .types
                            .get(operand.ty().index() as usize)
                            .ok_or_else(mismatch)?;
                        if Some(u64::from(value.size_bytes())) != declaration.layout().size_bytes()
                        {
                            return Err(mismatch());
                        }
                        value.bits()
                    }
                    _ => return Err(unsupported()),
                };
                if bits >= (1u128 << scalar.width()) {
                    return Err(mismatch());
                }
                Ok(Value::Constant(bits))
            }
            Operand::Copy(place) | Operand::Move(place) => {
                let moved = matches!(operand, Operand::Move(_));
                match self.destination(place, out)? {
                    Destination::Local(local) => Ok(Value::Local { local, moved }),
                    Destination::Memory(access) if scalar != ScalarV30::Unit => {
                        Ok(Value::Read { access, moved })
                    }
                    _ => Err(unsupported()),
                }
            }
        }
    }

    fn typed_operand(&self, operand: &Operand, out: &mut Writer<'_, '_>) -> Result<TypedOperand> {
        out.budget.charge_work(3)?;
        let ty = operand.ty();
        let declaration = self.types.get(ty.index() as usize).ok_or_else(mismatch)?;
        let kind = if let Shape::Pointer(pointer) = declaration.shape() {
            // Stable witness references have logical scalar carriers. Ordinary
            // memory operands cannot transport their loan/version metadata yet.
            if self.slots.witness_class(pointer.pointee(), out)?.is_some() {
                return Err(unsupported());
            }
            let place = match operand {
                Operand::Copy(place) | Operand::Move(place) => place,
                Operand::Constant(_) => return Err(unsupported()),
            };
            // Pointer-valued memory has no stored-provenance interpretation.
            // Do not let the scalar reader manufacture a tagged carrier.
            if !place.projections().is_empty() {
                return Err(unsupported());
            }
            let Destination::Local(local) = self.destination(place, out)? else {
                return Err(unsupported());
            };
            let moved = matches!(operand, Operand::Move(_));
            match pointer.metadata() {
                PointerMetadata::None => OperandKind::Pointer { local, moved },
                PointerMetadata::SliceLength
                    if matches!(
                        self.types
                            .get(pointer.pointee().index() as usize)
                            .map(Type::shape),
                        Some(Shape::Slice { .. })
                    ) =>
                {
                    OperandKind::Slice {
                        local,
                        moved,
                        metadata_bits: slice_metadata_bits_v36(declaration, out)?,
                    }
                }
                _ => return Err(unsupported()),
            }
        } else if let Some(metadata_bits) = self.slots.descriptor_slice_bits(ty, out)? {
            let place = match operand {
                Operand::Copy(place) | Operand::Move(place) => place,
                Operand::Constant(_) => return Err(unsupported()),
            };
            if !place.projections().is_empty() {
                return Err(unsupported());
            }
            let Destination::Local(local) = self.destination(place, out)? else {
                return Err(unsupported());
            };
            OperandKind::Slice {
                local,
                moved: matches!(operand, Operand::Move(_)),
                metadata_bits,
            }
        } else {
            OperandKind::Scalar {
                value: self.value(operand, out)?,
                scalar: self.scalar(ty, out)?,
            }
        };
        Ok(TypedOperand { ty, kind })
    }

    fn statement(&self, statement: &Statement, out: &mut Writer<'_, '_>) -> Result<Event> {
        out.budget.charge_work(2)?;
        if let Some(event) = pointer_events::derive(self, statement, out)? {
            return Ok(Event::Pointer(event));
        }
        match statement {
            Statement::Assign(assignment) => {
                if assignment.destination().ty() != assignment.value().result_type() {
                    return Err(mismatch());
                }
                let destination = self.destination(assignment.destination(), out)?;
                match assignment.value().kind() {
                    Rvalue::Use(value) => {
                        if value.ty() != assignment.destination().ty() {
                            return Err(mismatch());
                        }
                        let scalar = self.scalar(value.ty(), out)?;
                        let value = self.value(value, out)?;
                        if scalar == ScalarV30::Unit
                            && matches!(destination, Destination::Memory(_))
                        {
                            return Err(unsupported());
                        }
                        Ok(Event::Transfer {
                            destination,
                            value,
                            scalar,
                        })
                    }
                    Rvalue::Load(load) => {
                        self.ordinary(load.volatility(), load.atomic().is_some())?;
                        if load.source().ty() != assignment.destination().ty() {
                            return Err(mismatch());
                        }
                        let scalar = self.scalar(load.source().ty(), out)?;
                        if scalar == ScalarV30::Unit {
                            return Err(unsupported());
                        }
                        Ok(Event::Transfer {
                            destination,
                            value: Value::Read {
                                access: self.access(load.source(), out)?,
                                moved: false,
                            },
                            scalar,
                        })
                    }
                    Rvalue::AddressOf { place, mutability } => {
                        let Destination::Local(destination) = destination else {
                            return Err(unsupported());
                        };
                        let pointer = match self
                            .types
                            .get(assignment.destination().ty().index() as usize)
                            .map(Type::shape)
                        {
                            Some(Shape::Pointer(pointer)) => pointer,
                            _ => return Err(mismatch()),
                        };
                        if pointer.pointee() != place.ty()
                            || pointer.kind() != PointerKind::Raw
                            || pointer.mutability() != *mutability
                            || pointer.metadata() != PointerMetadata::None
                        {
                            return Err(mismatch());
                        }
                        Ok(Event::Address {
                            destination,
                            access: self.access(place, out)?,
                        })
                    }
                    value @ (Rvalue::Unary { .. } | Rvalue::Binary { .. }) => {
                        if !matches!(destination, Destination::Local(_)) {
                            return Err(unsupported());
                        }
                        self.scalar(assignment.destination().ty(), out)?;
                        value.try_visit_operands(|operand| {
                            if matches!(self.value(operand, out)?, Value::Read { .. }) {
                                return Err(unsupported());
                            }
                            Ok(())
                        })?;
                        Ok(Event::Scalar)
                    }
                    // Includes every cast, so pointers cannot be laundered into
                    // scalar payloads and then written into untracked bytes.
                    _ => Err(unsupported()),
                }
            }
            Statement::Store(store) => {
                self.ordinary(store.volatility(), store.atomic().is_some())?;
                if store.destination().ty() != store.value().ty() {
                    return Err(mismatch());
                }
                let scalar = self.scalar(store.value().ty(), out)?;
                if scalar == ScalarV30::Unit {
                    return Err(unsupported());
                }
                Ok(Event::Transfer {
                    destination: Destination::Memory(self.access(store.destination(), out)?),
                    value: self.value(store.value(), out)?,
                    scalar,
                })
            }
            Statement::Deinitialize(place) => match self.destination(place, out)? {
                Destination::Local(_) => Ok(Event::Scalar),
                Destination::Memory(access) => Ok(Event::Deinitialize(access)),
            },
            Statement::StorageLive(local) | Statement::StorageDead(local) => {
                let Some(descriptor) = self.descriptor(local.index(), out)? else {
                    return Ok(Event::Scalar);
                };
                let local = self.local(local.index())?;
                Ok(if matches!(statement, Statement::StorageLive(_)) {
                    Event::StorageLive { descriptor, local }
                } else {
                    Event::StorageDead { descriptor, local }
                })
            }
            Statement::Nop => Ok(Event::Scalar),
            _ => Err(unsupported()),
        }
    }
}

impl TypedOperand {
    pub(super) const fn ty(self) -> TypeId {
        self.ty
    }

    pub(super) const fn scalar(self) -> Option<ScalarV30> {
        match self.kind {
            OperandKind::Scalar { scalar, .. } => Some(scalar),
            _ => None,
        }
    }

    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        match self.kind {
            OperandKind::Scalar { value, scalar } => {
                write!(out, "InvocationSourceOperandV36::Scalar {{ value: ")
                    .map_err(|_| out.error())?;
                emit_value(value, out)?;
                write!(out, ", bits: {}int }}", scalar.width()).map_err(|_| out.error())
            }
            OperandKind::Pointer { local, moved } => write!(
                out,
                "InvocationSourceOperandV36::Pointer {{ local: {local}int, moved: {moved} }}"
            )
            .map_err(|_| out.error()),
            OperandKind::Slice { local, moved, metadata_bits } => write!(
                out,
                "InvocationSourceOperandV36::Slice {{ local: {local}int, moved: {moved}, metadata_bits: {metadata_bits}int }}"
            )
            .map_err(|_| out.error()),
        }
    }
}

fn emit_value(value: Value, out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.charge_work(1)?;
    match value {
        Value::Constant(bits) => write!(out, "InvocationSourceByteValueV36::Constant({bits}int)")
            .map_err(|_| out.error()),
        Value::Local { local, moved } => write!(
            out,
            "InvocationSourceByteValueV36::Local {{ local: {local}int, moved: {moved} }}"
        )
        .map_err(|_| out.error()),
        Value::Read { access, moved } => {
            write!(out, "InvocationSourceByteValueV36::Read {{ access: ")
                .map_err(|_| out.error())?;
            emit_access(access, out)?;
            write!(out, ", moved: {moved} }}").map_err(|_| out.error())
        }
    }
}

fn emit_access(access: Access, out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.charge_work(1)?;
    write!(out, "InvocationSourceByteAccessV36 {{ base: ").map_err(|_| out.error())?;
    let offset = match access.address {
        Address::Slot { descriptor, offset } => {
            write!(out, "InvocationSourceByteBaseV36::Slot {{ descriptor: {descriptor}int, slot: invocation_source_slot_{descriptor}_v36() }}").map_err(|_| out.error())?;
            offset
        }
        Address::Pointer { local, offset } => {
            write!(out, "InvocationSourceByteBaseV36::PointerLocal({local}int)")
                .map_err(|_| out.error())?;
            offset
        }
    };
    write!(
        out,
        ", offset: {offset}int, width: {}int, alignment: {}int }}",
        access.bytes, access.alignment
    )
    .map_err(|_| out.error())
}

fn emit_event(event: Event, out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.charge_work(1)?;
    match event {
        Event::Pointer(event) => {
            write!(out, "InvocationSourceByteEventV36::Pointer(").map_err(|_| out.error())?;
            pointer_events::emit(event, out)?;
            write!(out, ")").map_err(|_| out.error())?;
        }
        Event::WitnessBorrow(borrow) => borrow.emit(out)?,
        Event::Scalar => {
            write!(out, "InvocationSourceByteEventV36::Scalar").map_err(|_| out.error())?
        }
        Event::Transfer {
            destination,
            value,
            scalar,
        } => {
            write!(
                out,
                "InvocationSourceByteEventV36::Transfer {{ destination: "
            )
            .map_err(|_| out.error())?;
            match destination {
                Destination::Local(local) => {
                    write!(out, "InvocationSourceByteDestinationV36::Local({local}int)")
                        .map_err(|_| out.error())?
                }
                Destination::Memory(access) => {
                    write!(out, "InvocationSourceByteDestinationV36::Memory(")
                        .map_err(|_| out.error())?;
                    emit_access(access, out)?;
                    write!(out, ")").map_err(|_| out.error())?;
                }
            }
            write!(out, ", value: ").map_err(|_| out.error())?;
            emit_value(value, out)?;
            write!(out, ", bits: {}int }}", scalar.width()).map_err(|_| out.error())?;
        }
        Event::Address {
            destination,
            access,
        } => {
            write!(
                out,
                "InvocationSourceByteEventV36::Address {{ destination: {destination}int, access: "
            )
            .map_err(|_| out.error())?;
            emit_access(access, out)?;
            write!(out, " }}").map_err(|_| out.error())?;
        }
        Event::Deinitialize(access) => {
            write!(out, "InvocationSourceByteEventV36::Deinitialize(").map_err(|_| out.error())?;
            emit_access(access, out)?;
            write!(out, ")").map_err(|_| out.error())?;
        }
        Event::StorageLive { descriptor, local } | Event::StorageDead { descriptor, local } => {
            let variant = if matches!(event, Event::StorageLive { .. }) {
                "StorageLive"
            } else {
                "StorageDead"
            };
            write!(out, "InvocationSourceByteEventV36::{variant} {{ descriptor: {descriptor}int, slot: invocation_source_slot_{descriptor}_v36(), local: {local}int }}").map_err(|_| out.error())?;
        }
    }
    Ok(())
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<SourceByteBody<'_, '_, '_>>()
        + h::<Context<'_, '_, '_>>()
        + h::<Vec<Event>>()
        + h::<Vec<Range<usize>>>()
        + h::<Range<usize>>()
        + h::<Event>()
        + h::<Value>()
        + h::<TypedOperand>()
        + h::<OperandKind>()
        + h::<BackendRepr>()
        + h::<BackendScalar>()
        + h::<BackendPrimitive>()
        + h::<u32>()
        + h::<Access>()
        + h::<Destination>()
        + h::<ScalarV30>()
        + h::<TypeId>()
        + h::<Address>()
        + pointer_events::headers()
        + 24 * size_of::<usize>()
        + 20 * size_of::<&()>()
}

pub(super) const SOURCE_BYTES_V36: &str = concat!(
    include_str!("original_semantic_mir_source_logical_locals_v38.vrs"),
    r#"
struct InvocationSourceByteStateV36 {
    machine: MemoryStateV30,
    // Exact descriptor ordinals name current logical source allocations. These
    // are not local values, target pointers, or inferred latest generations.
    slots: Map<int, MemoryPointerV30>,
    logical: InvocationSourceLogicalV38,
}

// This structural invariant does not grant a descriptor's source identity;
// every use still joins its exact literal SourceSlot in the dispatcher.
open spec fn invocation_source_byte_state_well_formed_v36(source: InvocationSourceByteStateV36) -> bool {
    byte_memory_well_formed_v30(source.machine.memory)
        && invocation_source_logical_well_formed_v38(source.logical, source.machine.values.len())
        && byte_frame_runtime_well_formed_v30(source.machine.frames)
        && byte_private_frames_live_v30(source.machine.memory, source.machine.frames)
        && private_generation_counters_valid_v30(source.machine.generations, source.machine.memory)
        && source.slots.dom().finite()
        && (forall|descriptor: int| source.slots.contains_key(descriptor) ==>
            descriptor >= 0 && source.slots[descriptor].byte_offset == 0
                && source.machine.memory.live.contains_key(source.slots[descriptor].allocation)
                && match source.slots[descriptor].allocation {
                    MemoryAllocationV30::Private { owner, invocation, generation, .. } =>
                        generation >= 0 && exists|i: int| 0 <= i < source.machine.frames.active.len()
                            && source.machine.frames.active[i].owner == owner
                            && source.machine.frames.active[i].invocation == invocation,
                    _ => false,
                })
        && (forall|left: int, right: int|
            source.slots.contains_key(left) && source.slots.contains_key(right)
                && left != right ==>
                    source.slots[left].allocation != source.slots[right].allocation)
}

enum InvocationSourceByteBaseV36 {
    Slot { descriptor: int, slot: InvocationSourceSlotV36 },
    PointerLocal(int),
}

struct InvocationSourceByteAccessV36 {
    base: InvocationSourceByteBaseV36,
    offset: int,
    width: int,
    alignment: int,
}

enum InvocationSourceByteValueV36 {
    Constant(int),
    Local { local: int, moved: bool },
    Read { access: InvocationSourceByteAccessV36, moved: bool },
}

enum InvocationSourceOperandV36 {
    Scalar { value: InvocationSourceByteValueV36, bits: int },
    Pointer { local: int, moved: bool },
    Slice { local: int, moved: bool, metadata_bits: int },
}

enum InvocationSourceByteDestinationV36 {
    Local(int),
    Memory(InvocationSourceByteAccessV36),
}

enum InvocationSourceByteEventV36 {
    Scalar,
    WitnessBorrow { destination: int, origin: int, source_type: int, generation: int,
        instance: int, block: int, statement: int },
    Pointer(InvocationSourcePointerEventV36),
    Transfer { destination: InvocationSourceByteDestinationV36,
        value: InvocationSourceByteValueV36, bits: int },
    Address { destination: int, access: InvocationSourceByteAccessV36 },
    Deinitialize(InvocationSourceByteAccessV36),
    StorageLive { descriptor: int, slot: InvocationSourceSlotV36, local: int },
    StorageDead { descriptor: int, slot: InvocationSourceSlotV36, local: int },
}

open spec fn invocation_source_byte_refused_v36(source: InvocationSourceByteStateV36)
    -> InvocationSourceByteStateV36
{
    InvocationSourceByteStateV36 { machine: invocation_source_refused_v36(source.machine),
        ..source }
}

open spec fn invocation_source_byte_value_typed_v36(value: MemoryValueV30, bits: int) -> bool {
    match value {
        MemoryValueV30::Unit => bits == 0,
        MemoryValueV30::Scalar(value) =>
            (bits == 1 && 0 <= value < 2)
            || ((bits == 8 || bits == 16 || bits == 32 || bits == 64)
                && 0 <= value < memory_value_modulus_v30(bits / 8)),
        // Stored bytes never acquire pointer provenance by integer conversion.
        _ => false,
    }
}

open spec fn invocation_source_byte_put_local_v36(
    source: InvocationSourceByteStateV36, local: int, value: MemoryValueV30,
) -> InvocationSourceByteStateV36 {
    if !source.machine.valid || !invocation_source_byte_state_well_formed_v36(source)
        || local < 0 || source.machine.values.len() <= local {
        invocation_source_byte_refused_v36(source)
    } else {
        InvocationSourceByteStateV36 { machine: MemoryStateV30 {
            pc: source.machine.pc, values: source.machine.values.update(local, value),
            memory: source.machine.memory, generations: source.machine.generations,
            frames: source.machine.frames, valid: true },
            logical: invocation_source_logical_write_v38(source.logical, local), ..source }
    }
}

open spec fn invocation_source_byte_slot_v36(
    source: InvocationSourceByteStateV36, descriptor: int, slot: InvocationSourceSlotV36,
    root: int, instance: int,
) -> Option<MemoryPointerV30> {
    if !source.machine.valid || !invocation_source_byte_state_well_formed_v36(source)
        || !source.slots.contains_key(descriptor)
        || descriptor < 0 || slot.root != root || slot.instance != instance
        || source.machine.frames.active.len() == 0 {
        None
    } else {
        let pointer = source.slots[descriptor];
        let frame = source.machine.frames.active.last();
        let exact = match pointer.allocation {
            MemoryAllocationV30::Private { owner, invocation, site, generation } =>
                owner == slot.owner && owner == frame.owner && invocation == frame.invocation
                    && site == slot.site && 0 <= generation,
            _ => false,
        };
        if exact && pointer.byte_offset == 0 && source.machine.memory.live.contains_key(pointer.allocation)
            && source.machine.memory.live[pointer.allocation].bytes.len() == slot.extent
            && source.machine.memory.live[pointer.allocation].base_alignment == slot.alignment {
            Some(pointer)
        } else { None }
    }
}

open spec fn invocation_source_byte_address_v36(
    source: InvocationSourceByteStateV36, access: InvocationSourceByteAccessV36,
    root: int, instance: int,
) -> Option<MemoryPointerV30> {
    let base = match access.base {
        InvocationSourceByteBaseV36::Slot { descriptor, slot } =>
            invocation_source_byte_slot_v36(source, descriptor, slot, root, instance),
        InvocationSourceByteBaseV36::PointerLocal(local) =>
            if 0 <= local < source.machine.values.len() {
                match source.machine.values[local] {
                    MemoryValueV30::Pointer(pointer) => Some(pointer),
                    _ => None,
                }
            } else { None },
    };
    match base {
        Some(base) => {
            let pointer = MemoryPointerV30 { byte_offset: base.byte_offset + access.offset, ..base };
            if source.machine.valid && invocation_source_byte_state_well_formed_v36(source)
                && 0 <= access.offset
                && byte_range_live_v30(source.machine.memory, pointer, access.width) {
                Some(pointer)
            } else { None }
        }
        None => None,
    }
}

open spec fn invocation_source_byte_deinitialize_v36(
    source: InvocationSourceByteStateV36, pointer: MemoryPointerV30, width: int,
) -> InvocationSourceByteStateV36 {
    if !source.machine.valid || !invocation_source_byte_state_well_formed_v36(source)
        || !byte_range_live_v30(source.machine.memory, pointer, width) {
        invocation_source_byte_refused_v36(source)
    } else {
        InvocationSourceByteStateV36 { machine: MemoryStateV30 {
            pc: source.machine.pc, values: source.machine.values,
            memory: byte_deinitialize_v37(source.machine.memory, pointer, width),
            generations: source.machine.generations, frames: source.machine.frames, valid: true },
            ..source }
    }
}

struct InvocationSourceByteEvaluationV36 {
    source: InvocationSourceByteStateV36,
    value: MemoryValueV30,
}

open spec fn invocation_source_byte_evaluate_v36(
    source: InvocationSourceByteStateV36, value: InvocationSourceByteValueV36,
    bits: int, root: int, instance: int, little_endian: bool,
) -> InvocationSourceByteEvaluationV36 {
    let evaluated = match value {
        InvocationSourceByteValueV36::Constant(value) => InvocationSourceByteEvaluationV36 {
            source, value: if bits == 0 && value == 0 { MemoryValueV30::Unit }
                else { MemoryValueV30::Scalar(value) } },
        InvocationSourceByteValueV36::Local { local, moved } => {
            if 0 <= local < source.machine.values.len() {
                let value = source.machine.values[local];
                InvocationSourceByteEvaluationV36 { source: if moved {
                    invocation_source_byte_put_local_v36(source, local, MemoryValueV30::Undefined)
                } else { source }, value }
            } else {
                InvocationSourceByteEvaluationV36 { source: invocation_source_byte_refused_v36(source),
                    value: MemoryValueV30::Undefined }
            }
        }
        InvocationSourceByteValueV36::Read { access, moved } => {
            match invocation_source_byte_address_v36(source, access, root, instance) {
                Some(pointer) => {
                    if invocation_source_read_enabled_v36(source.machine, pointer, access.width, access.alignment)
                        && (bits == 1 && access.width == 1 || bits == access.width * 8) {
                        let value = MemoryValueV30::Scalar(byte_load_v30(
                            source.machine.memory, pointer, access.width, little_endian));
                        InvocationSourceByteEvaluationV36 { source: if moved {
                            invocation_source_byte_deinitialize_v36(source, pointer, access.width)
                        } else { source }, value }
                    } else {
                        InvocationSourceByteEvaluationV36 { source: invocation_source_byte_refused_v36(source),
                            value: MemoryValueV30::Undefined }
                    }
                }
                None => InvocationSourceByteEvaluationV36 {
                    source: invocation_source_byte_refused_v36(source), value: MemoryValueV30::Undefined },
            }
        }
    };
    if !evaluated.source.machine.valid
        || !invocation_source_byte_state_well_formed_v36(evaluated.source)
        || !invocation_source_byte_value_typed_v36(evaluated.value, bits) {
        InvocationSourceByteEvaluationV36 {
            source: invocation_source_byte_refused_v36(evaluated.source), value: evaluated.value }
    } else { evaluated }
}

// A carrier copy does not read pointee bytes or prove the pointee current. The
// consuming access, call contract and frame-end escape checks remain distinct.
open spec fn invocation_source_carrier_evaluate_v36(
    source: InvocationSourceByteStateV36, local: int, moved: bool, metadata_bits: int,
) -> InvocationSourceByteEvaluationV36 {
    if !source.machine.valid || !invocation_source_byte_state_well_formed_v36(source)
        || local < 0 || source.machine.values.len() <= local {
        InvocationSourceByteEvaluationV36 {
            source: invocation_source_byte_refused_v36(source), value: MemoryValueV30::Undefined }
    } else {
        let value = source.machine.values[local];
        let typed = match value {
            MemoryValueV30::Pointer(_) => metadata_bits == 0,
            MemoryValueV30::Slice(value) =>
                (metadata_bits == 8 || metadata_bits == 16 || metadata_bits == 32 || metadata_bits == 64)
                    && 0 <= value.length < memory_value_modulus_v30(metadata_bits / 8),
            _ => false,
        };
        if !typed {
            InvocationSourceByteEvaluationV36 {
                source: invocation_source_byte_refused_v36(source), value }
        } else {
            InvocationSourceByteEvaluationV36 { source: if moved {
                invocation_source_byte_put_local_v36(source, local, MemoryValueV30::Undefined)
            } else { source }, value }
        }
    }
}

open spec fn invocation_source_operand_evaluate_v36(
    source: InvocationSourceByteStateV36, operand: InvocationSourceOperandV36,
    root: int, instance: int, little_endian: bool,
) -> InvocationSourceByteEvaluationV36 {
    match operand {
        InvocationSourceOperandV36::Scalar { value, bits } =>
            invocation_source_byte_evaluate_v36(source, value, bits, root, instance, little_endian),
        InvocationSourceOperandV36::Pointer { local, moved } =>
            invocation_source_carrier_evaluate_v36(source, local, moved, 0),
        InvocationSourceOperandV36::Slice { local, moved, metadata_bits } =>
            invocation_source_carrier_evaluate_v36(source, local, moved, metadata_bits),
    }
}

// Fresh source allocation is independent of the target and any refinement map.
// Physical hoisting/reuse must be related to this effect by a separate theorem.
open spec fn invocation_source_byte_activate_v36(
    source: InvocationSourceByteStateV36, descriptor: int, slot: InvocationSourceSlotV36,
    local: int, root: int, instance: int,
) -> InvocationSourceByteStateV36 {
    if !source.machine.valid || !invocation_source_byte_state_well_formed_v36(source)
        || source.machine.frames.active.len() == 0 || source.slots.contains_key(descriptor)
        || descriptor < 0 || slot.root != root || slot.instance != instance
        || slot.owner < 0 || slot.extent < 0 || slot.alignment <= 0
        || slot.site.function < 0 || slot.site.block < 0 || slot.site.operation < 0
        || local < 0 || source.machine.values.len() <= local {
        invocation_source_byte_refused_v36(source)
    } else {
        let frame = source.machine.frames.active.last();
        let site = MemoryPrivateSiteV30 { owner: frame.owner, invocation: frame.invocation, site: slot.site };
        let generation = private_generation_v30(source.machine.generations, site);
        let allocation = private_allocation_v30(site, generation);
        if frame.owner != slot.owner || generation < 0 || source.machine.memory.live.contains_key(allocation) {
            invocation_source_byte_refused_v36(source)
        } else {
            InvocationSourceByteStateV36 { machine: MemoryStateV30 {
                pc: source.machine.pc, values: source.machine.values.update(local, MemoryValueV30::Undefined),
                memory: byte_allocate_v30(source.machine.memory, allocation, slot.extent, slot.alignment),
                generations: source.machine.generations.insert(site, generation + 1),
                frames: source.machine.frames, valid: true },
                slots: source.slots.insert(descriptor, MemoryPointerV30 { allocation, byte_offset: 0, view: None }),
                logical: invocation_source_logical_write_v38(source.logical, local) }
        }
    }
}

open spec fn invocation_source_byte_end_v36(
    source: InvocationSourceByteStateV36, descriptor: int, slot: InvocationSourceSlotV36,
    local: int, root: int, instance: int,
) -> InvocationSourceByteStateV36 {
    match invocation_source_byte_slot_v36(source, descriptor, slot, root, instance) {
        Some(pointer) => {
            let cleared = invocation_source_byte_put_local_v36(source, local, MemoryValueV30::Undefined);
            // Both live values and initialized fragments in other allocations
            // may retain provenance, even without a complete relocation cell.
            if cleared.machine.valid
                && !invocation_memory_names_allocation_v37(cleared.machine.memory, pointer.allocation)
                && (forall|i: int| 0 <= i < cleared.machine.values.len() ==>
                    !invocation_value_names_allocation_v36(cleared.machine.values[i], pointer.allocation)) {
                InvocationSourceByteStateV36 { machine: MemoryStateV30 {
                    pc: cleared.machine.pc, values: cleared.machine.values,
                    memory: byte_end_lifetime_v30(cleared.machine.memory, pointer.allocation),
                    generations: cleared.machine.generations, frames: cleared.machine.frames, valid: true },
                    slots: cleared.slots.remove(descriptor), ..cleared }
            } else { invocation_source_byte_refused_v36(cleared) }
        }
        None => invocation_source_byte_refused_v36(source),
    }
}

open spec fn invocation_source_byte_step_v36(
    source: InvocationSourceByteStateV36, event: InvocationSourceByteEventV36,
    root: int, instance: int, little_endian: bool,
) -> InvocationSourceByteStateV36 {
    match event {
        InvocationSourceByteEventV36::WitnessBorrow { destination, origin, source_type,
            generation, instance, block, statement } => invocation_source_borrow_witness_v38(
                source, destination, origin, source_type, generation, instance, block, statement),
        // A scalar marker is not a no-op. The exact statement's existing
        // primitive graph must run on the current values between byte events.
        InvocationSourceByteEventV36::Scalar => invocation_source_byte_refused_v36(source),
        InvocationSourceByteEventV36::Pointer(event) =>
            invocation_source_pointer_step_v36(source, event, root, instance, little_endian),
        InvocationSourceByteEventV36::Transfer { destination, value, bits } => {
            let evaluated = invocation_source_byte_evaluate_v36(source, value, bits, root, instance, little_endian);
            if !evaluated.source.machine.valid { evaluated.source }
            else { match destination {
                InvocationSourceByteDestinationV36::Local(local) =>
                    invocation_source_byte_put_local_v36(evaluated.source, local, evaluated.value),
                InvocationSourceByteDestinationV36::Memory(access) => {
                    match invocation_source_byte_address_v36(evaluated.source, access, root, instance) {
                        Some(pointer) => {
                            if bits != 0 && (bits == 1 && access.width == 1 || bits == access.width * 8) {
                                InvocationSourceByteStateV36 { machine: invocation_source_store_v36(
                                    evaluated.source.machine, pointer, access.width, access.alignment,
                                    evaluated.value, little_endian), ..evaluated.source }
                            } else { invocation_source_byte_refused_v36(evaluated.source) }
                        }
                        None => invocation_source_byte_refused_v36(evaluated.source),
                    }
                }
            } }
        }
        InvocationSourceByteEventV36::Address { destination, access } => {
            match invocation_source_byte_address_v36(source, access, root, instance) {
                Some(pointer) => invocation_source_byte_put_local_v36(source, destination, MemoryValueV30::Pointer(pointer)),
                None => invocation_source_byte_refused_v36(source),
            }
        }
        InvocationSourceByteEventV36::Deinitialize(access) => {
            match invocation_source_byte_address_v36(source, access, root, instance) {
                Some(pointer) => invocation_source_byte_deinitialize_v36(source, pointer, access.width),
                None => invocation_source_byte_refused_v36(source),
            }
        }
        InvocationSourceByteEventV36::StorageLive { descriptor, slot, local } =>
            invocation_source_byte_activate_v36(source, descriptor, slot, local, root, instance),
        InvocationSourceByteEventV36::StorageDead { descriptor, slot, local } =>
            invocation_source_byte_end_v36(source, descriptor, slot, local, root, instance),
    }
}
"#
);

#[cfg(test)]
#[path = "original_semantic_mir_invocation_source_bytes_v36_tests.rs"]
mod tests;
