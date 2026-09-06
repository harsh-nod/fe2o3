#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, FunctionRole, KernelId, MAX_SIMULATION_BUNDLE_BYTES_V6,
    OperationKind, ScalarType, SemanticAggregateStorageMapV5, SemanticAggregateStorageMapV6,
    SemanticArgumentOwnershipV1, SemanticComponentStorageBindingV2, SemanticKernargSlotV2,
    SemanticKernelStorageV2, SemanticKirComponentRepresentationV2,
    SemanticKirStorageRepresentationV1, SemanticStorageBindingV1, SemanticStorageMapV1,
    SemanticStorageMapV2, SemanticStorageMapV5, SemanticStorageMapV6, SemanticStorageProjectionV2,
    Type, VerifiedCanonicalKernelIrV7, VerifiedCanonicalKernelIrV10, VerifiedCanonicalKernelIrV11,
    VerifiedSimulationBundleV3, VerifiedSimulationBundleV4, VerifiedSimulationBundleV5,
    VerifiedSimulationBundleV6,
};
use fe2o3_kir_sim::{AdmittedSimulationModuleV1, DynamicWorkgroupMemoryRequestV1, ScalarBitsV1};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticAbiArgumentV1, SemanticAbiExtensionV1,
    SemanticAbiPassModeV1, SemanticAbiPointeeKindV1, SemanticAbiPointerCaptureV1,
    SemanticAbiRegisterKindV1, SemanticBackendPrimitiveV1, SemanticBackendReprV1,
    SemanticBackendScalarV1, SemanticCanonAbiV1, SemanticEnumEncodingV1, SemanticFieldsShapeV1,
    SemanticLocalRoleV1, SemanticMirLimitsV1, SemanticMutabilityV1, SemanticPointerKindV1,
    SemanticPointerMetadataV1, SemanticRustTypeKindV1, SemanticRustcVariantsV1,
    SemanticScalarTypeV1, SemanticScalarValidityRangeV1, SemanticSourceArgumentOwnershipV1,
    SemanticTypeDeclV1, SemanticTypeIdV1, SemanticTypeLayoutDetailsV1, SemanticTypeShapeV1,
};
use fe2o3_runtime::{
    BackendBindingV1, BackendDeviceDescriptionV1, BackendLaunchV1, BackendMemoryRegionV1,
    BackendPollV1, BackendSemanticLaunchV1, MAX_RUNTIME_ALLOCATIONS_V1,
    MAX_RUNTIME_DEPENDENCIES_V1, MAX_RUNTIME_EVENTS_V1, MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1,
    MAX_RUNTIME_KERNEL_NAME_BYTES_V1, MAX_RUNTIME_KERNELS_V1, MAX_RUNTIME_MODULES_V1,
    MAX_RUNTIME_STREAMS_V1, MAX_RUNTIME_SUBMISSIONS_V1, RuntimeAccessV1, RuntimeBackendFailureV1,
    RuntimeBackendV1, RuntimeCapabilitiesV1, RuntimeMemoryKindV1,
};
use fe2o3_runtime_model::IdentityDigestV1;
use fe2o3_virtual_runtime::{
    VirtualArgumentV1, VirtualBufferAccessV1, VirtualBufferHandleV1, VirtualCompletionHandleV1,
    VirtualDispatchRequestV1, VirtualModuleHandleV1, VirtualQueueHandleV1, VirtualRunProgressV1,
    VirtualRuntimeConfigV1, VirtualRuntimeErrorV1, VirtualRuntimeV1, VirtualTargetProfileV1,
};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::sync::TryLockError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const DEVICE_HANDLE: u64 = 1;
const QUEUE_CAPACITY: u32 = 64;
const COMMAND_QUEUE_CAPACITY: usize = 64;
const FAILED_SIMULATION_CODE: i64 = -1;
const WORKER_LIVENESS_POLL_INTERVAL: Duration = Duration::from_millis(10);
const MAX_RECURSIVE_ARGUMENT_COMPONENTS_V2: usize = 256;

/// Stable evidence describing what this backend can establish.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimRuntimeEvidenceV1 {
    pub mode: &'static str,
    pub simulated: bool,
    pub hardware: bool,
    pub performance_prediction: bool,
}

/// Construction and execution policy for one simulator backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimRuntimeBackendConfigV1 {
    pub virtual_runtime: VirtualRuntimeConfigV1,
}

/// Typed, bounded backend error. A disconnected worker is terminal; validation
/// and virtual-runtime rejections happen before the requested custody transfer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimRuntimeBackendErrorV1 {
    InvalidBundle(String),
    UnsupportedBundle(String),
    InvalidHandle(&'static str),
    InvalidKernel(String),
    InvalidArguments(String),
    VirtualRuntime(String),
    CommandQueueFull,
    WorkerDisconnected,
    WorkerPanicked,
}

impl fmt::Display for SimRuntimeBackendErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBundle(detail) => write!(formatter, "invalid simulation bundle: {detail}"),
            Self::UnsupportedBundle(detail) => {
                write!(formatter, "unsupported simulation bundle: {detail}")
            }
            Self::InvalidHandle(kind) => write!(formatter, "invalid simulator {kind} handle"),
            Self::InvalidKernel(detail) => write!(formatter, "invalid simulator kernel: {detail}"),
            Self::InvalidArguments(detail) => {
                write!(formatter, "invalid simulator launch arguments: {detail}")
            }
            Self::VirtualRuntime(detail) => {
                write!(formatter, "virtual runtime rejected operation: {detail}")
            }
            Self::CommandQueueFull => formatter.write_str("simulator command queue is full"),
            Self::WorkerDisconnected => formatter.write_str("simulator worker disconnected"),
            Self::WorkerPanicked => formatter.write_str("simulator worker panicked"),
        }
    }
}

impl Error for SimRuntimeBackendErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CompletionOutcomeV1 {
    Pending,
    Succeeded,
    Failed(i64),
}

struct CompletionCellV1 {
    outcome: Mutex<CompletionOutcomeV1>,
    changed: Condvar,
}

impl CompletionCellV1 {
    fn pending() -> Self {
        Self {
            outcome: Mutex::new(CompletionOutcomeV1::Pending),
            changed: Condvar::new(),
        }
    }

    fn finish(&self, outcome: CompletionOutcomeV1) {
        if let Ok(mut current) = self.outcome.lock() {
            *current = outcome;
            self.changed.notify_all();
        }
    }

    fn poll(&self, worker_alive: &AtomicBool) -> Result<BackendPollV1, SimRuntimeBackendErrorV1> {
        match self.outcome.try_lock() {
            Ok(outcome)
                if *outcome == CompletionOutcomeV1::Pending
                    && !worker_alive.load(Ordering::Acquire) =>
            {
                Err(SimRuntimeBackendErrorV1::WorkerDisconnected)
            }
            Ok(outcome) => Ok(backend_poll(*outcome)),
            Err(TryLockError::WouldBlock) if worker_alive.load(Ordering::Acquire) => {
                Ok(BackendPollV1::Pending)
            }
            Err(TryLockError::WouldBlock) => Err(SimRuntimeBackendErrorV1::WorkerDisconnected),
            Err(TryLockError::Poisoned(_)) => Err(SimRuntimeBackendErrorV1::WorkerPanicked),
        }
    }

    fn wait(
        &self,
        deadline: Instant,
        worker_alive: &AtomicBool,
    ) -> Result<BackendPollV1, SimRuntimeBackendErrorV1> {
        let mut outcome = self
            .outcome
            .lock()
            .map_err(|_| SimRuntimeBackendErrorV1::WorkerPanicked)?;
        while *outcome == CompletionOutcomeV1::Pending {
            if !worker_alive.load(Ordering::Acquire) {
                return Err(SimRuntimeBackendErrorV1::WorkerDisconnected);
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(BackendPollV1::Pending);
            }
            let timeout = deadline
                .saturating_duration_since(now)
                .min(WORKER_LIVENESS_POLL_INTERVAL);
            let waited = self
                .changed
                .wait_timeout(outcome, timeout)
                .map_err(|_| SimRuntimeBackendErrorV1::WorkerPanicked)?;
            outcome = waited.0;
            if waited.1.timed_out()
                && *outcome == CompletionOutcomeV1::Pending
                && Instant::now() >= deadline
            {
                return Ok(BackendPollV1::Pending);
            }
        }
        Ok(backend_poll(*outcome))
    }
}

fn backend_poll(outcome: CompletionOutcomeV1) -> BackendPollV1 {
    match outcome {
        CompletionOutcomeV1::Pending => BackendPollV1::Pending,
        CompletionOutcomeV1::Succeeded => BackendPollV1::Succeeded,
        CompletionOutcomeV1::Failed(code) => BackendPollV1::Failed { code },
    }
}

#[derive(Clone)]
struct AllocationRecordV1 {
    buffer: VirtualBufferHandleV1,
    byte_len: u64,
    alignment: u64,
}

struct ModuleRecordV1 {
    module: VirtualModuleHandleV1,
    kernels: HashMap<String, Arc<KernelRecordV1>>,
}

#[derive(Clone)]
struct KernelRecordV1 {
    module: Option<VirtualModuleHandleV1>,
    kernel: KernelId,
    signature: [u8; 32],
    explicit_byte_len: usize,
    arguments: Vec<ArgumentRecordV1>,
    requires_dynamic_workgroup_memory: bool,
    unsupported: Option<String>,
}

#[derive(Clone)]
struct ArgumentRecordV1 {
    offset: usize,
    size: usize,
    ty: Type,
    materialization: ArgumentMaterializationV1,
}

#[derive(Clone)]
enum ArgumentMaterializationV1 {
    ExactBytes {
        validity: Vec<SemanticScalarValidityRangeV1>,
        guards: Vec<EnumVariantGuardV1>,
    },
    EnumDiscriminant {
        decoder: EnumDecoderV1,
        guards: Vec<EnumVariantGuardV1>,
    },
    Region {
        metadata: Option<PhysicalSlotV1>,
    },
}

#[derive(Clone, Copy)]
struct PhysicalSlotV1 {
    offset: usize,
    size: usize,
}

#[derive(Clone)]
struct EnumVariantGuardV1 {
    decoder: EnumDecoderV1,
    required_variant: u32,
}

#[derive(Clone)]
struct EnumDecoderV1 {
    byte_offset: usize,
    byte_width: usize,
    variants: Vec<EnumVariantValueV1>,
    encoding: EnumDecoderEncodingV1,
}

#[derive(Clone, Copy)]
struct EnumVariantValueV1 {
    index: u32,
    discriminant: u128,
    uninhabited: bool,
}

#[derive(Clone)]
enum EnumDecoderEncodingV1 {
    Single {
        variant: u32,
    },
    Direct {
        physical_bits: u16,
        logical_signed: bool,
        logical_bits: u16,
        validity: Option<SemanticScalarValidityRangeV1>,
    },
    Niche {
        physical_bits: u16,
        source_validity: SemanticScalarValidityRangeV1,
        untagged_variant: u32,
        niche_variants_start: u32,
        niche_variants_end: u32,
        niche_start: u128,
    },
}

struct SubmissionRecordV1 {
    stream: u64,
    completion: Arc<CompletionCellV1>,
}

enum WorkerCommandV1 {
    CreateQueue {
        response: mpsc::SyncSender<Result<VirtualQueueHandleV1, String>>,
    },
    ReleaseQueue {
        queue: VirtualQueueHandleV1,
        response: mpsc::SyncSender<Result<(), String>>,
    },
    Allocate {
        byte_len: usize,
        response: mpsc::SyncSender<Result<VirtualBufferHandleV1, String>>,
    },
    ReleaseAllocation {
        buffer: VirtualBufferHandleV1,
        response: mpsc::SyncSender<Result<(), String>>,
    },
    Write {
        buffer: VirtualBufferHandleV1,
        offset: usize,
        bytes: Vec<u8>,
        response: mpsc::SyncSender<Result<(), String>>,
    },
    Read {
        buffer: VirtualBufferHandleV1,
        offset: usize,
        byte_len: usize,
        response: mpsc::SyncSender<Result<Vec<u8>, String>>,
    },
    RegisterModule {
        module: AdmittedSimulationModuleV1,
        response: mpsc::SyncSender<Result<VirtualModuleHandleV1, String>>,
    },
    ReleaseModule {
        module: VirtualModuleHandleV1,
        response: mpsc::SyncSender<Result<(), String>>,
    },
    Submit {
        id: u64,
        queue: VirtualQueueHandleV1,
        module: VirtualModuleHandleV1,
        request: PreparedRequestV1,
        dependencies: Vec<u64>,
        completion: Arc<CompletionCellV1>,
    },
    #[cfg(test)]
    Panic,
    #[cfg(test)]
    Block {
        started: mpsc::SyncSender<()>,
        release: mpsc::Receiver<()>,
    },
    #[cfg(test)]
    Noop,
    Shutdown,
}

struct PreparedRequestV1 {
    kernel: KernelId,
    grid: [u64; 3],
    workgroup: [u32; 3],
    arguments: Vec<VirtualArgumentV1>,
    dynamic_workgroup_memory: Option<DynamicWorkgroupMemoryRequestV1>,
}

/// Explicit CPU semantic simulator behind the normal runtime facade.
pub struct SimRuntimeBackendV1 {
    config: SimRuntimeBackendConfigV1,
    commands: Option<mpsc::SyncSender<WorkerCommandV1>>,
    worker: Option<JoinHandle<()>>,
    worker_alive: Arc<AtomicBool>,
    next_handle: u64,
    streams: HashMap<u64, VirtualQueueHandleV1>,
    allocations: HashMap<u64, AllocationRecordV1>,
    modules: HashMap<u64, ModuleRecordV1>,
    kernels: HashMap<u64, Arc<KernelRecordV1>>,
    submissions: HashMap<u64, SubmissionRecordV1>,
    events: HashMap<u64, u64>,
    terminal: bool,
}

impl SimRuntimeBackendV1 {
    pub fn gfx942(runtime_identity: [u8; 32]) -> Result<Self, SimRuntimeBackendErrorV1> {
        Self::new(SimRuntimeBackendConfigV1 {
            virtual_runtime: VirtualRuntimeConfigV1 {
                runtime_identity: IdentityDigestV1::from_untrusted_bytes(runtime_identity),
                target: VirtualTargetProfileV1::Gfx942XnackMinus,
                runtime_limits: Default::default(),
                simulation_limits: Default::default(),
            },
        })
    }

    pub fn gfx950(runtime_identity: [u8; 32]) -> Result<Self, SimRuntimeBackendErrorV1> {
        Self::new(SimRuntimeBackendConfigV1 {
            virtual_runtime: VirtualRuntimeConfigV1 {
                runtime_identity: IdentityDigestV1::from_untrusted_bytes(runtime_identity),
                target: VirtualTargetProfileV1::Gfx950XnackMinus,
                runtime_limits: Default::default(),
                simulation_limits: Default::default(),
            },
        })
    }

    pub fn new(config: SimRuntimeBackendConfigV1) -> Result<Self, SimRuntimeBackendErrorV1> {
        let runtime = VirtualRuntimeV1::new(config.virtual_runtime)
            .map_err(|error| SimRuntimeBackendErrorV1::VirtualRuntime(error.to_string()))?;
        let (commands, receiver) = mpsc::sync_channel(COMMAND_QUEUE_CAPACITY);
        let worker_alive = Arc::new(AtomicBool::new(true));
        let worker_alive_for_thread = worker_alive.clone();
        let worker = thread::Builder::new()
            .name("fe2o3-cpu-simulator".to_owned())
            .spawn(move || worker_main(runtime, receiver, worker_alive_for_thread))
            .map_err(|_| SimRuntimeBackendErrorV1::WorkerDisconnected)?;
        Ok(Self {
            config,
            commands: Some(commands),
            worker: Some(worker),
            worker_alive,
            next_handle: 2,
            streams: HashMap::new(),
            allocations: HashMap::new(),
            modules: HashMap::new(),
            kernels: HashMap::new(),
            submissions: HashMap::new(),
            events: HashMap::new(),
            terminal: false,
        })
    }

    pub const fn evidence(&self) -> SimRuntimeEvidenceV1 {
        SimRuntimeEvidenceV1 {
            mode: "cpu-kir-semantic-simulation",
            simulated: true,
            hardware: false,
            performance_prediction: false,
        }
    }

    pub const fn uses_gpu(&self) -> bool {
        false
    }

    pub fn submission_failure(&self, submission: u64) -> Option<SimRuntimeBackendErrorV1> {
        let record = self.submissions.get(&submission)?;
        match record.completion.outcome.lock().ok().map(|value| *value) {
            Some(CompletionOutcomeV1::Failed(_)) => Some(SimRuntimeBackendErrorV1::VirtualRuntime(
                "semantic simulation failed; poll returned the stable failure code".to_owned(),
            )),
            _ => None,
        }
    }

    fn handle(&mut self) -> Result<u64, SimRuntimeBackendErrorV1> {
        let handle = self.next_handle;
        self.next_handle = handle
            .checked_add(1)
            .ok_or(SimRuntimeBackendErrorV1::InvalidHandle("capacity"))?;
        Ok(handle)
    }

    fn require_live(&self) -> Result<(), RuntimeBackendFailureV1<SimRuntimeBackendErrorV1>> {
        if self.terminal || !self.worker_alive.load(Ordering::Acquire) {
            Err(RuntimeBackendFailureV1::Terminal(
                SimRuntimeBackendErrorV1::WorkerDisconnected,
            ))
        } else {
            Ok(())
        }
    }

    fn call<T>(
        &mut self,
        build: impl FnOnce(mpsc::SyncSender<Result<T, String>>) -> WorkerCommandV1,
    ) -> Result<T, RuntimeBackendFailureV1<SimRuntimeBackendErrorV1>> {
        self.require_live()?;
        let (send, receive) = mpsc::sync_channel(1);
        let Some(commands) = &self.commands else {
            self.terminal = true;
            return Err(RuntimeBackendFailureV1::Terminal(
                SimRuntimeBackendErrorV1::WorkerDisconnected,
            ));
        };
        if commands.send(build(send)).is_err() {
            self.terminal = true;
            return Err(RuntimeBackendFailureV1::Terminal(
                SimRuntimeBackendErrorV1::WorkerDisconnected,
            ));
        }
        match receive.recv() {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(detail)) => Err(RuntimeBackendFailureV1::Rejected(
                SimRuntimeBackendErrorV1::VirtualRuntime(detail),
            )),
            Err(_) => {
                self.terminal = true;
                Err(RuntimeBackendFailureV1::Terminal(
                    SimRuntimeBackendErrorV1::WorkerDisconnected,
                ))
            }
        }
    }
}

impl Drop for SimRuntimeBackendV1 {
    fn drop(&mut self) {
        if let Some(commands) = self.commands.take() {
            let _ = commands.try_send(WorkerCommandV1::Shutdown);
            drop(commands);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl RuntimeBackendV1 for SimRuntimeBackendV1 {
    type Error = SimRuntimeBackendErrorV1;

    fn enumerate_devices_v1(
        &mut self,
    ) -> Result<Vec<BackendDeviceDescriptionV1>, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        Ok(vec![BackendDeviceDescriptionV1 {
            backend_device: DEVICE_HANDLE,
            name: "fe2o3 deterministic CPU simulator".to_owned(),
            target: self.config.virtual_runtime.target.label().to_owned(),
            global_memory_bytes: self
                .config
                .virtual_runtime
                .runtime_limits
                .max_total_user_bytes as u64,
            capabilities: RuntimeCapabilitiesV1 {
                typed_async_launch: true,
                streams: true,
                events: true,
                device_memory: true,
                host_visible_memory: true,
                peer_copy: false,
                multi_device: false,
                atomics: true,
                collectives: false,
            },
        }])
    }

    fn create_stream_v1(
        &mut self,
        device: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if device != DEVICE_HANDLE {
            return Err(rejected_handle("device"));
        }
        require_capacity(self.streams.len(), MAX_RUNTIME_STREAMS_V1)?;
        let handle = self.handle().map_err(RuntimeBackendFailureV1::Rejected)?;
        let queue = self.call(|response| WorkerCommandV1::CreateQueue { response })?;
        self.streams.insert(handle, queue);
        Ok(handle)
    }

    fn destroy_stream_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let queue = *self
            .streams
            .get(&stream)
            .ok_or_else(|| rejected_handle("stream"))?;
        self.call(|response| WorkerCommandV1::ReleaseQueue { queue, response })?;
        self.streams.remove(&stream);
        Ok(())
    }

    fn allocate_v1(
        &mut self,
        device: u64,
        _kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if device != DEVICE_HANDLE {
            return Err(rejected_handle("device"));
        }
        require_capacity(self.allocations.len(), MAX_RUNTIME_ALLOCATIONS_V1)?;
        if byte_len == 0 || alignment == 0 || !alignment.is_power_of_two() {
            return Err(rejected_arguments("allocation length or alignment"));
        }
        let byte_len_usize =
            usize::try_from(byte_len).map_err(|_| rejected_arguments("allocation length"))?;
        let handle = self.handle().map_err(RuntimeBackendFailureV1::Rejected)?;
        let buffer = self.call(|response| WorkerCommandV1::Allocate {
            byte_len: byte_len_usize,
            response,
        })?;
        self.allocations.insert(
            handle,
            AllocationRecordV1 {
                buffer,
                byte_len,
                alignment,
            },
        );
        Ok(handle)
    }

    fn release_allocation_v1(
        &mut self,
        allocation: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let buffer = self
            .allocations
            .get(&allocation)
            .ok_or_else(|| rejected_handle("allocation"))?
            .buffer;
        self.call(|response| WorkerCommandV1::ReleaseAllocation { buffer, response })?;
        self.allocations.remove(&allocation);
        Ok(())
    }

    fn write_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let allocation_record = self
            .allocations
            .get(&allocation)
            .ok_or_else(|| rejected_handle("allocation"))?;
        let buffer = allocation_record.buffer;
        require_range(allocation_record.byte_len, byte_offset, bytes.len())
            .map_err(RuntimeBackendFailureV1::Rejected)?;
        let offset =
            usize::try_from(byte_offset).map_err(|_| rejected_arguments("write offset"))?;
        self.call(|response| WorkerCommandV1::Write {
            buffer,
            offset,
            bytes: bytes.to_vec(),
            response,
        })
    }

