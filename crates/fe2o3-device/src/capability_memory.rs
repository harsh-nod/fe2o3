//! Kernel-branded, address-space-qualified memory-view contracts.
//!
//! The exported kernel entry keeps using the established shared-slice and
//! write-only-disjoint-slice ABI. `#[kernel(typed)]` binds those physical
//! arguments to these logical views for its private, kernel-branded body.
//! Supported operations delegate to the existing reviewed memory contracts;
//! there is no raw-pointer fallback implementation in this module.

use core::fmt;
use core::marker::PhantomData;
use core::mem::{align_of, size_of};

use crate::{
    Blocked, DisjointBlock, DisjointIndex, InitialEpoch, KernelCapabilityBrand, KernelContext,
    KernelLaunch, KernelTarget, MemoryScope, NextEpoch, SynchronizationEpoch, ThreadIndex,
    UnbrandedCapability, WorkgroupCapability, WriteOnlyDisjointSlice, memory,
};

/// Version of the source memory-view representation and role contract.
pub const CAPABILITY_MEMORY_VIEW_CONTRACT_VERSION_V1: u16 = 1;

/// Version of the explicit unsafe raw-memory admission contract.
pub const UNSAFE_RAW_MEMORY_OBLIGATION_CONTRACT_VERSION_V1: u16 = 1;

/// Stable address-space identity recorded at an unsafe raw-memory boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MemoryAddressSpaceV1 {
    Private = 1,
    Workgroup = 2,
    Global = 3,
}

/// Stable access identity recorded at an unsafe raw-memory boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MemoryAccessV1 {
    ReadOnly = 1,
    WriteOnly = 2,
    ReadWrite = 3,
    AtomicReadWrite = 4,
}

/// Stable alias identity recorded at an unsafe raw-memory boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MemoryAliasV1 {
    Shared = 1,
    Exclusive = 2,
    Disjoint = 3,
    Atomic = 4,
}

/// Closed set of facts an unsafe raw-memory caller must establish.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct UnsafeRawMemoryObligationSetV1(u16);

impl UnsafeRawMemoryObligationSetV1 {
    pub const ADDRESS_SPACE: u16 = 1 << 0;
    pub const ALLOCATION_PROVENANCE: u16 = 1 << 1;
    pub const EXTENT: u16 = 1 << 2;
    pub const ALIGNMENT_AND_LAYOUT: u16 = 1 << 3;
    pub const ACCESS: u16 = 1 << 4;
    pub const ALIAS_AND_RACE_FREEDOM: u16 = 1 << 5;
    pub const LIFETIME: u16 = 1 << 6;
    pub const INITIALIZED_READS: u16 = 1 << 7;

    const BASE: u16 = Self::ADDRESS_SPACE
        | Self::ALLOCATION_PROVENANCE
        | Self::EXTENT
        | Self::ALIGNMENT_AND_LAYOUT
        | Self::ACCESS
        | Self::ALIAS_AND_RACE_FREEDOM
        | Self::LIFETIME;

    const fn for_access(access: MemoryAccessV1) -> Self {
        let initialized_reads = match access {
            MemoryAccessV1::WriteOnly => 0,
            MemoryAccessV1::ReadOnly
            | MemoryAccessV1::ReadWrite
            | MemoryAccessV1::AtomicReadWrite => Self::INITIALIZED_READS,
        };
        Self(Self::BASE | initialized_reads)
    }

    /// Stable bit representation for receipts and MIR admission.
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Whether this set contains one declared obligation bit.
    pub const fn contains(self, obligation: u16) -> bool {
        self.0 & obligation == obligation
    }
}

/// Exact declaration attached to every unsafe raw-memory view construction.
///
/// This record grants no authority. It makes the caller's complete proof
/// obligation explicit and gives MIR admission a stable value to authenticate.
/// A proof-required build must reject any obligation that remains unresolved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_unsafe_raw_memory_obligation_v1"]
pub struct UnsafeRawMemoryObligationV1 {
    contract_version: u16,
    address_space: MemoryAddressSpaceV1,
    access: MemoryAccessV1,
    aliasing: MemoryAliasV1,
    required: UnsafeRawMemoryObligationSetV1,
}

impl UnsafeRawMemoryObligationV1 {
    /// Exact record for one statically typed address-space and role pair.
    pub const fn for_view<Space: AddressSpace, Role: MemoryRole>() -> Self {
        Self {
            contract_version: UNSAFE_RAW_MEMORY_OBLIGATION_CONTRACT_VERSION_V1,
            address_space: Space::IDENTITY_V1,
            access: Role::ACCESS_V1,
            aliasing: Role::ALIASING_V1,
            required: UnsafeRawMemoryObligationSetV1::for_access(Role::ACCESS_V1),
        }
    }

    pub const fn contract_version(self) -> u16 {
        self.contract_version
    }

    pub const fn address_space(self) -> MemoryAddressSpaceV1 {
        self.address_space
    }

    pub const fn access(self) -> MemoryAccessV1 {
        self.access
    }

    pub const fn aliasing(self) -> MemoryAliasV1 {
        self.aliasing
    }

    pub const fn required(self) -> UnsafeRawMemoryObligationSetV1 {
        self.required
    }
}

mod sealed {
    pub trait AddressSpace {}
    pub trait AccessMode {}
    pub trait AliasMode {}
    pub trait Element {}
    pub trait Role {}
    pub trait RawRole {}
    pub trait View {}
}

/// A compiler-recognized logical device address space.
pub trait AddressSpace: sealed::AddressSpace {
    #[doc(hidden)]
    const IDENTITY_V1: MemoryAddressSpaceV1;
}

/// The operations exposed by a memory role.
pub trait AccessMode: sealed::AccessMode {}

/// The alias discipline required when a view is issued.
pub trait AliasMode: sealed::AliasMode {}

/// Scalar elements admitted by typed global-memory operations in V1.
pub trait CapabilityMemoryElementV1: sealed::Element + Copy {}

macro_rules! capability_memory_elements_v1 {
    ($($element:ty),+ $(,)?) => {
        $(
            impl sealed::Element for $element {}
            impl CapabilityMemoryElementV1 for $element {}
        )+
    };
}

