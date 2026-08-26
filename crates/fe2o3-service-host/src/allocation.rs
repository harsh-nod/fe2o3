//! Linear service ownership for real KFD-backed allocations.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU64, Ordering};

use fe2o3_kfd::{
    CheckedGfx942XnackMinusDevice, Gfx942DeviceContentDescriptorV1, Gfx942DeviceMemoryLeaseV1,
    Gfx942DeviceMemoryMappedV1, Gfx942DeviceMemoryUnmappedV1, Gfx942FixedDispatchDataKindV1,
    Gfx942FixedDispatchDataV1, Gfx942InitializedHostVisibleMemoryV1, Gfx942RepeatedByteContentV1,
    GttCpuWritableV1, GttGpuAccessibleMutableV1, HostVisibleCoherentGttV1, MemorySessionError,
    SharedGttAllocationV1, SharedGttMemorySessionV1, SharedMemorySessionPhaseV1,
    HOST_VISIBLE_MEMORY_PAGE_BYTES_V1,
};

/// Canonical scope and non-claims for the first service allocation owner.
pub const SERVICE_ALLOCATION_OWNERSHIP_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-service-allocation-owner-r8-v1\n",
    "backend=checked-gfx942-xnack-minus-device,shared-kfd-vm-session\n",
    "device=device-local-vram-hbm,linear-unmapped-mapped-or-fixed-dispatch-kfd-custody,optional-exact-host-verified-owned-image-or-private-recipe-complete-safe-slice-repeated-byte-public-device-local-initialization\n",
    "host=host-visible-coherent-gtt,linear-cpu-writable-gpu-mapped-sealed-full-initialized-or-fixed-dispatch-custody\n",
    "identity=service-scoped-process-local-owner-device-vm-allocation-labels-retained-beside-private-kfd-native-tokens\n",
    "views=typed-role-kind-offset-extent-alignment,no-handle-fd-gpu-address-or-persistent-raw-pointer-accessor\n",
    "cpu-write=scoped-mutable-slice-before-gpu-map,safe-caller-may-return-or-retain-raw-cpu-pointer-or-address,no-safe-post-borrow-dereference;separate-owned-full-extent-copy-or-bounded-memory-repeated-byte-fill-mints-sealed-initialized-mapped-authority\n",
    "dispatch-ranges=device-local-or-host-visible,owner-allocation-kind-generation-ordinal-offset-and-extent-bound,no-native-address,device-ordinals-before-host-ordinals;optional-host-snapshot-range-must-be-fully-initialized-and-strictly-enclose-one-same-generation-interior-range\n",
    "subleases=one-atomic-move-only-layout-per-allocation,typed-role-kind-and-exact-generation,pairwise-disjoint-nonempty-aligned-bounded-members,checked-member-contained-subranges,legacy-ranges-denied-after-partition,replacement-stales-old-layout\n",
    "bounds=32-live-allocations,device-192gib,host-2gib,page-and-device-alignment-max-4096\n",
    "release=gpu-never-published-or-exact-completed-recycled-queue-return,reverse-order-unmap-then-free,consuming-owner\n",
    "failure=preflight-retains-owner,consumed-token-failure-quarantines-retained-session,no-drop-cleanup\n",
    "excluded=caller-minted-initialization,persistent-mapped-cpu-borrow,full-write-coverage-from-dispatch,content-interpretation,effect-correctness-beyond-inspected-metadata,hardware-execution\n",
);

/// SHA-256 of [`SERVICE_ALLOCATION_OWNERSHIP_MANIFEST_V1`].
pub const SERVICE_ALLOCATION_OWNERSHIP_MANIFEST_SHA256_V1: &str =
    "e4a4a8d8a94615dbe4234f01fb8eaa19c5f222e8fba37eb0cb239f8c202baa09";

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
pub(crate) struct AllocationBindingV1 {
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
    sublease_index: Option<usize>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ServiceAllocationSubleaseRangeV1 {
    offset_bytes: u64,
    extent_bytes: u64,
    alignment: u64,
}

/// Move-only custody of one atomic, pairwise-disjoint allocation partition.
///
/// Native allocation ownership remains in [`ServiceAllocationSessionV1`] and
/// later moves intact into the queue ledger. This value is the unique public
/// witness for the logical partition recorded beside that native owner. It
/// cannot be cloned or forged, and an allocation accepts at most one layout.
///
/// ```compile_fail
/// use fe2o3_service_host::{
///     DeviceLocalAllocationV1, DeviceWorkspaceRoleV1, ServiceAllocationSubleaseSetV1,
/// };
///
/// fn cannot_clone(
///     subleases: ServiceAllocationSubleaseSetV1<
///         DeviceWorkspaceRoleV1,
///         DeviceLocalAllocationV1,
///         2,
///     >,
/// ) {
///     let _ = subleases.clone();
/// }
/// ```
#[must_use = "logical sublease custody must remain retained"]
pub struct ServiceAllocationSubleaseSetV1<R, K, const N: usize>
where
    R: ServiceAllocationRoleMarkerV1,
    K: ServiceAllocationKindMarkerV1,
{
    key: ServiceAllocationKeyV1<R, K>,
    ranges: [ServiceAllocationSubleaseRangeV1; N],
}

impl<R, K, const N: usize> fmt::Debug for ServiceAllocationSubleaseSetV1<R, K, N>
where
    R: ServiceAllocationRoleMarkerV1,
    K: ServiceAllocationKindMarkerV1,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceAllocationSubleaseSetV1")
            .field("role", &R::NAME)
            .field("kind", &K::NAME)
            .field("member_count", &N)
            .finish_non_exhaustive()
    }
}

impl<R, K, const N: usize> ServiceAllocationSubleaseSetV1<R, K, N>
where
    R: ServiceAllocationRoleMarkerV1,
    K: ServiceAllocationKindMarkerV1,
{
    /// Returns the fixed number of partition members.
    pub const fn len(&self) -> usize {
        N
    }

    /// Returns whether the fixed partition is empty.
    ///
    /// Successful construction always makes this `false`.
    pub const fn is_empty(&self) -> bool {
        N == 0
    }

    /// Returns one member's checked byte offset.
    pub fn offset_bytes(&self, index: usize) -> Option<u64> {
        self.ranges.get(index).map(|range| range.offset_bytes)
    }

    /// Returns one member's checked byte extent.
    pub fn extent_bytes(&self, index: usize) -> Option<u64> {
        self.ranges.get(index).map(|range| range.extent_bytes)
    }

    /// Returns one member's checked address alignment.
    pub fn alignment(&self, index: usize) -> Option<u64> {
        self.ranges.get(index).map(|range| range.alignment)
    }
}

/// A pair of checked typed ranges from one allocation.
pub type ServiceAllocationRangePairV1<R, K> = (
    ServiceAllocationRangeV1<R, K>,
    ServiceAllocationRangeV1<R, K>,
);

pub(crate) enum AllocationTokenV1 {
    DeviceUnmapped(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>),
    DeviceMapped(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>),
    FixedDispatch(Gfx942FixedDispatchDataV1),
    HostCpuWritable(SharedGttAllocationV1<HostVisibleCoherentGttV1, GttCpuWritableV1>),
    HostMapped(SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>),
    HostMappedInitialized(Gfx942InitializedHostVisibleMemoryV1),
}

pub(crate) struct OwnedAllocationV1 {
    binding: AllocationBindingV1,
    token: Option<AllocationTokenV1>,
    subleases: Option<Vec<ServiceAllocationSubleaseRangeV1>>,
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

/// Addressless, owner-checked device range admitted for service batch binding.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ServiceDeviceDispatchRangeV1 {
    pub(crate) binding: AllocationBindingV1,
    pub(crate) data_index: usize,
    pub(crate) offset_bytes: u64,
    pub(crate) extent_bytes: u64,
    pub(crate) sublease_index: Option<usize>,
}

impl fmt::Debug for ServiceDeviceDispatchRangeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceDeviceDispatchRangeV1")
            .field("data_index", &self.data_index)
            .field("offset_bytes", &self.offset_bytes)
            .field("extent_bytes", &self.extent_bytes)
            .finish_non_exhaustive()
    }
}

impl ServiceDeviceDispatchRangeV1 {
    /// Returns the checked byte offset without exposing a device address.
    pub const fn offset_bytes(self) -> u64 {
        self.offset_bytes
    }

    /// Returns the checked byte extent.
    pub const fn extent_bytes(self) -> u64 {
        self.extent_bytes
    }

    /// Narrows this checked range without changing its allocation generation,
    /// data ordinal, or retained sublease-member binding.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceAllocationErrorV1::InvalidRange`] unless the result is
    /// nonempty, wholly contained in this range, and aligned relative to the
    /// retained allocation base.
    pub fn checked_subrange(
        self,
        relative_offset_bytes: u64,
        extent_bytes: u64,
        alignment: u64,
    ) -> Result<Self, ServiceAllocationErrorV1> {
        let offset_bytes = checked_dispatch_subrange(
            self.binding,
            self.offset_bytes,
            self.extent_bytes,
            relative_offset_bytes,
            extent_bytes,
            alignment,
        )?;
        Ok(Self {
            offset_bytes,
            extent_bytes,
            ..self
        })
    }
}

/// Addressless, owner-checked coherent host-visible range admitted for dispatch.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ServiceHostDispatchRangeV1 {
    pub(crate) binding: AllocationBindingV1,
    pub(crate) data_index: usize,
    pub(crate) offset_bytes: u64,
    pub(crate) extent_bytes: u64,
    pub(crate) sublease_index: Option<usize>,
}

/// Checked fully initialized coherent range retained for completed snapshot copying.
///
/// This inert value carries no address or copy authority. It can be minted only
/// by the allocation owner while the exact host-visible allocation generation
/// retains sealed full initialization. A fixed dispatch must separately bind
/// an inspected writable interior before a recycled queue can authorize a copy.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ServiceHostDispatchSnapshotRangeV1 {
    range: ServiceHostDispatchRangeV1,
}

impl fmt::Debug for ServiceHostDispatchSnapshotRangeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceHostDispatchSnapshotRangeV1")
            .field("range", &self.range)
            .finish_non_exhaustive()
    }
}

impl ServiceHostDispatchSnapshotRangeV1 {
    /// Returns the exact initialized enclosing range without exposing a native address.
    pub const fn enclosing_dispatch_range(self) -> ServiceHostDispatchRangeV1 {
        self.range
    }

    /// Returns the checked byte offset without exposing a native address.
    pub const fn offset_bytes(self) -> u64 {
        self.range.offset_bytes
    }

    /// Returns the checked snapshot extent.
    pub const fn extent_bytes(self) -> u64 {
        self.range.extent_bytes
    }

    pub(crate) const fn dispatch_range(self) -> ServiceHostDispatchRangeV1 {
        self.range
    }

    pub(crate) const fn from_initialized_range(range: ServiceHostDispatchRangeV1) -> Self {
        Self { range }
    }
}

impl fmt::Debug for ServiceHostDispatchRangeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServiceHostDispatchRangeV1")
            .field("data_index", &self.data_index)
            .field("offset_bytes", &self.offset_bytes)
            .field("extent_bytes", &self.extent_bytes)
            .finish_non_exhaustive()
    }
}

impl ServiceHostDispatchRangeV1 {
    /// Returns the checked byte offset without exposing a native address.
    pub const fn offset_bytes(self) -> u64 {
        self.offset_bytes
    }

    /// Returns the checked byte extent.
    pub const fn extent_bytes(self) -> u64 {
        self.extent_bytes
    }

    /// Narrows this checked range while retaining its host allocation and
    /// sublease-member bindings.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceAllocationErrorV1::InvalidRange`] unless the result is
    /// nonempty, wholly contained in this range, and aligned relative to the
    /// retained allocation base.
    pub fn checked_subrange(
        self,
        relative_offset_bytes: u64,
        extent_bytes: u64,
        alignment: u64,
    ) -> Result<Self, ServiceAllocationErrorV1> {
        let offset_bytes = checked_dispatch_subrange(
            self.binding,
            self.offset_bytes,
            self.extent_bytes,
            relative_offset_bytes,
            extent_bytes,
            alignment,
        )?;
        Ok(Self {
            offset_bytes,
            extent_bytes,
            ..self
        })
    }
}

/// One addressless service range accepted by fixed-dispatch composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceDispatchRangeV1 {
    /// Device-local allocation range.
    Device(ServiceDeviceDispatchRangeV1),
    /// Coherent host-visible allocation range.
    HostVisible(ServiceHostDispatchRangeV1),
}

impl ServiceDispatchRangeV1 {
    pub(crate) const fn binding(self) -> AllocationBindingV1 {
        match self {
            Self::Device(range) => range.binding,
            Self::HostVisible(range) => range.binding,
        }
    }