    fn read_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        destination: &mut [u8],
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let allocation_record = self
            .allocations
            .get(&allocation)
            .ok_or_else(|| rejected_handle("allocation"))?;
        let buffer = allocation_record.buffer;
        require_range(allocation_record.byte_len, byte_offset, destination.len())
            .map_err(RuntimeBackendFailureV1::Rejected)?;
        let offset = usize::try_from(byte_offset).map_err(|_| rejected_arguments("read offset"))?;
        let bytes = self.call(|response| WorkerCommandV1::Read {
            buffer,
            offset,
            byte_len: destination.len(),
            response,
        })?;
        if bytes.len() != destination.len() {
            self.terminal = true;
            return Err(RuntimeBackendFailureV1::Terminal(
                SimRuntimeBackendErrorV1::WorkerDisconnected,
            ));
        }
        destination.copy_from_slice(&bytes);
        Ok(())
    }

    fn load_module_v1(
        &mut self,
        device: u64,
        image: &[u8],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if device != DEVICE_HANDLE {
            return Err(rejected_handle("device"));
        }
        if image.len() > MAX_SIMULATION_BUNDLE_BYTES_V6 {
            return Err(RuntimeBackendFailureV1::Rejected(
                SimRuntimeBackendErrorV1::InvalidBundle(
                    "bundle exceeds the V6 byte limit".to_owned(),
                ),
            ));
        }
        require_capacity(self.modules.len(), MAX_RUNTIME_MODULES_V1)?;
        let parsed = parse_bundle(image, self.config.virtual_runtime.target)
            .map_err(RuntimeBackendFailureV1::Rejected)?;
        let handle = self.handle().map_err(RuntimeBackendFailureV1::Rejected)?;
        let virtual_module = self.call(|response| WorkerCommandV1::RegisterModule {
            module: parsed.admitted,
            response,
        })?;
        let kernels = parsed
            .kernels
            .into_iter()
            .map(|(name, mut kernel)| {
                kernel.module = Some(virtual_module);
                (name, Arc::new(kernel))
            })
            .collect();
        self.modules.insert(
            handle,
            ModuleRecordV1 {
                module: virtual_module,
                kernels,
            },
        );
        Ok(handle)
    }

    fn unload_module_v1(
        &mut self,
        module: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let virtual_module = self
            .modules
            .get(&module)
            .ok_or_else(|| rejected_handle("module"))?
            .module;
        self.call(|response| WorkerCommandV1::ReleaseModule {
            module: virtual_module,
            response,
        })?;
        self.modules.remove(&module);
        self.kernels
            .retain(|_, kernel| kernel.module != Some(virtual_module));
        Ok(())
    }

    fn resolve_kernel_v1(
        &mut self,
        module: u64,
        name: &str,
        signature: [u8; 32],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if name.len() > MAX_RUNTIME_KERNEL_NAME_BYTES_V1 {
            return Err(rejected_arguments("kernel name length"));
        }
        require_capacity(self.kernels.len(), MAX_RUNTIME_KERNELS_V1)?;
        let record = self
            .modules
            .get(&module)
            .ok_or_else(|| rejected_handle("module"))?;
        let kernel = record
            .kernels
            .get(name)
            .ok_or_else(|| {
                RuntimeBackendFailureV1::Rejected(SimRuntimeBackendErrorV1::InvalidKernel(
                    name.to_owned(),
                ))
            })?
            .clone();
        if kernel.signature != signature {
            return Err(RuntimeBackendFailureV1::Rejected(
                SimRuntimeBackendErrorV1::InvalidKernel(
                    "semantic ABI signature mismatch".to_owned(),
                ),
            ));
        }
        if let Some(detail) = &kernel.unsupported {
            return Err(RuntimeBackendFailureV1::Rejected(
                SimRuntimeBackendErrorV1::UnsupportedBundle(detail.clone()),
            ));
        }
        let handle = self.handle().map_err(RuntimeBackendFailureV1::Rejected)?;
        self.kernels.insert(handle, kernel);
        Ok(handle)
    }

    fn submit_v1(
        &mut self,
        launch: BackendLaunchV1<'_>,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if launch.semantic_launch != BackendSemanticLaunchV1::Ordinary {
            return Err(rejected_arguments("semantic launch"));
        }
        require_capacity(self.submissions.len(), MAX_RUNTIME_SUBMISSIONS_V1)?;
        if launch.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(rejected_arguments("dependency count"));
        }
        if launch.explicit_kernarg.len() > MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1 {
            return Err(rejected_arguments("explicit kernarg length"));
        }
        let queue = *self
            .streams
            .get(&launch.stream)
            .ok_or_else(|| rejected_handle("stream"))?;
        let kernel = self
            .kernels
            .get(&launch.kernel)
            .ok_or_else(|| rejected_handle("kernel"))?
            .clone();
        let arguments = prepare_arguments(
            &kernel,
            launch.explicit_kernarg,
            launch.bindings,
            &self.allocations,
        )
        .map_err(RuntimeBackendFailureV1::Rejected)?;
        let mut dependencies = Vec::with_capacity(launch.dependencies.len());
        for event in launch.dependencies {
            dependencies.push(
                *self
                    .events
                    .get(event)
                    .ok_or_else(|| rejected_handle("event"))?,
            );
        }
        if dependencies
            .iter()
            .enumerate()
            .any(|(index, dependency)| dependencies[..index].contains(dependency))
        {
            return Err(rejected_arguments("duplicate dependency"));
        }
        let handle = self.handle().map_err(RuntimeBackendFailureV1::Rejected)?;
        let completion = Arc::new(CompletionCellV1::pending());
        let command = WorkerCommandV1::Submit {
            id: handle,
            queue,
            module: kernel
                .module
                .expect("resolved kernels have a registered module"),
            request: PreparedRequestV1 {
                kernel: kernel.kernel.clone(),
                grid: launch.geometry.grid.map(u64::from),
                workgroup: launch.geometry.workgroup,
                arguments,
                dynamic_workgroup_memory: dynamic_workgroup_memory_request(
                    kernel.requires_dynamic_workgroup_memory,
                    launch.geometry.dynamic_shared_bytes,
                ),
            },
            dependencies,
            completion: completion.clone(),
        };
        match self
            .commands
            .as_ref()
            .ok_or({
                RuntimeBackendFailureV1::Terminal(SimRuntimeBackendErrorV1::WorkerDisconnected)
            })?
            .try_send(command)
        {
            Ok(()) => {}
            Err(mpsc::TrySendError::Full(_)) => {
                return Err(RuntimeBackendFailureV1::Rejected(
                    SimRuntimeBackendErrorV1::CommandQueueFull,
                ));
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.terminal = true;
                return Err(RuntimeBackendFailureV1::Terminal(
                    SimRuntimeBackendErrorV1::WorkerDisconnected,
                ));
            }
        }
        self.submissions.insert(
            handle,
            SubmissionRecordV1 {
                stream: launch.stream,
                completion,
            },
        );
        Ok(handle)
    }

    fn poll_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let result = self
            .submissions
            .get(&submission)
            .ok_or_else(|| rejected_handle("submission"))?
            .completion
            .poll(&self.worker_alive);
        if result.is_err() {
            self.terminal = true;
        }
        result.map_err(RuntimeBackendFailureV1::Terminal)
    }

    fn wait_v1(
        &mut self,
        submission: u64,
        deadline: Instant,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let result = self
            .submissions
            .get(&submission)
            .ok_or_else(|| rejected_handle("submission"))?
            .completion
            .wait(deadline, &self.worker_alive);
        if result.is_err() {
            self.terminal = true;
        }
        result.map_err(RuntimeBackendFailureV1::Terminal)
    }

    fn release_submission_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        let record = self
            .submissions
            .get(&submission)
            .ok_or_else(|| rejected_handle("submission"))?;
        match record.completion.poll(&self.worker_alive) {
            Ok(BackendPollV1::Pending) => {
                return Err(RuntimeBackendFailureV1::Rejected(
                    SimRuntimeBackendErrorV1::InvalidHandle("pending submission"),
                ));
            }
            Ok(BackendPollV1::Succeeded | BackendPollV1::Failed { .. }) => {}
            Err(error) => {
                self.terminal = true;
                return Err(RuntimeBackendFailureV1::Terminal(error));
            }
        }
        if self.events.values().any(|retained| *retained == submission) {
            return Err(RuntimeBackendFailureV1::Rejected(
                SimRuntimeBackendErrorV1::InvalidHandle("event-retained submission"),
            ));
        }
        self.submissions.remove(&submission);
        Ok(())
    }

    fn record_event_v1(
        &mut self,
        stream: u64,
        submission: u64,
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        require_capacity(self.events.len(), MAX_RUNTIME_EVENTS_V1)?;
        if self
            .submissions
            .get(&submission)
            .is_none_or(|record| record.stream != stream)
        {
            return Err(rejected_handle("submission"));
        }
        let handle = self.handle().map_err(RuntimeBackendFailureV1::Rejected)?;
        self.events.insert(handle, submission);
        Ok(handle)
    }

    fn release_event_v1(&mut self, event: u64) -> Result<(), RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        if self.events.remove(&event).is_none() {
            return Err(rejected_handle("event"));
        }
        Ok(())
    }

    fn peer_copy_v1(
        &mut self,
        _stream: u64,
        _source: BackendMemoryRegionV1,
        _destination: BackendMemoryRegionV1,
        _dependencies: &[u64],
    ) -> Result<u64, RuntimeBackendFailureV1<Self::Error>> {
        self.require_live()?;
        Err(RuntimeBackendFailureV1::Rejected(
            SimRuntimeBackendErrorV1::UnsupportedBundle(
                "the V1 simulator exposes exactly one logical device and no peer copy".to_owned(),
            ),
        ))
    }
}

struct ParsedBundleV1 {
    admitted: AdmittedSimulationModuleV1,
    kernels: HashMap<String, KernelRecordV1>,
}

enum ParsedCanonicalKirV1 {
    V7(VerifiedCanonicalKernelIrV7),
    V10(VerifiedCanonicalKernelIrV10),
    V11(VerifiedCanonicalKernelIrV11),
}

fn parse_bundle(
    image: &[u8],
    target: VirtualTargetProfileV1,
) -> Result<ParsedBundleV1, SimRuntimeBackendErrorV1> {
    let (semantic, storage_kernels, component_kernels, canonical, module) =
        if VerifiedSimulationBundleV6::has_magic_prefix(image) {
            let bundle =
                VerifiedSimulationBundleV6::from_canonical_bytes(copy_bundle_image_v2(image)?)
                    .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            bundle
                .revalidate()
                .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            if bundle.target() != target.label() {
                return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                    "bundle target {} does not match backend target {}",
                    bundle.target(),
                    target.label()
                )));
            }
            let semantic = AdmittedInertSemanticMirV1::decode_current_production_canonical(
                bundle.semantic_mir(),
                SemanticMirLimitsV1::default(),
            )
            .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            let storage = SemanticStorageMapV6::from_canonical_json_bytes(bundle.storage_map())
                .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            let component = SemanticAggregateStorageMapV6::from_canonical_json_bytes(
                bundle.aggregate_storage_map(),
            )
            .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            if semantic.wire_version().as_u16() != storage.semantic_mir_version()
                || semantic.target_layout_identity().as_bytes() != storage.target_layout_identity()
            {
                return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                    "semantic MIR and V6 storage-map target identity differ".to_owned(),
                ));
            }
            let (canonical, module) =
                VerifiedCanonicalKernelIrV11::from_canonical_bytes_with_module(
                    copy_bundle_image_v2(bundle.canonical_kir_v11())?,
                )
                .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            (
                semantic,
                storage.kernels().to_vec(),
                Some(component.kernels().to_vec()),
                ParsedCanonicalKirV1::V11(canonical),
                module,
            )
        } else if VerifiedSimulationBundleV5::has_magic_prefix(image) {
            let bundle =
                VerifiedSimulationBundleV5::from_canonical_bytes(copy_bundle_image_v2(image)?)
                    .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            bundle
                .revalidate()
                .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            if bundle.target() != target.label() {
                return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                    "bundle target {} does not match backend target {}",
                    bundle.target(),
                    target.label()
                )));
            }
            let semantic = AdmittedInertSemanticMirV1::decode_current_production_canonical(
                bundle.semantic_mir(),
                SemanticMirLimitsV1::default(),
            )
            .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            let storage = SemanticStorageMapV5::from_canonical_json_bytes(bundle.storage_map())
                .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            let component = SemanticAggregateStorageMapV5::from_canonical_json_bytes(
                bundle.aggregate_storage_map(),
            )
            .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            if semantic.wire_version().as_u16() != storage.semantic_mir_version()
                || semantic.target_layout_identity().as_bytes() != storage.target_layout_identity()
            {
                return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                    "semantic MIR and V5 storage-map target identity differ".to_owned(),
                ));
            }
            let (canonical, module) =
                VerifiedCanonicalKernelIrV10::from_canonical_bytes_with_module(
                    copy_bundle_image_v2(bundle.canonical_kir_v10())?,
                )
                .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            (
                semantic,
                storage.kernels().to_vec(),
                Some(component.kernels().to_vec()),
                ParsedCanonicalKirV1::V10(canonical),
                module,
            )
        } else {
            let (bundle, component_storage) = if VerifiedSimulationBundleV4::has_magic_prefix(image)
            {
                let bundle =
                    VerifiedSimulationBundleV4::from_canonical_bytes(copy_bundle_image_v2(image)?)
                        .map_err(|error| {
                            SimRuntimeBackendErrorV1::InvalidBundle(error.to_string())
                        })?;
                bundle
                    .revalidate()
                    .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
                let storage = SemanticStorageMapV2::from_canonical_json_bytes(bundle.storage_map())
                    .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
                (bundle.into_inner_v3(), Some(storage))
            } else {
                (
                    VerifiedSimulationBundleV3::from_canonical_bytes(copy_bundle_image_v2(image)?)
                        .map_err(|error| {
                            SimRuntimeBackendErrorV1::InvalidBundle(error.to_string())
                        })?,
                    None,
                )
            };
            bundle
                .revalidate()
                .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            if bundle.inner_v2().inner_v1().target() != target.label() {
                return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                    "bundle target {} does not match backend target {}",
                    bundle.inner_v2().inner_v1().target(),
                    target.label()
                )));
            }
            let semantic = AdmittedInertSemanticMirV1::decode_current_production_canonical(
                bundle.semantic_mir(),
                SemanticMirLimitsV1::default(),
            )
            .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            let storage = SemanticStorageMapV1::from_canonical_json_bytes(bundle.storage_map())
                .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            if semantic.wire_version().as_u16() != storage.semantic_mir_version()
                || semantic.target_layout_identity().as_bytes() != storage.target_layout_identity()
            {
                return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                    "semantic MIR and storage-map target identity differ".to_owned(),
                ));
            }
            let (canonical, module) =
                VerifiedCanonicalKernelIrV7::from_canonical_bytes_with_module(
                    copy_bundle_image_v2(bundle.inner_v2().inner_v1().canonical_kir_v7())?,
                )
                .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
            (
                semantic,
                storage.kernels().to_vec(),
                component_storage.map(|storage| storage.kernels().to_vec()),
                ParsedCanonicalKirV1::V7(canonical),
                module,
            )
        };
    let mut kernels = HashMap::new();
    for storage_kernel in &storage_kernels {
        let component_kernel = component_kernels.as_ref().map(|storage| {
            storage
                .iter()
                .find(|kernel| kernel.semantic_root() == storage_kernel.semantic_root())
                .ok_or_else(|| {
                    SimRuntimeBackendErrorV1::InvalidBundle(
                        "component storage map does not cover a semantic root".to_owned(),
                    )
                })
        });
        let component_kernel = component_kernel.transpose()?;
        if let Some(component_kernel) = component_kernel
            && (component_kernel.semantic_body() != storage_kernel.semantic_body()
                || component_kernel.kir_function_ordinal() != storage_kernel.kir_function_ordinal())
        {
            return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                "V1 and V2 semantic storage kernel identities differ".to_owned(),
            ));
        }
        let root = semantic
            .functions()
            .get(storage_kernel.semantic_root() as usize)
            .ok_or_else(|| {
                SimRuntimeBackendErrorV1::InvalidBundle("semantic root out of range".to_owned())
            })?;
        let selected = semantic
            .select_kernel_body_for_root_v1(
                fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1::from_index(
                    storage_kernel.semantic_root(),
                ),
            )
            .ok_or_else(|| {
                SimRuntimeBackendErrorV1::InvalidBundle(
                    "semantic root has no kernel body".to_owned(),
                )
            })?;
        if selected.body().index() != storage_kernel.semantic_body() {
            return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                "storage map selected a different semantic body".to_owned(),
            ));
        }
        let function = module
            .functions
            .get(storage_kernel.kir_function_ordinal() as usize)
            .ok_or_else(|| {
                SimRuntimeBackendErrorV1::InvalidBundle("KIR function out of range".to_owned())
            })?;
        if function.role != FunctionRole::KernelEntry {
            return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                "storage map names a non-kernel KIR function".to_owned(),
            ));
        }
        let body = function.body.as_ref().ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidBundle("KIR kernel body is absent".to_owned())
        })?;
        let kernel = module
            .kernels
            .iter()
            .find(|kernel| kernel.entry == function.id)
            .ok_or_else(|| {
                SimRuntimeBackendErrorV1::InvalidBundle("KIR kernel entry missing".to_owned())
            })?;
        if storage_kernel.arguments().len() != root.abi().source_input_types().len() {
            return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                "source argument roster differs from semantic ABI".to_owned(),
            ));
        }
        let semantic_body = semantic
            .functions()
            .get(storage_kernel.semantic_body() as usize)
            .ok_or_else(|| {
                SimRuntimeBackendErrorV1::InvalidBundle("semantic body out of range".to_owned())
            })?;
        for item in storage_kernel.arguments() {
            let source = item.source_ordinal() as usize;
            let local = semantic_body
                .locals()
                .get(item.semantic_local() as usize)
                .ok_or_else(|| {
                    SimRuntimeBackendErrorV1::InvalidBundle(
                        "semantic argument local out of range".to_owned(),
                    )
                })?;
            if local.ty().index() != item.semantic_type()
                || root
                    .abi()
                    .source_input_types()
                    .get(source)
                    .is_none_or(|ty| ty.index() != item.semantic_type())
                || local.role() != SemanticLocalRoleV1::Argument(item.source_ordinal())
                || !ownership_matches(
                    item.ownership(),
                    root.abi().source_argument_ownership().get(source).copied(),
                )
            {
                return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                    "semantic argument type, local, or ownership was substituted".to_owned(),
                ));
            }
            if let SemanticStorageBindingV1::ExactKirParameter {
                kir_parameter_ordinal,
                kir_value_ordinal,
                representation,
            } = item.storage()
            {
                let ordinal = *kir_parameter_ordinal as usize;
                if body.parameters.get(ordinal).map(|value| value.0) != Some(*kir_value_ordinal)
                    || !representation_matches(
                        *representation,
                        function.signature.parameters.get(ordinal),
                    )
                {
                    return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                        "KIR parameter storage correspondence was substituted".to_owned(),
                    ));
                }
            }
        }
        let (arguments, explicit_byte_len, unsupported) =
            if let Some(component_kernel) = component_kernel {
                argument_layout_v2(
                    storage_kernel.arguments(),
                    component_kernel,
                    root.abi(),
                    semantic.types(),
                    &body.parameters,
                    &function.signature.parameters,
                )?
            } else {
                argument_layout(
                    storage_kernel.arguments(),
                    root.abi(),
                    semantic.types(),
                    &function.signature.parameters,
                )?
            };
        if kernels
            .insert(
                kernel.id.as_str().to_owned(),
                KernelRecordV1 {
                    module: None,
                    kernel: kernel.id.clone(),
                    signature: *root.abi().identity().as_bytes(),
                    explicit_byte_len,
                    arguments,
                    requires_dynamic_workgroup_memory: kernel_reaches_dynamic_workgroup_memory(
                        &module,
                        &kernel.entry,
                    )?,
                    unsupported,
                },
            )
            .is_some()
        {
            return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                "duplicate KIR kernel name".to_owned(),
            ));
        }
    }
    if kernels.len() != module.kernels.len() {
        return Err(SimRuntimeBackendErrorV1::InvalidBundle(
            "storage map does not cover every KIR kernel".to_owned(),
        ));
    }
    if component_kernels
        .as_ref()
        .is_some_and(|storage| storage.len() != kernels.len())
    {
        return Err(SimRuntimeBackendErrorV1::InvalidBundle(
            "component storage map has an extra semantic kernel".to_owned(),
        ));
    }
    let admitted = match canonical {
        ParsedCanonicalKirV1::V7(canonical) => AdmittedSimulationModuleV1::admit(
            canonical,
            fe2o3_kir_sim::SimulationLimitsV1::default(),
        ),
        ParsedCanonicalKirV1::V10(canonical) => AdmittedSimulationModuleV1::admit_v10(
            canonical,
            fe2o3_kir_sim::SimulationLimitsV1::default(),
        ),
        ParsedCanonicalKirV1::V11(canonical) => AdmittedSimulationModuleV1::admit_v11(
            canonical,
            fe2o3_kir_sim::SimulationLimitsV1::default(),
        ),
    }
    .map_err(|error| SimRuntimeBackendErrorV1::UnsupportedBundle(error.to_string()))?;
    Ok(ParsedBundleV1 { admitted, kernels })
}

fn copy_bundle_image_v2(image: &[u8]) -> Result<Vec<u8>, SimRuntimeBackendErrorV1> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(image.len()).map_err(|_| {
        SimRuntimeBackendErrorV1::InvalidBundle(
            "simulation bundle image allocation failed".to_owned(),
        )
    })?;
    copy.extend_from_slice(image);
    Ok(copy)
}

fn dynamic_workgroup_memory_request(
    kernel_requires_dynamic_workgroup_memory: bool,
    dynamic_shared_bytes: u32,
) -> Option<DynamicWorkgroupMemoryRequestV1> {
    (kernel_requires_dynamic_workgroup_memory || dynamic_shared_bytes != 0)
        .then(|| DynamicWorkgroupMemoryRequestV1::new(dynamic_shared_bytes))
}

fn kernel_reaches_dynamic_workgroup_memory(
    module: &fe2o3_kernel_ir::Module,
    entry: &fe2o3_kernel_ir::FunctionId,
) -> Result<bool, SimRuntimeBackendErrorV1> {
    let entry = module
        .functions
        .iter()
        .position(|function| &function.id == entry)
        .ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidBundle("kernel entry missing".to_owned())
        })?;
    let mut seen = Vec::new();
    seen.try_reserve_exact(module.functions.len())
        .map_err(|_| {
            SimRuntimeBackendErrorV1::InvalidBundle(
                "dynamic LDS reachability allocation failed".to_owned(),
            )
        })?;
    seen.resize(module.functions.len(), false);
    let mut pending = Vec::new();
    pending
        .try_reserve_exact(module.functions.len())
        .map_err(|_| {
            SimRuntimeBackendErrorV1::InvalidBundle(
                "dynamic LDS reachability allocation failed".to_owned(),
            )
        })?;
    seen[entry] = true;
    pending.push(entry);
    while let Some(index) = pending.pop() {
        let Some(body) = module.functions[index].body.as_ref() else {
            continue;
        };
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            match &operation.kind {
                OperationKind::WorkgroupMemory(memory) if memory.extent.is_dynamic() => {
                    return Ok(true);
                }
                OperationKind::Call { callee, .. } => {
                    if let Some(index) = module
                        .functions
                        .iter()
                        .position(|function| function.id == *callee)
                        && !seen[index]
                    {
                        seen[index] = true;
                        pending.push(index);
                    }
                }
                _ => {}
            }
        }
    }
    Ok(false)
}

