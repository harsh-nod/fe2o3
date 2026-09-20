//! Strict original-N consistency at the existing source-progress proof mark.
//! This owning stage is not legacy evidence, an optimizer or artifact admission.
#![allow(dead_code)]
use super::canonical_assertion_facts_v1::ProjectedAssertionConditionV1;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionScalarSsaEmissionErrorV1 as CaptureError,
    ProductionScalarSsaEmissionOwnerV1 as Capture,
    ProductionU32BoundSnapshotGuardRequestV1 as Request,
    ProductionU32GuardConsistencyV1 as GuardOutcome, ProductionU32GuardReportV1 as GuardReport,
};
use fe2o3_mir_model::{
    SemanticU32InductionBoundSnapshotErrorV1 as ModelError,
    SemanticU32InductionBoundSnapshotMeterV1 as ModelMeter,
    SemanticU32InductionBoundSnapshotReportV1 as Report,
    analyze_semantic_u32_induction_bound_snapshots_with_meter_v1,
};
use std::mem::size_of;

#[path = "guarded_source_progress_resources_v1.rs"]
pub(crate) mod resources;
#[cfg(test)]
#[path = "guarded_source_progress_v1_tests.rs"]
mod tests;
type Result<T> = std::result::Result<T, ProductionRankedProjectionErrorV1>;

pub(crate) fn resource(error: Resource) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(
        error,
    ))
}
fn capture_error(error: CaptureError) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::CanonicalAssertions(
        CanonicalAssertionErrorV1::GuardedProgress(Box::new(error)),
    )
}
fn mismatch(detail: &'static str) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::Incomplete(detail)
}

struct Meter<'a, 'w>(&'a mut Budget<'w>);
impl ModelMeter for Meter<'_, '_> {
    type Error = Resource;
    fn charge_work(&mut self, amount: usize) -> std::result::Result<(), Resource> {
        self.0.charge_work(amount)
    }
    fn reserve_storage(&mut self, amount: usize) -> std::result::Result<(), Resource> {
        self.0.reserve_storage(amount)
    }
}
fn model_error(error: ModelError<Resource>) -> ProductionRankedProjectionErrorV1 {
    match error {
        ModelError::Analysis(error) => capture_error(CaptureError::BoundSnapshotAnalysis(error)),
        ModelError::Meter(error) => resource(error),
    }
}

struct RootReport {
    root: SemanticFunctionIdV1,
    body: SemanticFunctionIdV1,
    report: Report,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Key {
    root: u32,
    function: u32,
    block: u32,
    statement: u32,
}

/// Sealed inert consumption coordinates; not a detached source proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ConsumedProgressSiteV1 {
    key: Key,
    report: usize,
    ordinal: usize,
    guard: usize,
    consumed: bool,
}
impl ConsumedProgressSiteV1 {
    pub(crate) fn root(&self) -> u32 {
        self.key.root
    }
    pub(crate) fn function(&self) -> u32 {
        self.key.function
    }
    pub(crate) fn block(&self) -> u32 {
        self.key.block
    }
    pub(crate) fn statement(&self) -> u32 {
        self.key.statement
    }
    pub(crate) fn certificate_ordinal(&self) -> usize {
        self.ordinal
    }
}

