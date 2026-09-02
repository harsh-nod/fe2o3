#![forbid(unsafe_code)]

//! Bounded, authority-free virtual GPU lifecycle over admitted Kernel IR.
//!
//! This crate performs deterministic CPU semantic simulation. Its device, VM,
//! mappings, queues, dispatches, and completions are model state only. No value
//! produced here grants compiler, artifact, load, launch, KFD, hardware,
//! equivalence, performance, or universal-correctness authority.

use std::error::Error;
use std::fmt;

use fe2o3_kernel_ir::{AccessMode, KernelId, ScalarType, VerifiedCanonicalKernelIrIdentityV7};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    EventPolicyV1, ScalarBitsV1, SharedBufferV1, SimulationArgumentV1,
    SimulationConflictAssessmentV1, SimulationErrorV1, SimulationLimitsV1,
    SimulationRaceAssessmentV1, SimulationRequestV1, SimulationScheduleIdentityV1,
    SimulationScheduleRequestV1, SimulationTargetV1,
};
use fe2o3_runtime_model::{
    AQL_PACKET_BYTES_V1, AllocationIdV1, AllocationKeyV1, CodeLoadPlanIdV1, CompletionIdV1,
    CompletionKeyV1, DeviceGenerationV1, DeviceKeyV1, DispatchIdV1, DispatchKeyV1,
    DispatchResourceV1, IdentityDigestV1, LoadedCodeIdV1, LoadedCodeKeyV1, MAX_ALLOCATIONS_V1,
    MAX_DISPATCHES_V1, MAX_LOADED_CODE_V1, MAX_QUEUE_CAPACITY_V1, MAX_QUEUES_V1, MappingIdV1,
    MappingKeyV1, MemoryAccessV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1,
    QueueKeyV1, QueuePlanIdV1, RuntimeArtifactIdV1, RuntimeStateV1, RuntimeTransitionV1,
    TransitionErrorV1, VmIdV1, VmKeyV1,
};

pub const VIRTUAL_RUNTIME_SCHEMA_V1: &str = "fe2o3-virtual-runtime-v1";
pub const VIRTUAL_RUNTIME_OUTCOME_SCHEMA_V1: &str = "fe2o3-virtual-runtime-outcome-v1";

const FIRST_USER_ALLOCATION_ID: u64 = 1_024;
const FIRST_SYNTHETIC_VA: u64 = 0x1_0000;
const VA_ALIGNMENT: u64 = 0x1_0000;

/// Exact semantic target profile selected for this virtual runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualTargetProfileV1 {
    /// Target-neutral KIR using the AMDGPU-compatible 64-bit scalar layout.
    Amdgpu64TargetNeutral,
    /// Exact `gfx942:xnack-` KIR target, simulated with the 64-bit layout.
    Gfx942XnackMinus,
    /// Exact `gfx950:xnack-` KIR target, simulated with the 64-bit layout.
    Gfx950XnackMinus,
}

impl VirtualTargetProfileV1 {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Amdgpu64TargetNeutral => "amdgpu64-target-neutral",
            Self::Gfx942XnackMinus => "gfx942:xnack-",
            Self::Gfx950XnackMinus => "gfx950:xnack-",
        }
    }

    pub const fn simulation_target(self) -> SimulationTargetV1 {
        SimulationTargetV1::amdgpu_64()
    }
}

/// Hard-bounded storage and scheduling policy for one virtual runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualRuntimeLimitsV1 {
    pub max_user_allocations: usize,
    pub max_total_user_bytes: usize,
    pub max_modules: usize,
    pub max_queues: usize,
    pub max_dispatches: usize,
    pub max_dependencies_per_dispatch: usize,
    pub max_schedule_decisions: usize,
}

impl Default for VirtualRuntimeLimitsV1 {
    fn default() -> Self {
        Self {
            max_user_allocations: 1_024,
            max_total_user_bytes: 1 << 30,
            max_modules: 64,
            max_queues: 64,
            max_dispatches: 8_192,
            max_dependencies_per_dispatch: 256,
            max_schedule_decisions: 1 << 20,
        }
    }
}

impl VirtualRuntimeLimitsV1 {
    fn validate(self) -> Result<Self, VirtualRuntimeErrorV1> {
        let fields = [
            ("max_user_allocations", self.max_user_allocations),
            ("max_total_user_bytes", self.max_total_user_bytes),
            ("max_modules", self.max_modules),
            ("max_queues", self.max_queues),
            ("max_dispatches", self.max_dispatches),
            (
                "max_dependencies_per_dispatch",
                self.max_dependencies_per_dispatch,
            ),
            ("max_schedule_decisions", self.max_schedule_decisions),
        ];
        if let Some((field, _)) = fields.into_iter().find(|(_, value)| *value == 0) {
            return Err(VirtualRuntimeErrorV1::InvalidLimit(field));
        }
        if self.max_user_allocations > MAX_ALLOCATIONS_V1.saturating_sub(128) {
            return Err(VirtualRuntimeErrorV1::InvalidLimit("max_user_allocations"));
        }
        if self.max_modules > MAX_LOADED_CODE_V1 {
            return Err(VirtualRuntimeErrorV1::InvalidLimit("max_modules"));
        }
        if self.max_queues > MAX_QUEUES_V1 {
            return Err(VirtualRuntimeErrorV1::InvalidLimit("max_queues"));
        }
        if self.max_dispatches > MAX_DISPATCHES_V1 {
            return Err(VirtualRuntimeErrorV1::InvalidLimit("max_dispatches"));
        }
        Ok(self)
    }
}

/// Complete immutable configuration bound into a runtime instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualRuntimeConfigV1 {
    pub runtime_identity: IdentityDigestV1,
    pub target: VirtualTargetProfileV1,
    pub runtime_limits: VirtualRuntimeLimitsV1,
    pub simulation_limits: SimulationLimitsV1,
}

macro_rules! handle {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            runtime_identity: IdentityDigestV1,
            ordinal: u64,
        }

        impl $name {
            pub const fn runtime_identity(self) -> IdentityDigestV1 {
                self.runtime_identity
            }

            pub const fn ordinal(self) -> u64 {
                self.ordinal
            }
        }
    };
}

