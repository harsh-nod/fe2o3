use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
    InvocationObserverV1, observe_resource_preflight_v1,
};

type WorkgroupObserverV1<'o, 'p, 'r> = Option<&'o InvocationObserverV1<'p, 'r>>;

fn observe_workgroup_quota_v1(observer: WorkgroupObserverV1<'_, '_, '_>, resource: &'static str) {
    if let Some(observer) = observer {
        observer.deny(ProductionAnalysisResourceLimitV1 {
            phase: ProductionAnalysisResourcePhaseV1::WorkgroupMemory,
            resource,
        });
    }
}

pub(crate) fn observe_workgroup_dependency_failure_v1(
    observer: WorkgroupObserverV1<'_, '_, '_>,
    failure: &PlironMemoryOrderAnalysisFailureV1,
) {
    use crate::production_analysis::pliron_invocation_trace::PlironTraceFailureV1 as Trace;
    use ProductionAnalysisResourcePhaseV1 as Phase;
    let (phase, resource) = match failure {
        PlironMemoryOrderAnalysisFailureV1::Trace(Trace::ResourceLimit) => {
            (Phase::InvocationTrace, "workgroup trace resource limit")
        }
        PlironMemoryOrderAnalysisFailureV1::Trace(Trace::LaunchTooLarge { .. }) => {
            (Phase::InvocationTrace, "workgroup trace launch limit")
        }
        PlironMemoryOrderAnalysisFailureV1::Trace(Trace::Sparse(
            crate::SparseIndexFailureV1::ResourceLimit { resource, .. },
        )) => (Phase::SparseIndex, *resource),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::VersionLimitExceeded,
        ) => (Phase::MemoryOrder, "workgroup memory-version limit"),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::PublicationEdgeLimitExceeded,
        ) => (
            Phase::MemoryOrder,
            "workgroup memory publication-edge limit",
        ),
        PlironMemoryOrderAnalysisFailureV1::MemoryOrder(
            PlironMemoryOrderFailureV1::IssueLimitExceeded,
        ) => (Phase::MemoryOrder, "workgroup memory-order issue limit"),
        // Provenance diagnostics have already lost their typed failure. The
        // closed schedule observes that cache at its preparation boundary.
        _ => return,
    };
    if let Some(observer) = observer {
        observer.deny(ProductionAnalysisResourceLimitV1 { phase, resource });
    }
}

pub(crate) fn preflight_prepared_workgroup_memory_with_observation_v1(
    census: ProductionAnalysisInputCensusV1,
    admission: ProductionMemoryOrderResourceAdmissionV1,
    memory_order_issues: Result<&[PlironMemoryOrderIssueV1], PlironMemoryOrderAnalysisFailureV1>,
    limits: ProductionAnalysisResourceLimitsV1,
    observer: WorkgroupObserverV1<'_, '_, '_>,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    observe_resource_preflight_v1(observer, |observer| {
        if let Err(failure) = &memory_order_issues {
            observe_workgroup_dependency_failure_v1(observer, failure);
        }
        preflight_prepared_workgroup_memory_resource_upper_bound_v1(
            census,
            admission,
            memory_order_issues,
            limits,
        )
    })
}

#[cfg(test)]
pub(crate) fn run_pliron_workgroup_memory_check_with_analyses_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
) -> PlironWorkgroupMemoryReportV1 {
    run_pliron_workgroup_memory_with_observation_v1(context, function, analyses, None)
}

pub(crate) fn run_pliron_workgroup_memory_with_observation_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    observer: WorkgroupObserverV1<'_, '_, '_>,
) -> PlironWorkgroupMemoryReportV1 {
    match observer {
        None => run_pliron_workgroup_memory_observed_inner_v1(context, function, analyses, None),
        Some(observer) => observer.with_projection(&Ok, |nested| {
            run_pliron_workgroup_memory_observed_inner_v1(context, function, analyses, Some(nested))
        }),
    }
}

pub(crate) fn require_pliron_workgroup_memory_with_observation_v1(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    observer: WorkgroupObserverV1<'_, '_, '_>,
) -> Result<PlironWorkgroupMemoryReportV1, PlironWorkgroupMemoryCheckErrorV1> {
    let report =
        run_pliron_workgroup_memory_with_observation_v1(context, function, analyses, observer);
    if report.is_clean() {
        Ok(report)
    } else {
        Err(PlironWorkgroupMemoryCheckErrorV1 { report })
    }
}

#[cfg(test)]
fn prepend_collective_path_v1(
    local: &[CollectiveTransposePathEventV1],
    summary: CollectiveTransposePathSummaryV1,
) -> Result<CollectiveTransposePathSummaryV1, String> {
    prepend_collective_path_with_observation_v1(local, summary, None)
}

#[cfg(test)]
fn merge_collective_path_v1(
    complete: &mut CollectiveTransposePathSummaryV1,
    candidate: CollectiveTransposePathSummaryV1,
) -> Result<(), String> {
    merge_collective_path_with_observation_v1(complete, candidate, None)
}
