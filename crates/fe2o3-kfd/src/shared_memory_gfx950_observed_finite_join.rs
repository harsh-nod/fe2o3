//! One fixed kernel on the existing observed memory engine and queue lifecycle.

use super::*;
use fe2o3_amdhsa_loader::AdmittedProfile;
use fe2o3_aql::{
    AqlDispatchGeometryV1, AqlKernelDispatchPacketV1, AqlPreparedKernelDispatchV1,
    ObservedGpuAddressV1,
};
use sha2::{Digest, Sha256};

#[path = "shared_memory_gfx950_observed_finite_join_packet.rs"]
pub(super) mod packet;
use packet::{Progress, Snapshot};

const EXTENTS: [usize; 3] = [1024, 1536, 24];
const INITIAL: [u32; 6] = [1, 3, 0, 0, 0, 0];
const SYMBOL: &str = "finite_join_source_boundary";
const OBJECT_BYTES: usize = 9752;
const OBJECT_SHA: [u8; 32] = [
    0xa0, 0x68, 0x45, 0x6b, 0x3a, 0x48, 0x2c, 0xbc, 0xb2, 0xb0, 0x87, 0xac, 0xaa, 0x1c, 0xc9, 0x46,
    0xed, 0xb9, 0xf1, 0x85, 0x76, 0x4a, 0x88, 0x0e, 0x7f, 0x74, 0x38, 0xd2, 0xdf, 0x15, 0xd1, 0x99,
];
const KERNARG_BYTES: usize = 280;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Role {
    Input,
    Payload,
    State,
}
impl Role {
    const ALL: [Self; 3] = [Self::Input, Self::Payload, Self::State];
    fn index(self) -> usize {
        match self {
            Self::Input => 0,
            Self::Payload => 1,
            Self::State => 2,
        }
    }
}

/// A role-bound initialized root on the fixed-kernel owner. No address or
/// conversion to an ordinary unpublished-allocation token is exposed.
pub struct Gfx950ObservedFiniteJoinRootV1 {
    role: Role,
    token: Mapped<HostVisibleCoherentGttV1>,
}
impl Gfx950ObservedFiniteJoinRootV1 {
    /// Actual logical allocation extent, not the page-rounded backing size.
    pub fn requested_bytes(&self) -> usize {
        self.token.layout.requested_bytes
    }
    /// Alignment checked by the actual shared memory engine.
    pub const fn alignment(&self) -> u64 {
        PAGE as u64
    }
}

/// Consuming engineering owner for precisely the retained fixed FiniteJoin image.
/// Dropping a live owner retains its native custody until process exit.
pub struct Gfx950ObservedFiniteJoinOwnerV1 {
    owner: Option<Box<NativeOwner>>,
}

impl Drop for Gfx950ObservedFiniteJoinOwnerV1 {
    fn drop(&mut self) {
        if let Some(mut owner) = self.owner.take() {
            owner.quarantine();
            std::mem::forget(owner);
        }
    }
}

impl Gfx950ObservedFiniteJoinOwnerV1 {
    /// Consume a fresh observed session for this exact engineering workload.
    ///
    /// # Safety
    /// The caller must independently review the retained image/source/ISA,
    /// selected device, system-atomic coherence and lifetime premises. Use a
    /// bounded disposable process with no unrelated GPU work. This unsafe
    /// contract permits the later fixed consuming execution, not production
    /// admission or arbitrary code. Any error is terminal and requires exit.
    #[allow(unsafe_code)]
    pub unsafe fn new(
        session: Gfx950ObservedMemorySessionV1,
        object: Vec<u8>,
        timeout_ms: u32,
    ) -> Result<Self, Gfx950ObservedQueueFailureV1> {
        let mut owner = Box::new(NativeOwner::new(session));
        let result = (|| {
            validate_image(&object, timeout_ms)?;
            let engine = active_engine(&mut owner.memory)?;
            engine.check_currentness().map_err(describe)?;
            if !engine.allocations.is_empty() || !engine.device_memory.is_empty() {
                return Err("fixed kernel requires a fresh same-engine session".into());
            }
            owner.finite_join = Some(Workload::new(object, timeout_ms));
            Ok(())
        })();
        match result {
            Ok(()) => Ok(Self { owner: Some(owner) }),
            Err(message) => {
                owner.quarantine();
                Err(retained(owner, message))
            }
        }
    }

