use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use crate::{
    AccessMode, AddressSpace, Axis, BinaryOp, BlockId, ByteExpression, CastKind, Constant,
    Function, FunctionId, FunctionOperationLocation, IndexKind, IntrinsicKind, InvocationRange1d,
    KernelId, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind,
    RegionValidationError, ScalarType, Type, ValueId, VerificationErrors, VerifiedKernelIrModuleV1,
    analyze_control_flow, analyze_interprocedural_effects_from_verified_v1, verify_module_ref,
};

mod actual_owner_v18;
mod affine_engine_v2;
mod candidate_pair_bound_v1;
mod complete_body_v19;
#[cfg(test)]
mod distinct_invocation_v1_tests;
mod exact_origin_v18;
mod gfx942_inline_u32_v30;
mod guarded_access_v1;
mod report_construction_v18;
pub(crate) use guarded_access_v1::origins::structural_origins_v1;
mod closed_scalar_v18;
mod ordered_composition_v1;
mod physical_entry_v20;
mod physical_global_copy_v21;
mod physical_lds_exchange_v22;
mod pointer_derivation;
mod private_slots;
mod receipt_v1;
mod scalar_cfg_v18;
mod storage_discriminant_v18;
pub use actual_owner_v18::*;
pub use candidate_pair_bound_v1::*;
pub use closed_scalar_v18::*;
pub use scalar_cfg_v18::*;

pub use complete_body_v19::derive_complete_body_memory_obligations_v19;
pub use guarded_access_v1::FormalGuardedMemoryResourceErrorV1;
pub use guarded_access_v1::{
    CanonicalGuardedGlobalReadErrorV1, CanonicalGuardedGlobalReadFactV1,
    CanonicalGuardedGlobalReadFactV18, CanonicalGuardedGlobalReadLimitsV1,
    CanonicalGuardedGlobalReadOutcomeV1, CanonicalGuardedGlobalReadOutcomeV18,
    CanonicalGuardedGlobalReadReasonV1, CanonicalGuardedNoWrapFactV1,
    CanonicalGuardedNoWrapFactV18, CanonicalGuardedPredicateFactV1,
    CanonicalGuardedPredicateFactV18, CanonicalGuardedReadIndexOriginV1,
    CheckedCanonicalGuardedGlobalReadsV1, CheckedCanonicalGuardedGlobalReadsV18,
    with_canonical_guarded_global_reads_v1, with_canonical_guarded_global_reads_v18,
};
pub use ordered_composition_v1::*;
pub use physical_entry_v20::{
    PhysicalEntryKernargAbiRequirementV20, PhysicalEntryKernargReadV20,
    PhysicalEntryKernargSlotV20, PhysicalEntryMemoryErrorV20, PhysicalEntryMemoryObligationsV20,
    PhysicalEntryMemoryStorageV20, PhysicalEntryStoreV20,
    derive_physical_entry_memory_obligations_v20,
};
pub use physical_global_copy_v21::*;
pub use physical_lds_exchange_v22::*;
pub use receipt_v1::*;
pub use storage_discriminant_v18::{
    CanonicalStorageDiscriminantReadErrorV18, CanonicalStorageDiscriminantReadObligationV18,
    StorageDiscriminantReadRequirementV18, derive_canonical_storage_discriminant_read_v18,
};

use guarded_access_v1::{GuardedAnalysisV1, GuardedControlV1, GuardedResourceErrorV1};
use pointer_derivation::{
    AccessDerivationContext, AccessDerivationError, derive_access,
    derive_conservative_guarded_access,
};
use private_slots::{classify_eligible_private_slots, collect_private_load_sources};

/// A caller-supplied one-dimensional launch extent used for formal extraction.
///
/// This input is not authenticated against a runtime launch. `Unknown` is
/// accepted so pipeline stages can fail closed without inventing a launch size;
/// it can never produce a complete analysis.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExplicitLaunchExtent1d {
    Exact(u64),
    Unknown,
}

/// Caller-supplied ranked launch extents used for formal extraction.
///
/// Active axes are interpreted in X-major row-major order. Inactive axes must
/// have extent one. As with [`ExplicitLaunchExtent1d`], these values are
/// descriptive inputs and do not authenticate a runtime launch.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExplicitLaunchExtent {
    Exact { rank: u8, extents: [u64; 3] },
    Unknown,
}

impl From<ExplicitLaunchExtent1d> for ExplicitLaunchExtent {
    fn from(value: ExplicitLaunchExtent1d) -> Self {
        match value {
            ExplicitLaunchExtent1d::Exact(x) => Self::Exact {
                rank: 1,
                extents: [x, 1, 1],
            },
            ExplicitLaunchExtent1d::Unknown => Self::Unknown,
        }
    }
}

/// Returns the row-major logical invocation index for one ranked coordinate.
///
/// Invalid ranks, inactive-axis shapes, out-of-range coordinates, and
/// arithmetic overflow return `None`.
pub fn row_major_invocation_index(
    rank: u8,
    extents: [u64; 3],
    coordinate: [u64; 3],
) -> Option<u64> {
    if !(1..=3).contains(&rank)
        || (rank < 2 && (extents[1] != 1 || coordinate[1] != 0))
        || (rank < 3 && (extents[2] != 1 || coordinate[2] != 0))
        || extents.contains(&0)
        || coordinate
            .into_iter()
            .zip(extents)
            .any(|(coordinate, extent)| coordinate >= extent)
    {
        return None;
    }
    coordinate[2]
        .checked_mul(extents[1])?
        .checked_add(coordinate[1])?
        .checked_mul(extents[0])?
        .checked_add(coordinate[0])
}

/// Caller-supplied pointer-sized integer width used for formal extraction.
///
/// The current formal affine model is defined only for 64-bit AMDGPU index
/// arithmetic. This input is not a target-authentication token; other widths
/// are explicit fail-closed inputs.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FormalIndexWidth {
    Bits32,
    Bits64,
    Unknown,
}

/// A compiler-derived identity for one pointer or slice kernel parameter.
///
/// This identity names a formal parameter, not a runtime allocation. There is
/// deliberately no public constructor and no conversion to `AllocationId`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormalAllocationIdentity {
    parameter_index: u32,
}

impl FormalAllocationIdentity {
    pub const fn parameter_index(self) -> u32 {
        self.parameter_index
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FormalParameterKind {
    Pointer,
    Slice,
}

/// Static information about one allocation-bearing kernel parameter.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormalAllocationParameter {
    identity: FormalAllocationIdentity,
    value: ValueId,
    kind: FormalParameterKind,
    address_space: AddressSpace,
    access: AccessMode,
}

impl FormalAllocationParameter {
    pub const fn identity(&self) -> FormalAllocationIdentity {
        self.identity
    }

    pub const fn value(&self) -> ValueId {
        self.value
    }

    pub const fn kind(&self) -> FormalParameterKind {
        self.kind
    }

    pub const fn address_space(&self) -> AddressSpace {
        self.address_space
    }

