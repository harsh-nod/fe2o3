//! Exact source/F joins for the borrowed conditional selected-memory domain.
//! Copied choice records cannot enter this relation without their checked owner.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalSelectedPointerIncomingV30 as ActualIncoming,
    CanonicalSelectedPointerNodeV30 as ActualNode, CanonicalSelectedPointerStepV30 as ActualStep,
    CanonicalSelectedSliceChoiceV30 as ActualChoice,
    CanonicalSelectedSliceInjectionV30 as ActualInjection,
    CheckedCanonicalSelectedSliceDomainsV30 as Domains,
};
use slice_view_v1::DescriptorRoleScopeV18;
use std::ops::Range;

#[path = "production_selected_final_source_build_v30.rs"]
mod build;

#[cfg(test)]
include!("production_selected_final_source_oracle_v30_tests.rs");
#[path = "production_selected_final_forwarding_v30.rs"]
mod forwarding;
#[path = "production_selected_final_source_index_v30.rs"]
mod index;

// These are locators in the retained original graph, not substitute authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum SourceEdgeV30 {
    Incoming(usize),
    Invocation(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SelectedFinalChoiceJoinV30 {
    actual: usize,
    original_leaf: usize,
    original_guard: usize,
    obligations: RangeLocatorV30,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SelectedFinalEdgeJoinV30 {
    actual: usize,
    original: SelectedFinalEdgeOriginV30,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectedFinalEdgeOriginV30 {
    Source(SourceEdgeV30),
    Forwarding { anchor: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectedFinalForwardingStepV30 {
    Cast { input: usize, target: usize },
    Incoming { actual: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SelectedFinalObligationJoinV30 {
    original: usize,
    forwarding: RangeLocatorV30,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RangeLocatorV30 {
    first: usize,
    count: usize,
}

impl RangeLocatorV30 {
    fn range(self) -> Result<Range<usize>, ArgumentResourceV1> {
        Ok(self.first..argument_sum_v1(&[self.first, self.count])?)
    }
}

struct SelectedFinalAccessV30 {
    original: usize,
    disposition: ProductionOptimizedSourceOperationV18,
    choices: RangeLocatorV30,
    edges: RangeLocatorV30,
}

struct SelectedFinalRowsV30 {
    accesses: Vec<SelectedFinalAccessV30>,
    choices: Vec<SelectedFinalChoiceJoinV30>,
    edges: Vec<SelectedFinalEdgeJoinV30>,
    obligations: Vec<SelectedFinalObligationJoinV30>,
    forwarding: Vec<SelectedFinalForwardingStepV30>,
}

/// Private scoped intake for the selected final relation. Both original and
/// actual complete graphs stay borrowed; every logical alternative is retained.
pub(in super::super::super) struct CheckedSelectedFinalSourcesV30<'s> {
    optimized: &'s ProductionOptimizedSourceCorrespondenceV18<'s>,
    domains: &'s Domains<'s, 's>,
    root: usize,
    function: FunctionCoordinate,
    source: &'s [PendingSourceSelectedAccessV30],
    transport: &'s [SelectedTransportRowV30],
    rows: &'s SelectedFinalRowsV30,
    scope: DescriptorRoleScopeV18,
}

fn formal<T>(
    original: &ProductionSourceCorrespondenceV18<'_>,
    result: Result<T, fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1>,
) -> SourceOwnedResultV18<T> {
    result.map_err(|error| optimized_source_observed_formal_error_v18(original, &error))
}

fn exact_owner(
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    domains: &Domains<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    optimized
        .original
        .global_expression_entry_v23(optimized, budget)?;
    let owner = formal(optimized.original, domains.owner(budget))?;
    if !std::ptr::eq(owner, optimized.checked.output().owner()) {
        return optimized
            .original
            .source
            .missing("selected final domain owner differs from the exact optimized source output");
    }
    Ok(())
}

impl CheckedSelectedFinalSourcesV30<'_> {
    fn query(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        if let Err(error) = self.scope.observe(self.optimized.original, budget) {
            self.domains.refuse_retained_custody();
            return self.optimized.original.retain_query(Err(error));
        }
        exact_owner(self.optimized, self.domains, budget)
    }

    pub(in super::super::super) fn original_access_count(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.query(budget)?;
        Ok(self.rows.accesses.len())
    }

    pub(in super::super::super) fn retained_choice_count(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.query(budget)?;
        Ok(self.rows.choices.len())
    }

    pub(in super::super::super) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

struct SelectedFinalCaptureV30<'s, C> {
    optimized: &'s ProductionOptimizedSourceCorrespondenceV18<'s>,
    domains: &'s Domains<'s, 's>,
    root: usize,
    consume: C,
}

struct SelectedFinalExecutionV30<'s, 'budget, 'work, C> {
    capture: SelectedFinalCaptureV30<'s, C>,
    budget: &'budget mut ArgumentBudgetV1<'work>,
}

fn headers<T, C>() -> Result<usize, ArgumentResourceV1> {
    fn h<V>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<V>(),
            argument_product_v1(2, size_of::<SourceOwnedResultV18<V>>())?,
        ])
    }
    argument_sum_v1(&[
        index::ProjectionIndexV30::headers()?,
        build::headers()?,
        h::<SelectedFinalCaptureV30<'_, C>>()?,
        h::<SelectedFinalExecutionV30<'_, '_, '_, C>>()?,
        h::<CheckedSelectedFinalSourcesV30<'_>>()?,
        h::<DescriptorRoleScopeV18>()?,
        h::<&[PendingSourceSelectedAccessV30]>()?,
        h::<&[SelectedTransportRowV30]>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>>()?,
        h::<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18>()?,
        h::<
            Result<
                &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
                fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
            >,
        >()?,
        h::<T>()?,
        h::<SourceOwnedResultV18<T>>()?,
        h::<Range<usize>>()?,
        h::<&index::ProjectionIndexV30<'_>>()?,
        h::<&SelectedFinalRowsV30>()?,
        h::<&CheckedSelectedFinalSourcesV30<'_>>()?,
        argument_product_v1(4, h::<usize>()?)?,
        h::<()>()?,
    ])
}

impl<'work, C> SelectedFinalExecutionV30<'_, '_, 'work, C> {
    fn run<T>(self, floor: usize) -> SourceOwnedResultV18<T>
    where
        C: for<'scope> FnOnce(
            &CheckedSelectedFinalSourcesV30<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<T>,
    {
        let Self {
            capture:
                SelectedFinalCaptureV30 {
                    optimized,
                    domains,
                    root,
                    consume,
                },
            budget,
        } = self;
        exact_owner(optimized, domains, budget)?;
        optimized.replay_selected_transport_v30(root, budget)?;
        let range = optimized.index.selected_roots.get(root).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("selected final original root range"),
        )?;
        let transport = optimized.index.selected.get(range.clone()).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("selected final original transport range"),
        )?;
        let source = scoped_raw_admission_v29::checked_selected_source_rows_v30(
            optimized.original,
            root,
            budget,
        )?;
        let function =
            optimized_source_root_function_v18(optimized.original, optimized, root, budget)?
                .coordinate;
        let projection =
            index::ProjectionIndexV30::build(optimized, root, transport, source, budget)?;
        #[cfg(test)]
        let projection = construction_projection_for_test_v30(projection, budget)?;
        let rows = build::build(optimized, domains, function, &projection, source, budget)?;
        let view = CheckedSelectedFinalSourcesV30 {
            optimized,
            domains,
            root,
            function,
            source,
            transport,
            rows: &rows,
            scope: DescriptorRoleScopeV18::new(budget),
        };
        let result = consume(&view, budget);
        // A raw callback Accounting error is lost custody even when its
        // ledger/floor still match. Deny refund before any backing drops.
        let result = if matches!(
            &result,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Accounting
            ))
        ) {
            optimized.original.source.cleanup.deny_refund();
            domains.refuse_retained_custody();
            optimized.original.retain_query(result)
        } else {
            result
        };
        if budget.storage() != view.scope.required_storage() {
            optimized.original.source.cleanup.deny_refund();
            domains.refuse_retained_custody();
        }
        let postflight = view.query(budget);
        drop(view);
        drop(rows);
        drop(projection);
        postflight?;
        let value = result?;
        let retained = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.release_storage(retained)?;
        Ok(value)
    }
}

