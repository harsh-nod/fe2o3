//! Bounded shared KFD VM authority for typed host-visible GTT allocations.

use core::fmt;
use core::marker::PhantomData;
use std::os::fd::BorrowedFd;
use std::sync::atomic::Ordering;

use fe2o3_kfd_uapi::{
    KFD_ALLOC_MEMORY_FLAGS_AQL_QUEUE, KFD_ALLOC_MEMORY_FLAGS_DEVICE_LOCAL,
    KFD_ALLOC_MEMORY_FLAGS_DEVICE_LOCAL_PUBLIC, KFD_ALLOC_MEMORY_FLAGS_EXECUTABLE,
    KFD_ALLOC_MEMORY_FLAGS_HOST_VISIBLE_COHERENT, KFD_ALLOC_MEMORY_FLAGS_KERNARG,
    KfdAllocMemoryFlags,
};
use fe2o3_runtime_model::{
    AllocationGenerationV1, AllocationIdV1, DeviceIdentityStateV1, DeviceKeyV1, GpuVaRangeV1,
    MappingIdV1, MemoryAccessV1, MemoryAllocationKeyV1, MemoryAllocationSpecV1, MemoryCoherenceV1,
    MemoryKindV1, MemoryLifecycleStateV1, MemoryMappingKeyV1, MemoryPublicationIdV1,
    MemoryPublicationKeyV1, MemoryTransitionErrorV1, MemoryTransitionV1, ModelAdmissionStatusV1,
    ModelDeviceAdmissionV1, PartialOperationStatusV1, PartialProgressObservationV1,
    UntrustedAllocationHandleObservationV1, UntrustedVmHandleObservationV1, VaReservationIdV1,
    VaReservationKeyV1, VmIdV1, VmKeyV1,
};
use sha2::{Digest, Sha256};

use super::memory::{
    HOST_VISIBLE_MEMORY_PAGE_BYTES_V1, MemoryBackend, MemoryModelJournalSummary,
    MemorySessionError, NEXT_MODEL_VM_ID, begin_process_vm_attempt, finish_process_vm_attempt,
};
use crate::CheckedGfx942XnackMinusDevice;
use crate::queue::Gfx942DeviceContentDescriptorV1;

pub const MAX_SHARED_GTT_ALLOCATIONS_V1: usize = 64;
pub const MAX_SHARED_GTT_SINGLE_CPU_BYTES_V1: u64 = 1 << 31;
pub const MAX_SHARED_GTT_GPU_VA_BYTES_V1: u64 = 8 << 30;
pub const MIN_AQL_QUEUE_BYTES_V1: u64 = 4_096;
pub const MAX_AQL_QUEUE_BYTES_V1: u64 = 1 << 31;
pub const MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1: usize = 64;
pub const MAX_GFX942_DEVICE_MEMORY_BYTES_V1: u64 = 192 << 30;
pub const MAX_GFX942_DEVICE_MEMORY_ALIGNMENT_V1: u64 = HOST_VISIBLE_MEMORY_PAGE_BYTES_V1;

/// Canonical contract for bounded device-local allocation leases.
pub const GFX942_DEVICE_MEMORY_LEASE_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-device-memory-lease-r1-v1\n",
    "device_profile_sha256=e12ea33b259666e7928612403109640b03b0d637b893a2c15b87d17a4211c8de\n",
    "kfd_device_memory_schema_sha256=8592027abc19962181c29b42962909e152d4ef4194036a1659dc601992cf709a\n",
    "target=gfx942:xnack-,SPX/NPS1,KFD-1.18,one-selected-current-device-and-vm\n",
    "profile=device-local-vram-hbm-writable:0x80000001\n",
    "bounds=allocation-records:64,retained-bytes:206158430208,alignment-power-of-two-max:4096,page:4096\n",
    "lifecycle=linear-non-clone-unmapped-to-mapped-to-unmapped-to-released\n",
    "mapping=exact-one-selected-gpu,no-peer,no-retry-after-native-attempt\n",
    "authority=retained-kfd-render-vm-device-and-allocation-generation,no-public-handle-va-pointer-or-fd\n",
    "currentness=before-and-after-every-native-transition,contracted-composite\n",
    "failure=retain-reservation-and-possible-handle,global-quarantine-after-ambiguous-native-result,no-drop-cleanup\n",
    "model=dedicated-bounded-linear-engine,no-runtime-memory-model-projection\n",
    "dispatch-bridge=crate-private-exact-set-consumption-of-every-live-mapped-lease,device-vm-allocation-generation-and-private-va-facts,no-public-mint\n",
    "excluded=cpu-map,initialization-or-copy-mint,alias-or-effect-proof,quiescence,kernel-address-export,public-launch,hardware-completion,peer-map\n",
);

pub const GFX942_DEVICE_MEMORY_LEASE_MANIFEST_SHA256_V1: &str =
    "29cbf694e5cbd1ae30258d85653042b23178b0a8d1676e2a1560df22cbbdfed3";

pub const GFX942_DEVICE_MEMORY_LEASE_MANIFEST_SHA256_BYTES_V1: [u8; 32] = [
    0x29, 0xcb, 0xf6, 0x94, 0xe5, 0xcb, 0xd1, 0xae, 0x30, 0x25, 0x8d, 0x85, 0x65, 0x30, 0x42, 0xb2,
    0x31, 0x78, 0xb0, 0xa8, 0xd1, 0x67, 0x6e, 0x2a, 0x15, 0x60, 0xdf, 0x22, 0xcb, 0xbd, 0xfe, 0xd3,
];

/// Canonical contract for CPU initialization of public device-local storage.
pub const GFX942_DEVICE_MEMORY_INITIALIZATION_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-device-memory-initialization-r1-v1\n",
    "uapi_profile_sha256=3b9f1164fc74672f019cbd092c142b4a6d830920424b87282cfdf5b8f50afd81\n",
    "target=gfx942:xnack-,SPX/NPS1,KFD-1.18,one-selected-current-device-and-vm\n",
    "allocation=device-local-vram-hbm-public-writable:0xa0000001,separate-from-uninitialized:0x80000001\n",
    "source=owned-nonempty-byte-slice,exact-length-and-sha256-must-match-content-descriptor-before-native-allocation\n",
    "cpu-map=returned-mmap-offset,prot-none-then-dontfork-then-read-write,whole-request-copy,readback-sha256,explicit-munmap-before-gpu-map\n",
    "authority=linear-initialized-mapped-lease,private-allocation-device-vm-generation-and-address,public-layout-and-content-descriptor-only\n",
    "failure=preflight-no-side-effects,post-allocation-or-mapping-failure-quarantines-session,no-retry-or-drop-cleanup\n",
    "excluded=caller-asserted-initialization,callback-content-claim,persistent-cpu-mapping,gpu-address,copy-queue,execution,hardware-coherence-proof\n",
);

/// SHA-256 of [`GFX942_DEVICE_MEMORY_INITIALIZATION_MANIFEST_V1`].
pub const GFX942_DEVICE_MEMORY_INITIALIZATION_MANIFEST_SHA256_V1: &str =
    "6b9eb7763e83ec2bdf4f19b62af05451f3fa288ae50a6c47ddba8e8778071162";

/// Canonical contract for the bounded multi-allocation R2 adapter.
pub const SHARED_GTT_MEMORY_PROFILE_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-shared-gtt-memory-r4-v1\n",
    "base_memory_profile_sha256=7bdca672c4921ee56a850d41040045f4a8fbe5a20176628a4ea982dd80fbe8ec\n",
    "kfd_memory_schema_sha256=e2d6987b7c8e61a405b2f775d5d004f458a096241459e4cfdf90bd4497f4d58a\n",
    "profiles=host-visible-coherent:0x84000002,kernarg:0x86000002,aql-queue:0x8e000002,executable:0xc4000002\n",
    "bounds=allocations:64,single-cpu-bytes:2147483648,total-gpu-va-bytes:8589934592,page:4096\n",
    "aql=logical-ring:power-of-two-4096..2147483648,gpu-va:checked-double,cpu-vma:single-physical-copy\n",
    "va_allocator=kernel-selected-prot-none-guards-retained-until-successful-free,checked-nonoverlap\n",
    "authority=one-retained-kfd-render-vm,multiple-linear-redacted-tokens,no-fd-handle-va-or-pointer-export\n",
    "queue-bridge=crate-private-role-marked-linear-mapped-capabilities,ring-control-eop-cwsr-completion-signal-dispatch-code-and-dispatch-kernarg-roles,private-va-mapping-publication-facts,no-public-mint\n",
    "device-dispatch-bridge=exact-complete-distinct-set-of-every-live-mapped-c3-lease-required-before-model-transfer,actual-linear-lease-retained,private-address-facts\n",
    "queue-gtt-policy=ring:aql-special,control-and-completion-signals:host-visible-coherent,eop-and-cwsr:executable,not-rocr-equivalence\n",
    "cpu_views=closure-scoped-before-map;mapped-completion-access-only-through-slot-bounded-acquire-observe-and-release-reset,no-safe-borrow-escape\n",
    "completion-bridge=exact-retained-host-coherent-mapping,private-64-byte-slot-index,backend-atomic-i64-acquire-observe-and-release-reset,currentness-sandwiched\n",
    "executable=cpu-construction-rw-to-vma-read-only-before-gpu-map,gpu-writable-flag-remains-contracted\n",
    "failure=global-quarantine-after-started-or-ambiguous-native-transaction,no-drop-cleanup-or-retry\n",
    "fork=current-base-contract,prot-none-dontfork-before-rw,no-raw-fork-clone-during-setup\n",
    "model=completion-only-append-journal,profile-kind-and-gpu-va-span,no-cpu-vma-or-seal-transition\n",
    "proof=no-concrete-verus-refinement,kernel-and-model-coupling-contracted,hostile-tests-only\n",
    "excluded=queue-ioctl,doorbell,packet-publication,dispatch,completion-policy-or-aggregation,userptr,peer-map\n",
);

