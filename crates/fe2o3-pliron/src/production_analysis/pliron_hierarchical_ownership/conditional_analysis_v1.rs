//! Pending ownership obligations, without coverage or lowering authority.

use super::*;
use crate::production_analysis::{
    pliron_invocation_trace::{
        preflight_execution_layout_resource_upper_bound_v1,
        preflight_invocation_trace_resource_upper_bound_v1,
    },
    pliron_presburger_adapter::preflight_presburger_resource_upper_bound_v1,
    pliron_provenance_alias::preflight_provenance_alias_resource_upper_bound_v1,
    pliron_race::preflight_race_resource_upper_bound_v1,
    pliron_ranked_bounds::preflight_ranked_bounds_resource_upper_bound_v1,
    pliron_sparse_index::preflight_sparse_index_resource_upper_bound_v1,
};
use pliron::context::Ptr;

/// An inert request naming an actual contract occurrence, not a source binding.
#[derive(Clone, Copy)]
pub(crate) struct ConditionalOwnershipSelectionV1<'ctx> {
    context: &'ctx Context,
    function: Ptr<Operation>,
    operation: Ptr<Operation>,
    view: Value,
}

impl<'ctx> ConditionalOwnershipSelectionV1<'ctx> {
    /// No pointer is dereferenced and no relationship is certified here.
    pub(crate) fn new(
        context: &'ctx Context,
        function: &FuncOp,
        operation: Ptr<Operation>,
        view: Value,
    ) -> Self {
        Self {
            context,
            function: function.get_operation(),
            operation,
            view,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConditionalOwnershipSelectionErrorV1 {
    Empty,
    Limit,
    Owner,
    MissingOperation,
    NotContract,
    WrongView,
    Duplicate,
}

#[derive(Debug)]
pub(crate) enum ConditionalOwnershipAnalysisErrorV1 {
    Resource(ProductionAnalysisResourceLimitV1),
    Selection {
        index: usize,
        reason: ConditionalOwnershipSelectionErrorV1,
    },
    /// Collection cannot produce a contract roster; this is the legacy report.
    Contracts(HierarchicalOwnershipReportV1),
}

impl From<ProductionAnalysisResourceLimitV1> for ConditionalOwnershipAnalysisErrorV1 {
    fn from(error: ProductionAnalysisResourceLimitV1) -> Self {
        Self::Resource(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionalOwnershipBlockerV1 {
    Prerequisite,
    Extent,
    Trace,
    UnsupportedSelectionProfile,
    /// No constructor in this module can discharge this dependency.
    CanonicalCoverageAndSourceReplay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionalOwnershipCheckV1 {
    NotApplicable,
    Checked,
    /// This local check failed; the findings retain Incomplete versus Rejected.
    Rejected,
    Blocked(ConditionalOwnershipBlockerV1),
}

/// One actual contract, in inventory order. Checked is local to the named check.
pub struct ConditionalOwnershipRowV1 {
    operation: Ptr<Operation>,
    location: HierarchicalOwnershipLocationV1,
    view: Value,
    selected: bool,
    effect_site: ConditionalOwnershipCheckV1,
    extent: ConditionalOwnershipCheckV1,
    trace: ConditionalOwnershipCheckV1,
    coverage: ConditionalOwnershipCheckV1,
    findings: Vec<HierarchicalOwnershipFindingV1>,
    regions: Vec<HierarchicalOwnershipRegionV1>,
}

impl fmt::Debug for ConditionalOwnershipRowV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("ConditionalOwnershipRowV1")
            .field("location", &self.location)
            .field("selected", &self.selected)
            .field("effect_site", &self.effect_site)
            .field("extent", &self.extent)
            .field("trace", &self.trace)
            .field("coverage", &self.coverage)
            .field("findings", &self.findings)
            .field("regions", &self.regions)
            .finish()
    }
}

impl ConditionalOwnershipRowV1 {
    pub(crate) const fn operation(&self) -> Ptr<Operation> {
        self.operation
    }
    pub const fn location(&self) -> HierarchicalOwnershipLocationV1 {
        self.location
    }
    pub(crate) const fn view(&self) -> Value {
        self.view
    }
    pub const fn selected(&self) -> bool {
        self.selected
    }
    pub const fn effect_site(&self) -> ConditionalOwnershipCheckV1 {
        self.effect_site
    }
    pub const fn extent(&self) -> ConditionalOwnershipCheckV1 {
        self.extent
    }
    pub const fn trace(&self) -> ConditionalOwnershipCheckV1 {
        self.trace
    }
    pub const fn coverage(&self) -> ConditionalOwnershipCheckV1 {
        self.coverage
    }
    pub fn findings(&self) -> &[HierarchicalOwnershipFindingV1] {
        &self.findings
    }
    pub fn regions(&self) -> &[HierarchicalOwnershipRegionV1] {
        &self.regions
    }
}

/// Diagnostic-only borrow of a context, not an epoch or transaction capability.
/// No clean-report conversion, established-coverage constructor or authority API.
pub(crate) struct ConditionalOwnershipAnalysisV1<'ctx> {
    _context: &'ctx Context,
    payload: ConditionalOwnershipPayloadV1,
}

/// Arena-relative diagnostics. Only the consuming production session may keep
/// this payload beyond the diagnostic borrow; it must retain the same context.
#[derive(Debug)]
pub(crate) struct ConditionalOwnershipPayloadV1 {
    function: Ptr<Operation>,
    legacy_report: HierarchicalOwnershipReportV1,
    prerequisites: ConditionalOwnershipCheckV1,
    mandatory_bounds_failure: Option<String>,
    rows: Vec<ConditionalOwnershipRowV1>,
}

impl fmt::Debug for ConditionalOwnershipAnalysisV1<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.payload.fmt(out)
    }
}

impl ConditionalOwnershipAnalysisV1<'_> {
    pub(crate) fn into_payload(self) -> ConditionalOwnershipPayloadV1 {
        self.payload
    }
}

impl std::ops::Deref for ConditionalOwnershipAnalysisV1<'_> {
    type Target = ConditionalOwnershipPayloadV1;

