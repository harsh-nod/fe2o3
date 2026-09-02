use crate::api::{ApiError, DirectRuntimeApi, DispatchApi, QueueHandle, SymbolFacts};
use crate::environment::{AdapterCore, HsaRuntimeAdapterError, ReviewedHsaRuntimeAdapterV1};
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_core::GpuContext;
use fe2o3_runtime::{
    BackendBindingV1, BackendDeviceDescriptionV1, BackendLaunchV1, BackendMemoryRegionV1,
    BackendPollV1, RuntimeAccessV1, RuntimeBackendFailureV1, RuntimeBackendV1,
    RuntimeCapabilitiesV1, RuntimeMemoryKindV1,
};
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, Instant};

const BACKEND_DEVICE_V1: u64 = 1;
const HSA_SYMBOL_KIND_KERNEL: u32 = 1;
const IMPLICIT_KERNARG_BYTES: usize = 256;
const BLOCK_COUNT_X: usize = 0;
const BLOCK_COUNT_Y: usize = 4;
const BLOCK_COUNT_Z: usize = 8;
const GROUP_SIZE_X: usize = 12;
const GROUP_SIZE_Y: usize = 14;
const GROUP_SIZE_Z: usize = 16;
const REMAINDER_X: usize = 18;
const REMAINDER_Y: usize = 20;
const REMAINDER_Z: usize = 22;
const GRID_DIMS: usize = 64;
const DYNAMIC_LDS_SIZE: usize = 120;
const QUEUE_PTR: usize = 200;
const WAIT_SPINS_V1: u32 = 32;
const WAIT_YIELDS_V1: u32 = 8;
const WAIT_INITIAL_SLEEP_V1: Duration = Duration::from_micros(50);
const WAIT_MAX_SLEEP_V1: Duration = Duration::from_millis(1);

/// Failure reported by the reviewed HSA implementation of the runtime SPI.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ReviewedHsaRuntimeBackendErrorV1 {
    BackendTerminal,
    Unsupported(&'static str),
    InvalidHandle(&'static str),
    InvalidArgument(&'static str),
    ResourceBusy(&'static str),
    IdentityExhausted,
    NativeCall {
        operation: &'static str,
        status: i32,
    },
}

impl fmt::Display for ReviewedHsaRuntimeBackendErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendTerminal => formatter.write_str("reviewed HSA backend is terminal"),
            Self::Unsupported(operation) => {
                write!(formatter, "unsupported HSA operation: {operation}")
            }
            Self::InvalidHandle(kind) => write!(formatter, "unknown or retired HSA {kind} handle"),
            Self::InvalidArgument(field) => {
                write!(formatter, "invalid HSA backend argument: {field}")
            }
            Self::ResourceBusy(kind) => write!(formatter, "HSA {kind} still has pending work"),
            Self::IdentityExhausted => {
                formatter.write_str("reviewed HSA backend identity exhausted")
            }
            Self::NativeCall { operation, status } => {
                write!(formatter, "{operation} failed with status {status}")
            }
        }
    }
}

impl Error for ReviewedHsaRuntimeBackendErrorV1 {}

impl From<ApiError> for ReviewedHsaRuntimeBackendErrorV1 {
    fn from(error: ApiError) -> Self {
        Self::NativeCall {
            operation: error.operation,
            status: error.status,
        }
    }
}

type BackendFailure = RuntimeBackendFailureV1<ReviewedHsaRuntimeBackendErrorV1>;
type BackendResult<T> = Result<T, BackendFailure>;
/// Latest submission on each stream known to happen before a published packet.
type CausalFrontier = BTreeMap<u64, u64>;

/// Runtime-facade adapter over one reviewed HIP-correlated HSA device.
///
/// Handles returned through `RuntimeBackendV1` are adapter-local identities;
/// native HSA handles and host addresses never cross the SPI boundary.
pub struct ReviewedHsaRuntimeBackendV1 {
    state: BackendState<DirectRuntimeApi>,
    _not_sync: PhantomData<Cell<()>>,
}

impl ReviewedHsaRuntimeBackendV1 {
    /// Opens the reviewed gfx942 HSA device correlated with `context`.
    ///
    /// # Safety
    ///
    /// For the complete lifetime of the returned backend, every code object
    /// passed to the safe runtime facade must be trusted for in-process GPU
    /// execution, every typed signature must describe its exact kernarg ABI,
    /// and every declared binding must conservatively cover the kernel's memory
    /// effects. Use the worker-hosted runtime boundary for untrusted modules.
    pub unsafe fn new(context: Arc<GpuContext>) -> Result<Self, HsaRuntimeAdapterError> {
        Self::new_for_processor(context, "gfx942")
    }

    /// Opens the reviewed gfx950 HSA device correlated with `context`.
    ///
    /// # Safety
    ///
    /// The caller must uphold the same lifetime-wide code-object, kernarg, and
    /// memory-effect invariants documented on [`Self::new`].
    pub unsafe fn new_gfx950(context: Arc<GpuContext>) -> Result<Self, HsaRuntimeAdapterError> {
        Self::new_for_processor(context, "gfx950")
    }

    fn new_for_processor(
        context: Arc<GpuContext>,
        processor: &'static str,
    ) -> Result<Self, HsaRuntimeAdapterError> {
        let core = ReviewedHsaRuntimeAdapterV1::with_api_for_processor(
            context,
            DirectRuntimeApi::new(),
            processor,
        )?;
        Ok(Self {
            state: BackendState::new(core),
            _not_sync: PhantomData,
        })
    }
}

impl RuntimeBackendV1 for ReviewedHsaRuntimeBackendV1 {
    type Error = ReviewedHsaRuntimeBackendErrorV1;

    fn enumerate_devices_v1(&mut self) -> BackendResult<Vec<BackendDeviceDescriptionV1>> {
        self.state.enumerate_devices_v1()
    }

    fn create_stream_v1(&mut self, device: u64) -> BackendResult<u64> {
        self.state.create_stream_v1(device)
    }

    fn destroy_stream_v1(&mut self, stream: u64) -> BackendResult<()> {
        self.state.destroy_stream_v1(stream)
    }

    fn allocate_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> BackendResult<u64> {
        self.state.allocate_v1(device, kind, byte_len, alignment)
    }

    fn release_allocation_v1(&mut self, allocation: u64) -> BackendResult<()> {
        self.state.release_allocation_v1(allocation)
    }

    fn write_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        bytes: &[u8],
    ) -> BackendResult<()> {
        self.state
            .write_allocation_v1(allocation, byte_offset, bytes)
    }

    fn read_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        destination: &mut [u8],
    ) -> BackendResult<()> {
        self.state
            .read_allocation_v1(allocation, byte_offset, destination)
    }

    fn load_module_v1(&mut self, device: u64, image: &[u8]) -> BackendResult<u64> {
        self.state.load_module_v1(device, image)
    }

    fn unload_module_v1(&mut self, module: u64) -> BackendResult<()> {
        self.state.unload_module_v1(module)
    }

    fn resolve_kernel_v1(
        &mut self,
        module: u64,
        name: &str,
        signature: [u8; 32],
    ) -> BackendResult<u64> {
        self.state.resolve_kernel_v1(module, name, signature)
    }

    fn submit_v1(&mut self, launch: BackendLaunchV1<'_>) -> BackendResult<u64> {
        self.state.submit_v1(launch)
    }

    fn poll_v1(&mut self, submission: u64) -> BackendResult<BackendPollV1> {
        self.state.poll_v1(submission)
    }

    fn wait_v1(&mut self, submission: u64, deadline: Instant) -> BackendResult<BackendPollV1> {
        self.state.wait_v1(submission, deadline)
    }

    fn release_submission_v1(&mut self, submission: u64) -> BackendResult<()> {
        self.state.release_submission_v1(submission)
    }

    fn record_event_v1(&mut self, stream: u64, submission: u64) -> BackendResult<u64> {
        self.state.record_event_v1(stream, submission)
    }

    fn release_event_v1(&mut self, event: u64) -> BackendResult<()> {
        self.state.release_event_v1(event)
    }

    fn peer_copy_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> BackendResult<u64> {
        self.state
            .peer_copy_v1(stream, source, destination, dependencies)
    }
}

impl Drop for ReviewedHsaRuntimeBackendV1 {
    fn drop(&mut self) {
        if self.state.force_cleanup().is_err() {
            std::process::abort();
        }
    }
}

struct BackendState<A: DispatchApi> {
    core: AdapterCore<A>,
    streams: BTreeMap<u64, StreamRecord>,
    allocations: BTreeMap<u64, AllocationRecord>,
    modules: BTreeMap<u64, ModuleRecord>,
    kernels: BTreeMap<u64, KernelRecord>,
    submissions: BTreeMap<u64, SubmissionRecord>,
    events: BTreeMap<u64, EventRecord>,
    pending_accesses: PendingAccessIndex,
    retained_images: Vec<Box<[u8]>>,
    retained_addresses: Vec<usize>,
    next_identity: u64,
    terminal: bool,
}

struct StreamRecord {
    queue: Option<QueueHandle>,
    faulted: bool,
    submissions: BTreeSet<u64>,
    order_frontier: CausalFrontier,
}

#[derive(Clone, Copy)]
struct AllocationRecord {
    address: usize,
    byte_len: u64,
}

struct ModuleRecord {
    _bytes: Box<[u8]>,
    reader: Option<u64>,
    executable: Option<u64>,
    _loaded: u64,
}

#[derive(Clone, Copy)]
struct KernelRecord {
    module: u64,
    kernel_object: u64,
    kernarg_size: u32,
    kernarg_alignment: u32,
    group_segment_size: u32,
    private_segment_size: u32,
    _signature: [u8; 32],
}

struct SubmissionRecord {
    stream: u64,
    module: u64,
    regions: Vec<BackendMemoryRegionV1>,
    dependencies: Vec<u64>,
    order_frontier: CausalFrontier,
    pending_dependents: usize,
    event_references: usize,
    signal: Option<u64>,
    kernarg_address: Option<usize>,
    outcome: Option<BackendPollV1>,
}

struct EventRecord {
    submission: u64,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct PendingRegionKey {
    byte_offset: u64,
    submission: u64,
    region_index: usize,
}

#[derive(Clone, Copy)]
struct PendingRegion {
    key: PendingRegionKey,
    byte_end: u64,
    access: RuntimeAccessV1,
}

struct PendingIntervalNode {
    region: PendingRegion,
    height: usize,
    max_end: u64,
    max_write_end: u64,
    max_submission: u64,
    max_write_submission: u64,
    left: Option<Box<Self>>,
    right: Option<Box<Self>>,
}

impl PendingIntervalNode {
    fn new(region: PendingRegion) -> Box<Self> {
        let (max_write_end, max_write_submission) = if region.access == RuntimeAccessV1::Read {
            (0, 0)
        } else {
            (region.byte_end, region.key.submission)
        };
        Box::new(Self {
            region,
            height: 1,
            max_end: region.byte_end,
            max_write_end,
            max_submission: region.key.submission,
            max_write_submission,
            left: None,
            right: None,
        })
    }

