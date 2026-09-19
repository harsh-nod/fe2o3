//! Project retained source anchors into the shared lowerer check before materialization.
//! Root agreement does not establish callback expansion, scope closure or authority.

use crate::collector::workgroup_scope_custody_v29::{
    ScopeCallKindV29, ScopeCallableV29, ScopeEventKindV29,
};
use crate::collector::{
    CallBoundaryV29, ContextRootVisitErrorV29, RetainedContextEntriesV29,
    RetainedExecutionSourceV29,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionCheckedContextRootV29, ProductionContextCallBoundaryV29,
    ProductionContextRootErrorV29, ProductionContextRootInputV29,
    ProductionExecutionSourceInputV29, ProductionScopeCallKindV29,
    ProductionScopeCallableCandidateV29, ProductionScopeEventCandidateV29,
    ProductionScopeEventKindV29, ProductionSourceLaunchRosterV1, with_checked_execution_source_v29,
};
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use super::ProductionPipelineError;

#[cfg(test)]
#[path = "production_context_projection_capacity_v29_tests.rs"]
mod projection_capacity_tests;

#[cfg(test)]
#[path = "production_pending_context_observer_v29.rs"]
mod pending_observer_v29;

fn project_boundary(source: &CallBoundaryV29) -> ProductionContextCallBoundaryV29 {
    let (block, statement_count) = source.location();
    let (destination, destination_type) = source.destination();
    ProductionContextCallBoundaryV29 {
        block,
        statement_count,
        destination,
        destination_type,
        target: source.continuation(),
        unwind: source.unwind(),
    }
}

pub(crate) fn check_context_handoff_v29(
    entries: &RetainedContextEntriesV29,
    ssa: &ProductionSemanticSsaOwnerV1,
    launch: &ProductionSourceLaunchRosterV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    use_root: impl for<'a> FnMut(
        ProductionCheckedContextRootV29<'a>,
        &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), ProductionContextRootErrorV29>,
) -> Result<(), ProductionPipelineError> {
    let Some(source) = execution_source_v29(entries, ssa, budget)? else {
        return Ok(());
    };
    with_projected_execution_source_v29(&source, budget, |input, budget| {
        with_checked_execution_source_v29(ssa, launch, input, budget, use_root)
    })
    .map_err(ProductionPipelineError::ContextHandoff)
}

fn execution_source_v29<'receipt>(
    entries: &'receipt RetainedContextEntriesV29,
    ssa: &ProductionSemanticSsaOwnerV1,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<Option<RetainedExecutionSourceV29<'receipt>>, ProductionPipelineError> {
    entries
        .materialization_source_v29(ssa.source_semantic(), budget)
        .map_err(|error| match error {
            ContextRootVisitErrorV29::Source(error) => ProductionPipelineError::SemanticImport(
                crate::collector::ProductionSemanticImportErrorV1::BodyConstruction(Box::new(
                    error,
                )),
            ),
            ContextRootVisitErrorV29::Resource(error) => {
                ProductionPipelineError::ContextHandoff(error.into())
            }
            ContextRootVisitErrorV29::Consumer(never) => match never {},
        })
}

fn projection_bytes<T>(count: usize) -> Result<usize, ProductionContextRootErrorV29> {
    count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(Resource::Arithmetic.into())
}

fn projection_rows<T>(
    count: usize,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    charged: &mut usize,
) -> Result<Vec<T>, ProductionContextRootErrorV29> {
    let requested = projection_bytes::<T>(count)?;
    #[cfg(test)]
    let count = projection_capacity_tests::allocation_request(count)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let actual = projection_bytes::<T>(rows.capacity())?;
    #[cfg(test)]
    projection_capacity_tests::record_capacity(actual);
    let extra = actual.checked_sub(requested).ok_or(Resource::Accounting)?;
    let next = charged.checked_add(extra).ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(extra)?;
    *charged = next;
    Ok(rows)
}

fn project_class(class: ScopeCallableV29) -> ProductionScopeCallableCandidateV29 {
    match class {
        ScopeCallableV29::Ordinary => ProductionScopeCallableCandidateV29::Ordinary,
        ScopeCallableV29::Provider { function, identity } => {
            ProductionScopeCallableCandidateV29::Provider { function, identity }
        }
        ScopeCallableV29::Derive {
            binding,
            operation,
            context,
            workgroup,
        } => ProductionScopeCallableCandidateV29::Derive {
            binding,
            operation,
            context,
            workgroup,
        },
    }
}

