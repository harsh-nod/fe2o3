//! Session-owned, still-pending ownership analysis. No clean-stage conversion.
//!
//! The analysis envelope includes occurrence bindings, structural capture and
//! conditional analysis, including exact recipe rehashing. Legacy root-snapshot
//! printing and initial construction hashing remain outside that envelope, as
//! do preexisting arena/recipe allocations.
//! This owner therefore does not establish a complete constructor resource bound.

use super::*;
use crate::production_analysis::{
    BuiltIdentityV1, IdentityCaptureFailureV1, LivePlironStructuralIdentityProviderV1,
    PlironAnalysisManagerV1, PlironStructuralIdentityProviderV1,
    ProductionAnalysisResourceContractV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourcePhaseV1 as Phase, ProductionAnalysisResourceUpperBoundV1 as Bound,
    conditional_analysis_v1::{
        ConditionalOwnershipAnalysisErrorV1, ConditionalOwnershipPayloadV1,
        ConditionalOwnershipSelectionV1, run_conditional_ownership_analysis_v1,
    },
};
use pliron::{builtin::ops::FuncOp, context::Ptr, op::Op, operation::Operation, value::Value};
use std::panic::{AssertUnwindSafe, catch_unwind};

pub use crate::production_analysis::conditional_analysis_v1::{
    ConditionalOwnershipBlockerV1 as ProductionConditionalOwnershipBlockerV1,
    ConditionalOwnershipCheckV1 as ProductionConditionalOwnershipCheckV1,
    ConditionalOwnershipRowV1 as ProductionConditionalOwnershipRowV1,
};

/// Inert recipe occurrence request. Positions are not live operation ordinals
/// or CPU argument offsets, and a view identity is not a type-based match.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionConditionalOwnershipSiteV1 {
    pub block: u32,
    pub operation: u32,
    pub view: ProductionRankedValueV1,
}

pub(super) struct OwnershipOccurrenceV1 {
    pub(super) site: ProductionConditionalOwnershipSiteV1,
    pub(super) operation: Ptr<Operation>,
    pub(super) view: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionConditionalOwnershipErrorV1 {
    Selection { index: usize, reason: &'static str },
    Contracts(crate::HierarchicalOwnershipReportV1),
}

impl fmt::Display for ProductionConditionalOwnershipErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Selection { index, reason } => {
                write!(out, "conditional ownership selection {index}: {reason}")
            }
            Self::Contracts(report) => write!(out, "conditional ownership inventory: {report:?}"),
        }
    }
}

impl Error for ProductionConditionalOwnershipErrorV1 {}

/// Owns the original graph, bindings, diagnostics and cumulative analysis
/// reservation. All complete-pipeline obligations remain pending.
///
/// This is not a verified lowering input, even if individual rows are checked:
///
/// ```compile_fail
/// use fe2o3_pliron::{ProductionConditionalRankedAnalysisV1, ProductionRankedKernelLoweringInputV1};
/// fn promote(pending: ProductionConditionalRankedAnalysisV1) -> ProductionRankedKernelLoweringInputV1 {
///     pending.into()
/// }
/// ```
///
/// Neither the owned session nor arena pointers can be recovered:
///
/// ```compile_fail
/// use fe2o3_pliron::{ProductionConditionalRankedAnalysisV1, ProductionPlironSessionV1};
/// fn recover(pending: ProductionConditionalRankedAnalysisV1) -> ProductionPlironSessionV1 {
///     pending._session
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_pliron::ProductionConditionalRankedAnalysisV1;
/// fn duplicate(pending: ProductionConditionalRankedAnalysisV1) {
///     let _ = pending.clone();
/// }
/// ```
pub struct ProductionConditionalRankedAnalysisV1 {
    // Field order drops every graph-relative payload before its context, and
    // no method refunds the conservative owner or cache reservations.
    analysis: PreparedConditionalAnalysisV1,
    _stage: ProductionStageHandleV1<ConstructedGraphStageV1>,
    _root: ProductionRootHandleV1<ConstructedGraphStageV1>,
    _session: ProductionPlironSessionV1,
}

struct PreparedConditionalAnalysisV1 {
    payload: ConditionalOwnershipPayloadV1,
    selections: Vec<ProductionConditionalOwnershipSiteV1>,
    _identity: BuiltIdentityV1,
    _mutation_epoch: u64,
    _analyses: PlironAnalysisManagerV1,
}

