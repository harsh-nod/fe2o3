//! Consuming fixed execution and observed extraction on one V18 session.
use super::*;
use crate::{
    KirNeutralOptimizationErrorV18 as Failure, KirOptimizationMapPolicy3V18,
    PlironOptimizationReportV1, Policy3ExecutionWitnessV18,
    fixed_policy_v3::{ExecutionProfileV1, FixedPolicy, POLICY3_CANONICAL_CAP},
    kir_occurrence_capture_v1::{Capture, KirNeutralOccurrenceRowsV1, Limits},
    kir_optimization_map_v12::CaptureV12,
    optimization_v12::{execute_captured_fixed_policy_v1, policy3_execution_resources_v18},
};
use std::mem::size_of;

pub(crate) struct ExecutedV18Parts<M = KirOptimizationMapPolicy3V18, X = Policy3ExecutionWitnessV18>
{
    pub(crate) owner: VerifiedCanonicalKernelIrModuleV18,
    pub(crate) report: PlironOptimizationReportV1,
    pub(crate) bridge: KirBridgeReportV18,
    pub(crate) map: M,
    pub(crate) occurrences: KirNeutralOccurrenceRowsV1,
    pub(crate) execution: X,
    pub(crate) retained: usize,
}

fn headers<M, X>() -> Result<usize, ResourceError> {
    type Payload = Box<dyn std::any::Any + Send>;
    let slots = [
        (2, size_of::<ExecutedV18Parts<M, X>>()),
        (8, size_of::<Result<ExecutedV18Parts<M, X>, Failure>>()),
        (
            2,
            size_of::<std::thread::Result<Result<ExecutedV18Parts<M, X>, Failure>>>(),
        ),
        (4, size_of::<Payload>()),
        (1, size_of::<AssertUnwindSafe<Payload>>()),
        (2, size_of::<std::thread::Result<()>>()),
        (1, size_of::<std::ops::Range<usize>>()),
        (2, size_of::<KirPlironGraphV18<'_>>()),
        (2, size_of::<Capture>()),
        (2, size_of::<CaptureV12>()),
        (2, size_of::<Limits>()),
        (
            2,
            size_of::<crate::kir_occurrence_capture_v1::ObserverAdmissionV18>(),
        ),
        (2, size_of::<ExecutionProfileV1>()),
        (2, size_of::<crate::PlironOptimizationPlanV1>()),
        (2, size_of::<crate::OperationGraphAnalysisV1>()),
        (4, size_of::<usize>()),
    ];
    slots.into_iter().try_fold(0, |sum, (count, size)| {
        resources::add(sum, resources::mul(count, size)?)
    })
}

pub(crate) fn optimize_v18_graph(
    input: &VerifiedCanonicalKernelIrModuleV18,
    layouts: StorageLayoutLimitsV1,
    wrapper: usize,
    budget: &mut Budget<'_>,
) -> Result<ExecutedV18Parts, Failure> {
    optimize_graph::<ScalarPolicyV18>(input, layouts, wrapper, budget)
}

pub(crate) fn optimize_integer_v18_graph(
    input: &VerifiedCanonicalKernelIrModuleV18,
    layouts: StorageLayoutLimitsV1,
    wrapper: usize,
    budget: &mut Budget<'_>,
) -> Result<
    ExecutedV18Parts<
        crate::KirOptimizationMapIntegerContinuationV18,
        crate::IntegerContinuationExecutionWitnessV18,
    >,
    Failure,
> {
    optimize_graph::<IntegerPolicyV18>(input, layouts, wrapper, budget)
}

fn optimize_graph<P: ExecutionPolicyV18>(
    input: &VerifiedCanonicalKernelIrModuleV18,
    layouts: StorageLayoutLimitsV1,
    wrapper: usize,
    budget: &mut Budget<'_>,
) -> Result<ExecutedV18Parts<P::Map, P::Execution>, Failure> {
    let ledger = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    // Atomic admission occurs before an upstream graph or cleanup payload exists.
    budget.charge_work(1 + resources::CLEANUP_ATTEMPTS)?;
    budget.reserve_storage(headers::<P::Map, P::Execution>()?)?;
    let caught = catch_unwind(AssertUnwindSafe(|| {
        if input.canonical_bytes().len() > POLICY3_CANONICAL_CAP {
            return Err(Failure::Limit);
        }
        let (mut graph, structural, imported) =
            super::structural::import(input, budget).map_err(Failure::Bridge)?;
        budget.reserve_storage(imported.retained_storage())?;
        graph.validate_custody(budget).map_err(Failure::Bridge)?;
        let root = graph.session.operations[&graph.root.identity];
        let (limits, observer_admission) =
            crate::kir_occurrence_capture_v1::ObserverAdmissionV18::for_graph(
                &graph.session.context,
                root,
                input.module(),
                budget,
            )
            .map_err(Failure::Mapping)?;
        let (profile, map_limits) = P::resources(
            input.canonical_bytes().len(),
            limits.nodes,
            observer_admission,
        )
        .map_err(Failure::Execution)?;
        let allowance = observer_admission.work(limits).map_err(Failure::Mapping)?;
        budget.charge_work(allowance)?;
        budget.reserve_storage(limits.storage().map_err(Failure::Mapping)?)?;
        budget.charge_work(profile.work())?;
        for storage in [
            profile.persistent_storage(),
            profile.temporary_storage(),
            profile.retained_storage(),
            wrapper,
        ] {
            budget.reserve_storage(storage)?;
        }
        let mut roster_work = 0usize;
        let roster = optimization_roster_metered_v1::<true, _>(
            &graph.session.context,
            root,
            input.module(),
            &graph.origins,
            limits.nodes,
            &mut |units| {
                roster_work = roster_work
                    .checked_add(units)
                    .ok_or(crate::KirOptimizationMapErrorV12::Arithmetic)?;
                if roster_work > allowance {
                    return Err(crate::KirOptimizationMapErrorV12::Limit);
                }
                Ok(())
            },
        )
        .map_err(Failure::Mapping)?;
        let occurrences = Capture::new_for_policy_v18(
            &graph.session.context,
            root,
            input.module(),
            &roster,
            limits,
            roster_work,
            observer_admission,
            P::POLICY,
        )
        .map_err(Failure::Mapping)?;
        let capture = CaptureV12::new_for_policy_admitted(
            map_limits,
            &roster,
            P::POLICY,
            Some(observer_admission.definition_arity_bound()),
        )
        .map_err(Failure::Mapping)?;
        drop(roster);
        let mut passes = Vec::new();
        passes
            .try_reserve_exact(P::POLICY.passes().len())
            .map_err(|_| ResourceError::Allocation)?;
        passes.extend_from_slice(P::POLICY.passes());
        let pass_limits = crate::PlironOptimizationLimitsV1::new(256, 32_768, 25_268_224)
            .map_err(|_| ResourceError::Accounting)?;
        let plan = crate::PlironOptimizationPlanV1::new(passes, pass_limits)
            .map_err(|_| ResourceError::Accounting)?;
        graph.retained_storage =
            resources::add(graph.retained_storage, profile.persistent_storage())?;
        let execution = execute_captured_fixed_policy_v1(
            &mut graph.session,
            &graph.root,
            &plan,
            &capture,
            Some(&occurrences),
            P::POLICY,
            budget,
        )
        .map_err(Failure::Execution)?;
        if let Some(error) = capture.failure().or_else(|| occurrences.failure()) {
            return Err(Failure::Mapping(error));
        }
        let (report, cse_work) = execution;
        let report = report.map_err(Failure::Pass)?;
        graph.epoch = *graph
            .session
            .operation_graph_epochs
            .get(&graph.root.identity)
            .ok_or(ResourceError::Accounting)?;
        graph.validate_custody(budget).map_err(Failure::Bridge)?;
        let actual = graph
            .session
            .analyze_operation_graph_v1(&graph.root)
            .map_err(|error| Failure::Bridge(error.into()))?;
        if actual.replay_identity() != report.final_graph_identity()
            || graph.epoch != report.final_graph_identity().epoch()
        {
            return Err(Failure::Endpoint);
        }
        drop(plan);
        budget.release_storage(profile.temporary_storage())?;
        let (owner, bridge, extracted) = graph
            .extract(layouts, budget, false, Some(&structural))
            .map_err(Failure::Bridge)?;
        budget.reserve_storage(extracted.retained_storage())?;
        if owner.canonical_bytes().len() > POLICY3_CANONICAL_CAP
            || bridge.input != *input.identity()
            || bridge.output != *owner.identity()
            || bridge.table != graph.table_identity()
        {
            return Err(Failure::Endpoint);
        }
        let scratch = limits.storage().map_err(Failure::Mapping)?;
        budget.reserve_storage(scratch)?;
        let roster = occurrences
            .with_roster_meter(|meter| {
                optimization_roster_metered_v1::<true, _>(
                    &graph.session.context,
                    root,
                    input.module(),
                    &graph.origins,
                    limits.nodes,
                    meter,
                )
            })
            .map_err(Failure::Mapping)?;
        let (map, map_storage) =
            P::finish_map(&capture, input, &owner, &roster, budget).map_err(Failure::Mapping)?;
        budget.reserve_storage(map_storage)?;
        let rows = P::finish_rows(
            &occurrences,
            &graph.session.context,
            &roster,
            &map,
            owner.module(),
            budget,
        )
        .map_err(Failure::Mapping)?;
        let row_storage = rows.retained_storage().map_err(Failure::Mapping)?;
        drop(roster);
        budget.release_storage(
            scratch
                .checked_sub(row_storage)
                .ok_or(ResourceError::Accounting)?,
        )?;
        let execution = P::execution(
            input,
            &owner,
            bridge.table,
            &report,
            &map,
            ExecutionProfileV1 {
                resources: profile,
                registered_nodes: limits.nodes,
                cse_work,
            },
            budget,
        )?;
        let retained = [
            extracted.retained_storage(),
            map_storage,
            row_storage,
            profile.retained_storage(),
            wrapper,
        ]
        .into_iter()
        .try_fold(0, resources::add)?;
        drop(occurrences);
        drop(capture);
        drop(structural);
        drop(graph);
        Ok(ExecutedV18Parts {
            owner,
            report,
            bridge,
            map,
            occurrences: rows,
            execution,
            retained,
        })
    }));
    let result = match caught {
        Ok(result) => result,
        Err(payload) => {
            resources::discard_caught_payload(payload);
            Err(Failure::Panicked)
        }
    };
    if budget.work_ledger_identity_v1() != ledger {
        drop(result);
        return Err(ResourceError::Accounting.into());
    }
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ResourceError::Accounting)?;
    budget.release_storage(release)?;
    result
}

