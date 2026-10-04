//! Bounded, specification-only memory contracts for independent GPU threads.
//!
//! These records are executable proof inputs. They neither authenticate their
//! symbolic identities nor grant runtime access, loading, or launch authority.

use core::fmt;

/// Largest allocation extent modeled by the v1 proof vocabulary.
pub const MAX_ALLOCATION_BYTES_V1: u64 = 1_u64 << 48;

/// Largest one-dimensional launch modeled by the v1 proof vocabulary.
pub const MAX_LAUNCH_THREADS_V1: u64 = u32::MAX as u64;

/// Largest number of read bindings accepted by one independent-thread check.
pub const MAX_READ_BINDINGS_V1: usize = 16;

mod sealed {
    pub trait Sealed {}
}

/// Marker implemented only by specification facts from this module.
///
/// The sealed marker exists to make the trust-domain distinction explicit. It
/// is not a runtime authorization trait and has no conversion to one.
pub trait SpecificationFactV1: sealed::Sealed {}

/// Symbolic allocation provenance used by a proof environment.
///
/// Safe construction is intentional: this is a name in a specification, not
/// evidence that any runtime allocation exists.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct AllocationProvenanceIdV1(u32);

impl AllocationProvenanceIdV1 {
    pub const fn new(value: u32) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

impl sealed::Sealed for AllocationProvenanceIdV1 {}
impl SpecificationFactV1 for AllocationProvenanceIdV1 {}

/// Symbolic address-space identity used by a proof environment.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct AddressSpaceIdV1(u16);

impl AddressSpaceIdV1 {
    pub const fn new(value: u16) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

impl sealed::Sealed for AddressSpaceIdV1 {}
impl SpecificationFactV1 for AddressSpaceIdV1 {}

/// Bounded symbolic allocation metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AllocationSpecV1 {
    provenance: AllocationProvenanceIdV1,
    address_space: AddressSpaceIdV1,
    base_address: u64,
    byte_length: u64,
    address_space_size: u64,
}

impl AllocationSpecV1 {
    pub const fn new(
        provenance: AllocationProvenanceIdV1,
        address_space: AddressSpaceIdV1,
        base_address: u64,
        byte_length: u64,
        address_space_size: u64,
    ) -> Result<Self, ObligationFailureV1> {
        let allocation = Self {
            provenance,
            address_space,
            base_address,
            byte_length,
            address_space_size,
        };
        match allocation.representation_failure() {
            Some(failure) => Err(failure),
            None => Ok(allocation),
        }
    }

    pub const fn provenance(self) -> AllocationProvenanceIdV1 {
        self.provenance
    }

    pub const fn address_space(self) -> AddressSpaceIdV1 {
        self.address_space
    }

    pub const fn base_address(self) -> u64 {
        self.base_address
    }

    pub const fn byte_length(self) -> u64 {
        self.byte_length
    }

    pub const fn address_space_size(self) -> u64 {
        self.address_space_size
    }

    pub const fn end_address(self) -> Option<u64> {
        self.base_address.checked_add(self.byte_length)
    }

    pub const fn contains(self, region: ByteRegionV1) -> bool {
        if self.provenance.0 != region.provenance.0
            || self.address_space.0 != region.address_space.0
            || region.byte_length == 0
        {
            return false;
        }
        match region.end_offset() {
            Some(end) => end <= self.byte_length,
            None => false,
        }
    }