    pub const fn access(&self) -> AccessMode {
        self.access
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FormalMemoryAccessKind {
    Read,
    Write,
    Atomic,
}

/// A proved condition on an actual access, not an authenticated runtime length.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FormalGuardedPathV1 {
    ExplicitPredicate,
    TrueEdge {
        source: BlockId,
        ordinal: usize,
        target: BlockId,
    },
}

/// The may-access domain is the outer launch intersected with index < slice.len.
/// Coordinates are inert without the original verified-module extraction owner.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormalSliceBoundedDomainV1 {
    allocation: FormalAllocationIdentity,
    slice: ValueId,
    index: ValueId,
    length: ValueId,
    predicate: ValueId,
    selected_offset: ValueId,
    pointer: ValueId,
    element_bytes: u64,
    path: FormalGuardedPathV1,
}

impl FormalSliceBoundedDomainV1 {
    pub const fn allocation(self) -> FormalAllocationIdentity {
        self.allocation
    }
    pub const fn slice(self) -> ValueId {
        self.slice
    }
    pub const fn index(self) -> ValueId {
        self.index
    }
    pub const fn length(self) -> ValueId {
        self.length
    }
    pub const fn predicate(self) -> ValueId {
        self.predicate
    }
    pub const fn selected_offset(self) -> ValueId {
        self.selected_offset
    }
    pub const fn pointer(self) -> ValueId {
        self.pointer
    }
    pub const fn element_bytes(self) -> u64 {
        self.element_bytes
    }
    pub const fn path(self) -> FormalGuardedPathV1 {
        self.path
    }

    /// Descriptive evaluation only; neither input authenticates a runtime binding.
    pub const fn may_access_untrusted_index(
        self,
        index: u64,
        slice_len: u64,
        launch_len: u64,
    ) -> bool {
        index < launch_len && index < slice_len
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FormalAccessDomainV1 {
    LaunchEnvelope,
    SliceBounded(FormalSliceBoundedDomainV1),
    RuntimeSliceReadBounded(FormalRuntimeSliceReadDomainV1),
}

/// An ordinary read bounded by an exact runtime slice-length true edge.
/// The index is opaque, not an invocation number or an affine byte address.
/// Coordinates are inert without fresh extraction from the verified module.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormalRuntimeSliceReadDomainV1 {
    allocation: FormalAllocationIdentity,
    slice: ValueId,
    index: ValueId,
    guard_index: ValueId,
    length: ValueId,
    predicate: ValueId,
    pointer: ValueId,
    element_bytes: u64,
    path: FormalGuardedPathV1,
}

impl FormalRuntimeSliceReadDomainV1 {
    pub const fn allocation(self) -> FormalAllocationIdentity {
        self.allocation
    }
    pub const fn slice(self) -> ValueId {
        self.slice
    }
    pub const fn index(self) -> ValueId {
        self.index
    }
    pub const fn guard_index(self) -> ValueId {
        self.guard_index
    }
    pub const fn length(self) -> ValueId {
        self.length
    }
    pub const fn predicate(self) -> ValueId {
        self.predicate
    }
    pub const fn pointer(self) -> ValueId {
        self.pointer
    }
    pub const fn element_bytes(self) -> u64 {
        self.element_bytes
    }
    pub const fn path(self) -> FormalGuardedPathV1 {
        self.path
    }
}

/// A compiler-derived, per-invocation byte region rooted at a formal kernel
/// parameter.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormalMemoryAccess {
    location: FunctionOperationLocation,
    allocation: FormalAllocationIdentity,
    kind: FormalMemoryAccessKind,
    address_space: AddressSpace,
    byte_offset: ByteExpression,
    byte_width: u64,
    alignment: u64,
    invocations: InvocationRange1d,
    domain: FormalAccessDomainV1,
}

impl FormalMemoryAccess {
    pub const fn location(&self) -> FunctionOperationLocation {
        self.location
    }

    pub const fn allocation(&self) -> FormalAllocationIdentity {
        self.allocation
    }

    pub const fn kind(&self) -> FormalMemoryAccessKind {
        self.kind
    }

    pub const fn address_space(&self) -> AddressSpace {
        self.address_space
    }

    pub const fn byte_offset(&self) -> ByteExpression {
        self.byte_offset
    }

    pub const fn byte_width(&self) -> u64 {
        self.byte_width
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    pub const fn invocations(&self) -> InvocationRange1d {
        self.invocations
    }

    /// Inspect this before interpreting byte_offset over the outer launch universe.
    pub const fn domain(&self) -> FormalAccessDomainV1 {
        self.domain
    }
}

/// Runtime allocation size needed for one compiler-derived access family.
///
/// This is an obligation for a later authenticated host binding. It does not
/// assert that the runtime argument has this size.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormalBoundsRequirement {
    location: FunctionOperationLocation,
    allocation: FormalAllocationIdentity,
    kind: FormalBoundsKindV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FormalBoundsKindV1 {
    FixedMinimumBytes(u64),
    SliceElementAtGuardedIndex(FormalSliceBoundedDomainV1),
    RuntimeSliceElementAtGuardedIndex(FormalRuntimeSliceReadDomainV1),
}

impl FormalBoundsRequirement {
    pub const fn location(self) -> FunctionOperationLocation {
        self.location
    }

    pub const fn allocation(self) -> FormalAllocationIdentity {
        self.allocation
    }

    pub const fn minimum_byte_len(self) -> Option<u64> {
        match self.kind {
            FormalBoundsKindV1::FixedMinimumBytes(bytes) => Some(bytes),
            _ => None,
        }
    }

    pub const fn kind(self) -> FormalBoundsKindV1 {
        self.kind
    }

    /// Descriptive check only; the caller remains responsible for
    /// authenticating which runtime allocation and extent are being checked.
    pub const fn is_met_by_untrusted_byte_len(self, byte_len: u64) -> bool {
        match self.minimum_byte_len() {
            Some(minimum) => byte_len >= minimum,
            None => false,
        }
    }
}

/// A half-open byte range relative to a formal parameter's runtime base.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormalByteRange {
    start: u64,
    end_exclusive: u64,
}

impl FormalByteRange {
    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn end_exclusive(self) -> u64 {
        self.end_exclusive
    }

    const fn overlaps(self, other: Self) -> bool {
        self.start < other.end_exclusive && other.start < self.end_exclusive
    }
}

/// A host-side requirement needed because distinct formal parameters may name
/// overlapping ranges of the same runtime allocation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RuntimeAliasRequirement {
    left: FormalAllocationIdentity,
    right: FormalAllocationIdentity,
    left_accessed_bytes: FormalAliasRegionV1,
    right_accessed_bytes: FormalAliasRegionV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FormalAliasRegionV1 {
    FixedBytes(FormalByteRange),
    WholeFormalAllocation,
}

impl FormalAliasRegionV1 {
    pub const fn fixed_bytes(self) -> Option<FormalByteRange> {
        match self {
            Self::FixedBytes(range) => Some(range),
            Self::WholeFormalAllocation => None,
        }
    }
}

impl RuntimeAliasRequirement {
    pub const fn left(self) -> FormalAllocationIdentity {
        self.left
    }

    pub const fn right(self) -> FormalAllocationIdentity {
        self.right
    }

    pub const fn left_accessed_bytes(self) -> Option<FormalByteRange> {
        self.left_accessed_bytes.fixed_bytes()
    }

    pub const fn right_accessed_bytes(self) -> Option<FormalByteRange> {
        self.right_accessed_bytes.fixed_bytes()
    }

