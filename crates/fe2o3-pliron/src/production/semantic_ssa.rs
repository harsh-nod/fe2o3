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
    SsaBlockIdV1, SsaBlockInputV1, SsaConstructionInputV1, SsaConstructionPlanV1, SsaEdgeInputV1,
    SsaEdgeRoleV1, SsaEventV1, SsaPlannerErrorV1, SsaPlannerLimitsV1, SsaPlannerResourceReportV1,
    SsaPlannerResourceV1, SsaVariableIdV1, plan_ssa_with_limits_v1,
    semantic_mir_v1::{
        AdmittedInertSemanticMirV1, SemanticAbiPassModeV1, SemanticAssertMessageV1,
        SemanticBackendReprV1, SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
        SemanticControlFlowEdgeV1, SemanticEdgeRoleV1, SemanticFunctionDeclV1,
        SemanticFunctionIdV1, SemanticFunctionIdentityV1, SemanticLocalRoleV1, SemanticOperandV1,
        SemanticPlaceV1, SemanticProjectionKindV1, SemanticRvalueKindV1, SemanticStatementKindV1,
        SemanticTerminatorKindV1, SemanticTypeDeclV1, SemanticTypeIdV1,
        SemanticTypeLayoutDetailsV1, SemanticTypeShapeV1,
    },
};
use sha2::{Digest as _, Sha256};

use super::{ProductionSemanticMirErrorV1, ProductionSemanticMirOwnerV1};

const PRODUCTION_SEMANTIC_SSA_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.production-semantic-ssa-owner.v1\0";

/// Bounded planner policy retained for deterministic replay.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProductionSemanticSsaLimitsV1 {
    planner: SsaPlannerLimitsV1,
}

impl ProductionSemanticSsaLimitsV1 {
    pub const fn new(planner: SsaPlannerLimitsV1) -> Self {
        Self { planner }
    }

    pub const fn planner(self) -> SsaPlannerLimitsV1 {
        self.planner
    }
}

/// Fail-closed errors from semantic-to-SSA planning or deterministic replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionSemanticSsaErrorV1 {
    SemanticOwner(ProductionSemanticMirErrorV1),
    Planner {
        function: SemanticFunctionIdV1,
        error: SsaPlannerErrorV1,
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
            Self::Planner { function, error } => write!(
                formatter,
                "production semantic SSA planning failed for function {}: {error}",
                function.index(),
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
            Self::Planner { error, .. } => Some(error),
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

    pub const fn state_entries(self) -> usize {
        self.state_entries
    }

    pub const fn work_units(self) -> usize {
        self.work_units
    }
}

/// Identity of the unchanged semantic source and its exact per-function plans.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionSemanticSsaIdentityV1([u8; 32]);

impl ProductionSemanticSsaIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Aggregate bounded-work summary for all semantic functions.
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
    retained_cross_edge_variables: Box<[SsaVariableIdV1]>,
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

    /// Returns storage-retained locals that need state across a reachable CFG
    /// edge. KIR lowering must either materialize these locals or reject them.
    pub fn retained_cross_edge_variables(&self) -> &[SsaVariableIdV1] {
        &self.retained_cross_edge_variables
    }
}

/// Move-only custody of unchanged semantic MIR and one replayable SSA plan per
/// semantic function.
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
    plans: Box<[ProductionSemanticSsaFunctionPlanV1]>,
    summary: ProductionSemanticSsaSummaryV1,
    identity: ProductionSemanticSsaIdentityV1,
}

