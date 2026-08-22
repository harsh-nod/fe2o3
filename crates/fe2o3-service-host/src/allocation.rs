//! Linear service ownership for real KFD-backed allocations.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU64, Ordering};

use fe2o3_kfd::{
    CheckedGfx942XnackMinusDevice, Gfx942DeviceMemoryLeaseV1, Gfx942DeviceMemoryMappedV1,
    Gfx942DeviceMemoryUnmappedV1, GttCpuWritableV1, GttGpuAccessibleMutableV1,
    HOST_VISIBLE_MEMORY_PAGE_BYTES_V1, HostVisibleCoherentGttV1, MemorySessionError,
    SharedGttAllocationV1, SharedGttMemorySessionV1, SharedMemorySessionPhaseV1,
};

/// Canonical scope and non-claims for the first service allocation owner.
pub const SERVICE_ALLOCATION_OWNERSHIP_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-m1-service-allocation-owner-r1-v1\n",
    "backend=checked-gfx942-xnack-minus-device,shared-kfd-vm-session\n",
    "device=device-local-vram-hbm,linear-unmapped-or-mapped-kfd-lease\n",
    "host=host-visible-coherent-gtt,linear-cpu-writable-or-gpu-mapped-token\n",
    "identity=service-scoped-process-local-owner-device-vm-allocation-labels-retained-beside-private-kfd-native-tokens\n",
    "views=typed-role-kind-offset-extent-alignment,no-handle-fd-gpu-address-or-persistent-raw-pointer-accessor\n",
    "cpu-write=scoped-mutable-slice-before-gpu-map,safe-caller-may-return-or-retain-raw-cpu-pointer-or-address,no-safe-post-borrow-dereference\n",
    "bounds=32-live-allocations,device-192gib,host-2gib,page-and-device-alignment-max-4096\n",
    "release=gpu-never-published-only,reverse-order-unmap-then-free,consuming-owner\n",
    "failure=preflight-retains-owner,consumed-token-failure-quarantines-retained-session,no-drop-cleanup\n",
    "excluded=device-content-initialization,copy,packet,queue,launch,completion,hardware-execution,m1-closure\n",
);

/// SHA-256 of [`SERVICE_ALLOCATION_OWNERSHIP_MANIFEST_V1`].
pub const SERVICE_ALLOCATION_OWNERSHIP_MANIFEST_SHA256_V1: &str =
    "b1b901ea2b950510f7f22d50c5ec89dd20eb0a37baadc9056518c204b6c653c7";

/// Maximum live allocations owned by one service allocation session.
const MAX_SERVICE_ALLOCATIONS_V1: usize = 32;
/// Maximum total requested device-local bytes owned by one service session.
const MAX_SERVICE_DEVICE_BYTES_V1: u64 = 192 << 30;
/// Maximum total requested host-visible bytes owned by one service session.
const MAX_SERVICE_HOST_BYTES_V1: u64 = 2 << 30;
const MAX_SERVICE_ALIGNMENT_V1: u64 = HOST_VISIBLE_MEMORY_PAGE_BYTES_V1;

static NEXT_OWNER_GENERATION_V1: AtomicU64 = AtomicU64::new(1);

mod sealed {
    pub trait Role {}
    pub trait Kind {}
}

/// A sealed compile-time allocation role.
pub trait ServiceAllocationRoleMarkerV1: sealed::Role + 'static {
    /// Stable role name used in redacted observations.
    const NAME: &'static str;
    /// Stable private-ledger discriminator for this sealed role.
    const ROLE_ID: u8;
}

/// A sealed role admitted for device-local storage.
pub trait DeviceAllocationRoleMarkerV1: ServiceAllocationRoleMarkerV1 {}

/// A sealed role admitted for host-visible coherent storage.
pub trait HostAllocationRoleMarkerV1: ServiceAllocationRoleMarkerV1 {}

/// Device-local input storage.
pub enum DeviceInputRoleV1 {}
/// Device-local persistent state storage.
pub enum DeviceStateRoleV1 {}
/// Device-local temporary workspace storage.
pub enum DeviceWorkspaceRoleV1 {}
/// Device-local output storage.
pub enum DeviceOutputRoleV1 {}
/// Host-visible upload staging storage.
pub enum HostUploadRoleV1 {}
/// Host-visible download staging storage.
pub enum HostDownloadRoleV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum AllocationRoleV1 {
    DeviceInput,
    DeviceState,
    DeviceWorkspace,
    DeviceOutput,
    HostUpload,
    HostDownload,
}

macro_rules! define_role {
    ($marker:ty, $role:ident, $name:literal, device) => {
        impl sealed::Role for $marker {}
        impl ServiceAllocationRoleMarkerV1 for $marker {
            const NAME: &'static str = $name;
            const ROLE_ID: u8 = AllocationRoleV1::$role as u8;
        }
        impl DeviceAllocationRoleMarkerV1 for $marker {}
    };
    ($marker:ty, $role:ident, $name:literal, host) => {
        impl sealed::Role for $marker {}
        impl ServiceAllocationRoleMarkerV1 for $marker {
            const NAME: &'static str = $name;
            const ROLE_ID: u8 = AllocationRoleV1::$role as u8;
        }
        impl HostAllocationRoleMarkerV1 for $marker {}
    };
}

define_role!(DeviceInputRoleV1, DeviceInput, "device-input", device);
define_role!(DeviceStateRoleV1, DeviceState, "device-state", device);
define_role!(
    DeviceWorkspaceRoleV1,
    DeviceWorkspace,
    "device-workspace",
    device
);
define_role!(DeviceOutputRoleV1, DeviceOutput, "device-output", device);
define_role!(HostUploadRoleV1, HostUpload, "host-upload", host);
define_role!(HostDownloadRoleV1, HostDownload, "host-download", host);

/// Stable device-local role names accepted by this owner.
pub const DEVICE_LOCAL_ALLOCATION_ROLES_V1: [&str; 4] = [
    DeviceInputRoleV1::NAME,
    DeviceStateRoleV1::NAME,
    DeviceWorkspaceRoleV1::NAME,
    DeviceOutputRoleV1::NAME,
];

/// Stable host-visible role names accepted by this owner.
pub const HOST_VISIBLE_ALLOCATION_ROLES_V1: [&str; 2] =
    [HostUploadRoleV1::NAME, HostDownloadRoleV1::NAME];

/// A sealed compile-time allocation kind.
pub trait ServiceAllocationKindMarkerV1: sealed::Kind + 'static {
    /// Stable kind name used in redacted observations.
    const NAME: &'static str;
    /// Stable private-ledger discriminator for this sealed kind.
    const KIND_ID: u8;
}

