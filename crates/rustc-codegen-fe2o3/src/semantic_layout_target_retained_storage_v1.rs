//! Heap-only observation of the actual rustc semantic-layout target owner.
//! Observation does not authenticate a target or change layout admission.

use super::{ActiveCodegenProfileV1, SemanticLayoutTargetV1};
use std::mem::size_of;

fn fixed<T: Copy>(_: &T) {}
fn owned_shape<T>(_: &T) {}

impl SemanticLayoutTargetV1 {
    /// Visit separately owned retained heap payloads as (element count, width).
    ///
    /// The two Box<str> payloads use their actual lengths. A present profile
    /// contributes one Box payload of size_of::<ActiveCodegenProfileV1>(), which
    /// already includes its inline Option<String>/String headers; then the CPU
    /// String, if present, and features String contribute their actual capacities.
    /// The profile's pointer/Option and the two string Box headers are already in
    /// this owner's inline header and must not be counted again.
    ///
    /// Visits are exactly 2 without a profile, 4 with a profile/absent CPU, and
    /// 5 with a profile/present CPU. A present empty String still gets a zero-byte
    /// visit. No separate root visit occurs: the enclosing observer must charge
    /// its complete inline header/root once before entry or already include it
    /// in an enclosing owner/array header.
    ///
    /// The same callback must checked-multiply count/width, checked-add bytes
    /// and items, and apply explicit remaining byte/item bounds. Charge one
    /// item per callback. The profile Box is charged before its child fields are
    /// traversed; the first Err stops immediately and is returned unchanged.
    /// Earlier callback effects are not rolled back: discard the entire partial
    /// observation on Err. A permissive callback is not a bounded owner report.
    ///
    /// No text scanning, allocation, cloning, normalization, hashing, target
    /// validation or authority is performed. Constructor scratch maps, rustc
    /// arenas and borrowed static target metadata are not retained by this owner.
    /// This measures logical retained payload, not allocator overhead/peak/RSS.
    pub(crate) fn visit_retained_heap_storage_v1<E>(
        &self,
        mut visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            llvm_target,
            data_layout,
            default_pointer_width_bits,
            active_codegen_profile,
        } = self;
        fixed(default_pointer_width_bits);
        owned_shape::<Box<str>>(llvm_target);
        owned_shape::<Box<str>>(data_layout);
        owned_shape::<Option<Box<ActiveCodegenProfileV1>>>(active_codegen_profile);
        visit(llvm_target.len(), size_of::<u8>())?;
        visit(data_layout.len(), size_of::<u8>())?;
        if let Some(profile) = active_codegen_profile {
            visit(1, size_of::<ActiveCodegenProfileV1>())?;
            let ActiveCodegenProfileV1 { cpu, features } = profile.as_ref();
            owned_shape::<Option<String>>(cpu);
            owned_shape::<String>(features);
            if let Some(cpu) = cpu {
                visit(cpu.capacity(), size_of::<u8>())?;
            }
            visit(features.capacity(), size_of::<u8>())?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "semantic_layout_target_retained_storage_v1_tests.rs"]
mod tests;
