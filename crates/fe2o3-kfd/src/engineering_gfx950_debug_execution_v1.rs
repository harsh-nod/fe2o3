//! Static retained preparation; the separate unsafe successor creates only an empty queue.
use super::{
    Allocation, Backend, Gfx950DebugColdOwnerV1, Gfx950DebugColdPreparationFactsV1,
    require_cold_context, trap,
};
use crate::memory::MemoryBackend;
use fe2o3_amdhsa_loader::{
    AdmittedProfile, ImageRange, InterSegmentGapOrdinal, MaterializationPlan, SegmentOrdinal,
};
use sha2::{Digest, Sha256};

#[path = "runtime_debug_queue_lifecycle_v1.rs"]
mod lifecycle;
#[path = "runtime_debug_execution_profile_v1.rs"]
mod profile;
use Gfx950DebugExecutionPreparationErrorV1 as E;
pub use lifecycle::{
    Gfx950DebugNativeRequirementV1, Gfx950DebugNativeUnavailableV1,
    Gfx950DebugQueueLifecycleErrorV1, Gfx950DebugQueueLifecycleEventV1,
    Gfx950DebugQueueLifecyclePhaseV1, validate_gfx950_debug_queue_lifecycle_v1,
};
pub use profile::{
    Gfx950DebugExecutionContractV1, Gfx950DebugExecutionPreparationErrorV1,
    Gfx950DebugQueueGeometryV1,
};

/// Move-only preparation retaining the genuine cold owner's complete resources.
///
/// This value is static admission. Its separately consuming local empty-queue
/// route remains only partial Lane 1, not debugger/execution acceptance.
/// Preparation itself performs no trap/runtime/queue action. The separate unsafe
/// local successor is packet-incapable; it does not enable dispatch, a stop, or
/// register/memory sampling. Drop preserves the cold owner's existing
/// process-lifetime retention/poisoning; it is NOT successful native cleanup.
///
/// The digest and checked image describe retained content, not source authority
/// or a proved debugtrap site. None of these getters revalidates native state.
///
/// ~~~compile_fail
/// use fe2o3_kfd::Gfx950DebugExecutionPreparationV1;
/// fn duplicate(v: Gfx950DebugExecutionPreparationV1) { let _ = v.clone(); }
/// ~~~
/// ~~~compile_fail
/// use fe2o3_kfd::Gfx950DebugExecutionPreparationV1;
/// fn fabricate() { let _ = Gfx950DebugExecutionPreparationV1 {}; }
/// ~~~
/// ~~~compile_fail
/// use fe2o3_kfd::Gfx950DebugExecutionPreparationV1;
/// fn launch(v: Gfx950DebugExecutionPreparationV1) { v.create_queue(); }
/// ~~~
/// ~~~compile_fail
/// use fe2o3_kfd::Gfx950DebugExecutionPreparationV1;
/// fn abandon(v: Gfx950DebugExecutionPreparationV1) { v.complete_teardown(true); }
/// ~~~
/// ~~~compile_fail
/// use fe2o3_kfd::Gfx950DebugExecutionPreparationV1;
/// fn require_send<T: Send>() {}
/// require_send::<Gfx950DebugExecutionPreparationV1>();
/// ~~~
/// ~~~compile_fail
/// use fe2o3_kfd::Gfx950DebugMetadataNoQueueOwnerV1;
/// fn promote(v: Gfx950DebugMetadataNoQueueOwnerV1) { v.prepare_debug_execution_profile(); }
/// ~~~
pub struct Gfx950DebugExecutionPreparationV1 {
    retained: Gfx950DebugColdOwnerV1,
    geometry: Gfx950DebugQueueGeometryV1,
    closure_sha256: [u8; 32],
}
impl Gfx950DebugColdOwnerV1 {
    /// Consumes actual cold custody and verifies its static CPU-side image and
    /// resource relations. No ioctl, native allocation, mapping, permission change or
    /// currentness check is performed. Normal loader parsing retains its existing
    /// bounded domain; this adapter allocates no image clone or scratch vector.
    ///
    /// Error also consumes the cold owner, preserving its retention policy.
    pub fn prepare_debug_execution_profile(self) -> Result<Gfx950DebugExecutionPreparationV1, E> {
        let (geometry, closure_sha256) = inspect(&self)?;
        Ok(Gfx950DebugExecutionPreparationV1 {
            retained: self,
            geometry,
            closure_sha256,
        })
    }
}
impl Gfx950DebugExecutionPreparationV1 {
    /// Enter the fixed packet-incapable LOCAL runtime/empty-queue workflow.
    /// No same-client debugger acceptance or trap-execution proof is produced.
    /// This may block in the actual runtime-enable ioctl.
    ///
    /// # Safety
    ///
    /// The caller must establish and retain an isolated disposable process with
    /// no foreign KFD/ROCr runtime, queue, code injection or concurrent runtime
    /// actor through completion or process termination, including after failure
    /// or Drop. The in-library gate does not prove whole-process exclusion.
    /// An external owned-family supervisor must bound blocking native calls and
    /// terminate/reap on ambiguity. No packet operation is exposed by the result.
    ///
    /// ```compile_fail,E0133
    /// use fe2o3_kfd::Gfx950DebugExecutionPreparationV1;
    /// fn unsafe_required(v: Gfx950DebugExecutionPreparationV1) {
    ///     let _ = v.begin_empty_queue_runtime();
    /// }
    /// ```
    pub unsafe fn begin_empty_queue_runtime(
        self,
    ) -> Result<
        super::empty_queue::Gfx950DebugRuntimeEnableReturnedV1,
        super::empty_queue::Gfx950DebugLocalFailureV1,
    > {
        super::empty_queue::begin(self.retained, self.geometry, self.closure_sha256)
    }