/// Device-local VRAM/HBM allocation kind.
pub enum DeviceLocalAllocationV1 {}
/// Host-visible coherent GTT allocation kind.
pub enum HostVisibleAllocationV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum AllocationKindV1 {
    DeviceLocal,
    HostVisible,
}

impl sealed::Kind for DeviceLocalAllocationV1 {}
impl ServiceAllocationKindMarkerV1 for DeviceLocalAllocationV1 {
    const NAME: &'static str = "device-local";
    const KIND_ID: u8 = AllocationKindV1::DeviceLocal as u8;
}

impl sealed::Kind for HostVisibleAllocationV1 {}
impl ServiceAllocationKindMarkerV1 for HostVisibleAllocationV1 {
    const NAME: &'static str = "host-visible-coherent";
    const KIND_ID: u8 = AllocationKindV1::HostVisible as u8;
}

/// Typestate proving that this owner published no GPU address or queue binding.
///
/// Scoped host writes may lend a CPU slice before GPU mapping. Safe caller code
/// can derive and retain a raw CPU pointer or numerical address from that
/// slice. Rust provides no safe dereference after the borrow ends; unsafe later
/// use is outside this owner's guarantees. This state claims only that this
/// owner published no GPU address or queue binding.
pub enum NeverPublishedV1 {}

/// Redacted service allocation owner phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceAllocationPhaseV1 {
    /// The retained KFD session accepts operations.
    Active,
    /// The KFD session and native records remain retained, while in-process
    /// use and cleanup are denied. A consuming failure may have destroyed the
    /// public typed token.
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnerBindingV1 {
    owner_generation: u64,
    device_owner_generation: u64,
    vm_owner_generation: u64,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct AllocationBindingV1 {
    owner: OwnerBindingV1,
    id: u64,
    generation: u64,
    role_id: u8,
    kind_id: u8,
    extent_bytes: u64,
    alignment: u64,
}

/// An unforgeable, non-authoritative key for one retained allocation.
///
/// Copying this key does not copy allocation authority. The non-Clone session
/// retains process-local service bindings beside KFD's private native tokens
/// and records.
///
/// ```compile_fail
/// use fe2o3_service_host::{
///     DeviceInputRoleV1, DeviceLocalAllocationV1, ServiceAllocationKeyV1,
/// };
///
/// fn forge() -> ServiceAllocationKeyV1<DeviceInputRoleV1, DeviceLocalAllocationV1> {
///     ServiceAllocationKeyV1 { id: 1 }
/// }
/// ```
pub struct ServiceAllocationKeyV1<R, K>
where
    R: ServiceAllocationRoleMarkerV1,
    K: ServiceAllocationKindMarkerV1,
{
    binding: AllocationBindingV1,
    marker: PhantomData<fn() -> (R, K)>,
}

impl<R, K> Copy for ServiceAllocationKeyV1<R, K>
where
    R: ServiceAllocationRoleMarkerV1,
    K: ServiceAllocationKindMarkerV1,
{
}

impl<R, K> Clone for ServiceAllocationKeyV1<R, K>
where
    R: ServiceAllocationRoleMarkerV1,
    K: ServiceAllocationKindMarkerV1,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<R, K> fmt::Debug for ServiceAllocationKeyV1<R, K>
where
    R: ServiceAllocationRoleMarkerV1,
    K: ServiceAllocationKindMarkerV1,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceAllocationKeyV1")
            .field("role", &R::NAME)
            .field("kind", &K::NAME)
            .field("extent_bytes", &self.binding.extent_bytes)
            .field("alignment", &self.binding.alignment)
            .finish_non_exhaustive()
    }
}

impl<R, K> ServiceAllocationKeyV1<R, K>
where
    R: ServiceAllocationRoleMarkerV1,
    K: ServiceAllocationKindMarkerV1,
{
    /// Returns the exact requested allocation extent.
    pub const fn extent_bytes(self) -> u64 {
        self.binding.extent_bytes
    }

    /// Returns the admitted base alignment.
    pub const fn alignment(self) -> u64 {
        self.binding.alignment
    }

    /// Returns the static role name.
    pub const fn role_name(self) -> &'static str {
        R::NAME
    }

    /// Returns the static allocation-kind name.
    pub const fn kind_name(self) -> &'static str {
        K::NAME
    }
}

/// A checked typed subrange of one mapped allocation.
///
/// This value deliberately has no GPU-address, native-handle, persistent raw
/// pointer, or file-descriptor accessor. It is an inert future-runner binding
/// description, not packet, launch, copy, initialization, or completion
/// authority.
#[derive(Clone, Copy)]
pub struct ServiceAllocationRangeV1<R, K>
where
    R: ServiceAllocationRoleMarkerV1,
    K: ServiceAllocationKindMarkerV1,
{
    key: ServiceAllocationKeyV1<R, K>,
    offset_bytes: u64,
    extent_bytes: u64,
    alignment: u64,
}

impl<R, K> fmt::Debug for ServiceAllocationRangeV1<R, K>
where
    R: ServiceAllocationRoleMarkerV1,
    K: ServiceAllocationKindMarkerV1,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceAllocationRangeV1")
            .field("role", &R::NAME)
            .field("kind", &K::NAME)
            .field("offset_bytes", &self.offset_bytes)
            .field("extent_bytes", &self.extent_bytes)
            .field("alignment", &self.alignment)
            .finish_non_exhaustive()
    }
}

impl<R, K> ServiceAllocationRangeV1<R, K>
where
    R: ServiceAllocationRoleMarkerV1,
    K: ServiceAllocationKindMarkerV1,
{
    /// Returns the allocation key without native address authority.
    pub const fn key(self) -> ServiceAllocationKeyV1<R, K> {
        self.key
    }

    /// Returns the byte offset within the allocation.
    pub const fn offset_bytes(self) -> u64 {
        self.offset_bytes
    }

    /// Returns the checked subrange extent.
    pub const fn extent_bytes(self) -> u64 {
        self.extent_bytes
    }

    /// Returns the required address alignment checked by the owner.
    pub const fn alignment(self) -> u64 {
        self.alignment
    }
}

/// A pair of checked typed ranges from one allocation.
pub type ServiceAllocationRangePairV1<R, K> = (
    ServiceAllocationRangeV1<R, K>,
    ServiceAllocationRangeV1<R, K>,
);

enum AllocationTokenV1 {
    DeviceUnmapped(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>),
    DeviceMapped(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>),
    HostCpuWritable(SharedGttAllocationV1<HostVisibleCoherentGttV1, GttCpuWritableV1>),
    HostMapped(SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>),
}

struct OwnedAllocationV1 {
    binding: AllocationBindingV1,
    token: Option<AllocationTokenV1>,
}