    /// Initialize the first, immutable F32[256] root from exact IEEE bits.
    pub fn initialize_input(
        &mut self,
        bits: &[u32; 256],
    ) -> Result<Gfx950ObservedFiniteJoinRootV1, MemorySessionError> {
        self.initialize(Role::Input, Some(bits))
    }
    /// Initialize the second F32[384] root before any kernel publication.
    pub fn initialize_payload(
        &mut self,
        bits: &[u32; 384],
    ) -> Result<Gfx950ObservedFiniteJoinRootV1, MemorySessionError> {
        self.initialize(Role::Payload, Some(bits))
    }
    /// Construct six genuine AtomicU32 values, then acquire-check fresh state.
    pub fn initialize_state(
        &mut self,
    ) -> Result<Gfx950ObservedFiniteJoinRootV1, MemorySessionError> {
        self.initialize(Role::State, None)
    }
    fn initialize(
        &mut self,
        role: Role,
        words: Option<&[u32]>,
    ) -> Result<Gfx950ObservedFiniteJoinRootV1, MemorySessionError> {
        let owner = self
            .owner
            .as_mut()
            .ok_or(MemorySessionError::SharedSessionQuarantined)?;
        let result = (|| {
            let workload = owner
                .finite_join
                .as_mut()
                .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
            if workload.initialized != role.index() || workload.roots.is_some() {
                return Err(MemorySessionError::InvalidAllocationAuthority);
            }
            let engine = active_engine(&mut owner.memory)
                .map_err(|_| MemorySessionError::SharedSessionQuarantined)?;
            let token = initialize_root(engine, role, words, |mapping| {
                LinuxGfx950MemoryBackend::initialize_engineering_finite_join_state(mapping)?;
                if LinuxGfx950MemoryBackend::observe_engineering_finite_join_state(mapping)?
                    != INITIAL
                {
                    return Err(MemorySessionError::KernelResultMalformed(
                        "fresh finite join atomics",
                    ));
                }
                Ok(())
            })?;
            workload.identities[role.index()] = Some(token.storage_identity());
            workload.initialized += 1;
            if role == Role::Input {
                workload.input = words.unwrap().to_vec();
            }
            Ok(Gfx950ObservedFiniteJoinRootV1 { role, token })
        })();
        if result.is_err() {
            owner.quarantine();
        }
        result
    }

    /// Permanently retain uncertain native resources. No cleanup or retry.
    pub fn quarantine(&mut self) {
        if let Some(owner) = self.owner.as_mut() {
            owner.quarantine();
        }
    }

    /// Consume the exact ordered same-owner roots and publish at most one
    /// fixed packet. Results exist only after full native release and close.
    pub fn execute(
        mut self,
        roots: [Gfx950ObservedFiniteJoinRootV1; 3],
    ) -> Result<Gfx950ObservedFiniteJoinV1, Gfx950ObservedQueueFailureV1> {
        // Retain custody even if a host panic interrupts the consuming path.
        let mut owner = ManuallyDrop::new(self.owner.take().expect("linear fixed owner"));
        let result = (|| {
            owner
                .finite_join
                .as_mut()
                .ok_or("missing fixed workload")?
                .roots = Some(roots);
            owner.prepare_finite_join()?;
            run_lifecycle(owner.as_mut())?;
            owner.finish_finite_join()
        })();
        match result {
            Ok(observation) => {
                drop(ManuallyDrop::into_inner(owner));
                Ok(observation)
            }
            Err(message) => {
                owner.quarantine();
                Err(retained(ManuallyDrop::into_inner(owner), message))
            }
        }
    }
}

fn retained(owner: Box<NativeOwner>, message: String) -> Gfx950ObservedQueueFailureV1 {
    Gfx950ObservedQueueFailureV1 {
        message,
        retained: ManuallyDrop::new(owner),
    }
}

/// Closed same-engine kernel observations. No addresses, authority or reusable
/// allocation tokens escape. Timings are host observations, not a benchmark.
#[derive(Debug)]
pub struct Gfx950ObservedFiniteJoinV1 {
    /// Actual selected device identity.
    pub unique_id: u64,
    /// Queue ID retained only after confirmed destruction.
    pub destroyed_queue_id: u32,
    /// Input read back in full after destruction and checked against upload.
    pub input_bits: [u32; 256],
    /// Actual completed payload bits, staged until native close.
    pub payload_bits: [u32; 384],
    /// Six acquired, validated terminal state words.
    pub final_state: [u32; 6],
    /// Write/read indices at acquired completion.
    pub completion_counters: [u64; 2],
    /// Fresh write/read indices after confirmed destruction.
    pub destroyed_counters: [u64; 2],
    /// Host time around packet publication through acquired completion.
    pub dispatch_host_ns: u64,
}

