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
    Atomic,
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
    let mut context = AccessDerivationContext {
        definitions: &definitions,
        value_types: &value_types,
        allocation_by_value: &allocation_by_value,
        private_load_sources: &private_load_sources,
        pointer_derivations: PointerDerivationCache::default(),
    };
    let mut accesses = Vec::new();

    for block in &body.blocks {
        if !definitions.is_reachable(block.id) {
            continue;
        }
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
                    if let Some(invocations) = access_invocations {
                        match derive_access(
                            location,
                            *pointer,
                            FormalMemoryAccessKind::Read,
                            *access,
                            invocations,
                            &mut context,
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
                }
                | OperationKind::GuardedStore {
                    pointer, access, ..
                } => {
                    if let Some(invocations) = access_invocations {
                        match derive_access(
                            location,
                            *pointer,
                            FormalMemoryAccessKind::Write,
                            *access,
                            invocations,
                            &mut context,
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
                // The verified gfx950 transpose chain owns its static LDS, accepts only a
                // read-only global U8 slice, and defines guarded zero-fill for every source
                // coordinate. It creates no caller-visible write or alias obligation.
                OperationKind::Gfx950LdsTranspose(_) => {}
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
                            &mut context,
                        ) {
                            Ok(access) => accesses.push(access),
                            Err(exact_reason) => match derive_conservative_guarded_access(
                                location,
                                *pointer,
                                *access,
                                invocations,
                                &mut context,
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
                OperationKind::Atomic(atomic) => {
                    if let Some(invocations) = access_invocations {
                        match derive_access(
                            location,
                            atomic.pointer,
                            FormalMemoryAccessKind::Atomic,
                            atomic.access,
                            invocations,
                            &mut context,
                        ) {
                            Ok(access) => accesses.push(access),
                            Err(reason) => {
                                reasons.insert(reason);
                            }
                        }
                    }
                }
                OperationKind::Alloca { .. }
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

fn classify_eligible_private_slots(
    function: &Function,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    reasons: &mut BTreeSet<FormalMemoryIncompleteReason>,
) -> BTreeSet<ValueId> {
    let body = function
        .body
        .as_ref()
        .expect("verified kernel entry is defined");
    let blocks = body
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
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
    let exact_slot = |value| {
        definitions
            .exact_ssa_origin(value, value_types)
            .filter(|origin| slots.contains(origin))
    };
    let mut escapes = BTreeMap::<ValueId, (FunctionOperationLocation, ValueId)>::new();
    let record_terminator_escape = |value: ValueId, escapes: &mut BTreeMap<_, _>| {
        if let Some(slot) = exact_slot(value)
            && let Some((_, location)) = definitions.operations.get(&slot)
        {
            escapes.entry(slot).or_insert((*location, value));
        }
    };
    let record_edge_escapes =
        |target: BlockId, arguments: &[ValueId], escapes: &mut BTreeMap<_, _>| {
            let parameters = blocks.get(&target).map(|block| block.parameters.as_slice());
            let exact_edge = parameters.is_some_and(|parameters| {
                arguments.len() == parameters.len()
                    && arguments
                        .iter()
                        .zip(parameters)
                        .all(|(argument, parameter)| {
                            value_types
                                .get(argument)
                                .is_some_and(|ty| ty == &parameter.ty)
                        })
            });
            for (index, argument) in arguments.iter().copied().enumerate() {
                let Some(slot) = exact_slot(argument) else {
                    continue;
                };
                let exact_transport = exact_edge
                    && parameters
                        .and_then(|parameters| parameters.get(index))
                        .is_some_and(|parameter| exact_slot(parameter.id) == Some(slot));
                if !exact_transport && let Some((_, location)) = definitions.operations.get(&slot) {
                    escapes.entry(slot).or_insert((*location, argument));
                }
            }
        };

    for block in body
        .blocks
        .iter()
        .filter(|block| definitions.is_reachable(block.id))
    {
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let location = FunctionOperationLocation::new(block.id, operation_index);
            for operand in operation.kind.operands() {
                let Some(slot) = exact_slot(operand) else {
                    continue;
                };
                let exact_access = match &operation.kind {
                    OperationKind::Load { pointer, access } => {
                        *pointer == operand
                            && access.address_space == AddressSpace::Private
                            && exact_slot(*pointer) == Some(slot)
                    }
                    OperationKind::Store {
                        pointer,
                        value,
                        access,
                    } => {
                        *pointer == operand
                            && access.address_space == AddressSpace::Private
                            && exact_slot(*pointer) == Some(slot)
                            && exact_slot(*value) != Some(slot)
                    }
                    OperationKind::Cast {
                        kind: CastKind::RestrictPointerAccess,
                        value,
                        ..
                    } => {
                        *value == operand
                            && matches!(
                                operation.results.as_slice(),
                                [result] if exact_slot(result.id) == Some(slot)
                            )
                    }
                    _ => false,
                };
                if !exact_access {
                    escapes.entry(slot).or_insert((location, operand));
                }
            }
        }

        let Some(terminator) = &block.terminator else {
            continue;
        };
        match terminator {
            crate::Terminator::Branch { target, arguments } => {
                record_edge_escapes(*target, arguments, &mut escapes);
            }
            crate::Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            } => {
                record_terminator_escape(*condition, &mut escapes);
                record_edge_escapes(*then_target, then_arguments, &mut escapes);
                record_edge_escapes(*else_target, else_arguments, &mut escapes);
            }
            crate::Terminator::Switch {
                selector,
                cases,
                default_target,
                default_arguments,
            } => {
                record_terminator_escape(*selector, &mut escapes);
                for case in cases {
                    record_edge_escapes(case.target, &case.arguments, &mut escapes);
                }
                record_edge_escapes(*default_target, default_arguments, &mut escapes);
            }
            crate::Terminator::IntegerSwitch {
                selector,
                cases,
                default_target,
                default_arguments,
            } => {
                record_terminator_escape(*selector, &mut escapes);
                for case in cases {
                    record_edge_escapes(case.target, &case.arguments, &mut escapes);
                }
                record_edge_escapes(*default_target, default_arguments, &mut escapes);
            }
            crate::Terminator::Return { values } => {
                for value in values {
                    record_terminator_escape(*value, &mut escapes);
                }
            }
            crate::Terminator::Unreachable => {}
        }
    }

    for (_, (location, pointer)) in &escapes {
        reasons.insert(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
            location: *location,
            pointer: *pointer,
        });
    }
    let escaped_slots = escapes.keys().copied().collect::<BTreeSet<_>>();
    slots.difference(&escaped_slots).copied().collect()
}

fn exact_private_alloca_access_origin(
    pointer: ValueId,
    access: MemoryAccess,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    eligible_private_slots: &BTreeSet<ValueId>,
) -> Option<ValueId> {
    if access.address_space != AddressSpace::Private {
        return None;
    }
    definitions
        .exact_ssa_origin(pointer, value_types)
        .filter(|origin| eligible_private_slots.contains(origin))
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

fn collect_definitions(function: &Function) -> Definitions<'_> {
    let mut operations = BTreeMap::new();
    let mut block_parameter_inputs = BTreeMap::new();
    let body = function
        .body
        .as_ref()
        .expect("verified function is defined");
    let control_flow = analyze_control_flow(function)
        .expect("verified function has analyzable bounded control flow");
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
    Definitions {
        operations,
        block_parameter_inputs,
        block_parameter_origins,
        affine_expressions,
        reachable_blocks,
        entry,
    }
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
        let mut current = value;
        let mut visited = BTreeSet::new();
        loop {
            if !visited.insert(current) {
                return None;
            }
            let origin = self.unique_ssa_origin(current)?;
            if origin != current {
                if value_types.get(&current) != value_types.get(&origin) {
                    return None;
                }
                current = origin;
                continue;
            }
            let Some((operation, _)) = self.operations.get(&current) else {
                return Some(current);
            };
            let OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: source,
                to,
            } = &operation.kind
            else {
                return Some(current);
            };
            let (Some(Type::Pointer(from)), Type::Pointer(to_pointer)) =
                (value_types.get(source), to)
            else {
                return None;
            };
            if from.pointee != to_pointer.pointee
                || from.address_space != to_pointer.address_space
                || from.access != AccessMode::ReadWrite
                || to_pointer.access != AccessMode::ReadOnly
            {
                return None;
            }
            current = *source;
        }
    }
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
    let values = inputs.keys().copied().collect::<Vec<_>>();
    let positions = values
        .iter()
        .copied()
        .enumerate()
        .map(|(position, value)| (value, position))
        .collect::<BTreeMap<_, _>>();
    let mut edges = vec![Vec::new(); values.len()];
    let mut reverse_edges = vec![Vec::new(); values.len()];
    let mut local_origins = vec![OriginSummary::Empty; values.len()];
    let mut invalid = vec![false; values.len()];
    for (value, incoming) in inputs {
        let position = positions[value];
        invalid[position] = incoming.is_empty();
        for input in incoming {
            if let Some(dependency) = positions.get(input).copied() {
                edges[position].push(dependency);
                reverse_edges[dependency].push(position);
            } else {
                local_origins[position].include(*input);
            }
        }
        edges[position].sort_unstable();
        edges[position].dedup();
    }

    let mut visited = vec![false; values.len()];
    let mut postorder = Vec::with_capacity(values.len());
    for start in 0..values.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut stack = vec![(start, 0usize)];
        while let Some((node, next_edge)) = stack.pop() {
            if let Some(dependency) = edges[node].get(next_edge).copied() {
                stack.push((node, next_edge + 1));
                if !visited[dependency] {
                    visited[dependency] = true;
                    stack.push((dependency, 0));
                }
            } else {
                postorder.push(node);
            }
        }
    }

    let mut component_of = vec![usize::MAX; values.len()];
    let mut component_count = 0usize;
    for start in postorder.into_iter().rev() {
        if component_of[start] != usize::MAX {
            continue;
        }
        component_of[start] = component_count;
        let mut stack = vec![start];
        while let Some(node) = stack.pop() {
            for dependent in &reverse_edges[node] {
                if component_of[*dependent] == usize::MAX {
                    component_of[*dependent] = component_count;
                    stack.push(*dependent);
                }
            }
        }
        component_count += 1;
    }

    let mut dependencies = vec![BTreeSet::new(); component_count];
    let mut dependents = vec![BTreeSet::new(); component_count];
    let mut component_origins = vec![OriginSummary::Empty; component_count];
    let mut component_invalid = vec![false; component_count];
    for node in 0..values.len() {
        let component = component_of[node];
        component_invalid[component] |= invalid[node];
        match local_origins[node] {
            OriginSummary::One(origin) => component_origins[component].include(origin),
            OriginSummary::Ambiguous => component_invalid[component] = true,
            OriginSummary::Empty => {}
        }
        for dependency in &edges[node] {
            let dependency = component_of[*dependency];
            if component != dependency {
                dependencies[component].insert(dependency);
                dependents[dependency].insert(component);
            }
        }
    }

    let mut remaining_dependencies = dependencies.iter().map(BTreeSet::len).collect::<Vec<_>>();
    let mut pending = remaining_dependencies
        .iter()
        .enumerate()
        .filter_map(|(component, count)| (*count == 0).then_some(component))
        .collect::<Vec<_>>();
    let mut component_results = vec![None; component_count];
    while let Some(component) = pending.pop() {
        let mut summary = component_origins[component];
        let mut failed = component_invalid[component];
        for dependency in &dependencies[component] {
            match component_results[*dependency] {
                Some(origin) => summary.include(origin),
                None => failed = true,
            }
        }
        component_results[component] = match (failed, summary) {
            (false, OriginSummary::One(origin)) => Some(origin),
            _ => None,
        };
        for dependent in &dependents[component] {
            remaining_dependencies[*dependent] -= 1;
            if remaining_dependencies[*dependent] == 0 {
                pending.push(*dependent);
            }
        }
    }

    values
        .into_iter()
        .enumerate()
        .map(|(position, value)| (value, component_results[component_of[position]]))
        .collect()
}

