// Production errors retain complete diagnostic evidence in their public variants.
#![allow(clippy::result_large_err)]

//! Closed, compiler-owned Pliron session for the production pipeline.
//!
//! It supports a closed builtin-module recipe and a bounded target-neutral
//! ranked-kernel recipe. Ranked graphs are constructed inside the owned context
//! and must pass the whole-function bounds transition before a move-only
//! lowering input exists. Callers cannot inject callbacks, arbitrary passes,
//! text, raw contexts, or contextless pointers.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    marker::PhantomData,
    num::NonZeroU64,
};

use crate::{PlironAtomicTargetContextV1, production_analysis::ProductionAnalysisResourceLimitsV1};
use fe2o3_pliron_owner_core::{ContextIdentity, DialectRegistration, NameError};

use super::{
    ContextBuildError, ContextManifest, NameKind, OperationGraphSnapshotV1, OperationHandle,
    OperationHandleError, OperationShapeV1, PlironSession, ShellLimits, validate_name,
};

mod conditional_ranked_v1;
mod middle_end_evidence_v4;
mod middle_end_evidence_v5;
mod mir_pliron_semantic_contract_derivation_v1;
mod mir_pliron_semantic_contract_v1;
mod noncanonical_loop_proof_v1;
mod parallel_reference_contract_v1;
mod ranked;
mod semantic_expression_v2;
mod semantic_mir;
mod semantic_ssa;
mod total_output_refinement_v2;

pub use conditional_ranked_v1::*;
pub use middle_end_evidence_v4::*;
pub use middle_end_evidence_v5::*;
pub use mir_pliron_semantic_contract_derivation_v1::*;
pub use mir_pliron_semantic_contract_v1::*;
pub use noncanonical_loop_proof_v1::*;
pub use parallel_reference_contract_v1::*;
pub use ranked::*;
pub use semantic_expression_v2::*;
pub use semantic_mir::*;
pub use semantic_ssa::*;
pub use total_output_refinement_v2::*;

/// Hard cap for construction recipes registered during one production session.
pub const HARD_MAX_PRODUCTION_CONSTRUCTIONS: usize = 4_096;

/// Resource limits for one closed production Pliron session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSessionLimitsV1 {
    shell: ShellLimits,
    max_constructions: usize,
}

impl ProductionSessionLimitsV1 {
    /// Creates non-zero limits bounded by implementation hard caps.
    pub fn new(
        shell: ShellLimits,
        max_constructions: usize,
    ) -> Result<Self, ProductionSessionLimitErrorV1> {
        if max_constructions == 0 {
            return Err(ProductionSessionLimitErrorV1::ZeroConstructions);
        }
        if max_constructions > HARD_MAX_PRODUCTION_CONSTRUCTIONS {
            return Err(ProductionSessionLimitErrorV1::TooManyConstructions);
        }
        Ok(Self {
            shell,
            max_constructions,
        })
    }

    pub const fn shell(self) -> ShellLimits {
        self.shell
    }

    pub const fn max_constructions(self) -> usize {
        self.max_constructions
    }
}

impl Default for ProductionSessionLimitsV1 {
    fn default() -> Self {
        Self {
            shell: ShellLimits::default(),
            max_constructions: 64,
        }
    }
}

/// Invalid production-session resource configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSessionLimitErrorV1 {
    ZeroConstructions,
    TooManyConstructions,
}

impl fmt::Display for ProductionSessionLimitErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroConstructions => {
                formatter.write_str("production construction limit must be non-zero")
            }
            Self::TooManyConstructions => {
                formatter.write_str("production construction limit exceeds the hard cap")
            }
        }
    }
}

impl Error for ProductionSessionLimitErrorV1 {}

/// A closed, bounded construction recipe admitted by the production session.
///
/// The private recipe representation prevents callers from adding a constructor
/// callback or smuggling a raw Pliron capability into the session.
#[derive(Debug, Eq, PartialEq)]
pub struct ProductionConstructionV1 {
    kind: ProductionConstructionKindV1,
}

#[derive(Debug, Eq, PartialEq)]
enum ProductionConstructionKindV1 {
    BuiltinModule {
        root_name: String,
    },
    RankedKernel {
        root_name: String,
        kernel: ProductionRankedKernelV1,
        policy_checked_refinement_staging: Vec<ProductionPolicyCheckedRefinementStagingV2>,
    },
}

