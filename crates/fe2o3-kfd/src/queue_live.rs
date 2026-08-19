//! Safe, bounded Linux composition for one gfx942 compute-AQL queue.

use core::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_kfd_uapi::{
    KfdAqlComputeQueueBuffers, admit_kfd_aql_queue_ring_size, admit_kfd_queue_percentage,
    admit_kfd_queue_priority,
};
use fe2o3_runtime_model::{
    ComputeAqlQueuePlanV1, ComputeAqlQueueResourcesV1, ComputeAqlResourceBindingV1,
    ComputeAqlTargetProfileV1, DeviceIdentityStateV1, IdentityDigestV1, MemoryAccessV1,
    MemoryCoherenceV1, MemoryKindV1, MemoryLifecycleStateV1, QueueConfigurationIdV1,
    QueueGenerationV1, QueueInstanceIdV1, QueueKeyV1, QueuePlanIdV1,
};
use sha2::{Digest, Sha256};

use super::*;
use crate::queue_linux::{LinuxDoorbellErrorV1, LinuxDoorbellSliceV1};
use crate::shared_memory::{
    AqlContextSaveResourceRoleV1, AqlControlResourceRoleV1, AqlEndOfPipeResourceRoleV1,
    AqlQueueGttV1, AqlRingResourceRoleV1, ExecutableGttV1, GttGpuAccessibleExecutableV1,
    GttGpuAccessibleMutableV1, HostVisibleCoherentGttV1, SharedGttMemorySessionV1,
    SharedGttQueueResourceAuthorityV1,
};
use crate::{
    CheckedGfx942XnackMinusDevice, GFX942_QUEUE_RESOURCE_PROFILE_SHA256_V1,
    Gfx942AqlQueueResourcePlanV1, Gfx942QueueResourcePlanningError, MemorySessionError,
    SHARED_GTT_MEMORY_PROFILE_SHA256_V1, plan_gfx942_aql_queue_resources,
};

const CONTROL_BYTES: usize = 4_096;
static NEXT_QUEUE_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// Canonical claim boundary for the first live, non-dispatching queue session.
pub const GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-compute-aql-session-r4-v1\n",
    "target=gfx942:xnack-,SPX/NPS1,KFD-1.18,one-selected-current-device\n",
    "memory_profile_sha256=1054b1c31ad143c7218eee24bcc529b17851338a152ed0cf028c46898c6a17a4\n",
    "queue_resource_profile_sha256=b8317e4288e14c6d7546b53887ec2a10e1938ffba9595271d174a2a652320f4f\n",
    "resources=linear-private-ring-control-eop-cwsr-authorities,exact-one-vm,transferred-model-ownership\n",
    "gtt_policy=ring:aql-queue,control:host-visible-coherent,eop-and-cwsr:executable;fe2o3-policy-not-rocr-equivalence\n",
    "doorbell=complete-8192-byte-kfd-slice,exact-returned-offset,madv-dontfork,no-address-or-mmio-accessor\n",
    "lifecycle=explicit-create,active-or-disabled-direct-destroy,explicit-resource-return,no-drop-ioctl-store-munmap-or-free\n",
    "currentness=pid-and-device-before-and-after-ioctls-and-doorbell-boundaries\n",
    "failure=errno-indeterminate,retained-or-quarantined-authority,process-teardown-recovery-only\n",
    "proof=queue-model-obligations-only,concrete-ioctl-mmap-driver-firmware-refinement-contracted\n",
    "excluded=packet-publication,doorbell-store,dispatch,completion,update,multi-queue-concurrency\n",
);

/// SHA-256 of [`GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1`].
pub const GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1: &str =
    "b850dfe51698c97fa35f6faff22339844d454b6a220f3643e3bf886187097518";

type RingAuthority = SharedGttQueueResourceAuthorityV1<
    AqlRingResourceRoleV1,
    AqlQueueGttV1,
    GttGpuAccessibleMutableV1,
>;
type ControlAuthority = SharedGttQueueResourceAuthorityV1<
    AqlControlResourceRoleV1,
    HostVisibleCoherentGttV1,
    GttGpuAccessibleMutableV1,
>;
type EopAuthority = SharedGttQueueResourceAuthorityV1<
    AqlEndOfPipeResourceRoleV1,
    ExecutableGttV1,
    GttGpuAccessibleExecutableV1,