    const fn representation_failure(self) -> Option<ObligationFailureV1> {
        if self.byte_length == 0 {
            return Some(ObligationFailureV1::EmptyAllocation);
        }
        if self.byte_length > MAX_ALLOCATION_BYTES_V1 {
            return Some(ObligationFailureV1::AllocationBoundExceeded {
                actual: self.byte_length,
                maximum: MAX_ALLOCATION_BYTES_V1,
            });
        }
        if self.address_space_size == 0 {
            return Some(ObligationFailureV1::EmptyAddressSpace);
        }
        match self.end_address() {
            Some(end) if end <= self.address_space_size => None,
            Some(end) => Some(ObligationFailureV1::AllocationOutsideAddressSpace {
                end,
                address_space_size: self.address_space_size,
            }),
            None => Some(ObligationFailureV1::ArithmeticOverflow),
        }
    }
}

impl sealed::Sealed for AllocationSpecV1 {}
impl SpecificationFactV1 for AllocationSpecV1 {}

/// A nonempty half-open byte range retaining symbolic allocation provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ByteRegionV1 {
    provenance: AllocationProvenanceIdV1,
    address_space: AddressSpaceIdV1,
    byte_offset: u64,
    byte_length: u64,
}

impl ByteRegionV1 {
    pub const fn new(
        provenance: AllocationProvenanceIdV1,
        address_space: AddressSpaceIdV1,
        byte_offset: u64,
        byte_length: u64,
    ) -> Result<Self, ObligationFailureV1> {
        if byte_length == 0 {
            return Err(ObligationFailureV1::EmptyRegion);
        }
        if byte_length > MAX_ALLOCATION_BYTES_V1 {
            return Err(ObligationFailureV1::RegionBoundExceeded {
                actual: byte_length,
                maximum: MAX_ALLOCATION_BYTES_V1,
            });
        }
        if byte_offset.checked_add(byte_length).is_none() {
            return Err(ObligationFailureV1::ArithmeticOverflow);
        }
        Ok(Self {
            provenance,
            address_space,
            byte_offset,
            byte_length,
        })
    }

    pub const fn for_allocation(
        allocation: AllocationSpecV1,
        byte_offset: u64,
        byte_length: u64,
    ) -> Result<Self, ObligationFailureV1> {
        let region = match Self::new(
            allocation.provenance,
            allocation.address_space,
            byte_offset,
            byte_length,
        ) {
            Ok(region) => region,
            Err(failure) => return Err(failure),
        };
        if allocation.contains(region) {
            Ok(region)
        } else {
            Err(ObligationFailureV1::RegionOutsideAllocation)
        }
    }

    pub const fn provenance(self) -> AllocationProvenanceIdV1 {
        self.provenance
    }

    pub const fn address_space(self) -> AddressSpaceIdV1 {
        self.address_space
    }

    pub const fn byte_offset(self) -> u64 {
        self.byte_offset
    }

    pub const fn byte_length(self) -> u64 {
        self.byte_length
    }

    pub const fn end_offset(self) -> Option<u64> {
        self.byte_offset.checked_add(self.byte_length)
    }

    pub const fn overlaps(self, other: Self) -> bool {
        if self.provenance.0 != other.provenance.0 || self.address_space.0 != other.address_space.0
        {
            return false;
        }
        let Some(self_end) = self.end_offset() else {
            return true;
        };
        let Some(other_end) = other.end_offset() else {
            return true;
        };
        self.byte_offset < other_end && other.byte_offset < self_end
    }
}

impl sealed::Sealed for ByteRegionV1 {}
impl SpecificationFactV1 for ByteRegionV1 {}

/// Permission modeled for a byte region.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionKindV1 {
    SharedRead,
    ExclusiveWrite,
}

/// Whether bytes are known initialized in the source-level proof model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitializationStateV1 {
    Uninitialized,
    Initialized,
}

/// Access whose proof preconditions are being checked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessKindV1 {
    Read,
    Write,
}

/// A specification permission. This is not a runtime capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegionPermissionV1 {
    kind: PermissionKindV1,
    region: ByteRegionV1,
}

impl RegionPermissionV1 {
    pub const fn shared_read(region: ByteRegionV1) -> Self {
        Self {
            kind: PermissionKindV1::SharedRead,
            region,
        }
    }

    pub const fn exclusive_write(region: ByteRegionV1) -> Self {
        Self {
            kind: PermissionKindV1::ExclusiveWrite,
            region,
        }
    }

    pub const fn kind(self) -> PermissionKindV1 {
        self.kind
    }

    pub const fn region(self) -> ByteRegionV1 {
        self.region
    }