#[derive(Clone, Copy)]
enum AffineWork {
    Enter(ValueId),
    Finish(ValueId),
}

/// Evaluates the supported affine operation graph once. The explicit stack
/// avoids host stack growth on large generated expressions, while `visiting`
/// turns any operation/phi recurrence into a deterministic unsupported result.
fn compute_affine_expressions(
    operations: &BTreeMap<ValueId, (&Operation, FunctionOperationLocation)>,
    block_parameter_origins: &BTreeMap<ValueId, Option<ValueId>>,
) -> BTreeMap<ValueId, Result<AffineExpression, IndexExpressionError>> {
    let mut expressions = BTreeMap::new();
    let mut visiting = BTreeSet::new();
    let roots = operations
        .iter()
        .filter_map(|(value, (operation, _))| {
            matches!(operation.results.as_slice(), [result] if result.ty == Type::INDEX)
                .then_some(*value)
        })
        .collect::<Vec<_>>();

    for root in roots {
        if expressions.contains_key(&root) {
            continue;
        }
        let mut work = vec![AffineWork::Enter(root)];
        while let Some(item) = work.pop() {
            match item {
                AffineWork::Enter(value) => {
                    let Some(value) = block_parameter_origins
                        .get(&value)
                        .copied()
                        .unwrap_or(Some(value))
                    else {
                        continue;
                    };
                    if expressions.contains_key(&value) {
                        continue;
                    }
                    if !visiting.insert(value) {
                        expressions.insert(value, Err(IndexExpressionError::Unsupported));
                        continue;
                    }
                    work.push(AffineWork::Finish(value));
                    let Some((operation, _)) = operations.get(&value) else {
                        continue;
                    };
                    if !matches!(operation.results.as_slice(), [result] if result.ty == Type::INDEX)
                    {
                        continue;
                    }
                    if let OperationKind::Binary { lhs, rhs, .. } = operation.kind {
                        for dependency in [rhs, lhs] {
                            if let Some(dependency) = block_parameter_origins
                                .get(&dependency)
                                .copied()
                                .unwrap_or(Some(dependency))
                            {
                                work.push(AffineWork::Enter(dependency));
                            }
                        }
                    }
                }
                AffineWork::Finish(value) => {
                    visiting.remove(&value);
                    if expressions.contains_key(&value) {
                        continue;
                    }
                    let expression = operations
                        .get(&value)
                        .and_then(|(operation, _)| {
                            matches!(operation.results.as_slice(), [result] if result.ty == Type::INDEX)
                                .then_some(*operation)
                        })
                        .map_or(Err(IndexExpressionError::Unsupported), |operation| {
                            match &operation.kind {
                                OperationKind::Constant(Constant::Index(value)) => {
                                    Ok(AffineExpression::constant(*value))
                                }
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
                                    let operand = |value: ValueId| -> Result<
                                        AffineExpression,
                                        IndexExpressionError,
                                    > {
                                        let value = block_parameter_origins
                                            .get(&value)
                                            .copied()
                                            .unwrap_or(Some(value))
                                            .ok_or(IndexExpressionError::Unsupported)?;
                                        expressions
                                            .get(&value)
                                            .copied()
                                            .unwrap_or(Err(IndexExpressionError::Unsupported))
                                    };
                                    let lhs = operand(*lhs)?;
                                    let rhs = operand(*rhs)?;
                                    match op {
                                        BinaryOp::Add => lhs
                                            .checked_add(rhs)
                                            .ok_or(IndexExpressionError::Overflow),
                                        BinaryOp::Multiply
                                            if lhs.invocation_coefficient == 0 =>
                                        {
                                            rhs.checked_multiply_constant(lhs.constant)
                                                .ok_or(IndexExpressionError::Overflow)
                                        }
                                        BinaryOp::Multiply
                                            if rhs.invocation_coefficient == 0 =>
                                        {
                                            lhs.checked_multiply_constant(rhs.constant)
                                                .ok_or(IndexExpressionError::Overflow)
                                        }
                                        BinaryOp::Multiply => {
                                            Err(IndexExpressionError::Unsupported)
                                        }
                                        _ => Err(IndexExpressionError::Unsupported),
                                    }
                                }
                                _ => Err(IndexExpressionError::Unsupported),
                            }
                        });
                    expressions.insert(value, expression);
                }
            }
        }
    }
    expressions
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
    value_types: &BTreeMap<ValueId, Type>,
    eligible_private_slots: &BTreeSet<ValueId>,
) -> BTreeMap<ValueId, ValueId> {
    let body = function
        .body
        .as_ref()
        .expect("verified function is defined");
    let slots = eligible_private_slots;
    if slots.is_empty() || body.blocks.is_empty() {
        return BTreeMap::new();
    }

    let block_ids = body
        .blocks
        .iter()
        .filter(|block| definitions.is_reachable(block.id))
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let mut predecessors = block_ids
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for block in &body.blocks {
        if !definitions.is_reachable(block.id) {
            continue;
        }
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
            if !definitions.is_reachable(block.id) {
                continue;
            }
            let next_incoming = if block.id == entry {
                Some(initial.clone())
            } else {
                predecessors.get(&block.id).and_then(|blocks| {
                    let mut states = blocks.iter().filter_map(|block| outgoing.get(block));
                    let first = states.next()?.clone();
                    Some(states.fold(first, |mut joined, state| {
                        for slot in slots {
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
            transfer_private_slot_stores(
                block,
                definitions,
                value_types,
                eligible_private_slots,
                &mut next_outgoing,
            );
            changed |= outgoing.get(&block.id) != Some(&next_outgoing);
            outgoing.insert(block.id, next_outgoing);
        }
        if !changed {
            break;
        }
    }

    let mut sources = BTreeMap::new();
    for block in &body.blocks {
        if !definitions.is_reachable(block.id) {
            continue;
        }
        let Some(mut state) = incoming.get(&block.id).cloned() else {
            continue;
        };
        for operation in &block.operations {
            if let OperationKind::Load { pointer, access } = operation.kind
                && let Some(slot) = exact_private_alloca_access_origin(
                    pointer,
                    access,
                    definitions,
                    value_types,
                    eligible_private_slots,
                )
                && let Some(PrivateSlotState::Exact(source)) = state.get(&slot).copied()
            {
                for result in &operation.results {
                    sources.insert(result.id, source);
                }
            }
            transfer_private_slot_store(
                operation,
                definitions,
                value_types,
                eligible_private_slots,
                &mut state,
            );
        }
    }
    sources
}

fn transfer_private_slot_stores(
    block: &crate::BasicBlock,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    eligible_private_slots: &BTreeSet<ValueId>,
    state: &mut BTreeMap<ValueId, PrivateSlotState>,
) {
    for operation in &block.operations {
        transfer_private_slot_store(
            operation,
            definitions,
            value_types,
            eligible_private_slots,
            state,
        );
    }
}

fn transfer_private_slot_store(
    operation: &Operation,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    eligible_private_slots: &BTreeSet<ValueId>,
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
    if let Some(slot) = exact_private_alloca_access_origin(
        pointer,
        access,
        definitions,
        value_types,
        eligible_private_slots,
    ) {
        state.insert(
            slot,
            definitions
                .exact_ssa_origin(value, value_types)
                .map_or(PrivateSlotState::Unknown, PrivateSlotState::Exact),
        );
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

#[derive(Clone, Copy)]
struct PointerExpression {
    allocation: FormalAllocationIdentity,
    byte_offset: AffineExpression,
}

#[derive(Clone)]
enum PointerDerivationFailure {
    AtAccess(ValueId),
    Located(FormalMemoryIncompleteReason),
}

impl PointerDerivationFailure {
    fn materialize(
        &self,
        access_location: FunctionOperationLocation,
    ) -> FormalMemoryIncompleteReason {
        match self {
            Self::AtAccess(pointer) => FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                location: access_location,
                pointer: *pointer,
            },
            Self::Located(reason) => reason.clone(),
        }
    }
}

type CachedPointerDerivation<T> = Result<T, PointerDerivationFailure>;

#[derive(Default)]
struct PointerDerivationCache {
    allocations: BTreeMap<ValueId, CachedPointerDerivation<FormalAllocationIdentity>>,
    expressions: BTreeMap<ValueId, CachedPointerDerivation<PointerExpression>>,
}

fn cache_pointer_allocation_failure(
    origin: ValueId,
    failure: &PointerDerivationFailure,
    reverse_dependencies: &BTreeMap<ValueId, Vec<ValueId>>,
    cache: &mut PointerDerivationCache,
) {
    let mut pending = vec![origin];
    let mut visited = BTreeSet::new();
    while let Some(value) = pending.pop() {
        if !visited.insert(value) {
            continue;
        }
        cache
            .allocations
            .entry(value)
            .or_insert_with(|| Err(failure.clone()));
        pending.extend(
            reverse_dependencies
                .get(&value)
                .into_iter()
                .flatten()
                .copied(),
        );
    }
}

struct AccessDerivationContext<'analysis, 'module> {
    definitions: &'analysis Definitions<'module>,
    value_types: &'analysis BTreeMap<ValueId, Type>,
    allocation_by_value: &'analysis BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &'analysis BTreeMap<ValueId, ValueId>,
    pointer_derivations: PointerDerivationCache,
}

fn derive_access(
    location: FunctionOperationLocation,
    pointer: ValueId,
    kind: FormalMemoryAccessKind,
    access: MemoryAccess,
    invocations: InvocationRange1d,
    context: &mut AccessDerivationContext<'_, '_>,
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
        &mut context.pointer_derivations,
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
    context: &mut AccessDerivationContext<'_, '_>,
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
        &mut context.pointer_derivations,
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
    cache: &mut PointerDerivationCache,
    access_location: FunctionOperationLocation,
) -> Result<FormalAllocationIdentity, FormalMemoryIncompleteReason> {
    derive_pointer_allocation_cached(
        pointer,
        definitions,
        value_types,
        allocation_by_value,
        private_load_sources,
        cache,
    )
    .map_err(|failure| failure.materialize(access_location))
}

fn derive_pointer_allocation_cached(
    pointer: ValueId,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    allocation_by_value: &BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &BTreeMap<ValueId, ValueId>,
    cache: &mut PointerDerivationCache,
) -> CachedPointerDerivation<FormalAllocationIdentity> {
    if let Some(result) = cache.allocations.get(&pointer) {
        return result.clone();
    }

    let mut pending = vec![pointer];
    let mut visited = BTreeSet::new();
    let mut allocations = BTreeSet::new();
    let mut allocation_sources = BTreeSet::new();
    let mut reverse_dependencies = BTreeMap::<ValueId, Vec<ValueId>>::new();
    let mut failure_origin = None;
    let mut failure_covers_visited = false;
    let result = 'derivation: {
        while let Some(current) = pending.pop() {
            if !visited.insert(current) {
                continue;
            }
            if let Some(cached) = cache.allocations.get(&current) {
                match cached {
                    Ok(allocation) => {
                        allocations.insert(*allocation);
                        allocation_sources.insert(current);
                        if allocations.len() > 1 {
                            break 'derivation Err(PointerDerivationFailure::AtAccess(current));
                        }
                    }
                    Err(failure) => {
                        failure_origin = Some(current);
                        break 'derivation Err(failure.clone());
                    }
                }
                continue;
            }
            if let Some(inputs) = definitions.block_parameter_inputs.get(&current) {
                if inputs.is_empty() {
                    failure_origin = Some(current);
                    break 'derivation Err(PointerDerivationFailure::AtAccess(current));
                }
                for input in inputs {
                    reverse_dependencies
                        .entry(*input)
                        .or_default()
                        .push(current);
                }
                pending.extend(inputs.iter().copied());
                continue;
            }
            if let Some(allocation) = allocation_by_value.get(&current).copied()
                && matches!(
                    value_types.get(&current),
                    Some(Type::Pointer(_) | Type::Slice(_))
                )
            {
                allocations.insert(allocation);
                allocation_sources.insert(current);
                if allocations.len() > 1 {
                    break 'derivation Err(PointerDerivationFailure::AtAccess(current));
                }
                continue;
            }
            let Some((operation, definition_location)) = definitions.operations.get(&current)
            else {
                failure_origin = Some(current);
                break 'derivation Err(PointerDerivationFailure::AtAccess(current));
            };
            match &operation.kind {
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value,
                    ..
                } => {
                    reverse_dependencies
                        .entry(*value)
                        .or_default()
                        .push(current);
                    pending.push(*value);
                }
                OperationKind::SliceData { slice } => {
                    reverse_dependencies
                        .entry(*slice)
                        .or_default()
                        .push(current);
                    pending.push(*slice);
                }
                OperationKind::GetElementPointer { base, .. } => {
                    reverse_dependencies.entry(*base).or_default().push(current);
                    pending.push(*base);
                }
                OperationKind::Load { .. } => {
                    let Some(source) = private_load_sources.get(&current).copied() else {
                        failure_origin = Some(current);
                        break 'derivation Err(PointerDerivationFailure::Located(
                            FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                                location: *definition_location,
                                pointer: current,
                            },
                        ));
                    };
                    reverse_dependencies
                        .entry(source)
                        .or_default()
                        .push(current);
                    pending.push(source);
                }
                _ => {
                    failure_origin = Some(current);
                    break 'derivation Err(PointerDerivationFailure::Located(
                        FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                            location: *definition_location,
                            pointer: current,
                        },
                    ));
                }
            }
        }

        let mut allocations = allocations.into_iter();
        let Some(allocation) = allocations.next() else {
            failure_covers_visited = true;
            break 'derivation Err(PointerDerivationFailure::AtAccess(pointer));
        };
        if allocations.next().is_some() {
            break 'derivation Err(PointerDerivationFailure::AtAccess(pointer));
        }
        let mut resolved = allocation_sources.iter().copied().collect::<Vec<_>>();
        let mut has_allocation_path = allocation_sources;
        while let Some(value) = resolved.pop() {
            for dependent in reverse_dependencies.get(&value).into_iter().flatten() {
                if has_allocation_path.insert(*dependent) {
                    resolved.push(*dependent);
                }
            }
        }
        for value in has_allocation_path {
            cache.allocations.entry(value).or_insert(Ok(allocation));
        }
        Ok(allocation)
    };
    if let Err(failure) = &result {
        if failure_covers_visited {
            for value in &visited {
                cache
                    .allocations
                    .entry(*value)
                    .or_insert_with(|| Err(failure.clone()));
            }
        } else if let Some(origin) = failure_origin {
            cache_pointer_allocation_failure(origin, failure, &reverse_dependencies, cache);
        }
    }
    cache.allocations.insert(pointer, result.clone());
    result
}

