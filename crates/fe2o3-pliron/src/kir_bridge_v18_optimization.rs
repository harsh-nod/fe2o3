//! Consuming fixed execution and observed extraction on one V18 session.
use super::*;
use crate::{
    KirNeutralOptimizationErrorV18 as Failure, KirOptimizationMapPolicy3V18,
    PlironOptimizationReportV1, Policy3ExecutionWitnessV18,
    fixed_policy_v3::{ExecutionProfileV1, FixedPolicy, POLICY3_CANONICAL_CAP},
    kir_occurrence_capture_v1::{Capture, KirNeutralOccurrenceRowsV1, Limits},
    kir_optimization_map_v12::CaptureV12,
    optimization_v12::{execute_captured_fixed_policy_v1, policy3_execution_resources_v1},
};
use std::mem::size_of;

pub(crate) struct ExecutedV18Parts {
    pub(crate) owner: VerifiedCanonicalKernelIrModuleV18,
    pub(crate) report: PlironOptimizationReportV1,
    pub(crate) bridge: KirBridgeReportV18,
    pub(crate) map: KirOptimizationMapPolicy3V18,
    pub(crate) occurrences: KirNeutralOccurrenceRowsV1,
    pub(crate) execution: Policy3ExecutionWitnessV18,
    pub(crate) retained: usize,
}

fn headers() -> Result<usize, ResourceError> {
    type Payload = Box<dyn std::any::Any + Send>;
    let slots = [
        (2, size_of::<ExecutedV18Parts>()),
        (8, size_of::<Result<ExecutedV18Parts, Failure>>()),
        (
            2,
            size_of::<std::thread::Result<Result<ExecutedV18Parts, Failure>>>(),
        ),
        (4, size_of::<Payload>()),
        (1, size_of::<AssertUnwindSafe<Payload>>()),
        (2, size_of::<std::thread::Result<()>>()),
        (1, size_of::<std::ops::Range<usize>>()),
        (2, size_of::<KirPlironGraphV18<'_>>()),
        (2, size_of::<Capture>()),
        (2, size_of::<CaptureV12>()),
        (2, size_of::<Limits>()),
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
    let ledger = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    // Atomic admission occurs before an upstream graph or cleanup payload exists.
    budget.charge_work(1 + resources::CLEANUP_ATTEMPTS)?;
    budget.reserve_storage(headers()?)?;
    let caught = catch_unwind(AssertUnwindSafe(|| {
        if input.canonical_bytes().len() > POLICY3_CANONICAL_CAP {
            return Err(Failure::Limit);
        }
        let (mut graph, imported) =
            KirPlironGraphV18::import(input, budget).map_err(Failure::Bridge)?;
        budget.reserve_storage(imported.retained_storage())?;
        graph.validate_custody(budget).map_err(Failure::Bridge)?;
        let root = graph.session.operations[&graph.root.identity];
        let limits = Limits::for_graph(&graph.session.context, root, input.module(), budget)
            .and_then(Limits::for_policy3)
            .map_err(Failure::Mapping)?;
        let (profile, map_limits) =
            policy3_execution_resources_v1(input.canonical_bytes().len(), limits.nodes)
                .map_err(Failure::Execution)?;
        budget.charge_work(limits.work().map_err(Failure::Mapping)?)?;
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
        let allowance = limits.work().map_err(Failure::Mapping)?;
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
        let occurrences = Capture::new_for_policy(
            &graph.session.context,
            root,
            input.module(),
            &roster,
            limits,
            roster_work,
            FixedPolicy::Checked3,
        )
        .map_err(Failure::Mapping)?;
        let capture = CaptureV12::new_for_policy(map_limits, &roster, FixedPolicy::Checked3)
            .map_err(Failure::Mapping)?;
        drop(roster);
        let mut passes = Vec::new();
        passes
            .try_reserve_exact(FixedPolicy::Checked3.passes().len())
            .map_err(|_| ResourceError::Allocation)?;
        passes.extend_from_slice(FixedPolicy::Checked3.passes());
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
            FixedPolicy::Checked3,
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
            .extract_canonical_v18(layouts, budget)
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
        let (map, map_storage) = capture
            .finish_policy3_v18(input, &owner, &roster, budget)
            .map_err(Failure::Mapping)?;
        budget.reserve_storage(map_storage)?;
        let rows = occurrences
            .finish_policy3_v18(&graph.session.context, &roster, &map, owner.module())
            .map_err(Failure::Mapping)?;
        let row_storage = rows.retained_storage().map_err(Failure::Mapping)?;
        drop(roster);
        budget.release_storage(
            scratch
                .checked_sub(row_storage)
                .ok_or(ResourceError::Accounting)?,
        )?;
        let execution = Policy3ExecutionWitnessV18::from_execution(
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