struct AllocationOwnerV1 {
    session: SharedGttMemorySessionV1,
    owner: OwnerBindingV1,
    phase: ServiceAllocationPhaseV1,
    next_allocation_id: u64,
    allocations: Vec<OwnedAllocationV1>,
    device_bytes: u64,
    host_bytes: u64,
}

/// A non-Clone owner of one exact KFD device/VM and its service allocations.
///
/// ```compile_fail
/// use fe2o3_service_host::ServiceAllocationSessionV1;
///
/// fn cannot_clone(owner: ServiceAllocationSessionV1) {
///     let _ = owner.clone();
/// }
/// ```
#[must_use = "the KFD session and allocations require explicit release or quarantine retention"]
pub struct ServiceAllocationSessionV1 {
    owner: AllocationOwnerV1,
    quiescence: PhantomData<NeverPublishedV1>,
}

impl fmt::Debug for ServiceAllocationSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceAllocationSessionV1")
            .field("phase", &self.owner.phase)
            .field("allocation_count", &self.owner.allocations.len())
            .field("device_bytes", &self.owner.device_bytes)
            .field("host_bytes", &self.owner.host_bytes)
            .finish_non_exhaustive()
    }
}

/// Acquisition failure before a service allocation owner exists.
pub enum ServiceAllocationAcquireErrorV1 {
    /// The real KFD device/VM session could not be acquired.
    Memory(MemorySessionError),
    /// The process-local service owner generation was exhausted.
    OwnerGenerationExhausted {
        /// The exact KFD session retained instead of being discarded.
        retained_session: Box<SharedGttMemorySessionV1>,
    },
    /// An input KFD session was quarantined or already retained allocations.
    KfdSessionNotFresh {
        /// The exact rejected KFD session retained instead of being discarded.
        retained_session: Box<SharedGttMemorySessionV1>,
    },
}

impl fmt::Debug for ServiceAllocationAcquireErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Memory(error) => formatter.debug_tuple("Memory").field(error).finish(),
            Self::OwnerGenerationExhausted { .. } => formatter
                .debug_struct("OwnerGenerationExhausted")
                .finish_non_exhaustive(),
            Self::KfdSessionNotFresh { .. } => formatter
                .debug_struct("KfdSessionNotFresh")
                .finish_non_exhaustive(),
        }
    }
}

impl fmt::Display for ServiceAllocationAcquireErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl core::error::Error for ServiceAllocationAcquireErrorV1 {}

impl ServiceAllocationAcquireErrorV1 {
    /// Recovers a KFD session retained after local owner-generation exhaustion.
    ///
    /// Memory-session acquisition failure has no successfully acquired session
    /// and therefore returns `None`.
    pub fn into_retained_session(self) -> Option<SharedGttMemorySessionV1> {
        match self {
            Self::Memory(_) => None,
            Self::OwnerGenerationExhausted { retained_session }
            | Self::KfdSessionNotFresh { retained_session } => Some(*retained_session),
        }
    }
}

/// Fail-closed allocation, mapping, range, or teardown error.
#[derive(Debug)]
pub enum ServiceAllocationErrorV1 {
    /// This owner was quarantined after a lost or ambiguous transition.
    Quarantined,
    /// The fixed live-allocation bound was exhausted.
    AllocationCapacity {
        /// Fixed maximum live records.
        maximum: usize,
    },
    /// The requested byte extent was zero or could not be represented.
    InvalidExtent,
    /// The fixed per-kind retained-byte bound would be exceeded.
    ByteCapacity {
        /// Fixed maximum retained requested bytes for the allocation kind.
        maximum_bytes: u64,
    },
    /// The host ledger could not reserve space before entering KFD.
    AllocationRegistryReservation,
    /// Alignment was zero, non-power-of-two, or above the fixed maximum.
    InvalidAlignment,
    /// The key belongs to another owner, device binding, or VM binding.
    OwnerBindingMismatch,
    /// The allocation id or generation was stale or substituted.
    AllocationGenerationMismatch,
    /// The compile-time role did not match the retained role.
    RoleMismatch,
    /// The compile-time kind did not match the retained kind.
    KindMismatch,
    /// The allocation was not in the required mapping state.
    AllocationState,
    /// A requested range was empty, out of bounds, or misaligned.
    InvalidRange,
    /// Two ranges overlap or belong to different allocations.
    AliasingRange,
    /// The underlying production KFD memory transition failed.
    Memory(MemorySessionError),
}

impl fmt::Display for ServiceAllocationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl core::error::Error for ServiceAllocationErrorV1 {}

impl From<MemorySessionError> for ServiceAllocationErrorV1 {
    fn from(value: MemorySessionError) -> Self {
        Self::Memory(value)
    }
}

/// Redacted evidence that all never-published allocations were released.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceAllocationReleaseObservationV1 {
    allocation_count: usize,
    device_bytes: u64,
    host_bytes: u64,
}

impl ServiceAllocationReleaseObservationV1 {
    /// Returns the number of allocations explicitly unmapped and freed.
    pub const fn allocation_count(self) -> usize {
        self.allocation_count
    }

    /// Returns the total requested device-local bytes released.
    pub const fn device_bytes(self) -> u64 {
        self.device_bytes
    }

    /// Returns the total requested host-visible bytes released.
    pub const fn host_bytes(self) -> u64 {
        self.host_bytes
    }
}

/// Retained fail-closed ownership after a cleanup transition failed.
///
/// This value exposes no retry or release method. The contained KFD session
/// retains every unreleased native record for process-level quarantine. A
/// typed allocation token consumed by the failed transition is not recovered.
#[must_use = "quarantined KFD allocation authority must remain retained"]
pub struct QuarantinedServiceAllocationsV1 {
    owner: AllocationOwnerV1,
}

impl fmt::Debug for QuarantinedServiceAllocationsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuarantinedServiceAllocationsV1")
            .field("allocation_count", &self.owner.allocations.len())
            .field("device_bytes", &self.owner.device_bytes)
            .field("host_bytes", &self.owner.host_bytes)
            .finish_non_exhaustive()
    }
}

impl QuarantinedServiceAllocationsV1 {
    /// Returns the number of service records still retained beside the KFD session.
    pub fn retained_allocation_count(&self) -> usize {
        self.owner.allocations.len()
    }

    /// Returns the retained requested device-local bytes.
    pub const fn retained_device_bytes(&self) -> u64 {
        self.owner.device_bytes
    }

    /// Returns the retained requested host-visible bytes.
    pub const fn retained_host_bytes(&self) -> u64 {
        self.owner.host_bytes
    }
}

/// A release error paired with the still-owned quarantined KFD session.
#[must_use = "release failure retains the KFD session and native records"]
pub struct ServiceAllocationReleaseFailureV1 {
    error: ServiceAllocationErrorV1,
    retained: QuarantinedServiceAllocationsV1,
}

