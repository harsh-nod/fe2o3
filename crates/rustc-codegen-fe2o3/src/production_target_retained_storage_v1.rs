//! Heap-only composition for the actual authenticated production target.
//! No target constructor, session authentication or admission rule is changed.

use super::AuthenticatedProductionTargetV1;
use fe2o3_amd_target::ProductionAmdTargetProfileV1;

fn fixed<T: Copy>(_: &T) {}

impl AuthenticatedProductionTargetV1 {
    /// Visit the actual retained layout's heap with the same enclosing callback.
    ///
    /// The profile enum is inline and owns no heap. The layout owner delegates
    /// all boxed/string payloads through its owner-defined visitor. This method
    /// emits no extra root/header visit: account this complete wrapper header
    /// once, which already contains the inline layout header and profile.
    ///
    /// The caller must precharge that header/root, use checked arithmetic and
    /// explicit byte/item limits, and discard partial observations on the first
    /// Err. The child Result is propagated unchanged. Copy plus exhaustive enum
    /// matching guards the current profile; exhaustive field matching guards the
    /// move-only wrapper. Neither creates authority or makes the wrapper Copy.
    ///
    /// No target verification, rustc query, text normalization, hashing, clone,
    /// allocation, runtime work or full-bindings/peak/RSS claim is performed.
    pub(crate) fn visit_retained_heap_storage_v1<E>(
        &self,
        visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            profile,
            rustc_layout,
        } = self;
        fixed(profile);
        match profile {
            ProductionAmdTargetProfileV1::Gfx942 | ProductionAmdTargetProfileV1::Gfx950 => {}
        }
        rustc_layout.visit_retained_heap_storage_v1(visit)
    }
}

#[cfg(test)]
#[path = "production_target_retained_storage_v1_tests.rs"]
mod tests;
