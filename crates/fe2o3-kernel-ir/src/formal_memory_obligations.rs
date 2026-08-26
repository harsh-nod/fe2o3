use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use crate::{
    AccessMode, AddressSpace, Axis, BinaryOp, BlockId, ByteExpression, Constant, Function,
    FunctionId, FunctionOperationLocation, IndexKind, IntrinsicKind, InvocationRange1d, KernelId,
    LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind,
    RegionValidationError, ScalarType, Type, ValueId, VerificationErrors, VerifiedKernelIrModuleV1,
    analyze_interprocedural_effects_from_verified_v1, verify_module_ref,
};

mod receipt_v1;

pub use receipt_v1::*;

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
}

/// Runtime allocation size needed for one compiler-derived access family.
///
/// This is an obligation for a later authenticated host binding. It does not
/// assert that the runtime argument has this size.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FormalBoundsRequirement {
    location: FunctionOperationLocation,
    allocation: FormalAllocationIdentity,
    minimum_byte_len: u64,
}

impl FormalBoundsRequirement {
    pub const fn location(self) -> FunctionOperationLocation {
        self.location
    }

    pub const fn allocation(self) -> FormalAllocationIdentity {
        self.allocation
    }

    pub const fn minimum_byte_len(self) -> u64 {
        self.minimum_byte_len
    }

    /// Descriptive check only; the caller remains responsible for
    /// authenticating which runtime allocation and extent are being checked.
    pub const fn is_met_by_untrusted_byte_len(self, byte_len: u64) -> bool {
        byte_len >= self.minimum_byte_len
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
    left_accessed_bytes: FormalByteRange,
    right_accessed_bytes: FormalByteRange,
}

impl RuntimeAliasRequirement {
    pub const fn left(self) -> FormalAllocationIdentity {
        self.left
    }

    pub const fn right(self) -> FormalAllocationIdentity {
        self.right
    }

    pub const fn left_accessed_bytes(self) -> FormalByteRange {
        self.left_accessed_bytes
    }