// Private policy selection keeps the session, observer, map and execution
// witness in one nominal family. Callers cannot choose an arbitrary pass list.
trait ExecutionPolicyV18 {
    const POLICY: FixedPolicy;
    type Map;
    type Execution;
    fn resources(
        bytes: usize,
        nodes: usize,
        admission: crate::kir_occurrence_capture_v1::ObserverAdmissionV18,
    ) -> Result<
        (
            crate::PlironOptimizationResourcesV12,
            crate::kir_optimization_map_v12::CaptureLimitsV12,
        ),
        crate::PlironOptimizationErrorV12,
    >;
    fn finish_map(
        capture: &CaptureV12,
        input: &VerifiedCanonicalKernelIrModuleV18,
        output: &VerifiedCanonicalKernelIrModuleV18,
        roster: &crate::kir_optimization_map_v12::LiveRosterV12,
        budget: &mut Budget<'_>,
    ) -> Result<(Self::Map, usize), crate::KirOptimizationMapErrorV12>;
    fn finish_rows(
        capture: &Capture,
        ctx: &pliron::context::Context,
        roster: &crate::kir_optimization_map_v12::LiveRosterV12,
        map: &Self::Map,
        output: &fe2o3_kernel_ir::Module,
        budget: &mut Budget<'_>,
    ) -> Result<KirNeutralOccurrenceRowsV1, crate::KirOptimizationMapErrorV12>;
    fn execution(
        input: &VerifiedCanonicalKernelIrModuleV18,
        output: &VerifiedCanonicalKernelIrModuleV18,
        table: fe2o3_kernel_ir::CanonicalStorageTableIdentityV18,
        report: &PlironOptimizationReportV1,
        map: &Self::Map,
        profile: ExecutionProfileV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self::Execution, ResourceError>;
}

macro_rules! execution_policy_v18 {
    ($name:ident, $policy:ident, $map:ty, $execution:ty, $resources:path, $finish:ident) => {
        struct $name;
        impl ExecutionPolicyV18 for $name {
            const POLICY: FixedPolicy = FixedPolicy::$policy;
            type Map = $map;
            type Execution = $execution;
            fn resources(
                bytes: usize,
                nodes: usize,
                admission: crate::kir_occurrence_capture_v1::ObserverAdmissionV18,
            ) -> Result<
                (
                    crate::PlironOptimizationResourcesV12,
                    crate::kir_optimization_map_v12::CaptureLimitsV12,
                ),
                crate::PlironOptimizationErrorV12,
            > {
                $resources(bytes, nodes, admission)
            }
            fn finish_map(
                capture: &CaptureV12,
                input: &VerifiedCanonicalKernelIrModuleV18,
                output: &VerifiedCanonicalKernelIrModuleV18,
                roster: &crate::kir_optimization_map_v12::LiveRosterV12,
                budget: &mut Budget<'_>,
            ) -> Result<(Self::Map, usize), crate::KirOptimizationMapErrorV12> {
                capture.$finish(input, output, roster, budget)
            }
            fn finish_rows(
                capture: &Capture,
                ctx: &pliron::context::Context,
                roster: &crate::kir_optimization_map_v12::LiveRosterV12,
                map: &Self::Map,
                output: &fe2o3_kernel_ir::Module,
                budget: &mut Budget<'_>,
            ) -> Result<KirNeutralOccurrenceRowsV1, crate::KirOptimizationMapErrorV12> {
                capture.$finish(ctx, roster, map, output, budget)
            }
            fn execution(
                input: &VerifiedCanonicalKernelIrModuleV18,
                output: &VerifiedCanonicalKernelIrModuleV18,
                table: fe2o3_kernel_ir::CanonicalStorageTableIdentityV18,
                report: &PlironOptimizationReportV1,
                map: &Self::Map,
                profile: ExecutionProfileV1,
                budget: &mut Budget<'_>,
            ) -> Result<Self::Execution, ResourceError> {
                <$execution>::from_execution(input, output, table, report, map, profile, budget)
            }
        }
    };
}
execution_policy_v18!(
    ScalarPolicyV18,
    Checked3,
    KirOptimizationMapPolicy3V18,
    Policy3ExecutionWitnessV18,
    policy3_execution_resources_v18,
    finish_policy3_v18
);
execution_policy_v18!(
    IntegerPolicyV18,
    Integer6,
    crate::KirOptimizationMapIntegerContinuationV18,
    crate::IntegerContinuationExecutionWitnessV18,
    crate::optimization_v12::integer_execution_resources_v18,
    finish_integer_v18
);