fn argument_layout(
    storage: &[fe2o3_kernel_ir::SemanticArgumentStorageV1],
    abi: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionAbiV1,
    semantic_types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    kir_types: &[Type],
) -> Result<(Vec<ArgumentRecordV1>, usize, Option<String>), SimRuntimeBackendErrorV1> {
    let mut arguments = Vec::new();
    arguments.try_reserve_exact(storage.len()).map_err(|_| {
        SimRuntimeBackendErrorV1::InvalidBundle(
            "source argument roster allocation failed".to_owned(),
        )
    })?;
    let mut source_abi_arguments = Vec::new();
    source_abi_arguments
        .try_reserve_exact(storage.len())
        .map_err(|_| {
            SimRuntimeBackendErrorV1::InvalidBundle(
                "source ABI roster allocation failed".to_owned(),
            )
        })?;
    source_abi_arguments.extend(
        abi.arguments()
            .iter()
            .filter(|argument| argument.is_source()),
    );
    if source_abi_arguments.len() != storage.len() {
        return Err(SimRuntimeBackendErrorV1::InvalidBundle(
            "physical semantic ABI source-argument roster differs".to_owned(),
        ));
    }
    let mut next = 0usize;
    let mut maximum_alignment = 1usize;
    let mut unsupported = None;
    for item in storage {
        let source = item.source_ordinal() as usize;
        let source_ty = *abi.source_input_types().get(source).ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidBundle("source argument out of range".to_owned())
        })?;
        if source_ty.index() != item.semantic_type() {
            return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                "storage-map source type differs from semantic ABI".to_owned(),
            ));
        }
        let declaration = semantic_types
            .get(source_ty.index() as usize)
            .ok_or_else(|| {
                SimRuntimeBackendErrorV1::InvalidBundle("semantic type out of range".to_owned())
            })?;
        let size =
            usize::try_from(declaration.layout().size_bytes().unwrap_or(0)).map_err(|_| {
                SimRuntimeBackendErrorV1::UnsupportedBundle(
                    "argument size does not fit host usize".to_owned(),
                )
            })?;
        let alignment = usize::try_from(declaration.layout().alignment_bytes()).map_err(|_| {
            SimRuntimeBackendErrorV1::UnsupportedBundle(
                "argument alignment does not fit host usize".to_owned(),
            )
        })?;
        if size == 0 || alignment == 0 || !alignment.is_power_of_two() {
            unsupported.get_or_insert_with(|| {
                "unsized, zero-sized, or invalid-layout arguments are not materialized".to_owned()
            });
        }
        let offset = align_up(next, alignment.max(1))?;
        next = offset.checked_add(size).ok_or_else(|| {
            SimRuntimeBackendErrorV1::UnsupportedBundle("kernarg layout overflow".to_owned())
        })?;
        maximum_alignment = maximum_alignment.max(alignment.max(1));
        let (ordinal, representation) = match item.storage() {
            SemanticStorageBindingV1::ExactKirParameter {
                kir_parameter_ordinal,
                representation,
                ..
            } => (*kir_parameter_ordinal as usize, *representation),
            SemanticStorageBindingV1::Unavailable { .. } | SemanticStorageBindingV1::Ambiguous => {
                unsupported.get_or_insert_with(|| {
                    "argument has no exact KIR storage correspondence".to_owned()
                });
                (source, SemanticKirStorageRepresentationV1::OpaqueFlattened)
            }
        };
        let kir_ty = kir_types.get(ordinal).cloned().ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidBundle("KIR argument out of range".to_owned())
        })?;
        if ordinal != source {
            unsupported.get_or_insert_with(|| {
                "reordered or expanded KIR parameters require an explicit physical packing map"
                    .to_owned()
            });
        }
        let expected_size = match representation {
            SemanticKirStorageRepresentationV1::Scalar => {
                scalar_bytes(kir_ty.as_scalar()).unwrap_or(0)
            }
            SemanticKirStorageRepresentationV1::RegionPointer => 8,
            SemanticKirStorageRepresentationV1::RegionSlice => 16,
            SemanticKirStorageRepresentationV1::OpaqueFlattened => {
                unsupported.get_or_insert_with(|| {
                    "aggregate or capability value requires a semantic projection".to_owned()
                });
                size
            }
        };
        if size != expected_size {
            unsupported.get_or_insert_with(|| format!(
                "source argument {source} layout size {size} differs from its exact KIR representation size {expected_size}"
            ));
        }
        let physical = source_abi_arguments[source];
        if physical.value().adjusted().is_some()
            || !matches!(
                (representation, physical.mode()),
                (
                    SemanticKirStorageRepresentationV1::Scalar
                        | SemanticKirStorageRepresentationV1::RegionPointer,
                    SemanticAbiPassModeV1::Direct(_)
                ) | (
                    SemanticKirStorageRepresentationV1::RegionSlice,
                    SemanticAbiPassModeV1::Pair { .. }
                )
            )
        {
            unsupported.get_or_insert_with(|| {
                "semantic ABI pass mode has no exact simulator packing rule".to_owned()
            });
        }
        arguments.push(ArgumentRecordV1 {
            offset,
            size: if matches!(
                representation,
                SemanticKirStorageRepresentationV1::RegionSlice
            ) {
                8
            } else {
                size
            },
            ty: kir_ty,
            materialization: match representation {
                SemanticKirStorageRepresentationV1::Scalar => {
                    ArgumentMaterializationV1::ExactBytes {
                        validity: Vec::new(),
                        guards: Vec::new(),
                    }
                }
                SemanticKirStorageRepresentationV1::RegionPointer => {
                    ArgumentMaterializationV1::Region { metadata: None }
                }
                SemanticKirStorageRepresentationV1::RegionSlice => {
                    ArgumentMaterializationV1::Region {
                        metadata: Some(PhysicalSlotV1 {
                            offset: offset + 8,
                            size: 8,
                        }),
                    }
                }
                SemanticKirStorageRepresentationV1::OpaqueFlattened => {
                    ArgumentMaterializationV1::ExactBytes {
                        validity: Vec::new(),
                        guards: Vec::new(),
                    }
                }
            },
        });
    }
    if arguments.len() != kir_types.len() {
        unsupported
            .get_or_insert_with(|| "source-to-KIR argument count is not one-to-one".to_owned());
    }
    Ok((arguments, align_up(next, maximum_alignment)?, unsupported))
}

fn argument_layout_v2(
    legacy: &[fe2o3_kernel_ir::SemanticArgumentStorageV1],
    kernel_storage: &SemanticKernelStorageV2,
    abi: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionAbiV1,
    semantic_types: &[SemanticTypeDeclV1],
    kir_values: &[fe2o3_kernel_ir::ValueId],
    kir_types: &[Type],
) -> Result<(Vec<ArgumentRecordV1>, usize, Option<String>), SimRuntimeBackendErrorV1> {
    let storage = kernel_storage.arguments();
    if storage.len() != legacy.len() || storage.len() != abi.source_input_types().len() {
        return Err(SimRuntimeBackendErrorV1::InvalidBundle(
            "V2 source argument roster differs from V1 or semantic ABI".to_owned(),
        ));
    }
    let mut source_abi_arguments = Vec::new();
    source_abi_arguments
        .try_reserve_exact(storage.len())
        .map_err(|_| {
            SimRuntimeBackendErrorV1::InvalidBundle(
                "V2 source ABI roster allocation failed".to_owned(),
            )
        })?;
    source_abi_arguments.extend(
        abi.arguments()
            .iter()
            .filter(|argument| argument.is_source()),
    );
    if source_abi_arguments.len() != storage.len() {
        return Err(SimRuntimeBackendErrorV1::InvalidBundle(
            "V2 physical semantic ABI source-argument roster differs".to_owned(),
        ));
    }

    let explicit_byte_len =
        usize::try_from(kernel_storage.explicit_kernarg_bytes()).map_err(|_| {
            SimRuntimeBackendErrorV1::UnsupportedBundle(
                "V2 physical kernarg size does not fit this host".to_owned(),
            )
        })?;
    let _explicit_alignment = usize::try_from(kernel_storage.explicit_kernarg_alignment())
        .map_err(|_| {
            SimRuntimeBackendErrorV1::UnsupportedBundle(
                "V2 physical kernarg alignment does not fit this host".to_owned(),
            )
        })?;
    if explicit_byte_len > MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1 {
        return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
            "V2 physical kernarg exceeds the runtime byte limit".to_owned(),
        ));
    }
    validate_compiler_packing_plan_v2(kernel_storage, kir_types)?;

    for (source, item) in storage.iter().enumerate() {
        let legacy = legacy.get(source).ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidBundle("V1 source argument is absent".to_owned())
        })?;
        if item.source_ordinal() as usize != source
            || item.source_ordinal() != legacy.source_ordinal()
            || item.semantic_local() != legacy.semantic_local()
            || item.semantic_type() != legacy.semantic_type()
            || item.ownership() != legacy.ownership()
        {
            return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                "V2 source argument identity differs from V1".to_owned(),
            ));
        }
        let declaration = semantic_types
            .get(item.semantic_type() as usize)
            .ok_or_else(|| {
                SimRuntimeBackendErrorV1::InvalidBundle(
                    "V2 semantic argument type is out of range".to_owned(),
                )
            })?;
        if declaration.layout().is_uninhabited() {
            return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                "source argument {source} has an uninhabited Rust layout"
            )));
        }
        usize::try_from(declaration.layout().size_bytes().ok_or_else(|| {
            SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                "source argument {source} has an unsized Rust layout"
            ))
        })?)
        .map_err(|_| {
            SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                "source argument {source} size does not fit this host"
            ))
        })?;
        usize::try_from(declaration.layout().alignment_bytes()).map_err(|_| {
            SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                "source argument {source} alignment does not fit this host"
            ))
        })?;
    }
    let mut arguments = Vec::new();
    arguments.try_reserve_exact(kir_types.len()).map_err(|_| {
        SimRuntimeBackendErrorV1::InvalidBundle(
            "V2 KIR argument roster allocation failed".to_owned(),
        )
    })?;
    arguments.resize_with(kir_types.len(), || None);
    let mut unsupported = None;
    for (source, item) in storage.iter().enumerate() {
        let source_type = SemanticTypeIdV1::from_index(item.semantic_type());
        let source_declaration = semantic_types
            .get(item.semantic_type() as usize)
            .ok_or_else(|| {
                SimRuntimeBackendErrorV1::InvalidBundle(
                    "V2 semantic argument type is out of range".to_owned(),
                )
            })?;
        let source_size =
            usize::try_from(source_declaration.layout().size_bytes().ok_or_else(|| {
                SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                    "source argument {source} has an unsized Rust layout"
                ))
            })?)
            .map_err(|_| {
                SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                    "source argument {source} size does not fit this host"
                ))
            })?;
        let physical = source_abi_arguments[source];
        if physical.value().source_ty() != source_type {
            return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
                "source argument {source} physical ABI type was substituted"
            )));
        }
        let components = match item.storage() {
            SemanticComponentStorageBindingV2::ExactKirComponents { components } => components,
            SemanticComponentStorageBindingV2::Unavailable { reason } => {
                unsupported.get_or_insert_with(|| {
                    format!("source argument {source} has unavailable V2 storage: {reason:?}")
                });
                continue;
            }
            SemanticComponentStorageBindingV2::Ambiguous => {
                unsupported.get_or_insert_with(|| {
                    format!("source argument {source} has ambiguous V2 storage")
                });
                continue;
            }
        };
        if physical.value().adjusted().is_some() {
            unsupported.get_or_insert_with(|| {
                format!("source argument {source} has an adjusted ABI value")
            });
        }
        let has_slice = components.iter().any(|component| {
            component.representation() == SemanticKirComponentRepresentationV2::RegionSlice
        });
        let has_pointer = components.iter().any(|component| {
            component.representation() == SemanticKirComponentRepresentationV2::RegionPointer
        });
        let is_scalar_aggregate = matches!(
            source_declaration.shape(),
            SemanticTypeShapeV1::Unit
                | SemanticTypeShapeV1::Array { .. }
                | SemanticTypeShapeV1::Tuple(_)
                | SemanticTypeShapeV1::Aggregate(_)
        ) && !has_slice
            && !has_pointer;
        if is_scalar_aggregate {
            validate_scalar_aggregate_abi_v2(
                semantic_types,
                source_type,
                physical,
                components,
                kir_types,
                source,
                abi.canon_abi(),
                abi.spec_abi_unadjusted(),
            )?;
        }
        match physical.mode() {
            SemanticAbiPassModeV1::Ignore if source_size == 0 && components.is_empty() => {}
            SemanticAbiPassModeV1::Direct(_)
                if !has_slice && (!has_pointer || components.len() == 1) => {}
            SemanticAbiPassModeV1::Pair { .. }
                if has_slice && !has_pointer && components.len() == 1 => {}
            SemanticAbiPassModeV1::Pair { .. }
                if !has_slice && !has_pointer && components.len() == 2 => {}
            SemanticAbiPassModeV1::Cast { .. } | SemanticAbiPassModeV1::Indirect { .. }
                if is_scalar_aggregate => {}
            SemanticAbiPassModeV1::Ignore => {
                unsupported.get_or_insert_with(|| {
                    format!("source argument {source} is ABI-ignored but has retained storage")
                });
            }
            SemanticAbiPassModeV1::Cast { .. } => {
                unsupported.get_or_insert_with(|| {
                    format!("source argument {source} uses an unsupported cast ABI")
                });
            }
            SemanticAbiPassModeV1::Indirect { .. } => {
                unsupported.get_or_insert_with(|| {
                    format!("source argument {source} uses an unsupported indirect ABI")
                });
            }
            SemanticAbiPassModeV1::Direct(_) | SemanticAbiPassModeV1::Pair { .. } => {
                unsupported.get_or_insert_with(|| {
                    format!(
                        "source argument {source} physical ABI mode disagrees with its component representation"
                    )
                });
            }
        };
        for component in components {
            let ordinal = component.kir_parameter_ordinal() as usize;
            let Some(kir_ty) = kir_types.get(ordinal).cloned() else {
                return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
                    "source argument {source} component references an absent KIR parameter"
                )));
            };
            if kir_values.get(ordinal).map(|value| value.0) != Some(component.kir_value_ordinal())
                || arguments[ordinal].is_some()
            {
                return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
                    "source argument {source} component has substituted or duplicate KIR storage"
                )));
            }
            let record = match component.representation() {
                SemanticKirComponentRepresentationV2::ScalarValue => {
                    let projected = project_semantic_component_v2(
                        semantic_types,
                        source_type,
                        component.path(),
                        0,
                        components,
                        component,
                        source,
                    )?;
                    let expected = scalar_type_for_semantic_component_v2(
                        semantic_types,
                        projected.semantic_type,
                        projected.is_enum_discriminant,
                    )?;
                    if kir_ty.as_scalar() != Some(expected) {
                        return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
                            "source argument {source} component semantic and KIR scalar types differ"
                        )));
                    }
                    let scalar_size = scalar_bytes(Some(expected)).ok_or_else(|| {
                        SimRuntimeBackendErrorV1::UnsupportedBundle(
                            "semantic scalar has no concrete KIR byte width".to_owned(),
                        )
                    })?;
                    let expected_size = projected
                        .discriminant_decoder
                        .as_ref()
                        .map_or(scalar_size, |decoder| decoder.byte_width);
                    let semantic_alignment = expected_size.next_power_of_two();
                    let value_slot = validate_component_physical_slot_v2(
                        component.value_slot(),
                        expected_size,
                        semantic_alignment,
                        source,
                    )?;
                    let source_width = if projected.discriminant_decoder.is_some() {
                        expected_size
                    } else {
                        scalar_size
                    };
                    let materialization = if let Some(decoder) = projected.discriminant_decoder {
                        ArgumentMaterializationV1::EnumDiscriminant {
                            decoder,
                            guards: projected.guards,
                        }
                    } else {
                        ArgumentMaterializationV1::ExactBytes {
                            validity: semantic_component_validity_v2(
                                semantic_types,
                                projected.semantic_type,
                            )?,
                            guards: projected.guards,
                        }
                    };
                    if projected
                        .byte_offset
                        .checked_add(source_width)
                        .is_none_or(|end| end > source_size)
                    {
                        return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
                            "source argument {source} component exceeds its Rust layout"
                        )));
                    }
                    ArgumentRecordV1 {
                        offset: value_slot.offset,
                        size: value_slot.size,
                        ty: kir_ty,
                        materialization,
                    }
                }
                SemanticKirComponentRepresentationV2::RegionPointer
                | SemanticKirComponentRepresentationV2::RegionSlice => {
                    if let Err(detail) = validate_region_component_v2(
                        semantic_types,
                        source_type,
                        item.ownership(),
                        component,
                        &kir_ty,
                        physical,
                        source_size,
                    ) {
                        unsupported.get_or_insert_with(|| {
                            format!(
                                "source argument {source} has no exact region materialization: {detail}"
                            )
                        });
                    }
                    ArgumentRecordV1 {
                        offset: physical_slot_v2(component.value_slot())?.offset,
                        size: physical_slot_v2(component.value_slot())?.size,
                        ty: kir_ty,
                        materialization: ArgumentMaterializationV1::Region {
                            metadata: component
                                .metadata_slot()
                                .map(physical_slot_v2)
                                .transpose()?,
                        },
                    }
                }
            };
            arguments[ordinal] = Some(record);
        }
    }
    if arguments.iter().any(Option::is_none) && unsupported.is_none() {
        return Err(SimRuntimeBackendErrorV1::InvalidBundle(
            "V2 component storage does not cover every KIR parameter exactly once".to_owned(),
        ));
    }
    let mut materialized = Vec::new();
    materialized
        .try_reserve_exact(kir_types.len())
        .map_err(|_| {
            SimRuntimeBackendErrorV1::InvalidBundle(
                "V2 materialized argument allocation failed".to_owned(),
            )
        })?;
    materialized.extend(arguments.into_iter().flatten());
    Ok((materialized, explicit_byte_len, unsupported))
}

fn validate_compiler_packing_plan_v2(
    kernel: &SemanticKernelStorageV2,
    kir_types: &[Type],
) -> Result<(), SimRuntimeBackendErrorV1> {
    let mut components_by_ordinal = Vec::new();
    components_by_ordinal
        .try_reserve_exact(kir_types.len())
        .map_err(|_| {
            SimRuntimeBackendErrorV1::InvalidBundle(
                "compiler packing-plan index allocation failed".to_owned(),
            )
        })?;
    components_by_ordinal.resize_with(kir_types.len(), || None);
    for component in kernel
        .arguments()
        .iter()
        .filter_map(|argument| argument.storage().components())
        .flatten()
    {
        let ordinal = usize::try_from(component.kir_parameter_ordinal()).map_err(|_| {
            SimRuntimeBackendErrorV1::InvalidBundle(
                "compiler packing plan references an absent KIR parameter".to_owned(),
            )
        })?;
        let slot = components_by_ordinal.get_mut(ordinal).ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidBundle(
                "compiler packing plan references an absent KIR parameter".to_owned(),
            )
        })?;
        if slot.replace(component).is_some() {
            return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                "compiler packing plan assigns a KIR parameter more than once".to_owned(),
            ));
        }
    }

    let mut next = 0_usize;
    let mut maximum_alignment = 1_usize;
    for (ordinal, ty) in kir_types.iter().enumerate() {
        let component = components_by_ordinal[ordinal].ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidBundle(
                "compiler packing plan does not cover every KIR parameter".to_owned(),
            )
        })?;
        let (width, alignment, metadata_relative) = match ty {
            Type::Scalar(scalar) => {
                let width = scalar_bytes(Some(*scalar)).ok_or_else(|| {
                    SimRuntimeBackendErrorV1::UnsupportedBundle(
                        "KIR scalar has no exact physical width".to_owned(),
                    )
                })?;
                (width, width.next_power_of_two(), None)
            }
            Type::Pointer(_) => (8, 8, None),
            Type::Slice(_) => (8, 8, Some(8)),
            Type::Unit => {
                return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
                    "unit KIR parameters have no exact physical slot".to_owned(),
                ));
            }
        };
        next = align_up(next, alignment)?;
        let expected_value = SemanticKernargSlotV2::new(
            u32::try_from(next).map_err(|_| {
                SimRuntimeBackendErrorV1::UnsupportedBundle(
                    "compiler packing offset does not fit the V4 wire".to_owned(),
                )
            })?,
            u32::try_from(width).expect("supported scalar widths fit u32"),
            u32::try_from(alignment).expect("supported scalar alignments fit u32"),
        );
        let expected_metadata = metadata_relative
            .map(|relative| {
                Ok(SemanticKernargSlotV2::new(
                    u32::try_from(next.checked_add(relative).ok_or_else(|| {
                        SimRuntimeBackendErrorV1::UnsupportedBundle(
                            "compiler packing metadata offset overflows".to_owned(),
                        )
                    })?)
                    .map_err(|_| {
                        SimRuntimeBackendErrorV1::UnsupportedBundle(
                            "compiler packing metadata offset does not fit the V4 wire".to_owned(),
                        )
                    })?,
                    u32::try_from(width).expect("supported metadata widths fit u32"),
                    u32::try_from(alignment).expect("supported metadata alignments fit u32"),
                ))
            })
            .transpose()?;
        if component.value_slot() != expected_value
            || component.metadata_slot() != expected_metadata
        {
            return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                "V4 physical slots differ from the compiler-rederived canonical KIR packing plan"
                    .to_owned(),
            ));
        }
        next = next
            .checked_add(metadata_relative.map_or(width, |relative| relative + width))
            .ok_or_else(|| {
                SimRuntimeBackendErrorV1::UnsupportedBundle(
                    "compiler packing size overflows".to_owned(),
                )
            })?;
        maximum_alignment = maximum_alignment.max(alignment);
    }
    let total = align_up(next, maximum_alignment)?;
    if usize::try_from(kernel.explicit_kernarg_bytes()) != Ok(total)
        || usize::try_from(kernel.explicit_kernarg_alignment()) != Ok(maximum_alignment)
    {
        return Err(SimRuntimeBackendErrorV1::InvalidBundle(
            "V4 explicit kernarg extent differs from the compiler-rederived canonical plan"
                .to_owned(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecursiveArgumentLeafV2 {
    path: Vec<SemanticStorageProjectionV2>,
    semantic_type: SemanticTypeIdV1,
    byte_offset: usize,
    backend_scalar: SemanticBackendScalarV1,
}

/// Rebuilds the logical simulator leaf roster from retained compiler layout.
///
/// These leaves are not reads of an aggregate, its padding bytes, or an
/// indirect ABI carrier. Each leaf remains an independently supplied logical
/// simulator input whose identity and placement are checked below.
fn recursive_argument_leaves_v2(
    semantic_types: &[SemanticTypeDeclV1],
    source_type: SemanticTypeIdV1,
    source: usize,
) -> Result<Vec<RecursiveArgumentLeafV2>, SimRuntimeBackendErrorV1> {
    #[allow(clippy::too_many_arguments)]
    fn append(
        semantic_types: &[SemanticTypeDeclV1],
        semantic_type: SemanticTypeIdV1,
        source: usize,
        path: &mut Vec<SemanticStorageProjectionV2>,
        byte_offset: usize,
        structural_nodes: &mut usize,
        leaves: &mut Vec<RecursiveArgumentLeafV2>,
    ) -> Result<(), SimRuntimeBackendErrorV1> {
        *structural_nodes = structural_nodes.checked_add(1).ok_or_else(|| {
            SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                "source argument {source} recursive component count overflows"
            ))
        })?;
        if *structural_nodes > MAX_RECURSIVE_ARGUMENT_COMPONENTS_V2
            || path.len() > MAX_RECURSIVE_ARGUMENT_COMPONENTS_V2
            || leaves.len() > MAX_RECURSIVE_ARGUMENT_COMPONENTS_V2
        {
            return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                "source argument {source} exceeds the recursive component limit of {MAX_RECURSIVE_ARGUMENT_COMPONENTS_V2}"
            )));
        }
        let declaration = semantic_types
            .get(semantic_type.index() as usize)
            .ok_or_else(|| {
                SimRuntimeBackendErrorV1::InvalidBundle(format!(
                    "source argument {source} recursive component type is absent"
                ))
            })?;
        if declaration.layout().is_uninhabited() || declaration.layout().size_bytes().is_none() {
            return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                "source argument {source} recursive component is uninhabited or unsized"
            )));
        }
        match declaration.shape() {
            SemanticTypeShapeV1::Unit => Ok(()),
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
                let SemanticBackendReprV1::Scalar(backend_scalar) =
                    declaration.layout().backend_repr()
                else {
                    return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                        "source argument {source} recursive scalar leaf lacks a scalar backend representation"
                    )));
                };
                leaves.try_reserve(1).map_err(|_| {
                    SimRuntimeBackendErrorV1::InvalidBundle(format!(
                        "source argument {source} recursive leaf allocation failed"
                    ))
                })?;
                leaves.push(RecursiveArgumentLeafV2 {
                    path: path.clone(),
                    semantic_type,
                    byte_offset,
                    backend_scalar: *backend_scalar,
                });
                Ok(())
            }
            SemanticTypeShapeV1::Array { element, length } => {
                let SemanticFieldsShapeV1::Array {
                    stride_bytes,
                    count,
                } = declaration.layout().fields()
                else {
                    return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
                        "source argument {source} recursive array lacks exact stride evidence"
                    )));
                };
                if count != length || *length > MAX_RECURSIVE_ARGUMENT_COMPONENTS_V2 as u64 {
                    return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                        "source argument {source} recursive array length is inconsistent or exceeds the component limit"
                    )));
                }
                for index in 0..*length {
                    path.try_reserve(1).map_err(|_| {
                        SimRuntimeBackendErrorV1::InvalidBundle(format!(
                            "source argument {source} recursive path allocation failed"
                        ))
                    })?;
                    path.push(SemanticStorageProjectionV2::ArrayElement { index });
                    let relative = index.checked_mul(*stride_bytes).ok_or_else(|| {
                        SimRuntimeBackendErrorV1::InvalidBundle(format!(
                            "source argument {source} recursive array offset overflows"
                        ))
                    })?;
                    let relative = usize::try_from(relative).map_err(|_| {
                        SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                            "source argument {source} recursive array offset does not fit this host"
                        ))
                    })?;
                    let offset = byte_offset.checked_add(relative).ok_or_else(|| {
                        SimRuntimeBackendErrorV1::InvalidBundle(format!(
                            "source argument {source} recursive array offset overflows"
                        ))
                    })?;
                    append(
                        semantic_types,
                        *element,
                        source,
                        path,
                        offset,
                        structural_nodes,
                        leaves,
                    )?;
                    path.pop();
                }
                Ok(())
            }
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details()
                else {
                    return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
                        "source argument {source} recursive aggregate lacks exact field offsets"
                    )));
                };
                if fields.fields().len() != layout.field_offsets().len() {
                    return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
                        "source argument {source} recursive aggregate field roster differs from its layout"
                    )));
                }
                for (index, (&field, &relative)) in fields
                    .fields()
                    .iter()
                    .zip(layout.field_offsets())
                    .enumerate()
                {
                    path.try_reserve(1).map_err(|_| {
                        SimRuntimeBackendErrorV1::InvalidBundle(format!(
                            "source argument {source} recursive path allocation failed"
                        ))
                    })?;
                    path.push(SemanticStorageProjectionV2::Field {
                        index: u32::try_from(index).map_err(|_| {
                            SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                                "source argument {source} recursive field index does not fit the wire"
                            ))
                        })?,
                    });
                    let relative = usize::try_from(relative).map_err(|_| {
                        SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                            "source argument {source} recursive field offset does not fit this host"
                        ))
                    })?;
                    let offset = byte_offset.checked_add(relative).ok_or_else(|| {
                        SimRuntimeBackendErrorV1::InvalidBundle(format!(
                            "source argument {source} recursive field offset overflows"
                        ))
                    })?;
                    append(
                        semantic_types,
                        field,
                        source,
                        path,
                        offset,
                        structural_nodes,
                        leaves,
                    )?;
                    path.pop();
                }
                Ok(())
            }
            SemanticTypeShapeV1::Enum { .. } => {
                Err(SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                    "source argument {source} enum requires exact discriminant and variant evidence"
                )))
            }
            SemanticTypeShapeV1::Pointer(_) => {
                Err(SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                    "source argument {source} recursive aggregate contains a pointer without an owned region binding"
                )))
            }
            _ => Err(SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                "source argument {source} has no pointer-free recursive component contract"
            ))),
        }
    }

    let source_declaration = semantic_types
        .get(source_type.index() as usize)
        .ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidBundle(format!(
                "source argument {source} aggregate type is absent"
            ))
        })?;
    let source_size =
        usize::try_from(source_declaration.layout().size_bytes().ok_or_else(|| {
            SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                "source argument {source} aggregate is unsized"
            ))
        })?)
        .map_err(|_| {
            SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                "source argument {source} aggregate size does not fit this host"
            ))
        })?;
    let mut leaves = Vec::new();
    leaves
        .try_reserve(MAX_RECURSIVE_ARGUMENT_COMPONENTS_V2.min(8))
        .map_err(|_| {
            SimRuntimeBackendErrorV1::InvalidBundle(format!(
                "source argument {source} recursive leaf allocation failed"
            ))
        })?;
    let mut structural_nodes = 0;
    append(
        semantic_types,
        source_type,
        source,
        &mut Vec::new(),
        0,
        &mut structural_nodes,
        &mut leaves,
    )?;
    let mut ranges = Vec::new();
    ranges.try_reserve_exact(leaves.len()).map_err(|_| {
        SimRuntimeBackendErrorV1::InvalidBundle(format!(
            "source argument {source} recursive range allocation failed"
        ))
    })?;
    for leaf in &leaves {
        let width = usize::try_from(leaf.backend_scalar.primitive().size_bytes().ok_or_else(
            || {
                SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                    "source argument {source} recursive scalar has no exact byte width"
                ))
            },
        )?)
        .map_err(|_| {
            SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                "source argument {source} recursive scalar width does not fit this host"
            ))
        })?;
        let end = leaf.byte_offset.checked_add(width).ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidBundle(format!(
                "source argument {source} recursive scalar range overflows"
            ))
        })?;
        if end > source_size {
            return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
                "source argument {source} recursive scalar exceeds its Rust layout"
            )));
        }
        ranges.push((leaf.byte_offset, end));
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
            "source argument {source} recursive scalar leaves overlap"
        )));
    }
    Ok(leaves)
}