    pub const fn right_accessed_bytes(self) -> FormalByteRange {
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
    UnsupportedPointerDerivation {
        location: FunctionOperationLocation,
        pointer: ValueId,
    },
    UnsupportedIndexExpression {
        location: FunctionOperationLocation,
        index: ValueId,
        allocation: FormalAllocationIdentity,
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
        }
    }
}

impl Error for FormalMemoryObligationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidModule(errors) => Some(errors),
            Self::InvalidInvocationRange(error) => Some(error),
            Self::MissingKernel { .. } => None,
        }
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
    let module = verified.module();
    let effect_summaries = analyze_interprocedural_effects_from_verified_v1(verified)
        .expect("verified module remains valid while deriving effect summaries");
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
    let definitions = collect_definitions(function);
    let private_load_sources = collect_private_load_sources(function, &definitions);
    let value_types = collect_types(function);
    let context = AccessDerivationContext {
        definitions: &definitions,
        value_types: &value_types,
        allocation_by_value: &allocation_by_value,
        private_load_sources: &private_load_sources,
    };
    let mut accesses = Vec::new();

    for block in &body.blocks {
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let location = FunctionOperationLocation::new(block.id, operation_index);
            match &operation.kind {
                OperationKind::Call { callee, .. }
                    if !operation.has_complete_effect_summary()
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
                    if access.address_space == AddressSpace::Private => {}
                OperationKind::Load { pointer, access } => {
                    if direct_private_alloca_access_is_internal(*pointer, *access, &definitions) {
                        continue;
                    }
                    if let Some(invocations) = access_invocations {
                        match derive_access(
                            location,
                            *pointer,
                            FormalMemoryAccessKind::Read,
                            *access,
                            invocations,
                            &context,
                        ) {
                            Ok(access) => accesses.push(access),
                            Err(reason) => {
                                reasons.insert(reason);
                            }
                        }
                    }
                }
                OperationKind::Store { access, .. }
                    if access.address_space == AddressSpace::Private => {}
                OperationKind::Store {
                    pointer, access, ..
                } => {
                    if direct_private_alloca_access_is_internal(*pointer, *access, &definitions) {
                        continue;
                    }
                    if let Some(invocations) = access_invocations {
                        match derive_access(
                            location,
                            *pointer,
                            FormalMemoryAccessKind::Write,
                            *access,
                            invocations,
                            &context,
                        ) {
                            Ok(access) => accesses.push(access),
                            Err(reason) => {
                                reasons.insert(reason);
                            }
                        }
                    }
                }
                OperationKind::Alloca {
                    address_space: AddressSpace::Private,
                    ..
                } => {}
                OperationKind::Matrix(matrix) if matrix.memory_effects().is_empty() => {}
                OperationKind::GuardedLoad { access, .. }
                    if access.address_space == AddressSpace::Private => {}
                OperationKind::GuardedLoad {
                    pointer, access, ..
                } => {
                    if let Some(invocations) = access_invocations {
                        match derive_access(
                            location,
                            *pointer,
                            FormalMemoryAccessKind::Read,
                            *access,
                            invocations,
                            &context,
                        ) {
                            Ok(access) => accesses.push(access),
                            Err(exact_reason) => match derive_conservative_guarded_access(
                                location,
                                *pointer,
                                *access,
                                invocations,
                                &context,
                            ) {
                                Ok(access) => accesses.push(access),
                                Err(_) => {
                                    reasons.insert(exact_reason);
                                }
                            },
                        }
                    }
                    reasons.insert(
                        FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { location },
                    );
                }
                OperationKind::Alloca { .. }
                | OperationKind::Barrier(_)
                | OperationKind::Atomic(_)
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

    let bounds_requirements = derive_bounds_requirements(&accesses, &mut reasons);
    let runtime_alias_requirements = derive_alias_requirements(&accesses);
    let inter_invocation_conflicts = derive_inter_invocation_conflicts(&accesses);
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

fn direct_private_alloca_access_is_internal(
    pointer: ValueId,
    access: MemoryAccess,
    definitions: &Definitions<'_>,
) -> bool {
    if access.address_space != AddressSpace::Private {
        return false;
    }
    definitions
        .operations
        .get(&pointer)
        .is_some_and(|(operation, _)| {
            matches!(
                operation.kind,
                OperationKind::Alloca {
                    count: None,
                    address_space: AddressSpace::Private,
                    ..
                }
            )
        })
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
}

fn collect_definitions(function: &Function) -> Definitions<'_> {
    let mut operations = BTreeMap::new();
    let body = function
        .body
        .as_ref()
        .expect("verified function is defined");
    for block in &body.blocks {
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let location = FunctionOperationLocation::new(block.id, operation_index);
            for result in &operation.results {
                operations.insert(result.id, (operation, location));
            }
        }
    }
    Definitions { operations }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrivateSlotState {
    Uninitialized,
    Exact(ValueId),
    Unknown,
}

impl PrivateSlotState {
    fn join(self, other: Self) -> Self {
        if self == other { self } else { Self::Unknown }
    }
}

fn collect_private_load_sources(
    function: &Function,
    definitions: &Definitions<'_>,
) -> BTreeMap<ValueId, ValueId> {
    let body = function
        .body
        .as_ref()
        .expect("verified function is defined");
    let slots = definitions
        .operations
        .iter()
        .filter_map(|(value, (operation, _))| {
            matches!(
                operation.kind,
                OperationKind::Alloca {
                    count: None,
                    address_space: AddressSpace::Private,
                    ..
                }
            )
            .then_some(*value)
        })
        .collect::<BTreeSet<_>>();
    if slots.is_empty() || body.blocks.is_empty() {
        return BTreeMap::new();
    }

    let block_ids = body
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let mut predecessors = block_ids
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for block in &body.blocks {
        if let Some(terminator) = &block.terminator {
            for successor in terminator.successors() {
                if let Some(incoming) = predecessors.get_mut(&successor) {
                    incoming.insert(block.id);
                }
            }
        }
    }

    let entry = body.blocks[0].id;
    let initial = slots
        .iter()
        .copied()
        .map(|slot| (slot, PrivateSlotState::Uninitialized))
        .collect::<BTreeMap<_, _>>();
    let mut incoming = BTreeMap::<BlockId, BTreeMap<ValueId, PrivateSlotState>>::new();
    let mut outgoing = BTreeMap::<BlockId, BTreeMap<ValueId, PrivateSlotState>>::new();
    incoming.insert(entry, initial.clone());

    loop {
        let mut changed = false;
        for block in &body.blocks {
            let next_incoming = if block.id == entry {
                Some(initial.clone())
            } else {
                predecessors.get(&block.id).and_then(|blocks| {
                    let mut states = blocks.iter().filter_map(|block| outgoing.get(block));
                    let first = states.next()?.clone();
                    Some(states.fold(first, |mut joined, state| {
                        for slot in &slots {
                            let left = joined
                                .get(slot)
                                .copied()
                                .unwrap_or(PrivateSlotState::Unknown);
                            let right = state
                                .get(slot)
                                .copied()
                                .unwrap_or(PrivateSlotState::Unknown);
                            joined.insert(*slot, left.join(right));
                        }
                        joined
                    }))
                })
            };
            let Some(next_incoming) = next_incoming else {
                continue;
            };
            changed |= incoming.get(&block.id) != Some(&next_incoming);
            incoming.insert(block.id, next_incoming.clone());
            let mut next_outgoing = next_incoming;
            transfer_private_slot_stores(block, definitions, &mut next_outgoing);
            changed |= outgoing.get(&block.id) != Some(&next_outgoing);
            outgoing.insert(block.id, next_outgoing);
        }
        if !changed {
            break;
        }
    }

    let mut sources = BTreeMap::new();
    for block in &body.blocks {
        let Some(mut state) = incoming.get(&block.id).cloned() else {
            continue;
        };
        for operation in &block.operations {
            if let OperationKind::Load {
                pointer: slot,
                access,
            } = operation.kind
                && direct_private_alloca_access_is_internal(slot, access, definitions)
                && let Some(PrivateSlotState::Exact(source)) = state.get(&slot).copied()
            {
                for result in &operation.results {
                    sources.insert(result.id, source);
                }
            }
            transfer_private_slot_store(operation, definitions, &mut state);
        }
    }
    sources
}

fn transfer_private_slot_stores(
    block: &crate::BasicBlock,
    definitions: &Definitions<'_>,
    state: &mut BTreeMap<ValueId, PrivateSlotState>,
) {
    for operation in &block.operations {
        transfer_private_slot_store(operation, definitions, state);
    }
}

fn transfer_private_slot_store(
    operation: &Operation,
    definitions: &Definitions<'_>,
    state: &mut BTreeMap<ValueId, PrivateSlotState>,
) {
    let OperationKind::Store {
        pointer,
        value,
        access,
    } = operation.kind
    else {
        return;
    };
    if direct_private_alloca_access_is_internal(pointer, access, definitions) {
        state.insert(pointer, PrivateSlotState::Exact(value));
    }
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

struct AccessDerivationContext<'analysis, 'module> {
    definitions: &'analysis Definitions<'module>,
    value_types: &'analysis BTreeMap<ValueId, Type>,
    allocation_by_value: &'analysis BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &'analysis BTreeMap<ValueId, ValueId>,
}

fn derive_access(
    location: FunctionOperationLocation,
    pointer: ValueId,
    kind: FormalMemoryAccessKind,
    access: MemoryAccess,
    invocations: InvocationRange1d,
    context: &AccessDerivationContext<'_, '_>,
) -> Result<FormalMemoryAccess, FormalMemoryIncompleteReason> {
    let byte_width = context
        .value_types
        .get(&pointer)
        .and_then(pointer_byte_width)
        .ok_or(FormalMemoryIncompleteReason::ElementWidthUnavailable { location, pointer })?;
    let pointer_expression = derive_pointer_expression(
        pointer,
        context.definitions,
        context.value_types,
        context.allocation_by_value,
        context.private_load_sources,
        location,
    )?;
    Ok(FormalMemoryAccess {
        location,
        allocation: pointer_expression.allocation,
        kind,
        address_space: access.address_space,
        byte_offset: pointer_expression.byte_offset.into_byte_expression(),
        byte_width,
        alignment: u64::from(access.alignment),
        invocations,
    })
}

/// Retains the allocation-level read effect when a guarded address cannot be
/// represented by the affine extractor. The owner-held ranked proof remains
/// responsible for the predicate and bounds; this conservative effect prevents
/// that separate proof from erasing alias and race obligations.
fn derive_conservative_guarded_access(
    location: FunctionOperationLocation,
    pointer: ValueId,
    access: MemoryAccess,
    invocations: InvocationRange1d,
    context: &AccessDerivationContext<'_, '_>,
) -> Result<FormalMemoryAccess, FormalMemoryIncompleteReason> {
    let byte_width = context
        .value_types
        .get(&pointer)
        .and_then(pointer_byte_width)
        .ok_or(FormalMemoryIncompleteReason::ElementWidthUnavailable { location, pointer })?;
    let allocation = derive_pointer_allocation(
        pointer,
        context.definitions,
        context.value_types,
        context.allocation_by_value,
        context.private_load_sources,
        location,
    )?;
    Ok(FormalMemoryAccess {
        location,
        allocation,
        kind: FormalMemoryAccessKind::Read,
        address_space: access.address_space,
        byte_offset: ByteExpression::Unbounded,
        byte_width,
        alignment: u64::from(access.alignment),
        invocations,
    })
}

fn derive_pointer_allocation(
    pointer: ValueId,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    allocation_by_value: &BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &BTreeMap<ValueId, ValueId>,
    access_location: FunctionOperationLocation,
) -> Result<FormalAllocationIdentity, FormalMemoryIncompleteReason> {
    derive_pointer_allocation_inner(
        pointer,
        definitions,
        value_types,
        allocation_by_value,
        private_load_sources,
        access_location,
        &mut BTreeSet::new(),
    )
}

fn derive_pointer_allocation_inner(
    pointer: ValueId,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    allocation_by_value: &BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &BTreeMap<ValueId, ValueId>,
    access_location: FunctionOperationLocation,
    visited_private_loads: &mut BTreeSet<ValueId>,
) -> Result<FormalAllocationIdentity, FormalMemoryIncompleteReason> {
    if let Some(allocation) = allocation_by_value.get(&pointer).copied()
        && matches!(value_types.get(&pointer), Some(Type::Pointer(_)))
    {
        return Ok(allocation);
    }
    let Some((operation, definition_location)) = definitions.operations.get(&pointer) else {
        return Err(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
            location: access_location,
            pointer,
        });
    };
    match &operation.kind {
        OperationKind::SliceData { slice } => allocation_by_value.get(slice).copied().ok_or(
            FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                location: *definition_location,
                pointer,
            },
        ),
        OperationKind::GetElementPointer { base, .. } => derive_pointer_allocation_inner(
            *base,
            definitions,
            value_types,
            allocation_by_value,
            private_load_sources,
            access_location,
            visited_private_loads,
        ),
        OperationKind::Load { .. } => {
            if !visited_private_loads.insert(pointer) {
                return Err(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                    location: *definition_location,
                    pointer,
                });
            }
            let source = private_load_sources.get(&pointer).copied().ok_or(
                FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                    location: *definition_location,
                    pointer,
                },
            )?;
            let derived = derive_pointer_allocation_inner(
                source,
                definitions,
                value_types,
                allocation_by_value,
                private_load_sources,
                access_location,
                visited_private_loads,
            );
            visited_private_loads.remove(&pointer);
            derived
        }
        _ => Err(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
            location: *definition_location,
            pointer,
        }),
    }
}