pub const SHARED_GTT_MEMORY_PROFILE_SHA256_V1: &str =
    "032e68de9b493deb70326fe8e65bb90248ff3a0d02d6a77f3e939df15262b33e";

pub const SHARED_GTT_MEMORY_PROFILE_SHA256_BYTES_V1: [u8; 32] = [
    0x03, 0x2e, 0x68, 0xde, 0x9b, 0x49, 0x3d, 0xeb, 0x70, 0x32, 0x6f, 0xe8, 0xe6, 0x5b, 0xb9, 0x02,
    0x48, 0xff, 0x3a, 0x0d, 0x02, 0xd6, 0xa7, 0x7f, 0x3e, 0x93, 0x9d, 0xf1, 0x52, 0x62, 0xb3, 0x3e,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SharedGttProfileV1 {
    HostVisibleCoherent,
    Kernarg,
    AqlQueue,
    Executable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942DeviceMemoryLayoutV1 {
    requested_bytes: u64,
    backing_bytes: u64,
    alignment: u64,
    uapi_flags: u32,
}

impl Gfx942DeviceMemoryLayoutV1 {
    pub const fn requested_bytes(self) -> u64 {
        self.requested_bytes
    }

    pub const fn backing_bytes(self) -> u64 {
        self.backing_bytes
    }

    pub const fn alignment(self) -> u64 {
        self.alignment
    }

    pub const fn uapi_flags(self) -> u32 {
        self.uapi_flags
    }
}

mod device_memory_state {
    pub trait Sealed {}
}

pub trait Gfx942DeviceMemoryStateV1: device_memory_state::Sealed + 'static {}

pub enum Gfx942DeviceMemoryUnmappedV1 {}
pub enum Gfx942DeviceMemoryMappedV1 {}

impl device_memory_state::Sealed for Gfx942DeviceMemoryUnmappedV1 {}
impl device_memory_state::Sealed for Gfx942DeviceMemoryMappedV1 {}
impl Gfx942DeviceMemoryStateV1 for Gfx942DeviceMemoryUnmappedV1 {}
impl Gfx942DeviceMemoryStateV1 for Gfx942DeviceMemoryMappedV1 {}

/// Linear ownership of one bounded gfx942 device-local allocation.
///
/// The lease intentionally exposes layout only. A mapped lease still has no
/// numeric GPU address; a later queue/dispatch binding must consume it before
/// such an address can exist in a launch authority.
///
/// ```compile_fail
/// use fe2o3_kfd::{Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryUnmappedV1};
///
/// fn cannot_copy_or_extract_address(
///     lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>,
/// ) {
///     let _copy = lease.clone();
///     let _address = lease.gpu_va();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kfd::{
///     Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryMappedV1,
///     SharedGttMemorySessionV1,
/// };
///
/// fn mapped_memory_cannot_be_freed(
///     session: &mut SharedGttMemorySessionV1,
///     lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
/// ) {
///     session.release_gfx942_device_memory(lease).unwrap();
/// }
/// ```
#[must_use = "device-memory authority must be transitioned or explicitly released"]
pub struct Gfx942DeviceMemoryLeaseV1<S: Gfx942DeviceMemoryStateV1> {
    id: u64,
    generation: u64,
    device: DeviceKeyV1,
    vm: VmKeyV1,
    layout: Gfx942DeviceMemoryLayoutV1,
    marker: PhantomData<S>,
}

/// Linear mapped device-local storage whose exact byte extent was written and
/// read back through an owned CPU mapping before GPU publication.
///
/// The authority binds the private allocation generation to one content
/// descriptor. It has no public constructor, address, handle, pointer, or
/// mutable-byte accessor and is neither `Clone` nor `Copy`.
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942InitializedDeviceMemoryV1;
///
/// fn cannot_clone(value: Gfx942InitializedDeviceMemoryV1) {
///     let _ = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kfd::Gfx942InitializedDeviceMemoryV1;
///
/// fn cannot_extract_native_authority(value: Gfx942InitializedDeviceMemoryV1) {
///     let _ = value.lease;
/// }
/// ```
#[must_use = "initialized device-memory authority must be retained or explicitly released"]
pub struct Gfx942InitializedDeviceMemoryV1 {
    lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    content: Gfx942DeviceContentDescriptorV1,
}

impl fmt::Debug for Gfx942InitializedDeviceMemoryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942InitializedDeviceMemoryV1")
            .field("layout", &self.lease.layout())
            .field("content", &self.content)
            .finish_non_exhaustive()
    }
}

impl Gfx942InitializedDeviceMemoryV1 {
    /// Returns the checked layout without native identities or addresses.
    pub const fn layout(&self) -> Gfx942DeviceMemoryLayoutV1 {
        self.lease.layout()
    }

    /// Returns the exact descriptor verified against mapped bytes.
    pub const fn content(&self) -> Gfx942DeviceContentDescriptorV1 {
        self.content
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
        Gfx942DeviceContentDescriptorV1,
    ) {
        (self.lease, self.content)
    }
}

impl<S: Gfx942DeviceMemoryStateV1> fmt::Debug for Gfx942DeviceMemoryLeaseV1<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942DeviceMemoryLeaseV1")
            .field("layout", &self.layout)
            .finish_non_exhaustive()
    }
}

impl<S: Gfx942DeviceMemoryStateV1> Gfx942DeviceMemoryLeaseV1<S> {
    pub const fn layout(&self) -> Gfx942DeviceMemoryLayoutV1 {
        self.layout
    }

    fn retag<T: Gfx942DeviceMemoryStateV1>(self) -> Gfx942DeviceMemoryLeaseV1<T> {
        Gfx942DeviceMemoryLeaseV1 {
            id: self.id,
            generation: self.generation,
            device: self.device,
            vm: self.vm,
            layout: self.layout,
            marker: PhantomData,
        }
    }
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
pub(crate) enum AqlCompletionSignalResourceRoleV1 {}
pub(crate) enum AqlDispatchCodeResourceRoleV1 {}
pub(crate) enum AqlDispatchKernargResourceRoleV1 {}

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
define_resource_role!(AqlCompletionSignalResourceRoleV1);
define_resource_role!(AqlDispatchCodeResourceRoleV1);
define_resource_role!(AqlDispatchKernargResourceRoleV1);

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

/// Private identity and numeric facts retained beside one mapped C3 lease.
///
/// The facts have no public accessor. The containing linear authority owns the
/// exact lease and is the only C3 form accepted by queue-model transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Gfx942DeviceMemoryDispatchFactsV1 {
    id: u64,
    generation: u64,
    device: DeviceKeyV1,
    vm: VmKeyV1,
    gpu_va: u64,
    layout: Gfx942DeviceMemoryLayoutV1,
}

impl Gfx942DeviceMemoryDispatchFactsV1 {
    pub(crate) const fn identity(&self) -> (u64, u64, DeviceKeyV1, VmKeyV1) {
        (self.id, self.generation, self.device, self.vm)
    }

    pub(crate) const fn vm(&self) -> VmKeyV1 {
        self.vm
    }

    pub(crate) fn checked_gpu_subrange(
        &self,
        offset: u64,
        byte_len: u64,
        alignment: u64,
    ) -> Option<u64> {
        if byte_len == 0
            || alignment == 0
            || !alignment.is_power_of_two()
            || !offset.is_multiple_of(alignment)
        {
            return None;
        }
        let end = offset.checked_add(byte_len)?;
        if end > self.layout.requested_bytes {
            return None;
        }
        let address = self.gpu_va.checked_add(offset)?;
        address.is_multiple_of(alignment).then_some(address)
    }
}

/// Linear private bridge from one actual mapped C3 lease into dispatch.
pub(crate) struct Gfx942DeviceMemoryDispatchAuthorityV1 {
    lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    facts: Gfx942DeviceMemoryDispatchFactsV1,
}

impl Gfx942DeviceMemoryDispatchAuthorityV1 {
    pub(crate) const fn facts(&self) -> &Gfx942DeviceMemoryDispatchFactsV1 {
        &self.facts
    }