>;
type ContextSaveAuthority = SharedGttQueueResourceAuthorityV1<
    AqlContextSaveResourceRoleV1,
    ExecutableGttV1,
    GttGpuAccessibleExecutableV1,
>;

struct QueueResourceAuthorityV1 {
    ring: RingAuthority,
    control: ControlAuthority,
    eop: EopAuthority,
    context_save: ContextSaveAuthority,
    view: NativeQueueResourceViewV1,
}

struct LinuxNativeQueueBackendV1 {
    session: SharedGttMemorySessionV1,
    foundation: Option<QueueModelFoundationV1>,
    foundation_in_engine: bool,
}

impl NativeQueueBackendV1 for LinuxNativeQueueBackendV1 {
    type ResourceAuthority = QueueResourceAuthorityV1;

    fn opener_pid(&self) -> u32 {
        self.session.opener_pid()
    }

    fn take_model_foundation(
        &mut self,
    ) -> Result<QueueModelFoundationV1, NativeQueueAdapterErrorV1> {
        let foundation =
            self.foundation
                .take()
                .ok_or(NativeQueueAdapterErrorV1::InvalidResource(
                    "queue model ownership",
                ))?;
        self.foundation_in_engine = true;
        Ok(foundation)
    }

    fn resource_view(
        &self,
        authority: &Self::ResourceAuthority,
    ) -> Result<NativeQueueResourceViewV1, NativeQueueAdapterErrorV1> {
        validate_resource_authority(authority)?;
        Ok(authority.view)
    }

    fn check_currentness(&mut self) -> Result<(), &'static str> {
        self.session
            .check_queue_currentness()
            .map_err(|_| "shared GTT/device currentness")
    }

    fn create(
        &mut self,
        mut args: fe2o3_kfd_uapi::KfdIoctlCreateQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlCreateQueueArgs> {
        let status = match crate::queue_linux::create_queue(self.session.kfd_fd(), &mut args) {
            Ok(()) => fe2o3_runtime_model::QueueSyscallStatusV1::Succeeded,
            Err(_) => fe2o3_runtime_model::QueueSyscallStatusV1::Indeterminate,
        };
        QueueKernelOutcomeV1 {
            value: args,
            status,
        }
    }

    fn update(
        &mut self,
        args: fe2o3_kfd_uapi::KfdIoctlUpdateQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlUpdateQueueArgs> {
        let status = match crate::queue_linux::update_queue(self.session.kfd_fd(), &args) {
            Ok(()) => fe2o3_runtime_model::QueueSyscallStatusV1::Succeeded,
            Err(_) => fe2o3_runtime_model::QueueSyscallStatusV1::Indeterminate,
        };
        QueueKernelOutcomeV1 {
            value: args,
            status,
        }
    }

    fn destroy(
        &mut self,
        mut args: fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs> {
        let status = match crate::queue_linux::destroy_queue(self.session.kfd_fd(), &mut args) {
            Ok(()) => fe2o3_runtime_model::QueueSyscallStatusV1::Succeeded,
            Err(_) => fe2o3_runtime_model::QueueSyscallStatusV1::Indeterminate,
        };
        QueueKernelOutcomeV1 {
            value: args,
            status,
        }
    }
}

/// Redacted observation of one confirmed live queue and mapped doorbell slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComputeAqlQueueObservationV1 {
    queue_id: u32,
    ring_bytes: u32,
    doorbell_slice_bytes: usize,
    doorbell_byte_offset: u64,
}

impl ComputeAqlQueueObservationV1 {
    /// Process-local KFD observation, not queue authority.
    pub const fn queue_id(self) -> u32 {
        self.queue_id
    }
    pub const fn ring_bytes(self) -> u32 {
        self.ring_bytes
    }
    pub const fn doorbell_slice_bytes(self) -> usize {
        self.doorbell_slice_bytes
    }
    /// Relative offset within the owned process slice, never a CPU/GPU address.
    pub const fn doorbell_byte_offset(self) -> u64 {
        self.doorbell_byte_offset
    }
}

/// Evidence returned only after confirmed DESTROY and explicit resource return.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComputeAqlQueueDestroyedV1 {
    queue_id: u32,
    released_resources: u8,
}