    pub(crate) const fn data_index(self) -> usize {
        match self {
            Self::Device(range) => range.data_index,
            Self::HostVisible(range) => range.data_index,
        }
    }

    pub(crate) const fn offset_bytes(self) -> u64 {
        match self {
            Self::Device(range) => range.offset_bytes,
            Self::HostVisible(range) => range.offset_bytes,
        }
    }

    pub(crate) const fn extent_bytes(self) -> u64 {
        match self {
            Self::Device(range) => range.extent_bytes,
            Self::HostVisible(range) => range.extent_bytes,
        }
    }
}

impl From<ServiceDeviceDispatchRangeV1> for ServiceDispatchRangeV1 {
    fn from(value: ServiceDeviceDispatchRangeV1) -> Self {
        Self::Device(value)
    }
}

impl From<ServiceHostDispatchRangeV1> for ServiceDispatchRangeV1 {
    fn from(value: ServiceHostDispatchRangeV1) -> Self {
        Self::HostVisible(value)
    }
}

pub(crate) struct ServiceQueueAllocationLedgerV1 {
    owner: OwnerBindingV1,
    next_allocation_id: u64,
    allocations: Vec<OwnedAllocationV1>,
    device_bytes: u64,
    host_bytes: u64,
    device_bindings: Vec<AllocationBindingV1>,
    host_bindings: Vec<AllocationBindingV1>,
}

pub(crate) struct ServiceQueueAllocationTransferV1 {
    pub(crate) session: SharedGttMemorySessionV1,
    pub(crate) ledger: ServiceQueueAllocationLedgerV1,
    pub(crate) data: Vec<Gfx942FixedDispatchDataV1>,
}

pub(crate) struct ServiceQueueAllocationRestoreFailureV1 {
    pub(crate) ledger: ServiceQueueAllocationLedgerV1,
    pub(crate) session: SharedGttMemorySessionV1,
    pub(crate) data: Vec<Gfx942FixedDispatchDataV1>,
    pub(crate) error: ServiceAllocationErrorV1,
}

pub(crate) struct ServiceQueueAllocationReplacementV1 {
    allocation_index: usize,
    data_index: usize,
    old_binding: AllocationBindingV1,
    new_binding: AllocationBindingV1,
    new_subleases: Option<Vec<ServiceAllocationSubleaseRangeV1>>,
}

pub(crate) struct ServiceQueuePartitionedAllocationInsertionV1<const N: usize> {
    binding: AllocationBindingV1,
    data_index: usize,
    ranges: [ServiceAllocationSubleaseRangeV1; N],
    retained_ranges: Vec<ServiceAllocationSubleaseRangeV1>,
}

impl<const N: usize> ServiceQueuePartitionedAllocationInsertionV1<N> {
    pub(crate) const fn data_index(&self) -> usize {
        self.data_index
    }
}

pub(crate) struct ServiceQueuePartitionedAllocationRemovalV1 {
    allocation_index: usize,
    data_index: usize,
    binding: AllocationBindingV1,
}

impl ServiceQueuePartitionedAllocationRemovalV1 {
    pub(crate) const fn data_index(&self) -> usize {
        self.data_index
    }
}

pub(crate) struct ServiceQueueHostAllocationReplacementV1 {
    allocation_index: usize,
    host_binding_index: usize,
    data_index: usize,
    old_binding: AllocationBindingV1,
    new_binding: AllocationBindingV1,
}

impl ServiceQueueHostAllocationReplacementV1 {
    pub(crate) const fn data_index(&self) -> usize {
        self.data_index
    }
}

impl ServiceQueueAllocationReplacementV1 {
    pub(crate) const fn data_index(&self) -> usize {
        self.data_index
    }
}

impl ServiceQueueAllocationLedgerV1 {
    pub(crate) fn device_allocation_count(&self) -> usize {
        self.device_bindings.len()
    }

    pub(crate) fn validate_range<R>(&self, range: R) -> Result<(), ServiceAllocationErrorV1>
    where
        R: Into<ServiceDispatchRangeV1>,
    {
        let range = range.into();
        let expected = self
            .dispatch_binding(range.data_index())
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        if range.binding().owner != self.owner {
            return Err(ServiceAllocationErrorV1::OwnerBindingMismatch);
        }
        if expected != range.binding() {
            return Err(ServiceAllocationErrorV1::AllocationGenerationMismatch);
        }
        let allocation = self
            .allocations
            .iter()
            .find(|allocation| allocation.binding == expected)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        match range {
            ServiceDispatchRangeV1::Device(range) => {
                validate_dispatch_range(range, expected, allocation.subleases.as_deref())
            }
            ServiceDispatchRangeV1::HostVisible(range) => {
                validate_host_dispatch_range(range, expected, allocation.subleases.as_deref())
            }
        }
    }

    pub(crate) fn validate_host_dispatch_snapshot(
        &self,
        interior: ServiceHostDispatchRangeV1,
        snapshot: ServiceHostDispatchSnapshotRangeV1,
    ) -> Result<(), ServiceAllocationErrorV1> {
        self.validate_range(interior)?;
        self.validate_range(snapshot.range)?;
        validate_host_dispatch_snapshot(interior, snapshot.range)
    }

    fn dispatch_binding(&self, data_index: usize) -> Option<AllocationBindingV1> {
        self.device_bindings.get(data_index).copied().or_else(|| {
            data_index
                .checked_sub(self.device_bindings.len())
                .and_then(|index| self.host_bindings.get(index).copied())
        })
    }