    pub fn preparation_facts(&self) -> Gfx950DebugColdPreparationFactsV1 {
        self.retained.facts()
    }
    pub fn kernel_resources(&self) -> fe2o3_amdhsa_loader::SelectedKernelResourceBindingV1 {
        self.retained.resources.get().context.kernels[&1].resources
    }
    pub const fn queue_geometry(&self) -> Gfx950DebugQueueGeometryV1 {
        self.geometry
    }
    pub const fn closure_sha256(&self) -> [u8; 32] {
        self.closure_sha256
    }
    /// A descriptive closed refusal, never a dispatch/teardown capability.
    pub const fn native_boundary(&self) -> Gfx950DebugNativeUnavailableV1 {
        Gfx950DebugNativeUnavailableV1::PREPARATION_ONLY
    }
}

pub(super) fn inspect(
    owner: &Gfx950DebugColdOwnerV1,
) -> Result<(Gfx950DebugQueueGeometryV1, [u8; 32]), E> {
    let retained = owner.resources.get();
    let context = &retained.context;
    require_cold_context(context).map_err(|_| E::ActiveContext)?;
    let device = context.backend.engineering_debug_device();
    if context.unique_id != device.observation().unique_id() {
        return Err(E::DeviceProfile);
    }
    crate::engineering_gfx950_profile::validate_profile(
        device.topology_snapshot(),
        context.unique_id,
    )
    .map_err(|_| E::DeviceProfile)?;
    if context.ring.write() != 0
        || context.ring.last_read() != 0
        || context.next_buffer != 1
        || context.next_kernel != 2
    {
        return Err(E::ActiveContext);
    }
    if context.kernels.len() != 1 || context.handles.len() != 2 {
        return Err(E::ResourceCardinality);
    }
    let kernel = context.kernels.get(&1).ok_or(E::ResourceCardinality)?;
    let mapped = retained.trap.as_ref().ok_or(E::ResourceCardinality)?;
    let metadata = retained.metadata.as_ref().ok_or(E::ResourceCardinality)?;
    if kernel.object.is_empty() || kernel.object.len() > profile::MAX_OBJECT_BYTES {
        return Err(E::ObjectBound);
    }
    let digest: [u8; 32] = Sha256::digest(&kernel.object).into();
    if digest != kernel.metadata.object_sha256
        || digest != owner.facts.artifact_sha256
        || kernel.object.len() != owner.facts.artifact_bytes
    {
        return Err(E::ObjectIdentity);
    }
    let closure =
        fe2o3_amdhsa_loader::validate(&kernel.object, AdmittedProfile::Gfx950XnackOffCov6)
            .map_err(|_| E::LoaderClosure)?
            .bind_kernel(kernel.inspected.name())
            .map_err(|_| E::LoaderClosure)?;
    if closure.selected_kernel() != &kernel.inspected
        || closure.resources() != kernel.resources
        || closure.identity_inputs().object_sha256() != digest
    {
        return Err(E::LoaderClosure);
    }
    profile::check_kernel(kernel.resources.into())?;
    let plan = closure.envelope().materialization();
    let image_len = usize::try_from(plan.image_len()).map_err(|_| E::ImageBound)?;
    if image_len == 0 || image_len > profile::MAX_IMAGE_BYTES || kernel.code.requested != image_len
    {
        return Err(E::ImageBound);
    }
    let descriptor = closure
        .selected_binding()
        .descriptor_address()
        .checked_sub(closure.envelope().plan().image_start())
        .ok_or(E::ImageIdentity)?;
    if descriptor != kernel.descriptor_offset
        || !descriptor.is_multiple_of(64)
        || descriptor
            .checked_add(64)
            .is_none_or(|v| v > image_len as u64)
    {
        return Err(E::ImageIdentity);
    }
    check_mapping(&kernel.code)?;
    check_mapping(mapped)?;
    let code_end = kernel
        .code
        .va
        .checked_add(kernel.code.backing as u64)
        .ok_or(E::ResourceIdentity)?;
    let trap_end = mapped
        .va
        .checked_add(mapped.backing as u64)
        .ok_or(E::ResourceIdentity)?;
    if kernel.code.handle == mapped.handle
        || !(code_end <= mapped.va || trap_end <= kernel.code.va)
        || !context.handles.contains(&kernel.code.handle)
        || !context.handles.contains(&mapped.handle)
    {
        return Err(E::ResourceIdentity);
    }
    let (Some(code_offset), Some(trap_offset)) = (kernel.code.mmap_offset, mapped.mmap_offset)
    else {
        return Err(E::ResourceIdentity);
    };
    if code_offset == trap_offset
        || context.mmap_offsets.len() != 2
        || !context.mmap_offsets.contains(&code_offset)
        || !context.mmap_offsets.contains(&trap_offset)
    {
        return Err(E::ResourceIdentity);
    }
    let backing = (kernel.code.backing as u64)
        .checked_add(mapped.backing as u64)
        .ok_or(E::ResourceAccounting)?;
    if backing != context.total_bytes || backing != owner.facts.mapped_backing_bytes {
        return Err(E::ResourceAccounting);
    }
    if !Backend::with_bytes(&kernel.code.mapping, kernel.code.backing, |bytes| {
        image_matches(bytes, plan)
    }) {
        return Err(E::ImageIdentity);
    }
    trap::check_text().map_err(|_| E::TrapIdentity)?;
    if mapped.requested != trap::text().len()
        || owner.facts.trap_sha256 != trap::TEXT_SHA256
        || owner.facts.trap_bytes != trap::text().len()
        || !Backend::with_bytes(&mapped.mapping, mapped.backing, |bytes| {
            bytes.get(..mapped.requested) == Some(trap::text())
                && all_zero(bytes.get(mapped.requested..))
        })
    {
        return Err(E::TrapIdentity);
    }
    if !metadata.matches_kernel(kernel) {
        return Err(E::MetadataIdentity);
    }
    let facts = metadata.facts();
    if facts.artifact_sha256 != digest
        || facts.artifact_bytes != kernel.object.len()
        || facts.logical_retained_bytes != owner.facts.metadata_retained_bytes
    {
        return Err(E::MetadataIdentity);
    }
    let geometry = profile::geometry(backing, kernel.object.len(), facts.logical_retained_bytes)?;
    Ok((geometry, closure.identity_inputs().closure_sha256()))
}
fn check_mapping(a: &Allocation) -> Result<(), E> {
    let rounded = a
        .requested
        .checked_add(4095)
        .map(|v| v & !4095)
        .ok_or(E::ResourceIdentity)?;
    if a.requested == 0
        || a.handle == 0
        || a.va == 0
        || !a.va.is_multiple_of(4096)
        || a.backing != rounded
        || Backend::mapping_address(&a.mapping) != a.va
        || Backend::reservation_address(&a.reservation) != a.va
    {
        return Err(E::ResourceIdentity);
    }
    Ok(())
}
fn range(bytes: &[u8], r: ImageRange) -> Option<&[u8]> {
    let start = usize::try_from(r.offset_from_image_start()).ok()?;
    let len = usize::try_from(r.byte_len()).ok()?;
    bytes.get(start..start.checked_add(len)?)
}
fn all_zero(bytes: Option<&[u8]>) -> bool {
    bytes.is_some_and(|b| b.iter().all(|v| *v == 0))
}
fn image_matches(bytes: &[u8], plan: &MaterializationPlan<'_>) -> bool {
    let Ok(image_len) = usize::try_from(plan.image_len()) else {
        return false;
    };
    let Some(image) = bytes.get(..image_len) else {
        return false;
    };
    if !all_zero(bytes.get(image_len..)) {
        return false;
    }
    for ordinal in [
        SegmentOrdinal::First,
        SegmentOrdinal::Second,
        SegmentOrdinal::Third,
    ] {
        let copy = plan.copy_phase().segment(ordinal);
        if range(image, copy.destination()) != Some(copy.source().bytes()) {
            return false;
        }
        let zeros = plan.zero_phase().segment(ordinal);
        for zero in [
            zeros.mapping_prefix(),
            zeros.memory_suffix(),
            zeros.mapping_tail(),
        ] {
            if !all_zero(range(image, zero.destination())) {
                return false;
            }
        }
    }
    [
        InterSegmentGapOrdinal::FirstToSecond,
        InterSegmentGapOrdinal::SecondToThird,
    ]
    .into_iter()
    .all(|o| {
        all_zero(range(
            image,
            plan.zero_phase().inter_segment_gap(o).destination(),
        ))
    })
}

#[cfg(test)]
#[path = "runtime_debug_execution_preparation_v1_tests.rs"]
mod tests;