/// One original C and actual projected roots, never a self-borrowed guard result.
/// New receipts cover this wrapper and owned report/site capacities; inherited
/// ranked/source payload allocation domains are unchanged, not RSS accounting.
/// Full surviving entry-floor, Budget-slot and Work-ledger custody are replayed.
pub(crate) struct GuardedRankedSourceV1 {
    capture: Capture,
    roots: Box<[ProductionRankedRootProgramV1]>,
    reports: Vec<RootReport>,
    sites: Vec<ConsumedProgressSiteV1>,
    retained: usize,
    required_floor: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

impl ProductionRankedRootInputV1 {
    /// New route prepays the exact name allocation; LaunchContract has no heap
    /// fields. The containing vector/header is owned and prepaid by the caller.
    pub(crate) fn guarded_with_budget_v1(
        logical_name: &str,
        kernel_binding: [u8; 32],
        source_launch: &LaunchContract,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        Ok(Self {
            logical_name: resources::name(logical_name, budget).map_err(resource)?,
            kernel_binding,
            source_launch: source_launch.clone(),
        })
    }
    pub(crate) fn guarded_name_capacity_v1(&self) -> usize {
        self.logical_name.capacity()
    }
}

fn derive_reports(capture: &Capture, budget: &mut Budget<'_>) -> Result<(Vec<RootReport>, usize)> {
    let source = capture.original().semantic_ssa().source_semantic();
    let launch = capture.original().source_launch();
    let mut reports =
        resources::table::<RootReport>(launch.roots().len(), budget).map_err(resource)?;
    let mut retained = resources::bytes::<RootReport>(reports.capacity()).map_err(resource)?;
    for root in launch.roots() {
        let selection = select_body_with_budget(source, root.selected_root(), budget)?
            .ok_or_else(|| mismatch("guarded progress requires the actual selected source body"))?;
        let floor = budget.storage();
        let report = analyze_semantic_u32_induction_bound_snapshots_with_meter_v1(
            source,
            selection.body(),
            Default::default(),
            &mut Meter(budget),
        )
        .map_err(model_error)?;
        let payload = report
            .retained_storage()
            .checked_sub(size_of::<Report>())
            .ok_or_else(|| resource(Resource::Accounting))?;
        if reports.len() == reports.capacity() {
            return Err(resource(Resource::Accounting));
        }
        reports.push(RootReport {
            root: root.selected_root(),
            body: selection.body(),
            report,
        });
        retained = retained
            .checked_add(payload)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        let required = floor
            .checked_add(payload)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget
            .release_storage(
                budget
                    .storage()
                    .checked_sub(required)
                    .ok_or_else(|| resource(Resource::Accounting))?,
            )
            .map_err(resource)?;
    }
    Ok((reports, retained))
}

// Prepay the existing closed selector's bounded wrapper/ABI scans. Direct root
// selection remains logarithmic in the root roster; no duplicate CFG is built.
fn select_body_with_budget(
    source: &AdmittedInertSemanticMirV1,
    root: SemanticFunctionIdV1,
    budget: &mut Budget<'_>,
) -> Result<Option<SemanticKernelBodySelectionV1>> {
    budget
        .charge_work(32 + (usize::BITS - source.roots().len().leading_zeros()) as usize)
        .map_err(resource)?;
    if let Some(function) = source.functions().get(root.index() as usize)
        && let Some(entry) = function.blocks().get(function.entry().index() as usize)
        && let SemanticTerminatorKindV1::Call(call) = entry.terminator().kind()
    {
        let returned = call
            .destination()
            .and_then(|destination| {
                function
                    .blocks()
                    .get(destination.edge().target().index() as usize)
            })
            .map_or(0, |block| block.statements().len());
        let work = function
            .abi()
            .source_input_types()
            .len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(call.arguments().len().checked_mul(4)?))
            .and_then(|n| n.checked_add(entry.statements().len()))
            .and_then(|n| n.checked_add(returned))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.charge_work(work).map_err(resource)?;
    }
    Ok(source.select_kernel_body_for_root_v1(root))
}