impl fmt::Debug for ServiceAllocationReleaseFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceAllocationReleaseFailureV1")
            .field("error", &self.error)
            .field("retained", &self.retained)
            .finish()
    }
}

impl ServiceAllocationReleaseFailureV1 {
    /// Returns the release error without discarding retained authority.
    pub const fn error(&self) -> &ServiceAllocationErrorV1 {
        &self.error
    }

    /// Returns the quarantined KFD allocation owner.
    pub fn into_retained(self) -> QuarantinedServiceAllocationsV1 {
        self.retained
    }
}

impl ServiceAllocationSessionV1 {
    /// Acquires a real shared KFD VM from an already checked gfx942 device.
    pub fn acquire(
        device: CheckedGfx942XnackMinusDevice,
    ) -> Result<Self, ServiceAllocationAcquireErrorV1> {
        let session = device
            .acquire_shared_gtt_memory_session()
            .map_err(ServiceAllocationAcquireErrorV1::Memory)?;
        Self::from_kfd_session(session)
    }

    /// Takes ownership of an existing real shared KFD VM session.
    ///
    /// The session must be active and contain no retained shared or
    /// device-local allocation record. Rejection returns the exact session in
    /// [`ServiceAllocationAcquireErrorV1::KfdSessionNotFresh`]; callers recover
    /// it by consuming the error with
    /// [`ServiceAllocationAcquireErrorV1::into_retained_session`].
    pub fn from_kfd_session(
        session: SharedGttMemorySessionV1,
    ) -> Result<Self, ServiceAllocationAcquireErrorV1> {
        if !is_fresh_kfd_session(
            session.phase(),
            session.retained_allocation_count(),
            session.retained_device_memory_lease_count(),
        ) {
            return Err(ServiceAllocationAcquireErrorV1::KfdSessionNotFresh {
                retained_session: Box::new(session),
            });
        }
        let owner_generation = match NEXT_OWNER_GENERATION_V1.fetch_update(
            Ordering::Relaxed,
            Ordering::Relaxed,
            |value| value.checked_add(3),
        ) {
            Ok(generation) => generation,
            Err(_) => {
                return Err(ServiceAllocationAcquireErrorV1::OwnerGenerationExhausted {
                    retained_session: Box::new(session),
                });
            }
        };
        // The successful fetch-update proved that adding three is valid.
        let device_owner_generation = owner_generation + 1;
        let vm_owner_generation = owner_generation + 2;
        Ok(Self {
            owner: AllocationOwnerV1 {
                session,
                owner: OwnerBindingV1 {
                    owner_generation,
                    device_owner_generation,
                    vm_owner_generation,
                },
                phase: ServiceAllocationPhaseV1::Active,
                next_allocation_id: 1,
                allocations: Vec::new(),
                device_bytes: 0,
                host_bytes: 0,
            },
            quiescence: PhantomData,
        })
    }

    /// Returns the fail-closed owner phase.
    pub const fn phase(&self) -> ServiceAllocationPhaseV1 {
        self.owner.phase
    }

    /// Returns the number of live typed allocation records.
    pub fn allocation_count(&self) -> usize {
        self.owner.allocations.len()
    }