    pub const fn compatible_with(self, other: Self) -> bool {
        !self.region.overlaps(other.region)
            || (matches!(self.kind, PermissionKindV1::SharedRead)
                && matches!(other.kind, PermissionKindV1::SharedRead))
    }
}

impl sealed::Sealed for RegionPermissionV1 {}
impl SpecificationFactV1 for RegionPermissionV1 {}

/// A specification permission paired with modeled initialization state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegionCapabilityV1 {
    permission: RegionPermissionV1,
    initialization: InitializationStateV1,
}

impl RegionCapabilityV1 {
    pub const fn new(
        permission: RegionPermissionV1,
        initialization: InitializationStateV1,
    ) -> Self {
        Self {
            permission,
            initialization,
        }
    }

    pub const fn initialized_read(region: ByteRegionV1) -> Self {
        Self::new(
            RegionPermissionV1::shared_read(region),
            InitializationStateV1::Initialized,
        )
    }

    pub const fn writable(region: ByteRegionV1, initialization: InitializationStateV1) -> Self {
        Self::new(RegionPermissionV1::exclusive_write(region), initialization)
    }

    pub const fn permission(self) -> RegionPermissionV1 {
        self.permission
    }

    pub const fn initialization(self) -> InitializationStateV1 {
        self.initialization
    }

    pub const fn permits(self, access: AccessKindV1) -> bool {
        match access {
            AccessKindV1::Read => {
                matches!(self.permission.kind, PermissionKindV1::SharedRead)
                    && matches!(self.initialization, InitializationStateV1::Initialized)
            }
            AccessKindV1::Write => {
                matches!(self.permission.kind, PermissionKindV1::ExclusiveWrite)
            }
        }
    }

    pub const fn state_after(self, access: AccessKindV1) -> Option<InitializationStateV1> {
        if !self.permits(access) {
            return None;
        }
        match access {
            AccessKindV1::Read => Some(self.initialization),
            AccessKindV1::Write => Some(InitializationStateV1::Initialized),
        }
    }
}

impl sealed::Sealed for RegionCapabilityV1 {}
impl SpecificationFactV1 for RegionCapabilityV1 {}

/// An allocation and capability presented together for one obligation check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegionBindingV1 {
    allocation: AllocationSpecV1,
    capability: RegionCapabilityV1,
}

impl RegionBindingV1 {
    pub const fn new(allocation: AllocationSpecV1, capability: RegionCapabilityV1) -> Self {
        Self {
            allocation,
            capability,
        }
    }

    pub const fn allocation(self) -> AllocationSpecV1 {
        self.allocation
    }

    pub const fn capability(self) -> RegionCapabilityV1 {
        self.capability
    }
}

impl sealed::Sealed for RegionBindingV1 {}
impl SpecificationFactV1 for RegionBindingV1 {}

/// Symbolic identity for one source-level launch domain.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct LaunchIdentityV1(u64);

impl LaunchIdentityV1 {
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl sealed::Sealed for LaunchIdentityV1 {}
impl SpecificationFactV1 for LaunchIdentityV1 {}

/// Bounded one-dimensional launch domain branded by symbolic identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrandedLaunchDomain1dV1 {
    identity: LaunchIdentityV1,
    thread_count: u64,
}

impl BrandedLaunchDomain1dV1 {
    pub const fn new(
        identity: LaunchIdentityV1,
        thread_count: u64,
    ) -> Result<Self, ObligationFailureV1> {
        if thread_count > MAX_LAUNCH_THREADS_V1 {
            return Err(ObligationFailureV1::LaunchBoundExceeded {
                actual: thread_count,
                maximum: MAX_LAUNCH_THREADS_V1,
            });
        }
        Ok(Self {
            identity,
            thread_count,
        })
    }

    pub const fn identity(self) -> LaunchIdentityV1 {
        self.identity
    }

    pub const fn thread_count(self) -> u64 {
        self.thread_count
    }

    pub const fn is_empty(self) -> bool {
        self.thread_count == 0
    }

