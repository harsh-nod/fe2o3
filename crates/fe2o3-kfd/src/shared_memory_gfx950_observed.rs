//! Allocation-only reuse of the existing shared GTT transaction engine.

use super::*;

#[cfg(test)]
pub(super) fn exercise_multiwave_join_resources<B: MemoryBackend>(
    core: &mut ObservedMemoryCore<B>,
    mutation: usize,
    initialize: impl FnOnce(&mut B::Mapping) -> Result<(), MemorySessionError>,
    hook: impl FnMut(&'static str, &mut SharedMemoryEngine<B>),
) -> Result<(), String> {
    queue::exercise_multiwave_join_resources(core, mutation, initialize, hook)
}

#[cfg(test)]
pub(super) fn exercise_finite_join_resources<B: MemoryBackend>(
    core: &mut ObservedMemoryCore<B>,
    mutation: usize,
    initialize: impl FnOnce(&mut B::Mapping) -> Result<(), MemorySessionError>,
    hook: impl FnMut(&'static str, &mut SharedMemoryEngine<B>),
) -> Result<(), String> {
    queue::exercise_finite_join_resources(core, mutation, initialize, hook)
}

#[path = "shared_memory_gfx950_observed_queue.rs"]
mod queue;
pub use queue::{
    Gfx950ObservedBarrierAndV1, Gfx950ObservedBarrierAndV2, Gfx950ObservedFiniteJoinOwnerV1,
    Gfx950ObservedFiniteJoinRootV1, Gfx950ObservedFiniteJoinV1, Gfx950ObservedMultiwaveJoinOwnerV1,
    Gfx950ObservedMultiwaveJoinRootV1, Gfx950ObservedMultiwaveJoinV1, Gfx950ObservedQueueFailureV1,
    Gfx950ObservedQueueLifecycleV1, observe_gfx950_queue_barrier_and_v1,
    observe_gfx950_queue_barrier_and_v2, observe_gfx950_queue_lifecycle_v1,
};

#[cfg(test)]
pub(super) fn exercise_barrier_resources<B: MemoryBackend>(
    core: &mut ObservedMemoryCore<B>,
    initialize: impl FnOnce(&mut B::Mapping) -> Result<(), MemorySessionError>,
    before_signal: impl FnOnce(&mut SharedMemoryEngine<B>),
    before_release: impl FnOnce(&mut SharedMemoryEngine<B>),
) -> Result<(), String> {
    queue::exercise_barrier_resources(core, initialize, before_signal, before_release)
}

#[cfg(test)]
pub(super) fn exercise_queue_resources<B: MemoryBackend>(
    core: &mut ObservedMemoryCore<B>,
    before_release: impl FnOnce(&mut SharedMemoryEngine<B>),
) -> Result<(), String> {
    queue::exercise_resources(core, before_release)
}

/// Phase of an allocation-observation session, never a queue capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx950ObservedMemoryPhaseV1 {
    /// The retained, checked device and allocation engine accept operations.
    Active,
    /// An error retained all uncertain resources and permanently denied reuse.
    Quarantined,
    /// Every allocation was explicitly released and the device descriptors closed.
    Closed,
}

/// Sealed full initialization of one coherent GTT allocation on this session.
///
/// No inner shared token, GPU address, CPU mapping, or queue conversion is public.
/// Initialization records a real complete copy before GPU mapping, not device
/// execution or a device-atomic coherence proof.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx950ObservedInitializedGttV1;
/// fn duplicate(value: Gfx950ObservedInitializedGttV1) { let _ = value.clone(); }
/// ```
pub struct Gfx950ObservedInitializedGttV1 {
    token: SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>,
}

impl Gfx950ObservedInitializedGttV1 {
    /// Exact fully initialized logical byte extent.
    pub fn requested_bytes(&self) -> usize {
        self.token.layout.requested_bytes
    }

    /// The page-aligned base is retained privately by the actual KFD engine.
    pub const fn alignment(&self) -> u64 {
        HOST_VISIBLE_MEMORY_PAGE_BYTES_V1
    }
}

impl core::fmt::Debug for Gfx950ObservedInitializedGttV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Gfx950ObservedInitializedGttV1")
            .field("requested_bytes", &self.requested_bytes())
            .finish_non_exhaustive()
    }
}