capability_memory_elements_v1!(i8, u8, i16, u16, i32, u32, i64, u64, f32, f64);

/// A closed pairing of access, alias, and physical transport contracts.
///
/// `Physical` is intentionally part of the sealed role definition. It lets the
/// logical view retain the reviewed Rust value that arrived at the physical
/// kernel entry instead of reconstructing a slice from raw parts.
pub trait MemoryRole: sealed::Role {
    type Access: AccessMode;
    type Aliasing: AliasMode;
    #[doc(hidden)]
    const ACCESS_V1: MemoryAccessV1;
    #[doc(hidden)]
    const ALIASING_V1: MemoryAliasV1;
    #[doc(hidden)]
    type Physical<'kernel, T: 'kernel>;
}

/// Sealed roles that can be constructed at the explicit unsafe raw boundary.
#[doc(hidden)]
pub trait RawMemoryRoleV1: MemoryRole + sealed::RawRole {
    #[doc(hidden)]
    unsafe fn __from_raw_parts<'memory, T: CapabilityMemoryElementV1 + 'memory>(
        pointer: *mut T,
        len: usize,
    ) -> Self::Physical<'memory, T>;
}

/// Global device memory.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_capability_global_address_space_v1"]
pub enum GlobalAddressSpace {}

/// Workgroup-shared device memory.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_capability_workgroup_address_space_v1"]
pub enum WorkgroupAddressSpace {}

/// Invocation-private device memory.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_capability_private_address_space_v1"]
pub enum PrivateAddressSpace {}

impl sealed::AddressSpace for GlobalAddressSpace {}
impl sealed::AddressSpace for WorkgroupAddressSpace {}
impl sealed::AddressSpace for PrivateAddressSpace {}
impl AddressSpace for GlobalAddressSpace {
    const IDENTITY_V1: MemoryAddressSpaceV1 = MemoryAddressSpaceV1::Global;
}
impl AddressSpace for WorkgroupAddressSpace {
    const IDENTITY_V1: MemoryAddressSpaceV1 = MemoryAddressSpaceV1::Workgroup;
}
impl AddressSpace for PrivateAddressSpace {
    const IDENTITY_V1: MemoryAddressSpaceV1 = MemoryAddressSpaceV1::Private;
}

/// Read operations with no mutation surface.
#[derive(Debug)]
pub enum ReadAccess {}

/// Write operations with no read surface.
#[derive(Debug)]
pub enum WriteAccess {}

/// Read and write operations under one exclusive alias.
#[derive(Debug)]
pub enum ReadWriteAccess {}

/// Atomic read-modify-write access only.
#[derive(Debug)]
pub enum AtomicReadWriteAccess {}

impl sealed::AccessMode for ReadAccess {}
impl sealed::AccessMode for WriteAccess {}
impl sealed::AccessMode for ReadWriteAccess {}
impl sealed::AccessMode for AtomicReadWriteAccess {}
impl AccessMode for ReadAccess {}
impl AccessMode for WriteAccess {}
impl AccessMode for ReadWriteAccess {}
impl AccessMode for AtomicReadWriteAccess {}

/// Shared, immutable aliasing for the view lifetime.
#[derive(Debug)]
pub enum SharedAliases {}

/// One exclusive alias for the complete view lifetime.
#[derive(Debug)]
pub enum ExclusiveAliases {}

/// An invocation-to-element mapping whose selected writes are disjoint.
#[derive(Debug)]
pub enum DisjointAliases<IndexSpace> {
    _IndexSpace(
        core::convert::Infallible,
        PhantomData<fn(IndexSpace) -> IndexSpace>,
    ),
}

/// Aliasing admitted only through atomics of one exact width.
#[derive(Debug)]
pub enum AtomicAliases {}

impl sealed::AliasMode for SharedAliases {}
impl sealed::AliasMode for ExclusiveAliases {}
impl<IndexSpace> sealed::AliasMode for DisjointAliases<IndexSpace> {}
impl sealed::AliasMode for AtomicAliases {}
impl AliasMode for SharedAliases {}
impl AliasMode for ExclusiveAliases {}
impl<IndexSpace> AliasMode for DisjointAliases<IndexSpace> {}
impl AliasMode for AtomicAliases {}

/// Shared read-only memory.
///
/// Issuance requires every element to be initialized and readable for the
/// complete view lifetime. Concurrent mutation requires a separately admitted
/// synchronization or atomic contract; this role does not supply one.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_capability_read_only_v1"]
pub enum ReadOnly {}

/// Write-only memory partitioned by an exact disjoint index mapping.
///
/// The view exposes no read operation, so its storage need not initially hold
/// valid `T` values. A successful write initializes only the selected element.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_capability_disjoint_write_v1"]
pub enum DisjointWrite<IndexSpace> {
    _IndexSpace(
        core::convert::Infallible,
        PhantomData<fn(IndexSpace) -> IndexSpace>,
    ),
}

/// Exclusive read-write memory.
///
/// The issuing operation must establish that no other alias accesses the same
/// elements for the view lifetime.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_capability_exclusive_read_write_v1"]
pub enum ExclusiveReadWrite {}

/// Shared read-modify-write memory accessed only through scoped atomics.
///
/// `Scope` is part of the logical type and cannot be changed by a caller. The
/// source importer must prove coherent storage, one exact atomic width, and no
/// conflicting non-atomic aliases before it may bind this role.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_capability_atomic_read_write_v1"]
pub enum AtomicReadWrite<Scope: MemoryScope> {
    _Scope(core::convert::Infallible, PhantomData<fn(Scope) -> Scope>),
}

impl sealed::Role for ReadOnly {}
impl<IndexSpace> sealed::Role for DisjointWrite<IndexSpace> {}
impl sealed::Role for ExclusiveReadWrite {}
impl<Scope: MemoryScope> sealed::Role for AtomicReadWrite<Scope> {}
impl sealed::RawRole for ReadOnly {}
impl<IndexSpace> sealed::RawRole for DisjointWrite<IndexSpace> {}
impl sealed::RawRole for ExclusiveReadWrite {}
impl<Scope: MemoryScope> sealed::RawRole for AtomicReadWrite<Scope> {}