#[derive(Clone, Copy)]
struct PointerExpression {
    allocation: FormalAllocationIdentity,
    byte_offset: AffineExpression,
}

fn derive_pointer_expression(
    pointer: ValueId,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    allocation_by_value: &BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &BTreeMap<ValueId, ValueId>,
    access_location: FunctionOperationLocation,
) -> Result<PointerExpression, FormalMemoryIncompleteReason> {
    derive_pointer_expression_inner(
        pointer,
        definitions,
        value_types,
        allocation_by_value,
        private_load_sources,
        access_location,
        &mut BTreeSet::new(),
    )
}

fn derive_pointer_expression_inner(
    pointer: ValueId,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    allocation_by_value: &BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &BTreeMap<ValueId, ValueId>,
    access_location: FunctionOperationLocation,
    visited_private_slots: &mut BTreeSet<ValueId>,
) -> Result<PointerExpression, FormalMemoryIncompleteReason> {
    if let Some(allocation) = allocation_by_value.get(&pointer).copied()
        && matches!(value_types.get(&pointer), Some(Type::Pointer(_)))
    {
        return Ok(PointerExpression {
            allocation,
            byte_offset: AffineExpression::ZERO,
        });
    }
    let Some((operation, definition_location)) = definitions.operations.get(&pointer) else {
        return Err(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
            location: access_location,
            pointer,
        });
    };
    match &operation.kind {
        OperationKind::SliceData { slice } => allocation_by_value
            .get(slice)
            .copied()
            .map(|allocation| PointerExpression {
                allocation,
                byte_offset: AffineExpression::ZERO,
            })
            .ok_or(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                location: *definition_location,
                pointer,
            }),
        OperationKind::GetElementPointer { base, offset } => {
            let base_expression = derive_pointer_expression_inner(
                *base,
                definitions,
                value_types,
                allocation_by_value,
                private_load_sources,
                access_location,
                visited_private_slots,
            )?;
            let element_width = value_types.get(base).and_then(pointer_byte_width).ok_or(
                FormalMemoryIncompleteReason::ElementWidthUnavailable {
                    location: *definition_location,
                    pointer: *base,
                },
            )?;
            let index = derive_affine_index(*offset, definitions).map_err(|error| match error {
                IndexExpressionError::Unsupported => {
                    FormalMemoryIncompleteReason::UnsupportedIndexExpression {
                        location: *definition_location,
                        index: *offset,
                        allocation: base_expression.allocation,
                    }
                }
                IndexExpressionError::Overflow => {
                    FormalMemoryIncompleteReason::AddressArithmeticOverflow {
                        location: *definition_location,
                    }
                }
            })?;
            let byte_delta = index.checked_multiply_constant(element_width).ok_or(
                FormalMemoryIncompleteReason::AddressArithmeticOverflow {
                    location: *definition_location,
                },
            )?;
            let byte_offset = base_expression.byte_offset.checked_add(byte_delta).ok_or(
                FormalMemoryIncompleteReason::AddressArithmeticOverflow {
                    location: *definition_location,
                },
            )?;
            Ok(PointerExpression {
                allocation: base_expression.allocation,
                byte_offset,
            })
        }
        OperationKind::Load {
            pointer: private_slot,
            access,
        } if direct_private_alloca_access_is_internal(*private_slot, *access, definitions) => {
            if !visited_private_slots.insert(pointer) {
                return Err(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                    location: *definition_location,
                    pointer,
                });
            }
            let stored_value = private_load_sources.get(&pointer).copied().ok_or(
                FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                    location: *definition_location,
                    pointer,
                },
            )?;
            let derived = derive_pointer_expression_inner(
                stored_value,
                definitions,
                value_types,
                allocation_by_value,
                private_load_sources,
                access_location,
                visited_private_slots,
            );
            visited_private_slots.remove(&pointer);
            derived
        }
        _ => Err(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
            location: *definition_location,
            pointer,
        }),
    }
}