pub(super) struct ObservedMemoryCore<B: MemoryBackend> {
    engine: Option<SharedMemoryEngine<B>>,
    phase: Gfx950ObservedMemoryPhaseV1,
}

impl<B: MemoryBackend> ObservedMemoryCore<B> {
    #[cfg(test)]
    pub(super) fn test_engine(&mut self) -> &mut SharedMemoryEngine<B> {
        self.engine.as_mut().expect("test retained engine")
    }

    #[cfg(test)]
    pub(super) fn test_phase(&self) -> Gfx950ObservedMemoryPhaseV1 {
        self.phase
    }

    pub(super) fn acquire(backend: B) -> Result<Self, MemorySessionError> {
        Ok(Self {
            engine: Some(SharedMemoryEngine::acquire(backend)?),
            phase: Gfx950ObservedMemoryPhaseV1::Active,
        })
    }

    fn apply<T>(
        &mut self,
        action: impl FnOnce(&mut SharedMemoryEngine<B>) -> Result<T, MemorySessionError>,
    ) -> Result<T, MemorySessionError> {
        if self.phase != Gfx950ObservedMemoryPhaseV1::Active {
            return Err(MemorySessionError::SharedSessionQuarantined);
        }
        let result = action(self.engine.as_mut().expect("active engine retained"));
        if result.is_err() {
            self.quarantine();
        }
        result
    }

    pub(super) fn quarantine(&mut self) {
        if self.phase == Gfx950ObservedMemoryPhaseV1::Closed {
            return;
        }
        self.phase = Gfx950ObservedMemoryPhaseV1::Quarantined;
        if let Some(engine) = self.engine.as_mut() {
            engine.phase = SharedMemorySessionPhaseV1::Quarantined;
        }
    }

    pub(super) fn initialize(
        &mut self,
        source: Box<[u8]>,
    ) -> Result<Gfx950ObservedInitializedGttV1, MemorySessionError> {
        self.apply(|engine| {
            if source.is_empty() {
                return Err(MemorySessionError::InvalidRequestedSize);
            }
            let mut token = engine.allocate::<HostVisibleCoherentGttV1>(source.len())?;
            engine.with_bytes_mut(&mut token, |bytes| bytes.copy_from_slice(&source))?;
            let token = engine.map_mutable(token)?;
            Ok(Gfx950ObservedInitializedGttV1 { token })
        })
    }

    pub(super) fn read(
        &mut self,
        token: &Gfx950ObservedInitializedGttV1,
    ) -> Result<Box<[u8]>, MemorySessionError> {
        self.apply(|engine| {
            engine.copy_mapped_host_visible_subrange(
                &token.token,
                0,
                token.requested_bytes() as u64,
            )
        })
    }

    pub(super) fn release(
        &mut self,
        token: Gfx950ObservedInitializedGttV1,
    ) -> Result<(), MemorySessionError> {
        self.apply(|engine| {
            let token = engine.unmap_mutable(token.token)?;
            engine.release(token, SharedAllocationPhaseV1::CpuWritable)
        })
    }

    pub(super) fn close(&mut self) -> Result<(), MemorySessionError> {
        self.apply(|engine| {
            if engine
                .allocations
                .iter()
                .any(|record| !record.is_fully_released())
                || !engine.device_memory.is_empty()
            {
                return Err(MemorySessionError::InvalidAllocationAuthority);
            }
            engine.check_currentness()
        })?;
        // Every mapping/BO/VA release succeeded before descriptor ownership ends.
        drop(self.engine.take());
        self.phase = Gfx950ObservedMemoryPhaseV1::Closed;
        Ok(())
    }
}

