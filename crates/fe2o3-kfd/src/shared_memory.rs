//! Bounded shared KFD VM authority for typed host-visible GTT allocations.

use core::fmt;
use core::marker::PhantomData;
use std::os::fd::BorrowedFd;
use std::sync::atomic::Ordering;

use fe2o3_kfd_uapi::{
    KFD_ALLOC_MEMORY_FLAGS_AQL_QUEUE, KFD_ALLOC_MEMORY_FLAGS_EXECUTABLE,
    KFD_ALLOC_MEMORY_FLAGS_HOST_VISIBLE_COHERENT, KFD_ALLOC_MEMORY_FLAGS_KERNARG,
    KfdAllocMemoryFlags,
};
use fe2o3_runtime_model::{
    AllocationGenerationV1, AllocationIdV1, DeviceIdentityStateV1, GpuVaRangeV1, MappingIdV1,
    MemoryAccessV1, MemoryAllocationKeyV1, MemoryAllocationSpecV1, MemoryCoherenceV1, MemoryKindV1,
    MemoryLifecycleStateV1, MemoryMappingKeyV1, MemoryPublicationIdV1, MemoryPublicationKeyV1,
    MemoryTransitionErrorV1, MemoryTransitionV1, ModelAdmissionStatusV1, ModelDeviceAdmissionV1,
    PartialOperationStatusV1, PartialProgressObservationV1, UntrustedAllocationHandleObservationV1,
    UntrustedVmHandleObservationV1, VaReservationIdV1, VaReservationKeyV1, VmIdV1, VmKeyV1,
};

use super::memory::{
    HOST_VISIBLE_MEMORY_PAGE_BYTES_V1, MemoryBackend, MemoryModelJournalSummary,
    MemorySessionError, NEXT_MODEL_VM_ID, begin_process_vm_attempt, finish_process_vm_attempt,
};
use crate::CheckedGfx942XnackMinusDevice;

pub const MAX_SHARED_GTT_ALLOCATIONS_V1: usize = 64;
pub const MAX_SHARED_GTT_SINGLE_CPU_BYTES_V1: u64 = 1 << 31;
pub const MAX_SHARED_GTT_GPU_VA_BYTES_V1: u64 = 8 << 30;
pub const MIN_AQL_QUEUE_BYTES_V1: u64 = 4_096;
pub const MAX_AQL_QUEUE_BYTES_V1: u64 = 1 << 31;

/// Canonical contract for the bounded multi-allocation R2 adapter.
pub const SHARED_GTT_MEMORY_PROFILE_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-shared-gtt-memory-r2-v1\n",
    "base_memory_profile_sha256=7bdca672c4921ee56a850d41040045f4a8fbe5a20176628a4ea982dd80fbe8ec\n",
    "kfd_memory_schema_sha256=e2d6987b7c8e61a405b2f775d5d004f458a096241459e4cfdf90bd4497f4d58a\n",
    "profiles=host-visible-coherent:0x84000002,kernarg:0x86000002,aql-queue:0x8e000002,executable:0xc4000002\n",
    "bounds=allocations:64,single-cpu-bytes:2147483648,total-gpu-va-bytes:8589934592,page:4096\n",
    "aql=logical-ring:power-of-two-4096..2147483648,gpu-va:checked-double,cpu-vma:single-physical-copy\n",
    "va_allocator=kernel-selected-prot-none-guards-retained-until-successful-free,checked-nonoverlap\n",
    "authority=one-retained-kfd-render-vm,multiple-linear-redacted-tokens,no-fd-handle-va-or-pointer-export\n",
    "queue-bridge=crate-private-role-marked-linear-mapped-capabilities,private-va-mapping-publication-facts,no-public-mint\n",
    "queue-gtt-policy=ring:aql-special,control:host-visible-coherent,eop-and-cwsr:executable,not-rocr-equivalence\n",
    "cpu_views=closure-scoped,session-exclusive,no-safe-borrow-escape,no-view-while-gpu-mapped\n",
    "executable=cpu-construction-rw-to-vma-read-only-before-gpu-map,gpu-writable-flag-remains-contracted\n",
    "failure=global-quarantine-after-started-or-ambiguous-native-transaction,no-drop-cleanup-or-retry\n",
    "fork=current-base-contract,prot-none-dontfork-before-rw,no-raw-fork-clone-during-setup\n",
    "model=completion-only-append-journal,profile-kind-and-gpu-va-span,no-cpu-vma-or-seal-transition\n",
    "proof=no-concrete-verus-refinement,kernel-and-model-coupling-contracted,hostile-tests-only\n",
    "excluded=queue-ioctl,doorbell,packet-publication,dispatch,completion,userptr,vram,peer-map\n",
);

pub const SHARED_GTT_MEMORY_PROFILE_SHA256_V1: &str =
    "1054b1c31ad143c7218eee24bcc529b17851338a152ed0cf028c46898c6a17a4";

pub const SHARED_GTT_MEMORY_PROFILE_SHA256_BYTES_V1: [u8; 32] = [
    0x10, 0x54, 0xb1, 0xc3, 0x1a, 0xd1, 0x43, 0xc7, 0x21, 0x8e, 0xee, 0x24, 0xbc, 0xc5, 0x29, 0xb1,
    0x78, 0x51, 0x33, 0x8a, 0x15, 0x2e, 0xd0, 0xcf, 0x02, 0x8c, 0x46, 0x89, 0x8c, 0x6a, 0x17, 0xa4,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedGttProfileV1 {
    HostVisibleCoherent,
    Kernarg,
    AqlQueue,
    Executable,
}

mod sealed {
    pub trait Profile {}
    pub trait State {}
    pub trait MutableProfile {}
}

pub trait GttProfileV1: sealed::Profile + 'static {
    const PROFILE: SharedGttProfileV1;
    const FLAGS: KfdAllocMemoryFlags;
    const KIND: MemoryKindV1;
    const NAME: &'static str;
}

pub trait MutableGpuGttProfileV1: GttProfileV1 + sealed::MutableProfile {}

pub enum HostVisibleCoherentGttV1 {}
pub enum KernargGttV1 {}
pub enum AqlQueueGttV1 {}
pub enum ExecutableGttV1 {}

macro_rules! define_profile {
    ($type:ty, $profile:expr, $flags:expr, $kind:expr, $name:expr) => {
        impl sealed::Profile for $type {}
        impl GttProfileV1 for $type {
            const PROFILE: SharedGttProfileV1 = $profile;
            const FLAGS: KfdAllocMemoryFlags = $flags;
            const KIND: MemoryKindV1 = $kind;
            const NAME: &'static str = $name;
        }
    };
}

define_profile!(
    HostVisibleCoherentGttV1,
    SharedGttProfileV1::HostVisibleCoherent,
    KfdAllocMemoryFlags::HOST_VISIBLE_COHERENT,
    MemoryKindV1::HostVisibleCoherent,
    "host-visible coherent GTT"
);
define_profile!(
    KernargGttV1,
    SharedGttProfileV1::Kernarg,
    KfdAllocMemoryFlags::KERNARG,
    MemoryKindV1::Kernarg,
    "kernarg GTT"
);
define_profile!(
    AqlQueueGttV1,
    SharedGttProfileV1::AqlQueue,
    KfdAllocMemoryFlags::AQL_QUEUE,
    MemoryKindV1::QueueStorage,
    "AQL queue GTT"
);
define_profile!(
    ExecutableGttV1,
    SharedGttProfileV1::Executable,
    KfdAllocMemoryFlags::EXECUTABLE,
    MemoryKindV1::Executable,
    "host-visible executable GTT"
);

impl sealed::MutableProfile for HostVisibleCoherentGttV1 {}
impl sealed::MutableProfile for KernargGttV1 {}
impl sealed::MutableProfile for AqlQueueGttV1 {}
impl MutableGpuGttProfileV1 for HostVisibleCoherentGttV1 {}
impl MutableGpuGttProfileV1 for KernargGttV1 {}
impl MutableGpuGttProfileV1 for AqlQueueGttV1 {}

pub trait GttAllocationStateV1: sealed::State + 'static {}
pub trait CpuReadableGttStateV1: GttAllocationStateV1 {}

pub enum GttCpuWritableV1 {}
pub enum GttExecutableImmutableV1 {}
pub enum GttGpuAccessibleMutableV1 {}
pub enum GttGpuAccessibleExecutableV1 {}

