//! Stable imports for target-neutral kernel source.
//!
//! This prelude exposes capability identities and operations, not target
//! adapters or legacy independent `current()` acquisition APIs. Import gfx950
//! operations separately from [`crate::gfx950::prelude`].

pub use crate::capability_memory::{
    AddressSpace, AliasMode, AtomicAliases, AtomicReadWrite, AtomicReadWriteAccess,
    CAPABILITY_MEMORY_VIEW_CONTRACT_VERSION_V1, CapabilityMemoryElementV1, DisjointAliases,
    DisjointWrite, ExclusiveAliases, ExclusiveReadWrite, Global, GlobalAddressSpace,
    MemoryAccessV1, MemoryAddressSpaceV1, MemoryAliasV1, MemoryRole, PrivateAddressSpace,
    PrivateMemoryView, PublishedWorkgroupMemoryTransition, ReadAccess, ReadOnly, ReadWriteAccess,
    SharedAliases, UNSAFE_RAW_MEMORY_OBLIGATION_CONTRACT_VERSION_V1,
    UnsafeRawMemoryObligationSetV1, UnsafeRawMemoryObligationV1, WorkgroupAddressSpace,
    WorkgroupIndex1D, WorkgroupMemoryBrand, WorkgroupMemoryView, WriteAccess,
};
pub use crate::context::{
    CurrentTarget, KernelCapabilityBrand, KernelContext, KernelLaunch, KernelTarget,
    RegisteredLaunch,
};
pub use crate::execution::{
    Acquire, AcquireRelease, AtomicElement, AtomicFailureOrdering, AtomicLoadOrdering,
    AtomicOrderPair, AtomicRmwOrdering, AtomicStoreOrdering, CollectiveSubgroupWidth, DeviceScope,
    DynamicPhaseEpoch, EXECUTION_CAPABILITY_CONTRACT_VERSION_V1, GlobalAndWorkgroupMemory,
    GlobalMemory, InitialEpoch, MatrixSubgroupWidth, MemoryOrdering, MemoryScope, MemorySemantics,
    MemorySpaces, NextEpoch, PendingAsyncCopy, PublishedLdsPairTransition, PublishedLdsTransition,
    Relaxed, Release, ReusablePhaseCompletion, ReusableSynchronizationEpoch, ReusableWorkgroup,
    ReusableWorkgroupBrand, ReusableWorkgroupLds, ScopedAtomic, SequentiallyConsistent, Subgroup,
    SubgroupBarrierSemantics, SubgroupBarrierTransition, SubgroupBrand, SubgroupFenceSemantics,
    SubgroupScope, SynchronizationEpoch, SystemScope, ValidAtomicOrderPair, WorkgroupAtomicScope,
    WorkgroupBarrierSemantics, WorkgroupBrand, WorkgroupCapability, WorkgroupCollectiveTransition,
    WorkgroupEpoch, WorkgroupFenceSemantics, WorkgroupLds, WorkgroupLdsInvocationInitialized,
    WorkgroupLdsPublished, WorkgroupLdsUninitialized, WorkgroupMemory, WorkgroupScope,
};
pub use crate::group::{Grid, Group, Workgroup};
pub use crate::numerical::{NumericalPolicy, NumericalPolicyCapability, StrictIeee};
pub use crate::thread::{
    DisjointIndex, GlobalGridSize, GlobalWorkitemId, GridSize, Index1D, Index2D, Invocation3D,
    ThreadIndex, WorkgroupId, WorkgroupSize, WorkitemId,
};
pub use crate::wave::{SubgroupLane, SubgroupWidth, SubgroupWidth32, SubgroupWidth64};
pub use crate::{
    Bf16MfmaAFragment, Bf16MfmaBFragment, DeviceMath, F32AccumulatorFragment,
    F32AccumulatorMatrixViewError, GlobalBf16MfmaAMatrix, GlobalBf16MfmaBMatrix,
    GlobalF32AccumulatorMatrix, KernelError, KernelResult, LdsElement, MatrixCapability,
    MatrixGlobalAccess, PolicyDeviceMath, PolicyMatrixCapability, kernel,
};