impl ProductionConditionalRankedAnalysisV1 {
    /// Borrows the exact recipe retained when this frozen owner was prepared.
    /// This authenticates custody metadata, not a fresh graph analysis. No
    /// mutation or session recovery is exposed, and all pipeline checks remain
    /// pending.
    ///
    /// ```compile_fail
    /// use fe2o3_pliron::{ProductionConditionalRankedAnalysisV1, ProductionRankedKernelV1};
    /// fn escape(owner: ProductionConditionalRankedAnalysisV1) -> &'static ProductionRankedKernelV1 {
    ///     owner.kernel().unwrap()
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_pliron::{ProductionConditionalRankedAnalysisV1, ProductionRankedKernelV1};
    /// fn mutate(owner: &mut ProductionConditionalRankedAnalysisV1) -> &mut ProductionRankedKernelV1 {
    ///     owner.kernel().unwrap()
    /// }
    /// ```
    pub fn kernel(&self) -> Result<&ProductionRankedKernelV1, ProductionSessionErrorV1> {
        let session = &self._session;
        if session.poisoned || session.inner.is_poisoned() {
            return Err(ProductionSessionErrorV1::SessionPoisoned);
        }
        session.authenticate_owner(self._stage.owner)?;
        session.authenticate_owner(self._root.owner)?;
        let record = session
            .constructed_roots
            .get(&self._stage.identity)
            .ok_or(ProductionSessionErrorV1::StaleStage)?;
        if self._root.stage != self._stage.identity
            || self._root.identity != record.identity
            || self._root.graph_snapshot != record.graph_snapshot
            || self._root.exact_graph_identity != record.exact_graph_identity
        {
            return Err(ProductionSessionErrorV1::StageRootMismatch);
        }
        if record.ranked_function != Some(self.analysis.payload.function()) {
            return Err(ProductionSessionErrorV1::RankedGraphChanged);
        }
        record
            .ranked_kernel
            .as_ref()
            .ok_or(ProductionSessionErrorV1::WrongConstructionKind)
    }

    pub fn legacy_report(&self) -> &crate::HierarchicalOwnershipReportV1 {
        self.analysis.payload.legacy_report()
    }

    pub fn prerequisites(&self) -> ProductionConditionalOwnershipCheckV1 {
        self.analysis.payload.prerequisites()
    }

    pub fn mandatory_bounds_failure(&self) -> Option<&str> {
        self.analysis.payload.mandatory_bounds_failure()
    }

    pub fn rows(&self) -> &[ProductionConditionalOwnershipRowV1] {
        self.analysis.payload.rows()
    }

    pub fn selections(&self) -> &[ProductionConditionalOwnershipSiteV1] {
        &self.analysis.selections
    }

    /// These obligations have no complete-pipeline receipt in this owner.
    pub fn pending_pipeline_checks(&self) -> &'static [crate::KernelCheckPassKindV1] {
        &crate::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
    }
}

fn resource(error: ProductionAnalysisResourceLimitV1) -> ProductionSessionErrorV1 {
    ProductionSessionErrorV1::AnalysisResourceLimit {
        phase: error.phase,
        producing_pass: None,
        resource: error.resource,
    }
}

fn overflow() -> ProductionSessionErrorV1 {
    resource(ProductionAnalysisResourceLimitV1 {
        phase: Phase::HierarchicalOwnership,
        resource: "conditional owner work or capacity",
    })
}

fn selection(index: usize, reason: &'static str) -> ProductionSessionErrorV1 {
    ProductionSessionErrorV1::ConditionalOwnership(
        ProductionConditionalOwnershipErrorV1::Selection { index, reason },
    )
}

fn analysis_error(error: ConditionalOwnershipAnalysisErrorV1) -> ProductionSessionErrorV1 {
    use crate::production_analysis::conditional_analysis_v1::ConditionalOwnershipSelectionErrorV1 as E;
    match error {
        ConditionalOwnershipAnalysisErrorV1::Resource(error) => resource(error),
        ConditionalOwnershipAnalysisErrorV1::Contracts(report) => {
            ProductionSessionErrorV1::ConditionalOwnership(
                ProductionConditionalOwnershipErrorV1::Contracts(report),
            )
        }
        ConditionalOwnershipAnalysisErrorV1::Selection { index, reason } => selection(
            index,
            match reason {
                E::Empty => "empty",
                E::Limit => "limit",
                E::Owner => "foreign live owner",
                E::MissingOperation => "missing live operation",
                E::NotContract => "not a live contract",
                E::WrongView => "wrong live view",
                E::Duplicate => "duplicate live operation",
            },
        ),
    }
}

fn bound(work: usize, retained: usize) -> Result<Bound, ProductionSessionErrorV1> {
    Bound::checked_phase(Phase::HierarchicalOwnership, work, retained, 0).map_err(resource)
}

fn bytes<T>(count: usize) -> Result<usize, ProductionSessionErrorV1> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(overflow)
}

impl ProductionPlironSessionV1 {
    pub(super) fn analysis_limits_after_ownership_bindings_v1(
        &self,
    ) -> Result<ProductionAnalysisResourceLimitsV1, ProductionSessionErrorV1> {
        self.analysis_resource_limits()
            .remaining_after_retained(
                Phase::HierarchicalOwnership,
                self.ownership_binding_resources,
            )
            .map_err(resource)
    }