impl ProductionConstructionV1 {
    /// Builds a validated builtin-module recipe without allocating Pliron IR.
    pub fn builtin_module(root_name: &str) -> Result<Self, NameError> {
        validate_name(root_name, NameKind::Dialect)?;
        Ok(Self {
            kind: ProductionConstructionKindV1::BuiltinModule {
                root_name: root_name.to_owned(),
            },
        })
    }

    fn root_name(&self) -> &str {
        match &self.kind {
            ProductionConstructionKindV1::BuiltinModule { root_name }
            | ProductionConstructionKindV1::RankedKernel { root_name, .. } => root_name,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct StageIdentityV1(NonZeroU64);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RootIdentityV1(NonZeroU64);

/// Canonical identity of the complete retained ranked recipe bound to a live graph view.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct ProductionExactGraphIdentityV1([u8; 32]);

impl ProductionExactGraphIdentityV1 {
    fn from_ranked(kernel: &ProductionRankedKernelV1) -> Self {
        Self(middle_end_evidence_v4::derive_exact_ranked_graph_identity_v1(kernel))
    }

    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for ProductionExactGraphIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionExactGraphIdentityV1")
            .finish_non_exhaustive()
    }
}

/// Typestate for a recipe registered but not yet materialized in Pliron.
#[derive(Debug)]
pub struct ConstructionRegisteredStageV1 {
    _private: (),
}

/// Typestate for a recursively verified, session-owned Pliron graph.
#[derive(Debug)]
pub struct ConstructedGraphStageV1 {
    _private: (),
}

/// Typestate for a ranked function that passed the fixed generic verifier pipeline.
#[derive(Debug)]
pub struct KernelChecksVerifiedGraphStageV1 {
    _private: (),
}

/// An opaque, move-only stage capability owned by one production session.
///
/// It contains no Pliron pointer. Its owner and registry identity are private
/// and are authenticated before every operation.
#[must_use = "dropping a stage capability abandons its bounded production transition"]
pub struct ProductionStageHandleV1<Stage> {
    owner: ContextIdentity,
    identity: StageIdentityV1,
    _stage: PhantomData<fn() -> Stage>,
}

impl<Stage> fmt::Debug for ProductionStageHandleV1<Stage> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionStageHandleV1")
            .finish_non_exhaustive()
    }
}

/// An opaque, move-only root capability owned by one production session/stage.
///
/// Neither the private operation handle nor its upstream pointer can be
/// recovered by callers:
///
/// ```compile_fail
/// use fe2o3_pliron::{ConstructedGraphStageV1, ProductionRootHandleV1};
/// use pliron::{context::Ptr, operation::Operation};
///
/// fn escape(root: &ProductionRootHandleV1<ConstructedGraphStageV1>) -> Ptr<Operation> {
///     root.operation
/// }
/// ```
#[must_use = "dropping a root capability makes its production graph inaccessible"]
pub struct ProductionRootHandleV1<Stage> {
    owner: ContextIdentity,
    stage: StageIdentityV1,
    identity: RootIdentityV1,
    operation: OperationHandle,
    graph_snapshot: OperationGraphSnapshotV1,
    exact_graph_identity: Option<ProductionExactGraphIdentityV1>,
    _stage: PhantomData<fn() -> Stage>,
}

impl<Stage> fmt::Debug for ProductionRootHandleV1<Stage> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionRootHandleV1")
            .finish_non_exhaustive()
    }
}

