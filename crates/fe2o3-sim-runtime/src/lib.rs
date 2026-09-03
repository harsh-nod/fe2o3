#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, FunctionRole, KernelId, MAX_SIMULATION_BUNDLE_BYTES_V3, ScalarType,
    SemanticArgumentOwnershipV1, SemanticKirStorageRepresentationV1, SemanticStorageBindingV1,
    SemanticStorageMapV1, Type, VerifiedCanonicalKernelIrV7, VerifiedSimulationBundleV3,
};
use fe2o3_kir_sim::{AdmittedSimulationModuleV1, ScalarBitsV1};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticAbiPassModeV1, SemanticLocalRoleV1, SemanticMirLimitsV1,
    SemanticSourceArgumentOwnershipV1,
};
use fe2o3_runtime::{
    BackendBindingV1, BackendDeviceDescriptionV1, BackendLaunchV1, BackendMemoryRegionV1,
    BackendPollV1, MAX_RUNTIME_ALLOCATIONS_V1, MAX_RUNTIME_DEPENDENCIES_V1, MAX_RUNTIME_EVENTS_V1,
    MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1, MAX_RUNTIME_KERNEL_NAME_BYTES_V1,
    MAX_RUNTIME_KERNELS_V1, MAX_RUNTIME_MODULES_V1, MAX_RUNTIME_STREAMS_V1,
    MAX_RUNTIME_SUBMISSIONS_V1, RuntimeAccessV1, RuntimeBackendFailureV1, RuntimeBackendV1,
    RuntimeCapabilitiesV1, RuntimeMemoryKindV1,
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
    unsupported: Option<String>,
}

#[derive(Clone)]
struct ArgumentRecordV1 {
    offset: usize,
    size: usize,
    ty: Type,
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
        if image.len() > MAX_SIMULATION_BUNDLE_BYTES_V3 {
            return Err(RuntimeBackendFailureV1::Rejected(
                SimRuntimeBackendErrorV1::InvalidBundle(
                    "bundle exceeds the V3 byte limit".to_owned(),
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
        require_capacity(self.submissions.len(), MAX_RUNTIME_SUBMISSIONS_V1)?;
        if launch.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(rejected_arguments("dependency count"));
        }
        if launch.explicit_kernarg.len() > MAX_RUNTIME_EXPLICIT_KERNARG_BYTES_V1 {
            return Err(rejected_arguments("explicit kernarg length"));
        }
        if launch.geometry.dynamic_shared_bytes != 0 {
            return Err(RuntimeBackendFailureV1::Rejected(
                SimRuntimeBackendErrorV1::UnsupportedBundle(
                    "dynamic shared memory is not represented by RuntimeBackendV1 simulation arguments"
                        .to_owned(),
                ),
            ));
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

fn parse_bundle(
    image: &[u8],
    target: VirtualTargetProfileV1,
) -> Result<ParsedBundleV1, SimRuntimeBackendErrorV1> {
    let bundle = VerifiedSimulationBundleV3::from_canonical_bytes(image.to_vec())
        .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
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
    let (canonical, module) = VerifiedCanonicalKernelIrV7::from_canonical_bytes_with_module(
        bundle.inner_v2().inner_v1().canonical_kir_v7().to_vec(),
    )
    .map_err(|error| SimRuntimeBackendErrorV1::InvalidBundle(error.to_string()))?;
    let mut kernels = HashMap::new();
    for storage_kernel in storage.kernels() {
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
        let (arguments, explicit_byte_len, unsupported) = argument_layout(
            storage_kernel.arguments(),
            root.abi(),
            semantic.types(),
            &function.signature.parameters,
        )?;
        if kernels
            .insert(
                kernel.id.as_str().to_owned(),
                KernelRecordV1 {
                    module: None,
                    kernel: kernel.id.clone(),
                    signature: *root.abi().identity().as_bytes(),
                    explicit_byte_len,
                    arguments,
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
    let admitted =
        AdmittedSimulationModuleV1::admit(canonical, fe2o3_kir_sim::SimulationLimitsV1::default())
            .map_err(|error| SimRuntimeBackendErrorV1::UnsupportedBundle(error.to_string()))?;
    Ok(ParsedBundleV1 { admitted, kernels })
}

fn argument_layout(
    storage: &[fe2o3_kernel_ir::SemanticArgumentStorageV1],
    abi: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionAbiV1,
    semantic_types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    kir_types: &[Type],
) -> Result<(Vec<ArgumentRecordV1>, usize, Option<String>), SimRuntimeBackendErrorV1> {
    let mut arguments = Vec::with_capacity(storage.len());
    let source_abi_arguments = abi
        .arguments()
        .iter()
        .filter(|argument| argument.is_source())
        .collect::<Vec<_>>();
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
            size,
            ty: kir_ty,
        });
    }
    if arguments.len() != kir_types.len() {
        unsupported
            .get_or_insert_with(|| "source-to-KIR argument count is not one-to-one".to_owned());
    }
    Ok((arguments, align_up(next, maximum_alignment)?, unsupported))
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
    let mut output = Vec::with_capacity(kernel.arguments.len());
    for argument in &kernel.arguments {
        let bytes = kernarg
            .get(argument.offset..argument.offset + argument.size)
            .ok_or_else(|| {
                SimRuntimeBackendErrorV1::InvalidArguments(
                    "argument range exceeds kernarg".to_owned(),
                )
            })?;
        match &argument.ty {
            Type::Scalar(ty) => {
                let mut little = [0_u8; 16];
                little[..bytes.len()].copy_from_slice(bytes);
                let bits = u128::from_le_bytes(little);
                let value =
                    ScalarBitsV1::new(*ty, bits, fe2o3_kir_sim::SimulationTargetV1::amdgpu_64())
                        .map_err(|error| {
                            SimRuntimeBackendErrorV1::InvalidArguments(error.to_string())
                        })?;
                output.push(VirtualArgumentV1::Scalar(value));
            }
            Type::Pointer(pointer) => {
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
            Type::Slice(slice) => {
                require_zero_pointer_placeholder(&bytes[..8])?;
                let binding = binding_at(argument.offset, &mut consumed, &by_offset)?;
                let length_bytes = bytes.get(8..16).ok_or_else(|| {
                    SimRuntimeBackendErrorV1::InvalidArguments(
                        "slice metadata is truncated".to_owned(),
                    )
                })?;
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
            Type::Unit => {
                return Err(SimRuntimeBackendErrorV1::UnsupportedBundle(
                    "unit KIR parameters are not materialized".to_owned(),
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
                    Ok(dependencies) => match runtime.submit(
                        queue,
                        module,
                        VirtualDispatchRequestV1 {
                            kernel: request.kernel,
                            grid: request.grid,
                            workgroup: request.workgroup,
                            arguments: request.arguments,
                            dependencies,
                        },
                    ) {
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
                                    completion, ..
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
                    },
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
    use fe2o3_runtime::{RuntimeContextV1, RuntimeErrorV1, RuntimeValidationErrorV1};
    use std::time::Duration;

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
}