    pub const fn thread(self, linear: u64) -> Option<BrandedThreadId1dV1> {
        if linear < self.thread_count {
            Some(BrandedThreadId1dV1 {
                launch: self.identity,
                linear,
            })
        } else {
            None
        }
    }

    pub const fn contains(self, thread: BrandedThreadId1dV1) -> bool {
        thread.launch.0 == self.identity.0 && thread.linear < self.thread_count
    }
}

impl sealed::Sealed for BrandedLaunchDomain1dV1 {}
impl SpecificationFactV1 for BrandedLaunchDomain1dV1 {}

/// In-domain symbolic thread identity tied to one launch brand.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct BrandedThreadId1dV1 {
    launch: LaunchIdentityV1,
    linear: u64,
}

impl BrandedThreadId1dV1 {
    pub const fn launch(self) -> LaunchIdentityV1 {
        self.launch
    }

    pub const fn linear(self) -> u64 {
        self.linear
    }
}

impl sealed::Sealed for BrandedThreadId1dV1 {}
impl SpecificationFactV1 for BrandedThreadId1dV1 {}

/// Affine per-thread write mapping `base + thread * stride`.
///
/// A positive stride at least as large as `element_bytes` is sufficient for
/// pairwise-disjoint write regions for distinct thread IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AffineWriteMappingV1 {
    base_offset: u64,
    stride_bytes: u64,
    element_bytes: u64,
}

impl AffineWriteMappingV1 {
    pub const fn new(
        base_offset: u64,
        stride_bytes: u64,
        element_bytes: u64,
    ) -> Result<Self, ObligationFailureV1> {
        if element_bytes == 0 {
            return Err(ObligationFailureV1::EmptyRegion);
        }
        if element_bytes > MAX_ALLOCATION_BYTES_V1 {
            return Err(ObligationFailureV1::RegionBoundExceeded {
                actual: element_bytes,
                maximum: MAX_ALLOCATION_BYTES_V1,
            });
        }
        if stride_bytes < element_bytes {
            return Err(ObligationFailureV1::WriteMappingNotDisjoint {
                stride_bytes,
                element_bytes,
            });
        }
        Ok(Self {
            base_offset,
            stride_bytes,
            element_bytes,
        })
    }

    pub const fn identity(element_bytes: u64) -> Result<Self, ObligationFailureV1> {
        Self::new(0, element_bytes, element_bytes)
    }

    pub const fn base_offset(self) -> u64 {
        self.base_offset
    }

    pub const fn stride_bytes(self) -> u64 {
        self.stride_bytes
    }

    pub const fn element_bytes(self) -> u64 {
        self.element_bytes
    }

    pub const fn region_for(
        self,
        domain: BrandedLaunchDomain1dV1,
        thread: BrandedThreadId1dV1,
        allocation: AllocationSpecV1,
    ) -> Result<ByteRegionV1, ObligationFailureV1> {
        if !domain.contains(thread) {
            return Err(ObligationFailureV1::ThreadOutsideLaunchDomain);
        }
        let scaled = match thread.linear.checked_mul(self.stride_bytes) {
            Some(value) => value,
            None => return Err(ObligationFailureV1::ArithmeticOverflow),
        };
        let offset = match self.base_offset.checked_add(scaled) {
            Some(value) => value,
            None => return Err(ObligationFailureV1::ArithmeticOverflow),
        };
        ByteRegionV1::for_allocation(allocation, offset, self.element_bytes)
    }

    pub const fn fits_domain(
        self,
        domain: BrandedLaunchDomain1dV1,
        allocation: AllocationSpecV1,
    ) -> bool {
        if domain.is_empty() {
            return self.base_offset <= allocation.byte_length;
        }
        let Some(last) = domain.thread(domain.thread_count - 1) else {
            return false;
        };
        self.region_for(domain, last, allocation).is_ok()
    }

