//! Deterministic SSA-planning custody for admitted production semantic MIR.
//!
//! This stage does not rewrite the admitted semantic document and does not
//! claim that planning proves a Rust-to-SSA semantic refinement. It retains a
//! replayable, bounded plan for every semantic function while preserving the
//! exact source owner for the existing ranked proof lineage.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    error::Error,
    fmt,
};

use fe2o3_mir_model::{
    SemanticCallExpansionErrorV1, SemanticCallExpansionLimitsV1, SemanticCallExpansionV1,
    SemanticCallInstanceIdV1, SemanticExpandedLocalOriginV1, SemanticExpandedRootV1,
    SemanticExpandedStatementOriginV1, SemanticExpandedTerminatorOriginV1,
    SemanticOptionDominanceV1, SsaBlockIdV1, SsaBlockInputV1, SsaConstructionInputV1,
    SsaConstructionPlanV1, SsaEdgeInputV1, SsaEdgeRoleV1, SsaEventV1, SsaPlannerErrorV1,
    SsaPlannerLimitsV1, SsaPlannerResourceReportV1, SsaPlannerResourceV1, SsaVariableIdV1,
    plan_ssa_with_limits_v1,
    semantic_mir_v1::{
        AdmittedInertSemanticMirV1, SemanticAbiPassModeV1, SemanticAssertMessageV1,
        SemanticBackendReprV1, SemanticBlockIdV1, SemanticCallableDeclV1,
        SemanticCompilerIntrinsicOperationV1, SemanticControlFlowEdgeV1, SemanticEdgeRoleV1,
        SemanticFunctionDeclV1, SemanticFunctionIdV1, SemanticFunctionIdentityV1,
        SemanticLocalIdV1, SemanticLocalRoleV1, SemanticOperandV1, SemanticPlaceV1,
        SemanticProjectionKindV1, SemanticRvalueKindV1, SemanticStatementKindV1,
        SemanticTerminatorKindV1, SemanticTypeDeclV1, SemanticTypeIdV1,
        SemanticTypeLayoutDetailsV1, SemanticTypeShapeV1,
    },
    semantic_option_producers_v1,
};
use sha2::{Digest as _, Sha256};

use super::{ProductionSemanticMirErrorV1, ProductionSemanticMirOwnerV1};

const PRODUCTION_SEMANTIC_SSA_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.production-semantic-ssa-owner.v1\0";

/// Fixed compiler envelope for variables retained across one semantic module.
pub const HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_VARIABLES_V1: usize = 1_048_576;
/// Fixed compiler envelope for input blocks retained across one semantic module.
pub const HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_BLOCKS_V1: usize = 1_048_576;
/// Fixed compiler envelope for input edges retained across one semantic module.
pub const HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EDGES_V1: usize = 262_144;
/// Fixed compiler envelope for input events retained across one semantic module.
pub const HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EVENTS_V1: usize = 4_194_304;
/// Fixed compiler envelope for edge definitions retained across one semantic module.
pub const HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EDGE_DEFINITIONS_V1: usize = 262_144;
/// Fixed compiler envelope for output items retained across one semantic module.
pub const HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_OUTPUT_ITEMS_V1: usize = 4_194_304;
/// Fixed compiler envelope for logical storage words across one semantic module.
pub const HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_STORAGE_WORDS_V1: usize = 8_388_608;
/// Fixed compiler envelope for deterministic work across one semantic module.
pub const HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_WORK_UNITS_V1: usize = 268_435_456;

/// A malformed or unbounded semantic-SSA module resource policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticSsaModuleLimitsErrorV1 {
    InvalidLimits,
}

impl fmt::Display for ProductionSemanticSsaModuleLimitsErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => {
                formatter.write_str("invalid production semantic SSA module limits")
            }
        }
    }
}

impl Error for ProductionSemanticSsaModuleLimitsErrorV1 {}

/// Fixed upper envelope for cumulative resources retained by one semantic module.
///
/// The hard maxima are four times the independent function-planner ceilings.
/// This is one constant compiler resource envelope, not a multiplier applied
/// to the module's function count. Individual functions remain subject to the
/// unchanged [`SsaPlannerLimitsV1`] policy before module accounting begins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticSsaModuleLimitsV1 {
    max_variables: usize,
    max_blocks: usize,
    max_edges: usize,
    max_events: usize,
    max_edge_definitions: usize,
    max_output_items: usize,
    max_storage_words: usize,
    max_work_units: usize,
}