impl MemoryRole for ReadOnly {
    type Access = ReadAccess;
    type Aliasing = SharedAliases;
    const ACCESS_V1: MemoryAccessV1 = MemoryAccessV1::ReadOnly;
    const ALIASING_V1: MemoryAliasV1 = MemoryAliasV1::Shared;
    type Physical<'kernel, T: 'kernel> = &'kernel [T];
}

impl<IndexSpace> MemoryRole for DisjointWrite<IndexSpace> {
    type Access = WriteAccess;
    type Aliasing = DisjointAliases<IndexSpace>;
    const ACCESS_V1: MemoryAccessV1 = MemoryAccessV1::WriteOnly;
    const ALIASING_V1: MemoryAliasV1 = MemoryAliasV1::Disjoint;
    type Physical<'kernel, T: 'kernel> = WriteOnlyDisjointSlice<T, IndexSpace>;
}

impl MemoryRole for ExclusiveReadWrite {
    type Access = ReadWriteAccess;
    type Aliasing = ExclusiveAliases;
    const ACCESS_V1: MemoryAccessV1 = MemoryAccessV1::ReadWrite;
    const ALIASING_V1: MemoryAliasV1 = MemoryAliasV1::Exclusive;
    type Physical<'kernel, T: 'kernel> = &'kernel mut [T];
}

impl<Scope: MemoryScope> MemoryRole for AtomicReadWrite<Scope> {
    type Access = AtomicReadWriteAccess;
    type Aliasing = AtomicAliases;
    const ACCESS_V1: MemoryAccessV1 = MemoryAccessV1::AtomicReadWrite;
    const ALIASING_V1: MemoryAliasV1 = MemoryAliasV1::Atomic;
    type Physical<'kernel, T: 'kernel> = &'kernel [T];
}

impl RawMemoryRoleV1 for ReadOnly {
    unsafe fn __from_raw_parts<'memory, T: CapabilityMemoryElementV1 + 'memory>(
        pointer: *mut T,
        len: usize,
    ) -> Self::Physical<'memory, T> {
        // SAFETY: exactly the obligations recorded by the constructor are
        // delegated to its unsafe caller.
        unsafe { core::slice::from_raw_parts(pointer.cast_const(), len) }
    }
}

impl<IndexSpace> RawMemoryRoleV1 for DisjointWrite<IndexSpace> {
    unsafe fn __from_raw_parts<'memory, T: CapabilityMemoryElementV1 + 'memory>(
        pointer: *mut T,
        len: usize,
    ) -> Self::Physical<'memory, T> {
        // SAFETY: exactly the obligations recorded by the constructor are
        // delegated to its unsafe caller.
        unsafe { WriteOnlyDisjointSlice::from_raw_parts(pointer, len) }
    }
}

impl RawMemoryRoleV1 for ExclusiveReadWrite {
    unsafe fn __from_raw_parts<'memory, T: CapabilityMemoryElementV1 + 'memory>(
        pointer: *mut T,
        len: usize,
    ) -> Self::Physical<'memory, T> {
        // SAFETY: exactly the obligations recorded by the constructor are
        // delegated to its unsafe caller.
        unsafe { core::slice::from_raw_parts_mut(pointer, len) }
    }
}

impl<Scope: MemoryScope> RawMemoryRoleV1 for AtomicReadWrite<Scope> {
    unsafe fn __from_raw_parts<'memory, T: CapabilityMemoryElementV1 + 'memory>(
        pointer: *mut T,
        len: usize,
    ) -> Self::Physical<'memory, T> {
        // SAFETY: exactly the obligations recorded by the constructor are
        // delegated to its unsafe caller.
        unsafe { core::slice::from_raw_parts(pointer.cast_const(), len) }
    }
}

type InvariantLifetime<'kernel> = fn(&'kernel ()) -> &'kernel ();
type InvariantType<T> = fn(T) -> T;

/// Exact workgroup and epoch identity carried by a workgroup-memory view.
///
/// The fields are private and the type is invariant in every parameter. It is
/// an identity, not independently constructible authority.
#[doc(hidden)]
#[rustc_diagnostic_item = "fe2o3_device_workgroup_memory_brand_v1"]
pub struct WorkgroupMemoryBrand<'workgroup, KernelBrand, Epoch: SynchronizationEpoch> {
    _workgroup: PhantomData<InvariantLifetime<'workgroup>>,
    _kernel: PhantomData<InvariantType<KernelBrand>>,
    _epoch: PhantomData<InvariantType<Epoch>>,
}

/// Invocation-rank mapping for one workgroup-local allocation.
#[derive(Debug)]
#[rustc_diagnostic_item = "fe2o3_device_workgroup_memory_index_space_1d_v1"]
pub enum WorkgroupIndex1D {}