    pub const fn is_injective_for(
        self,
        domain: BrandedLaunchDomain1dV1,
        left: BrandedThreadId1dV1,
        right: BrandedThreadId1dV1,
        allocation: AllocationSpecV1,
    ) -> bool {
        if !domain.contains(left) || !domain.contains(right) {
            return false;
        }
        if left.linear == right.linear {
            return true;
        }
        let Ok(left_region) = self.region_for(domain, left, allocation) else {
            return false;
        };
        let Ok(right_region) = self.region_for(domain, right, allocation) else {
            return false;
        };
        !left_region.overlaps(right_region)
    }
}

impl sealed::Sealed for AffineWriteMappingV1 {}
impl SpecificationFactV1 for AffineWriteMappingV1 {}

/// Stable category of an executable proof obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObligationKindV1 {
    AllocationRepresentable,
    RegionInBounds,
    AccessPermitted,
    PermissionsCompatible,
    ThreadInDomain,
    WriteMappingFitsDomain,
    WriteMappingInjective,
    InitializationTransition,
}

/// Precise reason an executable proof obligation was not satisfied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ObligationFailureV1 {
    EmptyAllocation,
    EmptyAddressSpace,
    EmptyRegion,
    AllocationBoundExceeded {
        actual: u64,
        maximum: u64,
    },
    RegionBoundExceeded {
        actual: u64,
        maximum: u64,
    },
    LaunchBoundExceeded {
        actual: u64,
        maximum: u64,
    },
    ReadBindingBoundExceeded {
        actual: usize,
        maximum: usize,
    },
    AllocationOutsideAddressSpace {
        end: u64,
        address_space_size: u64,
    },
    RegionOutsideAllocation,
    ArithmeticOverflow,
    ReadRequiresSharedPermission,
    ReadRequiresInitialization,
    WriteRequiresExclusivePermission,
    PermissionsConflict,
    ThreadOutsideLaunchDomain,
    WriteMappingNotDisjoint {
        stride_bytes: u64,
        element_bytes: u64,
    },
    WriteMappingOutsideAllocation,
    WriteRegionMismatch,
    InvalidInitializationTransition,
}

impl fmt::Display for ObligationFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

/// Result of evaluating one proof obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObligationResultV1 {
    Satisfied(ObligationKindV1),
    Unsatisfied {
        kind: ObligationKindV1,
        failure: ObligationFailureV1,
    },
}

impl ObligationResultV1 {
    pub const fn is_satisfied(self) -> bool {
        matches!(self, Self::Satisfied(_))
    }
}

/// One self-contained, machine-checkable proof obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProofObligationV1 {
    AllocationRepresentable(AllocationSpecV1),
    RegionInBounds {
        allocation: AllocationSpecV1,
        region: ByteRegionV1,
    },
    AccessPermitted {
        capability: RegionCapabilityV1,
        access: AccessKindV1,
    },
    PermissionsCompatible {
        left: RegionPermissionV1,
        right: RegionPermissionV1,
    },
    ThreadInDomain {
        domain: BrandedLaunchDomain1dV1,
        thread: BrandedThreadId1dV1,
    },
    WriteMappingFitsDomain {
        domain: BrandedLaunchDomain1dV1,
        mapping: AffineWriteMappingV1,
        allocation: AllocationSpecV1,
    },
    WriteMappingInjective {
        domain: BrandedLaunchDomain1dV1,
        mapping: AffineWriteMappingV1,
        allocation: AllocationSpecV1,
        left: BrandedThreadId1dV1,
        right: BrandedThreadId1dV1,
    },
    InitializationTransition {
        capability: RegionCapabilityV1,
        access: AccessKindV1,
        after: InitializationStateV1,
    },
}