    pub const fn left_region(self) -> FormalAliasRegionV1 {
        self.left_accessed_bytes
    }
    pub const fn right_region(self) -> FormalAliasRegionV1 {
        self.right_accessed_bytes
    }
}

/// A compiler-derived possible race within one formal allocation.
///
/// This is an unsatisfied obligation, not a statement about runtime behavior.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InterInvocationConflictRequirement {
    left: FunctionOperationLocation,
    right: FunctionOperationLocation,
    allocation: FormalAllocationIdentity,
}

impl InterInvocationConflictRequirement {
    pub const fn left(self) -> FunctionOperationLocation {
        self.left
    }

    pub const fn right(self) -> FunctionOperationLocation {
        self.right
    }

    pub const fn allocation(self) -> FormalAllocationIdentity {
        self.allocation
    }
}

/// Why compiler-derived formal extraction could not account for all memory
/// behavior of the selected kernel launch.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FormalMemoryIncompleteReason {
    UnsupportedIndexWidth {
        width: FormalIndexWidth,
    },
    LaunchExtentUnknown,
    LaunchExtentZero,
    LaunchRankUnsupported {
        rank: u8,
    },
    LaunchRankMismatch {
        domain_rank: u8,
        extent_rank: u8,
    },
    LaunchExtentShapeMismatch {
        rank: u8,
        extents: [u64; 3],
    },
    LaunchExtentOverflow {
        rank: u8,
        extents: [u64; 3],
    },
    StaticLaunchExtentMismatch {
        expected: u32,
        actual: u64,
    },
    StaticLaunchAxisExtentMismatch {
        axis: Axis,
        expected: u32,
        actual: u64,
    },
    CallEffectsUnavailable {
        location: FunctionOperationLocation,
        callee: FunctionId,
    },
    UnsupportedMemoryEffect {
        location: FunctionOperationLocation,
    },
    /// A conditional read is represented exactly in KIR, but the affine
    /// extractor needs owner-held ranked bounds/race proof for its predicate.
    GuardedAccessRequiresRankedProof {
        location: FunctionOperationLocation,
    },
    /// Kernel IR execution enters the first block without block arguments.
    UnsupportedEntryBlockParameters {
        block: BlockId,
    },
    UnsupportedPointerDerivation {
        location: FunctionOperationLocation,
        pointer: ValueId,
    },
    UnsupportedIndexExpression {
        location: FunctionOperationLocation,
        index: ValueId,
        allocation: FormalAllocationIdentity,
    },
    GuardedAccessPathUnavailable {
        location: FunctionOperationLocation,
        predicate: ValueId,
    },
    ElementWidthUnavailable {
        location: FunctionOperationLocation,
        pointer: ValueId,
    },
    AddressArithmeticOverflow {
        location: FunctionOperationLocation,
    },
}

/// Describes exactly what the analysis result establishes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FormalMemoryAnalysisBasis {
    /// Formal parameter identities and access expressions came from verified
    /// IR. The launch extent and index width remain unauthenticated caller
    /// inputs, and no runtime pointer, allocation extent, launch geometry, or
    /// alias relationship has been authenticated.
    CompilerDerivedIrWithUnauthenticatedLaunchInputs,
}

/// Partial or complete formal facts for one kernel launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormalMemoryObligations {
    kernel: KernelId,
    entry: FunctionId,
    index_width: FormalIndexWidth,
    invocations: Option<InvocationRange1d>,
    allocations: Vec<FormalAllocationParameter>,
    accesses: Vec<FormalMemoryAccess>,
    bounds_requirements: Vec<FormalBoundsRequirement>,
    runtime_alias_requirements: Vec<RuntimeAliasRequirement>,
    inter_invocation_conflicts: Vec<InterInvocationConflictRequirement>,
}

impl FormalMemoryObligations {
    pub fn kernel(&self) -> &KernelId {
        &self.kernel
    }

    pub fn entry(&self) -> &FunctionId {
        &self.entry
    }

    pub const fn index_width(&self) -> FormalIndexWidth {
        self.index_width
    }

    pub const fn analysis_basis(&self) -> FormalMemoryAnalysisBasis {
        FormalMemoryAnalysisBasis::CompilerDerivedIrWithUnauthenticatedLaunchInputs
    }

    pub const fn invocations(&self) -> Option<InvocationRange1d> {
        self.invocations
    }

    pub fn allocations(&self) -> &[FormalAllocationParameter] {
        &self.allocations
    }

    pub fn accesses(&self) -> &[FormalMemoryAccess] {
        &self.accesses
    }

    pub fn bounds_requirements(&self) -> &[FormalBoundsRequirement] {
        &self.bounds_requirements
    }

    pub fn runtime_alias_requirements(&self) -> &[RuntimeAliasRequirement] {
        &self.runtime_alias_requirements
    }

    pub fn inter_invocation_conflicts(&self) -> &[InterInvocationConflictRequirement] {
        &self.inter_invocation_conflicts
    }
}

/// Completeness of compiler-derived formal extraction.
///
/// `Complete` means all modeled IR effects were translated into obligations
/// under the caller-supplied launch extent and 64-bit index-width input. It
/// does not authenticate those inputs or mean the obligations hold for a
/// runtime launch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormalMemoryObligationAnalysis {
    Complete(FormalMemoryObligations),
    Incomplete {
        partial: FormalMemoryObligations,
        reasons: Vec<FormalMemoryIncompleteReason>,
    },
}

impl FormalMemoryObligationAnalysis {
    pub const fn obligations(&self) -> &FormalMemoryObligations {
        match self {
            Self::Complete(obligations) => obligations,
            Self::Incomplete { partial, .. } => partial,
        }
    }

    pub fn incomplete_reasons(&self) -> &[FormalMemoryIncompleteReason] {
        match self {
            Self::Complete(_) => &[],
            Self::Incomplete { reasons, .. } => reasons,
        }
    }

    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete(_))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormalMemoryObligationError {
    InvalidModule(VerificationErrors),
    MissingKernel { kernel: KernelId },
    InvalidInvocationRange(RegionValidationError),
    GuardedResource(FormalGuardedMemoryResourceErrorV1),
}

impl fmt::Display for FormalMemoryObligationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidModule(errors) => errors.fmt(formatter),
            Self::MissingKernel { kernel } => {
                write!(formatter, "kernel {kernel} is not present in the module")
            }
            Self::InvalidInvocationRange(error) => {
                write!(
                    formatter,
                    "formal launch invocation range is invalid: {error}"
                )
            }
            Self::GuardedResource(error) => error.fmt(formatter),
        }
    }
}

impl Error for FormalMemoryObligationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidModule(errors) => Some(errors),
            Self::InvalidInvocationRange(error) => Some(error),
            Self::MissingKernel { .. } => None,
            Self::GuardedResource(error) => Some(error),
        }
    }
}

impl From<GuardedResourceErrorV1> for FormalMemoryObligationError {
    fn from(error: GuardedResourceErrorV1) -> Self {
        Self::GuardedResource(error)
    }
}