// Keep the independently retained source, layout, physical ABI, and canonical
// ABI axes explicit at this consumer trust boundary.
#[allow(clippy::too_many_arguments)]
fn validate_scalar_aggregate_abi_v2(
    semantic_types: &[SemanticTypeDeclV1],
    source_type: SemanticTypeIdV1,
    physical: &SemanticAbiArgumentV1,
    components: &[fe2o3_kernel_ir::SemanticKirComponentStorageV2],
    kir_types: &[Type],
    source: usize,
    canon_abi: SemanticCanonAbiV1,
    spec_abi_unadjusted: bool,
) -> Result<(), SimRuntimeBackendErrorV1> {
    let source_declaration = semantic_types
        .get(source_type.index() as usize)
        .ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidBundle(format!(
                "source argument {source} aggregate type is absent"
            ))
        })?;
    let source_size = source_declaration.layout().size_bytes();
    let leaves = recursive_argument_leaves_v2(semantic_types, source_type, source)?;
    if leaves.len() != components.len() {
        return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
            "source argument {source} recursive component roster is incomplete"
        )));
    }
    for (index, (leaf, component)) in leaves.iter().zip(components).enumerate() {
        if component.representation() != SemanticKirComponentRepresentationV2::ScalarValue
            || component.metadata_slot().is_some()
            || component.path() != leaf.path
            || index != 0
                && component.kir_parameter_ordinal()
                    != components[index - 1]
                        .kir_parameter_ordinal()
                        .checked_add(1)
                        .ok_or_else(|| {
                            SimRuntimeBackendErrorV1::InvalidBundle(format!(
                                "source argument {source} recursive KIR ordinal overflows"
                            ))
                        })?
        {
            return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
                "source argument {source} recursive component order or representation differs"
            )));
        }
        let projected = project_semantic_component_v2(
            semantic_types,
            source_type,
            component.path(),
            0,
            components,
            component,
            source,
        )?;
        if projected.is_enum_discriminant
            || projected.semantic_type != leaf.semantic_type
            || projected.byte_offset != leaf.byte_offset
        {
            return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
                "source argument {source} recursive component offset or identity differs"
            )));
        }
        validate_aggregate_abi_leaf_v2(
            semantic_types,
            leaf.semantic_type,
            leaf.backend_scalar,
            component,
            kir_types,
            source,
        )?;
    }
    match (
        physical.mode(),
        source_declaration.layout().backend_repr(),
        leaves.as_slice(),
    ) {
        (SemanticAbiPassModeV1::Ignore, SemanticBackendReprV1::Memory { sized: true }, [])
            if source_size == Some(0) =>
        {
            Ok(())
        }
        (SemanticAbiPassModeV1::Direct(_), SemanticBackendReprV1::Scalar(expected), [leaf]) => {
            if leaf.byte_offset != 0 || leaf.backend_scalar != *expected {
                return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
                    "source argument {source} direct aggregate projection differs from the compiler ABI"
                )));
            }
            Ok(())
        }
        (
            SemanticAbiPassModeV1::Direct(attributes),
            SemanticBackendReprV1::Memory { sized: true },
            [_, ..],
        ) if spec_abi_unadjusted
            && *attributes
                == fe2o3_mir_model::semantic_mir_v1::SemanticAbiValueAttributesV1::plain()
            && source_size.is_some_and(|size| size != 0) =>
        {
            Ok(())
        }
        (
            SemanticAbiPassModeV1::Pair { .. },
            SemanticBackendReprV1::ScalarPair { first, second },
            [first_leaf, second_leaf],
        ) => {
            let second_offset =
                usize::try_from(first.primitive().size_bytes().ok_or_else(|| {
                    SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                        "source argument {source} first pair component has no exact ABI width"
                    ))
                })?)
                .map_err(|_| {
                    SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                        "source argument {source} pair component width does not fit this host"
                    ))
                })?;
            let second_alignment =
                usize::try_from(second.primitive().alignment_bytes()).map_err(|_| {
                    SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
                        "source argument {source} pair component alignment does not fit this host"
                    ))
                })?;
            let second_offset = align_up(second_offset, second_alignment)?;
            if first_leaf.byte_offset != 0
                || second_leaf.byte_offset != second_offset
                || first_leaf.backend_scalar != *first
                || second_leaf.backend_scalar != *second
            {
                return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
                    "source argument {source} pair projection order differs from the compiler ABI"
                )));
            }
            Ok(())
        }
        (
            SemanticAbiPassModeV1::Cast { pad_i32, cast },
            SemanticBackendReprV1::Memory { sized: true },
            [_, ..],
        ) if matches!(
            canon_abi,
            SemanticCanonAbiV1::Rust
                | SemanticCanonAbiV1::RustCold
                | SemanticCanonAbiV1::RustPreserveNone
        ) && source_size.is_some_and(|size| size != 0)
            && !pad_i32
            && cast.prefix().iter().all(Option::is_none)
            && cast.rest_offset_bytes().is_none()
            && cast.rest().unit().kind() == SemanticAbiRegisterKindV1::Integer
            && source_size == Some(cast.rest().unit().size_bytes())
            && source_size == Some(cast.rest_total_bytes())
            && !cast.rest_consecutive()
            && !cast.attributes().regular().no_alias()
            && cast.attributes().regular().pointer_capture().is_none()
            && !cast.attributes().regular().non_null()
            && !cast.attributes().regular().read_only()
            && !cast.attributes().regular().in_register()
            && cast.attributes().extension() == SemanticAbiExtensionV1::None
            && cast.attributes().pointee_size_bytes() == 0
            && cast.attributes().pointee_alignment_bytes().is_none() =>
        {
            Ok(())
        }
        (
            SemanticAbiPassModeV1::Indirect {
                attributes,
                metadata_attributes,
                on_stack,
            },
            SemanticBackendReprV1::Memory { sized: true },
            [_, ..],
        ) if source_size.is_some_and(|size| size != 0)
            && metadata_attributes.is_none()
            && !on_stack
            && attributes.regular().no_alias()
            && matches!(
                attributes.regular().pointer_capture(),
                Some(
                    SemanticAbiPointerCaptureV1::CapturesAddress
                        | SemanticAbiPointerCaptureV1::CapturesNone
                )
            )
            && attributes.regular().non_null()
            && attributes.regular().no_undef()
            && attributes.extension() == SemanticAbiExtensionV1::None
            && attributes.pointee_size_bytes()
                == source_declaration.layout().rustc_size_bytes()
            && attributes.pointee_alignment_bytes()
                == Some(source_declaration.layout().alignment_bytes()) =>
        {
            Ok(())
        }
        _ => Err(SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
            "source argument {source} aggregate ABI has no exact recursive logical-component rule"
        ))),
    }
}

fn validate_aggregate_abi_leaf_v2(
    semantic_types: &[SemanticTypeDeclV1],
    semantic_type: SemanticTypeIdV1,
    expected: fe2o3_mir_model::semantic_mir_v1::SemanticBackendScalarV1,
    component: &fe2o3_kernel_ir::SemanticKirComponentStorageV2,
    kir_types: &[Type],
    source: usize,
) -> Result<(), SimRuntimeBackendErrorV1> {
    let declaration = semantic_types
        .get(semantic_type.index() as usize)
        .ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidBundle(format!(
                "source argument {source} aggregate leaf type is absent"
            ))
        })?;
    let SemanticBackendReprV1::Scalar(actual) = declaration.layout().backend_repr() else {
        return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(format!(
            "source argument {source} aggregate ABI leaf is not a scalar"
        )));
    };
    let ordinal = component.kir_parameter_ordinal() as usize;
    let expected_kir = scalar_type_for_semantic_component_v2(semantic_types, semantic_type, false)?;
    if actual != &expected || kir_types.get(ordinal).and_then(Type::as_scalar) != Some(expected_kir)
    {
        return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
            "source argument {source} aggregate leaf differs from the compiler ABI component"
        )));
    }
    Ok(())
}

fn validate_region_component_v2(
    semantic_types: &[SemanticTypeDeclV1],
    semantic_type: SemanticTypeIdV1,
    ownership: SemanticArgumentOwnershipV1,
    component: &fe2o3_kernel_ir::SemanticKirComponentStorageV2,
    kir_type: &Type,
    physical: &SemanticAbiArgumentV1,
    source_size: usize,
) -> Result<(), String> {
    if !component.path().is_empty() {
        return Err("embedded pointer components have no owned runtime region binding".to_owned());
    }
    if physical.value().pointee_override().is_some() {
        return Err("semantic ABI pointee override has no exact region rule".to_owned());
    }
    let declaration = semantic_types
        .get(semantic_type.index() as usize)
        .ok_or_else(|| "semantic region source type is absent".to_owned())?;
    let (
        semantic_element,
        kir_element,
        kir_address_space,
        kir_access,
        expected_access,
        expected_size,
        mode_ok,
    ) = match declaration.shape() {
        SemanticTypeShapeV1::Pointer(pointer) => {
            if pointer.pointer_width_bits() != 64 {
                return Err(
                    "semantic pointer width is not the runtime target width of 64 bits".to_owned(),
                );
            }
            if !matches!(pointer.address_space(), 0 | 1) {
                return Err("semantic pointer address space is not device-global".to_owned());
            }
            let expected_access = match pointer.mutability() {
                SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
                SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
            };
            let ownership_matches_pointer = matches!(
                (ownership, pointer.kind(), pointer.mutability()),
                (
                    SemanticArgumentOwnershipV1::SharedBorrow,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable
                ) | (
                    SemanticArgumentOwnershipV1::UniqueBorrow,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Mutable
                ) | (
                    SemanticArgumentOwnershipV1::RawPointer,
                    SemanticPointerKindV1::Raw,
                    _
                ) | (
                    SemanticArgumentOwnershipV1::ExclusiveOwner,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable
                )
            );
            if !ownership_matches_pointer {
                return Err(
                    "semantic pointer kind, mutability, and source ownership disagree".to_owned(),
                );
            }
            match (component.representation(), pointer.metadata(), kir_type) {
                (
                    SemanticKirComponentRepresentationV2::RegionPointer,
                    SemanticPointerMetadataV1::None,
                    Type::Pointer(kir),
                ) => (
                    pointer.pointee(),
                    kir.pointee.as_ref(),
                    kir.address_space,
                    kir.access,
                    expected_access,
                    8,
                    matches!(physical.mode(), SemanticAbiPassModeV1::Direct(_)),
                ),
                (
                    SemanticKirComponentRepresentationV2::RegionSlice,
                    SemanticPointerMetadataV1::SliceLength,
                    Type::Slice(kir),
                ) => {
                    let pointee = semantic_types
                        .get(pointer.pointee().index() as usize)
                        .ok_or_else(|| "semantic slice pointee type is absent".to_owned())?;
                    let SemanticTypeShapeV1::Slice { element } = pointee.shape() else {
                        return Err(
                            "slice-length pointer metadata does not reference a semantic slice"
                                .to_owned(),
                        );
                    };
                    (
                        *element,
                        kir.element.as_ref(),
                        kir.address_space,
                        kir.access,
                        expected_access,
                        16,
                        matches!(physical.mode(), SemanticAbiPassModeV1::Pair { .. }),
                    )
                }
                (SemanticKirComponentRepresentationV2::RegionPointer, _, _) => {
                    return Err(
                    "region-pointer representation disagrees with semantic metadata or KIR type"
                        .to_owned(),
                );
                }
                (SemanticKirComponentRepresentationV2::RegionSlice, _, _) => {
                    return Err(
                        "region-slice representation disagrees with semantic metadata or KIR type"
                            .to_owned(),
                    );
                }
                (SemanticKirComponentRepresentationV2::ScalarValue, _, _) => {
                    return Err("scalar component was dispatched as a region".to_owned());
                }
            }
        }
        SemanticTypeShapeV1::Aggregate(fields) => {
            let semantic_element = validate_owned_slice_wrapper_v2(
                semantic_types,
                declaration,
                fields.fields(),
                ownership,
            )?;
            let Type::Slice(kir) = kir_type else {
                return Err("owned slice wrapper does not map to a KIR slice".to_owned());
            };
            if component.representation() != SemanticKirComponentRepresentationV2::RegionSlice {
                return Err(
                    "owned slice wrapper requires a whole-value region-slice component".to_owned(),
                );
            }
            let expected_access = match ownership {
                SemanticArgumentOwnershipV1::SharedBorrow => AccessMode::ReadOnly,
                SemanticArgumentOwnershipV1::UniqueBorrow => AccessMode::ReadWrite,
                SemanticArgumentOwnershipV1::ExclusiveOwner => kir.access,
                _ => {
                    return Err(
                        "owned slice wrapper requires shared, unique, or exclusive ownership"
                            .to_owned(),
                    );
                }
            };
            if ownership == SemanticArgumentOwnershipV1::ExclusiveOwner
                && !matches!(kir.access, AccessMode::ReadWrite | AccessMode::WriteOnly)
            {
                return Err("exclusive slice ownership requires mutable KIR access".to_owned());
            }
            (
                semantic_element,
                kir.element.as_ref(),
                kir.address_space,
                kir.access,
                expected_access,
                16,
                matches!(physical.mode(), SemanticAbiPassModeV1::Pair { .. }),
            )
        }
        _ => {
            return Err(
                "semantic source type is neither an exact pointer nor an owned slice wrapper"
                    .to_owned(),
            );
        }
    };
    if kir_address_space != AddressSpace::Global || kir_access != expected_access {
        return Err(
            "semantic address space or mutability disagrees with the KIR region".to_owned(),
        );
    }
    let expected_element =
        scalar_type_for_semantic_component_v2(semantic_types, semantic_element, false).map_err(
            |_| "semantic region element has no exact scalar KIR representation".to_owned(),
        )?;
    if kir_element != &Type::Scalar(expected_element) {
        return Err("semantic pointee element disagrees with the KIR region element".to_owned());
    }
    if source_size != expected_size || !mode_ok {
        return Err(
            "semantic source layout and physical ABI mode are not exact for the region".to_owned(),
        );
    }
    let value = physical_slot_v2(component.value_slot()).map_err(|error| error.to_string())?;
    if value.size != 8 || component.value_slot().byte_alignment() != 8 {
        return Err("region pointer slot differs from the target pointer ABI".to_owned());
    }
    let metadata = component.metadata_slot();
    match component.representation() {
        SemanticKirComponentRepresentationV2::RegionPointer if metadata.is_none() => {}
        SemanticKirComponentRepresentationV2::RegionSlice => {
            let metadata =
                metadata.ok_or_else(|| "region slice metadata slot is absent".to_owned())?;
            let metadata = physical_slot_v2(metadata).map_err(|error| error.to_string())?;
            if metadata.size != 8
                || component
                    .metadata_slot()
                    .is_none_or(|slot| slot.byte_alignment() != 8)
            {
                return Err(
                    "region slice metadata slot differs from the target usize ABI".to_owned(),
                );
            }
        }
        _ => return Err("region metadata slot disagrees with its representation".to_owned()),
    }
    Ok(())
}

fn validate_owned_slice_wrapper_v2(
    semantic_types: &[SemanticTypeDeclV1],
    declaration: &SemanticTypeDeclV1,
    fields: &[SemanticTypeIdV1],
    ownership: SemanticArgumentOwnershipV1,
) -> Result<SemanticTypeIdV1, String> {
    let [pointer_field, length_field, marker_field] = fields else {
        return Err(
            "owned slice wrapper must contain pointer, length, and marker fields".to_owned(),
        );
    };
    let layout = declaration.layout();
    if layout.size_bytes() != Some(16)
        || layout.alignment_bytes() != 8
        || layout.is_uninhabited()
        || !matches!(
            layout.variants(),
            SemanticRustcVariantsV1::Single { index: 0 }
        )
        || declaration.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
    {
        return Err("owned slice wrapper layout is not the exact 64-bit pair ABI".to_owned());
    }
    let exact_fields = matches!(
        layout.fields(),
        SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes,
            memory_order_source_indices,
        } if source_order_offsets_bytes.as_ref() == [0, 8, 16]
            && memory_order_source_indices.as_ref() == [0, 1, 2]
    ) && matches!(
        layout.details(),
        SemanticTypeLayoutDetailsV1::Aggregate(aggregate)
            if aggregate.field_offsets() == [0, 8, 16] && aggregate.padding().is_empty()
    );
    let full_u64_range = SemanticScalarValidityRangeV1::new(0, u64::MAX.into());
    let exact_backend_pair = matches!(
        layout.backend_repr(),
        SemanticBackendReprV1::ScalarPair {
            first: SemanticBackendScalarV1::Initialized {
                primitive: SemanticBackendPrimitiveV1::Pointer {
                    address_space: 0,
                    size_bytes: 8,
                    alignment_bytes: 8,
                },
                valid_range: first_range,
            },
            second: SemanticBackendScalarV1::Initialized {
                primitive: SemanticBackendPrimitiveV1::Integer {
                    signed: false,
                    bits: 64,
                    alignment_bytes: 8,
                },
                valid_range: second_range,
            },
        } if *first_range == full_u64_range && *second_range == full_u64_range
    );
    let properties = declaration.abi_properties();
    let exact_pointer_evidence = properties.rustc_layout_is_noundef()
        && !properties.pass_indirectly_in_non_rustic_abis()
        && !properties.has_unsized_foreign_tail()
        && properties.second_pointee().is_none()
        && properties.first_pointee().is_some_and(|pointee| {
            pointee.kind() == SemanticAbiPointeeKindV1::Raw
                && pointee.guaranteed_size_bytes() == 0
                && pointee.reliable_alignment_bytes() == 1
        });
    if !exact_fields || !exact_backend_pair || !exact_pointer_evidence {
        return Err("owned slice wrapper lacks exact compiler pointer-pair evidence".to_owned());
    }
    let pointer = semantic_types
        .get(pointer_field.index() as usize)
        .ok_or_else(|| "owned slice wrapper pointer field type is absent".to_owned())?;
    let SemanticTypeShapeV1::Pointer(pointer) = pointer.shape() else {
        return Err("owned slice wrapper first field is not a pointer".to_owned());
    };
    if pointer.pointer_width_bits() != 64
        || pointer.address_space() != 0
        || pointer.kind() != SemanticPointerKindV1::Raw
        || pointer.mutability() != SemanticMutabilityV1::Mutable
        || pointer.metadata() != SemanticPointerMetadataV1::None
    {
        return Err("owned slice wrapper pointer field contract changed".to_owned());
    }
    let length = semantic_types
        .get(length_field.index() as usize)
        .ok_or_else(|| "owned slice wrapper length field type is absent".to_owned())?;
    if !matches!(
        length.shape(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        })
    ) {
        return Err("owned slice wrapper length field is not target usize".to_owned());
    }
    let marker = semantic_types
        .get(marker_field.index() as usize)
        .ok_or_else(|| "owned slice wrapper marker field type is absent".to_owned())?;
    if marker.layout().size_bytes() != Some(0) || marker.layout().is_uninhabited() {
        return Err("owned slice wrapper marker field is not inhabited and zero-sized".to_owned());
    }
    if !matches!(
        ownership,
        SemanticArgumentOwnershipV1::SharedBorrow
            | SemanticArgumentOwnershipV1::UniqueBorrow
            | SemanticArgumentOwnershipV1::ExclusiveOwner
    ) {
        return Err("owned slice wrapper ownership is not borrow-checked".to_owned());
    }
    Ok(pointer.pointee())
}

fn physical_slot_v2(
    slot: SemanticKernargSlotV2,
) -> Result<PhysicalSlotV1, SimRuntimeBackendErrorV1> {
    Ok(PhysicalSlotV1 {
        offset: usize::try_from(slot.byte_offset()).map_err(|_| {
            SimRuntimeBackendErrorV1::UnsupportedBundle(
                "physical kernarg offset does not fit this host".to_owned(),
            )
        })?,
        size: usize::try_from(slot.byte_width()).map_err(|_| {
            SimRuntimeBackendErrorV1::UnsupportedBundle(
                "physical kernarg width does not fit this host".to_owned(),
            )
        })?,
    })
}

fn validate_component_physical_slot_v2(
    slot: SemanticKernargSlotV2,
    expected_size: usize,
    expected_alignment: usize,
    source: usize,
) -> Result<PhysicalSlotV1, SimRuntimeBackendErrorV1> {
    let physical = physical_slot_v2(slot)?;
    if physical.size != expected_size
        || usize::try_from(slot.byte_alignment()).ok() != Some(expected_alignment)
    {
        return Err(SimRuntimeBackendErrorV1::InvalidBundle(format!(
            "source argument {source} component physical slot has the wrong ABI width or alignment"
        )));
    }
    Ok(physical)
}

struct ProjectedSemanticComponentV2 {
    semantic_type: SemanticTypeIdV1,
    byte_offset: usize,
    guards: Vec<EnumVariantGuardV1>,
    discriminant_decoder: Option<EnumDecoderV1>,
    is_enum_discriminant: bool,
}