macro_rules! define_state {
    ($type:ty) => {
        impl sealed::State for $type {}
        impl GttAllocationStateV1 for $type {}
    };
}

define_state!(GttCpuWritableV1);
define_state!(GttExecutableImmutableV1);
define_state!(GttGpuAccessibleMutableV1);
define_state!(GttGpuAccessibleExecutableV1);
impl CpuReadableGttStateV1 for GttCpuWritableV1 {}
impl CpuReadableGttStateV1 for GttExecutableImmutableV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SharedGttAllocationLayoutV1 {
    profile: SharedGttProfileV1,
    requested_bytes: usize,
    cpu_mapping_bytes: usize,
    gpu_va_bytes: u64,
    uapi_flags: u32,
}

impl SharedGttAllocationLayoutV1 {
    pub const fn profile(self) -> SharedGttProfileV1 {
        self.profile
    }

    pub const fn requested_bytes(self) -> usize {
        self.requested_bytes
    }

    pub const fn cpu_mapping_bytes(self) -> usize {
        self.cpu_mapping_bytes
    }

    pub const fn gpu_va_bytes(self) -> u64 {
        self.gpu_va_bytes
    }

    pub const fn uapi_flags(self) -> u32 {
        self.uapi_flags
    }
}

#[must_use = "allocation authority must be transitioned or explicitly released"]
/// A redacted, linear allocation authority.
///
/// Native GPU addresses and handles deliberately have no public accessor.
///
/// ```compile_fail
/// use fe2o3_kfd::{
///     GttCpuWritableV1, HostVisibleCoherentGttV1, SharedGttAllocationV1,
/// };
///
/// fn cannot_extract_native_authority(
///     token: &SharedGttAllocationV1<HostVisibleCoherentGttV1, GttCpuWritableV1>,
/// ) {
///     let _gpu_va = token.gpu_va();
///     let _handle = token.handle();
/// }
/// ```
pub struct SharedGttAllocationV1<P: GttProfileV1, S: GttAllocationStateV1> {
    id: u64,
    generation: u64,
    layout: SharedGttAllocationLayoutV1,
    marker: PhantomData<(P, S)>,
}

impl<P: GttProfileV1, S: GttAllocationStateV1> fmt::Debug for SharedGttAllocationV1<P, S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SharedGttAllocationV1")
            .field("layout", &self.layout)
            .finish_non_exhaustive()
    }
}

impl<P: GttProfileV1, S: GttAllocationStateV1> SharedGttAllocationV1<P, S> {
    pub const fn layout(&self) -> SharedGttAllocationLayoutV1 {
        self.layout
    }

    fn retag<T: GttAllocationStateV1>(self) -> SharedGttAllocationV1<P, T> {
        SharedGttAllocationV1 {
            id: self.id,
            generation: self.generation,
            layout: self.layout,
            marker: PhantomData,
        }
    }
}

trait GpuMappedGttStateV1: GttAllocationStateV1 {
    const PHASE: SharedAllocationPhaseV1;
}

impl GpuMappedGttStateV1 for GttGpuAccessibleMutableV1 {
    const PHASE: SharedAllocationPhaseV1 = SharedAllocationPhaseV1::GpuAccessibleMutable;
}

impl GpuMappedGttStateV1 for GttGpuAccessibleExecutableV1 {
    const PHASE: SharedAllocationPhaseV1 = SharedAllocationPhaseV1::GpuAccessibleExecutable;
}

/// Crate-private facts carried beside a linear mapped allocation token.
///
/// These numeric values are never public API and are not authority by
/// themselves. The containing non-Clone capability retains the allocation
/// token required for later queue ownership and eventual explicit teardown.
#[allow(dead_code)]
pub(crate) struct SharedGttMappedResourceFactsV1 {
    gpu_va: u64,
    logical_bytes: usize,
    cpu_mapping_bytes: usize,
    gpu_va_bytes: u64,
    mapping: MemoryMappingKeyV1,
    publication: MemoryPublicationKeyV1,
}

#[allow(dead_code)]
impl SharedGttMappedResourceFactsV1 {
    pub(crate) const fn gpu_va(&self) -> u64 {
        self.gpu_va
    }

    pub(crate) const fn logical_bytes(&self) -> usize {
        self.logical_bytes
    }

    pub(crate) const fn cpu_mapping_bytes(&self) -> usize {
        self.cpu_mapping_bytes
    }

    pub(crate) const fn gpu_va_bytes(&self) -> u64 {
        self.gpu_va_bytes
    }

    pub(crate) const fn mapping(&self) -> MemoryMappingKeyV1 {
        self.mapping
    }

    pub(crate) const fn publication(&self) -> MemoryPublicationKeyV1 {
        self.publication
    }

    pub(crate) fn checked_gpu_subrange(
        &self,
        offset: u64,
        byte_len: u64,
        alignment: u64,
    ) -> Option<u64> {
        if alignment == 0 || !alignment.is_power_of_two() {
            return None;
        }
        let address = self.gpu_va.checked_add(offset)?;
        let end = offset.checked_add(byte_len)?;
        if byte_len == 0 || end > self.gpu_va_bytes || !address.is_multiple_of(alignment) {
            return None;
        }
        Some(address)
    }

    pub(crate) fn checked_disjoint_gpu_subranges(
        &self,
        left: (u64, u64, u64),
        right: (u64, u64, u64),
    ) -> Option<(u64, u64)> {
        let left_address = self.checked_gpu_subrange(left.0, left.1, left.2)?;
        let right_address = self.checked_gpu_subrange(right.0, right.1, right.2)?;
        if ranges_overlap(left_address, left.1, right_address, right.1) {
            return None;
        }
        Some((left_address, right_address))
    }
}

mod resource_role {
    pub trait Sealed {}
}

pub(crate) trait SharedGttQueueResourceRoleV1: resource_role::Sealed + 'static {}

pub(crate) enum AqlRingResourceRoleV1 {}
pub(crate) enum AqlControlResourceRoleV1 {}
pub(crate) enum AqlEndOfPipeResourceRoleV1 {}
pub(crate) enum AqlContextSaveResourceRoleV1 {}

macro_rules! define_resource_role {
    ($role:ty) => {
        impl resource_role::Sealed for $role {}
        impl SharedGttQueueResourceRoleV1 for $role {}
    };
}

define_resource_role!(AqlRingResourceRoleV1);
define_resource_role!(AqlControlResourceRoleV1);
define_resource_role!(AqlEndOfPipeResourceRoleV1);
define_resource_role!(AqlContextSaveResourceRoleV1);

/// Linear crate-private bridge from shared memory into a later queue owner.
///
/// Its role marker prevents ring/control/EOP/CWSR substitution. It does not
/// publish the mapping or issue a queue ioctl.
#[allow(dead_code)]
pub(crate) struct SharedGttQueueResourceAuthorityV1<R, P, S>
where
    R: SharedGttQueueResourceRoleV1,
    P: GttProfileV1,
    S: GttAllocationStateV1,
{
    token: SharedGttAllocationV1<P, S>,
    facts: SharedGttMappedResourceFactsV1,
    role: PhantomData<R>,
}