fn derive_guard<'s>(
    capture: &'s Capture,
    reports: &[RootReport],
    budget: &mut Budget<'_>,
) -> Result<(GuardReport<'s>, usize)> {
    budget.charge_work(reports.len()).map_err(resource)?;
    let count = reports
        .iter()
        .try_fold(0usize, |n, row| {
            n.checked_add(row.report.certificates().len())
        })
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    budget
        .reserve_storage(size_of::<Vec<Request<'_>>>())
        .map_err(resource)?;
    let mut requests = resources::table::<Request<'_>>(count, budget).map_err(resource)?;
    let request_storage = size_of::<Vec<Request<'_>>>()
        .checked_add(resources::bytes::<Request<'_>>(requests.capacity()).map_err(resource)?)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    budget.charge_work(reports.len()).map_err(resource)?;
    for row in reports {
        for ordinal in 0..row.report.certificates().len() {
            budget.charge_work(1).map_err(resource)?;
            requests.push(Request::new(row.root, &row.report, ordinal));
        }
    }
    let (report, receipt) = capture
        .analyze_u32_bound_snapshot_guard_consistency_v1(&requests, Default::default(), budget)
        .map_err(capture_error)?;
    let retained = receipt.retained_storage();
    budget.reserve_storage(retained).map_err(resource)?;
    drop(requests);
    budget.release_storage(request_storage).map_err(resource)?;
    Ok((report, retained))
}

fn derive_sites(
    capture: &Capture,
    reports: &[RootReport],
    guards: &GuardReport<'_>,
    budget: &mut Budget<'_>,
) -> Result<Vec<ConsumedProgressSiteV1>> {
    budget.charge_work(1).map_err(resource)?;
    if !std::ptr::eq(capture, guards.owner()) {
        return Err(mismatch("guarded progress has a foreign original C owner"));
    }
    let mut sites = resources::table::<ConsumedProgressSiteV1>(guards.rows().len(), budget)
        .map_err(resource)?;
    let mut guard = 0;
    for (report_index, row) in reports.iter().enumerate() {
        budget.charge_work(1).map_err(resource)?;
        for (ordinal, certificate) in row.report.certificates().iter().enumerate() {
            budget.charge_work(8).map_err(resource)?;
            let actual = guards
                .rows()
                .get(guard)
                .ok_or_else(|| mismatch("guarded progress lost a complete guard request"))?;
            if actual.root() != row.root
                || actual.function() != row.body
                || actual.certificate_ordinal() != ordinal
            {
                return Err(mismatch(
                    "guarded progress substituted its root/report request",
                ));
            }
            let GuardOutcome::Joined(fact) = actual.outcome() else {
                return Err(mismatch(
                    "guarded progress original-N mapping is unavailable",
                ));
            };
            if !std::ptr::eq(fact.source(), capture.original())
                || fact.root() != row.root
                || fact.function() != row.body
                || fact.certificate_ordinal() != ordinal
                || fact.authorizes_compiler_transform()
            {
                return Err(mismatch(
                    "guarded progress substituted original-N fact custody",
                ));
            }
            if sites.len() == sites.capacity() {
                return Err(resource(Resource::Accounting));
            }
            sites.push(ConsumedProgressSiteV1 {
                key: Key {
                    root: row.root.index(),
                    function: row.body.index(),
                    block: certificate.checked_addition().block().block().index(),
                    statement: certificate.checked_addition().statement(),
                },
                report: report_index,
                ordinal,
                guard,
                consumed: false,
            });
            guard += 1;
        }
    }
    if guard != guards.rows().len() {
        return Err(mismatch("guarded progress has unrequested guard rows"));
    }
    let units = sites
        .len()
        .checked_mul(2 + (usize::BITS - sites.len().leading_zeros()) as usize)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    budget.charge_work(units).map_err(resource)?;
    sites.sort_unstable_by_key(|site| site.key);
    budget.charge_work(sites.len()).map_err(resource)?;
    if sites.windows(2).any(|rows| rows[0].key == rows[1].key) {
        return Err(mismatch(
            "guarded progress has duplicate claimed source producers",
        ));
    }
    Ok(sites)
}

impl GuardedRankedSourceV1 {
    /// Consumes an already RESERVED genuine C. Success keeps the complete
    /// returned receipt RESERVED on the same slot/ledger; every error drops C
    /// before restoring the unrelated entry floor. No old capture is projected
    /// first, and no borrowed guard result is retained beside its source owner.
    pub(crate) fn try_project_v1(
        capture: Capture,
        inputs: &[ProductionRankedRootInputV1],
        references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        let consumed = capture.retained_analysis_storage_v1();
        resources::owned(
            budget,
            consumed,
            resource,
            || capture_error(CaptureError::Panicked),
            |budget| {
                let delta = size_of::<Self>()
                    .checked_sub(size_of::<Capture>())
                    .ok_or_else(|| resource(Resource::Arithmetic))?;
                budget.reserve_storage(delta).map_err(resource)?;
                let (reports, report_storage) = derive_reports(&capture, budget)?;
                let (guards, guard_storage) = derive_guard(&capture, &reports, budget)?;
                let mut sites = derive_sites(&capture, &reports, &guards, budget)?;
                let site_storage = resources::bytes::<ConsumedProgressSiteV1>(sites.capacity())
                    .map_err(resource)?;
                let slot = budget as *mut _ as usize;
                let ledger = budget.work_ledger_identity_v1();
                let roots = {
                    let source =
                        RankedProjectionSourceV1::from_materialized_checked(capture.original())?;
                    let mut progress = GuardedSourceProgressV1 {
                        capture: &capture,
                        reports: &reports,
                        guards: &guards,
                        sites: &mut sites,
                        slot,
                        ledger,
                        floor: budget.storage(),
                    };
                    let roots = project_ranked_roots_with_progress_v1(
                        &source,
                        inputs,
                        references,
                        budget,
                        Some(&mut progress),
                    )?;
                    progress.finish(budget)?;
                    roots
                };
                drop(guards);
                budget.release_storage(guard_storage).map_err(resource)?;
                let retained = consumed
                    .checked_add(delta)
                    .and_then(|n| n.checked_add(report_storage))
                    .and_then(|n| n.checked_add(site_storage))
                    .ok_or_else(|| resource(Resource::Arithmetic))?;
                Ok((
                    Self {
                        capture,
                        roots,
                        reports,
                        sites,
                        retained,
                        required_floor: budget.storage(),
                        slot,
                        ledger,
                    },
                    retained,
                ))
            },
        )
    }

    pub(crate) fn capture(&self) -> &Capture {
        &self.capture
    }
    pub(crate) fn roots(&self) -> &[ProductionRankedRootProgramV1] {
        &self.roots
    }
    pub(crate) fn sites(&self) -> &[ConsumedProgressSiteV1] {
        &self.sites
    }
    pub(crate) fn retained_storage(&self) -> usize {
        self.retained
    }
    pub(crate) const fn grants_authority(&self) -> bool {
        false
    }
    pub(crate) fn certificate_count(&self, root: usize) -> Option<usize> {
        self.reports
            .get(root)
            .map(|row| row.report.certificates().len())
    }

    #[cfg(test)]
    pub(crate) fn snapshot_report_for_root_v1(&self, root: usize) -> Option<&Report> {
        self.reports.get(root).map(|row| &row.report)
    }

    /// Only the enclosing constructor uses this after dropping and releasing
    /// its prepaid temporary input roster. No general caller floor reset exists.
    pub(crate) fn release_input_floor_v1(
        &mut self,
        budget: &Budget<'_>,
        released: usize,
    ) -> Result<()> {
        if budget as *const _ as usize != self.slot
            || budget.work_ledger_identity_v1() != self.ledger
            || budget.storage().checked_add(released) != Some(self.required_floor)
            || budget.storage() < self.retained
        {
            return Err(resource(Resource::Accounting));
        }
        self.required_floor = budget.storage();
        Ok(())
    }

    pub(crate) fn replay_consistency_v1(&self, budget: &mut Budget<'_>) -> Result<()> {
        if budget as *mut _ as usize != self.slot
            || budget.work_ledger_identity_v1() != self.ledger
            || budget.storage() < self.required_floor
        {
            return Err(resource(Resource::Accounting));
        }
        resources::owned(
            budget,
            0,
            resource,
            || capture_error(CaptureError::Panicked),
            |budget| {
                budget.charge_work(self.roots.len()).map_err(resource)?;
                if self.roots.len() != self.reports.len() {
                    return Err(mismatch(
                        "guarded progress replay lost its complete root reports",
                    ));
                }
                let source = self.capture.original().semantic_ssa().source_semantic();
                for (root, row) in self.roots.iter().zip(&self.reports) {
                    budget.charge_work(4).map_err(resource)?;
                    if root.semantic_root != row.root
                        || root.semantic_u32_induction.function() != row.body
                        || row.report.function() != row.body
                        || row.report.semantic_mir_sha256() != source.semantic_sha256()
                    {
                        return Err(mismatch(
                            "guarded progress replay changed root/report custody",
                        ));
                    }
                    // The subset guard API has no request for an empty report.
                    // Independently rederive that absence instead of granting it
                    // complete-source coverage merely because no row was requested.
                    if row.report.certificates().is_empty() {
                        let floor = budget.storage();
                        let fresh = analyze_semantic_u32_induction_bound_snapshots_with_meter_v1(
                            source,
                            row.body,
                            Default::default(),
                            &mut Meter(budget),
                        )
                        .map_err(model_error)?;
                        budget.charge_work(size_of::<Report>()).map_err(resource)?;
                        if fresh != row.report {
                            return Err(mismatch(
                                "guarded progress replay omitted a source certificate",
                            ));
                        }
                        drop(fresh);
                        budget
                            .release_storage(
                                budget
                                    .storage()
                                    .checked_sub(floor)
                                    .ok_or_else(|| resource(Resource::Accounting))?,
                            )
                            .map_err(resource)?;
                    }
                }
                let (guards, guard_storage) = derive_guard(&self.capture, &self.reports, budget)?;
                budget
                    .reserve_storage(size_of::<Vec<ConsumedProgressSiteV1>>())
                    .map_err(resource)?;
                let sites = derive_sites(&self.capture, &self.reports, &guards, budget)?;
                let site_storage = resources::bytes::<ConsumedProgressSiteV1>(sites.capacity())
                    .map_err(resource)?;
                budget.charge_work(sites.len()).map_err(resource)?;
                if sites.len() != self.sites.len()
                    || sites.iter().zip(&self.sites).any(|(fresh, old)| {
                        let mut expected = *fresh;
                        expected.consumed = true;
                        expected != *old
                    })
                {
                    return Err(mismatch(
                        "guarded progress replay changed complete consumed sites",
                    ));
                }
                drop(sites);
                budget
                    .release_storage(
                        site_storage
                            .checked_add(size_of::<Vec<ConsumedProgressSiteV1>>())
                            .ok_or_else(|| resource(Resource::Arithmetic))?,
                    )
                    .map_err(resource)?;
                drop(guards);
                budget.release_storage(guard_storage).map_err(resource)?;
                Ok(((), 0))
            },
        )
    }
}

pub(super) struct GuardedSourceProgressV1<'a, 's> {
    capture: &'s Capture,
    reports: &'a [RootReport],
    guards: &'a GuardReport<'s>,
    sites: &'a mut [ConsumedProgressSiteV1],
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
}

impl GuardedSourceProgressV1<'_, '_> {
    pub(super) fn with_source<T, F: ProjectedAssertionFactsV1>(
        &mut self,
        ordinal: usize,
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        facts: &mut F,
        run: impl FnOnce(&mut GuardedFacts<'_, '_, '_, F>) -> Result<T>,
    ) -> Result<T> {
        self.check(facts)?;
        facts.charge_private_array_work(3)?;
        let row = self
            .reports
            .get(ordinal)
            .ok_or_else(|| mismatch("guarded progress lost its complete root roster"))?;
        if row.root != root || row.body != function {
            return Err(mismatch(
                "guarded progress changed actual root/body selection",
            ));
        }
        run(&mut GuardedFacts {
            facts,
            progress: self,
            root,
            function,
        })
    }

    fn check(&self, facts: &impl ProjectedAssertionFactsV1) -> Result<()> {
        if facts.helper_value_ledger_v1()? != (self.slot, self.ledger)
            || facts.scalar_private_storage_v1()? < self.floor
        {
            return Err(resource(Resource::Accounting));
        }
        Ok(())
    }

    fn finish(&self, budget: &mut Budget<'_>) -> Result<()> {
        if budget as *mut _ as usize != self.slot
            || budget.work_ledger_identity_v1() != self.ledger
            || budget.storage() != self.floor
        {
            return Err(resource(Resource::Accounting));
        }
        budget.charge_work(self.sites.len()).map_err(resource)?;
        if self.sites.iter().any(|site| !site.consumed) {
            return Err(mismatch(
                "guarded progress left a claimed U32 source site unconsumed",
            ));
        }
        Ok(())
    }
}

pub(super) struct GuardedFacts<'a, 'r, 's, F> {
    facts: &'a mut F,
    progress: &'a mut GuardedSourceProgressV1<'r, 's>,
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
}

// This inventory is exhaustive so a new ranked result-producing recipe cannot
// silently bypass the duplicate-definition check at the strict boundary.
fn defines_ranked_value(
    operation: &ProductionRankedOperationV1,
    value: ProductionRankedValueIdV1,
) -> bool {
    use ProductionRankedOperationV1::*;
    match operation {
        View { result, .. }
        | ViewInSpace { result, .. }
        | PipelineCreate { result, .. }
        | IndexConstant { result, .. }
        | IndexUnsignedCast { result, .. }
        | IndexUnknown { result }
        | InvocationIndex { result, .. }
        | IndexBinary { result, .. }
        | DeterministicJoin { result, .. }
        | CheckedTiledIndex2D { result, .. }
        | CheckedRowStripedIndex2D { result, .. }
        | Dimension { result, .. }
        | TensorResultComponent { result, .. }
        | SemanticSymbol { result, .. }
        | SemanticConstant { result, .. }
        | SemanticBinary { result, .. }
        | SemanticExpression { result, .. } => *result == value,
        PredicatedCheckedTiledIndex2D {
            result, success, ..
        }
        | PredicatedCheckedRowStripedIndex2D {
            result, success, ..
        } => *result == value || *success == value,
        ExecutionLayout { .. }
        | PipelineEvent { .. }
        | Access { .. }
        | PredicatedAccess { .. }
        | ValueAccess { .. }
        | AtomicAccess { .. }
        | AtomicValueAccess { .. }
        | OwnershipContract { .. }
        | AllocationEffect { .. }
        | Barrier { .. }
        | Fence { .. }
        | TensorLayout { .. }
        | CollectiveSemantics { .. }
        | RequireEquivalent { .. }
        | RequireAuthenticatedReferenceEquivalent { .. }
        | RequestAuthenticatedReferenceEquivalent { .. }
        | RequireEffectRefinement { .. }
        | RequestEffectRefinement { .. }
        | RequireNumericalRefinement { .. }
        | RequestNumericalRefinement { .. }
        | RequireTensorRefinement { .. }
        | RequestTensorRefinement { .. } => false,
    }
}

fn require_ranked_bound(
    induction: &ProjectedUniformInductionV1,
    entry_operations: &[ProductionRankedOperationV1],
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<()> {
    facts.charge_private_array_work(12)?;
    let source = &induction.source_progress;
    let Some(cast) = &induction.bound_cast else {
        return if induction.bound == source.ranked_bound {
            Ok(())
        } else {
            Err(mismatch("guarded progress changed its uncast ranked bound"))
        };
    };
    // Earlier arithmetic reconciliation authenticates this source recipe and
    // then replaces only induction.bound with the emitted unsigned-cast result.
    // Do not relabel the raw source-progress value as that post-cast value.
    if cast.header != induction.header
        || cast.comparison_statement != source.header_statement
        || cast.source_type != source.bound_operand.ty()
        || simple_operand_local(&cast.source_operand).is_none()
        || cast.source_operand != source.bound_operand
        || cast.ranked_value != source.ranked_bound
        || !matches!(cast.bit_width, 8 | 16 | 32)
        || induction.bound == source.ranked_bound
    {
        return Err(mismatch(
            "guarded progress changed its exact unsigned-bound recipe",
        ));
    }
    let ProductionRankedValueV1::Local(result) = induction.bound else {
        return Err(mismatch(
            "guarded progress unsigned bound is not an emitted result",
        ));
    };
    let mut found = false;
    for operation in entry_operations {
        facts.charge_private_array_work(6)?;
        if !defines_ranked_value(operation, result) {
            continue;
        }
        if found
            || !matches!(operation, ProductionRankedOperationV1::IndexUnsignedCast {
                result: actual, source: raw, bit_width,
            } if *actual == result && *raw == source.ranked_bound && *bit_width == cast.bit_width)
        {
            return Err(mismatch(
                "guarded progress unsigned bound has a changed or duplicate definition",
            ));
        }
        found = true;
    }
    if !found {
        return Err(mismatch(
            "guarded progress unsigned bound has no exact emitted cast",
        ));
    }
    Ok(())
}

impl<F: ProjectedAssertionFactsV1> ProjectedAssertionFactsV1 for GuardedFacts<'_, '_, '_, F> {
    fn require_guarded_source_progress_v1(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        induction: &ProjectedUniformInductionV1,
        entry_operations: &[ProductionRankedOperationV1],
    ) -> Result<()> {
        self.progress.check(self.facts)?;
        self.facts.charge_private_array_work(5)?;
        let source = self
            .progress
            .capture
            .original()
            .semantic_ssa()
            .source_semantic();
        if !source
            .functions()
            .get(self.function.index() as usize)
            .is_some_and(|actual| std::ptr::eq(actual, function))
            || !std::ptr::eq(source.types(), types)
        {
            return Err(mismatch(
                "guarded progress received a foreign actual source function",
            ));
        }
        let progress = &induction.source_progress;
        let Some(ty) = types.get(progress.induction_type.index() as usize) else {
            return Err(mismatch(
                "guarded progress source type is outside actual declarations",
            ));
        };
        if !matches!(
            ty.shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32
            })
        ) {
            return Ok(());
        }
        let ProjectedSourceInductionUpdateV1::Checked {
            producer_block,
            producer_statement,
            result_local,
        } = progress.update
        else {
            return Err(mismatch("guarded progress is not a checked update"));
        };
        let key = Key {
            root: self.root.index(),
            function: self.function.index(),
            block: u32::try_from(producer_block).map_err(|_| resource(Resource::Arithmetic))?,
            statement: u32::try_from(producer_statement)
                .map_err(|_| resource(Resource::Arithmetic))?,
        };
        self.facts.charge_private_array_work(
            2 + (usize::BITS - self.progress.sites.len().leading_zeros()) as usize,
        )?;
        let index = self
            .progress
            .sites
            .binary_search_by_key(&key, |site| site.key)
            .map_err(|_| {
                mismatch("guarded progress U32 candidate has no complete source/N join")
            })?;
        let site = self.progress.sites[index];
        self.facts.charge_private_array_work(30)?;
        if site.consumed {
            return Err(mismatch("guarded progress consumed a U32 producer twice"));
        }
        let report = self
            .progress
            .reports
            .get(site.report)
            .ok_or_else(|| mismatch("guarded progress report index changed"))?;
        let certificate = report
            .report
            .certificates()
            .get(site.ordinal)
            .ok_or_else(|| mismatch("guarded progress certificate ordinal changed"))?;
        let guard = self
            .progress
            .guards
            .rows()
            .get(site.guard)
            .ok_or_else(|| mismatch("guarded progress guard ordinal changed"))?;
        let GuardOutcome::Joined(fact) = guard.outcome() else {
            return Err(mismatch("guarded progress lost its joined original-N fact"));
        };
        let actual_producer = function
            .blocks()
            .get(producer_block)
            .and_then(|block| block.statements().get(producer_statement))
            .ok_or_else(|| {
                mismatch("guarded progress checked producer is outside actual source")
            })?;
        let SemanticStatementKindV1::Assign(actual_producer) = actual_producer.kind() else {
            return Err(mismatch(
                "guarded progress checked producer is not an assignment",
            ));
        };
        let SemanticRvalueKindV1::CheckedBinary(checked) = actual_producer.value().kind() else {
            return Err(mismatch(
                "guarded progress checked producer changed its operation",
            ));
        };
        let actual_guard = function
            .blocks()
            .get(induction.header)
            .and_then(|block| block.statements().get(progress.header_statement))
            .ok_or_else(|| mismatch("guarded progress comparison is outside actual source"))?;
        let SemanticStatementKindV1::Assign(actual_guard) = actual_guard.kind() else {
            return Err(mismatch("guarded progress comparison is not an assignment"));
        };
        let SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            right: actual_bound,
            ..
        } = actual_guard.value().kind()
        else {
            return Err(mismatch(
                "guarded progress comparison changed its operation",
            ));
        };
        if report.root != self.root
            || report.body != self.function
            || !std::ptr::eq(fact.source(), self.progress.capture.original())
            || fact.root() != self.root
            || fact.function() != self.function
            || fact.certificate_ordinal() != site.ordinal
            || certificate.induction().local() != progress.induction
            || certificate.induction().ty() != progress.induction_type
            || certificate.header().block().index() as usize != induction.header
            || certificate.guard().statement() as usize != progress.header_statement
            || certificate.preheader().block().index() as usize != induction.preheader
            || certificate.body_entry().block().index() as usize != induction.body_entry
            || certificate.exit().block().index() as usize != induction.exit
            || certificate.update().block().block().index() as usize != induction.latch
            || certificate.update().statement() as usize != progress.latch_statement
            || certificate.checked_result().local() != result_local
            || certificate.guard_bound().ty() != progress.bound_operand.ty()
            || simple_operand_local(&progress.bound_operand)
                != Some(certificate.guard_bound().local())
            || actual_bound != &progress.bound_operand
            || checked.operation() != SemanticCheckedBinaryOpV1::Add
            || checked.right() != &progress.step_operand
            || progress.step_value != 1
            || induction.step != progress.ranked_step
        {
            return Err(mismatch(
                "guarded progress candidate differs from the exact checked source/N site",
            ));
        }
        require_ranked_bound(induction, entry_operations, self.facts)?;
        self.progress.sites[index].consumed = true;
        Ok(())
    }

    fn masked_assertion_source_proved_v1(
        &mut self,
        function: &SemanticFunctionDeclV1,
        block: usize,
        expected: bool,
        successor: SemanticBlockIdV1,
    ) -> Result<bool> {
        self.progress.check(self.facts)?;
        self.facts
            .masked_assertion_source_proved_v1(function, block, expected, successor)
    }
    fn require_unit_local_call(
        &mut self,
        block: usize,
        call: &SemanticDirectCallV1,
        source: SemanticSourceProvenanceV1,
    ) -> Result<()> {
        self.progress.check(self.facts)?;
        self.facts.require_unit_local_call(block, call, source)
    }
    fn charge_private_array_work(&mut self, amount: usize) -> Result<()> {
        self.progress.check(self.facts)?;
        self.facts.charge_private_array_work(amount)
    }
    fn helper_value_ledger_v1(
        &self,
    ) -> Result<(
        usize,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    )> {
        self.facts.helper_value_ledger_v1()
    }
    fn scalar_private_storage_v1(&self) -> Result<usize> {
        self.facts.scalar_private_storage_v1()
    }
    fn reserve_scalar_private_storage_v1(&mut self, amount: usize) -> Result<()> {
        self.progress.check(self.facts)?;
        self.facts.reserve_scalar_private_storage_v1(amount)
    }
    fn release_scalar_private_storage_v1(&mut self, amount: usize) -> Result<()> {
        self.progress.check(self.facts)?;
        if amount
            > self
                .facts
                .scalar_private_storage_v1()?
                .saturating_sub(self.progress.floor)
        {
            return Err(resource(Resource::Accounting));
        }
        self.facts.release_scalar_private_storage_v1(amount)
    }
    fn private_array_initializer_count(
        &mut self,
        block: usize,
        statement: usize,
    ) -> Result<Option<u64>> {
        self.progress.check(self.facts)?;
        self.facts.private_array_initializer_count(block, statement)
    }
    fn slice_access(
        &mut self,
        site: ProjectedSemanticAccessSiteV1,
        ordinal: u32,
        assertion: u32,
    ) -> Result<slice_projection_v1::ProjectedSliceInputV1> {
        self.progress.check(self.facts)?;
        self.facts.slice_access(site, ordinal, assertion)
    }
    fn is_materialized_block(&mut self, block: usize) -> Result<bool> {
        self.progress.check(self.facts)?;
        self.facts.is_materialized_block(block)
    }
    fn condition(
        &mut self,
        block: usize,
        expected: bool,
        success: SemanticBlockIdV1,
    ) -> Result<ProjectedAssertionConditionV1> {
        self.progress.check(self.facts)?;
        self.facts.condition(block, expected, success)
    }
}
