//! Isolated direct-KFD engineering execution. No protected capability is minted.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

use fe2o3_amdhsa_loader::{AdmittedProfile, SelectedKernelResourceBindingV1};
use fe2o3_aql::{
    AqlCompletionObservationV1, AqlDispatchGeometryV1, AqlKernelDispatchPacketV1,
    AqlPacketPublicationTargetV1, AqlRingCapacityV1, AqlSingleProducerRingModelV1,
    ObservedGpuAddressV1,
};
use fe2o3_hsaco::{ArgumentAccess, ExplicitValueKind, InspectedKernel};
use fe2o3_kfd_uapi::{
    KFD_IOC_QUEUE_TYPE_COMPUTE_AQL, KfdAllocMemoryFlags, KfdIoctlAllocMemoryOfGpuArgs,
    KfdIoctlCreateQueueArgs, KfdIoctlDestroyQueueArgs,
};
use sha2::{Digest, Sha256};

use crate::engineering_gfx950_profile::*;
use crate::engineering_wire::*;
use crate::memory::MemoryBackend;
use crate::memory_linux::{
    LinuxCpuMapping, LinuxGfx950MemoryBackend as Backend, LinuxVaReservation,
};
use crate::queue_linux::{
    LinuxDoorbellSliceV1, LinuxKfdRuntimeDisabledV1, LinuxKfdRuntimeEnabledV1,
    LinuxQueueExceptionEventV1,
};
use crate::{CheckedGfx950XnackMinusDevice, DeviceSelector, OpenedKfd};

#[path = "engineering_gfx950_peer.rs"]
mod peer;
pub use peer::{
    Gfx950EngineeringPeerBufferV1, Gfx950EngineeringPeerDispatchV1, Gfx950EngineeringPeerGroupV1,
    Gfx950EngineeringPeerKernelV1, Gfx950EngineeringPeerPointerV1,
};

type Result<T> = std::result::Result<T, String>;
const MAX_KERNELS: usize = 256;

fn explain(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}

struct Allocation {
    reservation: LinuxVaReservation,
    mapping: LinuxCpuMapping,
    handle: u64,
    va: u64,
    requested: usize,
    backing: usize,
    mmap_offset: Option<u64>,
}

struct Kernel {
    object: Vec<u8>,
    inspected: InspectedKernel,
    metadata: KernelMetadataV1,
    resources: SelectedKernelResourceBindingV1,
    code: Allocation,
    descriptor_offset: u64,
}

struct PreparedDispatch {
    bytes: Vec<u8>,
    geometry: AqlDispatchGeometryV1,
    descriptor: u64,
    alignment: u64,
    group_bytes: u32,
}

/// Crate-private owner used only by explicit disposable-process engineering
/// entries, including the separately opted-in peer group. Any uncertain native
/// result retains the owner until process teardown instead of retrying frees.
struct Context {
    backend: Backend,
    unique_id: u64,
    runtime: Option<LinuxKfdRuntimeEnabledV1>,
    event: Option<LinuxQueueExceptionEventV1>,
    queue_id: Option<u32>,
    doorbell: Option<LinuxDoorbellSliceV1>,
    internal: Vec<Allocation>,
    buffers: BTreeMap<u64, Allocation>,
    kernels: BTreeMap<u64, Kernel>,
    handles: BTreeSet<u64>,
    mmap_offsets: BTreeSet<u64>,
    total_bytes: u64,
    next_buffer: u64,
    next_kernel: u64,
    ring: AqlSingleProducerRingModelV1,
    completed_write: u64,
    queue_epoch: u64,
    last_observed_read: u64,
    performance: Option<PerformanceOptions>,
    counters: PerformanceCountersV1,
}

#[derive(Clone, Copy)]
struct PerformanceOptions {
    cache_kernel_admission: bool,
    operational_currentness: bool,
    profile: bool,
}

fn add_counter(counter: &mut u64, value: u64) -> Result<()> {
    *counter = counter
        .checked_add(value)
        .ok_or("performance counter exhausted")?;
    Ok(())
}

fn record_elapsed(counter: &mut u64, started: Option<Instant>) -> Result<()> {
    if let Some(started) = started {
        add_counter(
            counter,
            u64::try_from(started.elapsed().as_nanos()).map_err(explain)?,
        )?;
    }
    Ok(())
}

fn require_fresh_configuration(
    configured: bool,
    next_buffer: u64,
    next_kernel: u64,
    write: u64,
) -> Result<()> {
    if configured || next_buffer != 1 || next_kernel != 1 || write != 0 {
        return Err("performance configuration requires a fresh worker".into());
    }
    Ok(())
}

fn next_queue_epoch(
    epoch: u64,
    expected_epoch: u64,
    completed: u64,
    expected_completed: u64,
    write: u64,
) -> Result<u64> {
    require_completed_frontier(completed, write)?;
    if epoch != expected_epoch || completed != expected_completed {
        return Err("queue rollover identity/frontier mismatch".into());
    }
    epoch.checked_add(1).ok_or("queue epoch exhausted".into())
}

const RING: usize = 0;
const CONTROL: usize = 1;
const SIGNAL: usize = 2;
const KERNARG: usize = 3;
const EOP: usize = 4;
const CWSR: usize = 5;

impl Context {
    fn open(device: CheckedGfx950XnackMinusDevice) -> Result<Self> {
        let unique_id = device.observation().unique_id();
        validate_profile(device.topology_snapshot(), unique_id).map_err(str::to_owned)?;
        if rustix::param::page_size() != PAGE_BYTES {
            return Err("unsupported host page size".into());
        }
        let mut context = Self {
            backend: Backend::new(device),
            unique_id,
            runtime: None,
            event: None,
            queue_id: None,
            doorbell: None,
            internal: Vec::new(),
            buffers: BTreeMap::new(),
            kernels: BTreeMap::new(),
            handles: BTreeSet::new(),
            mmap_offsets: BTreeSet::new(),
            total_bytes: 0,
            next_buffer: 1,
            next_kernel: 1,
            ring: AqlSingleProducerRingModelV1::new(
                AqlRingCapacityV1::from_ring_bytes(RING_BYTES as u32).map_err(explain)?,
                0,
                0,
            )
            .map_err(explain)?,
            completed_write: 0,
            queue_epoch: 0,
            last_observed_read: 0,
            performance: None,
            counters: PerformanceCountersV1::default(),
        };
        if let Err(error) = context.initialize() {
            std::mem::forget(context);
            return Err(error);
        }
        Ok(context)
    }

    fn initialize(&mut self) -> Result<()> {
        self.check_currentness(true)?;
        self.backend.acquire_vm().map_err(explain)?;
        self.initialize_queue()
    }

