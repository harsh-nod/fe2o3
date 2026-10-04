//! Complete, private descriptors for already emitted non-scalar transitions.
//! These do not retain an input wrapper's PC or registry admission predicate.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum TransitionBodyV56<'a> {
    Pointer(PointerByteOperationV30),
    Alloca(AllocaByteOperationV30),
    Storage(StorageByteOperationV37),
    View(StorageViewByteOperationV39),
    IntegralCast(IntegralByteCastV40),
    Checked(CheckedByteOperationV48),
    Float(FloatByteOperationV52),
    TaggedSelect(TaggedSelectV55<'a>),
    Index(IndexByteOperationV37),
    Trap,
}

impl<'a> TransitionBodyV56<'a> {
    pub(super) fn capture(
        plan: &ByteOperationV30<'a, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(1)?;
        Ok(Some(match plan {
            ByteOperationV30::Pointer(value) => Self::Pointer(*value),
            ByteOperationV30::Alloca(value) => Self::Alloca(*value),
            ByteOperationV30::Storage(value) => Self::Storage(*value),
            ByteOperationV30::View(value) => Self::View(*value),
            ByteOperationV30::IntegralCast(value) => Self::IntegralCast(*value),
            ByteOperationV30::Checked(value) => Self::Checked(*value),
            ByteOperationV30::Float(value) => Self::Float(*value),
            ByteOperationV30::TaggedSelect(value) => Self::TaggedSelect(*value),
            ByteOperationV30::Index(value) => Self::Index(*value),
            ByteOperationV30::Trap(_) => Self::Trap,
            ByteOperationV30::Scalar(_) => return Ok(None),
        }))
    }

    pub(super) fn requires_same_interpretation(self) -> bool {
        matches!(self, Self::Storage(_) | Self::View(_))
    }

    pub(super) fn same(
        self,
        other: TransitionBodyV56<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        // All non-Select rows contain only fixed-size primitive fields. Select
        // retains its exact type and compares it with the charged type walker.
        out.budget.charge_work(32)?;
        Ok(match (self, other) {
            (Self::Pointer(a), Self::Pointer(b)) => a == b,
            (Self::Alloca(a), Self::Alloca(b)) => a == b,
            (Self::Storage(a), Self::Storage(b)) => a == b,
            (Self::View(a), Self::View(b)) => a == b,
            (Self::IntegralCast(a), Self::IntegralCast(b)) => a == b,
            (Self::Checked(a), Self::Checked(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::TaggedSelect(a), Self::TaggedSelect(b)) => a.same_body_v56(&b, out)?,
            (Self::Index(a), Self::Index(b)) => a == b,
            (Self::Trap, Self::Trap) => true,
            _ => false,
        })
    }
}