impl ComputeAqlQueueDestroyedV1 {
    pub const fn queue_id(self) -> u32 {
        self.queue_id
    }
    pub const fn released_resources(self) -> u8 {
        self.released_resources
    }
}

#[derive(Debug)]
pub enum ComputeAqlQueueSessionErrorV1 {
    Planning(Gfx942QueueResourcePlanningError),
    Memory(MemorySessionError),
    Contract(&'static str),
    Native(&'static str),
    Doorbell(String),
}

impl fmt::Display for ComputeAqlQueueSessionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ComputeAqlQueueSessionErrorV1 {}

impl From<Gfx942QueueResourcePlanningError> for ComputeAqlQueueSessionErrorV1 {
    fn from(value: Gfx942QueueResourcePlanningError) -> Self {
        Self::Planning(value)
    }
}

impl From<MemorySessionError> for ComputeAqlQueueSessionErrorV1 {
    fn from(value: MemorySessionError) -> Self {
        Self::Memory(value)
    }
}

impl From<LinuxDoorbellErrorV1> for ComputeAqlQueueSessionErrorV1 {
    fn from(value: LinuxDoorbellErrorV1) -> Self {
        Self::Doorbell(value.to_string())
    }
}

#[must_use = "queue destruction and resource return are explicit"]
pub struct ComputeAqlQueueSessionV1 {
    engine: Option<NativeQueueEngineV1<LinuxNativeQueueBackendV1>>,
    key: QueueKeyV1,
    doorbell: Option<LinuxDoorbellSliceV1>,
    observation: ComputeAqlQueueObservationV1,
}

impl fmt::Debug for ComputeAqlQueueSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ComputeAqlQueueSessionV1")
            .field("observation", &self.observation)
            .finish_non_exhaustive()
    }
}

impl CheckedGfx942XnackMinusDevice {
    /// Allocates exact fe2o3 GTT roles, creates one queue, and maps its complete
    /// doorbell slice. This API deliberately exposes no MMIO or packet store.
    pub fn create_compute_aql_queue(
        self,
        ring_bytes: u32,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        let geometry = plan_gfx942_aql_queue_resources(
            self.topology_snapshot(),
            self.observation().unique_id(),
            ring_bytes,
        )?;
        let mut memory = self.acquire_shared_gtt_memory_session()?;

        let mut ring =
            memory
                .allocate_aql_queue(usize::try_from(ring_bytes).map_err(|_| {
                    ComputeAqlQueueSessionErrorV1::Contract("ring size conversion")
                })?)?;
        let mut control = memory.allocate_host_visible_coherent(CONTROL_BYTES)?;
        let mut eop = memory.allocate_executable(
            usize::try_from(geometry.end_of_pipe().mapping_bytes())
                .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("EOP size conversion"))?,
        )?;
        let mut context_save = memory.allocate_executable(
            usize::try_from(geometry.context_save().mapping_bytes()).map_err(|_| {
                ComputeAqlQueueSessionErrorV1::Contract("context-save size conversion")
            })?,
        )?;
        memory.with_bytes_mut(&mut ring, |bytes| bytes.fill(0))?;
        memory.with_bytes_mut(&mut control, |bytes| bytes.fill(0))?;
        memory.with_bytes_mut(&mut eop, |bytes| bytes.fill(0))?;
        memory.with_bytes_mut(&mut context_save, |bytes| bytes.fill(0))?;
        let eop = memory.seal_executable(eop)?;
        let context_save = memory.seal_executable(context_save)?;
        let ring = memory.map_to_gpu(ring)?;
        let control = memory.map_to_gpu(control)?;
        let eop = memory.map_executable_to_gpu(eop)?;
        let context_save = memory.map_executable_to_gpu(context_save)?;
        let ring = memory.retain_aql_ring_resource(ring)?;
        let control = memory.retain_aql_control_resource(control)?;
        let eop = memory.retain_aql_eop_resource(eop)?;
        let context_save = memory.retain_aql_context_save_resource(context_save)?;
        let authority = build_resource_authority(
            memory.queue_model_device(),
            geometry,
            ring,
            control,
            eop,
            context_save,
        )?;
        let (identity, model) = memory.take_queue_model_foundation()?;
        let backend = LinuxNativeQueueBackendV1 {
            session: memory,
            foundation: Some(QueueModelFoundationV1 {
                identity,
                memory: model,
            }),
            foundation_in_engine: false,
        };
        let mut engine = NativeQueueEngineV1::new(backend).map_err(map_native)?;
        let key = engine.admit(authority).map_err(map_native)?;
        engine.create(key).map_err(map_native)?;
        let outputs = engine
            .create_outputs(key)
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing CREATE outputs",
            ))?;
        let queue_id = engine
            .native_queue_id(key)
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract("missing queue id"))?;
        let mut session = ComputeAqlQueueSessionV1 {
            engine: Some(engine),
            key,
            doorbell: None,
            observation: ComputeAqlQueueObservationV1 {
                queue_id,
                ring_bytes,
                doorbell_slice_bytes: 0,
                doorbell_byte_offset: 0,
            },
        };
        session.check_currentness()?;
        let doorbell = {
            let engine = session.engine.as_ref().expect("session engine");
            LinuxDoorbellSliceV1::map(engine.backend.session.kfd_fd(), outputs, engine.opener_pid)?
        };
        session.observation.doorbell_slice_bytes = doorbell.slice_bytes();
        session.observation.doorbell_byte_offset = doorbell.queue_byte_offset();
        session.doorbell = Some(doorbell);
        session.check_currentness()?;
        Ok(session)
    }
}