    fn initialize_queue(&mut self) -> Result<()> {
        if self.queue_id.is_some()
            || self.runtime.is_some()
            || self.event.is_some()
            || self.doorbell.is_some()
            || !self.internal.is_empty()
        {
            return Err("queue initialization requires destroyed private resources".into());
        }
        self.check_currentness(true)?;
        self.runtime = Some(
            LinuxKfdRuntimeEnabledV1::enable(self.backend.kfd_fd(), self.backend.opener_pid())
                .map_err(explain)?,
        );
        self.event = Some(
            LinuxQueueExceptionEventV1::create(self.backend.kfd_fd(), self.backend.opener_pid())
                .map_err(explain)?,
        );
        let ring = self.allocate_resource(
            RING_BYTES,
            KfdAllocMemoryFlags::USERPTR_EXECUTABLE,
            |bytes| crate::queue::submit::initialize_invalid_ring(bytes).map_err(explain),
        )?;
        self.internal.push(ring);
        let control = self.allocate_resource(
            PAGE_BYTES,
            KfdAllocMemoryFlags::USERPTR_QUEUE_CONTROL,
            |bytes| crate::queue::submit::initialize_amd_aql_control(bytes).map_err(explain),
        )?;
        self.internal.push(control);
        Backend::initialize_engineering_error_payload(&mut self.internal[CONTROL].mapping)
            .map_err(explain)?;
        let signal =
            self.allocate_resource(PAGE_BYTES, KfdAllocMemoryFlags::KERNARG, |_| Ok(()))?;
        self.internal.push(signal);
        Backend::initialize_engineering_signal(&mut self.internal[SIGNAL].mapping)
            .map_err(explain)?;
        let kernarg = self.allocate_resource(
            MAX_KERNARG_BYTES_V1 as usize,
            KfdAllocMemoryFlags::KERNARG,
            |_| Ok(()),
        )?;
        self.internal.push(kernarg);
        let eop =
            self.allocate_resource(PAGE_BYTES, KfdAllocMemoryFlags::EXECUTABLE, |_| Ok(()))?;
        self.internal.push(eop);
        let payload = self.internal[CONTROL]
            .va
            .checked_add(256)
            .ok_or("error payload address")?;
        let event_id = self
            .event
            .as_ref()
            .ok_or("missing event")?
            .event_id_observation();
        let cwsr = self.allocate_resource(
            CWSR_BYTES,
            KfdAllocMemoryFlags::USERPTR_EXECUTABLE,
            |bytes| initialize_cwsr(bytes, payload, event_id),
        )?;
        self.internal.push(cwsr);
        self.check_currentness(true)?;
        let expected = KfdIoctlCreateQueueArgs {
            ring_base_address: self.internal[RING].va,
            write_pointer_address: self.internal[CONTROL].va + 0x38,
            read_pointer_address: self.internal[CONTROL].va + 0x80,
            doorbell_offset: u64::MAX,
            ring_size: RING_BYTES as u32,
            gpu_id: self.backend.gpu_id(),
            queue_type: KFD_IOC_QUEUE_TYPE_COMPUTE_AQL,
            queue_percentage: 100,
            queue_priority: 0,
            queue_id: u32::MAX,
            eop_buffer_address: self.internal[EOP].va,
            eop_buffer_size: PAGE_BYTES as u64,
            ctx_save_restore_address: self.internal[CWSR].va,
            ctx_save_restore_size: CONTEXT_BYTES_PER_XCC as u32,
            ctl_stack_size: CONTROL_STACK_BYTES,
            sdma_engine_id: 0,
            pad: 0,
        };
        let mut returned = expected;
        crate::queue_linux::create_queue(self.backend.kfd_fd(), &mut returned).map_err(explain)?;
        let plan = admit_doorbell(
            returned.queue_id,
            returned.doorbell_offset,
            self.backend.gpu_id(),
        )
        .map_err(str::to_owned)?;
        let mut unchanged = returned;
        unchanged.queue_id = expected.queue_id;
        unchanged.doorbell_offset = expected.doorbell_offset;
        if unchanged != expected {
            return Err("CREATE_QUEUE changed immutable inputs".into());
        }
        self.queue_id = Some(returned.queue_id);
        self.runtime
            .as_mut()
            .ok_or("missing runtime")?
            .mark_queue_created()
            .map_err(explain)?;
        self.doorbell = Some(
            LinuxDoorbellSliceV1::map_gfx950(
                self.backend.kfd_fd(),
                plan,
                self.backend.opener_pid(),
            )
            .map_err(explain)?,
        );
        self.check_idle()
    }

    fn profile_started(&self) -> Option<Instant> {
        self.performance
            .filter(|options| options.profile)
            .map(|_| Instant::now())
    }

    fn configure_performance(&mut self, options: PerformanceOptions) -> Result<()> {
        require_fresh_configuration(
            self.performance.is_some(),
            self.next_buffer,
            self.next_kernel,
            self.ring.write(),
        )?;
        self.check_currentness(true)?;
        self.check_idle()?;
        self.performance = Some(options);
        Ok(())
    }

    fn check_currentness(&mut self, lifecycle: bool) -> Result<()> {
        let started = self.profile_started();
        let operational = !lifecycle
            && self
                .performance
                .is_some_and(|options| options.operational_currentness);
        let result = if operational {
            self.backend.check_engineering_operational_currentness()
        } else {
            self.backend.check_currentness()
        };
        if started.is_some() {
            if operational {
                add_counter(&mut self.counters.operational_currentness_checks, 1)?;
                record_elapsed(&mut self.counters.operational_currentness_ns, started)?;
            } else {
                add_counter(&mut self.counters.full_currentness_checks, 1)?;
                record_elapsed(&mut self.counters.full_currentness_ns, started)?;
            }
        }
        result.map_err(explain)
    }

    fn allocate_resource(
        &mut self,
        requested: usize,
        flags: KfdAllocMemoryFlags,
        initialize: impl FnOnce(&mut [u8]) -> Result<()>,
    ) -> Result<Allocation> {
        if requested == 0
            || requested as u64 > MAX_BUFFER_BYTES
            || self.handles.len() >= MAX_ALLOCATIONS + 16 + MAX_KERNELS
        {
            return Err("allocation bounds".into());
        }
        let backing = requested
            .checked_add(PAGE_BYTES - 1)
            .ok_or("allocation rounding")?
            & !(PAGE_BYTES - 1);
        let total = self
            .total_bytes
            .checked_add(backing as u64)
            .filter(|total| *total <= MAX_TOTAL_BYTES)
            .ok_or("total allocation bound")?;
        self.check_currentness(true)?;
        let mut reservation = self.backend.reserve_va(backing).map_err(explain)?;
        let va = Backend::reservation_address(&reservation);
        let aperture = self.backend.gpuvm_aperture();
        if va == 0
            || va < aperture.base()
            || !va.is_multiple_of(PAGE_BYTES as u64)
            || va
                .checked_add(backing as u64 - 1)
                .is_none_or(|end| end > aperture.limit())
        {
            return Err("reserved GPU aperture range".into());
        }
        let userptr = flags == KfdAllocMemoryFlags::USERPTR_EXECUTABLE
            || flags == KfdAllocMemoryFlags::USERPTR_QUEUE_CONTROL;
        let mut prepared = if userptr {
            Some(
                self.backend
                    .prepare_userptr(&mut reservation, backing)
                    .map_err(explain)?,
            )
        } else {
            None
        };
        let outcome = if userptr {
            self.backend.alloc_userptr(va, backing as u64, flags)
        } else {
            self.backend.alloc(va, backing as u64, flags)
        };
        outcome.result.map_err(explain)?;
        let args = outcome.value;
        let mmap_offset = admit_allocation_output(
            &args,
            &KfdIoctlAllocMemoryOfGpuArgs::new(va, backing as u64, self.backend.gpu_id(), flags),
            userptr,
            &self.handles,
            &self.mmap_offsets,
        )?;
        self.handles.insert(args.handle);
        // USERPTR's returned BO offset is opaque and never used for mmap or
        // ownership. Only separately CPU-mapped BO offsets must be unique.
        if let Some(offset) = mmap_offset {
            self.mmap_offsets.insert(offset);
        }
        self.total_bytes = total;
        let mut mapping = if let Some(mapping) = prepared.take() {
            mapping
        } else {
            let mut mapping = self
                .backend
                .map_cpu(&mut reservation, args.mmap_offset, backing)
                .map_err(explain)?;
            self.backend
                .prepare_cpu_mapping(&mut mapping)
                .map_err(explain)?;
            mapping
        };
        if Backend::mapping_address(&mapping) != va {
            return Err("CPU/GPU identity mapping".into());
        }
        Backend::with_bytes_mut(&mut mapping, backing, |bytes| {
            bytes.fill(0);
            initialize(bytes)
        })?;
        let outcome = self.backend.map_gpu(args.handle, 0);
        outcome.result.map_err(explain)?;
        if outcome.value != 1 {
            return Err("incomplete selected-device mapping".into());
        }
        self.check_currentness(true)?;
        Ok(Allocation {
            reservation,
            mapping,
            handle: args.handle,
            va,
            requested,
            backing,
            mmap_offset,
        })
    }

    fn release_resource(&mut self, mut allocation: Allocation) -> Result<()> {
        self.check_currentness(true)?;
        let outcome = self.backend.unmap_gpu(allocation.handle, 0);
        outcome.result.map_err(explain)?;
        if outcome.value != 1 {
            return Err("incomplete selected-device unmapping".into());
        }
        self.backend
            .unmap_cpu(&mut allocation.mapping)
            .map_err(explain)?;
        self.backend.free(allocation.handle).map_err(explain)?;
        self.backend
            .release_va_reservation(&mut allocation.reservation)
            .map_err(explain)?;
        if !self.handles.remove(&allocation.handle) {
            return Err("allocation handle ownership".into());
        }
        if allocation
            .mmap_offset
            .is_some_and(|offset| !self.mmap_offsets.remove(&offset))
        {
            return Err("allocation mmap-offset ownership".into());
        }
        self.total_bytes = self
            .total_bytes
            .checked_sub(allocation.backing as u64)
            .ok_or("allocation accounting")?;
        self.check_currentness(true)
    }