fn project_semantic_component_v2(
    types: &[SemanticTypeDeclV1],
    root: SemanticTypeIdV1,
    path: &[SemanticStorageProjectionV2],
    root_offset: usize,
    components: &[fe2o3_kernel_ir::SemanticKirComponentStorageV2],
    current_component: &fe2o3_kernel_ir::SemanticKirComponentStorageV2,
    source: usize,
) -> Result<ProjectedSemanticComponentV2, SimRuntimeBackendErrorV1> {
    let mut semantic_type = root;
    let mut byte_offset = root_offset;
    let mut guards = Vec::new();
    guards.try_reserve_exact(path.len()).map_err(|_| {
        SimRuntimeBackendErrorV1::InvalidBundle("component enum-guard allocation failed".to_owned())
    })?;
    let mut selected_variant = None;
    for (position, projection) in path.iter().enumerate() {
        let declaration = types.get(semantic_type.index() as usize).ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidBundle(
                "component path references an absent semantic type".to_owned(),
            )
        })?;
        match *projection {
            SemanticStorageProjectionV2::Field { index } => {
                let (field_type, relative) = match declaration.shape() {
                    SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                        let layout = match declaration.layout().details() {
                            SemanticTypeLayoutDetailsV1::Aggregate(layout) => layout,
                            SemanticTypeLayoutDetailsV1::None => {
                                return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                                    "aggregate component has no exact field layout".to_owned(),
                                ));
                            }
                        };
                        let field_type = *fields.fields().get(index as usize).ok_or_else(|| {
                            SimRuntimeBackendErrorV1::InvalidBundle(
                                "aggregate component field is out of range".to_owned(),
                            )
                        })?;
                        let offset =
                            *layout.field_offsets().get(index as usize).ok_or_else(|| {
                                SimRuntimeBackendErrorV1::InvalidBundle(
                                    "aggregate component field offset is absent".to_owned(),
                                )
                            })?;
                        (field_type, offset)
                    }
                    SemanticTypeShapeV1::Enum { variants, .. } => {
                        let variant = selected_variant.take().ok_or_else(|| {
                            SimRuntimeBackendErrorV1::InvalidBundle(
                                "enum payload field has no preceding variant projection".to_owned(),
                            )
                        })?;
                        let fields = variants.get(variant as usize).ok_or_else(|| {
                            SimRuntimeBackendErrorV1::InvalidBundle(
                                "enum payload variant is out of range".to_owned(),
                            )
                        })?;
                        let field_type =
                            *fields
                                .fields()
                                .fields()
                                .get(index as usize)
                                .ok_or_else(|| {
                                    SimRuntimeBackendErrorV1::InvalidBundle(
                                        "enum payload field is out of range".to_owned(),
                                    )
                                })?;
                        let aggregate = match declaration.layout().variants() {
                            SemanticRustcVariantsV1::Multiple(layout) => layout
                                .variants()
                                .get(variant as usize)
                                .ok_or_else(|| {
                                    SimRuntimeBackendErrorV1::InvalidBundle(
                                        "enum payload layout variant is out of range".to_owned(),
                                    )
                                })?
                                .aggregate(),
                            SemanticRustcVariantsV1::Single { index: retained }
                                if *retained == variant =>
                            {
                                match declaration.layout().details() {
                                    SemanticTypeLayoutDetailsV1::Aggregate(aggregate) => aggregate,
                                    SemanticTypeLayoutDetailsV1::None => {
                                        return Err(
                                            SimRuntimeBackendErrorV1::UnsupportedBundle(
                                                "single-variant enum payload has no retained field layout"
                                                    .to_owned(),
                                            ),
                                        );
                                    }
                                }
                            }
                            SemanticRustcVariantsV1::Single { .. } => {
                                return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                                    "enum payload projection selects a non-retained single variant"
                                        .to_owned(),
                                ));
                            }
                            SemanticRustcVariantsV1::Empty => {
                                return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
                                    "uninhabited enum has no payload layout".to_owned(),
                                ));
                            }
                        };
                        let offset =
                            *aggregate
                                .field_offsets()
                                .get(index as usize)
                                .ok_or_else(|| {
                                    SimRuntimeBackendErrorV1::InvalidBundle(
                                        "enum payload field offset is absent".to_owned(),
                                    )
                                })?;
                        (field_type, offset)
                    }
                    _ => {
                        return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                            "field projection does not name an aggregate".to_owned(),
                        ));
                    }
                };
                let relative = usize::try_from(relative).map_err(|_| {
                    SimRuntimeBackendErrorV1::UnsupportedBundle(
                        "component field byte offset does not fit this host".to_owned(),
                    )
                })?;
                byte_offset = byte_offset.checked_add(relative).ok_or_else(|| {
                    SimRuntimeBackendErrorV1::InvalidBundle(
                        "component field byte offset overflow".to_owned(),
                    )
                })?;
                semantic_type = field_type;
            }
            SemanticStorageProjectionV2::ArrayElement { index } => {
                let SemanticTypeShapeV1::Array { element, length } = declaration.shape() else {
                    return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                        "array projection does not name an array".to_owned(),
                    ));
                };
                if index >= *length {
                    return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                        "array component index is out of range".to_owned(),
                    ));
                }
                let fe2o3_mir_model::semantic_mir_v1::SemanticFieldsShapeV1::Array {
                    stride_bytes,
                    count,
                } = declaration.layout().fields()
                else {
                    return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                        "array component has no exact stride layout".to_owned(),
                    ));
                };
                if count != length {
                    return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                        "array semantic and layout lengths differ".to_owned(),
                    ));
                }
                let relative = index.checked_mul(*stride_bytes).ok_or_else(|| {
                    SimRuntimeBackendErrorV1::InvalidBundle(
                        "array component byte offset overflow".to_owned(),
                    )
                })?;
                let relative = usize::try_from(relative).map_err(|_| {
                    SimRuntimeBackendErrorV1::UnsupportedBundle(
                        "array component byte offset does not fit this host".to_owned(),
                    )
                })?;
                byte_offset = byte_offset.checked_add(relative).ok_or_else(|| {
                    SimRuntimeBackendErrorV1::InvalidBundle(
                        "array component byte offset does not fit this host".to_owned(),
                    )
                })?;
                semantic_type = *element;
            }
            SemanticStorageProjectionV2::EnumVariant { index } => {
                let SemanticTypeShapeV1::Enum { variants, .. } = declaration.shape() else {
                    return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                        "enum-variant projection does not name an enum".to_owned(),
                    ));
                };
                if selected_variant.is_some() || variants.get(index as usize).is_none() {
                    return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                        "enum-variant projection is duplicate or out of range".to_owned(),
                    ));
                }
                let mut discriminants = components.iter().filter(|candidate| {
                    candidate.representation() == SemanticKirComponentRepresentationV2::ScalarValue
                        && candidate.path().len() == position + 1
                        && candidate.path()[..position] == path[..position]
                        && candidate.path()[position]
                            == SemanticStorageProjectionV2::EnumDiscriminant
                });
                let discriminant = match (discriminants.next(), discriminants.next()) {
                    (Some(component), None) => component,
                    _ => {
                        return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
                            "enum payload requires one exact physical discriminant component"
                                .to_owned(),
                        ));
                    }
                };
                let decoder = physical_enum_decoder_v2(
                    enum_decoder_v2(types, semantic_type, byte_offset)?,
                    discriminant.value_slot(),
                    source,
                )?;
                guards.push(EnumVariantGuardV1 {
                    decoder,
                    required_variant: index,
                });
                selected_variant = Some(index);
            }
            SemanticStorageProjectionV2::EnumDiscriminant => {
                if position + 1 != path.len() || selected_variant.is_some() {
                    return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                        "enum-discriminant projection must be a terminal enum projection"
                            .to_owned(),
                    ));
                }
                let SemanticTypeShapeV1::Enum { discriminant, .. } = declaration.shape() else {
                    return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                        "enum-discriminant projection does not name an enum".to_owned(),
                    ));
                };
                let semantic_decoder = enum_decoder_v2(types, semantic_type, byte_offset)?;
                let semantic_byte_offset = semantic_decoder.byte_offset;
                let discriminant_decoder = physical_enum_decoder_v2(
                    semantic_decoder,
                    current_component.value_slot(),
                    source,
                )?;
                return Ok(ProjectedSemanticComponentV2 {
                    semantic_type: *discriminant,
                    byte_offset: semantic_byte_offset,
                    guards,
                    discriminant_decoder: Some(discriminant_decoder),
                    is_enum_discriminant: true,
                });
            }
        }
    }
    if selected_variant.is_some() {
        return Err(SimRuntimeBackendErrorV1::InvalidBundle(
            "enum-variant projection has no payload field".to_owned(),
        ));
    }
    Ok(ProjectedSemanticComponentV2 {
        semantic_type,
        byte_offset,
        guards,
        discriminant_decoder: None,
        is_enum_discriminant: false,
    })
}

fn scalar_type_for_semantic_component_v2(
    types: &[SemanticTypeDeclV1],
    semantic_type: SemanticTypeIdV1,
    _is_enum_discriminant: bool,
) -> Result<ScalarType, SimRuntimeBackendErrorV1> {
    let declaration = types.get(semantic_type.index() as usize).ok_or_else(|| {
        SimRuntimeBackendErrorV1::InvalidBundle(
            "component scalar semantic type is absent".to_owned(),
        )
    })?;
    let scalar = match declaration.shape() {
        SemanticTypeShapeV1::Scalar(scalar) => *scalar,
        SemanticTypeShapeV1::ValidityScalar(validity) => validity.scalar(),
        SemanticTypeShapeV1::Pointer(_) => {
            return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
                "by-value aggregate pointer components are not deserialized".to_owned(),
            ));
        }
        _ if declaration.layout().size_bytes() == Some(0) => {
            return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                "zero-sized semantic leaves must not name KIR parameters".to_owned(),
            ));
        }
        _ => {
            return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
                "component path does not terminate at a scalar Rust value".to_owned(),
            ));
        }
    };
    match scalar {
        SemanticScalarTypeV1::Bool => Ok(ScalarType::Bool),
        SemanticScalarTypeV1::Char => Ok(ScalarType::U32),
        SemanticScalarTypeV1::Integer { signed, bits } => match (signed, bits) {
            (true, 8) => Ok(ScalarType::I8),
            (true, 16) => Ok(ScalarType::I16),
            (true, 32) => Ok(ScalarType::I32),
            (true, 64) => Ok(ScalarType::I64),
            (true, 128) => Ok(ScalarType::I128),
            (false, 8) => Ok(ScalarType::U8),
            (false, 16) => Ok(ScalarType::U16),
            (false, 32) => Ok(ScalarType::U32),
            (false, 64) => Ok(ScalarType::U64),
            (false, 128) => Ok(ScalarType::U128),
            _ => Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
                "semantic integer component width is unsupported".to_owned(),
            )),
        },
        SemanticScalarTypeV1::Float { bits: 16 } => Ok(ScalarType::F16),
        SemanticScalarTypeV1::Float { bits: 32 } => Ok(ScalarType::F32),
        SemanticScalarTypeV1::Float { bits: 64 } => Ok(ScalarType::F64),
        SemanticScalarTypeV1::Float { .. } => Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
            "semantic floating component width is unsupported".to_owned(),
        )),
    }
}

fn semantic_component_validity_v2(
    types: &[SemanticTypeDeclV1],
    semantic_type: SemanticTypeIdV1,
) -> Result<Vec<SemanticScalarValidityRangeV1>, SimRuntimeBackendErrorV1> {
    let Some(declaration) = types.get(semantic_type.index() as usize) else {
        return Ok(Vec::new());
    };
    let ranges = match declaration.shape() {
        SemanticTypeShapeV1::ValidityScalar(validity) => validity.valid_ranges(),
        _ => &[],
    };
    let backend = match declaration.layout().backend_repr() {
        SemanticBackendReprV1::Scalar(scalar) if ranges.is_empty() => scalar.valid_range(),
        _ => None,
    };
    let count = if ranges.is_empty() {
        usize::from(backend.is_some())
    } else {
        ranges.len()
    };
    let mut output = Vec::new();
    output.try_reserve_exact(count).map_err(|_| {
        SimRuntimeBackendErrorV1::InvalidBundle("semantic validity allocation failed".to_owned())
    })?;
    if ranges.is_empty() {
        output.extend(backend);
    } else {
        output.extend_from_slice(ranges);
    }
    Ok(output)
}

fn enum_decoder_v2(
    types: &[SemanticTypeDeclV1],
    semantic_type: SemanticTypeIdV1,
    base_offset: usize,
) -> Result<EnumDecoderV1, SimRuntimeBackendErrorV1> {
    let declaration = types.get(semantic_type.index() as usize).ok_or_else(|| {
        SimRuntimeBackendErrorV1::InvalidBundle("enum semantic type is absent".to_owned())
    })?;
    let SemanticTypeShapeV1::Enum {
        discriminant,
        variants,
    } = declaration.shape()
    else {
        return Err(SimRuntimeBackendErrorV1::InvalidBundle(
            "enum decoder does not name an enum type".to_owned(),
        ));
    };
    let logical = types
        .get(discriminant.index() as usize)
        .and_then(|declaration| match declaration.shape() {
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }) => {
                Some((*signed, *bits))
            }
            SemanticTypeShapeV1::ValidityScalar(validity) => match validity.scalar() {
                SemanticScalarTypeV1::Integer { signed, bits } => Some((signed, bits)),
                _ => None,
            },
            _ => None,
        })
        .ok_or_else(|| {
            SimRuntimeBackendErrorV1::UnsupportedBundle(
                "enum logical discriminant is not a fixed-width integer".to_owned(),
            )
        })?;
    let mut values = Vec::new();
    values.try_reserve_exact(variants.len()).map_err(|_| {
        SimRuntimeBackendErrorV1::InvalidBundle("enum variant roster allocation failed".to_owned())
    })?;
    values.extend(
        variants
            .iter()
            .enumerate()
            .map(|(index, variant)| EnumVariantValueV1 {
                index: index as u32,
                discriminant: variant.discriminant(),
                uninhabited: variant.is_uninhabited(),
            }),
    );
    match declaration.layout().variants() {
        SemanticRustcVariantsV1::Single { index } => {
            let variant = values.get(*index as usize).ok_or_else(|| {
                SimRuntimeBackendErrorV1::InvalidBundle(
                    "single enum variant index is out of range".to_owned(),
                )
            })?;
            if variant.uninhabited {
                return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
                    "single enum variant is uninhabited".to_owned(),
                ));
            }
            Ok(EnumDecoderV1 {
                byte_offset: base_offset,
                byte_width: 0,
                variants: values,
                encoding: EnumDecoderEncodingV1::Single { variant: *index },
            })
        }
        SemanticRustcVariantsV1::Multiple(layout) => match layout.encoding() {
            SemanticEnumEncodingV1::Direct(direct) => {
                let (_, physical_bits) = integer_primitive_v2(direct.tag().primitive())?;
                let width =
                    usize::try_from(direct.tag().primitive().size_bytes().ok_or_else(|| {
                        SimRuntimeBackendErrorV1::UnsupportedBundle(
                            "direct enum tag has unsupported width".to_owned(),
                        )
                    })?)
                    .map_err(|_| {
                        SimRuntimeBackendErrorV1::UnsupportedBundle(
                            "direct enum tag width does not fit this host".to_owned(),
                        )
                    })?;
                Ok(EnumDecoderV1 {
                    byte_offset: base_offset
                        .checked_add(usize::try_from(direct.tag_offset_bytes()).map_err(|_| {
                            SimRuntimeBackendErrorV1::UnsupportedBundle(
                                "direct enum tag offset does not fit this host".to_owned(),
                            )
                        })?)
                        .ok_or_else(|| {
                            SimRuntimeBackendErrorV1::InvalidBundle(
                                "direct enum tag offset overflow".to_owned(),
                            )
                        })?,
                    byte_width: width,
                    variants: values,
                    encoding: EnumDecoderEncodingV1::Direct {
                        physical_bits,
                        logical_signed: logical.0,
                        logical_bits: logical.1,
                        validity: direct.tag().valid_range(),
                    },
                })
            }
            SemanticEnumEncodingV1::Niche(niche) => {
                let (_, physical_bits) = integer_primitive_v2(niche.tag().primitive())?;
                let width =
                    usize::try_from(niche.tag().primitive().size_bytes().ok_or_else(|| {
                        SimRuntimeBackendErrorV1::UnsupportedBundle(
                            "niche enum tag has unsupported width".to_owned(),
                        )
                    })?)
                    .map_err(|_| {
                        SimRuntimeBackendErrorV1::UnsupportedBundle(
                            "niche enum tag width does not fit this host".to_owned(),
                        )
                    })?;
                Ok(EnumDecoderV1 {
                    byte_offset: base_offset
                        .checked_add(
                            usize::try_from(niche.source().expected_offset_bytes()).map_err(
                                |_| {
                                    SimRuntimeBackendErrorV1::UnsupportedBundle(
                                        "niche enum tag offset does not fit this host".to_owned(),
                                    )
                                },
                            )?,
                        )
                        .ok_or_else(|| {
                            SimRuntimeBackendErrorV1::InvalidBundle(
                                "niche enum tag offset overflow".to_owned(),
                            )
                        })?,
                    byte_width: width,
                    variants: values,
                    encoding: EnumDecoderEncodingV1::Niche {
                        physical_bits,
                        source_validity: niche.source_niche().valid_range(),
                        untagged_variant: niche.untagged_variant(),
                        niche_variants_start: niche.niche_variant_range().0,
                        niche_variants_end: niche.niche_variant_range().1,
                        niche_start: niche.niche_start(),
                    },
                })
            }
        },
        SemanticRustcVariantsV1::Empty => Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
            "uninhabited enum has no runtime discriminant".to_owned(),
        )),
    }
}

fn physical_enum_decoder_v2(
    mut decoder: EnumDecoderV1,
    slot: SemanticKernargSlotV2,
    source: usize,
) -> Result<EnumDecoderV1, SimRuntimeBackendErrorV1> {
    if decoder.byte_width == 0 {
        return Err(SimRuntimeBackendErrorV1::InvalidBundle(
            "single-variant enum must not retain a physical discriminant parameter".to_owned(),
        ));
    }
    let physical = validate_component_physical_slot_v2(
        slot,
        decoder.byte_width,
        decoder.byte_width.next_power_of_two(),
        source,
    )?;
    decoder.byte_offset = physical.offset;
    Ok(decoder)
}

fn integer_primitive_v2(
    primitive: SemanticBackendPrimitiveV1,
) -> Result<(bool, u16), SimRuntimeBackendErrorV1> {
    match primitive {
        SemanticBackendPrimitiveV1::Integer { signed, bits, .. }
            if matches!(bits, 8 | 16 | 32 | 64 | 128) =>
        {
            Ok((signed, bits))
        }
        SemanticBackendPrimitiveV1::Pointer { .. } => {
            Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
                "pointer-niche enum tags are not deserialized as host addresses".to_owned(),
            ))
        }
        SemanticBackendPrimitiveV1::Integer { .. } | SemanticBackendPrimitiveV1::Float { .. } => {
            Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
                "enum tag primitive is not a supported fixed-width integer".to_owned(),
            ))
        }
    }
}

fn prepare_arguments(
    kernel: &KernelRecordV1,
    kernarg: &[u8],
    bindings: &[BackendBindingV1],
    allocations: &HashMap<u64, AllocationRecordV1>,
) -> Result<Vec<VirtualArgumentV1>, SimRuntimeBackendErrorV1> {
    if kernarg.len() != kernel.explicit_byte_len {
        return Err(SimRuntimeBackendErrorV1::InvalidArguments(format!(
            "explicit kernarg has {} bytes; exact semantic layout requires {}",
            kernarg.len(),
            kernel.explicit_byte_len
        )));
    }
    let mut by_offset = HashMap::new();
    for binding in bindings {
        if by_offset
            .insert(binding.kernarg_byte_offset as usize, binding)
            .is_some()
        {
            return Err(SimRuntimeBackendErrorV1::InvalidArguments(
                "duplicate kernarg pointer binding".to_owned(),
            ));
        }
    }
    let mut consumed = HashSet::new();
    let mut output = Vec::new();
    output
        .try_reserve_exact(kernel.arguments.len())
        .map_err(|_| {
            SimRuntimeBackendErrorV1::InvalidArguments(
                "materialized launch argument allocation failed".to_owned(),
            )
        })?;
    for argument in &kernel.arguments {
        let bytes = kernarg
            .get(argument.offset..argument.offset + argument.size)
            .ok_or_else(|| {
                SimRuntimeBackendErrorV1::InvalidArguments(
                    "argument range exceeds kernarg".to_owned(),
                )
            })?;
        match (&argument.materialization, &argument.ty) {
            (ArgumentMaterializationV1::ExactBytes { validity, guards }, Type::Scalar(ty)) => {
                require_enum_guards_active_v2(guards, kernarg)?;
                let bits = read_little_scalar_v2(bytes)?;
                if !validity.is_empty()
                    && !validity
                        .iter()
                        .copied()
                        .any(|range| validity_range_contains_v2(range, bits))
                {
                    return Err(SimRuntimeBackendErrorV1::InvalidArguments(
                        "by-value aggregate scalar component violates Rust validity".to_owned(),
                    ));
                }
                let value =
                    ScalarBitsV1::new(*ty, bits, fe2o3_kir_sim::SimulationTargetV1::amdgpu_64())
                        .map_err(|error| {
                            SimRuntimeBackendErrorV1::InvalidArguments(error.to_string())
                        })?;
                output.push(VirtualArgumentV1::Scalar(value));
            }
            (ArgumentMaterializationV1::EnumDiscriminant { decoder, guards }, Type::Scalar(ty)) => {
                require_enum_guards_active_v2(guards, kernarg)?;
                let bits = decode_enum_v2(decoder, kernarg)?.discriminant;
                let value =
                    ScalarBitsV1::new(*ty, bits, fe2o3_kir_sim::SimulationTargetV1::amdgpu_64())
                        .map_err(|error| {
                            SimRuntimeBackendErrorV1::InvalidArguments(error.to_string())
                        })?;
                output.push(VirtualArgumentV1::Scalar(value));
            }
            (ArgumentMaterializationV1::Region { metadata: None }, Type::Pointer(pointer)) => {
                require_zero_pointer_placeholder(bytes)?;
                let binding = binding_at(argument.offset, &mut consumed, &by_offset)?;
                output.push(buffer_argument(
                    binding,
                    pointer.pointee.as_ref(),
                    pointer.address_space,
                    pointer.access,
                    allocations,
                    None,
                )?);
            }
            (
                ArgumentMaterializationV1::Region {
                    metadata: Some(metadata),
                },
                Type::Slice(slice),
            ) => {
                require_zero_pointer_placeholder(bytes)?;
                let binding = binding_at(argument.offset, &mut consumed, &by_offset)?;
                let length_bytes = kernarg
                    .get(metadata.offset..metadata.offset.saturating_add(metadata.size))
                    .ok_or_else(|| {
                        SimRuntimeBackendErrorV1::InvalidArguments(
                            "slice metadata is truncated".to_owned(),
                        )
                    })?;
                if length_bytes.len() != 8 {
                    return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                        "slice metadata slot differs from the target index width".to_owned(),
                    ));
                }
                let elements = usize::try_from(u64::from_le_bytes(
                    length_bytes.try_into().expect("fixed slice metadata"),
                ))
                .map_err(|_| {
                    SimRuntimeBackendErrorV1::InvalidArguments(
                        "slice length does not fit host usize".to_owned(),
                    )
                })?;
                output.push(buffer_argument(
                    binding,
                    slice.element.as_ref(),
                    slice.address_space,
                    slice.access,
                    allocations,
                    Some(elements),
                )?);
            }
            (_, Type::Unit) => {
                return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
                    "unit KIR parameters are not materialized".to_owned(),
                ));
            }
            _ => {
                return Err(SimRuntimeBackendErrorV1::InvalidBundle(
                    "argument materialization disagrees with its KIR type".to_owned(),
                ));
            }
        }
    }
    if consumed.len() != bindings.len() {
        return Err(SimRuntimeBackendErrorV1::InvalidArguments(
            "launch contains an unmatched memory binding".to_owned(),
        ));
    }
    Ok(output)
}

