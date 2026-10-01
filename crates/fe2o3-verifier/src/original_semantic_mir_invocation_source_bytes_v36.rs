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
#[path = "original_semantic_mir_source_aggregate_events_v42.rs"]
mod aggregates;
#[path = "original_semantic_mir_source_discriminants_v41.rs"]
mod discriminants;
#[path = "original_semantic_mir_source_slice_reads_v41.rs"]
mod slice_reads;
#[path = "original_semantic_mir_source_witness_borrows_v38.rs"]
pub(super) mod witness_events;
#[path = "original_semantic_mir_source_witness_transfers_v40.rs"]
mod witness_transfers;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Address {
    Slot {
        descriptor: usize,
        offset: u64,
    },
    Object {
        local: usize,
        offset: u64,
    },
    Pointer {
        local: usize,
        offset: u64,
    },
    Slice {
        source: slice_reads::Slice,
        offset: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Access {
    address: Address,
    ty: TypeId,
    bytes: u64,
    alignment: u64,
}

impl Access {
    pub(super) fn emit(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        emit_access(*self, out)
    }
}

// The original call destination is fetched here rather than supplied by a
// consumer. Type-wide aggregate schemas cannot grant a local storage identity.
#[allow(clippy::too_many_arguments)]
pub(super) fn object_call_destination_v42(
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    root: usize,
    instance: usize,
    block: usize,
    expected_type: TypeId,
    out: &mut Writer<'_, '_>,
) -> Result<Option<Access>> {
    slots.with_source_query_v42(out, |out| {
        out.budget.reserve_storage(headers())?;
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
        out.budget.charge_work(5)?;
        if !row.active
            || row.locals.len() != function.locals().len()
            || row.blocks.len() != function.blocks().len()
        {
            return Err(mismatch());
        }
        let Some(Terminator::Call(call)) = function
            .blocks()
            .get(block)
            .map(|body| body.terminator().kind())
        else {
            return Err(mismatch());
        };
        let destination = call.destination().ok_or_else(mismatch)?.place();
        if destination.ty() != expected_type {
            return Err(mismatch());
        }
        if !slots.has_original_object(root, instance, destination.local().index(), out)? {
            return Ok(None);
        }
        let context = Context {
            slots,
            types: semantic.types(),
            function,
            root,
            instance,
            locals: row.locals.clone(),
        };
        if context.scalar(expected_type, out)? == ScalarV30::Unit {
            return Err(unsupported());
        }
        let access = context.access(destination, out)?;
        if !matches!(access.address, Address::Object { .. }) || access.ty != expected_type {
            return Err(mismatch());
        }
        Ok(Some(access))
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Value {
    Constant(u128),
    Local {
        local: usize,
        moved: bool,
    },
    Read {
        access: Access,
        moved: bool,
    },
    Component {
        local: usize,
        source_type: TypeId,
        ordinal: usize,
        moved: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OperandKind {
    Aggregate {
        place: aggregates::AggregatePlace,
        moved: bool,
    },
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
    Component(aggregates::AggregatePlace),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Event {
    /// The existing scalar graph must still interpret and check this statement.
    Scalar,
    WitnessBorrow(witness_events::Borrow),
    WitnessTransfer(witness_transfers::Transfer),
    Pointer(pointer_events::Event),
    Discriminant(discriminants::Read),
    Checked(aggregates::Checked),
    AggregateTransfer(aggregates::Transfer),
    AggregateDeinitialize(aggregates::AggregatePlace),
    AggregateReset {
        local: usize,
    },
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
    ObjectLive {
        descriptor: usize,
        local: usize,
        activation: u32,
    },
    ObjectDead {
        local: usize,
    },
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

fn statement_kind(statement: &Statement) -> &'static str {
    match statement {
        Statement::Assign(assignment) => match assignment.value().kind() {
            Rvalue::Use(Operand::Copy(place)) if place.projections().is_empty() => {
                "Assign.Use.Copy.Local"
            }
            Rvalue::Use(Operand::Copy(_)) => "Assign.Use.Copy.Projected",
            Rvalue::Use(Operand::Move(place)) if place.projections().is_empty() => {
                "Assign.Use.Move.Local"
            }
            Rvalue::Use(Operand::Move(_)) => "Assign.Use.Move.Projected",
            Rvalue::Use(Operand::Constant(value)) => match value.value() {
                Constant::ZeroSized => "Assign.Use.Constant.ZeroSized",
                Constant::Scalar(_) => "Assign.Use.Constant.Scalar",
                Constant::Bytes(_) => "Assign.Use.Constant.Bytes",
                Constant::Pointer(_) => "Assign.Use.Constant.Pointer",
                Constant::Callable(_) => "Assign.Use.Constant.Callable",
            },
            Rvalue::Unary { .. } => "Assign.Unary",
            Rvalue::Binary { .. } => "Assign.Binary",
            Rvalue::CheckedBinary(_) => "Assign.CheckedBinary",
            Rvalue::UncheckedBinary(_) => "Assign.UncheckedBinary",
            Rvalue::Cast { .. } => "Assign.Cast",
            Rvalue::Borrow { .. } => "Assign.Borrow",
            Rvalue::AddressOf { .. } => "Assign.AddressOf",
            Rvalue::Length(_) => "Assign.Length",
            Rvalue::Discriminant(_) => "Assign.Discriminant",
            Rvalue::Aggregate(_) => "Assign.Aggregate",
            Rvalue::Load(_) => "Assign.Load",
        },
        Statement::Store(_) => "Store",
        Statement::AtomicRmw(_) => "AtomicRmw",
        Statement::AtomicCompareExchange(_) => "AtomicCompareExchange",
        Statement::SetDiscriminant { .. } => "SetDiscriminant",
        Statement::Deinitialize(_) => "Deinitialize",
        Statement::StorageLive(_) => "StorageLive",
        Statement::StorageDead(_) => "StorageDead",
        Statement::Assume(_) => "Assume",
        Statement::Nop => "Nop",
    }
}

fn statement_result_type(statement: &Statement, types: &[Type]) -> Option<(u32, &'static str)> {
    let Statement::Assign(assignment) = statement else {
        return None;
    };
    let ty = assignment.value().result_type();
    let shape = match types.get(ty.index() as usize).map(Type::shape) {
        Some(Shape::Unit) => "Unit",
        Some(Shape::Never) => "Never",
        Some(Shape::Scalar(_)) => "Scalar",
        Some(Shape::ValidityScalar(_)) => "ValidityScalar",
        Some(Shape::Pointer(_)) => "Pointer",
        Some(Shape::Array { .. }) => "Array",
        Some(Shape::Slice { .. }) => "Slice",
        Some(Shape::Tuple(_)) => "Tuple",
        Some(Shape::Aggregate(_)) => "Aggregate",
        Some(Shape::Union(_)) => "Union",
        Some(Shape::Enum { .. }) => "Enum",
        Some(Shape::FunctionPointer { .. }) => "FunctionPointer",
        Some(Shape::Opaque) => "Opaque",
        None => "Missing",
    };
    Some((ty.index(), shape))
}

fn statement_error(error: Error, site: [usize; 5], statement: &Statement, types: &[Type]) -> Error {
    // Never turn a resource/owner failure into an unsupported-language report.
    match error {
        Error::Statement(reason) => Error::SourceStatement {
            root: site[0],
            instance: site[1],
            function: site[2],
            block: site[3],
            statement: site[4],
            kind: statement_kind(statement),
            result_type: statement_result_type(statement, types),
            reason,
        },
        other => other,
    }
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
        let mut first_epoch = 1usize;
        for (block_ordinal, block) in function.blocks().iter().enumerate() {
            out.budget.charge_work(1)?;
            let start = events.len();
            for (statement_ordinal, statement) in block.statements().iter().enumerate() {
                out.budget.charge_work(1)?;
                let site = [
                    root,
                    instance,
                    row.function.index() as usize,
                    block_ordinal,
                    statement_ordinal,
                ];
                let borrowed = match statement.kind() {
                    Statement::Assign(assignment) => witness_events::Borrow::derive(
                        &context,
                        row.function,
                        block_ordinal,
                        statement_ordinal,
                        assignment,
                        out,
                    )
                    .map_err(|error| {
                        statement_error(error, site, statement.kind(), context.types)
                    })?,
                    _ => None,
                };
                let object_lifetime = match statement.kind() {
                    Statement::StorageLive(local) | Statement::StorageDead(local)
                        if slots.has_original_object(root, instance, local.index(), out)? =>
                    {
                        let flat = context.local(local.index())?;
                        Some(if matches!(statement.kind(), Statement::StorageLive(_)) {
                            let activation = u32::try_from(
                                first_epoch
                                    .checked_add(statement_ordinal)
                                    .ok_or(Resource::Arithmetic)?,
                            )
                            .map_err(|_| Resource::Arithmetic)?;
                            let row = slots
                                .object_activation(root, instance, local.index(), activation, out)?
                                .ok_or_else(unsupported)?;
                            if row.origin != (fe2o3_lower_mir_kernel::ProductionSourceObjectActivationV40::StorageLive {
                                block: fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1::from_index(u32::try_from(block_ordinal).map_err(|_| Resource::Arithmetic)?), statement: statement_ordinal,
                            }) { return Err(mismatch()); }
                            Event::ObjectLive {
                                descriptor: row.descriptor,
                                local: flat,
                                activation,
                            }
                        } else {
                            Event::ObjectDead { local: flat }
                        })
                    }
                    _ => None,
                };
                let event = match (borrowed, object_lifetime) {
                    (Some(_), Some(_)) => return Err(mismatch()),
                    (None, Some(event)) => event,
                    (Some(borrow), None) => Event::WitnessBorrow(borrow),
                    (None, None) => context.statement(statement.kind(), out).map_err(|error| {
                        statement_error(error, site, statement.kind(), context.types)
                    })?,
                };
                if events.len() == events.capacity() {
                    return Err(Resource::Accounting.into());
                }
                events.push(event);
            }
            blocks.push(start..events.len());
            first_epoch = first_epoch
                .checked_add(block.statements().len())
                .ok_or(Resource::Arithmetic)?;
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

    pub(super) fn assertion_operands(
        &self,
        block: usize,
        original: &Terminator,
        out: &mut Writer<'_, '_>,
    ) -> Result<(TypedOperand, [Option<TypedOperand>; 2])> {
        use fe2o3_mir_model::semantic_mir_v1::SemanticAssertMessageV1 as Message;
        let context = self.context(out)?;
        out.budget.charge_work(4)?;
        let captured = context
            .function
            .blocks()
            .get(block)
            .map(|block| block.terminator().kind())
            .ok_or_else(mismatch)?;
        if !std::ptr::eq(captured, original) {
            return Err(mismatch());
        }
        let Terminator::Assert {
            condition, message, ..
        } = captured
        else {
            return Err(mismatch());
        };
        let condition = context.typed_operand(condition, out)?;
        if condition.scalar() != Some(ScalarV30::Bool) {
            return Err(unsupported());
        }
        let inputs = match message {
            Message::BoundsCheck { length, index } => [Some(length), Some(index)],
            Message::Overflow { left, right, .. } => [Some(left), Some(right)],
            Message::MisalignedPointerDereference {
                required_alignment,
                found_alignment,
            } => [Some(required_alignment), Some(found_alignment)],
            Message::DivisionByZero(value) | Message::RemainderByZero(value) => [Some(value), None],
            Message::NullPointerDereference
            | Message::ResumedAfterReturn
            | Message::ResumedAfterPanic => [None, None],
        };
        let mut failure = [None, None];
        for (at, input) in inputs.into_iter().enumerate() {
            out.budget.charge_work(1)?;
            if let Some(input) = input {
                let value = context.typed_operand(input, out)?;
                if value
                    .scalar()
                    .is_none_or(|scalar| scalar == ScalarV30::Unit)
                {
                    return Err(unsupported());
                }
                failure[at] = Some(value);
            }
        }
        Ok((condition, failure))
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
        } else if self.slots.has_original_object(
            self.root,
            self.instance,
            place.local().index(),
            out,
        )? {
            Address::Object {
                local: self.local(place.local().index())?,
                offset: 0,
            }
        } else {
            let (descriptor, frame) = self
                .slots
                .legacy_descriptor_by_source(self.root, self.instance, place.local().index(), out)?
                .ok_or(Error::Statement(
                    "original MIR byte access has no retained source allocation",
                ))?;
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
        self.project_access(base, ty, projections, place.ty(), out)
    }

    fn read_access(&self, place: &Place, out: &mut Writer<'_, '_>) -> Result<Access> {
        match slice_reads::derive(self, place, out)? {
            Some((source, element)) => self.project_access(
                Address::Slice { source, offset: 0 },
                element,
                &place.projections()[2..],
                place.ty(),
                out,
            ),
            None => self.access(place, out),
        }
    }

    fn project_access(
        &self,
        base: Address,
        mut ty: TypeId,
        projections: &[fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1],
        result_type: TypeId,
        out: &mut Writer<'_, '_>,
    ) -> Result<Access> {
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
                _ => {
                    return Err(Error::Statement(
                        "original MIR byte projection kind is not modeled",
                    ));
                }
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
        if ty != result_type {
            return Err(mismatch());
        }
        let layout = self
            .types
            .get(ty.index() as usize)
            .ok_or_else(mismatch)?
            .layout();
        let address = match base {
            Address::Slot { descriptor, .. } => Address::Slot { descriptor, offset },
            Address::Object { local, .. } => Address::Object { local, offset },
            Address::Pointer { local, .. } => Address::Pointer { local, offset },
            Address::Slice { source, .. } => Address::Slice { source, offset },
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
        if !place.projections().is_empty()
            && let Some(component) = aggregates::AggregatePlace::derive(self, place, out)?
        {
            return Ok(Destination::Component(component));
        }
        let declaration = self
            .function
            .locals()
            .get(place.local().index() as usize)
            .ok_or_else(mismatch)?;
        if place.projections().is_empty()
            && !self.slots.has_original_object(
                self.root,
                self.instance,
                place.local().index(),
                out,
            )?
            && self.descriptor(place.local().index(), out)?.is_none()
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
                    _ => {
                        return Err(Error::Statement(
                            "original MIR scalar byte constant kind is not modeled",
                        ));
                    }
                };
                if bits >= (1u128 << scalar.width()) {
                    return Err(mismatch());
                }
                Ok(Value::Constant(bits))
            }
            Operand::Copy(place) | Operand::Move(place) => {
                let moved = matches!(operand, Operand::Move(_));
                let root_type = self
                    .function
                    .locals()
                    .get(place.local().index() as usize)
                    .ok_or_else(mismatch)?
                    .ty();
                if !place.projections().is_empty()
                    && !self.slots.has_original_object(
                        self.root,
                        self.instance,
                        place.local().index(),
                        out,
                    )?
                    && self.descriptor(place.local().index(), out)?.is_none()
                    && let Some((range, ty)) =
                        self.slots
                            .aggregate_component_range(root_type, place.projections(), out)?
                {
                    if range.len() != 1 || ty != place.ty() {
                        return Err(mismatch());
                    }
                    let leaf = self.slots.aggregate_leaf(root_type, range.start, out)?;
                    if leaf.source_type(out)? != ty || leaf.scalar(out)? != scalar {
                        return Err(mismatch());
                    }
                    return Ok(Value::Component {
                        local: self.local(place.local().index())?,
                        source_type: root_type,
                        ordinal: range.start,
                        moved,
                    });
                }
                if !moved && !place.projections().is_empty() && scalar != ScalarV30::Unit {
                    return Ok(Value::Read {
                        access: self.read_access(place, out)?,
                        moved: false,
                    });
                }
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
        } else if matches!(
            declaration.shape(),
            Shape::Tuple(_) | Shape::Aggregate(_) | Shape::Array { .. }
        ) {
            let (Operand::Copy(place) | Operand::Move(place)) = operand else {
                return Err(unsupported());
            };
            OperandKind::Aggregate {
                place: aggregates::AggregatePlace::derive(self, place, out)?
                    .ok_or_else(unsupported)?,
                moved: matches!(operand, Operand::Move(_)),
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
        if let Statement::Assign(assignment) = statement
            && let Some(transfer) = aggregates::Transfer::derive(self, assignment, out)?
        {
            return Ok(Event::AggregateTransfer(transfer));
        }
        if let Statement::Deinitialize(place) = statement
            && let Some(place) = aggregates::AggregatePlace::derive(self, place, out)?
        {
            return Ok(Event::AggregateDeinitialize(place));
        }
        if let Statement::Assign(assignment) = statement
            && let Some(checked) = aggregates::Checked::derive(self, assignment, out)?
        {
            return Ok(Event::Checked(checked));
        }
        if let Statement::Assign(assignment) = statement
            && let Some(read) = discriminants::Read::derive(self, assignment, out)?
        {
            return Ok(Event::Discriminant(read));
        }
        if let Statement::Assign(assignment) = statement
            && let Some(transfer) = witness_transfers::Transfer::derive(self, assignment, out)?
        {
            return Ok(Event::WitnessTransfer(transfer));
        }
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
                                access: self.read_access(load.source(), out)?,
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
                Destination::Component(place) => Ok(Event::AggregateDeinitialize(place)),
            },
            Statement::StorageLive(local) | Statement::StorageDead(local) => {
                let Some(descriptor) = self.descriptor(local.index(), out)? else {
                    let ty = self
                        .function
                        .locals()
                        .get(local.index() as usize)
                        .ok_or_else(mismatch)?
                        .ty();
                    if matches!(
                        self.types.get(ty.index() as usize).map(Type::shape),
                        Some(Shape::Tuple(_) | Shape::Aggregate(_) | Shape::Array { .. })
                    ) && self.slots.aggregate_leaf_count(ty, out)?.is_some()
                    {
                        return Ok(Event::AggregateReset {
                            local: self.local(local.index())?,
                        });
                    }
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
            OperandKind::Aggregate { place, moved } => {
                write!(out, "InvocationSourceOperandV36::Aggregate {{ place: ").map_err(|_| out.error())?;
                place.emit(out)?;
                write!(out, ", moved: {moved} }}").map_err(|_| out.error())
            }
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
        Value::Component { local, source_type, ordinal, moved } => write!(out,
            "InvocationSourceByteValueV36::Component {{ local: {local}int, source_type: {}int, ordinal: {ordinal}int, moved: {moved} }}", source_type.index()).map_err(|_| out.error()),
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
        Address::Object { local, offset } => {
            write!(out, "InvocationSourceByteBaseV36::ObjectLocal({local}int)")
                .map_err(|_| out.error())?;
            offset
        }
        Address::Slot { descriptor, offset } => {
            write!(out, "InvocationSourceByteBaseV36::Slot {{ descriptor: {descriptor}int, slot: invocation_source_slot_{descriptor}_v36() }}").map_err(|_| out.error())?;
            offset
        }
        Address::Pointer { local, offset } => {
            write!(out, "InvocationSourceByteBaseV36::PointerLocal({local}int)")
                .map_err(|_| out.error())?;
            offset
        }
        Address::Slice { source, offset } => {
            slice_reads::emit(source, out)?;
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
        Event::Checked(checked) => checked.emit(out)?,
        Event::AggregateTransfer(transfer) => transfer.emit(out)?,
        Event::AggregateDeinitialize(place) => {
            write!(out, "InvocationSourceByteEventV36::AggregateDeinitialize(")
                .map_err(|_| out.error())?;
            place.emit(out)?;
            write!(out, ")").map_err(|_| out.error())?;
        }
        Event::AggregateReset { local } => {
            write!(
                out,
                "InvocationSourceByteEventV36::AggregateReset {{ local: {local}int }}"
            )
            .map_err(|_| out.error())?;
        }
        Event::ObjectLive {
            descriptor,
            local,
            activation,
        } => {
            write!(out, "InvocationSourceByteEventV36::ObjectLive {{ descriptor: {descriptor}int, slot: invocation_source_slot_{descriptor}_v36(), local: {local}int, activation: {activation}int }}").map_err(|_| out.error())?;
        }
        Event::ObjectDead { local } => {
            write!(
                out,
                "InvocationSourceByteEventV36::ObjectDead {{ local: {local}int }}"
            )
            .map_err(|_| out.error())?;
        }
        Event::Pointer(event) => {
            write!(out, "InvocationSourceByteEventV36::Pointer(").map_err(|_| out.error())?;
            pointer_events::emit(event, out)?;
            write!(out, ")").map_err(|_| out.error())?;
        }
        Event::Discriminant(read) => read.emit(out)?,
        Event::WitnessBorrow(borrow) => borrow.emit(out)?,
        Event::WitnessTransfer(transfer) => transfer.emit(out)?,
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
                Destination::Component(place) => {
                    write!(out, "InvocationSourceByteDestinationV36::Component(")
                        .map_err(|_| out.error())?;
                    place.emit(out)?;
                    write!(out, ")").map_err(|_| out.error())?;
                }
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
        + h::<Option<Event>>()
        + h::<super::slots::ObjectActivation>()
        + h::<Option<super::slots::ObjectActivation>>()
        + h::<fe2o3_lower_mir_kernel::ProductionSourceObjectActivationV40>()
        + h::<Value>()
        + h::<TypedOperand>()
        + h::<(TypedOperand, [Option<TypedOperand>; 2])>()
        + h::<[Option<&Operand>; 2]>()
        + h::<std::iter::Enumerate<std::array::IntoIter<Option<&Operand>, 2>>>()
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
        + h::<[usize; 5]>()
        + h::<&Statement>()
        + h::<&str>()
        + h::<&[Type]>()
        + h::<Option<(u32, &'static str)>>()
        + pointer_events::headers()
        + witness_transfers::headers()
        + slice_reads::headers()
        + discriminants::headers()
        + aggregates::headers()
        + 24 * size_of::<usize>()
        + 20 * size_of::<&()>()
}

pub(super) const SOURCE_BYTES_V36: &str = concat!(
    include_str!("original_semantic_mir_source_aggregate_values_v42.vrs"),
    include_str!("original_semantic_mir_source_aggregate_laws_v42.vrs"),
    include_str!("original_semantic_mir_source_logical_locals_v38.vrs"),
    include_str!("original_semantic_mir_source_slice_reads_v41.vrs"),
    include_str!("original_semantic_mir_source_discriminants_v41.vrs"),
    r#"
struct InvocationSourceByteStateV36 {
    machine: MemoryStateV30,
    // Exact descriptor ordinals name current logical source allocations. These
    // are not local values, target pointers, or inferred latest generations.
    slots: Map<int, MemoryPointerV30>,
    objects: Map<int, InvocationSourceObjectV40>,
    logical: InvocationSourceLogicalV38,
}

struct InvocationSourceObjectV40 {
    descriptor: int,
    slot: InvocationSourceSlotV36,
    activation: int,
}

// Object bindings use the generated original-owner roster. Dynamic allocation
// currentness is still checked independently at each address use.
open spec fn invocation_source_byte_state_well_formed_v36(source: InvocationSourceByteStateV36) -> bool {
    byte_memory_well_formed_v30(source.machine.memory)
        && invocation_source_logical_well_formed_v38(source.logical, source.machine.values.len())
        && (forall|local: int| source.logical.aggregates.contains_key(local) ==>
            source.machine.values[local] == MemoryValueV30::Undefined
                && !source.objects.contains_key(local))
        && byte_frame_runtime_well_formed_v30(source.machine.frames)
        && byte_private_frames_live_v30(source.machine.memory, source.machine.frames)
        && private_generation_counters_valid_v30(source.machine.generations, source.machine.memory)
        && source.slots.dom().finite()
        && source.objects.dom().finite()
        && (forall|local: int| source.objects.contains_key(local) ==>
            0 <= local < source.machine.values.len()
                && source.objects[local].activation >= 0
                && invocation_source_object_binding_v40(local, source.objects[local])
                && source.slots.contains_key(source.objects[local].descriptor))
        && (forall|left: int, right: int| source.objects.contains_key(left)
            && source.objects.contains_key(right) && left != right ==>
                source.objects[left].descriptor != source.objects[right].descriptor)
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
    SliceElement(InvocationSourceSliceReadV41),
    Slot { descriptor: int, slot: InvocationSourceSlotV36 },
    ObjectLocal(int),
    PointerLocal(int),
}

struct InvocationSourceByteAccessV36 {
    base: InvocationSourceByteBaseV36,
    offset: int,
    width: int,
    alignment: int,
}

enum InvocationSourceByteValueV36 {
    Component { local: int, source_type: int, ordinal: int, moved: bool },
    Constant(int),
    Local { local: int, moved: bool },
    Read { access: InvocationSourceByteAccessV36, moved: bool },
}

enum InvocationSourceOperandV36 {
    Aggregate { place: InvocationSourceAggregatePlaceV42, moved: bool },
    Scalar { value: InvocationSourceByteValueV36, bits: int },
    Pointer { local: int, moved: bool },
    Slice { local: int, moved: bool, metadata_bits: int },
}

enum InvocationSourceByteDestinationV36 {
    Component(InvocationSourceAggregatePlaceV42),
    Local(int),
    Memory(InvocationSourceByteAccessV36),
}

enum InvocationSourceByteEventV36 {
    AggregateDeinitialize(InvocationSourceAggregatePlaceV42),
    AggregateReset { local: int },
    AggregateTransfer { destination: InvocationSourceAggregatePlaceV42,
        source: InvocationSourceAggregatePlaceV42, moved: bool },
    Checked { destination: int, source_type: int, operation: int, bits: int, signed: bool,
        left: InvocationSourceByteValueV36, right: InvocationSourceByteValueV36 },
    Discriminant(InvocationSourceDiscriminantReadV41),
    Scalar,
    WitnessBorrow { destination: int, origin: int, source_type: int, generation: int,
        instance: int, block: int, statement: int },
    WitnessTransfer { destination: int, input: int, source_type: int, reference: bool, moved: bool },
    Pointer(InvocationSourcePointerEventV36),
    Transfer { destination: InvocationSourceByteDestinationV36,
        value: InvocationSourceByteValueV36, bits: int },
    Address { destination: int, access: InvocationSourceByteAccessV36 },
    Deinitialize(InvocationSourceByteAccessV36),
    ObjectLive { descriptor: int, slot: InvocationSourceSlotV36, local: int, activation: int },
    ObjectDead { local: int },
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
        InvocationSourceByteBaseV36::SliceElement(slice) =>
            invocation_source_slice_read_base_v41(source, slice, access.offset,
                access.width, access.alignment),
        InvocationSourceByteBaseV36::ObjectLocal(local) =>
            if source.objects.contains_key(local) {
                let object = source.objects[local];
                invocation_source_byte_slot_v36(source, object.descriptor, object.slot, root, instance)
            } else { None },
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
        InvocationSourceByteValueV36::Component { local, source_type, ordinal, moved } => {
            if !(0 <= ordinal < invocation_source_aggregate_leaf_count_v42(source_type)) {
                InvocationSourceByteEvaluationV36 { source: invocation_source_byte_refused_v36(source), value: MemoryValueV30::Undefined }
            } else {
                let component = invocation_source_aggregate_leaf_evaluate_v42(source, local, source_type,
                    invocation_source_aggregate_leaf_path_v42(source_type, ordinal), moved);
                match component.value {
                    Some(InvocationSourceValueV42::Carrier(value)) => InvocationSourceByteEvaluationV36 { source: component.source, value },
                    _ => InvocationSourceByteEvaluationV36 { source: invocation_source_byte_refused_v36(component.source), value: MemoryValueV30::Undefined },
                }
            }
        },
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
        InvocationSourceOperandV36::Aggregate { .. } => InvocationSourceByteEvaluationV36 {
            source: invocation_source_byte_refused_v36(source), value: MemoryValueV30::Undefined },
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
                logical: invocation_source_logical_write_v38(source.logical, local), ..source }
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

open spec fn invocation_source_object_end_v40(
    source: InvocationSourceByteStateV36, local: int, root: int, instance: int,
) -> InvocationSourceByteStateV36 {
    if !source.machine.valid || !invocation_source_byte_state_well_formed_v36(source)
        || !source.objects.contains_key(local) {
        invocation_source_byte_refused_v36(source)
    } else {
        let object = source.objects[local];
        let removed = InvocationSourceByteStateV36 { objects: source.objects.remove(local), ..source };
        invocation_source_byte_end_v36(removed, object.descriptor, object.slot, local, root, instance)
    }
}

open spec fn invocation_source_object_activate_v40(
    source: InvocationSourceByteStateV36, descriptor: int, slot: InvocationSourceSlotV36,
    local: int, activation: int, root: int, instance: int,
) -> InvocationSourceByteStateV36 {
    let object = InvocationSourceObjectV40 { descriptor, slot, activation };
    if !invocation_source_object_binding_v40(local, object) {
        invocation_source_byte_refused_v36(source)
    }
    else {
        // Restart invalidates the old dynamic identity even when source bytes
        // or the chosen physical backing happen to remain numerically equal.
        let before = if source.objects.contains_key(local) {
            invocation_source_object_end_v40(source, local, root, instance)
        } else { source };
        let active = invocation_source_byte_activate_v36(before, descriptor, slot, local, root, instance);
        if !active.machine.valid { active }
        else { InvocationSourceByteStateV36 { objects: active.objects.insert(local, object), ..active } }
    }
}

open spec fn invocation_source_byte_step_v36(
    source: InvocationSourceByteStateV36, event: InvocationSourceByteEventV36,
    root: int, instance: int, little_endian: bool,
) -> InvocationSourceByteStateV36 {
    match event {
        InvocationSourceByteEventV36::ObjectLive { descriptor, slot, local, activation } =>
            invocation_source_object_activate_v40(source, descriptor, slot, local, activation, root, instance),
        InvocationSourceByteEventV36::ObjectDead { local } =>
            invocation_source_object_end_v40(source, local, root, instance),
        InvocationSourceByteEventV36::WitnessBorrow { destination, origin, source_type,
            generation, instance, block, statement } => invocation_source_borrow_witness_v38(
                source, destination, origin, source_type, generation, instance, block, statement),
        InvocationSourceByteEventV36::WitnessTransfer { destination, input, source_type,
            reference, moved } => invocation_source_transfer_witness_v40(
                source, destination, input, source_type, reference, moved),
        // A scalar marker is not a no-op. The exact statement's existing
        // primitive graph must run on the current values between byte events.
        InvocationSourceByteEventV36::Scalar => invocation_source_byte_refused_v36(source),
        InvocationSourceByteEventV36::Pointer(event) =>
            invocation_source_pointer_step_v36(source, event, root, instance, little_endian),
        InvocationSourceByteEventV36::Discriminant(read) =>
            invocation_source_discriminant_read_v41(source, read, root, instance, little_endian).source,
        InvocationSourceByteEventV36::Checked { destination, source_type, operation, bits, signed, left, right } =>
            invocation_source_checked_v42(source, destination, source_type, operation, bits, signed, left, right, root, instance, little_endian),
        InvocationSourceByteEventV36::AggregateTransfer { destination, source: input, moved } =>
            invocation_source_aggregate_transfer_v42(source, destination, input, moved),
        InvocationSourceByteEventV36::AggregateDeinitialize(place) =>
            invocation_source_aggregate_place_deinitialize_v42(source, place),
        InvocationSourceByteEventV36::AggregateReset { local } =>
            invocation_source_byte_put_local_v36(source, local, MemoryValueV30::Undefined),
        InvocationSourceByteEventV36::Transfer { destination, value, bits } => {
            let evaluated = invocation_source_byte_evaluate_v36(source, value, bits, root, instance, little_endian);
            if !evaluated.source.machine.valid { evaluated.source }
            else { match destination {
                InvocationSourceByteDestinationV36::Component(place) =>
                    invocation_source_aggregate_place_replace_v42(evaluated.source, place,
                        InvocationSourceAggregateV42 { source_type: place.result_type,
                            leaves: Map::empty().insert(seq![], evaluated.value) }),
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