    fn check_idle(&mut self) -> Result<()> {
        self.check_currentness(false)?;
        let (write, read) =
            Backend::observe_aql_counters(&mut self.internal[CONTROL].mapping, PAGE_BYTES)
                .map_err(explain)?;
        require_completed_frontier(self.completed_write, self.ring.write())?;
        validate_counters(self.ring.write(), self.last_observed_read, (write, read))?;
        self.last_observed_read = read;
        if Backend::observe_i64_acquire(&mut self.internal[CONTROL].mapping, PAGE_BYTES, 256)
            .map_err(explain)?
            != 0
        {
            return Err("queue exception payload is nonzero".into());
        }
        Ok(())
    }

    fn allocate(&mut self, bytes: u64) -> Result<ResponseV1> {
        self.check_idle()?;
        if self.buffers.len() >= MAX_ALLOCATIONS {
            return Err("live user allocation limit".into());
        }
        let id = self.next_buffer;
        self.next_buffer = id.checked_add(1).ok_or("buffer ID exhausted")?;
        let bytes_usize = usize::try_from(bytes).map_err(explain)?;
        let allocation = self.allocate_resource(
            bytes_usize,
            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
            |_| Ok(()),
        )?;
        self.buffers.insert(id, allocation);
        Ok(ResponseV1::Allocated { buffer: id, bytes })
    }

    fn free(&mut self, id: u64) -> Result<()> {
        self.check_idle()?;
        let allocation = self.buffers.remove(&id).ok_or("unknown buffer")?;
        self.release_resource(allocation)
    }

    fn write(&mut self, id: u64, offset: u64, bytes: &[u8]) -> Result<()> {
        let started = self.profile_started();
        self.check_idle()?;
        let allocation = self.buffers.get_mut(&id).ok_or("unknown buffer")?;
        let range = checked_range(allocation.requested as u64, offset, bytes.len() as u64)
            .map_err(str::to_owned)?;
        Backend::with_bytes_mut(&mut allocation.mapping, allocation.requested, |mapped| {
            mapped[range].copy_from_slice(bytes)
        });
        self.check_currentness(false)?;
        if started.is_some() {
            add_counter(&mut self.counters.writes, 1)?;
            add_counter(&mut self.counters.write_bytes, bytes.len() as u64)?;
            record_elapsed(&mut self.counters.write_ns, started)?;
        }
        Ok(())
    }

    fn read(&mut self, id: u64, offset: u64, bytes: u32) -> Result<Vec<u8>> {
        let started = self.profile_started();
        self.check_idle()?;
        let allocation = self.buffers.get(&id).ok_or("unknown buffer")?;
        let range = checked_range(allocation.requested as u64, offset, u64::from(bytes))
            .map_err(str::to_owned)?;
        let result = Backend::with_bytes(&allocation.mapping, allocation.requested, |mapped| {
            mapped[range].to_vec()
        });
        self.check_currentness(false)?;
        if started.is_some() {
            add_counter(&mut self.counters.reads, 1)?;
            add_counter(&mut self.counters.read_bytes, u64::from(bytes))?;
            record_elapsed(&mut self.counters.read_ns, started)?;
        }
        Ok(result)
    }

    fn load(
        &mut self,
        object: Vec<u8>,
        expected_hash: [u8; 32],
        symbol: String,
    ) -> Result<ResponseV1> {
        self.check_idle()?;
        self.check_currentness(true)?;
        if self.kernels.len() >= MAX_KERNELS
            || <[u8; 32]>::from(Sha256::digest(&object)) != expected_hash
        {
            return Err("kernel count or object hash".into());
        }
        let admission_started = self.profile_started();
        let closure = fe2o3_amdhsa_loader::validate(&object, AdmittedProfile::Gfx950XnackOffCov6)
            .map_err(explain)?
            .bind_kernel(&symbol)
            .map_err(explain)?;
        let resources = closure.resources();
        let inspected = closure.selected_kernel().clone();
        if admission_started.is_some() {
            add_counter(&mut self.counters.kernel_admissions, 1)?;
            record_elapsed(&mut self.counters.kernel_admission_ns, admission_started)?;
        }
        if resources.wavefront_size() != 64
            || resources.private_segment_fixed_size() != 0
            || resources.group_segment_fixed_size() > 160 * 1024
            || resources.kernarg_segment_size() > u64::from(MAX_KERNARG_BYTES_V1)
            || resources.kernarg_segment_alignment() > PAGE_BYTES as u64
            || resources.cluster_dims().is_some()
        {
            return Err("unsupported gfx950 engineering kernel resources".into());
        }
        let metadata = describe_kernel(closure.selected_kernel(), expected_hash)?;
        let descriptor_offset = closure
            .selected_binding()
            .descriptor_address()
            .checked_sub(closure.envelope().plan().image_start())
            .ok_or("descriptor image offset")?;
        let image_len =
            usize::try_from(closure.envelope().materialization().image_len()).map_err(explain)?;
        if descriptor_offset
            .checked_add(64)
            .is_none_or(|end| end > image_len as u64)
            || !descriptor_offset.is_multiple_of(64)
        {
            return Err("descriptor image range/alignment".into());
        }
        let code = self.allocate_resource(image_len, KfdAllocMemoryFlags::EXECUTABLE, |bytes| {
            closure.materialize_into(bytes).map_err(explain)
        })?;
        drop(closure);
        let id = self.next_kernel;
        self.next_kernel = id.checked_add(1).ok_or("kernel ID exhausted")?;
        self.kernels.insert(
            id,
            Kernel {
                object,
                inspected,
                metadata: metadata.clone(),
                resources,
                code,
                descriptor_offset,
            },
        );
        Ok(ResponseV1::LoadedKernel {
            kernel: id,
            metadata,
        })
    }

    fn prepare_dispatch(
        &mut self,
        id: u64,
        bytes: Vec<u8>,
        workgroup: [u16; 3],
        grid: [u32; 3],
        pointers: &[PointerFixupV1],
    ) -> Result<PreparedDispatch> {
        self.prepare_dispatch_with_peer_bindings(id, bytes, workgroup, grid, pointers, None)
    }

    fn prepare_dispatch_with_peer_bindings(
        &mut self,
        id: u64,
        mut bytes: Vec<u8>,
        workgroup: [u16; 3],
        grid: [u32; 3],
        pointers: &[PointerFixupV1],
        peer_bindings: Option<&BTreeMap<u64, (u64, u64)>>,
    ) -> Result<PreparedDispatch> {
        let prepare_started = self.profile_started();
        self.check_idle()?;
        let geometry =
            AqlDispatchGeometryV1::new(grid, workgroup.map(u32::from)).map_err(explain)?;
        let admission_started = self.profile_started();
        let kernel = self.kernels.get(&id).ok_or("unknown kernel")?;
        let product = workgroup
            .iter()
            .try_fold(1_u32, |n, value| n.checked_mul(u32::from(*value)))
            .ok_or("workgroup product")?;
        if product > kernel.resources.max_flat_workgroup_size()
            || kernel
                .resources
                .required_workgroup_size()
                .is_some_and(|required| required != workgroup.map(u32::from))
        {
            return Err("kernel workgroup contract".into());
        }
        for (axis, maximum) in kernel.resources.max_workgroups().into_iter().enumerate() {
            if maximum
                .is_some_and(|maximum| grid[axis].div_ceil(u32::from(workgroup[axis])) > maximum)
            {
                return Err("kernel workgroup count".into());
            }
        }
        // Only immutable owned load-time metadata is cached. Buffer ownership,
        // aliasing, argument values, geometry, and queue state are checked anew.
        let closure = if self
            .performance
            .is_some_and(|options| options.cache_kernel_admission)
        {
            None
        } else {
            Some(
                fe2o3_amdhsa_loader::validate(&kernel.object, AdmittedProfile::Gfx950XnackOffCov6)
                    .map_err(explain)?
                    .bind_kernel(&kernel.metadata.symbol)
                    .map_err(explain)?,
            )
        };
        if closure.is_some() && admission_started.is_some() {
            add_counter(&mut self.counters.kernel_admissions, 1)?;
            record_elapsed(&mut self.counters.kernel_admission_ns, admission_started)?;
        }
        patch_pointer_arguments(&kernel.metadata, &mut bytes, pointers, |id| {
            if let Some(bindings) = peer_bindings {
                bindings.get(&id).copied()
            } else {
                self.buffers
                    .get(&id)
                    .map(|allocation| (allocation.va, allocation.requested as u64))
            }
        })?;
        crate::queue::dispatch_binding::initialize_engineering_cov6_kernarg(
            closure
                .as_ref()
                .map_or(&kernel.inspected, |closure| closure.selected_kernel()),
            geometry,
            &mut bytes,
        )
        .map_err(explain)?;
        let descriptor = kernel
            .code
            .va
            .checked_add(kernel.descriptor_offset)
            .ok_or("descriptor address")?;
        let alignment = u64::from(kernel.metadata.kernarg_alignment);
        let group_bytes = kernel.metadata.group_segment_bytes;
        drop(closure);
        record_elapsed(&mut self.counters.dispatch_prepare_ns, prepare_started)?;
        Ok(PreparedDispatch {
            bytes,
            geometry,
            descriptor,
            alignment,
            group_bytes,
        })
    }