impl ProductionSemanticSsaModuleLimitsV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        max_variables: usize,
        max_blocks: usize,
        max_edges: usize,
        max_events: usize,
        max_edge_definitions: usize,
        max_output_items: usize,
        max_storage_words: usize,
        max_work_units: usize,
    ) -> Result<Self, ProductionSemanticSsaModuleLimitsErrorV1> {
        let limits = Self {
            max_variables,
            max_blocks,
            max_edges,
            max_events,
            max_edge_definitions,
            max_output_items,
            max_storage_words,
            max_work_units,
        };
        if limits.max_variables > HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_VARIABLES_V1
            || limits.max_blocks == 0
            || limits.max_blocks > HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_BLOCKS_V1
            || limits.max_edges > HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EDGES_V1
            || limits.max_events > HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EVENTS_V1
            || limits.max_edge_definitions
                > HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EDGE_DEFINITIONS_V1
            || limits.max_output_items > HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_OUTPUT_ITEMS_V1
            || limits.max_storage_words == 0
            || limits.max_storage_words > HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_STORAGE_WORDS_V1
            || limits.max_work_units == 0
            || limits.max_work_units > HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_WORK_UNITS_V1
        {
            return Err(ProductionSemanticSsaModuleLimitsErrorV1::InvalidLimits);
        }
        Ok(limits)
    }

    pub const fn production() -> Self {
        Self {
            max_variables: HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_VARIABLES_V1,
            max_blocks: HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_BLOCKS_V1,
            max_edges: HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EDGES_V1,
            max_events: HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EVENTS_V1,
            max_edge_definitions: HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_EDGE_DEFINITIONS_V1,
            max_output_items: HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_OUTPUT_ITEMS_V1,
            max_storage_words: HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_STORAGE_WORDS_V1,
            max_work_units: HARD_MAX_PRODUCTION_SEMANTIC_SSA_MODULE_WORK_UNITS_V1,
        }
    }

    pub const fn max_variables(self) -> usize {
        self.max_variables
    }

    pub const fn max_blocks(self) -> usize {
        self.max_blocks
    }

    pub const fn max_edges(self) -> usize {
        self.max_edges
    }

    pub const fn max_events(self) -> usize {
        self.max_events
    }

    pub const fn max_edge_definitions(self) -> usize {
        self.max_edge_definitions
    }

    pub const fn max_output_items(self) -> usize {
        self.max_output_items
    }

    pub const fn max_storage_words(self) -> usize {
        self.max_storage_words
    }

    pub const fn max_work_units(self) -> usize {
        self.max_work_units
    }
}

impl Default for ProductionSemanticSsaModuleLimitsV1 {
    fn default() -> Self {
        Self::production()
    }
}

/// Bounded planner policy retained for deterministic replay.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProductionSemanticSsaLimitsV1 {
    planner: SsaPlannerLimitsV1,
    module: ProductionSemanticSsaModuleLimitsV1,
}

impl ProductionSemanticSsaLimitsV1 {
    pub const fn new(planner: SsaPlannerLimitsV1) -> Self {
        Self {
            planner,
            module: ProductionSemanticSsaModuleLimitsV1::production(),
        }
    }

    /// Retains a stricter validated module envelope without changing the
    /// independent per-function planner limits.
    pub const fn with_module_limits(
        planner: SsaPlannerLimitsV1,
        module: ProductionSemanticSsaModuleLimitsV1,
    ) -> Self {
        Self { planner, module }
    }

    pub const fn planner(self) -> SsaPlannerLimitsV1 {
        self.planner
    }

    pub const fn module(self) -> ProductionSemanticSsaModuleLimitsV1 {
        self.module
    }
}

/// Fail-closed errors from semantic-to-SSA planning or deterministic replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionSemanticSsaErrorV1 {
    SemanticOwner(ProductionSemanticMirErrorV1),
    CallExpansion(SemanticCallExpansionErrorV1),
    Planner {
        function: SemanticFunctionIdV1,
        error: SsaPlannerErrorV1,
    },
    /// The cause uses execution coordinates, not coordinates in the admitted source.
    /// Block tuples are (call instance, source function, source block). Missing
    /// locations remain absent; diagnostics never infer a source coordinate.
    ExpandedExecution {
        root: SemanticFunctionIdV1,
        execution_view_identity: [u8; 32],
        source_block: Option<(
            SemanticCallInstanceIdV1,
            SemanticFunctionIdV1,
            SemanticBlockIdV1,
        )>,
        source_target: Option<(
            SemanticCallInstanceIdV1,
            SemanticFunctionIdV1,
            SemanticBlockIdV1,
        )>,
        source_statement: Option<SemanticExpandedStatementOriginV1>,
        source_terminator: Option<SemanticExpandedTerminatorOriginV1>,
        source_local: Option<SemanticExpandedLocalOriginV1>,
        /// Bounded, inert source/frame detail for an undefined return transfer.
        /// This never supplies a definition or changes the underlying failure.
        return_transfer_diagnostic: Option<String>,
        error: Box<ProductionSemanticSsaErrorV1>,
    },
    /// Inert storage observation at the original rejecting gate.
    ResourceStage {
        stage: &'static str,
        auxiliary_storage_words: usize,
        plan_storage_words: Option<usize>,
        live_storage_words: usize,
        peak_storage_words: usize,
        requested_storage_words: usize,
        error: Box<ProductionSemanticSsaErrorV1>,
    },
    /// Inert attribution at the unchanged borrow-classifier work gate.
    BorrowFlowWork {
        stage: &'static str,
        phase_work_units: [usize; 11],
        ordered_proof_calls: usize,
        remaining_work_units: usize,
        requested_work_units: usize,
        error: Box<ProductionSemanticSsaErrorV1>,
    },
    ResourceOverflow,
    AggregateResourceLimit {
        resource: SsaPlannerResourceV1,
        required: usize,
        limit: usize,
    },
    PartialMove {
        function: SemanticFunctionIdV1,
        block: u32,
        statement: Option<u32>,
        local: u32,
        violation: SemanticPartialMoveViolationV1,
    },
    PartialMoveResourceLimit {
        function: SemanticFunctionIdV1,
        resource: SsaPlannerResourceV1,
        required: usize,
        limit: usize,
    },
    ReplayMismatch,
}