    /// Allocates uninitialized device-local VRAM/HBM from the retained KFD session.
    ///
    /// ```compile_fail
    /// use fe2o3_service_host::{HostUploadRoleV1, ServiceAllocationSessionV1};
    ///
    /// fn host_role_cannot_be_device_local(owner: &mut ServiceAllocationSessionV1) {
    ///     let _ = owner.allocate_device_local::<HostUploadRoleV1>(4096, 4096);
    /// }
    /// ```
    pub fn allocate_device_local<R>(
        &mut self,
        requested_bytes: u64,
        alignment: u64,
    ) -> Result<ServiceAllocationKeyV1<R, DeviceLocalAllocationV1>, ServiceAllocationErrorV1>
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        self.require_active()?;
        self.preflight_allocation(requested_bytes, alignment, AllocationKindV1::DeviceLocal)?;
        self.owner
            .allocations
            .try_reserve(1)
            .map_err(|_| ServiceAllocationErrorV1::AllocationRegistryReservation)?;
        let binding =
            self.reserve_binding::<R, DeviceLocalAllocationV1>(requested_bytes, alignment)?;
        let token = match self
            .owner
            .session
            .allocate_gfx942_device_memory(requested_bytes, alignment)
        {
            Ok(token) => token,
            Err(error) => {
                self.sync_phase_after_nonconsuming_failure();
                return Err(error.into());
            }
        };
        self.owner.allocations.push(OwnedAllocationV1 {
            binding,
            token: Some(AllocationTokenV1::DeviceUnmapped(token)),
        });
        self.owner.device_bytes += requested_bytes;
        Ok(key(binding))
    }

    /// Maps one exact uninitialized device-local allocation to the owned GPU/VM.
    pub fn map_device_local<R>(
        &mut self,
        key: ServiceAllocationKeyV1<R, DeviceLocalAllocationV1>,
    ) -> Result<ServiceAllocationRangeV1<R, DeviceLocalAllocationV1>, ServiceAllocationErrorV1>
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        self.require_active()?;
        let index = self.validate_key(key)?;
        let token = self.take_token(index)?;
        let AllocationTokenV1::DeviceUnmapped(token) = token else {
            self.owner.allocations[index].token = Some(token);
            return Err(ServiceAllocationErrorV1::AllocationState);
        };
        match self.owner.session.map_gfx942_device_memory(token) {
            Ok(mapped) => {
                self.owner.allocations[index].token = Some(AllocationTokenV1::DeviceMapped(mapped));
                self.full_range(key)
            }
            Err(error) => {
                self.owner.phase = ServiceAllocationPhaseV1::Quarantined;
                Err(error.into())
            }
        }
    }

    /// Allocates CPU-writable host-visible coherent GTT from the retained KFD session.
    pub fn allocate_host_visible<R>(
        &mut self,
        requested_bytes: usize,
    ) -> Result<ServiceAllocationKeyV1<R, HostVisibleAllocationV1>, ServiceAllocationErrorV1>
    where
        R: HostAllocationRoleMarkerV1,
    {
        self.require_active()?;
        let requested_u64 =
            u64::try_from(requested_bytes).map_err(|_| ServiceAllocationErrorV1::InvalidExtent)?;
        self.preflight_allocation(
            requested_u64,
            MAX_SERVICE_ALIGNMENT_V1,
            AllocationKindV1::HostVisible,
        )?;
        self.owner
            .allocations
            .try_reserve(1)
            .map_err(|_| ServiceAllocationErrorV1::AllocationRegistryReservation)?;
        let binding = self.reserve_binding::<R, HostVisibleAllocationV1>(
            requested_u64,
            MAX_SERVICE_ALIGNMENT_V1,
        )?;
        let token = match self
            .owner
            .session
            .allocate_host_visible_coherent(requested_bytes)
        {
            Ok(token) => token,
            Err(error) => {
                self.sync_phase_after_nonconsuming_failure();
                return Err(error.into());
            }
        };
        self.owner.allocations.push(OwnedAllocationV1 {
            binding,
            token: Some(AllocationTokenV1::HostCpuWritable(token)),
        });
        self.owner.host_bytes += requested_u64;
        Ok(key(binding))
    }

    /// Provides scoped mutable CPU access before a host-visible allocation is mapped.
    ///
    /// Successful writes initialize only the host-visible bytes touched by the
    /// closure. The owner exposes no direct persistent-pointer accessor. A safe
    /// callback can nevertheless derive and retain or return a raw CPU pointer
    /// or numerical address from the slice. Rust provides no safe dereference
    /// after the callback borrow ends; unsafe later use is outside this owner's
    /// guarantees. These writes establish no device-local content or copy
    /// completion.
    ///
    /// ```no_run
    /// use fe2o3_service_host::{
    ///     HostUploadRoleV1, HostVisibleAllocationV1, ServiceAllocationErrorV1,
    ///     ServiceAllocationKeyV1, ServiceAllocationSessionV1,
    /// };
    ///
    /// fn retain_raw_cpu_pointer(
    ///     owner: &mut ServiceAllocationSessionV1,
    ///     key: ServiceAllocationKeyV1<HostUploadRoleV1, HostVisibleAllocationV1>,
    /// ) -> Result<*mut u8, ServiceAllocationErrorV1> {
    ///     owner.with_host_bytes_mut(key, |bytes| bytes.as_mut_ptr())
    /// }
    /// ```
    pub fn with_host_bytes_mut<R, T>(
        &mut self,
        key: ServiceAllocationKeyV1<R, HostVisibleAllocationV1>,
        write: impl FnOnce(&mut [u8]) -> T,
    ) -> Result<T, ServiceAllocationErrorV1>
    where
        R: HostAllocationRoleMarkerV1,
    {
        self.require_active()?;
        let index = self.validate_key(key)?;
        let (session, allocations) = (&mut self.owner.session, &mut self.owner.allocations);
        let token = allocations[index]
            .token
            .as_mut()
            .ok_or(ServiceAllocationErrorV1::Quarantined)?;
        let AllocationTokenV1::HostCpuWritable(token) = token else {
            return Err(ServiceAllocationErrorV1::AllocationState);
        };
        let result = session.with_bytes_mut(token, |bytes| apply_scoped_host_write(bytes, write));
        if result.is_err() && session.phase() == SharedMemorySessionPhaseV1::Quarantined {
            self.owner.phase = ServiceAllocationPhaseV1::Quarantined;
        }
        result.map_err(Into::into)
    }

    /// Maps one exact host-visible coherent allocation to the owned GPU/VM.
    pub fn map_host_visible<R>(
        &mut self,
        key: ServiceAllocationKeyV1<R, HostVisibleAllocationV1>,
    ) -> Result<ServiceAllocationRangeV1<R, HostVisibleAllocationV1>, ServiceAllocationErrorV1>
    where
        R: HostAllocationRoleMarkerV1,
    {
        self.require_active()?;
        let index = self.validate_key(key)?;
        let token = self.take_token(index)?;
        let AllocationTokenV1::HostCpuWritable(token) = token else {
            self.owner.allocations[index].token = Some(token);
            return Err(ServiceAllocationErrorV1::AllocationState);
        };
        match self.owner.session.map_to_gpu(token) {
            Ok(mapped) => {
                self.owner.allocations[index].token = Some(AllocationTokenV1::HostMapped(mapped));
                self.full_range(key)
            }
            Err(error) => {
                self.owner.phase = ServiceAllocationPhaseV1::Quarantined;
                Err(error.into())
            }
        }
    }

    /// Checks one mapped, nonempty, aligned typed subrange.
    pub fn range<R, K>(
        &self,
        key: ServiceAllocationKeyV1<R, K>,
        offset_bytes: u64,
        extent_bytes: u64,
        alignment: u64,
    ) -> Result<ServiceAllocationRangeV1<R, K>, ServiceAllocationErrorV1>
    where
        R: ServiceAllocationRoleMarkerV1,
        K: ServiceAllocationKindMarkerV1,
    {
        self.require_active()?;
        let index = self.validate_key(key)?;
        if !is_mapped(self.owner.allocations[index].token.as_ref()) {
            return Err(ServiceAllocationErrorV1::AllocationState);
        }
        if extent_bytes == 0
            || alignment == 0
            || !alignment.is_power_of_two()
            || alignment > key.binding.alignment
            || !offset_bytes.is_multiple_of(alignment)
            || offset_bytes
                .checked_add(extent_bytes)
                .is_none_or(|end| end > key.binding.extent_bytes)
        {
            return Err(ServiceAllocationErrorV1::InvalidRange);
        }
        Ok(ServiceAllocationRangeV1 {
            key,
            offset_bytes,
            extent_bytes,
            alignment,
        })
    }

    /// Checks two non-overlapping subranges of the same mapped allocation.
    pub fn disjoint_ranges<R, K>(
        &self,
        key: ServiceAllocationKeyV1<R, K>,
        left: (u64, u64, u64),
        right: (u64, u64, u64),
    ) -> Result<ServiceAllocationRangePairV1<R, K>, ServiceAllocationErrorV1>
    where
        R: ServiceAllocationRoleMarkerV1,
        K: ServiceAllocationKindMarkerV1,
    {
        let left = self.range(key, left.0, left.1, left.2)?;
        let right = self.range(key, right.0, right.1, right.2)?;
        let left_end = left
            .offset_bytes
            .checked_add(left.extent_bytes)
            .ok_or(ServiceAllocationErrorV1::InvalidRange)?;
        let right_end = right
            .offset_bytes
            .checked_add(right.extent_bytes)
            .ok_or(ServiceAllocationErrorV1::InvalidRange)?;
        if left.offset_bytes < right_end && right.offset_bytes < left_end {
            return Err(ServiceAllocationErrorV1::AliasingRange);
        }
        Ok((left, right))
    }

    /// Explicitly unmaps and frees every never-published allocation in reverse order.
    ///
    /// No GPU address or queue authority is exposed by this type, so
    /// `NeverPublishedV1` is GPU quiescence by construction. This does not
    /// provide guarantees about unsafe later use of a raw CPU pointer that a
    /// safe callback retained from its scoped slice. This method is not
    /// available on a future in-flight owner; such a transition must consume
    /// exact completion authority at the private KFD dispatch boundary.
    ///
    /// ```compile_fail
    /// use fe2o3_service_host::ServiceAllocationSessionV1;
    ///
    /// fn cannot_release_twice(owner: ServiceAllocationSessionV1) {
    ///     owner.release_unpublished().unwrap();
    ///     owner.release_unpublished().unwrap();
    /// }
    /// ```
    pub fn release_unpublished(
        mut self,
    ) -> Result<ServiceAllocationReleaseObservationV1, ServiceAllocationReleaseFailureV1> {
        let observation = ServiceAllocationReleaseObservationV1 {
            allocation_count: self.owner.allocations.len(),
            device_bytes: self.owner.device_bytes,
            host_bytes: self.owner.host_bytes,
        };
        if self.owner.phase != ServiceAllocationPhaseV1::Active {
            return Err(self.into_release_failure(ServiceAllocationErrorV1::Quarantined));
        }
        while let Some(mut allocation) = self.owner.allocations.pop() {
            let remaining_bytes =
                match remaining_bytes_after_release(&self.owner, allocation.binding) {
                    Ok(remaining_bytes) => remaining_bytes,
                    Err(error) => {
                        self.owner.allocations.push(allocation);
                        return Err(self.into_release_failure(error));
                    }
                };
            let Some(token) = allocation.token.take() else {
                self.owner.allocations.push(allocation);
                return Err(self.into_release_failure(ServiceAllocationErrorV1::Quarantined));
            };
            if let Err(error) = release_token(&mut self.owner.session, token) {
                self.owner.allocations.push(allocation);
                return Err(self.into_release_failure(error));
            }
            self.owner.device_bytes = remaining_bytes.0;
            self.owner.host_bytes = remaining_bytes.1;
        }
        Ok(observation)
    }

    fn require_active(&self) -> Result<(), ServiceAllocationErrorV1> {
        if self.owner.phase == ServiceAllocationPhaseV1::Active {
            Ok(())
        } else {
            Err(ServiceAllocationErrorV1::Quarantined)
        }
    }

    fn preflight_allocation(
        &self,
        extent_bytes: u64,
        alignment: u64,
        kind: AllocationKindV1,
    ) -> Result<(), ServiceAllocationErrorV1> {
        if self.owner.allocations.len() >= MAX_SERVICE_ALLOCATIONS_V1 {
            return Err(ServiceAllocationErrorV1::AllocationCapacity {
                maximum: MAX_SERVICE_ALLOCATIONS_V1,
            });
        }
        if extent_bytes == 0 {
            return Err(ServiceAllocationErrorV1::InvalidExtent);
        }
        if alignment == 0 || !alignment.is_power_of_two() || alignment > MAX_SERVICE_ALIGNMENT_V1 {
            return Err(ServiceAllocationErrorV1::InvalidAlignment);
        }
        let (retained, maximum) = match kind {
            AllocationKindV1::DeviceLocal => (self.owner.device_bytes, MAX_SERVICE_DEVICE_BYTES_V1),
            AllocationKindV1::HostVisible => (self.owner.host_bytes, MAX_SERVICE_HOST_BYTES_V1),
        };
        if retained
            .checked_add(extent_bytes)
            .is_none_or(|total| total > maximum)
        {
            return Err(ServiceAllocationErrorV1::ByteCapacity {
                maximum_bytes: maximum,
            });
        }
        Ok(())
    }

    fn reserve_binding<R, K>(
        &mut self,
        extent_bytes: u64,
        alignment: u64,
    ) -> Result<AllocationBindingV1, ServiceAllocationErrorV1>
    where
        R: ServiceAllocationRoleMarkerV1,
        K: ServiceAllocationKindMarkerV1,
    {
        let id = self.owner.next_allocation_id;
        self.owner.next_allocation_id = id
            .checked_add(1)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        Ok(AllocationBindingV1 {
            owner: self.owner.owner,
            id,
            generation: 1,
            role_id: R::ROLE_ID,
            kind_id: K::KIND_ID,
            extent_bytes,
            alignment,
        })
    }

    fn validate_key<R, K>(
        &self,
        key: ServiceAllocationKeyV1<R, K>,
    ) -> Result<usize, ServiceAllocationErrorV1>
    where
        R: ServiceAllocationRoleMarkerV1,
        K: ServiceAllocationKindMarkerV1,
    {
        if key.binding.owner != self.owner.owner {
            return Err(ServiceAllocationErrorV1::OwnerBindingMismatch);
        }
        if key.binding.role_id != R::ROLE_ID {
            return Err(ServiceAllocationErrorV1::RoleMismatch);
        }
        if key.binding.kind_id != K::KIND_ID {
            return Err(ServiceAllocationErrorV1::KindMismatch);
        }
        let allocation = self
            .owner
            .allocations
            .iter()
            .position(|allocation| allocation.binding.id == key.binding.id)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        if self.owner.allocations[allocation].binding != key.binding {
            return Err(ServiceAllocationErrorV1::AllocationGenerationMismatch);
        }
        Ok(allocation)
    }

    fn take_token(&mut self, index: usize) -> Result<AllocationTokenV1, ServiceAllocationErrorV1> {
        self.owner.allocations[index]
            .token
            .take()
            .ok_or(ServiceAllocationErrorV1::Quarantined)
    }

    fn full_range<R, K>(
        &self,
        key: ServiceAllocationKeyV1<R, K>,
    ) -> Result<ServiceAllocationRangeV1<R, K>, ServiceAllocationErrorV1>
    where
        R: ServiceAllocationRoleMarkerV1,
        K: ServiceAllocationKindMarkerV1,
    {
        self.range(key, 0, key.binding.extent_bytes, key.binding.alignment)
    }

    fn sync_phase_after_nonconsuming_failure(&mut self) {
        if self.owner.session.phase() == SharedMemorySessionPhaseV1::Quarantined {
            self.owner.phase = ServiceAllocationPhaseV1::Quarantined;
        }
    }

    fn into_release_failure(
        mut self,
        error: ServiceAllocationErrorV1,
    ) -> ServiceAllocationReleaseFailureV1 {
        self.owner.phase = ServiceAllocationPhaseV1::Quarantined;
        ServiceAllocationReleaseFailureV1 {
            error,
            retained: QuarantinedServiceAllocationsV1 { owner: self.owner },
        }
    }
}