    /// The enclosing process opted into unauthenticated machine code. Exact
    /// target/ABI/ownership checks do not prove the code honors these bounds.
    unsafe fn dispatch(
        &mut self,
        id: u64,
        bytes: Vec<u8>,
        workgroup: [u16; 3],
        grid: [u32; 3],
        pointers: &[PointerFixupV1],
        timeout_ms: u32,
    ) -> Result<u64> {
        let prepared = self.prepare_dispatch(id, bytes, workgroup, grid, pointers)?;
        // SAFETY: the same dedicated-process and trusted-code contract applies.
        unsafe { self.execute_prepared_dispatch(prepared, timeout_ms) }
    }

    unsafe fn execute_prepared_dispatch(
        &mut self,
        prepared: PreparedDispatch,
        timeout_ms: u32,
    ) -> Result<u64> {
        let PreparedDispatch {
            bytes,
            geometry,
            descriptor,
            alignment,
            group_bytes,
        } = prepared;
        let reservation = self
            .ring
            .reserve_one(self.last_observed_read)
            .map_err(explain)?;
        let next = reservation.next_write();
        let publish_started = self.profile_started();
        let started = Instant::now();
        Backend::with_bytes_mut(
            &mut self.internal[KERNARG].mapping,
            MAX_KERNARG_BYTES_V1 as usize,
            |mapped| {
                mapped.fill(0);
                mapped[..bytes.len()].copy_from_slice(&bytes);
            },
        );
        Backend::reset_completion_signal_release(&mut self.internal[SIGNAL].mapping, PAGE_BYTES, 0)
            .map_err(explain)?;
        let packet = AqlKernelDispatchPacketV1::new_unpublished(
            geometry,
            0,
            group_bytes,
            ObservedGpuAddressV1::new(descriptor).map_err(explain)?,
            ObservedGpuAddressV1::new(self.internal[KERNARG].va).map_err(explain)?,
            alignment,
            ObservedGpuAddressV1::new(self.internal[SIGNAL].va).map_err(explain)?,
        )
        .map_err(explain)?;
        self.check_currentness(false)?;
        let prior =
            Backend::fetch_add_aql_write(&mut self.internal[CONTROL].mapping, PAGE_BYTES, 1)
                .map_err(explain)?;
        if prior != reservation.packet_id() {
            return Err("queue write reservation substitution".into());
        }
        packet.publish_with(&mut Publication {
            mapping: &mut self.internal[RING].mapping,
            slot: reservation.slot_index(),
        })?;
        self.doorbell
            .as_mut()
            .ok_or("missing doorbell")?
            .store_packet_id_release(reservation.packet_id())
            .map_err(explain)?;
        record_elapsed(&mut self.counters.dispatch_publish_ns, publish_started)?;
        let wait_started = self.profile_started();
        let deadline = started
            .checked_add(Duration::from_millis(u64::from(timeout_ms)))
            .ok_or("dispatch deadline")?;
        let mut next_currentness = started;
        loop {
            if wait_started.is_some() {
                add_counter(&mut self.counters.completion_polls, 1)?;
            }
            let completion = Backend::observe_completion_signal_acquire(
                &mut self.internal[SIGNAL].mapping,
                PAGE_BYTES,
                0,
            )
            .map_err(explain)?;
            let counters =
                Backend::observe_aql_counters(&mut self.internal[CONTROL].mapping, PAGE_BYTES)
                    .map_err(explain)?;
            let exception =
                Backend::observe_i64_acquire(&mut self.internal[CONTROL].mapping, PAGE_BYTES, 256)
                    .map_err(explain)?;
            let completed = dispatch_completed(
                next,
                self.last_observed_read,
                counters,
                completion,
                exception,
            )?;
            self.last_observed_read = counters.1;
            if completed {
                break;
            }
            let now = Instant::now();
            if now >= deadline {
                return Err(format!(
                    "dispatch timeout: expected_write={next}, write={}, read={}, completion={completion:?}, exception={exception}; process teardown required",
                    counters.0, counters.1
                ));
            }
            if now >= next_currentness {
                self.check_currentness(false)?;
                next_currentness = now + Duration::from_millis(100);
            }
            std::thread::sleep(Duration::from_micros(50));
        }
        self.completed_write = next;
        record_elapsed(&mut self.counters.dispatch_wait_ns, wait_started)?;
        self.check_idle()?;
        if publish_started.is_some() {
            add_counter(&mut self.counters.dispatches, 1)?;
        }
        u64::try_from(started.elapsed().as_nanos()).map_err(explain)
    }

    unsafe fn dispatch_sequence(
        &mut self,
        dispatches: Vec<SequenceDispatchV1>,
        payload: Vec<u8>,
    ) -> ResponseV1 {
        let mut elapsed_ns = Vec::with_capacity(dispatches.len());
        let mut attempted_dispatches = 0_u32;
        let result = (|| -> Result<()> {
            let expected_payload = CommandV1::DispatchSequence {
                dispatches: dispatches.clone(),
            }
            .payload_bytes()
            .map_err(explain)?;
            if expected_payload != payload.len() {
                return Err("sequence payload length".into());
            }
            self.check_currentness(true)?;
            self.check_idle()?;
            require_sequence_capacity(
                self.ring.write(),
                self.last_observed_read,
                dispatches.len(),
            )?;
            let mut prepared = Vec::with_capacity(dispatches.len());
            let mut offset = 0_usize;
            // Validate every argument and resource binding before publishing any
            // packet. Only sequential dispatch follows, so lifetimes cannot change.
            for dispatch in dispatches {
                let end = offset
                    .checked_add(dispatch.payload_bytes as usize)
                    .ok_or("sequence payload overflow")?;
                prepared.push((
                    self.prepare_dispatch(
                        dispatch.kernel,
                        payload
                            .get(offset..end)
                            .ok_or("sequence payload bounds")?
                            .to_vec(),
                        dispatch.workgroup,
                        dispatch.grid,
                        &dispatch.pointers,
                    )?,
                    dispatch.timeout_ms,
                ));
                offset = end;
            }
            for (dispatch, timeout) in prepared {
                attempted_dispatches += 1;
                // SAFETY: same owner, no intervening free/load/write operation,
                // and each preceding signal was observed complete before reuse.
                elapsed_ns.push(unsafe { self.execute_prepared_dispatch(dispatch, timeout) }?);
            }
            self.check_currentness(true)?;
            self.check_idle()
        })();
        match result {
            Ok(()) => ResponseV1::DispatchSequenceCompleted { elapsed_ns },
            Err(message) => ResponseV1::DispatchSequenceFailed {
                completed_dispatches: elapsed_ns.len() as u32,
                attempted_dispatches,
                elapsed_ns,
                message,
                fatal: true,
            },
        }
    }