/// A fail-closed reason that a projected Rust move cannot be certified as an
/// SSA-only transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticPartialMoveViolationV1 {
    MissingTypeContext,
    UnsupportedProjection,
    UnionField,
    MaybeMovedValueUsed,
}

impl fmt::Display for SemanticPartialMoveViolationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingTypeContext => "projected move requires authenticated module type context",
            Self::UnsupportedProjection => {
                "projected move has an aliasing or dynamically selected path"
            }
            Self::UnionField => "projected move selects overlapping union storage",
            Self::MaybeMovedValueUsed => "value may already be partially or wholly moved",
        })
    }
}

impl fmt::Display for ProductionSemanticSsaErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemanticOwner(error) => {
                write!(
                    formatter,
                    "production semantic SSA source owner failed: {error}"
                )
            }
            Self::CallExpansion(error) => write!(
                formatter,
                "production semantic call expansion failed: {error}",
            ),
            Self::Planner { function, error } => write!(
                formatter,
                "production semantic SSA planning failed for function {}: {error}",
                function.index(),
            ),
            Self::ExpandedExecution {
                root,
                execution_view_identity,
                source_block,
                source_target,
                source_statement,
                source_terminator,
                source_local,
                return_transfer_diagnostic,
                error,
            } => {
                write!(
                    formatter,
                    "production semantic SSA execution root {} view ",
                    root.index()
                )?;
                for byte in execution_view_identity {
                    write!(formatter, "{byte:02x}")?;
                }
                for (label, origin) in [("source", source_block), ("target source", source_target)]
                {
                    if let Some((instance, function, block)) = origin {
                        write!(
                            formatter,
                            "; {label} instance {} function {} block {}",
                            instance.index(),
                            function.index(),
                            block.index()
                        )?;
                    }
                }
                if let Some(statement) = source_statement {
                    write!(formatter, " statement origin {statement:?}")?;
                }
                if let Some(terminator) = source_terminator {
                    write!(formatter, " terminator origin {terminator:?}")?;
                }
                if let Some(local) = source_local {
                    write!(
                        formatter,
                        "; local source instance {} function {} local {}",
                        local.instance().index(),
                        local.function().index(),
                        local.local().index()
                    )?;
                }
                write!(formatter, "; execution-coordinate cause: {error}")?;
                if let Some(diagnostic) = return_transfer_diagnostic {
                    write!(formatter, "; undefined-return detail: {diagnostic}")?;
                }
                Ok(())
            }
            Self::ResourceStage {
                stage,
                auxiliary_storage_words,
                plan_storage_words,
                live_storage_words,
                peak_storage_words,
                requested_storage_words,
                error,
            } => {
                write!(
                    formatter,
                    "{error}; storage-stage {stage} A={auxiliary_storage_words} P="
                )?;
                match plan_storage_words {
                    Some(words) => write!(formatter, "{words}")?,
                    None => formatter.write_str("not-planned")?,
                }
                write!(
                    formatter,
                    " live={live_storage_words} peak={peak_storage_words} requested={requested_storage_words}"
                )
            }
            Self::BorrowFlowWork {
                stage, phase_work_units: p, ordered_proof_calls,
                remaining_work_units, requested_work_units, error,
            } => write!(formatter,
                "{error}; borrow-flow-stage {stage} remaining={remaining_work_units} requested={requested_work_units} ordered-proofs={ordered_proof_calls} facts={} candidates={} uses={} components={} owner-roots={} cfg={} owner-changes={} loans={} paths={} descendants={} transfers={}",
                p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7], p[8], p[9], p[10],
            ),
            Self::ResourceOverflow => formatter
                .write_str("production semantic SSA aggregate resource accounting overflowed"),
            Self::AggregateResourceLimit {
                resource,
                required,
                limit,
            } => write!(
                formatter,
                "production semantic SSA aggregate {resource} {required} exceeds limit {limit}",
            ),
            Self::PartialMove {
                function,
                block,
                statement,
                local,
                violation,
            } => write!(
                formatter,
                "production semantic SSA partial-move validation failed for function {} block {block} {} local {local}: {violation}",
                function.index(),
                statement
                    .map(|statement| format!("statement {statement}"))
                    .unwrap_or_else(|| "terminator".to_owned()),
            ),
            Self::PartialMoveResourceLimit {
                function,
                resource,
                required,
                limit,
            } => write!(
                formatter,
                "production semantic SSA partial-move validation for function {} requires {required} {resource}, limit is {limit}",
                function.index(),
            ),
            Self::ReplayMismatch => formatter.write_str(
                "production semantic SSA replay changed its source, plans, resources, or identity",
            ),
        }
    }
}