impl fmt::Debug for ProductionSemanticSsaOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionSemanticSsaOwnerV1")
            .field("source_semantic_sha256", &self.source_semantic_sha256)
            .field("summary", &self.summary)
            .field("identity", &self.identity)
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
        Ok(Self {
            source_owner,
            source_semantic_sha256,
            limits,
            plans,
            summary,
            identity,
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
        if plans != self.plans || summary != self.summary || identity != self.identity {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
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

    pub fn plans(&self) -> &[ProductionSemanticSsaFunctionPlanV1] {
        &self.plans
    }

    /// Borrows the plan bound to one exact semantic function identity.
    pub fn plan_for_function(
        &self,
        function: SemanticFunctionIdV1,
    ) -> Option<&ProductionSemanticSsaFunctionPlanV1> {
        self.plans
            .get(function.index() as usize)
            .filter(|plan| plan.function == function)
    }

    pub const fn summary(&self) -> ProductionSemanticSsaSummaryV1 {
        self.summary
    }

    pub const fn identity(&self) -> ProductionSemanticSsaIdentityV1 {
        self.identity
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
        let transparent_borrows = transparent_borrow_sites_v1(function, semantic.callables());
        let function_plan = plan_semantic_function_ssa_with_borrow_sites_v1(
            function_id,
            function,
            Some(semantic.types()),
            semantic.callables(),
            limits,
            &transparent_borrows,
        )?;
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
    let transparent_borrows = transparent_borrow_sites_v1(function, callables);
    plan_semantic_function_ssa_with_borrow_sites_v1(
        function_id,
        function,
        Some(types),
        callables,
        limits,
        &transparent_borrows,
    )
}

fn plan_semantic_function_ssa_with_borrow_sites_v1(
    function_id: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    callables: &[SemanticCallableDeclV1],
    limits: ProductionSemanticSsaLimitsV1,
    transparent_borrows: &BTreeSet<SemanticTransparentBorrowSiteV1>,
) -> Result<ProductionSemanticSsaFunctionPlanV1, ProductionSemanticSsaErrorV1> {
    let (input, implicit_entry_variables) =
        semantic_function_ssa_input_v1(function, types, callables, transparent_borrows);
    let auxiliary_resources = semantic_ssa_auxiliary_resources_v1(function, &input)?;
    enforce_function_resource_limit_v1(function_id, auxiliary_resources, limits)?;
    let plan = plan_ssa_with_limits_v1(&input, limits.planner()).map_err(|error| {
        ProductionSemanticSsaErrorV1::Planner {
            function: function_id,
            error,
        }
    })?;
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
    )?;
    let partial_moves = validate_partial_moves_v1(
        function_id,
        function,
        types,
        &plan,
        auxiliary_resources,
        limits,
    )?;
    let retained_cross_edge_variables =
        retained_cross_edge_variables_v1(&input, &plan).into_boxed_slice();
    Ok(ProductionSemanticSsaFunctionPlanV1 {
        function: function_id,
        function_identity: function.identity(),
        plan,
        partial_moves,
        implicit_entry_variables: implicit_entry_variables.into_boxed_slice(),
        retained_cross_edge_variables,
        auxiliary_resources,
    })
}

fn semantic_ssa_auxiliary_resources_v1(
    function: &SemanticFunctionDeclV1,
    input: &SsaConstructionInputV1,
) -> Result<SemanticSsaAuxiliaryResourcesV1, ProductionSemanticSsaErrorV1> {
    let blocks = input.blocks().len();
    let variables = input.promotable().len();
    let statements = function.blocks().iter().try_fold(0_usize, |total, block| {
        total
            .checked_add(block.statements().len())
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)
    })?;
    let (events, edges, edge_definitions) = input.blocks().iter().try_fold(
        (0_usize, 0_usize, 0_usize),
        |(events, edges, definitions), block| {
            let events = events
                .checked_add(block.events().len())
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            let edges = edges
                .checked_add(block.edges().len())
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            let definitions = block.edges().iter().try_fold(definitions, |total, edge| {
                total
                    .checked_add(edge.definitions().len())
                    .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)
            })?;
            Ok((events, edges, definitions))
        },
    )?;
    let (projected_moves, maximum_projection_depth) = projected_local_move_metrics_v1(function)?;

    // Logical words conservatively cover adapter rows, borrow/implicit-entry
    // indices, retained-local scratch, and every persistent partial-move state.
    // Each move-path entry reserves eight tree/header words plus its full path.
    let adapter_items = variables
        .checked_mul(8)
        .and_then(|value| value.checked_add(blocks.checked_mul(12)?))
        .and_then(|value| value.checked_add(statements.checked_mul(8)?))
        .and_then(|value| value.checked_add(events.checked_mul(4)?))
        .and_then(|value| value.checked_add(edges.checked_mul(6)?))
        .and_then(|value| value.checked_add(edge_definitions.checked_mul(2)?))
        .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
    let path_words = maximum_projection_depth
        .checked_add(8)
        .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
    let partial_state_copies = blocks
        .checked_add(2)
        .and_then(|value| value.checked_mul(projected_moves))
        .and_then(|value| value.checked_mul(path_words))
        .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
    let storage_words = adapter_items
        .checked_add(partial_state_copies)
        .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;

    // A block can be revisited once for each newly merged path. On each visit,
    // every outgoing edge can clone and merge the complete path set.
    let partial_rounds = edges
        .checked_mul(
            projected_moves
                .checked_add(1)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
        )
        .and_then(|value| value.checked_add(blocks))
        .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
    let partial_work = partial_rounds
        .checked_mul(
            projected_moves
                .checked_add(1)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
        )
        .and_then(|value| value.checked_mul(path_words))
        .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
    let adapter_work = adapter_items
        .checked_add(events)
        .and_then(|value| value.checked_add(edge_definitions))
        .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
    Ok(SemanticSsaAuxiliaryResourcesV1 {
        storage_words,
        work_units: adapter_work
            .checked_add(partial_work)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
    })
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
mod partial_moves;

use accounting::{
    accumulate_summary_v1, derive_semantic_ssa_identity_v1, retained_cross_edge_variables_v1,
};
pub use adapter::authenticated_ambient_workgroup_lds_scope_zst_v1;
use adapter::{
    SemanticTransparentBorrowSiteV1, semantic_function_ssa_input_v1, transparent_borrow_sites_v1,
};
use partial_moves::{projected_local_move_metrics_v1, validate_partial_moves_v1};

#[cfg(test)]
mod tests;