pub(super) struct Workload {
    object: Vec<u8>,
    timeout_ms: u32,
    initialized: usize,
    identities: [Option<SharedGttAllocationIdentityV1>; 3],
    input: Vec<u32>,
    roots: Option<[Gfx950ObservedFiniteJoinRootV1; 3]>,
    code: Option<Executable>,
    kernarg: Option<Mapped<KernargGttV1>>,
    descriptor_offset: u64,
    signal_cpu: Option<Cpu<HostVisibleCoherentGttV1>>,
    signal: Option<Mapped<HostVisibleCoherentGttV1>>,
    progress: Progress,
    dispatch_host_ns: u64,
}
impl Workload {
    fn new(object: Vec<u8>, timeout_ms: u32) -> Self {
        Self {
            object,
            timeout_ms,
            initialized: 0,
            identities: [None; 3],
            input: Vec::new(),
            roots: None,
            code: None,
            kernarg: None,
            descriptor_offset: 0,
            signal_cpu: None,
            signal: None,
            progress: Progress::default(),
            dispatch_host_ns: 0,
        }
    }
    pub(super) fn observe_counters(&mut self, counters: (u64, u64)) -> QueueResult<()> {
        self.progress.counters(counters)
    }
    pub(super) fn confirm_destroyed(&mut self) -> QueueResult<()> {
        self.progress.destroy()
    }
    pub(super) fn released(&self) -> bool {
        self.progress.released && self.signal_cpu.is_none() && self.signal.is_none()
    }
}

fn geometry() -> QueueResult<AqlDispatchGeometryV1> {
    AqlDispatchGeometryV1::new([256, 1, 1], [128, 1, 1]).map_err(describe)
}
fn validate_image(object: &[u8], timeout_ms: u32) -> QueueResult<()> {
    if object.len() != OBJECT_BYTES
        || <[u8; 32]>::from(Sha256::digest(object)) != OBJECT_SHA
        || !(1..=10_000).contains(&timeout_ms)
    {
        return Err("fixed retained image or 1..10000ms deadline".into());
    }
    let closure = fe2o3_amdhsa_loader::validate(object, AdmittedProfile::Gfx950XnackOffCov6)
        .map_err(describe)?
        .bind_kernel(SYMBOL)
        .map_err(describe)?;
    let r = closure.resources();
    let k = closure.selected_kernel();
    if r.kernarg_segment_size() != 280
        || r.kernarg_segment_alignment() != 8
        || r.wavefront_size() != 64
        || r.group_segment_fixed_size() != 0
        || r.private_segment_fixed_size() != 0
        || r.cluster_dims().is_some()
        || r.max_flat_workgroup_size() < 128
        || r.required_workgroup_size()
            .is_some_and(|v| v != [128, 1, 1])
        || r.max_workgroups()
            .into_iter()
            .zip([2, 1, 1])
            .any(|(max, n)| max.is_some_and(|m| n > m))
        || !k.arguments_were_emitted()
        || k.explicit_arguments().len() != 3
        || k.implicit_argument_offset() != Some(24)
        || k.implicit_argument_size() != 256
    {
        return Err("fixed image ABI, resources or geometry".into());
    }
    for (i, a) in k.explicit_arguments().iter().enumerate() {
        if a.offset() != i as u64 * 8
            || a.size() != 8
            || a.value_kind() != fe2o3_hsaco::ExplicitValueKind::GlobalBuffer
            || a.pointee_alignment().is_some_and(|n| n != 4)
            || a.access().is_some_and(|access| {
                access
                    != if i == 0 {
                        fe2o3_hsaco::ArgumentAccess::ReadOnly
                    } else {
                        fe2o3_hsaco::ArgumentAccess::ReadWrite
                    }
            })
        {
            return Err("fixed image explicit pointer ABI".into());
        }
    }
    let mut bytes = [0; KERNARG_BYTES];
    crate::queue::dispatch_binding::initialize_engineering_cov6_kernarg(k, geometry()?, &mut bytes)
        .map_err(describe)?;
    Ok(())
}