#[derive(Clone, Copy)]
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

#[derive(Clone, Copy)]
enum IndexExpressionError {
    Unsupported,
    Overflow,
}

fn derive_affine_index(
    value: ValueId,
    definitions: &Definitions<'_>,
) -> Result<AffineExpression, IndexExpressionError> {
    let (operation, _) = definitions
        .operations
        .get(&value)
        .ok_or(IndexExpressionError::Unsupported)?;
    if !matches!(operation.results.as_slice(), [result] if result.ty == Type::INDEX) {
        return Err(IndexExpressionError::Unsupported);
    }
    match &operation.kind {
        OperationKind::Constant(Constant::Index(value)) => Ok(AffineExpression::constant(*value)),
        OperationKind::Intrinsic(intrinsic)
            if intrinsic.kind
                == (IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::X,
                }) =>
        {
            Ok(AffineExpression::INVOCATION)
        }
        OperationKind::Binary { op, lhs, rhs } => {
            let lhs = derive_affine_index(*lhs, definitions)?;
            let rhs = derive_affine_index(*rhs, definitions)?;
            match op {
                BinaryOp::Add => lhs.checked_add(rhs).ok_or(IndexExpressionError::Overflow),
                BinaryOp::Multiply if lhs.invocation_coefficient == 0 => rhs
                    .checked_multiply_constant(lhs.constant)
                    .ok_or(IndexExpressionError::Overflow),
                BinaryOp::Multiply if rhs.invocation_coefficient == 0 => lhs
                    .checked_multiply_constant(rhs.constant)
                    .ok_or(IndexExpressionError::Overflow),
                BinaryOp::Multiply => Err(IndexExpressionError::Unsupported),
                _ => Err(IndexExpressionError::Unsupported),
            }
        }
        _ => Err(IndexExpressionError::Unsupported),
    }
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
) -> Vec<FormalBoundsRequirement> {
    accesses
        .iter()
        .filter_map(|access| {
            let range = match access.byte_offset {
                // Only guarded reads are admitted with an unbounded affine
                // expression, and their bounds stay behind the distinct ranked
                // proof reason emitted at extraction time.
                ByteExpression::Unbounded => return None,
                ByteExpression::Affine { .. } => access_envelope(access).or_else(|| {
                    reasons.insert(FormalMemoryIncompleteReason::AddressArithmeticOverflow {
                        location: access.location,
                    });
                    None
                })?,
            };
            Some(FormalBoundsRequirement {
                location: access.location,
                allocation: access.allocation,
                minimum_byte_len: range.end_exclusive,
            })
        })
        .collect()
}