impl Error for ProductionSemanticSsaErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SemanticOwner(error) => Some(error),
            Self::CallExpansion(error) => Some(error),
            Self::Planner { error, .. } => Some(error),
            Self::ExpandedExecution { error, .. }
            | Self::ResourceStage { error, .. }
            | Self::BorrowFlowWork { error, .. } => {
                Some(error.as_ref())
            }
            Self::ResourceOverflow
            | Self::AggregateResourceLimit { .. }
            | Self::PartialMove { .. }
            | Self::PartialMoveResourceLimit { .. }
            | Self::ReplayMismatch => None,
        }
    }
}

/// Bounded field-sensitive availability certificate for projected Rust moves.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProductionSemanticPartialMoveCertificateV1 {
    projected_moves: usize,
    state_entries: usize,
    work_units: usize,
}

impl ProductionSemanticPartialMoveCertificateV1 {
    pub const fn projected_moves(self) -> usize {
        self.projected_moves
    }

    /// Peak logical state-storage words, including live analysis workspace.
    pub const fn state_entries(self) -> usize {
        self.state_entries
    }

    pub const fn work_units(self) -> usize {
        self.work_units
    }
}

/// Identity of the unchanged source, checked execution views, and exact SSA plans.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionSemanticSsaIdentityV1([u8; 32]);

impl ProductionSemanticSsaIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Aggregate SSA work for original functions and additional execution views.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProductionSemanticSsaSummaryV1 {
    function_count: usize,
    promotable_variables: usize,
    memory_variables: usize,
    input_blocks: usize,
    reachable_blocks: usize,
    pruned_blocks: usize,
    input_edges: usize,
    input_events: usize,
    input_edge_definitions: usize,
    generated_definitions: usize,
    output_items: usize,
    storage_words: usize,
    work_units: usize,
}

impl ProductionSemanticSsaSummaryV1 {
    pub const fn function_count(self) -> usize {
        self.function_count
    }

    pub const fn promotable_variables(self) -> usize {
        self.promotable_variables
    }

    pub const fn memory_variables(self) -> usize {
        self.memory_variables
    }

    pub const fn input_blocks(self) -> usize {
        self.input_blocks
    }

    pub const fn reachable_blocks(self) -> usize {
        self.reachable_blocks
    }

    pub const fn pruned_blocks(self) -> usize {
        self.pruned_blocks
    }

    pub const fn input_edges(self) -> usize {
        self.input_edges
    }

    pub const fn input_events(self) -> usize {
        self.input_events
    }

    pub const fn input_edge_definitions(self) -> usize {
        self.input_edge_definitions
    }

    pub const fn generated_definitions(self) -> usize {
        self.generated_definitions
    }

    pub const fn output_items(self) -> usize {
        self.output_items
    }

    pub const fn storage_words(self) -> usize {
        self.storage_words
    }

    pub const fn work_units(self) -> usize {
        self.work_units
    }
}

/// One function's exact planner result, bound to its semantic identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionSemanticSsaFunctionPlanV1 {
    function: SemanticFunctionIdV1,
    function_identity: SemanticFunctionIdentityV1,
    plan: SsaConstructionPlanV1,
    partial_moves: ProductionSemanticPartialMoveCertificateV1,
    implicit_entry_variables: Box<[SsaVariableIdV1]>,
    frame_initializations: frame_initialization::FrameInitializationsV1,
    defined_math_results: defined_math_results::DefinedMathResultsV1,
    defined_matrix_results: defined_matrix_results::DefinedMatrixResultsV1,
    defined_reusable_lds_results: defined_reusable_lds_results::DefinedReusableLdsResultsV1,
    guarded_grid_results: guarded_grid_results::GuardedGridResultsV1,
    defined_reusable_phase_results: defined_reusable_phase_results::DefinedReusablePhaseResultsV1,
    retained_cross_edge_variables: Box<[SsaVariableIdV1]>,
    event_origins: execution::ExecutionEventOriginsV1,
    value_origins: source_uses_v1::definitions_v1::ValueOriginsV1,
    auxiliary_resources: SemanticSsaAuxiliaryResourcesV1,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct SemanticSsaAuxiliaryResourcesV1 {
    storage_words: usize,
    work_units: usize,
}

impl ProductionSemanticSsaFunctionPlanV1 {
    pub const fn function(&self) -> SemanticFunctionIdV1 {
        self.function
    }

    pub const fn function_identity(&self) -> SemanticFunctionIdentityV1 {
        self.function_identity
    }

    pub const fn plan(&self) -> &SsaConstructionPlanV1 {
        &self.plan
    }

    pub const fn resources(&self) -> &SsaPlannerResourceReportV1 {
        self.plan.resources()
    }

    pub const fn partial_move_certificate(&self) -> ProductionSemanticPartialMoveCertificateV1 {
        self.partial_moves
    }

    /// Returns compiler-authenticated, zero-sized capability locals whose
    /// Rust MIR producer was elided and which therefore enter SSA explicitly.
    pub fn implicit_entry_variables(&self) -> &[SsaVariableIdV1] {
        &self.implicit_entry_variables
    }

    /// Ambient source certificates instantiated after exact frame-entry storage kills.
    /// These are not function-entry definitions; lowering must honor each recorded site.
    pub fn frame_initializations(&self) -> &[ProductionSemanticSsaFrameInitializationV1] {
        self.frame_initializations.entries()
    }