/// A local memory authority bound to one kernel's nominal identity.
///
/// The physical kernel entry never accepts this type. Its private body receives
/// it only after the macro-generated entry binds an established two-word
/// source argument. `Space`, `Role`, `Brand`, and `'kernel` remain invariant
/// compile-time facts. Private fields and the compiler-only binding functions
/// prevent construction by ordinary safe source.
#[must_use = "a memory view carries kernel-scoped access authority"]
#[repr(transparent)]
#[rustc_diagnostic_item = "fe2o3_device_capability_memory_view_v1"]
pub struct CapabilityMemoryView<'kernel, T: 'kernel, Space, Role, Brand>
where
    Space: AddressSpace,
    Role: MemoryRole,
{
    physical: Role::Physical<'kernel, T>,
    _lifetime: PhantomData<InvariantLifetime<'kernel>>,
    _element: PhantomData<InvariantType<T>>,
    _space: PhantomData<Space>,
    _role: PhantomData<Role>,
    _brand: PhantomData<InvariantType<Brand>>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'kernel, T: 'kernel, Space, Role, Brand> sealed::View
    for CapabilityMemoryView<'kernel, T, Space, Role, Brand>
where
    Space: AddressSpace,
    Role: MemoryRole,
{
}

/// Sealed identity used by generated code to reject source lookalikes.
///
/// # Safety
///
/// The implementation must remain the exact compiler-recognized view ADT.
#[doc(hidden)]
pub unsafe trait CapabilityMemoryViewTypeV1: sealed::View {
    type Element;
    type Space: AddressSpace;
    type Role: MemoryRole;
    type Brand;
}

// SAFETY: this is the sole compiler-recognized memory-view definition.
unsafe impl<'kernel, T: 'kernel, Space, Role, Brand> CapabilityMemoryViewTypeV1
    for CapabilityMemoryView<'kernel, T, Space, Role, Brand>
where
    Space: AddressSpace,
    Role: MemoryRole,
{
    type Element = T;
    type Space = Space;
    type Role = Role;
    type Brand = Brand;
}

/// A kernel-branded view of global device memory.
pub type Global<'kernel, T, Role, Brand = UnbrandedCapability> =
    CapabilityMemoryView<'kernel, T, GlobalAddressSpace, Role, Brand>;

/// An invocation-private view carrying one exact kernel brand.
pub type PrivateMemoryView<'memory, T, Role, KernelBrand> =
    CapabilityMemoryView<'memory, T, PrivateAddressSpace, Role, KernelBrand>;

/// A workgroup-memory view carrying exact workgroup and epoch identity.
pub type WorkgroupMemoryView<'memory, 'workgroup, T, Role, KernelBrand, Epoch> =
    CapabilityMemoryView<
        'memory,
        T,
        WorkgroupAddressSpace,
        Role,
        WorkgroupMemoryBrand<'workgroup, KernelBrand, Epoch>,
    >;

/// Migration spelling for [`PrivateMemoryView`].
///
/// The alias keeps the exact role and kernel brand; it grants no default or
/// weaker capability.
#[deprecated(note = "use PrivateMemoryView")]
pub type Private<'memory, T, Role, KernelBrand> = PrivateMemoryView<'memory, T, Role, KernelBrand>;

/// Initial-epoch migration spelling for [`WorkgroupMemoryView`].
///
/// Existing four-parameter source remains initial-epoch only. Code crossing a
/// synchronization boundary must name `WorkgroupMemoryView` and its new epoch.
#[deprecated(note = "use WorkgroupMemoryView with an explicit epoch")]
pub type Workgroup<'workgroup, T, Role, KernelBrand> =
    WorkgroupMemoryView<'workgroup, 'workgroup, T, Role, KernelBrand, InitialEpoch>;

pub(crate) fn fields<'kernel, T: 'kernel, Space, Role, Brand>(
    physical: Role::Physical<'kernel, T>,
) -> CapabilityMemoryView<'kernel, T, Space, Role, Brand>
where
    Space: AddressSpace,
    Role: MemoryRole,
{
    CapabilityMemoryView {
        physical,
        _lifetime: PhantomData,
        _element: PhantomData,
        _space: PhantomData,
        _role: PhantomData,
        _brand: PhantomData,
        _not_send_sync: PhantomData,
    }
}

fn assert_exact_raw_obligation<Space: AddressSpace, Role: MemoryRole>(
    obligation: UnsafeRawMemoryObligationV1,
) {
    assert!(
        obligation == UnsafeRawMemoryObligationV1::for_view::<Space, Role>(),
        "raw-memory obligation does not match the requested view"
    );
}

impl<'memory, 'kernel, T, Role, Kernel, Target, Launch>
    CapabilityMemoryView<
        'memory,
        T,
        PrivateAddressSpace,
        Role,
        KernelCapabilityBrand<'kernel, Kernel, Target, Launch>,
    >
where
    'kernel: 'memory,
    T: CapabilityMemoryElementV1 + 'memory,
    Role: RawMemoryRoleV1,
    Target: KernelTarget,
    Launch: KernelLaunch,
{
    /// Exact record required by this private raw-memory view type.
    pub const UNSAFE_RAW_OBLIGATION_V1: UnsafeRawMemoryObligationV1 =
        UnsafeRawMemoryObligationV1::for_view::<PrivateAddressSpace, Role>();

    /// Constructs one private-memory view at the explicit unsafe boundary.
    ///
    /// # Safety
    ///
    /// The caller must establish every fact in `UNSAFE_RAW_OBLIGATION_V1`:
    /// `pointer..pointer + len` belongs to this invocation's private address
    /// space and one live allocation; the extent arithmetic, alignment, element
    /// layout, lifetime, initialization, access, and alias contract are valid;
    /// and no conflicting invocation or device access races with this view.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_private_memory_from_raw_parts_v1"]
    pub unsafe fn from_raw_parts(
        context: &'memory KernelContext<'kernel, Kernel, Target, Launch>,
        pointer: *mut T,
        len: usize,
        obligation: UnsafeRawMemoryObligationV1,
    ) -> Self {
        let _ = context;
        assert_exact_raw_obligation::<PrivateAddressSpace, Role>(obligation);
        // SAFETY: the caller accepts the complete, exact record above.
        fields(unsafe { Role::__from_raw_parts(pointer, len) })
    }
}

impl<'memory, 'workgroup, T, Role, KernelBrand, Epoch>
    CapabilityMemoryView<
        'memory,
        T,
        WorkgroupAddressSpace,
        Role,
        WorkgroupMemoryBrand<'workgroup, KernelBrand, Epoch>,
    >
where
    'workgroup: 'memory,
    T: CapabilityMemoryElementV1 + 'memory,
    Role: RawMemoryRoleV1,
    Epoch: SynchronizationEpoch,
{
    /// Exact record required by this workgroup raw-memory view type.
    pub const UNSAFE_RAW_OBLIGATION_V1: UnsafeRawMemoryObligationV1 =
        UnsafeRawMemoryObligationV1::for_view::<WorkgroupAddressSpace, Role>();

    /// Constructs one workgroup-memory view at the explicit unsafe boundary.
    ///
    /// # Safety
    ///
    /// The caller must establish every fact in `UNSAFE_RAW_OBLIGATION_V1`:
    /// `pointer..pointer + len` belongs to the supplied workgroup and epoch in
    /// workgroup address space and one live allocation; extent arithmetic,
    /// alignment, element layout, lifetime, initialization, access, and alias
    /// requirements hold; and all cross-invocation accesses are race-free.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_workgroup_memory_from_raw_parts_v1"]
    pub unsafe fn from_raw_parts(
        workgroup: &'memory WorkgroupCapability<'workgroup, KernelBrand, Epoch>,
        pointer: *mut T,
        len: usize,
        obligation: UnsafeRawMemoryObligationV1,
    ) -> Self {
        let _ = workgroup;
        assert_exact_raw_obligation::<WorkgroupAddressSpace, Role>(obligation);
        // SAFETY: the caller accepts the complete, exact record above.
        fields(unsafe { Role::__from_raw_parts(pointer, len) })
    }
}

impl<'kernel, Kernel, Target, Launch> KernelContext<'kernel, Kernel, Target, Launch>
where
    Target: KernelTarget,
    Launch: KernelLaunch,
{
    /// Allocates invocation-private read-write memory with a static extent.
    ///
    /// This is a logical source operation and contributes no kernel argument.
    /// Production import must authenticate and lower the terminal to private
    /// storage before the fallback body can be reached.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_private_memory_allocate_v1"]
    pub fn private_memory<T: CapabilityMemoryElementV1, const ELEMENTS: usize>(
        &self,
    ) -> PrivateMemoryView<
        'kernel,
        T,
        ExclusiveReadWrite,
        KernelCapabilityBrand<'kernel, Kernel, Target, Launch>,
    > {
        const {
            assert!(ELEMENTS > 0, "private memory cannot be empty");
        }
        unreachable!("private-memory allocation requires authenticated lowering")
    }
}

/// Result of publishing disjoint workgroup writes into the next epoch.
pub type PublishedWorkgroupMemoryTransition<'workgroup, T, KernelBrand, Epoch> = (
    WorkgroupCapability<'workgroup, KernelBrand, NextEpoch<Epoch>>,
    WorkgroupMemoryView<'workgroup, 'workgroup, T, ReadOnly, KernelBrand, NextEpoch<Epoch>>,
);