    pub(crate) fn into_lease(self) -> Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1> {
        self.lease
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeviceMemoryPhaseV1 {
    Ambiguous,
    Unmapped,
    Mapped,
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

struct DeviceMemoryRecord<B: MemoryBackend> {
    id: u64,
    generation: u64,
    device: DeviceKeyV1,
    vm: VmKeyV1,
    layout: Gfx942DeviceMemoryLayoutV1,
    gpu_va: u64,
    mmap_offset: u64,
    reservation: Option<B::Reservation>,
    mapping: Option<B::Mapping>,
    handle: Option<u64>,
    free_attempted: bool,
    phase: DeviceMemoryPhaseV1,
}

struct SharedMemoryEngine<B: MemoryBackend> {
    backend: B,
    phase: SharedMemorySessionPhaseV1,
    allocations: Vec<SharedAllocationRecord<B>>,
    next_id: u64,
    retained_gpu_va_bytes: u64,
    device_memory: Vec<DeviceMemoryRecord<B>>,
    next_device_memory_id: u64,
    retained_device_memory_bytes: u64,
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
            device_memory: Vec::new(),
            next_device_memory_id: 1,
            retained_device_memory_bytes: 0,
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
        }) || self.device_memory.iter().any(|record| {
            record.phase != DeviceMemoryPhaseV1::Released
                && ranges_overlap(
                    gpu_va,
                    layout.gpu_va_bytes,
                    record.gpu_va,
                    record.layout.backing_bytes,
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
        }) || self.device_memory.iter().any(|record| {
            record.phase != DeviceMemoryPhaseV1::Released
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

    fn allocate_device_memory(
        &mut self,
        device: DeviceKeyV1,
        vm: VmKeyV1,
        requested_bytes: u64,
        alignment: u64,
    ) -> Result<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>, MemorySessionError> {
        self.allocate_device_memory_with_flags(
            device,
            vm,
            requested_bytes,
            alignment,
            KfdAllocMemoryFlags::DEVICE_LOCAL,
        )
    }

    fn allocate_device_memory_with_flags(
        &mut self,
        device: DeviceKeyV1,
        vm: VmKeyV1,
        requested_bytes: u64,
        alignment: u64,
        flags: KfdAllocMemoryFlags,
    ) -> Result<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>, MemorySessionError> {
        self.require_active()?;
        if vm.device != device {
            return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
        }
        let layout = device_memory_layout(requested_bytes, alignment, flags)?;
        if self.device_memory.len() >= MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1 {
            return Err(MemorySessionError::DeviceMemoryAllocationCapacity {
                maximum: MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1,
            });
        }
        let new_total = self
            .retained_device_memory_bytes
            .checked_add(layout.backing_bytes)
            .ok_or(MemorySessionError::SizeOverflow)?;
        if new_total > MAX_GFX942_DEVICE_MEMORY_BYTES_V1 {
            return Err(MemorySessionError::DeviceMemoryByteCapacity {
                maximum_bytes: MAX_GFX942_DEVICE_MEMORY_BYTES_V1,
            });
        }
        let id = self.next_device_memory_id;
        let next_id = id.checked_add(1).ok_or(MemorySessionError::SizeOverflow)?;
        let reservation_bytes =
            usize::try_from(layout.backing_bytes).map_err(|_| MemorySessionError::SizeOverflow)?;

        self.check_currentness()?;
        let reservation = match self.backend.reserve_va(reservation_bytes) {
            Ok(reservation) => reservation,
            Err(error) => return self.quarantine(error),
        };
        let gpu_va = B::reservation_address(&reservation);
        let index = self.device_memory.len();
        self.device_memory.push(DeviceMemoryRecord {
            id,
            generation: 1,
            device,
            vm,
            layout,
            gpu_va,
            mmap_offset: 0,
            reservation: Some(reservation),
            mapping: None,
            handle: None,
            free_attempted: false,
            phase: DeviceMemoryPhaseV1::Ambiguous,
        });
        self.next_device_memory_id = next_id;
        self.retained_device_memory_bytes = new_total;

        if let Err(error) =
            validate_gpu_va_range(gpu_va, layout.backing_bytes, self.backend.gpuvm_aperture())
        {
            return self.quarantine(error);
        }
        if !gpu_va.is_multiple_of(layout.alignment) {
            return self.quarantine(MemorySessionError::AddressNotPageAligned);
        }
        if self.allocations.iter().any(|record| {
            record.phase != SharedAllocationPhaseV1::Released
                && ranges_overlap(
                    gpu_va,
                    layout.backing_bytes,
                    record.gpu_va,
                    record.layout.gpu_va_bytes,
                )
        }) || self.device_memory[..index].iter().any(|record| {
            record.phase != DeviceMemoryPhaseV1::Released
                && ranges_overlap(
                    gpu_va,
                    layout.backing_bytes,
                    record.gpu_va,
                    record.layout.backing_bytes,
                )
        }) {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "overlapping device-memory GPU VA reservation",
            ));
        }

        let outcome = self.backend.alloc(gpu_va, layout.backing_bytes, flags);
        let args = outcome.value;
        self.device_memory[index].handle = (args.handle != 0).then_some(args.handle);
        self.device_memory[index].mmap_offset = args.mmap_offset;
        if let Err(error) = outcome.result {
            return self.quarantine(error);
        }
        if args.va_addr != gpu_va
            || args.size != layout.backing_bytes
            || args.gpu_id != self.backend.gpu_id()
            || args.flags != flags.bits()
            || args.handle == 0
            || args.mmap_offset == 0
            || !args
                .mmap_offset
                .is_multiple_of(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1)
        {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "device-local ALLOC_MEMORY_OF_GPU output",
            ));
        }
        if self.allocations.iter().any(|record| {
            record.phase != SharedAllocationPhaseV1::Released
                && (record.handle == Some(args.handle) || record.mmap_offset == args.mmap_offset)
        }) || self.device_memory[..index].iter().any(|record| {
            record.phase != DeviceMemoryPhaseV1::Released
                && (record.handle == Some(args.handle) || record.mmap_offset == args.mmap_offset)
        }) {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "device-memory handle or mmap-offset collision",
            ));
        }
        self.check_currentness()?;
        self.device_memory[index].phase = DeviceMemoryPhaseV1::Unmapped;
        Ok(Gfx942DeviceMemoryLeaseV1 {
            id,
            generation: 1,
            device,
            vm,
            layout,
            marker: PhantomData,
        })
    }

    fn initialize_public_device_memory(
        &mut self,
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>,
        bytes: &[u8],
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<Gfx942InitializedDeviceMemoryV1, MemorySessionError> {
        let expected_len =
            u64::try_from(bytes.len()).map_err(|_| MemorySessionError::SizeOverflow)?;
        let source_sha256: [u8; 32] = Sha256::digest(bytes).into();
        if lease.layout.uapi_flags != KFD_ALLOC_MEMORY_FLAGS_DEVICE_LOCAL_PUBLIC
            || content.byte_len() != expected_len
            || content.sha256() != source_sha256
            || expected_len != lease.layout.requested_bytes
        {
            return self.quarantine(MemorySessionError::DeviceContentMismatch);
        }
        let index = self.device_memory_index(&lease, DeviceMemoryPhaseV1::Unmapped)?;
        self.check_currentness()?;
        let mapping_bytes = usize::try_from(self.device_memory[index].layout.backing_bytes)
            .map_err(|_| MemorySessionError::SizeOverflow)?;
        let mmap_offset = self.device_memory[index].mmap_offset;
        let mapping_result = {
            let (backend, records) = (&mut self.backend, &mut self.device_memory);
            let reservation = records[index]
                .reservation
                .as_mut()
                .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
            backend.map_cpu(reservation, mmap_offset, mapping_bytes, true)
        };
        let mapping = match mapping_result {
            Ok(mapping) => mapping,
            Err(error) => return self.quarantine(error),
        };
        self.device_memory[index].mapping = Some(mapping);
        let prepare_result = {
            let (backend, records) = (&mut self.backend, &mut self.device_memory);
            let mapping = records[index]
                .mapping
                .as_mut()
                .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
            backend.prepare_cpu_mapping(mapping)
        };
        if let Err(error) = prepare_result {
            return self.quarantine(error);
        }
        let observed: [u8; 32] = {
            let record = &mut self.device_memory[index];
            let mapping = record
                .mapping
                .as_mut()
                .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
            B::with_bytes_mut(mapping, bytes.len(), |mapped| mapped.copy_from_slice(bytes));
            B::with_bytes(mapping, bytes.len(), |mapped| Sha256::digest(mapped).into())
        };
        if observed != content.sha256() {
            return self.quarantine(MemorySessionError::DeviceContentMismatch);
        }
        let unmap_result = {
            let (backend, records) = (&mut self.backend, &mut self.device_memory);
            let mapping = records[index]
                .mapping
                .as_mut()
                .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
            backend.unmap_cpu(mapping)
        };
        if let Err(error) = unmap_result {
            return self.quarantine(error);
        }
        self.device_memory[index].mapping = None;
        self.check_currentness()?;
        let lease = self.map_device_memory(lease)?;
        Ok(Gfx942InitializedDeviceMemoryV1 { lease, content })
    }

    fn device_memory_index<S: Gfx942DeviceMemoryStateV1>(
        &self,
        lease: &Gfx942DeviceMemoryLeaseV1<S>,
        expected: DeviceMemoryPhaseV1,
    ) -> Result<usize, MemorySessionError> {
        self.require_active()?;
        self.device_memory
            .iter()
            .position(|record| {
                record.id == lease.id
                    && record.generation == lease.generation
                    && record.device == lease.device
                    && record.vm == lease.vm
                    && record.layout == lease.layout
                    && record.phase == expected
            })
            .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)
    }

    fn validate_dispatch_device_memory_set(
        &self,
        authorities: &[Gfx942DeviceMemoryDispatchAuthorityV1],
        expected_device: DeviceKeyV1,
        expected_vm: VmKeyV1,
    ) -> Result<(), MemorySessionError> {
        self.require_active()?;
        let retained: Vec<_> = self
            .device_memory
            .iter()
            .filter(|record| record.phase != DeviceMemoryPhaseV1::Released)
            .collect();
        if retained.len() != authorities.len() {
            return Err(MemorySessionError::DeviceMemoryQueueBindingRequired);
        }
        for (index, authority) in authorities.iter().enumerate() {
            let facts = authority.facts();
            if facts.device != expected_device
                || facts.vm != expected_vm
                || authorities[..index]
                    .iter()
                    .any(|prior| prior.facts().identity() == facts.identity())
                || !retained.iter().any(|record| {
                    record.id == facts.id
                        && record.generation == facts.generation
                        && record.device == facts.device
                        && record.vm == facts.vm
                        && record.gpu_va == facts.gpu_va
                        && record.layout == facts.layout
                        && record.phase == DeviceMemoryPhaseV1::Mapped
                        && record.handle.is_some()
                        && record.reservation.is_some()
                })
            {
                return Err(MemorySessionError::DeviceMemoryQueueBindingRequired);
            }
        }
        Ok(())
    }

    fn map_device_memory(
        &mut self,
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>,
    ) -> Result<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>, MemorySessionError> {
        let index = self.device_memory_index(&lease, DeviceMemoryPhaseV1::Unmapped)?;
        if self.device_memory[index].mapping.is_some() {
            return self.quarantine(MemorySessionError::InvalidDeviceMemoryAuthority);
        }
        self.check_currentness()?;
        let handle = self.device_memory[index]
            .handle
            .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
        self.device_memory[index].phase = DeviceMemoryPhaseV1::Ambiguous;
        let outcome = self.backend.map_gpu(handle, 0);
        if outcome.value > 1 {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "device-memory MAP_MEMORY_TO_GPU cumulative n_success",
            ));
        }
        if let Err(error) = outcome.result {
            return self.quarantine(error);
        }
        if outcome.value != 1 {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "device-memory MAP_MEMORY_TO_GPU full prefix",
            ));
        }
        self.check_currentness()?;
        self.device_memory[index].phase = DeviceMemoryPhaseV1::Mapped;
        Ok(lease.retag())
    }

    fn unmap_device_memory(
        &mut self,
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>, MemorySessionError> {
        let index = self.device_memory_index(&lease, DeviceMemoryPhaseV1::Mapped)?;
        self.check_currentness()?;
        let handle = self.device_memory[index]
            .handle
            .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
        self.device_memory[index].phase = DeviceMemoryPhaseV1::Ambiguous;
        let outcome = self.backend.unmap_gpu(handle, 0);
        if outcome.value > 1 {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "device-memory UNMAP_MEMORY_FROM_GPU cumulative n_success",
            ));
        }
        if let Err(error) = outcome.result {
            return self.quarantine(error);
        }
        if outcome.value != 1 {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "device-memory UNMAP_MEMORY_FROM_GPU full prefix",
            ));
        }
        self.check_currentness()?;
        self.device_memory[index].phase = DeviceMemoryPhaseV1::Unmapped;
        Ok(lease.retag())
    }

    fn release_device_memory(
        &mut self,
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>,
    ) -> Result<(), MemorySessionError> {
        let index = self.device_memory_index(&lease, DeviceMemoryPhaseV1::Unmapped)?;
        self.check_currentness()?;
        if self.device_memory[index].mapping.is_some() {
            return self.quarantine(MemorySessionError::InvalidDeviceMemoryAuthority);
        }
        if self.device_memory[index].free_attempted {
            return self.quarantine(MemorySessionError::KernelResultMalformed(
                "device-memory FREE_MEMORY_OF_GPU exactly-once",
            ));
        }
        let handle = self.device_memory[index]
            .handle
            .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
        self.device_memory[index].free_attempted = true;
        self.device_memory[index].phase = DeviceMemoryPhaseV1::Ambiguous;
        if let Err(error) = self.backend.free(handle) {
            return self.quarantine(error);
        }
        self.device_memory[index].handle = None;
        self.check_currentness()?;
        let release_result = {
            let (backend, records) = (&mut self.backend, &mut self.device_memory);
            let reservation = records[index]
                .reservation
                .as_mut()
                .ok_or(MemorySessionError::InvalidDeviceMemoryAuthority)?;
            backend.release_va_reservation(reservation)
        };
        if let Err(error) = release_result {
            return self.quarantine(error);
        }
        self.device_memory[index].reservation = None;
        self.check_currentness()?;
        self.device_memory[index].phase = DeviceMemoryPhaseV1::Released;
        self.retained_device_memory_bytes = self
            .retained_device_memory_bytes
            .checked_sub(lease.layout.backing_bytes)
            .ok_or(MemorySessionError::KernelResultMalformed(
                "retained device-memory accounting",
            ))?;
        Ok(())
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

    fn observe_aql_counters<P: MutableGpuGttProfileV1>(
        &mut self,
        token: &mut SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>,
    ) -> Result<(u64, u64), MemorySessionError> {
        self.check_currentness()?;
        let index = self.index(token, SharedAllocationPhaseV1::GpuAccessibleMutable)?;
        let requested = self.allocations[index].layout.requested_bytes;
        let mapping = self.allocations[index]
            .mapping
            .as_mut()
            .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
        let value = B::observe_aql_counters(mapping, requested)?;
        self.check_currentness()?;
        Ok(value)
    }

    fn fetch_add_aql_write<P: MutableGpuGttProfileV1>(
        &mut self,
        token: &mut SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>,
        increment: u64,
    ) -> Result<u64, MemorySessionError> {
        self.check_currentness()?;
        let index = self.index(token, SharedAllocationPhaseV1::GpuAccessibleMutable)?;
        let requested = self.allocations[index].layout.requested_bytes;
        let mapping = self.allocations[index]
            .mapping
            .as_mut()
            .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
        let value = B::fetch_add_aql_write(mapping, requested, increment)?;
        self.check_currentness()?;
        Ok(value)
    }

    fn write_aql_slot<P: MutableGpuGttProfileV1>(
        &mut self,
        token: &mut SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>,
        slot_index: u32,
        packet: &[u8; 64],
    ) -> Result<(), MemorySessionError> {
        self.check_currentness()?;
        let index = self.index(token, SharedAllocationPhaseV1::GpuAccessibleMutable)?;
        let requested = self.allocations[index].layout.requested_bytes;
        let mapping = self.allocations[index]
            .mapping
            .as_mut()
            .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
        B::write_aql_slot(mapping, requested, slot_index, packet)?;
        self.check_currentness()
    }

    fn publish_aql_header<P: MutableGpuGttProfileV1>(
        &mut self,
        token: &mut SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>,
        slot_index: u32,
        header: u16,
    ) -> Result<(), MemorySessionError> {
        self.check_currentness()?;
        let index = self.index(token, SharedAllocationPhaseV1::GpuAccessibleMutable)?;
        let requested = self.allocations[index].layout.requested_bytes;
        let mapping = self.allocations[index]
            .mapping
            .as_mut()
            .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
        B::publish_aql_header(mapping, requested, slot_index, header)?;
        self.check_currentness()
    }

    fn observe_completion_signal(
        &mut self,
        token: &mut SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>,
        slot_index: u32,
    ) -> Result<fe2o3_aql::AqlCompletionObservationV1, MemorySessionError> {
        self.check_currentness()?;
        let index = self.index(token, SharedAllocationPhaseV1::GpuAccessibleMutable)?;
        let requested = self.allocations[index].layout.requested_bytes;
        let mapping = self.allocations[index]
            .mapping
            .as_mut()
            .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
        let observation = B::observe_completion_signal_acquire(mapping, requested, slot_index)?;
        self.check_currentness()?;
        Ok(observation)
    }

    fn reset_completion_signal(
        &mut self,
        token: &mut SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>,
        slot_index: u32,
    ) -> Result<(), MemorySessionError> {
        self.check_currentness()?;
        let index = self.index(token, SharedAllocationPhaseV1::GpuAccessibleMutable)?;
        let requested = self.allocations[index].layout.requested_bytes;
        let mapping = self.allocations[index]
            .mapping
            .as_mut()
            .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
        B::reset_completion_signal_release(mapping, requested, slot_index)?;
        self.check_currentness()
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

fn validate_initialization_source(
    bytes: &[u8],
    content: Gfx942DeviceContentDescriptorV1,
) -> Result<u64, MemorySessionError> {
    let byte_len = u64::try_from(bytes.len()).map_err(|_| MemorySessionError::SizeOverflow)?;
    let sha256: [u8; 32] = Sha256::digest(bytes).into();
    if byte_len == 0 || content.byte_len() != byte_len || content.sha256() != sha256 {
        return Err(MemorySessionError::DeviceContentMismatch);
    }
    Ok(byte_len)
}

fn device_memory_layout(
    requested_bytes: u64,
    alignment: u64,
    flags: KfdAllocMemoryFlags,
) -> Result<Gfx942DeviceMemoryLayoutV1, MemorySessionError> {
    if requested_bytes == 0 || requested_bytes > MAX_GFX942_DEVICE_MEMORY_BYTES_V1 {
        return Err(MemorySessionError::InvalidDeviceMemorySize);
    }
    if alignment == 0
        || !alignment.is_power_of_two()
        || alignment > MAX_GFX942_DEVICE_MEMORY_ALIGNMENT_V1
    {
        return Err(MemorySessionError::InvalidDeviceMemoryAlignment);
    }
    let backing_bytes = requested_bytes
        .checked_add(HOST_VISIBLE_MEMORY_PAGE_BYTES_V1 - 1)
        .ok_or(MemorySessionError::SizeOverflow)?
        / HOST_VISIBLE_MEMORY_PAGE_BYTES_V1
        * HOST_VISIBLE_MEMORY_PAGE_BYTES_V1;
    if backing_bytes > MAX_GFX942_DEVICE_MEMORY_BYTES_V1 {
        return Err(MemorySessionError::InvalidDeviceMemorySize);
    }
    if flags != KfdAllocMemoryFlags::DEVICE_LOCAL
        && flags != KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC
    {
        return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
    }
    Ok(Gfx942DeviceMemoryLayoutV1 {
        requested_bytes,
        backing_bytes,
        alignment,
        uapi_flags: flags.bits(),
    })
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

    pub fn retained_device_memory_lease_count(&self) -> usize {
        self.engine
            .device_memory
            .iter()
            .filter(|record| record.phase != DeviceMemoryPhaseV1::Released)
            .count()
    }

    pub const fn retained_device_memory_bytes(&self) -> u64 {
        self.engine.retained_device_memory_bytes
    }

    /// Allocates uninitialized writable device-local VRAM/HBM.
    ///
    /// The resulting lease is not GPU mapped and carries no numeric address.
    /// This operation grants neither initialization nor copy authority.
    pub fn allocate_gfx942_device_memory(
        &mut self,
        requested_bytes: u64,
        alignment: u64,
    ) -> Result<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>, MemorySessionError> {
        self.engine.allocate_device_memory(
            self.model_device.model_key(),
            self.vm,
            requested_bytes,
            alignment,
        )
    }

    /// Allocates CPU-visible device-local storage, writes one exact owned byte
    /// extent, verifies the mapped bytes, removes CPU access, and maps the
    /// allocation to the selected GPU.
    ///
    /// The descriptor is data only. It cannot mint authority unless its exact
    /// length and digest match both the caller bytes and the bytes read back
    /// from this allocation's owned mapping. The ordinary uninitialized
    /// device-local path remains a distinct API and flag profile.
    pub fn initialize_gfx942_device_memory(
        &mut self,
        bytes: Box<[u8]>,
        alignment: u64,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<Gfx942InitializedDeviceMemoryV1, MemorySessionError> {
        let requested_bytes = validate_initialization_source(&bytes, content)?;
        let lease = self.engine.allocate_device_memory_with_flags(
            self.model_device.model_key(),
            self.vm,
            requested_bytes,
            alignment,
            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
        )?;
        self.engine
            .initialize_public_device_memory(lease, &bytes, content)
    }

    /// Maps one exact device-local lease to the selected GPU only.
    ///
    /// Mapping does not expose a GPU address or bind this storage to a queue,
    /// packet, dispatch generation, copy operation, or completion.
    pub fn map_gfx942_device_memory(
        &mut self,
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>,
    ) -> Result<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>, MemorySessionError> {
        self.engine.map_device_memory(lease)
    }

    pub fn unmap_gfx942_device_memory(
        &mut self,
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>, MemorySessionError> {
        self.engine.unmap_device_memory(lease)
    }

    /// Frees one unmapped lease exactly once, then releases its VA guard.
    pub fn release_gfx942_device_memory(
        &mut self,
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>,
    ) -> Result<(), MemorySessionError> {
        self.engine.release_device_memory(lease)
    }

    /// Unmaps and releases one initialized device-local allocation.
    pub fn release_initialized_gfx942_device_memory(
        &mut self,
        initialized: Gfx942InitializedDeviceMemoryV1,
    ) -> Result<(), MemorySessionError> {
        let (lease, _) = initialized.into_parts();
        let lease = self.engine.unmap_device_memory(lease)?;
        self.engine.release_device_memory(lease)
    }

    /// Unmaps and releases one addressless fixed-dispatch data authority.
    ///
    /// This consumes both uninitialized and fully initialized variants without
    /// exposing their private allocation identity or address.
    pub fn release_fixed_dispatch_data(
        &mut self,
        data: crate::Gfx942FixedDispatchDataV1,
    ) -> Result<(), MemorySessionError> {
        let (lease, _, _) = data.into_parts();
        let lease = self.engine.unmap_device_memory(lease)?;
        self.engine.release_device_memory(lease)
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

    pub(crate) fn quarantine_queue_composition(
        &mut self,
        detail: &'static str,
    ) -> Result<(), MemorySessionError> {
        self.engine
            .quarantine(MemorySessionError::KernelResultMalformed(detail))
    }

    pub(crate) fn cwsr_shadow_plan(
        &mut self,
        token: &SharedGttAllocationV1<ExecutableGttV1, GttCpuWritableV1>,
    ) -> Result<crate::queue_linux::CwsrShadowPlanV1, MemorySessionError> {
        self.engine.check_currentness()?;
        let index = self
            .engine
            .index(token, SharedAllocationPhaseV1::CpuWritable)?;
        let record = &self.engine.allocations[index];
        let reservation = record
            .reservation
            .as_ref()
            .ok_or(MemorySessionError::InvalidAllocationAuthority)?;
        let reservation_address =
            <crate::memory_linux::LinuxMemoryBackend as MemoryBackend>::reservation_address(
                reservation,
            );
        if record.profile != SharedGttProfileV1::Executable
            || reservation_address != record.gpu_va
            || record.layout.requested_bytes != crate::queue::submit::GFX942_CWSR_TOTAL_BYTES_V1
            || record.layout.cpu_mapping_bytes != crate::queue::submit::GFX942_CWSR_TOTAL_BYTES_V1
            || record.layout.gpu_va_bytes != crate::queue::submit::GFX942_CWSR_TOTAL_BYTES_V1 as u64
        {
            return self
                .engine
                .quarantine(MemorySessionError::KernelResultMalformed(
                    "owned CWSR reservation geometry",
                ));
        }
        let plan = crate::queue_linux::CwsrShadowPlanV1::from_owned_reservation(
            record.gpu_va,
            record.layout.requested_bytes,
            self.engine.backend.page_size(),
        )
        .map_err(|_| MemorySessionError::KernelResultMalformed("CWSR shadow plan"));
        match plan {
            Ok(plan) => {
                self.engine.check_currentness()?;
                Ok(plan)
            }
            Err(error) => self.engine.quarantine(error),
        }
    }

    pub(crate) const fn queue_model_device(&self) -> ModelDeviceAdmissionV1 {
        self.model_device
    }

    pub(crate) fn plan_aql_queue_resources(
        &self,
        ring_bytes: u32,
    ) -> Result<crate::Gfx942AqlQueueResourcePlanV1, crate::Gfx942QueueResourcePlanningError> {
        self.engine.backend.plan_aql_queue_resources(ring_bytes)
    }

    pub(crate) fn take_queue_model_foundation(
        &mut self,
    ) -> Result<(DeviceIdentityStateV1, MemoryLifecycleStateV1), MemorySessionError> {
        self.check_queue_currentness()?;
        if self.retained_device_memory_lease_count() != 0 {
            return Err(MemorySessionError::DeviceMemoryQueueBindingRequired);
        }
        self.take_queue_model_foundation_after_device_memory_check()
    }

    /// Transfers queue-model ownership only when every retained C3 allocation
    /// is represented by one exact, distinct, mapped dispatch authority.
    ///
    /// Validation completes before model mutation. The ordinary queue path
    /// continues to reject any retained device memory.
    pub(crate) fn take_queue_model_foundation_with_dispatch_memory(
        &mut self,
        authorities: &[Gfx942DeviceMemoryDispatchAuthorityV1],
    ) -> Result<(DeviceIdentityStateV1, MemoryLifecycleStateV1), MemorySessionError> {
        self.check_queue_currentness()?;
        self.engine.validate_dispatch_device_memory_set(
            authorities,
            self.model_device.model_key(),
            self.vm,
        )?;
        self.take_queue_model_foundation_after_device_memory_check()
    }

    /// Revalidates the exact complete mapped device-memory set after queue
    /// model ownership has already transferred into a live queue engine.
    pub(crate) fn validate_live_queue_dispatch_memory(
        &mut self,
        authorities: &[Gfx942DeviceMemoryDispatchAuthorityV1],
    ) -> Result<(), MemorySessionError> {
        self.check_queue_currentness()?;
        self.engine.validate_dispatch_device_memory_set(
            authorities,
            self.model_device.model_key(),
            self.vm,
        )
    }

    fn take_queue_model_foundation_after_device_memory_check(
        &mut self,
    ) -> Result<(DeviceIdentityStateV1, MemoryLifecycleStateV1), MemorySessionError> {
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

    #[allow(dead_code)]
    pub(crate) fn retain_aql_completion_signal_resource(
        &self,
        token: SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>,
    ) -> Result<
        SharedGttQueueResourceAuthorityV1<
            AqlCompletionSignalResourceRoleV1,
            HostVisibleCoherentGttV1,
            GttGpuAccessibleMutableV1,
        >,
        MemorySessionError,
    > {
        self.retain_queue_resource(token)
    }

    pub(crate) fn retain_aql_dispatch_code_resource(
        &self,
        token: SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>,
    ) -> Result<
        SharedGttQueueResourceAuthorityV1<
            AqlDispatchCodeResourceRoleV1,
            ExecutableGttV1,
            GttGpuAccessibleExecutableV1,
        >,
        MemorySessionError,
    > {
        self.retain_queue_resource(token)
    }

    pub(crate) fn retain_aql_dispatch_kernarg_resource(
        &self,
        token: SharedGttAllocationV1<KernargGttV1, GttGpuAccessibleMutableV1>,
    ) -> Result<
        SharedGttQueueResourceAuthorityV1<
            AqlDispatchKernargResourceRoleV1,
            KernargGttV1,
            GttGpuAccessibleMutableV1,
        >,
        MemorySessionError,
    > {
        self.retain_queue_resource(token)
    }

    pub(crate) fn retain_gfx942_device_memory_for_dispatch(
        &self,
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<Gfx942DeviceMemoryDispatchAuthorityV1, MemorySessionError> {
        let index = self
            .engine
            .device_memory_index(&lease, DeviceMemoryPhaseV1::Mapped)?;
        let record = &self.engine.device_memory[index];
        if record.device != self.model_device.model_key() || record.vm != self.vm {
            return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
        }
        Ok(Gfx942DeviceMemoryDispatchAuthorityV1 {
            facts: Gfx942DeviceMemoryDispatchFactsV1 {
                id: record.id,
                generation: record.generation,
                device: record.device,
                vm: record.vm,
                gpu_va: record.gpu_va,
                layout: record.layout,
            },
            lease,
        })
    }

    pub(crate) fn validate_gfx942_dispatch_allocation_requests(
        &self,
        requests: &[(u64, u64)],
    ) -> Result<(), MemorySessionError> {
        self.engine.require_active()?;
        let retained_records = self.retained_device_memory_lease_count();
        if retained_records
            .checked_add(requests.len())
            .is_none_or(|count| count > MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1)
        {
            return Err(MemorySessionError::DeviceMemoryAllocationCapacity {
                maximum: MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1,
            });
        }
        let mut retained_bytes = self.engine.retained_device_memory_bytes;
        for &(requested_bytes, alignment) in requests {
            let layout = device_memory_layout(
                requested_bytes,
                alignment,
                KfdAllocMemoryFlags::DEVICE_LOCAL,
            )?;
            retained_bytes = retained_bytes
                .checked_add(layout.backing_bytes)
                .ok_or(MemorySessionError::SizeOverflow)?;
            if retained_bytes > MAX_GFX942_DEVICE_MEMORY_BYTES_V1 {
                return Err(MemorySessionError::DeviceMemoryByteCapacity {
                    maximum_bytes: MAX_GFX942_DEVICE_MEMORY_BYTES_V1,
                });
            }
        }
        Ok(())
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

    #[allow(dead_code)]
    pub(crate) fn observe_aql_control_counters(
        &mut self,
        authority: &mut SharedGttQueueResourceAuthorityV1<
            AqlControlResourceRoleV1,
            HostVisibleCoherentGttV1,
            GttGpuAccessibleMutableV1,
        >,
    ) -> Result<(u64, u64), MemorySessionError> {
        self.engine.observe_aql_counters(&mut authority.token)
    }

    #[allow(dead_code)]
    pub(crate) fn fetch_add_aql_control_write(
        &mut self,
        authority: &mut SharedGttQueueResourceAuthorityV1<
            AqlControlResourceRoleV1,
            HostVisibleCoherentGttV1,
            GttGpuAccessibleMutableV1,
        >,
        increment: u64,
    ) -> Result<u64, MemorySessionError> {
        self.engine
            .fetch_add_aql_write(&mut authority.token, increment)
    }

    #[allow(dead_code)]
    pub(crate) fn write_aql_ring_slot(
        &mut self,
        authority: &mut SharedGttQueueResourceAuthorityV1<
            AqlRingResourceRoleV1,
            AqlQueueGttV1,
            GttGpuAccessibleMutableV1,
        >,
        slot_index: u32,
        packet: &[u8; 64],
    ) -> Result<(), MemorySessionError> {
        self.engine
            .write_aql_slot(&mut authority.token, slot_index, packet)
    }

    #[allow(dead_code)]
    pub(crate) fn publish_aql_ring_header(
        &mut self,
        authority: &mut SharedGttQueueResourceAuthorityV1<
            AqlRingResourceRoleV1,
            AqlQueueGttV1,
            GttGpuAccessibleMutableV1,
        >,
        slot_index: u32,
        header: u16,
    ) -> Result<(), MemorySessionError> {
        self.engine
            .publish_aql_header(&mut authority.token, slot_index, header)
    }

    #[allow(dead_code)]
    pub(crate) fn observe_aql_completion_signal(
        &mut self,
        authority: &mut SharedGttQueueResourceAuthorityV1<
            AqlCompletionSignalResourceRoleV1,
            HostVisibleCoherentGttV1,
            GttGpuAccessibleMutableV1,
        >,
        slot_index: u32,
    ) -> Result<fe2o3_aql::AqlCompletionObservationV1, MemorySessionError> {
        self.engine
            .observe_completion_signal(&mut authority.token, slot_index)
    }

    #[allow(dead_code)]
    pub(crate) fn reset_aql_completion_signal(
        &mut self,
        authority: &mut SharedGttQueueResourceAuthorityV1<
            AqlCompletionSignalResourceRoleV1,
            HostVisibleCoherentGttV1,
            GttGpuAccessibleMutableV1,
        >,
        slot_index: u32,
    ) -> Result<(), MemorySessionError> {
        self.engine
            .reset_completion_signal(&mut authority.token, slot_index)
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
    assert!(KFD_ALLOC_MEMORY_FLAGS_DEVICE_LOCAL == 0x8000_0001);
    assert!(KFD_ALLOC_MEMORY_FLAGS_DEVICE_LOCAL_PUBLIC == 0xa000_0001);
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
        unmap_errno: bool,
        alloc_oom: bool,
        corrupt_flags: bool,
        currentness_calls: usize,
        fail_currentness_at: Option<usize>,
        reserve_va_calls: usize,
        alloc_calls: usize,
        map_cpu_calls: usize,
        map_gpu_calls: usize,
        unmap_gpu_calls: usize,
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
                unmap_errno: false,
                alloc_oom: false,
                corrupt_flags: false,
                currentness_calls: 0,
                fail_currentness_at: None,
                reserve_va_calls: 0,
                alloc_calls: 0,
                map_cpu_calls: 0,
                map_gpu_calls: 0,
                unmap_gpu_calls: 0,
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
            crate::InclusiveAperture::from_checked_parts_for_memory_tests(
                0x1_0000,
                0x1_0000_0000_0000,
            )
        }
        fn page_size(&self) -> usize {
            4096
        }
        fn check_currentness(&mut self) -> Result<(), MemorySessionError> {
            self.currentness_calls += 1;
            if self.fail_currentness_at == Some(self.currentness_calls) {
                Err(MemorySessionError::Injected("currentness"))
            } else {
                self.check("currentness")
            }
        }
        fn acquire_vm(&mut self) -> Result<(), MemorySessionError> {
            self.check("acquire_vm")
        }
        fn reserve_va(&mut self, bytes: usize) -> Result<Self::Reservation, MemorySessionError> {
            self.reserve_va_calls += 1;
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
            self.alloc_calls += 1;
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
                result: if self.alloc_oom {
                    Err(MemorySessionError::Syscall {
                        operation: "AMDKFD_IOC_ALLOC_MEMORY_OF_GPU",
                        source: rustix::io::Errno::NOMEM,
                    })
                } else {
                    self.check("alloc")
                },
            }
        }
        fn map_cpu(
            &mut self,
            _reservation: &mut Self::Reservation,
            _mmap_offset: u64,
            bytes: usize,
            _retain_gpu_va_guard: bool,
        ) -> Result<Self::Mapping, MemorySessionError> {
            self.map_cpu_calls += 1;
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
            self.map_gpu_calls += 1;
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
            self.unmap_gpu_calls += 1;
            KernelOutcome {
                value: self.unmap_progress,
                result: if self.unmap_errno {
                    Err(MemorySessionError::Injected("unmap_gpu"))
                } else {
                    self.check("unmap_gpu")
                },
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

    fn device_vm(generation: u64) -> (DeviceKeyV1, VmKeyV1) {
        let device = DeviceKeyV1 {
            physical: fe2o3_runtime_model::PhysicalDeviceIdV1(9),
            generation: fe2o3_runtime_model::DeviceGenerationV1(generation),
        };
        (
            device,
            VmKeyV1 {
                device,
                id: VmIdV1(11),
            },
        )
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
        engine.backend.alloc_oom = true;
        assert!(engine.allocate::<KernargGttV1>(4096).is_err());
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert!(matches!(
            engine.with_bytes(&first, SharedAllocationPhaseV1::CpuWritable, |_| ()),
            Err(MemorySessionError::SharedSessionQuarantined)
        ));
    }

    #[test]
    fn completion_arena_allocation_oom_requires_quarantined_teardown() {
        let mut engine = acquired();
        engine.backend.alloc_oom = true;
        assert!(matches!(
            engine.allocate::<HostVisibleCoherentGttV1>(
                crate::queue::completion::COMPLETION_SIGNAL_ARENA_BYTES_V1,
            ),
            Err(MemorySessionError::Syscall {
                operation: "AMDKFD_IOC_ALLOC_MEMORY_OF_GPU",
                ..
            })
        ));
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(engine.backend.alloc_calls, 1);
        assert_eq!(engine.retained_gpu_va_bytes, 0);
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

    #[test]
    fn device_memory_profile_manifest_and_layout_are_frozen() {
        let digest = Sha256::digest(GFX942_DEVICE_MEMORY_LEASE_MANIFEST_V1);
        assert_eq!(
            digest.as_slice(),
            GFX942_DEVICE_MEMORY_LEASE_MANIFEST_SHA256_BYTES_V1
        );
        let mut digest_hex = String::with_capacity(64);
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in digest.iter().copied() {
            digest_hex.push(char::from(HEX[usize::from(byte >> 4)]));
            digest_hex.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        assert_eq!(digest_hex, GFX942_DEVICE_MEMORY_LEASE_MANIFEST_SHA256_V1);
        assert!(
            GFX942_DEVICE_MEMORY_LEASE_MANIFEST_V1
                .contains(fe2o3_kfd_uapi::KFD_DEVICE_MEMORY_LIFECYCLE_SCHEMA_MANIFEST_SHA256)
        );
        let initialization_digest = Sha256::digest(GFX942_DEVICE_MEMORY_INITIALIZATION_MANIFEST_V1);
        let initialization_hex: String = initialization_digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(
            initialization_hex,
            GFX942_DEVICE_MEMORY_INITIALIZATION_MANIFEST_SHA256_V1
        );
        assert!(
            GFX942_DEVICE_MEMORY_INITIALIZATION_MANIFEST_V1
                .contains(fe2o3_kfd_uapi::KFD_PUBLIC_DEVICE_MEMORY_SCHEMA_MANIFEST_SHA256)
        );

        assert!(matches!(
            device_memory_layout(0, 4096, KfdAllocMemoryFlags::DEVICE_LOCAL),
            Err(MemorySessionError::InvalidDeviceMemorySize)
        ));
        assert!(matches!(
            device_memory_layout(
                MAX_GFX942_DEVICE_MEMORY_BYTES_V1 + 1,
                4096,
                KfdAllocMemoryFlags::DEVICE_LOCAL,
            ),
            Err(MemorySessionError::InvalidDeviceMemorySize)
        ));
        for alignment in [0, 3, 8192] {
            assert!(matches!(
                device_memory_layout(1, alignment, KfdAllocMemoryFlags::DEVICE_LOCAL),
                Err(MemorySessionError::InvalidDeviceMemoryAlignment)
            ));
        }
        let layout = device_memory_layout(4097, 256, KfdAllocMemoryFlags::DEVICE_LOCAL).unwrap();
        assert_eq!(layout.requested_bytes(), 4097);
        assert_eq!(layout.backing_bytes(), 8192);
        assert_eq!(layout.alignment(), 256);
        assert_eq!(layout.uapi_flags(), 0x8000_0001);
    }

    #[test]
    fn device_memory_lifecycle_is_linear_redacted_and_single_device() {
        let mut engine = acquired();
        let (device, vm) = device_vm(7);
        let lease = engine
            .allocate_device_memory(device, vm, 4097, 256)
            .unwrap();
        assert_eq!(engine.backend.flags, vec![0x8000_0001]);
        assert_eq!(engine.backend.reserve_va_calls, 1);
        assert_eq!(engine.backend.alloc_calls, 1);
        assert_eq!(engine.backend.map_cpu_calls, 0);
        assert_eq!(engine.retained_device_memory_bytes, 8192);
        assert_eq!(engine.device_memory.len(), 1);
        assert_eq!(engine.device_memory[0].device, device);
        assert_eq!(engine.device_memory[0].vm, vm);

        let lease = engine.map_device_memory(lease).unwrap();
        assert_eq!(engine.backend.map_gpu_calls, 1);
        assert_eq!(engine.device_memory[0].phase, DeviceMemoryPhaseV1::Mapped);
        let lease = engine.unmap_device_memory(lease).unwrap();
        assert_eq!(engine.backend.unmap_gpu_calls, 1);
        assert_eq!(engine.device_memory[0].phase, DeviceMemoryPhaseV1::Unmapped);
        engine.release_device_memory(lease).unwrap();
        assert_eq!(engine.backend.free_calls, 1);
        assert_eq!(engine.backend.release_va_calls, 1);
        assert_eq!(engine.retained_device_memory_bytes, 0);
        assert_eq!(engine.device_memory[0].phase, DeviceMemoryPhaseV1::Released);
    }

    fn content(bytes: &[u8]) -> Gfx942DeviceContentDescriptorV1 {
        let role = crate::Gfx942DeviceContentRoleV1::new([0x51; 32], 7).unwrap();
        Gfx942DeviceContentDescriptorV1::from_bytes(role, bytes).unwrap()
    }

    #[test]
    fn public_device_memory_initialization_checks_mapped_bytes_before_gpu_map() {
        let mut engine = acquired();
        let (device, vm) = device_vm(7);
        let bytes = vec![0x5a; 4097];
        let descriptor = content(&bytes);
        let lease = engine
            .allocate_device_memory_with_flags(
                device,
                vm,
                bytes.len() as u64,
                4096,
                KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
            )
            .unwrap();
        let initialized = engine
            .initialize_public_device_memory(lease, &bytes, descriptor)
            .unwrap();
        assert_eq!(engine.backend.flags, vec![0xa000_0001]);
        assert_eq!(engine.backend.map_cpu_calls, 1);
        assert_eq!(engine.backend.map_gpu_calls, 1);
        assert!(engine.device_memory[0].mapping.is_none());
        assert_eq!(engine.device_memory[0].phase, DeviceMemoryPhaseV1::Mapped);
        assert_eq!(initialized.content(), descriptor);
        assert_eq!(initialized.layout().requested_bytes(), bytes.len() as u64);

        let (lease, retained) = initialized.into_parts();
        assert_eq!(retained, descriptor);
        let lease = engine.unmap_device_memory(lease).unwrap();
        engine.release_device_memory(lease).unwrap();
        assert_eq!(engine.retained_device_memory_bytes, 0);
    }

    #[test]
    fn initialization_source_preflight_rejects_descriptor_substitution() {
        let bytes = vec![0x5a; 4096];
        let wrong = content(&vec![0xa5; 4096]);
        assert!(matches!(
            validate_initialization_source(&bytes, wrong),
            Err(MemorySessionError::DeviceContentMismatch)
        ));
    }

    #[test]
    fn internal_post_allocation_descriptor_substitution_quarantines() {
        let mut engine = acquired();
        let (device, vm) = device_vm(7);
        let bytes = vec![0x5a; 4096];
        let wrong = content(&vec![0xa5; 4096]);
        let lease = engine
            .allocate_device_memory_with_flags(
                device,
                vm,
                bytes.len() as u64,
                4096,
                KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
            )
            .unwrap();
        assert!(matches!(
            engine.initialize_public_device_memory(lease, &bytes, wrong),
            Err(MemorySessionError::DeviceContentMismatch)
        ));
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(engine.backend.map_cpu_calls, 0);
        assert_eq!(engine.backend.map_gpu_calls, 0);
    }

    #[test]
    fn public_device_memory_mapping_failures_quarantine_without_gpu_publication() {
        for operation in ["map_cpu", "prepare_cpu_mapping", "unmap_cpu"] {
            let mut engine = acquired();
            let (device, vm) = device_vm(7);
            let bytes = vec![0x3c; 4096];
            let descriptor = content(&bytes);
            let lease = engine
                .allocate_device_memory_with_flags(
                    device,
                    vm,
                    bytes.len() as u64,
                    4096,
                    KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
                )
                .unwrap();
            engine.backend.fail_operation = Some(operation);
            assert!(
                engine
                    .initialize_public_device_memory(lease, &bytes, descriptor)
                    .is_err()
            );
            assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
            assert_eq!(engine.backend.map_gpu_calls, 0);
        }
    }

    #[test]
    fn dispatch_transfer_requires_exact_complete_distinct_mapped_lease_set() {
        let mut engine = acquired();
        let (device, vm) = device_vm(7);
        let first = engine
            .allocate_device_memory(device, vm, 4096, 4096)
            .and_then(|lease| engine.map_device_memory(lease))
            .unwrap();
        let second = engine
            .allocate_device_memory(device, vm, 8192, 4096)
            .and_then(|lease| engine.map_device_memory(lease))
            .unwrap();
        let authority = |lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>| {
            let record = engine
                .device_memory
                .iter()
                .find(|record| record.id == lease.id)
                .unwrap();
            Gfx942DeviceMemoryDispatchAuthorityV1 {
                facts: Gfx942DeviceMemoryDispatchFactsV1 {
                    id: record.id,
                    generation: record.generation,
                    device: record.device,
                    vm: record.vm,
                    gpu_va: record.gpu_va,
                    layout: record.layout,
                },
                lease,
            }
        };
        let mut exact = [authority(first), authority(second)];
        assert!(
            engine
                .validate_dispatch_device_memory_set(&exact, device, vm)
                .is_ok()
        );
        assert!(matches!(
            engine.validate_dispatch_device_memory_set(&exact[..1], device, vm),
            Err(MemorySessionError::DeviceMemoryQueueBindingRequired)
        ));

        exact[1].facts = exact[0].facts;
        assert!(matches!(
            engine.validate_dispatch_device_memory_set(&exact, device, vm),
            Err(MemorySessionError::DeviceMemoryQueueBindingRequired)
        ));
        exact[1].facts.generation += 1;
        assert!(matches!(
            engine.validate_dispatch_device_memory_set(&exact, device, vm),
            Err(MemorySessionError::DeviceMemoryQueueBindingRequired)
        ));
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
    }

    #[test]
    fn returned_dispatch_session_preserves_and_releases_the_exact_mapped_c3_lease() {
        let mut engine = acquired();
        let (device, vm) = device_vm(7);
        let lease = engine
            .allocate_device_memory(device, vm, 8192, 4096)
            .and_then(|lease| engine.map_device_memory(lease))
            .unwrap();
        let record = engine
            .device_memory
            .iter()
            .find(|record| record.id == lease.id)
            .unwrap();
        let expected = (
            record.id,
            record.generation,
            record.device,
            record.vm,
            record.layout,
        );
        let authority = Gfx942DeviceMemoryDispatchAuthorityV1 {
            facts: Gfx942DeviceMemoryDispatchFactsV1 {
                id: record.id,
                generation: record.generation,
                device: record.device,
                vm: record.vm,
                gpu_va: record.gpu_va,
                layout: record.layout,
            },
            lease,
        };

        assert_eq!(
            (
                authority.facts.id,
                authority.facts.generation,
                authority.facts.device,
                authority.facts.vm,
                authority.facts.layout,
            ),
            expected
        );
        let returned_session = (engine, authority.into_lease());
        let (mut engine, returned) = returned_session;
        assert_eq!(
            (
                returned.id,
                returned.generation,
                returned.device,
                returned.vm,
                returned.layout,
            ),
            expected
        );
        let returned = engine.unmap_device_memory(returned).unwrap();
        engine.release_device_memory(returned).unwrap();
        assert_eq!(engine.backend.unmap_gpu_calls, 1);
        assert_eq!(engine.backend.free_calls, 1);
        assert_eq!(engine.backend.release_va_calls, 1);
        assert_eq!(engine.retained_device_memory_bytes, 0);
    }

    #[test]
    fn device_memory_oom_retains_possible_native_authority_and_poison() {
        let mut engine = acquired();
        let (device, vm) = device_vm(1);
        engine.backend.alloc_oom = true;
        assert!(matches!(
            engine.allocate_device_memory(device, vm, 4096, 4096),
            Err(MemorySessionError::Syscall {
                source: rustix::io::Errno::NOMEM,
                ..
            })
        ));
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(engine.device_memory.len(), 1);
        assert!(engine.device_memory[0].reservation.is_some());
        assert!(engine.device_memory[0].handle.is_some());
        assert_eq!(
            engine.device_memory[0].phase,
            DeviceMemoryPhaseV1::Ambiguous
        );
        assert_eq!(engine.retained_device_memory_bytes, 4096);
        assert_eq!(engine.backend.free_calls, 0);
        assert_eq!(engine.backend.release_va_calls, 0);
    }

    #[test]
    fn device_memory_rejects_wrong_device_generation_and_address_overflow() {
        let mut mismatch = acquired();
        let (device, vm) = device_vm(1);
        let (other_device, _) = device_vm(2);
        assert!(matches!(
            mismatch.allocate_device_memory(other_device, vm, 4096, 4096),
            Err(MemorySessionError::InvalidDeviceMemoryAuthority)
        ));
        assert_eq!(mismatch.backend.reserve_va_calls, 0);

        let lease = mismatch
            .allocate_device_memory(device, vm, 4096, 4096)
            .unwrap();
        let substituted = Gfx942DeviceMemoryLeaseV1 {
            id: lease.id,
            generation: lease.generation,
            device: other_device,
            vm: VmKeyV1 {
                device: other_device,
                id: lease.vm.id,
            },
            layout: lease.layout,
            marker: PhantomData::<Gfx942DeviceMemoryUnmappedV1>,
        };
        let stale_generation = Gfx942DeviceMemoryLeaseV1 {
            id: lease.id,
            generation: lease.generation + 1,
            device: lease.device,
            vm: lease.vm,
            layout: lease.layout,
            marker: PhantomData::<Gfx942DeviceMemoryUnmappedV1>,
        };
        assert!(matches!(
            mismatch.map_device_memory(substituted),
            Err(MemorySessionError::InvalidDeviceMemoryAuthority)
        ));
        assert!(matches!(
            mismatch.map_device_memory(stale_generation),
            Err(MemorySessionError::InvalidDeviceMemoryAuthority)
        ));
        assert_eq!(mismatch.phase(), SharedMemorySessionPhaseV1::Active);
        assert_eq!(mismatch.backend.map_gpu_calls, 0);

        let mut overflow = acquired();
        overflow.backend.fixed_va = Some(u64::MAX - 2047);
        assert!(
            overflow
                .allocate_device_memory(device, vm, 4096, 4096)
                .is_err()
        );
        assert_eq!(overflow.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(overflow.backend.alloc_calls, 0);
        assert!(overflow.device_memory[0].reservation.is_some());
    }

    #[test]
    fn device_memory_map_and_unmap_ambiguity_retain_and_poison() {
        let (device, vm) = device_vm(1);

        let mut map_zero = acquired();
        let lease = map_zero
            .allocate_device_memory(device, vm, 4096, 4096)
            .unwrap();
        map_zero.backend.map_progress = 0;
        assert!(map_zero.map_device_memory(lease).is_err());
        assert_eq!(map_zero.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert!(map_zero.device_memory[0].handle.is_some());
        assert!(map_zero.device_memory[0].reservation.is_some());

        let mut map_errno = acquired();
        let lease = map_errno
            .allocate_device_memory(device, vm, 4096, 4096)
            .unwrap();
        map_errno.backend.map_errno = true;
        assert!(map_errno.map_device_memory(lease).is_err());
        assert_eq!(map_errno.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(map_errno.backend.free_calls, 0);

        let mut unmap_errno = acquired();
        let lease = unmap_errno
            .allocate_device_memory(device, vm, 4096, 4096)
            .unwrap();
        let lease = unmap_errno.map_device_memory(lease).unwrap();
        unmap_errno.backend.unmap_errno = true;
        assert!(unmap_errno.unmap_device_memory(lease).is_err());
        assert_eq!(unmap_errno.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(
            unmap_errno.device_memory[0].phase,
            DeviceMemoryPhaseV1::Ambiguous
        );
        assert_eq!(unmap_errno.backend.free_calls, 0);
    }

    #[test]
    fn device_memory_free_and_va_release_ambiguity_are_never_retried() {
        let (device, vm) = device_vm(1);

        let mut free = acquired();
        let lease = free.allocate_device_memory(device, vm, 4096, 4096).unwrap();
        free.backend.fail_operation = Some("free");
        assert!(free.release_device_memory(lease).is_err());
        assert_eq!(free.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(free.backend.free_calls, 1);
        assert_eq!(free.backend.release_va_calls, 0);
        assert!(free.device_memory[0].handle.is_some());
        assert!(free.device_memory[0].reservation.is_some());

        let mut va = acquired();
        let lease = va.allocate_device_memory(device, vm, 4096, 4096).unwrap();
        va.backend.fail_operation = Some("release_va_reservation");
        assert!(va.release_device_memory(lease).is_err());
        assert_eq!(va.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(va.backend.free_calls, 1);
        assert_eq!(va.backend.release_va_calls, 1);
        assert!(va.device_memory[0].handle.is_none());
        assert!(va.device_memory[0].reservation.is_some());
    }

    #[test]
    fn device_memory_post_side_effect_currentness_failures_retain_and_poison() {
        let (device, vm) = device_vm(1);

        let mut allocation = acquired();
        allocation.backend.fail_currentness_at = Some(4);
        assert!(
            allocation
                .allocate_device_memory(device, vm, 4096, 4096)
                .is_err()
        );
        assert_eq!(allocation.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert!(allocation.device_memory[0].handle.is_some());
        assert!(allocation.device_memory[0].reservation.is_some());

        let mut map = acquired();
        let lease = map.allocate_device_memory(device, vm, 4096, 4096).unwrap();
        map.backend.fail_currentness_at = Some(6);
        assert!(map.map_device_memory(lease).is_err());
        assert_eq!(map.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(map.device_memory[0].phase, DeviceMemoryPhaseV1::Ambiguous);

        let mut unmap = acquired();
        let lease = unmap
            .allocate_device_memory(device, vm, 4096, 4096)
            .unwrap();
        let lease = unmap.map_device_memory(lease).unwrap();
        unmap.backend.fail_currentness_at = Some(8);
        assert!(unmap.unmap_device_memory(lease).is_err());
        assert_eq!(unmap.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(unmap.device_memory[0].phase, DeviceMemoryPhaseV1::Ambiguous);

        let mut free = acquired();
        let lease = free.allocate_device_memory(device, vm, 4096, 4096).unwrap();
        free.backend.fail_currentness_at = Some(6);
        assert!(free.release_device_memory(lease).is_err());
        assert_eq!(free.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert!(free.device_memory[0].handle.is_none());
        assert!(free.device_memory[0].reservation.is_some());

        let mut va_release = acquired();
        let lease = va_release
            .allocate_device_memory(device, vm, 4096, 4096)
            .unwrap();
        va_release.backend.fail_currentness_at = Some(7);
        assert!(va_release.release_device_memory(lease).is_err());
        assert_eq!(va_release.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert!(va_release.device_memory[0].handle.is_none());
        assert!(va_release.device_memory[0].reservation.is_none());
        assert_eq!(va_release.retained_device_memory_bytes, 4096);
    }

    #[test]
    fn released_device_memory_rejects_forged_double_release_and_use() {
        let mut engine = acquired();
        let (device, vm) = device_vm(1);
        let lease = engine
            .allocate_device_memory(device, vm, 4096, 4096)
            .unwrap();
        let forge = || Gfx942DeviceMemoryLeaseV1 {
            id: lease.id,
            generation: lease.generation,
            device: lease.device,
            vm: lease.vm,
            layout: lease.layout,
            marker: PhantomData::<Gfx942DeviceMemoryUnmappedV1>,
        };
        let double_release = forge();
        let use_after_release = forge();
        engine.release_device_memory(lease).unwrap();
        assert!(matches!(
            engine.release_device_memory(double_release),
            Err(MemorySessionError::InvalidDeviceMemoryAuthority)
        ));
        assert!(matches!(
            engine.map_device_memory(use_after_release),
            Err(MemorySessionError::InvalidDeviceMemoryAuthority)
        ));
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
        assert_eq!(engine.backend.free_calls, 1);
        assert_eq!(engine.backend.release_va_calls, 1);
    }

    #[test]
    fn device_memory_capacity_is_preflighted_and_success_reclaims_bytes() {
        let mut bytes = acquired();
        let (device, vm) = device_vm(1);
        let lease = bytes
            .allocate_device_memory(device, vm, MAX_GFX942_DEVICE_MEMORY_BYTES_V1, 4096)
            .unwrap();
        assert!(matches!(
            bytes.allocate_device_memory(device, vm, 1, 1),
            Err(MemorySessionError::DeviceMemoryByteCapacity { .. })
        ));
        assert_eq!(bytes.backend.alloc_calls, 1);
        bytes.release_device_memory(lease).unwrap();
        assert_eq!(bytes.retained_device_memory_bytes, 0);
        assert!(bytes.allocate_device_memory(device, vm, 1, 1).is_ok());

        let mut records = acquired();
        let mut leases = Vec::new();
        for _ in 0..MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1 {
            leases.push(records.allocate_device_memory(device, vm, 1, 1).unwrap());
        }
        assert!(matches!(
            records.allocate_device_memory(device, vm, 1, 1),
            Err(MemorySessionError::DeviceMemoryAllocationCapacity { .. })
        ));
        assert_eq!(
            records.backend.alloc_calls,
            MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1
        );
        assert_eq!(leases.len(), MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1);
    }
}