#[allow(dead_code, private_bounds)]
impl<R, P, S> SharedGttQueueResourceAuthorityV1<R, P, S>
where
    R: SharedGttQueueResourceRoleV1,
    P: GttProfileV1,
    S: GpuMappedGttStateV1,
{
    pub(crate) const fn facts(&self) -> &SharedGttMappedResourceFactsV1 {
        &self.facts
    }

    pub(crate) fn into_token(self) -> SharedGttAllocationV1<P, S> {
        self.token
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedMemorySessionPhaseV1 {
    Active,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SharedAllocationPhaseV1 {
    CpuWritable,
    ExecutableImmutable,
    GpuAccessibleMutable,
    GpuAccessibleExecutable,
    Released,
}

struct SharedAllocationRecord<B: MemoryBackend> {
    id: u64,
    generation: u64,
    profile: SharedGttProfileV1,
    layout: SharedGttAllocationLayoutV1,
    gpu_va: u64,
    mmap_offset: u64,
    reservation: Option<B::Reservation>,
    mapping: Option<B::Mapping>,
    handle: Option<u64>,
    free_attempted: bool,
    phase: SharedAllocationPhaseV1,
}

struct SharedMemoryEngine<B: MemoryBackend> {
    backend: B,
    phase: SharedMemorySessionPhaseV1,
    allocations: Vec<SharedAllocationRecord<B>>,
    next_id: u64,
    retained_gpu_va_bytes: u64,
}

impl<B: MemoryBackend> SharedMemoryEngine<B> {
    fn acquire(mut backend: B) -> Result<Self, MemorySessionError> {
        if backend.opener_pid() != std::process::id() {
            return Err(MemorySessionError::ProcessChanged);
        }
        if backend.page_size() != HOST_VISIBLE_MEMORY_PAGE_BYTES_V1 as usize {
            return Err(MemorySessionError::UnsupportedPageSize(backend.page_size()));
        }
        backend.check_currentness()?;
        backend.acquire_vm()?;
        backend.check_currentness()?;
        Ok(Self {
            backend,
            phase: SharedMemorySessionPhaseV1::Active,
            allocations: Vec::new(),
            next_id: 1,
            retained_gpu_va_bytes: 0,
        })
    }

    fn phase(&self) -> SharedMemorySessionPhaseV1 {
        self.phase
    }

    fn quarantine<T>(&mut self, error: MemorySessionError) -> Result<T, MemorySessionError> {
        self.phase = SharedMemorySessionPhaseV1::Quarantined;
        Err(error)
    }

    fn require_active(&self) -> Result<(), MemorySessionError> {
        if self.phase == SharedMemorySessionPhaseV1::Active {
            Ok(())
        } else {
            Err(MemorySessionError::SharedSessionQuarantined)
        }
    }

    fn check_currentness(&mut self) -> Result<(), MemorySessionError> {
        if self.backend.opener_pid() != std::process::id() {
            return self.quarantine(MemorySessionError::ProcessChanged);
        }
        if let Err(error) = self.backend.check_currentness() {
            return self.quarantine(error);
        }
        Ok(())
    }

    fn allocate<P: GttProfileV1>(
        &mut self,
        requested_bytes: usize,
    ) -> Result<SharedGttAllocationV1<P, GttCpuWritableV1>, MemorySessionError> {
        self.require_active()?;
        let layout = profile_layout::<P>(requested_bytes)?;
        if self.allocations.len() >= MAX_SHARED_GTT_ALLOCATIONS_V1 {
            return Err(MemorySessionError::SharedAllocationCapacity {
                maximum: MAX_SHARED_GTT_ALLOCATIONS_V1,
            });
        }
        let new_total = self
            .retained_gpu_va_bytes
            .checked_add(layout.gpu_va_bytes)
            .ok_or(MemorySessionError::SizeOverflow)?;
        if new_total > MAX_SHARED_GTT_GPU_VA_BYTES_V1 {
            return Err(MemorySessionError::SharedVaCapacity {
                maximum_bytes: MAX_SHARED_GTT_GPU_VA_BYTES_V1,
            });
        }
        let id = self.next_id;
        let next_id = id.checked_add(1).ok_or(MemorySessionError::SizeOverflow)?;
        self.check_currentness()?;
        let reservation_bytes =
            usize::try_from(layout.gpu_va_bytes).map_err(|_| MemorySessionError::SizeOverflow)?;
        let mut reservation = match self.backend.reserve_va(reservation_bytes) {
            Ok(reservation) => reservation,
            Err(error) => return self.quarantine(error),
        };
        let gpu_va = B::reservation_address(&reservation);
        if let Err(error) =
            validate_gpu_va_range(gpu_va, layout.gpu_va_bytes, self.backend.gpuvm_aperture())
        {
            return self.quarantine(error);
        }
        if self.allocations.iter().any(|record| {
            record.phase != SharedAllocationPhaseV1::Released
                && ranges_overlap(
                    gpu_va,
                    layout.gpu_va_bytes,
                    record.gpu_va,
                    record.layout.gpu_va_bytes,
                )
        }) {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "overlapping GPU VA reservation",
            ));
        }
        let outcome = self.backend.alloc(gpu_va, layout.gpu_va_bytes, P::FLAGS);
        let args = outcome.value;
        if let Err(error) = outcome.result {
            return self.quarantine(error);
        }
        if args.va_addr != gpu_va
            || args.size != layout.gpu_va_bytes
            || args.gpu_id != self.backend.gpu_id()
            || args.flags != P::FLAGS.bits()
            || args.handle == 0
            || args.mmap_offset == 0
            || !args
                .mmap_offset
                .is_multiple_of(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1)
        {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "shared ALLOC_MEMORY_OF_GPU output",
            ));
        }
        if self.allocations.iter().any(|record| {
            record.phase != SharedAllocationPhaseV1::Released
                && (record.handle == Some(args.handle) || record.mmap_offset == args.mmap_offset)
        }) {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "shared allocation handle or mmap-offset collision",
            ));
        }
        self.check_currentness()?;
        let mut mapping = match self.backend.map_cpu(
            &mut reservation,
            args.mmap_offset,
            layout.cpu_mapping_bytes,
            true,
        ) {
            Ok(mapping) => mapping,
            Err(error) => return self.quarantine(error),
        };
        if let Err(error) = self.backend.prepare_cpu_mapping(&mut mapping) {
            return self.quarantine(error);
        }
        self.check_currentness()?;
        self.allocations.push(SharedAllocationRecord {
            id,
            generation: 1,
            profile: P::PROFILE,
            layout,
            gpu_va,
            mmap_offset: args.mmap_offset,
            reservation: Some(reservation),
            mapping: Some(mapping),
            handle: Some(args.handle),
            free_attempted: false,
            phase: SharedAllocationPhaseV1::CpuWritable,
        });
        self.next_id = next_id;
        self.retained_gpu_va_bytes = new_total;
        Ok(SharedGttAllocationV1 {
            id,
            generation: 1,
            layout,
            marker: PhantomData,
        })
    }

    fn index<P: GttProfileV1, S: GttAllocationStateV1>(
        &self,
        token: &SharedGttAllocationV1<P, S>,
        expected: SharedAllocationPhaseV1,
    ) -> Result<usize, MemorySessionError> {
        self.require_active()?;
        self.allocations
            .iter()
            .position(|record| {
                record.id == token.id
                    && record.generation == token.generation
                    && record.profile == P::PROFILE
                    && record.layout == token.layout
                    && record.phase == expected
            })
            .ok_or(MemorySessionError::InvalidAllocationAuthority)
    }

    fn evidence<P: GttProfileV1, S: GttAllocationStateV1>(
        &self,
        token: &SharedGttAllocationV1<P, S>,
    ) -> Result<(u64, u64, SharedGttAllocationLayoutV1, u64, u64), MemorySessionError> {
        self.require_active()?;
        let record = self
            .allocations
            .iter()
            .find(|record| {
                record.id == token.id
                    && record.generation == token.generation
                    && record.profile == P::PROFILE
                    && record.layout == token.layout
                    && record.phase != SharedAllocationPhaseV1::Released
            })
            .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
        let handle = record
            .handle
            .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
        Ok((
            record.id,
            record.generation,
            record.layout,
            record.gpu_va,
            handle,
        ))
    }

    fn with_bytes<P, S, R>(
        &mut self,
        token: &SharedGttAllocationV1<P, S>,
        expected: SharedAllocationPhaseV1,
        f: impl FnOnce(&[u8]) -> R,
    ) -> Result<R, MemorySessionError>
    where
        P: GttProfileV1,
        S: CpuReadableGttStateV1,
    {
        self.check_currentness()?;
        let index = self.index(token, expected)?;
        let requested = self.allocations[index].layout.requested_bytes;
        let outcome = {
            let mapping = self.allocations[index]
                .mapping
                .as_ref()
                .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                B::with_bytes(mapping, requested, f)
            }))
        };
        let post = self.check_currentness();
        match outcome {
            Ok(value) => {
                post?;
                Ok(value)
            }
            Err(payload) => {
                let _ = post;
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn with_bytes_mut<P: GttProfileV1, R>(
        &mut self,
        token: &mut SharedGttAllocationV1<P, GttCpuWritableV1>,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, MemorySessionError> {
        self.check_currentness()?;
        let index = self.index(token, SharedAllocationPhaseV1::CpuWritable)?;
        let requested = self.allocations[index].layout.requested_bytes;
        let outcome = {
            let mapping = self.allocations[index]
                .mapping
                .as_mut()
                .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                B::with_bytes_mut(mapping, requested, f)
            }))
        };
        let post = self.check_currentness();
        match outcome {
            Ok(value) => {
                post?;
                Ok(value)
            }
            Err(payload) => {
                let _ = post;
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn seal_executable(
        &mut self,
        token: SharedGttAllocationV1<ExecutableGttV1, GttCpuWritableV1>,
    ) -> Result<SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>, MemorySessionError>
    {
        self.check_currentness()?;
        let index = self.index(&token, SharedAllocationPhaseV1::CpuWritable)?;
        let result = {
            let (backend, allocations) = (&mut self.backend, &mut self.allocations);
            let mapping = allocations[index]
                .mapping
                .as_mut()
                .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
            backend.protect_cpu_read_only(mapping)
        };
        if let Err(error) = result {
            return self.quarantine(error);
        }
        self.check_currentness()?;
        self.allocations[index].phase = SharedAllocationPhaseV1::ExecutableImmutable;
        Ok(token.retag())
    }

    fn map_mutable<P: MutableGpuGttProfileV1>(
        &mut self,
        token: SharedGttAllocationV1<P, GttCpuWritableV1>,
    ) -> Result<SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>, MemorySessionError> {
        let index = self.index(&token, SharedAllocationPhaseV1::CpuWritable)?;
        self.map_index(index)?;
        self.allocations[index].phase = SharedAllocationPhaseV1::GpuAccessibleMutable;
        Ok(token.retag())
    }

    fn map_executable(
        &mut self,
        token: SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>,
    ) -> Result<
        SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>,
        MemorySessionError,
    > {
        let index = self.index(&token, SharedAllocationPhaseV1::ExecutableImmutable)?;
        self.map_index(index)?;
        self.allocations[index].phase = SharedAllocationPhaseV1::GpuAccessibleExecutable;
        Ok(token.retag())
    }

    fn map_index(&mut self, index: usize) -> Result<(), MemorySessionError> {
        self.check_currentness()?;
        let handle = self.allocations[index]
            .handle
            .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
        let outcome = self.backend.map_gpu(handle, 0);
        if outcome.value > 1 {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "shared MAP_MEMORY_TO_GPU cumulative n_success",
            ));
        }
        if let Err(error) = outcome.result {
            return self.quarantine(error);
        }
        if outcome.value != 1 {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "shared MAP_MEMORY_TO_GPU full prefix",
            ));
        }
        self.check_currentness()
    }

    fn unmap_mutable<P: MutableGpuGttProfileV1>(
        &mut self,
        token: SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>,
    ) -> Result<SharedGttAllocationV1<P, GttCpuWritableV1>, MemorySessionError> {
        let index = self.index(&token, SharedAllocationPhaseV1::GpuAccessibleMutable)?;
        self.unmap_index(index)?;
        self.allocations[index].phase = SharedAllocationPhaseV1::CpuWritable;
        Ok(token.retag())
    }

    fn unmap_executable(
        &mut self,
        token: SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>,
    ) -> Result<SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>, MemorySessionError>
    {
        let index = self.index(&token, SharedAllocationPhaseV1::GpuAccessibleExecutable)?;
        self.unmap_index(index)?;
        self.allocations[index].phase = SharedAllocationPhaseV1::ExecutableImmutable;
        Ok(token.retag())
    }

    fn unmap_index(&mut self, index: usize) -> Result<(), MemorySessionError> {
        self.check_currentness()?;
        let handle = self.allocations[index]
            .handle
            .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
        let outcome = self.backend.unmap_gpu(handle, 0);
        if outcome.value > 1 {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "shared UNMAP_MEMORY_FROM_GPU cumulative n_success",
            ));
        }
        if let Err(error) = outcome.result {
            return self.quarantine(error);
        }
        if outcome.value != 1 {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "shared UNMAP_MEMORY_FROM_GPU full prefix",
            ));
        }
        self.check_currentness()
    }

    fn release<P: GttProfileV1, S: GttAllocationStateV1>(
        &mut self,
        token: SharedGttAllocationV1<P, S>,
        expected: SharedAllocationPhaseV1,
    ) -> Result<(), MemorySessionError> {
        let index = self.index(&token, expected)?;
        self.check_currentness()?;
        if self.allocations[index].free_attempted {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "shared FREE_MEMORY_OF_GPU exactly-once",
            ));
        }
        let unmap_result = {
            let (backend, allocations) = (&mut self.backend, &mut self.allocations);
            let mapping = allocations[index]
                .mapping
                .as_mut()
                .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
            backend.unmap_cpu(mapping)
        };
        if let Err(error) = unmap_result {
            return self.quarantine(error);
        }
        self.allocations[index].mapping = None;
        self.check_currentness()?;
        let handle = self.allocations[index]
            .handle
            .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
        self.allocations[index].free_attempted = true;
        if let Err(error) = self.backend.free(handle) {
            return self.quarantine(error);
        }
        self.allocations[index].handle = None;
        self.check_currentness()?;
        let release_reservation = {
            let (backend, allocations) = (&mut self.backend, &mut self.allocations);
            let reservation = allocations[index]
                .reservation
                .as_mut()
                .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
            backend.release_va_reservation(reservation)
        };
        if let Err(error) = release_reservation {
            return self.quarantine(error);
        }
        self.allocations[index].reservation = None;
        self.check_currentness()?;
        self.allocations[index].phase = SharedAllocationPhaseV1::Released;
        self.retained_gpu_va_bytes = self
            .retained_gpu_va_bytes
            .checked_sub(token.layout.gpu_va_bytes)
            .ok_or(MemorySessionError::KernelResultMalformed(
                "shared retained GPU VA accounting",
            ))?;
        Ok(())
    }
}