fn initialize_root<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    role: Role,
    words: Option<&[u32]>,
    atomic_init: impl FnOnce(&mut B::Mapping) -> Result<(), MemorySessionError>,
) -> Result<Mapped<HostVisibleCoherentGttV1>, MemorySessionError> {
    engine.check_currentness()?;
    if (role == Role::State && words.is_some())
        || (role != Role::State && words.is_none_or(|w| w.len() * 4 != EXTENTS[role.index()]))
    {
        return Err(MemorySessionError::InvalidAllocationAuthority);
    }
    let mut token = engine.allocate::<HostVisibleCoherentGttV1>(EXTENTS[role.index()])?;
    if role == Role::State {
        let index = engine.index(&token, SharedAllocationPhaseV1::CpuWritable)?;
        let record = &mut engine.allocations[index];
        if record.profile != SharedGttProfileV1::HostVisibleCoherent
            || record.layout.requested_bytes != 24
        {
            return Err(MemorySessionError::InvalidAllocationAuthority);
        }
        atomic_init(
            record
                .mapping
                .as_mut()
                .ok_or(MemorySessionError::InvalidAllocationAuthority)?,
        )?;
        engine.check_currentness()?;
    } else {
        engine.with_bytes_mut(&mut token, |bytes| {
            for (dst, word) in bytes.chunks_exact_mut(4).zip(words.unwrap()) {
                dst.copy_from_slice(&word.to_le_bytes());
            }
        })?;
    }
    engine.map_mutable(token)
}

fn root_addresses<B: MemoryBackend>(
    engine: &SharedMemoryEngine<B>,
    roots: &[Gfx950ObservedFiniteJoinRootV1; 3],
    identities: &[Option<SharedGttAllocationIdentityV1>; 3],
) -> QueueResult<[u64; 3]> {
    let mut addresses = [0; 3];
    for (i, root) in roots.iter().enumerate() {
        if root.role != Role::ALL[i]
            || root.token.layout.requested_bytes != EXTENTS[i]
            || identities[i] != Some(root.token.storage_identity())
        {
            return Err("fixed root role, extent or retained identity".into());
        }
        let index = engine
            .index(&root.token, SharedAllocationPhaseV1::GpuAccessibleMutable)
            .map_err(describe)?;
        let record = &engine.allocations[index];
        if record.profile != SharedGttProfileV1::HostVisibleCoherent
            || record.layout.cpu_mapping_bytes != PAGE
            || record.layout.gpu_va_bytes != PAGE as u64
            || record.gpu_va == 0
            || !record.gpu_va.is_multiple_of(PAGE as u64)
        {
            return Err("fixed root backing/alignment".into());
        }
        addresses[i] = record.gpu_va;
        let end = addresses[i]
            .checked_add(EXTENTS[i] as u64)
            .ok_or("root extent overflow")?;
        for j in 0..i {
            let other = addresses[j]
                .checked_add(EXTENTS[j] as u64)
                .ok_or("root extent overflow")?;
            if addresses[i] < other && addresses[j] < end {
                return Err("fixed root alias".into());
            }
        }
    }
    Ok(addresses)
}

fn terminal(state: [u32; 6]) -> QueueResult<()> {
    if state[0] != 1
        || state[1] != 0
        || state[2] != 7
        || state[3] != 7
        || state[4] >> 6 != 0
        || state[5] != 0
        || (0..3).any(|i| !matches!((state[4] >> (i * 2)) & 3, 1 | 2))
    {
        return Err("finite join terminal state".into());
    }
    Ok(())
}