    fn refresh(&mut self) {
        self.height = 1 + pending_node_height(&self.left).max(pending_node_height(&self.right));
        self.max_end = self
            .region
            .byte_end
            .max(pending_node_max_end(&self.left, false))
            .max(pending_node_max_end(&self.right, false));
        self.max_submission = self
            .region
            .key
            .submission
            .max(pending_node_max_submission(&self.left, false))
            .max(pending_node_max_submission(&self.right, false));
        let (own_write_end, own_write_submission) = if self.region.access == RuntimeAccessV1::Read {
            (0, 0)
        } else {
            (self.region.byte_end, self.region.key.submission)
        };
        self.max_write_end = own_write_end
            .max(pending_node_max_end(&self.left, true))
            .max(pending_node_max_end(&self.right, true));
        self.max_write_submission = own_write_submission
            .max(pending_node_max_submission(&self.left, true))
            .max(pending_node_max_submission(&self.right, true));
    }
}

#[derive(Default)]
struct PendingIntervalTree {
    root: Option<Box<PendingIntervalNode>>,
    len: usize,
}

impl PendingIntervalTree {
    fn insert(&mut self, region: PendingRegion) {
        self.root = Some(insert_pending_interval_node(self.root.take(), region));
        self.len += 1;
    }

    fn remove(&mut self, key: PendingRegionKey) {
        let (root, removed) = remove_pending_interval_node(self.root.take(), key);
        debug_assert!(removed, "pending interval must exist until quiescence");
        self.root = root;
        if removed {
            self.len -= 1;
        }
    }

    fn conflicts(
        &self,
        byte_offset: u64,
        byte_end: u64,
        access: RuntimeAccessV1,
        ordered_through: u64,
        instrumentation: &QueryInstrumentation,
    ) -> bool {
        find_pending_interval_conflict(
            self.root.as_deref(),
            byte_offset,
            byte_end,
            access,
            ordered_through,
            instrumentation,
        )
    }

    const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[cfg(test)]
    fn height(&self) -> usize {
        pending_node_height(&self.root)
    }
}

/// Pending accesses partitioned by allocation and stream.
///
/// Each leaf tree is an AVL interval tree. Its submission maxima let hazard
/// admission prune the prefix already covered by the candidate's causal
/// frontier without walking transitive dependency edges.
#[derive(Default)]
struct PendingAccessIndex {
    allocations: BTreeMap<u64, BTreeMap<u64, PendingIntervalTree>>,
    stream_entries: BTreeMap<u64, usize>,
    instrumentation: QueryInstrumentation,
}

#[derive(Default)]
struct QueryInstrumentation {
    #[cfg(test)]
    visits: Cell<usize>,
}

impl QueryInstrumentation {
    fn record_visit(&self) {
        #[cfg(test)]
        self.visits.set(self.visits.get().saturating_add(1));
    }
}

impl PendingAccessIndex {
    fn insert(&mut self, submission: u64, stream: u64, regions: &[BackendMemoryRegionV1]) {
        for (region_index, region) in regions.iter().enumerate() {
            let byte_end = region
                .byte_offset
                .checked_add(region.byte_len)
                .expect("published regions were range-checked before indexing");
            self.allocations
                .entry(region.allocation)
                .or_default()
                .entry(stream)
                .or_default()
                .insert(PendingRegion {
                    key: PendingRegionKey {
                        byte_offset: region.byte_offset,
                        submission,
                        region_index,
                    },
                    byte_end,
                    access: region.access,
                });
            *self.stream_entries.entry(stream).or_default() += 1;
        }
    }

    fn remove(&mut self, submission: u64, stream: u64, regions: &[BackendMemoryRegionV1]) {
        for (region_index, region) in regions.iter().enumerate() {
            let mut remove_allocation = false;
            if let Some(streams) = self.allocations.get_mut(&region.allocation) {
                let mut remove_stream = false;
                if let Some(tree) = streams.get_mut(&stream) {
                    tree.remove(PendingRegionKey {
                        byte_offset: region.byte_offset,
                        submission,
                        region_index,
                    });
                    remove_stream = tree.is_empty();
                } else {
                    debug_assert!(false, "pending stream index must exist until quiescence");
                }
                if remove_stream {
                    streams.remove(&stream);
                }
                remove_allocation = streams.is_empty();
            } else {
                debug_assert!(
                    false,
                    "pending allocation index must exist until quiescence"
                );
            }
            if remove_allocation {
                self.allocations.remove(&region.allocation);
            }
            let remove_count = match self.stream_entries.get_mut(&stream) {
                Some(count) => {
                    *count = count
                        .checked_sub(1)
                        .expect("pending stream entry count cannot underflow");
                    *count == 0
                }
                None => {
                    debug_assert!(false, "pending stream entry count must exist");
                    false
                }
            };
            if remove_count {
                self.stream_entries.remove(&stream);
            }
        }
    }

    fn allocation_is_pending(&self, allocation: u64) -> bool {
        self.allocations.contains_key(&allocation)
    }

    fn stream_has_pending_accesses(&self, stream: u64) -> bool {
        self.stream_entries.contains_key(&stream)
    }

    fn host_conflicts(
        &self,
        allocation: u64,
        byte_offset: u64,
        byte_end: u64,
        host_writes: bool,
    ) -> bool {
        let access = if host_writes {
            RuntimeAccessV1::Write
        } else {
            RuntimeAccessV1::Read
        };
        self.allocations.get(&allocation).is_some_and(|streams| {
            streams
                .values()
                .any(|tree| tree.conflicts(byte_offset, byte_end, access, 0, &self.instrumentation))
        })
    }

    fn launch_conflicts(
        &self,
        stream: u64,
        regions: &[BackendMemoryRegionV1],
        order_frontier: &CausalFrontier,
    ) -> bool {
        regions.iter().any(|region| {
            let byte_end = region
                .byte_offset
                .checked_add(region.byte_len)
                .expect("launch regions were range-checked before hazard admission");
            self.allocations
                .get(&region.allocation)
                .is_some_and(|streams| {
                    streams.iter().any(|(prior_stream, tree)| {
                        *prior_stream != stream
                            && tree.conflicts(
                                region.byte_offset,
                                byte_end,
                                region.access,
                                order_frontier.get(prior_stream).copied().unwrap_or(0),
                                &self.instrumentation,
                            )
                    })
                })
        })
    }

    #[cfg(test)]
    fn reset_query_visits(&self) {
        self.instrumentation.visits.set(0);
    }

    #[cfg(test)]
    fn query_visits(&self) -> usize {
        self.instrumentation.visits.get()
    }
}

fn pending_node_height(node: &Option<Box<PendingIntervalNode>>) -> usize {
    node.as_deref().map_or(0, |node| node.height)
}

fn pending_node_max_end(node: &Option<Box<PendingIntervalNode>>, writes_only: bool) -> u64 {
    node.as_deref().map_or(0, |node| {
        if writes_only {
            node.max_write_end
        } else {
            node.max_end
        }
    })
}

fn pending_node_max_submission(node: &Option<Box<PendingIntervalNode>>, writes_only: bool) -> u64 {
    node.as_deref().map_or(0, |node| {
        if writes_only {
            node.max_write_submission
        } else {
            node.max_submission
        }
    })
}

fn pending_balance_factor(node: &PendingIntervalNode) -> isize {
    pending_node_height(&node.left) as isize - pending_node_height(&node.right) as isize
}

fn rotate_pending_right(mut root: Box<PendingIntervalNode>) -> Box<PendingIntervalNode> {
    let mut pivot = root
        .left
        .take()
        .expect("right rotation requires a left child");
    root.left = pivot.right.take();
    root.refresh();
    pivot.right = Some(root);
    pivot.refresh();
    pivot
}

fn rotate_pending_left(mut root: Box<PendingIntervalNode>) -> Box<PendingIntervalNode> {
    let mut pivot = root
        .right
        .take()
        .expect("left rotation requires a right child");
    root.right = pivot.left.take();
    root.refresh();
    pivot.left = Some(root);
    pivot.refresh();
    pivot
}

fn rebalance_pending_node(mut node: Box<PendingIntervalNode>) -> Box<PendingIntervalNode> {
    node.refresh();
    let balance = pending_balance_factor(&node);
    if balance > 1 {
        if node
            .left
            .as_deref()
            .is_some_and(|left| pending_balance_factor(left) < 0)
        {
            node.left = node.left.take().map(rotate_pending_left);
        }
        return rotate_pending_right(node);
    }
    if balance < -1 {
        if node
            .right
            .as_deref()
            .is_some_and(|right| pending_balance_factor(right) > 0)
        {
            node.right = node.right.take().map(rotate_pending_right);
        }
        return rotate_pending_left(node);
    }
    node
}

fn insert_pending_interval_node(
    node: Option<Box<PendingIntervalNode>>,
    region: PendingRegion,
) -> Box<PendingIntervalNode> {
    let Some(mut node) = node else {
        return PendingIntervalNode::new(region);
    };
    match region.key.cmp(&node.region.key) {
        Ordering::Less => {
            node.left = Some(insert_pending_interval_node(node.left.take(), region));
        }
        Ordering::Greater => {
            node.right = Some(insert_pending_interval_node(node.right.take(), region));
        }
        Ordering::Equal => {
            debug_assert!(false, "duplicate pending interval key");
            node.region = region;
        }
    }
    rebalance_pending_node(node)
}

fn remove_pending_interval_node(
    node: Option<Box<PendingIntervalNode>>,
    key: PendingRegionKey,
) -> (Option<Box<PendingIntervalNode>>, bool) {
    let Some(mut node) = node else {
        return (None, false);
    };
    let removed = match key.cmp(&node.region.key) {
        Ordering::Less => {
            let (left, removed) = remove_pending_interval_node(node.left.take(), key);
            node.left = left;
            removed
        }
        Ordering::Greater => {
            let (right, removed) = remove_pending_interval_node(node.right.take(), key);
            node.right = right;
            removed
        }
        Ordering::Equal => {
            return match (node.left.take(), node.right.take()) {
                (None, right) => (right, true),
                (left, None) => (left, true),
                (left, Some(right)) => {
                    let (new_right, mut successor) = take_min_pending_interval_node(right);
                    successor.left = left;
                    successor.right = new_right;
                    (Some(rebalance_pending_node(successor)), true)
                }
            };
        }
    };
    (Some(rebalance_pending_node(node)), removed)
}

fn take_min_pending_interval_node(
    mut node: Box<PendingIntervalNode>,
) -> (Option<Box<PendingIntervalNode>>, Box<PendingIntervalNode>) {
    let Some(left) = node.left.take() else {
        let right = node.right.take();
        node.refresh();
        return (right, node);
    };
    let (new_left, minimum) = take_min_pending_interval_node(left);
    node.left = new_left;
    (Some(rebalance_pending_node(node)), minimum)
}

fn find_pending_interval_conflict(
    node: Option<&PendingIntervalNode>,
    byte_offset: u64,
    byte_end: u64,
    access: RuntimeAccessV1,
    ordered_through: u64,
    instrumentation: &QueryInstrumentation,
) -> bool {
    let Some(node) = node else {
        return false;
    };
    instrumentation.record_visit();
    let writes_only = access == RuntimeAccessV1::Read;
    let relevant_end = |node: Option<&PendingIntervalNode>| {
        node.map_or(0, |node| {
            if writes_only {
                node.max_write_end
            } else {
                node.max_end
            }
        })
    };
    let relevant_submission = |node: Option<&PendingIntervalNode>| {
        node.map_or(0, |node| {
            if writes_only {
                node.max_write_submission
            } else {
                node.max_submission
            }
        })
    };
    if relevant_end(Some(node)) <= byte_offset || relevant_submission(Some(node)) <= ordered_through
    {
        return false;
    }
    if relevant_end(node.left.as_deref()) > byte_offset
        && relevant_submission(node.left.as_deref()) > ordered_through
        && find_pending_interval_conflict(
            node.left.as_deref(),
            byte_offset,
            byte_end,
            access,
            ordered_through,
            instrumentation,
        )
    {
        return true;
    }
    if node.region.key.submission > ordered_through
        && node.region.key.byte_offset < byte_end
        && byte_offset < node.region.byte_end
        && !matches!(
            (access, node.region.access),
            (RuntimeAccessV1::Read, RuntimeAccessV1::Read)
        )
    {
        return true;
    }
    if node.region.key.byte_offset >= byte_end {
        return false;
    }
    find_pending_interval_conflict(
        node.right.as_deref(),
        byte_offset,
        byte_end,
        access,
        ordered_through,
        instrumentation,
    )
}

impl<A: DispatchApi> BackendState<A> {
    fn new(core: AdapterCore<A>) -> Self {
        Self {
            core,
            streams: BTreeMap::new(),
            allocations: BTreeMap::new(),
            modules: BTreeMap::new(),
            kernels: BTreeMap::new(),
            submissions: BTreeMap::new(),
            events: BTreeMap::new(),
            pending_accesses: PendingAccessIndex::default(),
            retained_images: Vec::new(),
            retained_addresses: Vec::new(),
            next_identity: 1,
            terminal: false,
        }
    }