    /// Closed getter/bridge results defined at their original return transfers.
    /// These are structural SSA definitions, not branded Math issuers or proofs.
    pub fn defined_matrix_results(&self) -> &[ProductionSemanticMatrixBridgeResultV1] {
        self.defined_matrix_results.entries()
    }
    pub fn guarded_grid_leader_results(&self) -> &[ProductionGuardedGridLeaderResultV1] {
        self.guarded_grid_results.entries()
    }
    pub fn defined_reusable_lds_results(&self) -> &[ProductionSemanticReusableLdsResultV1] {
        self.defined_reusable_lds_results.entries()
    }

    pub fn defined_reusable_phase_results(&self) -> &[ProductionSemanticPhaseResultV1] {
        self.defined_reusable_phase_results.results()
    }
    pub fn defined_reusable_phase_relays(&self) -> &[ProductionSemanticPhaseRelayV1] {
        self.defined_reusable_phase_results.relays()
    }
    pub fn defined_math_results(&self) -> &[ProductionSemanticMathBridgeResultV1] {
        self.defined_math_results.entries()
    }

    /// Returns storage-retained locals that need state across a reachable CFG
    /// edge. KIR lowering must either materialize these locals or reject them.
    pub fn retained_cross_edge_variables(&self) -> &[SsaVariableIdV1] {
        &self.retained_cross_edge_variables
    }
}

/// Move-only custody of original semantic MIR and replayable SSA plans for its
/// source functions and checked call-expanded execution views.
///
/// This owner grants no proof, compiler-artifact, publication, load, launch,
/// or execution authority. The planner records structural SSA placement only.
///
/// ```compile_fail
/// use fe2o3_pliron::ProductionSemanticSsaOwnerV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionSemanticSsaOwnerV1>();
/// ```
#[must_use = "dropping the semantic SSA owner abandons its production custody"]
pub struct ProductionSemanticSsaOwnerV1 {
    source_owner: ProductionSemanticMirOwnerV1,
    source_semantic_sha256: [u8; 32],
    limits: ProductionSemanticSsaLimitsV1,
    source_plans: Box<[ProductionSemanticSsaFunctionPlanV1]>,
    source_summary: ProductionSemanticSsaSummaryV1,
    source_identity: ProductionSemanticSsaIdentityV1,
    execution: execution::ExecutionSsaPlansV1,
}

impl fmt::Debug for ProductionSemanticSsaOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionSemanticSsaOwnerV1")
            .field("source_semantic_sha256", &self.source_semantic_sha256)
            .field("summary", &self.summary())
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}

impl ProductionSemanticSsaOwnerV1 {
    pub fn try_new(
        source_owner: ProductionSemanticMirOwnerV1,
        limits: ProductionSemanticSsaLimitsV1,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        source_owner
            .verify_equivalence()
            .map_err(ProductionSemanticSsaErrorV1::SemanticOwner)?;
        let source_semantic_sha256 = *source_owner.semantic().semantic_sha256().as_bytes();
        let (plans, summary, identity) =
            construct_semantic_ssa_plans_v1(source_owner.semantic(), limits)?;
        let execution = execution::ExecutionSsaPlansV1::try_new(
            source_owner.semantic(),
            &plans,
            limits,
            summary,
            identity,
        )?;
        Ok(Self {
            source_owner,
            source_semantic_sha256,
            limits,
            source_plans: plans,
            source_summary: summary,
            source_identity: identity,
            execution,
        })
    }