fn materialize_code<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    len: usize,
    fill: impl FnOnce(&mut [u8]) -> QueueResult<()>,
) -> QueueResult<Executable> {
    let mut code = engine.allocate::<ExecutableGttV1>(len).map_err(describe)?;
    engine.with_bytes_mut(&mut code, fill).map_err(describe)??;
    let code = engine.seal_executable(code).map_err(describe)?;
    engine.map_executable(code).map_err(describe)
}
fn initialize_kernarg<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    bytes: &[u8; KERNARG_BYTES],
) -> QueueResult<Mapped<KernargGttV1>> {
    let mut kernarg = engine
        .allocate::<KernargGttV1>(KERNARG_BYTES)
        .map_err(describe)?;
    engine
        .with_bytes_mut(&mut kernarg, |out| out.copy_from_slice(bytes))
        .map_err(describe)?;
    engine.map_mutable(kernarg).map_err(describe)
}
fn release_fixed_resources<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    code: Executable,
    kernarg: Mapped<KernargGttV1>,
    roots: [Gfx950ObservedFiniteJoinRootV1; 3],
) -> QueueResult<()> {
    let kernarg = engine.unmap_mutable(kernarg).map_err(describe)?;
    engine
        .release(kernarg, SharedAllocationPhaseV1::CpuWritable)
        .map_err(describe)?;
    let code = engine.unmap_executable(code).map_err(describe)?;
    engine
        .release(code, SharedAllocationPhaseV1::ExecutableImmutable)
        .map_err(describe)?;
    for root in roots.into_iter().rev() {
        let cpu = engine.unmap_mutable(root.token).map_err(describe)?;
        engine
            .release(cpu, SharedAllocationPhaseV1::CpuWritable)
            .map_err(describe)?;
    }
    Ok(())
}