handle!(VirtualBufferHandleV1);
handle!(VirtualModuleHandleV1);
handle!(VirtualQueueHandleV1);
handle!(VirtualCompletionHandleV1);

/// Device permissions retained independently of host copy access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualBufferAccessV1 {
    ReadOnly,
    ReadWrite,
}

/// Borrowed exact bytes and initialization state for bounded result encoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualBufferSnapshotV1<'a> {
    pub bytes: &'a [u8],
    pub initialized: &'a [bool],
}

impl VirtualBufferAccessV1 {
    fn permits(self, requested: AccessMode) -> bool {
        matches!(
            (self, requested),
            (Self::ReadOnly, AccessMode::ReadOnly)
                | (Self::ReadWrite, AccessMode::ReadOnly)
                | (Self::ReadWrite, AccessMode::WriteOnly)
                | (Self::ReadWrite, AccessMode::ReadWrite)
        )
    }

    fn model_access(self) -> MemoryAccessV1 {
        match self {
            Self::ReadOnly => MemoryAccessV1::Read,
            Self::ReadWrite => MemoryAccessV1::ReadWrite,
        }
    }
}

/// A scalar or a typed view into one virtual allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VirtualArgumentV1 {
    Scalar(ScalarBitsV1),
    Buffer {
        buffer: VirtualBufferHandleV1,
        element: ScalarType,
        access: AccessMode,
        alignment: u32,
        byte_offset: usize,
        elements: usize,
    },
}

/// One prepared semantic CPU dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtualDispatchRequestV1 {
    pub kernel: KernelId,
    pub grid: [u64; 3],
    pub workgroup: [u32; 3],
    pub arguments: Vec<VirtualArgumentV1>,
    pub dependencies: Vec<VirtualCompletionHandleV1>,
}

/// Stable lifecycle state visible to callers and the headless CLI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualCompletionStateV1 {
    Prepared,
    Completed,
    AbortedDependency,
    AbortedSimulation,
    Ambiguous,
    FailedQuiescent,
}

/// Compact successful observation from one deterministic CPU execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtualSimulationSummaryV1 {
    pub kir_identity: VerifiedCanonicalKernelIrIdentityV7,
    pub target: VirtualTargetProfileV1,
    pub invocations_executed: u64,
    pub workgroups_visited: u64,
    pub scheduled_slots_visited: u64,
    pub steps_executed: u64,
    pub schedule: SimulationScheduleIdentityV1,
    pub schedule_transcript_identity: [u8; 32],
    pub schedule_decisions: u64,
    pub schedule_barrier_releases: u64,
    pub conflict_state: VirtualConflictStateV1,
    pub race_state: VirtualRaceStateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualConflictStateV1 {
    NoneObserved,
    Observed,
    Incomplete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtualRaceStateV1 {
    NoneObserved,
    Observed,
    Incomplete,
}

/// One scheduler action. `Idle` is returned when no prepared dispatch exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VirtualRunProgressV1 {
    Completed {
        completion: VirtualCompletionHandleV1,
        summary: VirtualSimulationSummaryV1,
    },
    AbortedDependency {
        completion: VirtualCompletionHandleV1,
        dependency: VirtualCompletionHandleV1,
    },
    Blocked,
    Idle,
}

/// Typed virtual-runtime misuse or semantic execution failure.
#[derive(Debug)]
pub enum VirtualRuntimeErrorV1 {
    InvalidRuntimeIdentity,
    InvalidLimit(&'static str),
    CapacityExceeded(&'static str),
    ByteLengthOverflow,
    InvalidBufferRange,
    UninitializedHostRead {
        offset: usize,
    },
    ForeignHandle {
        kind: &'static str,
    },
    UnknownHandle {
        kind: &'static str,
        ordinal: u64,
    },
    ReleasedHandle {
        kind: &'static str,
        ordinal: u64,
    },
    InvalidBufferAccess {
        ordinal: u64,
    },
    DuplicateDependency {
        ordinal: u64,
    },
    ForwardDependency {
        ordinal: u64,
    },
    CompletionNotPrepared {
        ordinal: u64,
    },
    CompletionNotAmbiguous {
        ordinal: u64,
    },
    QueueNotQuiescent {
        ordinal: u64,
    },
    ExactTargetMismatch {
        module_target: String,
        runtime_target: &'static str,
    },
    Model(TransitionErrorV1),
    Simulation {
        completion: VirtualCompletionHandleV1,
        source: Box<SimulationErrorV1>,
    },
    SimulatorBuffer(String),
}

impl fmt::Display for VirtualRuntimeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRuntimeIdentity => formatter.write_str("runtime identity must be nonzero"),
            Self::InvalidLimit(field) => write!(formatter, "invalid virtual runtime limit {field}"),
            Self::CapacityExceeded(kind) => {
                write!(formatter, "virtual runtime {kind} limit reached")
            }
            Self::ByteLengthOverflow => {
                formatter.write_str("virtual buffer byte length overflowed")
            }
            Self::InvalidBufferRange => formatter.write_str("virtual buffer range is invalid"),
            Self::UninitializedHostRead { offset } => {
                write!(
                    formatter,
                    "host read reached uninitialized virtual byte {offset}"
                )
            }
            Self::ForeignHandle { kind } => {
                write!(formatter, "{kind} handle belongs to another runtime")
            }
            Self::UnknownHandle { kind, ordinal } => {
                write!(formatter, "unknown {kind} handle {ordinal}")
            }
            Self::ReleasedHandle { kind, ordinal } => {
                write!(formatter, "released {kind} handle {ordinal}")
            }
            Self::InvalidBufferAccess { ordinal } => write!(
                formatter,
                "buffer {ordinal} does not permit requested device access"
            ),
            Self::DuplicateDependency { ordinal } => {
                write!(formatter, "completion {ordinal} is a duplicate dependency")
            }
            Self::ForwardDependency { ordinal } => {
                write!(formatter, "completion {ordinal} is not an earlier dispatch")
            }
            Self::CompletionNotPrepared { ordinal } => {
                write!(formatter, "completion {ordinal} is not prepared")
            }
            Self::CompletionNotAmbiguous { ordinal } => {
                write!(formatter, "completion {ordinal} is not ambiguous")
            }
            Self::QueueNotQuiescent { ordinal } => {
                write!(formatter, "queue {ordinal} is not quiescent")
            }
            Self::ExactTargetMismatch {
                module_target,
                runtime_target,
            } => write!(
                formatter,
                "module exact target {module_target} does not match virtual runtime target {runtime_target}"
            ),
            Self::Model(error) => write!(
                formatter,
                "runtime lifecycle model rejected transition: {error:?}"
            ),
            Self::Simulation { completion, source } => write!(
                formatter,
                "virtual completion {} failed semantic simulation: {source}",
                completion.ordinal
            ),
            Self::SimulatorBuffer(detail) => {
                write!(formatter, "could not construct simulator buffer: {detail}")
            }
        }
    }
}

