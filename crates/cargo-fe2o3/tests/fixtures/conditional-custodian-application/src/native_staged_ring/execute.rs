//! One original finite progress pass per caller timer wake, with explicit staging.
use super::*;
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_conditional_custodian_application::fill_write_only_gpu;
use fe2o3_host::{
    GeneratedRuntimeArgumentLimitsV1, GeneratedRuntimeResultBudgetV1, GeneratedRuntimeWriteSlice,
};
use fe2o3_runtime::completion::CompletionNodeStateV1 as Node;
use fe2o3_runtime::*;
use native_staged_ring_case::Transfer;
use outcomes::{intake, local};

pub(super) async fn run<'work>(
    application: &mut Proved<'work>,
    budget: &mut Budget<'work>,
    context: &mut Context,
    case: &Case,
    deadline: Instant,
) -> Result<String> {
    let mut plan = plan::Plan::new(context, case)?;
    let n = plan.devices.len();
    let result_budget = checked(GeneratedRuntimeResultBudgetV1::new(8192, n))?;
    let mut outputs = Vec::with_capacity(n);
    let mut observers = Vec::with_capacity(n);
    let mut scratch = Vec::with_capacity(n);
    for index in 0..n {
        let (output, observer) = GeneratedRuntimeWriteSlice::new_charged(
            vec![u32::MAX; case.elements(index)].into_boxed_slice(),
        );
        outputs.push(Some(output));
        observers.push(observer);
        scratch.push(vec![0; case.elements(index) * 4]);
    }
    let geometry = checked(AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]))?;
    let mut staged = vec![false; n];
    let mut refused = vec![false; n];
    let request = plan
        .request
        .take()
        .ok_or("original request already consumed")?;
    let (mut rows, failure) = checked(application.with_native_invocation_scope_async_v1(
        budget, deadline, async |native| {
            let settled = context.with_generated_gfx942_scope_settled_async_v1(
                n * 5, deadline, caller_wake, async |scope| {
                    let ticket = scope.try_admit_graph_v1(request, |node, original| {
                        let index = plan.nodes.iter().position(|ids| ids[0] == node)
                            .ok_or_else(|| intake("foreign unbound generated node"))?;
                        if original.observation().unique_id() != case.devices[index] {
                            return Err(intake("prepared device differs from original ring producer"));
                        }
                        let output = outputs[index].take().ok_or_else(|| intake("duplicate ring producer"))?;
                        native.prepare_generated_invocation::<fill_write_only_gpu::Marker, _>(
                            fill_write_only_gpu::RuntimeArguments::new(output), original, geometry, 30_000,
                            GeneratedRuntimeArgumentLimitsV1::new(16384, 16384, 1), &result_budget,
                        )
                    }).map_err(intake)?;
                    while scope.pending_v1() != 0 {
                        scope.progress_v1().map_err(intake)?;
                        for index in 0..n {
                            if staged[index] || refused[index] { continue; }
                            let ids = plan.nodes[index];
                            match scope.graph_node_state_v1(&ticket, ids[1]).map_err(intake)? {
                                Node::Blocked => continue,
                                Node::DependencyFailed { origin, .. } if origin == ids[0] => continue,
                                Node::Ready => {}
                                other => return Err(intake(("unexpected unperformed host staging state", other))),
                            }
                            match observers[index].try_stage_graph_completed_v1(scope, &ticket,
                                ids[1], ids[0], plan.sources[index], &mut scratch[index])
                            {
                                Ok(Some(())) => staged[index] = true,
                                Ok(None) => {}
                                Err(RuntimeGfx942ScopedGraphStagingErrorV1::Scope(
                                    RuntimeGfx942ScopeErrorV1::Context(RuntimeErrorV1::Validation(_) | RuntimeErrorV1::BackendRejected(_))))
                                    if matches!(scope.graph_node_state_v1(&ticket, ids[1]).map_err(intake)?,
                                        Node::Failed { origin, .. } if origin == ids[1]) => { refused[index] = true; }
                                Err(error) => return Err(intake(error)),
                            }
                        }
                        if scope.pending_v1() != 0 { caller_wake(deadline).await; }
                    }
                    // The same driver provides its original final local outcome;
                    // the enclosing settled API independently closes the epoch.
                    let first_failure = match scope.drive_with_wake_v1(caller_wake).await {
                        Ok(()) => None, Err(error) => Some(local(error)?),
                    };
                    let (rows, failures) = outcomes::collect(scope, &ticket, &plan, case,
                        &mut observers, &staged, &refused)?;
                    if match &first_failure { Some(error) => !failures.contains(error), None => !failures.is_empty() } {
                        return Err(intake("driver result differs from exact original compute failures"));
                    }
                    Ok((rows, first_failure))
                },
            ).await.map_err(intake)?;
            let (callback, completion) = settled.into_parts_v1();
            let (rows, first_failure) = callback?;
            if completion.err() != first_failure { return Err(intake("epoch closing result differs")); }
            Ok((rows, first_failure))
        },
    ).await)?;
    drop(observers);
    drop(outputs);
    checked(application.revalidate(deadline, budget))?;
    for (index, row) in rows.iter_mut().enumerate() {
        let reference = checked(
            context.find_current_replica_v1(plan.sources[index], plan.destinations[index]),
        )?;
        match row.copy {
            Transfer::Succeeded => {
                let reference =
                    reference.ok_or("successful transfer has no original settled replica")?;
                checked(context.validate_replica_v1(reference))?;
                checked(context.read_allocation(plan.destinations[index], 0, &mut scratch[index]))?;
                native_staged_ring_case::check_bytes(case, index, &scratch[index])?;
                row.checked_copy_bytes = scratch[index].len();
            }
            Transfer::DependencyFailed | Transfer::SettledFailure if reference.is_none() => {}
            _ => return Err("failed branch was incorrectly promoted as a replica".into()),
        }
    }
    let report = native_staged_ring_case::report(case, &plan.observed, &rows, failure.is_some())?;
    plan.close(context)?;
    let usage = result_budget.usage();
    if usage.reserved_peak_bytes != 0
        || usage.retained_members != 0
        || usage.quarantined_members != 0
        || usage.unissued_members != 0
        || usage.poisoned
    {
        return Err(format!(
            "original ring result account not settled: {usage:?}"
        ));
    }
    checked(application.revalidate(deadline, budget))?;
    Ok(report)
}