impl NativeOwner {
    fn prepare_finite_join(&mut self) -> QueueResult<()> {
        let w = self.finite_join.as_mut().ok_or("missing fixed workload")?;
        if w.initialized != 3 || w.code.is_some() || w.kernarg.is_some() {
            return Err("fixed activation phase".into());
        }
        let engine = active_engine(&mut self.memory)?;
        engine.check_currentness().map_err(describe)?;
        let addresses = root_addresses(
            engine,
            w.roots.as_ref().ok_or("missing fixed roots")?,
            &w.identities,
        )?;
        let closure = fe2o3_amdhsa_loader::validate(&w.object, AdmittedProfile::Gfx950XnackOffCov6)
            .map_err(describe)?
            .bind_kernel(SYMBOL)
            .map_err(describe)?;
        let offset = closure
            .selected_binding()
            .descriptor_address()
            .checked_sub(closure.envelope().plan().image_start())
            .ok_or("descriptor offset")?;
        let len =
            usize::try_from(closure.envelope().materialization().image_len()).map_err(describe)?;
        if !offset.is_multiple_of(64) || offset.checked_add(64).is_none_or(|end| end > len as u64) {
            return Err("descriptor range/alignment".into());
        }
        w.code = Some(materialize_code(engine, len, |bytes| {
            closure.materialize_into(bytes).map_err(describe)
        })?);
        w.descriptor_offset = offset;
        let mut bytes = [0; KERNARG_BYTES];
        for (i, address) in addresses.into_iter().enumerate() {
            bytes[i * 8..i * 8 + 8].copy_from_slice(&address.to_le_bytes());
        }
        crate::queue::dispatch_binding::initialize_engineering_cov6_kernarg(
            closure.selected_kernel(),
            geometry()?,
            &mut bytes,
        )
        .map_err(describe)?;
        w.kernarg = Some(initialize_kernarg(engine, &bytes)?);
        Ok(())
    }
    pub(super) fn allocate_finite_join_signal(&mut self) -> QueueResult<()> {
        let w = self.finite_join.as_mut().ok_or("missing fixed workload")?;
        if w.signal_cpu.is_some() || w.signal.is_some() {
            return Err("signal initialization repeated".into());
        }
        let engine = active_engine(&mut self.memory)?;
        let token = engine
            .allocate::<HostVisibleCoherentGttV1>(PAGE)
            .map_err(describe)?;
        let i = engine
            .index(&token, SharedAllocationPhaseV1::CpuWritable)
            .map_err(describe)?;
        LinuxGfx950MemoryBackend::initialize_engineering_signal_slots(
            engine.allocations[i]
                .mapping
                .as_mut()
                .ok_or("signal mapping")?,
            1,
        )
        .map_err(describe)?;
        engine.check_currentness().map_err(describe)?;
        w.signal_cpu = Some(token);
        Ok(())
    }
    pub(super) fn map_finite_join_signal(&mut self) -> QueueResult<()> {
        let w = self.finite_join.as_mut().ok_or("missing fixed workload")?;
        w.signal = Some(
            active_engine(&mut self.memory)?
                .map_mutable(w.signal_cpu.take().ok_or("missing initialized signal")?)
                .map_err(describe)?,
        );
        Ok(())
    }
    fn finite_join_packet(&mut self) -> QueueResult<AqlPreparedKernelDispatchV1> {
        let w = self.finite_join.as_ref().ok_or("missing fixed workload")?;
        let engine = active_engine(&mut self.memory)?;
        root_addresses(
            engine,
            w.roots.as_ref().ok_or("missing roots")?,
            &w.identities,
        )?;
        let code = resource_va(
            engine,
            w.code.as_ref().ok_or("missing executable")?,
            SharedAllocationPhaseV1::GpuAccessibleExecutable,
        )?
        .checked_add(w.descriptor_offset)
        .ok_or("descriptor overflow")?;
        let kernarg = resource_va(
            engine,
            w.kernarg.as_ref().ok_or("missing kernarg")?,
            SharedAllocationPhaseV1::GpuAccessibleMutable,
        )?;
        let signal = resource_va(
            engine,
            w.signal.as_ref().ok_or("missing signal")?,
            SharedAllocationPhaseV1::GpuAccessibleMutable,
        )?;
        AqlKernelDispatchPacketV1::new_unpublished(
            geometry()?,
            0,
            0,
            ObservedGpuAddressV1::new(code).map_err(describe)?,
            ObservedGpuAddressV1::new(kernarg).map_err(describe)?,
            8,
            ObservedGpuAddressV1::new(signal).map_err(describe)?,
        )
        .map_err(describe)
    }
    fn finite_join_snapshot(&mut self) -> QueueResult<Snapshot> {
        self.currentness()?;
        let engine = active_engine(&mut self.memory)?;
        let w = self.finite_join.as_mut().ok_or("missing fixed workload")?;
        let (kind, value) = engine
            .observe_completion_signal_state(w.signal.as_mut().ok_or("missing signal")?, 0)
            .map_err(describe)?;
        let mapped = self.mapped.as_mut().ok_or("missing queue")?;
        let (write, read) = engine
            .observe_aql_counters_in_current_scope(&mut mapped.control)
            .map_err(describe)?;
        let (slot, header, setup) = engine
            .observe_aql_packet_header(&mut mapped.ring, 0)
            .map_err(describe)?;
        self.currentness()?;
        Ok(Snapshot {
            write,
            read,
            slot,
            header,
            setup,
            kind,
            value,
        })
    }
    pub(super) fn check_completed_finite_join(&mut self) -> QueueResult<()> {
        let snapshot = self.finite_join_snapshot()?;
        self.finite_join
            .as_mut()
            .ok_or("missing fixed workload")?
            .progress
            .observe_completed(snapshot)
    }
    pub(super) fn release_finite_join_signal(&mut self) -> QueueResult<()> {
        self.check_completed_finite_join()?;
        let w = self.finite_join.as_mut().ok_or("missing fixed workload")?;
        if !w.progress.destroyed || w.progress.after_destroy.is_none() || w.progress.released {
            return Err("signal release before confirmed destruction".into());
        }
        let engine = active_engine(&mut self.memory)?;
        let cpu = engine
            .unmap_mutable(w.signal.take().ok_or("missing signal")?)
            .map_err(describe)?;
        engine
            .release(cpu, SharedAllocationPhaseV1::CpuWritable)
            .map_err(describe)?;
        w.progress.released = true;
        Ok(())
    }
    fn finish_finite_join(&mut self) -> QueueResult<Gfx950ObservedFiniteJoinV1> {
        let w = self.finite_join.as_mut().ok_or("missing workload")?;
        if !w.released() {
            return Err("fixed readback before complete queue teardown".into());
        }
        let engine = active_engine(&mut self.memory)?;
        let roots = w.roots.as_ref().ok_or("missing roots")?;
        root_addresses(engine, roots, &w.identities)?;
        let input = decode::<256>(
            &engine
                .copy_mapped_host_visible_subrange(&roots[0].token, 0, 1024)
                .map_err(describe)?,
        )?;
        if input.as_slice() != w.input.as_slice() {
            return Err("immutable device input changed".into());
        }
        let payload = decode::<384>(
            &engine
                .copy_mapped_host_visible_subrange(&roots[1].token, 0, 1536)
                .map_err(describe)?,
        )?;
        engine.check_currentness().map_err(describe)?;
        let i = engine
            .index(
                &roots[2].token,
                SharedAllocationPhaseV1::GpuAccessibleMutable,
            )
            .map_err(describe)?;
        let state = LinuxGfx950MemoryBackend::observe_engineering_finite_join_state(
            engine.allocations[i]
                .mapping
                .as_mut()
                .ok_or("state mapping")?,
        )
        .map_err(describe)?;
        engine.check_currentness().map_err(describe)?;
        terminal(state)?;
        let completed = w.progress.completed.ok_or("missing completion")?;
        let destroyed = w
            .progress
            .after_destroy
            .ok_or("missing destruction observation")?;
        let dispatch_host_ns = w.dispatch_host_ns;
        release_fixed_resources(
            engine,
            w.code.take().ok_or("missing code")?,
            w.kernarg.take().ok_or("missing kernarg")?,
            w.roots.take().ok_or("missing roots")?,
        )?;
        let observation = self
            .observation
            .as_ref()
            .ok_or("missing confirmed queue close")?;
        let result = Gfx950ObservedFiniteJoinV1 {
            unique_id: observation.unique_id,
            destroyed_queue_id: observation.queue_id,
            input_bits: input,
            payload_bits: payload,
            final_state: state,
            completion_counters: [completed.write, completed.read],
            destroyed_counters: [destroyed.write, destroyed.read],
            dispatch_host_ns,
        };
        self.memory
            .as_mut()
            .ok_or("missing session")?
            .close()
            .map_err(describe)?;
        Ok(result)
    }
}