/// Fail-closed errors from the closed production session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionSessionErrorV1 {
    SessionPoisoned,
    ConstructionLimitExceeded,
    DuplicateConstructionName(String),
    StageIdentitySpaceExhausted,
    RootIdentitySpaceExhausted,
    ForeignSession,
    StaleStage,
    StageRootMismatch,
    WrongConstructionKind,
    RankedGraphChanged,
    AnalysisResourceLimit {
        phase: crate::ProductionAnalysisResourcePhaseV1,
        producing_pass: Option<crate::KernelCheckPassKindV1>,
        resource: &'static str,
    },
    RankedRecipe(ProductionRankedKernelErrorV1),
    RankedTensorLayout(crate::PlironTensorLayoutCheckErrorV1),
    RankedBounds(crate::RankedBoundsCheckErrorV1),
    RankedAtomic(crate::PlironAtomicLegalityCheckErrorV1),
    RankedRace(crate::RankedRaceCheckErrorV1),
    RankedOwnership(crate::HierarchicalOwnershipCheckErrorV1),
    ConditionalOwnership(ProductionConditionalOwnershipErrorV1),
    RankedBarrier(crate::PlironBarrierCheckErrorV1),
    RankedPipeline(crate::PlironPipelineProtocolCheckErrorV1),
    RankedWorkgroup(crate::PlironWorkgroupMemoryCheckErrorV1),
    RankedSemantic(crate::PlironSemanticRefinementCheckErrorV1),
    RankedPassPreservation(crate::PlironPassPreservationErrorV1),
    RankedReportValidation(crate::ProductionAnalysisReportValidationErrorV1),
    Operation(OperationHandleError),
}

impl ProductionSessionErrorV1 {
    /// Structured repairs for errors produced by the unified kernel-check
    /// pipeline. Session bookkeeping and construction errors are not source
    /// diagnostics and therefore do not fabricate kernel edits.
    pub fn repair_hints(&self) -> Vec<crate::KernelCheckRepairV1> {
        use crate::KernelCheckPassKindV1 as Pass;
        match self {
            Self::RankedTensorLayout(error) => {
                vec![crate::tensor_layout_repair_for_error_v1(error)]
            }
            Self::RankedBounds(_) => {
                vec![crate::kernel_check_repair_for_pass_v1(Pass::MemoryBounds)]
            }
            Self::RankedAtomic(_) => {
                vec![crate::kernel_check_repair_for_pass_v1(Pass::AtomicLegality)]
            }
            Self::RankedRace(_) => vec![crate::kernel_check_repair_for_pass_v1(Pass::RaceFreedom)],
            Self::RankedOwnership(_) => {
                vec![crate::kernel_check_repair_for_pass_v1(
                    Pass::HierarchicalOwnership,
                )]
            }
            Self::RankedBarrier(_) => vec![crate::kernel_check_repair_for_pass_v1(
                Pass::BarrierConvergence,
            )],
            Self::RankedPipeline(_) => {
                vec![crate::kernel_check_repair_for_pass_v1(
                    Pass::PipelineProtocol,
                )]
            }
            Self::RankedWorkgroup(_) => {
                vec![crate::kernel_check_repair_for_pass_v1(
                    Pass::WorkgroupMemory,
                )]
            }
            Self::RankedSemantic(_) => {
                vec![crate::kernel_check_repair_for_pass_v1(
                    Pass::SemanticRefinement,
                )]
            }
            Self::RankedPassPreservation(error) => {
                vec![crate::pass_preservation_repair_for_error_v1(error)]
            }
            Self::RankedReportValidation(error) => {
                vec![crate::report_validation_repair_for_error_v1(error)]
            }
            _ => Vec::new(),
        }
    }
}