    fn destroy_queue(&mut self) -> Result<LinuxKfdRuntimeDisabledV1> {
        self.check_currentness(true)?;
        self.check_idle()?;
        let queue_id = self.queue_id.ok_or("queue already closed")?;
        let expected = KfdIoctlDestroyQueueArgs::new(queue_id);
        let mut args = expected;
        crate::queue_linux::destroy_queue(self.backend.kfd_fd(), &mut args).map_err(explain)?;
        if args != expected {
            return Err("DESTROY_QUEUE output drift".into());
        }
        self.queue_id = None;
        self.runtime
            .as_mut()
            .ok_or("missing runtime")?
            .mark_queue_destroyed()
            .map_err(explain)?;
        let _destroyed_event = self
            .event
            .take()
            .ok_or("missing event")?
            .destroy(self.backend.kfd_fd(), self.backend.opener_pid())
            .map_err(explain)?;
        self.runtime
            .as_mut()
            .ok_or("missing runtime")?
            .mark_event_destroyed()
            .map_err(explain)?;
        let disabled = self
            .runtime
            .take()
            .ok_or("missing runtime")?
            .disable(self.backend.kfd_fd(), self.backend.opener_pid())
            .map_err(explain)?;
        self.doorbell
            .take()
            .ok_or("missing doorbell")?
            .release()
            .map_err(explain)?;
        Ok(disabled)
    }

    fn rollover_queue(
        &mut self,
        expected_epoch: u64,
        expected_completed: u64,
    ) -> Result<ResponseV1> {
        let next_epoch = next_queue_epoch(
            self.queue_epoch,
            expected_epoch,
            self.completed_write,
            expected_completed,
            self.ring.write(),
        )?;
        self.check_currentness(true)?;
        self.check_idle()?;
        let retired_packets = self.completed_write;
        let disabled = self.destroy_queue()?;
        // The old queue is destroyed before its ring, signals, kernargs, EOP,
        // and CWSR are released. Completion never fabricates a hardware read.
        while let Some(allocation) = self.internal.pop() {
            self.release_resource(allocation)?;
        }
        disabled.complete();
        self.check_currentness(true)?;
        self.ring = AqlSingleProducerRingModelV1::new(
            AqlRingCapacityV1::from_ring_bytes(RING_BYTES as u32).map_err(explain)?,
            0,
            0,
        )
        .map_err(explain)?;
        self.completed_write = 0;
        self.last_observed_read = 0;
        self.initialize_queue()?;
        self.check_currentness(true)?;
        self.queue_epoch = next_epoch;
        Ok(ResponseV1::QueueRolledOver {
            retired_packets,
            queue_epoch: next_epoch,
        })
    }

    fn close_inner(&mut self) -> Result<()> {
        let disabled = self.destroy_queue()?;
        while let Some((_, kernel)) = self.kernels.pop_last() {
            self.release_resource(kernel.code)?;
        }
        while let Some((_, allocation)) = self.buffers.pop_last() {
            self.release_resource(allocation)?;
        }
        while let Some(allocation) = self.internal.pop() {
            self.release_resource(allocation)?;
        }
        if !self.handles.is_empty() || !self.mmap_offsets.is_empty() || self.total_bytes != 0 {
            return Err("incomplete teardown accounting".into());
        }
        disabled.complete();
        Ok(())
    }
}

struct Publication<'a> {
    mapping: &'a mut LinuxCpuMapping,
    slot: u32,
}

fn validate_counters(expected_write: u64, last_read: u64, counters: (u64, u64)) -> Result<()> {
    let (write, read) = counters;
    if write != expected_write
        || read < last_read
        || read > write
        || write - read > MAX_UNRETIRED_RING_PACKETS_V1
    {
        return Err("queue counter drift".into());
    }
    Ok(())
}

fn require_completed_frontier(completed_write: u64, write: u64) -> Result<()> {
    if completed_write != write {
        return Err("queue has incomplete dispatch".into());
    }
    Ok(())
}

fn require_sequence_capacity(write: u64, read: u64, dispatches: usize) -> Result<()> {
    if dispatches == 0
        || dispatches > MAX_SEQUENCE_DISPATCHES_V1
        || read > write
        || write
            .checked_add(dispatches as u64)
            .and_then(|next| next.checked_sub(read))
            .is_none_or(|outstanding| outstanding > MAX_UNRETIRED_RING_PACKETS_V1)
    {
        return Err("sequence exceeds retained ring capacity; rollover required".into());
    }
    Ok(())
}

fn dispatch_completed(
    expected_write: u64,
    last_read: u64,
    counters: (u64, u64),
    completion: AqlCompletionObservationV1,
    exception: i64,
) -> Result<bool> {
    if expected_write == 0 {
        return Err("no submitted dispatch".into());
    }
    validate_counters(expected_write, last_read, counters)?;
    if exception != 0 {
        return Err("queue exception".into());
    }
    match completion {
        AqlCompletionObservationV1::Completed => Ok(true),
        AqlCompletionObservationV1::Pending => Ok(false),
        AqlCompletionObservationV1::Unexpected(value) => {
            Err(format!("unexpected completion {value}"))
        }
    }
}

fn admit_allocation_output(
    args: &KfdIoctlAllocMemoryOfGpuArgs,
    expected: &KfdIoctlAllocMemoryOfGpuArgs,
    userptr: bool,
    handles: &BTreeSet<u64>,
    mmap_offsets: &BTreeSet<u64>,
) -> Result<Option<u64>> {
    if args.va_addr != expected.va_addr
        || args.size != expected.size
        || args.flags != expected.flags
        || args.gpu_id != expected.gpu_id
        || args.handle == 0
        || handles.contains(&args.handle)
        || (!userptr
            && (args.mmap_offset == 0
                || !args.mmap_offset.is_multiple_of(PAGE_BYTES as u64)
                || mmap_offsets.contains(&args.mmap_offset)))
    {
        return Err("ALLOC_MEMORY_OF_GPU output identity".into());
    }
    Ok((!userptr).then_some(args.mmap_offset))
}
impl AqlPacketPublicationTargetV1 for Publication<'_> {
    type Error = String;
    fn write_unpublished(&mut self, packet: &AqlKernelDispatchPacketV1) -> Result<()> {
        Backend::write_aql_slot(
            self.mapping,
            RING_BYTES,
            self.slot,
            &packet.encode_unpublished_le(),
        )
        .map_err(explain)
    }
    fn publish_release_header(&mut self, header: u16) -> Result<()> {
        Backend::publish_aql_header(self.mapping, RING_BYTES, self.slot, header).map_err(explain)
    }
}

fn initialize_cwsr(bytes: &mut [u8], payload: u64, event_id: u32) -> Result<()> {
    use fe2o3_kfd_uapi::{
        KfdContextSaveAreaHeaderV1, KfdQueueExceptionPayloadAddressV1, KfdSignalEventIdV1,
    };
    if bytes.len() != CWSR_BYTES {
        return Err("CWSR exact mapping".into());
    }
    let payload = KfdQueueExceptionPayloadAddressV1::new(payload).ok_or("error payload address")?;
    let event = KfdSignalEventIdV1::new(event_id).ok_or("error event ID")?;
    for xcc in 0..XCC_COUNT {
        let header = KfdContextSaveAreaHeaderV1::new_queue_exception(
            ((XCC_COUNT - xcc) * CONTEXT_BYTES_PER_XCC) as u32,
            DEBUG_BYTES_TOTAL,
            payload,
            event,
        )
        .map_err(explain)?;
        let offset = xcc * CONTEXT_BYTES_PER_XCC;
        bytes[offset..offset + 16].fill(0);
        bytes[offset + 16..offset + 20].copy_from_slice(&header.debug_offset().to_le_bytes());
        bytes[offset + 20..offset + 24].copy_from_slice(&header.debug_size().to_le_bytes());
        bytes[offset + 24..offset + 32]
            .copy_from_slice(&header.error_payload_address().to_le_bytes());
        bytes[offset + 32..offset + 36].copy_from_slice(&header.error_event_id().to_le_bytes());
        bytes[offset + 36..offset + 40].fill(0);
    }
    Ok(())
}

