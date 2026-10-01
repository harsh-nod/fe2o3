//! Data-only observation of retained layout-component backing.
use super::{RustLayoutEvidenceV1, RustPhysicalComponentV1};
use std::mem::size_of;

fn fixed<T: Copy>(_: &T) {}
fn fixed_type<T: Copy>() {}

impl RustLayoutEvidenceV1 {
    /// Visits the actual component Vec capacity once, not encoded size or length.
    ///
    /// The inline layout header belongs to the enclosing owner. The callback
    /// receives (count, byte width), must checked-multiply/add on the caller's
    /// original bounded byte/item ledger, and must discard partial observations
    /// on error. This method returns its first error unchanged. It performs no
    /// variable traversal: each component is statically Copy and owns no heap.
    /// No allocation, clone, canonical encoding, validation or authority change
    /// occurs. This is logical retained payload, not allocator, peak or RSS data.
    pub fn visit_retained_heap_storage_v1<E>(
        &self,
        mut visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            rust_type,
            abi_class,
            pointer_width,
            size,
            abi_alignment,
            components,
        } = self;
        fixed(rust_type);
        fixed(abi_class);
        fixed(pointer_width);
        fixed(size);
        fixed(abi_alignment);
        fixed_type::<RustPhysicalComponentV1>();
        let _: &Vec<RustPhysicalComponentV1> = components;
        visit(components.capacity(), size_of::<RustPhysicalComponentV1>())
    }
}