impl fmt::Display for ProductionSessionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionPoisoned => formatter.write_str("production Pliron session is poisoned"),
            Self::ConstructionLimitExceeded => {
                formatter.write_str("production construction registration limit exceeded")
            }
            Self::DuplicateConstructionName(_) => {
                formatter.write_str("duplicate production construction name")
            }
            Self::StageIdentitySpaceExhausted => {
                formatter.write_str("production stage identity space exhausted")
            }
            Self::RootIdentitySpaceExhausted => {
                formatter.write_str("production root identity space exhausted")
            }
            Self::ForeignSession => {
                formatter.write_str("production capability belongs to another session")
            }
            Self::StaleStage => formatter.write_str("production stage capability is stale"),
            Self::StageRootMismatch => {
                formatter.write_str("production root does not belong to the supplied stage")
            }
            Self::WrongConstructionKind => {
                formatter.write_str("production stage is not a ranked-kernel construction")
            }
            Self::RankedGraphChanged => {
                formatter.write_str("production ranked graph changed after safety verification")
            }
            Self::AnalysisResourceLimit {
                phase,
                producing_pass,
                resource,
            } => {
                write!(
                    formatter,
                    "production analysis resource limit exceeded [{}]: {resource}",
                    phase.code(),
                )?;
                if let Some(pass) = producing_pass {
                    write!(formatter, " (producing pass: {})", pass.name())?;
                }
                Ok(())
            }
            Self::RankedRecipe(error) => {
                write!(formatter, "production ranked recipe failed: {error}")
            }
            Self::RankedTensorLayout(error) => error.fmt(formatter),
            Self::RankedBounds(error) => error.fmt(formatter),
            Self::RankedAtomic(error) => error.fmt(formatter),
            Self::RankedRace(error) => error.fmt(formatter),
            Self::RankedOwnership(error) => error.fmt(formatter),
            Self::ConditionalOwnership(error) => error.fmt(formatter),
            Self::RankedBarrier(error) => error.fmt(formatter),
            Self::RankedPipeline(error) => error.fmt(formatter),
            Self::RankedWorkgroup(error) => error.fmt(formatter),
            Self::RankedSemantic(error) => error.fmt(formatter),
            Self::RankedPassPreservation(error) => error.fmt(formatter),
            Self::RankedReportValidation(error) => error.fmt(formatter),
            Self::Operation(_) => formatter.write_str("production Pliron operation failed"),
        }?;
        for repair in self.repair_hints() {
            write!(formatter, "\n{repair}")?;
        }
        Ok(())
    }
}

impl Error for ProductionSessionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Operation(error) => Some(error),
            Self::RankedRecipe(error) => Some(error),
            Self::RankedTensorLayout(error) => Some(error),
            Self::RankedBounds(error) => Some(error),
            Self::RankedAtomic(error) => Some(error),
            Self::RankedRace(error) => Some(error),
            Self::RankedOwnership(error) => Some(error),
            Self::ConditionalOwnership(error) => Some(error),
            Self::RankedBarrier(error) => Some(error),
            Self::RankedPipeline(error) => Some(error),
            Self::RankedWorkgroup(error) => Some(error),
            Self::RankedSemantic(error) => Some(error),
            Self::RankedPassPreservation(error) => Some(error),
            Self::RankedReportValidation(error) => Some(error),
            _ => None,
        }
    }
}

/// Closed compiler-owned production session.
///
/// The private [`PlironSession`] transitively owns the only raw Pliron context.
/// This type intentionally exposes no generic callback, textual importer,
/// arbitrary pass, operation handle, context, or pointer surface:
///
/// ```compile_fail
/// use fe2o3_pliron::ProductionPlironSessionV1;
/// use pliron::context::Context;
///
/// fn escape(session: &mut ProductionPlironSessionV1) -> &mut Context {
///     &mut session.inner.context
/// }
/// ```
pub struct ProductionPlironSessionV1 {
    inner: PlironSession,
    atomic_target: Option<PlironAtomicTargetContextV1>,
    limits: ProductionSessionLimitsV1,
    analysis_resource_limits: ProductionAnalysisResourceLimitsV1,
    ownership_binding_resources: crate::production_analysis::ProductionAnalysisResourceUpperBoundV1,
    registered: BTreeMap<StageIdentityV1, ProductionConstructionV1>,
    construction_names: BTreeSet<String>,
    constructed_roots: BTreeMap<StageIdentityV1, ConstructedRootV1>,
    registration_count: usize,
    next_stage: Option<NonZeroU64>,
    next_root: Option<NonZeroU64>,
    poisoned: bool,
}