/// Derives formal memory obligations from a structurally verified Kernel IR
/// module and caller-supplied launch-analysis inputs.
///
/// No caller-selected allocation identity is accepted. The launch extent and
/// index width are descriptive analysis inputs, not authenticated runtime or
/// target facts. The resulting formal identities, launch geometry, and memory
/// obligations require runtime authentication before any launch-safety or
/// race-freedom claim can be made.
pub fn derive_kernel_memory_obligations(
    module: &Module,
    kernel_id: &KernelId,
    launch_extent: ExplicitLaunchExtent1d,
    index_width: FormalIndexWidth,
) -> Result<FormalMemoryObligationAnalysis, FormalMemoryObligationError> {
    derive_kernel_memory_obligations_for_launch(
        module,
        kernel_id,
        launch_extent.into(),
        index_width,
    )
}

/// Derives formal memory obligations for an explicit ranked launch shape.
pub fn derive_kernel_memory_obligations_for_launch(
    module: &Module,
    kernel_id: &KernelId,
    launch_extent: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
) -> Result<FormalMemoryObligationAnalysis, FormalMemoryObligationError> {
    let verified = verify_module_ref(module).map_err(FormalMemoryObligationError::InvalidModule)?;
    derive_kernel_memory_obligations_from_verified_for_launch(
        verified,
        kernel_id,
        launch_extent,
        index_width,
    )
}

/// Derives formal memory obligations while reusing a prior Kernel IR
/// verification traversal.
///
/// The token is constructible only by [`verify_module_ref`]. This entry point
/// lets a fixed analysis pipeline verify once and share the result across
/// bounds, race, convergence, and initialization passes.
pub fn derive_kernel_memory_obligations_from_verified(
    verified: VerifiedKernelIrModuleV1<'_>,
    kernel_id: &KernelId,
    launch_extent: ExplicitLaunchExtent1d,
    index_width: FormalIndexWidth,
) -> Result<FormalMemoryObligationAnalysis, FormalMemoryObligationError> {
    derive_kernel_memory_obligations_from_verified_for_launch(
        verified,
        kernel_id,
        launch_extent.into(),
        index_width,
    )
}

/// Derives formal memory obligations for a ranked launch while reusing prior
/// Kernel IR verification.
pub fn derive_kernel_memory_obligations_from_verified_for_launch(
    verified: VerifiedKernelIrModuleV1<'_>,
    kernel_id: &KernelId,
    launch_extent: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
) -> Result<FormalMemoryObligationAnalysis, FormalMemoryObligationError> {
    derive_kernel_memory_obligations_with_v19_context(
        verified,
        kernel_id,
        launch_extent,
        index_width,
        None,
    )
}

/// Only the exact typed V19 entry supplies context. Generic callers retain all
/// previous unsupported-effect cases and cannot opt into this extension.
fn derive_kernel_memory_obligations_with_v19_context(
    verified: VerifiedKernelIrModuleV1<'_>,
    kernel_id: &KernelId,
    launch_extent: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
    canonical_v19: Option<&crate::VerifiedCanonicalKernelIrModuleV19>,
) -> Result<FormalMemoryObligationAnalysis, FormalMemoryObligationError> {
    derive_kernel_memory_obligations_with_composition_context(
        verified,
        kernel_id,
        launch_extent,
        index_width,
        canonical_v19,
        None,
    )
}

fn derive_kernel_memory_obligations_with_composition_context(
    verified: VerifiedKernelIrModuleV1<'_>,
    kernel_id: &KernelId,
    launch_extent: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
    canonical_v19: Option<&crate::VerifiedCanonicalKernelIrModuleV19>,
    composition: Option<&crate::VerifiedOrderedProgramCompositionV1>,
) -> Result<FormalMemoryObligationAnalysis, FormalMemoryObligationError> {
    let module = verified.module();
    let effect_summaries = analyze_interprocedural_effects_from_verified_v1(verified)
        .expect("verified module remains valid while deriving effect summaries");
    derive_kernel_memory_obligations_from_authenticated_module(
        module,
        kernel_id,
        launch_extent,
        index_width,
        canonical_v19,
        composition,
        &effect_summaries,
    )
}