impl<'workgroup, KernelBrand, Epoch> WorkgroupCapability<'workgroup, KernelBrand, Epoch>
where
    Epoch: SynchronizationEpoch,
{
    /// Returns this invocation's disjoint index for workgroup-local memory.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_workgroup_memory_index_1d_v1"]
    pub fn memory_index_1d(
        &self,
    ) -> Option<ThreadIndex<WorkgroupIndex1D, WorkgroupMemoryBrand<'workgroup, KernelBrand, Epoch>>>
    {
        let rank = usize::try_from(self.invocation_rank()).ok()?;
        Some(ThreadIndex::from_capability_index(rank))
    }

    /// Allocates workgroup memory for one disjoint write per invocation.
    ///
    /// The view remains write-only until `publish_memory` consumes it and an
    /// exact workgroup synchronization transition advances the epoch.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_workgroup_memory_allocate_v1"]
    pub fn allocate_memory<T: CapabilityMemoryElementV1, const ELEMENTS: usize>(
        &self,
    ) -> WorkgroupMemoryView<
        'workgroup,
        'workgroup,
        T,
        DisjointWrite<WorkgroupIndex1D>,
        KernelBrand,
        Epoch,
    > {
        const {
            assert!(ELEMENTS > 0, "workgroup memory cannot be empty");
        }
        unreachable!("workgroup-memory allocation requires authenticated lowering")
    }

    /// Publishes complete disjoint writes and advances the workgroup epoch.
    ///
    /// Production convergence, initialization, and barrier-order analysis must
    /// discharge this terminal before lowering.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_workgroup_memory_publish_v1"]
    pub fn publish_memory<T: CapabilityMemoryElementV1>(
        self,
        memory: WorkgroupMemoryView<
            'workgroup,
            'workgroup,
            T,
            DisjointWrite<WorkgroupIndex1D>,
            KernelBrand,
            Epoch,
        >,
    ) -> PublishedWorkgroupMemoryTransition<'workgroup, T, KernelBrand, Epoch> {
        let _ = memory;
        unreachable!("workgroup-memory publication requires authenticated lowering")
    }
}

impl<'kernel, T: 'kernel, Space, Brand> CapabilityMemoryView<'kernel, T, Space, ReadOnly, Brand>
where
    Space: AddressSpace,
{
    /// Number of elements in the view.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.physical.len()
    }

    /// Whether the view contains no elements.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.physical.is_empty()
    }
}

impl<'kernel, T, Kernel, Target, Launch>
    CapabilityMemoryView<
        'kernel,
        T,
        GlobalAddressSpace,
        ReadOnly,
        KernelCapabilityBrand<'kernel, Kernel, Target, Launch>,
    >
where
    T: CapabilityMemoryElementV1 + 'kernel,
    Target: KernelTarget,
    Launch: KernelLaunch,
{
    /// Binds one physical shared slice to the issuing kernel context.
    ///
    /// Safe source cannot call this without a genuine compiler-issued context.
    /// The generated root is the only place where both the physical argument
    /// and its matching nominal context are simultaneously available.
    #[doc(hidden)]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_capability_global_bind_read_only_v1"]
    pub fn __compiler_bind_read_only(
        context: &KernelContext<'kernel, Kernel, Target, Launch>,
        physical: &'kernel [T],
    ) -> Self {
        let _ = context;
        fields(physical)
    }
}

impl<'kernel, T: CapabilityMemoryElementV1, Brand>
    CapabilityMemoryView<'kernel, T, GlobalAddressSpace, ReadOnly, Brand>
{
    /// Performs one bounds-checked volatile load.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_capability_global_load_v1"]
    pub fn load(&self, index: usize) -> Option<T> {
        (index < self.physical.len()).then(|| memory::volatile_load(self.physical, index))
    }
}