fn access_envelope(access: &FormalMemoryAccess) -> Option<FormalByteRange> {
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
    range: FormalByteRange,
    writes: bool,
    address_space: AddressSpace,
}

fn derive_alias_requirements(accesses: &[FormalMemoryAccess]) -> Vec<RuntimeAliasRequirement> {
    let mut envelopes = BTreeMap::<FormalAllocationIdentity, AllocationEnvelope>::new();
    for access in accesses {
        let range = match access.byte_offset {
            ByteExpression::Unbounded => FormalByteRange {
                start: 0,
                end_exclusive: u64::MAX,
            },
            ByteExpression::Affine { .. } => {
                let Some(range) = access_envelope(access) else {
                    continue;
                };
                range
            }
        };
        envelopes
            .entry(access.allocation)
            .and_modify(|envelope| {
                envelope.range.start = envelope.range.start.min(range.start);
                envelope.range.end_exclusive =
                    envelope.range.end_exclusive.max(range.end_exclusive);
                envelope.writes |= access.kind == FormalMemoryAccessKind::Write;
            })
            .or_insert(AllocationEnvelope {
                range,
                writes: access.kind == FormalMemoryAccessKind::Write,
                address_space: access.address_space,
            });
    }

    let entries: Vec<_> = envelopes.into_iter().collect();
    let mut requirements = Vec::new();
    for (left_index, (left, left_envelope)) in entries.iter().enumerate() {
        for (right, right_envelope) in &entries[left_index + 1..] {
            if address_spaces_may_alias(left_envelope.address_space, right_envelope.address_space)
                && (left_envelope.writes || right_envelope.writes)
            {
                requirements.push(RuntimeAliasRequirement {
                    left: *left,
                    right: *right,
                    left_accessed_bytes: left_envelope.range,
                    right_accessed_bytes: right_envelope.range,
                });
            }
        }
    }
    requirements
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
) -> Vec<InterInvocationConflictRequirement> {
    let mut requirements = Vec::new();
    for (left_index, left) in accesses.iter().enumerate() {
        for right in &accesses[left_index..] {
            if left.allocation != right.allocation
                || (left.kind == FormalMemoryAccessKind::Read
                    && right.kind == FormalMemoryAccessKind::Read)
                || proves_distinct_invocation_disjointness(left, right)
            {
                continue;
            }
            requirements.push(InterInvocationConflictRequirement {
                left: left.location,
                right: right.location,
                allocation: left.allocation,
            });
        }
    }
    requirements
}

fn proves_distinct_invocation_disjointness(
    left: &FormalMemoryAccess,
    right: &FormalMemoryAccess,
) -> bool {
    if left.invocations.last() == 0 || right.invocations.last() == 0 {
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
