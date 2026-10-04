//! Heap-only observation of an existing debug file owner.

use super::DebugSourceMapFileV1;

fn fixed<T: Copy>(_: &T) {}

impl DebugSourceMapFileV1 {
    /// Visits the actual retained path capacity once, including spare capacity.
    ///
    /// The callback receives (element count, byte width). The enclosing caller
    /// must first pay this row's header/root, use checked multiplication and
    /// explicit shared byte/item bounds, and discard incomplete observations.
    /// This method adds neither the inline row header nor a root visit. There
    /// is one callback, no variable traversal, and its first error is returned
    /// unchanged. No bytes are cloned, normalized, encoded or authenticated.
    ///
    /// This is logical owned payload, not allocator rounding, scratch or RSS.
    /// It does not change construction, validation, wire bytes or authority.
    pub fn visit_retained_heap_storage_v1<E>(
        &self,
        mut visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            identity,
            byte_len,
            display_path,
        } = self;
        fixed(identity);
        fixed(byte_len);
        let _: &String = display_path;
        visit(display_path.capacity(), size_of::<u8>())
    }
}
