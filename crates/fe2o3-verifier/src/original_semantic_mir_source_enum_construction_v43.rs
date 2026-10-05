//! Original enum construction snapshots operands before touching its destination.
use super::super::slots::SourceTagClassV39 as Class;
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateKindV1 as AggregateKind, SemanticAssignmentV1 as Assignment,
    SemanticEnumEncodingV1 as Encoding, SemanticRustcVariantsV1 as Variants,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PayloadValue {
    Scalar {
        value: Value,
        bits: u32,
    },
    Reference {
        operand: TypedOperand,
        referent_bits: u32,
        referent_bytes: u64,
        referent_alignment: u64,
        mutable: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Payload {
    value: PayloadValue,
    offset: u64,
    bytes: u64,
    alignment: u64,
}

impl Payload {
    pub(super) fn logical(value: PayloadValue, bytes: u64) -> Self {
        Self {
            value,
            offset: 0,
            bytes,
            alignment: 1,
        }
    }

    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(out, "InvocationSourceEnumPayloadV43 {{ value: ").map_err(|_| out.error())?;
        match self.value {
            PayloadValue::Scalar { value, bits } => {
                write!(out, "InvocationSourceEnumValueV44::Scalar {{ value: ")
                    .map_err(|_| out.error())?;
                emit_value(value, out)?;
                write!(out, ", bits: {bits}int }}").map_err(|_| out.error())?;
            }
            PayloadValue::Reference {
                operand,
                referent_bits,
                referent_bytes,
                referent_alignment,
                mutable,
            } => {
                write!(out, "InvocationSourceEnumValueV44::Reference {{ operand: ")
                    .map_err(|_| out.error())?;
                operand.emit(out)?;
                write!(out, ", referent_bits: {referent_bits}int, referent_width: {referent_bytes}int, referent_alignment: {referent_alignment}int, mutable: {mutable} }}")
                    .map_err(|_| out.error())?;
            }
        }
        write!(
            out,
            ", offset: {}int, width: {}int, alignment: {}int }},",
            self.offset, self.bytes, self.alignment
        )
        .map_err(|_| out.error())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Construct {
    access: Access,
    source_type: TypeId,
    variant: u32,
    tag_alignment: u64,
    first: usize,
    count: usize,
}

fn inherited_alignment(parent: u64, offset: u64, natural: u64) -> u64 {
    let placed = if offset == 0 {
        parent
    } else {
        parent.min(1u64 << offset.trailing_zeros())
    };
    placed.min(natural)
}

impl Construct {
    pub(super) fn derive(
        context: &Context<'_, '_, '_>,
        assignment: &Assignment,
        payloads: &mut Vec<Payload>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(2)?;
        let Rvalue::Aggregate(aggregate) = assignment.value().kind() else {
            return Ok(None);
        };
        let AggregateKind::EnumVariant(variant) = aggregate.kind() else {
            return Ok(None);
        };
        let ty = assignment.value().result_type();
        if assignment.destination().ty() != ty {
            return Err(mismatch());
        }
        let recipe = context.slots.tag_contract(ty, out)?;
        if !matches!(
            recipe.class(out)?,
            Class::DirectScalar | Class::ScalarNiche { .. } | Class::PointerNullReference { .. }
        ) {
            return Err(unsupported());
        }
        let declaration = recipe.declaration(out)?;
        let (Shape::Enum { variants, .. }, Variants::Multiple(layout)) =
            (declaration.shape(), declaration.layout().variants())
        else {
            return Err(mismatch());
        };
        out.budget.charge_work(7)?;
        if !context
            .types
            .get(ty.index() as usize)
            .is_some_and(|original| std::ptr::eq(original, declaration))
            || variants.len() != layout.variants().len()
        {
            return Err(mismatch());
        }
        let variant_row = variants
            .get(*variant as usize)
            .filter(|row| !row.is_uninhabited())
            .ok_or_else(mismatch)?;
        let variant_layout = layout
            .variants()
            .get(*variant as usize)
            .ok_or_else(mismatch)?;
        let fields = variant_row.fields().fields();
        let offsets = variant_layout.aggregate().field_offsets();
        if fields.len() != aggregate.operands().len() || fields.len() != offsets.len() {
            return Err(mismatch());
        }
        let access = context.access(assignment.destination(), out)?;
        // Object identity comes from the original retained-local index. A tag
        // schema alone cannot authorize an external or pointer-relative write.
        if !matches!(access.address, Address::Object { .. })
            || access.ty != ty
            || declaration.layout().size_bytes() != Some(access.bytes)
        {
            return Err(unsupported());
        }
        let (tag_offset, primitive) = match layout.encoding() {
            Encoding::Direct(tag) => (tag.tag_offset_bytes(), tag.tag().primitive()),
            Encoding::Niche(tag) => (tag.source().expected_offset_bytes(), tag.tag().primitive()),
        };
        if tag_offset
            .checked_add(primitive.size_bytes().ok_or_else(unsupported)?)
            .ok_or(Resource::Arithmetic)?
            > access.bytes
        {
            return Err(mismatch());
        }
        let first = payloads.len();
        for ((&field, &offset), operand) in fields.iter().zip(offsets).zip(aggregate.operands()) {
            out.budget.charge_work(5)?;
            let original = context
                .types
                .get(field.index() as usize)
                .ok_or_else(mismatch)?;
            let bytes = original.layout().size_bytes().ok_or_else(unsupported)?;
            if operand.ty() != field
                || offset.checked_add(bytes).ok_or(Resource::Arithmetic)? > access.bytes
            {
                return Err(mismatch());
            }
            let value = if let Shape::Pointer(pointer) = original.shape() {
                out.budget.charge_work(7)?;
                let mutable = pointer.mutability()
                    == fe2o3_mir_model::semantic_mir_v1::SemanticMutabilityV1::Mutable;
                if pointer.kind() != PointerKind::Reference
                    || pointer.metadata() != PointerMetadata::None
                    || pointer.address_space() != 0
                    || pointer.pointer_width_bits() != 64
                    || bytes != 8
                    || mutable && !matches!(operand, Operand::Move(_))
                {
                    return Err(unsupported());
                }
                let referent = context
                    .types
                    .get(pointer.pointee().index() as usize)
                    .ok_or_else(mismatch)?;
                let scalar = context.scalar(pointer.pointee(), out)?;
                let operand = context.typed_operand(operand, out)?;
                if !matches!(operand.kind, OperandKind::Pointer { .. }) {
                    return Err(unsupported());
                }
                PayloadValue::Reference {
                    operand,
                    referent_bits: scalar.width(),
                    referent_bytes: referent.layout().size_bytes().ok_or_else(unsupported)?,
                    referent_alignment: referent.layout().alignment_bytes(),
                    mutable,
                }
            } else {
                PayloadValue::Scalar {
                    value: context.value(operand, out)?,
                    bits: context.scalar(field, out)?.width(),
                }
            };
            if payloads.len() == payloads.capacity() {
                return Err(Resource::Accounting.into());
            }
            payloads.push(Payload {
                value,
                offset,
                bytes,
                alignment: inherited_alignment(
                    access.alignment,
                    offset,
                    original.layout().alignment_bytes(),
                ),
            });
        }
        Ok(Some(Self {
            access,
            source_type: ty,
            variant: *variant,
            tag_alignment: inherited_alignment(
                access.alignment,
                tag_offset,
                primitive.alignment_bytes(),
            ),
            first,
            count: fields.len(),
        }))
    }

    pub(super) fn emit(self, payloads: &[Payload], out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(2)?;
        let end = self
            .first
            .checked_add(self.count)
            .ok_or(Resource::Arithmetic)?;
        let fields = payloads.get(self.first..end).ok_or_else(mismatch)?;
        write!(out, "InvocationSourceByteEventV36::EnumConstruct(InvocationSourceEnumConstructV43 {{ access: ")
            .map_err(|_| out.error())?;
        emit_access(self.access, out)?;
        write!(
            out,
            ", source_type: {}int, variant: {}int, tag_alignment: {}int, fields: seq![",
            self.source_type.index(),
            self.variant,
            self.tag_alignment
        )
        .map_err(|_| out.error())?;
        for field in fields {
            field.emit(out)?;
        }
        write!(out, "] }})").map_err(|_| out.error())
    }
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<Construct>()
        + h::<Option<Construct>>()
        + h::<Payload>()
        + h::<PayloadValue>()
        + h::<Vec<Payload>>()
        + h::<Class>()
        + h::<super::super::slots::SourceTagRecipeV39<'_, '_, '_>>()
        + h::<(u64, BackendPrimitive)>()
        + 14 * size_of::<usize>()
        + 10 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_source_enum_construction_v43_tests.rs"]
mod tests;