    fn enumerate_devices_v1(&mut self) -> BackendResult<Vec<BackendDeviceDescriptionV1>> {
        self.require_live()?;
        let target = self.core.environment.physical_device().target().to_string();
        Ok(vec![BackendDeviceDescriptionV1 {
            backend_device: BACKEND_DEVICE_V1,
            name: format!("reviewed ROCr HSA {target}"),
            target,
            global_memory_bytes: 0,
            capabilities: RuntimeCapabilitiesV1 {
                typed_async_launch: true,
                streams: true,
                events: true,
                device_memory: false,
                host_visible_memory: true,
                peer_copy: false,
                multi_device: false,
                atomics: false,
                collectives: false,
            },
        }])
    }

    fn create_stream_v1(&mut self, device: u64) -> BackendResult<u64> {
        self.require_device(device)?;
        let identity = self.next_identity()?;
        let queue_size = reviewed_queue_size(self.core.queue_min_size, self.core.queue_max_size)?;
        let mut queue = self
            .core
            .api
            .queue_create(self.core.agent, queue_size)
            .map_err(|error| RuntimeBackendFailureV1::Rejected(error.into()))?;
        if let Err(primary) = self.core.api.queue_async_error(&queue) {
            if self.core.api.queue_destroy(&mut queue).is_err() {
                return self.terminal(primary.into());
            }
            return Err(RuntimeBackendFailureV1::Quiescent(primary.into()));
        }
        self.streams.insert(
            identity,
            StreamRecord {
                queue: Some(queue),
                faulted: false,
                submissions: BTreeSet::new(),
                order_frontier: BTreeMap::new(),
            },
        );
        Ok(identity)
    }

    fn destroy_stream_v1(&mut self, stream: u64) -> BackendResult<()> {
        self.require_live()?;
        if !self.streams.contains_key(&stream) {
            return Err(rejected_invalid_handle("stream"));
        }
        let submissions: Vec<_> = self.streams[&stream].submissions.iter().copied().collect();
        for submission in submissions {
            if self.poll_v1(submission)? == BackendPollV1::Pending {
                return Err(RuntimeBackendFailureV1::Rejected(
                    ReviewedHsaRuntimeBackendErrorV1::ResourceBusy("stream"),
                ));
            }
        }
        let queue = self
            .streams
            .get_mut(&stream)
            .and_then(|record| record.queue.as_mut());
        if let Some(queue) = queue
            && let Err(error) = self.core.api.queue_destroy(queue)
        {
            return self.terminal(error.into());
        }
        self.streams.remove(&stream);
        Ok(())
    }

    fn allocate_v1(
        &mut self,
        device: u64,
        kind: RuntimeMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    ) -> BackendResult<u64> {
        self.require_device(device)?;
        if kind != RuntimeMemoryKindV1::HostVisible {
            return Err(RuntimeBackendFailureV1::Rejected(
                ReviewedHsaRuntimeBackendErrorV1::Unsupported("device-local allocation"),
            ));
        }
        if byte_len == 0 || alignment == 0 || !alignment.is_power_of_two() {
            return Err(rejected_invalid_argument("allocation size or alignment"));
        }
        let len =
            usize::try_from(byte_len).map_err(|_| rejected_invalid_argument("allocation size"))?;
        let requested_alignment = usize::try_from(alignment)
            .map_err(|_| rejected_invalid_argument("allocation alignment"))?;
        let identity = self.next_identity()?;
        let address = self
            .core
            .api
            .memory_allocate(self.core.kernarg_pool, len)
            .map_err(|error| RuntimeBackendFailureV1::Rejected(error.into()))?;
        if !address.is_multiple_of(requested_alignment) {
            let error = ReviewedHsaRuntimeBackendErrorV1::InvalidArgument("allocation alignment");
            return self.cleanup_unpublished_allocation(address, error);
        }
        if let Err(primary) = self.core.api.allow_access(self.core.agent, address) {
            return self.cleanup_unpublished_allocation(address, primary.into());
        }
        self.allocations
            .insert(identity, AllocationRecord { address, byte_len });
        Ok(identity)
    }

    fn release_allocation_v1(&mut self, allocation: u64) -> BackendResult<()> {
        self.require_live()?;
        let record = self
            .allocations
            .get(&allocation)
            .copied()
            .ok_or_else(|| rejected_invalid_handle("allocation"))?;
        if self.pending_accesses.allocation_is_pending(allocation) {
            return Err(RuntimeBackendFailureV1::Rejected(
                ReviewedHsaRuntimeBackendErrorV1::ResourceBusy("allocation"),
            ));
        }
        if let Err(error) = self.core.api.memory_free(record.address) {
            return self.terminal(error.into());
        }
        self.allocations.remove(&allocation);
        Ok(())
    }

    fn write_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        bytes: &[u8],
    ) -> BackendResult<()> {
        self.require_live()?;
        if self.pending_host_conflict(allocation, byte_offset, bytes.len(), true)? {
            return Err(RuntimeBackendFailureV1::Rejected(
                ReviewedHsaRuntimeBackendErrorV1::ResourceBusy("allocation"),
            ));
        }
        let address = self.checked_allocation_range(allocation, byte_offset, bytes.len())?;
        self.core.api.write_memory(address, bytes);
        Ok(())
    }

    fn read_allocation_v1(
        &mut self,
        allocation: u64,
        byte_offset: u64,
        destination: &mut [u8],
    ) -> BackendResult<()> {
        self.require_live()?;
        if self.pending_host_conflict(allocation, byte_offset, destination.len(), false)? {
            return Err(RuntimeBackendFailureV1::Rejected(
                ReviewedHsaRuntimeBackendErrorV1::ResourceBusy("allocation"),
            ));
        }
        let address = self.checked_allocation_range(allocation, byte_offset, destination.len())?;
        self.core.api.read_memory(address, destination);
        Ok(())
    }

    fn load_module_v1(&mut self, device: u64, image: &[u8]) -> BackendResult<u64> {
        self.require_device(device)?;
        if image.is_empty() {
            return Err(rejected_invalid_argument("empty code object"));
        }
        let identity = self.next_identity()?;
        let bytes = Vec::from(image).into_boxed_slice();
        let reader = self
            .core
            .api
            .reader_create(&bytes)
            .map_err(|error| RuntimeBackendFailureV1::Rejected(error.into()))?;
        let executable = match self.core.api.executable_create(self.core.profile) {
            Ok(executable) => executable,
            Err(primary) => {
                if self.core.api.reader_destroy(reader).is_err() {
                    self.retained_images.push(bytes);
                    return self.terminal(primary.into());
                }
                return Err(RuntimeBackendFailureV1::Quiescent(primary.into()));
            }
        };
        let loaded = match self
            .core
            .api
            .executable_load(executable, self.core.agent, reader)
        {
            Ok(loaded) => loaded,
            Err(primary) => {
                return self.cleanup_failed_module(bytes, reader, executable, primary);
            }
        };
        if let Err(primary) = self.core.api.executable_freeze(executable) {
            return self.cleanup_failed_module(bytes, reader, executable, primary);
        }
        self.modules.insert(
            identity,
            ModuleRecord {
                _bytes: bytes,
                reader: Some(reader),
                executable: Some(executable),
                _loaded: loaded,
            },
        );
        Ok(identity)
    }

    fn unload_module_v1(&mut self, module: u64) -> BackendResult<()> {
        self.require_live()?;
        if !self.modules.contains_key(&module) {
            return Err(rejected_invalid_handle("module"));
        }
        if self
            .submissions
            .values()
            .any(|submission| submission.module == module && submission.outcome.is_none())
        {
            return Err(RuntimeBackendFailureV1::Rejected(
                ReviewedHsaRuntimeBackendErrorV1::ResourceBusy("module"),
            ));
        }
        let executable = self.modules[&module].executable;
        if let Some(executable) = executable {
            if let Err(error) = self.core.api.executable_destroy(executable) {
                return self.terminal(error.into());
            }
            self.modules
                .get_mut(&module)
                .expect("module remains live")
                .executable = None;
        }
        let reader = self.modules[&module].reader;
        if let Some(reader) = reader {
            if let Err(error) = self.core.api.reader_destroy(reader) {
                return self.terminal(error.into());
            }
            self.modules
                .get_mut(&module)
                .expect("module remains live")
                .reader = None;
        }
        self.kernels.retain(|_, kernel| kernel.module != module);
        self.modules.remove(&module);
        Ok(())
    }