fn derive_kernel_memory_obligations_from_authenticated_module(
    module: &Module,
    kernel_id: &KernelId,
    launch_extent: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
    canonical_v19: Option<&crate::VerifiedCanonicalKernelIrModuleV19>,
    composition: Option<&crate::VerifiedOrderedProgramCompositionV1>,
    effect_summaries: &crate::InterproceduralEffectAnalysisV1,
) -> Result<FormalMemoryObligationAnalysis, FormalMemoryObligationError> {
    let ordered_composition =
        composition.is_some_and(|owner| ordered_composition_v1::contains(owner, module, kernel_id));
    let complete_body_v19 = canonical_v19.is_some_and(|owner| {
        std::ptr::eq(module, owner.module())
            && complete_body_v19::contains_verified_complete_body(owner, kernel_id)
    });
    let kernel = module
        .kernels
        .iter()
        .find(|kernel| &kernel.id == kernel_id)
        .ok_or_else(|| FormalMemoryObligationError::MissingKernel {
            kernel: kernel_id.clone(),
        })?;
    let function = module
        .function(&kernel.entry)
        .expect("verified kernel entry must exist");
    let body = function
        .body
        .as_ref()
        .expect("verified kernel entry must be a definition");

    let mut reasons = BTreeSet::new();
    let index_width_supported = index_width == FormalIndexWidth::Bits64;
    if !index_width_supported {
        reasons.insert(FormalMemoryIncompleteReason::UnsupportedIndexWidth { width: index_width });
    }
    let invocations = resolve_invocations(&kernel.domain, launch_extent, &mut reasons)?;
    let access_invocations = index_width_supported.then_some(invocations).flatten();
    let allocations = formal_allocations(function);
    let allocation_by_value: BTreeMap<_, _> = allocations
        .iter()
        .map(|allocation| (allocation.value, allocation.identity))
        .collect();
    let (definitions, guarded_control) = collect_definitions(function)?;
    if !body.blocks[0].parameters.is_empty() {
        reasons.insert(
            FormalMemoryIncompleteReason::UnsupportedEntryBlockParameters {
                block: definitions.entry,
            },
        );
    }
    let value_types = collect_types(function);
    let eligible_private_slots =
        classify_eligible_private_slots(function, &definitions, &value_types, &mut reasons);
    let private_load_sources = collect_private_load_sources(
        function,
        &definitions,
        &value_types,
        &eligible_private_slots,
    );
    let guarded = guarded_control
        .map(|control| {
            GuardedAnalysisV1::new(control, &definitions, function, kernel.domain.rank() == 1)
        })
        .transpose()?;
    let mut context = AccessDerivationContext::new(
        &definitions,
        &value_types,
        &allocations,
        &allocation_by_value,
        &private_load_sources,
        guarded,
    );
    let mut accesses = Vec::new();

    for block in &body.blocks {
        if !definitions.is_reachable(block.id) {
            continue;
        }
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let location = FunctionOperationLocation::new(block.id, operation_index);
            let proven_private = match operation.kind {
                OperationKind::Load { pointer, access }
                | OperationKind::Store {
                    pointer, access, ..
                }
                | OperationKind::GuardedLoad {
                    pointer, access, ..
                }
                | OperationKind::GuardedStore {
                    pointer, access, ..
                } if access.address_space == AddressSpace::Generic => {
                    let exact = definitions.exact_ssa_origin(pointer, &value_types)
                            .and_then(|origin| value_types.get(&origin))
                            .is_some_and(|ty| matches!(ty, Type::Pointer(p) if p.address_space == AddressSpace::Private));
                    exact
                        || if let Some(guarded) = &mut context.guarded {
                            guarded.proven_pointer_space_v18(pointer)?
                                == Some(AddressSpace::Private)
                        } else {
                            false
                        }
                }
                _ => false,
            };
            match &operation.kind {
                OperationKind::Call { callee, .. }
                    if !ordered_composition
                        && !operation.has_complete_effect_summary()
                        && !effect_summaries
                            .function(callee)
                            .is_some_and(|summary| summary.is_complete_and_pure()) =>
                {
                    reasons.insert(FormalMemoryIncompleteReason::CallEffectsUnavailable {
                        location,
                        callee: callee.clone(),
                    });
                }
                OperationKind::Call { .. } => {}
                OperationKind::Load { access, .. }
                    if access.address_space == AddressSpace::Private || proven_private => {}
                OperationKind::Load { pointer, access } => {
                    if let Some(invocations) = access_invocations {
                        match derive_access(
                            location,
                            *pointer,
                            FormalMemoryAccessKind::Read,
                            *access,
                            invocations,
                            None,
                            &mut context,
                        ) {
                            Ok(access) => guarded_access_v1::report_push(
                                &mut context.guarded,
                                &mut accesses,
                                access,
                            )?,
                            Err(AccessDerivationError::Incomplete(reason)) => {
                                reasons.insert(reason);
                            }
                            Err(AccessDerivationError::Resource(error)) => return Err(error.into()),
                        }
                    }
                }
                OperationKind::Store { access, .. }
                    if access.address_space == AddressSpace::Private || proven_private => {}
                OperationKind::Store {
                    pointer, access, ..
                }
                | OperationKind::GuardedStore {
                    pointer, access, ..
                } => {
                    if proven_private {
                        continue;
                    }
                    if let Some(invocations) = access_invocations {
                        match derive_access(
                            location,
                            *pointer,
                            FormalMemoryAccessKind::Write,
                            *access,
                            invocations,
                            match operation.kind {
                                OperationKind::GuardedStore { predicate, .. } => Some(predicate),
                                _ => None,
                            },
                            &mut context,
                        ) {
                            Ok(access) => guarded_access_v1::report_push(
                                &mut context.guarded,
                                &mut accesses,
                                access,
                            )?,
                            Err(AccessDerivationError::Incomplete(reason)) => {
                                reasons.insert(reason);
                            }
                            Err(AccessDerivationError::Resource(error)) => return Err(error.into()),
                        }
                    }
                }
                OperationKind::Alloca {
                    address_space: AddressSpace::Private,
                    ..
                } => {}
                OperationKind::Matrix(matrix) if matrix.memory_effects().is_empty() => {}
                // The verified gfx950 transpose chain owns its static LDS, accepts only a
                // read-only global U8 slice, and defines guarded zero-fill for every source
                // coordinate. It creates no caller-visible write or alias obligation.
                OperationKind::Gfx950LdsTranspose(_) => {}
                OperationKind::GuardedLoad { access, .. }
                    if access.address_space == AddressSpace::Private || proven_private => {}
                OperationKind::GuardedLoad {
                    pointer,
                    access,
                    predicate,
                    ..
                } => {
                    let mut checked_guard = false;
                    if let Some(invocations) = access_invocations {
                        match derive_access(
                            location,
                            *pointer,
                            FormalMemoryAccessKind::Read,
                            *access,
                            invocations,
                            Some(*predicate),
                            &mut context,
                        ) {
                            Ok(access) => {
                                checked_guard =
                                    matches!(access.domain, FormalAccessDomainV1::SliceBounded(_));
                                guarded_access_v1::report_push(
                                    &mut context.guarded,
                                    &mut accesses,
                                    access,
                                )?;
                            }
                            Err(AccessDerivationError::Incomplete(exact_reason)) => {
                                match derive_conservative_guarded_access(
                                    location,
                                    *pointer,
                                    *access,
                                    invocations,
                                    &mut context,
                                ) {
                                    Ok(access) => guarded_access_v1::report_push(
                                        &mut context.guarded,
                                        &mut accesses,
                                        access,
                                    )?,
                                    Err(_) => {
                                        reasons.insert(exact_reason);
                                    }
                                }
                            }
                            Err(AccessDerivationError::Resource(error)) => return Err(error.into()),
                        }
                    }
                    if !checked_guard {
                        reasons.insert(
                            FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof {
                                location,
                            },
                        );
                    }
                }
                OperationKind::Atomic(atomic) => {
                    if let Some(invocations) = access_invocations {
                        match derive_access(
                            location,
                            atomic.pointer,
                            FormalMemoryAccessKind::Atomic,
                            atomic.access,
                            invocations,
                            None,
                            &mut context,
                        ) {
                            Ok(access) => guarded_access_v1::report_push(
                                &mut context.guarded,
                                &mut accesses,
                                access,
                            )?,
                            Err(AccessDerivationError::Incomplete(reason)) => {
                                reasons.insert(reason);
                            }
                            Err(AccessDerivationError::Resource(error)) => return Err(error.into()),
                        }
                    }
                }
                OperationKind::InlineAssembly(_)
                    if gfx942_inline_u32_v30::has_closed_memory_effects(
                        operation,
                        &value_types,
                    ) => {}
                // U32 declaration/steps have no address or memory effect only
                // in this actual immutable whole-profile V19 context. Their
                // compiler ordering is unchanged; the real tail is above.
                OperationKind::Gfx942CompleteBodyDeclaration(_)
                | OperationKind::Gfx942CompleteBodyStep(_)
                    if complete_body_v19 => {}
                OperationKind::Gfx942OrderedProgram(_) if ordered_composition => {}
                OperationKind::Storage(_)
                | OperationKind::Execution(_)
                | OperationKind::Gfx942OrderedRegion(_)
                | OperationKind::Gfx942OrderedProgram(_)
                | OperationKind::Gfx942CompleteBodyDeclaration(_)
                | OperationKind::Gfx942CompleteBodyStep(_)
                | OperationKind::Gfx942PhysicalEntryDeclaration(_)
                | OperationKind::Gfx942PhysicalEntryStep(_)
                | OperationKind::Gfx942PhysicalGlobalCopyDeclaration(_)
                | OperationKind::Gfx942PhysicalGlobalCopyStep(_)
                | OperationKind::Gfx942PhysicalLdsExchangeDeclaration(_)
                | OperationKind::Gfx942PhysicalLdsExchangeStep(_)
                | OperationKind::VerificationContract(_)
                | OperationKind::VectorLoad(_)
                | OperationKind::VectorStore(_)
                | OperationKind::VectorLayoutConvert(_)
                | OperationKind::Alloca { .. }
                | OperationKind::Barrier(_)
                | OperationKind::Fence(_)
                | OperationKind::Matrix(_)
                | OperationKind::InlineAssembly(_)
                | OperationKind::WorkgroupBarrier(_)
                | OperationKind::WorkgroupMemory(_) => {
                    reasons
                        .insert(FormalMemoryIncompleteReason::UnsupportedMemoryEffect { location });
                }
                OperationKind::Constant(_)
                | OperationKind::Intrinsic(_)
                | OperationKind::MemoryIntrinsic(_)
                | OperationKind::Wave(_)
                | OperationKind::Unary { .. }
                | OperationKind::Binary { .. }
                | OperationKind::Compare { .. }
                | OperationKind::Cast { .. }
                | OperationKind::Select { .. }
                | OperationKind::SliceLength { .. }
                | OperationKind::SliceData { .. }
                | OperationKind::GetElementPointer { .. } => {
                    if !operation.memory_effects().is_empty() {
                        reasons.insert(FormalMemoryIncompleteReason::UnsupportedMemoryEffect {
                            location,
                        });
                    }
                }
            }
        }
    }

    let bounds_requirements =
        derive_bounds_requirements(&accesses, &mut reasons, &mut context.guarded)?;
    let runtime_alias_requirements = derive_alias_requirements(&accesses, &mut context.guarded)?;
    let inter_invocation_conflicts =
        derive_inter_invocation_conflicts(&accesses, &mut context.guarded)?;
    let obligations = FormalMemoryObligations {
        kernel: kernel.id.clone(),
        entry: kernel.entry.clone(),
        index_width,
        invocations,
        allocations,
        accesses,
        bounds_requirements,
        runtime_alias_requirements,
        inter_invocation_conflicts,
    };

    if reasons.is_empty() {
        Ok(FormalMemoryObligationAnalysis::Complete(obligations))
    } else {
        Ok(FormalMemoryObligationAnalysis::Incomplete {
            partial: obligations,
            reasons: reasons.into_iter().collect(),
        })
    }
}