impl Error for VirtualRuntimeErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Simulation { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<TransitionErrorV1> for VirtualRuntimeErrorV1 {
    fn from(value: TransitionErrorV1) -> Self {
        Self::Model(value)
    }
}

struct BufferRecordV1 {
    handle: VirtualBufferHandleV1,
    allocation: AllocationKeyV1,
    mapping: MappingKeyV1,
    access: VirtualBufferAccessV1,
    bytes: Vec<u8>,
    initialized: Vec<bool>,
    released: bool,
}

struct ModuleRecordV1 {
    handle: VirtualModuleHandleV1,
    allocation: AllocationKeyV1,
    mapping: MappingKeyV1,
    code: LoadedCodeKeyV1,
    module: AdmittedSimulationModuleV1,
    released: bool,
}

struct QueueRecordV1 {
    handle: VirtualQueueHandleV1,
    allocation: AllocationKeyV1,
    mapping: MappingKeyV1,
    queue: QueueKeyV1,
    quiescent: bool,
    released: bool,
}

struct DispatchRecordLocalV1 {
    completion: VirtualCompletionHandleV1,
    model_completion: CompletionKeyV1,
    queue: VirtualQueueHandleV1,
    module: VirtualModuleHandleV1,
    request: VirtualDispatchRequestV1,
    state: VirtualCompletionStateV1,
    summary: Option<VirtualSimulationSummaryV1>,
}

/// Persistent, bounded virtual runtime. It has no device or syscall authority.
pub struct VirtualRuntimeV1 {
    config: VirtualRuntimeConfigV1,
    model: RuntimeStateV1,
    vm: VmKeyV1,
    next_ordinal: u64,
    next_allocation_id: u64,
    next_va: u64,
    total_user_bytes: usize,
    buffers: Vec<BufferRecordV1>,
    modules: Vec<ModuleRecordV1>,
    queues: Vec<QueueRecordV1>,
    dispatches: Vec<DispatchRecordLocalV1>,
}

impl VirtualRuntimeV1 {
    pub fn new(config: VirtualRuntimeConfigV1) -> Result<Self, VirtualRuntimeErrorV1> {
        if config.runtime_identity.as_bytes() == &[0; 32] {
            return Err(VirtualRuntimeErrorV1::InvalidRuntimeIdentity);
        }
        let config = VirtualRuntimeConfigV1 {
            runtime_limits: config.runtime_limits.validate()?,
            simulation_limits: config
                .simulation_limits
                .validate()
                .map_err(|_| VirtualRuntimeErrorV1::InvalidLimit("simulation_limits"))?,
            ..config
        };
        let device = DeviceKeyV1 {
            physical: PhysicalDeviceIdV1(0),
            generation: DeviceGenerationV1(1),
        };
        let vm = VmKeyV1 {
            device,
            id: VmIdV1(1),
        };
        let model = RuntimeStateV1::new()
            .next(RuntimeTransitionV1::AddDevice { key: device })?
            .next(RuntimeTransitionV1::CreateVm { key: vm })?;
        Ok(Self {
            config,
            model,
            vm,
            next_ordinal: 1,
            next_allocation_id: FIRST_USER_ALLOCATION_ID,
            next_va: FIRST_SYNTHETIC_VA,
            total_user_bytes: 0,
            buffers: Vec::new(),
            modules: Vec::new(),
            queues: Vec::new(),
            dispatches: Vec::new(),
        })
    }

    pub const fn config(&self) -> VirtualRuntimeConfigV1 {
        self.config
    }

    pub const fn grants_hardware_authority(&self) -> bool {
        false
    }

    pub const fn predicts_performance(&self) -> bool {
        false
    }

    pub fn allocate_buffer(
        &mut self,
        byte_len: usize,
        access: VirtualBufferAccessV1,
    ) -> Result<VirtualBufferHandleV1, VirtualRuntimeErrorV1> {
        if byte_len == 0 {
            return Err(VirtualRuntimeErrorV1::InvalidBufferRange);
        }
        if self.buffers.len() >= self.config.runtime_limits.max_user_allocations {
            return Err(VirtualRuntimeErrorV1::CapacityExceeded("allocation"));
        }
        let total = self
            .total_user_bytes
            .checked_add(byte_len)
            .ok_or(VirtualRuntimeErrorV1::ByteLengthOverflow)?;
        if total > self.config.runtime_limits.max_total_user_bytes {
            return Err(VirtualRuntimeErrorV1::CapacityExceeded("allocation bytes"));
        }
        let handle = self.new_buffer_handle();
        let byte_len_u64 =
            u64::try_from(byte_len).map_err(|_| VirtualRuntimeErrorV1::ByteLengthOverflow)?;
        let (allocation, mapping, next_model, next_va) =
            self.provision_mapping(byte_len_u64, access.model_access(), self.next_allocation_id)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(byte_len)
            .map_err(|_| VirtualRuntimeErrorV1::CapacityExceeded("allocation bytes"))?;
        bytes.resize(byte_len, 0);
        let mut initialized = Vec::new();
        initialized
            .try_reserve_exact(byte_len)
            .map_err(|_| VirtualRuntimeErrorV1::CapacityExceeded("initialization bytes"))?;
        initialized.resize(byte_len, false);
        self.model = next_model;
        self.next_va = next_va;
        self.next_allocation_id += 1;
        self.total_user_bytes = total;
        self.buffers.push(BufferRecordV1 {
            handle,
            allocation,
            mapping,
            access,
            bytes,
            initialized,
            released: false,
        });
        Ok(handle)
    }