    pub(crate) fn reissue_partitioned_device_local<R, const N: usize>(
        &self,
        subleases: &ServiceAllocationSubleaseSetV1<R, DeviceLocalAllocationV1, N>,
    ) -> Result<[ServiceDeviceDispatchRangeV1; N], ServiceAllocationErrorV1>
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        let allocation_index =
            validate_sublease_set_binding(self.owner, &self.allocations, subleases)?;
        let binding = self.allocations[allocation_index].binding;
        if binding.role_id != R::ROLE_ID {
            return Err(ServiceAllocationErrorV1::RoleMismatch);
        }
        if binding.kind_id != AllocationKindV1::DeviceLocal as u8 {
            return Err(ServiceAllocationErrorV1::KindMismatch);
        }
        let data_index = self
            .device_bindings
            .iter()
            .position(|candidate| *candidate == binding)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        let ranges = core::array::from_fn(|index| ServiceDeviceDispatchRangeV1 {
            binding,
            data_index,
            offset_bytes: subleases.ranges[index].offset_bytes,
            extent_bytes: subleases.ranges[index].extent_bytes,
            sublease_index: Some(index),
        });
        for range in &ranges {
            self.validate_range(*range)?;
        }
        Ok(ranges)
    }

    pub(crate) fn reissue_host_visible<R>(
        &self,
        range: ServiceHostDispatchRangeV1,
    ) -> Result<ServiceHostDispatchRangeV1, ServiceAllocationErrorV1>
    where
        R: HostAllocationRoleMarkerV1,
    {
        if range.binding.owner != self.owner {
            return Err(ServiceAllocationErrorV1::OwnerBindingMismatch);
        }
        if range.binding.role_id != R::ROLE_ID {
            return Err(ServiceAllocationErrorV1::RoleMismatch);
        }
        if range.binding.kind_id != AllocationKindV1::HostVisible as u8 {
            return Err(ServiceAllocationErrorV1::KindMismatch);
        }
        let host_index = self
            .host_bindings
            .iter()
            .position(|candidate| *candidate == range.binding)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        let data_index = self
            .device_bindings
            .len()
            .checked_add(host_index)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        let reissued = ServiceHostDispatchRangeV1 {
            data_index,
            ..range
        };
        self.validate_range(reissued)?;
        Ok(reissued)
    }

    pub(crate) fn prepare_initialized_partition_insertion<R, const N: usize>(
        &mut self,
        extent_bytes: u64,
        alignment: u64,
        members: [(u64, u64, u64); N],
    ) -> Result<ServiceQueuePartitionedAllocationInsertionV1<N>, ServiceAllocationErrorV1>
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        if self.allocations.len() >= MAX_SERVICE_ALLOCATIONS_V1 {
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
        if self
            .device_bytes
            .checked_add(extent_bytes)
            .is_none_or(|total| total > MAX_SERVICE_DEVICE_BYTES_V1)
        {
            return Err(ServiceAllocationErrorV1::ByteCapacity {
                maximum_bytes: MAX_SERVICE_DEVICE_BYTES_V1,
            });
        }
        self.allocations
            .try_reserve(1)
            .map_err(|_| ServiceAllocationErrorV1::AllocationRegistryReservation)?;
        self.device_bindings
            .try_reserve(1)
            .map_err(|_| ServiceAllocationErrorV1::AllocationRegistryReservation)?;
        let next_allocation_id = self
            .next_allocation_id
            .checked_add(1)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        let binding = AllocationBindingV1 {
            owner: self.owner,
            id: self.next_allocation_id,
            generation: 1,
            role_id: R::ROLE_ID,
            kind_id: AllocationKindV1::DeviceLocal as u8,
            extent_bytes,
            alignment,
        };
        let ranges = validate_sublease_layout(binding, members)?;
        let mut retained_ranges = Vec::new();
        retained_ranges
            .try_reserve_exact(N)
            .map_err(|_| ServiceAllocationErrorV1::AllocationRegistryReservation)?;
        retained_ranges.extend_from_slice(&ranges);
        let data_index = self.device_bindings.len();
        self.next_allocation_id = next_allocation_id;
        Ok(ServiceQueuePartitionedAllocationInsertionV1 {
            binding,
            data_index,
            ranges,
            retained_ranges,
        })
    }

    pub(crate) fn commit_initialized_partition_insertion<R, const N: usize>(
        &mut self,
        insertion: ServiceQueuePartitionedAllocationInsertionV1<N>,
    ) -> (
        ServiceAllocationSubleaseSetV1<R, DeviceLocalAllocationV1, N>,
        [ServiceDeviceDispatchRangeV1; N],
    )
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        let ServiceQueuePartitionedAllocationInsertionV1 {
            binding,
            data_index,
            ranges,
            retained_ranges,
        } = insertion;
        debug_assert_eq!(data_index, self.device_bindings.len());
        self.allocations.push(OwnedAllocationV1 {
            binding,
            token: None,
            subleases: Some(retained_ranges),
        });
        self.device_bindings.push(binding);
        self.device_bytes += binding.extent_bytes;
        let subleases = ServiceAllocationSubleaseSetV1 {
            key: key(binding),
            ranges,
        };
        let dispatch_ranges = core::array::from_fn(|index| ServiceDeviceDispatchRangeV1 {
            binding,
            data_index,
            offset_bytes: ranges[index].offset_bytes,
            extent_bytes: ranges[index].extent_bytes,
            sublease_index: Some(index),
        });
        (subleases, dispatch_ranges)
    }

    pub(crate) fn prepare_partitioned_removal<R, const N: usize>(
        &self,
        subleases: &ServiceAllocationSubleaseSetV1<R, DeviceLocalAllocationV1, N>,
    ) -> Result<ServiceQueuePartitionedAllocationRemovalV1, ServiceAllocationErrorV1>
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        let allocation_index =
            validate_sublease_set_binding(self.owner, &self.allocations, subleases)?;
        let binding = self.allocations[allocation_index].binding;
        let data_index = self
            .device_bindings
            .iter()
            .position(|candidate| *candidate == binding)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        Ok(ServiceQueuePartitionedAllocationRemovalV1 {
            allocation_index,
            data_index,
            binding,
        })
    }

    pub(crate) fn commit_partitioned_removal(
        &mut self,
        removal: ServiceQueuePartitionedAllocationRemovalV1,
    ) {
        let removed = self.allocations.remove(removal.allocation_index);
        debug_assert!(removed.binding == removal.binding);
        debug_assert!(removed.token.is_none());
        let removed_binding = self.device_bindings.remove(removal.data_index);
        debug_assert!(removed_binding == removal.binding);
        self.device_bytes -= removal.binding.extent_bytes;
    }

    pub(crate) fn prepare_host_replacement<R>(
        &mut self,
        range: ServiceHostDispatchRangeV1,
        extent_bytes: u64,
    ) -> Result<ServiceQueueHostAllocationReplacementV1, ServiceAllocationErrorV1>
    where
        R: HostAllocationRoleMarkerV1,
    {
        self.validate_range(range)?;
        if range.offset_bytes != 0 || range.extent_bytes != range.binding.extent_bytes {
            return Err(ServiceAllocationErrorV1::InvalidRange);
        }
        if range.binding.role_id != R::ROLE_ID {
            return Err(ServiceAllocationErrorV1::RoleMismatch);
        }
        if extent_bytes == 0 {
            return Err(ServiceAllocationErrorV1::InvalidExtent);
        }
        let retained_without_old = self
            .host_bytes
            .checked_sub(range.binding.extent_bytes)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        if retained_without_old
            .checked_add(extent_bytes)
            .is_none_or(|total| total > MAX_SERVICE_HOST_BYTES_V1)
        {
            return Err(ServiceAllocationErrorV1::ByteCapacity {
                maximum_bytes: MAX_SERVICE_HOST_BYTES_V1,
            });
        }
        let next_allocation_id = self
            .next_allocation_id
            .checked_add(1)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        let allocation_index = self
            .allocations
            .iter()
            .position(|allocation| allocation.binding == range.binding)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        let host_binding_index = range
            .data_index
            .checked_sub(self.device_bindings.len())
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        if self.host_bindings.get(host_binding_index) != Some(&range.binding) {
            return Err(ServiceAllocationErrorV1::AllocationGenerationMismatch);
        }
        let new_binding = AllocationBindingV1 {
            owner: self.owner,
            id: self.next_allocation_id,
            generation: 1,
            role_id: R::ROLE_ID,
            kind_id: AllocationKindV1::HostVisible as u8,
            extent_bytes,
            alignment: MAX_SERVICE_ALIGNMENT_V1,
        };
        self.next_allocation_id = next_allocation_id;
        Ok(ServiceQueueHostAllocationReplacementV1 {
            allocation_index,
            host_binding_index,
            data_index: range.data_index,
            old_binding: range.binding,
            new_binding,
        })
    }

    pub(crate) fn commit_host_replacement_release(
        &mut self,
        replacement: &ServiceQueueHostAllocationReplacementV1,
    ) {
        let removed = self.allocations.remove(replacement.allocation_index);
        debug_assert!(removed.binding == replacement.old_binding);
        debug_assert!(removed.token.is_none());
        let removed_binding = self.host_bindings.remove(replacement.host_binding_index);
        debug_assert!(removed_binding == replacement.old_binding);
        self.host_bytes -= replacement.old_binding.extent_bytes;
    }

    pub(crate) fn commit_host_replacement(
        &mut self,
        replacement: ServiceQueueHostAllocationReplacementV1,
    ) -> ServiceHostDispatchRangeV1 {
        self.allocations.insert(
            replacement.allocation_index,
            OwnedAllocationV1 {
                binding: replacement.new_binding,
                token: None,
                subleases: None,
            },
        );
        self.host_bindings
            .insert(replacement.host_binding_index, replacement.new_binding);
        self.host_bytes += replacement.new_binding.extent_bytes;
        ServiceHostDispatchRangeV1 {
            binding: replacement.new_binding,
            data_index: replacement.data_index,
            offset_bytes: 0,
            extent_bytes: replacement.new_binding.extent_bytes,
            sublease_index: None,
        }
    }

    pub(crate) fn prepare_initialized_replacement<R>(
        &mut self,
        range: ServiceDeviceDispatchRangeV1,
        extent_bytes: u64,
        alignment: u64,
    ) -> Result<ServiceQueueAllocationReplacementV1, ServiceAllocationErrorV1>
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        self.validate_range(range)?;
        if range.offset_bytes != 0 || range.extent_bytes != range.binding.extent_bytes {
            return Err(ServiceAllocationErrorV1::InvalidRange);
        }
        if range.binding.role_id != R::ROLE_ID {
            return Err(ServiceAllocationErrorV1::RoleMismatch);
        }
        if extent_bytes == 0 {
            return Err(ServiceAllocationErrorV1::InvalidExtent);
        }
        if alignment == 0 || !alignment.is_power_of_two() || alignment > MAX_SERVICE_ALIGNMENT_V1 {
            return Err(ServiceAllocationErrorV1::InvalidAlignment);
        }
        let retained_without_old = self
            .device_bytes
            .checked_sub(range.binding.extent_bytes)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        if retained_without_old
            .checked_add(extent_bytes)
            .is_none_or(|total| total > MAX_SERVICE_DEVICE_BYTES_V1)
        {
            return Err(ServiceAllocationErrorV1::ByteCapacity {
                maximum_bytes: MAX_SERVICE_DEVICE_BYTES_V1,
            });
        }
        let next_allocation_id = self
            .next_allocation_id
            .checked_add(1)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        let allocation_index = self
            .allocations
            .iter()
            .position(|allocation| allocation.binding == range.binding)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        let new_binding = AllocationBindingV1 {
            owner: self.owner,
            id: self.next_allocation_id,
            generation: 1,
            role_id: R::ROLE_ID,
            kind_id: AllocationKindV1::DeviceLocal as u8,
            extent_bytes,
            alignment,
        };
        self.next_allocation_id = next_allocation_id;
        Ok(ServiceQueueAllocationReplacementV1 {
            allocation_index,
            data_index: range.data_index,
            old_binding: range.binding,
            new_binding,
            new_subleases: None,
        })
    }

    pub(crate) fn prepare_initialized_partition_replacement<
        R,
        const OLD_N: usize,
        const NEW_N: usize,
    >(
        &mut self,
        old: &ServiceAllocationSubleaseSetV1<R, DeviceLocalAllocationV1, OLD_N>,
        extent_bytes: u64,
        alignment: u64,
        new_members: [(u64, u64, u64); NEW_N],
    ) -> Result<
        (
            ServiceQueueAllocationReplacementV1,
            [ServiceAllocationSubleaseRangeV1; NEW_N],
        ),
        ServiceAllocationErrorV1,
    >
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        let allocation_index = validate_sublease_set_binding(self.owner, &self.allocations, old)?;
        let old_binding = self.allocations[allocation_index].binding;
        let data_index = self
            .device_bindings
            .iter()
            .position(|binding| *binding == old_binding)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        if extent_bytes == 0 {
            return Err(ServiceAllocationErrorV1::InvalidExtent);
        }
        if alignment == 0 || !alignment.is_power_of_two() || alignment > MAX_SERVICE_ALIGNMENT_V1 {
            return Err(ServiceAllocationErrorV1::InvalidAlignment);
        }
        let retained_without_old = self
            .device_bytes
            .checked_sub(old_binding.extent_bytes)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        if retained_without_old
            .checked_add(extent_bytes)
            .is_none_or(|total| total > MAX_SERVICE_DEVICE_BYTES_V1)
        {
            return Err(ServiceAllocationErrorV1::ByteCapacity {
                maximum_bytes: MAX_SERVICE_DEVICE_BYTES_V1,
            });
        }
        let next_allocation_id = self
            .next_allocation_id
            .checked_add(1)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        let new_binding = AllocationBindingV1 {
            owner: self.owner,
            id: self.next_allocation_id,
            generation: 1,
            role_id: R::ROLE_ID,
            kind_id: AllocationKindV1::DeviceLocal as u8,
            extent_bytes,
            alignment,
        };
        let ranges = validate_sublease_layout(new_binding, new_members)?;
        let mut new_subleases = Vec::new();
        new_subleases
            .try_reserve_exact(NEW_N)
            .map_err(|_| ServiceAllocationErrorV1::AllocationRegistryReservation)?;
        new_subleases.extend_from_slice(&ranges);
        self.next_allocation_id = next_allocation_id;
        Ok((
            ServiceQueueAllocationReplacementV1 {
                allocation_index,
                data_index,
                old_binding,
                new_binding,
                new_subleases: Some(new_subleases),
            },
            ranges,
        ))
    }

    pub(crate) fn commit_replacement_release(
        &mut self,
        replacement: &ServiceQueueAllocationReplacementV1,
    ) {
        let removed = self.allocations.remove(replacement.allocation_index);
        debug_assert!(removed.binding == replacement.old_binding);
        debug_assert!(removed.token.is_none());
        let removed_binding = self.device_bindings.remove(replacement.data_index);
        debug_assert!(removed_binding == replacement.old_binding);
        self.device_bytes -= replacement.old_binding.extent_bytes;
    }

    pub(crate) fn commit_initialized_replacement(
        &mut self,
        replacement: ServiceQueueAllocationReplacementV1,
    ) -> ServiceDeviceDispatchRangeV1 {
        debug_assert!(replacement.new_subleases.is_none());
        self.allocations.insert(
            replacement.allocation_index,
            OwnedAllocationV1 {
                binding: replacement.new_binding,
                token: None,
                subleases: None,
            },
        );
        self.device_bindings
            .insert(replacement.data_index, replacement.new_binding);
        self.device_bytes += replacement.new_binding.extent_bytes;
        ServiceDeviceDispatchRangeV1 {
            binding: replacement.new_binding,
            data_index: replacement.data_index,
            offset_bytes: 0,
            extent_bytes: replacement.new_binding.extent_bytes,
            sublease_index: None,
        }
    }

    pub(crate) fn commit_initialized_partitioned_replacement<R, const N: usize>(
        &mut self,
        replacement: ServiceQueueAllocationReplacementV1,
        ranges: [ServiceAllocationSubleaseRangeV1; N],
    ) -> (
        ServiceAllocationSubleaseSetV1<R, DeviceLocalAllocationV1, N>,
        [ServiceDeviceDispatchRangeV1; N],
    )
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        let new_binding = replacement.new_binding;
        let data_index = replacement.data_index;
        let new_subleases = replacement
            .new_subleases
            .expect("partitioned replacement retains its preallocated registry");
        debug_assert!(new_subleases.as_slice() == ranges.as_slice());
        self.allocations.insert(
            replacement.allocation_index,
            OwnedAllocationV1 {
                binding: new_binding,
                token: None,
                subleases: Some(new_subleases),
            },
        );
        self.device_bindings.insert(data_index, new_binding);
        self.device_bytes += new_binding.extent_bytes;
        let subleases = ServiceAllocationSubleaseSetV1 {
            key: key(new_binding),
            ranges,
        };
        let dispatch_ranges = core::array::from_fn(|index| ServiceDeviceDispatchRangeV1 {
            binding: new_binding,
            data_index,
            offset_bytes: ranges[index].offset_bytes,
            extent_bytes: ranges[index].extent_bytes,
            sublease_index: Some(index),
        });
        (subleases, dispatch_ranges)
    }

    pub(crate) fn restore(
        mut self,
        session: SharedGttMemorySessionV1,
        data: Vec<Gfx942FixedDispatchDataV1>,
    ) -> Result<ServiceAllocationSessionV1, ServiceQueueAllocationRestoreFailureV1> {
        let dispatch_bindings = self
            .device_bindings
            .iter()
            .chain(&self.host_bindings)
            .copied()
            .collect::<Vec<_>>();
        let valid = session.phase() == SharedMemorySessionPhaseV1::Active
            && data.len() == dispatch_bindings.len()
            && dispatch_bindings
                .iter()
                .zip(&data)
                .all(|(binding, dispatch)| {
                    self.allocations
                        .iter()
                        .find(|allocation| allocation.binding == *binding)
                        .is_some_and(|allocation| {
                            let layout = dispatch.layout();
                            allocation.token.is_none()
                                && layout.requested_bytes() == allocation.binding.extent_bytes
                                && layout.alignment() == allocation.binding.alignment
                                && match layout.kind() {
                                    Gfx942FixedDispatchDataKindV1::DeviceLocal => {
                                        allocation.binding.kind_id
                                            == AllocationKindV1::DeviceLocal as u8
                                    }
                                    Gfx942FixedDispatchDataKindV1::HostVisibleCoherent => {
                                        allocation.binding.kind_id
                                            == AllocationKindV1::HostVisible as u8
                                    }
                                }
                        })
                });
        if !valid {
            return Err(ServiceQueueAllocationRestoreFailureV1 {
                ledger: self,
                session,
                data,
                error: ServiceAllocationErrorV1::AllocationGenerationMismatch,
            });
        }

        for (binding, dispatch) in dispatch_bindings.into_iter().zip(data) {
            let allocation = self
                .allocations
                .iter_mut()
                .find(|allocation| allocation.binding == binding)
                .expect("validated dispatch binding");
            allocation.token = Some(AllocationTokenV1::FixedDispatch(dispatch));
        }
        Ok(ServiceAllocationSessionV1 {
            owner: AllocationOwnerV1 {
                session,
                owner: self.owner,
                phase: ServiceAllocationPhaseV1::Active,
                next_allocation_id: self.next_allocation_id,
                allocations: self.allocations,
                device_bytes: self.device_bytes,
                host_bytes: self.host_bytes,
            },
            quiescence: PhantomData,
        })
    }
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
    /// A zero-member logical partition was requested.
    InvalidSubleaseCount,
    /// The allocation already has a retained logical partition.
    AllocationAlreadyPartitioned,
    /// A range did not identify an exact member of the retained partition.
    SubleaseBindingMismatch,
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
            subleases: None,
        });
        self.owner.device_bytes += requested_bytes;
        Ok(key(binding))
    }

    /// Allocates a device-local extent whose complete owned bytes are written,
    /// read back, CPU-unmapped, and GPU-mapped by the retained KFD session.
    pub fn allocate_initialized_device_local<R>(
        &mut self,
        bytes: Box<[u8]>,
        alignment: u64,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<ServiceAllocationKeyV1<R, DeviceLocalAllocationV1>, ServiceAllocationErrorV1>
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        self.require_active()?;
        let requested_bytes =
            u64::try_from(bytes.len()).map_err(|_| ServiceAllocationErrorV1::InvalidExtent)?;
        self.preflight_allocation(requested_bytes, alignment, AllocationKindV1::DeviceLocal)?;
        self.owner
            .allocations
            .try_reserve(1)
            .map_err(|_| ServiceAllocationErrorV1::AllocationRegistryReservation)?;
        let binding =
            self.reserve_binding::<R, DeviceLocalAllocationV1>(requested_bytes, alignment)?;
        let initialized = match self
            .owner
            .session
            .initialize_gfx942_device_memory(bytes, alignment, content)
        {
            Ok(initialized) => initialized,
            Err(error) => {
                self.sync_phase_after_nonconsuming_failure();
                return Err(error.into());
            }
        };
        self.owner.allocations.push(OwnedAllocationV1 {
            binding,
            token: Some(AllocationTokenV1::FixedDispatch(
                Gfx942FixedDispatchDataV1::initialized(initialized),
            )),
            subleases: None,
        });
        self.owner.device_bytes += requested_bytes;
        Ok(key(binding))
    }

    /// Allocates a device-local extent whose complete logical bytes are filled
    /// from one private bounded-memory repeated-byte recipe, CPU-unmapped, and
    /// GPU-mapped by the retained KFD session without a redundant HBM readback.
    ///
    /// ```compile_fail
    /// use fe2o3_kfd::{Gfx942DeviceContentRoleV1, Gfx942RepeatedByteContentV1};
    /// use fe2o3_service_host::{HostUploadRoleV1, ServiceAllocationSessionV1};
    ///
    /// fn host_role_cannot_be_repeated_device_local(owner: &mut ServiceAllocationSessionV1) {
    ///     let role = Gfx942DeviceContentRoleV1::new([1; 32], 0).unwrap();
    ///     let initialization = Gfx942RepeatedByteContentV1::new(role, 4096, 0).unwrap();
    ///     let _ = owner
    ///         .allocate_initialized_device_local_repeated_byte::<HostUploadRoleV1>(
    ///             initialization,
    ///             4096,
    ///         );
    /// }
    /// ```
    pub fn allocate_initialized_device_local_repeated_byte<R>(
        &mut self,
        initialization: Gfx942RepeatedByteContentV1,
        alignment: u64,
    ) -> Result<ServiceAllocationKeyV1<R, DeviceLocalAllocationV1>, ServiceAllocationErrorV1>
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        self.require_active()?;
        let requested_bytes = initialization.content().byte_len();
        self.preflight_allocation(requested_bytes, alignment, AllocationKindV1::DeviceLocal)?;
        self.owner
            .allocations
            .try_reserve(1)
            .map_err(|_| ServiceAllocationErrorV1::AllocationRegistryReservation)?;
        let binding =
            self.reserve_binding::<R, DeviceLocalAllocationV1>(requested_bytes, alignment)?;
        let initialized = match self
            .owner
            .session
            .initialize_gfx942_device_memory_repeated_byte(initialization, alignment)
        {
            Ok(initialized) => initialized,
            Err(error) => {
                self.sync_phase_after_nonconsuming_failure();
                return Err(error.into());
            }
        };
        self.owner.allocations.push(OwnedAllocationV1 {
            binding,
            token: Some(AllocationTokenV1::FixedDispatch(
                Gfx942FixedDispatchDataV1::initialized(initialized),
            )),
            subleases: None,
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
            subleases: None,
        });
        self.owner.host_bytes += requested_u64;
        Ok(key(binding))
    }

    /// Allocates coherent GTT, copies the complete owned source, and maps it.
    ///
    /// Unlike the scoped write API, successful return carries a sealed
    /// full-extent initialization authority and may therefore satisfy an
    /// inspected read or read-write dispatch argument.
    pub fn allocate_initialized_host_visible<R>(
        &mut self,
        bytes: Box<[u8]>,
    ) -> Result<ServiceAllocationKeyV1<R, HostVisibleAllocationV1>, ServiceAllocationErrorV1>
    where
        R: HostAllocationRoleMarkerV1,
    {
        self.require_active()?;
        let requested_u64 =
            u64::try_from(bytes.len()).map_err(|_| ServiceAllocationErrorV1::InvalidExtent)?;
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
        let token = match self.owner.session.initialize_host_visible_coherent(bytes) {
            Ok(token) => token,
            Err(error) => {
                self.sync_phase_after_nonconsuming_failure();
                return Err(error.into());
            }
        };
        self.owner.allocations.push(OwnedAllocationV1 {
            binding,
            token: Some(AllocationTokenV1::HostMappedInitialized(token)),
            subleases: None,
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
        if self.owner.allocations[index].subleases.is_some() {
            return Err(ServiceAllocationErrorV1::AllocationAlreadyPartitioned);
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
            sublease_index: None,
        })
    }

    /// Atomically reserves one move-only, pairwise-disjoint logical partition.
    ///
    /// Every member is checked against the exact typed allocation generation.
    /// Success permanently records the layout beside the whole native allocation
    /// owner. Failure records no prefix, and a second reservation for the same
    /// allocation is rejected even when the caller retained a copied key.
    pub fn reserve_disjoint_subleases<R, K, const N: usize>(
        &mut self,
        key: ServiceAllocationKeyV1<R, K>,
        members: [(u64, u64, u64); N],
    ) -> Result<ServiceAllocationSubleaseSetV1<R, K, N>, ServiceAllocationErrorV1>
    where
        R: ServiceAllocationRoleMarkerV1,
        K: ServiceAllocationKindMarkerV1,
    {
        self.require_active()?;
        let allocation_index = self.validate_key(key)?;
        let allocation = &self.owner.allocations[allocation_index];
        if !is_mapped(allocation.token.as_ref()) {
            return Err(ServiceAllocationErrorV1::AllocationState);
        }
        let ranges =
            reserve_sublease_layout(&mut self.owner.allocations[allocation_index], members)?;
        Ok(ServiceAllocationSubleaseSetV1 { key, ranges })
    }

    /// Revalidates a complete logical partition and returns its inert ranges.
    ///
    /// The returned values may be copied because they carry no allocation or
    /// dispatch authority. Queue admission checks their private member indices
    /// against the retained partition after whole-owner transfer.
    pub fn sublease_ranges<R, K, const N: usize>(
        &self,
        subleases: &ServiceAllocationSubleaseSetV1<R, K, N>,
    ) -> Result<[ServiceAllocationRangeV1<R, K>; N], ServiceAllocationErrorV1>
    where
        R: ServiceAllocationRoleMarkerV1,
        K: ServiceAllocationKindMarkerV1,
    {
        self.require_active()?;
        let allocation_index =
            validate_sublease_set_binding(self.owner.owner, &self.owner.allocations, subleases)?;
        let allocation = &self.owner.allocations[allocation_index];
        if !is_mapped(allocation.token.as_ref()) {
            return Err(ServiceAllocationErrorV1::AllocationState);
        }
        Ok(core::array::from_fn(|index| {
            let range = subleases.ranges[index];
            ServiceAllocationRangeV1 {
                key: subleases.key,
                offset_bytes: range.offset_bytes,
                extent_bytes: range.extent_bytes,
                alignment: range.alignment,
                sublease_index: Some(index),
            }
        }))
    }

    /// Revalidates and narrows one member of a retained logical partition.
    ///
    /// The returned range remains bound to the parent member index. Queue
    /// admission accepts it only while the exact allocation generation and
    /// partition layout remain retained, and only when the narrowed interval
    /// stays nonempty, aligned, and wholly inside that member.
    pub fn sublease_range<R, K, const N: usize>(
        &self,
        subleases: &ServiceAllocationSubleaseSetV1<R, K, N>,
        member_index: usize,
        relative_offset_bytes: u64,
        extent_bytes: u64,
        alignment: u64,
    ) -> Result<ServiceAllocationRangeV1<R, K>, ServiceAllocationErrorV1>
    where
        R: ServiceAllocationRoleMarkerV1,
        K: ServiceAllocationKindMarkerV1,
    {
        self.require_active()?;
        let allocation_index =
            validate_sublease_set_binding(self.owner.owner, &self.owner.allocations, subleases)?;
        let allocation = &self.owner.allocations[allocation_index];
        if !is_mapped(allocation.token.as_ref()) {
            return Err(ServiceAllocationErrorV1::AllocationState);
        }
        let member = subleases
            .ranges
            .get(member_index)
            .ok_or(ServiceAllocationErrorV1::SubleaseBindingMismatch)?;
        let offset_bytes = checked_sublease_subrange(
            subleases.key.binding,
            *member,
            relative_offset_bytes,
            extent_bytes,
            alignment,
        )?;
        Ok(ServiceAllocationRangeV1 {
            key: subleases.key,
            offset_bytes,
            extent_bytes,
            alignment,
            sublease_index: Some(member_index),
        })
    }

    /// Erases a checked device role range into an addressless service-batch binding.
    ///
    /// The owner validates the exact allocation generation and mapped state.
    /// Queue composition later revalidates the retained service binding at the
    /// stable device-record ordinal before producing a KFD buffer binding.
    pub fn device_dispatch_range<R>(
        &self,
        range: ServiceAllocationRangeV1<R, DeviceLocalAllocationV1>,
    ) -> Result<ServiceDeviceDispatchRangeV1, ServiceAllocationErrorV1>
    where
        R: DeviceAllocationRoleMarkerV1,
    {
        self.require_active()?;
        let allocation_index = self.validate_key(range.key)?;
        let token = self.owner.allocations[allocation_index]
            .token
            .as_ref()
            .ok_or(ServiceAllocationErrorV1::Quarantined)?;
        if !matches!(
            token,
            AllocationTokenV1::DeviceMapped(_) | AllocationTokenV1::FixedDispatch(_)
        ) {
            return Err(ServiceAllocationErrorV1::AllocationState);
        }
        let data_index = self.owner.allocations[..allocation_index]
            .iter()
            .filter(|allocation| allocation.binding.kind_id == AllocationKindV1::DeviceLocal as u8)
            .count();
        Ok(ServiceDeviceDispatchRangeV1 {
            binding: range.key.binding,
            data_index,
            offset_bytes: range.offset_bytes,
            extent_bytes: range.extent_bytes,
            sublease_index: range.sublease_index,
        })
    }

    /// Erases a checked coherent host-visible range into an addressless binding.
    pub fn host_dispatch_range<R>(
        &self,
        range: ServiceAllocationRangeV1<R, HostVisibleAllocationV1>,
    ) -> Result<ServiceHostDispatchRangeV1, ServiceAllocationErrorV1>
    where
        R: HostAllocationRoleMarkerV1,
    {
        self.require_active()?;
        let allocation_index = self.validate_key(range.key)?;
        let token = self.owner.allocations[allocation_index]
            .token
            .as_ref()
            .ok_or(ServiceAllocationErrorV1::Quarantined)?;
        if !matches!(
            token,
            AllocationTokenV1::HostMapped(_)
                | AllocationTokenV1::HostMappedInitialized(_)
                | AllocationTokenV1::FixedDispatch(_)
        ) {
            return Err(ServiceAllocationErrorV1::AllocationState);
        }
        let device_count = self
            .owner
            .allocations
            .iter()
            .filter(|allocation| allocation.binding.kind_id == AllocationKindV1::DeviceLocal as u8)
            .count();
        let host_index = self.owner.allocations[..allocation_index]
            .iter()
            .filter(|allocation| {
                allocation.binding.kind_id == AllocationKindV1::HostVisible as u8
                    && matches!(
                        allocation.token.as_ref(),
                        Some(
                            AllocationTokenV1::HostMapped(_)
                                | AllocationTokenV1::HostMappedInitialized(_)
                                | AllocationTokenV1::FixedDispatch(_)
                        )
                    )
            })
            .count();
        Ok(ServiceHostDispatchRangeV1 {
            binding: range.key.binding,
            data_index: device_count + host_index,
            offset_bytes: range.offset_bytes,
            extent_bytes: range.extent_bytes,
            sublease_index: range.sublease_index,
        })
    }

    /// Marks one checked coherent range as eligible for an enclosing completed snapshot.
    ///
    /// The allocation must retain sealed full initialization in the exact
    /// owner and allocation generation. This method grants no copy authority;
    /// queue admission must still associate the range with one inspected
    /// writable interior, and a matching generation must complete and recycle.
    ///
    /// # Errors
    ///
    /// Rejects stale, non-host, uninitialized, unmapped, or otherwise invalid
    /// range custody.
    pub fn host_dispatch_snapshot_range(
        &self,
        range: ServiceHostDispatchRangeV1,
    ) -> Result<ServiceHostDispatchSnapshotRangeV1, ServiceAllocationErrorV1> {
        self.validate_host_dispatch_range(range)?;
        let allocation = self
            .owner
            .allocations
            .iter()
            .find(|allocation| allocation.binding == range.binding)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        let fully_initialized = match allocation.token.as_ref() {
            Some(AllocationTokenV1::HostMappedInitialized(_)) => true,
            Some(AllocationTokenV1::FixedDispatch(dispatch)) => {
                dispatch.layout().kind() == Gfx942FixedDispatchDataKindV1::HostVisibleCoherent
                    && dispatch.is_fully_initialized()
            }
            Some(
                AllocationTokenV1::DeviceUnmapped(_)
                | AllocationTokenV1::DeviceMapped(_)
                | AllocationTokenV1::HostCpuWritable(_)
                | AllocationTokenV1::HostMapped(_),
            )
            | None => false,
        };
        if !fully_initialized {
            return Err(ServiceAllocationErrorV1::AllocationState);
        }
        Ok(ServiceHostDispatchSnapshotRangeV1 { range })
    }

    pub(crate) fn validate_device_dispatch_range(
        &self,
        range: ServiceDeviceDispatchRangeV1,
    ) -> Result<(), ServiceAllocationErrorV1> {
        self.require_active()?;
        if range.binding.owner != self.owner.owner {
            return Err(ServiceAllocationErrorV1::OwnerBindingMismatch);
        }
        let allocation = self
            .owner
            .allocations
            .iter()
            .filter(|allocation| allocation.binding.kind_id == AllocationKindV1::DeviceLocal as u8)
            .nth(range.data_index)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        if allocation.binding != range.binding {
            return Err(ServiceAllocationErrorV1::AllocationGenerationMismatch);
        }
        if !matches!(
            allocation.token.as_ref(),
            Some(AllocationTokenV1::DeviceMapped(_) | AllocationTokenV1::FixedDispatch(_))
        ) {
            return Err(ServiceAllocationErrorV1::AllocationState);
        }
        validate_dispatch_range(range, allocation.binding, allocation.subleases.as_deref())
    }

    pub(crate) fn validate_host_dispatch_range(
        &self,
        range: ServiceHostDispatchRangeV1,
    ) -> Result<(), ServiceAllocationErrorV1> {
        self.require_active()?;
        if range.binding.owner != self.owner.owner {
            return Err(ServiceAllocationErrorV1::OwnerBindingMismatch);
        }
        let device_count = self
            .owner
            .allocations
            .iter()
            .filter(|allocation| allocation.binding.kind_id == AllocationKindV1::DeviceLocal as u8)
            .count();
        let host_index = range
            .data_index
            .checked_sub(device_count)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        let allocation = self
            .owner
            .allocations
            .iter()
            .filter(|allocation| {
                allocation.binding.kind_id == AllocationKindV1::HostVisible as u8
                    && matches!(
                        allocation.token.as_ref(),
                        Some(
                            AllocationTokenV1::HostMapped(_)
                                | AllocationTokenV1::HostMappedInitialized(_)
                                | AllocationTokenV1::FixedDispatch(_)
                        )
                    )
            })
            .nth(host_index)
            .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
        if allocation.binding != range.binding {
            return Err(ServiceAllocationErrorV1::AllocationGenerationMismatch);
        }
        validate_host_dispatch_range(range, allocation.binding, allocation.subleases.as_deref())
    }

    pub(crate) fn validate_host_dispatch_snapshot(
        &self,
        interior: ServiceHostDispatchRangeV1,
        snapshot: ServiceHostDispatchSnapshotRangeV1,
    ) -> Result<(), ServiceAllocationErrorV1> {
        self.validate_host_dispatch_range(interior)?;
        self.host_dispatch_snapshot_range(snapshot.range)?;
        validate_host_dispatch_snapshot(interior, snapshot.range)
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
        self,
    ) -> Result<ServiceAllocationReleaseObservationV1, ServiceAllocationReleaseFailureV1> {
        self.release_all()
    }

    pub(crate) fn into_queue_transfer(
        mut self,
    ) -> Result<
        ServiceQueueAllocationTransferV1,
        (ServiceAllocationSessionV1, ServiceAllocationErrorV1),
    > {
        if let Err(error) = self.require_active() {
            return Err((self, error));
        }
        let device_count = self
            .owner
            .allocations
            .iter()
            .filter(|allocation| allocation.binding.kind_id == AllocationKindV1::DeviceLocal as u8)
            .count();
        let host_count = self
            .owner
            .allocations
            .iter()
            .filter(|allocation| {
                allocation.binding.kind_id == AllocationKindV1::HostVisible as u8
                    && matches!(
                        allocation.token.as_ref(),
                        Some(
                            AllocationTokenV1::HostMapped(_)
                                | AllocationTokenV1::HostMappedInitialized(_)
                                | AllocationTokenV1::FixedDispatch(_)
                        )
                    )
            })
            .count();
        let Some(dispatch_count) = device_count.checked_add(host_count) else {
            return Err((
                self,
                ServiceAllocationErrorV1::AllocationRegistryReservation,
            ));
        };
        if dispatch_count == 0 {
            return Err((self, ServiceAllocationErrorV1::AllocationState));
        }
        if self.owner.allocations.iter().any(|allocation| {
            allocation.binding.kind_id == AllocationKindV1::DeviceLocal as u8
                && !matches!(
                    allocation.token.as_ref(),
                    Some(AllocationTokenV1::DeviceMapped(_) | AllocationTokenV1::FixedDispatch(_))
                )
        }) {
            return Err((self, ServiceAllocationErrorV1::AllocationState));
        }
        let mut data = Vec::new();
        let mut device_bindings = Vec::new();
        let mut host_bindings = Vec::new();
        if data.try_reserve(dispatch_count).is_err()
            || device_bindings.try_reserve(device_count).is_err()
            || host_bindings.try_reserve(host_count).is_err()
        {
            return Err((
                self,
                ServiceAllocationErrorV1::AllocationRegistryReservation,
            ));
        }
        for allocation in &mut self.owner.allocations {
            if allocation.binding.kind_id != AllocationKindV1::DeviceLocal as u8 {
                continue;
            }
            let token = allocation
                .token
                .take()
                .expect("device transfer preflight checked token");
            let dispatch = match token {
                AllocationTokenV1::DeviceMapped(lease) => {
                    Gfx942FixedDispatchDataV1::uninitialized(lease)
                }
                AllocationTokenV1::FixedDispatch(dispatch) => dispatch,
                _ => unreachable!("device transfer preflight checked state"),
            };
            data.push(dispatch);
            device_bindings.push(allocation.binding);
        }
        for allocation in &mut self.owner.allocations {
            if allocation.binding.kind_id != AllocationKindV1::HostVisible as u8
                || !matches!(
                    allocation.token.as_ref(),
                    Some(
                        AllocationTokenV1::HostMapped(_)
                            | AllocationTokenV1::HostMappedInitialized(_)
                            | AllocationTokenV1::FixedDispatch(_)
                    )
                )
            {
                continue;
            }
            let token = allocation
                .token
                .take()
                .expect("host transfer preflight checked token");
            let dispatch = match token {
                AllocationTokenV1::HostMapped(token) => {
                    Gfx942FixedDispatchDataV1::host_visible_uninitialized(token)
                }
                AllocationTokenV1::HostMappedInitialized(token) => {
                    Gfx942FixedDispatchDataV1::host_visible_initialized(token)
                }
                AllocationTokenV1::FixedDispatch(dispatch) => dispatch,
                _ => unreachable!("host transfer preflight checked state"),
            };
            data.push(dispatch);
            host_bindings.push(allocation.binding);
        }
        let owner = self.owner;
        Ok(ServiceQueueAllocationTransferV1 {
            session: owner.session,
            ledger: ServiceQueueAllocationLedgerV1 {
                owner: owner.owner,
                next_allocation_id: owner.next_allocation_id,
                allocations: owner.allocations,
                device_bytes: owner.device_bytes,
                host_bytes: owner.host_bytes,
                device_bindings,
                host_bindings,
            },
            data,
        })
    }

    pub(crate) fn release_quiescent(
        self,
    ) -> Result<ServiceAllocationReleaseObservationV1, ServiceAllocationReleaseFailureV1> {
        self.release_all()
    }

    fn release_all(
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

fn validate_dispatch_range(
    range: ServiceDeviceDispatchRangeV1,
    binding: AllocationBindingV1,
    subleases: Option<&[ServiceAllocationSubleaseRangeV1]>,
) -> Result<(), ServiceAllocationErrorV1> {
    if range.extent_bytes == 0
        || range
            .offset_bytes
            .checked_add(range.extent_bytes)
            .is_none_or(|end| end > binding.extent_bytes)
    {
        return Err(ServiceAllocationErrorV1::InvalidRange);
    }
    validate_sublease_member(
        range.offset_bytes,
        range.extent_bytes,
        range.sublease_index,
        subleases,
    )
}

fn validate_host_dispatch_range(
    range: ServiceHostDispatchRangeV1,
    binding: AllocationBindingV1,
    subleases: Option<&[ServiceAllocationSubleaseRangeV1]>,
) -> Result<(), ServiceAllocationErrorV1> {
    if range.extent_bytes == 0
        || range
            .offset_bytes
            .checked_add(range.extent_bytes)
            .is_none_or(|end| end > binding.extent_bytes)
    {
        return Err(ServiceAllocationErrorV1::InvalidRange);
    }
    validate_sublease_member(
        range.offset_bytes,
        range.extent_bytes,
        range.sublease_index,
        subleases,
    )
}

pub(crate) fn validate_host_dispatch_snapshot(
    interior: ServiceHostDispatchRangeV1,
    snapshot: ServiceHostDispatchRangeV1,
) -> Result<(), ServiceAllocationErrorV1> {
    let interior_end = interior
        .offset_bytes
        .checked_add(interior.extent_bytes)
        .ok_or(ServiceAllocationErrorV1::InvalidRange)?;
    let snapshot_end = snapshot
        .offset_bytes
        .checked_add(snapshot.extent_bytes)
        .ok_or(ServiceAllocationErrorV1::InvalidRange)?;
    if interior.binding != snapshot.binding
        || interior.data_index != snapshot.data_index
        || interior.sublease_index != snapshot.sublease_index
        || snapshot.offset_bytes >= interior.offset_bytes
        || interior_end >= snapshot_end
    {
        return Err(ServiceAllocationErrorV1::InvalidRange);
    }
    Ok(())
}

fn reserve_sublease_layout<const N: usize>(
    allocation: &mut OwnedAllocationV1,
    members: [(u64, u64, u64); N],
) -> Result<[ServiceAllocationSubleaseRangeV1; N], ServiceAllocationErrorV1> {
    if allocation.subleases.is_some() {
        return Err(ServiceAllocationErrorV1::AllocationAlreadyPartitioned);
    }
    let ranges = validate_sublease_layout(allocation.binding, members)?;
    let mut retained = Vec::new();
    retained
        .try_reserve_exact(N)
        .map_err(|_| ServiceAllocationErrorV1::AllocationRegistryReservation)?;
    retained.extend_from_slice(&ranges);
    allocation.subleases = Some(retained);
    Ok(ranges)
}

fn validate_sublease_set_binding<R, K, const N: usize>(
    owner: OwnerBindingV1,
    allocations: &[OwnedAllocationV1],
    subleases: &ServiceAllocationSubleaseSetV1<R, K, N>,
) -> Result<usize, ServiceAllocationErrorV1>
where
    R: ServiceAllocationRoleMarkerV1,
    K: ServiceAllocationKindMarkerV1,
{
    let binding = subleases.key.binding;
    if binding.owner != owner {
        return Err(ServiceAllocationErrorV1::OwnerBindingMismatch);
    }
    if binding.role_id != R::ROLE_ID {
        return Err(ServiceAllocationErrorV1::RoleMismatch);
    }
    if binding.kind_id != K::KIND_ID {
        return Err(ServiceAllocationErrorV1::KindMismatch);
    }
    let allocation_index = allocations
        .iter()
        .position(|allocation| allocation.binding.id == binding.id)
        .ok_or(ServiceAllocationErrorV1::AllocationGenerationMismatch)?;
    let allocation = &allocations[allocation_index];
    if allocation.binding != binding {
        return Err(ServiceAllocationErrorV1::AllocationGenerationMismatch);
    }
    if allocation.subleases.as_deref() != Some(subleases.ranges.as_slice()) {
        return Err(ServiceAllocationErrorV1::SubleaseBindingMismatch);
    }
    Ok(allocation_index)
}

fn validate_sublease_layout<const N: usize>(
    binding: AllocationBindingV1,
    members: [(u64, u64, u64); N],
) -> Result<[ServiceAllocationSubleaseRangeV1; N], ServiceAllocationErrorV1> {
    if N == 0 {
        return Err(ServiceAllocationErrorV1::InvalidSubleaseCount);
    }
    let ranges =
        members.map(
            |(offset_bytes, extent_bytes, alignment)| ServiceAllocationSubleaseRangeV1 {
                offset_bytes,
                extent_bytes,
                alignment,
            },
        );
    for range in &ranges {
        if range.extent_bytes == 0
            || range.alignment == 0
            || !range.alignment.is_power_of_two()
            || range.alignment > binding.alignment
            || !range.offset_bytes.is_multiple_of(range.alignment)
            || range
                .offset_bytes
                .checked_add(range.extent_bytes)
                .is_none_or(|end| end > binding.extent_bytes)
        {
            return Err(ServiceAllocationErrorV1::InvalidRange);
        }
    }
    for left in 0..N {
        let left_end = ranges[left].offset_bytes + ranges[left].extent_bytes;
        for right in (left + 1)..N {
            let right_end = ranges[right].offset_bytes + ranges[right].extent_bytes;
            if ranges[left].offset_bytes < right_end && ranges[right].offset_bytes < left_end {
                return Err(ServiceAllocationErrorV1::AliasingRange);
            }
        }
    }
    Ok(ranges)
}

fn validate_sublease_member(
    offset_bytes: u64,
    extent_bytes: u64,
    sublease_index: Option<usize>,
    subleases: Option<&[ServiceAllocationSubleaseRangeV1]>,
) -> Result<(), ServiceAllocationErrorV1> {
    match (sublease_index, subleases) {
        (None, None) => Ok(()),
        (Some(index), Some(subleases)) => {
            let expected = subleases
                .get(index)
                .ok_or(ServiceAllocationErrorV1::SubleaseBindingMismatch)?;
            let expected_end = expected
                .offset_bytes
                .checked_add(expected.extent_bytes)
                .ok_or(ServiceAllocationErrorV1::SubleaseBindingMismatch)?;
            if offset_bytes >= expected.offset_bytes
                && offset_bytes
                    .checked_add(extent_bytes)
                    .is_some_and(|end| end <= expected_end)
            {
                Ok(())
            } else {
                Err(ServiceAllocationErrorV1::SubleaseBindingMismatch)
            }
        }
        _ => Err(ServiceAllocationErrorV1::SubleaseBindingMismatch),
    }
}

fn checked_sublease_subrange(
    binding: AllocationBindingV1,
    member: ServiceAllocationSubleaseRangeV1,
    relative_offset_bytes: u64,
    extent_bytes: u64,
    alignment: u64,
) -> Result<u64, ServiceAllocationErrorV1> {
    let offset_bytes = member
        .offset_bytes
        .checked_add(relative_offset_bytes)
        .ok_or(ServiceAllocationErrorV1::InvalidRange)?;
    let member_end = member
        .offset_bytes
        .checked_add(member.extent_bytes)
        .ok_or(ServiceAllocationErrorV1::InvalidRange)?;
    if extent_bytes == 0
        || alignment == 0
        || !alignment.is_power_of_two()
        || alignment > binding.alignment
        || !offset_bytes.is_multiple_of(alignment)
        || offset_bytes
            .checked_add(extent_bytes)
            .is_none_or(|end| end > member_end)
    {
        return Err(ServiceAllocationErrorV1::InvalidRange);
    }
    Ok(offset_bytes)
}

fn checked_dispatch_subrange(
    binding: AllocationBindingV1,
    parent_offset_bytes: u64,
    parent_extent_bytes: u64,
    relative_offset_bytes: u64,
    extent_bytes: u64,
    alignment: u64,
) -> Result<u64, ServiceAllocationErrorV1> {
    let offset_bytes = parent_offset_bytes
        .checked_add(relative_offset_bytes)
        .ok_or(ServiceAllocationErrorV1::InvalidRange)?;
    let parent_end = parent_offset_bytes
        .checked_add(parent_extent_bytes)
        .ok_or(ServiceAllocationErrorV1::InvalidRange)?;
    if extent_bytes == 0
        || alignment == 0
        || !alignment.is_power_of_two()
        || alignment > binding.alignment
        || !offset_bytes.is_multiple_of(alignment)
        || offset_bytes
            .checked_add(extent_bytes)
            .is_none_or(|end| end > parent_end)
    {
        return Err(ServiceAllocationErrorV1::InvalidRange);
    }
    Ok(offset_bytes)
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
        Some(
            AllocationTokenV1::DeviceMapped(_)
                | AllocationTokenV1::FixedDispatch(_)
                | AllocationTokenV1::HostMapped(_)
                | AllocationTokenV1::HostMappedInitialized(_)
        )
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
        AllocationTokenV1::FixedDispatch(token) => session
            .release_fixed_dispatch_data(token)
            .map_err(Into::into),
        AllocationTokenV1::HostCpuWritable(token) => session.release(token).map_err(Into::into),
        AllocationTokenV1::HostMapped(token) => {
            let token = session
                .unmap_from_gpu(token)
                .map_err(ServiceAllocationErrorV1::Memory)?;
            session.release(token).map_err(Into::into)
        }
        AllocationTokenV1::HostMappedInitialized(token) => session
            .release_fixed_dispatch_data(Gfx942FixedDispatchDataV1::host_visible_initialized(token))
            .map_err(Into::into),
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
    fn repeated_byte_entry_point_preserves_typed_device_state_custody() {
        type EntryPoint = fn(
            &mut ServiceAllocationSessionV1,
            Gfx942RepeatedByteContentV1,
            u64,
        ) -> Result<
            ServiceAllocationKeyV1<DeviceStateRoleV1, DeviceLocalAllocationV1>,
            ServiceAllocationErrorV1,
        >;

        let entry_point: EntryPoint =
            ServiceAllocationSessionV1::allocate_initialized_device_local_repeated_byte::<
                DeviceStateRoleV1,
            >;
        let role = fe2o3_kfd::Gfx942DeviceContentRoleV1::new([0x52; 32], 3).unwrap();
        let initialization = Gfx942RepeatedByteContentV1::new(role, 4097, 0).unwrap();
        assert_eq!(initialization.content().role(), role);
        assert_eq!(initialization.content().byte_len(), 4097);
        assert_eq!(initialization.repeated_byte(), 0);
        let _ = entry_point;
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

    #[test]
    fn sublease_registration_is_atomic_and_duplicate_consumption_is_rejected() {
        let expected = binding(
            AllocationRoleV1::DeviceWorkspace,
            AllocationKindV1::DeviceLocal,
        );
        let mut allocation = OwnedAllocationV1 {
            binding: expected,
            token: None,
            subleases: None,
        };

        assert!(matches!(
            reserve_sublease_layout(&mut allocation, [(0, 8_192, 4_096), (4_096, 4_096, 4_096)]),
            Err(ServiceAllocationErrorV1::AliasingRange)
        ));
        assert!(allocation.subleases.is_none());
        assert!(matches!(
            reserve_sublease_layout(
                &mut allocation,
                [(0, 4_096, 4_096), (4_096, 0, 4_096), (8_192, 4_096, 4_096)]
            ),
            Err(ServiceAllocationErrorV1::InvalidRange)
        ));
        assert!(allocation.subleases.is_none());
        assert!(matches!(
            reserve_sublease_layout(&mut allocation, []),
            Err(ServiceAllocationErrorV1::InvalidSubleaseCount)
        ));
        assert!(allocation.subleases.is_none());

        let retained =
            reserve_sublease_layout(&mut allocation, [(0, 4_096, 4_096), (8_192, 8_192, 4_096)])
                .unwrap();
        assert_eq!(allocation.subleases.as_deref(), Some(retained.as_slice()));
        assert!(matches!(
            reserve_sublease_layout(&mut allocation, [(4_096, 4_096, 4_096)]),
            Err(ServiceAllocationErrorV1::AllocationAlreadyPartitioned)
        ));
        assert_eq!(allocation.subleases.as_deref(), Some(retained.as_slice()));
    }

    #[test]
    fn checked_sublease_subranges_are_member_bounded_and_aligned() {
        let expected = binding(
            AllocationRoleV1::DeviceWorkspace,
            AllocationKindV1::DeviceLocal,
        );
        let member = ServiceAllocationSubleaseRangeV1 {
            offset_bytes: 4_096,
            extent_bytes: 8_192,
            alignment: 4_096,
        };
        assert_eq!(
            checked_sublease_subrange(expected, member, 2_048, 2_048, 2_048).unwrap(),
            6_144
        );
        for (relative, extent, alignment) in [
            (0, 0, 4_096),
            (1, 4_096, 4_096),
            (8_192, 1, 1),
            (4_096, 4_097, 1),
            (0, 4_096, 8_192),
            (0, 4_096, 3),
        ] {
            assert!(matches!(
                checked_sublease_subrange(expected, member, relative, extent, alignment),
                Err(ServiceAllocationErrorV1::InvalidRange)
            ));
        }
    }

    #[test]
    fn sublease_set_is_owner_role_kind_allocation_and_layout_bound() {
        let expected = binding(
            AllocationRoleV1::DeviceWorkspace,
            AllocationKindV1::DeviceLocal,
        );
        let ranges = validate_sublease_layout(expected, [(0, 4_096, 4_096)]).unwrap();
        let allocations = vec![OwnedAllocationV1 {
            binding: expected,
            token: None,
            subleases: Some(ranges.to_vec()),
        }];
        let exact =
            ServiceAllocationSubleaseSetV1::<DeviceWorkspaceRoleV1, DeviceLocalAllocationV1, 1> {
                key: key(expected),
                ranges,
            };
        assert_eq!(
            validate_sublease_set_binding(expected.owner, &allocations, &exact).unwrap(),
            0
        );

        let mut wrong_owner_binding = expected;
        wrong_owner_binding.owner.vm_owner_generation += 1;
        let wrong_owner =
            ServiceAllocationSubleaseSetV1::<DeviceWorkspaceRoleV1, DeviceLocalAllocationV1, 1> {
                key: key(wrong_owner_binding),
                ranges,
            };
        assert!(matches!(
            validate_sublease_set_binding(expected.owner, &allocations, &wrong_owner),
            Err(ServiceAllocationErrorV1::OwnerBindingMismatch)
        ));

        let wrong_role =
            ServiceAllocationSubleaseSetV1::<DeviceStateRoleV1, DeviceLocalAllocationV1, 1> {
                key: key(expected),
                ranges,
            };
        assert!(matches!(
            validate_sublease_set_binding(expected.owner, &allocations, &wrong_role),
            Err(ServiceAllocationErrorV1::RoleMismatch)
        ));
        let wrong_kind =
            ServiceAllocationSubleaseSetV1::<DeviceWorkspaceRoleV1, HostVisibleAllocationV1, 1> {
                key: key(expected),
                ranges,
            };
        assert!(matches!(
            validate_sublease_set_binding(expected.owner, &allocations, &wrong_kind),
            Err(ServiceAllocationErrorV1::KindMismatch)
        ));

        let mut wrong_allocation_binding = expected;
        wrong_allocation_binding.id += 1;
        let wrong_allocation =
            ServiceAllocationSubleaseSetV1::<DeviceWorkspaceRoleV1, DeviceLocalAllocationV1, 1> {
                key: key(wrong_allocation_binding),
                ranges,
            };
        assert!(matches!(
            validate_sublease_set_binding(expected.owner, &allocations, &wrong_allocation),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));

        let mut wrong_layout_ranges = ranges;
        wrong_layout_ranges[0].extent_bytes = 8_192;
        let wrong_layout =
            ServiceAllocationSubleaseSetV1::<DeviceWorkspaceRoleV1, DeviceLocalAllocationV1, 1> {
                key: key(expected),
                ranges: wrong_layout_ranges,
            };
        assert!(matches!(
            validate_sublease_set_binding(expected.owner, &allocations, &wrong_layout),
            Err(ServiceAllocationErrorV1::SubleaseBindingMismatch)
        ));
    }

    fn queue_ledger(expected: AllocationBindingV1) -> ServiceQueueAllocationLedgerV1 {
        ServiceQueueAllocationLedgerV1 {
            owner: expected.owner,
            next_allocation_id: 2,
            allocations: vec![OwnedAllocationV1 {
                binding: expected,
                token: None,
                subleases: None,
            }],
            device_bytes: expected.extent_bytes,
            host_bytes: 0,
            device_bindings: vec![expected],
            host_bindings: vec![],
        }
    }

    #[test]
    fn partition_insertion_and_removal_shift_only_the_host_data_suffix() {
        let mut device = binding(
            AllocationRoleV1::DeviceWorkspace,
            AllocationKindV1::DeviceLocal,
        );
        device.extent_bytes = 8_192;
        let old_ranges = validate_sublease_layout(device, [(0, 8_192, 4_096)]).unwrap();
        let mut host = binding(
            AllocationRoleV1::HostDownload,
            AllocationKindV1::HostVisible,
        );
        host.id = 2;
        host.extent_bytes = 4_096;
        let mut ledger = ServiceQueueAllocationLedgerV1 {
            owner: device.owner,
            next_allocation_id: 3,
            allocations: vec![
                OwnedAllocationV1 {
                    binding: device,
                    token: None,
                    subleases: Some(old_ranges.to_vec()),
                },
                OwnedAllocationV1 {
                    binding: host,
                    token: None,
                    subleases: None,
                },
            ],
            device_bytes: device.extent_bytes,
            host_bytes: host.extent_bytes,
            device_bindings: vec![device],
            host_bindings: vec![host],
        };
        let old_host = ServiceHostDispatchRangeV1 {
            binding: host,
            data_index: 1,
            offset_bytes: 0,
            extent_bytes: host.extent_bytes,
            sublease_index: None,
        };
        assert!(ledger.validate_range(old_host).is_ok());
        assert!(matches!(
            ledger.reissue_host_visible::<HostUploadRoleV1>(old_host),
            Err(ServiceAllocationErrorV1::RoleMismatch)
        ));

        let insertion = ledger
            .prepare_initialized_partition_insertion::<DeviceWorkspaceRoleV1, 2>(
                12_288,
                4_096,
                [(0, 4_096, 4_096), (4_096, 8_192, 4_096)],
            )
            .unwrap();
        assert_eq!(insertion.data_index(), 1);
        let (inserted, inserted_ranges) =
            ledger.commit_initialized_partition_insertion::<DeviceWorkspaceRoleV1, 2>(insertion);
        assert!(ledger.device_bindings == [device, inserted.key.binding]);
        assert_eq!(inserted_ranges[0].data_index, 1);
        assert_eq!(inserted_ranges[1].data_index, 1);
        assert!(matches!(
            ledger.validate_range(old_host),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
        let shifted_host = ledger
            .reissue_host_visible::<HostDownloadRoleV1>(old_host)
            .expect("retained host allocation must be reissued");
        assert_eq!(shifted_host.data_index, 2);
        assert!(ledger.validate_range(shifted_host).is_ok());
        assert!(ledger.validate_range(inserted_ranges[0]).is_ok());

        let removal = ledger
            .prepare_partitioned_removal(&inserted)
            .expect("fresh inserted witness");
        assert_eq!(removal.data_index(), 1);
        ledger.commit_partitioned_removal(removal);
        assert!(ledger.device_bindings == [device]);
        assert!(ledger.validate_range(old_host).is_ok());
        assert_eq!(
            ledger
                .reissue_host_visible::<HostDownloadRoleV1>(shifted_host)
                .unwrap()
                .data_index,
            1
        );
        assert!(matches!(
            ledger.validate_range(inserted_ranges[0]),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
        assert!(matches!(
            ledger.reissue_partitioned_device_local(&inserted),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
    }

    #[test]
    fn partition_removal_reissues_later_device_partition_at_current_ordinal() {
        let mut first = binding(
            AllocationRoleV1::DeviceWorkspace,
            AllocationKindV1::DeviceLocal,
        );
        first.extent_bytes = 4_096;
        let first_ranges = validate_sublease_layout(first, [(0, 4_096, 4_096)]).unwrap();
        let first_subleases =
            ServiceAllocationSubleaseSetV1::<DeviceWorkspaceRoleV1, DeviceLocalAllocationV1, 1> {
                key: key(first),
                ranges: first_ranges,
            };

        let mut second = first;
        second.id = 2;
        second.extent_bytes = 8_192;
        let second_ranges =
            validate_sublease_layout(second, [(0, 4_096, 4_096), (4_096, 4_096, 4_096)]).unwrap();
        let second_subleases =
            ServiceAllocationSubleaseSetV1::<DeviceWorkspaceRoleV1, DeviceLocalAllocationV1, 2> {
                key: key(second),
                ranges: second_ranges,
            };
        let mut ledger = ServiceQueueAllocationLedgerV1 {
            owner: first.owner,
            next_allocation_id: 3,
            allocations: vec![
                OwnedAllocationV1 {
                    binding: first,
                    token: None,
                    subleases: Some(first_ranges.to_vec()),
                },
                OwnedAllocationV1 {
                    binding: second,
                    token: None,
                    subleases: Some(second_ranges.to_vec()),
                },
            ],
            device_bytes: first.extent_bytes + second.extent_bytes,
            host_bytes: 0,
            device_bindings: vec![first, second],
            host_bindings: vec![],
        };
        let old_second = ledger
            .reissue_partitioned_device_local(&second_subleases)
            .unwrap();
        assert!(old_second.iter().all(|range| range.data_index == 1));

        let removal = ledger
            .prepare_partitioned_removal(&first_subleases)
            .unwrap();
        ledger.commit_partitioned_removal(removal);
        assert!(matches!(
            ledger.validate_range(old_second[0]),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
        let reissued = ledger
            .reissue_partitioned_device_local(&second_subleases)
            .unwrap();
        assert!(reissued.iter().all(|range| range.data_index == 0));
        assert_eq!(reissued[0].offset_bytes, 0);
        assert_eq!(reissued[1].offset_bytes, 4_096);
        assert!(reissued
            .iter()
            .all(|range| ledger.validate_range(*range).is_ok()));
        assert!(matches!(
            ledger.reissue_partitioned_device_local(&first_subleases),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
    }

    #[test]
    fn host_replacement_advances_binding_and_preserves_exact_data_ordinal() {
        let mut host = binding(
            AllocationRoleV1::HostDownload,
            AllocationKindV1::HostVisible,
        );
        host.extent_bytes = 4_096;
        let mut ledger = ServiceQueueAllocationLedgerV1 {
            owner: host.owner,
            next_allocation_id: 2,
            allocations: vec![OwnedAllocationV1 {
                binding: host,
                token: None,
                subleases: None,
            }],
            device_bytes: 0,
            host_bytes: host.extent_bytes,
            device_bindings: vec![],
            host_bindings: vec![host],
        };
        let old = ServiceHostDispatchRangeV1 {
            binding: host,
            data_index: 0,
            offset_bytes: 0,
            extent_bytes: host.extent_bytes,
            sublease_index: None,
        };
        let replacement = ledger
            .prepare_host_replacement::<HostDownloadRoleV1>(old, 12_288)
            .unwrap();
        assert_eq!(replacement.data_index(), 0);
        ledger.commit_host_replacement_release(&replacement);
        let fresh = ledger.commit_host_replacement(replacement);
        assert_eq!(fresh.data_index, 0);
        assert_eq!(fresh.extent_bytes(), 12_288);
        assert_ne!(fresh.binding.id, old.binding.id);
        assert!(ledger.validate_range(fresh).is_ok());
        assert!(matches!(
            ledger.validate_range(old),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
        let snapshot = ServiceHostDispatchSnapshotRangeV1::from_initialized_range(fresh);
        assert!(snapshot.enclosing_dispatch_range() == fresh);
    }

    #[test]
    fn partition_insertion_rejects_invalid_layout_without_ledger_mutation() {
        let expected = binding(
            AllocationRoleV1::DeviceWorkspace,
            AllocationKindV1::DeviceLocal,
        );
        let mut ledger = queue_ledger(expected);
        let next_id = ledger.next_allocation_id;
        let error = match ledger
            .prepare_initialized_partition_insertion::<DeviceWorkspaceRoleV1, 2>(
                8_192,
                4_096,
                [(0, 8_192, 4_096), (4_096, 4_096, 4_096)],
            ) {
            Ok(_) => panic!("overlapping insertion layout was admitted"),
            Err(error) => error,
        };
        assert!(matches!(error, ServiceAllocationErrorV1::AliasingRange));
        assert_eq!(ledger.next_allocation_id, next_id);
        assert!(ledger.device_bindings == [expected]);
        assert_eq!(ledger.device_bytes, expected.extent_bytes);
    }

    #[test]
    fn coherent_dispatch_ranges_are_owner_generation_and_ordinal_bound() {
        let expected = binding(
            AllocationRoleV1::HostDownload,
            AllocationKindV1::HostVisible,
        );
        let ledger = ServiceQueueAllocationLedgerV1 {
            owner: expected.owner,
            next_allocation_id: 2,
            allocations: vec![OwnedAllocationV1 {
                binding: expected,
                token: None,
                subleases: None,
            }],
            device_bytes: 0,
            host_bytes: expected.extent_bytes,
            device_bindings: vec![],
            host_bindings: vec![expected],
        };
        let exact = ServiceHostDispatchRangeV1 {
            binding: expected,
            data_index: 0,
            offset_bytes: 0,
            extent_bytes: expected.extent_bytes,
            sublease_index: None,
        };
        assert!(ledger.validate_range(exact).is_ok());

        let mut stale = exact;
        stale.binding.generation += 1;
        assert!(matches!(
            ledger.validate_range(stale),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
        let mut wrong_owner = exact;
        wrong_owner.binding.owner.owner_generation += 1;
        assert!(matches!(
            ledger.validate_range(wrong_owner),
            Err(ServiceAllocationErrorV1::OwnerBindingMismatch)
        ));
        let mut wrong_index = exact;
        wrong_index.data_index = 1;
        assert!(matches!(
            ledger.validate_range(wrong_index),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
        let mut out_of_range = exact;
        out_of_range.offset_bytes = expected.extent_bytes;
        assert!(matches!(
            ledger.validate_range(out_of_range),
            Err(ServiceAllocationErrorV1::InvalidRange)
        ));
    }

    #[test]
    fn host_snapshot_ranges_require_exact_identity_and_strict_enclosure() {
        let expected = binding(
            AllocationRoleV1::HostDownload,
            AllocationKindV1::HostVisible,
        );
        let snapshot = ServiceHostDispatchRangeV1 {
            binding: expected,
            data_index: 2,
            offset_bytes: 0,
            extent_bytes: 12_288,
            sublease_index: Some(3),
        };
        let interior = ServiceHostDispatchRangeV1 {
            offset_bytes: 4_096,
            extent_bytes: 4_096,
            ..snapshot
        };
        assert!(validate_host_dispatch_snapshot(interior, snapshot).is_ok());

        let token = ServiceHostDispatchSnapshotRangeV1 { range: snapshot };
        assert_eq!(token.offset_bytes(), 0);
        assert_eq!(token.extent_bytes(), 12_288);
        let buffer =
            crate::batch::ServiceFixedDispatchBufferV1::new_host_visible_with_completed_snapshot(
                1, interior, token,
            )
            .unwrap();
        assert_eq!(buffer.explicit_argument_index(), 1);
        assert_eq!(buffer.completed_snapshot(), Some(token));

        let mut stale = snapshot;
        stale.binding.generation += 1;
        let mut wrong_owner = snapshot;
        wrong_owner.binding.owner.vm_owner_generation += 1;
        let mut wrong_ordinal = snapshot;
        wrong_ordinal.data_index += 1;
        let mut wrong_sublease = snapshot;
        wrong_sublease.sublease_index = Some(4);
        let no_prefix = ServiceHostDispatchRangeV1 {
            offset_bytes: interior.offset_bytes,
            ..snapshot
        };
        let no_suffix = ServiceHostDispatchRangeV1 {
            extent_bytes: interior.offset_bytes + interior.extent_bytes,
            ..snapshot
        };
        let overflowing = ServiceHostDispatchRangeV1 {
            offset_bytes: u64::MAX,
            extent_bytes: 1,
            ..snapshot
        };
        for rejected in [
            stale,
            wrong_owner,
            wrong_ordinal,
            wrong_sublease,
            no_prefix,
            no_suffix,
            overflowing,
        ] {
            assert!(matches!(
                validate_host_dispatch_snapshot(interior, rejected),
                Err(ServiceAllocationErrorV1::InvalidRange)
            ));
        }
    }

    #[test]
    fn queue_snapshot_revalidation_rejects_stale_enclosing_authority() {
        let expected = binding(
            AllocationRoleV1::HostDownload,
            AllocationKindV1::HostVisible,
        );
        let ledger = ServiceQueueAllocationLedgerV1 {
            owner: expected.owner,
            next_allocation_id: 2,
            allocations: vec![OwnedAllocationV1 {
                binding: expected,
                token: None,
                subleases: None,
            }],
            device_bytes: 0,
            host_bytes: expected.extent_bytes,
            device_bindings: vec![],
            host_bindings: vec![expected],
        };
        let snapshot_range = ServiceHostDispatchRangeV1 {
            binding: expected,
            data_index: 0,
            offset_bytes: 0,
            extent_bytes: 12_288,
            sublease_index: None,
        };
        let interior = ServiceHostDispatchRangeV1 {
            offset_bytes: 4_096,
            extent_bytes: 4_096,
            ..snapshot_range
        };
        let snapshot = ServiceHostDispatchSnapshotRangeV1 {
            range: snapshot_range,
        };
        assert!(ledger
            .validate_host_dispatch_snapshot(interior, snapshot)
            .is_ok());

        let mut stale_range = snapshot_range;
        stale_range.binding.generation += 1;
        let stale = ServiceHostDispatchSnapshotRangeV1 { range: stale_range };
        assert!(matches!(
            ledger.validate_host_dispatch_snapshot(interior, stale),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
    }

    #[test]
    fn dispatch_subranges_preserve_generation_ordinal_and_member_binding() {
        let expected = binding(
            AllocationRoleV1::DeviceWorkspace,
            AllocationKindV1::DeviceLocal,
        );
        let member = ServiceDeviceDispatchRangeV1 {
            binding: expected,
            data_index: 3,
            offset_bytes: 4_096,
            extent_bytes: 8_192,
            sublease_index: Some(7),
        };
        let narrowed = member.checked_subrange(2_048, 2_048, 2_048).unwrap();
        assert!(narrowed.binding == expected);
        assert_eq!(narrowed.data_index, 3);
        assert_eq!(narrowed.offset_bytes(), 6_144);
        assert_eq!(narrowed.extent_bytes(), 2_048);
        assert_eq!(narrowed.sublease_index, Some(7));
        for (relative, extent, alignment) in [
            (0, 0, 4_096),
            (1, 4_096, 4_096),
            (8_192, 1, 1),
            (4_096, 4_097, 1),
            (0, 4_096, 8_192),
        ] {
            assert!(matches!(
                member.checked_subrange(relative, extent, alignment),
                Err(ServiceAllocationErrorV1::InvalidRange)
            ));
        }
    }

    #[test]
    fn queue_range_revalidation_rejects_owner_generation_index_and_extent_drift() {
        let expected = binding(AllocationRoleV1::DeviceInput, AllocationKindV1::DeviceLocal);
        let ledger = queue_ledger(expected);
        let retained = ServiceDeviceDispatchRangeV1 {
            binding: expected,
            data_index: 0,
            offset_bytes: 4_096,
            extent_bytes: 4_096,
            sublease_index: None,
        };
        assert!(ledger.validate_range(retained).is_ok());

        let mut wrong_owner = retained;
        wrong_owner.binding.owner.vm_owner_generation += 1;
        assert!(matches!(
            ledger.validate_range(wrong_owner),
            Err(ServiceAllocationErrorV1::OwnerBindingMismatch)
        ));
        let mut wrong_generation = retained;
        wrong_generation.binding.generation += 1;
        assert!(matches!(
            ledger.validate_range(wrong_generation),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
        let mut wrong_index = retained;
        wrong_index.data_index = 1;
        assert!(matches!(
            ledger.validate_range(wrong_index),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
        let mut wrong_extent = retained;
        wrong_extent.extent_bytes = expected.extent_bytes;
        assert!(matches!(
            ledger.validate_range(wrong_extent),
            Err(ServiceAllocationErrorV1::InvalidRange)
        ));
    }

    #[test]
    fn partitioned_queue_admission_accepts_member_subranges_and_rejects_escape() {
        let expected = binding(
            AllocationRoleV1::DeviceWorkspace,
            AllocationKindV1::DeviceLocal,
        );
        let mut ledger = queue_ledger(expected);
        let ranges =
            validate_sublease_layout(expected, [(0, 4_096, 4_096), (8_192, 8_192, 4_096)]).unwrap();
        ledger.allocations[0].subleases = Some(ranges.to_vec());

        let legacy = ServiceDeviceDispatchRangeV1 {
            binding: expected,
            data_index: 0,
            offset_bytes: 0,
            extent_bytes: 4_096,
            sublease_index: None,
        };
        assert!(matches!(
            ledger.validate_range(legacy),
            Err(ServiceAllocationErrorV1::SubleaseBindingMismatch)
        ));
        let exact = ServiceDeviceDispatchRangeV1 {
            sublease_index: Some(0),
            ..legacy
        };
        assert!(ledger.validate_range(exact).is_ok());

        let mut wrong_member = exact;
        wrong_member.sublease_index = Some(1);
        assert!(matches!(
            ledger.validate_range(wrong_member),
            Err(ServiceAllocationErrorV1::SubleaseBindingMismatch)
        ));
        let mut partial = exact;
        partial.extent_bytes = 2_048;
        assert!(ledger.validate_range(partial).is_ok());
        let mut escaped = partial;
        escaped.offset_bytes = 3_072;
        escaped.extent_bytes = 2_048;
        assert!(matches!(
            ledger.validate_range(escaped),
            Err(ServiceAllocationErrorV1::SubleaseBindingMismatch)
        ));
        let mut duplicate_member = exact;
        duplicate_member.offset_bytes = 8_192;
        duplicate_member.extent_bytes = 8_192;
        assert!(matches!(
            ledger.validate_range(duplicate_member),
            Err(ServiceAllocationErrorV1::SubleaseBindingMismatch)
        ));
    }

    #[test]
    fn replacement_changes_generation_and_stales_the_prior_range() {
        let expected = binding(AllocationRoleV1::DeviceInput, AllocationKindV1::DeviceLocal);
        let mut ledger = queue_ledger(expected);
        let old = ServiceDeviceDispatchRangeV1 {
            binding: expected,
            data_index: 0,
            offset_bytes: 0,
            extent_bytes: expected.extent_bytes,
            sublease_index: None,
        };
        let replacement = ledger
            .prepare_initialized_replacement::<DeviceInputRoleV1>(old, 8_192, 4_096)
            .unwrap();
        ledger.commit_replacement_release(&replacement);
        let new = ledger.commit_initialized_replacement(replacement);

        assert_eq!(new.data_index, old.data_index);
        assert_eq!(new.extent_bytes, 8_192);
        assert_ne!(new.binding.id, old.binding.id);
        assert!(matches!(
            ledger.validate_range(old),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
        assert!(ledger.validate_range(new).is_ok());
        assert_eq!(ledger.device_bytes, 8_192);
    }

    #[test]
    fn replacement_clears_partition_and_stales_set_and_emitted_member() {
        let expected = binding(
            AllocationRoleV1::DeviceWorkspace,
            AllocationKindV1::DeviceLocal,
        );
        let mut ledger = queue_ledger(expected);
        let ranges =
            validate_sublease_layout(expected, [(0, expected.extent_bytes, expected.alignment)])
                .unwrap();
        ledger.allocations[0].subleases = Some(ranges.to_vec());
        let subleases =
            ServiceAllocationSubleaseSetV1::<DeviceWorkspaceRoleV1, DeviceLocalAllocationV1, 1> {
                key: key(expected),
                ranges,
            };
        let old = ServiceDeviceDispatchRangeV1 {
            binding: expected,
            data_index: 0,
            offset_bytes: 0,
            extent_bytes: expected.extent_bytes,
            sublease_index: Some(0),
        };
        assert!(ledger.validate_range(old).is_ok());

        let replacement = ledger
            .prepare_initialized_replacement::<DeviceWorkspaceRoleV1>(old, 8_192, 4_096)
            .unwrap();
        ledger.commit_replacement_release(&replacement);
        let new = ledger.commit_initialized_replacement(replacement);

        assert!(ledger.allocations[0].subleases.is_none());
        assert!(matches!(
            ledger.validate_range(old),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
        assert!(matches!(
            validate_sublease_set_binding(ledger.owner, &ledger.allocations, &subleases),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
        assert!(ledger.validate_range(new).is_ok());
    }

    #[test]
    fn partitioned_replacement_rebinds_every_member_and_stales_prior_custody() {
        let expected = binding(
            AllocationRoleV1::DeviceWorkspace,
            AllocationKindV1::DeviceLocal,
        );
        let mut ledger = queue_ledger(expected);
        let old_ranges =
            validate_sublease_layout(expected, [(0, 4_096, 4_096), (8_192, 8_192, 4_096)]).unwrap();
        ledger.allocations[0].subleases = Some(old_ranges.to_vec());
        let old =
            ServiceAllocationSubleaseSetV1::<DeviceWorkspaceRoleV1, DeviceLocalAllocationV1, 2> {
                key: key(expected),
                ranges: old_ranges,
            };

        let (replacement, new_ranges) = ledger
            .prepare_initialized_partition_replacement::<DeviceWorkspaceRoleV1, 2, 3>(
                &old,
                12_288,
                4_096,
                [
                    (0, 4_096, 4_096),
                    (4_096, 4_096, 4_096),
                    (8_192, 4_096, 4_096),
                ],
            )
            .unwrap();
        ledger.commit_replacement_release(&replacement);
        let (new, dispatch_ranges) = ledger
            .commit_initialized_partitioned_replacement::<DeviceWorkspaceRoleV1, 3>(
                replacement,
                new_ranges,
            );

        assert_eq!(new.len(), 3);
        assert_eq!(ledger.device_bytes, 12_288);
        assert!(matches!(
            validate_sublease_set_binding(ledger.owner, &ledger.allocations, &old),
            Err(ServiceAllocationErrorV1::AllocationGenerationMismatch)
        ));
        assert_eq!(
            validate_sublease_set_binding(ledger.owner, &ledger.allocations, &new).unwrap(),
            0
        );
        for (index, range) in dispatch_ranges.into_iter().enumerate() {
            assert_eq!(range.data_index, 0);
            assert_eq!(range.offset_bytes, index as u64 * 4_096);
            assert_eq!(range.extent_bytes, 4_096);
            assert_eq!(range.sublease_index, Some(index));
            assert!(ledger.validate_range(range).is_ok());
        }
    }

    #[test]
    fn partitioned_replacement_rejects_new_layout_before_generation_reservation() {
        let expected = binding(
            AllocationRoleV1::DeviceWorkspace,
            AllocationKindV1::DeviceLocal,
        );
        let mut ledger = queue_ledger(expected);
        let old_ranges = validate_sublease_layout(expected, [(0, 4_096, 4_096)]).unwrap();
        ledger.allocations[0].subleases = Some(old_ranges.to_vec());
        let old =
            ServiceAllocationSubleaseSetV1::<DeviceWorkspaceRoleV1, DeviceLocalAllocationV1, 1> {
                key: key(expected),
                ranges: old_ranges,
            };
        let next_id = ledger.next_allocation_id;
        assert!(matches!(
            ledger.prepare_initialized_partition_replacement::<DeviceWorkspaceRoleV1, 1, 2>(
                &old,
                8_192,
                4_096,
                [(0, 4_096, 4_096), (2_048, 4_096, 2_048)],
            ),
            Err(ServiceAllocationErrorV1::AliasingRange)
        ));
        assert_eq!(ledger.next_allocation_id, next_id);
        assert_eq!(ledger.device_bytes, expected.extent_bytes);
        assert_eq!(
            validate_sublease_set_binding(ledger.owner, &ledger.allocations, &old).unwrap(),
            0
        );
    }

    #[test]
    fn replacement_rejects_partial_range_and_role_drift_before_mutation() {
        let expected = binding(AllocationRoleV1::DeviceInput, AllocationKindV1::DeviceLocal);
        let mut ledger = queue_ledger(expected);
        let partial = ServiceDeviceDispatchRangeV1 {
            binding: expected,
            data_index: 0,
            offset_bytes: 0,
            extent_bytes: 4_096,
            sublease_index: None,
        };
        assert!(matches!(
            ledger.prepare_initialized_replacement::<DeviceInputRoleV1>(partial, 4_096, 4_096),
            Err(ServiceAllocationErrorV1::InvalidRange)
        ));
        let full = ServiceDeviceDispatchRangeV1 {
            extent_bytes: expected.extent_bytes,
            ..partial
        };
        assert!(matches!(
            ledger.prepare_initialized_replacement::<DeviceStateRoleV1>(full, 4_096, 4_096),
            Err(ServiceAllocationErrorV1::RoleMismatch)
        ));
        assert_eq!(ledger.next_allocation_id, 2);
        assert_eq!(ledger.device_bytes, expected.extent_bytes);
        assert!(ledger.validate_range(full).is_ok());
    }
}