fn read_little_scalar_v2(bytes: &[u8]) -> Result<u128, SimRuntimeBackendErrorV1> {
    if bytes.len() > 16 {
        return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
            "scalar component exceeds 128 bits".to_owned(),
        ));
    }
    let mut little = [0_u8; 16];
    little[..bytes.len()].copy_from_slice(bytes);
    Ok(u128::from_le_bytes(little))
}

fn enum_guards_active_v2(
    guards: &[EnumVariantGuardV1],
    kernarg: &[u8],
) -> Result<bool, SimRuntimeBackendErrorV1> {
    for guard in guards {
        if decode_enum_v2(&guard.decoder, kernarg)?.index != guard.required_variant {
            return Ok(false);
        }
    }
    Ok(true)
}

fn require_enum_guards_active_v2(
    guards: &[EnumVariantGuardV1],
    kernarg: &[u8],
) -> Result<(), SimRuntimeBackendErrorV1> {
    if enum_guards_active_v2(guards, kernarg)? {
        Ok(())
    } else {
        Err(SimRuntimeBackendErrorV1::InvalidArguments(
            "inactive enum payload is poison and cannot be materialized as a KIR argument"
                .to_owned(),
        ))
    }
}

fn decode_enum_v2(
    decoder: &EnumDecoderV1,
    kernarg: &[u8],
) -> Result<EnumVariantValueV1, SimRuntimeBackendErrorV1> {
    if let EnumDecoderEncodingV1::Single { variant } = decoder.encoding {
        return decoder
            .variants
            .get(variant as usize)
            .copied()
            .filter(|value| !value.uninhabited)
            .ok_or_else(|| {
                SimRuntimeBackendErrorV1::InvalidArguments(
                    "single-variant enum has no inhabited variant".to_owned(),
                )
            });
    }
    let end = decoder
        .byte_offset
        .checked_add(decoder.byte_width)
        .ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidBundle("enum tag byte range overflow".to_owned())
        })?;
    let bits = read_little_scalar_v2(kernarg.get(decoder.byte_offset..end).ok_or_else(|| {
        SimRuntimeBackendErrorV1::InvalidArguments(
            "enum tag byte range exceeds the source kernarg".to_owned(),
        )
    })?)?;
    let index = match decoder.encoding {
        EnumDecoderEncodingV1::Single { .. } => unreachable!("handled above"),
        EnumDecoderEncodingV1::Direct {
            physical_bits,
            logical_signed,
            logical_bits,
            validity,
        } => {
            if validity.is_some_and(|range| !validity_range_contains_v2(range, bits)) {
                return Err(SimRuntimeBackendErrorV1::InvalidArguments(
                    "direct enum tag violates its rustc validity range".to_owned(),
                ));
            }
            decoder
                .variants
                .iter()
                .find(|variant| {
                    !variant.uninhabited
                        && encoded_discriminant_v2(
                            variant.discriminant,
                            logical_signed,
                            logical_bits,
                            physical_bits,
                        ) == bits
                })
                .map(|variant| variant.index)
                .ok_or_else(|| {
                    SimRuntimeBackendErrorV1::InvalidArguments(
                        "direct enum tag does not select an inhabited variant".to_owned(),
                    )
                })?
        }
        EnumDecoderEncodingV1::Niche {
            physical_bits,
            source_validity,
            untagged_variant,
            niche_variants_start,
            niche_variants_end,
            niche_start,
        } => {
            let relative = bits.wrapping_sub(niche_start) & unsigned_mask_v2(physical_bits);
            let niche_count = u128::from(
                niche_variants_end
                    .checked_sub(niche_variants_start)
                    .ok_or_else(|| {
                        SimRuntimeBackendErrorV1::InvalidBundle(
                            "niche enum variant range is reversed".to_owned(),
                        )
                    })?,
            );
            if relative <= niche_count {
                u32::try_from(u128::from(niche_variants_start) + relative).map_err(|_| {
                    SimRuntimeBackendErrorV1::InvalidArguments(
                        "niche enum variant does not fit the semantic roster".to_owned(),
                    )
                })?
            } else if validity_range_contains_v2(source_validity, bits) {
                untagged_variant
            } else {
                return Err(SimRuntimeBackendErrorV1::InvalidArguments(
                    "niche enum tag is neither a niche nor a valid untagged value".to_owned(),
                ));
            }
        }
    };
    decoder
        .variants
        .get(index as usize)
        .copied()
        .filter(|value| value.index == index && !value.uninhabited)
        .ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidArguments(
                "enum tag selects an absent or uninhabited variant".to_owned(),
            )
        })
}

fn encoded_discriminant_v2(
    discriminant: u128,
    logical_signed: bool,
    logical_bits: u16,
    physical_bits: u16,
) -> u128 {
    if logical_signed && logical_bits > 0 && discriminant & (1_u128 << (logical_bits - 1)) != 0 {
        let logical = discriminant | !unsigned_mask_v2(logical_bits);
        logical & unsigned_mask_v2(physical_bits)
    } else {
        discriminant & unsigned_mask_v2(physical_bits)
    }
}

fn unsigned_mask_v2(bits: u16) -> u128 {
    if bits == 128 {
        u128::MAX
    } else {
        (1_u128 << bits) - 1
    }
}

fn validity_range_contains_v2(range: SemanticScalarValidityRangeV1, bits: u128) -> bool {
    if range.start() <= range.end() {
        (range.start()..=range.end()).contains(&bits)
    } else {
        bits >= range.start() || bits <= range.end()
    }
}

fn require_zero_pointer_placeholder(bytes: &[u8]) -> Result<(), SimRuntimeBackendErrorV1> {
    if bytes.len() != 8 || bytes.iter().any(|byte| *byte != 0) {
        return Err(SimRuntimeBackendErrorV1::InvalidArguments(
            "address-free pointer slot must contain the canonical zero placeholder".to_owned(),
        ));
    }
    Ok(())
}

fn binding_at<'a>(
    offset: usize,
    consumed: &mut HashSet<usize>,
    bindings: &'a HashMap<usize, &'a BackendBindingV1>,
) -> Result<&'a BackendBindingV1, SimRuntimeBackendErrorV1> {
    let binding = *bindings.get(&offset).ok_or_else(|| {
        SimRuntimeBackendErrorV1::InvalidArguments(format!(
            "missing pointer binding at byte {offset}"
        ))
    })?;
    consumed.insert(offset);
    Ok(binding)
}

fn buffer_argument(
    binding: &BackendBindingV1,
    element: &Type,
    address_space: AddressSpace,
    access: AccessMode,
    allocations: &HashMap<u64, AllocationRecordV1>,
    declared_elements: Option<usize>,
) -> Result<VirtualArgumentV1, SimRuntimeBackendErrorV1> {
    if address_space != AddressSpace::Global {
        return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
            "only global pointer and slice arguments are admitted".to_owned(),
        ));
    }
    let element = element.as_scalar().ok_or_else(|| {
        SimRuntimeBackendErrorV1::UnsupportedBundle(
            "aggregate buffer elements are not materialized".to_owned(),
        )
    })?;
    require_access(binding.region.access, access)?;
    let allocation = allocations
        .get(&binding.region.allocation)
        .ok_or(SimRuntimeBackendErrorV1::InvalidHandle("allocation"))?;
    let region_end = binding
        .region
        .byte_offset
        .checked_add(binding.region.byte_len)
        .ok_or_else(|| {
            SimRuntimeBackendErrorV1::InvalidArguments("buffer region overflow".to_owned())
        })?;
    if binding.region.byte_len == 0 || region_end > allocation.byte_len {
        return Err(SimRuntimeBackendErrorV1::InvalidArguments(
            "buffer region exceeds its allocation".to_owned(),
        ));
    }
    let element_bytes = scalar_bytes(Some(element)).ok_or_else(|| {
        SimRuntimeBackendErrorV1::UnsupportedBundle(
            "buffer scalar has no concrete width".to_owned(),
        )
    })?;
    let byte_len = usize::try_from(binding.region.byte_len).map_err(|_| {
        SimRuntimeBackendErrorV1::InvalidArguments(
            "buffer byte length does not fit host usize".to_owned(),
        )
    })?;
    if !byte_len.is_multiple_of(element_bytes) {
        return Err(SimRuntimeBackendErrorV1::InvalidArguments(
            "buffer region is not an integral number of KIR elements".to_owned(),
        ));
    }
    let elements = byte_len / element_bytes;
    if declared_elements.is_some_and(|declared| declared != elements) {
        return Err(SimRuntimeBackendErrorV1::InvalidArguments(
            "slice metadata length differs from the address-free bound region".to_owned(),
        ));
    }
    let byte_offset = usize::try_from(binding.region.byte_offset).map_err(|_| {
        SimRuntimeBackendErrorV1::InvalidArguments(
            "buffer offset does not fit host usize".to_owned(),
        )
    })?;
    let offset_alignment = if byte_offset == 0 {
        u64::MAX
    } else {
        1_u64 << byte_offset.trailing_zeros().min(63)
    };
    let effective_alignment = allocation.alignment.min(offset_alignment).min(1_u64 << 31);
    if effective_alignment < element_bytes as u64 {
        return Err(SimRuntimeBackendErrorV1::InvalidArguments(
            "buffer view does not satisfy its scalar element alignment".to_owned(),
        ));
    }
    let alignment = effective_alignment as u32;
    Ok(VirtualArgumentV1::Buffer {
        buffer: allocation.buffer,
        element,
        access,
        alignment,
        byte_offset,
        elements,
    })
}

fn require_access(
    actual: RuntimeAccessV1,
    required: AccessMode,
) -> Result<(), SimRuntimeBackendErrorV1> {
    let admitted = matches!(
        (actual, required),
        (RuntimeAccessV1::Read, AccessMode::ReadOnly)
            | (RuntimeAccessV1::Write, AccessMode::WriteOnly)
            | (RuntimeAccessV1::ReadWrite, _)
    );
    if admitted {
        Ok(())
    } else {
        Err(SimRuntimeBackendErrorV1::InvalidArguments(
            "runtime binding access does not cover the KIR parameter effect".to_owned(),
        ))
    }
}

fn ownership_matches(
    storage: SemanticArgumentOwnershipV1,
    semantic: Option<SemanticSourceArgumentOwnershipV1>,
) -> bool {
    matches!(
        (storage, semantic),
        (
            SemanticArgumentOwnershipV1::ByValue,
            Some(SemanticSourceArgumentOwnershipV1::ByValue)
        ) | (
            SemanticArgumentOwnershipV1::SharedBorrow,
            Some(SemanticSourceArgumentOwnershipV1::SharedBorrow)
        ) | (
            SemanticArgumentOwnershipV1::UniqueBorrow,
            Some(SemanticSourceArgumentOwnershipV1::UniqueBorrow)
        ) | (
            SemanticArgumentOwnershipV1::ExclusiveOwner,
            Some(SemanticSourceArgumentOwnershipV1::ExclusiveOwner)
        ) | (
            SemanticArgumentOwnershipV1::RawPointer,
            Some(SemanticSourceArgumentOwnershipV1::RawPointer)
        )
    )
}

fn representation_matches(
    representation: SemanticKirStorageRepresentationV1,
    ty: Option<&Type>,
) -> bool {
    matches!(
        (representation, ty),
        (
            SemanticKirStorageRepresentationV1::Scalar,
            Some(Type::Scalar(_))
        ) | (
            SemanticKirStorageRepresentationV1::RegionPointer,
            Some(Type::Pointer(_))
        ) | (
            SemanticKirStorageRepresentationV1::RegionSlice,
            Some(Type::Slice(_))
        ) | (SemanticKirStorageRepresentationV1::OpaqueFlattened, Some(_))
    )
}

fn require_range(
    allocation_bytes: u64,
    byte_offset: u64,
    byte_len: usize,
) -> Result<(), SimRuntimeBackendErrorV1> {
    let byte_len = u64::try_from(byte_len).map_err(|_| {
        SimRuntimeBackendErrorV1::InvalidArguments("host byte length does not fit u64".to_owned())
    })?;
    let end = byte_offset.checked_add(byte_len).ok_or_else(|| {
        SimRuntimeBackendErrorV1::InvalidArguments("host byte range overflow".to_owned())
    })?;
    if end > allocation_bytes {
        return Err(SimRuntimeBackendErrorV1::InvalidArguments(
            "host byte range exceeds allocation".to_owned(),
        ));
    }
    Ok(())
}

struct WorkerAliveGuardV1(Arc<AtomicBool>);

impl Drop for WorkerAliveGuardV1 {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn worker_main(
    mut runtime: VirtualRuntimeV1,
    commands: mpsc::Receiver<WorkerCommandV1>,
    worker_alive: Arc<AtomicBool>,
) {
    let _alive_guard = WorkerAliveGuardV1(worker_alive);
    let mut completions = HashMap::<u64, VirtualCompletionHandleV1>::new();
    while let Ok(command) = commands.recv() {
        match command {
            WorkerCommandV1::CreateQueue { response } => {
                let _ = response.send(
                    runtime
                        .create_queue(QUEUE_CAPACITY)
                        .map_err(display_virtual),
                );
            }
            WorkerCommandV1::ReleaseQueue { queue, response } => {
                let _ = response.send(runtime.release_queue(queue).map_err(display_virtual));
            }
            WorkerCommandV1::Allocate { byte_len, response } => {
                let _ = response.send(
                    runtime
                        .allocate_buffer(byte_len, VirtualBufferAccessV1::ReadWrite)
                        .map_err(display_virtual),
                );
            }
            WorkerCommandV1::ReleaseAllocation { buffer, response } => {
                let _ = response.send(runtime.release_buffer(buffer).map_err(display_virtual));
            }
            WorkerCommandV1::Write {
                buffer,
                offset,
                bytes,
                response,
            } => {
                let _ = response.send(
                    runtime
                        .copy_from_host(buffer, offset, &bytes)
                        .map_err(display_virtual),
                );
            }
            WorkerCommandV1::Read {
                buffer,
                offset,
                byte_len,
                response,
            } => {
                let mut bytes = vec![0; byte_len];
                let result = runtime
                    .copy_to_host(buffer, offset, &mut bytes)
                    .map(|()| bytes)
                    .map_err(display_virtual);
                let _ = response.send(result);
            }
            WorkerCommandV1::RegisterModule { module, response } => {
                let _ = response.send(runtime.register_module(module).map_err(display_virtual));
            }
            WorkerCommandV1::ReleaseModule { module, response } => {
                let _ = response.send(runtime.release_module(module).map_err(display_virtual));
            }
            WorkerCommandV1::Submit {
                id,
                queue,
                module,
                request,
                dependencies,
                completion,
            } => {
                let dependencies = dependencies
                    .into_iter()
                    .map(|dependency| completions.get(&dependency).copied().ok_or(()))
                    .collect::<Result<Vec<_>, _>>();
                let outcome = match dependencies {
                    Err(()) => CompletionOutcomeV1::Failed(FAILED_SIMULATION_CODE),
                    Ok(dependencies) => {
                        let virtual_request = VirtualDispatchRequestV1 {
                            kernel: request.kernel,
                            grid: request.grid,
                            workgroup: request.workgroup,
                            arguments: request.arguments,
                            dependencies,
                        };
                        let submitted = match request.dynamic_workgroup_memory {
                            Some(dynamic) => runtime.submit_with_dynamic_workgroup_memory(
                                queue,
                                module,
                                virtual_request,
                                dynamic,
                            ),
                            None => runtime.submit(queue, module, virtual_request),
                        };
                        match submitted {
                            Err(_) => CompletionOutcomeV1::Failed(FAILED_SIMULATION_CODE),
                            Ok(handle) => {
                                completions.insert(id, handle);
                                match runtime.run_next() {
                                    Ok(VirtualRunProgressV1::Completed { completion, .. })
                                        if completion == handle =>
                                    {
                                        CompletionOutcomeV1::Succeeded
                                    }
                                    Ok(VirtualRunProgressV1::AbortedDependency {
                                        completion,
                                        ..
                                    }) if completion == handle => {
                                        CompletionOutcomeV1::Failed(FAILED_SIMULATION_CODE)
                                    }
                                    Ok(
                                        VirtualRunProgressV1::Blocked
                                        | VirtualRunProgressV1::Idle
                                        | VirtualRunProgressV1::Completed { .. }
                                        | VirtualRunProgressV1::AbortedDependency { .. },
                                    )
                                    | Err(_) => CompletionOutcomeV1::Failed(FAILED_SIMULATION_CODE),
                                }
                            }
                        }
                    }
                };
                completion.finish(outcome);
            }
            #[cfg(test)]
            WorkerCommandV1::Panic => panic!("test-requested simulator worker panic"),
            #[cfg(test)]
            WorkerCommandV1::Block { started, release } => {
                let _ = started.send(());
                let _ = release.recv();
            }
            #[cfg(test)]
            WorkerCommandV1::Noop => {}
            WorkerCommandV1::Shutdown => break,
        }
    }
}

fn display_virtual(error: VirtualRuntimeErrorV1) -> String {
    error.to_string()
}

fn scalar_bytes(ty: Option<ScalarType>) -> Option<usize> {
    match ty? {
        ScalarType::Bool | ScalarType::I8 | ScalarType::U8 => Some(1),
        ScalarType::I16 | ScalarType::U16 | ScalarType::F16 | ScalarType::Bf16 => Some(2),
        ScalarType::I32 | ScalarType::U32 | ScalarType::F32 => Some(4),
        ScalarType::I64 | ScalarType::U64 | ScalarType::Index | ScalarType::F64 => Some(8),
        ScalarType::I128 | ScalarType::U128 => Some(16),
    }
}

fn align_up(value: usize, alignment: usize) -> Result<usize, SimRuntimeBackendErrorV1> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
            "argument alignment is invalid".to_owned(),
        ));
    }
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
        .ok_or_else(|| {
            SimRuntimeBackendErrorV1::UnsupportedBundle("argument layout overflow".to_owned())
        })
}

fn rejected_handle(kind: &'static str) -> RuntimeBackendFailureV1<SimRuntimeBackendErrorV1> {
    RuntimeBackendFailureV1::Rejected(SimRuntimeBackendErrorV1::InvalidHandle(kind))
}

fn rejected_arguments(detail: &'static str) -> RuntimeBackendFailureV1<SimRuntimeBackendErrorV1> {
    RuntimeBackendFailureV1::Rejected(SimRuntimeBackendErrorV1::InvalidArguments(
        detail.to_owned(),
    ))
}