    pub fn copy_from_host(
        &mut self,
        handle: VirtualBufferHandleV1,
        offset: usize,
        source: &[u8],
    ) -> Result<(), VirtualRuntimeErrorV1> {
        let record = self.buffer_mut(handle)?;
        let end = offset
            .checked_add(source.len())
            .ok_or(VirtualRuntimeErrorV1::InvalidBufferRange)?;
        let destination = record
            .bytes
            .get_mut(offset..end)
            .ok_or(VirtualRuntimeErrorV1::InvalidBufferRange)?;
        destination.copy_from_slice(source);
        record.initialized[offset..end].fill(true);
        Ok(())
    }

    /// Copies exact host bytes plus byte-granular initialization state.
    pub fn copy_from_host_with_initialization(
        &mut self,
        handle: VirtualBufferHandleV1,
        offset: usize,
        source: &[u8],
        initialized: &[bool],
    ) -> Result<(), VirtualRuntimeErrorV1> {
        if source.len() != initialized.len() {
            return Err(VirtualRuntimeErrorV1::InvalidBufferRange);
        }
        let record = self.buffer_mut(handle)?;
        let end = offset
            .checked_add(source.len())
            .ok_or(VirtualRuntimeErrorV1::InvalidBufferRange)?;
        let destination = record
            .bytes
            .get_mut(offset..end)
            .ok_or(VirtualRuntimeErrorV1::InvalidBufferRange)?;
        destination.copy_from_slice(source);
        record.initialized[offset..end].copy_from_slice(initialized);
        Ok(())
    }

    pub fn copy_to_host(
        &self,
        handle: VirtualBufferHandleV1,
        offset: usize,
        destination: &mut [u8],
    ) -> Result<(), VirtualRuntimeErrorV1> {
        let record = self.buffer(handle)?;
        let end = offset
            .checked_add(destination.len())
            .ok_or(VirtualRuntimeErrorV1::InvalidBufferRange)?;
        let source = record
            .bytes
            .get(offset..end)
            .ok_or(VirtualRuntimeErrorV1::InvalidBufferRange)?;
        if let Some(relative) = record.initialized[offset..end]
            .iter()
            .position(|initialized| !initialized)
        {
            return Err(VirtualRuntimeErrorV1::UninitializedHostRead {
                offset: offset + relative,
            });
        }
        destination.copy_from_slice(source);
        Ok(())
    }

    pub fn buffer_snapshot(
        &self,
        handle: VirtualBufferHandleV1,
    ) -> Result<VirtualBufferSnapshotV1<'_>, VirtualRuntimeErrorV1> {
        let record = self.buffer(handle)?;
        Ok(VirtualBufferSnapshotV1 {
            bytes: &record.bytes,
            initialized: &record.initialized,
        })
    }

    pub fn release_buffer(
        &mut self,
        handle: VirtualBufferHandleV1,
    ) -> Result<(), VirtualRuntimeErrorV1> {
        let index = self.buffer_index(handle)?;
        if self.buffers[index].released {
            return Err(VirtualRuntimeErrorV1::ReleasedHandle {
                kind: "buffer",
                ordinal: handle.ordinal,
            });
        }
        let mapping = self.buffers[index].mapping;
        let allocation = self.buffers[index].allocation;
        let next = self
            .model
            .next(RuntimeTransitionV1::Unmap { key: mapping })?
            .next(RuntimeTransitionV1::ReleaseAllocation { key: allocation })?;
        self.model = next;
        self.total_user_bytes -= self.buffers[index].bytes.len();
        self.buffers[index].bytes.clear();
        self.buffers[index].initialized.clear();
        self.buffers[index].released = true;
        Ok(())
    }

    pub fn register_module(
        &mut self,
        module: AdmittedSimulationModuleV1,
    ) -> Result<VirtualModuleHandleV1, VirtualRuntimeErrorV1> {
        if self.modules.len() >= self.config.runtime_limits.max_modules {
            return Err(VirtualRuntimeErrorV1::CapacityExceeded("module"));
        }
        self.validate_module_target(&module)?;
        let handle = self.new_module_handle();
        let byte_len = module.identity().canonical_length().max(1);
        let allocation_id = self.next_allocation_id;
        let (allocation, mapping, next, next_va) =
            self.provision_mapping(byte_len, MemoryAccessV1::ReadExecute, allocation_id)?;
        let code = LoadedCodeKeyV1 {
            vm: self.vm,
            id: LoadedCodeIdV1(handle.ordinal),
        };
        let digest = IdentityDigestV1::from_untrusted_bytes(*module.identity().digest());
        let next = next.next(RuntimeTransitionV1::LoadCode {
            key: code,
            load_plan_id: CodeLoadPlanIdV1::from_untrusted_digest(digest),
            artifact_id: RuntimeArtifactIdV1::from_untrusted_digest(digest),
            executable_mapping: mapping,
            entry_offset: 0,
        })?;
        self.model = next;
        self.next_va = next_va;
        self.next_allocation_id += 1;
        self.modules.push(ModuleRecordV1 {
            handle,
            allocation,
            mapping,
            code,
            module,
            released: false,
        });
        Ok(handle)
    }

    pub fn release_module(
        &mut self,
        handle: VirtualModuleHandleV1,
    ) -> Result<(), VirtualRuntimeErrorV1> {
        let index = self.module_index(handle)?;
        if self.modules[index].released {
            return Err(VirtualRuntimeErrorV1::ReleasedHandle {
                kind: "module",
                ordinal: handle.ordinal,
            });
        }
        let record = &self.modules[index];
        let next = self
            .model
            .next(RuntimeTransitionV1::UnloadCode { key: record.code })?
            .next(RuntimeTransitionV1::Unmap {
                key: record.mapping,
            })?
            .next(RuntimeTransitionV1::ReleaseAllocation {
                key: record.allocation,
            })?;
        self.model = next;
        self.modules[index].released = true;
        Ok(())
    }