    fn reserve_ownership_bindings_v1(
        &mut self,
        additional: Bound,
    ) -> Result<(), ProductionSessionErrorV1> {
        let cumulative = self
            .ownership_binding_resources
            .checked_then_retain(additional, Phase::HierarchicalOwnership)
            .map_err(resource)?;
        self.analysis_resource_limits()
            .require(Phase::HierarchicalOwnership, cumulative)
            .map_err(resource)?;
        self.ownership_binding_resources = cumulative;
        Ok(())
    }

    pub(super) fn prepare_ownership_occurrence_storage_v1(
        &mut self,
        kernel: &ProductionRankedKernelV1,
        tree_work: usize,
    ) -> Result<Vec<OwnershipOccurrenceV1>, ProductionSessionErrorV1> {
        // tree_work bounds the block/operation scan and the subsequent writes.
        self.reserve_ownership_bindings_v1(bound(tree_work, 0)?)?;
        let count = kernel
            .blocks()
            .iter()
            .flat_map(|block| block.operations())
            .filter(|operation| {
                matches!(
                    operation,
                    ProductionRankedOperationV1::OwnershipContract { .. }
                )
            })
            .count();
        self.reserve_ownership_bindings_v1(bound(0, bytes::<OwnershipOccurrenceV1>(count)?)?)?;
        let mut occurrences = Vec::new();
        occurrences
            .try_reserve_exact(count)
            .map_err(|_| overflow())?;
        if occurrences.capacity() != count {
            return Err(overflow());
        }
        Ok(occurrences)
    }