    /// Reconstructs every semantic adapter input and requires exact planner
    /// replay before custody may advance.
    pub fn verify_replay(&self) -> Result<(), ProductionSemanticSsaErrorV1> {
        self.source_owner
            .verify_equivalence()
            .map_err(ProductionSemanticSsaErrorV1::SemanticOwner)?;
        if self.source_semantic_sha256 != *self.source_semantic().semantic_sha256().as_bytes() {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        let (plans, summary, identity) =
            construct_semantic_ssa_plans_v1(self.source_semantic(), self.limits)?;
        if plans != self.source_plans
            || summary != self.source_summary
            || identity != self.source_identity
        {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        self.execution.verify_replay(
            self.source_semantic(),
            &plans,
            self.limits,
            summary,
            identity,
        )?;
        Ok(())
    }

    pub const fn source_owner(&self) -> &ProductionSemanticMirOwnerV1 {
        &self.source_owner
    }

    pub const fn source_semantic(&self) -> &AdmittedInertSemanticMirV1 {
        self.source_owner.semantic()
    }

    pub const fn source_semantic_sha256(&self) -> &[u8; 32] {
        &self.source_semantic_sha256
    }

    /// Original-coordinate plans; execution consumers use [`Self::execution_plan_for_root`].
    pub fn plans(&self) -> &[ProductionSemanticSsaFunctionPlanV1] {
        &self.source_plans
    }

    /// Borrows the plan bound to one exact semantic function identity.
    pub fn plan_for_function(
        &self,
        function: SemanticFunctionIdV1,
    ) -> Option<&ProductionSemanticSsaFunctionPlanV1> {
        self.source_plans
            .get(function.index() as usize)
            .filter(|plan| plan.function == function)
    }

    pub const fn summary(&self) -> ProductionSemanticSsaSummaryV1 {
        self.execution.summary()
    }

    pub const fn identity(&self) -> ProductionSemanticSsaIdentityV1 {
        self.execution.identity()
    }

    /// Borrows the replayed source-to-execution relation without replacing source custody.
    pub const fn execution_expansion(&self) -> &SemanticCallExpansionV1 {
        self.execution.expansion()
    }

    pub fn execution_view_for_root(
        &self,
        root: SemanticFunctionIdV1,
    ) -> Option<&SemanticExpandedRootV1> {
        self.execution.expansion().root(root)
    }

    /// Returns a plan in the selected execution view's coordinate space.
    pub fn execution_plan_for_root(
        &self,
        root: SemanticFunctionIdV1,
    ) -> Option<&ProductionSemanticSsaFunctionPlanV1> {
        let view = self.execution_view_for_root(root)?;
        if view.has_expanded_calls() {
            self.execution.plan(root)
        } else {
            self.plan_for_function(view.source_body())
        }
    }

    pub const fn grants_proof_or_artifact_authority(&self) -> bool {
        false
    }

    pub fn into_source_owner(
        self,
    ) -> Result<ProductionSemanticMirOwnerV1, ProductionSemanticSsaErrorV1> {
        self.verify_replay()?;
        Ok(self.source_owner)
    }
}

fn construct_semantic_ssa_plans_v1(
    semantic: &AdmittedInertSemanticMirV1,
    limits: ProductionSemanticSsaLimitsV1,
) -> Result<
    (
        Box<[ProductionSemanticSsaFunctionPlanV1]>,
        ProductionSemanticSsaSummaryV1,
        ProductionSemanticSsaIdentityV1,
    ),
    ProductionSemanticSsaErrorV1,
> {
    let mut plans = Vec::with_capacity(semantic.functions().len());
    let mut summary = ProductionSemanticSsaSummaryV1 {
        function_count: semantic.functions().len(),
        ..ProductionSemanticSsaSummaryV1::default()
    };
    for (function_index, function) in semantic.functions().iter().enumerate() {
        let function_id = SemanticFunctionIdV1::from_index(function_index as u32);
        let transparent_borrows = adapter::typed_transparent_borrow_sites_v1(function, semantic.types(), semantic.callables());
        let function_plan = plan_semantic_function_ssa_with_borrow_sites_v1(
            function_id,
            function,
            Some(semantic.types()),
            semantic.callables(),
            limits,
            &transparent_borrows,
            None,
        )
        .map_err(|error| {
            source_partial_move_diagnostic_v1::emit(semantic, function_id, error)
        })?;
        accumulate_summary_v1(
            &mut summary,
            &function_plan,
            function.locals().len(),
            limits,
        )?;
        plans.push(function_plan);
    }
    let plans = plans.into_boxed_slice();
    let identity =
        derive_semantic_ssa_identity_v1(semantic.semantic_sha256().as_bytes(), &plans, summary);
    Ok((plans, summary, identity))
}

/// Constructs the canonical bounded SSA plan for one admitted semantic function.
///
/// Module owners should normally use [`ProductionSemanticSsaOwnerV1`]. This
/// entry point exists for lowering components that operate on an isolated
/// function while retaining the same adapter and planner semantics.
pub fn plan_semantic_function_ssa_v1(
    function_id: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
    limits: ProductionSemanticSsaLimitsV1,
) -> Result<ProductionSemanticSsaFunctionPlanV1, ProductionSemanticSsaErrorV1> {
    plan_semantic_function_ssa_with_borrow_sites_v1(
        function_id,
        function,
        None,
        &[],
        limits,
        &BTreeSet::new(),
        None,
    )
}

/// Constructs an isolated function plan with module-authenticated compiler
/// capability types whose Rust borrows carry authority rather than addresses.
pub fn plan_semantic_function_ssa_with_callables_v1(
    function_id: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    limits: ProductionSemanticSsaLimitsV1,
) -> Result<ProductionSemanticSsaFunctionPlanV1, ProductionSemanticSsaErrorV1> {
    let transparent_borrows = transparent_borrow_sites_v1(function, callables);
    plan_semantic_function_ssa_with_borrow_sites_v1(
        function_id,
        function,
        None,
        callables,
        limits,
        &transparent_borrows,
        None,
    )
}

/// Constructs an isolated function plan with the authenticated module type and
/// callable tables required to certify field-sensitive Rust moves.
pub fn plan_semantic_function_ssa_with_module_v1(
    function_id: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    limits: ProductionSemanticSsaLimitsV1,
) -> Result<ProductionSemanticSsaFunctionPlanV1, ProductionSemanticSsaErrorV1> {
    let transparent_borrows = adapter::typed_transparent_borrow_sites_v1(function, types, callables);
    plan_semantic_function_ssa_with_borrow_sites_v1(
        function_id,
        function,
        Some(types),
        callables,
        limits,
        &transparent_borrows,
        None,
    )
}

fn plan_semantic_function_ssa_with_borrow_sites_v1(
    function_id: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    callables: &[SemanticCallableDeclV1],
    limits: ProductionSemanticSsaLimitsV1,
    transparent_borrows: &BTreeSet<SemanticTransparentBorrowSiteV1>,
    execution_view: Option<(
        &SemanticExpandedRootV1,
        frame_initialization::FrameInitializationsV1,
        defined_math_results::DefinedMathResultsV1,
        defined_matrix_results::DefinedMatrixResultsV1,
        defined_reusable_lds_results::DefinedReusableLdsResultsV1,
        defined_reusable_phase_results::DefinedReusablePhaseResultsV1,
        guarded_grid_results::GuardedGridResultsV1,
    )>,
) -> Result<ProductionSemanticSsaFunctionPlanV1, ProductionSemanticSsaErrorV1> {
    let mut event_origins = execution::ExecutionEventOriginsV1::default();
    let (view, frame_initializations, defined_math_results, defined_matrix_results, defined_reusable_lds_results, defined_reusable_phase_results, guarded_grid_results) = match execution_view {
        Some((view, frames, math, matrix, reusable, phase, grid)) => (Some(view), frames, math, matrix, reusable, phase, grid),
        None => (
            None,
            frame_initialization::FrameInitializationsV1::default(),
            defined_math_results::DefinedMathResultsV1::default(),
            defined_matrix_results::DefinedMatrixResultsV1::default(),
            defined_reusable_lds_results::DefinedReusableLdsResultsV1::default(),
            defined_reusable_phase_results::DefinedReusablePhaseResultsV1::default(),
            guarded_grid_results::GuardedGridResultsV1::default(),
        ),
    };
    event_origins = execution::ExecutionEventOriginsV1::for_function(function_id, function, limits)
        .map_err(|error| match view {
            Some(view) => execution::wrap_error(view, &event_origins, error),
            None => error,
        })?;
    let (input, implicit_entry_variables, adapter_analysis_work) = if let Some(view) = view {
        frame_initializations
            .verify_view(view)
            .map_err(|error| execution::wrap_error(view, &event_origins, error))?;
        defined_math_results
            .verify_view(view)
            .map_err(|error| execution::wrap_error(view, &event_origins, error))?;
        defined_matrix_results.verify_view(view)
            .map_err(|error| execution::wrap_error(view, &event_origins, error))?;
        guarded_grid_results.verify_view(view)
            .map_err(|error| execution::wrap_error(view, &event_origins, error))?;
        defined_reusable_lds_results.verify_view(view)
            .map_err(|error| execution::wrap_error(view, &event_origins, error))?;
        defined_reusable_phase_results.verify_view(view)
            .map_err(|error| execution::wrap_error(view, &event_origins, error))?;
        adapter::semantic_function_ssa_input_with_defined_results_v1(
            function,
            types,
            callables,
            transparent_borrows,
            &frame_initializations,
            &defined_math_results,
            &defined_matrix_results,
            &defined_reusable_lds_results,
            &defined_reusable_phase_results,
            &guarded_grid_results,
            |block, statement, events| {
                event_origins.record(block, statement, events);
            },
        )
        .map_err(|error| execution::wrap_error(view, &event_origins, error))?
    } else {
        adapter::semantic_function_ssa_input_with_event_origins_v1(
            function, types, callables, transparent_borrows,
            |block, statement, events| event_origins.record(block, statement, events),
        )
    };
    let result = (|| {
        let mut auxiliary_resources = semantic_ssa_auxiliary_resources_v1(function, &input)?;
        let diagnostic_resources = event_origins.resources()?;
        let initialization_resources = frame_initializations.resources();
        let result_resources = defined_math_results.resources();
        let matrix_resources = defined_matrix_results.resources();
        let reusable_resources = defined_reusable_lds_results.resources();
        let guarded_grid_resources = guarded_grid_results.resources();
        let phase_resources = defined_reusable_phase_results.resources();
        auxiliary_resources.storage_words = auxiliary_resources
            .storage_words
            .checked_add(diagnostic_resources.storage_words)
            .and_then(|words| words.checked_add(initialization_resources.storage_words))
            .and_then(|words| words.checked_add(result_resources.storage_words))
            .and_then(|words| words.checked_add(matrix_resources.storage_words))
            .and_then(|words| words.checked_add(reusable_resources.storage_words))
            .and_then(|words| words.checked_add(guarded_grid_resources.storage_words))
            .and_then(|words| words.checked_add(phase_resources.storage_words))
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        auxiliary_resources.work_units = auxiliary_resources
            .work_units
            .checked_add(adapter_analysis_work)
            .and_then(|work| work.checked_add(diagnostic_resources.work_units))
            .and_then(|work| work.checked_add(initialization_resources.work_units))
            .and_then(|work| work.checked_add(result_resources.work_units))
            .and_then(|work| work.checked_add(matrix_resources.work_units))
            .and_then(|work| work.checked_add(reusable_resources.work_units))
            .and_then(|work| work.checked_add(guarded_grid_resources.work_units))
            .and_then(|work| work.checked_add(phase_resources.work_units))
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        enforce_function_resource_limit_v1(function_id, auxiliary_resources, limits).map_err(
            |error| {
                resource_diagnostic_v1::wrap(
                    error,
                    resource_diagnostic_v1::Stage::Auxiliary,
                    auxiliary_resources.storage_words,
                    None,
                    (0, 0, 0),
                )
            },
        )?;
        let plan = plan_ssa_with_limits_v1(&input, limits.planner()).map_err(|error| {
            ProductionSemanticSsaErrorV1::Planner {
                function: function_id,
                error,
            }
        })?;
        let origin_resources =
            source_uses_v1::definitions_v1::ValueOriginsV1::resources_for(&plan)?;
        auxiliary_resources.storage_words = auxiliary_resources
            .storage_words
            .checked_add(origin_resources.storage_words)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        auxiliary_resources.work_units = auxiliary_resources
            .work_units
            .checked_add(origin_resources.work_units)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        enforce_function_resource_limit_v1(
            function_id,
            SemanticSsaAuxiliaryResourcesV1 {
                storage_words: auxiliary_resources
                    .storage_words
                    .checked_add(plan.resources().storage_words())
                    .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
                work_units: auxiliary_resources
                    .work_units
                    .checked_add(plan.resources().work_units())
                    .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
            },
            limits,
        )
        .map_err(|error| {
            resource_diagnostic_v1::wrap(
                error,
                resource_diagnostic_v1::Stage::Combined,
                auxiliary_resources.storage_words,
                Some(plan.resources().storage_words()),
                (0, 0, 0),
            )
        })?;
        let partial_moves = validate_partial_moves_v1(
            function_id,
            function,
            types,
            &plan,
            auxiliary_resources,
            limits,
        )?;
        let value_origins = source_uses_v1::definitions_v1::ValueOriginsV1::build(&input, &plan)?;
        let retained_cross_edge_variables =
            retained_cross_edge_variables_v1(&input, &plan).into_boxed_slice();
        Ok(ProductionSemanticSsaFunctionPlanV1 {
            function: function_id,
            function_identity: function.identity(),
            plan,
            partial_moves,
            implicit_entry_variables: implicit_entry_variables.into_boxed_slice(),
            frame_initializations,
            defined_math_results,
            defined_matrix_results,
            defined_reusable_lds_results,
            guarded_grid_results,
            defined_reusable_phase_results,
            retained_cross_edge_variables,
            event_origins: std::mem::take(&mut event_origins),
            value_origins,
            auxiliary_resources,
        })
    })();
    result.map_err(|error| match view {
        Some(view) => execution::wrap_error(view, &event_origins, error),
        None => error,
    })
}

fn semantic_ssa_auxiliary_resources_v1(
    function: &SemanticFunctionDeclV1,
    input: &SsaConstructionInputV1,
) -> Result<SemanticSsaAuxiliaryResourcesV1, ProductionSemanticSsaErrorV1> {
    partial_moves::auxiliary_resources_v1(function, input)
}

fn enforce_function_resource_limit_v1(
    function: SemanticFunctionIdV1,
    resources: SemanticSsaAuxiliaryResourcesV1,
    limits: ProductionSemanticSsaLimitsV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    for (resource, required, limit) in [
        (
            SsaPlannerResourceV1::StorageWords,
            resources.storage_words,
            limits.planner().max_storage_words(),
        ),
        (
            SsaPlannerResourceV1::WorkUnits,
            resources.work_units,
            limits.planner().max_work_units(),
        ),
    ] {
        if required > limit {
            return Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
                function,
                resource,
                required,
                limit,
            });
        }
    }
    Ok(())
}