    pub fn create_queue(
        &mut self,
        capacity: u32,
    ) -> Result<VirtualQueueHandleV1, VirtualRuntimeErrorV1> {
        if self.queues.len() >= self.config.runtime_limits.max_queues {
            return Err(VirtualRuntimeErrorV1::CapacityExceeded("queue"));
        }
        if capacity == 0 || capacity > MAX_QUEUE_CAPACITY_V1 || !capacity.is_power_of_two() {
            return Err(VirtualRuntimeErrorV1::InvalidLimit("queue capacity"));
        }
        let handle = self.new_queue_handle();
        let ring_bytes = u64::from(capacity)
            .checked_mul(AQL_PACKET_BYTES_V1)
            .ok_or(VirtualRuntimeErrorV1::ByteLengthOverflow)?;
        let allocation_id = self.next_allocation_id;
        let (allocation, mapping, next, next_va) =
            self.provision_mapping(ring_bytes, MemoryAccessV1::ReadWrite, allocation_id)?;
        let queue = QueueKeyV1 {
            vm: self.vm,
            id: QueueInstanceIdV1(handle.ordinal),
            generation: QueueGenerationV1(1),
        };
        let next = next.next(RuntimeTransitionV1::CreateQueue {
            key: queue,
            plan_id: QueuePlanIdV1::from_untrusted_digest(self.config.runtime_identity),
            ring_mapping: mapping,
            capacity,
        })?;
        self.model = next;
        self.next_va = next_va;
        self.next_allocation_id += 1;
        self.queues.push(QueueRecordV1 {
            handle,
            allocation,
            mapping,
            queue,
            quiescent: false,
            released: false,
        });
        Ok(handle)
    }

    pub fn submit(
        &mut self,
        queue: VirtualQueueHandleV1,
        module: VirtualModuleHandleV1,
        request: VirtualDispatchRequestV1,
    ) -> Result<VirtualCompletionHandleV1, VirtualRuntimeErrorV1> {
        if self.dispatches.len() >= self.config.runtime_limits.max_dispatches {
            return Err(VirtualRuntimeErrorV1::CapacityExceeded("dispatch"));
        }
        let queue_index = self.queue_index(queue)?;
        let module_index = self.module_index(module)?;
        self.require_live_queue(queue_index)?;
        self.require_live_module(module_index)?;
        if request.dependencies.len() > self.config.runtime_limits.max_dependencies_per_dispatch {
            return Err(VirtualRuntimeErrorV1::CapacityExceeded(
                "dispatch dependency",
            ));
        }
        let future_ordinal = self.next_ordinal;
        let mut dependencies = request.dependencies.clone();
        dependencies.sort_unstable();
        for pair in dependencies.windows(2) {
            if pair[0] == pair[1] {
                return Err(VirtualRuntimeErrorV1::DuplicateDependency {
                    ordinal: pair[0].ordinal,
                });
            }
        }
        for dependency in &dependencies {
            self.validate_handle("completion", dependency.runtime_identity)?;
            if dependency.ordinal >= future_ordinal
                || !self
                    .dispatches
                    .iter()
                    .any(|record| record.completion == *dependency)
            {
                return Err(VirtualRuntimeErrorV1::ForwardDependency {
                    ordinal: dependency.ordinal,
                });
            }
        }
        let resources = self.validate_and_collect_resources(&request.arguments)?;
        let completion = self.new_completion_handle();
        let dispatch_key = DispatchKeyV1 {
            queue: self.queues[queue_index].queue,
            id: DispatchIdV1(completion.ordinal),
        };
        let model_completion = CompletionKeyV1 {
            dispatch: dispatch_key,
            id: CompletionIdV1(completion.ordinal),
        };
        let next = self.model.next(RuntimeTransitionV1::PrepareDispatch {
            key: dispatch_key,
            code: self.modules[module_index].code,
            completion: model_completion,
            resources,
        })?;
        self.model = next;
        self.dispatches.push(DispatchRecordLocalV1 {
            completion,
            model_completion,
            queue,
            module,
            request,
            state: VirtualCompletionStateV1::Prepared,
            summary: None,
        });
        Ok(completion)
    }

    pub fn release_queue(
        &mut self,
        handle: VirtualQueueHandleV1,
    ) -> Result<(), VirtualRuntimeErrorV1> {
        let index = self.queue_index(handle)?;
        if self.queues[index].released {
            return Err(VirtualRuntimeErrorV1::ReleasedHandle {
                kind: "queue",
                ordinal: handle.ordinal,
            });
        }
        let record = &self.queues[index];
        let next = self
            .model
            .next(RuntimeTransitionV1::ReleaseQueue { key: record.queue })?
            .next(RuntimeTransitionV1::Unmap {
                key: record.mapping,
            })?
            .next(RuntimeTransitionV1::ReleaseAllocation {
                key: record.allocation,
            })?;
        self.model = next;
        self.queues[index].released = true;
        Ok(())
    }

    pub fn run_next(&mut self) -> Result<VirtualRunProgressV1, VirtualRuntimeErrorV1> {
        let mut blocked = false;
        for index in 0..self.dispatches.len() {
            if self.dispatches[index].state != VirtualCompletionStateV1::Prepared {
                continue;
            }
            match self.dependency_readiness(index) {
                DependencyReadinessV1::Blocked => {
                    blocked = true;
                    continue;
                }
                DependencyReadinessV1::Failed(dependency) => {
                    let completion = self.dispatches[index].completion;
                    self.model = self.model.next(RuntimeTransitionV1::AbortPrepared {
                        completion: self.dispatches[index].model_completion,
                    })?;
                    self.dispatches[index].state = VirtualCompletionStateV1::AbortedDependency;
                    return Ok(VirtualRunProgressV1::AbortedDependency {
                        completion,
                        dependency,
                    });
                }
                DependencyReadinessV1::Ready => return self.execute_dispatch(index),
            }
        }
        Ok(if blocked {
            VirtualRunProgressV1::Blocked
        } else {
            VirtualRunProgressV1::Idle
        })
    }

    pub fn completion_state(
        &self,
        completion: VirtualCompletionHandleV1,
    ) -> Result<VirtualCompletionStateV1, VirtualRuntimeErrorV1> {
        Ok(self.dispatch(completion)?.state)
    }