impl ComputeAqlQueueSessionV1 {
    pub const fn observation(&self) -> ComputeAqlQueueObservationV1 {
        self.observation
    }

    #[cfg(feature = "live-validation")]
    pub fn verify_doorbell_dontfork(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.check_currentness()?;
        self.doorbell
            .as_ref()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract("missing doorbell"))?
            .verify_dontfork_child_negative()?;
        self.check_currentness()
    }

    pub fn destroy(mut self) -> Result<ComputeAqlQueueDestroyedV1, ComputeAqlQueueSessionErrorV1> {
        let engine = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        engine.destroy(self.key).map_err(map_native)?;
        self.doorbell
            .take()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract("missing doorbell"))?
            .release()?;
        self.check_currentness()?;
        let authority = self
            .engine
            .as_mut()
            .expect("session engine")
            .release_destroyed_resources(self.key)
            .map_err(map_native)?;
        self.restore_model_ownership()?;
        release_resource_authority(
            &mut self
                .engine
                .as_mut()
                .expect("session engine")
                .backend
                .session,
            authority,
        )?;
        Ok(ComputeAqlQueueDestroyedV1 {
            queue_id: self.observation.queue_id,
            released_resources: 4,
        })
    }

    fn check_currentness(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let engine = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        engine.prepare_operation().map_err(map_native)
    }

    fn restore_model_ownership(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let engine = self
            .engine
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing queue engine",
            ))?;
        if !engine.backend.foundation_in_engine {
            return Ok(());
        }
        let domain = engine.identity.domain_id();
        let identity = core::mem::replace(&mut engine.identity, DeviceIdentityStateV1::new(domain));
        let memory = core::mem::replace(&mut engine.memory, MemoryLifecycleStateV1::new(domain));
        engine
            .backend
            .session
            .restore_queue_model_foundation(identity, memory)?;
        engine.backend.foundation_in_engine = false;
        Ok(())
    }
}

impl Drop for ComputeAqlQueueSessionV1 {
    fn drop(&mut self) {
        // Model ownership can be restored without native effects. There is
        // deliberately no ioctl, MMIO store, munmap, GPU unmap, or FREE here.
        let _ = self.restore_model_ownership();
    }
}