/// A real gfx950 VM/GTT owner with no model, code, queue, or dispatch authority.
///
/// Available only with the existing engineering feature. Acquisition consumes
/// separately checked observations and performs the existing ACQUIRE_VM path.
/// It neither authenticates the loaded driver/firmware nor registers a gfx942
/// model device. No conversion to a queue-capable shared session is provided.
/// On error the same owner is permanently quarantined; dropping it performs no
/// destructive memory cleanup. Use a bounded disposable process for observation.
///
/// ```compile_fail
/// use fe2o3_kfd::{Gfx950ObservedMemorySessionV1, SharedGttMemorySessionV1};
/// fn no_queue_conversion(value: Gfx950ObservedMemorySessionV1) {
///     let _: SharedGttMemorySessionV1 = value.into();
/// }
/// ```
#[must_use = "explicitly release all allocations and close, or retain quarantined custody"]
pub struct Gfx950ObservedMemorySessionV1 {
    core: ObservedMemoryCore<crate::memory_linux::LinuxGfx950MemoryBackend>,
    unique_id: u64,
    observation_profile: [u8; 32],
}

impl core::fmt::Debug for Gfx950ObservedMemorySessionV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Gfx950ObservedMemorySessionV1")
            .field("phase", &self.phase())
            .field("unique_id", &self.unique_id)
            .finish_non_exhaustive()
    }
}

impl Gfx950ObservedMemorySessionV1 {
    /// Consume checked gfx950 observations and acquire this process's actual VM.
    /// The common process/device registry refuses duplicate or failed attempts.
    pub fn acquire(
        device: crate::CheckedGfx950XnackMinusDevice,
    ) -> Result<Self, MemorySessionError> {
        let pid = std::process::id();
        let gpu_id = device.observation().kfd_gpu_id();
        let unique_id = device.observation().unique_id();
        let observation_profile = device.observation_profile_sha256_v1();
        begin_process_vm_attempt(pid, gpu_id)?;
        let result =
            ObservedMemoryCore::acquire(crate::memory_linux::LinuxGfx950MemoryBackend::new(device));
        finish_process_vm_attempt(result.is_ok(), pid, gpu_id);
        result.map(|core| Self {
            core,
            unique_id,
            observation_profile,
        })
    }

    /// Current actual owner phase; an error never reactivates this owner.
    pub fn phase(&self) -> Gfx950ObservedMemoryPhaseV1 {
        self.core.phase
    }
    /// Retained selected-device identity, not an admission certificate.
    pub fn unique_id(&self) -> u64 {
        self.unique_id
    }
    /// Exact checked-observation profile selected by the retained device token.
    pub fn observation_profile_sha256(&self) -> [u8; 32] {
        self.observation_profile
    }
    /// Allocate coherent GTT, copy the entire owned source, then map it once.
    pub fn initialize(
        &mut self,
        bytes: Box<[u8]>,
    ) -> Result<Gfx950ObservedInitializedGttV1, MemorySessionError> {
        self.core.initialize(bytes)
    }
    /// Copy the same initialized allocation while no address was ever published.
    pub fn read(
        &mut self,
        token: &Gfx950ObservedInitializedGttV1,
    ) -> Result<Box<[u8]>, MemorySessionError> {
        self.core.read(token)
    }
    /// Consume the exact token, unmap, then explicitly release its BO and VA.
    pub fn release(
        &mut self,
        token: Gfx950ObservedInitializedGttV1,
    ) -> Result<(), MemorySessionError> {
        self.core.release(token)
    }
    /// Permanently retain uncertain resources without cleanup or retry.
    pub fn quarantine(&mut self) {
        self.core.quarantine();
    }
    /// Close retained descriptors only after every allocation was released.
    pub fn close(&mut self) -> Result<(), MemorySessionError> {
        self.core.close()
    }
}