fn key<R, K>(binding: AllocationBindingV1) -> ServiceAllocationKeyV1<R, K>
where
    R: ServiceAllocationRoleMarkerV1,
    K: ServiceAllocationKindMarkerV1,
{
    ServiceAllocationKeyV1 {
        binding,
        marker: PhantomData,
    }
}

fn apply_scoped_host_write<T>(bytes: &mut [u8], write: impl FnOnce(&mut [u8]) -> T) -> T {
    write(bytes)
}

fn remaining_bytes_after_release(
    owner: &AllocationOwnerV1,
    binding: AllocationBindingV1,
) -> Result<(u64, u64), ServiceAllocationErrorV1> {
    if binding.kind_id == AllocationKindV1::DeviceLocal as u8 {
        let device_bytes = owner
            .device_bytes
            .checked_sub(binding.extent_bytes)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        Ok((device_bytes, owner.host_bytes))
    } else if binding.kind_id == AllocationKindV1::HostVisible as u8 {
        let host_bytes = owner
            .host_bytes
            .checked_sub(binding.extent_bytes)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        Ok((owner.device_bytes, host_bytes))
    } else {
        Err(ServiceAllocationErrorV1::KindMismatch)
    }
}

fn is_mapped(token: Option<&AllocationTokenV1>) -> bool {
    matches!(
        token,
        Some(AllocationTokenV1::DeviceMapped(_) | AllocationTokenV1::HostMapped(_))
    )
}