    pub fn completion_summary(
        &self,
        completion: VirtualCompletionHandleV1,
    ) -> Result<Option<&VirtualSimulationSummaryV1>, VirtualRuntimeErrorV1> {
        Ok(self.dispatch(completion)?.summary.as_ref())
    }

    /// Injects the model's explicit publication-with-unknown-completion boundary.
    pub fn mark_completion_ambiguous(
        &mut self,
        completion: VirtualCompletionHandleV1,
    ) -> Result<(), VirtualRuntimeErrorV1> {
        let index = self.dispatch_index(completion)?;
        if self.dispatches[index].state != VirtualCompletionStateV1::Prepared {
            return Err(VirtualRuntimeErrorV1::CompletionNotPrepared {
                ordinal: completion.ordinal,
            });
        }
        let model_completion = self.dispatches[index].model_completion;
        self.model = self
            .model
            .next(RuntimeTransitionV1::PublishDispatch {
                completion: model_completion,
            })?
            .next(RuntimeTransitionV1::MarkDispatchAmbiguous {
                completion: model_completion,
            })?;
        self.dispatches[index].state = VirtualCompletionStateV1::Ambiguous;
        Ok(())
    }

    pub fn quiesce_queue(
        &mut self,
        queue: VirtualQueueHandleV1,
    ) -> Result<(), VirtualRuntimeErrorV1> {
        let index = self.queue_index(queue)?;
        self.require_live_queue(index)?;
        let key = self.queues[index].queue;
        self.model = self
            .model
            .next(RuntimeTransitionV1::BeginQueueFailure { key })?
            .next(RuntimeTransitionV1::EstablishQueueQuiescence { key })?;
        self.queues[index].quiescent = true;
        Ok(())
    }

    pub fn settle_ambiguous_completion(
        &mut self,
        completion: VirtualCompletionHandleV1,
    ) -> Result<(), VirtualRuntimeErrorV1> {
        let index = self.dispatch_index(completion)?;
        if self.dispatches[index].state != VirtualCompletionStateV1::Ambiguous {
            return Err(VirtualRuntimeErrorV1::CompletionNotAmbiguous {
                ordinal: completion.ordinal,
            });
        }
        let queue = self.dispatches[index].queue;
        let queue_index = self.queue_index(queue)?;
        if !self.queues[queue_index].quiescent {
            return Err(VirtualRuntimeErrorV1::QueueNotQuiescent {
                ordinal: queue.ordinal,
            });
        }
        self.model = self
            .model
            .next(RuntimeTransitionV1::SettleAfterQuiescence {
                completion: self.dispatches[index].model_completion,
            })?;
        self.dispatches[index].state = VirtualCompletionStateV1::FailedQuiescent;
        Ok(())
    }

    fn execute_dispatch(
        &mut self,
        index: usize,
    ) -> Result<VirtualRunProgressV1, VirtualRuntimeErrorV1> {
        let completion = self.dispatches[index].completion;
        let module_index = self.module_index(self.dispatches[index].module)?;
        let (request, backing_handles) = self.build_simulation_request(index)?;
        let execution = self.modules[module_index].module.simulate_scheduled(
            &request,
            self.config.target.simulation_target(),
            self.config.simulation_limits,
            SimulationScheduleRequestV1::RecordCanonical {
                max_decisions: self.config.runtime_limits.max_schedule_decisions,
            },
        );
        let execution = match execution {
            Ok(execution) => execution,
            Err(source) => {
                self.model = self.model.next(RuntimeTransitionV1::AbortPrepared {
                    completion: self.dispatches[index].model_completion,
                })?;
                self.dispatches[index].state = VirtualCompletionStateV1::AbortedSimulation;
                return Err(VirtualRuntimeErrorV1::Simulation {
                    completion,
                    source: Box::new(source),
                });
            }
        };
        let coverage = execution.schedule_coverage();
        let summary = VirtualSimulationSummaryV1 {
            kir_identity: *execution.identity(),
            target: self.config.target,
            invocations_executed: execution.invocations_executed(),
            workgroups_visited: execution.workgroups_visited(),
            scheduled_slots_visited: execution.scheduled_slots_visited(),
            steps_executed: execution.steps_executed(),
            schedule: execution.schedule(),
            schedule_transcript_identity: *execution.schedule_transcript_identity(),
            schedule_decisions: coverage.decisions(),
            schedule_barrier_releases: coverage.barrier_releases(),
            conflict_state: match execution.conflict_assessment() {
                SimulationConflictAssessmentV1::NoConflictsObserved => {
                    VirtualConflictStateV1::NoneObserved
                }
                SimulationConflictAssessmentV1::ConflictsObserved { .. } => {
                    VirtualConflictStateV1::Observed
                }
                SimulationConflictAssessmentV1::Incomplete { .. } => {
                    VirtualConflictStateV1::Incomplete
                }
            },
            race_state: match execution.race_assessment() {
                SimulationRaceAssessmentV1::NoRacesObserved { .. } => {
                    VirtualRaceStateV1::NoneObserved
                }
                SimulationRaceAssessmentV1::RacesObserved { .. } => VirtualRaceStateV1::Observed,
                SimulationRaceAssessmentV1::Incomplete { .. } => VirtualRaceStateV1::Incomplete,
            },
        };
        let model_completion = self.dispatches[index].model_completion;
        self.model = self
            .model
            .next(RuntimeTransitionV1::PublishDispatch {
                completion: model_completion,
            })?
            .next(RuntimeTransitionV1::ObserveCompletion {
                completion: model_completion,
            })?;
        let (_, outputs) = execution.into_outputs();
        for (handle, output) in backing_handles.into_iter().zip(outputs) {
            let record = self.buffer_mut(handle)?;
            record.bytes = output.buffer.bytes().to_vec();
            record.initialized = output.buffer.initialized().to_vec();
        }
        self.dispatches[index].state = VirtualCompletionStateV1::Completed;
        self.dispatches[index].summary = Some(summary.clone());
        Ok(VirtualRunProgressV1::Completed {
            completion,
            summary,
        })
    }

