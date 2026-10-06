//! Original enum discriminants use the original tag contract and current source
//! object. A pointer niche observation never yields a reference payload.
use super::super::slots::SourceTagClassV39 as Class;
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssignmentV1 as Assignment, SemanticEnumEncodingV1 as Encoding,
    SemanticRustcVariantsV1 as Variants,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Read {
    destination: usize,
    access: Access,
    source_type: TypeId,
    tag_alignment: u64,
    bits: u32,
}

impl Read {
    pub(super) fn derive(
        context: &Context<'_, '_, '_>,
        assignment: &Assignment,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(2)?;
        let Rvalue::Discriminant(place) = assignment.value().kind() else {
            return Ok(None);
        };
        let ty = assignment.value().result_type();
        if assignment.destination().ty() != ty {
            return Err(mismatch());
        }
        let recipe = context.slots.tag_contract(place.ty(), out)?;
        if !matches!(
            recipe.class(out)?,
            Class::DirectScalar | Class::ScalarNiche { .. } | Class::PointerNullReference { .. }
        ) {
            return Err(unsupported());
        }
        let declaration = recipe.declaration(out)?;
        let (Shape::Enum { discriminant, .. }, Variants::Multiple(layout)) =
            (declaration.shape(), declaration.layout().variants())
        else {
            return Err(mismatch());
        };
        out.budget.charge_work(5)?;
        if *discriminant != ty
            || !context
                .types
                .get(place.ty().index() as usize)
                .is_some_and(|original| std::ptr::eq(original, declaration))
        {
            return Err(mismatch());
        }
        let ScalarV30::Integer { width: bits, .. } = context.scalar(ty, out)? else {
            return Err(unsupported());
        };
        let Destination::Local(destination) = context.destination(assignment.destination(), out)?
        else {
            return Err(unsupported());
        };
        let access = context.access(place, out)?;
        if access.ty != place.ty() || declaration.layout().size_bytes() != Some(access.bytes) {
            return Err(mismatch());
        }
        let (offset, primitive) = match layout.encoding() {
            Encoding::Direct(tag) => (tag.tag_offset_bytes(), tag.tag().primitive()),
            Encoding::Niche(tag) => (tag.source().expected_offset_bytes(), tag.tag().primitive()),
        };
        let inherited = if offset == 0 {
            access.alignment
        } else {
            access.alignment.min(1u64 << offset.trailing_zeros())
        };
        let tag_alignment = inherited.min(primitive.alignment_bytes());
        let width = primitive.size_bytes().ok_or_else(unsupported)?;
        if offset.checked_add(width).ok_or(Resource::Arithmetic)? > access.bytes {
            return Err(mismatch());
        }
        Ok(Some(Self {
            destination,
            access,
            source_type: place.ty(),
            tag_alignment,
            bits,
        }))
    }

    pub(super) fn emit(self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        write!(out, "InvocationSourceByteEventV36::Discriminant(InvocationSourceDiscriminantReadV41 {{ destination: {}int, access: ", self.destination).map_err(|_| out.error())?;
        emit_access(self.access, out)?;
        write!(
            out,
            ", source_type: {}int, tag_alignment: {}int, bits: {}int }})",
            self.source_type.index(),
            self.tag_alignment,
            self.bits
        )
        .map_err(|_| out.error())
    }
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<Read>()
        + h::<Option<Read>>()
        + h::<Class>()
        + h::<super::super::slots::SourceTagRecipeV39<'_, '_, '_>>()
        + h::<(u64, BackendPrimitive)>()
        + 8 * size_of::<usize>()
        + 6 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_source_discriminants_v41_tests.rs"]
mod tests;
