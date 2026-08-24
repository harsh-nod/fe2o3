//! Generic projection from admitted semantic MIR into safety-verifiable ranked PLIRON.
//!
//! Static proof facts come from indexed places and semantic array types.
//! Dynamic slice facts come only from canonical Rust bounds asserts whose
//! success edge uniquely controls an access to the same slice and index.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
};

use dialect_gpu::{AddressSpaceAttr, HierarchyAttr, MemoryOrderAttr, MemoryScopeAttr};
use dialect_kernel::{
    AccessKindAttr, AtomicOrderingAttr, AtomicScopeAttr, DYNAMIC_EXTENT, IndexBinaryKindAttr,
    MAX_DETERMINISTIC_JOIN_INPUTS_V1, MAX_RANKED_MEMORY_RANK, MemorySpaceAttr,
    SUPPORTED_ELEMENT_WIDTHS, TensorConvergenceAttr,
};
use fe2o3_kernel_analysis::{
    MAX_RANKED_BOUNDS_BLOCKS, MAX_RANKED_BOUNDS_EDGES, MAX_RANKED_BOUNDS_OPERATIONS,
};
use fe2o3_lower_mir_kernel::{
    ProductionRankedAccessSourceV1, ProductionRankedSemanticProjectionReceiptV1,
    ProductionSemanticKirErrorV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiPassModeV1, SemanticAbiPointeeKindV1, SemanticAggregateKindV1,
    SemanticAssertMessageV1, SemanticAtomicAccessV1, SemanticAtomicOrderingV1,
    SemanticAtomicScopeV1, SemanticBinaryOpV1, SemanticBlockIdV1, SemanticCallableDeclV1,
    SemanticCallableIdV1, SemanticCastKindV1, SemanticCompilerIntrinsicOperationV1,
    SemanticConstantValueV1, SemanticDirectCallV1, SemanticDirectTailCallV1,
    SemanticDisjointIndexSpaceV1, SemanticFunctionDeclV1, SemanticFunctionRoleV1,
    SemanticLocalIdV1, SemanticLocalRoleV1, SemanticMfmaAccumulatorContractV1,
    SemanticMfmaAccumulatorDistributionV1, SemanticMfmaOperandContractV1,
    SemanticMfmaOperandRoleV1, SemanticMfmaProfileV1, SemanticMfmaRegisterDistributionV1,
    SemanticMfmaStorageLayoutV1, SemanticOperandV1, SemanticPlaceV1, SemanticProjectionKindV1,
    SemanticRvalueKindV1, SemanticScalarTypeV1, SemanticSourceArgumentOwnershipV1,
    SemanticSourceProvenanceV1, SemanticStatementKindV1, SemanticTargetArchitectureV1,
    SemanticTerminatorKindV1, SemanticTypeDeclV1, SemanticTypeIdV1, SemanticTypeShapeV1,
    SemanticUnaryOpV1, SemanticUnwindActionV1,
};
use fe2o3_mir_model::{
    SemanticEnumPayloadAvailabilityV1, SemanticEnumPayloadDominanceV1,
    SemanticOptionAvailabilityV1, SemanticOptionDominanceV1, semantic_option_producers_v1,
};

use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedCompileErrorV1,
    ProductionRankedKernelErrorV1, ProductionRankedKernelV1, ProductionRankedOperationV1,
    ProductionRankedTerminatorV1, ProductionRankedValueIdV1, ProductionRankedValueV1,
    ProductionSemanticMirErrorV1, ProductionSemanticMirOwnerV1, ProductionSessionErrorV1,
    ProductionSessionLimitsV1, compile_ranked_kernel_for_lowering_v1,
};

const ROOT_NAME_V1: &str = "semantic_safety_module";
// Leave one operation for the ranked function terminator.
const MAX_PROJECTED_OPERATIONS_V1: usize = MAX_RANKED_BOUNDS_OPERATIONS - 1;
// Diagnostics are retained only until this bounded projection is consumed.
const MAX_PROJECTED_RANKED_IR_BYTES_V1: usize = MAX_RANKED_BOUNDS_OPERATIONS * 256;
const MAX_PROJECTED_TENSOR_STATE_ENTRIES_V1: usize = MAX_RANKED_BOUNDS_OPERATIONS * 4;
const MAX_PROJECTED_TENSOR_DATAFLOW_WORK_V1: usize = MAX_RANKED_BOUNDS_OPERATIONS * 16;
const MAX_PROJECTED_TENSOR_ENUM_DEPTH_V1: usize = 8;
const MAX_PROJECTED_LOOP_GRAPH_WORK_V1: usize =
    MAX_RANKED_BOUNDS_BLOCKS * (MAX_RANKED_BOUNDS_BLOCKS + MAX_RANKED_BOUNDS_EDGES);
const PRIVATE_ALLOCATION_ORIGIN_TAG_V1: u64 = 1_u64 << 63;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProjectedAccessSourceV1 {
    block: usize,
    operation: usize,
    access: AccessKindAttr,
    memory_space: MemorySpaceAttr,
    source: SemanticSourceProvenanceV1,
    semantic_site: Option<ProjectedSemanticAccessSiteV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedSemanticAccessSiteV1 {
    block: usize,
    statement: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GuardedRankedAccessV1 {
    view: ProductionRankedValueIdV1,
    indices: Vec<ProductionRankedValueV1>,
    comparisons: Vec<(ProductionRankedValueV1, ProductionRankedValueV1)>,
    access: AccessKindAttr,
    memory_space: MemorySpaceAttr,
    source: SemanticSourceProvenanceV1,
    semantic_site: Option<ProjectedSemanticAccessSiteV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GuardPredicateV1 {
    comparisons: Vec<(ProductionRankedValueV1, ProductionRankedValueV1)>,
}

impl GuardPredicateV1 {
    fn for_access(access: &GuardedRankedAccessV1) -> Self {
        Self {
            comparisons: access.comparisons.clone(),
        }
    }

    fn from_precondition(
        precondition: Option<(ProductionRankedValueV1, ProductionRankedValueV1)>,
    ) -> Self {
        Self {
            comparisons: precondition.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GuardedAccessSiteV1 {
    insertion_operation: usize,
    access: GuardedRankedAccessV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedDisjointIndexV1 {
    value: ProductionRankedValueV1,
    mapping: SemanticDisjointIndexSpaceV1,
    precondition: Option<(ProductionRankedValueV1, ProductionRankedValueV1)>,
    availability: Option<CapabilityAvailabilityV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CapabilityAvailabilityV1 {
    Option(SemanticOptionAvailabilityV1),
    EnumPayload(SemanticEnumPayloadAvailabilityV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedGridLeaderV1 {
    grid_leader: SemanticTypeIdV1,
    precondition: (ProductionRankedValueV1, ProductionRankedValueV1),
    availability: CapabilityAvailabilityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CapabilityEdgeKindV1 {
    Alias,
    AuthenticatedOptionPayload,
    AuthenticatedEnumPayload {
        construction_block: usize,
        availability: SemanticEnumPayloadAvailabilityV1,
    },
    IntoDisjoint {
        mapping: SemanticDisjointIndexSpaceV1,
    },
    CheckedShift {
        mapping: SemanticDisjointIndexSpaceV1,
        offset: u64,
        availability: SemanticOptionAvailabilityV1,
    },
    CheckedBlock {
        mapping: SemanticDisjointIndexSpaceV1,
        elements_per_lane: u64,
        availability: SemanticOptionAvailabilityV1,
    },
    CheckedTiled2d {
        mapping: SemanticDisjointIndexSpaceV1,
        availability: SemanticOptionAvailabilityV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingEnumPayloadStoreV1 {
    carrier: usize,
    variant: u32,
    source: usize,
    construction_block: usize,
    statement: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingEnumPayloadLoadV1 {
    carrier: usize,
    variant: u32,
    destination: usize,
    use_block: usize,
    statement: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CapabilityEdgeV1 {
    destination: usize,
    use_block: usize,
    kind: CapabilityEdgeKindV1,
}

struct IntrinsicProjectionV1 {
    local_contracts: ProjectionLocalContractsV1,
    guarded_accesses: Vec<GuardedRankedAccessV1>,
    option_predicates: Vec<Option<GuardPredicateV1>>,
    direct_switch_predicates: Vec<Option<GuardPredicateV1>>,
    deterministic_switches: Vec<Option<ProjectedDeterministicSwitchV1>>,
    uniform_inductions: Vec<ProjectedUniformInductionV1>,
    tensor_layouts: Vec<Option<ProductionRankedOperationV1>>,
    tensor_read_effects: Vec<Option<ProjectedTensorReadEffectV1>>,
    read_view_effects: Vec<Option<GuardedRankedAccessV1>>,
    extent_argument_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectedDeterministicSwitchV1 {
    discriminant: ProductionRankedValueV1,
    targets: Vec<(ProductionRankedValueV1, usize)>,
    otherwise: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DeterministicScalarSummaryV1 {
    Constant(u64),
    Exact(ProductionRankedValueV1),
    Derived(Vec<ProductionRankedValueV1>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeterministicScalarDefinitionV1 {
    Assignment { block: usize, statement: usize },
    Call { block: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectedUniformInductionV1 {
    preheader: usize,
    header: usize,
    body_entry: usize,
    latch: usize,
    exit: usize,
    loop_blocks: Vec<usize>,
    initial: ProductionRankedValueV1,
    bound: ProductionRankedValueV1,
    step: ProductionRankedValueV1,
}

impl ProjectedUniformInductionV1 {
    fn contains_block(&self, block: usize) -> bool {
        self.loop_blocks.binary_search(&block).is_ok()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedMfmaViewV1 {
    role: SemanticMfmaOperandRoleV1,
    storage_layout: SemanticMfmaStorageLayoutV1,
    allocation: AllocationContractV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedMfmaOperandV1 {
    contract: SemanticMfmaOperandContractV1,
    storage_layout: SemanticMfmaStorageLayoutV1,
    lane_root: u64,
    allocation: AllocationContractV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedTensorReadEffectV1 {
    allocation: AllocationContractV1,
    source: SemanticSourceProvenanceV1,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ProjectedTensorTerminatorEffectsV1 {
    layout: Option<ProductionRankedOperationV1>,
    global_read: Option<AllocationContractV1>,
    read_view: Option<ProjectedReadViewAccessV1>,
}

struct ProjectedTensorEffectsV1 {
    layouts: Vec<Option<ProductionRankedOperationV1>>,
    global_reads: Vec<Option<AllocationContractV1>>,
    read_views: Vec<Option<ProjectedReadViewAccessV1>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectedReadValueV1 {
    Constant(u64),
    Local(SemanticLocalIdV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedReadViewV1 {
    root: u64,
    element: SemanticTypeIdV1,
    allocation: AllocationContractV1,
    rows: ProjectedReadValueV1,
    columns: ProjectedReadValueV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedReadViewAccessV1 {
    view: ProjectedReadViewV1,
    row: ProjectedReadValueV1,
    column: ProjectedReadValueV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedMfmaAccumulatorV1 {
    contract: SemanticMfmaAccumulatorContractV1,
    lane_root: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectedTensorOriginV1 {
    MatrixContext { root: u64 },
    Lane { root: u64, wave_width: u32 },
    ViewResult(ProjectedMfmaViewV1),
    View(ProjectedMfmaViewV1),
    Operand(ProjectedMfmaOperandV1),
    Accumulator(ProjectedMfmaAccumulatorV1),
    ReadViewResult(ProjectedReadViewV1),
    ReadView(ProjectedReadViewV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProjectedTensorValueV1 {
    Known(ProjectedTensorOriginV1),
    ConstructedEnum(ProjectedTensorEnumEnvelopeV1),
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedTensorEnumEnvelopeV1 {
    origin: ProjectedTensorOriginV1,
    // Innermost to outermost. The fixed bound keeps every dataflow state Copy-sized.
    variants: [u32; MAX_PROJECTED_TENSOR_ENUM_DEPTH_V1],
    depth: u8,
}

type ProjectedTensorStateV1 = HashMap<usize, ProjectedTensorValueV1>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AllocationContractV1 {
    allocation_origin: u64,
    noalias_class: u64,
    writable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalProvenanceV1 {
    stable_argument_origins: Vec<Option<u32>>,
    allocation_origins: Vec<Option<u32>>,
}

struct ProjectionLocalContractsV1 {
    checked_reference_origins: Vec<Option<usize>>,
    allocations: Vec<Option<AllocationContractV1>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedBoundsCheckV1 {
    access_block: usize,
    slice_local: SemanticLocalIdV1,
    index_local: SemanticLocalIdV1,
    index: ProductionRankedValueV1,
    extent: ProductionRankedValueV1,
}

struct ProjectedBoundsChecksV1 {
    checks: Vec<ProjectedBoundsCheckV1>,
    argument_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectedViewV1 {
    result: ProductionRankedValueIdV1,
    element_width: u32,
    writable: bool,
    shape: Vec<u64>,
    dynamic_extents: Vec<ProductionRankedValueV1>,
    memory_space: MemorySpaceAttr,
    allocation_origin: u64,
    noalias_class: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProjectedEffectSourceV1 {
    access: AccessKindAttr,
    memory_space: MemorySpaceAttr,
    source: SemanticSourceProvenanceV1,
    semantic_site: Option<ProjectedSemanticAccessSiteV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProjectedBlockItemV1 {
    Effect {
        operation: ProductionRankedOperationV1,
        source: Option<ProjectedEffectSourceV1>,
    },
    Guarded(GuardedRankedAccessV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectedSemanticBlockV1 {
    items: Vec<ProjectedBlockItemV1>,
}

impl ProjectedSemanticBlockV1 {
    fn has_memory_access(&self) -> bool {
        self.items.iter().any(|item| match item {
            ProjectedBlockItemV1::Effect {
                operation:
                    ProductionRankedOperationV1::Access { .. }
                    | ProductionRankedOperationV1::AtomicAccess { .. }
                    | ProductionRankedOperationV1::AllocationEffect { .. },
                ..
            }
            | ProjectedBlockItemV1::Guarded(_) => true,
            ProjectedBlockItemV1::Effect { .. } => false,
        })
    }

    fn has_concurrent_memory_access(&self) -> bool {
        self.items.iter().any(|item| match item {
            ProjectedBlockItemV1::Effect {
                source: Some(source),
                ..
            } => source.memory_space != MemorySpaceAttr::Private,
            ProjectedBlockItemV1::Guarded(_) => true,
            ProjectedBlockItemV1::Effect { source: None, .. } => false,
        })
    }
}

/// Move-only result retaining both the exact admitted Rust semantics and the
/// owner-held PLIRON graph that passed every mandatory generic kernel check.
pub(crate) struct ProductionRankedSemanticProgramV1 {
    receipt: ProductionRankedSemanticProjectionReceiptV1,
}

/// Move-only custody of the exact ranked graph and all seven mandatory
/// production checks. Only the production projection can construct this owner.
#[must_use = "dropping ranked verification abandons its production lineage"]
pub(crate) struct AuthenticatedRankedVerificationV4 {
    middle_end_evidence: fe2o3_pliron::ProductionMiddleEndEvidenceV4,
}

impl AuthenticatedRankedVerificationV4 {
    pub(crate) fn ranked_ir(&self) -> &str {
        self.middle_end_evidence.ranked_ir()
    }

    pub(crate) const fn middle_end_evidence(&self) -> &fe2o3_pliron::ProductionMiddleEndEvidenceV4 {
        &self.middle_end_evidence
    }
}

impl ProductionRankedSemanticProgramV1 {
    pub(crate) fn ranked_ir(&self) -> &str {
        self.receipt.ranked_ir()
    }

    pub(crate) fn function_name(&self) -> &str {
        self.receipt.lowering().kernel().function_name()
    }

    pub(crate) fn semantic_function_count(&self) -> usize {
        self.receipt.semantic().semantic().functions().len()
    }

    pub(crate) fn semantic_callable_count(&self) -> usize {
        self.receipt.semantic().semantic().callables().len()
    }

    pub(crate) fn bounds_are_clean(&self) -> bool {
        self.receipt.lowering().bounds_report().is_clean()
    }

    pub(crate) fn all_kernel_checks_are_clean(&self) -> bool {
        self.receipt.lowering().all_mandatory_reports_are_clean()
    }

    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    pub(crate) fn into_verified_receipt(
        self,
    ) -> Result<
        (
            ProductionRankedSemanticProjectionReceiptV1,
            AuthenticatedRankedVerificationV4,
        ),
        fe2o3_pliron::ProductionMiddleEndEvidenceCodecErrorV4,
    > {
        let middle_end_evidence = fe2o3_pliron::ProductionMiddleEndEvidenceV4::try_new(
            self.receipt.semantic(),
            self.receipt.lowering(),
            self.receipt.ranked_ir(),
        )?;
        Ok((
            self.receipt,
            AuthenticatedRankedVerificationV4 {
                middle_end_evidence,
            },
        ))
    }
}

#[derive(Debug)]
pub(crate) enum ProductionRankedProjectionErrorV1 {
    SemanticOwner(ProductionSemanticMirErrorV1),
    Incomplete(&'static str),
    UnprovenAssert {
        block: usize,
        kind: &'static str,
        expected: bool,
        condition_local: Option<u32>,
        source: SemanticSourceProvenanceV1,
    },
    Unsupported(&'static str),
    Recipe(ProductionRankedKernelErrorV1),
    Custody(ProductionSemanticKirErrorV1),
    Construction(fe2o3_pliron::NameError),
    Compile {
        error: ProductionRankedCompileErrorV1,
        ranked_ir: String,
        access_sources: Vec<ProjectedAccessSourceV1>,
    },
}

impl fmt::Display for ProductionRankedProjectionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemanticOwner(error) => {
                write!(formatter, "exact semantic middle end failed: {error}")
            }
            Self::Custody(error) => write!(formatter, "ranked proof custody failed: {error}"),
            Self::Unsupported(detail) => {
                write!(formatter, "semantic-to-ranked projection rejected {detail}")
            }
            Self::Incomplete(detail) => {
                write!(
                    formatter,
                    "semantic-to-ranked projection incomplete: {detail}"
                )
            }
            Self::UnprovenAssert {
                block,
                kind,
                expected,
                condition_local,
                source,
            } => write!(
                formatter,
                "semantic-to-ranked projection incomplete: Rust {kind} assert terminator in semantic block bb{block} at {} expected condition{} to be {}; no exact dominating proof establishes it on every incoming path",
                source_label(*source),
                condition_local
                    .map(|local| format!(" local {local}"))
                    .unwrap_or_default(),
                expected,
            ),
            Self::Recipe(error) => write!(formatter, "semantic-to-ranked recipe failed: {error}"),
            Self::Construction(error) => write!(
                formatter,
                "semantic-to-ranked construction name was rejected: {error:?}",
            ),
            Self::Compile {
                error,
                ranked_ir,
                access_sources,
            } => {
                error.fmt(formatter)?;
                if let ProductionRankedCompileErrorV1::Session(
                    ProductionSessionErrorV1::RankedBounds(bounds),
                ) = error
                {
                    for finding in bounds.report().findings() {
                        if let fe2o3_kernel_analysis::RankedBoundsFindingV1::StaticOutOfBounds {
                            block,
                            operation,
                            ..
                        }
                        | fe2o3_kernel_analysis::RankedBoundsFindingV1::UnprovedBound {
                            block,
                            operation,
                            ..
                        } = finding
                            && let Some(access) = access_sources.iter().find(|source| {
                                source.block == *block && source.operation == *operation
                            })
                        {
                            write!(formatter, "\n  --> {}", source_label(access.source))?;
                            write!(
                                formatter,
                                "\n  = Rust {:?} projected to kernel.access at block {} op {}",
                                access.access, access.block, access.operation,
                            )?;
                        }
                    }
                }
                write!(
                    formatter,
                    "\n  = ranked PLIRON before rejected lowering:\n{}\n  = lowering stopped before target IR or artifact emission",
                    indent_ir(ranked_ir),
                )
            }
        }
    }
}

impl std::error::Error for ProductionRankedProjectionErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SemanticOwner(error) => Some(error),
            Self::Recipe(error) => Some(error),
            Self::Compile { error, .. } => Some(error),
            Self::Incomplete(_)
            | Self::UnprovenAssert { .. }
            | Self::Unsupported(_)
            | Self::Construction(_) => None,
            Self::Custody(error) => Some(error),
        }
    }
}

pub(crate) fn project_and_verify_ranked_semantic_mir_v1(
    semantic_owner: ProductionSemanticMirOwnerV1,
    source_rank: u8,
) -> Result<ProductionRankedSemanticProgramV1, ProductionRankedProjectionErrorV1> {
    semantic_owner
        .verify_equivalence()
        .map_err(ProductionRankedProjectionErrorV1::SemanticOwner)?;
    let semantic = semantic_owner.semantic();
    let selection = semantic.select_kernel_body_v1().ok_or(
        ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic closure that is neither one kernel root nor one transparent Result wrapper",
        ),
    )?;
    let root_function = semantic
        .functions()
        .get(selection.root().index() as usize)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "an out-of-range semantic kernel root",
        ))?;
    let function = semantic
        .functions()
        .get(selection.body().index() as usize)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "an out-of-range semantic kernel body",
        ))?;
    if root_function.role() != SemanticFunctionRoleV1::KernelRoot {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a root without the KernelRoot role",
        ));
    }

    let constants = constant_locals(function);
    let mut entry_operations = vec![source_execution_layout_v1(
        semantic.target().architecture(),
        root_function,
        source_rank,
    )?];
    let mut next_value = 0_u32;
    let mut incomplete = None;
    let mut projected_views = vec![None; function.locals().len()];
    let mut discarded_ir = String::new();
    let intrinsic = project_intrinsic_contracts(
        semantic.callables(),
        semantic.types(),
        function,
        &constants,
        &mut entry_operations,
        &mut next_value,
        &mut discarded_ir,
    )?;
    let bounds_checks = project_rust_bounds_checks(
        function,
        intrinsic.extent_argument_count,
        &mut entry_operations,
        &mut next_value,
    )?;
    let switch_predicates = switch_predicates(
        function,
        &intrinsic.option_predicates,
        &intrinsic.direct_switch_predicates,
    )?;
    let mut projected_blocks = Vec::new();
    let mut projected_effect_count = 0_usize;
    for (block_index, block) in function.blocks().iter().enumerate() {
        let mut operations = Vec::new();
        let mut guarded_sites = Vec::new();
        let mut local_sources = Vec::new();
        for (statement_index, statement) in block.statements().iter().enumerate() {
            let source_start = local_sources.len();
            let guarded_start = guarded_sites.len();
            retain_incomplete(
                project_statement_accesses(
                    semantic.types(),
                    function,
                    block_index,
                    &bounds_checks.checks,
                    statement,
                    &constants,
                    &intrinsic.local_contracts,
                    &intrinsic.guarded_accesses,
                    &mut guarded_sites,
                    &mut projected_views,
                    &mut operations,
                    &mut local_sources,
                    &mut next_value,
                    &mut discarded_ir,
                ),
                &mut incomplete,
            )?;
            bind_projected_access_site(
                &mut local_sources[source_start..],
                &mut guarded_sites[guarded_start..],
                ProjectedSemanticAccessSiteV1 {
                    block: block_index,
                    statement: Some(statement_index),
                },
            )?;
        }
        let source_start = local_sources.len();
        let guarded_start = guarded_sites.len();
        retain_incomplete(
            project_terminator_accesses(
                semantic.callables(),
                semantic.types(),
                function,
                block_index,
                &bounds_checks.checks,
                block.terminator().kind(),
                block.terminator().source(),
                &constants,
                &intrinsic.local_contracts,
                &intrinsic.guarded_accesses,
                &mut guarded_sites,
                &mut projected_views,
                &mut operations,
                &mut local_sources,
                &mut next_value,
                &mut discarded_ir,
            ),
            &mut incomplete,
        )?;
        if let Some(tensor_layout) = intrinsic.tensor_layouts.get(block_index).cloned().flatten() {
            reserve_operation(&mut operations)?;
            // This records contract consistency for the mandatory verifier. It
            // carries no load-refinement or artifact authority by itself; that
            // authority is joined later with the exact semantic owner and KIR.
            operations.push(tensor_layout);
        }
        if let Some(effect) = intrinsic
            .tensor_read_effects
            .get(block_index)
            .copied()
            .flatten()
        {
            let operation = operations.len();
            reserve_operation(&mut operations)?;
            operations.push(ProductionRankedOperationV1::AllocationEffect {
                kind: AccessKindAttr::Read,
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: effect.allocation.allocation_origin,
                noalias_class: effect.allocation.noalias_class,
            });
            local_sources.push(ProjectedAccessSourceV1 {
                block: block_index,
                operation,
                access: AccessKindAttr::Read,
                memory_space: MemorySpaceAttr::Global,
                source: effect.source,
                semantic_site: None,
            });
        }
        if let Some(access) = intrinsic
            .read_view_effects
            .get(block_index)
            .cloned()
            .flatten()
        {
            guarded_sites.try_reserve(1).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "strided read access-site storage cannot be reserved",
                )
            })?;
            guarded_sites.push(GuardedAccessSiteV1 {
                insertion_operation: operations.len(),
                access,
            });
        }
        bind_projected_access_site(
            &mut local_sources[source_start..],
            &mut guarded_sites[guarded_start..],
            ProjectedSemanticAccessSiteV1 {
                block: block_index,
                statement: None,
            },
        )?;
        let projected = order_projected_block_effects(
            operations,
            guarded_sites,
            local_sources,
            &mut entry_operations,
        )?;
        projected_effect_count = projected_effect_count
            .checked_add(projected.items.len())
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG projection operation count overflow",
            ))?;
        if projected_effect_count
            .checked_add(entry_operations.len())
            .is_none_or(|count| count > MAX_RANKED_BOUNDS_OPERATIONS)
        {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG projection exceeds the ranked operation limit",
            ));
        }
        projected_blocks.push(projected);
    }
    if bounds_checks.checks.iter().any(|check| {
        projected_blocks
            .get(check.access_block)
            .is_none_or(|block| !projected_block_uses_bounds_check(block, *check))
    }) {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a Rust bounds assertion does not authorize one matching projected access",
        ));
    }
    if !projected_blocks
        .iter()
        .any(ProjectedSemanticBlockV1::has_memory_access)
    {
        if let Some(detail) = incomplete {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(detail));
        }
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a kernel without a statically ranked indexed memory access",
        ));
    }
    if projected_blocks
        .iter()
        .any(ProjectedSemanticBlockV1::has_concurrent_memory_access)
        && !entry_operations.iter().any(|operation| {
            matches!(
                operation,
                ProductionRankedOperationV1::InvocationIndex { .. }
            )
        })
    {
        incomplete.get_or_insert(
            "a concurrent memory effect before exact invocation-index projection is available",
        );
    }
    let (blocks, sources) = build_ranked_cfg(
        semantic.types(),
        function,
        &switch_predicates,
        &intrinsic.deterministic_switches,
        &intrinsic.uniform_inductions,
        entry_operations,
        projected_blocks,
    )?;
    let access_sources = production_access_sources(&blocks, &sources)?;
    let ranked_ir = format_ranked_cfg(function_name(root_function)?, &blocks)?;
    let kernel = ProductionRankedKernelV1::new(
        function_name(root_function)?,
        bounds_checks.argument_count,
        blocks,
    )
    .map_err(ProductionRankedProjectionErrorV1::Recipe)?;
    let construction = ProductionConstructionV1::ranked_kernel(ROOT_NAME_V1, kernel)
        .map_err(ProductionRankedProjectionErrorV1::Construction)?;
    let lowering =
        compile_ranked_kernel_for_lowering_v1(construction, ProductionSessionLimitsV1::default())
            .map_err(|error| ProductionRankedProjectionErrorV1::Compile {
            error,
            ranked_ir: ranked_ir.clone(),
            access_sources: sources,
        })?;
    if let Some(detail) = incomplete {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(detail));
    }
    let receipt = ProductionRankedSemanticProjectionReceiptV1::assert_compiler_internal_projection(
        semantic_owner,
        lowering,
        ranked_ir,
        access_sources,
    )
    .map_err(ProductionRankedProjectionErrorV1::Custody)?;
    Ok(ProductionRankedSemanticProgramV1 { receipt })
}

fn production_access_sources(
    blocks: &[ProductionRankedBlockV1],
    sources: &[ProjectedAccessSourceV1],
) -> Result<Vec<ProductionRankedAccessSourceV1>, ProductionRankedProjectionErrorV1> {
    let mut ordinals = HashMap::<(usize, Option<usize>), u32>::new();
    let mut retained = Vec::new();
    retained.try_reserve(sources.len()).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "ranked access correspondence storage cannot be reserved",
        )
    })?;
    for source in sources {
        let operation = blocks
            .get(source.block)
            .and_then(|block| block.operations().get(source.operation))
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "ranked access correspondence is outside the projected graph",
            ))?;
        if !matches!(
            operation,
            ProductionRankedOperationV1::Access { .. }
                | ProductionRankedOperationV1::AtomicAccess { .. }
        ) {
            continue;
        }
        let site = source
            .semantic_site
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "ranked access correspondence has no exact semantic site",
            ))?;
        let ordinal = ordinals.entry((site.block, site.statement)).or_default();
        retained.push(ProductionRankedAccessSourceV1::new(
            u32::try_from(site.block).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "semantic access block does not fit u32",
                )
            })?,
            site.statement.map(u32::try_from).transpose().map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "semantic access statement does not fit u32",
                )
            })?,
            *ordinal,
            u32::try_from(source.block).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "ranked access block does not fit u32",
                )
            })?,
            u32::try_from(source.operation).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "ranked access operation does not fit u32",
                )
            })?,
        ));
        *ordinal = ordinal
            .checked_add(1)
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic access ordinal overflow",
            ))?;
    }
    Ok(retained)
}

fn project_rust_bounds_checks(
    function: &SemanticFunctionDeclV1,
    first_argument: usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
) -> Result<ProjectedBoundsChecksV1, ProductionRankedProjectionErrorV1> {
    #[derive(Clone, Copy, Default)]
    struct LocalDefinitionV1 {
        count: u32,
        length_source: Option<SemanticLocalIdV1>,
    }

    let mut definitions = vec![LocalDefinitionV1::default(); function.locals().len()];
    let mut predecessors = vec![Vec::new(); function.blocks().len()];
    for (block_index, block) in function.blocks().iter().enumerate() {
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() {
                continue;
            }
            let definition = definitions
                .get_mut(assignment.destination().local().index() as usize)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "a Rust bounds-check definition outside the semantic local table",
                ))?;
            definition.count = definition.count.saturating_add(1);
            definition.length_source = match assignment.value().kind() {
                SemanticRvalueKindV1::Length(place) => Some(place.local()),
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand,
                } => simple_operand_local(operand),
                _ => None,
            };
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
            && destination.place().projections().is_empty()
        {
            let definition = definitions
                .get_mut(destination.place().local().index() as usize)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "a Rust bounds-check call result outside the semantic local table",
                ))?;
            definition.count = definition.count.saturating_add(1);
            definition.length_source = None;
        }
        block
            .terminator()
            .kind()
            .try_for_each_edge::<ProductionRankedProjectionErrorV1>(|edge| {
                let target = edge.target().index() as usize;
                let target_predecessors = predecessors.get_mut(target).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "a Rust bounds-check CFG edge outside the semantic block table",
                    ),
                )?;
                target_predecessors.push(block_index);
                Ok(())
            })?;
    }

    let mut local_values = vec![None; function.locals().len()];
    let mut checks = Vec::new();
    for (block_index, block) in function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            message: SemanticAssertMessageV1::BoundsCheck { length, index },
            target,
            unwind,
        } = block.terminator().kind()
        else {
            continue;
        };
        if !*expected || !matches!(unwind, SemanticUnwindActionV1::Unreachable) {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a Rust bounds check without the canonical success/unreachable shape",
            ));
        }
        let condition_local = simple_operand_local(condition).ok_or(
            ProductionRankedProjectionErrorV1::Incomplete(
                "a Rust bounds-check condition without one exact local",
            ),
        )?;
        let index_local =
            simple_operand_local(index).ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "a Rust bounds-check index without one exact local",
            ))?;
        let length_local =
            simple_operand_local(length).ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "a Rust bounds-check length without one exact local",
            ))?;
        let condition_definition = definitions.get(condition_local.index() as usize).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a Rust bounds-check condition outside the semantic local table",
            ),
        )?;
        let index_definition = definitions.get(index_local.index() as usize).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a Rust bounds-check index outside the semantic local table",
            ),
        )?;
        let length_definition = definitions.get(length_local.index() as usize).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a Rust bounds-check length outside the semantic local table",
            ),
        )?;
        let index_is_immutable_argument = index_definition.count == 0
            && matches!(
                function.locals()[index_local.index() as usize].role(),
                SemanticLocalRoleV1::Argument(_)
            );
        if condition_definition.count != 1
            || (index_definition.count != 1 && !index_is_immutable_argument)
            || length_definition.count != 1
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a Rust bounds check whose condition, index, or length is not stable",
            ));
        }
        let slice_local = length_definition.length_source.ok_or(
            ProductionRankedProjectionErrorV1::Incomplete(
                "a Rust bounds-check length not derived from one exact slice",
            ),
        )?;
        let authentic_comparison = block.statements().iter().rev().find_map(|statement| {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                return None;
            };
            if !assignment.destination().projections().is_empty()
                || assignment.destination().local() != condition_local
            {
                return None;
            }
            match assignment.value().kind() {
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left,
                    right,
                } => Some(
                    simple_operand_local(left) == Some(index_local)
                        && simple_operand_local(right) == Some(length_local),
                ),
                _ => Some(false),
            }
        });
        if authentic_comparison != Some(true) {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a Rust bounds-check message not backed by its exact index < length condition",
            ));
        }
        let access_block = target.target().index() as usize;
        if predecessors.get(access_block).map(Vec::as_slice) != Some(&[block_index]) {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a Rust bounds-check success block not uniquely controlled by that check",
            ));
        }
        let mut unknown_for = |local: SemanticLocalIdV1| {
            let slot = local_values.get_mut(local.index() as usize).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "a Rust bounds-check operand outside the semantic local table",
                ),
            )?;
            if let Some(value) = *slot {
                return Ok(value);
            }
            let result = ProductionRankedValueIdV1::new(*next_value);
            *next_value =
                next_value
                    .checked_add(1)
                    .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                        "dynamic ranked-analysis value count overflow",
                    ))?;
            reserve_operation(operations)?;
            operations.push(ProductionRankedOperationV1::IndexUnknown { result });
            let value = ProductionRankedValueV1::Local(result);
            *slot = Some(value);
            Ok(value)
        };
        checks.try_reserve(1).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "Rust bounds-check projection storage cannot be reserved",
            )
        })?;
        checks.push(ProjectedBoundsCheckV1 {
            access_block,
            slice_local,
            index_local,
            index: unknown_for(index_local)?,
            extent: unknown_for(length_local)?,
        });
    }
    Ok(ProjectedBoundsChecksV1 {
        checks,
        argument_count: first_argument,
    })
}

fn project_authenticated_tensor_layouts_v1(
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
    local_allocations: &[Option<AllocationContractV1>],
    constants: &[Option<u64>],
) -> Result<ProjectedTensorEffectsV1, ProductionRankedProjectionErrorV1> {
    let block_count = function.blocks().len();
    let entry = function.entry().index() as usize;
    if entry >= block_count {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a tensor projection entry outside the semantic CFG",
        ));
    }
    let mut entries: Vec<Option<ProjectedTensorStateV1>> = vec![None; block_count];
    entries[entry] = Some(HashMap::new());
    let mut worklist = VecDeque::from([entry]);
    let mut stored_entries = 0_usize;
    let mut work = 0_usize;
    while let Some(block_index) = worklist.pop_front() {
        let entry_state = entries.get(block_index).and_then(Option::as_ref).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a queued tensor producer block without an entry state",
            ),
        )?;
        charge_tensor_dataflow_work_v1(
            &mut work,
            entry_state.len().checked_add(1).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "tensor producer clone work overflow",
                ),
            )?,
        )?;
        let mut state = entry_state.clone();
        transfer_tensor_statements_v1(function, block_index, &mut state, enum_payload_dominance)?;
        transfer_tensor_terminator_v1(
            callables,
            function,
            block_index,
            &mut state,
            local_allocations,
            constants,
            false,
        )?;
        charge_tensor_dataflow_work_v1(
            &mut work,
            function.blocks()[block_index]
                .statements()
                .len()
                .checked_add(state.len())
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "tensor producer dataflow work overflow",
                ))?,
        )?;
        if state.len() > MAX_PROJECTED_TENSOR_STATE_ENTRIES_V1 {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "tensor producer dataflow exceeds the charged projection limit",
            ));
        }
        let successors = charged_unique_tensor_successors_v1(
            function.blocks()[block_index].terminator().kind(),
            state.len(),
            &mut work,
        )?;
        for target in successors {
            let target_entry =
                entries
                    .get_mut(target)
                    .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                        "a tensor producer CFG edge outside the semantic function",
                    ))?;
            let changed = match target_entry {
                None => {
                    stored_entries = stored_entries.checked_add(state.len()).ok_or(
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "tensor producer stored-state accounting overflow",
                        ),
                    )?;
                    *target_entry = Some(state.clone());
                    true
                }
                Some(existing) => {
                    let before = existing.len();
                    charge_tensor_dataflow_work_v1(
                        &mut work,
                        before.checked_add(1).ok_or(
                            ProductionRankedProjectionErrorV1::Unsupported(
                                "tensor producer merge work overflow",
                            ),
                        )?,
                    )?;
                    let changed = merge_tensor_states_v1(existing, &state)?;
                    stored_entries = stored_entries
                        .checked_add(existing.len().saturating_sub(before))
                        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                            "tensor producer stored-state accounting overflow",
                        ))?;
                    changed
                }
            };
            if stored_entries > MAX_PROJECTED_TENSOR_STATE_ENTRIES_V1 {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "tensor producer states exceed the charged storage limit",
                ));
            }
            if changed {
                worklist.push_back(target);
            }
        }
    }

    let mut layouts = vec![None; block_count];
    let mut global_reads = vec![None; block_count];
    let mut read_views = vec![None; block_count];
    for (block_index, entry_state) in entries.into_iter().enumerate() {
        let Some(mut state) = entry_state else {
            continue;
        };
        charge_tensor_dataflow_work_v1(
            &mut work,
            state
                .len()
                .checked_add(function.blocks()[block_index].statements().len())
                .and_then(|work| work.checked_add(1))
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "tensor producer replay work overflow",
                ))?,
        )?;
        transfer_tensor_statements_v1(function, block_index, &mut state, enum_payload_dominance)?;
        let effects = transfer_tensor_terminator_v1(
            callables,
            function,
            block_index,
            &mut state,
            local_allocations,
            constants,
            true,
        )?;
        layouts[block_index] = effects.layout;
        global_reads[block_index] = effects.global_read;
        read_views[block_index] = effects.read_view;
    }
    Ok(ProjectedTensorEffectsV1 {
        layouts,
        global_reads,
        read_views,
    })
}

fn bind_tensor_read_effects_to_call_blocks_v1(
    function: &SemanticFunctionDeclV1,
    global_reads: &[Option<AllocationContractV1>],
) -> Result<Vec<Option<ProjectedTensorReadEffectV1>>, ProductionRankedProjectionErrorV1> {
    if global_reads.len() != function.blocks().len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "tensor read effects do not correspond one-to-one with semantic MIR blocks",
        ));
    }
    let mut effects = Vec::new();
    effects.try_reserve_exact(global_reads.len()).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "tensor read effect storage cannot be reserved",
        )
    })?;
    for (block, allocation) in function.blocks().iter().zip(global_reads.iter().copied()) {
        effects.push(allocation.map(|allocation| ProjectedTensorReadEffectV1 {
            allocation,
            source: block.terminator().source(),
        }));
    }
    Ok(effects)
}

fn project_read_value_to_ranked_v1(
    value: ProjectedReadValueV1,
    stable_argument_origins: &[Option<u32>],
    arguments: &mut [Option<u32>],
    next_argument: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
) -> Result<ProductionRankedValueV1, ProductionRankedProjectionErrorV1> {
    match value {
        ProjectedReadValueV1::Constant(value) => {
            reserve_operation(operations)?;
            let result = next_value_id(next_value)?;
            operations.push(ProductionRankedOperationV1::IndexConstant { result, value });
            Ok(ProductionRankedValueV1::Local(result))
        }
        ProjectedReadValueV1::Local(local) => {
            let local_index = local.index() as usize;
            let origin = stable_argument_origins
                .get(local_index)
                .copied()
                .flatten()
                .unwrap_or(local.index()) as usize;
            let slot =
                arguments
                    .get_mut(origin)
                    .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                        "a strided read index origin outside the semantic local table",
                    ))?;
            let argument = match *slot {
                Some(argument) => argument,
                None => {
                    let argument = u32::try_from(*next_argument).map_err(|_| {
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "too many strided read ranked arguments",
                        )
                    })?;
                    *next_argument = next_argument.checked_add(1).ok_or(
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "strided read ranked argument count overflow",
                        ),
                    )?;
                    *slot = Some(argument);
                    argument
                }
            };
            Ok(ProductionRankedValueV1::Argument(argument))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn project_strided_read_effects_v1(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    effects: &[Option<ProjectedReadViewAccessV1>],
    stable_argument_origins: &[Option<u32>],
    arguments: &mut [Option<u32>],
    next_argument: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
) -> Result<Vec<Option<GuardedRankedAccessV1>>, ProductionRankedProjectionErrorV1> {
    if effects.len() != function.blocks().len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "strided read effects do not correspond one-to-one with semantic MIR blocks",
        ));
    }
    let mut views: HashMap<u64, (ProjectedReadViewV1, ProjectedViewV1)> = HashMap::new();
    let mut projected = Vec::new();
    projected.try_reserve_exact(effects.len()).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "strided read effect storage cannot be reserved",
        )
    })?;
    for (block, effect) in function.blocks().iter().zip(effects.iter().copied()) {
        let Some(effect) = effect else {
            projected.push(None);
            continue;
        };
        if effect.view.allocation.writable {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a checked shared read view rooted in writable allocation authority",
            ));
        }
        let (view, rows, columns) = if let Some((source, view)) = views.get(&effect.view.root) {
            if *source != effect.view {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "one checked read-view origin changed its element, extent, or allocation contract",
                ));
            }
            let [rows, columns] = view.dynamic_extents.as_slice() else {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a checked read view without exactly two dynamic extents",
                ));
            };
            (view.result, *rows, *columns)
        } else {
            let rows = project_read_value_to_ranked_v1(
                effect.view.rows,
                stable_argument_origins,
                arguments,
                next_argument,
                operations,
                next_value,
            )?;
            let columns = project_read_value_to_ranked_v1(
                effect.view.columns,
                stable_argument_origins,
                arguments,
                next_argument,
                operations,
                next_value,
            )?;
            reserve_operation(operations)?;
            let result = next_value_id(next_value)?;
            let view = ProjectedViewV1 {
                result,
                element_width: type_width(types, effect.view.element)?,
                writable: false,
                shape: vec![DYNAMIC_EXTENT, DYNAMIC_EXTENT],
                dynamic_extents: vec![rows, columns],
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: effect.view.allocation.allocation_origin,
                noalias_class: effect.view.allocation.noalias_class,
            };
            operations.push(ProductionRankedOperationV1::ViewInSpace {
                result,
                element_width: view.element_width,
                writable: view.writable,
                shape: view.shape.clone(),
                dynamic_extents: view.dynamic_extents.clone(),
                memory_space: view.memory_space,
                allocation_origin: view.allocation_origin,
                noalias_class: view.noalias_class,
            });
            views.insert(effect.view.root, (effect.view, view));
            (result, rows, columns)
        };
        let row = project_read_value_to_ranked_v1(
            effect.row,
            stable_argument_origins,
            arguments,
            next_argument,
            operations,
            next_value,
        )?;
        let column = project_read_value_to_ranked_v1(
            effect.column,
            stable_argument_origins,
            arguments,
            next_argument,
            operations,
            next_value,
        )?;
        projected.push(Some(GuardedRankedAccessV1 {
            view,
            indices: vec![row, column],
            comparisons: vec![(row, rows), (column, columns)],
            access: AccessKindAttr::Read,
            memory_space: MemorySpaceAttr::Global,
            source: block.terminator().source(),
            semantic_site: None,
        }));
    }
    Ok(projected)
}

fn charge_tensor_dataflow_work_v1(
    work: &mut usize,
    additional: usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    *work = work
        .checked_add(additional)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "tensor producer dataflow work overflow",
        ))?;
    if *work > MAX_PROJECTED_TENSOR_DATAFLOW_WORK_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "tensor producer dataflow exceeds the charged projection limit",
        ));
    }
    Ok(())
}

fn charged_unique_tensor_successors_v1(
    terminator: &SemanticTerminatorKindV1,
    state_entries: usize,
    work: &mut usize,
) -> Result<Vec<usize>, ProductionRankedProjectionErrorV1> {
    let mut unique = HashSet::new();
    terminator.try_for_each_edge::<ProductionRankedProjectionErrorV1>(|edge| {
        charge_tensor_dataflow_work_v1(work, 1)?;
        let target = edge.target().index() as usize;
        if !unique.contains(&target) {
            unique.try_reserve(1).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "tensor producer successor storage cannot be reserved",
                )
            })?;
            unique.insert(target);
        }
        Ok(())
    })?;
    let mut successors = unique.into_iter().collect::<Vec<_>>();
    successors.sort_unstable();
    let merge_work =
        state_entries
            .checked_add(1)
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "tensor producer dataflow work overflow",
            ))?;
    for _ in &successors {
        charge_tensor_dataflow_work_v1(work, merge_work)?;
    }
    Ok(successors)
}

fn transfer_tensor_statements_v1(
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    state: &mut ProjectedTensorStateV1,
    enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let block = &function.blocks()[block_index];
    let use_block = SemanticBlockIdV1::from_index(block_index as u32);
    for statement in block.statements() {
        match statement.kind() {
            SemanticStatementKindV1::Assign(assignment) => {
                let destination = assignment.destination().local().index() as usize;
                let origin = if assignment.destination().projections().is_empty() {
                    match assignment.value().kind() {
                        SemanticRvalueKindV1::Use(operand) => {
                            tensor_origin_from_assignment_operand_v1(
                                operand,
                                state,
                                enum_payload_dominance,
                                use_block,
                            )
                        }
                        SemanticRvalueKindV1::Borrow { place, .. }
                        | SemanticRvalueKindV1::AddressOf { place, .. }
                            if place.projections().is_empty() =>
                        {
                            state.get(&(place.local().index() as usize)).copied()
                        }
                        SemanticRvalueKindV1::Aggregate(aggregate) => {
                            tensor_origin_from_enum_aggregate_v1(
                                aggregate,
                                state,
                                enum_payload_dominance,
                                use_block,
                            )?
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                consume_tensor_rvalue_operands_v1(assignment.value().kind(), state);
                if !assignment.destination().projections().is_empty() {
                    invalidate_tensor_local_v1(state, destination);
                    continue;
                }
                match origin {
                    Some(origin) => {
                        state.insert(destination, origin);
                    }
                    None => {
                        state.remove(&destination);
                    }
                }
            }
            SemanticStatementKindV1::Store(store) => {
                consume_tensor_operand_v1(state, store.value());
            }
            SemanticStatementKindV1::AtomicRmw(atomic) => {
                consume_tensor_operand_v1(state, atomic.value());
                invalidate_tensor_place_v1(state, atomic.destination());
            }
            SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                consume_tensor_operand_v1(state, atomic.expected());
                consume_tensor_operand_v1(state, atomic.replacement());
                invalidate_tensor_place_v1(state, atomic.destination());
            }
            SemanticStatementKindV1::SetDiscriminant { place, .. }
            | SemanticStatementKindV1::Deinitialize(place) => {
                invalidate_tensor_place_v1(state, place);
            }
            SemanticStatementKindV1::StorageLive(local)
            | SemanticStatementKindV1::StorageDead(local) => {
                state.remove(&(local.index() as usize));
            }
            SemanticStatementKindV1::Assume(operand) => {
                consume_tensor_operand_v1(state, operand);
            }
            SemanticStatementKindV1::Nop => {}
        }
    }
    Ok(())
}

fn consume_tensor_rvalue_operands_v1(
    rvalue: &SemanticRvalueKindV1,
    state: &mut ProjectedTensorStateV1,
) {
    let _: Result<(), std::convert::Infallible> = rvalue.try_visit_operands(|operand| {
        consume_tensor_operand_v1(state, operand);
        Ok(())
    });
}

fn consume_tensor_operand_v1(state: &mut ProjectedTensorStateV1, operand: &SemanticOperandV1) {
    let place = match operand {
        SemanticOperandV1::Copy(place)
            if place.projections().is_empty()
                && state
                    .get(&(place.local().index() as usize))
                    .is_some_and(projected_value_is_shared_read_v1) =>
        {
            return;
        }
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
        SemanticOperandV1::Constant(_) => return,
    };
    invalidate_tensor_place_v1(state, place);
}

fn projected_value_is_shared_read_v1(value: &ProjectedTensorValueV1) -> bool {
    matches!(
        value,
        ProjectedTensorValueV1::Known(
            ProjectedTensorOriginV1::ReadViewResult(_) | ProjectedTensorOriginV1::ReadView(_)
        ) | ProjectedTensorValueV1::ConstructedEnum(ProjectedTensorEnumEnvelopeV1 {
            origin: ProjectedTensorOriginV1::ReadViewResult(_)
                | ProjectedTensorOriginV1::ReadView(_),
            ..
        })
    )
}

fn consume_tensor_operands_v1(state: &mut ProjectedTensorStateV1, operands: &[SemanticOperandV1]) {
    for operand in operands {
        consume_tensor_operand_v1(state, operand);
    }
}

fn invalidate_tensor_place_v1(state: &mut ProjectedTensorStateV1, place: &SemanticPlaceV1) {
    invalidate_tensor_local_v1(state, place.local().index() as usize);
}

fn invalidate_tensor_local_v1(state: &mut ProjectedTensorStateV1, local: usize) {
    if state.contains_key(&local) {
        state.insert(local, ProjectedTensorValueV1::Invalid);
    }
}

fn tensor_origin_from_enum_aggregate_v1(
    aggregate: &fe2o3_mir_model::semantic_mir_v1::SemanticAggregateRvalueV1,
    state: &ProjectedTensorStateV1,
    enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
    use_block: SemanticBlockIdV1,
) -> Result<Option<ProjectedTensorValueV1>, ProductionRankedProjectionErrorV1> {
    let (SemanticAggregateKindV1::EnumVariant(variant), [payload]) =
        (aggregate.kind(), aggregate.operands())
    else {
        return Ok(None);
    };
    tensor_origin_from_assignment_operand_v1(payload, state, enum_payload_dominance, use_block)
        .map(|payload| wrap_tensor_enum_value_v1(payload, *variant))
        .transpose()
}

fn tensor_origin_from_assignment_operand_v1(
    operand: &SemanticOperandV1,
    state: &ProjectedTensorStateV1,
    enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
    use_block: SemanticBlockIdV1,
) -> Option<ProjectedTensorValueV1> {
    let place = raw_operand_place(operand)?;
    if place.projections().is_empty() {
        return state.get(&(place.local().index() as usize)).copied();
    }
    let (carrier, variant) = enum_payload_projection(place)?;
    match state.get(&carrier).copied()? {
        ProjectedTensorValueV1::ConstructedEnum(envelope) => {
            unwrap_tensor_enum_value_v1(envelope, variant)
        }
        ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::ViewResult(view))
            if variant == 0
                && enum_payload_dominance
                    .availability(SemanticLocalIdV1::from_index(carrier as u32), variant)
                    .is_some_and(|availability| {
                        enum_payload_dominance.allows(availability, use_block)
                    }) =>
        {
            Some(ProjectedTensorValueV1::Known(
                ProjectedTensorOriginV1::View(view),
            ))
        }
        ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::ReadViewResult(view))
            if variant == 0
                && enum_payload_dominance
                    .availability(SemanticLocalIdV1::from_index(carrier as u32), variant)
                    .is_some_and(|availability| {
                        enum_payload_dominance.allows(availability, use_block)
                    }) =>
        {
            Some(ProjectedTensorValueV1::Known(
                ProjectedTensorOriginV1::ReadView(view),
            ))
        }
        ProjectedTensorValueV1::Invalid => Some(ProjectedTensorValueV1::Invalid),
        _ => None,
    }
}

fn wrap_tensor_enum_value_v1(
    payload: ProjectedTensorValueV1,
    variant: u32,
) -> Result<ProjectedTensorValueV1, ProductionRankedProjectionErrorV1> {
    let mut envelope = match payload {
        ProjectedTensorValueV1::Known(origin) => ProjectedTensorEnumEnvelopeV1 {
            origin,
            variants: [0; MAX_PROJECTED_TENSOR_ENUM_DEPTH_V1],
            depth: 0,
        },
        ProjectedTensorValueV1::ConstructedEnum(envelope) => envelope,
        ProjectedTensorValueV1::Invalid => return Ok(ProjectedTensorValueV1::Invalid),
    };
    let depth = usize::from(envelope.depth);
    if depth == MAX_PROJECTED_TENSOR_ENUM_DEPTH_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "tensor enum transport exceeds the charged nesting limit",
        ));
    }
    envelope.variants[depth] = variant;
    envelope.depth =
        envelope
            .depth
            .checked_add(1)
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "tensor enum transport depth overflow",
            ))?;
    Ok(ProjectedTensorValueV1::ConstructedEnum(envelope))
}

fn unwrap_tensor_enum_value_v1(
    mut envelope: ProjectedTensorEnumEnvelopeV1,
    variant: u32,
) -> Option<ProjectedTensorValueV1> {
    let outer = usize::from(envelope.depth).checked_sub(1)?;
    if envelope.variants[outer] != variant {
        return None;
    }
    envelope.variants[outer] = 0;
    envelope.depth -= 1;
    if envelope.depth == 0 {
        Some(ProjectedTensorValueV1::Known(envelope.origin))
    } else {
        Some(ProjectedTensorValueV1::ConstructedEnum(envelope))
    }
}

fn projected_read_value_v1(
    operand: &SemanticOperandV1,
    constants: &[Option<u64>],
) -> Option<ProjectedReadValueV1> {
    constant_operand_value(operand, constants)
        .map(ProjectedReadValueV1::Constant)
        .or_else(|| simple_operand_local(operand).map(ProjectedReadValueV1::Local))
}

fn authenticate_strided_read_v1(
    call: &SemanticDirectCallV1,
    state: &ProjectedTensorStateV1,
    element: SemanticTypeIdV1,
    constants: &[Option<u64>],
) -> Option<ProjectedReadViewAccessV1> {
    if call.arguments().len() != 4 {
        return None;
    }
    let ProjectedTensorOriginV1::ReadView(view) =
        tensor_known_origin_v1(state, &call.arguments()[0])?
    else {
        return None;
    };
    if view.element != element {
        return None;
    }
    Some(ProjectedReadViewAccessV1 {
        view,
        row: projected_read_value_v1(&call.arguments()[1], constants)?,
        column: projected_read_value_v1(&call.arguments()[2], constants)?,
    })
}

fn transfer_tensor_terminator_v1(
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    state: &mut ProjectedTensorStateV1,
    local_allocations: &[Option<AllocationContractV1>],
    constants: &[Option<u64>],
    require_authenticated_site: bool,
) -> Result<ProjectedTensorTerminatorEffectsV1, ProductionRankedProjectionErrorV1> {
    let terminator = function.blocks()[block_index].terminator().kind();
    let SemanticTerminatorKindV1::Call(call) = terminator else {
        match terminator {
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                consume_tensor_operand_v1(state, discriminant);
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                consume_tensor_operands_v1(state, call.arguments());
            }
            SemanticTerminatorKindV1::Drop { place, .. } => {
                invalidate_tensor_place_v1(state, place);
            }
            SemanticTerminatorKindV1::Assert { condition, .. } => {
                consume_tensor_operand_v1(state, condition);
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
            SemanticTerminatorKindV1::Call(_) => unreachable!("matched call terminator"),
        }
        return Ok(ProjectedTensorTerminatorEffectsV1::default());
    };
    let intrinsic_operation = match callables.get(call.callee().index() as usize) {
        Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) => Some(operation),
        _ => None,
    };
    if matches!(
        intrinsic_operation,
        Some(SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad { .. })
    ) {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "the retired Option-returning BF16 matrix load; use Bf16MatrixLoadZeroFilledV2",
        ));
    }
    let is_global_fragment_load = matches!(
        intrinsic_operation,
        Some(SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 { .. })
    );
    if let Some(SemanticCompilerIntrinsicOperationV1::StridedReadView2DLoadOr { element, .. }) =
        intrinsic_operation
    {
        let read_view = authenticate_strided_read_v1(call, state, *element, constants);
        consume_tensor_operands_v1(state, call.arguments());
        if let Some(destination) = call.destination() {
            invalidate_tensor_place_v1(state, destination.place());
            if destination.place().projections().is_empty() {
                state.remove(&(destination.place().local().index() as usize));
            }
        }
        if read_view.is_none() && require_authenticated_site {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a strided read without one dominating checked view payload and exact scalar operands",
            ));
        }
        return Ok(ProjectedTensorTerminatorEffectsV1 {
            read_view,
            ..ProjectedTensorTerminatorEffectsV1::default()
        });
    }
    let Some(destination) = call.destination() else {
        consume_tensor_operands_v1(state, call.arguments());
        if is_global_fragment_load {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a typed global fragment load without one direct local result",
            ));
        }
        return Ok(ProjectedTensorTerminatorEffectsV1::default());
    };
    if !destination.place().projections().is_empty() {
        consume_tensor_operands_v1(state, call.arguments());
        invalidate_tensor_place_v1(state, destination.place());
        if is_global_fragment_load {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a typed global fragment load into a projected destination",
            ));
        }
        return Ok(ProjectedTensorTerminatorEffectsV1::default());
    }
    let destination_local = destination.place().local().index() as usize;
    let Some(operation) = intrinsic_operation else {
        consume_tensor_operands_v1(state, call.arguments());
        state.remove(&destination_local);
        return Ok(ProjectedTensorTerminatorEffectsV1::default());
    };
    if matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_)) {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a typed tensor producer with cleanup control flow",
        ));
    }
    let root = ((block_index as u64) << 32) | destination_local as u64;
    let (origin, layout) = match operation {
        SemanticCompilerIntrinsicOperationV1::StridedReadView2DFromSharedSlice {
            result,
            element,
            ..
        } => {
            let allocation = call
                .arguments()
                .first()
                .and_then(transparent_operand_place)
                .and_then(|place| local_allocations.get(place.local().index() as usize))
                .copied()
                .flatten();
            let rows = call
                .arguments()
                .get(2)
                .and_then(|operand| projected_read_value_v1(operand, constants));
            let columns = call
                .arguments()
                .get(3)
                .and_then(|operand| projected_read_value_v1(operand, constants));
            let origin = match (allocation, rows, columns) {
                (Some(allocation), Some(rows), Some(columns))
                    if call.arguments().len() == 5
                        && destination.place().ty() == *result
                        && !allocation.writable =>
                {
                    ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::ReadViewResult(
                        ProjectedReadViewV1 {
                            root,
                            element: *element,
                            allocation,
                            rows,
                            columns,
                        },
                    ))
                }
                _ => ProjectedTensorValueV1::Invalid,
            };
            (origin, None)
        }
        SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { context } => {
            if !call.arguments().is_empty() || destination.place().ty() != *context {
                (ProjectedTensorValueV1::Invalid, None)
            } else {
                (
                    ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::MatrixContext { root }),
                    None,
                )
            }
        }
        SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent { lane, wave_width } => {
            if !call.arguments().is_empty() || destination.place().ty() != *lane {
                (ProjectedTensorValueV1::Invalid, None)
            } else {
                (
                    ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Lane {
                        root,
                        wave_width: *wave_width,
                    }),
                    None,
                )
            }
        }
        SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewRowMajor {
            result,
            role,
            storage_layout,
            ..
        } => {
            let allocation = call
                .arguments()
                .first()
                .and_then(transparent_operand_place)
                .and_then(|place| local_allocations.get(place.local().index() as usize))
                .copied()
                .flatten();
            let origin = match allocation {
                Some(allocation)
                    if call.arguments().len() == 5 && destination.place().ty() == *result =>
                {
                    ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::ViewResult(
                        ProjectedMfmaViewV1 {
                            role: *role,
                            storage_layout: *storage_layout,
                            allocation,
                        },
                    ))
                }
                _ => ProjectedTensorValueV1::Invalid,
            };
            (origin, None)
        }
        SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 {
            fragment,
            contract,
            storage_layout,
            ..
        } => (
            project_tensor_load_origin_v1(
                call,
                state,
                destination.place().ty(),
                *fragment,
                *contract,
                *storage_layout,
            ),
            None,
        ),
        SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
            fragment,
            contract,
            ..
        } => {
            let lane = call
                .arguments()
                .first()
                .and_then(|operand| tensor_known_origin_v1(state, operand));
            let origin = match lane {
                Some(ProjectedTensorOriginV1::Lane {
                    root: lane_root,
                    wave_width,
                }) if call.arguments().len() == 1
                    && destination.place().ty() == *fragment
                    && wave_width == contract.wave_width =>
                {
                    ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Accumulator(
                        ProjectedMfmaAccumulatorV1 {
                            contract: *contract,
                            lane_root,
                        },
                    ))
                }
                _ => ProjectedTensorValueV1::Invalid,
            };
            (origin, None)
        }
        SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
            accumulator_fragment,
            lhs,
            rhs,
            accumulator,
            ..
        } => match authenticate_tensor_instruction_v1(call, state, *lhs, *rhs, *accumulator) {
            Ok((accumulator_origin, contract))
                if destination.place().ty() == *accumulator_fragment =>
            {
                (
                    ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Accumulator(
                        accumulator_origin,
                    )),
                    Some(ProductionRankedOperationV1::TensorLayout {
                        contract,
                        convergence: TensorConvergenceAttr::UniformSubgroup,
                        active_lanes: u32::from(contract.subgroup_width),
                    }),
                )
            }
            Ok(_) => (ProjectedTensorValueV1::Invalid, None),
            Err(detail) if require_authenticated_site => {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(detail));
            }
            Err(_) => (ProjectedTensorValueV1::Invalid, None),
        },
        _ => {
            consume_tensor_operands_v1(state, call.arguments());
            state.remove(&destination_local);
            return Ok(ProjectedTensorTerminatorEffectsV1::default());
        }
    };
    consume_tensor_operands_v1(state, call.arguments());
    state.insert(destination_local, origin);
    if require_authenticated_site
        && matches!(
            operation,
            SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate { .. }
        )
        && layout.is_none()
    {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "an MFMA call whose result type does not match its authenticated accumulator contract",
        ));
    }
    let global_read = match (is_global_fragment_load, origin) {
        (true, ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Operand(fragment))) => {
            Some(fragment.allocation)
        }
        (true, _) => {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a typed global fragment load without exact authenticated view, lane, allocation, and result provenance",
            ));
        }
        (false, _) => None,
    };
    Ok(ProjectedTensorTerminatorEffectsV1 {
        layout,
        global_read,
        read_view: None,
    })
}

fn authenticate_tensor_load_v1(
    call: &SemanticDirectCallV1,
    state: &ProjectedTensorStateV1,
    contract: SemanticMfmaOperandContractV1,
    storage_layout: SemanticMfmaStorageLayoutV1,
) -> Option<ProjectedMfmaOperandV1> {
    if call.arguments().len() != 4 {
        return None;
    }
    let view = tensor_known_origin_v1(state, &call.arguments()[0])?;
    let lane = tensor_known_origin_v1(state, &call.arguments()[1])?;
    let ProjectedTensorOriginV1::View(view) = view else {
        return None;
    };
    let ProjectedTensorOriginV1::Lane {
        root: lane_root,
        wave_width,
    } = lane
    else {
        return None;
    };
    (view.role == contract.role
        && view.storage_layout == storage_layout
        && wave_width == contract.wave_width)
        .then_some(ProjectedMfmaOperandV1 {
            contract,
            storage_layout,
            lane_root,
            allocation: view.allocation,
        })
}

#[allow(clippy::too_many_arguments)]
fn project_tensor_load_origin_v1(
    call: &SemanticDirectCallV1,
    state: &ProjectedTensorStateV1,
    destination_type: SemanticTypeIdV1,
    expected_output_type: SemanticTypeIdV1,
    contract: SemanticMfmaOperandContractV1,
    storage_layout: SemanticMfmaStorageLayoutV1,
) -> ProjectedTensorValueV1 {
    if destination_type != expected_output_type {
        return ProjectedTensorValueV1::Invalid;
    }
    let Some(fragment) = authenticate_tensor_load_v1(call, state, contract, storage_layout) else {
        return ProjectedTensorValueV1::Invalid;
    };
    ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Operand(fragment))
}

fn authenticate_tensor_instruction_v1(
    call: &SemanticDirectCallV1,
    state: &ProjectedTensorStateV1,
    lhs_contract: SemanticMfmaOperandContractV1,
    rhs_contract: SemanticMfmaOperandContractV1,
    accumulator_contract: SemanticMfmaAccumulatorContractV1,
) -> Result<
    (
        ProjectedMfmaAccumulatorV1,
        fe2o3_kernel_ir::TensorLayoutContractV1,
    ),
    &'static str,
> {
    if call.arguments().len() != 4 {
        return Err("an MFMA call without its exact context and three typed operands");
    }
    if !matches!(
        tensor_known_origin_v1(state, &call.arguments()[0]),
        Some(ProjectedTensorOriginV1::MatrixContext { .. })
    ) {
        return Err("an MFMA call without dominating compiler-issued matrix context");
    }
    let Some(ProjectedTensorOriginV1::Operand(lhs)) =
        tensor_known_origin_v1(state, &call.arguments()[1])
    else {
        return Err("an MFMA lhs without one dominating checked typed-load payload");
    };
    let Some(ProjectedTensorOriginV1::Operand(rhs)) =
        tensor_known_origin_v1(state, &call.arguments()[2])
    else {
        return Err("an MFMA rhs without one dominating checked typed-load payload");
    };
    let Some(ProjectedTensorOriginV1::Accumulator(accumulator)) =
        tensor_known_origin_v1(state, &call.arguments()[3])
    else {
        return Err("an MFMA accumulator without dominating zero or compatible prior MFMA");
    };
    if lhs.contract != lhs_contract || rhs.contract != rhs_contract {
        return Err("an MFMA call whose operand metadata does not match its exact load producers");
    }
    if accumulator.contract != accumulator_contract {
        return Err("an MFMA call whose accumulator metadata changed from its producer");
    }
    if lhs_contract.role != SemanticMfmaOperandRoleV1::A
        || rhs_contract.role != SemanticMfmaOperandRoleV1::B
    {
        return Err("an MFMA call with swapped or incompatible operand roles");
    }
    if lhs_contract.profile != SemanticMfmaProfileV1::Bf16F32M16N16K16
        || rhs_contract.profile != lhs_contract.profile
        || accumulator_contract.profile != lhs_contract.profile
    {
        return Err("an MFMA call with incompatible instruction profiles");
    }
    if lhs_contract.register_distribution != SemanticMfmaRegisterDistributionV1::Tile16x16
        || rhs_contract.register_distribution != SemanticMfmaRegisterDistributionV1::Tile16x16
        || accumulator_contract.distribution != SemanticMfmaAccumulatorDistributionV1::RowMajor
    {
        return Err("an MFMA call with incompatible register distributions");
    }
    if lhs_contract.wave_width != 64
        || rhs_contract.wave_width != 64
        || accumulator_contract.wave_width != 64
        || lhs.lane_root != rhs.lane_root
        || lhs.lane_root != accumulator.lane_root
    {
        return Err("an MFMA call whose operands do not share one authenticated wave64 lane");
    }
    let mut contract =
        fe2o3_kernel_ir::TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
            .with_zero_filled_predicate_inputs();
    if lhs.storage_layout == SemanticMfmaStorageLayoutV1::LdsXor4 {
        contract = contract.with_a_lds_xor4();
    }
    if rhs.storage_layout == SemanticMfmaStorageLayoutV1::LdsXor4 {
        contract = contract.with_b_lds_xor4();
    }
    Ok((accumulator, contract))
}

fn tensor_known_origin_v1(
    state: &ProjectedTensorStateV1,
    operand: &SemanticOperandV1,
) -> Option<ProjectedTensorOriginV1> {
    let local = simple_operand_local(operand)?.index() as usize;
    match state.get(&local) {
        Some(ProjectedTensorValueV1::Known(origin)) => Some(*origin),
        Some(ProjectedTensorValueV1::ConstructedEnum(_))
        | Some(ProjectedTensorValueV1::Invalid)
        | None => None,
    }
}

fn merge_tensor_states_v1(
    current: &mut ProjectedTensorStateV1,
    incoming: &ProjectedTensorStateV1,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    let mut changed = false;
    for (&key, existing) in current.iter_mut() {
        let merged = match incoming.get(&key).copied() {
            Some(candidate) if *existing == candidate => *existing,
            Some(_) | None => ProjectedTensorValueV1::Invalid,
        };
        if *existing != merged {
            *existing = merged;
            changed = true;
        }
    }
    for (&key, &candidate) in incoming {
        if let std::collections::hash_map::Entry::Vacant(entry) = current.entry(key) {
            entry.insert(match candidate {
                ProjectedTensorValueV1::Known(_)
                | ProjectedTensorValueV1::ConstructedEnum(_)
                | ProjectedTensorValueV1::Invalid => ProjectedTensorValueV1::Invalid,
            });
            changed = true;
        }
    }
    Ok(changed)
}

fn project_intrinsic_contracts(
    callables: &[SemanticCallableDeclV1],
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    constants: &[Option<u64>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<IntrinsicProjectionV1, ProductionRankedProjectionErrorV1> {
    reject_retired_production_intrinsics_v1(callables)?;
    let local_count = function.locals().len();
    let mut index_values = vec![None; local_count];
    let mut grid_leaders = vec![None; local_count];
    let mut option_predicates = vec![None; local_count];
    let mut edges_by_source = vec![Vec::new(); local_count];
    let mut enum_payload_stores = Vec::new();
    let mut enum_payload_loads = Vec::new();
    let option_producers = semantic_option_producers_v1(function, callables)
        .map_err(|error| ProductionRankedProjectionErrorV1::Unsupported(error.detail()))?;
    let option_dominance = SemanticOptionDominanceV1::analyze(function, &option_producers)
        .map_err(|error| ProductionRankedProjectionErrorV1::Unsupported(error.detail()))?;
    let enum_payload_dominance = SemanticEnumPayloadDominanceV1::analyze(function, types)
        .map_err(|error| ProductionRankedProjectionErrorV1::Unsupported(error.detail()))?;
    let LocalProvenanceV1 {
        stable_argument_origins,
        allocation_origins,
    } = local_provenance_v1(function)?;
    let local_allocations = local_allocation_contracts(types, function, &allocation_origins)?;
    let tensor_effects = project_authenticated_tensor_layouts_v1(
        callables,
        function,
        &enum_payload_dominance,
        &local_allocations,
        constants,
    )?;
    let tensor_read_effects =
        bind_tensor_read_effects_to_call_blocks_v1(function, &tensor_effects.global_reads)?;
    let tensor_layouts = tensor_effects.layouts;
    let read_view_sources = tensor_effects.read_views;
    let mut edge_count = 0_usize;
    let mut borrowed_locals = Vec::new();
    let mut runtime_index_arguments = vec![None; local_count];
    let mut next_runtime_argument = 1_usize;
    let read_view_effects = project_strided_read_effects_v1(
        types,
        function,
        &read_view_sources,
        &stable_argument_origins,
        &mut runtime_index_arguments,
        &mut next_runtime_argument,
        operations,
        next_value,
    )?;
    // Workgroup geometry and the runtime grid domain are independent. The
    // source launch contract authenticates the former; the latter remains
    // dynamic until host launch evidence is joined.
    let launch_extent = 0;
    let local_definitions = local_definition_counts(function);

    for (block_index, block) in function.blocks().iter().enumerate() {
        for (statement_index, statement) in block.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() {
                continue;
            }
            if let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind()
                && let SemanticAggregateKindV1::EnumVariant(variant) = aggregate.kind()
                && let [operand] = aggregate.operands()
                && let Some(source) = transparent_operand_place(operand)
            {
                if enum_payload_stores.len() == MAX_PROJECTED_OPERATIONS_V1 {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "single-payload enum stores exceed the charged projection limit",
                    ));
                }
                enum_payload_stores.try_reserve(1).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "single-payload enum store storage cannot be reserved",
                    )
                })?;
                enum_payload_stores.push(PendingEnumPayloadStoreV1 {
                    carrier: assignment.destination().local().index() as usize,
                    variant: *variant,
                    source: source.local().index() as usize,
                    construction_block: block_index,
                    statement: statement_index,
                });
                continue;
            }
            if let SemanticRvalueKindV1::Use(operand) = assignment.value().kind()
                && let Some(place) = raw_operand_place(operand)
                && let Some((carrier, variant)) = enum_payload_projection(place)
            {
                if enum_payload_loads.len() == MAX_PROJECTED_OPERATIONS_V1 {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "single-payload enum loads exceed the charged projection limit",
                    ));
                }
                enum_payload_loads.try_reserve(1).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "single-payload enum load storage cannot be reserved",
                    )
                })?;
                enum_payload_loads.push(PendingEnumPayloadLoadV1 {
                    carrier,
                    variant,
                    destination: assignment.destination().local().index() as usize,
                    use_block: block_index,
                    statement: statement_index,
                });
            }
            let (source, borrowed) = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => (transparent_operand_place(operand), false),
                SemanticRvalueKindV1::Borrow { place, .. }
                | SemanticRvalueKindV1::AddressOf { place, .. }
                    if place.projections().is_empty() =>
                {
                    (Some(place), true)
                }
                _ => (None, false),
            };
            let Some(source) = source else {
                continue;
            };
            let source = source.local().index() as usize;
            let destination = assignment.destination().local().index() as usize;
            if borrowed {
                if borrowed_locals.len() == MAX_PROJECTED_OPERATIONS_V1 {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "borrowed capability uses exceed the charged projection limit",
                    ));
                }
                borrowed_locals.try_reserve(1).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "borrowed capability use storage cannot be reserved",
                    )
                })?;
                borrowed_locals.push((source, block_index));
            }
            push_capability_edge(
                &mut edges_by_source,
                &mut edge_count,
                source,
                CapabilityEdgeV1 {
                    destination,
                    use_block: block_index,
                    kind: CapabilityEdgeKindV1::Alias,
                },
            )?;
        }

        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            callables.get(call.callee().index() as usize)
        else {
            continue;
        };
        let kind = match operation {
            SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint {
                index_space, ..
            } => CapabilityEdgeKindV1::IntoDisjoint {
                mapping: *index_space,
            },
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift {
                output_space,
                offset,
                ..
            }
            | SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift {
                output_space,
                offset,
                ..
            } => {
                let destination = simple_call_destination(call)?;
                let availability = option_dominance.availability(destination).ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "a checked shift lacks authenticated Option Some availability",
                    ),
                )?;
                CapabilityEdgeKindV1::CheckedShift {
                    mapping: *output_space,
                    offset: *offset,
                    availability,
                }
            }
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock {
                output_space,
                lanes_per_block,
                elements_per_lane,
                ..
            } => {
                if *lanes_per_block != 1 {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a blocked mapping with more than one lane before quotient facts are available",
                    ));
                }
                let expected = SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                    lanes_per_block: *lanes_per_block,
                    elements_per_lane: *elements_per_lane,
                };
                if *output_space != expected
                    || *elements_per_lane == 0
                    || lanes_per_block.checked_mul(*elements_per_lane).is_none()
                {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a malformed blocked mapping reached ranked projection",
                    ));
                }
                let destination = simple_call_destination(call)?;
                let availability = option_dominance.availability(destination).ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "a checked block lacks authenticated Option Some availability",
                    ),
                )?;
                CapabilityEdgeKindV1::CheckedBlock {
                    mapping: expected,
                    elements_per_lane: *elements_per_lane,
                    availability,
                }
            }
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedTiled2d {
                output_space,
                lanes_per_tile,
                tile_rows,
                tile_columns,
                elements_per_lane,
                ..
            } => {
                let expected = SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
                    lanes_per_tile: *lanes_per_tile,
                    tile_rows: *tile_rows,
                    tile_columns: *tile_columns,
                    elements_per_lane: *elements_per_lane,
                };
                if *output_space != expected
                    || !tiled_2d_geometry_valid_v1(
                        *lanes_per_tile,
                        *tile_rows,
                        *tile_columns,
                        *elements_per_lane,
                    )
                {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a malformed tiled-2d mapping reached ranked projection",
                    ));
                }
                let destination = simple_call_destination(call)?;
                let availability = option_dominance.availability(destination).ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "a checked tiled-2d witness lacks authenticated Option Some availability",
                    ),
                )?;
                CapabilityEdgeKindV1::CheckedTiled2d {
                    mapping: expected,
                    availability,
                }
            }
            _ => continue,
        };
        let destination = simple_call_destination(call)?.index() as usize;
        let source = call
            .arguments()
            .first()
            .and_then(simple_operand_local)
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "an index capability transform without one exact input local",
            ))?
            .index() as usize;
        push_capability_edge(
            &mut edges_by_source,
            &mut edge_count,
            source,
            CapabilityEdgeV1 {
                destination,
                use_block: block_index,
                kind,
            },
        )?;
    }

    enum_payload_stores.sort_unstable_by_key(|store| (store.carrier, store.variant));
    for load in enum_payload_loads {
        let key = (load.carrier, load.variant);
        let first =
            enum_payload_stores.partition_point(|store| (store.carrier, store.variant) < key);
        let end =
            enum_payload_stores.partition_point(|store| (store.carrier, store.variant) <= key);
        let matches = &enum_payload_stores[first..end];
        let Some(store) = matches.first() else {
            continue;
        };
        if matches.len() != 1 {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "an enum payload has multiple candidate capability stores",
            ));
        }
        let kind = if store.construction_block == load.use_block && store.statement < load.statement
        {
            CapabilityEdgeKindV1::Alias
        } else {
            if local_definitions.get(load.carrier).copied() != Some(1) {
                continue;
            }
            let Some(availability) = enum_payload_dominance.availability(
                SemanticLocalIdV1::from_index(load.carrier as u32),
                load.variant,
            ) else {
                continue;
            };
            CapabilityEdgeKindV1::AuthenticatedEnumPayload {
                construction_block: store.construction_block,
                availability,
            }
        };
        push_capability_edge(
            &mut edges_by_source,
            &mut edge_count,
            store.source,
            CapabilityEdgeV1 {
                destination: load.destination,
                use_block: load.use_block,
                kind,
            },
        )?;
    }

    let mut index_worklist = VecDeque::new();
    let mut grid_worklist = VecDeque::new();
    for block in function.blocks() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            callables.get(call.callee().index() as usize)
        else {
            continue;
        };
        if !matches!(
            operation,
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. }
                | SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { .. }
        ) {
            continue;
        }
        let destination = simple_call_destination(call)?;
        let destination = destination.index() as usize;
        if destination >= local_count {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an invocation-capability destination outside the semantic local table",
            ));
        }
        if index_values[destination].is_some() || grid_leaders[destination].is_some() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "multiple invocation capabilities for one semantic local",
            ));
        }
        reserve_operation(operations)?;
        let result = next_value_id(next_value)?;
        operations.push(ProductionRankedOperationV1::InvocationIndex {
            result,
            dimension: 0,
            launch_extent,
        });
        push_ranked_ir(
            ranked_ir,
            &format!(
                "  %{} = kernel.invocation_index <0, dynamic>\n",
                result.get()
            ),
        )?;
        match operation {
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. } => {
                index_values[destination] = Some(ProjectedDisjointIndexV1 {
                    value: ProductionRankedValueV1::Local(result),
                    mapping: SemanticDisjointIndexSpaceV1::Index1d,
                    precondition: None,
                    availability: None,
                });
                index_worklist.push_back(destination);
            }
            SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { grid_leader } => {
                let availability = option_dominance
                    .availability(SemanticLocalIdV1::from_index(destination as u32))
                    .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                        "a grid-leader capability lacks authenticated Option Some availability",
                    ))?;
                reserve_operation(operations)?;
                let one = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexConstant {
                    result: one,
                    value: 1,
                });
                push_ranked_ir(
                    ranked_ir,
                    &format!("  %{} = kernel.index_constant 1\n", one.get()),
                )?;
                option_predicates[destination] = Some(GuardPredicateV1 {
                    comparisons: vec![(
                        ProductionRankedValueV1::Local(result),
                        ProductionRankedValueV1::Local(one),
                    )],
                });
                grid_leaders[destination] = Some(ProjectedGridLeaderV1 {
                    grid_leader: *grid_leader,
                    precondition: (
                        ProductionRankedValueV1::Local(result),
                        ProductionRankedValueV1::Local(one),
                    ),
                    availability: CapabilityAvailabilityV1::Option(availability),
                });
                grid_worklist.push_back(destination);
            }
            _ => unreachable!(),
        }
    }

    // rustc may erase the move that binds an unforgeable zero-sized payload
    // from `Option::Some`. Recover that edge only when one exact authenticated
    // producer controls the borrow's block and its payload type matches.
    for (borrowed_local, use_block) in borrowed_locals {
        if grid_leaders[borrowed_local].is_some() {
            continue;
        }
        let borrowed_type = function.locals()[borrowed_local].ty();
        let use_block_id = SemanticBlockIdV1::from_index(use_block as u32);
        let mut producer = None;
        for (candidate_local, candidate) in grid_leaders.iter().copied().enumerate() {
            let Some(candidate) = candidate else {
                continue;
            };
            if candidate.grid_leader != borrowed_type
                || !capability_availability_allows(
                    &option_dominance,
                    &enum_payload_dominance,
                    candidate.availability,
                    use_block_id,
                )
            {
                continue;
            }
            if producer.replace(candidate_local).is_some() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a borrowed zero-sized capability has multiple active Option producers",
                ));
            }
        }
        if let Some(producer) = producer {
            push_capability_edge(
                &mut edges_by_source,
                &mut edge_count,
                producer,
                CapabilityEdgeV1 {
                    destination: borrowed_local,
                    use_block,
                    kind: CapabilityEdgeKindV1::AuthenticatedOptionPayload,
                },
            )?;
        }
    }

    let mut processed_edges = 0_usize;
    while let Some(source) = index_worklist.pop_front() {
        let input = index_values[source].ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "the capability worklist lost an index value",
        ))?;
        for edge in &edges_by_source[source] {
            processed_edges = processed_edges.checked_add(1).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "capability work accounting overflowed",
                ),
            )?;
            let authorization_block = match edge.kind {
                CapabilityEdgeKindV1::AuthenticatedEnumPayload {
                    construction_block, ..
                } => construction_block,
                _ => edge.use_block,
            };
            if !input.availability.is_none_or(|availability| {
                capability_availability_allows(
                    &option_dominance,
                    &enum_payload_dominance,
                    availability,
                    SemanticBlockIdV1::from_index(authorization_block as u32),
                )
            }) {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "an index capability is used outside its authenticated Some edge",
                ));
            }
            let projected = match edge.kind {
                CapabilityEdgeKindV1::Alias | CapabilityEdgeKindV1::AuthenticatedOptionPayload => {
                    input
                }
                CapabilityEdgeKindV1::AuthenticatedEnumPayload { availability, .. } => {
                    ProjectedDisjointIndexV1 {
                        availability: Some(CapabilityAvailabilityV1::EnumPayload(availability)),
                        ..input
                    }
                }
                CapabilityEdgeKindV1::IntoDisjoint { mapping } => {
                    ProjectedDisjointIndexV1 { mapping, ..input }
                }
                CapabilityEdgeKindV1::CheckedShift {
                    mapping,
                    offset,
                    availability,
                } => {
                    reserve_operation(operations)?;
                    let offset_value = next_value_id(next_value)?;
                    operations.push(ProductionRankedOperationV1::IndexConstant {
                        result: offset_value,
                        value: offset,
                    });
                    reserve_operation(operations)?;
                    let shifted = next_value_id(next_value)?;
                    operations.push(ProductionRankedOperationV1::IndexBinary {
                        result: shifted,
                        kind: IndexBinaryKindAttr::Add,
                        lhs: input.value,
                        rhs: ProductionRankedValueV1::Local(offset_value),
                    });
                    push_ranked_ir(
                        ranked_ir,
                        &format!(
                            "  %{} = kernel.index_constant {}\n  %{} = kernel.index_binary Add {}, %{}\n",
                            offset_value.get(),
                            offset,
                            shifted.get(),
                            ranked_value_text_v1(input.value),
                            offset_value.get(),
                        ),
                    )?;
                    let precondition = if offset == 0 {
                        input.precondition
                    } else {
                        reserve_operation(operations)?;
                        let upper = next_value_id(next_value)?;
                        operations.push(ProductionRankedOperationV1::IndexConstant {
                            result: upper,
                            value: u64::MAX - offset + 1,
                        });
                        push_ranked_ir(
                            ranked_ir,
                            &format!(
                                "  %{} = kernel.index_constant {}\n",
                                upper.get(),
                                u64::MAX - offset + 1,
                            ),
                        )?;
                        Some((input.value, ProductionRankedValueV1::Local(upper)))
                    };
                    ProjectedDisjointIndexV1 {
                        value: ProductionRankedValueV1::Local(shifted),
                        mapping,
                        precondition,
                        availability: Some(CapabilityAvailabilityV1::Option(availability)),
                    }
                }
                CapabilityEdgeKindV1::CheckedBlock {
                    mapping,
                    elements_per_lane,
                    availability,
                } => {
                    let maximum_raw = (u64::MAX - (elements_per_lane - 1)) / elements_per_lane;
                    reserve_operation(operations)?;
                    let upper = next_value_id(next_value)?;
                    operations.push(ProductionRankedOperationV1::IndexConstant {
                        result: upper,
                        value: maximum_raw + 1,
                    });
                    push_ranked_ir(
                        ranked_ir,
                        &format!(
                            "  %{} = kernel.index_constant {}\n",
                            upper.get(),
                            maximum_raw + 1,
                        ),
                    )?;
                    ProjectedDisjointIndexV1 {
                        mapping,
                        precondition: Some((input.value, ProductionRankedValueV1::Local(upper))),
                        availability: Some(CapabilityAvailabilityV1::Option(availability)),
                        ..input
                    }
                }
                CapabilityEdgeKindV1::CheckedTiled2d {
                    mapping,
                    availability,
                } => ProjectedDisjointIndexV1 {
                    mapping,
                    availability: Some(CapabilityAvailabilityV1::Option(availability)),
                    ..input
                },
            };
            if matches!(
                edge.kind,
                CapabilityEdgeKindV1::CheckedShift { .. }
                    | CapabilityEdgeKindV1::CheckedBlock { .. }
                    | CapabilityEdgeKindV1::CheckedTiled2d { .. }
            ) {
                let predicate = option_predicates.get_mut(edge.destination).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "a checked capability destination outside the semantic local table",
                    ),
                )?;
                if predicate.is_some() {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "multiple checked predicates for one semantic local",
                    ));
                }
                *predicate = Some(GuardPredicateV1::from_precondition(projected.precondition));
            }
            assign_index_capability(
                edge.destination,
                projected,
                &mut index_values,
                &grid_leaders,
                &mut index_worklist,
            )?;
        }
    }

    while let Some(source) = grid_worklist.pop_front() {
        let input = grid_leaders[source].ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "the capability worklist lost grid-leader authority",
        ))?;
        for edge in &edges_by_source[source] {
            if !matches!(
                edge.kind,
                CapabilityEdgeKindV1::Alias
                    | CapabilityEdgeKindV1::AuthenticatedOptionPayload
                    | CapabilityEdgeKindV1::AuthenticatedEnumPayload { .. }
            ) {
                continue;
            }
            let authorization_block = match edge.kind {
                CapabilityEdgeKindV1::AuthenticatedEnumPayload {
                    construction_block, ..
                } => construction_block,
                _ => edge.use_block,
            };
            if !capability_availability_allows(
                &option_dominance,
                &enum_payload_dominance,
                input.availability,
                SemanticBlockIdV1::from_index(authorization_block as u32),
            ) {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "grid-leader authority is aliased outside its authenticated Some edge",
                ));
            }
            processed_edges = processed_edges.checked_add(1).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "capability work accounting overflowed",
                ),
            )?;
            let destination = edge.destination;
            if destination >= grid_leaders.len() || index_values[destination].is_some() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a grid-leader alias escaped the semantic local table or changed capability kind",
                ));
            }
            let projected = match edge.kind {
                CapabilityEdgeKindV1::AuthenticatedEnumPayload { availability, .. } => {
                    ProjectedGridLeaderV1 {
                        availability: CapabilityAvailabilityV1::EnumPayload(availability),
                        ..input
                    }
                }
                _ => input,
            };
            match grid_leaders[destination] {
                None => {
                    grid_leaders[destination] = Some(projected);
                    grid_worklist.push_back(destination);
                }
                Some(existing) if existing == projected => {}
                Some(_) => {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "multiple grid-leader capabilities reach one semantic local",
                    ));
                }
            }
        }
    }
    if processed_edges > edge_count {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "capability worklist exceeded its charged def-use edges",
        ));
    }

    let mut views_by_origin: Vec<Option<ProjectedViewV1>> = vec![None; function.locals().len()];
    let mut guarded_accesses = Vec::new();
    for (block_index, block) in function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            callables.get(call.callee().index() as usize)
        else {
            continue;
        };
        let (element, index, precondition) = match operation {
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { element, .. } => {
                let projected = projected_disjoint_operand_v1(
                    call,
                    1,
                    &index_values,
                    &option_dominance,
                    &enum_payload_dominance,
                    block_index,
                )?;
                if projected.mapping != SemanticDisjointIndexSpaceV1::Index1d {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "identity accessor received a non-identity mapping",
                    ));
                }
                (*element, projected.value, projected.precondition)
            }
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
                element,
                index_space,
                ..
            } => {
                let projected = projected_disjoint_operand_v1(
                    call,
                    1,
                    &index_values,
                    &option_dominance,
                    &enum_payload_dominance,
                    block_index,
                )?;
                if projected.mapping != *index_space {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "disjoint accessor mapping identity changed",
                    ));
                }
                (*element, projected.value, projected.precondition)
            }
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
                element,
                grid_leader,
                ..
            } => {
                let leader_local = call
                    .arguments()
                    .get(1)
                    .and_then(simple_operand_local)
                    .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                        "an exclusive access without one exact grid-leader local",
                    ))?
                    .index() as usize;
                let leader = grid_leaders.get(leader_local).copied().flatten().ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "an exclusive access without authenticated grid-leader authority",
                    ),
                )?;
                if leader.grid_leader != *grid_leader {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "grid-leader capability type identity changed",
                    ));
                }
                if !capability_availability_allows(
                    &option_dominance,
                    &enum_payload_dominance,
                    leader.availability,
                    SemanticBlockIdV1::from_index(block_index as u32),
                ) {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "grid-leader authority is used outside its authenticated Some edge",
                    ));
                }
                let value = call
                    .arguments()
                    .get(2)
                    .and_then(|operand| constant_operand_value(operand, constants))
                    .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                        "a dynamic grid-exclusive index requires a deliberate ranked argument projection",
                    ))?;
                reserve_operation(operations)?;
                let constant_index = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexConstant {
                    result: constant_index,
                    value,
                });
                push_ranked_ir(
                    ranked_ir,
                    &format!(
                        "  %{} = kernel.index_constant {}\n",
                        constant_index.get(),
                        value,
                    ),
                )?;
                reserve_operation(operations)?;
                let index = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexBinary {
                    result: index,
                    kind: IndexBinaryKindAttr::Add,
                    lhs: leader.precondition.0,
                    rhs: ProductionRankedValueV1::Local(constant_index),
                });
                push_ranked_ir(
                    ranked_ir,
                    &format!(
                        "  %{} = kernel.index_binary Add {}, %{}\n",
                        index.get(),
                        ranked_value_text_v1(leader.precondition.0),
                        constant_index.get(),
                    ),
                )?;
                (
                    *element,
                    ProductionRankedValueV1::Local(index),
                    Some(leader.precondition),
                )
            }
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
                element,
                index_space,
                lanes_per_block,
                elements_per_lane,
                ..
            } => {
                let projected = projected_disjoint_operand_v1(
                    call,
                    1,
                    &index_values,
                    &option_dominance,
                    &enum_payload_dominance,
                    block_index,
                )?;
                let expected = SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                    lanes_per_block: *lanes_per_block,
                    elements_per_lane: *elements_per_lane,
                };
                if projected.mapping != expected || *index_space != expected {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "blocked accessor mapping identity changed",
                    ));
                }
                if *lanes_per_block != 1 {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a blocked mapping with more than one lane before quotient facts are available",
                    ));
                }
                let component = call
                    .arguments()
                    .get(2)
                    .and_then(|operand| constant_operand_value(operand, constants))
                    .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                        "a dynamic blocked component before ranked-value projection is available",
                    ))?;
                if component >= *elements_per_lane {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a blocked component is outside the authenticated elements-per-lane bound",
                    ));
                }
                reserve_operation(operations)?;
                let elements = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexConstant {
                    result: elements,
                    value: *elements_per_lane,
                });
                reserve_operation(operations)?;
                let block_base = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexBinary {
                    result: block_base,
                    kind: IndexBinaryKindAttr::Multiply,
                    lhs: projected.value,
                    rhs: ProductionRankedValueV1::Local(elements),
                });
                reserve_operation(operations)?;
                let component_value = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexConstant {
                    result: component_value,
                    value: component,
                });
                reserve_operation(operations)?;
                let index = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexBinary {
                    result: index,
                    kind: IndexBinaryKindAttr::Add,
                    lhs: ProductionRankedValueV1::Local(block_base),
                    rhs: ProductionRankedValueV1::Local(component_value),
                });
                push_ranked_ir(
                    ranked_ir,
                    &format!(
                        "  %{} = kernel.index_constant {}\n  %{} = kernel.index_binary Multiply {}, %{}\n  %{} = kernel.index_constant {}\n  %{} = kernel.index_binary Add %{}, %{}\n",
                        elements.get(),
                        elements_per_lane,
                        block_base.get(),
                        ranked_value_text_v1(projected.value),
                        elements.get(),
                        component_value.get(),
                        component,
                        index.get(),
                        block_base.get(),
                        component_value.get(),
                    ),
                )?;
                (
                    *element,
                    ProductionRankedValueV1::Local(index),
                    projected.precondition,
                )
            }
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut {
                element,
                index_space,
                lanes_per_tile,
                tile_rows,
                tile_columns,
                elements_per_lane,
                ..
            } => {
                let projected = projected_disjoint_operand_v1(
                    call,
                    1,
                    &index_values,
                    &option_dominance,
                    &enum_payload_dominance,
                    block_index,
                )?;
                let expected = SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
                    lanes_per_tile: *lanes_per_tile,
                    tile_rows: *tile_rows,
                    tile_columns: *tile_columns,
                    elements_per_lane: *elements_per_lane,
                };
                if projected.mapping != expected
                    || *index_space != expected
                    || !tiled_2d_geometry_valid_v1(
                        *lanes_per_tile,
                        *tile_rows,
                        *tile_columns,
                        *elements_per_lane,
                    )
                {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "tiled-2d accessor mapping identity changed",
                    ));
                }
                let component = project_runtime_index_operand_v1(
                    call.arguments().get(2),
                    constants,
                    &stable_argument_origins,
                    &mut runtime_index_arguments,
                    &mut next_runtime_argument,
                    operations,
                    next_value,
                )?;
                let rows = project_runtime_index_operand_v1(
                    call.arguments().get(3),
                    constants,
                    &stable_argument_origins,
                    &mut runtime_index_arguments,
                    &mut next_runtime_argument,
                    operations,
                    next_value,
                )?;
                let columns = project_runtime_index_operand_v1(
                    call.arguments().get(4),
                    constants,
                    &stable_argument_origins,
                    &mut runtime_index_arguments,
                    &mut next_runtime_argument,
                    operations,
                    next_value,
                )?;
                let row_stride = project_runtime_index_operand_v1(
                    call.arguments().get(5),
                    constants,
                    &stable_argument_origins,
                    &mut runtime_index_arguments,
                    &mut next_runtime_argument,
                    operations,
                    next_value,
                )?;
                reserve_operation(operations)?;
                let index = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::CheckedTiledIndex2D {
                    result: index,
                    invocation: projected.value,
                    component,
                    rows,
                    columns,
                    row_stride,
                    lanes_per_tile: *lanes_per_tile,
                    tile_rows: *tile_rows,
                    tile_columns: *tile_columns,
                    elements_per_lane: *elements_per_lane,
                });
                (
                    *element,
                    ProductionRankedValueV1::Local(index),
                    projected.precondition,
                )
            }
            _ => continue,
        };

        let receiver = call
            .arguments()
            .first()
            .and_then(simple_operand_local)
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "a checked disjoint receiver without one exact local",
            ))?
            .index() as usize;
        let allocation_contract = local_allocations.get(receiver).copied().flatten().ok_or(
            ProductionRankedProjectionErrorV1::Incomplete(
                "a checked disjoint receiver without one authenticated kernel-argument origin",
            ),
        )?;
        if !allocation_contract.writable {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a checked mutable access is rooted in a read-only Rust allocation",
            ));
        }
        let origin_index = allocation_contract.allocation_origin as usize;
        let element_width = type_width(types, element)?;
        let view = match views_by_origin
            .get(origin_index)
            .and_then(|view| view.as_ref())
        {
            Some(view)
                if view.element_width == element_width
                    && view.writable
                    && view.shape == [DYNAMIC_EXTENT]
                    && view.dynamic_extents == [ProductionRankedValueV1::Argument(0)]
                    && view.memory_space == MemorySpaceAttr::Global
                    && view.allocation_origin == allocation_contract.allocation_origin
                    && view.noalias_class == allocation_contract.noalias_class =>
            {
                view.result
            }
            Some(_) => {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "one allocation origin was projected with conflicting element widths",
                ));
            }
            None => {
                reserve_operation(operations)?;
                let view = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::ViewInSpace {
                    result: view,
                    element_width,
                    writable: true,
                    shape: vec![DYNAMIC_EXTENT],
                    dynamic_extents: vec![ProductionRankedValueV1::Argument(0)],
                    memory_space: MemorySpaceAttr::Global,
                    allocation_origin: allocation_contract.allocation_origin,
                    noalias_class: allocation_contract.noalias_class,
                });
                push_ranked_ir(
                    ranked_ir,
                    &format!(
                        "  %{} = kernel.ranked_view <{}, true, [dynamic], Global>(%arg0)\n",
                        view.get(),
                        element_width,
                    ),
                )?;
                let slot = views_by_origin.get_mut(origin_index).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "a kernel argument origin outside the semantic local table",
                    ),
                )?;
                *slot = Some(ProjectedViewV1 {
                    result: view,
                    element_width,
                    writable: true,
                    shape: vec![DYNAMIC_EXTENT],
                    dynamic_extents: vec![ProductionRankedValueV1::Argument(0)],
                    memory_space: MemorySpaceAttr::Global,
                    allocation_origin: allocation_contract.allocation_origin,
                    noalias_class: allocation_contract.noalias_class,
                });
                view
            }
        };
        let mut comparisons = Vec::with_capacity(2);
        if let Some(precondition) = precondition {
            comparisons.push(precondition);
        }
        comparisons.push((index, ProductionRankedValueV1::Argument(0)));
        let access = GuardedRankedAccessV1 {
            view,
            indices: vec![index],
            comparisons,
            access: AccessKindAttr::Write,
            memory_space: MemorySpaceAttr::Global,
            source: block.terminator().source(),
            semantic_site: None,
        };
        let destination = simple_call_destination(call)?.index() as usize;
        let predicate = option_predicates.get_mut(destination).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a checked disjoint destination outside the semantic local table",
            ),
        )?;
        if predicate.is_some() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "multiple checked predicates for one semantic local",
            ));
        }
        *predicate = Some(GuardPredicateV1::for_access(&access));
        guarded_accesses.push(access);
    }

    let mut direct_switch_predicates = vec![None; local_count];
    for block in function.blocks() {
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() {
                continue;
            }
            let SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left,
                right,
            } = assignment.value().kind()
            else {
                continue;
            };
            let destination = assignment.destination().local().index() as usize;
            let Some(lhs) = project_uniform_switch_operand_v1(
                left,
                constants,
                &stable_argument_origins,
                &local_definitions,
                function,
                &mut runtime_index_arguments,
                &mut next_runtime_argument,
                operations,
                next_value,
            )?
            else {
                continue;
            };
            let Some(rhs) = project_uniform_switch_operand_v1(
                right,
                constants,
                &stable_argument_origins,
                &local_definitions,
                function,
                &mut runtime_index_arguments,
                &mut next_runtime_argument,
                operations,
                next_value,
            )?
            else {
                continue;
            };
            retain_identical_direct_switch_predicate_v1(
                direct_switch_predicates.get_mut(destination).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "a direct switch predicate outside the semantic local table",
                    ),
                )?,
                GuardPredicateV1 {
                    comparisons: vec![(lhs, rhs)],
                },
            )?;
        }
    }
    let uniform_inductions = project_uniform_inductions_v1(
        types,
        function,
        constants,
        &stable_argument_origins,
        &local_definitions,
        &mut runtime_index_arguments,
        &mut next_runtime_argument,
        operations,
        next_value,
    )?;
    let projected_switch_predicates =
        switch_predicates(function, &option_predicates, &direct_switch_predicates)?;
    let deterministic_switches = project_deterministic_scalar_switches_v1(
        callables,
        function,
        constants,
        &local_definitions,
        &index_values,
        &local_allocations,
        &projected_switch_predicates,
        &mut runtime_index_arguments,
        &mut next_runtime_argument,
        operations,
        next_value,
    )?;

    let local_contracts = ProjectionLocalContractsV1 {
        checked_reference_origins: checked_reference_origins(
            function,
            callables,
            guarded_accesses.len(),
        )?,
        allocations: local_allocations,
    };
    Ok(IntrinsicProjectionV1 {
        local_contracts,
        extent_argument_count: if guarded_accesses.is_empty() && next_runtime_argument == 1 {
            0
        } else {
            next_runtime_argument
        },
        guarded_accesses,
        option_predicates,
        direct_switch_predicates,
        deterministic_switches,
        uniform_inductions,
        tensor_layouts,
        tensor_read_effects,
        read_view_effects,
    })
}

fn reject_retired_production_intrinsics_v1(
    callables: &[SemanticCallableDeclV1],
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if callables.iter().any(|callable| {
        matches!(
            callable,
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad { .. },
                ..
            }
        )
    }) {
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "the retired Option-returning BF16 matrix load; use Bf16MatrixLoadZeroFilledV2",
        ))
    } else {
        Ok(())
    }
}

fn retain_identical_direct_switch_predicate_v1(
    slot: &mut Option<GuardPredicateV1>,
    candidate: GuardPredicateV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match slot {
        None => *slot = Some(candidate),
        Some(existing) if *existing == candidate => {}
        Some(_) => {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "one comparison local has conflicting source definitions",
            ));
        }
    }
    Ok(())
}

struct DeterministicScalarProjectorV1<'a> {
    callables: &'a [SemanticCallableDeclV1],
    function: &'a SemanticFunctionDeclV1,
    constants: &'a [Option<u64>],
    local_definitions: &'a [u8],
    index_values: &'a [Option<ProjectedDisjointIndexV1>],
    local_allocations: &'a [Option<AllocationContractV1>],
    switch_predicates: &'a [Option<GuardPredicateV1>],
    argument_slots: &'a mut [Option<u32>],
    next_argument: &'a mut usize,
    operations: &'a mut Vec<ProductionRankedOperationV1>,
    next_value: &'a mut u32,
    definitions: Vec<Vec<DeterministicScalarDefinitionV1>>,
    states: Vec<u8>,
    summaries: Vec<Option<DeterministicScalarSummaryV1>>,
    ranked_constants: HashMap<u64, ProductionRankedValueV1>,
    reachability: HashMap<(usize, usize), bool>,
    reachability_marks: Vec<u32>,
    reachability_epoch: u32,
    reachability_work: usize,
}

impl<'a> DeterministicScalarProjectorV1<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        callables: &'a [SemanticCallableDeclV1],
        function: &'a SemanticFunctionDeclV1,
        constants: &'a [Option<u64>],
        local_definitions: &'a [u8],
        index_values: &'a [Option<ProjectedDisjointIndexV1>],
        local_allocations: &'a [Option<AllocationContractV1>],
        switch_predicates: &'a [Option<GuardPredicateV1>],
        argument_slots: &'a mut [Option<u32>],
        next_argument: &'a mut usize,
        operations: &'a mut Vec<ProductionRankedOperationV1>,
        next_value: &'a mut u32,
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        let local_count = function.locals().len();
        if constants.len() != local_count
            || local_definitions.len() != local_count
            || index_values.len() != local_count
            || local_allocations.len() != local_count
            || switch_predicates.len() != local_count
            || argument_slots.len() != local_count
        {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "deterministic scalar projection tables do not match the semantic local table",
            ));
        }
        let mut definitions = vec![Vec::new(); local_count];
        for (block_index, block) in function.blocks().iter().enumerate() {
            for (statement_index, statement) in block.statements().iter().enumerate() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                if !assignment.destination().projections().is_empty() {
                    continue;
                }
                let local = assignment.destination().local().index() as usize;
                let Some(slot) = definitions.get_mut(local) else {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a deterministic scalar assignment is outside the semantic local table",
                    ));
                };
                slot.try_reserve(1).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "deterministic scalar definition storage cannot be reserved",
                    )
                })?;
                slot.push(DeterministicScalarDefinitionV1::Assignment {
                    block: block_index,
                    statement: statement_index,
                });
            }
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && let Some(destination) = call.destination()
                && destination.place().projections().is_empty()
            {
                let local = destination.place().local().index() as usize;
                let Some(slot) = definitions.get_mut(local) else {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a deterministic scalar call destination is outside the semantic local table",
                    ));
                };
                slot.try_reserve(1).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "deterministic scalar definition storage cannot be reserved",
                    )
                })?;
                slot.push(DeterministicScalarDefinitionV1::Call { block: block_index });
            }
        }
        Ok(Self {
            callables,
            function,
            constants,
            local_definitions,
            index_values,
            local_allocations,
            switch_predicates,
            argument_slots,
            next_argument,
            operations,
            next_value,
            definitions,
            states: vec![0; local_count],
            summaries: vec![None; local_count],
            ranked_constants: HashMap::new(),
            reachability: HashMap::new(),
            reachability_marks: vec![0; function.blocks().len()],
            reachability_epoch: 0,
            reachability_work: 0,
        })
    }

    fn resolve_operand(
        &mut self,
        operand: &SemanticOperandV1,
    ) -> Result<Option<DeterministicScalarSummaryV1>, ProductionRankedProjectionErrorV1> {
        match operand {
            SemanticOperandV1::Constant(constant) => match constant.value() {
                SemanticConstantValueV1::Scalar(value) => Ok(u64::try_from(value.bits())
                    .ok()
                    .map(DeterministicScalarSummaryV1::Constant)),
                _ => Ok(None),
            },
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.resolve_place(place, place.projections().is_empty())
            }
        }
    }

    fn resolve_place(
        &mut self,
        place: &SemanticPlaceV1,
        preserve_exact: bool,
    ) -> Result<Option<DeterministicScalarSummaryV1>, ProductionRankedProjectionErrorV1> {
        if transparent_place(place).is_none() {
            return Ok(None);
        }
        let summary = self.resolve_local(place.local().index() as usize)?;
        if preserve_exact {
            Ok(summary)
        } else {
            self.derive([summary])
        }
    }

    fn resolve_local(
        &mut self,
        local: usize,
    ) -> Result<Option<DeterministicScalarSummaryV1>, ProductionRankedProjectionErrorV1> {
        let Some(state) = self.states.get(local).copied() else {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a deterministic scalar dependency is outside the semantic local table",
            ));
        };
        if state == 2 {
            return Ok(self.summaries[local].clone());
        }
        if state == 1 {
            return Ok(None);
        }
        self.states[local] = 1;
        let summary = if let Some(index) = self.index_values[local] {
            Some(DeterministicScalarSummaryV1::Exact(index.value))
        } else if let Some(value) = self.constants[local] {
            Some(DeterministicScalarSummaryV1::Constant(value))
        } else if self.local_definitions[local] == 0 {
            match self.function.locals()[local].role() {
                SemanticLocalRoleV1::Argument(argument) => Some(
                    DeterministicScalarSummaryV1::Exact(self.ranked_argument(argument as usize)?),
                ),
                SemanticLocalRoleV1::Return | SemanticLocalRoleV1::Temporary => None,
            }
        } else if self.definitions[local].len() != usize::from(self.local_definitions[local]) {
            None
        } else {
            let mut candidates = Vec::new();
            candidates
                .try_reserve(self.definitions[local].len())
                .map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "deterministic scalar definition meet storage cannot be reserved",
                    )
                })?;
            for definition_index in 0..self.definitions[local].len() {
                let definition = self.definitions[local][definition_index];
                let Some(candidate) = self.resolve_definition(definition)? else {
                    candidates.clear();
                    break;
                };
                candidates.push(candidate);
            }
            if candidates.is_empty() {
                None
            } else if candidates.windows(2).all(|pair| pair[0] == pair[1])
                || self.definitions[local]
                    .windows(2)
                    .all(|pair| self.definition_semantics_equal(pair[0], pair[1]))
            {
                candidates.into_iter().next()
            } else {
                self.merge_control_selected_definitions(local, candidates)?
            }
        };
        self.states[local] = 2;
        self.summaries[local] = summary.clone();
        Ok(summary)
    }

    fn resolve_definition(
        &mut self,
        definition: DeterministicScalarDefinitionV1,
    ) -> Result<Option<DeterministicScalarSummaryV1>, ProductionRankedProjectionErrorV1> {
        match definition {
            DeterministicScalarDefinitionV1::Assignment { block, statement } => {
                let value = self.function.blocks()[block].statements()[statement]
                    .kind()
                    .clone();
                let SemanticStatementKindV1::Assign(assignment) = value else {
                    unreachable!("indexed deterministic assignment changed kind")
                };
                self.resolve_rvalue(assignment.value().kind().clone())
            }
            DeterministicScalarDefinitionV1::Call { block } => {
                let terminator = self.function.blocks()[block].terminator().kind().clone();
                let SemanticTerminatorKindV1::Call(call) = terminator else {
                    unreachable!("indexed deterministic call changed kind")
                };
                self.resolve_call(&call)
            }
        }
    }

    fn merge_control_selected_definitions(
        &mut self,
        local: usize,
        candidates: Vec<DeterministicScalarSummaryV1>,
    ) -> Result<Option<DeterministicScalarSummaryV1>, ProductionRankedProjectionErrorV1> {
        if self.definitions[local]
            .windows(2)
            .any(|pair| Self::definition_block(pair[0]) == Self::definition_block(pair[1]))
        {
            return Ok(None);
        }
        let mut inputs = Vec::new();
        inputs.try_reserve(candidates.len()).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "deterministic scalar control meet storage cannot be reserved",
            )
        })?;
        inputs.extend(candidates.into_iter().map(Some));
        let mut observed_selector = false;
        for definition_index in 0..self.definitions[local].len() {
            let definition_block =
                Self::definition_block(self.definitions[local][definition_index]);
            let Some(controls) = self.definition_control_summaries(definition_block)? else {
                return Ok(None);
            };
            observed_selector |= !controls.is_empty();
            inputs.try_reserve(controls.len()).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "deterministic scalar control meet storage cannot be reserved",
                )
            })?;
            inputs.extend(controls.into_iter().map(Some));
        }
        if !observed_selector {
            return Ok(None);
        }
        self.derive(inputs)
    }

    const fn definition_block(definition: DeterministicScalarDefinitionV1) -> usize {
        match definition {
            DeterministicScalarDefinitionV1::Assignment { block, .. }
            | DeterministicScalarDefinitionV1::Call { block } => block,
        }
    }

    fn definition_semantics_equal(
        &self,
        left: DeterministicScalarDefinitionV1,
        right: DeterministicScalarDefinitionV1,
    ) -> bool {
        match (left, right) {
            (
                DeterministicScalarDefinitionV1::Assignment {
                    block: left_block,
                    statement: left_statement,
                },
                DeterministicScalarDefinitionV1::Assignment {
                    block: right_block,
                    statement: right_statement,
                },
            ) => {
                let left = self.function.blocks()[left_block].statements()[left_statement].kind();
                let right =
                    self.function.blocks()[right_block].statements()[right_statement].kind();
                let (SemanticStatementKindV1::Assign(left), SemanticStatementKindV1::Assign(right)) =
                    (left, right)
                else {
                    unreachable!("indexed deterministic assignments changed kind")
                };
                left.value() == right.value()
            }
            (
                DeterministicScalarDefinitionV1::Call { block: left },
                DeterministicScalarDefinitionV1::Call { block: right },
            ) => {
                let left = self.function.blocks()[left].terminator().kind();
                let right = self.function.blocks()[right].terminator().kind();
                let (SemanticTerminatorKindV1::Call(left), SemanticTerminatorKindV1::Call(right)) =
                    (left, right)
                else {
                    unreachable!("indexed deterministic calls changed kind")
                };
                left.callee() == right.callee()
                    && left.arguments() == right.arguments()
                    && left.variadic_argument_abis() == right.variadic_argument_abis()
                    && left.unwind() == right.unwind()
            }
            _ => false,
        }
    }

    fn definition_control_summaries(
        &mut self,
        definition_block: usize,
    ) -> Result<Option<Vec<DeterministicScalarSummaryV1>>, ProductionRankedProjectionErrorV1> {
        let mut controls = Vec::new();
        for block_index in 0..self.function.blocks().len() {
            let terminator = self.function.blocks()[block_index]
                .terminator()
                .kind()
                .clone();
            let successors = self.control_successors(&terminator)?;
            if successors.len() < 2 {
                continue;
            }
            let mut reaches = 0_usize;
            for successor in successors.iter().copied() {
                reaches += usize::from(self.block_can_reach(successor, definition_block)?);
            }
            if reaches == 0 || reaches == successors.len() {
                continue;
            }
            let SemanticTerminatorKindV1::SwitchInt { discriminant, .. } = &terminator else {
                return Ok(None);
            };
            let Some(control) = self.resolve_operand(discriminant)? else {
                return Ok(None);
            };
            controls.try_reserve(1).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "deterministic scalar selector storage cannot be reserved",
                )
            })?;
            controls.push(control);
        }
        Ok(Some(controls))
    }

    fn control_successors(
        &self,
        terminator: &SemanticTerminatorKindV1,
    ) -> Result<Vec<usize>, ProductionRankedProjectionErrorV1> {
        let proven_true = match terminator {
            SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } => simple_operand_local(discriminant)
                .and_then(|local| self.switch_predicates.get(local.index() as usize))
                .and_then(Option::as_ref)
                .filter(|predicate| predicate.comparisons.is_empty())
                .map(|_| {
                    let target = if targets.values().len() == 1 {
                        match targets.values()[0].value() {
                            0 => targets.otherwise().target(),
                            1 => targets.values()[0].edge().target(),
                            _ => {
                                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                                    "an authenticated empty predicate did not select a boolean switch",
                                ));
                            }
                        }
                    } else {
                        let Some(target) = targets.values().iter().find(|target| target.value() == 1)
                        else {
                            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                                "an authenticated empty predicate had no true switch variant",
                            ));
                        };
                        target.edge().target()
                    };
                    Ok(target.index() as usize)
                })
                .transpose()?,
            _ => None,
        };
        if let Some(successor) = proven_true {
            if successor >= self.function.blocks().len() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a deterministic scalar proven control edge is outside the semantic CFG",
                ));
            }
            return Ok(vec![successor]);
        }

        let mut successors = Vec::new();
        successors
            .try_reserve(terminator.edge_count())
            .map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "deterministic scalar successor storage cannot be reserved",
                )
            })?;
        let mut successor_set = HashSet::new();
        successor_set
            .try_reserve(terminator.edge_count())
            .map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "deterministic scalar successor set cannot be reserved",
                )
            })?;
        terminator.try_for_each_edge::<ProductionRankedProjectionErrorV1>(|edge| {
            let successor = edge.target().index() as usize;
            if successor >= self.function.blocks().len() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a deterministic scalar control edge is outside the semantic CFG",
                ));
            }
            if successor_set.insert(successor) {
                successors.push(successor);
            }
            Ok(())
        })?;
        Ok(successors)
    }

    fn block_can_reach(
        &mut self,
        start: usize,
        target: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        if start >= self.function.blocks().len() || target >= self.function.blocks().len() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a deterministic scalar reachability query is outside the semantic CFG",
            ));
        }
        if let Some(reaches) = self.reachability.get(&(start, target)).copied() {
            return Ok(reaches);
        }
        self.reachability_epoch = self.reachability_epoch.wrapping_add(1);
        if self.reachability_epoch == 0 {
            self.reachability_marks.fill(0);
            self.reachability_epoch = 1;
        }
        let epoch = self.reachability_epoch;
        let mut worklist = Vec::new();
        worklist.try_reserve(1).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "deterministic scalar reachability worklist cannot be reserved",
            )
        })?;
        worklist.push(start);
        self.reachability_marks[start] = epoch;
        let mut reaches = false;
        while let Some(block) = worklist.pop() {
            if block == target {
                reaches = true;
                break;
            }
            let terminator = self.function.blocks()[block].terminator().kind().clone();
            for successor in self.control_successors(&terminator)? {
                self.reachability_work = self.reachability_work.checked_add(1).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "deterministic scalar reachability work accounting overflowed",
                    ),
                )?;
                if self.reachability_work > MAX_PROJECTED_TENSOR_DATAFLOW_WORK_V1 {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "deterministic scalar reachability exceeded its explicit work limit",
                    ));
                }
                let Some(mark) = self.reachability_marks.get_mut(successor) else {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a deterministic scalar reachability edge is outside the semantic CFG",
                    ));
                };
                if *mark != epoch {
                    *mark = epoch;
                    worklist.try_reserve(1).map_err(|_| {
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "deterministic scalar reachability worklist cannot be reserved",
                        )
                    })?;
                    worklist.push(successor);
                }
            }
        }
        self.reachability.try_reserve(1).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "deterministic scalar reachability cache cannot be reserved",
            )
        })?;
        self.reachability.insert((start, target), reaches);
        Ok(reaches)
    }

    fn resolve_rvalue(
        &mut self,
        rvalue: SemanticRvalueKindV1,
    ) -> Result<Option<DeterministicScalarSummaryV1>, ProductionRankedProjectionErrorV1> {
        match rvalue {
            SemanticRvalueKindV1::Use(operand) => self.resolve_operand(&operand),
            SemanticRvalueKindV1::Unary { operand, .. }
            | SemanticRvalueKindV1::Cast { operand, .. } => {
                let summary = self.resolve_operand(&operand)?;
                self.derive([summary])
            }
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            } => self.resolve_binary(operation, &left, &right),
            SemanticRvalueKindV1::CheckedBinary(checked) => {
                let left = self.resolve_operand(checked.left())?;
                let right = self.resolve_operand(checked.right())?;
                self.derive([left, right])
            }
            // Unlike ordinary MIR arithmetic guarded by explicit control, this
            // operation carries a precondition not represented in the rvalue.
            SemanticRvalueKindV1::UncheckedBinary(_) => Ok(None),
            SemanticRvalueKindV1::Aggregate(aggregate) => {
                if let SemanticAggregateKindV1::EnumVariant(variant) = aggregate.kind() {
                    return Ok(Some(DeterministicScalarSummaryV1::Constant(u64::from(
                        *variant,
                    ))));
                }
                let mut inputs = Vec::new();
                inputs
                    .try_reserve(aggregate.operands().len())
                    .map_err(|_| {
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "deterministic scalar aggregate dependency storage cannot be reserved",
                        )
                    })?;
                for operand in aggregate.operands() {
                    inputs.push(self.resolve_operand(operand)?);
                }
                self.derive(inputs)
            }
            SemanticRvalueKindV1::Length(place) => self.resolve_place(&place, false),
            // Private addresses may differ per invocation even when the
            // referenced value is uniform. Address-space provenance must be
            // established before either rvalue can become a control summary.
            SemanticRvalueKindV1::Borrow { place, .. }
            | SemanticRvalueKindV1::AddressOf { place, .. } => {
                let local = place.local().index() as usize;
                if self
                    .local_allocations
                    .get(local)
                    .copied()
                    .flatten()
                    .is_none()
                {
                    return Ok(None);
                }
                let summary = self.resolve_place(&place, true)?;
                self.derive([summary])
            }
            SemanticRvalueKindV1::Discriminant(place) => {
                let summary = self.resolve_place(&place, true)?;
                match summary {
                    Some(DeterministicScalarSummaryV1::Constant(value)) => {
                        Ok(Some(DeterministicScalarSummaryV1::Constant(value)))
                    }
                    summary => self.derive([summary]),
                }
            }
            // A load may depend on mutable memory that is not represented by
            // its address. It can never be summarized as deterministic control.
            SemanticRvalueKindV1::Load(_) => Ok(None),
        }
    }

    fn resolve_call(
        &mut self,
        call: &SemanticDirectCallV1,
    ) -> Result<Option<DeterministicScalarSummaryV1>, ProductionRankedProjectionErrorV1> {
        if matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_)) {
            return Ok(None);
        }
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            self.callables.get(call.callee().index() as usize)
        else {
            return Ok(None);
        };
        if matches!(
            operation,
            SemanticCompilerIntrinsicOperationV1::ThreadIndexGet { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointIndexGet { .. }
        ) {
            let [argument] = call.arguments() else {
                return Ok(None);
            };
            return Ok(match self.resolve_operand(argument)? {
                Some(DeterministicScalarSummaryV1::Derived(values)) if values.len() == 1 => {
                    Some(DeterministicScalarSummaryV1::Exact(values[0]))
                }
                summary @ Some(
                    DeterministicScalarSummaryV1::Constant(_)
                    | DeterministicScalarSummaryV1::Exact(_),
                ) => summary,
                Some(DeterministicScalarSummaryV1::Derived(_)) | None => None,
            });
        }
        if !compiler_intrinsic_is_pure_total_scalar_dependency_v1(operation) {
            return Ok(None);
        }
        let mut inputs = Vec::new();
        inputs.try_reserve(call.arguments().len()).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "deterministic scalar call dependency storage cannot be reserved",
            )
        })?;
        for argument in call.arguments() {
            inputs.push(self.resolve_operand(argument)?);
        }
        self.derive(inputs)
    }

    fn resolve_binary(
        &mut self,
        operation: SemanticBinaryOpV1,
        left: &SemanticOperandV1,
        right: &SemanticOperandV1,
    ) -> Result<Option<DeterministicScalarSummaryV1>, ProductionRankedProjectionErrorV1> {
        let left = self.resolve_operand(left)?;
        let right = self.resolve_operand(right)?;
        let Some(kind) = deterministic_index_binary_kind_v1(operation) else {
            return self.derive([left, right]);
        };
        if matches!(
            kind,
            IndexBinaryKindAttr::Divide | IndexBinaryKindAttr::Remainder
        ) && !matches!(right, Some(DeterministicScalarSummaryV1::Constant(value)) if value != 0)
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a division or remainder used for deterministic control lacks a statically nonzero divisor",
            ));
        }
        let (Some(left), Some(right)) = (left, right) else {
            return Ok(None);
        };
        let lhs = self.materialize(left)?;
        let rhs = self.materialize(right)?;
        reserve_operation(self.operations)?;
        let result = next_value_id(self.next_value)?;
        self.operations
            .push(ProductionRankedOperationV1::IndexBinary {
                result,
                kind,
                lhs,
                rhs,
            });
        Ok(Some(DeterministicScalarSummaryV1::Exact(
            ProductionRankedValueV1::Local(result),
        )))
    }

    fn derive(
        &mut self,
        inputs: impl IntoIterator<Item = Option<DeterministicScalarSummaryV1>>,
    ) -> Result<Option<DeterministicScalarSummaryV1>, ProductionRankedProjectionErrorV1> {
        let mut dependencies = Vec::new();
        for input in inputs {
            let Some(input) = input else {
                return Ok(None);
            };
            let values = match input {
                DeterministicScalarSummaryV1::Constant(value) => {
                    let value = self.ranked_constant(value)?;
                    Self::retain_dependency(&mut dependencies, value)?;
                    continue;
                }
                DeterministicScalarSummaryV1::Exact(value) => {
                    Self::retain_dependency(&mut dependencies, value)?;
                    continue;
                }
                DeterministicScalarSummaryV1::Derived(values) => values,
            };
            for value in values {
                Self::retain_dependency(&mut dependencies, value)?;
            }
        }
        Ok((!dependencies.is_empty())
            .then_some(DeterministicScalarSummaryV1::Derived(dependencies)))
    }

    fn retain_dependency(
        dependencies: &mut Vec<ProductionRankedValueV1>,
        value: ProductionRankedValueV1,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if let Err(index) = dependencies.binary_search(&value) {
            dependencies.try_reserve(1).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "deterministic scalar dependency storage cannot be reserved",
                )
            })?;
            dependencies.insert(index, value);
            if dependencies.len() > MAX_DETERMINISTIC_JOIN_INPUTS_V1 {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a deterministic scalar expression exceeds the explicit dependency limit",
                ));
            }
        }
        Ok(())
    }

    fn materialize(
        &mut self,
        summary: DeterministicScalarSummaryV1,
    ) -> Result<ProductionRankedValueV1, ProductionRankedProjectionErrorV1> {
        match summary {
            DeterministicScalarSummaryV1::Constant(value) => self.ranked_constant(value),
            DeterministicScalarSummaryV1::Exact(value) => Ok(value),
            DeterministicScalarSummaryV1::Derived(dependencies) => {
                if !(1..=MAX_DETERMINISTIC_JOIN_INPUTS_V1).contains(&dependencies.len()) {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a deterministic scalar summary has missing or excessive dependencies",
                    ));
                }
                reserve_operation(self.operations)?;
                let result = next_value_id(self.next_value)?;
                self.operations
                    .push(ProductionRankedOperationV1::DeterministicJoin {
                        result,
                        dependencies,
                    });
                Ok(ProductionRankedValueV1::Local(result))
            }
        }
    }

    fn ranked_constant(
        &mut self,
        value: u64,
    ) -> Result<ProductionRankedValueV1, ProductionRankedProjectionErrorV1> {
        if let Some(value) = self.ranked_constants.get(&value).copied() {
            return Ok(value);
        }
        self.ranked_constants.try_reserve(1).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "deterministic scalar constant cache cannot be reserved",
            )
        })?;
        reserve_operation(self.operations)?;
        let result = next_value_id(self.next_value)?;
        let ranked = ProductionRankedValueV1::Local(result);
        self.operations
            .push(ProductionRankedOperationV1::IndexConstant { result, value });
        self.ranked_constants.insert(value, ranked);
        Ok(ranked)
    }

    fn ranked_argument(
        &mut self,
        origin: usize,
    ) -> Result<ProductionRankedValueV1, ProductionRankedProjectionErrorV1> {
        let slot = self.argument_slots.get_mut(origin).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a deterministic scalar argument origin is outside the semantic local table",
            ),
        )?;
        let argument = match *slot {
            Some(argument) => argument,
            None => {
                let argument = u32::try_from(*self.next_argument).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "too many deterministic scalar ranked arguments",
                    )
                })?;
                *self.next_argument = self.next_argument.checked_add(1).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "deterministic scalar ranked argument count overflow",
                    ),
                )?;
                *slot = Some(argument);
                argument
            }
        };
        Ok(ProductionRankedValueV1::Argument(argument))
    }
}

fn compiler_intrinsic_is_pure_total_scalar_dependency_v1(
    operation: &SemanticCompilerIntrinsicOperationV1,
) -> bool {
    matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::FabsF32
            | SemanticCompilerIntrinsicOperationV1::MathF32 { .. }
            | SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewRowMajor { .. }
            | SemanticCompilerIntrinsicOperationV1::StridedReadView2DFromSharedSlice { .. }
            | SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint { .. }
            | SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift { .. }
            | SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock { .. }
            | SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedTiled2d { .. }
            | SemanticCompilerIntrinsicOperationV1::ThreadIndexGet { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointIndexGet { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceLen { .. }
    )
}

const fn deterministic_index_binary_kind_v1(
    operation: SemanticBinaryOpV1,
) -> Option<IndexBinaryKindAttr> {
    match operation {
        SemanticBinaryOpV1::Add => Some(IndexBinaryKindAttr::Add),
        SemanticBinaryOpV1::Multiply => Some(IndexBinaryKindAttr::Multiply),
        SemanticBinaryOpV1::Divide => Some(IndexBinaryKindAttr::Divide),
        SemanticBinaryOpV1::Remainder => Some(IndexBinaryKindAttr::Remainder),
        SemanticBinaryOpV1::Subtract
        | SemanticBinaryOpV1::Offset
        | SemanticBinaryOpV1::BitXor
        | SemanticBinaryOpV1::BitAnd
        | SemanticBinaryOpV1::BitOr
        | SemanticBinaryOpV1::ShiftLeft
        | SemanticBinaryOpV1::ShiftRight
        | SemanticBinaryOpV1::Equal
        | SemanticBinaryOpV1::LessThan
        | SemanticBinaryOpV1::LessOrEqual
        | SemanticBinaryOpV1::NotEqual
        | SemanticBinaryOpV1::GreaterOrEqual
        | SemanticBinaryOpV1::GreaterThan => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn project_deterministic_scalar_switches_v1(
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    constants: &[Option<u64>],
    local_definitions: &[u8],
    index_values: &[Option<ProjectedDisjointIndexV1>],
    local_allocations: &[Option<AllocationContractV1>],
    predicates: &[Option<GuardPredicateV1>],
    argument_slots: &mut [Option<u32>],
    next_argument: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
) -> Result<Vec<Option<ProjectedDeterministicSwitchV1>>, ProductionRankedProjectionErrorV1> {
    if predicates.len() != function.locals().len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "deterministic scalar predicates do not match the semantic local table",
        ));
    }
    let mut projector = DeterministicScalarProjectorV1::new(
        callables,
        function,
        constants,
        local_definitions,
        index_values,
        local_allocations,
        predicates,
        argument_slots,
        next_argument,
        operations,
        next_value,
    )?;
    let mut switches = vec![None; function.blocks().len()];
    for (block_index, block) in function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = block.terminator().kind()
        else {
            continue;
        };
        if simple_operand_local(discriminant).is_some_and(|local| {
            predicates
                .get(local.index() as usize)
                .is_some_and(Option::is_some)
        }) {
            continue;
        }
        let Some(summary) = projector.resolve_operand(discriminant)? else {
            continue;
        };
        let discriminant = projector.materialize(summary)?;
        let mut projected_targets = Vec::new();
        projected_targets
            .try_reserve(targets.values().len())
            .map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "deterministic scalar switch target storage cannot be reserved",
                )
            })?;
        for target in targets.values() {
            let value = u64::try_from(target.value()).map_err(|_| {
                ProductionRankedProjectionErrorV1::Incomplete(
                    "a deterministic scalar switch variant does not fit ranked index width",
                )
            })?;
            let target_block = target.edge().target().index() as usize;
            if target_block >= function.blocks().len() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a deterministic scalar switch target is outside the semantic CFG",
                ));
            }
            projected_targets.push((projector.ranked_constant(value)?, target_block));
        }
        let otherwise = targets.otherwise().target().index() as usize;
        if otherwise >= function.blocks().len() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a deterministic scalar switch fallback is outside the semantic CFG",
            ));
        }
        switches[block_index] = Some(ProjectedDeterministicSwitchV1 {
            discriminant,
            targets: projected_targets,
            otherwise,
        });
    }
    Ok(switches)
}

#[allow(clippy::too_many_arguments)]
fn project_uniform_inductions_v1(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    constants: &[Option<u64>],
    stable_argument_origins: &[Option<u32>],
    local_definitions: &[u8],
    arguments: &mut [Option<u32>],
    next_argument: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
) -> Result<Vec<ProjectedUniformInductionV1>, ProductionRankedProjectionErrorV1> {
    let graph = projected_loop_cfg_graph_v1(function)?;
    let mut graph_work = 0_usize;
    let mut inductions = Vec::new();
    for (header, block) in function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } = block.terminator().kind()
        else {
            continue;
        };
        let Some(discriminant) = simple_operand_local(discriminant) else {
            continue;
        };
        if targets.values().len() != 1 || targets.values()[0].value() != 0 {
            continue;
        }
        let mut discriminant_definitions =
            block
                .statements()
                .iter()
                .enumerate()
                .filter_map(|(statement_index, statement)| {
                    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                        return None;
                    };
                    (assignment.destination().projections().is_empty()
                        && assignment.destination().local() == discriminant)
                        .then_some((statement_index, assignment))
                });
        let Some((comparison_index, comparison)) = discriminant_definitions.next() else {
            continue;
        };
        if discriminant_definitions.next().is_some() {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a uniform induction comparison with multiple header definitions",
            ));
        }
        let SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left,
            right: bound_operand,
        } = comparison.value().kind()
        else {
            continue;
        };
        let Some(induction) =
            resolve_loop_header_copy_alias_v1(block, comparison_index, left, local_definitions)?
        else {
            continue;
        };
        let body_entry = targets.otherwise().target().index() as usize;
        let exit = targets.values()[0].edge().target().index() as usize;
        if body_entry >= function.blocks().len() || exit >= function.blocks().len() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a uniform induction successor outside the semantic CFG",
            ));
        }
        let Some(topology) =
            project_natural_loop_topology_v1(&graph, header, body_entry, exit, &mut graph_work)?
        else {
            continue;
        };
        let mut initial = None;
        for statement in function.blocks()[topology.preheader].statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if assignment.destination().projections().is_empty()
                && assignment.destination().local() == induction
            {
                let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a uniform induction has a non-canonical preheader definition",
                    ));
                };
                if initial.replace(operand).is_some() {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a uniform induction has multiple preheader definitions",
                    ));
                }
            }
        }
        let mut step = None;
        for statement in function.blocks()[topology.latch].statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if assignment.destination().projections().is_empty()
                && assignment.destination().local() == induction
            {
                let candidate = match assignment.value().kind() {
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left,
                        right,
                    } if simple_operand_local(left) == Some(induction) => Some(right),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left,
                        right,
                    } if simple_operand_local(right) == Some(induction) => Some(left),
                    _ => None,
                }
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "a uniform induction has a non-canonical latch definition",
                ))?;
                if step.replace(candidate).is_some() {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a uniform induction has multiple latch definitions",
                    ));
                }
            }
        }
        for candidate in topology.loop_blocks.iter().copied() {
            if candidate == topology.latch {
                continue;
            }
            for statement in function.blocks()[candidate].statements() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                if !assignment.destination().projections().is_empty()
                    || assignment.destination().local() != induction
                {
                    continue;
                }
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a uniform induction is modified outside its unique latch",
                ));
            }
        }
        let (Some(initial), Some(step)) = (initial, step) else {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a uniform induction without exact initial and latch definitions",
            ));
        };
        if positive_unsigned_constant_operand_v1(step, constants, types).is_none() {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a uniform induction whose positive step is not statically established",
            ));
        }
        let Some(initial) = project_uniform_switch_operand_v1(
            initial,
            constants,
            stable_argument_origins,
            local_definitions,
            function,
            arguments,
            next_argument,
            operations,
            next_value,
        )?
        else {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a uniform induction with a lane-varying initial value",
            ));
        };
        let Some(bound) = project_uniform_switch_operand_v1(
            bound_operand,
            constants,
            stable_argument_origins,
            local_definitions,
            function,
            arguments,
            next_argument,
            operations,
            next_value,
        )?
        else {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a uniform induction with a lane-varying bound",
            ));
        };
        let Some(step) = project_uniform_switch_operand_v1(
            step,
            constants,
            stable_argument_origins,
            local_definitions,
            function,
            arguments,
            next_argument,
            operations,
            next_value,
        )?
        else {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a uniform induction with a lane-varying step",
            ));
        };
        inductions.push(ProjectedUniformInductionV1 {
            preheader: topology.preheader,
            header,
            body_entry,
            latch: topology.latch,
            exit,
            loop_blocks: topology.loop_blocks,
            initial,
            bound,
            step,
        });
    }
    inductions.sort_by(|left, right| {
        right
            .loop_blocks
            .len()
            .cmp(&left.loop_blocks.len())
            .then_with(|| left.header.cmp(&right.header))
    });
    for (index, left) in inductions.iter().enumerate() {
        for right in &inductions[index + 1..] {
            let overlaps = left
                .loop_blocks
                .iter()
                .any(|block| right.contains_block(*block));
            let right_is_nested = right
                .loop_blocks
                .iter()
                .all(|block| left.contains_block(*block));
            if overlaps && !right_is_nested {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "partially overlapping uniform induction regions",
                ));
            }
        }
    }
    Ok(inductions)
}

fn resolve_loop_header_copy_alias_v1(
    header: &fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1,
    comparison_index: usize,
    operand: &SemanticOperandV1,
    local_definitions: &[u8],
) -> Result<Option<SemanticLocalIdV1>, ProductionRankedProjectionErrorV1> {
    let Some(mut current) = simple_operand_local(operand) else {
        return Ok(None);
    };
    for _ in 0..=comparison_index {
        let current_index = current.index() as usize;
        if local_definitions.get(current_index).copied() != Some(1) {
            return Ok(Some(current));
        }
        let alias = header.statements()[..comparison_index]
            .iter()
            .rev()
            .find_map(|statement| {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    return None;
                };
                (assignment.destination().projections().is_empty()
                    && assignment.destination().local() == current)
                    .then_some(assignment)
            });
        let Some(alias) = alias else {
            return Ok(Some(current));
        };
        let SemanticRvalueKindV1::Use(source) = alias.value().kind() else {
            return Ok(Some(current));
        };
        if alias.destination().ty() != source.ty() {
            return Ok(Some(current));
        }
        let Some(source) = simple_operand_local(source) else {
            return Ok(Some(current));
        };
        current = source;
    }
    Err(ProductionRankedProjectionErrorV1::Incomplete(
        "a uniform induction comparison has a cyclic copy alias",
    ))
}

#[derive(Debug)]
struct ProjectedLoopCfgV1 {
    successors: Vec<Vec<usize>>,
    predecessors: Vec<Vec<usize>>,
    reachable: Vec<bool>,
    entry: usize,
}

#[derive(Debug)]
struct ProjectedNaturalLoopTopologyV1 {
    preheader: usize,
    latch: usize,
    loop_blocks: Vec<usize>,
}

fn project_loop_graph_charge_v1(
    work: &mut usize,
    amount: usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    *work = work
        .checked_add(amount)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis work overflow",
        ))?;
    if *work > MAX_PROJECTED_LOOP_GRAPH_WORK_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "uniform induction CFG analysis exceeds its work limit",
        ));
    }
    Ok(())
}

fn projected_loop_cfg_graph_v1(
    function: &SemanticFunctionDeclV1,
) -> Result<ProjectedLoopCfgV1, ProductionRankedProjectionErrorV1> {
    let block_count = function.blocks().len();
    if block_count == 0 || block_count > MAX_RANKED_BOUNDS_BLOCKS {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG exceeds the ranked block limit before loop analysis",
        ));
    }
    let checked_target = |target: SemanticBlockIdV1| {
        let target = target.index() as usize;
        (target < block_count).then_some(target).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a semantic CFG edge outside the function during loop analysis",
            ),
        )
    };
    let mut successors = Vec::with_capacity(block_count);
    let mut edge_count = 0_usize;
    for block in function.blocks() {
        let mut block_successors = match block.terminator().kind() {
            SemanticTerminatorKindV1::Goto(edge) => vec![checked_target(edge.target())?],
            SemanticTerminatorKindV1::SwitchInt { targets, .. } => {
                let mut successors = targets
                    .values()
                    .iter()
                    .map(|target| checked_target(target.edge().target()))
                    .collect::<Result<Vec<_>, _>>()?;
                let otherwise = checked_target(targets.otherwise().target())?;
                if !(targets.values().len() == 2
                    && targets.values().iter().any(|target| target.value() == 0)
                    && targets.values().iter().any(|target| target.value() == 1)
                    && switch_fallback_is_empty_unreachable_v1(function, otherwise))
                {
                    successors.push(otherwise);
                }
                successors
            }
            SemanticTerminatorKindV1::Call(call) => call
                .destination()
                .map(|destination| checked_target(destination.edge().target()))
                .transpose()?
                .into_iter()
                .collect(),
            SemanticTerminatorKindV1::Assert { target, .. }
            | SemanticTerminatorKindV1::Drop { target, .. } => {
                vec![checked_target(target.target())?]
            }
            SemanticTerminatorKindV1::FalseEdge { .. } => {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a false edge before uniform induction CFG normalization",
                ));
            }
            SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::TailCall(_)
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => Vec::new(),
        };
        block_successors.sort_unstable();
        block_successors.dedup();
        edge_count = edge_count.checked_add(block_successors.len()).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG edge count overflow during loop analysis",
            ),
        )?;
        if edge_count > MAX_RANKED_BOUNDS_EDGES {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG exceeds the ranked edge limit before loop analysis",
            ));
        }
        successors.push(block_successors);
    }
    let mut predecessors = vec![Vec::new(); block_count];
    for (source, targets) in successors.iter().enumerate() {
        for &target in targets {
            predecessors[target].push(source);
        }
    }
    let entry = function.entry().index() as usize;
    if entry >= block_count {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic entry block outside the function during loop analysis",
        ));
    }
    let mut reachable = vec![false; block_count];
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        if reachable[block] {
            continue;
        }
        reachable[block] = true;
        pending.extend(successors[block].iter().copied());
    }
    Ok(ProjectedLoopCfgV1 {
        successors,
        predecessors,
        reachable,
        entry,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UnsignedRangeProofV1 {
    minimum: u128,
    maximum: u128,
}

impl UnsignedRangeProofV1 {
    const fn exact(value: u128) -> Self {
        Self {
            minimum: value,
            maximum: value,
        }
    }

    const fn is_exact(self, value: u128) -> bool {
        self.minimum == value && self.maximum == value
    }
}

#[derive(Clone, Copy, Debug)]
struct ScalarAssignmentSiteV1 {
    block: usize,
    statement: usize,
}

struct SemanticAssertProofsV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    graph: ProjectedLoopCfgV1,
    definition_counts: Vec<u8>,
    assignments: Vec<Option<ScalarAssignmentSiteV1>>,
    dominance: HashMap<(usize, usize), bool>,
    zero_exclusion: HashMap<(usize, usize), bool>,
    work: usize,
}

impl<'a> SemanticAssertProofsV1<'a> {
    fn analyze(
        types: &'a [SemanticTypeDeclV1],
        function: &'a SemanticFunctionDeclV1,
    ) -> Result<Vec<bool>, ProductionRankedProjectionErrorV1> {
        let graph = projected_loop_cfg_graph_v1(function)?;
        let definition_counts = local_definition_counts(function);
        let mut assignments = vec![None; function.locals().len()];
        for (block_index, block) in function.blocks().iter().enumerate() {
            for (statement_index, statement) in block.statements().iter().enumerate() {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                if !assignment.destination().projections().is_empty() {
                    continue;
                }
                let local = assignment.destination().local().index() as usize;
                if definition_counts.get(local).copied() == Some(1) {
                    let Some(slot) = assignments.get_mut(local) else {
                        return Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "an assertion proof assignment is outside the semantic local table",
                        ));
                    };
                    *slot = Some(ScalarAssignmentSiteV1 {
                        block: block_index,
                        statement: statement_index,
                    });
                }
            }
        }
        let mut proof = Self {
            types,
            function,
            graph,
            definition_counts,
            assignments,
            dominance: HashMap::new(),
            zero_exclusion: HashMap::new(),
            work: 0,
        };
        let mut proved = vec![false; function.blocks().len()];
        for (block_index, block) in function.blocks().iter().enumerate() {
            let SemanticTerminatorKindV1::Assert {
                condition,
                expected,
                message,
                ..
            } = block.terminator().kind()
            else {
                continue;
            };
            if matches!(message, SemanticAssertMessageV1::BoundsCheck { .. }) {
                proved[block_index] = true;
                continue;
            }
            let mut visiting = HashSet::new();
            proved[block_index] = proof
                .range_of_operand(
                    condition,
                    block_index,
                    block.statements().len(),
                    &mut visiting,
                )?
                .is_some_and(|range| range.is_exact(u128::from(*expected)));
        }
        Ok(proved)
    }

    fn charge(&mut self, amount: usize) -> Result<(), ProductionRankedProjectionErrorV1> {
        project_loop_graph_charge_v1(&mut self.work, amount)
    }

    fn scalar_unsigned_maximum(&self, ty: SemanticTypeIdV1) -> Option<u128> {
        match self.types.get(ty.index() as usize)?.shape() {
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool) => Some(1),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits,
            }) => match *bits {
                1..=127 => Some((1_u128 << bits) - 1),
                128 => Some(u128::MAX),
                _ => None,
            },
            _ => None,
        }
    }

    fn unsigned_integer_bits(&self, ty: SemanticTypeIdV1) -> Option<u16> {
        match self.types.get(ty.index() as usize)?.shape() {
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits,
            }) => Some(*bits),
            _ => None,
        }
    }

    fn range_of_operand(
        &mut self,
        operand: &SemanticOperandV1,
        use_block: usize,
        use_statement: usize,
        visiting: &mut HashSet<usize>,
    ) -> Result<Option<UnsignedRangeProofV1>, ProductionRankedProjectionErrorV1> {
        self.charge(1)?;
        match operand {
            SemanticOperandV1::Constant(constant) => {
                let Some(maximum) = self.scalar_unsigned_maximum(constant.ty()) else {
                    return Ok(None);
                };
                let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                    return Ok(None);
                };
                (value.bits() <= maximum)
                    .then_some(UnsignedRangeProofV1::exact(value.bits()))
                    .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                        "an unsigned scalar constant exceeds its semantic type",
                    ))
                    .map(Some)
            }
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                if !place.projections().is_empty() {
                    return Ok(None);
                }
                self.range_of_local(
                    place.local().index() as usize,
                    use_block,
                    use_statement,
                    visiting,
                )
            }
        }
    }

    fn range_of_local(
        &mut self,
        local: usize,
        use_block: usize,
        use_statement: usize,
        visiting: &mut HashSet<usize>,
    ) -> Result<Option<UnsignedRangeProofV1>, ProductionRankedProjectionErrorV1> {
        self.charge(1)?;
        let Some(declaration) = self.function.locals().get(local) else {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an assertion proof local is outside the semantic local table",
            ));
        };
        let Some(maximum) = self.scalar_unsigned_maximum(declaration.ty()) else {
            return Ok(None);
        };
        if !visiting.insert(local) {
            return Ok(None);
        }
        let result = match self.definition_counts.get(local).copied() {
            Some(0) if matches!(declaration.role(), SemanticLocalRoleV1::Argument(_)) => {
                let minimum = if self.zero_excluding_edge_dominates(local, use_block)? {
                    1
                } else {
                    0
                };
                Some(UnsignedRangeProofV1 { minimum, maximum })
            }
            Some(1) => {
                let Some(site) = self.assignments.get(local).copied().flatten() else {
                    visiting.remove(&local);
                    return Ok(None);
                };
                if !self.assignment_dominates_use(site, use_block, use_statement)? {
                    visiting.remove(&local);
                    return Ok(None);
                }
                let SemanticStatementKindV1::Assign(assignment) =
                    self.function.blocks()[site.block].statements()[site.statement].kind()
                else {
                    visiting.remove(&local);
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "an indexed assertion proof assignment changed semantic kind",
                    ));
                };
                self.range_of_rvalue(assignment.value(), site, visiting)?
            }
            Some(_) | None => None,
        };
        let result = if let Some(mut range) = result
            && self.zero_excluding_edge_dominates(local, use_block)?
        {
            range.minimum = range.minimum.max(1);
            Some(range)
        } else {
            result
        };
        visiting.remove(&local);
        Ok(result)
    }

    fn range_of_rvalue(
        &mut self,
        value: &fe2o3_mir_model::semantic_mir_v1::SemanticRvalueV1,
        site: ScalarAssignmentSiteV1,
        visiting: &mut HashSet<usize>,
    ) -> Result<Option<UnsignedRangeProofV1>, ProductionRankedProjectionErrorV1> {
        let destination_maximum = self.scalar_unsigned_maximum(value.result_type());
        match value.kind() {
            SemanticRvalueKindV1::Use(operand) => {
                self.range_of_operand(operand, site.block, site.statement, visiting)
            }
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand,
            } => {
                let Some(source_bits) = self.unsigned_integer_bits(operand.ty()) else {
                    return Ok(None);
                };
                let Some(destination_bits) = self.unsigned_integer_bits(value.result_type()) else {
                    return Ok(None);
                };
                if destination_bits < source_bits {
                    return Ok(None);
                }
                self.range_of_operand(operand, site.block, site.statement, visiting)
            }
            SemanticRvalueKindV1::Binary {
                operation,
                left,
                right,
            } => {
                let left = self.range_of_operand(left, site.block, site.statement, visiting)?;
                let right = self.range_of_operand(right, site.block, site.statement, visiting)?;
                self.range_of_binary(*operation, left, right, destination_maximum)
            }
            _ => Ok(None),
        }
    }

    fn range_of_binary(
        &self,
        operation: SemanticBinaryOpV1,
        left: Option<UnsignedRangeProofV1>,
        right: Option<UnsignedRangeProofV1>,
        destination_maximum: Option<u128>,
    ) -> Result<Option<UnsignedRangeProofV1>, ProductionRankedProjectionErrorV1> {
        let (Some(left), Some(right)) = (left, right) else {
            return Ok(None);
        };
        let range = match operation {
            SemanticBinaryOpV1::Add => {
                let (Some(minimum), Some(maximum), Some(destination_maximum)) = (
                    left.minimum.checked_add(right.minimum),
                    left.maximum.checked_add(right.maximum),
                    destination_maximum,
                ) else {
                    return Ok(None);
                };
                (maximum <= destination_maximum)
                    .then_some(UnsignedRangeProofV1 { minimum, maximum })
            }
            SemanticBinaryOpV1::Divide if right.minimum != 0 => Some(UnsignedRangeProofV1 {
                minimum: left.minimum / right.maximum,
                maximum: left.maximum / right.minimum,
            }),
            SemanticBinaryOpV1::Remainder
                if right.is_exact(right.minimum) && right.minimum != 0 =>
            {
                Some(UnsignedRangeProofV1 {
                    minimum: 0,
                    maximum: left.maximum.min(right.minimum - 1),
                })
            }
            SemanticBinaryOpV1::Equal => Some(
                if left.maximum < right.minimum || right.maximum < left.minimum {
                    UnsignedRangeProofV1::exact(0)
                } else if left.minimum == left.maximum && left == right {
                    UnsignedRangeProofV1::exact(1)
                } else {
                    UnsignedRangeProofV1 {
                        minimum: 0,
                        maximum: 1,
                    }
                },
            ),
            SemanticBinaryOpV1::NotEqual => Some(
                if left.maximum < right.minimum || right.maximum < left.minimum {
                    UnsignedRangeProofV1::exact(1)
                } else if left.minimum == left.maximum && left == right {
                    UnsignedRangeProofV1::exact(0)
                } else {
                    UnsignedRangeProofV1 {
                        minimum: 0,
                        maximum: 1,
                    }
                },
            ),
            _ => None,
        };
        Ok(range)
    }

    fn assignment_dominates_use(
        &mut self,
        definition: ScalarAssignmentSiteV1,
        use_block: usize,
        use_statement: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        if definition.block == use_block {
            return Ok(definition.statement < use_statement);
        }
        self.block_dominates(definition.block, use_block)
    }

    fn zero_excluding_edge_dominates(
        &mut self,
        local: usize,
        use_block: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        if let Some(result) = self.zero_exclusion.get(&(local, use_block)).copied() {
            return Ok(result);
        }
        let mut excluding_edges = HashSet::new();
        for (switch_block, block) in self.function.blocks().iter().enumerate() {
            self.charge(1)?;
            let SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } = block.terminator().kind()
            else {
                continue;
            };
            if simple_operand_local(discriminant).map(|value| value.index() as usize) != Some(local)
                || targets.values().len() != 1
                || targets.values()[0].value() != 0
            {
                continue;
            }
            let definition_available = match self.definition_counts.get(local).copied() {
                Some(0) => true,
                Some(1) => {
                    let Some(site) = self.assignments.get(local).copied().flatten() else {
                        continue;
                    };
                    self.assignment_dominates_use(site, switch_block, block.statements().len())?
                }
                Some(_) | None => false,
            };
            if !definition_available {
                continue;
            }
            let zero_target = targets.values()[0].edge().target().index() as usize;
            let nonzero_target = targets.otherwise().target().index() as usize;
            if zero_target == nonzero_target {
                continue;
            }
            excluding_edges.try_reserve(1).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "assertion proof zero-excluding edge storage cannot be reserved",
                )
            })?;
            excluding_edges.insert((switch_block, nonzero_target));
        }
        let result = self.edge_set_dominates(&excluding_edges, use_block)?;
        self.zero_exclusion.insert((local, use_block), result);
        Ok(result)
    }

    fn block_dominates(
        &mut self,
        dominator: usize,
        block: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        if dominator >= self.graph.successors.len() || block >= self.graph.successors.len() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an assertion proof dominance query is outside the semantic CFG",
            ));
        }
        if let Some(result) = self.dominance.get(&(dominator, block)).copied() {
            return Ok(result);
        }
        if dominator == block {
            let result = self.graph.reachable[block];
            self.dominance.insert((dominator, block), result);
            return Ok(result);
        }
        if !self.graph.reachable[dominator] || !self.graph.reachable[block] {
            self.dominance.insert((dominator, block), false);
            return Ok(false);
        }
        let mut visited = vec![false; self.graph.successors.len()];
        let mut pending = Vec::new();
        if self.graph.entry != dominator {
            pending.push(self.graph.entry);
        }
        while let Some(current) = pending.pop() {
            self.charge(1)?;
            if current == dominator || visited[current] {
                continue;
            }
            if current == block {
                self.dominance.insert((dominator, block), false);
                return Ok(false);
            }
            visited[current] = true;
            self.charge(self.graph.successors[current].len())?;
            pending.extend(self.graph.successors[current].iter().copied());
        }
        self.dominance.insert((dominator, block), true);
        Ok(true)
    }

    fn edge_set_dominates(
        &mut self,
        excluding_edges: &HashSet<(usize, usize)>,
        block: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        if block >= self.graph.successors.len() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an assertion proof edge-set dominance query is outside the semantic CFG",
            ));
        }
        if excluding_edges.is_empty() || !self.graph.reachable[block] {
            return Ok(false);
        }
        let mut visited = vec![false; self.graph.successors.len()];
        let mut pending = vec![self.graph.entry];
        while let Some(current) = pending.pop() {
            self.charge(1)?;
            if visited[current] {
                continue;
            }
            if current == block {
                return Ok(false);
            }
            visited[current] = true;
            let successors = self.graph.successors[current].clone();
            self.charge(successors.len())?;
            for successor in successors {
                if !excluding_edges.contains(&(current, successor)) {
                    pending.push(successor);
                }
            }
        }
        Ok(true)
    }
}

#[allow(clippy::too_many_arguments)]
fn project_natural_loop_topology_v1(
    graph: &ProjectedLoopCfgV1,
    header: usize,
    body_entry: usize,
    exit: usize,
    work: &mut usize,
) -> Result<Option<ProjectedNaturalLoopTopologyV1>, ProductionRankedProjectionErrorV1> {
    if !graph.reachable.get(header).copied().unwrap_or(false) {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a uniform induction header is unreachable",
        ));
    }
    let mut reachable_without_header = vec![false; graph.successors.len()];
    let mut pending = Vec::new();
    if graph.entry != header {
        pending.push(graph.entry);
    }
    while let Some(block) = pending.pop() {
        project_loop_graph_charge_v1(work, 1)?;
        if block == header || reachable_without_header[block] {
            continue;
        }
        reachable_without_header[block] = true;
        project_loop_graph_charge_v1(work, graph.successors[block].len())?;
        pending.extend(graph.successors[block].iter().copied());
    }
    let header_predecessors = graph.predecessors[header]
        .iter()
        .copied()
        .filter(|predecessor| graph.reachable[*predecessor])
        .collect::<Vec<_>>();
    let backedges = header_predecessors
        .iter()
        .copied()
        .filter(|predecessor| !reachable_without_header[*predecessor])
        .collect::<Vec<_>>();
    let preheaders = header_predecessors
        .iter()
        .copied()
        .filter(|predecessor| reachable_without_header[*predecessor])
        .collect::<Vec<_>>();
    if backedges.is_empty() {
        return Ok(None);
    }
    if backedges.len() != 1 {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a uniform induction without one unique dominated backedge",
        ));
    }
    if preheaders.len() != 1 {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a uniform induction without one unique preheader",
        ));
    }
    let latch = backedges[0];
    let preheader = preheaders[0];
    if graph.successors[preheader].as_slice() != [header]
        || graph.successors[latch].as_slice() != [header]
    {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a uniform induction preheader or latch has non-canonical control",
        ));
    }
    let mut in_loop = vec![false; graph.successors.len()];
    in_loop[header] = true;
    in_loop[latch] = true;
    let mut pending = vec![latch];
    while let Some(block) = pending.pop() {
        project_loop_graph_charge_v1(work, 1)?;
        if block == header {
            continue;
        }
        project_loop_graph_charge_v1(work, graph.predecessors[block].len())?;
        for &predecessor in &graph.predecessors[block] {
            if !graph.reachable[predecessor] || in_loop[predecessor] {
                continue;
            }
            in_loop[predecessor] = true;
            pending.push(predecessor);
        }
    }
    if !in_loop.get(body_entry).copied().unwrap_or(false)
        || in_loop.get(exit).copied().unwrap_or(false)
    {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a uniform induction body and exit do not form a natural loop",
        ));
    }
    for (block, &inside) in in_loop.iter().enumerate() {
        if !inside {
            continue;
        }
        if reachable_without_header[block] {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "an irreducible entry enters a uniform induction body",
            ));
        }
        for &predecessor in &graph.predecessors[block] {
            if graph.reachable[predecessor]
                && !in_loop[predecessor]
                && !(block == header && predecessor == preheader)
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a uniform induction region has more than one entry",
                ));
            }
        }
    }
    let mut exits = Vec::new();
    for (source, &inside) in in_loop.iter().enumerate() {
        if !inside {
            continue;
        }
        project_loop_graph_charge_v1(work, graph.successors[source].len())?;
        for &target in &graph.successors[source] {
            if !in_loop[target] {
                exits.push((source, target));
            }
        }
    }
    if exits.as_slice() != [(header, exit)] {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a uniform induction region does not have one unique header exit",
        ));
    }
    let loop_blocks = in_loop
        .iter()
        .enumerate()
        .filter_map(|(block, inside)| inside.then_some(block))
        .collect();
    Ok(Some(ProjectedNaturalLoopTopologyV1 {
        preheader,
        latch,
        loop_blocks,
    }))
}

#[allow(clippy::too_many_arguments)]
fn project_uniform_switch_operand_v1(
    operand: &SemanticOperandV1,
    constants: &[Option<u64>],
    stable_argument_origins: &[Option<u32>],
    local_definitions: &[u8],
    function: &SemanticFunctionDeclV1,
    arguments: &mut [Option<u32>],
    next_argument: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
) -> Result<Option<ProductionRankedValueV1>, ProductionRankedProjectionErrorV1> {
    if let Some(value) = constant_operand_value(operand, constants) {
        reserve_operation(operations)?;
        let result = next_value_id(next_value)?;
        operations.push(ProductionRankedOperationV1::IndexConstant { result, value });
        return Ok(Some(ProductionRankedValueV1::Local(result)));
    }
    let Some(local) = simple_operand_local(operand) else {
        return Ok(None);
    };
    let local_index = local.index() as usize;
    let origin = stable_argument_origins
        .get(local_index)
        .copied()
        .flatten()
        .or_else(|| {
            (local_definitions.get(local_index).copied() == Some(0)
                && function
                    .locals()
                    .get(local_index)
                    .is_some_and(|local| matches!(local.role(), SemanticLocalRoleV1::Argument(_))))
            .then_some(local.index())
        });
    let Some(origin) = origin.map(|origin| origin as usize) else {
        return Ok(None);
    };
    let slot = arguments
        .get_mut(origin)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "a uniform switch argument origin outside the semantic local table",
        ))?;
    let argument = match *slot {
        Some(argument) => argument,
        None => {
            let argument = u32::try_from(*next_argument).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "too many uniform switch ranked arguments",
                )
            })?;
            *next_argument = next_argument.checked_add(1).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "uniform switch ranked argument count overflow",
                ),
            )?;
            *slot = Some(argument);
            argument
        }
    };
    Ok(Some(ProductionRankedValueV1::Argument(argument)))
}

fn project_runtime_index_operand_v1(
    operand: Option<&SemanticOperandV1>,
    constants: &[Option<u64>],
    stable_argument_origins: &[Option<u32>],
    arguments: &mut [Option<u32>],
    next_argument: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
) -> Result<ProductionRankedValueV1, ProductionRankedProjectionErrorV1> {
    let operand = operand.ok_or(ProductionRankedProjectionErrorV1::Unsupported(
        "a tiled-2d index operand is missing",
    ))?;
    if let Some(value) = constant_operand_value(operand, constants) {
        reserve_operation(operations)?;
        let result = next_value_id(next_value)?;
        operations.push(ProductionRankedOperationV1::IndexConstant { result, value });
        return Ok(ProductionRankedValueV1::Local(result));
    }
    let local =
        simple_operand_local(operand).ok_or(ProductionRankedProjectionErrorV1::Incomplete(
            "a tiled-2d runtime index is not a constant or one exact kernel argument",
        ))?;
    let local_index = local.index() as usize;
    // Preserve stable kernel-argument aliases when available. Other exact MIR
    // locals are projected as opaque ranked arguments: the checked tiled
    // operation still proves its own component and extent bounds, while the
    // analysis makes no unsound claim about how that dynamic value was formed.
    let origin = stable_argument_origins
        .get(local_index)
        .copied()
        .flatten()
        .unwrap_or(local.index()) as usize;
    let slot = arguments
        .get_mut(origin)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "a tiled-2d runtime argument origin is outside the semantic local table",
        ))?;
    let argument = match *slot {
        Some(argument) => argument,
        None => {
            let argument = u32::try_from(*next_argument).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported("too many tiled-2d ranked arguments")
            })?;
            *next_argument = next_argument.checked_add(1).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "tiled-2d ranked argument count overflow",
                ),
            )?;
            *slot = Some(argument);
            argument
        }
    };
    Ok(ProductionRankedValueV1::Argument(argument))
}

fn tiled_2d_geometry_valid_v1(
    lanes_per_tile: u64,
    tile_rows: u64,
    tile_columns: u64,
    elements_per_lane: u64,
) -> bool {
    lanes_per_tile != 0
        && tile_rows != 0
        && tile_columns != 0
        && elements_per_lane != 0
        && lanes_per_tile.is_multiple_of(tile_columns)
        && lanes_per_tile.checked_mul(elements_per_lane) == tile_rows.checked_mul(tile_columns)
        && (lanes_per_tile / tile_columns).checked_mul(elements_per_lane) == Some(tile_rows)
}

fn push_capability_edge(
    edges_by_source: &mut [Vec<CapabilityEdgeV1>],
    edge_count: &mut usize,
    source: usize,
    edge: CapabilityEdgeV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if source >= edges_by_source.len() || edge.destination >= edges_by_source.len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a capability def-use edge outside the semantic local table",
        ));
    }
    if *edge_count == MAX_PROJECTED_OPERATIONS_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "capability def-use edges exceed the charged projection limit",
        ));
    }
    edges_by_source[source].try_reserve(1).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "capability def-use edge storage cannot be reserved",
        )
    })?;
    edges_by_source[source].push(edge);
    *edge_count += 1;
    Ok(())
}

fn assign_index_capability(
    destination: usize,
    projected: ProjectedDisjointIndexV1,
    index_values: &mut [Option<ProjectedDisjointIndexV1>],
    grid_leaders: &[Option<ProjectedGridLeaderV1>],
    worklist: &mut VecDeque<usize>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if destination >= index_values.len() || grid_leaders[destination].is_some() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an index capability escaped the semantic local table or changed capability kind",
        ));
    }
    match index_values[destination] {
        None => {
            index_values[destination] = Some(projected);
            worklist.push_back(destination);
        }
        Some(existing) if existing == projected => {}
        Some(_) => {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "multiple index capabilities reach one semantic local",
            ));
        }
    }
    Ok(())
}

fn source_execution_layout_v1(
    architecture: SemanticTargetArchitectureV1,
    function: &SemanticFunctionDeclV1,
    source_rank: u8,
) -> Result<ProductionRankedOperationV1, ProductionRankedProjectionErrorV1> {
    let entry = function
        .kernel_entry()
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic kernel root is missing its authenticated entry contract",
        ))?;
    let required = entry
        .source_contract()
        .launch()
        .and_then(|launch| launch.required())
        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
            "concurrency verification requires exact source workgroup dimensions",
        ))?
        .as_array();
    let workgroup_extents = required.map(u64::from);
    let global_extents = match source_rank {
        1 if required[1] == 1 && required[2] == 1 => [0, 1, 1],
        2 if required[2] == 1 => [0, 0, 1],
        3 => [0, 0, 0],
        _ => {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "authenticated launch rank disagrees with source workgroup axes",
            ));
        }
    };
    let subgroup_size = match architecture {
        SemanticTargetArchitectureV1::AmdGpuGfx942 => 64,
    };
    let identity = entry.kernel_binding_identity().as_bytes()[..8]
        .try_into()
        .map(u64::from_le_bytes)
        .map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "the authenticated kernel identity cannot form a grid identity",
            )
        })?;
    Ok(ProductionRankedOperationV1::ExecutionLayout {
        grid_identity: identity,
        global_extents,
        workgroup_extents,
        subgroup_size,
        full_physical_workgroups: true,
    })
}

fn local_provenance_v1(
    function: &SemanticFunctionDeclV1,
) -> Result<LocalProvenanceV1, ProductionRankedProjectionErrorV1> {
    let definitions = local_definition_counts(function);
    let local_count = function.locals().len();
    let mut stable_argument_origins = vec![None; local_count];
    let mut allocation_origins = vec![None; local_count];
    let mut stable_edges = vec![Vec::new(); local_count];
    let mut allocation_edges = vec![Vec::new(); local_count];
    for (local_index, local) in function.locals().iter().enumerate() {
        if let SemanticLocalRoleV1::Argument(argument) = local.role() {
            stable_argument_origins[local_index] = Some(argument);
            allocation_origins[local_index] = Some(argument);
        }
    }
    let mut edge_count = 0_usize;
    for block in function.blocks() {
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let destination = assignment.destination();
            if !destination.projections().is_empty()
                || definitions
                    .get(destination.local().index() as usize)
                    .copied()
                    != Some(1)
            {
                continue;
            }
            let destination = destination.local().index() as usize;
            let stable_source = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) | SemanticRvalueKindV1::Cast { operand, .. } => {
                    simple_operand_local(operand)
                }
                _ => None,
            };
            if let Some(source) = stable_source {
                push_local_provenance_edge_v1(
                    &mut stable_edges,
                    source.index() as usize,
                    destination,
                    &mut edge_count,
                )?;
            };

            let allocation_source = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => simple_operand_local(operand),
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Pointer,
                    operand,
                } => simple_operand_local(operand),
                SemanticRvalueKindV1::Borrow { place, .. } => {
                    borrowed_allocation_local_v1(function, place)
                }
                SemanticRvalueKindV1::AddressOf { place, .. } => {
                    reborrowed_allocation_local_v1(place)
                }
                _ => None,
            };
            if let Some(source) = allocation_source {
                push_local_provenance_edge_v1(
                    &mut allocation_edges,
                    source.index() as usize,
                    destination,
                    &mut edge_count,
                )?;
            }
        }
    }

    propagate_exact_local_origins_v1(
        &mut stable_argument_origins,
        &stable_edges,
        "a runtime index may derive from multiple kernel arguments",
    )?;
    propagate_exact_local_origins_v1(
        &mut allocation_origins,
        &allocation_edges,
        "a local may alias multiple kernel allocation origins",
    )?;
    Ok(LocalProvenanceV1 {
        stable_argument_origins,
        allocation_origins,
    })
}

fn push_local_provenance_edge_v1(
    edges: &mut [Vec<usize>],
    source: usize,
    destination: usize,
    edge_count: &mut usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if source >= edges.len() || destination >= edges.len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a local provenance edge is outside the semantic local table",
        ));
    }
    *edge_count =
        edge_count
            .checked_add(1)
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "local provenance edge accounting overflowed",
            ))?;
    if *edge_count > MAX_PROJECTED_TENSOR_DATAFLOW_WORK_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "local provenance edges exceed the charged projection limit",
        ));
    }
    edges[source].try_reserve(1).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "local provenance edge storage cannot be reserved",
        )
    })?;
    edges[source].push(destination);
    Ok(())
}

fn propagate_exact_local_origins_v1<T: Copy + Eq>(
    origins: &mut [Option<T>],
    edges: &[Vec<usize>],
    conflict: &'static str,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let mut worklist = origins
        .iter()
        .enumerate()
        .filter_map(|(local, origin)| origin.map(|_| local))
        .collect::<VecDeque<_>>();
    let mut work = 0_usize;
    while let Some(source) = worklist.pop_front() {
        let Some(origin) = origins[source] else {
            continue;
        };
        for &destination in &edges[source] {
            work = work
                .checked_add(1)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "local provenance dataflow work accounting overflowed",
                ))?;
            if work > MAX_PROJECTED_TENSOR_DATAFLOW_WORK_V1 {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "local provenance dataflow exceeds the charged projection limit",
                ));
            }
            match origins[destination] {
                None => {
                    origins[destination] = Some(origin);
                    worklist.push_back(destination);
                }
                Some(existing) if existing == origin => {}
                Some(_) => {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(conflict));
                }
            }
        }
    }
    Ok(())
}

fn reborrowed_allocation_local_v1(place: &SemanticPlaceV1) -> Option<SemanticLocalIdV1> {
    let mut projections = place.projections().iter();
    matches!(
        projections.next().map(|projection| projection.kind()),
        Some(SemanticProjectionKindV1::Dereference)
    )
    .then(|| {
        projections
            .all(|projection| {
                matches!(
                    projection.kind(),
                    SemanticProjectionKindV1::Field(_)
                        | SemanticProjectionKindV1::Downcast(_)
                        | SemanticProjectionKindV1::OpaqueCast
                        | SemanticProjectionKindV1::Subtype
                )
            })
            .then_some(place.local())
    })
    .flatten()
}

fn borrowed_allocation_local_v1(
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
) -> Option<SemanticLocalIdV1> {
    if let Some(local) = reborrowed_allocation_local_v1(place) {
        return Some(local);
    }
    if !place.projections().is_empty() {
        return None;
    }
    let local = function.locals().get(place.local().index() as usize)?;
    let SemanticLocalRoleV1::Argument(argument) = local.role() else {
        return None;
    };
    matches!(
        function
            .abi()
            .source_argument_ownership()
            .get(argument as usize),
        Some(SemanticSourceArgumentOwnershipV1::ExclusiveOwner)
    )
    .then_some(place.local())
}

#[cfg(test)]
fn local_stable_argument_origins(
    function: &SemanticFunctionDeclV1,
) -> Result<Vec<Option<u32>>, ProductionRankedProjectionErrorV1> {
    Ok(local_provenance_v1(function)?.stable_argument_origins)
}

fn local_allocation_contracts(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    origins: &[Option<u32>],
) -> Result<Vec<Option<AllocationContractV1>>, ProductionRankedProjectionErrorV1> {
    if origins.len() != function.locals().len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "allocation-origin and semantic-local tables have different lengths",
        ));
    }
    let source_types = function.abi().source_input_types();
    let source_ownership = function.abi().source_argument_ownership();
    let abi_arguments = function.abi().adjusted_arguments();
    if source_ownership.len() != source_types.len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "source ownership and semantic argument tables have different lengths",
        ));
    }
    let mut arguments = vec![None; source_types.len()];
    for (argument_index, &ty) in source_types.iter().enumerate() {
        let type_decl = types.get(ty.index() as usize).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a kernel argument type is outside the semantic type table",
            ),
        )?;
        let abi_argument = abi_arguments.get(argument_index).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a kernel source argument is missing its authenticated FnAbi record",
            ),
        )?;
        let pointee = abi_argument
            .value()
            .pointee_override()
            .or(type_decl.abi_properties().first_pointee());
        let allocation_origin = u64::try_from(argument_index)
            .ok()
            .and_then(|index| index.checked_add(1))
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "a kernel argument index does not fit the allocation identity space",
            ))?;
        let first_pointer_noalias = match abi_argument.mode() {
            SemanticAbiPassModeV1::Direct(attributes) => attributes.regular().no_alias(),
            SemanticAbiPassModeV1::Pair { first, .. } => first.regular().no_alias(),
            SemanticAbiPassModeV1::Ignore
            | SemanticAbiPassModeV1::Cast { .. }
            | SemanticAbiPassModeV1::Indirect { .. } => false,
        };
        let Some(pointee) = pointee else {
            continue;
        };
        let abi_contract = allocation_contract_from_pointee(
            pointee.kind(),
            first_pointer_noalias,
            allocation_origin,
        );
        arguments[argument_index] = Some(authenticated_source_allocation_contract_v1(
            source_ownership[argument_index],
            pointee.kind(),
            abi_contract,
        )?);
    }
    Ok(origins
        .iter()
        .map(|origin| origin.and_then(|origin| arguments.get(origin as usize).copied().flatten()))
        .collect())
}

fn authenticated_source_allocation_contract_v1(
    ownership: SemanticSourceArgumentOwnershipV1,
    pointee: SemanticAbiPointeeKindV1,
    abi_contract: AllocationContractV1,
) -> Result<AllocationContractV1, ProductionRankedProjectionErrorV1> {
    Ok(match ownership {
        SemanticSourceArgumentOwnershipV1::UniqueBorrow
            if abi_contract.noalias_class != 0
                && matches!(
                    pointee,
                    SemanticAbiPointeeKindV1::MutableReference { .. }
                        | SemanticAbiPointeeKindV1::Box { .. }
                ) =>
        {
            abi_contract
        }
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner
            if matches!(pointee, SemanticAbiPointeeKindV1::Raw) =>
        {
            AllocationContractV1 {
                allocation_origin: abi_contract.allocation_origin,
                noalias_class: abi_contract.allocation_origin + 1,
                writable: true,
            }
        }
        SemanticSourceArgumentOwnershipV1::SharedBorrow
            if matches!(pointee, SemanticAbiPointeeKindV1::SharedReference { .. }) =>
        {
            abi_contract
        }
        SemanticSourceArgumentOwnershipV1::RawPointer
            if matches!(pointee, SemanticAbiPointeeKindV1::Raw) =>
        {
            abi_contract
        }
        SemanticSourceArgumentOwnershipV1::ByValue if abi_contract.noalias_class == 0 => {
            abi_contract
        }
        SemanticSourceArgumentOwnershipV1::UniqueBorrow
        | SemanticSourceArgumentOwnershipV1::ExclusiveOwner
        | SemanticSourceArgumentOwnershipV1::SharedBorrow
        | SemanticSourceArgumentOwnershipV1::RawPointer
        | SemanticSourceArgumentOwnershipV1::ByValue
        | SemanticSourceArgumentOwnershipV1::Unspecified => {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "source ownership disagrees with rustc ABI pointer provenance",
            ));
        }
    })
}

fn allocation_contract_from_pointee(
    pointee: SemanticAbiPointeeKindV1,
    first_pointer_noalias: bool,
    allocation_origin: u64,
) -> AllocationContractV1 {
    let (noalias_class, writable) = match pointee {
        SemanticAbiPointeeKindV1::SharedReference { frozen } => (1, !frozen),
        SemanticAbiPointeeKindV1::MutableReference { .. }
        | SemanticAbiPointeeKindV1::Box { .. }
            if first_pointer_noalias =>
        {
            (allocation_origin + 1, true)
        }
        SemanticAbiPointeeKindV1::MutableReference { .. }
        | SemanticAbiPointeeKindV1::Box { .. }
        | SemanticAbiPointeeKindV1::Raw => (0, true),
    };
    AllocationContractV1 {
        allocation_origin,
        noalias_class,
        writable,
    }
}

fn projected_disjoint_operand_v1(
    call: &SemanticDirectCallV1,
    argument: usize,
    values: &[Option<ProjectedDisjointIndexV1>],
    option_dominance: &SemanticOptionDominanceV1,
    enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
    use_block: usize,
) -> Result<ProjectedDisjointIndexV1, ProductionRankedProjectionErrorV1> {
    let local = call
        .arguments()
        .get(argument)
        .and_then(simple_operand_local)
        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
            "a checked disjoint access whose index witness is not one exact local",
        ))?;
    let projected = values
        .get(local.index() as usize)
        .copied()
        .flatten()
        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
            "a checked disjoint access not bound to authenticated index authority",
        ))?;
    if !projected.availability.is_none_or(|availability| {
        capability_availability_allows(
            option_dominance,
            enum_payload_dominance,
            availability,
            SemanticBlockIdV1::from_index(use_block as u32),
        )
    }) {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an index capability is used outside its authenticated Some edge",
        ));
    }
    Ok(projected)
}

fn capability_availability_allows(
    option: &SemanticOptionDominanceV1,
    enum_payload: &SemanticEnumPayloadDominanceV1,
    availability: CapabilityAvailabilityV1,
    block: SemanticBlockIdV1,
) -> bool {
    match availability {
        CapabilityAvailabilityV1::Option(availability) => option.allows(availability, block),
        CapabilityAvailabilityV1::EnumPayload(availability) => {
            enum_payload.allows(availability, block)
        }
    }
}

fn ranked_value_text_v1(value: ProductionRankedValueV1) -> String {
    match value {
        ProductionRankedValueV1::Local(identity) => format!("%{}", identity.get()),
        ProductionRankedValueV1::Argument(argument) => format!("%arg{argument}"),
        ProductionRankedValueV1::BlockArgument { block, argument } => {
            format!("%bb{block}_arg{argument}")
        }
    }
}

fn switch_predicates(
    function: &SemanticFunctionDeclV1,
    option_predicates: &[Option<GuardPredicateV1>],
    direct_switch_predicates: &[Option<GuardPredicateV1>],
) -> Result<Vec<Option<GuardPredicateV1>>, ProductionRankedProjectionErrorV1> {
    if direct_switch_predicates.len() != function.locals().len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "direct switch predicates do not match the semantic local table",
        ));
    }
    let mut predicates = direct_switch_predicates.to_vec();
    for block in function.blocks() {
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let destination = assignment.destination();
            if !destination.projections().is_empty() {
                continue;
            }
            let SemanticRvalueKindV1::Discriminant(source) = assignment.value().kind() else {
                continue;
            };
            if !source.projections().is_empty() {
                continue;
            }
            let Some(predicate) = option_predicates
                .get(source.local().index() as usize)
                .and_then(Clone::clone)
            else {
                continue;
            };
            let slot = predicates
                .get_mut(destination.local().index() as usize)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "an Option discriminant outside the semantic local table",
                ))?;
            if slot.is_some() {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "an Option discriminant local with multiple definitions",
                ));
            }
            *slot = Some(predicate);
        }
    }
    Ok(predicates)
}

fn operation_defines_value(operation: &ProductionRankedOperationV1) -> bool {
    matches!(
        operation,
        ProductionRankedOperationV1::View { .. }
            | ProductionRankedOperationV1::ViewInSpace { .. }
            | ProductionRankedOperationV1::IndexConstant { .. }
            | ProductionRankedOperationV1::IndexUnknown { .. }
            | ProductionRankedOperationV1::InvocationIndex { .. }
            | ProductionRankedOperationV1::IndexBinary { .. }
            | ProductionRankedOperationV1::CheckedTiledIndex2D { .. }
            | ProductionRankedOperationV1::Dimension { .. }
            | ProductionRankedOperationV1::SemanticSymbol { .. }
            | ProductionRankedOperationV1::SemanticConstant { .. }
            | ProductionRankedOperationV1::SemanticBinary { .. }
    )
}

fn bind_projected_access_site(
    sources: &mut [ProjectedAccessSourceV1],
    guarded_sites: &mut [GuardedAccessSiteV1],
    site: ProjectedSemanticAccessSiteV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    for source in sources {
        if source.semantic_site.replace(site).is_some() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a projected access was attributed to multiple semantic sites",
            ));
        }
    }
    for guarded in guarded_sites {
        if guarded.access.semantic_site.replace(site).is_some() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a guarded access was attributed to multiple semantic sites",
            ));
        }
    }
    Ok(())
}

fn order_projected_block_effects(
    operations: Vec<ProductionRankedOperationV1>,
    guarded_sites: Vec<GuardedAccessSiteV1>,
    sources: Vec<ProjectedAccessSourceV1>,
    entry_operations: &mut Vec<ProductionRankedOperationV1>,
) -> Result<ProjectedSemanticBlockV1, ProductionRankedProjectionErrorV1> {
    let mut source_at = vec![None; operations.len()];
    for source in sources {
        let slot = source_at.get_mut(source.operation).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a projected access source outside its semantic block",
            ),
        )?;
        if slot.is_some() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "multiple projected sources for one ranked effect",
            ));
        }
        *slot = Some(ProjectedEffectSourceV1 {
            access: source.access,
            memory_space: source.memory_space,
            source: source.source,
            semantic_site: source.semantic_site,
        });
    }
    let mut sites = guarded_sites.into_iter().peekable();
    let mut items = Vec::new();
    for (index, operation) in operations.into_iter().enumerate() {
        while sites
            .peek()
            .is_some_and(|site| site.insertion_operation == index)
        {
            items.push(ProjectedBlockItemV1::Guarded(
                sites.next().expect("peeked guarded access site").access,
            ));
        }
        if sites
            .peek()
            .is_some_and(|site| site.insertion_operation < index)
        {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "checked access sites are not in semantic statement order",
            ));
        }
        let source = source_at[index];
        if operation_defines_value(&operation) {
            if source.is_some() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a pure ranked definition carried a memory-effect source",
                ));
            }
            reserve_operation(entry_operations)?;
            entry_operations.push(operation);
        } else {
            items.push(ProjectedBlockItemV1::Effect { operation, source });
        }
    }
    for site in sites {
        if site.insertion_operation != source_at.len() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a checked access site is outside its semantic block",
            ));
        }
        items.push(ProjectedBlockItemV1::Guarded(site.access));
    }
    Ok(ProjectedSemanticBlockV1 { items })
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProjectedCfgTerminatorV1 {
    Branch(usize),
    Predicate {
        predicate: GuardPredicateV1,
        true_block: usize,
        false_block: usize,
    },
    AnalysisSplit {
        first_block: usize,
        second_block: usize,
    },
    ExactSwitch(ProjectedDeterministicSwitchV1),
    Return,
}

fn projected_cfg_terminator(
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    non_bounds_assert_proved: bool,
    constants: &[Option<u64>],
    switch_predicates: &[Option<GuardPredicateV1>],
    deterministic_switches: &[Option<ProjectedDeterministicSwitchV1>],
) -> Result<ProjectedCfgTerminatorV1, ProductionRankedProjectionErrorV1> {
    let block = function.blocks().get(block_index).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported("a semantic CFG block outside the function"),
    )?;
    let target = |target: fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1| {
        let target = target.index() as usize;
        (target < function.blocks().len()).then_some(target).ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a semantic CFG edge outside the function",
            ),
        )
    };
    match block.terminator().kind() {
        SemanticTerminatorKindV1::Goto(edge) => {
            Ok(ProjectedCfgTerminatorV1::Branch(target(edge.target())?))
        }
        SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } => {
            let predicate = simple_operand_local(discriminant).and_then(|discriminant| {
                switch_predicates
                    .get(discriminant.index() as usize)
                    .and_then(Clone::clone)
            });
            let Some(predicate) = predicate else {
                if let Some(projected) = deterministic_switches
                    .get(block_index)
                    .and_then(Clone::clone)
                {
                    return Ok(ProjectedCfgTerminatorV1::ExactSwitch(projected));
                }
                if targets.values().len() == 1 {
                    return Ok(ProjectedCfgTerminatorV1::AnalysisSplit {
                        first_block: target(targets.values()[0].edge().target())?,
                        second_block: target(targets.otherwise().target())?,
                    });
                }
                if targets.values().len() == 2
                    && targets.values()[0].value() == 0
                    && targets.values()[1].value() == 1
                    && switch_fallback_is_empty_unreachable_v1(
                        function,
                        target(targets.otherwise().target())?,
                    )
                {
                    return Ok(ProjectedCfgTerminatorV1::AnalysisSplit {
                        first_block: target(targets.values()[0].edge().target())?,
                        second_block: target(targets.values()[1].edge().target())?,
                    });
                }
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a general switch whose only extra successor is not one empty unreachable fallback",
                ));
            };
            if targets.values().len() == 1 {
                let explicit = &targets.values()[0];
                let explicit_block = target(explicit.edge().target())?;
                let otherwise_block = target(targets.otherwise().target())?;
                return match explicit.value() {
                    0 => Ok(ProjectedCfgTerminatorV1::Predicate {
                        predicate,
                        true_block: otherwise_block,
                        false_block: explicit_block,
                    }),
                    1 => Ok(ProjectedCfgTerminatorV1::Predicate {
                        predicate,
                        true_block: explicit_block,
                        false_block: otherwise_block,
                    }),
                    _ => Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a comparison predicate switch retained a non-boolean explicit value",
                    )),
                };
            }
            let zero = targets.values().iter().find(|target| target.value() == 0);
            let one = targets.values().iter().find(|target| target.value() == 1);
            if targets.values().len() != 2 || zero.is_none() || one.is_none() {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a comparison predicate switch whose exact boolean variants were not retained",
                ));
            }
            let otherwise = target(targets.otherwise().target())?;
            if !switch_fallback_is_empty_unreachable_v1(function, otherwise) {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a comparison predicate switch with a reachable non-boolean successor",
                ));
            }
            Ok(ProjectedCfgTerminatorV1::Predicate {
                predicate,
                true_block: target(one.expect("checked exact variant").edge().target())?,
                false_block: target(zero.expect("checked exact variant").edge().target())?,
            })
        }
        SemanticTerminatorKindV1::Call(call) => {
            if matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_)) {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a call with cleanup control flow before exact unwind projection",
                ));
            }
            match call.destination() {
                Some(destination) => Ok(ProjectedCfgTerminatorV1::Branch(target(
                    destination.edge().target(),
                )?)),
                None => Ok(ProjectedCfgTerminatorV1::Return),
            }
        }
        SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            message,
            target: edge,
            ..
        } => {
            if !matches!(message, SemanticAssertMessageV1::BoundsCheck { .. })
                && !non_bounds_assert_proved
                && constant_operand_value(condition, constants) != Some(u64::from(*expected))
            {
                return Err(ProductionRankedProjectionErrorV1::UnprovenAssert {
                    block: block_index,
                    kind: semantic_assert_kind_v1(message),
                    expected: *expected,
                    condition_local: simple_operand_local(condition).map(SemanticLocalIdV1::index),
                    source: block.terminator().source(),
                });
            }
            Ok(ProjectedCfgTerminatorV1::Branch(target(edge.target())?))
        }
        SemanticTerminatorKindV1::Drop { target: edge, .. } => {
            Ok(ProjectedCfgTerminatorV1::Branch(target(edge.target())?))
        }
        SemanticTerminatorKindV1::FalseEdge { .. } => {
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a false edge before exact semantic CFG normalization",
            ))
        }
        SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::TailCall(_)
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => Ok(ProjectedCfgTerminatorV1::Return),
    }
}

const fn semantic_assert_kind_v1(message: &SemanticAssertMessageV1) -> &'static str {
    match message {
        SemanticAssertMessageV1::BoundsCheck { .. } => "bounds-check",
        SemanticAssertMessageV1::Overflow { .. } => "arithmetic-overflow",
        SemanticAssertMessageV1::DivisionByZero(_) => "division-by-zero",
        SemanticAssertMessageV1::RemainderByZero(_) => "remainder-by-zero",
        SemanticAssertMessageV1::MisalignedPointerDereference { .. } => {
            "misaligned-pointer-dereference"
        }
        SemanticAssertMessageV1::NullPointerDereference => "null-pointer-dereference",
        SemanticAssertMessageV1::ResumedAfterReturn => "resumed-after-return",
        SemanticAssertMessageV1::ResumedAfterPanic => "resumed-after-panic",
    }
}

fn switch_fallback_is_empty_unreachable_v1(
    function: &SemanticFunctionDeclV1,
    block: usize,
) -> bool {
    function.blocks().get(block).is_some_and(|block| {
        block.statements().is_empty()
            && matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Unreachable
            )
    })
}

fn projected_block_expansion(
    block: &ProjectedSemanticBlockV1,
    terminator: &ProjectedCfgTerminatorV1,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    let mut count = 1_usize;
    for item in &block.items {
        if let ProjectedBlockItemV1::Guarded(access) = item {
            count = count
                .checked_add(GuardPredicateV1::for_access(access).comparisons.len() + 2)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "checked-access CFG block count overflow",
                ))?;
        }
    }
    if let ProjectedCfgTerminatorV1::Predicate { predicate, .. } = terminator {
        count = count
            .checked_add(predicate.comparisons.len().saturating_sub(1))
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG predicate block count overflow",
            ))?;
    }
    if let ProjectedCfgTerminatorV1::ExactSwitch(switch) = terminator {
        count = count
            .checked_add(switch.targets.len().saturating_sub(1))
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "deterministic switch CFG block count overflow",
            ))?;
    }
    Ok(count)
}

fn live_induction_block_arguments(
    block: u32,
    live: &[usize],
) -> Result<Vec<ProductionRankedValueV1>, ProductionRankedProjectionErrorV1> {
    live.iter()
        .enumerate()
        .map(|(argument, _)| {
            Ok(ProductionRankedValueV1::BlockArgument {
                block,
                argument: u32::try_from(argument).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "live induction argument count does not fit u32",
                    )
                })?,
            })
        })
        .collect()
}

fn forward_live_inductions(
    source_block: u32,
    source_live: &[usize],
    target_live: &[usize],
) -> Result<Vec<ProductionRankedValueV1>, ProductionRankedProjectionErrorV1> {
    target_live
        .iter()
        .map(|target| {
            let argument = source_live
                .iter()
                .position(|source| source == target)
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "a ranked edge introduces an induction outside its preheader",
                ))?;
            Ok(ProductionRankedValueV1::BlockArgument {
                block: source_block,
                argument: u32::try_from(argument).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "live induction argument count does not fit u32",
                    )
                })?,
            })
        })
        .collect()
}

fn build_ranked_cfg(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    switch_predicates: &[Option<GuardPredicateV1>],
    deterministic_switches: &[Option<ProjectedDeterministicSwitchV1>],
    uniform_inductions: &[ProjectedUniformInductionV1],
    entry_operations: Vec<ProductionRankedOperationV1>,
    projected_blocks: Vec<ProjectedSemanticBlockV1>,
) -> Result<
    (Vec<ProductionRankedBlockV1>, Vec<ProjectedAccessSourceV1>),
    ProductionRankedProjectionErrorV1,
> {
    if projected_blocks.len() != function.blocks().len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG projection lost a basic block",
        ));
    }
    let constants = constant_locals(function);
    let proved_assertions = SemanticAssertProofsV1::analyze(types, function)?;
    let terminators = (0..function.blocks().len())
        .map(|index| {
            projected_cfg_terminator(
                function,
                index,
                proved_assertions[index],
                &constants,
                switch_predicates,
                deterministic_switches,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let entry = function.entry().index() as usize;
    let reachable = reachable_projected_blocks(entry, &terminators)?;
    let live_inductions = (0..function.blocks().len())
        .map(|block| {
            uniform_inductions
                .iter()
                .enumerate()
                .filter_map(|(index, induction)| induction.contains_block(block).then_some(index))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut base_blocks = vec![None; projected_blocks.len()];
    let mut block_count = 1_usize;
    for (index, (block, terminator)) in projected_blocks.iter().zip(&terminators).enumerate() {
        if !reachable[index] {
            continue;
        }
        base_blocks[index] = Some(block_count);
        block_count = block_count
            .checked_add(projected_block_expansion(block, terminator)?)
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "semantic CFG block count overflow",
            ))?;
    }
    if block_count > fe2o3_kernel_analysis::MAX_RANKED_BOUNDS_BLOCKS {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG projection exceeds the ranked block limit",
        ));
    }
    let operation_count = projected_blocks
        .iter()
        .try_fold(entry_operations.len(), |count, block| {
            count.checked_add(block.items.len())
        })
        .and_then(|count| count.checked_add(block_count));
    if operation_count.is_none_or(|count| count > MAX_RANKED_BOUNDS_OPERATIONS) {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG projection exceeds the ranked operation limit",
        ));
    }
    let entry_target = base_blocks.get(entry).copied().flatten().ok_or(
        ProductionRankedProjectionErrorV1::Unsupported("semantic entry block outside the function"),
    )?;
    let mut blocks = Vec::with_capacity(block_count);
    blocks.push(ProductionRankedBlockV1::new(
        entry_operations,
        ProductionRankedTerminatorV1::Branch {
            target: ranked_block_id(entry_target)?,
        },
    ));
    let mut sources = Vec::new();
    for (semantic_index, (projected, terminator)) in
        projected_blocks.into_iter().zip(terminators).enumerate()
    {
        let Some(mut current) = base_blocks[semantic_index] else {
            continue;
        };
        let live = &live_inductions[semantic_index];
        let mut operations = Vec::new();
        for item in projected.items {
            match item {
                ProjectedBlockItemV1::Effect { operation, source } => {
                    if let Some(source) = source {
                        sources.push(ProjectedAccessSourceV1 {
                            block: current,
                            operation: operations.len(),
                            access: source.access,
                            memory_space: source.memory_space,
                            source: source.source,
                            semantic_site: source.semantic_site,
                        });
                    }
                    operations.push(operation);
                }
                ProjectedBlockItemV1::Guarded(access) => {
                    let predicate = GuardPredicateV1::for_access(&access);
                    let access_block = current + predicate.comparisons.len();
                    let failure_block = access_block + 1;
                    let continuation = failure_block + 1;
                    if !live.is_empty() {
                        append_predicate_blocks_with_index_arguments(
                            &mut blocks,
                            current,
                            operations,
                            &predicate,
                            access_block,
                            failure_block,
                            live.len(),
                        )?;
                    } else {
                        append_predicate_blocks(
                            &mut blocks,
                            current,
                            operations,
                            &predicate,
                            access_block,
                            failure_block,
                        )?;
                    }
                    let access_operations = vec![ProductionRankedOperationV1::Access {
                        kind: access.access,
                        view: ProductionRankedValueV1::Local(access.view),
                        indices: access.indices,
                    }];
                    if !live.is_empty() {
                        let block = ranked_block_id(access_block)?;
                        push_block_at_with_index_arguments(
                            &mut blocks,
                            access_block,
                            u32::try_from(live.len()).map_err(|_| {
                                ProductionRankedProjectionErrorV1::Unsupported(
                                    "live induction argument count does not fit u32",
                                )
                            })?,
                            access_operations,
                            ProductionRankedTerminatorV1::BranchArgs {
                                arguments: live_induction_block_arguments(block, live)?,
                                target: ranked_block_id(continuation)?,
                            },
                        )?;
                    } else {
                        push_block_at(
                            &mut blocks,
                            access_block,
                            access_operations,
                            ProductionRankedTerminatorV1::Branch {
                                target: ranked_block_id(continuation)?,
                            },
                        )?;
                    }
                    sources.push(ProjectedAccessSourceV1 {
                        block: access_block,
                        operation: 0,
                        access: access.access,
                        memory_space: access.memory_space,
                        source: access.source,
                        semantic_site: access.semantic_site,
                    });
                    push_block_at(
                        &mut blocks,
                        failure_block,
                        Vec::new(),
                        ProductionRankedTerminatorV1::Trap,
                    )?;
                    current = continuation;
                    operations = Vec::new();
                }
            }
        }
        if let Some((induction_index, induction)) = uniform_inductions
            .iter()
            .enumerate()
            .find(|(_, induction)| induction.preheader == semantic_index)
        {
            let block = ranked_block_id(current)?;
            let target_live = &live_inductions[induction.header];
            let mut arguments = Vec::with_capacity(target_live.len());
            for target_induction in target_live {
                if *target_induction == induction_index {
                    arguments.push(induction.initial);
                } else {
                    let source_argument = live
                        .iter()
                        .position(|source| source == target_induction)
                        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                            "a uniform induction preheader introduces an unrelated live induction",
                        ))?;
                    arguments.push(ProductionRankedValueV1::BlockArgument {
                        block,
                        argument: u32::try_from(source_argument).map_err(|_| {
                            ProductionRankedProjectionErrorV1::Unsupported(
                                "live induction argument count does not fit u32",
                            )
                        })?,
                    });
                }
            }
            push_block_at_with_index_arguments(
                &mut blocks,
                current,
                u32::try_from(live.len()).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "live induction argument count does not fit u32",
                    )
                })?,
                operations,
                ProductionRankedTerminatorV1::BranchArgs {
                    arguments,
                    target: ranked_block_id(projected_target(&base_blocks, induction.header)?)?,
                },
            )?;
            continue;
        }
        if let Some((induction_index, induction)) = uniform_inductions
            .iter()
            .enumerate()
            .find(|(_, induction)| induction.header == semantic_index)
        {
            let header = ranked_block_id(current)?;
            let body = projected_target(&base_blocks, induction.body_entry)?;
            let induction_argument = live
                .iter()
                .position(|candidate| *candidate == induction_index)
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "a uniform induction header does not carry its induction value",
                ))?;
            push_block_at_with_index_arguments(
                &mut blocks,
                current,
                u32::try_from(live.len()).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "live induction argument count does not fit u32",
                    )
                })?,
                operations,
                ProductionRankedTerminatorV1::IndexLessThanArgs {
                    lhs: ProductionRankedValueV1::BlockArgument {
                        block: header,
                        argument: u32::try_from(induction_argument).map_err(|_| {
                            ProductionRankedProjectionErrorV1::Unsupported(
                                "live induction argument count does not fit u32",
                            )
                        })?,
                    },
                    rhs: induction.bound,
                    true_arguments: forward_live_inductions(
                        header,
                        live,
                        &live_inductions[induction.body_entry],
                    )?,
                    false_arguments: forward_live_inductions(
                        header,
                        live,
                        &live_inductions[induction.exit],
                    )?,
                    true_block: ranked_block_id(body)?,
                    false_block: ranked_block_id(projected_target(&base_blocks, induction.exit)?)?,
                },
            )?;
            continue;
        }
        if let Some((induction_index, induction)) = uniform_inductions
            .iter()
            .enumerate()
            .find(|(_, induction)| induction.latch == semantic_index)
        {
            let latch = ranked_block_id(current)?;
            let target_live = &live_inductions[induction.header];
            let arguments = forward_live_inductions(latch, live, target_live)?;
            let add_argument = target_live
                .iter()
                .position(|candidate| *candidate == induction_index)
                .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                    "a uniform induction latch does not carry its induction value",
                ))?;
            push_block_at_with_index_arguments(
                &mut blocks,
                current,
                u32::try_from(live.len()).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "live induction argument count does not fit u32",
                    )
                })?,
                operations,
                ProductionRankedTerminatorV1::BranchArgsAddAt {
                    arguments,
                    add_argument: u32::try_from(add_argument).map_err(|_| {
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "live induction argument count does not fit u32",
                        )
                    })?,
                    step: induction.step,
                    target: ranked_block_id(projected_target(&base_blocks, induction.header)?)?,
                },
            )?;
            continue;
        }
        if !live.is_empty() {
            let block = ranked_block_id(current)?;
            let arguments_for =
                |target: usize| forward_live_inductions(block, live, &live_inductions[target]);
            let terminator = match terminator {
                ProjectedCfgTerminatorV1::Branch(target) => {
                    ProductionRankedTerminatorV1::BranchArgs {
                        arguments: arguments_for(target)?,
                        target: ranked_block_id(projected_target(&base_blocks, target)?)?,
                    }
                }
                ProjectedCfgTerminatorV1::Predicate {
                    predicate,
                    true_block,
                    false_block: _,
                } if predicate.comparisons.is_empty() => ProductionRankedTerminatorV1::BranchArgs {
                    arguments: arguments_for(true_block)?,
                    target: ranked_block_id(projected_target(&base_blocks, true_block)?)?,
                },
                ProjectedCfgTerminatorV1::Predicate {
                    predicate,
                    true_block,
                    false_block,
                } if predicate.comparisons.len() == 1 => {
                    let (lhs, rhs) = predicate.comparisons[0];
                    ProductionRankedTerminatorV1::IndexLessThanArgs {
                        lhs,
                        rhs,
                        true_arguments: arguments_for(true_block)?,
                        false_arguments: arguments_for(false_block)?,
                        true_block: ranked_block_id(projected_target(&base_blocks, true_block)?)?,
                        false_block: ranked_block_id(projected_target(&base_blocks, false_block)?)?,
                    }
                }
                ProjectedCfgTerminatorV1::Predicate { .. } => {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a uniform induction predicate requires unrepresentable control expansion",
                    ));
                }
                ProjectedCfgTerminatorV1::AnalysisSplit {
                    first_block,
                    second_block,
                } => ProductionRankedTerminatorV1::AnalysisSplitArgs {
                    control_dependencies: Vec::new(),
                    first_arguments: arguments_for(first_block)?,
                    second_arguments: arguments_for(second_block)?,
                    first_block: ranked_block_id(projected_target(&base_blocks, first_block)?)?,
                    second_block: ranked_block_id(projected_target(&base_blocks, second_block)?)?,
                },
                ProjectedCfgTerminatorV1::ExactSwitch(switch) if switch.targets.is_empty() => {
                    ProductionRankedTerminatorV1::BranchArgs {
                        arguments: arguments_for(switch.otherwise)?,
                        target: ranked_block_id(projected_target(&base_blocks, switch.otherwise)?)?,
                    }
                }
                ProjectedCfgTerminatorV1::ExactSwitch(switch) if switch.targets.len() == 1 => {
                    let (variant, target) = switch.targets[0];
                    ProductionRankedTerminatorV1::IndexEqualArgs {
                        lhs: switch.discriminant,
                        rhs: variant,
                        true_arguments: arguments_for(target)?,
                        false_arguments: arguments_for(switch.otherwise)?,
                        true_block: ranked_block_id(projected_target(&base_blocks, target)?)?,
                        false_block: ranked_block_id(projected_target(
                            &base_blocks,
                            switch.otherwise,
                        )?)?,
                    }
                }
                ProjectedCfgTerminatorV1::ExactSwitch(_) => {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a uniform induction deterministic switch requires unrepresentable control expansion",
                    ));
                }
                ProjectedCfgTerminatorV1::Return => {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a uniform induction body returns before its unique latch",
                    ));
                }
            };
            push_block_at_with_index_arguments(
                &mut blocks,
                current,
                u32::try_from(live.len()).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "live induction argument count does not fit u32",
                    )
                })?,
                operations,
                terminator,
            )?;
            continue;
        }
        match terminator {
            ProjectedCfgTerminatorV1::Branch(target) => push_block_at(
                &mut blocks,
                current,
                operations,
                ProductionRankedTerminatorV1::Branch {
                    target: ranked_block_id(projected_target(&base_blocks, target)?)?,
                },
            )?,
            ProjectedCfgTerminatorV1::Predicate {
                predicate,
                true_block,
                false_block: _,
            } if predicate.comparisons.is_empty() => push_block_at(
                &mut blocks,
                current,
                operations,
                ProductionRankedTerminatorV1::Branch {
                    target: ranked_block_id(projected_target(&base_blocks, true_block)?)?,
                },
            )?,
            ProjectedCfgTerminatorV1::Predicate {
                predicate,
                true_block,
                false_block,
            } => append_predicate_blocks(
                &mut blocks,
                current,
                operations,
                &predicate,
                projected_target(&base_blocks, true_block)?,
                projected_target(&base_blocks, false_block)?,
            )?,
            ProjectedCfgTerminatorV1::AnalysisSplit {
                first_block,
                second_block,
            } => push_block_at(
                &mut blocks,
                current,
                operations,
                ProductionRankedTerminatorV1::AnalysisSplit {
                    control_dependencies: Vec::new(),
                    first_block: ranked_block_id(projected_target(&base_blocks, first_block)?)?,
                    second_block: ranked_block_id(projected_target(&base_blocks, second_block)?)?,
                },
            )?,
            ProjectedCfgTerminatorV1::ExactSwitch(switch) => {
                append_exact_switch_blocks(
                    &mut blocks,
                    current,
                    operations,
                    &switch,
                    &base_blocks,
                )?;
            }
            ProjectedCfgTerminatorV1::Return => push_block_at(
                &mut blocks,
                current,
                operations,
                ProductionRankedTerminatorV1::Return,
            )?,
        }
    }
    if blocks.len() != block_count {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG projection produced a non-canonical block inventory",
        ));
    }
    Ok((blocks, sources))
}

fn reachable_projected_blocks(
    entry: usize,
    terminators: &[ProjectedCfgTerminatorV1],
) -> Result<Vec<bool>, ProductionRankedProjectionErrorV1> {
    if entry >= terminators.len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic entry block outside the function",
        ));
    }
    let mut reachable = vec![false; terminators.len()];
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        let slot =
            reachable
                .get_mut(block)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "a semantic CFG edge outside the function",
                ))?;
        if *slot {
            continue;
        }
        *slot = true;
        match &terminators[block] {
            ProjectedCfgTerminatorV1::Branch(target) => pending.push(*target),
            ProjectedCfgTerminatorV1::Predicate {
                predicate,
                true_block,
                false_block,
            } => {
                pending.push(*true_block);
                if !predicate.comparisons.is_empty() {
                    pending.push(*false_block);
                }
            }
            ProjectedCfgTerminatorV1::AnalysisSplit {
                first_block,
                second_block,
            } => {
                pending.push(*first_block);
                pending.push(*second_block);
            }
            ProjectedCfgTerminatorV1::ExactSwitch(switch) => {
                pending.extend(switch.targets.iter().map(|(_, target)| *target));
                pending.push(switch.otherwise);
            }
            ProjectedCfgTerminatorV1::Return => {}
        }
    }
    Ok(reachable)
}

fn projected_target(
    base_blocks: &[Option<usize>],
    semantic_block: usize,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    base_blocks.get(semantic_block).copied().flatten().ok_or(
        ProductionRankedProjectionErrorV1::Unsupported(
            "a projected CFG edge targets a pruned semantic block",
        ),
    )
}

fn append_predicate_blocks(
    blocks: &mut Vec<ProductionRankedBlockV1>,
    first_block: usize,
    first_operations: Vec<ProductionRankedOperationV1>,
    predicate: &GuardPredicateV1,
    true_block: usize,
    false_block: usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if predicate.comparisons.is_empty() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an empty predicate was materialized as conditional control flow",
        ));
    }
    for (index, &(lhs, rhs)) in predicate.comparisons.iter().enumerate() {
        let block = first_block + index;
        let operations = if index == 0 {
            first_operations.clone()
        } else {
            Vec::new()
        };
        let next = if index + 1 == predicate.comparisons.len() {
            true_block
        } else {
            block + 1
        };
        push_block_at(
            blocks,
            block,
            operations,
            ProductionRankedTerminatorV1::IndexLessThan {
                lhs,
                rhs,
                true_block: ranked_block_id(next)?,
                false_block: ranked_block_id(false_block)?,
            },
        )?;
    }
    Ok(())
}

fn append_predicate_blocks_with_index_arguments(
    blocks: &mut Vec<ProductionRankedBlockV1>,
    first_block: usize,
    first_operations: Vec<ProductionRankedOperationV1>,
    predicate: &GuardPredicateV1,
    true_block: usize,
    false_block: usize,
    argument_count: usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if predicate.comparisons.is_empty() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an empty predicate was materialized as conditional control flow",
        ));
    }
    for (index, &(lhs, rhs)) in predicate.comparisons.iter().enumerate() {
        let block = first_block + index;
        let operations = if index == 0 {
            first_operations.clone()
        } else {
            Vec::new()
        };
        let next = if index + 1 == predicate.comparisons.len() {
            true_block
        } else {
            block + 1
        };
        let block_id = ranked_block_id(block)?;
        let live = (0..argument_count).collect::<Vec<_>>();
        let arguments = live_induction_block_arguments(block_id, &live)?;
        push_block_at_with_index_arguments(
            blocks,
            block,
            u32::try_from(argument_count).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "live induction argument count does not fit u32",
                )
            })?,
            operations,
            ProductionRankedTerminatorV1::IndexLessThanArgs {
                lhs,
                rhs,
                true_arguments: arguments.clone(),
                false_arguments: Vec::new(),
                true_block: ranked_block_id(next)?,
                false_block: ranked_block_id(false_block)?,
            },
        )?;
    }
    Ok(())
}

fn append_exact_switch_blocks(
    blocks: &mut Vec<ProductionRankedBlockV1>,
    first_block: usize,
    first_operations: Vec<ProductionRankedOperationV1>,
    switch: &ProjectedDeterministicSwitchV1,
    base_blocks: &[Option<usize>],
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if switch.targets.is_empty() {
        return push_block_at(
            blocks,
            first_block,
            first_operations,
            ProductionRankedTerminatorV1::Branch {
                target: ranked_block_id(projected_target(base_blocks, switch.otherwise)?)?,
            },
        );
    }
    for (index, &(variant, target)) in switch.targets.iter().enumerate() {
        let block = first_block + index;
        let operations = if index == 0 {
            first_operations.clone()
        } else {
            Vec::new()
        };
        let false_block = if index + 1 == switch.targets.len() {
            projected_target(base_blocks, switch.otherwise)?
        } else {
            block + 1
        };
        push_block_at(
            blocks,
            block,
            operations,
            ProductionRankedTerminatorV1::IndexEqual {
                lhs: switch.discriminant,
                rhs: variant,
                true_block: ranked_block_id(projected_target(base_blocks, target)?)?,
                false_block: ranked_block_id(false_block)?,
            },
        )?;
    }
    Ok(())
}

fn push_block_at(
    blocks: &mut Vec<ProductionRankedBlockV1>,
    expected: usize,
    operations: Vec<ProductionRankedOperationV1>,
    terminator: ProductionRankedTerminatorV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if blocks.len() != expected {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG projection produced non-canonical block numbering",
        ));
    }
    blocks.push(ProductionRankedBlockV1::new(operations, terminator));
    Ok(())
}

fn push_block_at_with_index_arguments(
    blocks: &mut Vec<ProductionRankedBlockV1>,
    expected: usize,
    index_argument_count: u32,
    operations: Vec<ProductionRankedOperationV1>,
    terminator: ProductionRankedTerminatorV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if blocks.len() != expected {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG projection produced non-canonical block numbering",
        ));
    }
    blocks.push(ProductionRankedBlockV1::with_index_arguments(
        index_argument_count,
        operations,
        terminator,
    ));
    Ok(())
}

fn ranked_block_id(block: usize) -> Result<u32, ProductionRankedProjectionErrorV1> {
    u32::try_from(block).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "semantic CFG block identity does not fit u32",
        )
    })
}

fn format_ranked_cfg(
    function_name: &str,
    blocks: &[ProductionRankedBlockV1],
) -> Result<String, ProductionRankedProjectionErrorV1> {
    let mut output = String::new();
    push_ranked_ir(&mut output, &format!("func @{function_name} {{\n"))?;
    for (block_index, block) in blocks.iter().enumerate() {
        let arguments = (0..block.index_argument_count())
            .map(|argument| format!("%bb{block_index}_arg{argument}: index"))
            .collect::<Vec<_>>()
            .join(", ");
        push_ranked_ir(
            &mut output,
            &format!(
                "^bb{block_index}{}:\n",
                (!arguments.is_empty())
                    .then(|| format!("({arguments})"))
                    .unwrap_or_default()
            ),
        )?;
        for operation in block.operations() {
            push_ranked_ir(&mut output, &format_ranked_operation(operation))?;
        }
        let terminator = match block.terminator() {
            ProductionRankedTerminatorV1::IndexLessThan {
                lhs,
                rhs,
                true_block,
                false_block,
            } => format!(
                "  kernel.cond_br {} < {} ^bb{}, ^bb{}\n",
                ranked_value_text_v1(*lhs),
                ranked_value_text_v1(*rhs),
                true_block,
                false_block,
            ),
            ProductionRankedTerminatorV1::IndexLessThanArgs {
                lhs,
                rhs,
                true_arguments,
                false_arguments,
                true_block,
                false_block,
            } => format!(
                "  kernel.index_lt_br_args {}, {} ({}) ^bb{}, ({}) ^bb{}\n",
                ranked_value_text_v1(*lhs),
                ranked_value_text_v1(*rhs),
                format_ranked_values(true_arguments),
                true_block,
                format_ranked_values(false_arguments),
                false_block,
            ),
            ProductionRankedTerminatorV1::IndexEqual {
                lhs,
                rhs,
                true_block,
                false_block,
            } => format!(
                "  kernel.index_eq_br {}, {} ^bb{}, ^bb{}\n",
                ranked_value_text_v1(*lhs),
                ranked_value_text_v1(*rhs),
                true_block,
                false_block,
            ),
            ProductionRankedTerminatorV1::IndexEqualArgs {
                lhs,
                rhs,
                true_arguments,
                false_arguments,
                true_block,
                false_block,
            } => format!(
                "  kernel.index_eq_br_args {}, {} ({}) ^bb{}, ({}) ^bb{}\n",
                ranked_value_text_v1(*lhs),
                ranked_value_text_v1(*rhs),
                format_ranked_values(true_arguments),
                true_block,
                format_ranked_values(false_arguments),
                false_block,
            ),
            ProductionRankedTerminatorV1::AnalysisSplit {
                control_dependencies,
                first_block,
                second_block,
            } => format!(
                "  kernel.analysis_split controls=({}) ^bb{}, ^bb{}\n",
                format_ranked_values(control_dependencies),
                first_block,
                second_block,
            ),
            ProductionRankedTerminatorV1::AnalysisSplitArgs {
                control_dependencies,
                first_arguments,
                second_arguments,
                first_block,
                second_block,
            } => format!(
                "  kernel.analysis_split controls=({}) ({}) ^bb{}, ({}) ^bb{}\n",
                format_ranked_values(control_dependencies),
                format_ranked_values(first_arguments),
                first_block,
                format_ranked_values(second_arguments),
                second_block,
            ),
            ProductionRankedTerminatorV1::Branch { target } => {
                format!("  kernel.br ^bb{target}\n")
            }
            ProductionRankedTerminatorV1::BranchArgs { arguments, target } => format!(
                "  kernel.br_args ({}) ^bb{}\n",
                format_ranked_values(arguments),
                target,
            ),
            ProductionRankedTerminatorV1::BranchArgsAdd {
                value,
                step,
                target,
            } => format!(
                "  kernel.br_args_add {}, {} ^bb{}\n",
                ranked_value_text_v1(*value),
                ranked_value_text_v1(*step),
                target,
            ),
            ProductionRankedTerminatorV1::BranchArgsAddAt {
                arguments,
                add_argument,
                step,
                target,
            } => format!(
                "  kernel.br_args_add_at ({}) [{}] += {} ^bb{}\n",
                format_ranked_values(arguments),
                add_argument,
                ranked_value_text_v1(*step),
                target,
            ),
            ProductionRankedTerminatorV1::Return => "  kernel.return\n".to_owned(),
            ProductionRankedTerminatorV1::Trap => "  kernel.trap\n".to_owned(),
        };
        push_ranked_ir(&mut output, &terminator)?;
    }
    push_ranked_ir(&mut output, "}\n")?;
    Ok(output)
}

fn format_ranked_operation(operation: &ProductionRankedOperationV1) -> String {
    match operation {
        ProductionRankedOperationV1::ExecutionLayout {
            grid_identity,
            global_extents,
            workgroup_extents,
            subgroup_size,
            full_physical_workgroups,
        } => format!(
            "  gpu.execution_layout <grid={}, global={:?}, workgroup={:?}, subgroup={}, full_physical_workgroups={}>\n",
            grid_identity,
            global_extents,
            workgroup_extents,
            subgroup_size,
            full_physical_workgroups,
        ),
        ProductionRankedOperationV1::View {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
            allocation_origin,
            noalias_class,
        } => format!(
            "  %{} = kernel.ranked_view <{}, {}, {:?}, origin={}, noalias={}>({})\n",
            result.get(),
            element_width,
            writable,
            shape,
            allocation_origin,
            noalias_class,
            format_ranked_values(dynamic_extents),
        ),
        ProductionRankedOperationV1::ViewInSpace {
            result,
            element_width,
            writable,
            shape,
            dynamic_extents,
            memory_space,
            allocation_origin,
            noalias_class,
        } => format!(
            "  %{} = kernel.ranked_view <{}, {}, {:?}, {:?}, origin={}, noalias={}>({})\n",
            result.get(),
            element_width,
            writable,
            shape,
            memory_space,
            allocation_origin,
            noalias_class,
            format_ranked_values(dynamic_extents),
        ),
        ProductionRankedOperationV1::IndexConstant { result, value } => {
            format!("  %{} = kernel.index_constant {}\n", result.get(), value)
        }
        ProductionRankedOperationV1::IndexUnknown { result } => {
            format!("  %{} = kernel.index_unknown\n", result.get())
        }
        ProductionRankedOperationV1::InvocationIndex {
            result,
            dimension,
            launch_extent,
        } => format!(
            "  %{} = kernel.invocation_index <{}, {}>\n",
            result.get(),
            dimension,
            if *launch_extent == 0 {
                "dynamic".to_owned()
            } else {
                launch_extent.to_string()
            },
        ),
        ProductionRankedOperationV1::IndexBinary {
            result,
            kind,
            lhs,
            rhs,
        } => format!(
            "  %{} = kernel.index_binary {:?} {}, {}\n",
            result.get(),
            kind,
            ranked_value_text_v1(*lhs),
            ranked_value_text_v1(*rhs),
        ),
        ProductionRankedOperationV1::DeterministicJoin {
            result,
            dependencies,
        } => format!(
            "  %{} = kernel.deterministic_join ({})\n",
            result.get(),
            format_ranked_values(dependencies),
        ),
        ProductionRankedOperationV1::CheckedTiledIndex2D {
            result,
            invocation,
            component,
            rows,
            columns,
            row_stride,
            lanes_per_tile,
            tile_rows,
            tile_columns,
            elements_per_lane,
        } => format!(
            "  %{} = kernel.checked_tiled_index_2d <{}, {}, {}, {}>({}, {}, {}, {}, {})\n",
            result.get(),
            lanes_per_tile,
            tile_rows,
            tile_columns,
            elements_per_lane,
            ranked_value_text_v1(*invocation),
            ranked_value_text_v1(*component),
            ranked_value_text_v1(*rows),
            ranked_value_text_v1(*columns),
            ranked_value_text_v1(*row_stride),
        ),
        ProductionRankedOperationV1::Dimension {
            result,
            view,
            dimension,
        } => format!(
            "  %{} = kernel.dimension {} {}\n",
            result.get(),
            ranked_value_text_v1(*view),
            dimension,
        ),
        ProductionRankedOperationV1::Access {
            kind,
            view,
            indices,
        } => format!(
            "  kernel.access {:?} {}[{}]\n",
            kind,
            ranked_value_text_v1(*view),
            format_ranked_values(indices),
        ),
        ProductionRankedOperationV1::AtomicAccess {
            kind,
            ordering,
            scope,
            view,
            indices,
        } => format!(
            "  kernel.atomic_access {:?} <{:?}, {:?}> {}[{}]\n",
            kind,
            ordering,
            scope,
            ranked_value_text_v1(*view),
            format_ranked_values(indices),
        ),
        ProductionRankedOperationV1::AllocationEffect {
            kind,
            memory_space,
            allocation_origin,
            noalias_class,
        } => format!(
            "  kernel.allocation_effect {:?} <{:?}, origin={}, noalias={}>\n",
            kind, memory_space, allocation_origin, noalias_class,
        ),
        ProductionRankedOperationV1::Barrier {
            execution_scope,
            memory_scope,
            address_space,
            order,
        } => format!(
            "  gpu.barrier <{:?}, {:?}, {:?}, {:?}>\n",
            execution_scope, memory_scope, address_space, order,
        ),
        ProductionRankedOperationV1::Fence {
            memory_scope,
            address_space,
            order,
        } => format!(
            "  gpu.fence <{:?}, {:?}, {:?}>\n",
            memory_scope, address_space, order,
        ),
        ProductionRankedOperationV1::TensorLayout {
            contract,
            convergence,
            active_lanes,
        } => format!(
            "  kernel.tensor_layout <{:?}, {:?}, active_lanes={}>\n",
            contract, convergence, active_lanes,
        ),
        ProductionRankedOperationV1::SemanticSymbol { result, symbol } => {
            format!("  %{} = kernel.semantic_symbol {}\n", result.get(), symbol)
        }
        ProductionRankedOperationV1::SemanticConstant { result, value } => {
            format!("  %{} = kernel.semantic_constant {}\n", result.get(), value)
        }
        ProductionRankedOperationV1::SemanticBinary {
            result,
            kind,
            lhs,
            rhs,
        } => format!(
            "  %{} = kernel.semantic_binary {:?} {}, {}\n",
            result.get(),
            kind,
            ranked_value_text_v1(*lhs),
            ranked_value_text_v1(*rhs),
        ),
        ProductionRankedOperationV1::RequireEquivalent { actual, expected } => format!(
            "  kernel.require_equivalent {}, {}\n",
            ranked_value_text_v1(*actual),
            ranked_value_text_v1(*expected),
        ),
    }
}

fn format_ranked_values(values: &[ProductionRankedValueV1]) -> String {
    values
        .iter()
        .map(|value| ranked_value_text_v1(*value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn checked_reference_origins(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    guarded_access_count: usize,
) -> Result<Vec<Option<usize>>, ProductionRankedProjectionErrorV1> {
    let definitions = local_definition_counts(function);
    let mut origins = vec![None; function.locals().len()];
    let mut aliases_by_source = vec![Vec::new(); function.locals().len()];
    let mut edge_count = 0_usize;
    for block in function.blocks() {
        for statement in block.statements() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let destination = assignment.destination();
            if !destination.projections().is_empty()
                || definitions
                    .get(destination.local().index() as usize)
                    .copied()
                    != Some(1)
            {
                continue;
            }
            let source = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => transparent_operand_place(operand),
                SemanticRvalueKindV1::Load(load) if load.atomic().is_none() => {
                    transparent_place(load.source())
                }
                _ => None,
            };
            let Some(source) = source else {
                continue;
            };
            push_local_provenance_edge_v1(
                &mut aliases_by_source,
                source.local().index() as usize,
                destination.local().index() as usize,
                &mut edge_count,
            )?;
        }
    }

    let mut access = 0_usize;
    for block in function.blocks() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        if !matches!(
            callables.get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut { .. }
                    | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut { .. },
                ..
            })
        ) {
            continue;
        }
        let destination = simple_call_destination(call)?;
        if definitions.get(destination.index() as usize).copied() != Some(1) {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a checked disjoint result without one exact definition",
            ));
        }
        origins[destination.index() as usize] = Some(access);
        access += 1;
    }
    if access != guarded_access_count {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "checked disjoint access inventory changed during projection",
        ));
    }
    propagate_exact_local_origins_v1(
        &mut origins,
        &aliases_by_source,
        "a checked disjoint reference with conflicting origins",
    )?;
    Ok(origins)
}

fn local_definition_counts(function: &SemanticFunctionDeclV1) -> Vec<u8> {
    let mut definitions = vec![0_u8; function.locals().len()];
    let mut record = |place: &SemanticPlaceV1| {
        if matches!(
            place
                .projections()
                .first()
                .map(|projection| projection.kind()),
            Some(SemanticProjectionKindV1::Dereference)
        ) {
            return;
        }
        if let Some(slot) = definitions.get_mut(place.local().index() as usize) {
            *slot = slot.saturating_add(1);
        }
    };
    for block in function.blocks() {
        for statement in block.statements() {
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => record(assignment.destination()),
                SemanticStatementKindV1::Store(store) => record(store.destination()),
                SemanticStatementKindV1::AtomicRmw(atomic) => record(atomic.destination()),
                SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                    record(atomic.destination())
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => record(place),
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Assume(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
        {
            record(destination.place());
        }
    }
    definitions
}

fn checked_reference_origin(place: &SemanticPlaceV1, origins: &[Option<usize>]) -> Option<usize> {
    let origin = origins
        .get(place.local().index() as usize)
        .copied()
        .flatten()?;
    let mut projections = place.projections().iter();
    if !matches!(
        projections.next().map(|projection| projection.kind()),
        Some(SemanticProjectionKindV1::Dereference)
    ) || !projections.all(|projection| {
        matches!(
            projection.kind(),
            SemanticProjectionKindV1::Field(_)
                | SemanticProjectionKindV1::Downcast(_)
                | SemanticProjectionKindV1::OpaqueCast
                | SemanticProjectionKindV1::Subtype
        )
    }) {
        return None;
    }
    Some(origin)
}

fn transparent_operand_place(operand: &SemanticOperandV1) -> Option<&SemanticPlaceV1> {
    transparent_place(raw_operand_place(operand)?)
}

fn raw_operand_place(operand: &SemanticOperandV1) -> Option<&SemanticPlaceV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => Some(place),
        SemanticOperandV1::Constant(_) => None,
    }
}

fn enum_payload_projection(place: &SemanticPlaceV1) -> Option<(usize, u32)> {
    let [downcast, field] = place.projections() else {
        return None;
    };
    let SemanticProjectionKindV1::Downcast(variant) = downcast.kind() else {
        return None;
    };
    matches!(field.kind(), SemanticProjectionKindV1::Field(0))
        .then_some((place.local().index() as usize, variant))
}

fn transparent_place(place: &SemanticPlaceV1) -> Option<&SemanticPlaceV1> {
    place
        .projections()
        .iter()
        .all(|projection| {
            matches!(
                projection.kind(),
                SemanticProjectionKindV1::Field(_)
                    | SemanticProjectionKindV1::Downcast(_)
                    | SemanticProjectionKindV1::OpaqueCast
                    | SemanticProjectionKindV1::Subtype
            )
        })
        .then_some(place)
}

fn simple_operand_local(operand: &SemanticOperandV1) -> Option<SemanticLocalIdV1> {
    transparent_operand_place(operand)
        .filter(|place| place.projections().is_empty())
        .map(SemanticPlaceV1::local)
}

fn simple_call_destination(
    call: &SemanticDirectCallV1,
) -> Result<SemanticLocalIdV1, ProductionRankedProjectionErrorV1> {
    call.destination()
        .map(|destination| destination.place())
        .filter(|place| place.projections().is_empty())
        .map(SemanticPlaceV1::local)
        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
            "a compiler intrinsic without one exact local destination",
        ))
}

fn reserve_operation(
    operations: &mut Vec<ProductionRankedOperationV1>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if operations.len() == MAX_PROJECTED_OPERATIONS_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic intrinsic projection exceeding the ranked operation limit",
        ));
    }
    operations.try_reserve(1).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "semantic intrinsic projection storage cannot be reserved",
        )
    })
}

fn projected_block_uses_bounds_check(
    block: &ProjectedSemanticBlockV1,
    check: ProjectedBoundsCheckV1,
) -> bool {
    block.items.iter().any(|item| match item {
        ProjectedBlockItemV1::Effect {
            operation:
                ProductionRankedOperationV1::Access { indices, .. }
                | ProductionRankedOperationV1::AtomicAccess { indices, .. },
            ..
        } => indices.contains(&check.index),
        ProjectedBlockItemV1::Guarded(access) => {
            access.indices.contains(&check.index)
                && access.comparisons.contains(&(check.index, check.extent))
        }
        ProjectedBlockItemV1::Effect { .. } => false,
    })
}

fn retain_incomplete(
    result: Result<(), ProductionRankedProjectionErrorV1>,
    incomplete: &mut Option<&'static str>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match result {
        Err(ProductionRankedProjectionErrorV1::Incomplete(detail)) => {
            incomplete.get_or_insert(detail);
            Ok(())
        }
        result => result,
    }
}

#[allow(clippy::too_many_arguments)]
fn project_statement_accesses(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    bounds_checks: &[ProjectedBoundsCheckV1],
    statement: &fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1,
    constants: &[Option<u64>],
    local_contracts: &ProjectionLocalContractsV1,
    guarded_accesses: &[GuardedRankedAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let source = statement.source();
    match statement.kind() {
        SemanticStatementKindV1::Assign(assignment) => {
            project_place_access(
                types,
                function,
                block_index,
                bounds_checks,
                assignment.destination(),
                AccessKindAttr::Write,
                PlaceAccessRequirementV1::IfMemory,
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_rvalue_reads(
                types,
                function,
                block_index,
                bounds_checks,
                assignment.value().kind(),
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )
        }
        SemanticStatementKindV1::Store(store) => {
            project_place_access_with_atomic(
                types,
                function,
                block_index,
                bounds_checks,
                store.destination(),
                if store.atomic().is_some() {
                    AccessKindAttr::AtomicWrite
                } else {
                    AccessKindAttr::Write
                },
                store.atomic(),
                PlaceAccessRequirementV1::ExplicitMemory,
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_operand_read(
                types,
                function,
                block_index,
                bounds_checks,
                store.value(),
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )
        }
        SemanticStatementKindV1::AtomicRmw(atomic) => {
            project_place_access(
                types,
                function,
                block_index,
                bounds_checks,
                atomic.destination(),
                AccessKindAttr::Write,
                PlaceAccessRequirementV1::IfMemory,
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_atomic_address(
                types,
                function,
                block_index,
                bounds_checks,
                atomic.address(),
                atomic.access(),
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_operand_read(
                types,
                function,
                block_index,
                bounds_checks,
                atomic.value(),
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )
        }
        SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
            project_place_access(
                types,
                function,
                block_index,
                bounds_checks,
                atomic.destination(),
                AccessKindAttr::Write,
                PlaceAccessRequirementV1::IfMemory,
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_atomic_address(
                types,
                function,
                block_index,
                bounds_checks,
                atomic.address(),
                atomic.success(),
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_operand_read(
                types,
                function,
                block_index,
                bounds_checks,
                atomic.expected(),
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_operand_read(
                types,
                function,
                block_index,
                bounds_checks,
                atomic.replacement(),
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )
        }
        SemanticStatementKindV1::SetDiscriminant { place, .. }
        | SemanticStatementKindV1::Deinitialize(place) => project_place_access(
            types,
            function,
            block_index,
            bounds_checks,
            place,
            AccessKindAttr::Write,
            PlaceAccessRequirementV1::IfMemory,
            source,
            constants,
            local_contracts,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticStatementKindV1::StorageLive(local)
        | SemanticStatementKindV1::StorageDead(local) => {
            if function.locals().get(local.index() as usize).is_none() {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "a storage statement with an out-of-range local",
                ));
            }
            // Storage lifetime markers do not read or write the local's value.
            Ok(())
        }
        SemanticStatementKindV1::Assume(condition) => project_operand_read(
            types,
            function,
            block_index,
            bounds_checks,
            condition,
            source,
            constants,
            local_contracts,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticStatementKindV1::Nop => Ok(()),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_atomic_address(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    bounds_checks: &[ProjectedBoundsCheckV1],
    address: &SemanticPlaceV1,
    atomic: SemanticAtomicAccessV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    local_contracts: &ProjectionLocalContractsV1,
    guarded_accesses: &[GuardedRankedAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    project_place_access_with_atomic(
        types,
        function,
        block_index,
        bounds_checks,
        address,
        AccessKindAttr::AtomicReadModifyWrite,
        Some(atomic),
        PlaceAccessRequirementV1::ExplicitMemory,
        source,
        constants,
        local_contracts,
        guarded_accesses,
        guarded_sites,
        projected_views,
        operations,
        sources,
        next_value,
        ranked_ir,
    )
}

#[allow(clippy::too_many_arguments)]
fn project_terminator_accesses(
    callables: &[SemanticCallableDeclV1],
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    bounds_checks: &[ProjectedBoundsCheckV1],
    terminator: &SemanticTerminatorKindV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    local_contracts: &ProjectionLocalContractsV1,
    guarded_accesses: &[GuardedRankedAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match terminator {
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => project_operand_read(
            types,
            function,
            block_index,
            bounds_checks,
            discriminant,
            source,
            constants,
            local_contracts,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticTerminatorKindV1::Call(call) => project_direct_call_accesses(
            callables,
            types,
            function,
            block_index,
            bounds_checks,
            call,
            source,
            constants,
            local_contracts,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticTerminatorKindV1::TailCall(call) => project_tail_call_accesses(
            callables,
            types,
            function,
            block_index,
            bounds_checks,
            call,
            source,
            constants,
            local_contracts,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticTerminatorKindV1::Drop { .. } => {
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a drop terminator before exact drop-glue memory-effect summaries are available",
            ))
        }
        SemanticTerminatorKindV1::Assert { condition, .. } => project_operand_read(
            types,
            function,
            block_index,
            bounds_checks,
            condition,
            source,
            constants,
            local_contracts,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => Ok(()),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_direct_call_accesses(
    callables: &[SemanticCallableDeclV1],
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    bounds_checks: &[ProjectedBoundsCheckV1],
    call: &SemanticDirectCallV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    local_contracts: &ProjectionLocalContractsV1,
    guarded_accesses: &[GuardedRankedAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if matches!(
        callables.get(call.callee().index() as usize),
        Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier,
            ..
        })
    ) {
        reserve_operation(operations)?;
        operations.push(ProductionRankedOperationV1::Barrier {
            execution_scope: HierarchyAttr::Workgroup,
            memory_scope: MemoryScopeAttr::Workgroup,
            address_space: AddressSpaceAttr::Workgroup,
            order: MemoryOrderAttr::AcquireRelease,
        });
        push_ranked_ir(
            ranked_ir,
            "  gpu.barrier <workgroup, workgroup, workgroup, acquire_release>\n",
        )?;
        return Ok(());
    }
    if matches!(
        callables.get(call.callee().index() as usize),
        Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. }
                | SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint { .. }
                | SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift { .. }
                | SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointIndexGet { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut { .. }
                | SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive { .. }
                | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut { .. },
            ..
        })
    ) {
        return Ok(());
    }
    require_bounds_neutral_callable(callables, call.callee())?;
    for argument in call.arguments() {
        project_operand_read(
            types,
            function,
            block_index,
            bounds_checks,
            argument,
            source,
            constants,
            local_contracts,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        )?;
    }
    if let Some(destination) = call.destination() {
        project_place_access(
            types,
            function,
            block_index,
            bounds_checks,
            destination.place(),
            AccessKindAttr::Write,
            PlaceAccessRequirementV1::IfMemory,
            source,
            constants,
            local_contracts,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn project_tail_call_accesses(
    callables: &[SemanticCallableDeclV1],
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    bounds_checks: &[ProjectedBoundsCheckV1],
    call: &SemanticDirectTailCallV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    local_contracts: &ProjectionLocalContractsV1,
    guarded_accesses: &[GuardedRankedAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    require_bounds_neutral_callable(callables, call.callee())?;
    for argument in call.arguments() {
        project_operand_read(
            types,
            function,
            block_index,
            bounds_checks,
            argument,
            source,
            constants,
            local_contracts,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        )?;
    }
    Ok(())
}

fn require_bounds_neutral_callable(
    callables: &[SemanticCallableDeclV1],
    callable: SemanticCallableIdV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match callables.get(callable.index() as usize) {
        Some(SemanticCallableDeclV1::CompilerIntrinsic { .. }) => Ok(()),
        Some(
            SemanticCallableDeclV1::Defined { .. } | SemanticCallableDeclV1::DeviceFfiImport { .. },
        )
        | None => Err(ProductionRankedProjectionErrorV1::Incomplete(
            "a call terminator before exact callable memory-effect summaries are available",
        )),
    }
}

#[derive(Clone, Copy)]
enum ConstantDefinitionV1 {
    Missing,
    Direct(u64),
    Alias(SemanticLocalIdV1),
    Invalid,
}

fn constant_locals(function: &SemanticFunctionDeclV1) -> Vec<Option<u64>> {
    let mut definitions = vec![ConstantDefinitionV1::Missing; function.locals().len()];
    for block in function.blocks() {
        for statement in block.statements() {
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment)
                    if assignment.destination().projections().is_empty() =>
                {
                    record_constant_definition(
                        &mut definitions,
                        assignment.destination().local(),
                        match assignment.value().kind() {
                            SemanticRvalueKindV1::Use(operand) => constant_definition(operand),
                            _ => ConstantDefinitionV1::Invalid,
                        },
                    );
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place)
                    if place.projections().is_empty() =>
                {
                    record_constant_definition(
                        &mut definitions,
                        place.local(),
                        ConstantDefinitionV1::Invalid,
                    );
                }
                SemanticStatementKindV1::Store(store)
                    if store.destination().projections().is_empty() =>
                {
                    record_constant_definition(
                        &mut definitions,
                        store.destination().local(),
                        ConstantDefinitionV1::Invalid,
                    );
                }
                SemanticStatementKindV1::AtomicRmw(atomic)
                    if atomic.destination().projections().is_empty() =>
                {
                    record_constant_definition(
                        &mut definitions,
                        atomic.destination().local(),
                        ConstantDefinitionV1::Invalid,
                    );
                }
                SemanticStatementKindV1::AtomicCompareExchange(atomic)
                    if atomic.destination().projections().is_empty() =>
                {
                    record_constant_definition(
                        &mut definitions,
                        atomic.destination().local(),
                        ConstantDefinitionV1::Invalid,
                    );
                }
                SemanticStatementKindV1::Assign(_)
                | SemanticStatementKindV1::Store(_)
                | SemanticStatementKindV1::AtomicRmw(_)
                | SemanticStatementKindV1::AtomicCompareExchange(_)
                | SemanticStatementKindV1::SetDiscriminant { .. }
                | SemanticStatementKindV1::Deinitialize(_)
                | SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Assume(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
            && destination.place().projections().is_empty()
        {
            record_constant_definition(
                &mut definitions,
                destination.place().local(),
                ConstantDefinitionV1::Invalid,
            );
        }
    }
    let mut states = vec![0_u8; definitions.len()];
    let mut values = vec![None; definitions.len()];
    for index in 0..definitions.len() {
        resolve_constant(index, &definitions, &mut states, &mut values);
    }
    values
}

fn constant_definition(operand: &SemanticOperandV1) -> ConstantDefinitionV1 {
    match operand {
        SemanticOperandV1::Constant(constant) => match constant.value() {
            SemanticConstantValueV1::Scalar(value) => u64::try_from(value.bits())
                .map(ConstantDefinitionV1::Direct)
                .unwrap_or(ConstantDefinitionV1::Invalid),
            _ => ConstantDefinitionV1::Invalid,
        },
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            ConstantDefinitionV1::Alias(place.local())
        }
        SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_) => ConstantDefinitionV1::Invalid,
    }
}

fn constant_operand_value(operand: &SemanticOperandV1, constants: &[Option<u64>]) -> Option<u64> {
    match constant_definition(operand) {
        ConstantDefinitionV1::Direct(value) => Some(value),
        ConstantDefinitionV1::Alias(local) => {
            constants.get(local.index() as usize).copied().flatten()
        }
        ConstantDefinitionV1::Missing | ConstantDefinitionV1::Invalid => None,
    }
}

fn positive_unsigned_constant_operand_v1(
    operand: &SemanticOperandV1,
    constants: &[Option<u64>],
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
) -> Option<u64> {
    let SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
        signed: false,
        bits,
    }) = types.get(operand.ty().index() as usize)?.shape()
    else {
        return None;
    };
    if *bits == 0 || *bits > 64 {
        return None;
    }
    constant_operand_value(operand, constants).filter(|step| *step > 0)
}

fn record_constant_definition(
    definitions: &mut [ConstantDefinitionV1],
    local: SemanticLocalIdV1,
    definition: ConstantDefinitionV1,
) {
    if let Some(slot) = definitions.get_mut(local.index() as usize) {
        *slot = if matches!(slot, ConstantDefinitionV1::Missing) {
            definition
        } else {
            ConstantDefinitionV1::Invalid
        };
    }
}

fn resolve_constant(
    index: usize,
    definitions: &[ConstantDefinitionV1],
    states: &mut [u8],
    values: &mut [Option<u64>],
) -> Option<u64> {
    match states.get(index).copied() {
        Some(2) => return values[index],
        Some(1) | None => return None,
        Some(_) => {}
    }
    states[index] = 1;
    let value = match definitions[index] {
        ConstantDefinitionV1::Direct(value) => Some(value),
        ConstantDefinitionV1::Alias(local) => {
            resolve_constant(local.index() as usize, definitions, states, values)
        }
        ConstantDefinitionV1::Missing | ConstantDefinitionV1::Invalid => None,
    };
    states[index] = 2;
    values[index] = value;
    value
}

#[allow(clippy::too_many_arguments)]
fn project_rvalue_reads(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    bounds_checks: &[ProjectedBoundsCheckV1],
    value: &SemanticRvalueKindV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    local_contracts: &ProjectionLocalContractsV1,
    guarded_accesses: &[GuardedRankedAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match value {
        SemanticRvalueKindV1::Use(operand) => project_operand_read(
            types,
            function,
            block_index,
            bounds_checks,
            operand,
            source,
            constants,
            local_contracts,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticRvalueKindV1::Unary { operand, .. }
        | SemanticRvalueKindV1::Cast { operand, .. } => project_operand_read(
            types,
            function,
            block_index,
            bounds_checks,
            operand,
            source,
            constants,
            local_contracts,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticRvalueKindV1::Binary { left, right, .. } => {
            project_operand_read(
                types,
                function,
                block_index,
                bounds_checks,
                left,
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_operand_read(
                types,
                function,
                block_index,
                bounds_checks,
                right,
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )
        }
        SemanticRvalueKindV1::CheckedBinary(_) | SemanticRvalueKindV1::UncheckedBinary(_) => {
            let (left, right) = match value {
                SemanticRvalueKindV1::CheckedBinary(checked) => (checked.left(), checked.right()),
                SemanticRvalueKindV1::UncheckedBinary(unchecked) => {
                    (unchecked.left(), unchecked.right())
                }
                _ => unreachable!("outer pattern selects a two-operand arithmetic rvalue"),
            };
            project_operand_read(
                types,
                function,
                block_index,
                bounds_checks,
                left,
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )?;
            project_operand_read(
                types,
                function,
                block_index,
                bounds_checks,
                right,
                source,
                constants,
                local_contracts,
                guarded_accesses,
                guarded_sites,
                projected_views,
                operations,
                sources,
                next_value,
                ranked_ir,
            )
        }
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            for operand in aggregate.operands() {
                project_operand_read(
                    types,
                    function,
                    block_index,
                    bounds_checks,
                    operand,
                    source,
                    constants,
                    local_contracts,
                    guarded_accesses,
                    guarded_sites,
                    projected_views,
                    operations,
                    sources,
                    next_value,
                    ranked_ir,
                )?;
            }
            Ok(())
        }
        SemanticRvalueKindV1::Load(load) => project_place_access_with_atomic(
            types,
            function,
            block_index,
            bounds_checks,
            load.source(),
            if load.atomic().is_some() {
                AccessKindAttr::AtomicRead
            } else {
                AccessKindAttr::Read
            },
            load.atomic(),
            PlaceAccessRequirementV1::ExplicitMemory,
            source,
            constants,
            local_contracts,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. }
        | SemanticRvalueKindV1::Length(place)
        | SemanticRvalueKindV1::Discriminant(place) => project_place_access(
            types,
            function,
            block_index,
            bounds_checks,
            place,
            AccessKindAttr::Read,
            PlaceAccessRequirementV1::IfMemory,
            source,
            constants,
            local_contracts,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_operand_read(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    bounds_checks: &[ProjectedBoundsCheckV1],
    operand: &SemanticOperandV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    local_contracts: &ProjectionLocalContractsV1,
    guarded_accesses: &[GuardedRankedAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => project_place_access(
            types,
            function,
            block_index,
            bounds_checks,
            place,
            AccessKindAttr::Read,
            PlaceAccessRequirementV1::IfMemory,
            source,
            constants,
            local_contracts,
            guarded_accesses,
            guarded_sites,
            projected_views,
            operations,
            sources,
            next_value,
            ranked_ir,
        ),
        SemanticOperandV1::Constant(_) => Ok(()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlaceAccessRequirementV1 {
    IfMemory,
    ExplicitMemory,
}

#[derive(Clone, Copy)]
enum ProjectedIndexV1 {
    Constant(u64),
    Dynamic(ProductionRankedValueV1),
}

fn projected_bounds_check(
    checks: &[ProjectedBoundsCheckV1],
    block_index: usize,
    slice_local: SemanticLocalIdV1,
    index_local: SemanticLocalIdV1,
) -> Result<ProjectedBoundsCheckV1, ProductionRankedProjectionErrorV1> {
    let mut matches = checks.iter().copied().filter(|check| {
        check.access_block == block_index
            && check.slice_local == slice_local
            && check.index_local == index_local
    });
    let check = matches
        .next()
        .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
            "a dynamic slice access without its exact Rust bounds-check predecessor",
        ))?;
    if matches.next().is_some() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "multiple Rust bounds checks authorize one dynamic slice access",
        ));
    }
    Ok(check)
}

#[allow(clippy::too_many_arguments)]
fn project_place_access(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    bounds_checks: &[ProjectedBoundsCheckV1],
    place: &SemanticPlaceV1,
    access: AccessKindAttr,
    requirement: PlaceAccessRequirementV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    local_contracts: &ProjectionLocalContractsV1,
    guarded_accesses: &[GuardedRankedAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    project_place_access_with_atomic(
        types,
        function,
        block_index,
        bounds_checks,
        place,
        access,
        None,
        requirement,
        source,
        constants,
        local_contracts,
        guarded_accesses,
        guarded_sites,
        projected_views,
        operations,
        sources,
        next_value,
        ranked_ir,
    )
}

#[allow(clippy::too_many_arguments)]
fn project_place_access_with_atomic(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block_index: usize,
    bounds_checks: &[ProjectedBoundsCheckV1],
    place: &SemanticPlaceV1,
    access: AccessKindAttr,
    atomic: Option<SemanticAtomicAccessV1>,
    requirement: PlaceAccessRequirementV1,
    source: SemanticSourceProvenanceV1,
    constants: &[Option<u64>],
    local_contracts: &ProjectionLocalContractsV1,
    guarded_accesses: &[GuardedRankedAccessV1],
    guarded_sites: &mut Vec<GuardedAccessSiteV1>,
    projected_views: &mut [Option<ProjectedViewV1>],
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if access.is_atomic() != atomic.is_some() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an atomic access whose ordering/scope contract is missing or attached to a non-atomic access",
        ));
    }
    if let Some(origin) =
        checked_reference_origin(place, &local_contracts.checked_reference_origins)
    {
        if atomic.is_some() {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "an atomic access through a checked disjoint reference before exact atomic capability projection",
            ));
        }
        let mut guarded = guarded_accesses.get(origin).cloned().ok_or(
            ProductionRankedProjectionErrorV1::Unsupported(
                "a checked disjoint reference whose access origin is out of range",
            ),
        )?;
        guarded.access = access;
        guarded.source = source;
        guarded_sites.try_reserve(1).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "checked disjoint access-site storage cannot be reserved",
            )
        })?;
        guarded_sites.push(GuardedAccessSiteV1 {
            insertion_operation: operations.len(),
            access: guarded,
        });
        return Ok(());
    }
    let Some(local) = function.locals().get(place.local().index() as usize) else {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an indexed place with an out-of-range local",
        ));
    };
    let mut current = local.ty();
    let mut shape = Vec::new();
    let mut dynamic_extents = Vec::new();
    let mut indices = Vec::new();
    let mut comparisons = Vec::new();
    let mut crosses_memory_boundary = false;
    let mut dereferenced_memory_space = None;
    for projection in place.projections() {
        match projection.kind() {
            SemanticProjectionKindV1::Dereference => {
                crosses_memory_boundary = true;
                let Some(ty) = types.get(current.index() as usize) else {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "an indexed place with an out-of-range type",
                    ));
                };
                let SemanticTypeShapeV1::Pointer(pointer) = ty.shape() else {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a dereference whose semantic type is not a pointer",
                    ));
                };
                dereferenced_memory_space = Some(memory_space(pointer.address_space())?);
                current = pointer.pointee();
            }
            SemanticProjectionKindV1::Index(index) => {
                crosses_memory_boundary = true;
                if shape.len() == MAX_RANKED_MEMORY_RANK {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "an indexed place exceeding the ranked-memory rank limit",
                    ));
                }
                match types.get(current.index() as usize).map(|ty| ty.shape()) {
                    Some(SemanticTypeShapeV1::Array { length, .. }) => {
                        let value = constants
                            .get(index.index() as usize)
                            .copied()
                            .flatten()
                            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                                "a dynamic array index before exact static-extent guard projection",
                            ))?;
                        shape.push(*length);
                        indices.push(ProjectedIndexV1::Constant(value));
                    }
                    Some(SemanticTypeShapeV1::Slice { .. }) => {
                        let check = projected_bounds_check(
                            bounds_checks,
                            block_index,
                            place.local(),
                            index,
                        )?;
                        shape.push(DYNAMIC_EXTENT);
                        dynamic_extents.push(check.extent);
                        indices.push(ProjectedIndexV1::Dynamic(check.index));
                        comparisons.push((check.index, check.extent));
                    }
                    _ => {
                        return Err(ProductionRankedProjectionErrorV1::Unsupported(
                            "an index projection whose base is not an array or slice",
                        ));
                    }
                }
                current = projection.result_type();
            }
            SemanticProjectionKindV1::ConstantIndex {
                offset, from_end, ..
            } => {
                crosses_memory_boundary = true;
                if shape.len() == MAX_RANKED_MEMORY_RANK {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "an indexed place exceeding the ranked-memory rank limit",
                    ));
                }
                let extent = static_array_extent(types, current)?;
                let value = if from_end {
                    extent.checked_sub(offset).ok_or(
                        ProductionRankedProjectionErrorV1::Unsupported(
                            "a from-end constant index larger than its static extent",
                        ),
                    )?
                } else {
                    offset
                };
                shape.push(extent);
                indices.push(ProjectedIndexV1::Constant(value));
                current = projection.result_type();
            }
            SemanticProjectionKindV1::Field(_)
            | SemanticProjectionKindV1::Downcast(_)
            | SemanticProjectionKindV1::OpaqueCast
            | SemanticProjectionKindV1::Subtype => current = projection.result_type(),
            SemanticProjectionKindV1::Subslice { .. } => {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "an indexed place containing a subslice projection",
                ));
            }
        }
    }
    if indices.is_empty() {
        if crosses_memory_boundary {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a dereferenced memory access without a ranked index projection",
            ));
        }
        if requirement == PlaceAccessRequirementV1::ExplicitMemory {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "an explicit memory operation without a ranked index projection",
            ));
        }
        return Ok(());
    }
    reserve_projected_access(operations, sources, indices.len() + 2)?;
    let element_width = type_width(types, place.ty())?;
    let memory_space = if let Some(memory_space) = dereferenced_memory_space {
        memory_space
    } else if matches!(local.role(), SemanticLocalRoleV1::Argument(_)) {
        MemorySpaceAttr::Global
    } else {
        MemorySpaceAttr::Private
    };
    if memory_space == MemorySpaceAttr::Workgroup {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "workgroup memory before exact semantic CFG projection is available",
        ));
    }
    let allocation_contract = match memory_space {
        MemorySpaceAttr::Global => local_contracts
            .allocations
            .get(place.local().index() as usize)
            .copied()
            .flatten()
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "an indexed global allocation lacks authenticated Rust pointer provenance",
            ))?,
        MemorySpaceAttr::Private => {
            let identity = u64::from(place.local().index()).checked_add(1).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "a private allocation identity overflowed",
                ),
            )?;
            let identity = PRIVATE_ALLOCATION_ORIGIN_TAG_V1
                .checked_add(identity)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "a private allocation identity overflowed",
                ))?;
            AllocationContractV1 {
                allocation_origin: identity,
                noalias_class: identity,
                writable: true,
            }
        }
        MemorySpaceAttr::Workgroup => unreachable!(),
    };
    if access.writes_memory() && !allocation_contract.writable {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a write is rooted in a read-only Rust allocation",
        ));
    }
    let view_slot = projected_views
        .get_mut(place.local().index() as usize)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "an indexed place outside the ranked view table",
        ))?;
    let view_id = if let Some(view) = view_slot {
        if view.element_width != element_width
            || view.writable != allocation_contract.writable
            || view.shape != shape
            || view.dynamic_extents != dynamic_extents
            || view.memory_space != memory_space
            || view.allocation_origin != allocation_contract.allocation_origin
            || view.noalias_class != allocation_contract.noalias_class
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "one semantic allocation used through inconsistent ranked views",
            ));
        }
        view.result
    } else {
        let view_id = next_value_id(next_value)?;
        operations.push(ProductionRankedOperationV1::ViewInSpace {
            result: view_id,
            element_width,
            writable: allocation_contract.writable,
            shape: shape.clone(),
            dynamic_extents: dynamic_extents.clone(),
            memory_space,
            allocation_origin: allocation_contract.allocation_origin,
            noalias_class: allocation_contract.noalias_class,
        });
        push_ranked_ir(
            ranked_ir,
            &format!(
                "  %{} = kernel.ranked_view <{}, {}, {:?}, {:?}, origin={}, noalias={}>\n",
                view_id.get(),
                element_width,
                allocation_contract.writable,
                shape,
                memory_space,
                allocation_contract.allocation_origin,
                allocation_contract.noalias_class,
            ),
        )?;
        *view_slot = Some(ProjectedViewV1 {
            result: view_id,
            element_width,
            writable: allocation_contract.writable,
            shape: shape.clone(),
            dynamic_extents: dynamic_extents.clone(),
            memory_space,
            allocation_origin: allocation_contract.allocation_origin,
            noalias_class: allocation_contract.noalias_class,
        });
        view_id
    };
    let mut ranked_indices = Vec::with_capacity(indices.len());
    for value in indices {
        match value {
            ProjectedIndexV1::Constant(value) => {
                let index_id = next_value_id(next_value)?;
                operations.push(ProductionRankedOperationV1::IndexConstant {
                    result: index_id,
                    value,
                });
                push_ranked_ir(
                    ranked_ir,
                    &format!("  %{} = kernel.index_constant {}\n", index_id.get(), value,),
                )?;
                ranked_indices.push(ProductionRankedValueV1::Local(index_id));
            }
            ProjectedIndexV1::Dynamic(value) => ranked_indices.push(value),
        }
    }
    if !comparisons.is_empty() {
        if atomic.is_some() {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a dynamically bounds-checked atomic access before guarded atomic projection",
            ));
        }
        guarded_sites.try_reserve(1).map_err(|_| {
            ProductionRankedProjectionErrorV1::Unsupported(
                "Rust bounds-checked access-site storage cannot be reserved",
            )
        })?;
        guarded_sites.push(GuardedAccessSiteV1 {
            insertion_operation: operations.len(),
            access: GuardedRankedAccessV1 {
                view: view_id,
                indices: ranked_indices,
                comparisons,
                access,
                memory_space,
                source,
                semantic_site: None,
            },
        });
        return Ok(());
    }
    let operation = operations.len();
    if let Some(atomic) = atomic {
        operations.push(ProductionRankedOperationV1::AtomicAccess {
            kind: access,
            ordering: atomic_ordering_v1(atomic.ordering()),
            scope: atomic_scope_v1(atomic.scope()),
            view: ProductionRankedValueV1::Local(view_id),
            indices: ranked_indices.clone(),
        });
    } else {
        operations.push(ProductionRankedOperationV1::Access {
            kind: access,
            view: ProductionRankedValueV1::Local(view_id),
            indices: ranked_indices.clone(),
        });
    }
    push_ranked_ir(
        ranked_ir,
        &format!(
            "  kernel.{} {:?} %{}[{}]\n",
            if atomic.is_some() {
                "atomic_access"
            } else {
                "access"
            },
            access,
            view_id.get(),
            ranked_indices
                .iter()
                .map(|value| match value {
                    ProductionRankedValueV1::Local(identity) => format!("%{}", identity.get()),
                    ProductionRankedValueV1::Argument(argument) => format!("%arg{argument}"),
                    ProductionRankedValueV1::BlockArgument { block, argument } => {
                        format!("%bb{block}_arg{argument}")
                    }
                })
                .collect::<Vec<_>>()
                .join(", "),
        ),
    )?;
    sources.push(ProjectedAccessSourceV1 {
        block: 0,
        operation,
        access,
        memory_space,
        source,
        semantic_site: None,
    });
    Ok(())
}

const fn atomic_ordering_v1(ordering: SemanticAtomicOrderingV1) -> AtomicOrderingAttr {
    match ordering {
        SemanticAtomicOrderingV1::Relaxed => AtomicOrderingAttr::Relaxed,
        SemanticAtomicOrderingV1::Release => AtomicOrderingAttr::Release,
        SemanticAtomicOrderingV1::Acquire => AtomicOrderingAttr::Acquire,
        SemanticAtomicOrderingV1::AcquireRelease => AtomicOrderingAttr::AcquireRelease,
        SemanticAtomicOrderingV1::SequentiallyConsistent => {
            AtomicOrderingAttr::SequentiallyConsistent
        }
    }
}

const fn atomic_scope_v1(scope: SemanticAtomicScopeV1) -> AtomicScopeAttr {
    match scope {
        SemanticAtomicScopeV1::SingleThread => AtomicScopeAttr::SingleThread,
        SemanticAtomicScopeV1::Workgroup => AtomicScopeAttr::Workgroup,
        SemanticAtomicScopeV1::Agent => AtomicScopeAttr::Agent,
        SemanticAtomicScopeV1::Device => AtomicScopeAttr::Device,
        SemanticAtomicScopeV1::System => AtomicScopeAttr::System,
    }
}

fn reserve_projected_access(
    operations: &mut Vec<ProductionRankedOperationV1>,
    sources: &mut Vec<ProjectedAccessSourceV1>,
    additional_operations: usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let actual = operations.len().checked_add(additional_operations).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic statement projection with overflowing operation work",
        ),
    )?;
    if actual > MAX_PROJECTED_OPERATIONS_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic statement projection exceeding the ranked operation limit",
        ));
    }
    operations.try_reserve(additional_operations).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic statement projection whose operation storage cannot be reserved",
        )
    })?;
    sources.try_reserve(1).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic statement projection whose source storage cannot be reserved",
        )
    })
}

fn push_ranked_ir(
    ranked_ir: &mut String,
    fragment: &str,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let actual = ranked_ir.len().checked_add(fragment.len()).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic statement projection with overflowing diagnostic text",
        ),
    )?;
    if actual > MAX_PROJECTED_RANKED_IR_BYTES_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic statement projection exceeding the diagnostic text limit",
        ));
    }
    ranked_ir.try_reserve(fragment.len()).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic statement projection whose diagnostic storage cannot be reserved",
        )
    })?;
    ranked_ir.push_str(fragment);
    Ok(())
}

fn static_array_extent(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<u64, ProductionRankedProjectionErrorV1> {
    match types.get(ty.index() as usize).map(|ty| ty.shape()) {
        Some(SemanticTypeShapeV1::Array { length, .. }) => Ok(*length),
        Some(SemanticTypeShapeV1::Slice { .. }) => {
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a slice access before dynamic extent projection is available",
            ))
        }
        _ => Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an index projection whose base is not an array or slice",
        )),
    }
}

fn memory_space(address_space: u32) -> Result<MemorySpaceAttr, ProductionRankedProjectionErrorV1> {
    match address_space {
        0 | 1 | 4 => Ok(MemorySpaceAttr::Global),
        3 => Ok(MemorySpaceAttr::Workgroup),
        5 => Ok(MemorySpaceAttr::Private),
        _ => Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a pointer address space outside the generic memory model",
        )),
    }
}

fn type_width(
    types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<u32, ProductionRankedProjectionErrorV1> {
    let bytes = types
        .get(ty.index() as usize)
        .and_then(|ty| ty.layout().size_bytes())
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "an indexed element without a static layout size",
        ))?;
    let bits = u32::try_from(bytes.checked_mul(8).ok_or(
        ProductionRankedProjectionErrorV1::Unsupported("an overflowing element width"),
    )?)
    .map_err(|_| ProductionRankedProjectionErrorV1::Unsupported("an overflowing element width"))?;
    if !SUPPORTED_ELEMENT_WIDTHS.contains(&bits) {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an element width outside the ranked-memory dialect",
        ));
    }
    Ok(bits)
}

fn next_value_id(
    next: &mut u32,
) -> Result<ProductionRankedValueIdV1, ProductionRankedProjectionErrorV1> {
    let value = *next;
    *next = next
        .checked_add(1)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "too many ranked SSA values",
        ))?;
    Ok(ProductionRankedValueIdV1::new(value))
}

fn function_name(
    function: &SemanticFunctionDeclV1,
) -> Result<&str, ProductionRankedProjectionErrorV1> {
    let symbol = function
        .kernel_entry()
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "a kernel root without a kernel export",
        ))?
        .export_symbol()
        .as_bytes();
    std::str::from_utf8(symbol).map_err(|_| {
        ProductionRankedProjectionErrorV1::Unsupported("a non-UTF-8 kernel export symbol")
    })
}

fn source_label(source: SemanticSourceProvenanceV1) -> String {
    let Some(origin) = source.call_site().or_else(|| source.expansion()) else {
        return "Rust source location unavailable".to_owned();
    };
    let (line, column) = origin.start_coordinate();
    let digest = origin.file();
    format!(
        "Rust source {}:{}:{}",
        &crate::encode_hex(digest.as_bytes())[..12],
        line,
        column,
    )
}

fn indent_ir(ir: &str) -> String {
    ir.lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_mir_model::SemanticOptionProducerV1;
    use fe2o3_mir_model::semantic_mir_v1::*;

    const SCALAR_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
    const ARRAY_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const POINTER_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
    const ENUM_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
    const BOOL_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
    const U64_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
    const U8_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
    const I32_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);

    fn bytes(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn projection_types() -> Vec<SemanticTypeDeclV1> {
        vec![
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(1)),
                SemanticLayoutIdentityV1::from_sha256(bytes(1)),
                SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                }),
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(2)),
                SemanticLayoutIdentityV1::from_sha256(bytes(2)),
                SemanticTypeLayoutV1::new(Some(16), 4).unwrap(),
                SemanticTypeShapeV1::Array {
                    element: SCALAR_TYPE,
                    length: 4,
                },
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(3)),
                SemanticLayoutIdentityV1::from_sha256(bytes(3)),
                SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new(
                        SCALAR_TYPE,
                        SemanticMutabilityV1::Mutable,
                        1,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            ),
        ]
    }

    fn projection_types_with_enum() -> Vec<SemanticTypeDeclV1> {
        let mut types = projection_types();
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(4)),
            SemanticLayoutIdentityV1::from_sha256(bytes(4)),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::enum_type(
                SCALAR_TYPE,
                vec![
                    SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                    SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                ],
            )
            .unwrap(),
        ));
        types
    }

    fn assertion_proof_types() -> Vec<SemanticTypeDeclV1> {
        let mut types = projection_types_with_enum();
        for (tag, bytes, scalar) in [
            (40, 1, SemanticScalarTypeV1::Bool),
            (
                41,
                8,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                },
            ),
            (
                42,
                1,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 8,
                },
            ),
            (
                43,
                4,
                SemanticScalarTypeV1::Integer {
                    signed: true,
                    bits: 32,
                },
            ),
        ] {
            types.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(self::bytes(tag)),
                SemanticLayoutIdentityV1::from_sha256(self::bytes(tag)),
                SemanticTypeLayoutV1::new(Some(bytes), bytes).unwrap(),
                SemanticTypeShapeV1::Scalar(scalar),
            ));
        }
        types
    }

    fn enum_definition(local: SemanticLocalIdV1, variant: u32) -> SemanticStatementV1 {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(local, vec![], ENUM_TYPE).unwrap(),
            SemanticRvalueV1::new(
                ENUM_TYPE,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::EnumVariant(variant),
                        vec![],
                    )
                    .unwrap(),
                ),
            ),
        )))
    }

    fn enum_discriminant(
        carrier: SemanticLocalIdV1,
        destination: SemanticLocalIdV1,
    ) -> SemanticStatementV1 {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(destination, vec![], SCALAR_TYPE).unwrap(),
            SemanticRvalueV1::new(
                SCALAR_TYPE,
                SemanticRvalueKindV1::Discriminant(
                    SemanticPlaceV1::new(carrier, vec![], ENUM_TYPE).unwrap(),
                ),
            ),
        )))
    }

    fn local(tag: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(tag)),
            ty,
            role,
            SemanticSourceProvenanceV1::unavailable(),
        )
    }

    fn block(
        tag: u8,
        statements: Vec<SemanticStatementV1>,
        terminator: SemanticTerminatorKindV1,
    ) -> SemanticBasicBlockV1 {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256(bytes(tag)),
            SemanticSourceProvenanceV1::unavailable(),
            statements,
            SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
        )
        .unwrap()
    }

    fn cfg_edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
        SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
    }
    fn projection_function(blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
        projection_function_with_locals(
            blocks,
            vec![
                local(20, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(21, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
                local(22, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(23, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    fn projection_function_with_locals(
        blocks: Vec<SemanticBasicBlockV1>,
        locals: Vec<SemanticLocalDeclV1>,
    ) -> SemanticFunctionDeclV1 {
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(10)),
            SemanticLayoutIdentityV1::from_sha256(bytes(10)),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(11)),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(12)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(13)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(14)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(15)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    }

    fn projection_function_with_owned_argument(
        blocks: Vec<SemanticBasicBlockV1>,
        locals: Vec<SemanticLocalDeclV1>,
        ownership: SemanticSourceArgumentOwnershipV1,
    ) -> SemanticFunctionDeclV1 {
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(bytes(110)),
            SemanticLayoutIdentityV1::from_sha256(bytes(110)),
            SemanticCanonAbiV1::GpuKernel,
            false,
            false,
            vec![SemanticAbiValueV1::new(
                POINTER_TYPE,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            )],
            SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![ownership])
        .unwrap();
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(111)),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(112)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(113)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(114)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(115)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    }

    fn barrier_call(target: Option<u32>) -> SemanticTerminatorKindV1 {
        let destination = target.map(|target| {
            SemanticCallDestinationV1::new(
                scalar_place(),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(target),
                ),
            )
        });
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                destination,
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    }

    fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
        SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
    }

    fn scalar_place() -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], SCALAR_TYPE).unwrap()
    }

    fn ranked_place(offset: u64) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset,
                        minimum_length: 4,
                        from_end: false,
                    },
                    SCALAR_TYPE,
                )
                .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap()
    }

    fn dereferenced_place() -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(3),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR_TYPE)
                    .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap()
    }

    fn constant(value: u128) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            SCALAR_TYPE,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
        ))
    }

    fn bounds_check_function(
        operation: SemanticBinaryOpV1,
        expected: bool,
        swap_message_operands: bool,
        alternate_predecessor: bool,
    ) -> SemanticFunctionDeclV1 {
        let condition_local = SemanticLocalIdV1::from_index(2);
        let index_local = SemanticLocalIdV1::from_index(4);
        let length_local = SemanticLocalIdV1::from_index(5);
        let place = |local| SemanticPlaceV1::new(local, vec![], SCALAR_TYPE).unwrap();
        let operand = |local| SemanticOperandV1::Copy(place(local));
        let index_definition =
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(index_local),
                SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(0))),
            )));
        let slice =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], ARRAY_TYPE).unwrap();
        let length_definition =
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(length_local),
                SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Length(slice)),
            )));
        let comparison = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(condition_local),
            SemanticRvalueV1::new(
                SCALAR_TYPE,
                SemanticRvalueKindV1::Binary {
                    operation,
                    left: operand(index_local),
                    right: operand(length_local),
                },
            ),
        )));
        let (index, length) = if swap_message_operands {
            (operand(length_local), operand(index_local))
        } else {
            (operand(index_local), operand(length_local))
        };
        let success_block = if alternate_predecessor { 2 } else { 1 };
        let mut blocks = vec![block(
            80,
            vec![index_definition, length_definition, comparison],
            SemanticTerminatorKindV1::Assert {
                condition: operand(condition_local),
                expected,
                message: SemanticAssertMessageV1::BoundsCheck { length, index },
                target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, success_block),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        )];
        if alternate_predecessor {
            blocks.push(block(
                81,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
            ));
        }
        blocks.push(block(82, vec![], SemanticTerminatorKindV1::Return));
        projection_function_with_locals(
            blocks,
            vec![
                local(82, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(83, ARRAY_TYPE, SemanticLocalRoleV1::Temporary),
                local(84, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(85, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(86, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(87, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    fn project_test_bounds_checks(
        function: &SemanticFunctionDeclV1,
        first_argument: usize,
    ) -> Result<ProjectedBoundsChecksV1, ProductionRankedProjectionErrorV1> {
        let mut operations = Vec::new();
        let mut next_value = 0;
        project_rust_bounds_checks(function, first_argument, &mut operations, &mut next_value)
    }

    fn option_dominance_chain(
        producer_count: usize,
    ) -> (SemanticFunctionDeclV1, Vec<SemanticOptionProducerV1>) {
        assert!(producer_count > 0 && producer_count <= 64);
        let mut locals = Vec::with_capacity(1 + 2 * producer_count);
        locals.push(local(0, SCALAR_TYPE, SemanticLocalRoleV1::Return));
        let mut producers = Vec::with_capacity(producer_count);
        let mut blocks = Vec::with_capacity(3 * producer_count + 1);
        let final_some = 2 * producer_count;
        for index in 0..producer_count {
            let option_local = SemanticLocalIdV1::from_index((1 + 2 * index) as u32);
            let discriminator_local = SemanticLocalIdV1::from_index((2 + 2 * index) as u32);
            locals.push(local(
                (1 + 2 * index) as u8,
                POINTER_TYPE,
                SemanticLocalRoleV1::Temporary,
            ));
            locals.push(local(
                (2 + 2 * index) as u8,
                SCALAR_TYPE,
                SemanticLocalRoleV1::Temporary,
            ));
            let producer_block = 2 * index;
            let switch_block = producer_block + 1;
            let some_target = if index + 1 == producer_count {
                final_some
            } else {
                producer_block + 2
            };
            let none_target = final_some + 1 + index;
            let option_place = SemanticPlaceV1::new(option_local, vec![], POINTER_TYPE).unwrap();
            let discriminator_place =
                SemanticPlaceV1::new(discriminator_local, vec![], SCALAR_TYPE).unwrap();
            let call = SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    option_place.clone(),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, switch_block as u32),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap();
            blocks.push(block(
                producer_block as u8,
                vec![],
                SemanticTerminatorKindV1::Call(call),
            ));
            blocks.push(block(
                switch_block as u8,
                vec![statement(SemanticStatementKindV1::Assign(
                    SemanticAssignmentV1::new(
                        discriminator_place.clone(),
                        SemanticRvalueV1::new(
                            SCALAR_TYPE,
                            SemanticRvalueKindV1::Discriminant(option_place),
                        ),
                    ),
                ))],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(discriminator_place),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, none_target as u32),
                        )],
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, some_target as u32),
                    )
                    .unwrap(),
                },
            ));
            producers.push(SemanticOptionProducerV1::new(
                option_local,
                SemanticBlockIdV1::from_index(switch_block as u32),
            ));
        }
        blocks.push(block(
            final_some as u8,
            vec![],
            SemanticTerminatorKindV1::Return,
        ));
        for index in 0..producer_count {
            blocks.push(block(
                (final_some + 1 + index) as u8,
                vec![],
                SemanticTerminatorKindV1::Return,
            ));
        }
        (projection_function_with_locals(blocks, locals), producers)
    }

    fn atomic_access() -> SemanticAtomicAccessV1 {
        SemanticAtomicAccessV1::new(
            SemanticAtomicOrderingV1::Relaxed,
            SemanticAtomicScopeV1::Agent,
        )
    }

    type AuditOutput = (
        Vec<ProductionRankedOperationV1>,
        Vec<ProjectedAccessSourceV1>,
        String,
    );

    fn audit_function(
        function: &SemanticFunctionDeclV1,
    ) -> Result<AuditOutput, ProductionRankedProjectionErrorV1> {
        let types = projection_types();
        let constants = constant_locals(function);
        let mut operations = Vec::new();
        let mut sources = Vec::new();
        let mut projected_views = vec![None; function.locals().len()];
        let mut guarded_sites = Vec::new();
        let mut next_value = 0;
        let mut ranked_ir = String::new();
        let local_contracts = synthetic_local_contracts(function);
        for (block_index, basic_block) in function.blocks().iter().enumerate() {
            for semantic_statement in basic_block.statements() {
                project_statement_accesses(
                    &types,
                    function,
                    block_index,
                    &[],
                    semantic_statement,
                    &constants,
                    &local_contracts,
                    &[],
                    &mut guarded_sites,
                    &mut projected_views,
                    &mut operations,
                    &mut sources,
                    &mut next_value,
                    &mut ranked_ir,
                )?;
            }
            project_terminator_accesses(
                &[],
                &types,
                function,
                block_index,
                &[],
                basic_block.terminator().kind(),
                basic_block.terminator().source(),
                &constants,
                &local_contracts,
                &[],
                &mut guarded_sites,
                &mut projected_views,
                &mut operations,
                &mut sources,
                &mut next_value,
                &mut ranked_ir,
            )?;
        }
        Ok((operations, sources, ranked_ir))
    }

    fn synthetic_local_contracts(function: &SemanticFunctionDeclV1) -> ProjectionLocalContractsV1 {
        ProjectionLocalContractsV1 {
            checked_reference_origins: vec![None; function.locals().len()],
            allocations: (0..function.locals().len())
                .map(|local| {
                    let identity = local as u64 + 1;
                    Some(AllocationContractV1 {
                        allocation_origin: identity,
                        noalias_class: identity,
                        writable: true,
                    })
                })
                .collect(),
        }
    }

    fn audit_statements(
        statements: Vec<SemanticStatementV1>,
    ) -> Result<AuditOutput, ProductionRankedProjectionErrorV1> {
        audit_function(&projection_function(vec![block(
            30,
            statements,
            SemanticTerminatorKindV1::Return,
        )]))
    }

    fn access_kinds(operations: &[ProductionRankedOperationV1]) -> Vec<AccessKindAttr> {
        operations
            .iter()
            .filter_map(|operation| match operation {
                ProductionRankedOperationV1::Access { kind, .. }
                | ProductionRankedOperationV1::AtomicAccess { kind, .. }
                | ProductionRankedOperationV1::AllocationEffect { kind, .. } => Some(*kind),
                ProductionRankedOperationV1::View { .. }
                | ProductionRankedOperationV1::ExecutionLayout { .. }
                | ProductionRankedOperationV1::ViewInSpace { .. }
                | ProductionRankedOperationV1::IndexConstant { .. }
                | ProductionRankedOperationV1::IndexUnknown { .. }
                | ProductionRankedOperationV1::InvocationIndex { .. }
                | ProductionRankedOperationV1::DeterministicJoin { .. }
                | ProductionRankedOperationV1::IndexBinary { .. }
                | ProductionRankedOperationV1::CheckedTiledIndex2D { .. }
                | ProductionRankedOperationV1::Dimension { .. }
                | ProductionRankedOperationV1::Barrier { .. }
                | ProductionRankedOperationV1::Fence { .. }
                | ProductionRankedOperationV1::TensorLayout { .. }
                | ProductionRankedOperationV1::SemanticSymbol { .. }
                | ProductionRankedOperationV1::SemanticConstant { .. }
                | ProductionRankedOperationV1::SemanticBinary { .. }
                | ProductionRankedOperationV1::RequireEquivalent { .. } => None,
            })
            .collect()
    }

    fn single_guarded_cfg(
        entry: Vec<ProductionRankedOperationV1>,
        access: GuardedRankedAccessV1,
    ) -> (
        Vec<ProductionRankedBlockV1>,
        Vec<ProjectedAccessSourceV1>,
        String,
    ) {
        let function =
            projection_function(vec![block(29, vec![], SemanticTerminatorKindV1::Return)]);
        let (blocks, sources) = build_ranked_cfg(
            &projection_types(),
            &function,
            &vec![None; function.locals().len()],
            &vec![None; function.blocks().len()],
            &[],
            entry,
            vec![ProjectedSemanticBlockV1 {
                items: vec![ProjectedBlockItemV1::Guarded(access)],
            }],
        )
        .unwrap();
        let ranked_ir = format_ranked_cfg("guarded_test", &blocks).unwrap();
        (blocks, sources, ranked_ir)
    }

    fn assert_unsupported(
        result: Result<AuditOutput, ProductionRankedProjectionErrorV1>,
        expected: &'static str,
    ) {
        match result {
            Err(
                ProductionRankedProjectionErrorV1::Incomplete(detail)
                | ProductionRankedProjectionErrorV1::Unsupported(detail),
            ) => {
                assert_eq!(detail, expected)
            }
            Err(other) => panic!("expected unsupported projection, got {other}"),
            Ok(_) => panic!("hostile projection unexpectedly passed"),
        }
    }

    #[test]
    fn regular_and_atomic_stores_project_destination_and_value_footprints() {
        for atomic in [None, Some(atomic_access())] {
            let (operations, sources, _) = audit_statements(vec![statement(
                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    ranked_place(0),
                    SemanticOperandV1::Copy(ranked_place(1)),
                    SemanticVolatilityV1::NonVolatile,
                    atomic,
                )),
            )])
            .unwrap();
            assert_eq!(
                access_kinds(&operations),
                vec![
                    if atomic.is_some() {
                        AccessKindAttr::AtomicWrite
                    } else {
                        AccessKindAttr::Write
                    },
                    AccessKindAttr::Read,
                ]
            );
            assert_eq!(sources.len(), 2);
            assert_eq!(
                operations
                    .iter()
                    .filter(|operation| matches!(
                        operation,
                        ProductionRankedOperationV1::ViewInSpace { .. }
                    ))
                    .count(),
                1,
                "two effects on one semantic allocation created different PLIRON views",
            );
        }
    }

    #[test]
    fn checked_binary_projects_copy_and_move_operand_reads_in_order() {
        let checked = SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            SemanticCheckedBinaryOpV1::Multiply,
            SemanticOperandV1::Copy(ranked_place(0)),
            SemanticOperandV1::Move(ranked_place(1)),
        ));
        let function =
            projection_function(vec![block(30, vec![], SemanticTerminatorKindV1::Return)]);
        let types = projection_types();
        let mut operations = Vec::new();
        let mut sources = Vec::new();
        let mut guarded_sites = Vec::new();
        let mut projected_views = vec![None; function.locals().len()];
        let mut next_value = 0;
        let mut ranked_ir = String::new();
        let local_contracts = synthetic_local_contracts(&function);

        project_rvalue_reads(
            &types,
            &function,
            0,
            &[],
            &checked,
            SemanticSourceProvenanceV1::unavailable(),
            &[None; 4],
            &local_contracts,
            &[],
            &mut guarded_sites,
            &mut projected_views,
            &mut operations,
            &mut sources,
            &mut next_value,
            &mut ranked_ir,
        )
        .unwrap();

        assert_eq!(
            access_kinds(&operations),
            vec![AccessKindAttr::Read, AccessKindAttr::Read]
        );
        assert_eq!(sources.len(), 2);
        assert_eq!(
            ranked_ir.matches("kernel.access").count(),
            2,
            "both checked operands must survive ranked projection"
        );
    }

    #[test]
    fn guarded_disjoint_access_is_ordinary_clean_pliron_cfg() {
        let invocation = ProductionRankedValueIdV1::new(0);
        let view = ProductionRankedValueIdV1::new(1);
        let entry = vec![
            ProductionRankedOperationV1::InvocationIndex {
                result: invocation,
                dimension: 0,
                launch_extent: 64,
            },
            ProductionRankedOperationV1::ViewInSpace {
                result: view,
                element_width: 32,
                writable: true,
                shape: vec![DYNAMIC_EXTENT],
                dynamic_extents: vec![ProductionRankedValueV1::Argument(0)],
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: 0,
                noalias_class: 0,
            },
        ];
        let guarded = GuardedRankedAccessV1 {
            view,
            indices: vec![ProductionRankedValueV1::Local(invocation)],
            comparisons: vec![(
                ProductionRankedValueV1::Local(invocation),
                ProductionRankedValueV1::Argument(0),
            )],
            access: AccessKindAttr::Write,
            memory_space: MemorySpaceAttr::Global,
            source: SemanticSourceProvenanceV1::unavailable(),
            semantic_site: None,
        };
        let (blocks, sources, ranked_ir) = single_guarded_cfg(entry, guarded);
        assert_eq!(blocks.len(), 5);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].block, 2);
        assert!(matches!(
            blocks[3].terminator(),
            ProductionRankedTerminatorV1::Trap
        ));
        let kernel = ProductionRankedKernelV1::new("generic_checked_access", 1, blocks).unwrap();
        let construction =
            ProductionConstructionV1::ranked_kernel("checked_access_module", kernel).unwrap();
        let lowering = compile_ranked_kernel_for_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        assert!(lowering.bounds_report().is_clean());
        assert!(lowering.race_report().is_clean());
        assert!(ranked_ir.contains("kernel.cond_br") && ranked_ir.contains("kernel.access"));
        assert!(ranked_ir.contains("kernel.br ^bb4"));
    }

    #[test]
    fn rust_bounds_check_projects_only_the_exact_index_less_than_length_guard() {
        let function = bounds_check_function(SemanticBinaryOpV1::LessThan, true, false, false);
        let mut operations = Vec::new();
        let mut next_value = 0;
        let projected =
            project_rust_bounds_checks(&function, 3, &mut operations, &mut next_value).unwrap();

        assert_eq!(projected.argument_count, 3);
        assert_eq!(next_value, 2);
        assert!(matches!(
            operations.as_slice(),
            [
                ProductionRankedOperationV1::IndexUnknown { result: first },
                ProductionRankedOperationV1::IndexUnknown { result: second },
            ] if first.get() == 0 && second.get() == 1
        ));
        assert_eq!(projected.checks.len(), 1);
        assert_eq!(projected.checks[0].access_block, 1);
        assert_eq!(projected.checks[0].slice_local.index(), 1);
        assert_eq!(projected.checks[0].index_local.index(), 4);
        assert_eq!(
            projected.checks[0].index,
            ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0))
        );
        assert_eq!(
            projected.checks[0].extent,
            ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1))
        );
    }

    #[test]
    fn forged_rust_bounds_messages_and_conditions_fail_closed() {
        assert!(matches!(
            project_test_bounds_checks(
                &bounds_check_function(SemanticBinaryOpV1::GreaterThan, true, false, false),
                0,
            ),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a Rust bounds-check message not backed by its exact index < length condition"
            ))
        ));
        assert!(matches!(
            project_test_bounds_checks(
                &bounds_check_function(SemanticBinaryOpV1::LessThan, true, true, false),
                0,
            ),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a Rust bounds-check length not derived from one exact slice"
            ))
        ));
        assert!(matches!(
            project_test_bounds_checks(
                &bounds_check_function(SemanticBinaryOpV1::LessThan, false, false, false),
                0,
            ),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a Rust bounds check without the canonical success/unreachable shape"
            ))
        ));
    }

    #[test]
    fn rust_bounds_check_cannot_authorize_another_slice_or_a_bypass_edge() {
        let function = bounds_check_function(SemanticBinaryOpV1::LessThan, true, false, false);
        let projected = project_test_bounds_checks(&function, 0).unwrap();
        assert!(matches!(
            projected_bounds_check(
                &projected.checks,
                1,
                SemanticLocalIdV1::from_index(3),
                SemanticLocalIdV1::from_index(4),
            ),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a dynamic slice access without its exact Rust bounds-check predecessor"
            ))
        ));

        assert!(matches!(
            project_test_bounds_checks(
                &bounds_check_function(SemanticBinaryOpV1::LessThan, true, false, true),
                0,
            ),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a Rust bounds-check success block not uniquely controlled by that check"
            ))
        ));
    }

    #[test]
    fn safe_syncthreads_reaches_mandatory_barrier_and_workgroup_checks() {
        let kernel = ProductionRankedKernelV1::new(
            "safe_syncthreads",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![
                    ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: 1,
                        global_extents: [64, 1, 1],
                        workgroup_extents: [64, 1, 1],
                        subgroup_size: 64,
                        full_physical_workgroups: true,
                    },
                    ProductionRankedOperationV1::Barrier {
                        execution_scope: HierarchyAttr::Workgroup,
                        memory_scope: MemoryScopeAttr::Workgroup,
                        address_space: AddressSpaceAttr::Workgroup,
                        order: MemoryOrderAttr::AcquireRelease,
                    },
                ],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let construction =
            ProductionConstructionV1::ranked_kernel("safe_syncthreads_module", kernel).unwrap();
        let lowering = compile_ranked_kernel_for_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        assert!(lowering.barrier_report().is_clean());
        assert!(lowering.workgroup_report().is_clean());
    }

    #[test]
    fn guarded_access_and_barrier_retain_semantic_source_order() {
        let guarded = || {
            ProjectedBlockItemV1::Guarded(GuardedRankedAccessV1 {
                view: ProductionRankedValueIdV1::new(0),
                indices: vec![ProductionRankedValueV1::Argument(0)],
                comparisons: vec![(
                    ProductionRankedValueV1::Argument(0),
                    ProductionRankedValueV1::Argument(1),
                )],
                access: AccessKindAttr::Write,
                memory_space: MemorySpaceAttr::Global,
                source: SemanticSourceProvenanceV1::unavailable(),
                semantic_site: None,
            })
        };
        let barrier = || ProjectedBlockItemV1::Effect {
            operation: ProductionRankedOperationV1::Barrier {
                execution_scope: HierarchyAttr::Workgroup,
                memory_scope: MemoryScopeAttr::Workgroup,
                address_space: AddressSpaceAttr::Workgroup,
                order: MemoryOrderAttr::AcquireRelease,
            },
            source: None,
        };
        let function = projection_function(vec![
            block(70, vec![], barrier_call(Some(1))),
            block(71, vec![], SemanticTerminatorKindV1::Return),
        ]);

        let (after_blocks, _) = build_ranked_cfg(
            &projection_types(),
            &function,
            &vec![None; function.locals().len()],
            &vec![None; function.blocks().len()],
            &[],
            vec![],
            vec![
                ProjectedSemanticBlockV1 {
                    items: vec![guarded(), barrier()],
                },
                ProjectedSemanticBlockV1 { items: vec![] },
            ],
        )
        .unwrap();
        assert!(matches!(
            after_blocks[2].operations(),
            [ProductionRankedOperationV1::Access { .. }]
        ));
        assert!(matches!(
            after_blocks[4].operations(),
            [ProductionRankedOperationV1::Barrier { .. }]
        ));

        let (before_blocks, _) = build_ranked_cfg(
            &projection_types(),
            &function,
            &vec![None; function.locals().len()],
            &vec![None; function.blocks().len()],
            &[],
            vec![],
            vec![
                ProjectedSemanticBlockV1 {
                    items: vec![barrier()],
                },
                ProjectedSemanticBlockV1 {
                    items: vec![guarded()],
                },
            ],
        )
        .unwrap();
        assert!(matches!(
            before_blocks[1].operations(),
            [ProductionRankedOperationV1::Barrier { .. }]
        ));
        assert!(matches!(
            before_blocks[3].operations(),
            [ProductionRankedOperationV1::Access { .. }]
        ));
    }

    #[test]
    fn shifted_disjoint_access_retains_overflow_and_extent_guards() {
        let invocation = ProductionRankedValueIdV1::new(0);
        let offset = ProductionRankedValueIdV1::new(1);
        let shifted = ProductionRankedValueIdV1::new(2);
        let upper = ProductionRankedValueIdV1::new(3);
        let view = ProductionRankedValueIdV1::new(4);
        let entry = vec![
            ProductionRankedOperationV1::InvocationIndex {
                result: invocation,
                dimension: 0,
                launch_extent: 64,
            },
            ProductionRankedOperationV1::IndexConstant {
                result: offset,
                value: 4,
            },
            ProductionRankedOperationV1::IndexBinary {
                result: shifted,
                kind: IndexBinaryKindAttr::Add,
                lhs: ProductionRankedValueV1::Local(invocation),
                rhs: ProductionRankedValueV1::Local(offset),
            },
            ProductionRankedOperationV1::IndexConstant {
                result: upper,
                value: u64::MAX - 3,
            },
            ProductionRankedOperationV1::ViewInSpace {
                result: view,
                element_width: 32,
                writable: true,
                shape: vec![DYNAMIC_EXTENT],
                dynamic_extents: vec![ProductionRankedValueV1::Argument(0)],
                memory_space: MemorySpaceAttr::Global,
                allocation_origin: 0,
                noalias_class: 0,
            },
        ];
        let guarded = GuardedRankedAccessV1 {
            view,
            indices: vec![ProductionRankedValueV1::Local(shifted)],
            comparisons: vec![
                (
                    ProductionRankedValueV1::Local(invocation),
                    ProductionRankedValueV1::Local(upper),
                ),
                (
                    ProductionRankedValueV1::Local(shifted),
                    ProductionRankedValueV1::Argument(0),
                ),
            ],
            access: AccessKindAttr::Write,
            memory_space: MemorySpaceAttr::Global,
            source: SemanticSourceProvenanceV1::unavailable(),
            semantic_site: None,
        };
        let (blocks, sources, ranked_ir) = single_guarded_cfg(entry, guarded);
        assert_eq!(blocks.len(), 6);
        assert_eq!(sources[0].block, 3);
        assert!(matches!(
            blocks[4].terminator(),
            ProductionRankedTerminatorV1::Trap
        ));
        assert!(ranked_ir.contains("kernel.br ^bb5"));
        assert!(ranked_ir.contains("^bb3:"));

        let kernel = ProductionRankedKernelV1::new("shifted_checked_access", 1, blocks).unwrap();
        let construction =
            ProductionConstructionV1::ranked_kernel("shifted_access_module", kernel).unwrap();
        let lowering = compile_ranked_kernel_for_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        assert!(lowering.bounds_report().is_clean());
        assert!(lowering.race_report().is_clean());
    }

    #[test]
    fn shared_option_dominance_scales_with_cfg_and_producer_count() {
        let (small_function, small_producers) = option_dominance_chain(16);
        let small = SemanticOptionDominanceV1::analyze(&small_function, &small_producers).unwrap();
        let (large_function, large_producers) = option_dominance_chain(64);
        let large = SemanticOptionDominanceV1::analyze(&large_function, &large_producers).unwrap();

        assert!(large.work_units() <= small.work_units() * 5);
        for producer in large_producers {
            assert!(large.availability(producer.option_local()).is_some());
        }
    }

    #[test]
    fn enum_payload_dominance_tracks_only_the_exact_variant_branch() {
        let carrier = SemanticLocalIdV1::from_index(1);
        let discriminator = SemanticLocalIdV1::from_index(2);
        let discriminator_place = SemanticPlaceV1::new(discriminator, vec![], SCALAR_TYPE).unwrap();
        let function = projection_function_with_locals(
            vec![
                block(
                    80,
                    vec![
                        enum_definition(carrier, 0),
                        enum_discriminant(carrier, discriminator),
                    ],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(discriminator_place),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                block(
                    81,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(82, vec![], SemanticTerminatorKindV1::Return),
                block(83, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(20, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(21, ENUM_TYPE, SemanticLocalRoleV1::Temporary),
                local(22, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        let dominance =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types_with_enum())
                .unwrap();
        let zero = dominance.availability(carrier, 0).unwrap();
        let one = dominance.availability(carrier, 1).unwrap();

        assert!(dominance.allows(zero, SemanticBlockIdV1::from_index(1)));
        assert!(dominance.allows(zero, SemanticBlockIdV1::from_index(3)));
        assert!(!dominance.allows(zero, SemanticBlockIdV1::from_index(2)));
        assert!(dominance.allows(one, SemanticBlockIdV1::from_index(2)));
        assert!(!dominance.grants_authority());
    }

    #[test]
    fn enum_payload_branch_with_an_alternate_predecessor_is_not_authenticated() {
        let carrier = SemanticLocalIdV1::from_index(1);
        let discriminator = SemanticLocalIdV1::from_index(2);
        let function = projection_function_with_locals(
            vec![
                block(
                    84,
                    vec![
                        enum_definition(carrier, 0),
                        enum_discriminant(carrier, discriminator),
                    ],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(
                            SemanticPlaceV1::new(discriminator, vec![], SCALAR_TYPE).unwrap(),
                        ),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                block(85, vec![], SemanticTerminatorKindV1::Return),
                block(
                    86,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
            ],
            vec![
                local(20, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(21, ENUM_TYPE, SemanticLocalRoleV1::Temporary),
                local(22, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        let dominance =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types_with_enum())
                .unwrap();

        assert!(dominance.availability(carrier, 0).is_none());
        assert!(dominance.availability(carrier, 1).is_some());
    }

    #[test]
    fn multiply_defined_enum_carrier_cannot_authenticate_a_payload() {
        let carrier = SemanticLocalIdV1::from_index(1);
        let discriminator = SemanticLocalIdV1::from_index(2);
        let function = projection_function_with_locals(
            vec![
                block(
                    87,
                    vec![
                        enum_definition(carrier, 0),
                        enum_definition(carrier, 1),
                        enum_discriminant(carrier, discriminator),
                    ],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(
                            SemanticPlaceV1::new(discriminator, vec![], SCALAR_TYPE).unwrap(),
                        ),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                block(88, vec![], SemanticTerminatorKindV1::Return),
                block(89, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(20, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(21, ENUM_TYPE, SemanticLocalRoleV1::Temporary),
                local(22, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        let dominance =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types_with_enum())
                .unwrap();

        assert!(dominance.availability(carrier, 0).is_none());
        assert!(dominance.availability(carrier, 1).is_none());
    }

    #[test]
    fn private_address_space_five_remains_in_the_generic_memory_model() {
        assert_eq!(memory_space(5).unwrap(), MemorySpaceAttr::Private);
    }

    #[test]
    fn reassigned_option_discriminator_cannot_mint_payload_authority() {
        let option_local = SemanticLocalIdV1::from_index(3);
        let discriminator_local = SemanticLocalIdV1::from_index(2);
        let option_place = SemanticPlaceV1::new(option_local, vec![], POINTER_TYPE).unwrap();
        let discriminator_place =
            SemanticPlaceV1::new(discriminator_local, vec![], SCALAR_TYPE).unwrap();
        let discriminant = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            discriminator_place.clone(),
            SemanticRvalueV1::new(
                SCALAR_TYPE,
                SemanticRvalueKindV1::Discriminant(option_place.clone()),
            ),
        )));
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                option_place,
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let function = projection_function(vec![
            block(50, vec![], SemanticTerminatorKindV1::Call(call.clone())),
            block(
                51,
                vec![
                    discriminant,
                    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        discriminator_place.clone(),
                        SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(1))),
                    ))),
                ],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(discriminator_place),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
                        )],
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(52, vec![], SemanticTerminatorKindV1::Return),
            block(53, vec![], SemanticTerminatorKindV1::Return),
        ]);
        let producer =
            SemanticOptionProducerV1::new(option_local, SemanticBlockIdV1::from_index(1));
        let error = SemanticOptionDominanceV1::analyze(&function, &[producer]).unwrap_err();

        assert_eq!(
            error.detail(),
            "an Option capability discriminator does not have one exact definition"
        );
    }
    #[test]
    fn unrelated_switch_cannot_authenticate_option_payload() {
        let option_local = SemanticLocalIdV1::from_index(3);
        let discriminator_local = SemanticLocalIdV1::from_index(2);
        let option_place = SemanticPlaceV1::new(option_local, vec![], POINTER_TYPE).unwrap();
        let discriminator_place =
            SemanticPlaceV1::new(discriminator_local, vec![], SCALAR_TYPE).unwrap();
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                option_place.clone(),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let function = projection_function(vec![
            block(60, vec![], SemanticTerminatorKindV1::Call(call)),
            block(
                61,
                vec![statement(SemanticStatementKindV1::Assign(
                    SemanticAssignmentV1::new(
                        discriminator_place,
                        SemanticRvalueV1::new(
                            SCALAR_TYPE,
                            SemanticRvalueKindV1::Discriminant(option_place),
                        ),
                    ),
                ))],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: constant(1),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
                        )],
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(62, vec![], SemanticTerminatorKindV1::Return),
            block(63, vec![], SemanticTerminatorKindV1::Return),
        ]);
        let producer =
            SemanticOptionProducerV1::new(option_local, SemanticBlockIdV1::from_index(1));
        let error = SemanticOptionDominanceV1::analyze(&function, &[producer]).unwrap_err();

        assert_eq!(
            error.detail(),
            "an Option capability switch is not bound to its unique discriminator"
        );
    }

    #[test]
    fn alternate_predecessor_cannot_enter_an_authenticated_some_target() {
        let option_local = SemanticLocalIdV1::from_index(3);
        let discriminator_local = SemanticLocalIdV1::from_index(2);
        let option_place = SemanticPlaceV1::new(option_local, vec![], POINTER_TYPE).unwrap();
        let discriminator_place =
            SemanticPlaceV1::new(discriminator_local, vec![], SCALAR_TYPE).unwrap();
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                option_place.clone(),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let function = projection_function(vec![
            block(70, vec![], SemanticTerminatorKindV1::Call(call)),
            block(
                71,
                vec![statement(SemanticStatementKindV1::Assign(
                    SemanticAssignmentV1::new(
                        discriminator_place.clone(),
                        SemanticRvalueV1::new(
                            SCALAR_TYPE,
                            SemanticRvalueKindV1::Discriminant(option_place),
                        ),
                    ),
                ))],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(discriminator_place),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
                        )],
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(72, vec![], SemanticTerminatorKindV1::Return),
            block(
                73,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
            ),
        ]);
        let producer =
            SemanticOptionProducerV1::new(option_local, SemanticBlockIdV1::from_index(1));
        let error = SemanticOptionDominanceV1::analyze(&function, &[producer]).unwrap_err();

        assert_eq!(
            error.detail(),
            "an Option capability Some target is not uniquely controlled by its exact branch"
        );
    }

    #[test]
    fn option_payload_availability_excludes_the_none_merge() {
        let option_local = SemanticLocalIdV1::from_index(3);
        let discriminator_local = SemanticLocalIdV1::from_index(2);
        let option_place = SemanticPlaceV1::new(option_local, vec![], POINTER_TYPE).unwrap();
        let discriminator_place =
            SemanticPlaceV1::new(discriminator_local, vec![], SCALAR_TYPE).unwrap();
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                option_place.clone(),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let function = projection_function(vec![
            block(40, vec![], SemanticTerminatorKindV1::Call(call.clone())),
            block(
                41,
                vec![statement(SemanticStatementKindV1::Assign(
                    SemanticAssignmentV1::new(
                        discriminator_place.clone(),
                        SemanticRvalueV1::new(
                            SCALAR_TYPE,
                            SemanticRvalueKindV1::Discriminant(option_place),
                        ),
                    ),
                ))],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(discriminator_place),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![
                            SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
                            ),
                            SemanticSwitchTargetV1::new(
                                1,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 2),
                            ),
                        ],
                        cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                    )
                    .unwrap(),
                },
            ),
            block(
                42,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
            ),
            block(
                43,
                vec![],
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
            ),
            block(44, vec![], SemanticTerminatorKindV1::Return),
        ]);
        let producer =
            SemanticOptionProducerV1::new(option_local, SemanticBlockIdV1::from_index(1));
        let dominance = SemanticOptionDominanceV1::analyze(&function, &[producer]).unwrap();
        let authority = dominance.availability(option_local).unwrap();

        assert!(dominance.allows(authority, SemanticBlockIdV1::from_index(2)));
        assert!(!dominance.allows(authority, SemanticBlockIdV1::from_index(1)));
        assert!(!dominance.allows(authority, SemanticBlockIdV1::from_index(3)));
        assert!(
            !dominance.allows(authority, SemanticBlockIdV1::from_index(4)),
            "the merge is reachable from None and must not inherit payload authority",
        );
    }

    #[test]
    fn capability_alias_worklist_processes_each_charged_edge_once() {
        let mut edges = vec![Vec::new(); 4];
        let mut charged = 0;
        for source in 0..3 {
            push_capability_edge(
                &mut edges,
                &mut charged,
                source,
                CapabilityEdgeV1 {
                    destination: source + 1,
                    use_block: 0,
                    kind: CapabilityEdgeKindV1::Alias,
                },
            )
            .unwrap();
        }
        let seed = ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Argument(0),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        };
        let mut values = vec![None; 4];
        let grid = vec![None; 4];
        let mut worklist = VecDeque::from([0]);
        values[0] = Some(seed);
        let mut processed = 0;
        while let Some(source) = worklist.pop_front() {
            for edge in &edges[source] {
                processed += 1;
                assert_eq!(edge.kind, CapabilityEdgeKindV1::Alias);
                assign_index_capability(
                    edge.destination,
                    values[source].unwrap(),
                    &mut values,
                    &grid,
                    &mut worklist,
                )
                .unwrap();
            }
        }

        assert_eq!(charged, 3);
        assert_eq!(processed, charged);
        assert!(values.iter().all(|value| *value == Some(seed)));
    }

    #[test]
    fn capability_alias_cycle_terminates_with_exact_edge_charge() {
        let mut edges = vec![Vec::new(); 2];
        let mut charged = 0;
        for (source, destination) in [(0, 1), (1, 0)] {
            push_capability_edge(
                &mut edges,
                &mut charged,
                source,
                CapabilityEdgeV1 {
                    destination,
                    use_block: 0,
                    kind: CapabilityEdgeKindV1::Alias,
                },
            )
            .unwrap();
        }
        let seed = ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Argument(0),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        };
        let mut values = vec![Some(seed), None];
        let grid = vec![None; 2];
        let mut worklist = VecDeque::from([0]);
        let mut processed = 0;
        while let Some(source) = worklist.pop_front() {
            for edge in &edges[source] {
                processed += 1;
                assign_index_capability(
                    edge.destination,
                    values[source].unwrap(),
                    &mut values,
                    &grid,
                    &mut worklist,
                )
                .unwrap();
            }
        }

        assert_eq!(processed, charged);
        assert_eq!(values, vec![Some(seed), Some(seed)]);
    }
    #[test]
    fn conflicting_capability_def_use_paths_fail_closed() {
        let first = ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Argument(0),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        };
        let second = ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Argument(1),
            ..first
        };
        let mut values = vec![None, Some(first)];
        let grid = vec![None; 2];
        let mut worklist = VecDeque::new();

        assert!(matches!(
            assign_index_capability(1, second, &mut values, &grid, &mut worklist),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "multiple index capabilities reach one semantic local"
            ))
        ));
    }

    #[test]
    fn rust_pointee_kinds_define_conservative_alias_classes() {
        let shared = allocation_contract_from_pointee(
            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
            true,
            2,
        );
        let shared_interior_mutable = allocation_contract_from_pointee(
            SemanticAbiPointeeKindV1::SharedReference { frozen: false },
            false,
            5,
        );
        let unique = allocation_contract_from_pointee(
            SemanticAbiPointeeKindV1::MutableReference { unpin: true },
            true,
            3,
        );
        let unqualified = allocation_contract_from_pointee(
            SemanticAbiPointeeKindV1::MutableReference { unpin: false },
            false,
            4,
        );

        assert_eq!(shared.noalias_class, 1);
        assert!(!shared.writable);
        assert_eq!(shared_interior_mutable.noalias_class, 1);
        assert!(shared_interior_mutable.writable);
        assert_eq!(unique.noalias_class, 4);
        assert!(unique.writable);
        assert_eq!(unqualified.noalias_class, 0);
        assert!(unqualified.writable);
    }

    #[test]
    fn authenticated_source_ownership_distinguishes_borrows_owners_and_raw_pointers() {
        let raw = allocation_contract_from_pointee(SemanticAbiPointeeKindV1::Raw, false, 3);
        let exclusive = authenticated_source_allocation_contract_v1(
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            SemanticAbiPointeeKindV1::Raw,
            raw,
        )
        .unwrap();
        assert_eq!(exclusive.allocation_origin, 3);
        assert_eq!(exclusive.noalias_class, 4);
        assert!(exclusive.writable);

        for ownership in [
            SemanticSourceArgumentOwnershipV1::RawPointer,
            SemanticSourceArgumentOwnershipV1::ByValue,
        ] {
            assert_eq!(
                authenticated_source_allocation_contract_v1(
                    ownership,
                    SemanticAbiPointeeKindV1::Raw,
                    raw,
                )
                .unwrap()
                .noalias_class,
                0
            );
        }
    }

    #[test]
    fn authenticated_source_ownership_mismatches_fail_closed() {
        let shared = allocation_contract_from_pointee(
            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
            true,
            2,
        );
        let raw = allocation_contract_from_pointee(SemanticAbiPointeeKindV1::Raw, false, 3);
        for (ownership, pointee, contract) in [
            (
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
                SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                shared,
            ),
            (
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticAbiPointeeKindV1::Raw,
                raw,
            ),
            (
                SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                SemanticAbiPointeeKindV1::Raw,
                raw,
            ),
            (
                SemanticSourceArgumentOwnershipV1::Unspecified,
                SemanticAbiPointeeKindV1::Raw,
                raw,
            ),
            (
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                shared,
            ),
            (
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                allocation_contract_from_pointee(
                    SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                    true,
                    4,
                ),
            ),
            (
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticAbiPointeeKindV1::Box {
                    unpin: true,
                    global: true,
                },
                allocation_contract_from_pointee(
                    SemanticAbiPointeeKindV1::Box {
                        unpin: true,
                        global: true,
                    },
                    true,
                    5,
                ),
            ),
        ] {
            assert!(matches!(
                authenticated_source_allocation_contract_v1(ownership, pointee, contract),
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "source ownership disagrees with rustc ABI pointer provenance"
                ))
            ));
        }
    }

    #[test]
    fn source_execution_layout_derives_active_grid_axes_from_xyz_workgroup() {
        for (rank, workgroup, global_extents) in [
            (1, [128, 1, 1], [0, 1, 1]),
            (2, [64, 1, 1], [0, 0, 1]),
            (2, [8, 8, 1], [0, 0, 1]),
            (3, [64, 1, 1], [0, 0, 0]),
            (3, [4, 4, 4], [0, 0, 0]),
        ] {
            let dimensions = SemanticWorkgroupDimensionsV1::new(workgroup).unwrap();
            let launch =
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap();
            let source_contract =
                SemanticKernelSourceContractV1::new(Some(launch), None, None).unwrap();
            let function =
                projection_function(vec![block(30, vec![], SemanticTerminatorKindV1::Return)])
                    .with_kernel_entry(SemanticKernelEntryV1::new(
                        SemanticLinkSymbolV1::new(b"typed_kernel".to_vec()).unwrap(),
                        SemanticKernelBindingIdentityV1::from_sha256(bytes(42)),
                        source_contract,
                    ));

            assert_eq!(
                source_execution_layout_v1(
                    SemanticTargetArchitectureV1::AmdGpuGfx942,
                    &function,
                    rank,
                )
                .unwrap(),
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: u64::from_le_bytes([42; 8]),
                    global_extents,
                    workgroup_extents: workgroup.map(u64::from),
                    subgroup_size: 64,
                    full_physical_workgroups: true,
                }
            );
        }
    }

    #[test]
    fn checked_reference_provenance_covers_only_the_exact_pointee() {
        let origins = [None, None, None, Some(7)];
        assert_eq!(
            checked_reference_origin(&dereferenced_place(), &origins),
            Some(7)
        );
        let nested_index = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(3),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ARRAY_TYPE)
                    .unwrap(),
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: 0,
                        minimum_length: 4,
                        from_end: false,
                    },
                    SCALAR_TYPE,
                )
                .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap();
        assert_eq!(checked_reference_origin(&nested_index, &origins), None);

        let function = projection_function(vec![block(
            31,
            vec![
                statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), vec![], POINTER_TYPE)
                        .unwrap(),
                    SemanticRvalueV1::new(
                        POINTER_TYPE,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                            SemanticPlaceV1::new(
                                SemanticLocalIdV1::from_index(3),
                                vec![],
                                POINTER_TYPE,
                            )
                            .unwrap(),
                        )),
                    ),
                ))),
                statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    dereferenced_place(),
                    SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(1))),
                ))),
            ],
            SemanticTerminatorKindV1::Return,
        )]);
        assert_eq!(local_definition_counts(&function)[3], 1);
    }

    #[test]
    fn atomic_rmw_projects_result_one_atomic_address_effect_and_value() {
        let (operations, sources, _) = audit_statements(vec![statement(
            SemanticStatementKindV1::AtomicRmw(SemanticAtomicRmwV1::new(
                scalar_place(),
                ranked_place(0),
                SemanticOperandV1::Copy(ranked_place(1)),
                SemanticAtomicRmwOpV1::Add,
                atomic_access(),
            )),
        )])
        .unwrap();
        assert_eq!(
            access_kinds(&operations),
            vec![AccessKindAttr::AtomicReadModifyWrite, AccessKindAttr::Read,]
        );
        assert_eq!(sources.len(), 2);
        assert!(operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::AtomicAccess {
                kind: AccessKindAttr::AtomicReadModifyWrite,
                ordering: AtomicOrderingAttr::Relaxed,
                scope: AtomicScopeAttr::Agent,
                ..
            }
        )));
        assert!(!operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::Access {
                kind: AccessKindAttr::AtomicRead
                    | AccessKindAttr::AtomicWrite
                    | AccessKindAttr::AtomicReadModifyWrite,
                ..
            }
        )));
    }

    #[test]
    fn atomic_compare_exchange_projects_both_candidates_and_address_effects() {
        let (operations, sources, _) = audit_statements(vec![statement(
            SemanticStatementKindV1::AtomicCompareExchange(SemanticAtomicCompareExchangeV1::new(
                scalar_place(),
                ranked_place(0),
                SemanticOperandV1::Copy(ranked_place(1)),
                SemanticOperandV1::Move(ranked_place(2)),
                atomic_access(),
                SemanticAtomicOrderingV1::Relaxed,
                false,
            )),
        )])
        .unwrap();
        assert_eq!(
            access_kinds(&operations),
            vec![
                AccessKindAttr::AtomicReadModifyWrite,
                AccessKindAttr::Read,
                AccessKindAttr::Read,
            ]
        );
        assert_eq!(sources.len(), 3);
    }

    #[test]
    fn discriminant_and_deinitialize_places_are_not_silently_skipped() {
        let discriminant_read = SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            scalar_place(),
            SemanticRvalueV1::new(
                SCALAR_TYPE,
                SemanticRvalueKindV1::Discriminant(ranked_place(0)),
            ),
        ));
        let (operations, sources, _) = audit_statements(vec![
            statement(SemanticStatementKindV1::SetDiscriminant {
                place: ranked_place(1),
                variant_index: 0,
            }),
            statement(SemanticStatementKindV1::Deinitialize(ranked_place(2))),
            statement(discriminant_read),
        ])
        .unwrap();
        assert_eq!(
            access_kinds(&operations),
            vec![
                AccessKindAttr::Write,
                AccessKindAttr::Write,
                AccessKindAttr::Read,
            ]
        );
        assert_eq!(sources.len(), 3);
    }

    #[test]
    fn storage_markers_and_nop_are_explicit_zero_effect_statements() {
        let (operations, sources, _) = audit_statements(vec![
            statement(SemanticStatementKindV1::StorageLive(
                SemanticLocalIdV1::from_index(2),
            )),
            statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(2),
            )),
            statement(SemanticStatementKindV1::Nop),
        ])
        .unwrap();
        assert!(operations.is_empty());
        assert!(sources.is_empty());

        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::StorageLive(
                SemanticLocalIdV1::from_index(99),
            ))]),
            "a storage statement with an out-of-range local",
        );
    }

    #[test]
    fn explicit_or_dereferenced_unranked_memory_fails_closed() {
        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::Store(
                SemanticMemoryStoreV1::new(
                    scalar_place(),
                    constant(7),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                ),
            ))]),
            "an explicit memory operation without a ranked index projection",
        );

        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::Assign(
                SemanticAssignmentV1::new(
                    scalar_place(),
                    SemanticRvalueV1::new(
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                            dereferenced_place(),
                            SemanticVolatilityV1::Volatile,
                            None,
                        )),
                    ),
                ),
            ))]),
            "a dereferenced memory access without a ranked index projection",
        );
    }

    #[test]
    fn unsupported_place_forms_fail_before_a_clean_result() {
        let hostile = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Subslice {
                        from: 0,
                        to: 1,
                        from_end: false,
                    },
                    ARRAY_TYPE,
                )
                .unwrap(),
            ],
            ARRAY_TYPE,
        )
        .unwrap();
        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::Deinitialize(
                hostile,
            ))]),
            "an indexed place containing a subslice projection",
        );

        let dynamic = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                    SCALAR_TYPE,
                )
                .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap();
        assert_unsupported(
            audit_statements(vec![statement(SemanticStatementKindV1::Deinitialize(
                dynamic,
            ))]),
            "a dynamic array index before exact static-extent guard projection",
        );
    }

    #[test]
    fn unreachable_blocks_are_still_audited_for_memory_effects() {
        let function = projection_function(vec![
            block(40, vec![], SemanticTerminatorKindV1::Return),
            block(
                41,
                vec![statement(SemanticStatementKindV1::Store(
                    SemanticMemoryStoreV1::new(
                        scalar_place(),
                        constant(1),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    ),
                ))],
                SemanticTerminatorKindV1::Unreachable,
            ),
        ]);
        assert_unsupported(
            audit_function(&function),
            "an explicit memory operation without a ranked index projection",
        );
    }

    #[test]
    fn unresolved_call_and_drop_effects_fail_before_a_clean_result() {
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        assert_unsupported(
            audit_function(&projection_function(vec![block(
                42,
                vec![],
                SemanticTerminatorKindV1::Call(call),
            )])),
            "a call terminator before exact callable memory-effect summaries are available",
        );

        let edge = SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::DropReturn,
            SemanticBlockIdV1::from_index(0),
        );
        assert_unsupported(
            audit_function(&projection_function(vec![block(
                43,
                vec![],
                SemanticTerminatorKindV1::Drop {
                    place: scalar_place(),
                    drop_glue: SemanticFunctionIdV1::from_index(0),
                    target: edge,
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            )])),
            "a drop terminator before exact drop-glue memory-effect summaries are available",
        );
    }

    #[test]
    fn statement_projection_stops_at_the_ranked_operation_bound() {
        let function =
            projection_function(vec![block(50, vec![], SemanticTerminatorKindV1::Return)]);
        let types = projection_types();
        let semantic_statement =
            statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                ranked_place(0),
                constant(1),
                SemanticVolatilityV1::NonVolatile,
                None,
            )));
        let mut operations = vec![
            ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(0),
                value: 0,
            };
            MAX_PROJECTED_OPERATIONS_V1 - 2
        ];
        let original = operations.len();
        let mut sources = Vec::new();
        let mut projected_views = vec![None; function.locals().len()];
        let mut guarded_sites = Vec::new();
        let mut next_value = 0;
        let mut ranked_ir = String::new();
        let local_contracts = synthetic_local_contracts(&function);
        let error = project_statement_accesses(
            &types,
            &function,
            0,
            &[],
            &semantic_statement,
            &[None; 4],
            &local_contracts,
            &[],
            &mut guarded_sites,
            &mut projected_views,
            &mut operations,
            &mut sources,
            &mut next_value,
            &mut ranked_ir,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionRankedProjectionErrorV1::Unsupported(
                "a semantic statement projection exceeding the ranked operation limit"
            )
        ));
        assert_eq!(operations.len(), original);
        assert!(sources.is_empty());
        assert!(ranked_ir.is_empty());
    }

    #[test]
    fn constant_aliases_resolve_once_in_linear_time() {
        let definitions = [
            ConstantDefinitionV1::Direct(64),
            ConstantDefinitionV1::Alias(SemanticLocalIdV1::from_index(0)),
            ConstantDefinitionV1::Alias(SemanticLocalIdV1::from_index(1)),
        ];
        let mut states = [0; 3];
        let mut values = [None; 3];
        assert_eq!(
            resolve_constant(2, &definitions, &mut states, &mut values),
            Some(64),
        );
        assert_eq!(values, [Some(64); 3]);
        assert_eq!(states, [2; 3]);
    }

    #[test]
    fn cyclic_or_multiply_defined_indices_are_not_constants() {
        let cycle = [
            ConstantDefinitionV1::Alias(SemanticLocalIdV1::from_index(1)),
            ConstantDefinitionV1::Alias(SemanticLocalIdV1::from_index(0)),
        ];
        let mut states = [0; 2];
        let mut values = [None; 2];
        assert_eq!(resolve_constant(0, &cycle, &mut states, &mut values), None,);

        let mut definitions = [ConstantDefinitionV1::Missing];
        record_constant_definition(
            &mut definitions,
            SemanticLocalIdV1::from_index(0),
            ConstantDefinitionV1::Direct(63),
        );
        record_constant_definition(
            &mut definitions,
            SemanticLocalIdV1::from_index(0),
            ConstantDefinitionV1::Direct(64),
        );
        assert!(matches!(definitions[0], ConstantDefinitionV1::Invalid));
    }

    #[test]
    fn source_label_is_explicit_when_unavailable() {
        assert_eq!(
            source_label(SemanticSourceProvenanceV1::unavailable()),
            "Rust source location unavailable",
        );
    }

    fn tensor_operand(local: u32) -> SemanticOperandV1 {
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], SCALAR_TYPE)
                .unwrap(),
        )
    }

    fn tensor_payload(carrier: u32, variant: u32) -> SemanticOperandV1 {
        SemanticOperandV1::Move(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(carrier),
                vec![
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Downcast(variant),
                        ENUM_TYPE,
                    )
                    .unwrap(),
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), SCALAR_TYPE)
                        .unwrap(),
                ],
                SCALAR_TYPE,
            )
            .unwrap(),
        )
    }

    fn tensor_test_call() -> SemanticDirectCallV1 {
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![
                tensor_operand(0),
                tensor_operand(1),
                tensor_operand(2),
                tensor_operand(3),
            ],
            Some(SemanticCallDestinationV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(4), vec![], SCALAR_TYPE)
                    .unwrap(),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 0),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap()
    }

    fn mfma_operand_contract(role: SemanticMfmaOperandRoleV1) -> SemanticMfmaOperandContractV1 {
        SemanticMfmaOperandContractV1 {
            role,
            profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
            register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
            wave_width: 64,
        }
    }

    fn mfma_accumulator_contract() -> SemanticMfmaAccumulatorContractV1 {
        SemanticMfmaAccumulatorContractV1 {
            profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
            distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
            wave_width: 64,
        }
    }

    fn tensor_test_allocation() -> AllocationContractV1 {
        AllocationContractV1 {
            allocation_origin: 1,
            noalias_class: 1,
            writable: false,
        }
    }

    fn zero_filled_tensor_load_callable() -> SemanticCallableDeclV1 {
        compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 {
                fragment: SCALAR_TYPE,
                view: SCALAR_TYPE,
                lane: SCALAR_TYPE,
                contract: mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
                storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            },
        )
    }

    fn legacy_tensor_load_callable() -> SemanticCallableDeclV1 {
        compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad {
            option_fragment: ENUM_TYPE,
            fragment: SCALAR_TYPE,
            view: SCALAR_TYPE,
            lane: SCALAR_TYPE,
            contract: mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
            storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
        })
    }

    fn tensor_load_function(destination: Option<SemanticPlaceV1>) -> SemanticFunctionDeclV1 {
        let destination = destination.map(|place| {
            SemanticCallDestinationV1::new(place, cfg_edge(SemanticEdgeRoleV1::CallReturn, 0))
        });
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![
                tensor_operand(0),
                tensor_operand(1),
                tensor_operand(2),
                tensor_operand(3),
            ],
            destination,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        projection_function_with_locals(
            vec![block(133, vec![], SemanticTerminatorKindV1::Call(call))],
            vec![
                local(133, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(134, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(135, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(136, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(137, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    fn authenticated_tensor_load_state() -> ProjectedTensorStateV1 {
        HashMap::from([
            (
                0,
                ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::View(ProjectedMfmaViewV1 {
                    role: SemanticMfmaOperandRoleV1::A,
                    storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                    allocation: tensor_test_allocation(),
                })),
            ),
            (
                1,
                ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Lane {
                    root: 20,
                    wave_width: 64,
                }),
            ),
        ])
    }

    fn strided_read_view() -> ProjectedReadViewV1 {
        ProjectedReadViewV1 {
            root: 41,
            element: SCALAR_TYPE,
            allocation: AllocationContractV1 {
                allocation_origin: 3,
                noalias_class: 1,
                writable: false,
            },
            rows: ProjectedReadValueV1::Constant(7),
            columns: ProjectedReadValueV1::Local(SemanticLocalIdV1::from_index(2)),
        }
    }

    fn strided_read_call_function(destination: Option<SemanticPlaceV1>) -> SemanticFunctionDeclV1 {
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![
                tensor_operand(0),
                tensor_operand(1),
                tensor_operand(2),
                tensor_operand(3),
            ],
            destination.map(|place| {
                SemanticCallDestinationV1::new(place, cfg_edge(SemanticEdgeRoleV1::CallReturn, 0))
            }),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        projection_function_with_locals(
            vec![block(132, vec![], SemanticTerminatorKindV1::Call(call))],
            vec![
                local(132, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(133, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(134, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(135, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(136, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    fn strided_read_callable() -> SemanticCallableDeclV1 {
        compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::StridedReadView2DLoadOr {
                view: SCALAR_TYPE,
                element: SCALAR_TYPE,
            },
        )
    }

    #[test]
    fn strided_read_requires_exact_dominating_view_and_records_discarded_loads() {
        let function = strided_read_call_function(None);
        let view = strided_read_view();
        let mut state = HashMap::from([(
            0,
            ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::ReadView(view)),
        )]);
        let effects = transfer_tensor_terminator_v1(
            &[strided_read_callable()],
            &function,
            0,
            &mut state,
            &[None; 5],
            &[None; 5],
            true,
        )
        .unwrap();
        assert_eq!(
            effects.read_view,
            Some(ProjectedReadViewAccessV1 {
                view,
                row: ProjectedReadValueV1::Local(SemanticLocalIdV1::from_index(1)),
                column: ProjectedReadValueV1::Local(SemanticLocalIdV1::from_index(2)),
            })
        );
        assert_eq!(
            state.get(&0),
            Some(&ProjectedTensorValueV1::Known(
                ProjectedTensorOriginV1::ReadView(view)
            ))
        );

        let mut invalid = HashMap::from([(0, ProjectedTensorValueV1::Invalid)]);
        assert!(matches!(
            transfer_tensor_terminator_v1(
                &[strided_read_callable()],
                &function,
                0,
                &mut invalid,
                &[None; 5],
                &[None; 5],
                true,
            ),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a strided read without one dominating checked view payload and exact scalar operands"
            ))
        ));
    }

    #[test]
    fn strided_read_projects_rank_two_guarded_access_without_fabricated_indices() {
        let function =
            projection_function(vec![block(131, vec![], SemanticTerminatorKindV1::Return)]);
        let effect = ProjectedReadViewAccessV1 {
            view: strided_read_view(),
            row: ProjectedReadValueV1::Local(SemanticLocalIdV1::from_index(1)),
            column: ProjectedReadValueV1::Constant(4),
        };
        let mut arguments = vec![None; function.locals().len()];
        let mut next_argument = 1;
        let mut operations = Vec::new();
        let mut next_value = 0;
        let projected = project_strided_read_effects_v1(
            &projection_types(),
            &function,
            &[Some(effect)],
            &vec![None; function.locals().len()],
            &mut arguments,
            &mut next_argument,
            &mut operations,
            &mut next_value,
        )
        .unwrap();
        assert!(matches!(
            operations.as_slice(),
            [
                ProductionRankedOperationV1::IndexConstant { value: 7, .. },
                ProductionRankedOperationV1::ViewInSpace {
                    writable: false,
                    shape,
                    memory_space: MemorySpaceAttr::Global,
                    allocation_origin: 3,
                    ..
                },
                ProductionRankedOperationV1::IndexConstant { value: 4, .. }
            ] if shape == &[DYNAMIC_EXTENT, DYNAMIC_EXTENT]
        ));
        let access = projected[0].as_ref().unwrap();
        assert_eq!(access.access, AccessKindAttr::Read);
        assert_eq!(access.indices.len(), 2);
        assert_eq!(access.comparisons.len(), 2);
    }

    #[test]
    fn every_authenticated_global_fragment_load_emits_a_read_even_when_unused() {
        let function = tensor_load_function(Some(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(4), vec![], SCALAR_TYPE).unwrap(),
        ));
        let mut state = authenticated_tensor_load_state();
        let effects = transfer_tensor_terminator_v1(
            &[zero_filled_tensor_load_callable()],
            &function,
            0,
            &mut state,
            &[None; 5],
            &[],
            true,
        )
        .unwrap();

        assert_eq!(effects.global_read, Some(tensor_test_allocation()));
        assert!(effects.layout.is_none());
        assert!(matches!(
            state.get(&4),
            Some(ProjectedTensorValueV1::Known(
                ProjectedTensorOriginV1::Operand(_)
            ))
        ));
    }

    #[test]
    fn operand_a_and_b_reads_remain_distinct_effects_in_their_mir_call_blocks() {
        let function = projection_function(vec![
            block(138, vec![], SemanticTerminatorKindV1::Return),
            block(139, vec![], SemanticTerminatorKindV1::Return),
            block(140, vec![], SemanticTerminatorKindV1::Return),
        ]);
        let operand_a = tensor_test_allocation();
        let operand_b = AllocationContractV1 {
            allocation_origin: 2,
            noalias_class: 1,
            writable: false,
        };
        let effects = bind_tensor_read_effects_to_call_blocks_v1(
            &function,
            &[Some(operand_a), None, Some(operand_b)],
        )
        .unwrap();

        assert_eq!(effects.len(), 3);
        assert_eq!(effects[0].map(|effect| effect.allocation), Some(operand_a));
        assert_eq!(effects[1], None);
        assert_eq!(effects[2].map(|effect| effect.allocation), Some(operand_b));
        assert_ne!(
            effects[0].unwrap().allocation.allocation_origin,
            effects[2].unwrap().allocation.allocation_origin
        );
        assert!(bind_tensor_read_effects_to_call_blocks_v1(&function, &[Some(operand_a)]).is_err());
    }

    #[test]
    fn global_fragment_loads_fail_closed_before_later_mfma_consumption() {
        let direct_destination =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(4), vec![], SCALAR_TYPE).unwrap();
        let function = tensor_load_function(Some(direct_destination));
        let mut merged_state = authenticated_tensor_load_state();
        assert!(merge_tensor_states_v1(&mut merged_state, &HashMap::new()).unwrap());
        let error = transfer_tensor_terminator_v1(
            &[zero_filled_tensor_load_callable()],
            &function,
            0,
            &mut merged_state,
            &[None; 5],
            &[],
            true,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionRankedProjectionErrorV1::Incomplete(
                "a typed global fragment load without exact authenticated view, lane, allocation, and result provenance"
            )
        ));

        let mut invalid_lane = authenticated_tensor_load_state();
        invalid_lane.insert(1, ProjectedTensorValueV1::Invalid);
        assert!(matches!(
            transfer_tensor_terminator_v1(
                &[zero_filled_tensor_load_callable()],
                &function,
                0,
                &mut invalid_lane,
                &[None; 5],
                &[],
                false,
            ),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a typed global fragment load without exact authenticated view, lane, allocation, and result provenance"
            ))
        ));
    }

    #[test]
    fn global_fragment_loads_cannot_discard_or_project_their_result() {
        let function = tensor_load_function(None);
        assert!(matches!(
            transfer_tensor_terminator_v1(
                &[zero_filled_tensor_load_callable()],
                &function,
                0,
                &mut authenticated_tensor_load_state(),
                &[None; 5],
                &[],
                true,
            ),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a typed global fragment load without one direct local result"
            ))
        ));

        let projected = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), SCALAR_TYPE).unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap();
        let function = tensor_load_function(Some(projected));
        assert!(matches!(
            transfer_tensor_terminator_v1(
                &[zero_filled_tensor_load_callable()],
                &function,
                0,
                &mut authenticated_tensor_load_state(),
                &[None; 5],
                &[],
                true,
            ),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a typed global fragment load into a projected destination"
            ))
        ));
    }

    #[test]
    fn zero_filled_v2_load_is_a_direct_authenticated_operand() {
        let call = tensor_test_call();
        let contract = mfma_operand_contract(SemanticMfmaOperandRoleV1::A);
        let state = HashMap::from([
            (
                0,
                ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::View(ProjectedMfmaViewV1 {
                    role: SemanticMfmaOperandRoleV1::A,
                    storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                    allocation: tensor_test_allocation(),
                })),
            ),
            (
                1,
                ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Lane {
                    root: 20,
                    wave_width: 64,
                }),
            ),
        ]);

        assert!(matches!(
            project_tensor_load_origin_v1(
                &call,
                &state,
                SCALAR_TYPE,
                SCALAR_TYPE,
                contract,
                SemanticMfmaStorageLayoutV1::RowMajor,
            ),
            ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Operand(_))
        ));
        assert_eq!(
            project_tensor_load_origin_v1(
                &call,
                &state,
                ARRAY_TYPE,
                SCALAR_TYPE,
                contract,
                SemanticMfmaStorageLayoutV1::RowMajor,
            ),
            ProjectedTensorValueV1::Invalid
        );
        assert_eq!(
            project_tensor_load_origin_v1(
                &call,
                &state,
                SCALAR_TYPE,
                SCALAR_TYPE,
                mfma_operand_contract(SemanticMfmaOperandRoleV1::B),
                SemanticMfmaStorageLayoutV1::RowMajor,
            ),
            ProjectedTensorValueV1::Invalid
        );
    }

    #[test]
    fn production_ranked_projection_rejects_the_retired_option_load_before_analysis() {
        assert!(matches!(
            reject_retired_production_intrinsics_v1(&[legacy_tensor_load_callable()]),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "the retired Option-returning BF16 matrix load; use Bf16MatrixLoadZeroFilledV2"
            ))
        ));
        assert!(
            reject_retired_production_intrinsics_v1(&[zero_filled_tensor_load_callable()]).is_ok()
        );

        let function = tensor_load_function(None);
        assert!(matches!(
            transfer_tensor_terminator_v1(
                &[legacy_tensor_load_callable()],
                &function,
                0,
                &mut authenticated_tensor_load_state(),
                &[None; 5],
                &[],
                false,
            ),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "the retired Option-returning BF16 matrix load; use Bf16MatrixLoadZeroFilledV2"
            ))
        ));
    }

    fn authenticated_tensor_state(
        lhs_storage: SemanticMfmaStorageLayoutV1,
        rhs_storage: SemanticMfmaStorageLayoutV1,
    ) -> ProjectedTensorStateV1 {
        HashMap::from([
            (
                0,
                ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::MatrixContext { root: 10 }),
            ),
            (
                1,
                ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Operand(
                    ProjectedMfmaOperandV1 {
                        contract: mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
                        storage_layout: lhs_storage,
                        lane_root: 20,
                        allocation: tensor_test_allocation(),
                    },
                )),
            ),
            (
                2,
                ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Operand(
                    ProjectedMfmaOperandV1 {
                        contract: mfma_operand_contract(SemanticMfmaOperandRoleV1::B),
                        storage_layout: rhs_storage,
                        lane_root: 20,
                        allocation: tensor_test_allocation(),
                    },
                )),
            ),
            (
                3,
                ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Accumulator(
                    ProjectedMfmaAccumulatorV1 {
                        contract: mfma_accumulator_contract(),
                        lane_root: 20,
                    },
                )),
            ),
        ])
    }

    #[test]
    fn authenticated_mfma_producers_derive_independent_storage_and_zero_fill() {
        let call = tensor_test_call();
        let state = authenticated_tensor_state(
            SemanticMfmaStorageLayoutV1::LdsXor4,
            SemanticMfmaStorageLayoutV1::RowMajor,
        );
        let (_, contract) = authenticate_tensor_instruction_v1(
            &call,
            &state,
            mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
            mfma_operand_contract(SemanticMfmaOperandRoleV1::B),
            mfma_accumulator_contract(),
        )
        .unwrap();

        assert_eq!(
            contract.a.lds_swizzle,
            fe2o3_kernel_ir::TensorLdsSwizzleV1::Xor4
        );
        assert_eq!(
            contract.b.lds_swizzle,
            fe2o3_kernel_ir::TensorLdsSwizzleV1::None
        );
        assert_eq!(
            contract.tail_mask,
            fe2o3_kernel_ir::TensorTailMaskV1::ZeroFilledPredicateInputs
        );
    }

    #[test]
    fn swapped_missing_and_cross_lane_mfma_producers_fail_closed() {
        let call = tensor_test_call();
        let mut state = authenticated_tensor_state(
            SemanticMfmaStorageLayoutV1::RowMajor,
            SemanticMfmaStorageLayoutV1::RowMajor,
        );
        assert!(
            authenticate_tensor_instruction_v1(
                &call,
                &state,
                mfma_operand_contract(SemanticMfmaOperandRoleV1::B),
                mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
                mfma_accumulator_contract(),
            )
            .unwrap_err()
            .contains("metadata")
        );

        state.remove(&1);
        assert!(
            authenticate_tensor_instruction_v1(
                &call,
                &state,
                mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
                mfma_operand_contract(SemanticMfmaOperandRoleV1::B),
                mfma_accumulator_contract(),
            )
            .unwrap_err()
            .contains("lhs")
        );

        let mut state = authenticated_tensor_state(
            SemanticMfmaStorageLayoutV1::RowMajor,
            SemanticMfmaStorageLayoutV1::RowMajor,
        );
        let ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Operand(rhs)) = state[&2] else {
            unreachable!()
        };
        state.insert(
            2,
            ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Operand(
                ProjectedMfmaOperandV1 {
                    lane_root: 21,
                    ..rhs
                },
            )),
        );
        assert!(
            authenticate_tensor_instruction_v1(
                &call,
                &state,
                mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
                mfma_operand_contract(SemanticMfmaOperandRoleV1::B),
                mfma_accumulator_contract(),
            )
            .unwrap_err()
            .contains("authenticated wave64 lane")
        );
    }

    #[test]
    fn result_ok_payloads_require_their_exact_dominating_edges() {
        let carrier = SemanticLocalIdV1::from_index(1);
        let discriminator = SemanticLocalIdV1::from_index(2);
        let discriminator_place = SemanticPlaceV1::new(discriminator, vec![], SCALAR_TYPE).unwrap();
        let result_function = projection_function_with_locals(
            vec![
                block(
                    90,
                    vec![
                        enum_definition(carrier, 0),
                        enum_discriminant(carrier, discriminator),
                    ],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(discriminator_place),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                block(91, vec![], SemanticTerminatorKindV1::Return),
                block(92, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(90, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(91, ENUM_TYPE, SemanticLocalRoleV1::Temporary),
                local(92, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        let result_dominance = SemanticEnumPayloadDominanceV1::analyze(
            &result_function,
            &projection_types_with_enum(),
        )
        .unwrap();
        let result_state = HashMap::from([(
            1,
            ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::ViewResult(
                ProjectedMfmaViewV1 {
                    role: SemanticMfmaOperandRoleV1::A,
                    storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                    allocation: tensor_test_allocation(),
                },
            )),
        )]);
        assert!(matches!(
            tensor_origin_from_assignment_operand_v1(
                &tensor_payload(1, 0),
                &result_state,
                &result_dominance,
                SemanticBlockIdV1::from_index(1),
            ),
            Some(ProjectedTensorValueV1::Known(
                ProjectedTensorOriginV1::View(_)
            ))
        ));
        assert!(
            tensor_origin_from_assignment_operand_v1(
                &tensor_payload(1, 0),
                &result_state,
                &result_dominance,
                SemanticBlockIdV1::from_index(2),
            )
            .is_none()
        );
    }

    #[test]
    fn exact_enum_transport_preserves_tensor_origin_through_nested_wrappers() {
        let function =
            projection_function(vec![block(96, vec![], SemanticTerminatorKindV1::Return)]);
        let enum_dominance =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let origin = ProjectedTensorOriginV1::Operand(ProjectedMfmaOperandV1 {
            contract: mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
            storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            lane_root: 20,
            allocation: tensor_test_allocation(),
        });
        let mut state = HashMap::from([(0, ProjectedTensorValueV1::Known(origin))]);
        let first = SemanticAggregateRvalueV1::new(
            SemanticAggregateKindV1::EnumVariant(0),
            vec![tensor_operand(0)],
        )
        .unwrap();
        let first = tensor_origin_from_enum_aggregate_v1(
            &first,
            &state,
            &enum_dominance,
            SemanticBlockIdV1::from_index(0),
        )
        .unwrap()
        .unwrap();
        state.insert(1, first);
        let second = SemanticAggregateRvalueV1::new(
            SemanticAggregateKindV1::EnumVariant(1),
            vec![tensor_operand(1)],
        )
        .unwrap();
        let second = tensor_origin_from_enum_aggregate_v1(
            &second,
            &state,
            &enum_dominance,
            SemanticBlockIdV1::from_index(0),
        )
        .unwrap()
        .unwrap();
        state.insert(2, second);

        let first_again = tensor_origin_from_assignment_operand_v1(
            &tensor_payload(2, 1),
            &state,
            &enum_dominance,
            SemanticBlockIdV1::from_index(0),
        )
        .unwrap();
        assert_eq!(first_again, first);
        state.insert(3, first_again);
        assert_eq!(
            tensor_origin_from_assignment_operand_v1(
                &tensor_payload(3, 0),
                &state,
                &enum_dominance,
                SemanticBlockIdV1::from_index(0),
            ),
            Some(ProjectedTensorValueV1::Known(origin))
        );
    }

    #[test]
    fn enum_transport_rejects_wrong_variant_extra_fields_and_bypass_join() {
        let function =
            projection_function(vec![block(97, vec![], SemanticTerminatorKindV1::Return)]);
        let enum_dominance =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let origin = ProjectedTensorOriginV1::Operand(ProjectedMfmaOperandV1 {
            contract: mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
            storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            lane_root: 20,
            allocation: tensor_test_allocation(),
        });
        let state = HashMap::from([(0, ProjectedTensorValueV1::Known(origin))]);
        let aggregate = SemanticAggregateRvalueV1::new(
            SemanticAggregateKindV1::EnumVariant(4),
            vec![tensor_operand(0)],
        )
        .unwrap();
        let wrapped = tensor_origin_from_enum_aggregate_v1(
            &aggregate,
            &state,
            &enum_dominance,
            SemanticBlockIdV1::from_index(0),
        )
        .unwrap()
        .unwrap();
        let wrapped_state = HashMap::from([(1, wrapped)]);
        assert!(
            tensor_origin_from_assignment_operand_v1(
                &tensor_payload(1, 3),
                &wrapped_state,
                &enum_dominance,
                SemanticBlockIdV1::from_index(0),
            )
            .is_none()
        );

        let extra_fields = SemanticAggregateRvalueV1::new(
            SemanticAggregateKindV1::EnumVariant(4),
            vec![tensor_operand(0), tensor_operand(0)],
        )
        .unwrap();
        assert!(
            tensor_origin_from_enum_aggregate_v1(
                &extra_fields,
                &state,
                &enum_dominance,
                SemanticBlockIdV1::from_index(0),
            )
            .unwrap()
            .is_none()
        );

        let mut joined = wrapped_state;
        assert!(merge_tensor_states_v1(&mut joined, &HashMap::new()).unwrap());
        assert_eq!(joined[&1], ProjectedTensorValueV1::Invalid);
    }

    #[test]
    fn enum_transport_nesting_has_an_explicit_resource_bound() {
        let origin = ProjectedTensorOriginV1::Lane {
            root: 1,
            wave_width: 64,
        };
        let mut value = ProjectedTensorValueV1::Known(origin);
        for variant in 0..MAX_PROJECTED_TENSOR_ENUM_DEPTH_V1 {
            value = wrap_tensor_enum_value_v1(value, variant as u32).unwrap();
        }
        assert!(matches!(
            wrap_tensor_enum_value_v1(value, 99),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "tensor enum transport exceeds the charged nesting limit"
            ))
        ));
    }

    fn tensor_move_operand(local: u32) -> SemanticOperandV1 {
        SemanticOperandV1::Move(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], SCALAR_TYPE)
                .unwrap(),
        )
    }

    fn tensor_state_origin() -> ProjectedTensorValueV1 {
        ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Lane {
            root: 31,
            wave_width: 64,
        })
    }

    #[test]
    fn copy_and_move_transfer_tensor_origin_exactly_once() {
        let place = |local| SemanticPlaceV1::new(local, vec![], SCALAR_TYPE).unwrap();
        let assignment = |destination, operand| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(destination),
                SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(operand)),
            )))
        };
        for first in [tensor_operand(1), tensor_move_operand(1)] {
            let function = projection_function_with_locals(
                vec![block(
                    102,
                    vec![
                        assignment(SemanticLocalIdV1::from_index(2), first),
                        assignment(SemanticLocalIdV1::from_index(3), tensor_operand(1)),
                    ],
                    SemanticTerminatorKindV1::Return,
                )],
                vec![
                    local(102, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                    local(103, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                    local(104, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                    local(105, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                ],
            );
            let payload =
                SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
            let mut state = HashMap::from([(1, tensor_state_origin())]);
            transfer_tensor_statements_v1(&function, 0, &mut state, &payload).unwrap();
            assert_eq!(state[&1], ProjectedTensorValueV1::Invalid);
            assert_eq!(state[&2], tensor_state_origin());
            assert_eq!(state[&3], ProjectedTensorValueV1::Invalid);
        }
    }

    #[test]
    fn partial_assignment_discriminant_and_deinitialize_invalidate_enum_transport() {
        let payload_place = match tensor_payload(1, 4) {
            SemanticOperandV1::Move(place) => place,
            _ => unreachable!(),
        };
        let statements = [
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                payload_place.clone(),
                SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(0))),
            ))),
            statement(SemanticStatementKindV1::SetDiscriminant {
                place: SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], ENUM_TYPE)
                    .unwrap(),
                variant_index: 5,
            }),
            statement(SemanticStatementKindV1::Deinitialize(payload_place)),
        ];
        for invalidating_statement in statements {
            let function = projection_function_with_locals(
                vec![block(
                    106,
                    vec![invalidating_statement],
                    SemanticTerminatorKindV1::Return,
                )],
                vec![
                    local(106, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                    local(107, ENUM_TYPE, SemanticLocalRoleV1::Temporary),
                ],
            );
            let payload =
                SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types_with_enum())
                    .unwrap();
            let wrapped = wrap_tensor_enum_value_v1(tensor_state_origin(), 4).unwrap();
            let mut state = HashMap::from([(1, wrapped)]);
            transfer_tensor_statements_v1(&function, 0, &mut state, &payload).unwrap();
            assert_eq!(state[&1], ProjectedTensorValueV1::Invalid);
            assert_eq!(
                tensor_origin_from_assignment_operand_v1(
                    &tensor_payload(1, 4),
                    &state,
                    &payload,
                    SemanticBlockIdV1::from_index(0),
                ),
                Some(ProjectedTensorValueV1::Invalid)
            );
        }
    }

    #[test]
    fn call_operands_consume_tensor_origins_even_without_a_known_producer() {
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![tensor_move_operand(1)],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let function = projection_function_with_locals(
            vec![block(108, vec![], SemanticTerminatorKindV1::Call(call))],
            vec![
                local(108, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(109, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        let mut state = HashMap::from([(1, tensor_state_origin())]);
        assert_eq!(
            transfer_tensor_terminator_v1(&[], &function, 0, &mut state, &[None; 4], &[], false)
                .unwrap(),
            ProjectedTensorTerminatorEffectsV1::default()
        );
        assert_eq!(state[&1], ProjectedTensorValueV1::Invalid);
    }

    #[test]
    fn tensor_origin_on_only_one_predecessor_becomes_invalid_at_the_join() {
        let mut current = HashMap::from([(
            7,
            ProjectedTensorValueV1::Known(ProjectedTensorOriginV1::Lane {
                root: 1,
                wave_width: 64,
            }),
        )]);
        assert!(merge_tensor_states_v1(&mut current, &HashMap::new()).unwrap());
        assert_eq!(current[&7], ProjectedTensorValueV1::Invalid);
    }

    #[test]
    fn duplicate_tensor_cfg_successors_are_charged_once_and_merged_once() {
        let targets = (0..65_536_u128)
            .map(|value| {
                SemanticSwitchTargetV1::new(value, cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1))
            })
            .collect();
        let terminator = SemanticTerminatorKindV1::SwitchInt {
            discriminant: constant(0),
            targets: SemanticSwitchTargetsV1::new(
                targets,
                cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
            )
            .unwrap(),
        };
        let mut work = 0;
        assert_eq!(
            charged_unique_tensor_successors_v1(&terminator, 7, &mut work).unwrap(),
            vec![1, 2]
        );
        assert_eq!(work, 65_537 + 2 * (7 + 1));
    }

    #[test]
    fn tensor_cfg_successor_deduplication_is_deterministic_and_resource_bounded() {
        let terminator = SemanticTerminatorKindV1::SwitchInt {
            discriminant: constant(0),
            targets: SemanticSwitchTargetsV1::new(
                vec![
                    SemanticSwitchTargetV1::new(0, cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3)),
                    SemanticSwitchTargetV1::new(1, cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1)),
                    SemanticSwitchTargetV1::new(2, cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3)),
                    SemanticSwitchTargetV1::new(3, cfg_edge(SemanticEdgeRoleV1::SwitchValue, 2)),
                ],
                cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
            )
            .unwrap(),
        };
        for _ in 0..4 {
            let mut work = 0;
            assert_eq!(
                charged_unique_tensor_successors_v1(&terminator, 0, &mut work).unwrap(),
                vec![1, 2, 3]
            );
            assert_eq!(work, 8);
        }

        let mut exhausted_work = MAX_PROJECTED_TENSOR_DATAFLOW_WORK_V1 - 1;
        assert!(matches!(
            charged_unique_tensor_successors_v1(&terminator, 0, &mut exhausted_work),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "tensor producer dataflow exceeds the charged projection limit"
            ))
        ));
    }

    #[test]
    fn uniform_switch_projection_accepts_only_immutable_arguments_or_constants() {
        let function = projection_function_with_locals(
            vec![block(93, vec![], SemanticTerminatorKindV1::Return)],
            vec![
                local(93, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(94, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(95, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        let definitions = local_definition_counts(&function);
        let origins = local_stable_argument_origins(&function).unwrap();
        let mut arguments = vec![None; function.locals().len()];
        let mut next_argument = 1;
        let mut operations = Vec::new();
        let mut next_value = 0;
        assert!(matches!(
            project_uniform_switch_operand_v1(
                &tensor_operand(1),
                &[None; 3],
                &origins,
                &definitions,
                &function,
                &mut arguments,
                &mut next_argument,
                &mut operations,
                &mut next_value,
            )
            .unwrap(),
            Some(ProductionRankedValueV1::Argument(1))
        ));
        assert!(
            project_uniform_switch_operand_v1(
                &tensor_operand(2),
                &[None; 3],
                &origins,
                &definitions,
                &function,
                &mut arguments,
                &mut next_argument,
                &mut operations,
                &mut next_value,
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    fn local_provenance_preserves_only_value_and_pointer_identity_operations() {
        let pointer_place = |local| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], POINTER_TYPE)
                .unwrap()
        };
        let assign = |destination, ty, value| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(destination), vec![], ty)
                    .unwrap(),
                SemanticRvalueV1::new(ty, value),
            )))
        };
        let loaded_scalar = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(3),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR_TYPE)
                    .unwrap(),
            ],
            SCALAR_TYPE,
        )
        .unwrap();
        let function = projection_function_with_locals(
            vec![block(
                96,
                vec![
                    assign(
                        2,
                        POINTER_TYPE,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(pointer_place(1))),
                    ),
                    assign(
                        3,
                        POINTER_TYPE,
                        SemanticRvalueKindV1::Cast {
                            kind: SemanticCastKindV1::Pointer,
                            operand: SemanticOperandV1::Copy(pointer_place(2)),
                        },
                    ),
                    assign(
                        4,
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                            loaded_scalar,
                            SemanticVolatilityV1::NonVolatile,
                            None,
                        )),
                    ),
                    assign(
                        5,
                        SCALAR_TYPE,
                        SemanticRvalueKindV1::Cast {
                            kind: SemanticCastKindV1::PointerExposeProvenance,
                            operand: SemanticOperandV1::Copy(pointer_place(3)),
                        },
                    ),
                ],
                SemanticTerminatorKindV1::Return,
            )],
            vec![
                local(96, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(97, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(98, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(99, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(100, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(101, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );

        let provenance = local_provenance_v1(&function).unwrap();
        assert_eq!(
            provenance.stable_argument_origins,
            vec![None, Some(0), Some(0), Some(0), None, Some(0)]
        );
        assert_eq!(
            provenance.allocation_origins,
            vec![None, Some(0), Some(0), Some(0), None, None]
        );
    }

    #[test]
    fn local_allocation_provenance_requires_an_exact_reborrow() {
        let assign_borrow = |destination, place| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(destination),
                    vec![],
                    POINTER_TYPE,
                )
                .unwrap(),
                SemanticRvalueV1::new(
                    POINTER_TYPE,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place,
                    },
                ),
            )))
        };
        let dereference =
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SCALAR_TYPE).unwrap();
        let pointee_place = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![dereference],
            SCALAR_TYPE,
        )
        .unwrap();
        let pointer_slot =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], POINTER_TYPE).unwrap();
        let function = projection_function_with_locals(
            vec![block(
                102,
                vec![
                    assign_borrow(2, pointee_place),
                    assign_borrow(3, pointer_slot),
                ],
                SemanticTerminatorKindV1::Return,
            )],
            vec![
                local(102, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(103, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(104, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(105, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );

        let provenance = local_provenance_v1(&function).unwrap();
        assert_eq!(
            provenance.allocation_origins,
            vec![None, Some(0), Some(0), None]
        );
        assert_eq!(
            provenance.stable_argument_origins,
            vec![None, Some(0), None, None]
        );
    }

    #[test]
    fn direct_borrow_preserves_only_an_authenticated_exclusive_owner() {
        let direct_place =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], POINTER_TYPE).unwrap();
        let direct_borrow = |place| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], POINTER_TYPE)
                    .unwrap(),
                SemanticRvalueV1::new(
                    POINTER_TYPE,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place,
                    },
                ),
            )))
        };
        let build = |ownership| {
            projection_function_with_owned_argument(
                vec![block(
                    116,
                    vec![direct_borrow(direct_place.clone())],
                    SemanticTerminatorKindV1::Return,
                )],
                vec![
                    local(116, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                    local(117, POINTER_TYPE, SemanticLocalRoleV1::Argument(0)),
                    local(118, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                ],
                ownership,
            )
        };

        assert_eq!(
            local_provenance_v1(&build(SemanticSourceArgumentOwnershipV1::ExclusiveOwner))
                .unwrap()
                .allocation_origins,
            vec![None, Some(0), Some(0)]
        );
        for ownership in [
            SemanticSourceArgumentOwnershipV1::RawPointer,
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            SemanticSourceArgumentOwnershipV1::UniqueBorrow,
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::Unspecified,
        ] {
            assert_eq!(
                local_provenance_v1(&build(ownership))
                    .unwrap()
                    .allocation_origins,
                vec![None, Some(0), None]
            );
        }
    }

    #[test]
    fn local_provenance_merge_conflicts_fail_closed() {
        let mut origins = vec![Some(0), Some(1), None];
        let edges = vec![vec![2], vec![2], vec![]];
        assert!(matches!(
            propagate_exact_local_origins_v1(&mut origins, &edges, "conflicting test origins",),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "conflicting test origins"
            ))
        ));
    }

    #[test]
    fn repeated_comparison_definitions_must_be_identical() {
        let first = GuardPredicateV1 {
            comparisons: vec![(
                ProductionRankedValueV1::Argument(1),
                ProductionRankedValueV1::Argument(2),
            )],
        };
        let conflicting = GuardPredicateV1 {
            comparisons: vec![(
                ProductionRankedValueV1::Argument(2),
                ProductionRankedValueV1::Argument(1),
            )],
        };
        let mut slot = None;
        retain_identical_direct_switch_predicate_v1(&mut slot, first.clone()).unwrap();
        retain_identical_direct_switch_predicate_v1(&mut slot, first.clone()).unwrap();
        assert_eq!(slot, Some(first));
        assert!(matches!(
            retain_identical_direct_switch_predicate_v1(&mut slot, conflicting),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "one comparison local has conflicting source definitions"
            ))
        ));
    }

    fn explicit_binary_switch_with_fallback(
        variants: [u128; 2],
        fallback_statements: Vec<SemanticStatementV1>,
        fallback_terminator: SemanticTerminatorKindV1,
    ) -> SemanticFunctionDeclV1 {
        explicit_binary_switch_with_targets(
            variants,
            [1, 2],
            fallback_statements,
            fallback_terminator,
        )
    }

    fn explicit_binary_switch_with_targets(
        variants: [u128; 2],
        variant_targets: [u32; 2],
        fallback_statements: Vec<SemanticStatementV1>,
        fallback_terminator: SemanticTerminatorKindV1,
    ) -> SemanticFunctionDeclV1 {
        projection_function_with_locals(
            vec![
                block(
                    98,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: tensor_operand(1),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![
                                SemanticSwitchTargetV1::new(
                                    variants[0],
                                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, variant_targets[0]),
                                ),
                                SemanticSwitchTargetV1::new(
                                    variants[1],
                                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, variant_targets[1]),
                                ),
                            ],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                        )
                        .unwrap(),
                    },
                ),
                block(99, vec![], SemanticTerminatorKindV1::Return),
                block(100, vec![], SemanticTerminatorKindV1::Return),
                block(101, fallback_statements, fallback_terminator),
            ],
            vec![
                local(98, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(99, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
            ],
        )
    }

    #[test]
    fn explicit_zero_one_switch_elides_only_an_empty_unreachable_fallback() {
        let function = explicit_binary_switch_with_fallback(
            [0, 1],
            vec![],
            SemanticTerminatorKindV1::Unreachable,
        );
        assert_eq!(
            projected_cfg_terminator(&function, 0, false, &[], &[const { None }; 2], &[]).unwrap(),
            ProjectedCfgTerminatorV1::AnalysisSplit {
                first_block: 1,
                second_block: 2,
            }
        );
    }

    fn non_bounds_assert_function(condition: SemanticOperandV1) -> SemanticFunctionDeclV1 {
        projection_function_with_locals(
            vec![
                block(
                    138,
                    vec![],
                    SemanticTerminatorKindV1::Assert {
                        condition,
                        expected: true,
                        message: SemanticAssertMessageV1::DivisionByZero(tensor_operand(2)),
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(139, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(138, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(139, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(140, SCALAR_TYPE, SemanticLocalRoleV1::Argument(1)),
            ],
        )
    }

    #[test]
    fn non_bounds_asserts_are_elided_only_after_exact_constant_success() {
        let unresolved = non_bounds_assert_function(tensor_operand(1));
        assert!(matches!(
            projected_cfg_terminator(&unresolved, 0, false, &[], &[const { None }; 3], &[]),
            Err(ProductionRankedProjectionErrorV1::UnprovenAssert {
                block: 0,
                kind: "division-by-zero",
                ..
            })
        ));

        let proven = non_bounds_assert_function(constant(1));
        assert_eq!(
            projected_cfg_terminator(&proven, 0, false, &[], &[const { None }; 3], &[]).unwrap(),
            ProjectedCfgTerminatorV1::Branch(1)
        );
    }

    #[derive(Clone, Copy)]
    enum AssertionProofShape {
        Complete,
        MissingPath,
        SameSuccessor,
        Reassignment,
        CallDestination,
        Unresolved,
        NarrowCast,
        SignedSource,
        Overflow,
    }

    fn typed_place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
    }

    fn typed_operand(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
        SemanticOperandV1::Copy(typed_place(local, ty))
    }

    fn typed_constant(ty: SemanticTypeIdV1, value: u128, bytes: u8) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, bytes).unwrap()),
        ))
    }

    fn typed_assignment(
        local: u32,
        ty: SemanticTypeIdV1,
        value: SemanticRvalueKindV1,
    ) -> SemanticStatementV1 {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            typed_place(local, ty),
            SemanticRvalueV1::new(ty, value),
        )))
    }

    fn zero_switch(
        local: u32,
        ty: SemanticTypeIdV1,
        zero: u32,
        nonzero: u32,
    ) -> SemanticTerminatorKindV1 {
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: typed_operand(local, ty),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, zero),
                )],
                cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, nonzero),
            )
            .unwrap(),
        }
    }

    fn assertion_proof_function(shape: AssertionProofShape) -> SemanticFunctionDeclV1 {
        let source_ty = if matches!(shape, AssertionProofShape::SignedSource) {
            I32_TYPE
        } else {
            SCALAR_TYPE
        };
        let first_check = zero_switch(
            1,
            source_ty,
            if matches!(shape, AssertionProofShape::SameSuccessor) {
                4
            } else {
                3
            },
            4,
        );
        let second_check = if matches!(
            shape,
            AssertionProofShape::MissingPath | AssertionProofShape::Unresolved
        ) {
            SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 5))
        } else {
            zero_switch(
                1,
                source_ty,
                if matches!(shape, AssertionProofShape::SameSuccessor) {
                    5
                } else {
                    3
                },
                5,
            )
        };
        let first_result = match shape {
            AssertionProofShape::Reassignment => {
                SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 6))
            }
            AssertionProofShape::CallDestination => SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        typed_place(1, source_ty),
                        cfg_edge(SemanticEdgeRoleV1::CallReturn, 6),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
            _ => zero_switch(3, BOOL_TYPE, 3, 6),
        };
        let first_statements = matches!(shape, AssertionProofShape::Reassignment)
            .then(|| {
                vec![typed_assignment(
                    1,
                    source_ty,
                    SemanticRvalueKindV1::Use(typed_constant(source_ty, 0, 4)),
                )]
            })
            .unwrap_or_default();

        let mut proof_statements = vec![typed_assignment(
            4,
            if matches!(shape, AssertionProofShape::NarrowCast) {
                U8_TYPE
            } else {
                U64_TYPE
            },
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: typed_operand(1, source_ty),
            },
        )];
        let asserted_value = if matches!(shape, AssertionProofShape::NarrowCast) {
            4
        } else {
            proof_statements.extend([
                typed_assignment(
                    5,
                    U64_TYPE,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: typed_operand(4, U64_TYPE),
                        right: typed_constant(
                            U64_TYPE,
                            if matches!(shape, AssertionProofShape::Overflow) {
                                u128::from(u64::MAX)
                            } else {
                                15
                            },
                            8,
                        ),
                    },
                ),
                typed_assignment(
                    6,
                    U64_TYPE,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Divide,
                        left: typed_operand(5, U64_TYPE),
                        right: typed_constant(U64_TYPE, 16, 8),
                    },
                ),
            ]);
            6
        };
        proof_statements.push(typed_assignment(
            7,
            BOOL_TYPE,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Equal,
                left: typed_operand(
                    asserted_value,
                    if asserted_value == 4 && matches!(shape, AssertionProofShape::NarrowCast) {
                        U8_TYPE
                    } else {
                        U64_TYPE
                    },
                ),
                right: typed_constant(
                    if asserted_value == 4 && matches!(shape, AssertionProofShape::NarrowCast) {
                        U8_TYPE
                    } else {
                        U64_TYPE
                    },
                    0,
                    if asserted_value == 4 && matches!(shape, AssertionProofShape::NarrowCast) {
                        1
                    } else {
                        8
                    },
                ),
            },
        ));
        let assert_message_operand = typed_operand(
            asserted_value,
            if asserted_value == 4 && matches!(shape, AssertionProofShape::NarrowCast) {
                U8_TYPE
            } else {
                U64_TYPE
            },
        );

        projection_function_with_locals(
            vec![
                block(150, vec![], zero_switch(2, BOOL_TYPE, 1, 2)),
                block(
                    151,
                    vec![],
                    if matches!(shape, AssertionProofShape::Unresolved) {
                        SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4))
                    } else {
                        first_check
                    },
                ),
                block(152, vec![], second_check),
                block(153, vec![], SemanticTerminatorKindV1::Return),
                block(154, first_statements, first_result),
                block(155, vec![], zero_switch(3, BOOL_TYPE, 3, 6)),
                block(
                    156,
                    proof_statements,
                    SemanticTerminatorKindV1::Assert {
                        condition: typed_operand(7, BOOL_TYPE),
                        expected: false,
                        message: SemanticAssertMessageV1::DivisionByZero(assert_message_operand),
                        target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 7),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(157, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(150, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(151, source_ty, SemanticLocalRoleV1::Argument(0)),
                local(152, BOOL_TYPE, SemanticLocalRoleV1::Argument(1)),
                local(153, BOOL_TYPE, SemanticLocalRoleV1::Argument(2)),
                local(
                    154,
                    if matches!(shape, AssertionProofShape::NarrowCast) {
                        U8_TYPE
                    } else {
                        U64_TYPE
                    },
                    SemanticLocalRoleV1::Temporary,
                ),
                local(155, U64_TYPE, SemanticLocalRoleV1::Temporary),
                local(156, U64_TYPE, SemanticLocalRoleV1::Temporary),
                local(157, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    #[test]
    fn nonzero_assert_proof_requires_every_path_and_value_preserving_arithmetic() {
        let types = assertion_proof_types();
        let complete = assertion_proof_function(AssertionProofShape::Complete);
        assert!(SemanticAssertProofsV1::analyze(&types, &complete).unwrap()[6]);

        for shape in [
            AssertionProofShape::MissingPath,
            AssertionProofShape::SameSuccessor,
            AssertionProofShape::Reassignment,
            AssertionProofShape::CallDestination,
            AssertionProofShape::Unresolved,
            AssertionProofShape::NarrowCast,
            AssertionProofShape::SignedSource,
            AssertionProofShape::Overflow,
        ] {
            let function = assertion_proof_function(shape);
            assert!(!SemanticAssertProofsV1::analyze(&types, &function).unwrap()[6]);
        }
    }

    #[test]
    fn explicit_binary_switch_keeps_reachable_or_malformed_fallbacks_fail_closed() {
        for function in [
            explicit_binary_switch_with_fallback([0, 1], vec![], SemanticTerminatorKindV1::Return),
            explicit_binary_switch_with_fallback(
                [0, 1],
                vec![statement(SemanticStatementKindV1::Nop)],
                SemanticTerminatorKindV1::Unreachable,
            ),
            explicit_binary_switch_with_fallback(
                [0, 2],
                vec![],
                SemanticTerminatorKindV1::Unreachable,
            ),
        ] {
            assert!(matches!(
                projected_cfg_terminator(&function, 0, false, &[], &[const { None }; 2], &[]),
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a general switch whose only extra successor is not one empty unreachable fallback"
                ))
            ));
        }
    }

    #[test]
    fn checked_option_switch_uses_variant_values_independent_of_target_order() {
        let function = explicit_binary_switch_with_targets(
            [0, 1],
            [2, 1],
            vec![],
            SemanticTerminatorKindV1::Unreachable,
        );
        let predicate = GuardPredicateV1 {
            comparisons: vec![(
                ProductionRankedValueV1::Argument(0),
                ProductionRankedValueV1::Argument(1),
            )],
        };
        assert_eq!(
            projected_cfg_terminator(
                &function,
                0,
                false,
                &[],
                &[None, Some(predicate.clone())],
                &[],
            )
            .unwrap(),
            ProjectedCfgTerminatorV1::Predicate {
                predicate,
                true_block: 1,
                false_block: 2,
            }
        );
    }

    fn single_explicit_boolean_switch(
        explicit_value: u128,
        explicit_target: u32,
        otherwise_target: u32,
    ) -> SemanticFunctionDeclV1 {
        projection_function_with_locals(
            vec![
                block(
                    102,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: tensor_operand(1),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                explicit_value,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, explicit_target),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise_target),
                        )
                        .unwrap(),
                    },
                ),
                block(103, vec![], SemanticTerminatorKindV1::Return),
                block(104, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(100, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(101, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
            ],
        )
    }

    fn deterministic_scalar_switch_projection(
        callables: &[SemanticCallableDeclV1],
        function: &SemanticFunctionDeclV1,
        index_values: &[Option<ProjectedDisjointIndexV1>],
        operations: Vec<ProductionRankedOperationV1>,
        next_value: u32,
    ) -> Result<
        (
            Vec<Option<ProjectedDeterministicSwitchV1>>,
            Vec<ProductionRankedOperationV1>,
            usize,
        ),
        ProductionRankedProjectionErrorV1,
    > {
        deterministic_scalar_switch_projection_with_allocations(
            callables,
            function,
            index_values,
            &vec![None; function.locals().len()],
            operations,
            next_value,
        )
    }

    fn deterministic_scalar_switch_projection_with_allocations(
        callables: &[SemanticCallableDeclV1],
        function: &SemanticFunctionDeclV1,
        index_values: &[Option<ProjectedDisjointIndexV1>],
        local_allocations: &[Option<AllocationContractV1>],
        mut operations: Vec<ProductionRankedOperationV1>,
        mut next_value: u32,
    ) -> Result<
        (
            Vec<Option<ProjectedDeterministicSwitchV1>>,
            Vec<ProductionRankedOperationV1>,
            usize,
        ),
        ProductionRankedProjectionErrorV1,
    > {
        let constants = constant_locals(function);
        let definitions = local_definition_counts(function);
        let mut arguments = vec![None; function.locals().len()];
        let mut next_argument = 1;
        let switches = project_deterministic_scalar_switches_v1(
            callables,
            function,
            &constants,
            &definitions,
            index_values,
            local_allocations,
            &vec![None; function.locals().len()],
            &mut arguments,
            &mut next_argument,
            &mut operations,
            &mut next_value,
        )?;
        Ok((switches, operations, next_argument))
    }

    fn deterministic_expression_switch(
        definitions: Vec<SemanticStatementV1>,
        locals: Vec<SemanticLocalDeclV1>,
        discriminant: u32,
    ) -> SemanticFunctionDeclV1 {
        projection_function_with_locals(
            vec![
                block(
                    110,
                    definitions,
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: tensor_operand(discriminant),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![
                                SemanticSwitchTargetV1::new(
                                    3,
                                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                                ),
                                SemanticSwitchTargetV1::new(
                                    7,
                                    cfg_edge(SemanticEdgeRoleV1::SwitchValue, 2),
                                ),
                            ],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                        )
                        .unwrap(),
                    },
                ),
                block(111, vec![], SemanticTerminatorKindV1::Return),
                block(112, vec![], SemanticTerminatorKindV1::Return),
                block(113, vec![], SemanticTerminatorKindV1::Return),
            ],
            locals,
        )
    }

    fn scalar_assignment(local: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], SCALAR_TYPE)
                .unwrap(),
            SemanticRvalueV1::new(SCALAR_TYPE, value),
        )))
    }

    fn scalar_binary(
        operation: SemanticBinaryOpV1,
        left: SemanticOperandV1,
        right: SemanticOperandV1,
    ) -> SemanticRvalueKindV1 {
        SemanticRvalueKindV1::Binary {
            operation,
            left,
            right,
        }
    }

    fn deterministic_expression_locals(two_arguments: bool) -> Vec<SemanticLocalDeclV1> {
        let mut locals = vec![
            local(110, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(111, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
        ];
        if two_arguments {
            locals.push(local(112, SCALAR_TYPE, SemanticLocalRoleV1::Argument(1)));
        } else {
            locals.push(local(112, SCALAR_TYPE, SemanticLocalRoleV1::Temporary));
        }
        locals.extend([
            local(113, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            local(114, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            local(115, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
        ]);
        locals
    }

    #[test]
    fn deterministic_scalar_projection_preserves_divide_add_dependencies_and_switch_values() {
        let function = deterministic_expression_switch(
            vec![
                scalar_assignment(
                    3,
                    scalar_binary(SemanticBinaryOpV1::Divide, tensor_operand(1), constant(16)),
                ),
                scalar_assignment(
                    4,
                    scalar_binary(SemanticBinaryOpV1::Add, tensor_operand(3), constant(1)),
                ),
            ],
            deterministic_expression_locals(false),
            4,
        );
        let (switches, operations, _) = deterministic_scalar_switch_projection(
            &[],
            &function,
            &vec![None; function.locals().len()],
            vec![],
            0,
        )
        .unwrap();

        let projected = switches[0].as_ref().expect("switch must be exact");
        assert_eq!(projected.targets.len(), 2);
        assert_eq!(projected.targets[0].1, 1);
        assert_eq!(projected.targets[1].1, 2);
        assert_eq!(projected.otherwise, 3);
        let binaries = operations
            .iter()
            .filter_map(|operation| match operation {
                ProductionRankedOperationV1::IndexBinary {
                    result,
                    kind,
                    lhs,
                    rhs,
                } => Some((*result, *kind, *lhs, *rhs)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(binaries.len(), 2);
        assert_eq!(binaries[0].1, IndexBinaryKindAttr::Divide);
        assert_eq!(binaries[0].2, ProductionRankedValueV1::Argument(1));
        assert_eq!(binaries[1].1, IndexBinaryKindAttr::Add);
        assert_eq!(binaries[1].2, ProductionRankedValueV1::Local(binaries[0].0));
        assert!(!operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::DeterministicJoin { .. }
        )));
        let constant_values = operations
            .iter()
            .filter_map(|operation| match operation {
                ProductionRankedOperationV1::IndexConstant { value, .. } => Some(*value),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(constant_values.contains(&1));
        assert!(constant_values.contains(&3));
        assert!(constant_values.contains(&7));
        assert!(constant_values.contains(&16));

        let (blocks, _) = build_ranked_cfg(
            &projection_types(),
            &function,
            &vec![None; function.locals().len()],
            &switches,
            &[],
            operations,
            (0..function.blocks().len())
                .map(|_| ProjectedSemanticBlockV1 { items: vec![] })
                .collect(),
        )
        .unwrap();
        assert_eq!(
            blocks
                .iter()
                .filter(|block| matches!(
                    block.terminator(),
                    ProductionRankedTerminatorV1::IndexEqual { .. }
                ))
                .count(),
            2,
            "every explicit source variant must remain one exact equality edge"
        );
    }

    #[test]
    fn deterministic_scalar_projection_rejects_partial_dynamic_or_zero_division() {
        for divisor in [tensor_operand(2), constant(0)] {
            let function = deterministic_expression_switch(
                vec![scalar_assignment(
                    3,
                    scalar_binary(SemanticBinaryOpV1::Divide, tensor_operand(1), divisor),
                )],
                deterministic_expression_locals(true),
                3,
            );
            assert!(matches!(
                deterministic_scalar_switch_projection(
                    &[],
                    &function,
                    &vec![None; function.locals().len()],
                    vec![],
                    0,
                ),
                Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "a division or remainder used for deterministic control lacks a statically nonzero divisor"
                ))
            ));
        }
    }

    #[test]
    fn deterministic_scalar_projection_rejects_one_missing_dependency() {
        let function = deterministic_expression_switch(
            vec![scalar_assignment(
                4,
                scalar_binary(
                    SemanticBinaryOpV1::Add,
                    tensor_operand(1),
                    tensor_operand(2),
                ),
            )],
            deterministic_expression_locals(false),
            4,
        );
        let (switches, operations, _) = deterministic_scalar_switch_projection(
            &[],
            &function,
            &vec![None; function.locals().len()],
            vec![],
            0,
        )
        .unwrap();
        assert!(switches[0].is_none());
        assert!(!operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::DeterministicJoin { .. }
        )));
    }

    #[test]
    fn repeated_deterministic_definitions_meet_only_when_dependencies_match() {
        let definition = |argument| {
            scalar_assignment(
                3,
                scalar_binary(
                    SemanticBinaryOpV1::Divide,
                    tensor_operand(argument),
                    constant(16),
                ),
            )
        };
        for second_argument in [1, 2] {
            let function = deterministic_expression_switch(
                vec![
                    definition(1),
                    definition(second_argument),
                    scalar_assignment(
                        4,
                        scalar_binary(SemanticBinaryOpV1::Add, tensor_operand(3), constant(1)),
                    ),
                ],
                deterministic_expression_locals(true),
                4,
            );
            let (switches, operations, _) = deterministic_scalar_switch_projection(
                &[],
                &function,
                &vec![None; function.locals().len()],
                vec![],
                0,
            )
            .unwrap();
            assert_eq!(switches[0].is_some(), second_argument == 1);
            assert!(!operations.iter().any(|operation| matches!(
                operation,
                ProductionRankedOperationV1::DeterministicJoin { .. }
            )));
        }
    }

    fn control_selected_definition_switch(selector: SemanticLocalRoleV1) -> SemanticFunctionDeclV1 {
        let destination = SemanticLocalIdV1::from_index(2);
        let place = SemanticPlaceV1::new(destination, vec![], SCALAR_TYPE).unwrap();
        projection_function_with_locals(
            vec![
                block(
                    132,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: tensor_operand(1),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                block(
                    133,
                    vec![scalar_assignment(
                        2,
                        scalar_binary(SemanticBinaryOpV1::Add, tensor_operand(3), constant(1)),
                    )],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(
                    134,
                    vec![scalar_assignment(
                        2,
                        scalar_binary(SemanticBinaryOpV1::Subtract, tensor_operand(3), constant(1)),
                    )],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(
                    135,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Move(place),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 4),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 5),
                        )
                        .unwrap(),
                    },
                ),
                block(136, vec![], SemanticTerminatorKindV1::Return),
                block(137, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(132, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(133, SCALAR_TYPE, selector),
                local(134, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(135, SCALAR_TYPE, SemanticLocalRoleV1::Argument(1)),
            ],
        )
    }

    #[test]
    fn control_selected_definitions_include_the_exact_selector_dependency() {
        let uniform = control_selected_definition_switch(SemanticLocalRoleV1::Argument(0));
        let (switches, operations, _) = deterministic_scalar_switch_projection(
            &[],
            &uniform,
            &vec![None; uniform.locals().len()],
            vec![],
            0,
        )
        .unwrap();
        assert!(switches[3].is_some());
        assert!(operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::DeterministicJoin { dependencies, .. }
                if dependencies.contains(&ProductionRankedValueV1::Argument(1))
                    && dependencies.contains(&ProductionRankedValueV1::Argument(2))
        )));

        let varying = control_selected_definition_switch(SemanticLocalRoleV1::Temporary);
        let invocation = ProductionRankedValueIdV1::new(0);
        let mut index_values = vec![None; varying.locals().len()];
        index_values[1] = Some(ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Local(invocation),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        });
        let (switches, operations, _) = deterministic_scalar_switch_projection(
            &[],
            &varying,
            &index_values,
            vec![ProductionRankedOperationV1::InvocationIndex {
                result: invocation,
                dimension: 0,
                launch_extent: 0,
            }],
            1,
        )
        .unwrap();
        assert!(switches[3].is_some());
        assert!(operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::DeterministicJoin { dependencies, .. }
                if dependencies.contains(&ProductionRankedValueV1::Local(invocation))
        )));

        let unknown = control_selected_definition_switch(SemanticLocalRoleV1::Temporary);
        let (switches, _, _) = deterministic_scalar_switch_projection(
            &[],
            &unknown,
            &vec![None; unknown.locals().len()],
            vec![],
            0,
        )
        .unwrap();
        assert!(switches[3].is_none());
    }

    #[test]
    fn lane_derived_scalar_control_retains_the_invocation_dependency() {
        let function = deterministic_expression_switch(
            vec![scalar_assignment(
                4,
                scalar_binary(SemanticBinaryOpV1::Add, tensor_operand(1), constant(1)),
            )],
            deterministic_expression_locals(false),
            4,
        );
        let invocation = ProductionRankedValueIdV1::new(0);
        let mut index_values = vec![None; function.locals().len()];
        index_values[1] = Some(ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Local(invocation),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        });
        let (switches, operations, _) = deterministic_scalar_switch_projection(
            &[],
            &function,
            &index_values,
            vec![ProductionRankedOperationV1::InvocationIndex {
                result: invocation,
                dimension: 0,
                launch_extent: 0,
            }],
            1,
        )
        .unwrap();
        assert!(switches[0].is_some());
        assert!(operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::IndexBinary {
                kind: IndexBinaryKindAttr::Add,
                lhs: ProductionRankedValueV1::Local(value),
                ..
            } if *value == invocation
        )));
        assert!(!operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::DeterministicJoin { .. }
        )));
    }

    fn compiler_intrinsic_callable(
        operation: SemanticCompilerIntrinsicOperationV1,
    ) -> SemanticCallableDeclV1 {
        let abi = projection_function(vec![block(116, vec![], SemanticTerminatorKindV1::Return)])
            .abi()
            .clone();
        SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256(bytes(116)),
                SemanticItemDefinitionIdentityV1::from_sha256(bytes(117)),
                SemanticMonomorphizationIdentityV1::from_sha256(bytes(118)),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(119)),
                SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(120)),
                SemanticSourceProvenanceV1::unavailable(),
                abi,
            ),
            operation,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(121)),
        }
    }

    fn call_discriminant_switch(
        callee: u32,
        arguments: Vec<SemanticOperandV1>,
    ) -> SemanticFunctionDeclV1 {
        let carrier = SemanticLocalIdV1::from_index(3);
        let discriminant = SemanticLocalIdV1::from_index(4);
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                SemanticPlaceV1::new(carrier, vec![], ENUM_TYPE).unwrap(),
                cfg_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        projection_function_with_locals(
            vec![
                block(122, vec![], SemanticTerminatorKindV1::Call(call)),
                block(
                    123,
                    vec![enum_discriminant(carrier, discriminant)],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: tensor_operand(4),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 2),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                        )
                        .unwrap(),
                    },
                ),
                block(124, vec![], SemanticTerminatorKindV1::Return),
                block(125, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(120, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(121, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(122, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(123, ENUM_TYPE, SemanticLocalRoleV1::Temporary),
                local(124, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    #[test]
    fn authenticated_pure_checked_constructor_projects_all_scalar_dependencies() {
        let callable = compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewRowMajor {
                result: ENUM_TYPE,
                view: POINTER_TYPE,
                error: SCALAR_TYPE,
                role: SemanticMfmaOperandRoleV1::A,
                storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            },
        );
        let function = call_discriminant_switch(0, vec![tensor_operand(1), constant(16)]);
        let (switches, operations, _) = deterministic_scalar_switch_projection(
            &[callable.clone()],
            &function,
            &vec![None; function.locals().len()],
            vec![],
            0,
        )
        .unwrap();
        assert!(switches[1].is_some());
        assert!(operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::DeterministicJoin { dependencies, .. }
                if dependencies.contains(&ProductionRankedValueV1::Argument(1))
                    && dependencies.len() == 2
        )));

        let missing = call_discriminant_switch(0, vec![tensor_operand(1), tensor_operand(2)]);
        let (switches, operations, _) = deterministic_scalar_switch_projection(
            &[callable],
            &missing,
            &vec![None; missing.locals().len()],
            vec![],
            0,
        )
        .unwrap();
        assert!(switches[1].is_none());
        assert!(!operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::DeterministicJoin { .. }
        )));

        let strided_callable = compiler_intrinsic_callable(
            SemanticCompilerIntrinsicOperationV1::StridedReadView2DFromSharedSlice {
                result: ENUM_TYPE,
                view: POINTER_TYPE,
                error: SCALAR_TYPE,
                element: SCALAR_TYPE,
            },
        );
        let strided = call_discriminant_switch(
            0,
            vec![
                tensor_operand(1),
                constant(0),
                constant(4),
                constant(8),
                constant(8),
            ],
        );
        let (switches, operations, _) = deterministic_scalar_switch_projection(
            &[strided_callable.clone()],
            &strided,
            &vec![None; strided.locals().len()],
            vec![],
            0,
        )
        .unwrap();
        assert!(switches[1].is_some());
        assert!(operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::DeterministicJoin { dependencies, .. }
                if dependencies.contains(&ProductionRankedValueV1::Argument(1))
                    && dependencies.len() == 4
        )));

        let varying_strided = call_discriminant_switch(
            0,
            vec![
                tensor_operand(1),
                constant(0),
                tensor_operand(2),
                constant(8),
                constant(8),
            ],
        );
        let (switches, operations, _) = deterministic_scalar_switch_projection(
            &[strided_callable.clone()],
            &varying_strided,
            &vec![None; varying_strided.locals().len()],
            vec![],
            0,
        )
        .unwrap();
        assert!(switches[1].is_none());
        assert!(!operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::DeterministicJoin { .. }
        )));

        let invocation = ProductionRankedValueIdV1::new(0);
        let mut lane_values = vec![None; varying_strided.locals().len()];
        lane_values[2] = Some(ProjectedDisjointIndexV1 {
            value: ProductionRankedValueV1::Local(invocation),
            mapping: SemanticDisjointIndexSpaceV1::Index1d,
            precondition: None,
            availability: None,
        });
        let (switches, operations, _) = deterministic_scalar_switch_projection(
            &[strided_callable],
            &varying_strided,
            &lane_values,
            vec![ProductionRankedOperationV1::InvocationIndex {
                result: invocation,
                dimension: 0,
                launch_extent: 0,
            }],
            1,
        )
        .unwrap();
        assert!(switches[1].is_some());
        assert!(operations.iter().any(|operation| matches!(
            operation,
            ProductionRankedOperationV1::DeterministicJoin { dependencies, .. }
                if dependencies.contains(&ProductionRankedValueV1::Local(invocation))
        )));
    }

    #[test]
    fn unknown_calls_memory_reads_and_private_addresses_remain_unresolved() {
        let unknown = call_discriminant_switch(0, vec![tensor_operand(1)]);
        let (switches, _, _) = deterministic_scalar_switch_projection(
            &[],
            &unknown,
            &vec![None; unknown.locals().len()],
            vec![],
            0,
        )
        .unwrap();
        assert!(switches[1].is_none());

        let loaded = deterministic_expression_switch(
            vec![scalar_assignment(
                4,
                SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], SCALAR_TYPE)
                        .unwrap(),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                )),
            )],
            deterministic_expression_locals(false),
            4,
        );
        let (switches, _, _) = deterministic_scalar_switch_projection(
            &[],
            &loaded,
            &vec![None; loaded.locals().len()],
            vec![],
            0,
        )
        .unwrap();
        assert!(switches[0].is_none());

        let pointer = SemanticLocalIdV1::from_index(3);
        let borrowed = deterministic_expression_switch(
            vec![
                statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(pointer, vec![], POINTER_TYPE).unwrap(),
                    SemanticRvalueV1::new(
                        POINTER_TYPE,
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: SemanticPlaceV1::new(
                                SemanticLocalIdV1::from_index(1),
                                vec![],
                                SCALAR_TYPE,
                            )
                            .unwrap(),
                        },
                    ),
                ))),
                scalar_assignment(
                    4,
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::PointerExposeProvenance,
                        operand: SemanticOperandV1::Copy(
                            SemanticPlaceV1::new(pointer, vec![], POINTER_TYPE).unwrap(),
                        ),
                    },
                ),
            ],
            vec![
                local(126, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(127, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(128, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(129, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
                local(130, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(131, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
            4,
        );
        let (switches, _, _) = deterministic_scalar_switch_projection(
            &[],
            &borrowed,
            &vec![None; borrowed.locals().len()],
            vec![],
            0,
        )
        .unwrap();
        assert!(switches[0].is_none());

        let mut allocations = vec![None; borrowed.locals().len()];
        allocations[1] = Some(AllocationContractV1 {
            allocation_origin: 1,
            noalias_class: 1,
            writable: false,
        });
        let (switches, _, _) = deterministic_scalar_switch_projection_with_allocations(
            &[],
            &borrowed,
            &vec![None; borrowed.locals().len()],
            &allocations,
            vec![],
            0,
        )
        .unwrap();
        assert!(switches[0].is_some());
    }

    #[test]
    fn comparison_predicate_accepts_canonical_single_explicit_boolean_target() {
        let predicate = GuardPredicateV1 {
            comparisons: vec![(
                ProductionRankedValueV1::Argument(0),
                ProductionRankedValueV1::Argument(1),
            )],
        };
        for function in [
            single_explicit_boolean_switch(0, 2, 1),
            single_explicit_boolean_switch(1, 1, 2),
        ] {
            assert_eq!(
                projected_cfg_terminator(
                    &function,
                    0,
                    false,
                    &[],
                    &[None, Some(predicate.clone())],
                    &[],
                )
                .unwrap(),
                ProjectedCfgTerminatorV1::Predicate {
                    predicate: predicate.clone(),
                    true_block: 1,
                    false_block: 2,
                }
            );
        }
        assert!(matches!(
            projected_cfg_terminator(
                &single_explicit_boolean_switch(2, 1, 2),
                0,
                false,
                &[],
                &[None, Some(predicate)],
                &[],
            ),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "a comparison predicate switch retained a non-boolean explicit value"
            ))
        ));
    }

    fn uniform_induction_function(bound_role: SemanticLocalRoleV1) -> SemanticFunctionDeclV1 {
        let induction = SemanticLocalIdV1::from_index(1);
        let predicate = SemanticLocalIdV1::from_index(2);
        let bound = SemanticLocalIdV1::from_index(3);
        let place = |local| SemanticPlaceV1::new(local, vec![], SCALAR_TYPE).unwrap();
        let operand = |local| SemanticOperandV1::Copy(place(local));
        let assign = |destination, value| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(destination),
                SemanticRvalueV1::new(SCALAR_TYPE, value),
            )))
        };
        projection_function_with_locals(
            vec![
                block(
                    100,
                    vec![assign(induction, SemanticRvalueKindV1::Use(constant(0)))],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(
                    101,
                    vec![assign(
                        predicate,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::LessThan,
                            left: operand(induction),
                            right: operand(bound),
                        },
                    )],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: operand(predicate),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                block(
                    102,
                    vec![assign(
                        induction,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::Add,
                            left: operand(induction),
                            right: constant(16),
                        },
                    )],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(103, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(100, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(101, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(102, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(103, SCALAR_TYPE, bound_role),
            ],
        )
    }

    #[derive(Clone, Copy, Debug)]
    enum InductionCfgShape {
        Chain,
        HeaderCopyAlias,
        Branched,
        TwoLatches,
        ExtraExit,
        AnalysisSplit,
        IrreducibleEntry,
        MultiplePreheaders,
        ReusedComparisonLocal,
        MultipleHeaderComparisons,
    }

    fn multi_block_induction_function(
        shape: InductionCfgShape,
        bound_role: SemanticLocalRoleV1,
        step: u64,
    ) -> SemanticFunctionDeclV1 {
        multi_block_induction_function_with_operands(
            shape,
            bound_role,
            constant(0),
            constant(step.into()),
        )
    }

    fn multi_block_induction_function_with_operands(
        shape: InductionCfgShape,
        bound_role: SemanticLocalRoleV1,
        initial_value: SemanticOperandV1,
        step_value: SemanticOperandV1,
    ) -> SemanticFunctionDeclV1 {
        let induction = SemanticLocalIdV1::from_index(1);
        let predicate = SemanticLocalIdV1::from_index(2);
        let bound = SemanticLocalIdV1::from_index(3);
        let body_predicate = SemanticLocalIdV1::from_index(4);
        let place = |local| SemanticPlaceV1::new(local, vec![], SCALAR_TYPE).unwrap();
        let operand = |local| SemanticOperandV1::Copy(place(local));
        let assign = |destination, value| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(destination),
                SemanticRvalueV1::new(SCALAR_TYPE, value),
            )))
        };
        let initialize = || assign(induction, SemanticRvalueKindV1::Use(initial_value.clone()));
        let compare = || {
            assign(
                predicate,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: operand(induction),
                    right: operand(bound),
                },
            )
        };
        let increment = || {
            assign(
                induction,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: operand(induction),
                    right: step_value.clone(),
                },
            )
        };
        let switch = |discriminant: SemanticOperandV1, false_target, true_target| {
            SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        cfg_edge(SemanticEdgeRoleV1::SwitchValue, false_target),
                    )],
                    cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, true_target),
                )
                .unwrap(),
            }
        };
        let blocks = match shape {
            InductionCfgShape::Chain => vec![
                block(
                    110,
                    vec![initialize()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(111, vec![compare()], switch(operand(predicate), 5, 2)),
                block(
                    112,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(
                    113,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
                ),
                block(
                    114,
                    vec![increment()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(115, vec![], SemanticTerminatorKindV1::Return),
            ],
            InductionCfgShape::HeaderCopyAlias => vec![
                block(
                    116,
                    vec![initialize()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(
                    117,
                    vec![
                        assign(
                            SemanticLocalIdV1::from_index(5),
                            SemanticRvalueKindV1::Use(operand(induction)),
                        ),
                        assign(
                            predicate,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::LessThan,
                                left: operand(SemanticLocalIdV1::from_index(5)),
                                right: operand(bound),
                            },
                        ),
                    ],
                    switch(operand(predicate), 5, 2),
                ),
                block(
                    118,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(
                    119,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
                ),
                block(
                    120,
                    vec![increment()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(121, vec![], SemanticTerminatorKindV1::Return),
            ],
            InductionCfgShape::Branched => vec![
                block(
                    120,
                    vec![initialize()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(121, vec![compare()], switch(operand(predicate), 6, 2)),
                block(122, vec![], switch(operand(body_predicate), 3, 4)),
                block(
                    123,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 5)),
                ),
                block(
                    124,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 5)),
                ),
                block(
                    125,
                    vec![increment()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(126, vec![], SemanticTerminatorKindV1::Return),
            ],
            InductionCfgShape::TwoLatches => vec![
                block(
                    130,
                    vec![initialize()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(131, vec![compare()], switch(operand(predicate), 5, 2)),
                block(132, vec![], switch(operand(body_predicate), 3, 4)),
                block(
                    133,
                    vec![increment()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(
                    134,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(135, vec![], SemanticTerminatorKindV1::Return),
            ],
            InductionCfgShape::ExtraExit => vec![
                block(
                    140,
                    vec![initialize()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(141, vec![compare()], switch(operand(predicate), 4, 2)),
                block(142, vec![], switch(operand(body_predicate), 5, 3)),
                block(
                    143,
                    vec![increment()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(144, vec![], SemanticTerminatorKindV1::Return),
                block(145, vec![], SemanticTerminatorKindV1::Return),
            ],
            InductionCfgShape::AnalysisSplit => vec![
                block(
                    150,
                    vec![initialize()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(151, vec![compare()], switch(operand(predicate), 5, 2)),
                block(152, vec![], switch(operand(body_predicate), 3, 3)),
                block(
                    153,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
                ),
                block(
                    154,
                    vec![increment()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(155, vec![], SemanticTerminatorKindV1::Return),
            ],
            InductionCfgShape::IrreducibleEntry => vec![
                block(160, vec![], switch(operand(body_predicate), 1, 3)),
                block(
                    161,
                    vec![initialize()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
                ),
                block(162, vec![compare()], switch(operand(predicate), 5, 3)),
                block(
                    163,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 4)),
                ),
                block(
                    164,
                    vec![increment()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 2)),
                ),
                block(165, vec![], SemanticTerminatorKindV1::Return),
            ],
            InductionCfgShape::MultiplePreheaders => vec![
                block(170, vec![], switch(operand(body_predicate), 1, 2)),
                block(
                    171,
                    vec![initialize()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(
                    172,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(173, vec![compare()], switch(operand(predicate), 6, 4)),
                block(
                    174,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 5)),
                ),
                block(
                    175,
                    vec![increment()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(176, vec![], SemanticTerminatorKindV1::Return),
            ],
            InductionCfgShape::ReusedComparisonLocal => vec![
                block(
                    180,
                    vec![
                        initialize(),
                        assign(predicate, SemanticRvalueKindV1::Use(constant(0))),
                    ],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(181, vec![compare()], switch(operand(predicate), 4, 2)),
                block(
                    182,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(
                    183,
                    vec![increment()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(184, vec![], SemanticTerminatorKindV1::Return),
            ],
            InductionCfgShape::MultipleHeaderComparisons => vec![
                block(
                    190,
                    vec![initialize()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(
                    191,
                    vec![compare(), compare()],
                    switch(operand(predicate), 4, 2),
                ),
                block(
                    192,
                    vec![],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(
                    193,
                    vec![increment()],
                    SemanticTerminatorKindV1::Goto(cfg_edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(194, vec![], SemanticTerminatorKindV1::Return),
            ],
        };
        projection_function_with_locals(
            blocks,
            vec![
                local(100, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(101, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(102, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(103, SCALAR_TYPE, bound_role),
                local(104, SCALAR_TYPE, SemanticLocalRoleV1::Argument(1)),
                local(105, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    fn project_test_inductions(
        function: &SemanticFunctionDeclV1,
    ) -> Result<
        (
            Vec<ProjectedUniformInductionV1>,
            Vec<ProductionRankedOperationV1>,
            usize,
        ),
        ProductionRankedProjectionErrorV1,
    > {
        let constants = constant_locals(function);
        let origins = local_stable_argument_origins(function).unwrap();
        let definitions = local_definition_counts(function);
        let mut arguments = vec![None; function.locals().len()];
        let mut next_argument = 1;
        let mut operations = Vec::new();
        let mut next_value = 0;
        let inductions = project_uniform_inductions_v1(
            &projection_types(),
            function,
            &constants,
            &origins,
            &definitions,
            &mut arguments,
            &mut next_argument,
            &mut operations,
            &mut next_value,
        )?;
        Ok((inductions, operations, next_argument))
    }

    fn assert_incomplete<T>(
        result: Result<T, ProductionRankedProjectionErrorV1>,
        expected: &'static str,
    ) {
        match result {
            Err(ProductionRankedProjectionErrorV1::Incomplete(actual)) => {
                assert_eq!(actual, expected);
            }
            Err(other) => panic!("expected incomplete projection, got {other}"),
            Ok(_) => panic!("expected incomplete projection"),
        }
    }

    fn assert_loop_unsupported<T>(
        result: Result<T, ProductionRankedProjectionErrorV1>,
        expected: &'static str,
    ) {
        match result {
            Err(ProductionRankedProjectionErrorV1::Unsupported(actual)) => {
                assert_eq!(actual, expected);
            }
            Err(other) => panic!("expected unsupported projection, got {other}"),
            Ok(_) => panic!("expected unsupported projection"),
        }
    }

    #[test]
    fn dynamic_parameter_induction_projects_to_legal_ranked_ssa_edges() {
        let function = uniform_induction_function(SemanticLocalRoleV1::Argument(0));
        let constants = constant_locals(&function);
        let origins = local_stable_argument_origins(&function).unwrap();
        let definitions = local_definition_counts(&function);
        let mut arguments = vec![None; function.locals().len()];
        let mut next_argument = 1;
        let mut entry_operations = Vec::new();
        let mut next_value = 0;
        let inductions = project_uniform_inductions_v1(
            &projection_types(),
            &function,
            &constants,
            &origins,
            &definitions,
            &mut arguments,
            &mut next_argument,
            &mut entry_operations,
            &mut next_value,
        )
        .unwrap();
        assert_eq!(inductions.len(), 1);

        let (blocks, _) = build_ranked_cfg(
            &projection_types(),
            &function,
            &vec![None; function.locals().len()],
            &vec![None; function.blocks().len()],
            &inductions,
            entry_operations,
            (0..function.blocks().len())
                .map(|_| ProjectedSemanticBlockV1 { items: vec![] })
                .collect(),
        )
        .unwrap();
        assert_eq!(blocks[2].index_argument_count(), 1);
        assert_eq!(blocks[3].index_argument_count(), 1);
        assert!(matches!(
            blocks[2].terminator(),
            ProductionRankedTerminatorV1::IndexLessThanArgs { .. }
        ));
        assert!(matches!(
            blocks[3].terminator(),
            ProductionRankedTerminatorV1::BranchArgsAddAt { .. }
        ));
        ProductionRankedKernelV1::new("uniform_dynamic_loop", next_argument, blocks).unwrap();
    }

    #[test]
    fn multi_block_induction_threads_ssa_and_preserves_body_effects() {
        let function = multi_block_induction_function(
            InductionCfgShape::Chain,
            SemanticLocalRoleV1::Argument(0),
            16,
        );
        let (inductions, entry_operations, next_argument) =
            project_test_inductions(&function).unwrap();
        assert_eq!(inductions[0].loop_blocks, vec![1, 2, 3, 4]);
        let barrier = ProductionRankedOperationV1::Barrier {
            execution_scope: HierarchyAttr::Workgroup,
            memory_scope: MemoryScopeAttr::Workgroup,
            address_space: AddressSpaceAttr::Workgroup,
            order: MemoryOrderAttr::AcquireRelease,
        };
        let mut projected = (0..function.blocks().len())
            .map(|_| ProjectedSemanticBlockV1 { items: vec![] })
            .collect::<Vec<_>>();
        projected[2].items.push(ProjectedBlockItemV1::Effect {
            operation: barrier.clone(),
            source: None,
        });
        let (blocks, _) = build_ranked_cfg(
            &projection_types(),
            &function,
            &vec![None; function.locals().len()],
            &vec![None; function.blocks().len()],
            &inductions,
            entry_operations,
            projected,
        )
        .unwrap();
        for block in &blocks[2..=5] {
            assert_eq!(block.index_argument_count(), 1);
        }
        assert_eq!(blocks[3].operations(), [barrier]);
        assert!(matches!(
            blocks[3].terminator(),
            ProductionRankedTerminatorV1::BranchArgs { arguments, .. }
                if arguments.len() == 1
        ));
        assert!(matches!(
            blocks[4].terminator(),
            ProductionRankedTerminatorV1::BranchArgs { arguments, .. }
                if arguments.len() == 1
        ));
        assert!(matches!(
            blocks[5].terminator(),
            ProductionRankedTerminatorV1::BranchArgsAddAt { .. }
        ));
        ProductionRankedKernelV1::new("multi_block_uniform_loop", next_argument, blocks).unwrap();
    }

    #[test]
    fn uniform_induction_resolves_an_exact_header_copy_alias() {
        let function = multi_block_induction_function(
            InductionCfgShape::HeaderCopyAlias,
            SemanticLocalRoleV1::Argument(0),
            16,
        );
        let (inductions, entry_operations, next_argument) =
            project_test_inductions(&function).unwrap();
        assert_eq!(inductions.len(), 1);
        assert_eq!(inductions[0].loop_blocks, vec![1, 2, 3, 4]);

        let (blocks, _) = build_ranked_cfg(
            &projection_types(),
            &function,
            &vec![None; function.locals().len()],
            &vec![None; function.blocks().len()],
            &inductions,
            entry_operations,
            (0..function.blocks().len())
                .map(|_| ProjectedSemanticBlockV1 { items: vec![] })
                .collect(),
        )
        .unwrap();
        ProductionRankedKernelV1::new("header_copy_alias_loop", next_argument, blocks).unwrap();
    }

    #[test]
    fn internal_uniform_predicate_forwards_exact_ssa_arguments() {
        let function = multi_block_induction_function(
            InductionCfgShape::Branched,
            SemanticLocalRoleV1::Argument(0),
            16,
        );
        let (inductions, entry_operations, next_argument) =
            project_test_inductions(&function).unwrap();
        let predicate = GuardPredicateV1 {
            comparisons: vec![(
                ProductionRankedValueV1::Argument(0),
                ProductionRankedValueV1::Argument(1),
            )],
        };
        let mut predicates = vec![None; function.locals().len()];
        predicates[4] = Some(predicate);
        let (blocks, _) = build_ranked_cfg(
            &projection_types(),
            &function,
            &predicates,
            &vec![None; function.blocks().len()],
            &inductions,
            entry_operations,
            (0..function.blocks().len())
                .map(|_| ProjectedSemanticBlockV1 { items: vec![] })
                .collect(),
        )
        .unwrap();
        assert!(matches!(
            blocks[3].terminator(),
            ProductionRankedTerminatorV1::IndexLessThanArgs {
                true_arguments,
                false_arguments,
                ..
            } if true_arguments.len() == 1 && false_arguments.len() == 1
        ));
        ProductionRankedKernelV1::new("branched_uniform_loop", next_argument, blocks).unwrap();
    }

    #[test]
    fn internal_deterministic_switch_forwards_exact_ssa_arguments() {
        let function = multi_block_induction_function(
            InductionCfgShape::Branched,
            SemanticLocalRoleV1::Argument(0),
            16,
        );
        let (inductions, entry_operations, next_argument) =
            project_test_inductions(&function).unwrap();
        let zero = entry_operations
            .iter()
            .find_map(|operation| match operation {
                ProductionRankedOperationV1::IndexConstant { result, value: 0 } => {
                    Some(ProductionRankedValueV1::Local(*result))
                }
                _ => None,
            })
            .expect("the induction initializer must materialize zero");
        let mut deterministic_switches = vec![None; function.blocks().len()];
        deterministic_switches[2] = Some(ProjectedDeterministicSwitchV1 {
            discriminant: ProductionRankedValueV1::Argument(1),
            targets: vec![(zero, 3)],
            otherwise: 4,
        });
        let (blocks, _) = build_ranked_cfg(
            &projection_types(),
            &function,
            &vec![None; function.locals().len()],
            &deterministic_switches,
            &inductions,
            entry_operations,
            (0..function.blocks().len())
                .map(|_| ProjectedSemanticBlockV1 { items: vec![] })
                .collect(),
        )
        .unwrap();
        assert!(matches!(
            blocks[3].terminator(),
            ProductionRankedTerminatorV1::IndexEqualArgs {
                true_arguments,
                false_arguments,
                ..
            } if true_arguments.len() == 1 && false_arguments.len() == 1
        ));
        ProductionRankedKernelV1::new("deterministic_uniform_loop", next_argument, blocks).unwrap();
    }

    #[test]
    fn malformed_induction_topologies_fail_closed() {
        for (shape, message) in [
            (
                InductionCfgShape::TwoLatches,
                "a uniform induction without one unique dominated backedge",
            ),
            (
                InductionCfgShape::ExtraExit,
                "a uniform induction region does not have one unique header exit",
            ),
            (
                InductionCfgShape::IrreducibleEntry,
                "a uniform induction without one unique dominated backedge",
            ),
            (
                InductionCfgShape::MultiplePreheaders,
                "a uniform induction without one unique preheader",
            ),
        ] {
            let function =
                multi_block_induction_function(shape, SemanticLocalRoleV1::Argument(0), 16);
            match project_test_inductions(&function) {
                Ok((inductions, _, _)) => assert!(
                    inductions.is_empty(),
                    "a malformed loop must not mint an induction: {shape:?}"
                ),
                Err(ProductionRankedProjectionErrorV1::Incomplete(actual)) => {
                    assert_eq!(actual, message)
                }
                Err(other) => panic!("expected incomplete projection, got {other}"),
            }
        }
    }

    #[test]
    fn non_positive_induction_step_fails_closed() {
        let function = multi_block_induction_function(
            InductionCfgShape::Chain,
            SemanticLocalRoleV1::Argument(0),
            0,
        );
        assert_incomplete(
            project_test_inductions(&function),
            "a uniform induction whose positive step is not statically established",
        );
    }

    #[test]
    fn varying_initial_and_nonconstant_step_fail_closed() {
        let varying = SemanticOperandV1::Copy(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(5), vec![], SCALAR_TYPE).unwrap(),
        );
        let varying_initial = multi_block_induction_function_with_operands(
            InductionCfgShape::Chain,
            SemanticLocalRoleV1::Argument(0),
            varying.clone(),
            constant(16),
        );
        assert_incomplete(
            project_test_inductions(&varying_initial),
            "a uniform induction with a lane-varying initial value",
        );

        let varying_step = multi_block_induction_function_with_operands(
            InductionCfgShape::Chain,
            SemanticLocalRoleV1::Argument(0),
            constant(0),
            varying,
        );
        assert_incomplete(
            project_test_inductions(&varying_step),
            "a uniform induction whose positive step is not statically established",
        );
    }

    #[test]
    fn loop_comparison_requires_one_exact_header_definition() {
        let reused = multi_block_induction_function(
            InductionCfgShape::ReusedComparisonLocal,
            SemanticLocalRoleV1::Argument(0),
            16,
        );
        assert_eq!(project_test_inductions(&reused).unwrap().0.len(), 1);

        let ambiguous = multi_block_induction_function(
            InductionCfgShape::MultipleHeaderComparisons,
            SemanticLocalRoleV1::Argument(0),
            16,
        );
        assert_incomplete(
            project_test_inductions(&ambiguous),
            "a uniform induction comparison with multiple header definitions",
        );
    }

    #[test]
    fn signed_step_bits_cannot_mint_a_positive_induction_step() {
        let signed_type = SemanticTypeIdV1::from_index(3);
        let wide_unsigned_type = SemanticTypeIdV1::from_index(4);
        let mut types = projection_types();
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(5)),
            SemanticLayoutIdentityV1::from_sha256(bytes(5)),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 32,
            }),
        ));
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(6)),
            SemanticLayoutIdentityV1::from_sha256(bytes(6)),
            SemanticTypeLayoutV1::new(Some(16), 16).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 128,
            }),
        ));
        let signed_minus_one = SemanticOperandV1::Constant(SemanticConstantV1::new(
            signed_type,
            SemanticConstantValueV1::Scalar(
                SemanticScalarValueV1::new(u32::MAX.into(), 4).unwrap(),
            ),
        ));
        let wide_unsigned_one = SemanticOperandV1::Constant(SemanticConstantV1::new(
            wide_unsigned_type,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 16).unwrap()),
        ));

        assert_eq!(
            positive_unsigned_constant_operand_v1(&signed_minus_one, &[], &types),
            None
        );
        assert_eq!(
            positive_unsigned_constant_operand_v1(&wide_unsigned_one, &[], &types),
            None
        );
        assert_eq!(
            positive_unsigned_constant_operand_v1(&constant(u32::MAX.into()), &[], &types),
            Some(u32::MAX.into())
        );
    }

    #[test]
    fn body_control_threads_typed_induction_edges() {
        let function = multi_block_induction_function(
            InductionCfgShape::AnalysisSplit,
            SemanticLocalRoleV1::Argument(0),
            16,
        );
        let (inductions, entry_operations, _) = project_test_inductions(&function).unwrap();
        let (blocks, _) = build_ranked_cfg(
            &projection_types(),
            &function,
            &vec![None; function.locals().len()],
            &vec![None; function.blocks().len()],
            &inductions,
            entry_operations,
            (0..function.blocks().len())
                .map(|_| ProjectedSemanticBlockV1 { items: vec![] })
                .collect(),
        )
        .unwrap();
        assert!(blocks.iter().any(|block| matches!(
            block.terminator(),
            ProductionRankedTerminatorV1::AnalysisSplitArgs {
                first_arguments,
                second_arguments,
                ..
            } if first_arguments.len() == 1 && second_arguments.len() == 1
        )));
    }

    #[test]
    fn guarded_body_expansion_threads_ssa_and_terminates_failure() {
        let function = multi_block_induction_function(
            InductionCfgShape::Chain,
            SemanticLocalRoleV1::Argument(0),
            16,
        );
        let (inductions, entry_operations, _) = project_test_inductions(&function).unwrap();
        let mut projected = (0..function.blocks().len())
            .map(|_| ProjectedSemanticBlockV1 { items: vec![] })
            .collect::<Vec<_>>();
        projected[2]
            .items
            .push(ProjectedBlockItemV1::Guarded(GuardedRankedAccessV1 {
                view: ProductionRankedValueIdV1::new(0),
                indices: vec![ProductionRankedValueV1::Argument(0)],
                comparisons: vec![(
                    ProductionRankedValueV1::Argument(0),
                    ProductionRankedValueV1::Argument(1),
                )],
                access: AccessKindAttr::Read,
                memory_space: MemorySpaceAttr::Global,
                source: SemanticSourceProvenanceV1::unavailable(),
                semantic_site: None,
            }));
        let (blocks, _) = build_ranked_cfg(
            &projection_types(),
            &function,
            &vec![None; function.locals().len()],
            &vec![None; function.blocks().len()],
            &inductions,
            entry_operations,
            projected,
        )
        .unwrap();
        assert!(blocks.iter().any(|block| matches!(
            block.terminator(),
            ProductionRankedTerminatorV1::IndexLessThanArgs {
                true_arguments,
                false_arguments,
                ..
            } if true_arguments.len() == 1 && false_arguments.is_empty()
        )));
        assert!(
            blocks
                .iter()
                .any(|block| matches!(block.terminator(), ProductionRankedTerminatorV1::Trap))
        );
    }

    #[test]
    fn loop_graph_work_overflow_and_amplification_are_bounded() {
        let mut work = usize::MAX;
        assert_loop_unsupported(
            project_loop_graph_charge_v1(&mut work, 1),
            "uniform induction CFG analysis work overflow",
        );
        let mut work = MAX_PROJECTED_LOOP_GRAPH_WORK_V1;
        assert_loop_unsupported(
            project_loop_graph_charge_v1(&mut work, 1),
            "uniform induction CFG analysis exceeds its work limit",
        );
        assert!(MAX_PROJECTED_LOOP_GRAPH_WORK_V1.checked_add(1).is_some());
    }

    #[test]
    fn lane_derived_induction_bound_cannot_mint_uniform_control() {
        let function = uniform_induction_function(SemanticLocalRoleV1::Temporary);
        let constants = constant_locals(&function);
        let origins = local_stable_argument_origins(&function).unwrap();
        let definitions = local_definition_counts(&function);
        let mut arguments = vec![None; function.locals().len()];
        let mut next_argument = 1;
        let mut operations = Vec::new();
        let mut next_value = 0;
        assert_incomplete(
            project_uniform_inductions_v1(
                &projection_types(),
                &function,
                &constants,
                &origins,
                &definitions,
                &mut arguments,
                &mut next_argument,
                &mut operations,
                &mut next_value,
            ),
            "a uniform induction with a lane-varying bound",
        );
    }

    #[test]
    fn adversarial_tensor_state_growth_has_explicit_budgets() {
        assert!(MAX_PROJECTED_TENSOR_STATE_ENTRIES_V1 < HARD_MAX_LOCALS_V1 as usize * 2);
        assert!(MAX_PROJECTED_TENSOR_DATAFLOW_WORK_V1 < HARD_MAX_VALIDATION_WORK_V1 as usize);
        assert!(
            MAX_PROJECTED_TENSOR_STATE_ENTRIES_V1
                .checked_add(1)
                .is_some_and(|entries| entries > MAX_PROJECTED_TENSOR_STATE_ENTRIES_V1)
        );
        assert!(
            MAX_PROJECTED_TENSOR_DATAFLOW_WORK_V1
                .checked_add(1)
                .is_some_and(|work| work > MAX_PROJECTED_TENSOR_DATAFLOW_WORK_V1)
        );
    }
}
