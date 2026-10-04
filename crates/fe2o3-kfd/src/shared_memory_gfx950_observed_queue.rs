//! Closed queue observations over the same real observed memory owner.

#[path = "shared_memory_gfx950_observed_finite_join.rs"]
mod finite_join;

#[path = "shared_memory_gfx950_observed_multiwave_join.rs"]
mod multiwave_join;
#[cfg(test)]
pub(super) use finite_join::exercise_resources as exercise_finite_join_resources;
pub use finite_join::{
    Gfx950ObservedFiniteJoinOwnerV1, Gfx950ObservedFiniteJoinRootV1, Gfx950ObservedFiniteJoinV1,
};
#[cfg(test)]
pub(super) use multiwave_join::exercise_resources as exercise_multiwave_join_resources;
pub use multiwave_join::{
    Gfx950ObservedMultiwaveJoinOwnerV1, Gfx950ObservedMultiwaveJoinRootV1,
    Gfx950ObservedMultiwaveJoinV1,
};

#[path = "shared_memory_gfx950_observed_barrier.rs"]
mod barrier;
#[cfg(test)]
pub(super) use barrier::exercise_resources as exercise_barrier_resources;
pub use barrier::{
    Gfx950ObservedBarrierAndV1, Gfx950ObservedBarrierAndV2, observe_gfx950_queue_barrier_and_v1,
    observe_gfx950_queue_barrier_and_v2,
};

use super::*;
use crate::memory_linux::LinuxGfx950MemoryBackend;
use crate::queue::submit::{CwsrGeometryV1, initialize_amd_aql_control, initialize_invalid_ring};
use crate::queue_linux::{
    CwsrShadowPlanV1, LinuxCwsrShadowPagesV1, LinuxCwsrShadowsAfterEventDestroyedV1,
    LinuxCwsrShadowsReadyForReleaseV1, LinuxDoorbellSliceV1, LinuxKfdRuntimeEnabledV1,
    LinuxQueueExceptionEventV1, LinuxUnpublishedCwsrShadowPagesV1,
};
use fe2o3_kfd_uapi::{
    KFD_IOC_QUEUE_TYPE_COMPUTE_AQL, KfdIoctlCreateQueueArgs, KfdIoctlDestroyQueueArgs,
};
use std::mem::ManuallyDrop;

const PAGE: usize = 4096;
const GEOMETRY: CwsrGeometryV1 = CwsrGeometryV1::Gfx950Observed;
type Engine = SharedMemoryEngine<LinuxGfx950MemoryBackend>;
type QueueResult<T> = Result<T, String>;
type Cpu<P> = SharedGttAllocationV1<P, GttCpuWritableV1>;
type Mapped<P> = SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>;
type Executable = SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>;

fn describe(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

/// Actual create/map/destroy/release observations, not dispatch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx950ObservedQueueLifecycleV1 {
    unique_id: u64,
    queue_id: u32,
    doorbell_offset: u64,
    preserved_allocations: usize,
}
impl Gfx950ObservedQueueLifecycleV1 {
    /// Retained selected-device identity.
    pub fn unique_id(&self) -> u64 {
        self.unique_id
    }
    /// Queue ID observed only after its confirmed destruction.
    pub fn destroyed_queue_id(&self) -> u32 {
        self.queue_id
    }
    /// Offset within the complete 8192-byte mapping, never a GPU address.
    pub fn doorbell_byte_offset(&self) -> u64 {
        self.doorbell_offset
    }
    /// Original allocation records preserved through the same-engine round trip.
    pub fn preserved_allocation_records(&self) -> usize {
        self.preserved_allocations
    }
    /// The four private queue allocations were explicitly unmapped and released.
    pub fn released_queue_allocations(&self) -> usize {
        4
    }
    /// This closed path has no packet writer or doorbell-store operation.
    pub fn published_packets(&self) -> u64 {
        0
    }
}

/// Terminal failure retaining native custody until disposable process teardown.
/// No retry, cleanup, address, queue or memory-session extraction is exposed.
#[must_use = "failed queue custody requires disposable process teardown"]
pub struct Gfx950ObservedQueueFailureV1 {
    message: String,
    retained: ManuallyDrop<Box<NativeOwner>>,
}
impl Gfx950ObservedQueueFailureV1 {
    /// Failure context without releasing the native owner.
    pub fn message(&self) -> &str {
        &self.message
    }
}
impl core::fmt::Debug for Gfx950ObservedQueueFailureV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let _ = &self.retained;
        f.debug_struct("Gfx950ObservedQueueFailureV1")
            .field("message", &self.message)
            .field("retained", &true)
            .finish()
    }
}
impl core::fmt::Display for Gfx950ObservedQueueFailureV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.message)
    }
}
impl core::error::Error for Gfx950ObservedQueueFailureV1 {}