fn resolve_invocations(
    domain: &LaunchDomain,
    launch_extent: ExplicitLaunchExtent,
    reasons: &mut BTreeSet<FormalMemoryIncompleteReason>,
) -> Result<Option<InvocationRange1d>, FormalMemoryObligationError> {
    let ExplicitLaunchExtent::Exact { rank, extents } = launch_extent else {
        reasons.insert(FormalMemoryIncompleteReason::LaunchExtentUnknown);
        return Ok(None);
    };
    if !(1..=3).contains(&rank) {
        reasons.insert(FormalMemoryIncompleteReason::LaunchRankUnsupported { rank });
        return Ok(None);
    }
    if domain.rank() != rank {
        reasons.insert(FormalMemoryIncompleteReason::LaunchRankMismatch {
            domain_rank: domain.rank(),
            extent_rank: rank,
        });
        return Ok(None);
    }
    if (rank < 2 && extents[1] != 1) || (rank < 3 && extents[2] != 1) {
        reasons.insert(FormalMemoryIncompleteReason::LaunchExtentShapeMismatch { rank, extents });
        return Ok(None);
    }
    if extents.contains(&0) {
        reasons.insert(FormalMemoryIncompleteReason::LaunchExtentZero);
        return Ok(None);
    }
    for (index, expected) in domain.extents().enumerate() {
        let LaunchExtent::Static(expected) = expected else {
            continue;
        };
        let actual = extents[index];
        if u64::from(expected) == actual {
            continue;
        }
        if rank == 1 {
            reasons.insert(FormalMemoryIncompleteReason::StaticLaunchExtentMismatch {
                expected,
                actual,
            });
        } else {
            reasons.insert(
                FormalMemoryIncompleteReason::StaticLaunchAxisExtentMismatch {
                    axis: [Axis::X, Axis::Y, Axis::Z][index],
                    expected,
                    actual,
                },
            );
        }
        return Ok(None);
    }
    let Some(count) = extents[..usize::from(rank)]
        .iter()
        .try_fold(1_u64, |count, extent| count.checked_mul(*extent))
    else {
        reasons.insert(FormalMemoryIncompleteReason::LaunchExtentOverflow { rank, extents });
        return Ok(None);
    };
    InvocationRange1d::from_count(count)
        .map(Some)
        .map_err(FormalMemoryObligationError::InvalidInvocationRange)
}

fn formal_allocations(function: &Function) -> Vec<FormalAllocationParameter> {
    let body = function
        .body
        .as_ref()
        .expect("verified kernel entry is defined");
    body.parameters
        .iter()
        .copied()
        .zip(&function.signature.parameters)
        .enumerate()
        .filter_map(|(parameter_index, (value, ty))| {
            let (kind, address_space, access) = match ty {
                Type::Pointer(pointer) => (
                    FormalParameterKind::Pointer,
                    pointer.address_space,
                    pointer.access,
                ),
                Type::Slice(slice) => (
                    FormalParameterKind::Slice,
                    slice.address_space,
                    slice.access,
                ),
                _ => return None,
            };
            Some(FormalAllocationParameter {
                identity: FormalAllocationIdentity {
                    parameter_index: u32::try_from(parameter_index)
                        .expect("verified body length fits ValueId space"),
                },
                value,
                kind,
                address_space,
                access,
            })
        })
        .collect()
}

struct Definitions<'module> {
    operations: BTreeMap<ValueId, (&'module Operation, FunctionOperationLocation)>,
    block_parameter_inputs: BTreeMap<ValueId, Vec<ValueId>>,
    block_parameter_origins: BTreeMap<ValueId, Option<ValueId>>,
    affine_expressions: BTreeMap<ValueId, Result<AffineExpression, IndexExpressionError>>,
    reachable_blocks: BTreeSet<BlockId>,
    entry: BlockId,
}

fn collect_definitions(
    function: &Function,
) -> Result<(Definitions<'_>, Option<GuardedControlV1>), GuardedResourceErrorV1> {
    let mut operations = BTreeMap::new();
    let mut block_parameter_inputs = BTreeMap::new();
    let body = function
        .body
        .as_ref()
        .expect("verified function is defined");
    let control_flow = analyze_control_flow(function)
        .expect("verified function has analyzable bounded control flow");
    let guarded_control = GuardedControlV1::collect(function, &control_flow)?;
    let entry = body.blocks[0].id;
    let reachable_blocks = body
        .blocks
        .iter()
        .filter_map(|block| control_flow.is_reachable(block.id).then_some(block.id))
        .collect::<BTreeSet<_>>();
    for block in &body.blocks {
        if !control_flow.is_reachable(block.id) {
            continue;
        }
        let incoming = control_flow
            .incoming_edges(block.id)
            .expect("verified block is indexed by control-flow analysis");
        for (ordinal, parameter) in block.parameters.iter().enumerate() {
            // The initial entry transition has no SSA arguments. Backedges to
            // entry therefore cannot authenticate an entry-block parameter.
            let inputs = if block.id == entry {
                Vec::new()
            } else {
                incoming
                    .iter()
                    .filter(|edge| {
                        control_flow
                            .edge_source(**edge)
                            .is_some_and(|source| control_flow.is_reachable(source))
                    })
                    .map(|edge| {
                        control_flow
                            .edge_arguments(function, *edge)
                            .get(ordinal)
                            .copied()
                            .expect("verified edge argument matches its block parameter")
                    })
                    .collect()
            };
            block_parameter_inputs.insert(parameter.id, inputs);
        }
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let location = FunctionOperationLocation::new(block.id, operation_index);
            for result in &operation.results {
                operations.insert(result.id, (operation, location));
            }
        }
    }
    let block_parameter_origins = compute_unique_block_parameter_origins(&block_parameter_inputs);
    let affine_expressions = compute_affine_expressions(&operations, &block_parameter_origins);
    Ok((
        Definitions {
            operations,
            block_parameter_inputs,
            block_parameter_origins,
            affine_expressions,
            reachable_blocks,
            entry,
        },
        guarded_control,
    ))
}