    fn deref(&self) -> &Self::Target {
        &self.payload
    }
}

impl ConditionalOwnershipPayloadV1 {
    pub(crate) const fn function(&self) -> Ptr<Operation> {
        self.function
    }

    pub(crate) const fn legacy_report(&self) -> &HierarchicalOwnershipReportV1 {
        &self.legacy_report
    }
    pub(crate) const fn prerequisites(&self) -> ConditionalOwnershipCheckV1 {
        self.prerequisites
    }
    /// Retained even when the legacy trace exit masks a mandatory bounds failure.
    pub(crate) fn mandatory_bounds_failure(&self) -> Option<&str> {
        self.mandatory_bounds_failure.as_deref()
    }
    pub(crate) fn rows(&self) -> &[ConditionalOwnershipRowV1] {
        &self.rows
    }
}

fn resource_error(resource: &'static str) -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
        resource,
    }
}

fn selection_error(
    index: usize,
    reason: ConditionalOwnershipSelectionErrorV1,
) -> ConditionalOwnershipAnalysisErrorV1 {
    ConditionalOwnershipAnalysisErrorV1::Selection { index, reason }
}

/// New selection/row work and retained diagnostics, additional to both legacy
/// analysis bounds. Units are the existing PLIRON logical resource domain.
fn pending_upper_bound(
    census: ProductionAnalysisInputCensusV1,
    selections: usize,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let contracts = census
        .ownership_contracts
        .min(MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1);
    let scans = checked_ownership_sum_v1(&[selections, contracts, 8])?;
    let work = checked_ownership_sum_v1(&[
        // Additional constant-time observable-write name eligibility queries
        // share the contract inventory traversal, before any name formatting.
        census.operations,
        checked_ownership_product_v1(
            scans,
            census
                .operations
                .checked_add(1)
                .ok_or_else(ownership_resource_overflow_v1)?,
        )?,
        checked_ownership_product_v1(selections, selections)?,
        checked_ownership_product_v1(
            contracts,
            selections
                .checked_add(8)
                .ok_or_else(ownership_resource_overflow_v1)?,
        )?,
        checked_ownership_product_v1(census.identifier_bytes, 4)?,
        MAX_HIERARCHICAL_OWNERSHIP_DIAGNOSTIC_BYTES_V1,
    ])?;
    let retained = checked_ownership_sum_v1(&[
        std::mem::size_of::<ConditionalOwnershipAnalysisV1<'_>>(),
        checked_ownership_product_v1(contracts, std::mem::size_of::<ConditionalOwnershipRowV1>())?,
        // The collected contracts and nested error strings can coexist with
        // both diagnostic results. Conservatively retain their reservation.
        checked_ownership_product_v1(contracts, std::mem::size_of::<ContractV1>())?,
        checked_ownership_product_v1(census.identifier_bytes, 4)?,
        checked_ownership_product_v1(
            contracts
                .checked_add(4)
                .ok_or_else(ownership_resource_overflow_v1)?,
            MAX_HIERARCHICAL_OWNERSHIP_DIAGNOSTIC_BYTES_V1,
        )?,
    ])?;
    ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
        work,
        retained,
        0,
    )
}