    fn build_simulation_request(
        &self,
        dispatch_index: usize,
    ) -> Result<(SimulationRequestV1, Vec<VirtualBufferHandleV1>), VirtualRuntimeErrorV1> {
        let request = &self.dispatches[dispatch_index].request;
        let mut handles = Vec::new();
        for argument in &request.arguments {
            if let VirtualArgumentV1::Buffer { buffer, .. } = argument
                && !handles.contains(buffer)
            {
                handles.push(*buffer);
            }
        }
        let mut shared = Vec::new();
        for (index, handle) in handles.iter().enumerate() {
            let record = self.buffer(*handle)?;
            let mut views = request
                .arguments
                .iter()
                .filter_map(|argument| match argument {
                    VirtualArgumentV1::Buffer {
                        buffer,
                        element,
                        alignment,
                        ..
                    } if buffer == handle => Some((*element, *alignment)),
                    _ => None,
                });
            let (element, mut alignment) = views
                .next()
                .expect("buffer handles were collected from the same arguments");
            for (candidate, candidate_alignment) in views {
                if candidate != element {
                    return Err(VirtualRuntimeErrorV1::SimulatorBuffer(format!(
                        "buffer {} cannot be viewed with multiple scalar element types",
                        handle.ordinal
                    )));
                }
                alignment = alignment.max(candidate_alignment);
            }
            let buffer = BufferArgumentV1::new(
                element,
                AccessMode::ReadWrite,
                alignment,
                record.bytes.clone(),
                record.initialized.clone(),
                self.config.target.simulation_target(),
            )
            .map_err(|error| VirtualRuntimeErrorV1::SimulatorBuffer(error.to_string()))?;
            shared.push(SharedBufferV1 {
                id: BufferBackingIdV1(index as u32),
                buffer,
            });
        }
        let mut arguments = Vec::new();
        arguments
            .try_reserve_exact(request.arguments.len())
            .map_err(|_| VirtualRuntimeErrorV1::CapacityExceeded("dispatch arguments"))?;
        for argument in &request.arguments {
            match argument {
                VirtualArgumentV1::Scalar(value) => {
                    arguments.push(SimulationArgumentV1::Scalar(*value));
                }
                VirtualArgumentV1::Buffer {
                    buffer,
                    element,
                    access,
                    alignment,
                    byte_offset,
                    elements,
                } => {
                    let backing = handles
                        .iter()
                        .position(|candidate| candidate == buffer)
                        .expect("buffer handles were collected from the same arguments");
                    let view = BufferViewArgumentV1::new(
                        BufferBackingIdV1(backing as u32),
                        *element,
                        *access,
                        *alignment,
                        *byte_offset,
                        *elements,
                        self.config.target.simulation_target(),
                    )
                    .map_err(|error| VirtualRuntimeErrorV1::SimulatorBuffer(error.to_string()))?;
                    arguments.push(SimulationArgumentV1::BufferView(view));
                }
            }
        }
        Ok((
            SimulationRequestV1 {
                kernel: request.kernel.clone(),
                grid: fe2o3_kir_sim::GridShapeV1(request.grid),
                workgroup: fe2o3_kir_sim::WorkgroupShapeV1(request.workgroup),
                arguments,
                shared_buffers: shared,
                events: EventPolicyV1::Disabled,
            },
            handles,
        ))
    }

    fn dependency_readiness(&self, index: usize) -> DependencyReadinessV1 {
        for dependency in &self.dispatches[index].request.dependencies {
            let state = self
                .dispatch(*dependency)
                .expect("dependencies were validated at submission")
                .state;
            match state {
                VirtualCompletionStateV1::Completed => {}
                VirtualCompletionStateV1::AbortedDependency
                | VirtualCompletionStateV1::AbortedSimulation
                | VirtualCompletionStateV1::FailedQuiescent => {
                    return DependencyReadinessV1::Failed(*dependency);
                }
                VirtualCompletionStateV1::Prepared | VirtualCompletionStateV1::Ambiguous => {
                    return DependencyReadinessV1::Blocked;
                }
            }
        }
        DependencyReadinessV1::Ready
    }

    fn validate_and_collect_resources(
        &self,
        arguments: &[VirtualArgumentV1],
    ) -> Result<Vec<DispatchResourceV1>, VirtualRuntimeErrorV1> {
        let mut resources: Vec<DispatchResourceV1> = Vec::new();
        for argument in arguments {
            let VirtualArgumentV1::Buffer { buffer, access, .. } = argument else {
                continue;
            };
            let record = self.buffer(*buffer)?;
            if !record.access.permits(*access) {
                return Err(VirtualRuntimeErrorV1::InvalidBufferAccess {
                    ordinal: buffer.ordinal,
                });
            }
            let required_access = match access {
                AccessMode::ReadOnly => MemoryAccessV1::Read,
                AccessMode::WriteOnly | AccessMode::ReadWrite => MemoryAccessV1::ReadWrite,
            };
            if let Some(existing) = resources
                .iter_mut()
                .find(|resource| resource.mapping == record.mapping)
            {
                if required_access == MemoryAccessV1::ReadWrite {
                    existing.required_access = required_access;
                }
            } else {
                resources.push(DispatchResourceV1 {
                    mapping: record.mapping,
                    required_access,
                });
            }
        }
        resources.sort_unstable_by_key(|resource| resource.mapping);
        Ok(resources)
    }

    fn validate_module_target(
        &self,
        module: &AdmittedSimulationModuleV1,
    ) -> Result<(), VirtualRuntimeErrorV1> {
        let mut exact = module
            .module()
            .effective_capabilities()
            .into_iter()
            .filter_map(|capability| match capability {
                fe2o3_kernel_ir::TargetCapability::Extension { namespace, name }
                    if namespace == fe2o3_kernel_ir::AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE =>
                {
                    Some(name)
                }
                _ => None,
            });
        let Some(target) = exact.next() else {
            return Ok(());
        };
        if target != self.config.target.label() {
            return Err(VirtualRuntimeErrorV1::ExactTargetMismatch {
                module_target: target,
                runtime_target: self.config.target.label(),
            });
        }
        Ok(())
    }