impl Definitions<'_> {
    fn is_reachable(&self, block: BlockId) -> bool {
        self.reachable_blocks.contains(&block)
    }

    fn unique_ssa_origin(&self, value: ValueId) -> Option<ValueId> {
        self.block_parameter_origins
            .get(&value)
            .copied()
            .unwrap_or(Some(value))
    }

    fn exact_ssa_origin(
        &self,
        value: ValueId,
        value_types: &BTreeMap<ValueId, Type>,
    ) -> Option<ValueId> {
        exact_origin_v18::legacy(self, value, value_types)
    }
}

fn checked_address_cast_source_v18(operation: &Operation, from: &Type) -> Option<ValueId> {
    exact_origin_v18::infallible(exact_origin_v18::address(
        operation,
        from,
        &mut exact_origin_v18::LegacyCompare,
    ))
}

fn checked_slice_cast_source_v18(operation: &Operation, from: &Type) -> Option<ValueId> {
    exact_origin_v18::infallible(exact_origin_v18::slice(
        operation,
        from,
        &mut exact_origin_v18::LegacyCompare,
    ))
}

fn checked_pointer_cast_source_v18(operation: &Operation, from: &Type) -> Option<ValueId> {
    exact_origin_v18::infallible(exact_origin_v18::pointer(
        operation,
        from,
        &mut exact_origin_v18::LegacyCompare,
    ))
}

#[derive(Clone, Copy)]
enum OriginSummary {
    Empty,
    One(ValueId),
    Ambiguous,
}

impl OriginSummary {
    fn include(&mut self, origin: ValueId) {
        *self = match *self {
            Self::Empty => Self::One(origin),
            Self::One(current) if current == origin => Self::One(current),
            Self::One(_) | Self::Ambiguous => Self::Ambiguous,
        };
    }
}

/// Computes one cached origin per block parameter in linear graph work. SCCs
/// permit invariant self/backedge carriers, but a recurrence without a
/// non-parameter origin remains unsupported.
fn compute_unique_block_parameter_origins(
    inputs: &BTreeMap<ValueId, Vec<ValueId>>,
) -> BTreeMap<ValueId, Option<ValueId>> {
    guarded_access_v1::origins::legacy(inputs)
}

#[derive(Clone, Copy)]
enum AffineWork {
    Enter(ValueId),
    Finish(ValueId),
}

// Only U64 literals enter the affine cache. Fixed-width arithmetic and dynamic
// U64 ancestry remain outside the INDEX affine grammar.
fn affine_result_is_supported(operation: &Operation) -> bool {
    matches!(operation.results.as_slice(), [result]
        if result.ty == Type::INDEX
            || (result.ty == Type::Scalar(ScalarType::U64)
                && matches!(operation.kind, OperationKind::Constant(Constant::U64(_)))))
}

/// Evaluates the supported affine operation graph once. The explicit stack
/// avoids host stack growth on large generated expressions, while `visiting`
/// turns any operation/phi recurrence into a deterministic unsupported result.
fn compute_affine_expressions(
    operations: &BTreeMap<ValueId, (&Operation, FunctionOperationLocation)>,
    block_parameter_origins: &BTreeMap<ValueId, Option<ValueId>>,
) -> BTreeMap<ValueId, Result<AffineExpression, IndexExpressionError>> {
    affine_engine_v2::legacy(operations, block_parameter_origins)
}