mod accounting;
mod adapter;
mod defined_math_results;
mod defined_matrix_results;
mod defined_reusable_lds_results;
mod guarded_grid_results;
mod defined_reusable_phase_results;
mod execution;
mod frame_initialization;
mod partial_moves;
mod resource_diagnostic_v1;
mod source_partial_move_diagnostic_v1;
mod source_uses_v1;

pub use source_uses_v1::{
    ProductionSemanticSsaIncomingEdgeV1, ProductionSemanticSsaIncomingValuesV1,
    ProductionSemanticSsaSourceQueryErrorV1, ProductionSemanticSsaSourceQueryV1,
    ProductionSemanticSsaSourceOperandV1,
    ProductionSemanticSsaSourceSiteV1, ProductionSemanticSsaSourceUseV1,
    ProductionSemanticSsaValueOriginV1, ProductionSemanticSsaValueV1,
};

use accounting::{
    accumulate_summary_v1, derive_semantic_ssa_identity_v1, retained_cross_edge_variables_v1,
};
pub use adapter::authenticated_ambient_workgroup_lds_scope_zst_v1;
use adapter::{SemanticTransparentBorrowSiteV1, transparent_borrow_sites_v1};

#[cfg(test)]
use adapter::semantic_function_ssa_input_v1;
pub use frame_initialization::ProductionSemanticSsaFrameInitializationV1;
pub use defined_math_results::ProductionSemanticMathBridgeResultV1;
pub use defined_matrix_results::ProductionSemanticMatrixBridgeResultV1;
pub use defined_reusable_lds_results::ProductionSemanticReusableLdsResultV1;
pub use guarded_grid_results::{ProductionGuardedGridLeaderResultV1, ProductionGuardedGridActionKindV1, ProductionGuardedGridActionV1, ProductionGuardedGridIndexV1, ProductionGuardedGridSourceV1};
pub use defined_reusable_phase_results::{ProductionSemanticPhaseResultInputsV1, ProductionSemanticPhaseResultV1, ProductionSemanticPhaseRelayV1};
use partial_moves::validate_partial_moves_v1;

#[cfg(test)]
mod tests;