    fn resolve_kernel_v1(
        &mut self,
        module: u64,
        name: &str,
        signature: [u8; 32],
    ) -> BackendResult<u64> {
        self.require_live()?;
        if name.is_empty() || name.as_bytes().contains(&0) {
            return Err(rejected_invalid_argument("kernel name"));
        }
        let executable = self
            .modules
            .get(&module)
            .and_then(|record| record.executable)
            .ok_or_else(|| rejected_invalid_handle("module"))?;
        let runtime_name = format!("{name}.kd");
        let symbol = self
            .core
            .api
            .resolve_symbol(executable, self.core.agent, &runtime_name)
            .map_err(|error| RuntimeBackendFailureV1::Rejected(error.into()))?;
        validate_symbol(&symbol, &runtime_name)?;
        let identity = self.next_identity()?;
        self.kernels.insert(
            identity,
            KernelRecord {
                module,
                kernel_object: symbol.kernel_object,
                kernarg_size: symbol.kernarg_size,
                kernarg_alignment: symbol.kernarg_alignment,
                group_segment_size: symbol.group_segment_size,
                private_segment_size: symbol.private_segment_size,
                _signature: signature,
            },
        );
        Ok(identity)
    }

    fn submit_v1(&mut self, launch: BackendLaunchV1<'_>) -> BackendResult<u64> {
        self.require_live()?;
        let stream = self
            .streams
            .get(&launch.stream)
            .filter(|record| record.queue.is_some() && !record.faulted)
            .ok_or_else(|| rejected_invalid_handle("stream"))?;
        let queue_pointer = stream.queue.as_ref().expect("checked queue").pointer();
        let kernel = self
            .kernels
            .get(&launch.kernel)
            .copied()
            .ok_or_else(|| rejected_invalid_handle("kernel"))?;
        let total_len = usize::try_from(kernel.kernarg_size)
            .map_err(|_| rejected_invalid_argument("kernarg size"))?;
        let implicit_len = total_len
            .checked_sub(launch.explicit_kernarg.len())
            .ok_or_else(|| rejected_invalid_argument("explicit kernarg size"))?;
        if !matches!(implicit_len, 0 | IMPLICIT_KERNARG_BYTES) {
            return Err(rejected_invalid_argument(
                "explicit/implicit kernarg layout",
            ));
        }
        let aql_geometry = checked_aql_geometry(launch.geometry.grid, launch.geometry.workgroup)?;
        let group_segment_size = kernel
            .group_segment_size
            .checked_add(launch.geometry.dynamic_shared_bytes)
            .ok_or_else(|| rejected_invalid_argument("group segment size overflow"))?;
        let mut kernarg = vec![0; total_len];
        kernarg[..launch.explicit_kernarg.len()].copy_from_slice(launch.explicit_kernarg);
        for binding in launch.bindings {
            let device_address = self.patch_binding(launch.explicit_kernarg.len(), binding)?;
            let offset = usize::try_from(binding.kernarg_byte_offset)
                .map_err(|_| rejected_invalid_argument("kernarg patch offset"))?;
            kernarg[offset..offset + 8].copy_from_slice(&device_address.to_le_bytes());
        }
        if implicit_len == IMPLICIT_KERNARG_BYTES {
            initialize_implicit_kernarg(
                &mut kernarg,
                launch.explicit_kernarg.len(),
                aql_geometry,
                launch.geometry.dynamic_shared_bytes,
                queue_pointer,
            )?;
        }
        let mut dependency_signals = Vec::with_capacity(launch.dependencies.len());
        let mut dependency_submissions = BTreeSet::new();
        let mut order_frontier = self.streams[&launch.stream].order_frontier.clone();
        for event in launch.dependencies {
            let source = self
                .events
                .get(event)
                .ok_or_else(|| rejected_invalid_handle("event"))?
                .submission;
            let source_record = self
                .submissions
                .get(&source)
                .ok_or_else(|| rejected_invalid_handle("event submission"))?;
            let signal = source_record
                .signal
                .ok_or_else(|| rejected_invalid_handle("event signal"))?;
            dependency_signals.push(signal);
            dependency_submissions.insert(source);
            merge_order_frontier(&mut order_frontier, &source_record.order_frontier);
        }
        order_frontier.retain(|ordered_stream, _| {
            *ordered_stream == launch.stream
                || self
                    .pending_accesses
                    .stream_has_pending_accesses(*ordered_stream)
        });
        let regions: Vec<_> = launch
            .bindings
            .iter()
            .map(|binding| binding.region)
            .collect();
        self.validate_pending_hazards(launch.stream, &regions, &order_frontier)?;
        let identity = self.next_identity()?;
        let kernarg_address = self
            .core
            .api
            .memory_allocate(self.core.kernarg_pool, kernarg.len())
            .map_err(|error| RuntimeBackendFailureV1::Rejected(error.into()))?;
        let required_alignment = usize::try_from(kernel.kernarg_alignment)
            .map_err(|_| rejected_invalid_argument("kernarg alignment"))?;
        if required_alignment == 0
            || !required_alignment.is_power_of_two()
            || !kernarg_address.is_multiple_of(required_alignment)
        {
            return self.cleanup_unpublished_allocation(
                kernarg_address,
                ReviewedHsaRuntimeBackendErrorV1::InvalidArgument("kernarg alignment"),
            );
        }
        if let Err(primary) = self.core.api.allow_access(self.core.agent, kernarg_address) {
            return self.cleanup_unpublished_allocation(kernarg_address, primary.into());
        }
        self.core.api.write_memory(kernarg_address, &kernarg);
        let signal = match self.core.api.signal_create(1) {
            Ok(signal) => signal,
            Err(primary) => {
                return self.cleanup_pre_submit(kernarg_address, None, primary.into());
            }
        };
        let queue = self.streams[&launch.stream]
            .queue
            .as_ref()
            .expect("validated stream queue");
        if let Err(primary) = self.core.api.queue_async_error(queue) {
            return self.cleanup_pre_submit(kernarg_address, Some(signal), primary.into());
        }
        if let Err(primary) = self.core.api.publish_dispatch(
            queue,
            aql_geometry.grid(),
            launch.geometry.workgroup,
            kernel.private_segment_size,
            group_segment_size,
            kernel.kernel_object,
            kernarg_address,
            signal,
            &dependency_signals,
        ) {
            return self.cleanup_pre_submit(kernarg_address, Some(signal), primary.into());
        }
        order_frontier.insert(launch.stream, identity);
        let dependency_submissions: Vec<_> = dependency_submissions.into_iter().collect();
        for dependency in &dependency_submissions {
            let source = self
                .submissions
                .get_mut(dependency)
                .expect("validated dependency remains live during publication");
            source.pending_dependents = source
                .pending_dependents
                .checked_add(1)
                .expect("live dependent count cannot exceed address space");
        }
        self.pending_accesses
            .insert(identity, launch.stream, &regions);
        let stream_record = self
            .streams
            .get_mut(&launch.stream)
            .expect("published stream remains live");
        stream_record.submissions.insert(identity);
        stream_record.order_frontier.clone_from(&order_frontier);
        self.submissions.insert(
            identity,
            SubmissionRecord {
                stream: launch.stream,
                module: kernel.module,
                regions,
                dependencies: dependency_submissions,
                order_frontier,
                pending_dependents: 0,
                event_references: 0,
                signal: Some(signal),
                kernarg_address: Some(kernarg_address),
                outcome: None,
            },
        );
        Ok(identity)
    }

    fn poll_v1(&mut self, submission: u64) -> BackendResult<BackendPollV1> {
        self.require_live()?;
        let record = self
            .submissions
            .get(&submission)
            .ok_or_else(|| rejected_invalid_handle("submission"))?;
        if let Some(outcome) = record.outcome {
            return Ok(outcome);
        }
        let signal = record.signal.expect("live submission signal");
        let stream = record.stream;
        let observation = self.core.api.signal_load_acquire(signal);
        let queue = self
            .streams
            .get(&stream)
            .and_then(|record| record.queue.as_ref())
            .ok_or_else(|| rejected_invalid_handle("submission stream"))?;
        if observation != 0 {
            if let Err(error) = self.core.api.queue_async_error(queue) {
                return self.terminal(error.into());
            }
            return Ok(BackendPollV1::Pending);
        }
        let outcome = match self.core.api.queue_async_error(queue) {
            Ok(()) => BackendPollV1::Succeeded,
            Err(error) => {
                let other_signals: Vec<_> = self
                    .streams
                    .get(&stream)
                    .expect("submission stream remains live")
                    .submissions
                    .iter()
                    .filter_map(|identity| {
                        let other = &self.submissions[identity];
                        (*identity != submission && other.outcome.is_none())
                            .then_some(other.signal)
                            .flatten()
                    })
                    .collect();
                for signal in other_signals {
                    if self.core.api.signal_load_acquire(signal) != 0 {
                        return self.terminal(error.into());
                    }
                }
                self.streams
                    .get_mut(&stream)
                    .expect("submission stream remains live")
                    .faulted = true;
                BackendPollV1::Failed {
                    code: i64::from(error.status),
                }
            }
        };
        self.submissions
            .get_mut(&submission)
            .expect("submission remains live")
            .outcome = Some(outcome);
        self.retire_pending_submission(submission);
        Ok(outcome)
    }