fn require_capacity(
    current: usize,
    maximum: usize,
) -> Result<(), RuntimeBackendFailureV1<SimRuntimeBackendErrorV1>> {
    if current >= maximum {
        Err(rejected_arguments("backend resource capacity"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::SemanticArgumentStorageV2;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, CastKind, Constant, DebugSourceMapDocumentV2, DebugSourceMapFileV1,
        DebugSourceMapSpanV1, Function, FunctionId, Kernel, LaunchDomain, LaunchExtent,
        MemoryAccess, Module, Operation, PreparedSimulationBundleV6, SemanticAggregateStorageMapV6,
        SemanticKernelStorageV1, SemanticKernelStorageV2, SemanticStorageMapV6, Signature,
        SimulationProductionKirIdentityV6, SimulationSourceLineageV1, Terminator, ValueDef,
        ValueId, VerifiedCanonicalKernelIrV11, WorkgroupMemory, WorkgroupMemoryExtent,
    };
    use fe2o3_mir_model::semantic_mir_v1::{
        InertSemanticMirRequestV1, SemanticAbiCastV1, SemanticAbiIdentityV1,
        SemanticAbiPointeeInfoV1, SemanticAbiRegisterV1, SemanticAbiRegularAttributesV1,
        SemanticAbiUniformV1, SemanticAbiValueAttributesV1, SemanticAbiValueV1,
        SemanticAggregateLayoutV1, SemanticAggregateTypeV1, SemanticBasicBlockV1,
        SemanticBlockIdV1, SemanticBlockIdentityV1, SemanticConstGenericArgumentsIdentityV1,
        SemanticExternAbiV1, SemanticFunctionAbiV1, SemanticFunctionDeclV1, SemanticFunctionIdV1,
        SemanticFunctionIdentityV1, SemanticFunctionRoleV1, SemanticGenericTypeArgumentsIdentityV1,
        SemanticItemDefinitionIdentityV1, SemanticKernelBindingIdentityV1, SemanticKernelEntryV1,
        SemanticKernelLaunchBoundsV1, SemanticKernelSourceContractV1, SemanticLayoutIdentityV1,
        SemanticLinkSymbolV1, SemanticLocalDeclV1, SemanticLocalIdentityV1,
        SemanticMonomorphizationIdentityV1, SemanticPaddingV1, SemanticPointerTypeV1,
        SemanticSourceProvenanceV1, SemanticTargetDataLayoutV1, SemanticTerminatorKindV1,
        SemanticTerminatorV1, SemanticTypeAbiPropertiesV1, SemanticTypeIdentityV1,
        SemanticTypeLayoutV1, SemanticTypeShapeV1, SemanticValidityScalarTypeV1,
        SemanticWorkgroupDimensionsV1,
    };

    include!("sim_bundle_v6_tests.rs");

    fn runtime_dynamic_reachability_module(call_helper: bool) -> (Module, FunctionId) {
        let entry_id = FunctionId::new("entry");
        let mut entry_block = BasicBlock::new(BlockId(0));
        if call_helper {
            entry_block.operations.push(Operation::new(
                vec![],
                OperationKind::Call {
                    callee: "dynamic_helper".into(),
                    arguments: vec![],
                },
            ));
        }
        entry_block.terminator = Some(Terminator::Return { values: vec![] });
        let entry = Function::kernel_entry(
            entry_id.clone(),
            Signature::new(vec![], vec![]),
            vec![],
            vec![entry_block],
        );
        let scalar = Type::Scalar(ScalarType::U32);
        let mut helper_block = BasicBlock::new(BlockId(0));
        helper_block.operations.push(Operation::effect_free(
            ValueDef::new(
                ValueId(0),
                Type::pointer(
                    scalar.clone(),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: scalar,
                extent: WorkgroupMemoryExtent::Dynamic,
                alignment: 4,
            }),
        ));
        helper_block.terminator = Some(Terminator::Return { values: vec![] });
        let helper = Function::internal_helper(
            "dynamic_helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![helper_block],
        );
        let mut module = Module::new("sim-runtime-dynamic-reachability");
        module.functions.extend([entry, helper]);
        (module, entry_id)
    }

    #[test]
    fn runtime_dynamic_lds_detection_follows_only_reachable_calls() {
        let (reachable, entry) = runtime_dynamic_reachability_module(true);
        assert!(kernel_reaches_dynamic_workgroup_memory(&reachable, &entry).unwrap());

        let (unreachable, entry) = runtime_dynamic_reachability_module(false);
        assert!(!kernel_reaches_dynamic_workgroup_memory(&unreachable, &entry).unwrap());
        assert!(matches!(
            kernel_reaches_dynamic_workgroup_memory(
                &unreachable,
                &FunctionId::new("substituted_entry")
            ),
            Err(SimRuntimeBackendErrorV1::InvalidBundle(detail))
                if detail == "kernel entry missing"
        ));
    }

    #[test]
    fn runtime_dynamic_lds_launch_mapping_preserves_zero_and_hostile_extents() {
        assert_eq!(dynamic_workgroup_memory_request(false, 0), None);
        assert_eq!(
            dynamic_workgroup_memory_request(true, 0),
            Some(DynamicWorkgroupMemoryRequestV1::new(0))
        );
        assert_eq!(
            dynamic_workgroup_memory_request(false, 64),
            Some(DynamicWorkgroupMemoryRequestV1::new(64))
        );
        assert_eq!(
            dynamic_workgroup_memory_request(true, 96),
            Some(DynamicWorkgroupMemoryRequestV1::new(96))
        );
    }

    #[test]
    fn short_and_foreign_v4_prefixes_keep_the_v3_decode_error() {
        for image in [b"F2SIMB0".as_slice(), b"F2SIMB03payload".as_slice()] {
            let expected = VerifiedSimulationBundleV3::from_canonical_bytes(image.to_vec())
                .unwrap_err()
                .to_string();
            let actual = match parse_bundle(image, VirtualTargetProfileV1::Gfx942XnackMinus) {
                Ok(_) => panic!("non-V4 input unexpectedly parsed"),
                Err(error) => error,
            };
            assert_eq!(actual, SimRuntimeBackendErrorV1::InvalidBundle(expected));
        }
    }
    use fe2o3_runtime::{
        RuntimeArgumentsV1, RuntimeBindingV1, RuntimeContextV1, RuntimeErrorV1,
        RuntimeMemoryRegionV1, RuntimeValidationErrorV1,
    };
    use std::time::Duration;

    struct AggregateSliceScalarArgumentsFixtureV1 {
        head: u16,
        tail: u64,
        allocation: fe2o3_runtime::RuntimeAllocationIdV1,
        elements: u64,
        scalar: u32,
    }

    impl RuntimeArgumentsV1 for AggregateSliceScalarArgumentsFixtureV1 {
        const SIGNATURE_V1: [u8; 32] = [0xa4; 32];

        fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
            let mut bytes = vec![0; 40];
            bytes[0..2].copy_from_slice(&self.head.to_le_bytes());
            bytes[8..16].copy_from_slice(&self.tail.to_le_bytes());
            bytes[24..32].copy_from_slice(&self.elements.to_le_bytes());
            bytes[32..36].copy_from_slice(&self.scalar.to_le_bytes());
            bytes
        }

        fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
            vec![RuntimeBindingV1 {
                region: RuntimeMemoryRegionV1 {
                    allocation: self.allocation,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: self.elements * 4,
                },
                kernarg_byte_offset: 16,
            }]
        }
    }

    #[test]
    fn evidence_and_normal_memory_lifecycle_require_no_gpu() {
        let backend = SimRuntimeBackendV1::gfx942([0x31; 32]).unwrap();
        assert!(!backend.uses_gpu());
        assert_eq!(
            backend.evidence(),
            SimRuntimeEvidenceV1 {
                mode: "cpu-kir-semantic-simulation",
                simulated: true,
                hardware: false,
                performance_prediction: false,
            }
        );
        let mut context = RuntimeContextV1::open(backend).unwrap();
        assert_eq!(context.devices().len(), 1);
        assert_eq!(context.devices()[0].target(), "gfx942:xnack-");
        assert!(!context.devices()[0].capabilities().multi_device);
        assert!(!context.devices()[0].capabilities().peer_copy);
        assert!(!context.devices()[0].capabilities().collectives);
        let device = context.devices()[0].id();
        let stream = context.create_stream(device).unwrap();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 16, 8)
            .unwrap();
        let mut output = [0; 4];
        assert!(matches!(
            context.read_allocation(allocation, 0, &mut output),
            Err(RuntimeErrorV1::BackendRejected(_))
        ));
        context
            .write_allocation(allocation, 4, &[1, 2, 3, 4])
            .unwrap();
        context.read_allocation(allocation, 4, &mut output).unwrap();
        assert_eq!(output, [1, 2, 3, 4]);
        context.destroy_stream(stream).unwrap();
        context.release_allocation(allocation).unwrap();
        let backend = context.shutdown().unwrap();
        assert!(!backend.uses_gpu());
    }

    #[test]
    fn malformed_bundle_and_direct_spi_misuse_fail_without_custody_transfer() {
        let mut backend = SimRuntimeBackendV1::gfx942([0x32; 32]).unwrap();
        assert!(matches!(
            backend.load_module_v1(DEVICE_HANDLE, b"not-v3"),
            Err(RuntimeBackendFailureV1::Rejected(
                SimRuntimeBackendErrorV1::InvalidBundle(_)
            ))
        ));
        assert!(matches!(
            backend.create_stream_v1(99),
            Err(RuntimeBackendFailureV1::Rejected(
                SimRuntimeBackendErrorV1::InvalidHandle("device")
            ))
        ));
        assert!(matches!(
            backend.read_allocation_v1(77, 0, &mut [0]),
            Err(RuntimeBackendFailureV1::Rejected(
                SimRuntimeBackendErrorV1::InvalidHandle("allocation")
            ))
        ));
        let geometry = fe2o3_runtime::RuntimeLaunchGeometryV1 {
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_bytes: 0,
        };
        assert!(matches!(
            backend.submit_v1(BackendLaunchV1 {
                stream: 77,
                kernel: 88,
                explicit_kernarg: &[],
                bindings: &[],
                dependencies: &[],
                geometry,
                semantic_launch: BackendSemanticLaunchV1::Collective(
                    fe2o3_runtime::RuntimeCollectiveLaunchContractV1 {
                        operation: fe2o3_runtime::RuntimeCollectiveOperationV1::ReduceSum,
                        scope: fe2o3_runtime::RuntimeMemoryScopeV1::Workgroup,
                        order: fe2o3_runtime::RuntimeMemoryOrderV1::AcquireRelease,
                        participants: 64,
                        geometry,
                    },
                ),
            }),
            Err(RuntimeBackendFailureV1::Rejected(
                SimRuntimeBackendErrorV1::InvalidArguments(detail)
            )) if detail == "semantic launch"
        ));
        assert!(backend.submissions.is_empty());
        assert!(matches!(
            backend.peer_copy_v1(
                1,
                BackendMemoryRegionV1 {
                    allocation: 1,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 1,
                },
                BackendMemoryRegionV1 {
                    allocation: 2,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 1,
                },
                &[],
            ),
            Err(RuntimeBackendFailureV1::Rejected(
                SimRuntimeBackendErrorV1::UnsupportedBundle(_)
            ))
        ));
    }

    #[test]
    fn completion_wait_is_deadline_bounded_and_poll_is_nonblocking() {
        let cell = CompletionCellV1::pending();
        let before = Instant::now();
        let alive = AtomicBool::new(true);
        assert_eq!(cell.poll(&alive).unwrap(), BackendPollV1::Pending);
        assert_eq!(cell.wait(before, &alive).unwrap(), BackendPollV1::Pending);
        assert!(before.elapsed() < Duration::from_secs(1));
        cell.finish(CompletionOutcomeV1::Succeeded);
        assert_eq!(
            cell.wait(Instant::now() + Duration::from_secs(1), &alive)
                .unwrap(),
            BackendPollV1::Succeeded
        );
    }

    #[test]
    fn dead_worker_makes_pending_completion_terminal_without_waiting_for_deadline() {
        let cell = CompletionCellV1::pending();
        let alive = AtomicBool::new(false);
        assert_eq!(
            cell.poll(&alive),
            Err(SimRuntimeBackendErrorV1::WorkerDisconnected)
        );
        let before = Instant::now();
        assert_eq!(
            cell.wait(before + Duration::from_secs(60), &alive),
            Err(SimRuntimeBackendErrorV1::WorkerDisconnected)
        );
        assert!(before.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn pending_release_preserves_custody_and_worker_panic_terminalizes_backend() {
        let mut backend = SimRuntimeBackendV1::gfx942([0x34; 32]).unwrap();
        let completion = Arc::new(CompletionCellV1::pending());
        backend.submissions.insert(
            91,
            SubmissionRecordV1 {
                stream: 17,
                completion,
            },
        );
        assert!(matches!(
            backend.release_submission_v1(91),
            Err(RuntimeBackendFailureV1::Rejected(
                SimRuntimeBackendErrorV1::InvalidHandle("pending submission")
            ))
        ));
        assert!(backend.submissions.contains_key(&91));

        backend
            .commands
            .as_ref()
            .unwrap()
            .send(WorkerCommandV1::Panic)
            .unwrap();
        assert!(backend.worker.take().unwrap().join().is_err());
        assert!(matches!(
            backend.wait_v1(91, Instant::now() + Duration::from_secs(60)),
            Err(RuntimeBackendFailureV1::Terminal(
                SimRuntimeBackendErrorV1::WorkerDisconnected
            ))
        ));
        assert!(backend.submissions.contains_key(&91));
        assert!(matches!(
            backend.release_submission_v1(91),
            Err(RuntimeBackendFailureV1::Terminal(
                SimRuntimeBackendErrorV1::WorkerDisconnected
            ))
        ));
    }

    #[test]
    fn command_queue_backpressure_rejects_and_full_queue_drop_closes_before_join() {
        let backend = SimRuntimeBackendV1::gfx942([0x35; 32]).unwrap();
        let (started_send, started_receive) = mpsc::sync_channel(1);
        let (release_send, release_receive) = mpsc::sync_channel(1);
        let commands = backend.commands.as_ref().unwrap();
        commands
            .send(WorkerCommandV1::Block {
                started: started_send,
                release: release_receive,
            })
            .unwrap();
        started_receive.recv().unwrap();
        for _ in 0..COMMAND_QUEUE_CAPACITY {
            commands.try_send(WorkerCommandV1::Noop).unwrap();
        }
        assert!(matches!(
            commands.try_send(WorkerCommandV1::Noop),
            Err(mpsc::TrySendError::Full(WorkerCommandV1::Noop))
        ));

        let (dropped_send, dropped_receive) = mpsc::sync_channel(1);
        let dropper = thread::spawn(move || {
            drop(backend);
            dropped_send.send(()).unwrap();
        });
        assert!(
            dropped_receive
                .recv_timeout(Duration::from_millis(20))
                .is_err()
        );
        release_send.send(()).unwrap();
        dropped_receive
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        dropper.join().unwrap();
    }

    #[test]
    fn zero_runtime_identity_is_rejected_before_worker_creation() {
        assert!(matches!(
            SimRuntimeBackendV1::gfx950([0; 32]),
            Err(SimRuntimeBackendErrorV1::VirtualRuntime(_))
        ));
    }

    #[test]
    fn facade_still_rejects_invalid_ranges_before_backend_entry() {
        let backend = SimRuntimeBackendV1::gfx950([0x33; 32]).unwrap();
        let mut context = RuntimeContextV1::open(backend).unwrap();
        let device = context.devices()[0].id();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 8, 8)
            .unwrap();
        assert!(matches!(
            context.write_allocation(allocation, 8, &[1]),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidRange
            ))
        ));
        context.release_allocation(allocation).unwrap();
        context.shutdown().unwrap();
    }

    fn scalar_argument(
        offset: usize,
        size: usize,
        ty: ScalarType,
        materialization: ArgumentMaterializationV1,
    ) -> ArgumentRecordV1 {
        ArgumentRecordV1 {
            offset,
            size,
            ty: Type::Scalar(ty),
            materialization,
        }
    }

    fn materialization_kernel(arguments: Vec<ArgumentRecordV1>, bytes: usize) -> KernelRecordV1 {
        KernelRecordV1 {
            module: None,
            kernel: KernelId::new("aggregate_materialization_test"),
            signature: [1; 32],
            explicit_byte_len: bytes,
            arguments,
            requires_dynamic_workgroup_memory: false,
            unsupported: None,
        }
    }

    fn scalar_bits(arguments: Vec<VirtualArgumentV1>) -> Vec<u128> {
        arguments
            .into_iter()
            .map(|argument| match argument {
                VirtualArgumentV1::Scalar(value) => value.bits(),
                VirtualArgumentV1::Buffer { .. } => panic!("expected scalar component"),
            })
            .collect()
    }

    #[test]
    fn runtime_argument_fixture_physical_padding_and_slice_slots_drive_materialization() {
        let backend = SimRuntimeBackendV1::gfx942([0xa4; 32]).unwrap();
        let mut context = RuntimeContextV1::open(backend).unwrap();
        let device = context.devices()[0].id();
        let allocation = context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 16, 4)
            .unwrap();
        let fixture = AggregateSliceScalarArgumentsFixtureV1 {
            head: 0x1234,
            tail: 0x0102_0304_0506_0708,
            allocation,
            elements: 4,
            scalar: 0xaabb_ccdd,
        };
        let encoded = fixture.encode_explicit_kernarg_v1();
        assert_eq!(encoded.len(), 40);
        assert_eq!(&encoded[0..2], &0x1234_u16.to_le_bytes());
        assert!(encoded[2..8].iter().all(|byte| *byte == 0));
        assert_eq!(&encoded[8..16], &0x0102_0304_0506_0708_u64.to_le_bytes());
        assert!(encoded[16..24].iter().all(|byte| *byte == 0));
        assert_eq!(&encoded[24..32], &4_u64.to_le_bytes());
        assert_eq!(&encoded[32..36], &0xaabb_ccdd_u32.to_le_bytes());
        assert!(encoded[36..40].iter().all(|byte| *byte == 0));
        let fixture_bindings = fixture.bindings_v1();
        assert_eq!(fixture_bindings.len(), 1);
        assert_eq!(fixture_bindings[0].kernarg_byte_offset, 16);
        assert_eq!(fixture_bindings[0].region.byte_len, 16);
        context.release_allocation(allocation).unwrap();
        context.shutdown().unwrap();

        let kernel = materialization_kernel(
            vec![
                scalar_argument(
                    0,
                    2,
                    ScalarType::U16,
                    ArgumentMaterializationV1::ExactBytes {
                        validity: Vec::new(),
                        guards: Vec::new(),
                    },
                ),
                scalar_argument(
                    8,
                    8,
                    ScalarType::U64,
                    ArgumentMaterializationV1::ExactBytes {
                        validity: Vec::new(),
                        guards: Vec::new(),
                    },
                ),
                ArgumentRecordV1 {
                    offset: 16,
                    size: 8,
                    ty: Type::slice(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Global,
                        AccessMode::ReadOnly,
                    ),
                    materialization: ArgumentMaterializationV1::Region {
                        metadata: Some(PhysicalSlotV1 {
                            offset: 24,
                            size: 8,
                        }),
                    },
                },
                scalar_argument(
                    32,
                    4,
                    ScalarType::U32,
                    ArgumentMaterializationV1::ExactBytes {
                        validity: Vec::new(),
                        guards: Vec::new(),
                    },
                ),
            ],
            40,
        );
        let mut backend = SimRuntimeBackendV1::gfx942([0xa5; 32]).unwrap();
        let backend_allocation = backend
            .allocate_v1(DEVICE_HANDLE, RuntimeMemoryKindV1::HostVisible, 16, 4)
            .unwrap();
        let arguments = prepare_arguments(
            &kernel,
            &encoded,
            &[BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: backend_allocation,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 16,
                },
                kernarg_byte_offset: fixture_bindings[0].kernarg_byte_offset,
            }],
            &backend.allocations,
        )
        .unwrap();
        assert_eq!(arguments.len(), 4);
        assert!(
            matches!(&arguments[0], VirtualArgumentV1::Scalar(value) if value.bits() == 0x1234)
        );
        assert!(
            matches!(&arguments[1], VirtualArgumentV1::Scalar(value) if value.bits() == 0x0102_0304_0506_0708)
        );
        assert!(matches!(
            &arguments[2],
            VirtualArgumentV1::Buffer {
                element: ScalarType::U32,
                access: AccessMode::ReadOnly,
                elements: 4,
                ..
            }
        ));
        assert!(
            matches!(&arguments[3], VirtualArgumentV1::Scalar(value) if value.bits() == 0xaabb_ccdd)
        );
        let mut noncanonical_padding = encoded.clone();
        noncanonical_padding[2..8].fill(0x5a);
        noncanonical_padding[36..40].fill(0xa5);
        let materialized_with_padding = prepare_arguments(
            &kernel,
            &noncanonical_padding,
            &[BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: backend_allocation,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 16,
                },
                kernarg_byte_offset: fixture_bindings[0].kernarg_byte_offset,
            }],
            &backend.allocations,
        )
        .unwrap();
        assert_eq!(materialized_with_padding, arguments);
        backend.release_allocation_v1(backend_allocation).unwrap();
    }

    #[test]
    fn physical_component_offsets_are_independent_but_width_and_alignment_are_exact() {
        assert_eq!(
            validate_component_physical_slot_v2(SemanticKernargSlotV2::new(0, 8, 8), 8, 8, 0,)
                .unwrap()
                .offset,
            0
        );
        for substituted in [
            SemanticKernargSlotV2::new(8, 4, 8),
            SemanticKernargSlotV2::new(8, 8, 4),
        ] {
            assert!(matches!(
                validate_component_physical_slot_v2(substituted, 8, 8, 0),
                Err(SimRuntimeBackendErrorV1::InvalidBundle(detail))
                    if detail.contains("wrong ABI width or alignment")
            ));
        }
    }

    #[test]
    fn projected_components_require_compiler_rederived_canonical_packing() {
        let projected = SemanticArgumentStorageV2::new(
            0,
            1,
            0,
            SemanticArgumentOwnershipV1::ByValue,
            SemanticComponentStorageBindingV2::exact(vec![
                fe2o3_kernel_ir::SemanticKirComponentStorageV2::new(
                    vec![SemanticStorageProjectionV2::Field { index: 0 }],
                    0,
                    7,
                    SemanticKirComponentRepresentationV2::ScalarValue,
                    SemanticKernargSlotV2::new(0, 8, 8),
                    None,
                ),
            ]),
        );
        let exact = SemanticKernelStorageV2::new(0, 0, 0, 8, 8, vec![projected.clone()]);
        validate_compiler_packing_plan_v2(&exact, &[Type::Scalar(ScalarType::U64)]).unwrap();

        let substituted = SemanticArgumentStorageV2::new(
            0,
            1,
            0,
            SemanticArgumentOwnershipV1::ByValue,
            SemanticComponentStorageBindingV2::exact(vec![
                fe2o3_kernel_ir::SemanticKirComponentStorageV2::new(
                    vec![SemanticStorageProjectionV2::Field { index: 0 }],
                    0,
                    7,
                    SemanticKirComponentRepresentationV2::ScalarValue,
                    SemanticKernargSlotV2::new(8, 8, 8),
                    None,
                ),
            ]),
        );
        let wrong_offset = SemanticKernelStorageV2::new(0, 0, 0, 16, 8, vec![substituted]);
        assert!(matches!(
            validate_compiler_packing_plan_v2(
                &wrong_offset,
                &[Type::Scalar(ScalarType::U64)]
            ),
            Err(SimRuntimeBackendErrorV1::InvalidBundle(detail))
                if detail.contains("canonical KIR packing plan")
        ));

        let wrong_total = SemanticKernelStorageV2::new(0, 0, 0, 16, 8, vec![projected]);
        assert!(matches!(
            validate_compiler_packing_plan_v2(
                &wrong_total,
                &[Type::Scalar(ScalarType::U64)]
            ),
            Err(SimRuntimeBackendErrorV1::InvalidBundle(detail))
                if detail.contains("extent")
        ));

        let wrong_type = SemanticKernelStorageV2::new(
            0,
            0,
            0,
            8,
            8,
            vec![SemanticArgumentStorageV2::new(
                0,
                1,
                0,
                SemanticArgumentOwnershipV1::ByValue,
                SemanticComponentStorageBindingV2::exact(vec![
                    fe2o3_kernel_ir::SemanticKirComponentStorageV2::new(
                        vec![SemanticStorageProjectionV2::Field { index: 0 }],
                        0,
                        7,
                        SemanticKirComponentRepresentationV2::ScalarValue,
                        SemanticKernargSlotV2::new(0, 8, 8),
                        None,
                    ),
                ]),
            )],
        );
        assert!(matches!(
            validate_compiler_packing_plan_v2(
                &wrong_type,
                &[Type::Scalar(ScalarType::U32)]
            ),
            Err(SimRuntimeBackendErrorV1::InvalidBundle(detail))
                if detail.contains("canonical KIR packing plan")
        ));

        let wrong_ordinal = SemanticKernelStorageV2::new(
            0,
            0,
            0,
            8,
            8,
            vec![SemanticArgumentStorageV2::new(
                0,
                1,
                0,
                SemanticArgumentOwnershipV1::ByValue,
                SemanticComponentStorageBindingV2::exact(vec![
                    fe2o3_kernel_ir::SemanticKirComponentStorageV2::new(
                        vec![SemanticStorageProjectionV2::Field { index: 0 }],
                        1,
                        7,
                        SemanticKirComponentRepresentationV2::ScalarValue,
                        SemanticKernargSlotV2::new(0, 8, 8),
                        None,
                    ),
                ]),
            )],
        );
        assert!(matches!(
            validate_compiler_packing_plan_v2(
                &wrong_ordinal,
                &[Type::Scalar(ScalarType::U64)]
            ),
            Err(SimRuntimeBackendErrorV1::InvalidBundle(detail))
                if detail.contains("absent KIR parameter")
        ));
    }

    fn scalar_semantic_type(tag: u8, bits: u16, alignment: u64) -> SemanticTypeDeclV1 {
        let primitive = SemanticBackendPrimitiveV1::integer(false, bits, alignment);
        let maximum = if bits == 128 {
            u128::MAX
        } else {
            (1_u128 << bits) - 1
        };
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag.wrapping_add(1); 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(u64::from(bits / 8)),
                alignment,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    primitive,
                    SemanticScalarValidityRangeV1::new(0, maximum),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits,
            }),
        )
    }

    fn owned_region_slice_semantic_types(
        fields: [SemanticTypeIdV1; 3],
        pair_valid_max: Option<u64>,
    ) -> Vec<SemanticTypeDeclV1> {
        let element = scalar_semantic_type(0x80, 32, 4);
        let pointer_primitive = SemanticBackendPrimitiveV1::pointer(0, 8, 8);
        let pointer = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([0x81; 32]),
            SemanticLayoutIdentityV1::from_sha256([0x82; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    pointer_primitive,
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new(
                    SemanticTypeIdV1::from_index(0),
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        );
        let length = scalar_semantic_type(0x83, 64, 8);
        let marker = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([0x84; 32]),
            SemanticLayoutIdentityV1::from_sha256([0x85; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(0),
                1,
                SemanticBackendReprV1::Memory { sized: true },
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        );
        let backend_repr = if let Some(valid_max) = pair_valid_max {
            SemanticBackendReprV1::scalar_pair(
                SemanticBackendScalarV1::initialized(
                    pointer_primitive,
                    SemanticScalarValidityRangeV1::new(0, valid_max.into()),
                ),
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                    SemanticScalarValidityRangeV1::new(0, valid_max.into()),
                ),
            )
        } else {
            SemanticBackendReprV1::Memory { sized: true }
        };
        let wrapper = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([0x86; 32]),
            SemanticLayoutIdentityV1::from_sha256([0x87; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                backend_repr,
                false,
                SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields.into()).unwrap()),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false)
                .with_rustc_layout_is_noundef(true)
                .with_scalar_pointee_info(
                    Some(
                        SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
                    ),
                    None,
                ),
        );
        vec![element, pointer, length, marker, wrapper]
    }

    fn owned_region_slice_component(
        path: Vec<SemanticStorageProjectionV2>,
        value_slot: SemanticKernargSlotV2,
        metadata_slot: Option<SemanticKernargSlotV2>,
    ) -> fe2o3_kernel_ir::SemanticKirComponentStorageV2 {
        fe2o3_kernel_ir::SemanticKirComponentStorageV2::new(
            path,
            0,
            7,
            SemanticKirComponentRepresentationV2::RegionSlice,
            value_slot,
            metadata_slot,
        )
    }

    fn owned_region_slice_physical() -> SemanticAbiArgumentV1 {
        SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(4),
            SemanticAbiPassModeV1::Pair {
                first: SemanticAbiValueAttributesV1::plain(),
                second: SemanticAbiValueAttributesV1::plain(),
            },
        ))
    }

    #[test]
    fn owned_region_slice_requires_exact_compiler_pair_ownership_access_and_slots() {
        let exact_fields = [
            SemanticTypeIdV1::from_index(1),
            SemanticTypeIdV1::from_index(2),
            SemanticTypeIdV1::from_index(3),
        ];
        let types = owned_region_slice_semantic_types(exact_fields, Some(u64::MAX));
        let physical = owned_region_slice_physical();
        let component = owned_region_slice_component(
            Vec::new(),
            SemanticKernargSlotV2::new(0, 8, 8),
            Some(SemanticKernargSlotV2::new(8, 8, 8)),
        );
        for (ownership, access) in [
            (
                SemanticArgumentOwnershipV1::SharedBorrow,
                AccessMode::ReadOnly,
            ),
            (
                SemanticArgumentOwnershipV1::UniqueBorrow,
                AccessMode::ReadWrite,
            ),
            (
                SemanticArgumentOwnershipV1::ExclusiveOwner,
                AccessMode::WriteOnly,
            ),
        ] {
            validate_region_component_v2(
                &types,
                SemanticTypeIdV1::from_index(4),
                ownership,
                &component,
                &Type::slice(Type::Scalar(ScalarType::U32), AddressSpace::Global, access),
                &physical,
                16,
            )
            .unwrap();
        }

        for (ownership, access) in [
            (SemanticArgumentOwnershipV1::ByValue, AccessMode::ReadWrite),
            (
                SemanticArgumentOwnershipV1::SharedBorrow,
                AccessMode::ReadWrite,
            ),
            (
                SemanticArgumentOwnershipV1::ExclusiveOwner,
                AccessMode::ReadOnly,
            ),
        ] {
            assert!(
                validate_region_component_v2(
                    &types,
                    SemanticTypeIdV1::from_index(4),
                    ownership,
                    &component,
                    &Type::slice(Type::Scalar(ScalarType::U32), AddressSpace::Global, access,),
                    &physical,
                    16,
                )
                .is_err()
            );
        }

        let reordered = owned_region_slice_semantic_types(
            [
                SemanticTypeIdV1::from_index(2),
                SemanticTypeIdV1::from_index(1),
                SemanticTypeIdV1::from_index(3),
            ],
            Some(u64::MAX),
        );
        let lookalike = owned_region_slice_semantic_types(exact_fields, None);
        let narrowed = owned_region_slice_semantic_types(exact_fields, Some(u32::MAX.into()));
        for hostile in [&reordered, &lookalike, &narrowed] {
            assert!(
                validate_region_component_v2(
                    hostile,
                    SemanticTypeIdV1::from_index(4),
                    SemanticArgumentOwnershipV1::ExclusiveOwner,
                    &component,
                    &Type::slice(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Global,
                        AccessMode::WriteOnly,
                    ),
                    &physical,
                    16,
                )
                .is_err()
            );
        }

        for substituted in [
            owned_region_slice_component(
                vec![SemanticStorageProjectionV2::Field { index: 0 }],
                SemanticKernargSlotV2::new(0, 8, 8),
                Some(SemanticKernargSlotV2::new(8, 8, 8)),
            ),
            owned_region_slice_component(
                Vec::new(),
                SemanticKernargSlotV2::new(0, 4, 8),
                Some(SemanticKernargSlotV2::new(8, 8, 8)),
            ),
        ] {
            assert!(
                validate_region_component_v2(
                    &types,
                    SemanticTypeIdV1::from_index(4),
                    SemanticArgumentOwnershipV1::ExclusiveOwner,
                    &substituted,
                    &Type::slice(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Global,
                        AccessMode::WriteOnly,
                    ),
                    &physical,
                    16,
                )
                .is_err()
            );
        }

        let swapped = owned_region_slice_component(
            Vec::new(),
            SemanticKernargSlotV2::new(8, 8, 8),
            Some(SemanticKernargSlotV2::new(0, 8, 8)),
        );
        let swapped_plan = SemanticKernelStorageV2::new(
            0,
            0,
            0,
            16,
            8,
            vec![SemanticArgumentStorageV2::new(
                0,
                4,
                0,
                SemanticArgumentOwnershipV1::ExclusiveOwner,
                SemanticComponentStorageBindingV2::exact(vec![swapped]),
            )],
        );
        assert!(matches!(
            validate_compiler_packing_plan_v2(
                &swapped_plan,
                &[Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::WriteOnly,
                )],
            ),
            Err(SimRuntimeBackendErrorV1::InvalidBundle(detail))
                if detail.contains("physical slots")
        ));
    }

    fn aggregate_component(
        field: u32,
        ordinal: u32,
        slot: SemanticKernargSlotV2,
    ) -> fe2o3_kernel_ir::SemanticKirComponentStorageV2 {
        fe2o3_kernel_ir::SemanticKirComponentStorageV2::new(
            vec![SemanticStorageProjectionV2::Field { index: field }],
            ordinal,
            ordinal,
            SemanticKirComponentRepresentationV2::ScalarValue,
            slot,
            None,
        )
    }

    fn recursive_component(
        path: Vec<SemanticStorageProjectionV2>,
        ordinal: u32,
        slot: SemanticKernargSlotV2,
    ) -> fe2o3_kernel_ir::SemanticKirComponentStorageV2 {
        fe2o3_kernel_ir::SemanticKirComponentStorageV2::new(
            path,
            ordinal,
            ordinal,
            SemanticKirComponentRepresentationV2::ScalarValue,
            slot,
            None,
        )
    }

    #[test]
    fn aggregate_pair_projection_order_is_proven_against_rustc_backend_repr() {
        let u32_scalar = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
        );
        let u64_scalar = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        );
        let aggregate = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([0x73; 32]),
            SemanticLayoutIdentityV1::from_sha256([0x74; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::ScalarPair {
                    first: u32_scalar,
                    second: u64_scalar,
                },
                false,
                SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![
                    SemanticTypeIdV1::from_index(0),
                    SemanticTypeIdV1::from_index(1),
                ])
                .unwrap(),
            ),
        );
        let types = [
            scalar_semantic_type(0x70, 32, 4),
            scalar_semantic_type(0x71, 64, 8),
            aggregate,
        ];
        let physical = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(2),
            SemanticAbiPassModeV1::Pair {
                first: SemanticAbiValueAttributesV1::plain(),
                second: SemanticAbiValueAttributesV1::plain(),
            },
        ));
        let exact = [
            aggregate_component(0, 0, SemanticKernargSlotV2::new(0, 4, 4)),
            aggregate_component(1, 1, SemanticKernargSlotV2::new(8, 8, 8)),
        ];
        validate_scalar_aggregate_abi_v2(
            &types,
            SemanticTypeIdV1::from_index(2),
            &physical,
            &exact,
            &[Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U64)],
            0,
            SemanticCanonAbiV1::Rust,
            false,
        )
        .unwrap();

        let reordered = [
            aggregate_component(1, 0, SemanticKernargSlotV2::new(0, 8, 8)),
            aggregate_component(0, 1, SemanticKernargSlotV2::new(8, 4, 4)),
        ];
        assert!(matches!(
            validate_scalar_aggregate_abi_v2(
                &types,
                SemanticTypeIdV1::from_index(2),
                &physical,
                &reordered,
                &[
                    Type::Scalar(ScalarType::U64),
                    Type::Scalar(ScalarType::U32),
                ],
                0,
                SemanticCanonAbiV1::Rust,
                false,
            ),
            Err(SimRuntimeBackendErrorV1::InvalidBundle(detail))
                if detail.contains("recursive component order")
        ));

        let wrong_type = [Type::Scalar(ScalarType::U64), Type::Scalar(ScalarType::U64)];
        assert!(matches!(
            validate_scalar_aggregate_abi_v2(
                &types,
                SemanticTypeIdV1::from_index(2),
                &physical,
                &exact,
                &wrong_type,
                0,
                SemanticCanonAbiV1::Rust,
                false,
            ),
            Err(SimRuntimeBackendErrorV1::InvalidBundle(detail))
                if detail.contains("aggregate leaf")
        ));

        let wrong_ordinal = [
            aggregate_component(0, 0, SemanticKernargSlotV2::new(0, 4, 4)),
            aggregate_component(1, 2, SemanticKernargSlotV2::new(8, 8, 8)),
        ];
        assert!(matches!(
            validate_scalar_aggregate_abi_v2(
                &types,
                SemanticTypeIdV1::from_index(2),
                &physical,
                &wrong_ordinal,
                &[
                    Type::Scalar(ScalarType::U32),
                    Type::Scalar(ScalarType::U64),
                    Type::Scalar(ScalarType::U64),
                ],
                0,
                SemanticCanonAbiV1::Rust,
                false,
            ),
            Err(SimRuntimeBackendErrorV1::InvalidBundle(detail))
                if detail.contains("recursive component order")
        ));
    }

    #[test]
    fn aggregate_direct_and_zero_sized_ignore_are_explicit() {
        let u64_scalar = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        );
        let direct = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([0x76; 32]),
            SemanticLayoutIdentityV1::from_sha256([0x77; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::Scalar(u64_scalar),
                false,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(0)]).unwrap(),
            ),
        );
        let direct_types = [scalar_semantic_type(0x75, 64, 8), direct];
        let direct_abi = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(1),
            SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
        ));
        validate_scalar_aggregate_abi_v2(
            &direct_types,
            SemanticTypeIdV1::from_index(1),
            &direct_abi,
            &[aggregate_component(
                0,
                0,
                SemanticKernargSlotV2::new(0, 8, 8),
            )],
            &[Type::Scalar(ScalarType::U64)],
            0,
            SemanticCanonAbiV1::Rust,
            false,
        )
        .unwrap();

        let unit = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([0x78; 32]),
            SemanticLayoutIdentityV1::from_sha256([0x79; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(0),
                1,
                SemanticBackendReprV1::Memory { sized: true },
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        );
        let ignore_abi = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(0),
            SemanticAbiPassModeV1::Ignore,
        ));
        validate_scalar_aggregate_abi_v2(
            &[unit],
            SemanticTypeIdV1::from_index(0),
            &ignore_abi,
            &[],
            &[],
            0,
            SemanticCanonAbiV1::Rust,
            false,
        )
        .unwrap();
        assert!(matches!(
            validate_scalar_aggregate_abi_v2(
                &direct_types,
                SemanticTypeIdV1::from_index(1),
                &ignore_abi,
                &[],
                &[],
                0,
                SemanticCanonAbiV1::Rust,
                false,
            ),
            Err(SimRuntimeBackendErrorV1::InvalidBundle(detail))
                if detail.contains("recursive component roster is incomplete")
        ));
    }

    #[test]
    fn recursive_nested_indirect_contract_rejects_hostile_rosters_and_carriers() {
        use SemanticStorageProjectionV2::{ArrayElement, Field};

        let u32_scalar = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
        );
        let u64_scalar = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 64, 8),
            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
        );
        let pair = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([0x82; 32]),
            SemanticLayoutIdentityV1::from_sha256([0x83; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::ScalarPair {
                    first: u32_scalar,
                    second: u64_scalar,
                },
                false,
                SemanticAggregateLayoutV1::new(
                    vec![0, 8],
                    vec![SemanticPaddingV1::new(4, 4).unwrap()],
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                SemanticAggregateTypeV1::new(vec![
                    SemanticTypeIdV1::from_index(0),
                    SemanticTypeIdV1::from_index(1),
                ])
                .unwrap(),
            ),
        );
        let array = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([0x84; 32]),
            SemanticLayoutIdentityV1::from_sha256([0x85; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                4,
                2,
                SemanticFieldsShapeV1::array(2, 2),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::Memory { sized: true },
                None,
                false,
                None,
                2,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Array {
                element: SemanticTypeIdV1::from_index(2),
                length: 2,
            },
        );
        let nested = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([0x86; 32]),
            SemanticLayoutIdentityV1::from_sha256([0x87; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(24),
                8,
                SemanticBackendReprV1::Memory { sized: true },
                false,
                SemanticAggregateLayoutV1::new(
                    vec![0, 16],
                    vec![SemanticPaddingV1::new(20, 4).unwrap()],
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![
                    SemanticTypeIdV1::from_index(3),
                    SemanticTypeIdV1::from_index(4),
                ])
                .unwrap(),
            ),
        );
        let constrained_u16 = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([0x88; 32]),
            SemanticLayoutIdentityV1::from_sha256([0x89; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(2),
                2,
                SemanticBackendReprV1::Scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 16, 2),
                    SemanticScalarValidityRangeV1::new(1, 10),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::ValidityScalar(
                SemanticValidityScalarTypeV1::new(
                    SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 16,
                    },
                    vec![SemanticScalarValidityRangeV1::new(1, 10)],
                )
                .unwrap(),
            ),
        );
        let types = vec![
            scalar_semantic_type(0x80, 32, 4),
            scalar_semantic_type(0x81, 64, 8),
            constrained_u16,
            pair,
            array,
            nested,
        ];
        let components = vec![
            recursive_component(
                vec![Field { index: 0 }, Field { index: 0 }],
                0,
                SemanticKernargSlotV2::new(0, 4, 4),
            ),
            recursive_component(
                vec![Field { index: 0 }, Field { index: 1 }],
                1,
                SemanticKernargSlotV2::new(8, 8, 8),
            ),
            recursive_component(
                vec![Field { index: 1 }, ArrayElement { index: 0 }],
                2,
                SemanticKernargSlotV2::new(16, 2, 2),
            ),
            recursive_component(
                vec![Field { index: 1 }, ArrayElement { index: 1 }],
                3,
                SemanticKernargSlotV2::new(18, 2, 2),
            ),
        ];
        let kir_types = [
            Type::Scalar(ScalarType::U32),
            Type::Scalar(ScalarType::U64),
            Type::Scalar(ScalarType::U16),
            Type::Scalar(ScalarType::U16),
        ];
        let indirect_attributes = SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(
                true,
                Some(SemanticAbiPointerCaptureV1::CapturesAddress),
                true,
                false,
                false,
                true,
            ),
            SemanticAbiExtensionV1::None,
            24,
            Some(8),
        )
        .unwrap();
        let indirect = |metadata_attributes, on_stack| {
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                SemanticTypeIdV1::from_index(5),
                SemanticAbiPassModeV1::Indirect {
                    attributes: indirect_attributes,
                    metadata_attributes,
                    on_stack,
                },
            ))
        };
        validate_scalar_aggregate_abi_v2(
            &types,
            SemanticTypeIdV1::from_index(5),
            &indirect(None, false),
            &components,
            &kir_types,
            0,
            SemanticCanonAbiV1::GpuKernel,
            true,
        )
        .unwrap();

        let validity =
            semantic_component_validity_v2(&types, SemanticTypeIdV1::from_index(2)).unwrap();
        let validity_kernel = materialization_kernel(
            vec![scalar_argument(
                0,
                2,
                ScalarType::U16,
                ArgumentMaterializationV1::ExactBytes {
                    validity,
                    guards: Vec::new(),
                },
            )],
            2,
        );
        assert!(matches!(
            prepare_arguments(&validity_kernel, &11_u16.to_le_bytes(), &[], &HashMap::new()),
            Err(SimRuntimeBackendErrorV1::InvalidArguments(detail))
                if detail.contains("Rust validity")
        ));

        let mut overlapping_types = types.clone();
        overlapping_types[5] = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([0x93; 32]),
            SemanticLayoutIdentityV1::from_sha256([0x94; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(24),
                8,
                SemanticBackendReprV1::Memory { sized: true },
                false,
                SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![
                    SemanticTypeIdV1::from_index(3),
                    SemanticTypeIdV1::from_index(4),
                ])
                .unwrap(),
            ),
        );
        assert!(matches!(
            validate_scalar_aggregate_abi_v2(
                &overlapping_types,
                SemanticTypeIdV1::from_index(5),
                &indirect(None, false),
                &components,
                &kir_types,
                0,
                SemanticCanonAbiV1::GpuKernel,
                true,
            ),
            Err(SimRuntimeBackendErrorV1::InvalidBundle(detail))
                if detail.contains("overlap")
        ));

        for hostile_components in [
            components[..3].to_vec(),
            {
                let mut reordered = components.clone();
                reordered.swap(1, 2);
                reordered
            },
            {
                let mut wrong_path = components.clone();
                wrong_path[3] = recursive_component(
                    vec![Field { index: 1 }, ArrayElement { index: 0 }],
                    3,
                    SemanticKernargSlotV2::new(18, 2, 2),
                );
                wrong_path
            },
        ] {
            assert!(matches!(
                validate_scalar_aggregate_abi_v2(
                    &types,
                    SemanticTypeIdV1::from_index(5),
                    &indirect(None, false),
                    &hostile_components,
                    &kir_types,
                    0,
                    SemanticCanonAbiV1::GpuKernel,
                    true,
                ),
                Err(SimRuntimeBackendErrorV1::InvalidBundle(_))
            ));
        }
        for hostile in [
            indirect(Some(SemanticAbiValueAttributesV1::plain()), false),
            indirect(None, true),
        ] {
            assert!(matches!(
                validate_scalar_aggregate_abi_v2(
                    &types,
                    SemanticTypeIdV1::from_index(5),
                    &hostile,
                    &components,
                    &kir_types,
                    0,
                    SemanticCanonAbiV1::GpuKernel,
                    true,
                ),
                Err(SimRuntimeBackendErrorV1::UnsupportedBundle(detail))
                    if detail.contains("recursive logical-component rule")
            ));
        }
    }

    #[test]
    fn simple_rust_cast_contract_is_exact_and_foreign_casts_stay_unavailable() {
        let element = scalar_semantic_type(0x90, 8, 1);
        let array = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([0x91; 32]),
            SemanticLayoutIdentityV1::from_sha256([0x92; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                3,
                1,
                SemanticFieldsShapeV1::array(1, 3),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::Memory { sized: true },
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Array {
                element: SemanticTypeIdV1::from_index(0),
                length: 3,
            },
        );
        let types = [element, array];
        let components = (0..3)
            .map(|index| {
                recursive_component(
                    vec![SemanticStorageProjectionV2::ArrayElement { index }],
                    index as u32,
                    SemanticKernargSlotV2::new(index as u32, 1, 1),
                )
            })
            .collect::<Vec<_>>();
        let cast_with_attributes = |width, attributes| {
            let register =
                SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, width).unwrap();
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                SemanticTypeIdV1::from_index(1),
                SemanticAbiPassModeV1::cast(
                    false,
                    SemanticAbiCastV1::new(
                        [None; 8],
                        None,
                        SemanticAbiUniformV1::new(register, width).unwrap(),
                        attributes,
                    ),
                ),
            ))
        };
        let cast = |width| cast_with_attributes(width, SemanticAbiValueAttributesV1::plain());
        let kir_types = vec![Type::Scalar(ScalarType::U8); 3];
        validate_scalar_aggregate_abi_v2(
            &types,
            SemanticTypeIdV1::from_index(1),
            &cast(3),
            &components,
            &kir_types,
            0,
            SemanticCanonAbiV1::Rust,
            false,
        )
        .unwrap();
        for (physical, canon_abi) in [
            (cast(2), SemanticCanonAbiV1::Rust),
            (cast(3), SemanticCanonAbiV1::GpuKernel),
            (
                cast_with_attributes(
                    3,
                    SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(
                            true,
                            Some(SemanticAbiPointerCaptureV1::CapturesNone),
                            false,
                            false,
                            false,
                            false,
                        ),
                        SemanticAbiExtensionV1::None,
                        0,
                        None,
                    )
                    .unwrap(),
                ),
                SemanticCanonAbiV1::Rust,
            ),
        ] {
            assert!(matches!(
                validate_scalar_aggregate_abi_v2(
                    &types,
                    SemanticTypeIdV1::from_index(1),
                    &physical,
                    &components,
                    &kir_types,
                    0,
                    canon_abi,
                    false,
                ),
                Err(SimRuntimeBackendErrorV1::UnsupportedBundle(_))
            ));
        }
    }

    fn direct_enum_decoder() -> EnumDecoderV1 {
        EnumDecoderV1 {
            byte_offset: 4,
            byte_width: 1,
            variants: vec![
                EnumVariantValueV1 {
                    index: 0,
                    discriminant: 3,
                    uninhabited: false,
                },
                EnumVariantValueV1 {
                    index: 1,
                    discriminant: 7,
                    uninhabited: false,
                },
            ],
            encoding: EnumDecoderEncodingV1::Direct {
                physical_bits: 8,
                logical_signed: false,
                logical_bits: 8,
                validity: Some(SemanticScalarValidityRangeV1::new(3, 7)),
            },
        }
    }

    #[test]
    fn inactive_enum_payload_is_poisoned_before_kir_argument_use() {
        let decoder = direct_enum_decoder();
        let kernel = materialization_kernel(
            vec![
                scalar_argument(
                    0,
                    2,
                    ScalarType::U16,
                    ArgumentMaterializationV1::ExactBytes {
                        validity: Vec::new(),
                        guards: Vec::new(),
                    },
                ),
                scalar_argument(
                    8,
                    8,
                    ScalarType::U64,
                    ArgumentMaterializationV1::ExactBytes {
                        validity: Vec::new(),
                        guards: Vec::new(),
                    },
                ),
                scalar_argument(
                    4,
                    0,
                    ScalarType::U8,
                    ArgumentMaterializationV1::EnumDiscriminant {
                        decoder: decoder.clone(),
                        guards: Vec::new(),
                    },
                ),
                scalar_argument(
                    5,
                    2,
                    ScalarType::U16,
                    ArgumentMaterializationV1::ExactBytes {
                        validity: Vec::new(),
                        guards: vec![EnumVariantGuardV1 {
                            decoder,
                            required_variant: 1,
                        }],
                    },
                ),
            ],
            16,
        );
        let mut bytes = [0xcc; 16];
        bytes[0..2].copy_from_slice(&0x1234_u16.to_le_bytes());
        bytes[4] = 3;
        bytes[5..7].copy_from_slice(&0xffff_u16.to_le_bytes());
        bytes[8..16].copy_from_slice(&0x0102_0304_0506_0708_u64.to_le_bytes());
        assert!(matches!(
            prepare_arguments(&kernel, &bytes, &[], &HashMap::new()),
            Err(SimRuntimeBackendErrorV1::InvalidArguments(detail))
                if detail.contains("inactive enum payload is poison")
        ));

        bytes[4] = 7;
        bytes[5..7].copy_from_slice(&0xabcd_u16.to_le_bytes());
        assert_eq!(
            scalar_bits(prepare_arguments(&kernel, &bytes, &[], &HashMap::new()).unwrap()),
            vec![0x1234, 0x0102_0304_0506_0708, 7, 0xabcd]
        );
    }

    #[test]
    fn direct_and_niche_enum_discriminants_are_exact_and_invalid_tags_fail() {
        let direct = materialization_kernel(
            vec![scalar_argument(
                4,
                0,
                ScalarType::U8,
                ArgumentMaterializationV1::EnumDiscriminant {
                    decoder: direct_enum_decoder(),
                    guards: Vec::new(),
                },
            )],
            8,
        );
        let mut bytes = [0; 8];
        bytes[4] = 7;
        assert_eq!(
            scalar_bits(prepare_arguments(&direct, &bytes, &[], &HashMap::new()).unwrap()),
            vec![7]
        );
        bytes[4] = 5;
        assert!(matches!(
            prepare_arguments(&direct, &bytes, &[], &HashMap::new()),
            Err(SimRuntimeBackendErrorV1::InvalidArguments(detail))
                if detail.contains("direct enum tag")
        ));

        let niche_decoder = EnumDecoderV1 {
            byte_offset: 0,
            byte_width: 1,
            variants: vec![
                EnumVariantValueV1 {
                    index: 0,
                    discriminant: 11,
                    uninhabited: false,
                },
                EnumVariantValueV1 {
                    index: 1,
                    discriminant: 29,
                    uninhabited: false,
                },
            ],
            encoding: EnumDecoderEncodingV1::Niche {
                physical_bits: 8,
                source_validity: SemanticScalarValidityRangeV1::new(1, 255),
                untagged_variant: 1,
                niche_variants_start: 0,
                niche_variants_end: 0,
                niche_start: 0,
            },
        };
        let niche = materialization_kernel(
            vec![scalar_argument(
                0,
                0,
                ScalarType::U8,
                ArgumentMaterializationV1::EnumDiscriminant {
                    decoder: niche_decoder,
                    guards: Vec::new(),
                },
            )],
            1,
        );
        assert_eq!(
            scalar_bits(prepare_arguments(&niche, &[0], &[], &HashMap::new()).unwrap()),
            vec![11]
        );
        assert_eq!(
            scalar_bits(prepare_arguments(&niche, &[9], &[], &HashMap::new()).unwrap()),
            vec![29]
        );
    }

    #[test]
    fn aggregate_scalar_validity_is_checked_before_execution() {
        let kernel = materialization_kernel(
            vec![scalar_argument(
                0,
                1,
                ScalarType::U8,
                ArgumentMaterializationV1::ExactBytes {
                    validity: vec![SemanticScalarValidityRangeV1::new(1, 10)],
                    guards: Vec::new(),
                },
            )],
            1,
        );
        assert!(matches!(
            prepare_arguments(&kernel, &[0], &[], &HashMap::new()),
            Err(SimRuntimeBackendErrorV1::InvalidArguments(detail))
                if detail.contains("Rust validity")
        ));
        assert_eq!(
            scalar_bits(prepare_arguments(&kernel, &[7], &[], &HashMap::new()).unwrap()),
            vec![7]
        );
    }
}