impl<'kernel, T, Kernel, Target, Launch>
    CapabilityMemoryView<
        'kernel,
        T,
        GlobalAddressSpace,
        ExclusiveReadWrite,
        KernelCapabilityBrand<'kernel, Kernel, Target, Launch>,
    >
where
    T: CapabilityMemoryElementV1 + 'kernel,
    Target: KernelTarget,
    Launch: KernelLaunch,
{
    /// Binds one exclusively owned physical allocation to this kernel root.
    ///
    /// The mutable slice preserves allocation provenance and exclusive alias
    /// identity in the physical entry ABI. Only generated code that also owns
    /// the genuine matching context can call this safe binding operation.
    #[doc(hidden)]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_capability_global_bind_exclusive_read_write_v1"]
    pub fn __compiler_bind_exclusive_read_write(
        context: &KernelContext<'kernel, Kernel, Target, Launch>,
        physical: &'kernel mut [T],
    ) -> Self {
        let _ = context;
        fields(physical)
    }
}

impl<'kernel, T: CapabilityMemoryElementV1, Brand>
    CapabilityMemoryView<'kernel, T, GlobalAddressSpace, ExclusiveReadWrite, Brand>
{
    /// Number of elements in this exact exclusive allocation view.
    pub fn len(&self) -> usize {
        self.physical.len()
    }

    /// Whether this exact exclusive allocation view contains no elements.
    pub fn is_empty(&self) -> bool {
        self.physical.is_empty()
    }

    /// Performs one bounds-checked volatile load without changing its role.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_capability_global_exclusive_load_v1"]
    pub fn load(&self, index: usize) -> Option<T> {
        (index < self.physical.len()).then(|| memory::volatile_load(self.physical, index))
    }

    /// Performs one bounds-checked volatile store without changing its role.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_capability_global_exclusive_store_v1"]
    pub fn store(&mut self, index: usize, value: T) -> bool {
        if index >= self.physical.len() {
            return false;
        }
        // SAFETY: the checked index belongs to this live exclusive slice.
        unsafe { core::ptr::write_volatile(self.physical.as_mut_ptr().add(index), value) };
        true
    }
}

impl<'memory, T: CapabilityMemoryElementV1, Brand>
    CapabilityMemoryView<'memory, T, PrivateAddressSpace, ReadOnly, Brand>
{
    /// Performs one bounds-checked private-memory load.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_private_memory_load_v1"]
    pub fn load(&self, index: usize) -> Option<T> {
        (index < self.physical.len()).then(|| memory::volatile_load(self.physical, index))
    }
}

impl<'memory, T: CapabilityMemoryElementV1, Brand>
    CapabilityMemoryView<'memory, T, PrivateAddressSpace, ExclusiveReadWrite, Brand>
{
    /// Number of elements in this exclusive private view.
    pub fn len(&self) -> usize {
        self.physical.len()
    }

    /// Whether this exclusive private view has no elements.
    pub fn is_empty(&self) -> bool {
        self.physical.is_empty()
    }

    /// Performs one bounds-checked private-memory load.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_private_memory_exclusive_load_v1"]
    pub fn load(&self, index: usize) -> Option<T> {
        (index < self.physical.len()).then(|| memory::volatile_load(self.physical, index))
    }

    /// Performs one bounds-checked private-memory store.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_private_memory_exclusive_store_v1"]
    pub fn store(&mut self, index: usize, value: T) -> bool {
        if index >= self.physical.len() {
            return false;
        }
        // SAFETY: the checked index is inside the exclusive, live slice.
        unsafe { core::ptr::write_volatile(self.physical.as_mut_ptr().add(index), value) };
        true
    }
}

impl<'memory, T, IndexSpace, Brand>
    CapabilityMemoryView<'memory, T, PrivateAddressSpace, DisjointWrite<IndexSpace>, Brand>
where
    T: CapabilityMemoryElementV1,
{
    /// Performs one bounds-checked private-memory disjoint store.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_private_memory_disjoint_store_v1"]
    pub fn store(&mut self, index: DisjointIndex<IndexSpace, Brand>, value: T) -> bool {
        self.physical.write_disjoint_branded(index, value)
    }
}

impl<'memory, 'workgroup, T, KernelBrand, Epoch>
    CapabilityMemoryView<
        'memory,
        T,
        WorkgroupAddressSpace,
        ReadOnly,
        WorkgroupMemoryBrand<'workgroup, KernelBrand, Epoch>,
    >
where
    T: CapabilityMemoryElementV1,
    Epoch: SynchronizationEpoch,
{
    /// Performs one bounds-checked workgroup-memory load in the exact epoch.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_workgroup_memory_load_v1"]
    pub fn load(
        &self,
        workgroup: &WorkgroupCapability<'workgroup, KernelBrand, Epoch>,
        index: usize,
    ) -> Option<T> {
        let _ = workgroup;
        (index < self.physical.len()).then(|| memory::volatile_load(self.physical, index))
    }
}

impl<'memory, 'workgroup, T, KernelBrand, Epoch>
    CapabilityMemoryView<
        'memory,
        T,
        WorkgroupAddressSpace,
        ExclusiveReadWrite,
        WorkgroupMemoryBrand<'workgroup, KernelBrand, Epoch>,
    >
where
    T: CapabilityMemoryElementV1,
    Epoch: SynchronizationEpoch,
{
    /// Number of elements in this exclusive workgroup view.
    pub fn len(&self) -> usize {
        self.physical.len()
    }

    /// Whether this exclusive workgroup view has no elements.
    pub fn is_empty(&self) -> bool {
        self.physical.is_empty()
    }

    /// Performs one bounds-checked workgroup load in the exact epoch.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_workgroup_memory_exclusive_load_v1"]
    pub fn load(
        &self,
        workgroup: &WorkgroupCapability<'workgroup, KernelBrand, Epoch>,
        index: usize,
    ) -> Option<T> {
        let _ = workgroup;
        (index < self.physical.len()).then(|| memory::volatile_load(self.physical, index))
    }

    /// Performs one bounds-checked workgroup store in the exact epoch.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_workgroup_memory_exclusive_store_v1"]
    pub fn store(
        &mut self,
        workgroup: &WorkgroupCapability<'workgroup, KernelBrand, Epoch>,
        index: usize,
        value: T,
    ) -> bool {
        let _ = workgroup;
        if index >= self.physical.len() {
            return false;
        }
        // SAFETY: the checked index is inside the caller-proven exclusive slice.
        unsafe { core::ptr::write_volatile(self.physical.as_mut_ptr().add(index), value) };
        true
    }
}