impl ProductionPlironSessionV1 {
    /// Creates a fresh production session using only bounded typed dialect registrations.
    pub fn new(
        limits: ProductionSessionLimitsV1,
        registrations: impl IntoIterator<Item = DialectRegistration>,
    ) -> Result<Self, ContextBuildError> {
        Self::new_with_analysis_resource_limits_v1(
            limits,
            registrations,
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
    }

    pub(super) fn new_with_analysis_resource_limits_v1(
        limits: ProductionSessionLimitsV1,
        registrations: impl IntoIterator<Item = DialectRegistration>,
        analysis_resource_limits: ProductionAnalysisResourceLimitsV1,
    ) -> Result<Self, ContextBuildError> {
        let inner = PlironSession::new(limits.shell(), registrations)?;
        Ok(Self {
            inner,
            atomic_target: None,
            limits,
            analysis_resource_limits,
            ownership_binding_resources:
                crate::production_analysis::ProductionAnalysisResourceUpperBoundV1::checked_phase(
                    crate::ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
                    0,
                    0,
                    0,
                )
                .expect("zero resource reservation"),
            registered: BTreeMap::new(),
            construction_names: BTreeSet::new(),
            constructed_roots: BTreeMap::new(),
            registration_count: 0,
            next_stage: NonZeroU64::new(1),
            next_root: NonZeroU64::new(1),
            poisoned: false,
        })
    }

    pub(super) fn bind_atomic_target(&mut self, target: PlironAtomicTargetContextV1) {
        debug_assert!(self.registered.is_empty() && self.constructed_roots.is_empty());
        self.atomic_target = Some(target);
    }

    pub const fn manifest(&self) -> &ContextManifest {
        self.inner.manifest()
    }

    pub const fn limits(&self) -> ProductionSessionLimitsV1 {
        self.limits
    }

    pub(super) const fn analysis_resource_limits(&self) -> ProductionAnalysisResourceLimitsV1 {
        self.analysis_resource_limits
    }

    pub const fn is_poisoned(&self) -> bool {
        self.poisoned || self.inner.is_poisoned()
    }

    /// Registers one prevalidated recipe and returns a move-only stage capability.
    ///
    /// The count is monotonic for the session lifetime. A rejected duplicate or
    /// over-limit recipe changes no registry state and does not poison the
    /// session because no Pliron allocation has started.
    pub fn register_construction(
        &mut self,
        construction: ProductionConstructionV1,
    ) -> Result<ProductionStageHandleV1<ConstructionRegisteredStageV1>, ProductionSessionErrorV1>
    {
        self.validate_live()?;
        if self.registration_count >= self.limits.max_constructions() {
            return Err(ProductionSessionErrorV1::ConstructionLimitExceeded);
        }
        if self.construction_names.contains(construction.root_name()) {
            return Err(ProductionSessionErrorV1::DuplicateConstructionName(
                construction.root_name().to_owned(),
            ));
        }
        let raw_identity = self
            .next_stage
            .ok_or(ProductionSessionErrorV1::StageIdentitySpaceExhausted)?;
        let identity = StageIdentityV1(raw_identity);
        let next_stage = raw_identity.get().checked_add(1).and_then(NonZeroU64::new);

        self.construction_names
            .insert(construction.root_name().to_owned());
        self.registered.insert(identity, construction);
        self.registration_count += 1;
        self.next_stage = next_stage;

        Ok(ProductionStageHandleV1 {
            owner: self.inner.identity,
            identity,
            _stage: PhantomData,
        })
    }

    /// Materializes one registered recipe and recursively verifies its root.
    ///
    /// This transition consumes the registered-stage capability. Any failure in
    /// the underlying operation construction is treated as a TCB failure and
    /// terminally poisons the production session.
    pub fn construct_registered(
        &mut self,
        stage: ProductionStageHandleV1<ConstructionRegisteredStageV1>,
    ) -> Result<
        (
            ProductionStageHandleV1<ConstructedGraphStageV1>,
            ProductionRootHandleV1<ConstructedGraphStageV1>,
        ),
        ProductionSessionErrorV1,
    > {
        self.validate_live()?;
        self.authenticate_owner(stage.owner)?;
        let construction = self
            .registered
            .remove(&stage.identity)
            .ok_or(ProductionSessionErrorV1::StaleStage)?;
        self.preflight_construction(&construction)?;
        let root_name = construction.root_name().to_owned();
        let raw_root = self
            .next_root
            .ok_or(ProductionSessionErrorV1::RootIdentitySpaceExhausted)?;
        let root_identity = RootIdentityV1(raw_root);
        let next_root = raw_root.get().checked_add(1).and_then(NonZeroU64::new);

        let is_builtin = matches!(
            &construction.kind,
            ProductionConstructionKindV1::BuiltinModule { .. }
        );
        let materialized = match self.materialize_construction(construction, &root_name) {
            Ok(materialized) => materialized,
            Err(error) => {
                self.poisoned = true;
                return Err(error);
            }
        };
        let operation = materialized.operation;
        if is_builtin {
            match self
                .inner
                .validate_production_module(&operation, &root_name)
            {
                Ok(_) => {}
                Err(error) => {
                    self.poisoned = true;
                    return Err(ProductionSessionErrorV1::Operation(error));
                }
            }
        }

        let graph_snapshot = match self.inner.operation_graph_snapshot_v1(&operation) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.poisoned = true;
                return Err(ProductionSessionErrorV1::Operation(error));
            }
        };
        let exact_graph_identity = materialized
            .ranked_kernel
            .as_ref()
            .map(ProductionExactGraphIdentityV1::from_ranked);