    fn wait_v1(&mut self, submission: u64, deadline: Instant) -> BackendResult<BackendPollV1> {
        let mut attempts = 0_u32;
        let mut sleep = WAIT_INITIAL_SLEEP_V1;
        loop {
            let outcome = self.poll_v1(submission)?;
            if outcome != BackendPollV1::Pending || Instant::now() >= deadline {
                return Ok(outcome);
            }
            if attempts < WAIT_SPINS_V1 {
                core::hint::spin_loop();
            } else if attempts < WAIT_SPINS_V1 + WAIT_YIELDS_V1 {
                std::thread::yield_now();
            } else {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Ok(BackendPollV1::Pending);
                }
                std::thread::sleep(sleep.min(remaining));
                sleep = sleep.saturating_mul(2).min(WAIT_MAX_SLEEP_V1);
            }
            attempts = attempts.saturating_add(1);
        }
    }

    fn release_submission_v1(&mut self, submission: u64) -> BackendResult<()> {
        self.require_live()?;
        let record = self
            .submissions
            .get(&submission)
            .ok_or_else(|| rejected_invalid_handle("submission"))?;
        if record.outcome.is_none() {
            return Err(RuntimeBackendFailureV1::Rejected(
                ReviewedHsaRuntimeBackendErrorV1::ResourceBusy("submission"),
            ));
        }
        debug_assert!(
            record.regions.is_empty(),
            "quiescence must retire every indexed region before release"
        );
        if record.event_references != 0 || record.pending_dependents != 0 {
            return Err(RuntimeBackendFailureV1::Rejected(
                ReviewedHsaRuntimeBackendErrorV1::ResourceBusy("submission completion signal"),
            ));
        }
        let signal = record.signal;
        if let Some(signal) = signal
            && let Err(error) = self.core.api.signal_destroy(signal)
        {
            return self.terminal(error.into());
        }
        self.submissions
            .get_mut(&submission)
            .expect("submission remains live")
            .signal = None;
        let address = self.submissions[&submission].kernarg_address;
        if let Some(address) = address
            && let Err(error) = self.core.api.memory_free(address)
        {
            return Err(RuntimeBackendFailureV1::Quiescent(error.into()));
        }
        self.submissions
            .get_mut(&submission)
            .expect("submission remains live")
            .kernarg_address = None;
        let stream = self.submissions[&submission].stream;
        if let Some(stream_record) = self.streams.get_mut(&stream) {
            stream_record.submissions.remove(&submission);
        }
        self.submissions.remove(&submission);
        Ok(())
    }

    fn record_event_v1(&mut self, stream: u64, submission: u64) -> BackendResult<u64> {
        self.require_live()?;
        if self
            .streams
            .get(&stream)
            .is_none_or(|record| record.queue.is_none())
        {
            return Err(rejected_invalid_handle("stream"));
        }
        let record = self
            .submissions
            .get(&submission)
            .ok_or_else(|| rejected_invalid_handle("submission"))?;
        if record.stream != stream
            || record.signal.is_none()
            || matches!(record.outcome, Some(BackendPollV1::Failed { .. }))
        {
            return Err(rejected_invalid_argument("event stream/submission binding"));
        }
        let identity = self.next_identity()?;
        let source = self
            .submissions
            .get_mut(&submission)
            .expect("event submission remains live");
        source.event_references = source
            .event_references
            .checked_add(1)
            .expect("event reference count cannot exceed address space");
        self.events.insert(identity, EventRecord { submission });
        Ok(identity)
    }

    fn release_event_v1(&mut self, event: u64) -> BackendResult<()> {
        self.require_live()?;
        let record = self
            .events
            .remove(&event)
            .ok_or_else(|| rejected_invalid_handle("event"))?;
        let source = self
            .submissions
            .get_mut(&record.submission)
            .expect("live event retains its submission");
        source.event_references = source
            .event_references
            .checked_sub(1)
            .expect("event reference count cannot underflow");
        Ok(())
    }

    fn peer_copy_v1(
        &mut self,
        _stream: u64,
        _source: BackendMemoryRegionV1,
        _destination: BackendMemoryRegionV1,
        _dependencies: &[u64],
    ) -> BackendResult<u64> {
        self.require_live()?;
        Err(RuntimeBackendFailureV1::Rejected(
            ReviewedHsaRuntimeBackendErrorV1::Unsupported("peer copy or multi-device access"),
        ))
    }

    fn patch_binding(&self, explicit_len: usize, binding: &BackendBindingV1) -> BackendResult<u64> {
        let patch = usize::try_from(binding.kernarg_byte_offset)
            .map_err(|_| rejected_invalid_argument("kernarg patch offset"))?;
        if !patch.is_multiple_of(8) || patch.checked_add(8).is_none_or(|end| end > explicit_len) {
            return Err(rejected_invalid_argument("kernarg patch range"));
        }
        let allocation = self
            .allocations
            .get(&binding.region.allocation)
            .ok_or_else(|| rejected_invalid_handle("allocation"))?;
        let end = binding
            .region
            .byte_offset
            .checked_add(binding.region.byte_len)
            .ok_or_else(|| rejected_invalid_argument("allocation binding range"))?;
        if binding.region.byte_len == 0 || end > allocation.byte_len {
            return Err(rejected_invalid_argument("allocation binding range"));
        }
        let offset = usize::try_from(binding.region.byte_offset)
            .map_err(|_| rejected_invalid_argument("allocation binding offset"))?;
        let address = allocation
            .address
            .checked_add(offset)
            .ok_or_else(|| rejected_invalid_argument("allocation address overflow"))?;
        u64::try_from(address).map_err(|_| rejected_invalid_argument("allocation address"))
    }

    fn checked_allocation_range(
        &self,
        allocation: u64,
        byte_offset: u64,
        byte_len: usize,
    ) -> BackendResult<usize> {
        let record = self
            .allocations
            .get(&allocation)
            .ok_or_else(|| rejected_invalid_handle("allocation"))?;
        let byte_len =
            u64::try_from(byte_len).map_err(|_| rejected_invalid_argument("allocation range"))?;
        let end = byte_offset
            .checked_add(byte_len)
            .ok_or_else(|| rejected_invalid_argument("allocation range"))?;
        if byte_len == 0 || end > record.byte_len {
            return Err(rejected_invalid_argument("allocation range"));
        }
        let offset = usize::try_from(byte_offset)
            .map_err(|_| rejected_invalid_argument("allocation offset"))?;
        record
            .address
            .checked_add(offset)
            .ok_or_else(|| rejected_invalid_argument("allocation address overflow"))
    }

    fn pending_host_conflict(
        &self,
        allocation: u64,
        byte_offset: u64,
        byte_len: usize,
        host_writes: bool,
    ) -> BackendResult<bool> {
        let record = self
            .allocations
            .get(&allocation)
            .ok_or_else(|| rejected_invalid_handle("allocation"))?;
        let byte_len =
            u64::try_from(byte_len).map_err(|_| rejected_invalid_argument("allocation range"))?;
        let end = byte_offset
            .checked_add(byte_len)
            .ok_or_else(|| rejected_invalid_argument("allocation range"))?;
        if byte_len == 0 || end > record.byte_len {
            return Err(rejected_invalid_argument("allocation range"));
        }
        Ok(self
            .pending_accesses
            .host_conflicts(allocation, byte_offset, end, host_writes))
    }

    fn validate_pending_hazards(
        &self,
        stream: u64,
        regions: &[BackendMemoryRegionV1],
        order_frontier: &CausalFrontier,
    ) -> BackendResult<()> {
        if self
            .pending_accesses
            .launch_conflicts(stream, regions, order_frontier)
        {
            return Err(RuntimeBackendFailureV1::Rejected(
                ReviewedHsaRuntimeBackendErrorV1::ResourceBusy(
                    "cross-stream allocation hazard without an event dependency",
                ),
            ));
        }
        Ok(())
    }

    fn cleanup_failed_module(
        &mut self,
        bytes: Box<[u8]>,
        reader: u64,
        executable: u64,
        primary: ApiError,
    ) -> BackendResult<u64> {
        if self.core.api.executable_destroy(executable).is_err()
            || self.core.api.reader_destroy(reader).is_err()
        {
            self.retained_images.push(bytes);
            return self.terminal(primary.into());
        }
        Err(RuntimeBackendFailureV1::Quiescent(primary.into()))
    }

    fn retire_pending_submission(&mut self, submission: u64) {
        let (stream, regions, dependencies) = {
            let record = self
                .submissions
                .get_mut(&submission)
                .expect("quiescent submission remains live");
            debug_assert!(record.outcome.is_some());
            (
                record.stream,
                std::mem::take(&mut record.regions),
                std::mem::take(&mut record.dependencies),
            )
        };
        self.pending_accesses.remove(submission, stream, &regions);
        for dependency in dependencies {
            let source = self
                .submissions
                .get_mut(&dependency)
                .expect("pending dependency retains its source submission");
            source.pending_dependents = source
                .pending_dependents
                .checked_sub(1)
                .expect("pending dependent count cannot underflow");
        }
    }

    fn cleanup_unpublished_allocation<T>(
        &mut self,
        address: usize,
        primary: ReviewedHsaRuntimeBackendErrorV1,
    ) -> BackendResult<T> {
        if self.core.api.memory_free(address).is_err() {
            self.retained_addresses.push(address);
            return self.terminal(primary);
        }
        Err(RuntimeBackendFailureV1::Quiescent(primary))
    }

    fn cleanup_pre_submit<T>(
        &mut self,
        address: usize,
        signal: Option<u64>,
        primary: ReviewedHsaRuntimeBackendErrorV1,
    ) -> BackendResult<T> {
        if let Some(signal) = signal
            && self.core.api.signal_destroy(signal).is_err()
        {
            self.retained_addresses.push(address);
            return self.terminal(primary);
        }
        self.cleanup_unpublished_allocation(address, primary)
    }

    fn cleanup_failed_resources(&mut self) -> Result<(), ()> {
        let submission_ids: Vec<_> = self.submissions.keys().copied().collect();
        for identity in submission_ids {
            let signal = self.submissions[&identity].signal;
            if let Some(signal) = signal {
                if self.core.api.signal_load_acquire(signal) != 0 {
                    return Err(());
                }
                if self.core.api.signal_destroy(signal).is_err() {
                    return Err(());
                }
                self.submissions
                    .get_mut(&identity)
                    .expect("submission remains live")
                    .signal = None;
            }
            let address = self.submissions[&identity].kernarg_address;
            if let Some(address) = address {
                if self.core.api.memory_free(address).is_err() {
                    return Err(());
                }
                self.submissions
                    .get_mut(&identity)
                    .expect("submission remains live")
                    .kernarg_address = None;
            }
        }
        self.pending_accesses = PendingAccessIndex::default();
        for stream in self.streams.values_mut() {
            if let Some(mut queue) = stream.queue.take()
                && self.core.api.queue_destroy(&mut queue).is_err()
            {
                return Err(());
            }
        }
        self.streams.clear();
        let module_ids: Vec<_> = self.modules.keys().copied().collect();
        for identity in module_ids {
            let module = self
                .modules
                .get_mut(&identity)
                .expect("module remains live");
            if let Some(executable) = module.executable.take()
                && self.core.api.executable_destroy(executable).is_err()
            {
                return Err(());
            }
            if let Some(reader) = module.reader.take()
                && self.core.api.reader_destroy(reader).is_err()
            {
                return Err(());
            }
        }
        self.modules.clear();
        let allocations: Vec<_> = self
            .allocations
            .values()
            .map(|record| record.address)
            .collect();
        for address in allocations {
            if self.core.api.memory_free(address).is_err() {
                return Err(());
            }
        }
        self.allocations.clear();
        Ok(())
    }

    fn force_cleanup(&mut self) -> Result<(), ()> {
        if self.terminal {
            return Err(());
        }
        self.cleanup_failed_resources()
    }

    fn next_identity(&mut self) -> BackendResult<u64> {
        let identity = self.next_identity;
        self.next_identity = identity.checked_add(1).ok_or({
            RuntimeBackendFailureV1::Rejected(ReviewedHsaRuntimeBackendErrorV1::IdentityExhausted)
        })?;
        Ok(identity)
    }

    fn require_live(&self) -> BackendResult<()> {
        if self.terminal {
            Err(RuntimeBackendFailureV1::Terminal(
                ReviewedHsaRuntimeBackendErrorV1::BackendTerminal,
            ))
        } else {
            Ok(())
        }
    }

    fn require_device(&self, device: u64) -> BackendResult<()> {
        self.require_live()?;
        if device != BACKEND_DEVICE_V1 {
            return Err(rejected_invalid_handle("device"));
        }
        Ok(())
    }

    fn terminal<T>(&mut self, error: ReviewedHsaRuntimeBackendErrorV1) -> BackendResult<T> {
        self.terminal = true;
        Err(RuntimeBackendFailureV1::Terminal(error))
    }
}

fn rejected_invalid_handle(kind: &'static str) -> BackendFailure {
    RuntimeBackendFailureV1::Rejected(ReviewedHsaRuntimeBackendErrorV1::InvalidHandle(kind))
}