impl<'kernel, T: Copy + 'kernel, Space, IndexSpace, Brand>
    CapabilityMemoryView<'kernel, T, Space, DisjointWrite<IndexSpace>, Brand>
where
    Space: AddressSpace,
{
    /// Number of elements in the view.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.physical.len()
    }

    /// Whether the view contains no elements.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.physical.len() == 0
    }
}

impl<'kernel, T, IndexSpace, Kernel, Target, Launch>
    CapabilityMemoryView<
        'kernel,
        T,
        GlobalAddressSpace,
        DisjointWrite<IndexSpace>,
        KernelCapabilityBrand<'kernel, Kernel, Target, Launch>,
    >
where
    T: CapabilityMemoryElementV1 + 'kernel,
    Target: KernelTarget,
    Launch: KernelLaunch,
{
    /// Binds physical disjoint-write storage to the issuing kernel context.
    ///
    /// The context fixes the nominal kernel, target, launch, and lifetime. Safe
    /// source cannot substitute a different brand or construct the context.
    #[doc(hidden)]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_capability_global_bind_disjoint_write_v1"]
    pub fn __compiler_bind_disjoint_write(
        context: &KernelContext<'kernel, Kernel, Target, Launch>,
        physical: WriteOnlyDisjointSlice<T, IndexSpace>,
    ) -> Self {
        let _ = context;
        fields(physical)
    }
}

impl<'kernel, T: CapabilityMemoryElementV1, IndexSpace, Brand>
    CapabilityMemoryView<'kernel, T, GlobalAddressSpace, DisjointWrite<IndexSpace>, Brand>
{
    /// Performs one bounds-checked store selected by a matching disjoint witness.
    ///
    /// Both the index-space mapping and the kernel-context brand must match this
    /// view exactly.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_capability_global_store_v1"]
    pub fn store(&mut self, index: DisjointIndex<IndexSpace, Brand>, value: T) -> bool {
        self.physical.write_disjoint_branded(index, value)
    }
}

impl<
    'kernel,
    T: CapabilityMemoryElementV1,
    IndexSpace,
    const LANES_PER_BLOCK: usize,
    const ELEMENTS_PER_LANE: usize,
    Brand,
>
    CapabilityMemoryView<
        'kernel,
        T,
        GlobalAddressSpace,
        DisjointWrite<Blocked<IndexSpace, LANES_PER_BLOCK, ELEMENTS_PER_LANE>>,
        Brand,
    >
{
    /// Stores one component selected by this kernel's exact blocked witness.
    ///
    /// The witness preserves the kernel brand emitted by `checked_block`; the
    /// blocked geometry is also part of both types, so neither identity can be
    /// relabeled at the store boundary.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_capability_global_store_block_v1"]
    pub fn store_block(
        &mut self,
        block: &DisjointBlock<IndexSpace, LANES_PER_BLOCK, ELEMENTS_PER_LANE, Brand>,
        component: usize,
        value: T,
    ) -> bool {
        self.physical.write_block(block, component, value)
    }
}

impl<'memory, 'workgroup, T, IndexSpace, KernelBrand, Epoch>
    CapabilityMemoryView<
        'memory,
        T,
        WorkgroupAddressSpace,
        DisjointWrite<IndexSpace>,
        WorkgroupMemoryBrand<'workgroup, KernelBrand, Epoch>,
    >
where
    T: CapabilityMemoryElementV1,
    Epoch: SynchronizationEpoch,
{
    /// Performs one workgroup-memory store under a matching disjoint witness.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_workgroup_memory_disjoint_store_v1"]
    pub fn store(
        &mut self,
        workgroup: &WorkgroupCapability<'workgroup, KernelBrand, Epoch>,
        index: DisjointIndex<IndexSpace, WorkgroupMemoryBrand<'workgroup, KernelBrand, Epoch>>,
        value: T,
    ) -> bool {
        let _ = workgroup;
        self.physical.write_disjoint_branded(index, value)
    }
}

impl<'kernel, T, Scope, Kernel, Target, Launch>
    CapabilityMemoryView<
        'kernel,
        T,
        GlobalAddressSpace,
        AtomicReadWrite<Scope>,
        KernelCapabilityBrand<'kernel, Kernel, Target, Launch>,
    >
where
    T: CapabilityMemoryElementV1 + 'kernel,
    Scope: MemoryScope,
    Target: KernelTarget,
    Launch: KernelLaunch,
{
    /// Binds coherent global atomic storage to one kernel and static scope.
    ///
    /// Production import authenticates this exact terminal together with the
    /// physical ABI, coherent allocation, exact width, static scope, and
    /// absence of conflicting non-atomic aliases.
    #[doc(hidden)]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_capability_global_bind_atomic_v1"]
    pub fn __compiler_bind_atomic(
        context: &KernelContext<'kernel, Kernel, Target, Launch>,
        physical: &'kernel [T],
    ) -> Self {
        let _ = (context, physical);
        unreachable!("global atomic capability binding requires authenticated lowering")
    }
}

impl<'kernel, T: 'kernel, Space, Scope, Brand>
    CapabilityMemoryView<'kernel, T, Space, AtomicReadWrite<Scope>, Brand>
where
    Space: AddressSpace,
    Scope: MemoryScope,
{
    /// Number of atomic elements in the physical view.
    pub fn len(&self) -> usize {
        self.physical.len()
    }

    /// Whether the atomic view has no elements.
    pub fn is_empty(&self) -> bool {
        self.physical.is_empty()
    }
}

impl<'kernel, T: 'kernel, Space, Role, Brand> fmt::Debug
    for CapabilityMemoryView<'kernel, T, Space, Role, Brand>