    fn provision_mapping(
        &self,
        byte_len: u64,
        access: MemoryAccessV1,
        allocation_id: u64,
    ) -> Result<(AllocationKeyV1, MappingKeyV1, RuntimeStateV1, u64), VirtualRuntimeErrorV1> {
        let allocation = AllocationKeyV1 {
            vm: self.vm,
            id: AllocationIdV1(allocation_id),
        };
        let mapping = MappingKeyV1 {
            allocation,
            id: MappingIdV1(allocation_id),
        };
        let next_va = align_up(
            self.next_va
                .checked_add(byte_len)
                .ok_or(VirtualRuntimeErrorV1::ByteLengthOverflow)?,
            VA_ALIGNMENT,
        )?;
        let next = self
            .model
            .next(RuntimeTransitionV1::Allocate {
                key: allocation,
                byte_len,
            })?
            .next(RuntimeTransitionV1::Map {
                key: mapping,
                allocation_offset: 0,
                gpu_va: self.next_va,
                byte_len,
                access,
            })?;
        Ok((allocation, mapping, next, next_va))
    }

    fn new_buffer_handle(&mut self) -> VirtualBufferHandleV1 {
        let ordinal = self.take_ordinal();
        VirtualBufferHandleV1 {
            runtime_identity: self.config.runtime_identity,
            ordinal,
        }
    }

    fn new_module_handle(&mut self) -> VirtualModuleHandleV1 {
        let ordinal = self.take_ordinal();
        VirtualModuleHandleV1 {
            runtime_identity: self.config.runtime_identity,
            ordinal,
        }
    }

    fn new_queue_handle(&mut self) -> VirtualQueueHandleV1 {
        let ordinal = self.take_ordinal();
        VirtualQueueHandleV1 {
            runtime_identity: self.config.runtime_identity,
            ordinal,
        }
    }

    fn new_completion_handle(&mut self) -> VirtualCompletionHandleV1 {
        let ordinal = self.take_ordinal();
        VirtualCompletionHandleV1 {
            runtime_identity: self.config.runtime_identity,
            ordinal,
        }
    }

    fn take_ordinal(&mut self) -> u64 {
        let ordinal = self.next_ordinal;
        self.next_ordinal = self
            .next_ordinal
            .checked_add(1)
            .expect("bounded runtime ordinal cannot overflow");
        ordinal
    }

    fn validate_handle(
        &self,
        kind: &'static str,
        identity: IdentityDigestV1,
    ) -> Result<(), VirtualRuntimeErrorV1> {
        if identity != self.config.runtime_identity {
            return Err(VirtualRuntimeErrorV1::ForeignHandle { kind });
        }
        Ok(())
    }

    fn buffer_index(&self, handle: VirtualBufferHandleV1) -> Result<usize, VirtualRuntimeErrorV1> {
        self.validate_handle("buffer", handle.runtime_identity)?;
        self.buffers
            .iter()
            .position(|record| record.handle == handle)
            .ok_or(VirtualRuntimeErrorV1::UnknownHandle {
                kind: "buffer",
                ordinal: handle.ordinal,
            })
    }

    fn buffer(
        &self,
        handle: VirtualBufferHandleV1,
    ) -> Result<&BufferRecordV1, VirtualRuntimeErrorV1> {
        let index = self.buffer_index(handle)?;
        if self.buffers[index].released {
            return Err(VirtualRuntimeErrorV1::ReleasedHandle {
                kind: "buffer",
                ordinal: handle.ordinal,
            });
        }
        Ok(&self.buffers[index])
    }

    fn buffer_mut(
        &mut self,
        handle: VirtualBufferHandleV1,
    ) -> Result<&mut BufferRecordV1, VirtualRuntimeErrorV1> {
        let index = self.buffer_index(handle)?;
        if self.buffers[index].released {
            return Err(VirtualRuntimeErrorV1::ReleasedHandle {
                kind: "buffer",
                ordinal: handle.ordinal,
            });
        }
        Ok(&mut self.buffers[index])
    }

    fn module_index(&self, handle: VirtualModuleHandleV1) -> Result<usize, VirtualRuntimeErrorV1> {
        self.validate_handle("module", handle.runtime_identity)?;
        self.modules
            .iter()
            .position(|record| record.handle == handle)
            .ok_or(VirtualRuntimeErrorV1::UnknownHandle {
                kind: "module",
                ordinal: handle.ordinal,
            })
    }

    fn require_live_module(&self, index: usize) -> Result<(), VirtualRuntimeErrorV1> {
        if self.modules[index].released {
            return Err(VirtualRuntimeErrorV1::ReleasedHandle {
                kind: "module",
                ordinal: self.modules[index].handle.ordinal,
            });
        }
        Ok(())
    }

    fn queue_index(&self, handle: VirtualQueueHandleV1) -> Result<usize, VirtualRuntimeErrorV1> {
        self.validate_handle("queue", handle.runtime_identity)?;
        self.queues
            .iter()
            .position(|record| record.handle == handle)
            .ok_or(VirtualRuntimeErrorV1::UnknownHandle {
                kind: "queue",
                ordinal: handle.ordinal,
            })
    }

    fn require_live_queue(&self, index: usize) -> Result<(), VirtualRuntimeErrorV1> {
        if self.queues[index].released || self.queues[index].quiescent {
            return Err(VirtualRuntimeErrorV1::ReleasedHandle {
                kind: "queue",
                ordinal: self.queues[index].handle.ordinal,
            });
        }
        Ok(())
    }

    fn dispatch_index(
        &self,
        completion: VirtualCompletionHandleV1,
    ) -> Result<usize, VirtualRuntimeErrorV1> {
        self.validate_handle("completion", completion.runtime_identity)?;
        self.dispatches
            .iter()
            .position(|record| record.completion == completion)
            .ok_or(VirtualRuntimeErrorV1::UnknownHandle {
                kind: "completion",
                ordinal: completion.ordinal,
            })
    }

    fn dispatch(
        &self,
        completion: VirtualCompletionHandleV1,
    ) -> Result<&DispatchRecordLocalV1, VirtualRuntimeErrorV1> {
        let index = self.dispatch_index(completion)?;
        Ok(&self.dispatches[index])
    }
}

enum DependencyReadinessV1 {
    Ready,
    Blocked,
    Failed(VirtualCompletionHandleV1),
}

fn align_up(value: u64, alignment: u64) -> Result<u64, VirtualRuntimeErrorV1> {
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or(VirtualRuntimeErrorV1::ByteLengthOverflow)
}