impl<B: MemoryBackend> Drop for SharedMemoryEngine<B> {
    fn drop(&mut self) {
        // Native cleanup and destructive FREE are always explicit. Drop never
        // retries an operation whose kernel result could be ambiguous.
    }
}

fn profile_layout<P: GttProfileV1>(
    requested_bytes: usize,
) -> Result<SharedGttAllocationLayoutV1, MemorySessionError> {
    if requested_bytes == 0 {
        return Err(MemorySessionError::InvalidRequestedSize);
    }
    let requested = u64::try_from(requested_bytes).map_err(|_| MemorySessionError::SizeOverflow)?;
    let cpu_mapping_bytes = if P::PROFILE == SharedGttProfileV1::AqlQueue {
        if !(MIN_AQL_QUEUE_BYTES_V1..=MAX_AQL_QUEUE_BYTES_V1).contains(&requested)
            || !requested.is_power_of_two()
        {
            return Err(MemorySessionError::InvalidProfileSize(P::NAME));
        }
        requested
    } else {
        requested
            .checked_add(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1 - 1)
            .map(|bytes| bytes & !(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1 - 1))
            .ok_or(MemorySessionError::SizeOverflow)?
    };
    if cpu_mapping_bytes > MAX_SHARED_GTT_SINGLE_CPU_BYTES_V1 {
        return Err(MemorySessionError::InvalidProfileSize(P::NAME));
    }
    let gpu_va_bytes = if P::PROFILE == SharedGttProfileV1::AqlQueue {
        cpu_mapping_bytes
            .checked_mul(2)
            .ok_or(MemorySessionError::SizeOverflow)?
    } else {
        cpu_mapping_bytes
    };
    Ok(SharedGttAllocationLayoutV1 {
        profile: P::PROFILE,
        requested_bytes,
        cpu_mapping_bytes: usize::try_from(cpu_mapping_bytes)
            .map_err(|_| MemorySessionError::SizeOverflow)?,
        gpu_va_bytes,
        uapi_flags: P::FLAGS.bits(),
    })
}

fn validate_gpu_va_range(
    base: u64,
    byte_len: u64,
    aperture: crate::InclusiveAperture,
) -> Result<(), MemorySessionError> {
    if !base.is_multiple_of(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1) {
        return Err(MemorySessionError::AddressNotPageAligned);
    }
    let end = base
        .checked_add(byte_len)
        .ok_or(MemorySessionError::AddressOutsideAperture)?;
    let aperture_end = aperture
        .limit()
        .checked_add(1)
        .ok_or(MemorySessionError::AddressOutsideAperture)?;
    if byte_len == 0 || base < aperture.base() || end > aperture_end {
        return Err(MemorySessionError::AddressOutsideAperture);
    }
    Ok(())
}