        self.constructed_roots.insert(
            stage.identity,
            ConstructedRootV1 {
                identity: root_identity,
                graph_snapshot,
                exact_graph_identity,
                ranked_function: materialized.ranked_function,
                ranked_kernel: materialized.ranked_kernel,
                ranked_view_names: materialized.ranked_view_names,
                ownership_occurrences: materialized.ownership_occurrences,
                policy_checked_refinement_staging: materialized.policy_checked_refinement_staging,
                production_pipeline_report: None,
                production_analysis_resource_upper_bound: None,
                ranked_analysis_binding: None,
            },
        );
        self.next_root = next_root;

        Ok((
            ProductionStageHandleV1 {
                owner: stage.owner,
                identity: stage.identity,
                _stage: PhantomData,
            },
            ProductionRootHandleV1 {
                owner: stage.owner,
                stage: stage.identity,
                identity: root_identity,
                operation,
                graph_snapshot,
                exact_graph_identity,
                _stage: PhantomData,
            },
        ))
    }

    /// Returns a bounded pointer-free root description after authenticating the session and stage.
    pub fn root_shape(
        &mut self,
        stage: &ProductionStageHandleV1<ConstructedGraphStageV1>,
        root: &ProductionRootHandleV1<ConstructedGraphStageV1>,
    ) -> Result<OperationShapeV1, ProductionSessionErrorV1> {
        self.validate_live()?;
        self.authenticate_owner(stage.owner)?;
        self.authenticate_owner(root.owner)?;
        let (expected_root, expected_snapshot, expected_identity) = self
            .constructed_roots
            .get(&stage.identity)
            .map(|record| {
                (
                    record.identity,
                    record.graph_snapshot,
                    record.exact_graph_identity,
                )
            })
            .ok_or(ProductionSessionErrorV1::StaleStage)?;
        if root.stage != stage.identity
            || root.identity != expected_root
            || root.graph_snapshot != expected_snapshot
            || root.exact_graph_identity != expected_identity
        {
            return Err(ProductionSessionErrorV1::StageRootMismatch);
        }

        self.require_live_graph_snapshot_v1(&root.operation, root.graph_snapshot)?;

        match self.inner.operation_shape(&root.operation) {
            Ok(shape) => Ok(shape),
            Err(error) => {
                self.poisoned = true;
                Err(ProductionSessionErrorV1::Operation(error))
            }
        }
    }

    fn validate_live(&mut self) -> Result<(), ProductionSessionErrorV1> {
        if self.poisoned || self.inner.is_poisoned() {
            self.poisoned = true;
            return Err(ProductionSessionErrorV1::SessionPoisoned);
        }
        Ok(())
    }

    fn authenticate_owner(&self, owner: ContextIdentity) -> Result<(), ProductionSessionErrorV1> {
        if owner != self.inner.identity {
            return Err(ProductionSessionErrorV1::ForeignSession);
        }
        Ok(())
    }

    fn require_live_graph_snapshot_v1(
        &mut self,
        operation: &OperationHandle,
        snapshot: OperationGraphSnapshotV1,
    ) -> Result<(), ProductionSessionErrorV1> {
        match self
            .inner
            .require_operation_graph_snapshot_v1(operation, snapshot)
        {
            Ok(()) => Ok(()),
            Err(error) => {
                self.poisoned = true;
                Err(ProductionSessionErrorV1::Operation(error))
            }
        }
    }
}

#[cfg(test)]
#[path = "production/session_v1_tests.rs"]
mod tests;