fn derive_pointer_expression(
    pointer: ValueId,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    allocation_by_value: &BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &BTreeMap<ValueId, ValueId>,
    cache: &mut PointerDerivationCache,
    access_location: FunctionOperationLocation,
) -> Result<PointerExpression, FormalMemoryIncompleteReason> {
    derive_pointer_expression_cached(
        pointer,
        definitions,
        value_types,
        allocation_by_value,
        private_load_sources,
        cache,
    )
    .map_err(|failure| failure.materialize(access_location))
}

fn derive_pointer_expression_cached(
    pointer: ValueId,
    definitions: &Definitions<'_>,
    value_types: &BTreeMap<ValueId, Type>,
    allocation_by_value: &BTreeMap<ValueId, FormalAllocationIdentity>,
    private_load_sources: &BTreeMap<ValueId, ValueId>,
    cache: &mut PointerDerivationCache,
) -> CachedPointerDerivation<PointerExpression> {
    if let Some(result) = cache.expressions.get(&pointer) {
        return result.clone();
    }

    #[derive(Clone, Copy)]
    enum PointerWork {
        Enter(ValueId),
        Alias {
            value: ValueId,
            source: ValueId,
        },
        Gep {
            value: ValueId,
            base: ValueId,
            offset: ValueId,
            location: FunctionOperationLocation,
        },
    }

    let unsupported = PointerDerivationFailure::AtAccess;
    let mut visiting = BTreeSet::new();
    let mut work = vec![PointerWork::Enter(pointer)];
    while let Some(item) = work.pop() {
        match item {
            PointerWork::Enter(unresolved) => {
                if cache.expressions.contains_key(&unresolved) {
                    continue;
                }
                let Some(value) = definitions.unique_ssa_origin(unresolved) else {
                    cache
                        .expressions
                        .insert(unresolved, Err(unsupported(unresolved)));
                    continue;
                };
                if value != unresolved {
                    work.push(PointerWork::Alias {
                        value: unresolved,
                        source: value,
                    });
                    work.push(PointerWork::Enter(value));
                    continue;
                }
                if !visiting.insert(value) {
                    cache.expressions.insert(value, Err(unsupported(value)));
                    continue;
                }
                if let Some(allocation) = allocation_by_value.get(&value).copied()
                    && matches!(value_types.get(&value), Some(Type::Pointer(_)))
                {
                    visiting.remove(&value);
                    cache.expressions.insert(
                        value,
                        Ok(PointerExpression {
                            allocation,
                            byte_offset: AffineExpression::ZERO,
                        }),
                    );
                    continue;
                }
                let Some((operation, definition_location)) = definitions.operations.get(&value)
                else {
                    visiting.remove(&value);
                    cache.expressions.insert(value, Err(unsupported(value)));
                    continue;
                };
                match &operation.kind {
                    OperationKind::Cast {
                        kind: CastKind::RestrictPointerAccess,
                        value: source,
                        ..
                    } => {
                        work.push(PointerWork::Alias {
                            value,
                            source: *source,
                        });
                        work.push(PointerWork::Enter(*source));
                    }
                    OperationKind::SliceData { slice } => {
                        visiting.remove(&value);
                        let expression = derive_pointer_allocation_cached(
                            *slice,
                            definitions,
                            value_types,
                            allocation_by_value,
                            private_load_sources,
                            cache,
                        )
                        .map(|allocation| PointerExpression {
                            allocation,
                            byte_offset: AffineExpression::ZERO,
                        });
                        cache.expressions.insert(value, expression);
                    }
                    OperationKind::GetElementPointer { base, offset } => {
                        work.push(PointerWork::Gep {
                            value,
                            base: *base,
                            offset: *offset,
                            location: *definition_location,
                        });
                        work.push(PointerWork::Enter(*base));
                    }
                    OperationKind::Load { .. } if private_load_sources.contains_key(&value) => {
                        let Some(source) = private_load_sources.get(&value).copied() else {
                            visiting.remove(&value);
                            cache.expressions.insert(
                                value,
                                Err(PointerDerivationFailure::Located(
                                    FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                                        location: *definition_location,
                                        pointer: value,
                                    },
                                )),
                            );
                            continue;
                        };
                        work.push(PointerWork::Alias { value, source });
                        work.push(PointerWork::Enter(source));
                    }
                    _ => {
                        visiting.remove(&value);
                        cache.expressions.insert(
                            value,
                            Err(PointerDerivationFailure::Located(
                                FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                                    location: *definition_location,
                                    pointer: value,
                                },
                            )),
                        );
                    }
                }
            }
            PointerWork::Alias { value, source } => {
                visiting.remove(&value);
                if cache.expressions.contains_key(&value) {
                    continue;
                }
                let expression = cache
                    .expressions
                    .get(&source)
                    .cloned()
                    .unwrap_or_else(|| Err(unsupported(source)));
                cache.expressions.insert(value, expression);
            }
            PointerWork::Gep {
                value,
                base,
                offset,
                location,
            } => {
                visiting.remove(&value);
                if cache.expressions.contains_key(&value) {
                    continue;
                }
                let expression = (|| {
                    let base_expression = cache
                        .expressions
                        .get(&base)
                        .cloned()
                        .unwrap_or_else(|| Err(unsupported(base)))?;
                    let element_width = value_types.get(&base).and_then(pointer_byte_width).ok_or(
                        PointerDerivationFailure::Located(
                            FormalMemoryIncompleteReason::ElementWidthUnavailable {
                                location,
                                pointer: base,
                            },
                        ),
                    )?;
                    let index = derive_affine_index(offset, definitions).map_err(|error| {
                        PointerDerivationFailure::Located(match error {
                            IndexExpressionError::Unsupported => {
                                FormalMemoryIncompleteReason::UnsupportedIndexExpression {
                                    location,
                                    index: offset,
                                    allocation: base_expression.allocation,
                                }
                            }
                            IndexExpressionError::Overflow => {
                                FormalMemoryIncompleteReason::AddressArithmeticOverflow { location }
                            }
                        })
                    })?;
                    let byte_delta = index.checked_multiply_constant(element_width).ok_or(
                        PointerDerivationFailure::Located(
                            FormalMemoryIncompleteReason::AddressArithmeticOverflow { location },
                        ),
                    )?;
                    let byte_offset = base_expression.byte_offset.checked_add(byte_delta).ok_or(
                        PointerDerivationFailure::Located(
                            FormalMemoryIncompleteReason::AddressArithmeticOverflow { location },
                        ),
                    )?;
                    Ok(PointerExpression {
                        allocation: base_expression.allocation,
                        byte_offset,
                    })
                })();
                cache.expressions.insert(value, expression);
            }
        }
    }
    cache
        .expressions
        .get(&pointer)
        .cloned()
        .unwrap_or_else(|| Err(unsupported(pointer)))
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
                envelope.writes |= access.kind != FormalMemoryAccessKind::Read;
            })
            .or_insert(AllocationEnvelope {
                range,
                writes: access.kind != FormalMemoryAccessKind::Read,
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
                || (left.kind == FormalMemoryAccessKind::Atomic
                    && right.kind == FormalMemoryAccessKind::Atomic)
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