fn build_resource_authority(
    current_device: fe2o3_runtime_model::ModelDeviceAdmissionV1,
    geometry: Gfx942AqlQueueResourcePlanV1,
    ring: RingAuthority,
    control: ControlAuthority,
    eop: EopAuthority,
    context_save: ContextSaveAuthority,
) -> Result<QueueResourceAuthorityV1, ComputeAqlQueueSessionErrorV1> {
    let rf = ring.facts();
    let cf = control.facts();
    let ef = eop.facts();
    let sf = context_save.facts();
    let vm = rf.mapping().allocation.vm;
    if [
        cf.mapping().allocation.vm,
        ef.mapping().allocation.vm,
        sf.mapping().allocation.vm,
    ]
    .iter()
    .any(|other| *other != vm)
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "queue resource VM substitution",
        ));
    }
    let ring_base = rf
        .checked_gpu_subrange(0, u64::from(geometry.ring().mapping_bytes()) * 2, 4096)
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract("ring geometry"))?;
    if rf.logical_bytes() != geometry.ring().mapping_bytes() as usize
        || rf.gpu_va_bytes() != u64::from(geometry.ring().mapping_bytes()) * 2
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract("ring size/profile"));
    }
    if cf.logical_bytes() != CONTROL_BYTES || cf.gpu_va_bytes() != CONTROL_BYTES as u64 {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "control size/profile",
        ));
    }
    // KFD truncates each pointer to its GPU page and requires that page to be
    // one exact PAGE_SIZE GPUVM mapping. Both distinct counters live within
    // that single reviewed page.
    let (write_pointer, read_pointer) = cf
        .checked_disjoint_gpu_subranges((0, 8, 8), (8, 8, 8))
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract("control subranges"))?;
    let eop_base = ef
        .checked_gpu_subrange(0, geometry.end_of_pipe().mapping_bytes(), 4096)
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract("EOP geometry"))?;
    if ef.logical_bytes() as u64 != geometry.end_of_pipe().mapping_bytes()
        || ef.gpu_va_bytes() != geometry.end_of_pipe().mapping_bytes()
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract("EOP size/profile"));
    }
    let context_base = sf
        .checked_gpu_subrange(0, geometry.context_save().mapping_bytes(), 4096)
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
            "context-save geometry",
        ))?;
    if sf.logical_bytes() as u64 != geometry.context_save().mapping_bytes()
        || sf.gpu_va_bytes() != geometry.context_save().mapping_bytes()
    {
        return Err(ComputeAqlQueueSessionErrorV1::Contract(
            "context-save size/profile",
        ));
    }
    // CREATE_QUEUE fields are per-XCC. The retained CWSR BO covers the
    // driver's independently checked aggregate across all XCCs.
    let ctl_stack_size = geometry.context_save().control_stack_bytes_per_xcc();
    let queue_number = NEXT_QUEUE_INSTANCE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("queue identity exhausted"))?;
    let queue = QueueKeyV1 {
        vm,
        id: QueueInstanceIdV1(queue_number),
        generation: QueueGenerationV1(1),
    };
    let plan_id = QueuePlanIdV1::from_untrusted_digest(digest_id(
        b"plan",
        queue,
        &[rf.mapping(), cf.mapping(), ef.mapping(), sf.mapping()],
    ));
    let configuration = QueueConfigurationIdV1::from_untrusted_digest(digest_id(
        b"configuration",
        queue,
        &[rf.mapping(), cf.mapping(), ef.mapping(), sf.mapping()],
    ));
    let binding = |facts: &crate::shared_memory::SharedGttMappedResourceFactsV1, kind| {
        ComputeAqlResourceBindingV1 {
            mapping: facts.mapping(),
            publication: facts.publication(),
            expected_kind: kind,
            expected_coherence: MemoryCoherenceV1::HostCoherent,
            expected_access: MemoryAccessV1::ReadWrite,
        }
    };
    let plan = ComputeAqlQueuePlanV1 {
        schema_version: fe2o3_runtime_model::QUEUE_LIFECYCLE_SCHEMA_VERSION_V1,
        target: ComputeAqlTargetProfileV1::Gfx942XnackMinusSpxNps1Kfd1_18,
        domain_id: current_device.domain_id(),
        plan_id,
        current_device,
        queue,
        initial_configuration: configuration,
        resources: ComputeAqlQueueResourcesV1 {
            ring: binding(rf, MemoryKindV1::QueueStorage),
            control: binding(cf, MemoryKindV1::HostVisibleCoherent),
            eop: binding(ef, MemoryKindV1::Executable),
            context_save: binding(sf, MemoryKindV1::Executable),
        },
    };
    let view = NativeQueueResourceViewV1 {
        plan,
        buffers: KfdAqlComputeQueueBuffers {
            ring_base_address: ring_base,
            write_pointer_address: write_pointer,
            read_pointer_address: read_pointer,
            eop_buffer_address: eop_base,
            eop_buffer_size: geometry.end_of_pipe().mapping_bytes(),
            ctx_save_restore_address: context_base,
            ctx_save_restore_size: geometry.context_save().context_save_bytes_per_xcc(),
            ctl_stack_size,
        },
        ring_size: admit_kfd_aql_queue_ring_size(geometry.ring().mapping_bytes())
            .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("ring UAPI size"))?,
        initial_percentage: admit_kfd_queue_percentage(100)
            .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("queue percentage"))?,
        priority: admit_kfd_queue_priority(0)
            .map_err(|_| ComputeAqlQueueSessionErrorV1::Contract("queue priority"))?,
    };
    let authority = QueueResourceAuthorityV1 {
        ring,
        control,
        eop,
        context_save,
        view,
    };
    validate_resource_authority(&authority).map_err(map_native)?;
    Ok(authority)
}