fn project_event(kind: ScopeEventKindV29) -> ProductionScopeEventKindV29 {
    match kind {
        ScopeEventKindV29::Call { callee, kind } => ProductionScopeEventKindV29::Call {
            callee,
            kind: match kind {
                ScopeCallKindV29::Ordinary => ProductionScopeCallKindV29::Ordinary,
                ScopeCallKindV29::Provider => ProductionScopeCallKindV29::Provider,
                ScopeCallKindV29::Derive => ProductionScopeCallKindV29::Derive,
            },
        },
        ScopeEventKindV29::Return => ProductionScopeEventKindV29::Return,
        ScopeEventKindV29::Assert => ProductionScopeEventKindV29::Assert,
        ScopeEventKindV29::Unreachable => ProductionScopeEventKindV29::Unreachable,
        ScopeEventKindV29::UnwindResume => ProductionScopeEventKindV29::UnwindResume,
        ScopeEventKindV29::UnwindTerminate => ProductionScopeEventKindV29::UnwindTerminate,
        ScopeEventKindV29::Abort => ProductionScopeEventKindV29::Abort,
    }
}

pub(crate) fn with_projected_execution_source_v29<R>(
    source: &RetainedExecutionSourceV29<'_>,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    use_source: impl for<'a> FnOnce(
        ProductionExecutionSourceInputV29<'a>,
        &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<R, ProductionContextRootErrorV29>,
) -> Result<R, ProductionContextRootErrorV29> {
    let ledger = budget.work_ledger_identity_v1();
    let bytes = [
        projection_bytes::<ProductionContextRootInputV29<'_>>(source.roots().len())?,
        projection_bytes::<ProductionScopeCallableCandidateV29>(source.classes().len())?,
        projection_bytes::<ProductionScopeEventCandidateV29>(source.events().len())?,
    ]
    .into_iter()
    .try_fold(0_usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })?;
    let work = [
        (source.roots().len(), 32),
        (source.classes().len(), 12),
        (source.events().len(), 8),
    ]
    .into_iter()
    // Prepay the fixed capacity reconciliation for each of the three vectors.
    .try_fold(18_usize, |sum, (count, width)| {
        count
            .checked_mul(width)
            .and_then(|amount| sum.checked_add(amount))
            .ok_or(Resource::Arithmetic)
    })?;
    budget.charge_work(work)?;
    budget.reserve_storage(bytes)?;
    let mut charged = bytes;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut roots = projection_rows(source.roots().len(), budget, &mut charged)?;
        let mut classes = projection_rows(source.classes().len(), budget, &mut charged)?;
        let mut events = projection_rows(source.events().len(), budget, &mut charged)?;
        for entry in source.roots() {
            let (root, root_identity) = entry.root();
            let (helper, helper_identity) = entry.helper();
            let (issuer, issuer_identity) = entry.issuer();
            let (context_type, context_identity) = entry.context();
            roots.push(ProductionContextRootInputV29 {
                semantic_sha256: source.semantic_sha256(),
                root,
                root_identity,
                helper,
                helper_identity,
                issuer,
                issuer_identity,
                context_type,
                context_identity,
                issuance: project_boundary(entry.issuance()),
                helper_call: project_boundary(entry.helper_call()),
                helper_context_local: entry.helper_argument(),
                helper_arguments: entry.helper_operands(),
            });
        }
        classes.extend(source.classes().iter().copied().map(project_class));
        events.extend(
            source
                .events()
                .iter()
                .map(|event| ProductionScopeEventCandidateV29 {
                    function: event.function,
                    block: event.block,
                    statement_count: event.statement_count,
                    kind: project_event(event.kind),
                }),
        );
        use_source(
            ProductionExecutionSourceInputV29 {
                semantic_sha256: source.semantic_sha256(),
                roots: &roots,
                classes: &classes,
                events: &events,
            },
            budget,
        )
    }));
    // The temporary row backing has dropped. Consumer-owned storage is not ours
    // to refund, and a replaced ledger must never receive our release.
    let cleanup = if budget.work_ledger_identity_v1() == ledger {
        budget.release_storage(charged)
    } else {
        Err(Resource::Accounting)
    };
    match result {
        Ok(result) => {
            cleanup?;
            result
        }
        Err(payload) => resume_unwind(payload),
    }
}

#[cfg(test)]
impl<'tcx> super::ProductionCompilation<'tcx, super::CollectedRustStage<'tcx>> {
    pub(crate) fn observe_context_handoff_v29(
        self,
        use_root: impl for<'a> FnMut(
            ProductionCheckedContextRootV29<'a>,
            &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        ) -> Result<(), ProductionContextRootErrorV29>,
    ) -> Result<(), Box<ProductionPipelineError>> {
        self.import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .materialize_with_context_observer_v29(use_root)
            .map(|_| ())
    }
}