fn execute<'work, T, C>(
    capture: SelectedFinalCaptureV30<'_, C>,
    budget: &mut ArgumentBudgetV1<'work>,
) -> SourceOwnedResultV18<T>
where
    C: for<'scope> FnOnce(
        &CheckedSelectedFinalSourcesV30<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<T>,
{
    // The shared attempt owns and prepays the outer capture before this call.
    // Pay the nested execution/query frames before constructing their values.
    let floor = budget.storage();
    budget.reserve_storage(headers::<T, C>()?)?;
    SelectedFinalExecutionV30 { capture, budget }.run(floor)
}

impl ProductionOptimizedSourceCorrespondenceV18<'_> {
    // Mandatory selected final intake: no API accepts a stand-alone copied
    // domain, source leaf number, guard, or predecessor as evidence.
    pub(in super::super::super) fn with_selected_final_sources_v30<'work, T>(
        &self,
        domains: &Domains<'_, '_>,
        root: usize,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &CheckedSelectedFinalSourcesV30<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<T>,
    ) -> SourceOwnedResultV18<T> {
        self.original.global_expression_entry_v23(self, budget)?;
        let floor = budget.storage();
        let capture = SelectedFinalCaptureV30 {
            optimized: self,
            domains,
            root,
            consume,
        };
        let result =
            scoped_source_attempt_v29(self.original.source.cleanup, budget, floor, move |budget| {
                let result = execute(capture, budget);
                #[cfg(test)]
                observe_attempt_result_for_test_v30(budget);
                // Construction can refuse before a scoped view exists. Keep
                // that typed custody refusal inside the owning transaction.
                if matches!(
                    &result,
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ) {
                    self.original.source.cleanup.deny_refund();
                    domains.refuse_retained_custody();
                    self.original.retain_query(result)
                } else {
                    result
                }
            });
        self.original.retain_query(result)
    }
}