fn validate_selections(
    context: &Context,
    function: &FuncOp,
    prepared: &PreparedOwnershipContractsV1,
    selections: &[ConditionalOwnershipSelectionV1<'_>],
) -> Result<(), ConditionalOwnershipAnalysisErrorV1> {
    use ConditionalOwnershipSelectionErrorV1 as Error;
    for (index, selection) in selections.iter().enumerate() {
        if !std::ptr::eq(context, selection.context)
            || selection.function != function.get_operation()
        {
            return Err(selection_error(index, Error::Owner));
        }
        let site = prepared
            .inventory
            .operations()
            .iter()
            .find(|site| site.pointer() == selection.operation)
            .ok_or_else(|| selection_error(index, Error::MissingOperation))?;
        let location = HierarchicalOwnershipLocationV1 {
            block: site.block(),
            operation: site.operation(),
        };
        let contract = prepared
            .contracts
            .iter()
            .find(|contract| contract.location == location)
            .ok_or_else(|| selection_error(index, Error::NotContract))?;
        if contract.view != selection.view {
            return Err(selection_error(index, Error::WrongView));
        }
        if selections[..index]
            .iter()
            .any(|previous| previous.operation == selection.operation)
        {
            return Err(selection_error(index, Error::Duplicate));
        }
    }
    Ok(())
}

/// Reserve every prerequisite on the inherited manager before construction.
/// Existing cached work may be conservatively charged again; this leaf never
/// creates a default manager or assumes a cache was prepaid by its caller.
fn prepare_resources(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    census: ProductionAnalysisInputCensusV1,
    diagnostic_name_bytes: usize,
) -> Result<(), ProductionAnalysisResourceLimitV1> {
    use ProductionAnalysisResourcePhaseV1 as Phase;
    let sparse = preflight_sparse_index_resource_upper_bound_v1(
        context,
        function,
        census,
        analyses.remaining_resource_limits(Phase::SparseIndex)?,
    )?;
    analyses.admit_retained_resource_upper_bound(Phase::SparseIndex, sparse)?;
    analyses.prepare_sparse_indices(context, function);
    let presburger = preflight_presburger_resource_upper_bound_v1(
        analyses.sparse_indices().ok(),
        analyses.remaining_resource_limits(Phase::Presburger)?,
    )?;
    analyses.admit_retained_resource_upper_bound(Phase::Presburger, presburger)?;
    analyses.prepare_presburger(context, function);
    let layout = preflight_execution_layout_resource_upper_bound_v1(
        census,
        analyses.remaining_resource_limits(Phase::LaunchContract)?,
    )?;
    analyses.admit_retained_resource_upper_bound(Phase::LaunchContract, layout)?;
    analyses.prepare_execution_layout(context, function);
    let trace = preflight_invocation_trace_resource_upper_bound_v1(
        context,
        analyses.function_inventory().ok(),
        census,
        analyses.sparse_indices().ok(),
        analyses.execution_layout().ok().flatten(),
        analyses.remaining_resource_limits(Phase::InvocationTrace)?,
    )?;
    analyses
        .admit_retained_resource_upper_bound(Phase::InvocationTrace, trace.attempt_upper_bound())?;
    analyses.prepare_exact_trace(context, function);
    let trace_admission = trace.exact_admission(analyses.exact_trace())?;
    let provenance = preflight_provenance_alias_resource_upper_bound_v1(
        census,
        analyses.remaining_resource_limits(Phase::ProvenanceAlias)?,
    )?;
    analyses.admit_retained_resource_upper_bound(Phase::ProvenanceAlias, provenance)?;
    analyses.prepare_provenance_alias(context, function);
    let bounds = preflight_ranked_bounds_resource_upper_bound_v1(
        census,
        analyses.remaining_resource_limits(Phase::MemoryBounds)?,
    )?;
    let race = match analyses.sparse_indices() {
        Ok(sparse) => preflight_race_resource_upper_bound_v1(
            context,
            function,
            analyses.function_inventory().ok(),
            census,
            sparse,
            analyses.execution_layout().ok().flatten(),
            analyses.remaining_resource_limits(Phase::RaceFreedom)?,
        )?,
        // The shared prefix exits before bounds/race if sparse preparation fails.
        Err(_) => ProductionAnalysisResourceUpperBoundV1::default(),
    };
    // Display labels are excluded from the structural census. Extend only the
    // local ownership bound, not the authenticated census used by other passes.
    let mut ownership_census = census;
    ownership_census.identifier_bytes =
        checked_ownership_sum_v1(&[census.identifier_bytes, diagnostic_name_bytes])?;
    let ownership = preflight_hierarchical_ownership_resource_upper_bound_v1(
        ownership_census,
        trace_admission,
        analyses.remaining_resource_limits(Phase::HierarchicalOwnership)?,
    )?;
    let both = ownership
        .checked_then_retain(ownership, Phase::HierarchicalOwnership)?
        .checked_with_nested_sequence_discard(&[bounds, race], Phase::HierarchicalOwnership)?;
    analyses.admit_retained_resource_upper_bound(Phase::HierarchicalOwnership, both)
}

/// Runs diagnostics and available residual ownership checks. All selected
/// coverage remains pending; no source/canonical fact is accepted by this API.
///
/// The metered manager, census and caches belong to the caller's immutable
/// function. Reservations are conservative and monotonic, including errors;
/// their lifetime/transfer remains with that manager, as in the existing domain.
/// This does not consume or reset the separate canonical/candidate budget.
pub(crate) fn run_conditional_ownership_analysis_v1<'ctx>(
    context: &'ctx Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    selections: &[ConditionalOwnershipSelectionV1<'ctx>],
) -> Result<ConditionalOwnershipAnalysisV1<'ctx>, ConditionalOwnershipAnalysisErrorV1> {
    use ConditionalOwnershipBlockerV1 as Blocker;
    use ConditionalOwnershipCheckV1 as Check;
    use ConditionalOwnershipSelectionErrorV1 as SelectionError;
    let census = analyses
        .input_census()
        .ok_or_else(|| resource_error("conditional ownership requires metered census"))?;
    if selections.is_empty() {
        return Err(selection_error(0, SelectionError::Empty));
    }
    if selections.len() > MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1 {
        return Err(selection_error(
            MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1,
            SelectionError::Limit,
        ));
    }
    analyses.admit_retained_resource_upper_bound(
        ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
        pending_upper_bound(census, selections.len())?,
    )?;
    analyses.prepare_function_inventory(context, function);
    let mut diagnostic_name_bytes = 0_usize;
    if let Ok(inventory) = analyses.function_inventory() {
        if inventory.operations().len() != census.operations
            || inventory.blocks().len() != census.blocks
        {
            return Err(resource_error("conditional ownership inventory census mismatch").into());
        }
        let mut contracts = 0_usize;
        for site in inventory.operations() {
            let operation = Operation::get_op_dyn(site.pointer(), context);
            if let Some(contract) = operation.downcast_ref::<OwnershipContractOp>() {
                contracts += 1;
                if contracts <= MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1
                    && site.pointer().deref(context).get_num_operands() == 1
                {
                    diagnostic_name_bytes = checked_ownership_sum_v1(&[
                        diagnostic_name_bytes,
                        contract
                            .view(context)
                            .unique_name_byte_len(context)
                            .ok_or_else(|| {
                                resource_error("conditional ownership name size overflow")
                            })?,
                    ])?;
                }
            }
            // The reused prerequisites retain names for uncontracted writes,
            // too. Count each observable write, conservatively including names
            // already counted at a contract or a previous access. This shares
            // the prepaid inventory scan and does not allocate display text.
            if let Some(access) = operation.downcast_ref::<RankedAccessOp>()
                && access
                    .kind(context)
                    .is_some_and(|kind| kind.writes_memory())
                && site.pointer().deref(context).get_num_operands() > 0
            {
                let view = access.view(context);
                let view_op = view
                    .defining_op()
                    .map(|definition| Operation::get_op_dyn(definition, context))
                    .and_then(|definition| definition.downcast_ref::<RankedViewOp>().copied());
                if view_op
                    .is_some_and(|view| view.memory_space(context) == Some(MemorySpaceAttr::Global))
                {
                    diagnostic_name_bytes = checked_ownership_sum_v1(&[
                        diagnostic_name_bytes,
                        view.unique_name_byte_len(context).ok_or_else(|| {
                            resource_error("conditional ownership name size overflow")
                        })?,
                    ])?;
                }
            }
        }
        if contracts != census.ownership_contracts {
            return Err(resource_error("conditional ownership contract census mismatch").into());
        }
    }
    let names = checked_ownership_product_v1(diagnostic_name_bytes, 4)?;
    analyses.admit_retained_resource_upper_bound(
        ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
        ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
            names,
            names,
            0,
        )?,
    )?;
    let prepared = prepare_ownership_contracts_v1(context, function, analyses)
        .map_err(ConditionalOwnershipAnalysisErrorV1::Contracts)?;
    validate_selections(context, function, &prepared, selections)?;
    prepare_resources(context, function, analyses, census, diagnostic_name_bytes)?;
    let prerequisites = prepare_ownership_prerequisites_v1(context, function, analyses, &prepared);
    let mandatory_bounds_failure = prerequisites
        .as_ref()
        .ok()
        .and_then(|prepared| prepared.mandatory_bounds_failure.clone());
    let prerequisite_check = if prerequisites.is_ok() && mandatory_bounds_failure.is_none() {
        Check::Checked
    } else {
        Check::Rejected
    };
    let grid = prerequisites.as_ref().ok().map(|prepared| prepared.grid);
    let legacy_report = match prerequisites {
        Ok(prerequisites) => run_prepared_ownership_v1(
            context,
            function,
            analyses,
            &prepared.inventory,
            prepared.contracts.as_slice(),
            prepared.coverage_summary,
            prerequisites,
        ),
        Err(report) => report,
    };
    let mut rows = Vec::new();
    rows.try_reserve_exact(prepared.contracts.len())
        .map_err(|_| resource_error("conditional ownership row allocation"))?;
    if rows.capacity() > prepared.contracts.len() {
        let surplus = checked_ownership_product_v1(
            rows.capacity() - prepared.contracts.len(),
            std::mem::size_of::<ConditionalOwnershipRowV1>(),
        )?;
        analyses.admit_retained_resource_upper_bound(
            ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
            ProductionAnalysisResourceUpperBoundV1::checked_phase(
                ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
                0,
                surplus,
                0,
            )?,
        )?;
    }
    for contract in &prepared.contracts {
        let operation = prepared
            .inventory
            .operations()
            .iter()
            .find(|site| {
                site.block() == contract.location.block
                    && site.operation() == contract.location.operation
            })
            .ok_or_else(|| resource_error("conditional ownership contract occurrence missing"))?
            .pointer();
        let selected = selections
            .iter()
            .any(|selection| selection.operation == operation);
        let mut row = ConditionalOwnershipRowV1 {
            operation,
            location: contract.location,
            view: contract.view,
            selected,
            effect_site: Check::NotApplicable,
            extent: Check::NotApplicable,
            trace: Check::NotApplicable,
            coverage: Check::Blocked(Blocker::Prerequisite),
            findings: Vec::new(),
            regions: Vec::new(),
        };
        if contract.coverage == OwnershipCoverageAttr::ExactEffectDomain {
            row.effect_site =
                match validate_effect_domain_site(context, &prepared.inventory, contract) {
                    Ok(()) => Check::Checked,
                    Err(finding) => {
                        row.findings.push(*finding);
                        Check::Rejected
                    }
                };
            row.coverage = if selected {
                Check::Blocked(Blocker::UnsupportedSelectionProfile)
            } else if prerequisite_check != Check::Checked {
                Check::Blocked(Blocker::Prerequisite)
            } else {
                row.effect_site
            };
            rows.push(row);
            continue;
        }
        let extents = match analyses.sparse_indices() {
            Ok(sparse) => match resolve_extents(context, sparse, contract) {
                Ok(extents) => {
                    row.extent = Check::Checked;
                    Some(extents)
                }
                Err(finding) => {
                    row.findings.push(*finding);
                    row.extent = Check::Blocked(Blocker::Extent);
                    None
                }
            },
            Err(_) => {
                row.extent = Check::Blocked(Blocker::Prerequisite);
                None
            }
        };
        let element_count = if contract.coverage == OwnershipCoverageAttr::CollectiveContributions {
            None
        } else if let Some(extents) = &extents {
            match bounded_element_count(&contract.view_name, extents) {
                Ok(count) => Some(count),
                Err(finding) => {
                    row.findings.push(*finding);
                    row.extent = Check::Blocked(Blocker::Extent);
                    None
                }
            }
        } else {
            None
        };
        let traces = analyses.exact_trace().ok();
        row.trace = if traces.is_some() {
            Check::Checked
        } else {
            Check::Blocked(Blocker::Trace)
        };
        row.coverage = if selected && !supported_selection(context, contract) {
            Check::Blocked(Blocker::UnsupportedSelectionProfile)
        } else if prerequisite_check != Check::Checked {
            Check::Blocked(Blocker::Prerequisite)
        } else if selected {
            Check::Blocked(Blocker::CanonicalCoverageAndSourceReplay)
        } else if row.extent != Check::Checked {
            Check::Blocked(Blocker::Extent)
        } else if let (Some(traces), Some(grid)) = (traces, grid) {
            analyze_contract(
                context,
                contract,
                extents.as_deref(),
                element_count,
                traces,
                grid,
                &mut row.findings,
                &mut row.regions,
            );
            if row.findings.is_empty() {
                Check::Checked
            } else {
                Check::Rejected
            }
        } else {
            Check::Blocked(Blocker::Trace)
        };
        rows.push(row);
    }
    Ok(ConditionalOwnershipAnalysisV1 {
        _context: context,
        payload: ConditionalOwnershipPayloadV1 {
            function: function.get_operation(),
            legacy_report,
            prerequisites: prerequisite_check,
            mandatory_bounds_failure,
            rows,
        },
    })
}

fn supported_selection(context: &Context, contract: &ContractV1) -> bool {
    contract.coverage == OwnershipCoverageAttr::TotalView
        && contract.partition == OwnershipPartitionAttr::ExactSets
        && contract.view_op.memory_space(context) == Some(MemorySpaceAttr::Global)
        && contract
            .view_op
            .view_type(context)
            .is_some_and(|ty| ty.deref(context).shape() == [DYNAMIC_EXTENT])
}

#[cfg(test)]
#[path = "conditional_analysis_v1_tests.rs"]
mod tests;