fn rejected_invalid_argument(field: &'static str) -> BackendFailure {
    RuntimeBackendFailureV1::Rejected(ReviewedHsaRuntimeBackendErrorV1::InvalidArgument(field))
}

fn reviewed_queue_size(minimum: u32, maximum: u32) -> BackendResult<u32> {
    if minimum == 0 || maximum < minimum || !minimum.is_power_of_two() || !maximum.is_power_of_two()
    {
        return Err(rejected_invalid_argument("HSA queue limits"));
    }
    Ok(64_u32.clamp(minimum, maximum))
}

fn checked_aql_geometry(
    grid: [u32; 3],
    workgroup: [u32; 3],
) -> BackendResult<AqlDispatchGeometryV1> {
    AqlDispatchGeometryV1::new(grid, workgroup)
        .map_err(|_| rejected_invalid_argument("launch geometry"))
}

fn validate_symbol(symbol: &SymbolFacts, expected_name: &str) -> BackendResult<()> {
    if symbol.handle == 0
        || symbol.kernel_object == 0
        || symbol.kind != HSA_SYMBOL_KIND_KERNEL
        || symbol.name != expected_name
        || symbol.kernarg_size == 0
        || symbol.kernarg_alignment == 0
        || !symbol.kernarg_alignment.is_power_of_two()
    {
        return Err(rejected_invalid_argument("resolved HSA kernel symbol"));
    }
    Ok(())
}

fn merge_order_frontier(target: &mut CausalFrontier, source: &CausalFrontier) {
    for (stream, submission) in source {
        target
            .entry(*stream)
            .and_modify(|current| *current = (*current).max(*submission))
            .or_insert(*submission);
    }
}