impl ProofObligationV1 {
    pub const fn kind(self) -> ObligationKindV1 {
        match self {
            Self::AllocationRepresentable(_) => ObligationKindV1::AllocationRepresentable,
            Self::RegionInBounds { .. } => ObligationKindV1::RegionInBounds,
            Self::AccessPermitted { .. } => ObligationKindV1::AccessPermitted,
            Self::PermissionsCompatible { .. } => ObligationKindV1::PermissionsCompatible,
            Self::ThreadInDomain { .. } => ObligationKindV1::ThreadInDomain,
            Self::WriteMappingFitsDomain { .. } => ObligationKindV1::WriteMappingFitsDomain,
            Self::WriteMappingInjective { .. } => ObligationKindV1::WriteMappingInjective,
            Self::InitializationTransition { .. } => ObligationKindV1::InitializationTransition,
        }
    }

    pub const fn evaluate(self) -> ObligationResultV1 {
        let failure = match self {
            Self::AllocationRepresentable(allocation) => allocation.representation_failure(),
            Self::RegionInBounds { allocation, region } => {
                if allocation.contains(region) {
                    None
                } else {
                    Some(ObligationFailureV1::RegionOutsideAllocation)
                }
            }
            Self::AccessPermitted { capability, access } => access_failure(capability, access),
            Self::PermissionsCompatible { left, right } => {
                if left.compatible_with(right) {
                    None
                } else {
                    Some(ObligationFailureV1::PermissionsConflict)
                }
            }
            Self::ThreadInDomain { domain, thread } => {
                if domain.contains(thread) {
                    None
                } else {
                    Some(ObligationFailureV1::ThreadOutsideLaunchDomain)
                }
            }
            Self::WriteMappingFitsDomain {
                domain,
                mapping,
                allocation,
            } => {
                if mapping.fits_domain(domain, allocation) {
                    None
                } else {
                    Some(ObligationFailureV1::WriteMappingOutsideAllocation)
                }
            }
            Self::WriteMappingInjective {
                domain,
                mapping,
                allocation,
                left,
                right,
            } => {
                if mapping.is_injective_for(domain, left, right, allocation) {
                    None
                } else {
                    Some(ObligationFailureV1::WriteMappingNotDisjoint {
                        stride_bytes: mapping.stride_bytes,
                        element_bytes: mapping.element_bytes,
                    })
                }
            }
            Self::InitializationTransition {
                capability,
                access,
                after,
            } => match (capability.state_after(access), after) {
                (
                    Some(InitializationStateV1::Uninitialized),
                    InitializationStateV1::Uninitialized,
                )
                | (Some(InitializationStateV1::Initialized), InitializationStateV1::Initialized) => {
                    None
                }
                _ => Some(ObligationFailureV1::InvalidInitializationTransition),
            },
        };
        match failure {
            Some(failure) => ObligationResultV1::Unsatisfied {
                kind: self.kind(),
                failure,
            },
            None => ObligationResultV1::Satisfied(self.kind()),
        }
    }
}

impl sealed::Sealed for ProofObligationV1 {}
impl SpecificationFactV1 for ProofObligationV1 {}

const fn access_failure(
    capability: RegionCapabilityV1,
    access: AccessKindV1,
) -> Option<ObligationFailureV1> {
    match access {
        AccessKindV1::Read => {
            if !matches!(capability.permission.kind, PermissionKindV1::SharedRead) {
                Some(ObligationFailureV1::ReadRequiresSharedPermission)
            } else if !matches!(
                capability.initialization,
                InitializationStateV1::Initialized
            ) {
                Some(ObligationFailureV1::ReadRequiresInitialization)
            } else {
                None
            }
        }
        AccessKindV1::Write => {
            if matches!(capability.permission.kind, PermissionKindV1::ExclusiveWrite) {
                None
            } else {
                Some(ObligationFailureV1::WriteRequiresExclusivePermission)
            }
        }
    }
}

/// Inputs for checking one independent thread's memory contract.
///
/// `READS` is checked against [`MAX_READ_BINDINGS_V1`] at evaluation time.
/// This value is specification data and cannot authorize a runtime operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndependentThreadContractV1<const READS: usize> {
    domain: BrandedLaunchDomain1dV1,
    thread: BrandedThreadId1dV1,
    reads: [RegionBindingV1; READS],
    output: RegionBindingV1,
    write_mapping: AffineWriteMappingV1,
}