    /// Consumes the genuine constructed graph into a pending owner. Selections
    /// request conditional analysis, never establish coverage or source replay.
    /// Errors and contained upstream panics drop the consumed session.
    ///
    /// ```compile_fail
    /// use fe2o3_pliron::*;
    /// fn reuse(session: ProductionPlironSessionV1,
    ///     stage: ProductionStageHandleV1<ConstructedGraphStageV1>,
    ///     root: ProductionRootHandleV1<ConstructedGraphStageV1>) {
    ///     let _ = session.prepare_conditional_ranked_analysis_v1(stage, root, &[]);
    ///     let _ = session.is_poisoned();
    /// }
    /// ```
    pub fn prepare_conditional_ranked_analysis_v1(
        mut self,
        stage: ProductionStageHandleV1<ConstructedGraphStageV1>,
        root: ProductionRootHandleV1<ConstructedGraphStageV1>,
        selections: &[ProductionConditionalOwnershipSiteV1],
    ) -> Result<ProductionConditionalRankedAnalysisV1, ProductionSessionErrorV1> {
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.run_conditional_ranked_analysis_v1(&stage, &root, selections)
        }));
        let analysis = match result {
            Ok(result) => result?,
            Err(_) => {
                return Err(ProductionSessionErrorV1::Operation(
                    OperationHandleError::UpstreamPanicked,
                ));
            }
        };
        Ok(ProductionConditionalRankedAnalysisV1 {
            analysis,
            _stage: stage,
            _root: root,
            _session: self,
        })
    }

    fn run_conditional_ranked_analysis_v1(
        &mut self,
        stage: &ProductionStageHandleV1<ConstructedGraphStageV1>,
        root: &ProductionRootHandleV1<ConstructedGraphStageV1>,
        selections: &[ProductionConditionalOwnershipSiteV1],
    ) -> Result<PreparedConditionalAnalysisV1, ProductionSessionErrorV1> {
        self.root_shape(stage, root)?;
        if selections.is_empty() {
            return Err(selection(0, "empty"));
        }
        if selections.len() > crate::MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1 {
            return Err(selection(
                crate::MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1,
                "limit",
            ));
        }
        let record = self
            .constructed_roots
            .get(&stage.identity)
            .ok_or(ProductionSessionErrorV1::StaleStage)?;
        if record.production_pipeline_report.is_some()
            || record.production_analysis_resource_upper_bound.is_some()
            || record.ranked_analysis_binding.is_some()
        {
            return Err(ProductionSessionErrorV1::StaleStage);
        }
        let kernel = record
            .ranked_kernel
            .as_ref()
            .ok_or(ProductionSessionErrorV1::WrongConstructionKind)?;
        let mut resources =
            ProductionAnalysisResourceContractV1::new(self.analysis_resource_limits());
        resources
            .admit_retained(
                Phase::HierarchicalOwnership,
                self.ownership_binding_resources,
            )
            .map_err(resource)?;
        let exact = middle_end_evidence_v4::derive_exact_ranked_graph_identity_with_resources_v1(
            kernel,
            &mut resources,
        )
        .map(ProductionExactGraphIdentityV1)
        .map_err(resource)?;
        if Some(exact) != record.exact_graph_identity {
            return Err(ProductionSessionErrorV1::RankedGraphChanged);
        }
        let function = FuncOp::from_operation(
            record
                .ranked_function
                .ok_or(ProductionSessionErrorV1::WrongConstructionKind)?,
        );
        let selection_work = selections
            .len()
            .checked_mul(
                record
                    .ownership_occurrences
                    .len()
                    .checked_add(selections.len())
                    .ok_or_else(overflow)?,
            )
            .ok_or_else(overflow)?;
        let occurrences = record.ownership_occurrences.len();
        let work = occurrences
            .checked_add(selection_work)
            .ok_or_else(overflow)?;
        let retained = bytes::<ProductionConditionalOwnershipSiteV1>(selections.len())?
            .checked_add(bytes::<ConditionalOwnershipSelectionV1<'_>>(
                selections.len(),
            )?)
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<ProductionConditionalRankedAnalysisV1>())
            })
            .ok_or_else(overflow)?;
        resources
            .admit_retained(Phase::HierarchicalOwnership, bound(work, retained)?)
            .map_err(resource)?;
        let capture_limits = resources
            .remaining(Phase::StructuralIdentity)
            .map_err(resource)?;
        let mut provider =
            LivePlironStructuralIdentityProviderV1::new(&self.inner.context, &function);
        let mutation_epoch = provider
            .mutation_epoch()
            .map_err(|_| ProductionSessionErrorV1::RankedGraphChanged)?;
        let capture = provider
            .capture_with_resource_limits_v1(capture_limits)
            .map_err(|error| match error {
                IdentityCaptureFailureV1::ResourceLimit(error) => resource(error),
                IdentityCaptureFailureV1::Unavailable {
                    source_code,
                    detail,
                } => ProductionSessionErrorV1::RankedPassPreservation(
                    crate::PlironPassPreservationErrorV1::IdentityUnavailable {
                        source_code,
                        detail,
                    },
                ),
            })?;
        resources
            .admit_retained(Phase::StructuralIdentity, capture.resource_upper_bound)
            .map_err(resource)?;
        let initial = resources.cumulative();
        let mut analyses = PlironAnalysisManagerV1::new_with_resource_contract(
            &function,
            capture.input_census,
            initial,
            capture.resource_upper_bound.retained_storage_upper_bound(),
            self.analysis_resource_limits(),
        )
        .map_err(resource)?;
        let mut selected = Vec::new();
        let mut live = Vec::new();
        selected
            .try_reserve_exact(selections.len())
            .map_err(|_| overflow())?;
        live.try_reserve_exact(selections.len())
            .map_err(|_| overflow())?;
        if selected.capacity() != selections.len() || live.capacity() != selections.len() {
            return Err(overflow());
        }
        for (index, request) in selections.iter().enumerate() {
            let occurrence = record
                .ownership_occurrences
                .iter()
                .find(|site| {
                    site.site.block == request.block && site.site.operation == request.operation
                })
                .ok_or_else(|| selection(index, "missing recipe contract"))?;
            if occurrence.site.view != request.view {
                return Err(selection(index, "wrong recipe view"));
            }
            if selected.contains(request) {
                return Err(selection(index, "duplicate recipe contract"));
            }
            selected.push(*request);
            live.push(ConditionalOwnershipSelectionV1::new(
                &self.inner.context,
                &function,
                occurrence.operation,
                occurrence.view,
            ));
        }
        let payload = run_conditional_ownership_analysis_v1(
            &self.inner.context,
            &function,
            &mut analyses,
            &live,
        )
        .map_err(analysis_error)?
        .into_payload();
        drop(live);
        // Validate the complete captured roster, including unselected rows.
        // Identical printed replacement operations still have different custody.
        if payload.function() != function.get_operation()
            || payload.rows().len() != occurrences
            || record
                .ownership_occurrences
                .iter()
                .zip(payload.rows())
                .any(|(occurrence, row)| {
                    row.operation() != occurrence.operation || row.view() != occurrence.view
                })
        {
            return Err(ProductionSessionErrorV1::RankedGraphChanged);
        }
        if provider
            .mutation_epoch()
            .map_err(|_| ProductionSessionErrorV1::RankedGraphChanged)?
            != mutation_epoch
        {
            return Err(ProductionSessionErrorV1::RankedGraphChanged);
        }
        self.require_live_graph_snapshot_v1(&root.operation, root.graph_snapshot)?;
        Ok(PreparedConditionalAnalysisV1 {
            payload,
            selections: selected,
            _identity: capture.snapshot,
            _mutation_epoch: mutation_epoch,
            _analyses: analyses,
        })
    }
}

include!("conditional_pipeline_v1.rs");

#[cfg(test)]
#[path = "conditional_ranked_v1_tests.rs"]
mod tests;