fn is_fresh_kfd_session(
    phase: SharedMemorySessionPhaseV1,
    shared_allocation_count: usize,
    device_allocation_count: usize,
) -> bool {
    phase == SharedMemorySessionPhaseV1::Active
        && shared_allocation_count == 0
        && device_allocation_count == 0
}

fn release_token(
    session: &mut SharedGttMemorySessionV1,
    token: AllocationTokenV1,
) -> Result<(), ServiceAllocationErrorV1> {
    match token {
        AllocationTokenV1::DeviceUnmapped(token) => session
            .release_gfx942_device_memory(token)
            .map_err(Into::into),
        AllocationTokenV1::DeviceMapped(token) => {
            let token = session
                .unmap_gfx942_device_memory(token)
                .map_err(ServiceAllocationErrorV1::Memory)?;
            session
                .release_gfx942_device_memory(token)
                .map_err(Into::into)
        }
        AllocationTokenV1::HostCpuWritable(token) => session.release(token).map_err(Into::into),
        AllocationTokenV1::HostMapped(token) => {
            let token = session
                .unmap_from_gpu(token)
                .map_err(ServiceAllocationErrorV1::Memory)?;
            session.release(token).map_err(Into::into)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use alloc::vec;
    use core::fmt::Write as _;
    use sha2::{Digest, Sha256};

    #[test]
    fn allocation_manifest_hash_and_role_inventory_are_frozen() {
        let mut actual = String::new();
        for byte in Sha256::digest(SERVICE_ALLOCATION_OWNERSHIP_MANIFEST_V1) {
            write!(&mut actual, "{byte:02x}").unwrap();
        }
        assert_eq!(actual, SERVICE_ALLOCATION_OWNERSHIP_MANIFEST_SHA256_V1);
        assert_eq!(
            DEVICE_LOCAL_ALLOCATION_ROLES_V1,
            [
                "device-input",
                "device-state",
                "device-workspace",
                "device-output"
            ]
        );
        assert_eq!(
            HOST_VISIBLE_ALLOCATION_ROLES_V1,
            ["host-upload", "host-download"]
        );
        assert_eq!(MAX_SERVICE_ALIGNMENT_V1, HOST_VISIBLE_MEMORY_PAGE_BYTES_V1);
    }

    #[test]
    fn only_active_empty_kfd_sessions_cross_the_ownership_boundary() {
        assert!(is_fresh_kfd_session(
            SharedMemorySessionPhaseV1::Active,
            0,
            0
        ));
        for observation in [
            (SharedMemorySessionPhaseV1::Quarantined, 0, 0),
            (SharedMemorySessionPhaseV1::Active, 1, 0),
            (SharedMemorySessionPhaseV1::Active, 0, 1),
            (SharedMemorySessionPhaseV1::Quarantined, 1, 1),
        ] {
            assert!(!is_fresh_kfd_session(
                observation.0,
                observation.1,
                observation.2
            ));
        }
    }

    fn binding(role: AllocationRoleV1, kind: AllocationKindV1) -> AllocationBindingV1 {
        AllocationBindingV1 {
            owner: OwnerBindingV1 {
                owner_generation: 11,
                device_owner_generation: 12,
                vm_owner_generation: 13,
            },
            id: 1,
            generation: 1,
            role_id: role as u8,
            kind_id: kind as u8,
            extent_bytes: 16_384,
            alignment: 4_096,
        }
    }

    fn validate_binding<R, K>(
        expected_owner: OwnerBindingV1,
        expected: AllocationBindingV1,
        supplied: ServiceAllocationKeyV1<R, K>,
    ) -> Result<(), ServiceAllocationErrorV1>
    where
        R: ServiceAllocationRoleMarkerV1,
        K: ServiceAllocationKindMarkerV1,
    {
        if supplied.binding.owner != expected_owner {
            return Err(ServiceAllocationErrorV1::OwnerBindingMismatch);
        }
        if supplied.binding.role_id != R::ROLE_ID {
            return Err(ServiceAllocationErrorV1::RoleMismatch);
        }
        if supplied.binding.kind_id != K::KIND_ID {
            return Err(ServiceAllocationErrorV1::KindMismatch);
        }
        if supplied.binding != expected {
            return Err(ServiceAllocationErrorV1::AllocationGenerationMismatch);
        }
        Ok(())
    }

    #[test]
    fn owner_device_vm_and_allocation_generation_drift_fail_closed() {
        let expected = binding(AllocationRoleV1::DeviceState, AllocationKindV1::DeviceLocal);
        for mutate in [
            |value: &mut AllocationBindingV1| value.owner.owner_generation += 1,
            |value: &mut AllocationBindingV1| value.owner.device_owner_generation += 1,
            |value: &mut AllocationBindingV1| value.owner.vm_owner_generation += 1,
        ] {
            let mut supplied = expected;
            mutate(&mut supplied);
            let result = validate_binding(
                expected.owner,
                expected,
                key::<DeviceStateRoleV1, DeviceLocalAllocationV1>(supplied),
            );
            assert!(matches!(
                result,
                Err(ServiceAllocationErrorV1::OwnerBindingMismatch)
            ));
        }

        let mut stale = expected;
        stale.generation += 1;
        assert!(matches!(
            validate_binding(
                expected.owner,
                expected,
                key::<DeviceStateRoleV1, DeviceLocalAllocationV1>(stale),
            ),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
    }

    #[test]
    fn role_kind_extent_and_alignment_drift_fail_closed() {
        let expected = binding(AllocationRoleV1::DeviceState, AllocationKindV1::DeviceLocal);

        let mut wrong_role = expected;
        wrong_role.role_id = AllocationRoleV1::DeviceInput as u8;
        assert!(matches!(
            validate_binding(
                expected.owner,
                expected,
                key::<DeviceStateRoleV1, DeviceLocalAllocationV1>(wrong_role),
            ),
            Err(ServiceAllocationErrorV1::RoleMismatch)
        ));

        let mut wrong_kind = expected;
        wrong_kind.kind_id = AllocationKindV1::HostVisible as u8;
        assert!(matches!(
            validate_binding(
                expected.owner,
                expected,
                key::<DeviceStateRoleV1, DeviceLocalAllocationV1>(wrong_kind),
            ),
            Err(ServiceAllocationErrorV1::KindMismatch)
        ));

        for mutate in [
            |value: &mut AllocationBindingV1| value.extent_bytes += 1,
            |value: &mut AllocationBindingV1| value.alignment /= 2,
        ] {
            let mut supplied = expected;
            mutate(&mut supplied);
            assert!(matches!(
                validate_binding(
                    expected.owner,
                    expected,
                    key::<DeviceStateRoleV1, DeviceLocalAllocationV1>(supplied),
                ),
                Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
            ));
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum TestReleasePhase {
        NeverPublished,
        InFlight,
        Quiescent,
        Released,
        Quarantined,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct TestReleaseRecord {
        id: u64,
        kind: AllocationKindV1,
        extent_bytes: u64,
    }

    struct TestReleaseLedger {
        phase: TestReleasePhase,
        live: Vec<TestReleaseRecord>,
        device_bytes: u64,
        host_bytes: u64,
    }

    impl TestReleaseLedger {
        fn release(&mut self, fail_at: Option<u64>) -> Result<Vec<u64>, ServiceAllocationErrorV1> {
            if !matches!(
                self.phase,
                TestReleasePhase::NeverPublished | TestReleasePhase::Quiescent
            ) {
                return Err(ServiceAllocationErrorV1::AllocationState);
            }
            let mut released = Vec::new();
            while let Some(record) = self.live.pop() {
                if fail_at == Some(record.id) {
                    self.live.push(record);
                    self.phase = TestReleasePhase::Quarantined;
                    return Err(ServiceAllocationErrorV1::Quarantined);
                }
                match record.kind {
                    AllocationKindV1::DeviceLocal => {
                        self.device_bytes -= record.extent_bytes;
                    }
                    AllocationKindV1::HostVisible => {
                        self.host_bytes -= record.extent_bytes;
                    }
                }
                released.push(record.id);
            }
            self.phase = TestReleasePhase::Released;
            Ok(released)
        }
    }

    #[test]
    fn release_before_quiescence_and_double_release_are_rejected() {
        let mut in_flight = TestReleaseLedger {
            phase: TestReleasePhase::InFlight,
            live: vec![TestReleaseRecord {
                id: 1,
                kind: AllocationKindV1::DeviceLocal,
                extent_bytes: 4_096,
            }],
            device_bytes: 4_096,
            host_bytes: 0,
        };
        assert!(matches!(
            in_flight.release(None),
            Err(ServiceAllocationErrorV1::AllocationState)
        ));
        assert_eq!(in_flight.live.len(), 1);
        assert_eq!(in_flight.device_bytes, 4_096);

        let mut quiescent = TestReleaseLedger {
            phase: TestReleasePhase::Quiescent,
            live: vec![TestReleaseRecord {
                id: 1,
                kind: AllocationKindV1::DeviceLocal,
                extent_bytes: 4_096,
            }],
            device_bytes: 4_096,
            host_bytes: 0,
        };
        assert_eq!(quiescent.release(None).unwrap(), vec![1]);
        assert_eq!(quiescent.device_bytes, 0);
        assert!(matches!(
            quiescent.release(None),
            Err(ServiceAllocationErrorV1::AllocationState)
        ));
    }

    #[test]
    fn partial_unwind_is_reverse_order_and_retains_failure_and_older_records() {
        let mut ledger = TestReleaseLedger {
            phase: TestReleasePhase::NeverPublished,
            live: vec![
                TestReleaseRecord {
                    id: 1,
                    kind: AllocationKindV1::DeviceLocal,
                    extent_bytes: 4_096,
                },
                TestReleaseRecord {
                    id: 2,
                    kind: AllocationKindV1::HostVisible,
                    extent_bytes: 8_192,
                },
                TestReleaseRecord {
                    id: 3,
                    kind: AllocationKindV1::DeviceLocal,
                    extent_bytes: 16_384,
                },
                TestReleaseRecord {
                    id: 4,
                    kind: AllocationKindV1::HostVisible,
                    extent_bytes: 32_768,
                },
            ],
            device_bytes: 20_480,
            host_bytes: 40_960,
        };
        assert!(matches!(
            ledger.release(Some(2)),
            Err(ServiceAllocationErrorV1::Quarantined)
        ));
        assert_eq!(ledger.phase, TestReleasePhase::Quarantined);
        assert_eq!(
            ledger
                .live
                .iter()
                .map(|record| record.id)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(ledger.device_bytes, 4_096);
        assert_eq!(ledger.host_bytes, 8_192);
    }

    #[test]
    fn safe_scoped_host_write_may_return_a_raw_pointer_or_address() {
        let mut bytes = [0_u8; 16];
        let pointer = apply_scoped_host_write(&mut bytes, |slice| slice.as_mut_ptr());
        let address = apply_scoped_host_write(&mut bytes, |slice| slice.as_mut_ptr() as usize);

        assert_eq!(pointer, bytes.as_mut_ptr());
        assert_eq!(address, bytes.as_mut_ptr() as usize);
        // No API supplied by the borrow permits safe dereference after return.
    }

    #[test]
    fn overlapping_and_misaligned_range_descriptions_are_rejected() {
        fn checked(
            binding: AllocationBindingV1,
            offset: u64,
            extent: u64,
            alignment: u64,
        ) -> Result<(u64, u64), ServiceAllocationErrorV1> {
            if extent == 0
                || alignment == 0
                || !alignment.is_power_of_two()
                || alignment > binding.alignment
                || !offset.is_multiple_of(alignment)
                || offset
                    .checked_add(extent)
                    .is_none_or(|end| end > binding.extent_bytes)
            {
                return Err(ServiceAllocationErrorV1::InvalidRange);
            }
            Ok((offset, extent))
        }

        let retained = binding(AllocationRoleV1::DeviceState, AllocationKindV1::DeviceLocal);
        assert!(checked(retained, 2, 4_096, 4_096).is_err());
        assert!(checked(retained, 12_288, 8_192, 4_096).is_err());
        let left = checked(retained, 0, 8_192, 4_096).unwrap();
        let right = checked(retained, 4_096, 8_192, 4_096).unwrap();
        assert!(left.0 < right.0 + right.1 && right.0 < left.0 + left.1);
    }
}