fn decode<const N: usize>(bytes: &[u8]) -> QueueResult<[u32; N]> {
    if bytes.len() != N * 4 {
        return Err("fixed readback extent".into());
    }
    Ok(core::array::from_fn(|i| {
        u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().expect("exact word"))
    }))
}

#[cfg(test)]
pub(in crate::shared_memory::gfx950_observed) fn exercise_resources<B: MemoryBackend>(
    core: &mut ObservedMemoryCore<B>,
    mutation: usize,
    atomic_init: impl FnOnce(&mut B::Mapping) -> Result<(), MemorySessionError>,
    mut hook: impl FnMut(&'static str, &mut SharedMemoryEngine<B>),
) -> QueueResult<()> {
    let result = (|| {
        let engine = core.engine.as_mut().ok_or("missing engine")?;
        hook("input", engine);
        let input = initialize_root(engine, Role::Input, Some(&[17; 256]), |_| unreachable!())
            .map_err(describe)?;
        hook("payload", engine);
        let payload = initialize_root(engine, Role::Payload, Some(&[33; 384]), |_| unreachable!())
            .map_err(describe)?;
        hook("state", engine);
        let state = initialize_root(engine, Role::State, None, atomic_init).map_err(describe)?;
        let mut roots = [
            Gfx950ObservedFiniteJoinRootV1 {
                role: Role::Input,
                token: input,
            },
            Gfx950ObservedFiniteJoinRootV1 {
                role: Role::Payload,
                token: payload,
            },
            Gfx950ObservedFiniteJoinRootV1 {
                role: Role::State,
                token: state,
            },
        ];
        let ids = roots.each_ref().map(|r| Some(r.token.storage_identity()));
        match mutation {
            0 => {}
            1 => roots.swap(0, 1),
            2 => roots[0].token.session_id += 1,
            3 => roots[2].token.generation += 1,
            4 => engine.allocations[0].gpu_va += 2,
            5 => engine.allocations[1].gpu_va = engine.allocations[0].gpu_va,
            6 => roots[2].token.layout.requested_bytes = 28,
            _ => return Err("unknown root mutation".into()),
        }
        root_addresses(engine, &roots, &ids)?;
        hook("code", engine);
        let code = materialize_code(engine, PAGE, |bytes| {
            bytes.fill(0);
            Ok(())
        })?;
        hook("kernarg", engine);
        let kernarg = initialize_kernarg(engine, &[0; KERNARG_BYTES])?;
        hook("read", engine);
        let actual = decode::<256>(
            &engine
                .copy_mapped_host_visible_subrange(&roots[0].token, 0, 1024)
                .map_err(describe)?,
        )?;
        if actual != [17; 256] {
            return Err("immutable input changed".into());
        }
        hook("release", engine);
        release_fixed_resources(engine, code, kernarg, roots)?;
        Ok(())
    })();
    if result.is_err() {
        core.quarantine();
    }
    result
}

#[cfg(test)]
#[path = "shared_memory_gfx950_observed_finite_join_tests.rs"]
mod tests;
