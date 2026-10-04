//! Original promoted enum events. Retained objects use the byte interpreter.
use super::super::slots::EnumFieldV47;
use super::*;
use enum_construction::{Payload, PayloadValue};
use fe2o3_mir_model::semantic_mir_v1::SemanticAggregateKindV1 as AggregateKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EnumLocal {
    local: usize,
    ty: TypeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LogicalEvent {
    Construct {
        destination: usize,
        ty: TypeId,
        variant: u32,
        first: usize,
        count: usize,
    },
    Transfer {
        destination: usize,
        input: usize,
        ty: TypeId,
        moved: bool,
    },
    Extract {
        destination: usize,
        input: usize,
        ty: TypeId,
        variant: u32,
        field: u32,
        moved: bool,
    },
    Discriminant {
        destination: usize,
        input: usize,
        ty: TypeId,
        bits: u32,
    },
    FieldDeinitialize {
        input: usize,
        ty: TypeId,
        variant: u32,
        field: u32,
    },
    Reset {
        local: usize,
    },
}

fn enum_local(
    context: &Context<'_, '_, '_>,
    local: u32,
    out: &mut Writer<'_, '_>,
) -> Result<Option<EnumLocal>> {
    out.budget.charge_work(3)?;
    let ty = context
        .function
        .locals()
        .get(local as usize)
        .ok_or_else(mismatch)?
        .ty();
    if !matches!(
        context.types.get(ty.index() as usize).map(Type::shape),
        Some(Shape::Enum { .. })
    ) || context
        .slots
        .has_original_object(context.root, context.instance, local, out)?
        || context.descriptor(local, out)?.is_some()
    {
        return Ok(None);
    }
    Ok(Some(EnumLocal {
        local: context.local(local)?,
        ty,
    }))
}

fn whole(local: EnumLocal, place: &Place) -> Result<()> {
    if place.projections().is_empty() && local.ty == place.ty() {
        Ok(())
    } else {
        Err(unsupported())
    }
}

fn field(
    context: &Context<'_, '_, '_>,
    local: EnumLocal,
    place: &Place,
    out: &mut Writer<'_, '_>,
) -> Result<(u32, u32, EnumFieldV47)> {
    out.budget.charge_work(4)?;
    let [downcast, selected] = place.projections() else {
        return Err(unsupported());
    };
    let (Projection::Downcast(variant), Projection::Field(field)) =
        (downcast.kind(), selected.kind())
    else {
        return Err(unsupported());
    };
    let (ty, kind) = context
        .slots
        .logical_enum_field_v47(local.ty, variant, field as usize, out)?
        .ok_or_else(unsupported)?;
    if downcast.result_type() != local.ty || selected.result_type() != ty || place.ty() != ty {
        return Err(mismatch());
    }
    Ok((variant, field, kind))
}

pub(super) fn derive(
    context: &Context<'_, '_, '_>,
    statement: &Statement,
    payloads: &mut Vec<Payload>,
    out: &mut Writer<'_, '_>,
) -> Result<Option<LogicalEvent>> {
    out.budget.charge_work(2)?;
    let event = match statement {
        Statement::StorageLive(local) | Statement::StorageDead(local) => {
            let Some(local) = enum_local(context, local.index(), out)? else {
                return Ok(None);
            };
            LogicalEvent::Reset { local: local.local }
        }
        Statement::Deinitialize(place) => {
            let Some(local) = enum_local(context, place.local().index(), out)? else {
                return Ok(None);
            };
            if place.projections().is_empty() {
                whole(local, place)?;
                LogicalEvent::Reset { local: local.local }
            } else {
                let (variant, field, _) = field(context, local, place, out)?;
                LogicalEvent::FieldDeinitialize {
                    input: local.local,
                    ty: local.ty,
                    variant,
                    field,
                }
            }
        }
        Statement::Assign(assignment) => {
            if assignment.destination().ty() != assignment.value().result_type() {
                return Err(mismatch());
            }
            match assignment.value().kind() {
                Rvalue::Aggregate(aggregate) => {
                    let AggregateKind::EnumVariant(variant) = aggregate.kind() else {
                        return Ok(None);
                    };
                    let Some(local) =
                        enum_local(context, assignment.destination().local().index(), out)?
                    else {
                        return Ok(None);
                    };
                    whole(local, assignment.destination())?;
                    let (_, count) = context
                        .slots
                        .logical_enum_variant_v47(local.ty, *variant, out)?
                        .ok_or_else(unsupported)?;
                    if aggregate.operands().len() != count {
                        return Err(mismatch());
                    }
                    let first = payloads.len();
                    for (ordinal, operand) in aggregate.operands().iter().enumerate() {
                        out.budget.charge_work(3)?;
                        let (ty, kind) = context
                            .slots
                            .logical_enum_field_v47(local.ty, *variant, ordinal, out)?
                            .ok_or_else(unsupported)?;
                        if operand.ty() != ty {
                            return Err(mismatch());
                        }
                        let (value, bytes) = match kind {
                            EnumFieldV47::Scalar(scalar) => (
                                PayloadValue::Scalar {
                                    value: context.value(operand, out)?,
                                    bits: scalar.width(),
                                },
                                if scalar == ScalarV30::Bool {
                                    1
                                } else {
                                    u64::from(scalar.width() / 8)
                                },
                            ),
                            EnumFieldV47::Reference {
                                scalar,
                                bytes,
                                alignment,
                                mutable,
                            } => {
                                if mutable && !matches!(operand, Operand::Move(_)) {
                                    return Err(unsupported());
                                }
                                let operand = context.typed_operand(operand, out)?;
                                if !matches!(operand.kind, OperandKind::Pointer { .. }) {
                                    return Err(unsupported());
                                }
                                (
                                    PayloadValue::Reference {
                                        operand,
                                        referent_bits: scalar.width(),
                                        referent_bytes: bytes,
                                        referent_alignment: alignment,
                                        mutable,
                                    },
                                    8,
                                )
                            }
                        };
                        if payloads.len() == payloads.capacity() {
                            return Err(Resource::Accounting.into());
                        }
                        payloads.push(Payload::logical(value, bytes));
                    }
                    LogicalEvent::Construct {
                        destination: local.local,
                        ty: local.ty,
                        variant: *variant,
                        first,
                        count,
                    }
                }
                Rvalue::Use(Operand::Copy(place) | Operand::Move(place)) => {
                    let Some(local) = enum_local(context, place.local().index(), out)? else {
                        return Ok(None);
                    };
                    let Rvalue::Use(operand) = assignment.value().kind() else {
                        return Err(mismatch());
                    };
                    let moved = matches!(operand, Operand::Move(_));
                    let Destination::Local(destination) =
                        context.destination(assignment.destination(), out)?
                    else {
                        return Err(unsupported());
                    };
                    if place.projections().is_empty() {
                        whole(local, place)?;
                        if assignment.destination().ty() != local.ty {
                            return Err(mismatch());
                        }
                        if !moved {
                            let Some(Shape::Enum { variants, .. }) = context
                                .types
                                .get(local.ty.index() as usize)
                                .map(Type::shape)
                            else {
                                return Err(mismatch());
                            };
                            for (variant, row) in variants.iter().enumerate() {
                                out.budget.charge_work(1)?;
                                if row.is_uninhabited() {
                                    continue;
                                }
                                let variant =
                                    u32::try_from(variant).map_err(|_| Resource::Arithmetic)?;
                                for ordinal in 0..row.fields().fields().len() {
                                    let (_, kind) = context
                                        .slots
                                        .logical_enum_field_v47(local.ty, variant, ordinal, out)?
                                        .ok_or_else(unsupported)?;
                                    if matches!(kind, EnumFieldV47::Reference { mutable: true, .. })
                                    {
                                        return Err(unsupported());
                                    }
                                }
                            }
                        }
                        LogicalEvent::Transfer {
                            destination,
                            input: local.local,
                            ty: local.ty,
                            moved,
                        }
                    } else {
                        let (variant, field, kind) = field(context, local, place, out)?;
                        if matches!(kind, EnumFieldV47::Reference { mutable: true, .. }) && !moved {
                            return Err(unsupported());
                        }
                        LogicalEvent::Extract {
                            destination,
                            input: local.local,
                            ty: local.ty,
                            variant,
                            field,
                            moved,
                        }
                    }
                }
                Rvalue::Discriminant(place) => {
                    let Some(local) = enum_local(context, place.local().index(), out)? else {
                        return Ok(None);
                    };
                    whole(local, place)?;
                    let Some(Shape::Enum { discriminant, .. }) = context
                        .types
                        .get(local.ty.index() as usize)
                        .map(Type::shape)
                    else {
                        return Err(mismatch());
                    };
                    if *discriminant != assignment.destination().ty() {
                        return Err(mismatch());
                    }
                    let ScalarV30::Integer { width: bits, .. } =
                        context.scalar(*discriminant, out)?
                    else {
                        return Err(unsupported());
                    };
                    let Destination::Local(destination) =
                        context.destination(assignment.destination(), out)?
                    else {
                        return Err(unsupported());
                    };
                    LogicalEvent::Discriminant {
                        destination,
                        input: local.local,
                        ty: local.ty,
                        bits,
                    }
                }
                _ => return Ok(None),
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(event))
}

impl LogicalEvent {
    pub(super) fn emit(self, payloads: &[Payload], out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(out, "InvocationSourceByteEventV36::LogicalEnum(").map_err(|_| out.error())?;
        match self {
            Self::Construct { destination, ty, variant, first, count } => {
                let end = first.checked_add(count).ok_or(Resource::Arithmetic)?;
                let fields = payloads.get(first..end).ok_or_else(mismatch)?;
                write!(out, "InvocationSourceLogicalEnumEventV47::Construct {{ destination: {destination}int, source_type: {}int, variant: {variant}int, fields: seq![", ty.index()).map_err(|_| out.error())?;
                for payload in fields { payload.emit(out)?; }
                write!(out, "] }}").map_err(|_| out.error())?;
            }
            Self::Transfer { destination, input, ty, moved } => write!(out,
                "InvocationSourceLogicalEnumEventV47::Transfer {{ destination: {destination}int, input: {input}int, source_type: {}int, moved: {moved} }}", ty.index()).map_err(|_| out.error())?,
            Self::Extract { destination, input, ty, variant, field, moved } => write!(out,
                "InvocationSourceLogicalEnumEventV47::Extract {{ destination: {destination}int, input: {input}int, source_type: {}int, variant: {variant}int, field: {field}int, moved: {moved} }}", ty.index()).map_err(|_| out.error())?,
            Self::Discriminant { destination, input, ty, bits } => write!(out,
                "InvocationSourceLogicalEnumEventV47::Discriminant {{ destination: {destination}int, input: {input}int, source_type: {}int, bits: {bits}int }}", ty.index()).map_err(|_| out.error())?,
            Self::FieldDeinitialize { input, ty, variant, field } => write!(out,
                "InvocationSourceLogicalEnumEventV47::FieldDeinitialize {{ input: {input}int, source_type: {}int, variant: {variant}int, field: {field}int }}", ty.index()).map_err(|_| out.error())?,
            Self::Reset { local } => write!(out,
                "InvocationSourceLogicalEnumEventV47::Reset {{ local: {local}int }}").map_err(|_| out.error())?,
        }
        write!(out, ")").map_err(|_| out.error())
    }
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<LogicalEvent>()
        + h::<Option<LogicalEvent>>()
        + h::<EnumLocal>()
        + h::<Option<EnumLocal>>()
        + h::<(u32, u32, EnumFieldV47)>()
        + 10 * size_of::<usize>()
        + 8 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_source_enum_events_v47_tests.rs"]
mod tests;