fn collect_types(function: &Function) -> BTreeMap<ValueId, Type> {
    let mut types = BTreeMap::new();
    let body = function
        .body
        .as_ref()
        .expect("verified function is defined");
    for (value, ty) in body.parameters.iter().zip(&function.signature.parameters) {
        types.insert(*value, ty.clone());
    }
    for block in &body.blocks {
        for parameter in &block.parameters {
            types.insert(parameter.id, parameter.ty.clone());
        }
        for operation in &block.operations {
            for result in &operation.results {
                types.insert(result.id, result.ty.clone());
            }
        }
    }
    types
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AffineExpression {
    constant: u64,
    invocation_coefficient: u64,
}

impl AffineExpression {
    const ZERO: Self = Self {
        constant: 0,
        invocation_coefficient: 0,
    };

    const INVOCATION: Self = Self {
        constant: 0,
        invocation_coefficient: 1,
    };

    const fn constant(value: u64) -> Self {
        Self {
            constant: value,
            invocation_coefficient: 0,
        }
    }

    fn checked_add(self, other: Self) -> Option<Self> {
        Some(Self {
            constant: self.constant.checked_add(other.constant)?,
            invocation_coefficient: self
                .invocation_coefficient
                .checked_add(other.invocation_coefficient)?,
        })
    }

    fn checked_multiply_constant(self, multiplier: u64) -> Option<Self> {
        Some(Self {
            constant: self.constant.checked_mul(multiplier)?,
            invocation_coefficient: self.invocation_coefficient.checked_mul(multiplier)?,
        })
    }

    const fn into_byte_expression(self) -> ByteExpression {
        ByteExpression::invocation_affine(self.constant, self.invocation_coefficient)
    }

    fn evaluate(self, invocation: u64) -> Option<u64> {
        self.invocation_coefficient
            .checked_mul(invocation)
            .and_then(|offset| self.constant.checked_add(offset))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IndexExpressionError {
    Unsupported,
    Overflow,
}

fn derive_affine_index(
    value: ValueId,
    definitions: &Definitions<'_>,
) -> Result<AffineExpression, IndexExpressionError> {
    let value = definitions
        .unique_ssa_origin(value)
        .ok_or(IndexExpressionError::Unsupported)?;
    definitions
        .affine_expressions
        .get(&value)
        .copied()
        .unwrap_or(Err(IndexExpressionError::Unsupported))
}

fn pointer_byte_width(ty: &Type) -> Option<u64> {
    let Type::Pointer(pointer) = ty else {
        return None;
    };
    scalar_byte_width(pointer.pointee.as_scalar()?)
}

fn scalar_byte_width(scalar: ScalarType) -> Option<u64> {
    let bits = scalar.bit_width()?;
    (bits % 8 == 0).then_some(u64::from(bits / 8))
}

fn derive_bounds_requirements(
    accesses: &[FormalMemoryAccess],
    reasons: &mut BTreeSet<FormalMemoryIncompleteReason>,
    guarded: &mut Option<GuardedAnalysisV1<'_>>,
) -> Result<Vec<FormalBoundsRequirement>, GuardedResourceErrorV1> {
    derive_bounds_requirements_with_meter(accesses, reasons, guarded)
}

fn derive_bounds_requirements_with_meter(
    accesses: &[FormalMemoryAccess],
    reasons: &mut impl report_construction_v18::BoundsReasonSinkV18,
    meter: &mut impl report_construction_v18::ReportMeterV18,
) -> Result<Vec<FormalBoundsRequirement>, GuardedResourceErrorV1> {
    let mut bounds = Vec::new();
    for access in accesses {
        meter.bounds_work()?;
        let kind = match access.domain {
            FormalAccessDomainV1::SliceBounded(domain) => {
                FormalBoundsKindV1::SliceElementAtGuardedIndex(domain)
            }
            FormalAccessDomainV1::RuntimeSliceReadBounded(domain) => {
                FormalBoundsKindV1::RuntimeSliceElementAtGuardedIndex(domain)
            }
            FormalAccessDomainV1::LaunchEnvelope => {
                let range = match access.byte_offset {
                    // Only guarded reads are admitted with an unbounded affine
                    // expression, and their bounds stay behind the distinct ranked
                    // proof reason emitted at extraction time.
                    ByteExpression::Unbounded => continue,
                    ByteExpression::Affine { .. } => match access_envelope(access) {
                        Some(range) => range,
                        None => {
                            reasons.overflow(access.location, meter)?;
                            continue;
                        }
                    },
                };
                FormalBoundsKindV1::FixedMinimumBytes(range.end_exclusive)
            }
        };
        meter.push(
            &mut bounds,
            FormalBoundsRequirement {
                location: access.location,
                allocation: access.allocation,
                kind,
            },
        )?;
    }
    Ok(bounds)
}

fn access_envelope(access: &FormalMemoryAccess) -> Option<FormalByteRange> {
    if access.domain != FormalAccessDomainV1::LaunchEnvelope {
        return None;
    }
    let ByteExpression::Affine {
        constant,
        invocation_coefficient,
    } = access.byte_offset
    else {
        return None;
    };
    let expression = AffineExpression {
        constant,
        invocation_coefficient,
    };
    let last = access.invocations.last();
    let start = expression.evaluate(0)?;
    let end_exclusive = expression.evaluate(last)?.checked_add(access.byte_width)?;
    Some(FormalByteRange {
        start,
        end_exclusive,
    })
}

#[derive(Clone, Copy)]
struct AllocationEnvelope {
    range: FormalAliasRegionV1,
    writes: bool,
    address_space: AddressSpace,
}

fn derive_alias_requirements(
    accesses: &[FormalMemoryAccess],
    guarded: &mut Option<GuardedAnalysisV1<'_>>,
) -> Result<Vec<RuntimeAliasRequirement>, GuardedResourceErrorV1> {
    derive_alias_requirements_with_meter(accesses, guarded)
}

fn derive_alias_requirements_with_meter(
    accesses: &[FormalMemoryAccess],
    meter: &mut impl report_construction_v18::ReportMeterV18,
) -> Result<Vec<RuntimeAliasRequirement>, GuardedResourceErrorV1> {
    let mut entries = Vec::<(FormalAllocationIdentity, AllocationEnvelope)>::new();
    for access in accesses {
        meter.charge(8)?;
        let range = if access.domain != FormalAccessDomainV1::LaunchEnvelope {
            FormalAliasRegionV1::WholeFormalAllocation
        } else {
            match access.byte_offset {
                ByteExpression::Unbounded => FormalAliasRegionV1::FixedBytes(FormalByteRange {
                    start: 0,
                    end_exclusive: u64::MAX,
                }),
                ByteExpression::Affine { .. } => {
                    let Some(range) = access_envelope(access) else {
                        continue;
                    };
                    FormalAliasRegionV1::FixedBytes(range)
                }
            }
        };
        meter.push(
            &mut entries,
            (
                access.allocation,
                AllocationEnvelope {
                    range,
                    writes: access.kind != FormalMemoryAccessKind::Read,
                    address_space: access.address_space,
                },
            ),
        )?;
    }
    meter.sort(&mut entries, |a, b| a.0.cmp(&b.0))?;
    let mut count = 0_usize;
    for index in 0..entries.len() {
        meter.charge(8)?;
        let (allocation, next) = entries[index];
        if count != 0 && entries[count - 1].0 == allocation {
            let envelope = &mut entries[count - 1].1;
            envelope.range = envelope.range.union(next.range);
            envelope.writes |= next.writes;
        } else {
            entries[count] = (allocation, next);
            count += 1;
        }
    }
    entries.truncate(count);
    let mut requirements = Vec::new();
    for (left_index, (left, left_envelope)) in entries.iter().enumerate() {
        for (right, right_envelope) in &entries[left_index + 1..] {
            meter.charge(8)?;
            if address_spaces_may_alias(left_envelope.address_space, right_envelope.address_space)
                && (left_envelope.writes || right_envelope.writes)
            {
                meter.push(
                    &mut requirements,
                    RuntimeAliasRequirement {
                        left: *left,
                        right: *right,
                        left_accessed_bytes: left_envelope.range,
                        right_accessed_bytes: right_envelope.range,
                    },
                )?;
            }
        }
    }
    meter.retire(entries)?;
    Ok(requirements)
}

fn address_spaces_may_alias(left: AddressSpace, right: AddressSpace) -> bool {
    left == right
        || matches!(left, AddressSpace::Generic)
        || matches!(right, AddressSpace::Generic)
        || matches!(
            (left, right),
            (AddressSpace::Global, AddressSpace::Constant)
                | (AddressSpace::Constant, AddressSpace::Global)
        )
}

fn derive_inter_invocation_conflicts(
    accesses: &[FormalMemoryAccess],
    guarded: &mut Option<GuardedAnalysisV1<'_>>,
) -> Result<Vec<InterInvocationConflictRequirement>, GuardedResourceErrorV1> {
    derive_inter_invocation_conflicts_with_meter(accesses, guarded)
}

fn derive_inter_invocation_conflicts_with_meter(
    accesses: &[FormalMemoryAccess],
    meter: &mut impl report_construction_v18::ReportMeterV18,
) -> Result<Vec<InterInvocationConflictRequirement>, GuardedResourceErrorV1> {
    let mut requirements = Vec::new();
    for (left_index, left) in accesses.iter().enumerate() {
        for right in &accesses[left_index..] {
            meter.charge(32)?;
            if left.allocation != right.allocation
                || (left.kind == FormalMemoryAccessKind::Read
                    && right.kind == FormalMemoryAccessKind::Read)
                || (left.kind == FormalMemoryAccessKind::Atomic
                    && right.kind == FormalMemoryAccessKind::Atomic)
                || proves_distinct_invocation_disjointness(left, right)
            {
                continue;
            }
            meter.push(
                &mut requirements,
                InterInvocationConflictRequirement {
                    left: left.location,
                    right: right.location,
                    allocation: left.allocation,
                },
            )?;
        }
    }
    Ok(requirements)
}

fn proves_distinct_invocation_disjointness(
    left: &FormalMemoryAccess,
    right: &FormalMemoryAccess,
) -> bool {
    // Only the same singleton on both sides excludes every distinct invocation pair.
    if left.invocations == right.invocations && left.invocations.start() == left.invocations.last()
    {
        return true;
    }
    if left.byte_offset == right.byte_offset && left.byte_width == right.byte_width {
        let ByteExpression::Affine {
            invocation_coefficient,
            ..
        } = left.byte_offset
        else {
            return false;
        };
        return invocation_coefficient >= left.byte_width;
    }
    match (access_envelope(left), access_envelope(right)) {
        (Some(left), Some(right)) => !left.overlaps(right),
        _ => false,
    }
}