fn validate_resource_authority(
    authority: &QueueResourceAuthorityV1,
) -> Result<(), NativeQueueAdapterErrorV1> {
    let view = authority.view;
    let facts = [
        authority.ring.facts(),
        authority.control.facts(),
        authority.eop.facts(),
        authority.context_save.facts(),
    ];
    for (binding, facts) in view
        .plan
        .resources
        .ordered()
        .iter()
        .map(|(_, binding)| binding)
        .zip(facts)
    {
        if binding.mapping != facts.mapping() || binding.publication != facts.publication() {
            return Err(NativeQueueAdapterErrorV1::InvalidResource(
                "queue authority substitution",
            ));
        }
    }
    Ok(())
}

fn release_resource_authority(
    memory: &mut SharedGttMemorySessionV1,
    authority: QueueResourceAuthorityV1,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    let ring = memory.unmap_from_gpu(authority.ring.into_token())?;
    let control = memory.unmap_from_gpu(authority.control.into_token())?;
    let eop = memory.unmap_executable_from_gpu(authority.eop.into_token())?;
    let context_save = memory.unmap_executable_from_gpu(authority.context_save.into_token())?;
    memory.release(ring)?;
    memory.release(control)?;
    memory.release_executable(eop)?;
    memory.release_executable(context_save)?;
    Ok(())
}

fn digest_id(
    tag: &[u8],
    queue: QueueKeyV1,
    mappings: &[fe2o3_runtime_model::MemoryMappingKeyV1; 4],
) -> IdentityDigestV1 {
    let mut hasher = Sha256::new();
    hasher.update(GFX942_QUEUE_RESOURCE_PROFILE_SHA256_V1.as_bytes());
    hasher.update(SHARED_GTT_MEMORY_PROFILE_SHA256_V1.as_bytes());
    hasher.update(tag);
    hasher.update(queue.id.0.to_le_bytes());
    hasher.update(queue.generation.0.to_le_bytes());
    for mapping in mappings {
        hasher.update(mapping.allocation.vm.id.0.to_le_bytes());
        hasher.update(mapping.allocation.id.0.to_le_bytes());
        hasher.update(mapping.allocation.generation.0.to_le_bytes());
        hasher.update(mapping.id.0.to_le_bytes());
    }
    IdentityDigestV1::from_untrusted_bytes(hasher.finalize().into())
}

fn map_native(error: NativeQueueAdapterErrorV1) -> ComputeAqlQueueSessionErrorV1 {
    let detail = match error {
        NativeQueueAdapterErrorV1::ProcessChanged => "queue process changed",
        NativeQueueAdapterErrorV1::Currentness(_) => "queue currentness lost",
        NativeQueueAdapterErrorV1::InvalidResource(_) => "invalid queue resource",
        NativeQueueAdapterErrorV1::InvalidPhase => "invalid queue phase",
        NativeQueueAdapterErrorV1::JournalCapacity => "queue journal capacity",
        NativeQueueAdapterErrorV1::BackendFailedNoEffect(_) => {
            "queue syscall failed with no effect"
        }
        NativeQueueAdapterErrorV1::BackendIndeterminate(_) => "queue syscall result indeterminate",
        NativeQueueAdapterErrorV1::MalformedKernelResult(_, _) => "malformed queue kernel result",
        NativeQueueAdapterErrorV1::ModelProjection => "queue model projection",
        NativeQueueAdapterErrorV1::AuthorityPoisoned => "queue authority poisoned",
    };
    ComputeAqlQueueSessionErrorV1::Native(detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_manifest_digest_is_frozen() {
        let digest = Sha256::digest(GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1);
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(rendered, GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1);
    }
}