/// Create one queue, map its complete doorbell slice, then destroy and release it.
///
/// Consumes and returns the same observed memory owner. Ordinary initialized
/// roots are never referenced by the queue. Only private exact ring, control,
/// EOP and CWSR tokens can supply its native arguments. No packet, signal,
/// kernel, code or kernarg publication API exists. Success alone establishes no
/// loaded-driver authentication, model admission, coherence or retirement proof.
/// Run in a bounded disposable process. Failure retains/quarantines everything;
/// dropping the failure deliberately retains descriptors and mappings until exit.
///
/// ```compile_fail
/// use fe2o3_kfd::{Gfx950ObservedMemorySessionV1, observe_gfx950_queue_lifecycle_v1};
/// fn not_reusable(session: Gfx950ObservedMemorySessionV1) {
///     let _ = observe_gfx950_queue_lifecycle_v1(session);
///     let _ = observe_gfx950_queue_lifecycle_v1(session);
/// }
/// ```
pub fn observe_gfx950_queue_lifecycle_v1(
    session: Gfx950ObservedMemorySessionV1,
) -> Result<
    (
        Gfx950ObservedMemorySessionV1,
        Gfx950ObservedQueueLifecycleV1,
    ),
    Gfx950ObservedQueueFailureV1,
> {
    let mut owner = Box::new(NativeOwner::new(session));
    match run_lifecycle(owner.as_mut()) {
        Ok(()) => {
            let observation = owner
                .observation
                .take()
                .expect("successful lifecycle observation");
            let session = owner.memory.take().expect("same memory owner retained");
            Ok((session, observation))
        }
        Err(message) => Err(Gfx950ObservedQueueFailureV1 {
            message,
            retained: ManuallyDrop::new(owner),
        }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Step {
    Allocate,
    EnableRuntime,
    PrepareEventAndMappings,
    Create,
    MapDoorbell,
    CheckUnpublished,
    Destroy,
    DestroyEvent,
    DisableRuntime,
    ReleaseDoorbell,
    ReleaseMemory,
    Finish,
}
const STEPS: [Step; 12] = [
    Step::Allocate,
    Step::EnableRuntime,
    Step::PrepareEventAndMappings,
    Step::Create,
    Step::MapDoorbell,
    Step::CheckUnpublished,
    Step::Destroy,
    Step::DestroyEvent,
    Step::DisableRuntime,
    Step::ReleaseDoorbell,
    Step::ReleaseMemory,
    Step::Finish,
];
trait LifecycleBackend {
    fn preflight(&mut self) -> QueueResult<()>;
    fn currentness(&mut self) -> QueueResult<()>;
    fn step(&mut self, step: Step) -> QueueResult<()>;
    fn quarantine(&mut self);
}
fn run_lifecycle(backend: &mut impl LifecycleBackend) -> QueueResult<()> {
    let result = (|| {
        backend.preflight()?;
        for step in STEPS {
            backend.currentness()?;
            backend.step(step)?;
            backend.currentness()?;
        }
        Ok(())
    })();
    if result.is_err() {
        backend.quarantine();
    }
    result
}

struct CpuResources {
    ring: Cpu<UserptrAqlQueueProbeGttV1>,
    control: Cpu<UserptrAqlControlGttV1>,
    eop: Cpu<ExecutableGttV1>,
    cwsr: Cpu<ExecutableGttV1>,
}
struct MappedResources {
    ring: Mapped<UserptrAqlQueueProbeGttV1>,
    control: Mapped<UserptrAqlControlGttV1>,
    eop: Executable,
    cwsr: Executable,
}
fn allocate_resources<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    geometry: CwsrGeometryV1,
) -> QueueResult<CpuResources> {
    let mut ring = engine
        .allocate::<UserptrAqlQueueProbeGttV1>(PAGE)
        .map_err(describe)?;
    let mut control = engine
        .allocate::<UserptrAqlControlGttV1>(PAGE)
        .map_err(describe)?;
    let mut eop = engine.allocate::<ExecutableGttV1>(PAGE).map_err(describe)?;
    let mut cwsr = engine
        .allocate::<ExecutableGttV1>(geometry.total_bytes())
        .map_err(describe)?;
    engine
        .with_bytes_mut(&mut ring, initialize_invalid_ring)
        .map_err(describe)?
        .map_err(describe)?;
    engine
        .with_bytes_mut(&mut control, initialize_amd_aql_control)
        .map_err(describe)?
        .map_err(describe)?;
    engine
        .with_bytes_mut(&mut eop, |bytes| bytes.fill(0))
        .map_err(describe)?;
    engine
        .with_bytes_mut(&mut cwsr, |bytes| bytes.fill(0))
        .map_err(describe)?;
    Ok(CpuResources {
        ring,
        control,
        eop,
        cwsr,
    })
}
fn release_resources<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    resources: MappedResources,
) -> QueueResult<()> {
    let cwsr = engine.unmap_executable(resources.cwsr).map_err(describe)?;
    engine
        .release(cwsr, SharedAllocationPhaseV1::ExecutableImmutable)
        .map_err(describe)?;
    let eop = engine.unmap_executable(resources.eop).map_err(describe)?;
    engine
        .release(eop, SharedAllocationPhaseV1::ExecutableImmutable)
        .map_err(describe)?;
    let control = engine.unmap_mutable(resources.control).map_err(describe)?;
    engine
        .release(control, SharedAllocationPhaseV1::CpuWritable)
        .map_err(describe)?;
    let ring = engine.unmap_mutable(resources.ring).map_err(describe)?;
    engine
        .release(ring, SharedAllocationPhaseV1::CpuWritable)
        .map_err(describe)
}

fn active_engine(memory: &mut Option<Gfx950ObservedMemorySessionV1>) -> QueueResult<&mut Engine> {
    let memory = memory.as_mut().ok_or("missing observed memory owner")?;
    if memory.phase() != Gfx950ObservedMemoryPhaseV1::Active {
        return Err("observed memory owner is not active".into());
    }
    memory
        .core
        .engine
        .as_mut()
        .ok_or_else(|| "missing observed engine".into())
}
fn resource_va<P: GttProfileV1, S: GttAllocationStateV1>(
    engine: &Engine,
    token: &SharedGttAllocationV1<P, S>,
    phase: SharedAllocationPhaseV1,
) -> QueueResult<u64> {
    let index = engine.index(token, phase).map_err(describe)?;
    let record = &engine.allocations[index];
    if record.layout.requested_bytes != token.layout.requested_bytes
        || record.gpu_va == 0
        || !record.gpu_va.is_multiple_of(PAGE as u64)
        || record
            .gpu_va
            .checked_add(record.layout.gpu_va_bytes)
            .is_none()
    {
        return Err("queue resource exact retained extent/address".into());
    }
    Ok(record.gpu_va)
}
fn shadow_plan(engine: &Engine, token: &Cpu<ExecutableGttV1>) -> QueueResult<CwsrShadowPlanV1> {
    let index = engine
        .index(token, SharedAllocationPhaseV1::CpuWritable)
        .map_err(describe)?;
    let record = &engine.allocations[index];
    let reservation = record
        .reservation
        .as_ref()
        .ok_or("missing CWSR VA reservation")?;
    if record.profile != SharedGttProfileV1::Executable
        || record.userptr
        || record.layout.requested_bytes != GEOMETRY.total_bytes()
        || record.layout.cpu_mapping_bytes != GEOMETRY.total_bytes()
        || record.layout.gpu_va_bytes != GEOMETRY.total_bytes() as u64
        || <LinuxGfx950MemoryBackend as MemoryBackend>::reservation_address(reservation)
            != record.gpu_va
    {
        return Err("observed CWSR exact owned reservation".into());
    }
    CwsrShadowPlanV1::from_owned_reservation_for_geometry(
        record.gpu_va,
        record.layout.requested_bytes,
        engine.backend.page_size(),
        GEOMETRY,
    )
    .map_err(describe)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreservedRecord {
    id: u64,
    generation: u64,
    va: u64,
    handle: Option<u64>,
    profile: SharedGttProfileV1,
    phase: SharedAllocationPhaseV1,
    layout: SharedGttAllocationLayoutV1,
    free_attempted: bool,
}
fn preserved_records(engine: &Engine, count: usize) -> Vec<PreservedRecord> {
    engine
        .allocations
        .iter()
        .take(count)
        .map(|r| PreservedRecord {
            id: r.id,
            generation: r.generation,
            va: r.gpu_va,
            handle: r.handle,
            profile: r.profile,
            phase: r.phase,
            layout: r.layout,
            free_attempted: r.free_attempted,
        })
        .collect()
}
struct NativeOwner {
    memory: Option<Gfx950ObservedMemorySessionV1>,
    cpu: Option<CpuResources>,
    mapped: Option<MappedResources>,
    runtime: Option<LinuxKfdRuntimeEnabledV1>,
    event: Option<LinuxQueueExceptionEventV1>,
    unpublished: Option<LinuxUnpublishedCwsrShadowPagesV1>,
    shadows: Option<LinuxCwsrShadowPagesV1>,
    event_destroyed: Option<LinuxCwsrShadowsAfterEventDestroyedV1>,
    release: Option<LinuxCwsrShadowsReadyForReleaseV1>,
    doorbell: Option<LinuxDoorbellSliceV1>,
    queue_id: Option<u32>,
    doorbell_plan: Option<crate::engineering_gfx950_profile::Gfx950DoorbellPlanV1>,
    original: Vec<PreservedRecord>,
    original_session: u64,
    observation: Option<Gfx950ObservedQueueLifecycleV1>,
    runtime_attempted: bool,
    barrier: Option<barrier::Signal>,
    finite_join: Option<finite_join::Workload>,
    multiwave_join: Option<multiwave_join::Workload>,
}
impl NativeOwner {
    fn new(memory: Gfx950ObservedMemorySessionV1) -> Self {
        Self {
            memory: Some(memory),
            cpu: None,
            mapped: None,
            runtime: None,
            event: None,
            unpublished: None,
            shadows: None,
            event_destroyed: None,
            release: None,
            doorbell: None,
            queue_id: None,
            doorbell_plan: None,
            original: Vec::new(),
            original_session: 0,
            observation: None,
            runtime_attempted: false,
            barrier: None,
            finite_join: None,
            multiwave_join: None,
        }
    }
    fn check_unpublished(&mut self) -> QueueResult<()> {
        let engine = active_engine(&mut self.memory)?;
        self.runtime
            .as_ref()
            .ok_or("missing runtime")?
            .validate_queue_live_process(engine.backend.opener_pid())
            .map_err(describe)?;
        self.check_event_and_counters()
    }
    fn check_event(&mut self) -> QueueResult<()> {
        let engine = active_engine(&mut self.memory)?;
        self.event
            .as_ref()
            .ok_or("missing event")?
            .validate_live_with_shadows(
                engine.backend.kfd_fd(),
                engine.backend.opener_pid(),
                self.shadows.as_ref().ok_or("missing shadows")?,
            )
            .map_err(describe)?;
        Ok(())
    }
    fn check_event_and_counters(&mut self) -> QueueResult<()> {
        self.check_event()?;
        let engine = active_engine(&mut self.memory)?;
        let token = &self
            .mapped
            .as_ref()
            .ok_or("missing mapped queue resources")?
            .control;
        let index = engine
            .index(token, SharedAllocationPhaseV1::GpuAccessibleMutable)
            .map_err(describe)?;
        let record = &mut engine.allocations[index];
        let mapping = record.mapping.as_mut().ok_or("missing control mapping")?;
        let counters =
            LinuxGfx950MemoryBackend::observe_aql_counters(mapping, PAGE).map_err(describe)?;
        if let Some(signal) = self.barrier.as_mut() {
            signal.observe_counters(counters)?;
        } else if let Some(kernel) = self.finite_join.as_mut() {
            kernel.observe_counters(counters)?;
        } else if let Some(kernel) = self.multiwave_join.as_mut() {
            kernel.observe_counters(counters)?;
        } else if counters != (0, 0) {
            return Err("observed queue counters changed".into());
        }
        Ok(())
    }
}
impl LifecycleBackend for NativeOwner {
    fn preflight(&mut self) -> QueueResult<()> {
        let engine = active_engine(&mut self.memory)?;
        engine.check_currentness().map_err(describe)?;
        engine
            .backend
            .validate_observed_queue_profile()
            .map_err(describe)?;
        if engine
            .allocations
            .len()
            .checked_add(
                if self.barrier.is_some()
                    || self.finite_join.is_some()
                    || self.multiwave_join.is_some()
                {
                    5
                } else {
                    4
                },
            )
            .is_none_or(|n| n > MAX_SHARED_GTT_ALLOCATIONS_V1)
            || !engine.device_memory.is_empty()
            || engine.backend.page_size() != PAGE
        {
            return Err("observed queue resource preflight".into());
        }
        self.original_session = engine.session_id;
        self.original = preserved_records(engine, engine.allocations.len());
        Ok(())
    }
    fn currentness(&mut self) -> QueueResult<()> {
        active_engine(&mut self.memory)?
            .check_currentness()
            .map_err(describe)
    }
    fn quarantine(&mut self) {
        if let Some(memory) = self.memory.as_mut() {
            memory.quarantine();
        }
        if self.runtime_attempted {
            crate::queue_linux::permanently_poison_process_global_kfd_runtime_gate_v1();
        }
    }
    fn step(&mut self, step: Step) -> QueueResult<()> {
        match step {
            Step::Allocate => {
                self.cpu = Some(allocate_resources(
                    active_engine(&mut self.memory)?,
                    GEOMETRY,
                )?);
                if self.barrier.is_some() {
                    self.allocate_barrier_signal()?;
                } else if self.finite_join.is_some() {
                    self.allocate_finite_join_signal()?;
                } else if self.multiwave_join.is_some() {
                    self.allocate_multiwave_join_signal()?;
                }
            }
            Step::EnableRuntime => {
                self.runtime_attempted = true;
                let engine = active_engine(&mut self.memory)?;
                self.runtime = Some(
                    LinuxKfdRuntimeEnabledV1::enable(
                        engine.backend.kfd_fd(),
                        engine.backend.opener_pid(),
                    )
                    .map_err(describe)?,
                );
            }
            Step::PrepareEventAndMappings => {
                let engine = active_engine(&mut self.memory)?;
                let event = LinuxQueueExceptionEventV1::create(
                    engine.backend.kfd_fd(),
                    engine.backend.opener_pid(),
                )
                .map_err(describe)?;
                self.event = Some(event);
                let mut cpu = self.cpu.take().ok_or("missing CPU queue resources")?;
                let plan = shadow_plan(engine, &cpu.cwsr)?;
                self.unpublished = Some(
                    LinuxCwsrShadowPagesV1::install(
                        plan,
                        self.event.as_ref().expect("retained event"),
                    )
                    .map_err(describe)?,
                );
                let shadows = self
                    .unpublished
                    .as_ref()
                    .expect("retained shadows")
                    .shadows();
                engine
                    .with_bytes_mut(&mut cpu.cwsr, |bytes| {
                        shadows.initialize_and_validate_bo_headers(bytes)
                    })
                    .map_err(describe)?
                    .map_err(describe)?;
                self.event
                    .as_ref()
                    .expect("retained event")
                    .validate_live_with_shadows(
                        engine.backend.kfd_fd(),
                        engine.backend.opener_pid(),
                        shadows,
                    )
                    .map_err(describe)?;
                let eop = engine.seal_executable(cpu.eop).map_err(describe)?;
                let cwsr = engine.seal_executable(cpu.cwsr).map_err(describe)?;
                shadows
                    .restore_kernel_write_access_after_bo_seal()
                    .map_err(describe)?;
                self.mapped = Some(MappedResources {
                    ring: engine.map_mutable(cpu.ring).map_err(describe)?,
                    control: engine.map_mutable(cpu.control).map_err(describe)?,
                    eop: engine.map_executable(eop).map_err(describe)?,
                    cwsr: engine.map_executable(cwsr).map_err(describe)?,
                });
                if self.barrier.is_some() {
                    self.map_barrier_signal()?;
                } else if self.finite_join.is_some() {
                    self.map_finite_join_signal()?;
                } else if self.multiwave_join.is_some() {
                    self.map_multiwave_join_signal()?;
                }
            }
            Step::Create => {
                let engine = active_engine(&mut self.memory)?;
                let resources = self
                    .mapped
                    .as_ref()
                    .ok_or("missing mapped queue resources")?;
                let ring = resource_va(
                    engine,
                    &resources.ring,
                    SharedAllocationPhaseV1::GpuAccessibleMutable,
                )?;
                let control = resource_va(
                    engine,
                    &resources.control,
                    SharedAllocationPhaseV1::GpuAccessibleMutable,
                )?;
                let eop = resource_va(
                    engine,
                    &resources.eop,
                    SharedAllocationPhaseV1::GpuAccessibleExecutable,
                )?;
                let cwsr = resource_va(
                    engine,
                    &resources.cwsr,
                    SharedAllocationPhaseV1::GpuAccessibleExecutable,
                )?;
                let expected = create_args(ring, control, eop, cwsr, engine.backend.gpu_id())?;
                self.runtime
                    .as_ref()
                    .ok_or("missing runtime")?
                    .validate_active(engine.backend.kfd_fd(), engine.backend.opener_pid())
                    .map_err(describe)?;
                self.event
                    .as_ref()
                    .ok_or("missing event")?
                    .validate_live_with_shadows(
                        engine.backend.kfd_fd(),
                        engine.backend.opener_pid(),
                        self.unpublished
                            .as_ref()
                            .ok_or("missing unpublished shadows")?
                            .shadows(),
                    )
                    .map_err(describe)?;
                // KFD may retain the header pointer from this native boundary.
                self.shadows = Some(
                    self.unpublished
                        .take()
                        .expect("validated unpublished shadows")
                        .publish_for_native_queue_creation(),
                );
                let mut actual = expected;
                crate::queue_linux::create_queue(engine.backend.kfd_fd(), &mut actual)
                    .map_err(describe)?;
                let plan = validate_create_output(expected, actual)?;
                self.queue_id = Some(actual.queue_id);
                self.doorbell_plan = Some(plan);
                self.runtime
                    .as_mut()
                    .ok_or("missing runtime")?
                    .mark_queue_created()
                    .map_err(describe)?;
            }
            Step::MapDoorbell => {
                let engine = active_engine(&mut self.memory)?;
                let plan = self.doorbell_plan.ok_or("missing doorbell output")?;
                self.doorbell = Some(
                    LinuxDoorbellSliceV1::map_gfx950(
                        engine.backend.kfd_fd(),
                        plan,
                        engine.backend.opener_pid(),
                    )
                    .map_err(describe)?,
                );
            }
            Step::CheckUnpublished => {
                self.check_unpublished()?;
                if self.barrier.is_some() {
                    self.execute_barrier()?;
                } else if self.finite_join.is_some() {
                    self.execute_finite_join()?;
                } else if self.multiwave_join.is_some() {
                    self.execute_multiwave_join()?;
                }
            }
            Step::Destroy => {
                self.check_unpublished()?;
                if self.barrier.is_some() {
                    self.check_completed_barrier()?;
                } else if self.finite_join.is_some() {
                    self.check_completed_finite_join()?;
                } else if self.multiwave_join.is_some() {
                    self.check_completed_multiwave_join()?;
                }
                let engine = active_engine(&mut self.memory)?;
                let queue_id = self.queue_id.ok_or("missing live queue")?;
                let expected = KfdIoctlDestroyQueueArgs::new(queue_id);
                let mut actual = expected;
                crate::queue_linux::destroy_queue(engine.backend.kfd_fd(), &mut actual)
                    .map_err(describe)?;
                if actual != expected {
                    return Err("DESTROY_QUEUE immutable inputs changed".into());
                }
                self.runtime
                    .as_mut()
                    .ok_or("missing runtime")?
                    .mark_queue_destroyed()
                    .map_err(describe)?;
                self.queue_id = None;
                if let Some(signal) = self.barrier.as_mut() {
                    signal.confirm_destroyed()?;
                } else if let Some(kernel) = self.finite_join.as_mut() {
                    kernel.confirm_destroyed()?;
                } else if let Some(kernel) = self.multiwave_join.as_mut() {
                    kernel.confirm_destroyed()?;
                }
                self.observation = Some(Gfx950ObservedQueueLifecycleV1 {
                    unique_id: self.memory.as_ref().expect("retained memory").unique_id(),
                    queue_id,
                    doorbell_offset: self
                        .doorbell_plan
                        .ok_or("missing doorbell plan")?
                        .queue_byte_offset,
                    preserved_allocations: self.original.len(),
                });
            }
            Step::DestroyEvent => {
                self.check_event_and_counters()?;
                if self.barrier.is_some() {
                    self.check_completed_barrier()?;
                } else if self.finite_join.is_some() {
                    self.check_completed_finite_join()?;
                } else if self.multiwave_join.is_some() {
                    self.check_completed_multiwave_join()?;
                }
                let engine = active_engine(&mut self.memory)?;
                let destroyed = self
                    .event
                    .take()
                    .ok_or("missing event")?
                    .destroy(engine.backend.kfd_fd(), engine.backend.opener_pid())
                    .map_err(describe)?;
                self.runtime
                    .as_mut()
                    .ok_or("missing runtime")?
                    .mark_event_destroyed()
                    .map_err(describe)?;
                self.event_destroyed = Some(
                    self.shadows
                        .take()
                        .ok_or("missing published shadows")?
                        .after_event_destroy(destroyed)
                        .map_err(describe)?,
                );
            }
            Step::DisableRuntime => {
                let engine = active_engine(&mut self.memory)?;
                let disabled = self
                    .runtime
                    .take()
                    .ok_or("missing runtime")?
                    .disable(engine.backend.kfd_fd(), engine.backend.opener_pid())
                    .map_err(describe)?;
                self.release = Some(
                    self.event_destroyed
                        .take()
                        .ok_or("missing destroyed event custody")?
                        .after_runtime_destroy(disabled)
                        .map_err(describe)?,
                );
            }
            Step::ReleaseDoorbell => self
                .doorbell
                .take()
                .ok_or("missing doorbell")?
                .release()
                .map_err(describe)?,
            Step::ReleaseMemory => {
                self.release
                    .as_ref()
                    .ok_or("missing release custody")?
                    .validate_for_release()
                    .map_err(describe)?;
                if self.barrier.is_some() {
                    self.release_barrier_signal()?;
                } else if self.finite_join.is_some() {
                    self.release_finite_join_signal()?;
                } else if self.multiwave_join.is_some() {
                    self.release_multiwave_join_signal()?;
                }
                let resources = self.mapped.take().ok_or("missing mapped resources")?;
                release_resources(active_engine(&mut self.memory)?, resources)?;
            }
            Step::Finish => {
                let engine = active_engine(&mut self.memory)?;
                if engine.session_id != self.original_session
                    || engine.allocations.len()
                        != self.original.len()
                            + if self.barrier.is_some()
                                || self.finite_join.is_some()
                                || self.multiwave_join.is_some()
                            {
                                5
                            } else {
                                4
                            }
                    || preserved_records(engine, self.original.len()) != self.original
                    || engine.allocations[self.original.len()..]
                        .iter()
                        .any(|r| !r.is_fully_released())
                    || self.queue_id.is_some()
                    || self.doorbell.is_some()
                    || self.runtime.is_some()
                    || self.event.is_some()
                    || self.shadows.is_some()
                    || self.unpublished.is_some()
                    || self.cpu.is_some()
                    || self.mapped.is_some()
                    || self.event_destroyed.is_some()
                    || self
                        .barrier
                        .as_ref()
                        .is_some_and(|signal| !signal.released())
                    || self
                        .finite_join
                        .as_ref()
                        .is_some_and(|kernel| !kernel.released())
                    || self
                        .multiwave_join
                        .as_ref()
                        .is_some_and(|kernel| !kernel.released())
                {
                    return Err("same-owner queue release accounting".into());
                }
                self.release
                    .take()
                    .ok_or("missing final release custody")?
                    .complete()
                    .map_err(describe)?;
            }
        }
        Ok(())
    }
}

fn create_args(
    ring: u64,
    control: u64,
    eop: u64,
    cwsr: u64,
    gpu_id: u32,
) -> QueueResult<KfdIoctlCreateQueueArgs> {
    let ranges = [
        (ring, PAGE),
        (control, PAGE),
        (eop, PAGE),
        (cwsr, GEOMETRY.total_bytes()),
    ];
    for (index, &(base, bytes)) in ranges.iter().enumerate() {
        let end = base
            .checked_add(bytes as u64)
            .ok_or("queue resource range overflow")?;
        if base == 0 || !base.is_multiple_of(PAGE as u64) {
            return Err("queue resource alignment".into());
        }
        if ranges[..index]
            .iter()
            .any(|&(p, n)| p < end && base < p + n as u64)
        {
            return Err("queue resource alias".into());
        }
    }
    Ok(KfdIoctlCreateQueueArgs {
        ring_base_address: ring,
        write_pointer_address: control + 0x38,
        read_pointer_address: control + 0x80,
        doorbell_offset: u64::MAX,
        ring_size: PAGE as u32,
        gpu_id,
        queue_type: KFD_IOC_QUEUE_TYPE_COMPUTE_AQL,
        queue_percentage: 100,
        queue_priority: 0,
        queue_id: u32::MAX,
        eop_buffer_address: eop,
        eop_buffer_size: PAGE as u64,
        ctx_save_restore_address: cwsr,
        ctx_save_restore_size: GEOMETRY.context_bytes() as u32,
        ctl_stack_size: GEOMETRY.control_bytes() as u32,
        sdma_engine_id: 0,
        pad: 0,
    })
}
fn validate_create_output(
    expected: KfdIoctlCreateQueueArgs,
    actual: KfdIoctlCreateQueueArgs,
) -> QueueResult<crate::engineering_gfx950_profile::Gfx950DoorbellPlanV1> {
    let mut unchanged = actual;
    unchanged.queue_id = expected.queue_id;
    unchanged.doorbell_offset = expected.doorbell_offset;
    if unchanged != expected {
        return Err("CREATE_QUEUE immutable inputs changed".into());
    }
    crate::engineering_gfx950_profile::admit_doorbell(
        actual.queue_id,
        actual.doorbell_offset,
        expected.gpu_id,
    )
    .map_err(str::to_owned)
}

#[cfg(test)]
pub(super) fn exercise_resources<B: MemoryBackend>(
    core: &mut ObservedMemoryCore<B>,
    before_release: impl FnOnce(&mut SharedMemoryEngine<B>),
) -> QueueResult<()> {
    // Real coordinator + shared engine. Only kernel runtime/event/queue/doorbell
    // steps are inert: this fixture neither opens KFD nor installs CWSR shadows.
    struct Backend<'a, B: MemoryBackend, F> {
        core: &'a mut ObservedMemoryCore<B>,
        cpu: Option<CpuResources>,
        mapped: Option<MappedResources>,
        before_release: Option<F>,
    }
    impl<B: MemoryBackend, F: FnOnce(&mut SharedMemoryEngine<B>)> LifecycleBackend
        for Backend<'_, B, F>
    {
        fn preflight(&mut self) -> QueueResult<()> {
            if self.core.phase != Gfx950ObservedMemoryPhaseV1::Active {
                return Err("test observed owner is not active".into());
            }
            self.core.test_engine().require_active().map_err(describe)
        }
        fn currentness(&mut self) -> QueueResult<()> {
            self.core
                .test_engine()
                .check_currentness()
                .map_err(describe)
        }
        fn step(&mut self, step: Step) -> QueueResult<()> {
            match step {
                Step::Allocate => {
                    self.cpu = Some(allocate_resources(self.core.test_engine(), GEOMETRY)?);
                }
                Step::PrepareEventAndMappings => {
                    let cpu = self.cpu.take().ok_or("test CPU resources")?;
                    let engine = self.core.test_engine();
                    let eop = engine.seal_executable(cpu.eop).map_err(describe)?;
                    let cwsr = engine.seal_executable(cpu.cwsr).map_err(describe)?;
                    self.mapped = Some(MappedResources {
                        ring: engine.map_mutable(cpu.ring).map_err(describe)?,
                        control: engine.map_mutable(cpu.control).map_err(describe)?,
                        eop: engine.map_executable(eop).map_err(describe)?,
                        cwsr: engine.map_executable(cwsr).map_err(describe)?,
                    });
                }
                Step::ReleaseMemory => {
                    let before_release = self.before_release.take().ok_or("test release hook")?;
                    before_release(self.core.test_engine());
                    let mapped = self.mapped.take().ok_or("test mapped resources")?;
                    release_resources(self.core.test_engine(), mapped)?;
                }
                Step::Finish => {
                    if self.cpu.is_some() || self.mapped.is_some() || self.before_release.is_some()
                    {
                        return Err("test resource lifecycle incomplete".into());
                    }
                }
                _ => {}
            }
            Ok(())
        }
        fn quarantine(&mut self) {
            self.core.quarantine();
        }
    }
    run_lifecycle(&mut Backend {
        core,
        cpu: None,
        mapped: None,
        before_release: Some(before_release),
    })
}

#[cfg(test)]
#[path = "shared_memory_gfx950_observed_queue_tests.rs"]
mod tests;