fn initialize_implicit_kernarg(
    kernarg: &mut [u8],
    implicit_offset: usize,
    geometry: AqlDispatchGeometryV1,
    dynamic_shared_bytes: u32,
    queue_pointer: usize,
) -> BackendResult<()> {
    let queue_pointer =
        u64::try_from(queue_pointer).map_err(|_| rejected_invalid_argument("HSA queue pointer"))?;
    let shape = geometry.cov6_implicit_dispatch_shape();
    let block_count = shape.block_count();
    let group_size = shape.group_size();
    let remainder = shape.remainder();
    for (axis, offset) in [BLOCK_COUNT_X, BLOCK_COUNT_Y, BLOCK_COUNT_Z]
        .into_iter()
        .enumerate()
    {
        put_u32(kernarg, implicit_offset + offset, block_count[axis]);
    }
    put_u16(kernarg, implicit_offset + GROUP_SIZE_X, group_size[0]);
    put_u16(kernarg, implicit_offset + GROUP_SIZE_Y, group_size[1]);
    put_u16(kernarg, implicit_offset + GROUP_SIZE_Z, group_size[2]);
    put_u16(kernarg, implicit_offset + REMAINDER_X, remainder[0]);
    put_u16(kernarg, implicit_offset + REMAINDER_Y, remainder[1]);
    put_u16(kernarg, implicit_offset + REMAINDER_Z, remainder[2]);
    put_u16(
        kernarg,
        implicit_offset + GRID_DIMS,
        shape.grid_dimensions(),
    );
    put_u32(
        kernarg,
        implicit_offset + DYNAMIC_LDS_SIZE,
        dynamic_shared_bytes,
    );
    put_u64(kernarg, implicit_offset + QUEUE_PTR, queue_pointer);
    Ok(())
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{
        AgentFacts, EnvironmentApi, ExecutableApi, HipFacts, PoolFacts, RuntimeFacts,
    };
    use fe2o3_amd_target::AmdTargetId;
    use fe2o3_artifacts::DigestAlgorithm;
    use fe2o3_host::{
        HsaAgentIdentityV1, HsaEnvironmentObservationV1, HsaPhysicalDeviceIdentityV1,
        HsaRuntimeIdentityV1,
    };

    #[derive(Debug)]
    struct PublishedDispatch {
        grid: [u32; 3],
        kernarg: Vec<u8>,
        dependencies: Vec<u64>,
    }

    struct MockApi {
        next_handle: u64,
        next_address: usize,
        memory: BTreeMap<usize, Vec<u8>>,
        signals: BTreeMap<u64, i64>,
        published: Vec<PublishedDispatch>,
        kernarg_size: u32,
        asynchronous_error: Option<i32>,
        fail_memory_free_once: bool,
        fail_publish_once: bool,
        signal_loads: usize,
    }

    impl MockApi {
        fn new(kernarg_size: u32) -> Self {
            Self {
                next_handle: 100,
                next_address: 0x10_000,
                memory: BTreeMap::new(),
                signals: BTreeMap::new(),
                published: Vec::new(),
                kernarg_size,
                asynchronous_error: None,
                fail_memory_free_once: false,
                fail_publish_once: false,
                signal_loads: 0,
            }
        }

        fn handle(&mut self) -> u64 {
            let value = self.next_handle;
            self.next_handle += 1;
            value
        }

        fn memory_span(&self, address: usize, len: usize) -> (usize, usize) {
            self.memory
                .iter()
                .find_map(|(base, bytes)| {
                    let offset = address.checked_sub(*base)?;
                    (offset.checked_add(len)? <= bytes.len()).then_some((*base, offset))
                })
                .expect("mock address is within a live allocation")
        }
    }

    impl EnvironmentApi for MockApi {
        fn initialize(&mut self) -> Result<RuntimeFacts, ApiError> {
            unreachable!()
        }

        fn shut_down(&mut self) -> Result<(), ApiError> {
            Ok(())
        }

        fn observe_hip_device(&mut self, _ordinal: i32) -> Result<HipFacts, ApiError> {
            unreachable!()
        }

        fn collect_agents(&mut self) -> Result<Vec<AgentFacts>, ApiError> {
            unreachable!()
        }

        fn collect_kernarg_pools(&mut self) -> Result<Vec<PoolFacts>, ApiError> {
            unreachable!()
        }
    }

    impl ExecutableApi for MockApi {
        fn reader_create(&mut self, _bytes: &[u8]) -> Result<u64, ApiError> {
            Ok(self.handle())
        }

        fn reader_destroy(&mut self, _reader: u64) -> Result<(), ApiError> {
            Ok(())
        }

        fn executable_create(&mut self, _profile: u32) -> Result<u64, ApiError> {
            Ok(self.handle())
        }

        fn executable_load(
            &mut self,
            _executable: u64,
            _agent: u64,
            _reader: u64,
        ) -> Result<u64, ApiError> {
            Ok(self.handle())
        }

        fn executable_freeze(&mut self, _executable: u64) -> Result<(), ApiError> {
            Ok(())
        }

        fn executable_destroy(&mut self, _executable: u64) -> Result<(), ApiError> {
            Ok(())
        }

        fn resolve_symbol(
            &mut self,
            _executable: u64,
            _agent: u64,
            name: &str,
        ) -> Result<SymbolFacts, ApiError> {
            Ok(SymbolFacts {
                handle: self.handle(),
                kernel_object: self.handle(),
                kind: HSA_SYMBOL_KIND_KERNEL,
                kernarg_size: self.kernarg_size,
                kernarg_alignment: 16,
                group_segment_size: 32,
                private_segment_size: 64,
                name: name.to_owned(),
            })
        }
    }

    impl DispatchApi for MockApi {
        fn memory_allocate(&mut self, _pool: u64, len: usize) -> Result<usize, ApiError> {
            let address = self.next_address;
            self.next_address += 0x1000;
            self.memory.insert(address, vec![0; len]);
            Ok(address)
        }

        fn allow_access(&mut self, _agent: u64, _address: usize) -> Result<(), ApiError> {
            Ok(())
        }

        fn write_memory(&mut self, address: usize, bytes: &[u8]) {
            let (base, offset) = self.memory_span(address, bytes.len());
            self.memory.get_mut(&base).expect("live memory")[offset..offset + bytes.len()]
                .copy_from_slice(bytes);
        }

        fn read_memory(&mut self, address: usize, destination: &mut [u8]) {
            let (base, offset) = self.memory_span(address, destination.len());
            destination.copy_from_slice(&self.memory[&base][offset..offset + destination.len()]);
        }

        fn memory_free(&mut self, address: usize) -> Result<(), ApiError> {
            if self.fail_memory_free_once {
                self.fail_memory_free_once = false;
                return Err(ApiError {
                    operation: "mock memory free",
                    status: 71,
                });
            }
            self.memory.remove(&address);
            Ok(())
        }

        fn queue_create(&mut self, _agent: u64, size: u32) -> Result<QueueHandle, ApiError> {
            let handle = self.handle();
            Ok(QueueHandle::for_test(
                0x20_000 + handle as usize * 0x1000,
                handle,
                size,
            ))
        }

        fn queue_async_error(&mut self, _queue: &QueueHandle) -> Result<(), ApiError> {
            self.asynchronous_error.map_or(Ok(()), |status| {
                Err(ApiError {
                    operation: "mock queue asynchronous status",
                    status,
                })
            })
        }

        fn queue_destroy(&mut self, _queue: &mut QueueHandle) -> Result<(), ApiError> {
            Ok(())
        }

        fn signal_create(&mut self, initial_value: i64) -> Result<u64, ApiError> {
            let signal = self.handle();
            self.signals.insert(signal, initial_value);
            Ok(signal)
        }

        fn signal_destroy(&mut self, signal: u64) -> Result<(), ApiError> {
            self.signals.remove(&signal);
            Ok(())
        }

        fn signal_load_acquire(&mut self, signal: u64) -> i64 {
            self.signal_loads += 1;
            self.signals[&signal]
        }

        fn publish_dispatch(
            &mut self,
            _queue: &QueueHandle,
            grid: [u32; 3],
            _workgroup: [u32; 3],
            _private_segment_size: u32,
            _group_segment_size: u32,
            _kernel_object: u64,
            kernarg: usize,
            _completion_signal: u64,
            dependency_signals: &[u64],
        ) -> Result<u64, ApiError> {
            if self.fail_publish_once {
                self.fail_publish_once = false;
                return Err(ApiError {
                    operation: "mock publish dispatch",
                    status: 81,
                });
            }
            self.published.push(PublishedDispatch {
                grid,
                kernarg: self.memory[&kernarg].clone(),
                dependencies: dependency_signals.to_vec(),
            });
            Ok(self.published.len() as u64)
        }
    }

    fn environment() -> HsaEnvironmentObservationV1 {
        let target = AmdTargetId::parse("gfx942").unwrap();
        let runtime = HsaRuntimeIdentityV1::new(
            "ROCr",
            "1.18",
            DigestAlgorithm::Sha256.calculate(b"mock-runtime"),
            [1; 16],
        )
        .unwrap();
        let physical = HsaPhysicalDeviceIdentityV1::new([2; 16], 2, 0, target).unwrap();
        let agent = HsaAgentIdentityV1::new([1; 16], 20, [2; 16], target).unwrap();
        HsaEnvironmentObservationV1::new(runtime, physical, agent).unwrap()
    }

    fn state(kernarg_size: u32) -> BackendState<MockApi> {
        BackendState::new(AdapterCore {
            api: MockApi::new(kernarg_size),
            environment: environment(),
            agent: 20,
            profile: 0,
            queue_min_size: 64,
            queue_max_size: 1024,
            kernarg_pool: 30,
            completion_timeout: std::time::Duration::from_secs(5),
            next_identity: 1,
            runtime_live: true,
            _context: None,
        })
    }

    fn binding(allocation: u64) -> BackendBindingV1 {
        BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation,
                access: RuntimeAccessV1::ReadWrite,
                byte_offset: 0,
                byte_len: 32,
            },
            kernarg_byte_offset: 0,
        }
    }

    fn launch<'a>(
        stream: u64,
        kernel: u64,
        binding: &'a BackendBindingV1,
        dependencies: &'a [u64],
    ) -> BackendLaunchV1<'a> {
        BackendLaunchV1 {
            stream,
            kernel,
            explicit_kernarg: &[0; 16],
            bindings: std::slice::from_ref(binding),
            dependencies,
            geometry: fe2o3_runtime::RuntimeLaunchGeometryV1 {
                grid: [65, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 16,
            },
        }
    }

    fn complete(state: &mut BackendState<MockApi>, submission: u64) {
        let signal = state.submissions[&submission].signal.unwrap();
        state.core.api.signals.insert(signal, 0);
    }

    #[test]
    fn capabilities_and_host_visible_memory_are_honest() {
        let mut state = state(272);
        let device = state.enumerate_devices_v1().unwrap().remove(0);
        assert_eq!(device.backend_device, BACKEND_DEVICE_V1);
        assert!(device.capabilities.typed_async_launch);
        assert!(device.capabilities.streams);
        assert!(device.capabilities.events);
        assert!(device.capabilities.host_visible_memory);
        assert!(!device.capabilities.device_memory);
        assert!(!device.capabilities.peer_copy);
        assert!(!device.capabilities.multi_device);
        assert!(!device.capabilities.atomics);
        assert!(!device.capabilities.collectives);

        let stream = state.create_stream_v1(BACKEND_DEVICE_V1).unwrap();
        let allocation = state
            .allocate_v1(BACKEND_DEVICE_V1, RuntimeMemoryKindV1::HostVisible, 32, 16)
            .unwrap();
        state
            .write_allocation_v1(allocation, 8, &[1, 2, 3, 4])
            .unwrap();
        let mut bytes = [0; 4];
        state.read_allocation_v1(allocation, 8, &mut bytes).unwrap();
        assert_eq!(bytes, [1, 2, 3, 4]);
        assert!(matches!(
            state.allocate_v1(BACKEND_DEVICE_V1, RuntimeMemoryKindV1::DeviceLocal, 32, 16),
            Err(RuntimeBackendFailureV1::Rejected(
                ReviewedHsaRuntimeBackendErrorV1::Unsupported(_)
            ))
        ));
        assert!(matches!(
            state.peer_copy_v1(
                stream,
                binding(allocation).region,
                binding(allocation).region,
                &[]
            ),
            Err(RuntimeBackendFailureV1::Rejected(
                ReviewedHsaRuntimeBackendErrorV1::Unsupported(_)
            ))
        ));
        state.destroy_stream_v1(stream).unwrap();
        state.release_allocation_v1(allocation).unwrap();
    }

    #[test]
    fn pending_interval_index_preserves_range_and_access_semantics() {
        let mut index = PendingAccessIndex::default();
        let regions = [
            BackendMemoryRegionV1 {
                allocation: 7,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 8,
            },
            BackendMemoryRegionV1 {
                allocation: 7,
                access: RuntimeAccessV1::Write,
                byte_offset: 16,
                byte_len: 8,
            },
        ];
        index.insert(11, 3, &regions);

        assert!(!index.host_conflicts(7, 0, 8, false));
        assert!(index.host_conflicts(7, 0, 8, true));
        assert!(index.host_conflicts(7, 16, 24, false));
        assert!(!index.host_conflicts(7, 8, 16, true));
        assert!(!index.launch_conflicts(
            4,
            &[BackendMemoryRegionV1 {
                allocation: 7,
                access: RuntimeAccessV1::Read,
                byte_offset: 0,
                byte_len: 8,
            }],
            &BTreeMap::new(),
        ));
        assert!(index.launch_conflicts(
            4,
            &[BackendMemoryRegionV1 {
                allocation: 7,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 8,
            }],
            &BTreeMap::new(),
        ));
        assert!(!index.launch_conflicts(
            4,
            &[BackendMemoryRegionV1 {
                allocation: 7,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 24,
            }],
            &BTreeMap::from([(3, 11)]),
        ));

        index.remove(11, 3, &regions);
        assert!(!index.allocation_is_pending(7));
        assert!(!index.stream_has_pending_accesses(3));
    }

    #[test]
    fn pending_interval_tree_matches_naive_overlap_after_removals() {
        let mut tree = PendingIntervalTree::default();
        let mut regions = Vec::new();
        for submission in 1_u64..=127 {
            let byte_offset = submission.wrapping_mul(37) % 257;
            let byte_end = byte_offset + submission.wrapping_mul(13) % 32 + 1;
            let access = match submission % 3 {
                0 => RuntimeAccessV1::Read,
                1 => RuntimeAccessV1::Write,
                _ => RuntimeAccessV1::ReadWrite,
            };
            let region = PendingRegion {
                key: PendingRegionKey {
                    byte_offset,
                    submission,
                    region_index: submission as usize,
                },
                byte_end,
                access,
            };
            tree.insert(region);
            regions.push(region);
        }
        assert!(tree.height() <= 16);

        let assert_matches_naive = |tree: &PendingIntervalTree, regions: &[PendingRegion]| {
            for ordered_through in [0, 31, 63, 95, 127] {
                for byte_offset in (0..320).step_by(7) {
                    let byte_end = byte_offset + 19;
                    for access in [
                        RuntimeAccessV1::Read,
                        RuntimeAccessV1::Write,
                        RuntimeAccessV1::ReadWrite,
                    ] {
                        let expected = regions.iter().any(|region| {
                            region.key.submission > ordered_through
                                && region.key.byte_offset < byte_end
                                && byte_offset < region.byte_end
                                && !matches!(
                                    (access, region.access),
                                    (RuntimeAccessV1::Read, RuntimeAccessV1::Read)
                                )
                        });
                        assert_eq!(
                            tree.conflicts(
                                byte_offset,
                                byte_end,
                                access,
                                ordered_through,
                                &QueryInstrumentation::default(),
                            ),
                            expected,
                        );
                    }
                }
            }
        };
        assert_matches_naive(&tree, &regions);

        for region in regions
            .iter()
            .copied()
            .filter(|region| region.key.submission % 3 == 0)
        {
            tree.remove(region.key);
        }
        regions.retain(|region| region.key.submission % 3 != 0);
        assert!(tree.height() <= 16);
        assert_matches_naive(&tree, &regions);
    }

    #[test]
    fn typed_async_launch_patches_addresses_and_orders_event_dependencies() {
        let mut state = state(272);
        let first_stream = state.create_stream_v1(BACKEND_DEVICE_V1).unwrap();
        let second_stream = state.create_stream_v1(BACKEND_DEVICE_V1).unwrap();
        let allocation = state
            .allocate_v1(BACKEND_DEVICE_V1, RuntimeMemoryKindV1::HostVisible, 32, 16)
            .unwrap();
        let module = state.load_module_v1(BACKEND_DEVICE_V1, b"hsaco").unwrap();
        let kernel = state.resolve_kernel_v1(module, "kernel", [9; 32]).unwrap();
        let binding = binding(allocation);

        let first = state
            .submit_v1(launch(first_stream, kernel, &binding, &[]))
            .unwrap();
        assert_eq!(state.poll_v1(first).unwrap(), BackendPollV1::Pending);
        let before_wait = state.core.api.signal_loads;
        assert_eq!(
            state.wait_v1(first, Instant::now()).unwrap(),
            BackendPollV1::Pending
        );
        assert_eq!(state.core.api.signal_loads, before_wait + 1);
        let before_backoff_wait = state.core.api.signal_loads;
        let deadline = Instant::now() + Duration::from_millis(3);
        assert_eq!(
            state.wait_v1(first, deadline).unwrap(),
            BackendPollV1::Pending
        );
        let backoff_polls = state.core.api.signal_loads - before_backoff_wait;
        assert!((1..=128).contains(&backoff_polls));
        assert!(Instant::now() >= deadline);
        assert!(matches!(
            state.read_allocation_v1(allocation, 0, &mut [0; 4]),
            Err(RuntimeBackendFailureV1::Rejected(
                ReviewedHsaRuntimeBackendErrorV1::ResourceBusy(_)
            ))
        ));
        let event = state.record_event_v1(first_stream, first).unwrap();
        assert!(matches!(
            state.submit_v1(launch(second_stream, kernel, &binding, &[])),
            Err(RuntimeBackendFailureV1::Rejected(
                ReviewedHsaRuntimeBackendErrorV1::ResourceBusy(_)
            ))
        ));
        let second = state
            .submit_v1(launch(second_stream, kernel, &binding, &[event]))
            .unwrap();
        assert_eq!(state.submissions[&first].pending_dependents, 1);
        assert_eq!(state.submissions[&first].event_references, 1);

        let allocation_address = state.allocations[&allocation].address as u64;
        let first_dispatch = &state.core.api.published[0];
        assert_eq!(first_dispatch.grid, [65, 1, 1]);
        assert_eq!(&first_dispatch.kernarg[16..20], &1_u32.to_le_bytes());
        assert_eq!(&first_dispatch.kernarg[34..36], &1_u16.to_le_bytes());
        assert_eq!(
            u64::from_le_bytes(first_dispatch.kernarg[0..8].try_into().unwrap()),
            allocation_address
        );
        let expected_queue = state.streams[&first_stream]
            .queue
            .as_ref()
            .unwrap()
            .pointer() as u64;
        assert_eq!(
            u64::from_le_bytes(first_dispatch.kernarg[216..224].try_into().unwrap()),
            expected_queue
        );
        let first_signal = state.submissions[&first].signal.unwrap();
        assert_eq!(state.core.api.published[1].dependencies, [first_signal]);

        assert!(matches!(
            state.release_submission_v1(first),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        complete(&mut state, first);
        assert_eq!(state.poll_v1(first).unwrap(), BackendPollV1::Succeeded);
        assert!(matches!(
            state.release_submission_v1(first),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        state.release_event_v1(event).unwrap();
        assert_eq!(state.submissions[&first].event_references, 0);
        assert!(matches!(
            state.release_submission_v1(first),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        complete(&mut state, second);
        assert_eq!(state.poll_v1(second).unwrap(), BackendPollV1::Succeeded);
        assert_eq!(state.submissions[&first].pending_dependents, 0);
        state.release_submission_v1(first).unwrap();
        state.release_submission_v1(second).unwrap();

        state.destroy_stream_v1(first_stream).unwrap();
        state.destroy_stream_v1(second_stream).unwrap();
        state.unload_module_v1(module).unwrap();
        state.release_allocation_v1(allocation).unwrap();
    }

    #[test]
    fn global_grid_is_preserved_and_cov6_hidden_shape_matches_kfd_floor_counts_and_remainders() {
        let geometry = checked_aql_geometry([65, 3, 2], [64, 2, 1]).unwrap();
        assert_eq!(geometry.grid(), [65, 3, 2]);
        let mut kernarg = [0_u8; IMPLICIT_KERNARG_BYTES];
        initialize_implicit_kernarg(&mut kernarg, 0, geometry, 17, 0x1234).unwrap();
        assert_eq!(
            &kernarg[BLOCK_COUNT_X..BLOCK_COUNT_X + 4],
            &1_u32.to_le_bytes()
        );
        assert_eq!(
            &kernarg[BLOCK_COUNT_Y..BLOCK_COUNT_Y + 4],
            &1_u32.to_le_bytes()
        );
        assert_eq!(
            &kernarg[BLOCK_COUNT_Z..BLOCK_COUNT_Z + 4],
            &2_u32.to_le_bytes()
        );
        assert_eq!(&kernarg[REMAINDER_X..REMAINDER_X + 2], &1_u16.to_le_bytes());
        assert_eq!(&kernarg[REMAINDER_Y..REMAINDER_Y + 2], &1_u16.to_le_bytes());
        assert_eq!(&kernarg[REMAINDER_Z..REMAINDER_Z + 2], &0_u16.to_le_bytes());
        assert_eq!(&kernarg[GRID_DIMS..GRID_DIMS + 2], &3_u16.to_le_bytes());
    }

    #[test]
    fn pending_access_queries_are_scoped_to_the_relevant_allocation() {
        const INDEPENDENT_SUBMISSIONS: usize = 256;

        let mut state = state(272);
        let module = state.load_module_v1(BACKEND_DEVICE_V1, b"hsaco").unwrap();
        let kernel = state.resolve_kernel_v1(module, "kernel", [5; 32]).unwrap();
        let mut allocations = Vec::with_capacity(INDEPENDENT_SUBMISSIONS);
        for _ in 0..INDEPENDENT_SUBMISSIONS {
            let stream = state.create_stream_v1(BACKEND_DEVICE_V1).unwrap();
            let allocation = state
                .allocate_v1(BACKEND_DEVICE_V1, RuntimeMemoryKindV1::HostVisible, 32, 16)
                .unwrap();
            state
                .submit_v1(launch(stream, kernel, &binding(allocation), &[]))
                .unwrap();
            allocations.push(allocation);
        }

        state.pending_accesses.reset_query_visits();
        assert!(
            state
                .pending_host_conflict(allocations[INDEPENDENT_SUBMISSIONS - 1], 0, 4, true)
                .unwrap()
        );
        assert_eq!(state.pending_accesses.query_visits(), 1);

        let unused = state
            .allocate_v1(BACKEND_DEVICE_V1, RuntimeMemoryKindV1::HostVisible, 32, 16)
            .unwrap();
        state.pending_accesses.reset_query_visits();
        assert!(!state.pending_host_conflict(unused, 0, 4, true).unwrap());
        assert_eq!(state.pending_accesses.query_visits(), 0);
    }

    #[test]
    fn failed_publication_does_not_advance_order_or_pending_indexes() {
        let mut state = state(272);
        let stream = state.create_stream_v1(BACKEND_DEVICE_V1).unwrap();
        let allocation = state
            .allocate_v1(BACKEND_DEVICE_V1, RuntimeMemoryKindV1::HostVisible, 32, 16)
            .unwrap();
        let module = state.load_module_v1(BACKEND_DEVICE_V1, b"hsaco").unwrap();
        let kernel = state.resolve_kernel_v1(module, "kernel", [8; 32]).unwrap();
        state.core.api.fail_publish_once = true;

        assert!(matches!(
            state.submit_v1(launch(stream, kernel, &binding(allocation), &[])),
            Err(RuntimeBackendFailureV1::Quiescent(
                ReviewedHsaRuntimeBackendErrorV1::NativeCall { status: 81, .. }
            ))
        ));
        assert!(state.submissions.is_empty());
        assert!(state.streams[&stream].submissions.is_empty());
        assert!(state.streams[&stream].order_frontier.is_empty());
        assert!(!state.pending_accesses.allocation_is_pending(allocation));

        state
            .submit_v1(launch(stream, kernel, &binding(allocation), &[]))
            .unwrap();
        assert!(state.pending_accesses.allocation_is_pending(allocation));
    }

    #[test]
    fn same_stream_pending_work_skips_the_hazard_tree_and_retires_exactly() {
        const ORDERED_SUBMISSIONS: usize = 512;

        let mut state = state(272);
        let stream = state.create_stream_v1(BACKEND_DEVICE_V1).unwrap();
        let allocation = state
            .allocate_v1(BACKEND_DEVICE_V1, RuntimeMemoryKindV1::HostVisible, 32, 16)
            .unwrap();
        let module = state.load_module_v1(BACKEND_DEVICE_V1, b"hsaco").unwrap();
        let kernel = state.resolve_kernel_v1(module, "kernel", [6; 32]).unwrap();
        let mut submissions = Vec::with_capacity(ORDERED_SUBMISSIONS);
        for _ in 0..ORDERED_SUBMISSIONS {
            state.pending_accesses.reset_query_visits();
            submissions.push(
                state
                    .submit_v1(launch(stream, kernel, &binding(allocation), &[]))
                    .unwrap(),
            );
            assert_eq!(state.pending_accesses.query_visits(), 0);
        }
        assert_eq!(
            state.pending_accesses.stream_entries[&stream],
            ORDERED_SUBMISSIONS
        );
        assert!(state.pending_accesses.allocations[&allocation][&stream].height() <= 16);

        state.pending_accesses.reset_query_visits();
        assert!(state.pending_host_conflict(allocation, 0, 4, true).unwrap());
        assert!(state.pending_accesses.query_visits() <= 16);

        for submission in &submissions {
            complete(&mut state, *submission);
            assert_eq!(
                state.poll_v1(*submission).unwrap(),
                BackendPollV1::Succeeded
            );
        }
        assert!(!state.pending_accesses.allocation_is_pending(allocation));
        assert!(!state.pending_accesses.stream_has_pending_accesses(stream));
        state
            .write_allocation_v1(allocation, 0, &[1, 2, 3, 4])
            .unwrap();

        for submission in submissions {
            state.release_submission_v1(submission).unwrap();
        }
        state.destroy_stream_v1(stream).unwrap();
        state.unload_module_v1(module).unwrap();
        state.release_allocation_v1(allocation).unwrap();
    }

    #[test]
    fn same_stream_ordering_inherits_but_does_not_overextend_dependency_frontiers() {
        let mut state = state(272);
        let first_stream = state.create_stream_v1(BACKEND_DEVICE_V1).unwrap();
        let second_stream = state.create_stream_v1(BACKEND_DEVICE_V1).unwrap();
        let allocation = state
            .allocate_v1(BACKEND_DEVICE_V1, RuntimeMemoryKindV1::HostVisible, 32, 16)
            .unwrap();
        let module = state.load_module_v1(BACKEND_DEVICE_V1, b"hsaco").unwrap();
        let kernel = state.resolve_kernel_v1(module, "kernel", [7; 32]).unwrap();
        let mut binding = binding(allocation);
        binding.region.byte_len = 8;

        let first = state
            .submit_v1(launch(first_stream, kernel, &binding, &[]))
            .unwrap();
        let event = state.record_event_v1(first_stream, first).unwrap();
        state
            .submit_v1(launch(second_stream, kernel, &binding, &[event]))
            .unwrap();
        state
            .submit_v1(launch(second_stream, kernel, &binding, &[]))
            .expect("same-stream queue order inherits the event dependency");

        let mut later_binding = binding;
        later_binding.region.byte_offset = 16;
        state
            .submit_v1(launch(first_stream, kernel, &later_binding, &[]))
            .unwrap();
        let mut wide_binding = binding;
        wide_binding.region.byte_len = 24;
        assert!(matches!(
            state.submit_v1(launch(second_stream, kernel, &wide_binding, &[])),
            Err(RuntimeBackendFailureV1::Rejected(
                ReviewedHsaRuntimeBackendErrorV1::ResourceBusy(_)
            ))
        ));
    }

    #[test]
    fn partial_submission_release_is_retryable_but_live_queue_error_is_terminal() {
        let mut quiescent = state(272);
        let stream = quiescent.create_stream_v1(BACKEND_DEVICE_V1).unwrap();
        let allocation = quiescent
            .allocate_v1(BACKEND_DEVICE_V1, RuntimeMemoryKindV1::HostVisible, 32, 16)
            .unwrap();
        let module = quiescent
            .load_module_v1(BACKEND_DEVICE_V1, b"hsaco")
            .unwrap();
        let kernel = quiescent
            .resolve_kernel_v1(module, "kernel", [3; 32])
            .unwrap();
        let submission = quiescent
            .submit_v1(launch(stream, kernel, &binding(allocation), &[]))
            .unwrap();
        complete(&mut quiescent, submission);
        assert_eq!(
            quiescent.poll_v1(submission).unwrap(),
            BackendPollV1::Succeeded
        );
        quiescent.core.api.fail_memory_free_once = true;
        assert!(matches!(
            quiescent.release_submission_v1(submission),
            Err(RuntimeBackendFailureV1::Quiescent(
                ReviewedHsaRuntimeBackendErrorV1::NativeCall { status: 71, .. }
            ))
        ));
        assert!(!quiescent.terminal);
        assert!(quiescent.submissions.contains_key(&submission));
        assert!(quiescent.submissions[&submission].signal.is_none());
        assert!(quiescent.submissions[&submission].kernarg_address.is_some());
        quiescent.release_submission_v1(submission).unwrap();
        assert!(!quiescent.submissions.contains_key(&submission));
        quiescent.destroy_stream_v1(stream).unwrap();
        quiescent.unload_module_v1(module).unwrap();
        quiescent.release_allocation_v1(allocation).unwrap();

        let mut terminal = state(272);
        let stream = terminal.create_stream_v1(BACKEND_DEVICE_V1).unwrap();
        let allocation = terminal
            .allocate_v1(BACKEND_DEVICE_V1, RuntimeMemoryKindV1::HostVisible, 32, 16)
            .unwrap();
        let module = terminal
            .load_module_v1(BACKEND_DEVICE_V1, b"hsaco")
            .unwrap();
        let kernel = terminal
            .resolve_kernel_v1(module, "kernel", [4; 32])
            .unwrap();
        let submission = terminal
            .submit_v1(launch(stream, kernel, &binding(allocation), &[]))
            .unwrap();
        terminal.core.api.asynchronous_error = Some(93);
        assert!(matches!(
            terminal.poll_v1(submission),
            Err(RuntimeBackendFailureV1::Terminal(
                ReviewedHsaRuntimeBackendErrorV1::NativeCall { status: 93, .. }
            ))
        ));
        assert!(terminal.terminal);
    }
}