where
    Space: AddressSpace,
    Role: MemoryRole,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CapabilityMemoryView")
            .finish_non_exhaustive()
    }
}

impl<'kernel, T: 'kernel, Space, Role, Brand> CapabilityMemoryView<'kernel, T, Space, Role, Brand>
where
    Space: AddressSpace,
    Role: MemoryRole,
{
    /// Returns outer representation facts for compiler ABI validation.
    #[doc(hidden)]
    pub const fn __fe2o3_rust_layout_v1() -> (usize, usize) {
        (size_of::<Self>(), align_of::<Self>())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AtomicReadWrite, DisjointWrite, ExclusiveReadWrite, Global, MemoryAccessV1,
        MemoryAddressSpaceV1, MemoryAliasV1, PrivateAddressSpace, PrivateMemoryView, ReadOnly,
        UnsafeRawMemoryObligationSetV1, UnsafeRawMemoryObligationV1, WorkgroupAddressSpace,
    };
    use crate::{DisjointIndex, Index1D, SystemScope, WriteOnlyDisjointSlice};
    use core::mem::{align_of, size_of};

    enum KernelBrand {}

    #[test]
    fn representations_match_the_existing_physical_arguments() {
        assert_eq!(
            Global::<'static, u32, ReadOnly, KernelBrand>::__fe2o3_rust_layout_v1(),
            (size_of::<&[u32]>(), align_of::<&[u32]>())
        );
        assert_eq!(
            Global::<'static, u32, DisjointWrite<Index1D>, KernelBrand>::__fe2o3_rust_layout_v1(),
            (
                size_of::<WriteOnlyDisjointSlice<u32>>(),
                align_of::<WriteOnlyDisjointSlice<u32>>(),
            )
        );
        assert_eq!(
            Global::<'static, u32, AtomicReadWrite<SystemScope>, KernelBrand>::__fe2o3_rust_layout_v1(),
            (size_of::<&[u32]>(), align_of::<&[u32]>())
        );
        assert_eq!(
            Global::<'static, u32, ExclusiveReadWrite, KernelBrand>::__fe2o3_rust_layout_v1(),
            (size_of::<&mut [u32]>(), align_of::<&mut [u32]>())
        );
        assert_eq!(
            PrivateMemoryView::<
                'static,
                u32,
                ExclusiveReadWrite,
                KernelBrand,
            >::__fe2o3_rust_layout_v1(),
            (size_of::<&mut [u32]>(), align_of::<&mut [u32]>())
        );
    }

    #[test]
    fn read_only_load_is_bounded_and_value_based() {
        let values = [3_u32, 5, 8];
        let view: Global<'_, u32, ReadOnly, KernelBrand> =
            super::fields::<_, super::GlobalAddressSpace, _, _>(&values[..]);
        assert_eq!(view.load(1), Some(5));
        assert_eq!(view.load(3), None);
    }

    #[test]
    fn disjoint_store_consumes_a_matching_witness() {
        let mut values = [0_u32; 2];
        // SAFETY: this test exclusively owns the writable storage.
        let physical =
            unsafe { WriteOnlyDisjointSlice::from_raw_parts(values.as_mut_ptr(), values.len()) };
        let mut view: Global<'_, u32, DisjointWrite<Index1D>, KernelBrand> =
            super::fields::<_, super::GlobalAddressSpace, _, _>(physical);
        assert!(view.store(DisjointIndex::from_model_index(1), 13));
        assert!(!view.store(DisjointIndex::from_model_index(2), 21));
        assert_eq!(values, [0, 13]);
    }

    #[test]
    fn private_exclusive_view_has_checked_host_semantics() {
        let mut values = [2_u32, 3];
        let mut view: PrivateMemoryView<'_, u32, ExclusiveReadWrite, KernelBrand> =
            super::fields::<_, PrivateAddressSpace, _, _>(&mut values[..]);
        assert_eq!(view.load(1), Some(3));
        assert!(view.store(1, 5));
        assert!(!view.store(2, 8));
        assert_eq!(values, [2, 5]);
    }

    #[test]
    fn global_exclusive_view_has_checked_read_write_semantics() {
        let mut values = [2_u32, 3];
        let mut view: Global<'_, u32, ExclusiveReadWrite, KernelBrand> =
            super::fields::<_, super::GlobalAddressSpace, _, _>(&mut values[..]);
        assert_eq!(view.load(1), Some(3));
        assert!(view.store(1, 5));
        assert!(!view.store(2, 8));
        assert_eq!(values, [2, 5]);
    }

    #[test]
    fn raw_obligation_identity_is_exact_and_role_relative() {
        let private =
            UnsafeRawMemoryObligationV1::for_view::<PrivateAddressSpace, ExclusiveReadWrite>();
        assert_eq!(private.contract_version(), 1);
        assert_eq!(private.address_space(), MemoryAddressSpaceV1::Private);
        assert_eq!(private.access(), MemoryAccessV1::ReadWrite);
        assert_eq!(private.aliasing(), MemoryAliasV1::Exclusive);
        assert!(
            private
                .required()
                .contains(UnsafeRawMemoryObligationSetV1::INITIALIZED_READS)
        );

        let write_only =
            UnsafeRawMemoryObligationV1::for_view::<WorkgroupAddressSpace, DisjointWrite<Index1D>>(
            );
        assert_eq!(write_only.address_space(), MemoryAddressSpaceV1::Workgroup);
        assert_eq!(write_only.access(), MemoryAccessV1::WriteOnly);
        assert_eq!(write_only.aliasing(), MemoryAliasV1::Disjoint);
        assert!(
            !write_only
                .required()
                .contains(UnsafeRawMemoryObligationSetV1::INITIALIZED_READS)
        );
    }

    #[test]
    fn raw_obligation_substitution_fails_closed() {
        let global =
            UnsafeRawMemoryObligationV1::for_view::<super::GlobalAddressSpace, ExclusiveReadWrite>(
            );
        assert!(
            std::panic::catch_unwind(|| {
                super::assert_exact_raw_obligation::<PrivateAddressSpace, ExclusiveReadWrite>(
                    global,
                );
            })
            .is_err()
        );
    }
}