impl<const READS: usize> IndependentThreadContractV1<READS> {
    pub const fn new(
        domain: BrandedLaunchDomain1dV1,
        thread: BrandedThreadId1dV1,
        reads: [RegionBindingV1; READS],
        output: RegionBindingV1,
        write_mapping: AffineWriteMappingV1,
    ) -> Self {
        Self {
            domain,
            thread,
            reads,
            output,
            write_mapping,
        }
    }

    /// Evaluates every v1 independent-thread obligation in deterministic order.
    ///
    /// Success means only that these bounded records are internally consistent.
    /// It is not Verus evidence, machine-code refinement, or runtime authority.
    pub const fn evaluate(self) -> Result<IndependentThreadFactsV1<READS>, ObligationFailureV1> {
        if READS > MAX_READ_BINDINGS_V1 {
            return Err(ObligationFailureV1::ReadBindingBoundExceeded {
                actual: READS,
                maximum: MAX_READ_BINDINGS_V1,
            });
        }
        if !self.domain.contains(self.thread) {
            return Err(ObligationFailureV1::ThreadOutsideLaunchDomain);
        }
        if !self
            .write_mapping
            .fits_domain(self.domain, self.output.allocation)
        {
            return Err(ObligationFailureV1::WriteMappingOutsideAllocation);
        }
        let expected_write =
            match self
                .write_mapping
                .region_for(self.domain, self.thread, self.output.allocation)
            {
                Ok(region) => region,
                Err(failure) => return Err(failure),
            };
        if !same_region(self.output.capability.permission.region, expected_write) {
            return Err(ObligationFailureV1::WriteRegionMismatch);
        }
        if let Some(failure) = access_failure(self.output.capability, AccessKindV1::Write) {
            return Err(failure);
        }

        let mut index = 0;
        while index < READS {
            let read = self.reads[index];
            if !read.allocation.contains(read.capability.permission.region) {
                return Err(ObligationFailureV1::RegionOutsideAllocation);
            }
            if let Some(failure) = access_failure(read.capability, AccessKindV1::Read) {
                return Err(failure);
            }
            if !read
                .capability
                .permission
                .compatible_with(self.output.capability.permission)
            {
                return Err(ObligationFailureV1::PermissionsConflict);
            }
            index += 1;
        }

        Ok(IndependentThreadFactsV1 { contract: self })
    }
}

const fn same_region(left: ByteRegionV1, right: ByteRegionV1) -> bool {
    left.provenance.0 == right.provenance.0
        && left.address_space.0 == right.address_space.0
        && left.byte_offset == right.byte_offset
        && left.byte_length == right.byte_length
}

impl<const READS: usize> sealed::Sealed for IndependentThreadContractV1<READS> {}
impl<const READS: usize> SpecificationFactV1 for IndependentThreadContractV1<READS> {}

/// Internally consistent independent-thread specification facts.
///
/// This records the result of executable checks only. It is deliberately
/// copyable and has no method that interacts with a runtime backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndependentThreadFactsV1<const READS: usize> {
    contract: IndependentThreadContractV1<READS>,
}

impl<const READS: usize> IndependentThreadFactsV1<READS> {
    pub const fn domain(self) -> BrandedLaunchDomain1dV1 {
        self.contract.domain
    }

    pub const fn thread(self) -> BrandedThreadId1dV1 {
        self.contract.thread
    }

    pub const fn reads(self) -> [RegionBindingV1; READS] {
        self.contract.reads
    }

    pub const fn output(self) -> RegionBindingV1 {
        self.contract.output
    }

    pub const fn write_mapping(self) -> AffineWriteMappingV1 {
        self.contract.write_mapping
    }

    pub const fn output_state_after_write(self) -> InitializationStateV1 {
        InitializationStateV1::Initialized
    }
}

impl<const READS: usize> sealed::Sealed for IndependentThreadFactsV1<READS> {}
impl<const READS: usize> SpecificationFactV1 for IndependentThreadFactsV1<READS> {}