fn describe_kernel(kernel: &InspectedKernel, hash: [u8; 32]) -> Result<KernelMetadataV1> {
    let explicit_arguments = kernel
        .explicit_arguments()
        .iter()
        .map(|argument| {
            if !matches!(
                argument.value_kind(),
                ExplicitValueKind::ByValue | ExplicitValueKind::GlobalBuffer
            ) {
                return Err("unsupported explicit argument kind".into());
            }
            Ok(ExplicitArgumentV1 {
                offset: u32::try_from(argument.offset()).map_err(explain)?,
                bytes: u32::try_from(argument.size()).map_err(explain)?,
                global_buffer: argument.value_kind() == ExplicitValueKind::GlobalBuffer,
                pointee_alignment: argument
                    .pointee_alignment()
                    .map(u32::try_from)
                    .transpose()
                    .map_err(explain)?,
                access: argument.access().map(|access| match access {
                    ArgumentAccess::ReadOnly => BufferAccessV1::Read,
                    ArgumentAccess::WriteOnly => BufferAccessV1::Write,
                    ArgumentAccess::ReadWrite => BufferAccessV1::ReadWrite,
                }),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if !kernel.arguments_were_emitted() {
        return Err("kernel argument metadata absent".into());
    }
    Ok(KernelMetadataV1 {
        symbol: kernel.name().to_owned(),
        object_sha256: hash,
        kernarg_bytes: u32::try_from(kernel.kernarg_segment_size()).map_err(explain)?,
        kernarg_alignment: u32::try_from(kernel.kernarg_segment_alignment()).map_err(explain)?,
        group_segment_bytes: u32::try_from(kernel.group_segment_fixed_size()).map_err(explain)?,
        private_segment_bytes: u32::try_from(kernel.private_segment_fixed_size())
            .map_err(explain)?,
        wavefront_size: kernel.wavefront_size(),
        implicit_argument_offset: kernel
            .implicit_argument_offset()
            .map(u32::try_from)
            .transpose()
            .map_err(explain)?,
        implicit_argument_bytes: u32::try_from(kernel.implicit_argument_size()).map_err(explain)?,
        explicit_arguments,
    })
}

fn patch_pointer_arguments(
    metadata: &KernelMetadataV1,
    bytes: &mut [u8],
    pointers: &[PointerFixupV1],
    allocation: impl Fn(u64) -> Option<(u64, u64)>,
) -> Result<()> {
    if bytes.len() != metadata.kernarg_bytes as usize
        || pointers.len()
            != metadata
                .explicit_arguments
                .iter()
                .filter(|arg| arg.global_buffer)
                .count()
    {
        return Err("kernarg bytes or pointer cardinality".into());
    }
    let mut seen = BTreeSet::new();
    let mut values = Vec::with_capacity(pointers.len());
    for (index, pointer) in pointers.iter().enumerate() {
        let argument = metadata
            .explicit_arguments
            .iter()
            .find(|argument| argument.global_buffer && argument.offset == pointer.kernarg_offset)
            .ok_or("pointer does not match global-buffer metadata")?;
        if argument.bytes != 8
            || !seen.insert(pointer.kernarg_offset)
            || argument
                .access
                .is_some_and(|access| access != pointer.access)
        {
            return Err("pointer ABI or access mismatch".into());
        }
        let (base, requested) = allocation(pointer.buffer).ok_or("pointer buffer is not owned")?;
        checked_range(requested, pointer.buffer_offset, pointer.extent_bytes)
            .map_err(str::to_owned)?;
        let address = base
            .checked_add(pointer.buffer_offset)
            .filter(|address| *address != 0)
            .ok_or("pointer address overflow or null")?;
        let alignment = argument.pointee_alignment.unwrap_or(1);
        if !alignment.is_power_of_two()
            || alignment > PAGE_BYTES as u32
            || !address.is_multiple_of(u64::from(alignment))
        {
            return Err("pointer alignment".into());
        }
        let range = checked_range(bytes.len() as u64, u64::from(pointer.kernarg_offset), 8)
            .map_err(str::to_owned)?;
        if bytes[range.clone()].iter().any(|byte| *byte != 0) {
            return Err("caller pointer slot is not zero".into());
        }
        for prior in &pointers[..index] {
            if prior.buffer == pointer.buffer
                && prior.extent_bytes != 0
                && pointer.extent_bytes != 0
                && (prior.access != BufferAccessV1::Read || pointer.access != BufferAccessV1::Read)
                && prior.buffer_offset < pointer.buffer_offset + pointer.extent_bytes
                && pointer.buffer_offset < prior.buffer_offset + prior.extent_bytes
            {
                return Err("overlapping mutable pointer arguments".into());
            }
        }
        values.push((range, address));
    }
    for (range, value) in values {
        bytes[range].copy_from_slice(&value.to_le_bytes());
    }
    Ok(())
}

/// Runs one explicitly unqualified native-code worker over stdin/stdout.
///
/// # Safety
/// This is an expert machine-code execution boundary, not a safe HSACO loader.
/// Call only in a dedicated disposable process with no shared Rust memory or
/// unrelated work. The operator must authorize the selected device and trust
/// each supplied kernel to honor its declared argument extents/accesses and
/// the AMDHSA ABI. Structural validation does not prove those obligations.
/// Any error is terminal: the caller must immediately terminate this process.
/// No protected publication, load, launch, or service authority is granted.
///
/// ```compile_fail
/// // The expert process entry cannot be called through safe Rust alone.
/// fe2o3_kfd::run_gfx950_engineering_worker_unchecked_v1(1).unwrap();
/// ```
pub unsafe fn run_gfx950_engineering_worker_unchecked_v1(unique_id: u64) -> Result<()> {
    let kfd = OpenedKfd::open_default()
        .map_err(explain)?
        .admit_uapi()
        .map_err(explain)?;
    let device = kfd
        .bind_gfx950_xnack_minus(DeviceSelector::UniqueId(unique_id))
        .map_err(explain)?;
    let mut context = Context::open(device)?;
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut fatal_response_written = false;
    let result = (|| -> Result<()> {
        write_header_v1(
            &mut output,
            &ResponseV1::Ready {
                protocol: PROTOCOL_VERSION_V1,
                target: TARGET_V1.into(),
                device_unique_id: context.unique_id,
                authority: "none".into(),
            },
        )
        .map_err(explain)?;
        output.flush().map_err(explain)?;
        loop {
            let Some(command) = read_header_v1::<CommandV1>(&mut input).map_err(explain)? else {
                return context.close_inner();
            };
            let payload_bytes = command.payload_bytes().map_err(explain)?;
            let mut payload = vec![0; payload_bytes];
            input.read_exact(&mut payload).map_err(explain)?;
            let command_started = context.profile_started();
            let mut response_payload = Vec::new();
            let response = match command {
                CommandV1::ConfigurePerformance {
                    cache_kernel_admission,
                    operational_currentness,
                    profile,
                } => {
                    context.configure_performance(PerformanceOptions {
                        cache_kernel_admission,
                        operational_currentness,
                        profile,
                    })?;
                    ResponseV1::PerformanceConfigured
                }
                CommandV1::PerformanceSnapshot => {
                    if !context.performance.is_some_and(|options| options.profile) {
                        return Err("performance profiling was not enabled".into());
                    }
                    context.check_idle()?;
                    ResponseV1::PerformanceSnapshot {
                        counters: context.counters.clone(),
                    }
                }
                CommandV1::RolloverQueue {
                    expected_epoch,
                    expected_completed_packets,
                } => context.rollover_queue(expected_epoch, expected_completed_packets)?,
                CommandV1::DispatchSequence { dispatches } => {
                    // SAFETY: the entry's disposable-process/trusted-code contract
                    // covers every item, and the complete sequence is prevalidated.
                    unsafe { context.dispatch_sequence(dispatches, payload) }
                }
                CommandV1::Allocate { bytes } => context.allocate(bytes)?,
                CommandV1::Free { buffer } => {
                    context.free(buffer)?;
                    ResponseV1::Freed
                }
                CommandV1::Write { buffer, offset, .. } => {
                    context.write(buffer, offset, &payload)?;
                    ResponseV1::Written
                }
                CommandV1::Read {
                    buffer,
                    offset,
                    bytes,
                } => {
                    response_payload = context.read(buffer, offset, bytes)?;
                    ResponseV1::Read {
                        payload_bytes: bytes,
                    }
                }
                CommandV1::LoadKernel {
                    object_sha256,
                    symbol,
                    ..
                } => context.load(payload, object_sha256, symbol)?,
                CommandV1::Dispatch {
                    kernel,
                    workgroup,
                    grid,
                    pointers,
                    timeout_ms,
                    ..
                } => {
                    // SAFETY: this entire entry requires the dedicated-process
                    // and trusted-machine-code contract stated above. The
                    // pointer/target checks supplement, not discharge, it.
                    let elapsed_ns = unsafe {
                        context.dispatch(kernel, payload, workgroup, grid, &pointers, timeout_ms)
                    }?;
                    ResponseV1::Dispatched { elapsed_ns }
                }
                CommandV1::Close => {
                    context.close_inner()?;
                    ResponseV1::Closed
                }
            };
            let terminal = match &response {
                ResponseV1::DispatchSequenceFailed { message, .. } => Some(message.clone()),
                _ => None,
            };
            if command_started.is_some() {
                add_counter(&mut context.counters.commands, 1)?;
                record_elapsed(&mut context.counters.command_ns, command_started)?;
            }
            let closed = matches!(response, ResponseV1::Closed);
            write_header_v1(&mut output, &response).map_err(explain)?;
            output.write_all(&response_payload).map_err(explain)?;
            output.flush().map_err(explain)?;
            if let Some(message) = terminal {
                fatal_response_written = true;
                return Err(message);
            }
            if closed {
                return Ok(());
            }
        }
    })();
    if let Err(error) = &result {
        if !fatal_response_written {
            let _ = write_header_v1(
                &mut output,
                &ResponseV1::Error {
                    message: error.clone(),
                    fatal: true,
                },
            );
            let _ = output.flush();
        }
        std::mem::forget(context);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_capacity_never_fabricates_retirement_or_wraps() {
        let full = MAX_UNRETIRED_RING_PACKETS_V1;
        assert!(require_sequence_capacity(0, 0, 16).is_ok());
        assert!(require_sequence_capacity(full - 16, 0, 16).is_ok());
        assert!(require_sequence_capacity(full - 15, 0, 16).is_err());
        assert!(require_sequence_capacity(full, 0, 1).is_err());
        assert!(require_sequence_capacity(full, 1, 1).is_ok());
        assert!(require_sequence_capacity(0, 1, 1).is_err());
        assert!(require_sequence_capacity(u64::MAX, u64::MAX, 1).is_err());
        assert!(require_sequence_capacity(0, 0, 0).is_err());
        assert!(require_sequence_capacity(0, 0, 17).is_err());
    }

    #[test]
    fn sequence_prevalidates_all_bindings_before_any_publication() {
        let source = include_str!("engineering_gfx950.rs");
        let body = source
            .split("unsafe fn dispatch_sequence(")
            .nth(1)
            .unwrap()
            .split("fn destroy_queue(")
            .next()
            .unwrap();
        assert!(
            body.find("self.prepare_dispatch(").unwrap()
                < body.find("for (dispatch, timeout) in prepared").unwrap()
        );
        assert!(
            body.find("require_sequence_capacity(").unwrap()
                < body.find("self.execute_prepared_dispatch(").unwrap()
        );
        assert!(body.contains("attempted_dispatches += 1"));
        assert!(body.contains("completed_dispatches: elapsed_ns.len() as u32"));
        assert!(!body.contains("self.free("));
        assert!(!body.contains("self.load("));
    }

    #[test]
    fn rollover_requires_exact_epoch_and_successful_completion_frontier() {
        assert_eq!(next_queue_epoch(0, 0, 0, 0, 0).unwrap(), 1);
        assert_eq!(
            next_queue_epoch(
                9,
                9,
                MAX_UNRETIRED_RING_PACKETS_V1,
                MAX_UNRETIRED_RING_PACKETS_V1,
                MAX_UNRETIRED_RING_PACKETS_V1
            )
            .unwrap(),
            10
        );
        for (epoch, expected_epoch, complete, expected_complete, write) in [
            (1, 0, 3, 3, 3),
            (1, 1, 3, 2, 3),
            (1, 1, 3, 3, 4),
            (1, 1, 4, 4, 3),
            (u64::MAX, u64::MAX, 3, 3, 3),
        ] {
            assert!(
                next_queue_epoch(epoch, expected_epoch, complete, expected_complete, write)
                    .is_err()
            );
        }
    }

    #[test]
    fn rollover_destroys_old_queue_before_releasing_or_resetting_and_retains_user_resources() {
        let source = include_str!("engineering_gfx950.rs");
        let body = source
            .split("fn rollover_queue(")
            .nth(1)
            .unwrap()
            .split("fn close_inner(")
            .next()
            .unwrap();
        let destroy = body.find("self.destroy_queue()?").unwrap();
        let release = body.find("self.release_resource(allocation)?").unwrap();
        let reset = body.find("self.ring =").unwrap();
        let initialize = body.find("self.initialize_queue()?").unwrap();
        let epoch = body.find("self.queue_epoch = next_epoch").unwrap();
        assert!(destroy < release && release < reset && reset < initialize && initialize < epoch);
        assert!(!body.contains("buffers.pop"));
        assert!(!body.contains("kernels.pop"));
        assert!(!body.contains("last_observed_read = self.completed_write"));
    }

    #[test]
    fn performance_configuration_cannot_change_a_live_or_previously_used_session() {
        assert!(require_fresh_configuration(false, 1, 1, 0).is_ok());
        for (configured, buffer, kernel, write) in [
            (true, 1, 1, 0),
            (false, 2, 1, 0),
            (false, 1, 2, 0),
            (false, 1, 1, 1),
            (false, 0, 1, 0),
            (false, 1, 0, 0),
        ] {
            assert!(require_fresh_configuration(configured, buffer, kernel, write).is_err());
        }
    }

    #[test]
    fn profile_counters_fail_closed_and_disabled_timers_do_not_change_values() {
        let mut value = 7;
        record_elapsed(&mut value, None).unwrap();
        assert_eq!(value, 7);
        add_counter(&mut value, 3).unwrap();
        assert_eq!(value, 10);
        assert!(add_counter(&mut value, u64::MAX).is_err());
        assert_eq!(value, 10);
    }

    #[test]
    fn operational_fence_preserves_live_checks_and_full_public_api() {
        let source = include_str!("device_gfx950.rs");
        let body = source
            .split("pub(crate) fn check_engineering_operational_currentness")
            .nth(1)
            .unwrap()
            .split("fn check_currentness_inner")
            .next()
            .unwrap();
        for check in [
            "ensure_process",
            "observe_process_incarnation",
            "reset_fence.check_clear",
            "revalidate_descriptor",
            "revalidate_render_descriptor",
            "observe_uapi",
            "query_xnack_mode",
            "observe_drm_identity",
            "currentness_poisoned = true",
        ] {
            assert!(body.contains(check), "missing {check}");
        }
        assert!(!body.contains("discover_default_topology_for_target"));
        assert!(!body.contains("observe_process_apertures"));
        let public = source
            .split("pub fn check_observable_currentness")
            .nth(1)
            .unwrap()
            .split("pub(crate) fn check_engineering_operational_currentness")
            .next()
            .unwrap();
        assert!(public.contains("self.check_currentness_inner()"));
    }

    #[test]
    fn completion_requires_exact_write_read_order_signal_and_no_exception() {
        use AqlCompletionObservationV1::{Completed, Pending, Unexpected};
        assert!(!dispatch_completed(1, 0, (1, 0), Pending, 0).unwrap());
        assert!(dispatch_completed(1, 0, (1, 0), Completed, 0).unwrap());
        assert!(!dispatch_completed(1, 0, (1, 1), Pending, 0).unwrap());
        assert!(dispatch_completed(1, 0, (1, 1), Completed, 0).unwrap());
        assert!(dispatch_completed(9, 0, (9, 0), Completed, 0).unwrap());
        assert!(!dispatch_completed(9, 8, (9, 8), Pending, 0).unwrap());
        for (write, read) in [(0, 1), (2, 1), (1, 2)] {
            assert!(dispatch_completed(1, 0, (write, read), Completed, 0).is_err());
        }
        assert!(dispatch_completed(9, 8, (9, 6), Completed, 0).is_err());
        assert!(dispatch_completed(0, 0, (0, 0), Completed, 0).is_err());
        assert!(dispatch_completed(1, 0, (1, 1), Completed, 1).is_err());
        assert!(dispatch_completed(1, 0, (1, 1), Unexpected(-1), 0).is_err());
        assert!(require_completed_frontier(0, 1).is_err());
        assert!(require_completed_frontier(2, 1).is_err());
        require_completed_frontier(1, 1).unwrap();
    }

    #[test]
    fn completed_signal_never_fabricates_ring_capacity_or_allows_overwrite() {
        let capacity = AqlRingCapacityV1::from_ring_bytes(RING_BYTES as u32).unwrap();
        let full = MAX_UNRETIRED_RING_PACKETS_V1;
        let mut ring = AqlSingleProducerRingModelV1::new(capacity, full, 0).unwrap();
        assert!(
            dispatch_completed(full, 0, (full, 0), AqlCompletionObservationV1::Completed, 0)
                .unwrap()
        );
        require_completed_frontier(full, full).unwrap();
        assert!(ring.reserve_one(0).is_err());
        assert_eq!(ring.write(), full);
        assert_eq!(ring.last_read(), 0);
        assert!(validate_counters(full + 1, 0, (full + 1, 0)).is_err());
        let next = ring.reserve_one(1).unwrap();
        assert_eq!(next.packet_id(), full);
        assert_eq!(next.slot_index(), 0);
        assert!(ring.reserve_one(0).is_err());
    }

    #[test]
    fn userptr_output_offset_is_opaque_but_bo_mapping_identity_is_exact() {
        let expected = KfdIoctlAllocMemoryOfGpuArgs::new(
            0x1000,
            4096,
            7,
            KfdAllocMemoryFlags::USERPTR_EXECUTABLE,
        );
        let handles = BTreeSet::from([1]);
        let offsets = BTreeSet::from([0x4000]);
        let mut args = expected;
        args.handle = 2;
        for offset in [0, 1, 0x1000, 0x4000, u64::MAX] {
            args.mmap_offset = offset;
            assert_eq!(
                admit_allocation_output(&args, &expected, true, &handles, &offsets).unwrap(),
                None
            );
        }
        let expected =
            KfdIoctlAllocMemoryOfGpuArgs::new(0x1000, 4096, 7, KfdAllocMemoryFlags::KERNARG);
        args = expected;
        args.handle = 2;
        args.mmap_offset = 0x8000;
        assert_eq!(
            admit_allocation_output(&args, &expected, false, &handles, &offsets).unwrap(),
            Some(0x8000)
        );
        for mutation in 0..9 {
            let mut changed = args;
            match mutation {
                0 => changed.va_addr += 4096,
                1 => changed.size += 4096,
                2 => changed.flags ^= 1,
                3 => changed.gpu_id += 1,
                4 => changed.handle = 0,
                5 => changed.handle = 1,
                6 => changed.mmap_offset = 0,
                7 => changed.mmap_offset = 1,
                8 => changed.mmap_offset = 0x4000,
                _ => unreachable!(),
            }
            assert!(
                admit_allocation_output(&changed, &expected, false, &handles, &offsets).is_err(),
                "mutation {mutation}"
            );
        }
    }

    fn metadata() -> KernelMetadataV1 {
        KernelMetadataV1 {
            symbol: "engineering_test".into(),
            object_sha256: [0; 32],
            kernarg_bytes: 16,
            kernarg_alignment: 8,
            group_segment_bytes: 0,
            private_segment_bytes: 0,
            wavefront_size: 64,
            implicit_argument_offset: None,
            implicit_argument_bytes: 0,
            explicit_arguments: (0..2)
                .map(|index| ExplicitArgumentV1 {
                    offset: index * 8,
                    bytes: 8,
                    global_buffer: true,
                    pointee_alignment: Some(2),
                    access: Some(BufferAccessV1::Read),
                })
                .collect(),
        }
    }

    fn pointers() -> [PointerFixupV1; 2] {
        [
            PointerFixupV1 {
                kernarg_offset: 0,
                buffer: 1,
                buffer_offset: 0,
                extent_bytes: 8,
                access: BufferAccessV1::Read,
            },
            PointerFixupV1 {
                kernarg_offset: 8,
                buffer: 2,
                buffer_offset: 0,
                extent_bytes: 8,
                access: BufferAccessV1::Read,
            },
        ]
    }

    fn lookup(id: u64) -> Option<(u64, u64)> {
        match id {
            1 => Some((0x1000, 16)),
            2 => Some((0x2000, 16)),
            _ => None,
        }
    }

    #[test]
    fn pointer_binding_is_owned_exact_and_fail_atomic() {
        let metadata = metadata();
        let mut bytes = [0; 16];
        patch_pointer_arguments(&metadata, &mut bytes, &pointers(), lookup).unwrap();
        assert_eq!(u64::from_le_bytes(bytes[..8].try_into().unwrap()), 0x1000);
        assert_eq!(u64::from_le_bytes(bytes[8..].try_into().unwrap()), 0x2000);
        for mutation in 0..8 {
            let mut pointers = pointers();
            let mut bytes = [0; 16];
            match mutation {
                0 => pointers[1].buffer = 99,
                1 => pointers[1].kernarg_offset = 0,
                2 => pointers[1].kernarg_offset = 4,
                3 => pointers[1].extent_bytes = 17,
                4 => pointers[1].buffer_offset = u64::MAX,
                5 => pointers[1].buffer_offset = 1,
                6 => pointers[1].access = BufferAccessV1::Write,
                7 => bytes[8] = 1,
                _ => unreachable!(),
            }
            let before = bytes;
            assert!(
                patch_pointer_arguments(&metadata, &mut bytes, &pointers, lookup).is_err(),
                "mutation {mutation}"
            );
            assert_eq!(bytes, before, "mutation {mutation} changed caller bytes");
        }
    }

    #[test]
    fn pointer_aliasing_allows_reads_and_exact_zero_extent_only() {
        let mut metadata = metadata();
        let mut pointers = pointers();
        pointers[1].buffer = 1;
        patch_pointer_arguments(&metadata, &mut [0; 16], &pointers, lookup).unwrap();
        pointers[1].access = BufferAccessV1::Write;
        metadata.explicit_arguments[1].access = Some(BufferAccessV1::Write);
        assert!(patch_pointer_arguments(&metadata, &mut [0; 16], &pointers, lookup).is_err());
        pointers[1].buffer_offset = 8;
        patch_pointer_arguments(&metadata, &mut [0; 16], &pointers, lookup).unwrap();
        pointers[1].buffer_offset = 16;
        pointers[1].extent_bytes = 0;
        patch_pointer_arguments(&metadata, &mut [0; 16], &pointers, lookup).unwrap();
        pointers[1].buffer_offset = 18;
        assert!(patch_pointer_arguments(&metadata, &mut [0; 16], &pointers, lookup).is_err());
    }

    #[test]
    fn malformed_pointer_metadata_and_address_are_rejected() {
        for alignment in [0, 3, 8192] {
            let mut metadata = metadata();
            metadata.explicit_arguments[1].pointee_alignment = Some(alignment);
            assert!(patch_pointer_arguments(&metadata, &mut [0; 16], &pointers(), lookup).is_err());
        }
        assert!(
            patch_pointer_arguments(&metadata(), &mut [0; 16], &pointers()[..1], lookup).is_err()
        );
        assert!(patch_pointer_arguments(&metadata(), &mut [0; 15], &pointers(), lookup).is_err());
        assert!(
            patch_pointer_arguments(&metadata(), &mut [0; 16], &pointers(), |_| Some((0, 16)))
                .is_err()
        );
        let mut pointers = pointers();
        pointers[0].buffer_offset = 8;
        assert!(
            patch_pointer_arguments(&metadata(), &mut [0; 16], &pointers, |_| Some((
                u64::MAX - 4,
                16
            )))
            .is_err()
        );
    }
}
