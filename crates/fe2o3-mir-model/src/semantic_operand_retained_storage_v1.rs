//! Actual owned heap of one semantic operand, without admission or authority.
//! The enclosing caller accounts the operand header once.

use super::*;
use std::mem::size_of;

fn fixed<T: Copy>(_: &T) {}
fn copy_row<T: Copy>() {}

impl SemanticOperandV1 {
    /// Visit this operand's complete current owned heap without allocating.
    ///
    /// The first `(0, 1)` callback is a zero-byte operand visit and occurs before
    /// variant inspection. Each live Box then supplies `(length, element_size)`,
    /// including empty boxes. The caller must check multiplication/addition and
    /// debit its shared item/work limit on EVERY callback, including the first.
    /// Return the first error unchanged and discard any partial observation.
    ///
    /// The operand header and embedded place/constant/Box handles are excluded:
    /// they already belong to the caller's root header or collection slots.
    /// Projection rows are Copy and have no nested heap. Pointer/callable IDs
    /// refer to separate owners; this does not traverse those referents.
    /// No allocator metadata, stack, compiler arenas, peak or RSS is measured.
    /// This observer validates nothing and grants no source/proof authority.
    pub fn visit_retained_heap_storage_v1<E>(
        &self,
        mut visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        visit(0, 1)?;
        match self {
            Self::Copy(place) | Self::Move(place) => {
                let SemanticPlaceV1 {
                    local,
                    projections,
                    ty,
                } = place;
                fixed(local);
                fixed(ty);
                copy_row::<SemanticProjectionV1>();
                let _: &Box<[SemanticProjectionV1]> = projections;
                visit(projections.len(), size_of::<SemanticProjectionV1>())?;
            }
            Self::Constant(constant) => {
                let SemanticConstantV1 { ty, value } = constant;
                fixed(ty);
                match value {
                    SemanticConstantValueV1::ZeroSized => {}
                    SemanticConstantValueV1::Scalar(value) => fixed(value),
                    SemanticConstantValueV1::Pointer(value) => fixed(value),
                    SemanticConstantValueV1::Callable(value) => fixed(value),
                    SemanticConstantValueV1::Bytes(bytes) => {
                        let SemanticConstantBytesV1(bytes) = bytes;
                        let _: &Box<[u8]> = bytes;
                        visit(bytes.len(), size_of::<u8>())?;
                    }
                }
            }
        }
        Ok(())
    }
}