fn ranges_overlap(left: u64, left_len: u64, right: u64, right_len: u64) -> bool {
    let Some(left_end) = left.checked_add(left_len) else {
        return true;
    };
    let Some(right_end) = right.checked_add(right_len) else {
        return true;
    };
    left < right_end && right < left_end
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[must_use = "dropping the shared session performs no munmap, FREE, or retry"]
pub struct SharedGttMemorySessionV1 {
    engine: SharedMemoryEngine<crate::memory_linux::LinuxMemoryBackend>,
    identity: DeviceIdentityStateV1,
    model: MemoryLifecycleStateV1,
    model_device: ModelDeviceAdmissionV1,
    vm: VmKeyV1,
    model_transferred: bool,
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
impl CheckedGfx942XnackMinusDevice {
    /// Acquires one process VM that can retain several bounded typed GTT BOs.
    pub fn acquire_shared_gtt_memory_session(
        self,
    ) -> Result<SharedGttMemorySessionV1, MemorySessionError> {
        let pid = std::process::id();
        let gpu_id = self.observation().kfd_gpu_id();
        let vm_id = NEXT_MODEL_VM_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| MemorySessionError::Model("VM identity exhausted"))?;
        begin_process_vm_attempt(pid, gpu_id)?;
        let result = (|| {
            let mut engine =
                SharedMemoryEngine::acquire(crate::memory_linux::LinuxMemoryBackend::new(self))?;
            let model_device = engine.backend.model_device();
            let aperture = engine.backend.model_aperture();
            let (identity, model_vm) = engine.backend.bind_model_vm(VmIdV1(vm_id))?;
            let byte_len = aperture
                .limit()
                .checked_sub(aperture.base())
                .and_then(|length| length.checked_add(1))
                .ok_or(MemorySessionError::Model("invalid model aperture"))?;
            let model = MemoryLifecycleStateV1::new(model_device.domain_id())
                .next(MemoryTransitionV1::AcquireVm {
                    admission: model_vm,
                    mapping_devices: vec![model_device],
                    handle: UntrustedVmHandleObservationV1(vm_id),
                    aperture: GpuVaRangeV1 {
                        base: aperture.base(),
                        byte_len,
                    },
                })
                .map_err(|_| MemorySessionError::Model("VM acquisition projection"))?;
            Ok(SharedGttMemorySessionV1 {
                engine,
                identity,
                model,
                model_device,
                vm: model_vm.model_key(),
                model_transferred: false,
            })
        })();
        finish_process_vm_attempt(result.is_ok(), pid, gpu_id);
        result
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
impl SharedGttMemorySessionV1 {
    pub fn phase(&self) -> SharedMemorySessionPhaseV1 {
        self.engine.phase()
    }

    pub fn retained_allocation_count(&self) -> usize {
        self.engine
            .allocations
            .iter()
            .filter(|record| record.phase != SharedAllocationPhaseV1::Released)
            .count()
    }

    pub fn model_journal_summary(&self) -> MemoryModelJournalSummary {
        MemoryModelJournalSummary::from_model(&self.model)
    }

    pub(crate) fn opener_pid(&self) -> u32 {
        self.engine.backend.opener_pid()
    }

    pub(crate) fn kfd_fd(&self) -> BorrowedFd<'_> {
        self.engine.backend.kfd_fd()
    }

    pub(crate) fn check_queue_currentness(&mut self) -> Result<(), MemorySessionError> {
        self.engine.require_active()?;
        self.engine.check_currentness()
    }

    pub(crate) const fn queue_model_device(&self) -> ModelDeviceAdmissionV1 {
        self.model_device
    }

    pub(crate) fn take_queue_model_foundation(
        &mut self,
    ) -> Result<(DeviceIdentityStateV1, MemoryLifecycleStateV1), MemorySessionError> {
        self.check_queue_currentness()?;
        if self.model_transferred {
            return Err(MemorySessionError::Model(
                "shared queue model ownership already transferred",
            ));
        }
        let domain = self.model.domain_id();
        self.model_transferred = true;
        Ok((
            core::mem::replace(&mut self.identity, DeviceIdentityStateV1::new(domain)),
            core::mem::replace(&mut self.model, MemoryLifecycleStateV1::new(domain)),
        ))
    }

    pub(crate) fn restore_queue_model_foundation(
        &mut self,
        identity: DeviceIdentityStateV1,
        model: MemoryLifecycleStateV1,
    ) -> Result<(), MemorySessionError> {
        if !self.model_transferred
            || identity.domain_id() != self.model_device.domain_id()
            || model.domain_id() != self.model_device.domain_id()
            || identity.validate_global_invariants().is_err()
            || model.validate_global_invariants().is_err()
            || !identity.devices().iter().any(|record| {
                record.key == self.model_device.model_key()
                    && record.status == ModelAdmissionStatusV1::Active
            })
            || !identity.vms().iter().any(|record| {
                record.key == self.vm && record.status == ModelAdmissionStatusV1::Active
            })
            || !model.vms().iter().any(|record| {
                record.admission.model_key() == self.vm
                    && record.state == fe2o3_runtime_model::MemoryVmStateV1::Active
            })
        {
            return self.engine.quarantine(MemorySessionError::Model(
                "shared queue model ownership restoration",
            ));
        }
        self.identity = identity;
        self.model = model;
        self.model_transferred = false;
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn retain_aql_ring_resource(
        &self,
        token: SharedGttAllocationV1<AqlQueueGttV1, GttGpuAccessibleMutableV1>,
    ) -> Result<
        SharedGttQueueResourceAuthorityV1<
            AqlRingResourceRoleV1,
            AqlQueueGttV1,
            GttGpuAccessibleMutableV1,
        >,
        MemorySessionError,
    > {
        self.retain_queue_resource(token)
    }

    #[allow(dead_code)]
    pub(crate) fn retain_aql_control_resource(
        &self,
        token: SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>,
    ) -> Result<
        SharedGttQueueResourceAuthorityV1<
            AqlControlResourceRoleV1,
            HostVisibleCoherentGttV1,
            GttGpuAccessibleMutableV1,
        >,
        MemorySessionError,
    > {
        self.retain_queue_resource(token)
    }

    #[allow(dead_code)]
    pub(crate) fn retain_aql_eop_resource(
        &self,
        token: SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>,
    ) -> Result<
        SharedGttQueueResourceAuthorityV1<
            AqlEndOfPipeResourceRoleV1,
            ExecutableGttV1,
            GttGpuAccessibleExecutableV1,
        >,
        MemorySessionError,
    > {
        self.retain_queue_resource(token)
    }

    #[allow(dead_code)]
    pub(crate) fn retain_aql_context_save_resource(
        &self,
        token: SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>,
    ) -> Result<
        SharedGttQueueResourceAuthorityV1<
            AqlContextSaveResourceRoleV1,
            ExecutableGttV1,
            GttGpuAccessibleExecutableV1,
        >,
        MemorySessionError,
    > {
        self.retain_queue_resource(token)
    }

    #[allow(dead_code, private_bounds)]
    fn retain_queue_resource<R, P, S>(
        &self,
        token: SharedGttAllocationV1<P, S>,
    ) -> Result<SharedGttQueueResourceAuthorityV1<R, P, S>, MemorySessionError>
    where
        R: SharedGttQueueResourceRoleV1,
        P: GttProfileV1,
        S: GpuMappedGttStateV1,
    {
        let index = self.engine.index(&token, S::PHASE)?;
        let record = &self.engine.allocations[index];
        let (_, _, mapping) = model_keys(self.vm, record.id, record.generation);
        Ok(SharedGttQueueResourceAuthorityV1 {
            token,
            facts: SharedGttMappedResourceFactsV1 {
                gpu_va: record.gpu_va,
                logical_bytes: record.layout.requested_bytes,
                cpu_mapping_bytes: record.layout.cpu_mapping_bytes,
                gpu_va_bytes: record.layout.gpu_va_bytes,
                mapping,
                publication: MemoryPublicationKeyV1 {
                    mapping,
                    id: MemoryPublicationIdV1(record.id),
                },
            },
            role: PhantomData,
        })
    }

    pub fn allocate_host_visible_coherent(
        &mut self,
        requested_bytes: usize,
    ) -> Result<SharedGttAllocationV1<HostVisibleCoherentGttV1, GttCpuWritableV1>, MemorySessionError>
    {
        self.allocate_profile(requested_bytes)
    }

    pub fn allocate_kernarg(
        &mut self,
        requested_bytes: usize,
    ) -> Result<SharedGttAllocationV1<KernargGttV1, GttCpuWritableV1>, MemorySessionError> {
        self.allocate_profile(requested_bytes)
    }

    /// Allocates one physical ring with the driver-required doubled GPU VA.
    pub fn allocate_aql_queue(
        &mut self,
        ring_bytes: usize,
    ) -> Result<SharedGttAllocationV1<AqlQueueGttV1, GttCpuWritableV1>, MemorySessionError> {
        self.allocate_profile(ring_bytes)
    }

    pub fn allocate_executable(
        &mut self,
        requested_bytes: usize,
    ) -> Result<SharedGttAllocationV1<ExecutableGttV1, GttCpuWritableV1>, MemorySessionError> {
        self.allocate_profile(requested_bytes)
    }

    fn allocate_profile<P: GttProfileV1>(
        &mut self,
        requested_bytes: usize,
    ) -> Result<SharedGttAllocationV1<P, GttCpuWritableV1>, MemorySessionError> {
        let token = self.engine.allocate::<P>(requested_bytes)?;
        let (id, generation, layout, base, handle) = self.engine.evidence(&token)?;
        let (reservation, allocation, _) = model_keys(self.vm, id, generation);
        let projected = project_allocation(
            &self.model,
            reservation,
            allocation,
            base,
            layout,
            handle,
            P::KIND,
        );
        match projected {
            Ok(model) => {
                self.model = model;
                Ok(token)
            }
            Err(_) => self
                .engine
                .quarantine(MemorySessionError::Model("shared allocation projection")),
        }
    }

    pub fn with_bytes<P, S, R>(
        &mut self,
        token: &SharedGttAllocationV1<P, S>,
        f: impl FnOnce(&[u8]) -> R,
    ) -> Result<R, MemorySessionError>
    where
        P: GttProfileV1,
        S: CpuReadableGttStateV1,
    {
        let expected = if P::PROFILE == SharedGttProfileV1::Executable {
            SharedAllocationPhaseV1::ExecutableImmutable
        } else {
            SharedAllocationPhaseV1::CpuWritable
        };
        self.engine.with_bytes(token, expected, f)
    }

    pub fn with_bytes_mut<P: GttProfileV1, R>(
        &mut self,
        token: &mut SharedGttAllocationV1<P, GttCpuWritableV1>,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, MemorySessionError> {
        self.engine.with_bytes_mut(token, f)
    }

    pub fn seal_executable(
        &mut self,
        token: SharedGttAllocationV1<ExecutableGttV1, GttCpuWritableV1>,
    ) -> Result<SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>, MemorySessionError>
    {
        self.engine.seal_executable(token)
    }

    pub fn map_to_gpu<P: MutableGpuGttProfileV1>(
        &mut self,
        token: SharedGttAllocationV1<P, GttCpuWritableV1>,
    ) -> Result<SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>, MemorySessionError> {
        let (id, generation, _, _, _) = self.engine.evidence(&token)?;
        let (_, _, mapping) = model_keys(self.vm, id, generation);
        let mapped = self.engine.map_mutable(token)?;
        self.commit_map_projection(mapping)?;
        Ok(mapped)
    }

    pub fn map_executable_to_gpu(
        &mut self,
        token: SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>,
    ) -> Result<
        SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>,
        MemorySessionError,
    > {
        let (id, generation, _, _, _) = self.engine.evidence(&token)?;
        let (_, _, mapping) = model_keys(self.vm, id, generation);
        let mapped = self.engine.map_executable(token)?;
        self.commit_map_projection(mapping)?;
        Ok(mapped)
    }

    pub fn unmap_from_gpu<P: MutableGpuGttProfileV1>(
        &mut self,
        token: SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>,
    ) -> Result<SharedGttAllocationV1<P, GttCpuWritableV1>, MemorySessionError> {
        let (id, generation, _, _, _) = self.engine.evidence(&token)?;
        let (_, _, mapping) = model_keys(self.vm, id, generation);
        let unmapped = self.engine.unmap_mutable(token)?;
        self.commit_unmap_projection(mapping)?;
        Ok(unmapped)
    }

    pub fn unmap_executable_from_gpu(
        &mut self,
        token: SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>,
    ) -> Result<SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>, MemorySessionError>
    {
        let (id, generation, _, _, _) = self.engine.evidence(&token)?;
        let (_, _, mapping) = model_keys(self.vm, id, generation);
        let unmapped = self.engine.unmap_executable(token)?;
        self.commit_unmap_projection(mapping)?;
        Ok(unmapped)
    }

    pub fn release<P: GttProfileV1>(
        &mut self,
        token: SharedGttAllocationV1<P, GttCpuWritableV1>,
    ) -> Result<(), MemorySessionError> {
        self.release_with_phase(token, SharedAllocationPhaseV1::CpuWritable)
    }

    pub fn release_executable(
        &mut self,
        token: SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>,
    ) -> Result<(), MemorySessionError> {
        self.release_with_phase(token, SharedAllocationPhaseV1::ExecutableImmutable)
    }

    fn release_with_phase<P: GttProfileV1, S: GttAllocationStateV1>(
        &mut self,
        token: SharedGttAllocationV1<P, S>,
        phase: SharedAllocationPhaseV1,
    ) -> Result<(), MemorySessionError> {
        let (id, generation, _, _, _) = self.engine.evidence(&token)?;
        let (reservation, allocation, mapping) = model_keys(self.vm, id, generation);
        let projected = project_release(&self.model, reservation, allocation, mapping)
            .map_err(|_| MemorySessionError::Model("shared release projection"))?;
        self.engine.release(token, phase)?;
        self.model = projected;
        Ok(())
    }

    fn commit_map_projection(
        &mut self,
        mapping: MemoryMappingKeyV1,
    ) -> Result<(), MemorySessionError> {
        match project_map(&self.model, mapping, self.model_device) {
            Ok(model) => {
                self.model = model;
                Ok(())
            }
            Err(_) => self
                .engine
                .quarantine(MemorySessionError::Model("shared map projection")),
        }
    }

    fn commit_unmap_projection(
        &mut self,
        mapping: MemoryMappingKeyV1,
    ) -> Result<(), MemorySessionError> {
        match project_unmap(&self.model, mapping) {
            Ok(model) => {
                self.model = model;
                Ok(())
            }
            Err(_) => self
                .engine
                .quarantine(MemorySessionError::Model("shared unmap projection")),
        }
    }
}

fn model_keys(
    vm: VmKeyV1,
    id: u64,
    generation: u64,
) -> (
    VaReservationKeyV1,
    MemoryAllocationKeyV1,
    MemoryMappingKeyV1,
) {
    let reservation = VaReservationKeyV1 {
        vm,
        id: VaReservationIdV1(id),
    };
    let allocation = MemoryAllocationKeyV1 {
        vm,
        id: AllocationIdV1(id),
        generation: AllocationGenerationV1(generation),
    };
    let mapping = MemoryMappingKeyV1 {
        allocation,
        id: MappingIdV1(id),
    };
    (reservation, allocation, mapping)
}

fn project_allocation(
    model: &MemoryLifecycleStateV1,
    reservation: VaReservationKeyV1,
    allocation: MemoryAllocationKeyV1,
    base: u64,
    layout: SharedGttAllocationLayoutV1,
    handle: u64,
    kind: MemoryKindV1,
) -> Result<MemoryLifecycleStateV1, MemoryTransitionErrorV1> {
    model
        .next(MemoryTransitionV1::ReserveVa {
            key: reservation,
            range: GpuVaRangeV1 {
                base,
                byte_len: layout.gpu_va_bytes,
            },
            alignment: HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
        })
        .and_then(|state| {
            state.next(MemoryTransitionV1::Allocate {
                key: allocation,
                reservation,
                handle: UntrustedAllocationHandleObservationV1(handle),
                spec: MemoryAllocationSpecV1 {
                    byte_len: layout.gpu_va_bytes,
                    alignment: HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
                    kind,
                    coherence: MemoryCoherenceV1::HostCoherent,
                },
            })
        })
}

fn project_map(
    model: &MemoryLifecycleStateV1,
    mapping: MemoryMappingKeyV1,
    device: ModelDeviceAdmissionV1,
) -> Result<MemoryLifecycleStateV1, MemoryTransitionErrorV1> {
    model
        .next(MemoryTransitionV1::BeginMap {
            key: mapping,
            target_devices: vec![device.model_key()],
            access: MemoryAccessV1::ReadWrite,
        })
        .and_then(|state| {
            state.next(MemoryTransitionV1::ObserveMap {
                key: mapping,
                progress: PartialProgressObservationV1 {
                    n_success: 1,
                    status: PartialOperationStatusV1::Succeeded,
                },
            })
        })
}

fn project_unmap(
    model: &MemoryLifecycleStateV1,
    mapping: MemoryMappingKeyV1,
) -> Result<MemoryLifecycleStateV1, MemoryTransitionErrorV1> {
    model
        .next(MemoryTransitionV1::BeginUnmap { key: mapping })
        .and_then(|state| {
            state.next(MemoryTransitionV1::ObserveUnmap {
                key: mapping,
                progress: PartialProgressObservationV1 {
                    n_success: 1,
                    status: PartialOperationStatusV1::Succeeded,
                },
            })
        })
}

fn project_release(
    model: &MemoryLifecycleStateV1,
    reservation: VaReservationKeyV1,
    allocation: MemoryAllocationKeyV1,
    mapping: MemoryMappingKeyV1,
) -> Result<MemoryLifecycleStateV1, MemoryTransitionErrorV1> {
    let mut projected = model.clone();
    if projected
        .mappings()
        .iter()
        .any(|record| record.key == mapping)
    {
        projected = projected.next(MemoryTransitionV1::ReleaseMapping { key: mapping })?;
    }
    projected
        .next(MemoryTransitionV1::ReleaseAllocation { key: allocation })
        .and_then(|state| state.next(MemoryTransitionV1::ReleaseVaReservation { key: reservation }))
}

const _: () = {
    assert!(KFD_ALLOC_MEMORY_FLAGS_HOST_VISIBLE_COHERENT == 0x8400_0002);
    assert!(KFD_ALLOC_MEMORY_FLAGS_KERNARG == 0x8600_0002);
    assert!(KFD_ALLOC_MEMORY_FLAGS_AQL_QUEUE == 0x8e00_0002);
    assert!(KFD_ALLOC_MEMORY_FLAGS_EXECUTABLE == 0xc400_0002);
};

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kfd_uapi::KfdIoctlAllocMemoryOfGpuArgs;
    use sha2::{Digest, Sha256};

    use crate::memory::KernelOutcome;

    struct FakeMapping {
        bytes: Vec<u8>,
        active: bool,
        writable: bool,
    }

    struct FakeBackend {
        next_va: u64,
        next_handle: u64,
        flags: Vec<u32>,
        fail_operation: Option<&'static str>,
        fixed_va: Option<u64>,
        map_progress: u32,
        unmap_progress: u32,
        map_errno: bool,
        corrupt_flags: bool,
        free_calls: usize,
        release_va_calls: usize,
    }

    impl FakeBackend {
        fn good() -> Self {
            Self {
                next_va: 0x2_0000,
                next_handle: 1,
                flags: Vec::new(),
                fail_operation: None,
                fixed_va: None,
                map_progress: 1,
                unmap_progress: 1,
                map_errno: false,
                corrupt_flags: false,
                free_calls: 0,
                release_va_calls: 0,
            }
        }

        fn check(&self, operation: &'static str) -> Result<(), MemorySessionError> {
            if self.fail_operation == Some(operation) {
                Err(MemorySessionError::Injected(operation))
            } else {
                Ok(())
            }
        }
    }

    impl MemoryBackend for FakeBackend {
        type Reservation = (u64, usize);
        type Mapping = FakeMapping;

        fn opener_pid(&self) -> u32 {
            std::process::id()
        }
        fn gpu_id(&self) -> u32 {
            7
        }
        fn gpuvm_aperture(&self) -> crate::InclusiveAperture {
            crate::InclusiveAperture::from_checked_parts_for_memory_tests(0x1_0000, 0x4_0000_0000)
        }
        fn page_size(&self) -> usize {
            4096
        }
        fn check_currentness(&mut self) -> Result<(), MemorySessionError> {
            self.check("currentness")
        }
        fn acquire_vm(&mut self) -> Result<(), MemorySessionError> {
            self.check("acquire_vm")
        }
        fn reserve_va(&mut self, bytes: usize) -> Result<Self::Reservation, MemorySessionError> {
            self.check("reserve_va")?;
            let address = self.fixed_va.unwrap_or(self.next_va);
            self.next_va = self
                .next_va
                .checked_add(bytes as u64)
                .and_then(|value| value.checked_add(4096))
                .unwrap();
            Ok((address, bytes))
        }
        fn reservation_address(reservation: &Self::Reservation) -> u64 {
            reservation.0
        }
        fn alloc(
            &mut self,
            va: u64,
            bytes: u64,
            flags: KfdAllocMemoryFlags,
        ) -> KernelOutcome<KfdIoctlAllocMemoryOfGpuArgs> {
            self.flags.push(flags.bits());
            let handle = self.next_handle;
            self.next_handle += 1;
            let mut args = KfdIoctlAllocMemoryOfGpuArgs::new(va, bytes, 7, flags);
            args.handle = handle;
            args.mmap_offset = 0x40_000 + handle * 4096;
            if self.corrupt_flags {
                args.flags ^= 1;
            }
            KernelOutcome {
                value: args,
                result: self.check("alloc"),
            }
        }
        fn map_cpu(
            &mut self,
            _reservation: &mut Self::Reservation,
            _mmap_offset: u64,
            bytes: usize,
            _retain_gpu_va_guard: bool,
        ) -> Result<Self::Mapping, MemorySessionError> {
            self.check("map_cpu")?;
            Ok(FakeMapping {
                bytes: vec![0; bytes],
                active: true,
                writable: false,
            })
        }
        fn prepare_cpu_mapping(
            &mut self,
            mapping: &mut Self::Mapping,
        ) -> Result<(), MemorySessionError> {
            self.check("prepare_cpu_mapping")?;
            mapping.writable = true;
            Ok(())
        }
        fn protect_cpu_read_only(
            &mut self,
            mapping: &mut Self::Mapping,
        ) -> Result<(), MemorySessionError> {
            self.check("protect_cpu_read_only")?;
            mapping.writable = false;
            Ok(())
        }
        fn map_gpu(&mut self, _handle: u64, _old_success: u32) -> KernelOutcome<u32> {
            KernelOutcome {
                value: self.map_progress,
                result: if self.map_errno {
                    Err(MemorySessionError::Injected("map_gpu"))
                } else {
                    self.check("map_gpu")
                },
            }
        }
        fn unmap_gpu(&mut self, _handle: u64, _old_success: u32) -> KernelOutcome<u32> {
            KernelOutcome {
                value: self.unmap_progress,
                result: self.check("unmap_gpu"),
            }
        }
        fn with_bytes<R>(
            mapping: &Self::Mapping,
            requested_bytes: usize,
            f: impl FnOnce(&[u8]) -> R,
        ) -> R {
            assert!(mapping.active);
            f(&mapping.bytes[..requested_bytes])
        }
        fn with_bytes_mut<R>(
            mapping: &mut Self::Mapping,
            requested_bytes: usize,
            f: impl FnOnce(&mut [u8]) -> R,
        ) -> R {
            assert!(mapping.active && mapping.writable);
            f(&mut mapping.bytes[..requested_bytes])
        }
        fn unmap_cpu(&mut self, mapping: &mut Self::Mapping) -> Result<(), MemorySessionError> {
            self.check("unmap_cpu")?;
            mapping.active = false;
            Ok(())
        }
        fn release_va_reservation(
            &mut self,
            _reservation: &mut Self::Reservation,
        ) -> Result<(), MemorySessionError> {
            self.release_va_calls += 1;
            self.check("release_va_reservation")
        }
        fn free(&mut self, _handle: u64) -> Result<(), MemorySessionError> {
            self.free_calls += 1;
            self.check("free")
        }
    }

    fn acquired() -> SharedMemoryEngine<FakeBackend> {
        SharedMemoryEngine::acquire(FakeBackend::good()).unwrap()
    }

    #[test]
    fn shared_profile_manifest_is_frozen() {
        let digest = Sha256::digest(SHARED_GTT_MEMORY_PROFILE_MANIFEST_V1);
        let mut digest_hex = String::with_capacity(64);
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in digest.iter().copied() {
            digest_hex.push(char::from(HEX[usize::from(byte >> 4)]));
            digest_hex.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        assert_eq!(digest.as_slice(), SHARED_GTT_MEMORY_PROFILE_SHA256_BYTES_V1);
        assert_eq!(digest_hex, SHARED_GTT_MEMORY_PROFILE_SHA256_V1);
    }

    #[test]
    fn private_role_subranges_reject_overlap_misalignment_and_overflow() {
        let vm = VmKeyV1 {
            device: fe2o3_runtime_model::DeviceKeyV1 {
                physical: fe2o3_runtime_model::PhysicalDeviceIdV1(1),
                generation: fe2o3_runtime_model::DeviceGenerationV1(1),
            },
            id: VmIdV1(1),
        };
        let (_, _, mapping) = model_keys(vm, 7, 1);
        let facts = SharedGttMappedResourceFactsV1 {
            gpu_va: 0x20_000,
            logical_bytes: 8192,
            cpu_mapping_bytes: 8192,
            gpu_va_bytes: 8192,
            mapping,
            publication: MemoryPublicationKeyV1 {
                mapping,
                id: MemoryPublicationIdV1(7),
            },
        };
        assert_eq!(
            facts.checked_disjoint_gpu_subranges((0, 8, 8), (4096, 8, 8)),
            Some((0x20_000, 0x21_000))
        );
        assert_eq!(
            facts.checked_disjoint_gpu_subranges((0, 8, 8), (0, 8, 8)),
            None
        );
        assert_eq!(facts.checked_gpu_subrange(1, 8, 8), None);
        assert_eq!(facts.checked_gpu_subrange(8188, 8, 4), None);
        assert_eq!(facts.checked_gpu_subrange(0, 0, 8), None);

        let overflowing = SharedGttMappedResourceFactsV1 {
            gpu_va: u64::MAX - 4095,
            ..facts
        };
        assert_eq!(overflowing.checked_gpu_subrange(4096, 8, 8), None);
    }

    #[test]
    fn four_profiles_coexist_with_exact_flags_and_aql_geometry() {
        let mut engine = acquired();
        let mut ordinary = engine.allocate::<HostVisibleCoherentGttV1>(4097).unwrap();
        let mut kernarg = engine.allocate::<KernargGttV1>(256).unwrap();
        let mut aql = engine.allocate::<AqlQueueGttV1>(4096).unwrap();
        let mut executable = engine.allocate::<ExecutableGttV1>(8192).unwrap();
        assert_eq!(
            engine.backend.flags,
            vec![0x8400_0002, 0x8600_0002, 0x8e00_0002, 0xc400_0002]
        );
        assert_eq!(ordinary.layout().cpu_mapping_bytes(), 8192);
        assert_eq!(aql.layout().cpu_mapping_bytes(), 4096);
        assert_eq!(aql.layout().gpu_va_bytes(), 8192);
        engine
            .with_bytes_mut(&mut ordinary, |bytes| bytes[0] = 11)
            .unwrap();
        engine
            .with_bytes_mut(&mut kernarg, |bytes| bytes[0] = 22)
            .unwrap();
        engine
            .with_bytes_mut(&mut aql, |bytes| bytes[0] = 33)
            .unwrap();
        engine
            .with_bytes_mut(&mut executable, |bytes| bytes[0] = 44)
            .unwrap();
        assert_eq!(
            engine
                .with_bytes(&ordinary, SharedAllocationPhaseV1::CpuWritable, |b| b[0])
                .unwrap(),
            11
        );
        let executable = engine.seal_executable(executable).unwrap();
        assert_eq!(
            engine
                .with_bytes(
                    &executable,
                    SharedAllocationPhaseV1::ExecutableImmutable,
                    |b| b[0]
                )
                .unwrap(),
            44
        );
        let ordinary = engine.map_mutable(ordinary).unwrap();
        let ordinary = engine.unmap_mutable(ordinary).unwrap();
        let executable = engine.map_executable(executable).unwrap();
        let executable = engine.unmap_executable(executable).unwrap();
        engine
            .release(ordinary, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        engine
            .release(kernarg, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        engine
            .release(aql, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        engine
            .release(executable, SharedAllocationPhaseV1::ExecutableImmutable)
            .unwrap();
        assert_eq!(engine.retained_gpu_va_bytes, 0);
        assert_eq!(engine.backend.free_calls, 4);
        assert_eq!(engine.backend.release_va_calls, 4);
    }

    #[test]
    fn bounds_and_aql_shape_fail_before_native_mutation() {
        let mut engine = acquired();
        assert!(matches!(
            engine.allocate::<AqlQueueGttV1>(8193),
            Err(MemorySessionError::InvalidProfileSize(_))
        ));
        assert!(matches!(
            engine.allocate::<HostVisibleCoherentGttV1>(usize::MAX),
            Err(MemorySessionError::SizeOverflow) | Err(MemorySessionError::InvalidProfileSize(_))
        ));
        assert!(engine.backend.flags.is_empty());
        let mut tokens = Vec::new();
        for _ in 0..MAX_SHARED_GTT_ALLOCATIONS_V1 {
            tokens.push(engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap());
        }
        assert!(matches!(
            engine.allocate::<HostVisibleCoherentGttV1>(1),
            Err(MemorySessionError::SharedAllocationCapacity { .. })
        ));
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
        assert_eq!(tokens.len(), MAX_SHARED_GTT_ALLOCATIONS_V1);
    }

    #[test]
    fn overlap_and_kernel_output_substitution_quarantine_globally() {
        let mut overlap = acquired();
        overlap.backend.fixed_va = Some(0x2_0000);
        let _first = overlap.allocate::<HostVisibleCoherentGttV1>(4096).unwrap();
        assert!(overlap.allocate::<KernargGttV1>(4096).is_err());
        assert_eq!(overlap.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(overlap.backend.flags.len(), 1);

        let mut malformed = acquired();
        malformed.backend.corrupt_flags = true;
        assert!(malformed.allocate::<ExecutableGttV1>(4096).is_err());
        assert_eq!(malformed.phase(), SharedMemorySessionPhaseV1::Quarantined);
    }

    #[test]
    fn later_allocation_failure_revokes_use_of_prior_tokens() {
        let mut engine = acquired();
        let first = engine.allocate::<HostVisibleCoherentGttV1>(4096).unwrap();
        engine.backend.fail_operation = Some("alloc");
        assert!(engine.allocate::<KernargGttV1>(4096).is_err());
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert!(matches!(
            engine.with_bytes(&first, SharedAllocationPhaseV1::CpuWritable, |_| ()),
            Err(MemorySessionError::SharedSessionQuarantined)
        ));
    }

    #[test]
    fn ambiguous_map_seal_and_free_are_terminal_without_retry() {
        let mut map = acquired();
        let token = map.allocate::<HostVisibleCoherentGttV1>(4096).unwrap();
        map.backend.map_errno = true;
        assert!(map.map_mutable(token).is_err());
        assert_eq!(map.phase(), SharedMemorySessionPhaseV1::Quarantined);

        let mut seal = acquired();
        let token = seal.allocate::<ExecutableGttV1>(4096).unwrap();
        seal.backend.fail_operation = Some("protect_cpu_read_only");
        assert!(seal.seal_executable(token).is_err());
        assert_eq!(seal.phase(), SharedMemorySessionPhaseV1::Quarantined);

        let mut free = acquired();
        let token = free.allocate::<HostVisibleCoherentGttV1>(4096).unwrap();
        free.backend.fail_operation = Some("free");
        assert!(
            free.release(token, SharedAllocationPhaseV1::CpuWritable)
                .is_err()
        );
        assert_eq!(free.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(free.backend.free_calls, 1);

        let mut va_guard = acquired();
        let token = va_guard.allocate::<HostVisibleCoherentGttV1>(4096).unwrap();
        va_guard.backend.fail_operation = Some("release_va_reservation");
        assert!(
            va_guard
                .release(token, SharedAllocationPhaseV1::CpuWritable)
                .is_err()
        );
        assert_eq!(va_guard.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(va_guard.backend.free_calls, 1);
        assert_eq!(va_guard.backend.release_va_calls, 1);
    }
}
