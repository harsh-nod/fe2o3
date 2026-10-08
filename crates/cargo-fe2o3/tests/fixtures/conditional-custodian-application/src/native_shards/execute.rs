//! One existing graph driver, with exact original per-child result gates.
use super::*;
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_conditional_custodian_application::fill_write_only_gpu;
use fe2o3_host::{
    GeneratedRuntimeArgumentLimitsV1, GeneratedRuntimeResultBudgetV1, GeneratedRuntimeWriteSlice,
    NativeConditionalFillIntakeErrorV1 as Intake,
};
use fe2o3_runtime::completion::{
    CompletionGraphV1, CompletionNodeIdV1, CompletionNodeStateV1, CompletionNodeV1,
    FutureIdentityV1,
};
use fe2o3_runtime::{
    RuntimeCompletionStatusV1, RuntimeGfx942ScopeErrorV1 as ScopeError,
    RuntimeGfx942SettledFailureV1 as Failure, RuntimeGraphDeviceCoverageV1, RuntimeGraphRequestV1,
};
use native_shards_case::{Shard, State};

fn intake(error: impl std::fmt::Debug) -> Intake {
    Intake::Rejected(format!("native shard: {error:?}"))
}

fn local(error: ScopeError) -> std::result::Result<Failure, Intake> {
    match error {
        ScopeError::RejectedBeforePublication => Ok(Failure::RejectedBeforePublication),
        ScopeError::DeviceUnavailableBeforeActivation { device_uid } => {
            Ok(Failure::DeviceUnavailableBeforeActivation { device_uid })
        }
        ScopeError::Readback(error) => Ok(Failure::Readback(error)),
        // This caller creates no copies and never cancels a submitted node.
        other => Err(intake(other)),
    }
}

pub(super) async fn run<'work>(
    application: &mut Proved<'work>,
    budget: &mut Budget<'work>,
    context: &mut Context,
    case: &Case,
    deadline: Instant,
) -> Result<String> {
    if context.devices().len() != case.devices.len()
        || context
            .devices()
            .iter()
            .any(|device| device.target() != "gfx942:xnack-")
    {
        return Err("exact selected gfx942 Context roster required".into());
    }
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let mut observed = Vec::with_capacity(devices.len());
    let mut streams = Vec::with_capacity(devices.len());
    for &device in &devices {
        let original = checked(context.with_gfx942_preparation_device_v1(device, |owner| {
            Ok::<_, String>((
                owner.observation().unique_id(),
                owner.observation().render_minor(),
            ))
        }))?;
        observed.push(*original.value());
        streams.push(checked(context.create_stream(device))?);
    }
    case.check_roster(&observed)?;
    let group = checked(
        context.create_graph_group_v1(&streams, RuntimeGraphDeviceCoverageV1::AllAdmitted),
    )?;
    if group.devices().len() != devices.len()
        || devices
            .iter()
            .any(|device| !group.devices().contains(device))
    {
        return Err("original AllAdmitted group omitted a Context device".into());
    }
    let identities = streams
        .iter()
        .map(|&stream| checked(group.stream_identity(stream)))
        .collect::<Result<Vec<_>>>()?;
    let nodes = (0..devices.len())
        .map(|index| {
            CompletionNodeIdV1::new(index as u32 + 1).ok_or("bounded nonzero shard node".into())
        })
        .collect::<Result<Vec<_>>>()?;
    let graph = checked(CompletionGraphV1::new(
        group.context_identity(),
        identities.clone(),
        identities
            .iter()
            .enumerate()
            .map(|(index, &stream)| {
                CompletionNodeV1::future(
                    nodes[index],
                    FutureIdentityV1::new(stream, [index as u8 + 1; 32]),
                    None,
                )
            })
            .collect(),
    ))?;
    let request = checked(RuntimeGraphRequestV1::new_group_v1(graph, group))?;
    let result_budget = checked(GeneratedRuntimeResultBudgetV1::new(8192, devices.len()))?;
    let mut outputs = Vec::with_capacity(devices.len());
    let mut observers = Vec::with_capacity(devices.len());
    for index in 0..devices.len() {
        let (output, observer) = GeneratedRuntimeWriteSlice::new_charged(
            vec![u32::MAX; case.elements(index)].into_boxed_slice(),
        );
        outputs.push(Some(output));
        observers.push(observer);
    }
    let geometry = checked(AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]))?;
    let (shards, failure) = checked(application.with_native_invocation_scope_async_v1(
        budget, deadline, async |native| {
            let settled = context.with_generated_gfx942_scope_settled_async_v1(
                devices.len(), deadline, caller_wake, async |scope| {
                    let graph_ticket = scope.try_admit_graph_v1(request, |node, original| {
                        let index = nodes.iter().position(|&expected| expected == node)
                            .ok_or_else(|| intake("foreign prepared graph node"))?;
                        if original.observation().unique_id() != case.devices[index] {
                            return Err(intake("prepared device differs from original shard"));
                        }
                        let output = outputs[index].take().ok_or_else(|| intake("duplicate prepared shard"))?;
                        native.prepare_generated_invocation::<fill_write_only_gpu::Marker, _>(
                            fill_write_only_gpu::RuntimeArguments::new(output), original, geometry,
                            30_000, GeneratedRuntimeArgumentLimitsV1::new(16384, 16384, 1), &result_budget,
                        )
                    }).map_err(intake)?;
                    let driven = scope.drive_with_wake_v1(caller_wake).await;
                    // A local final result is kept, never an unknown/progress error.
                    // The outer settled API independently closes the original epoch.
                    let driver_failure = match driven { Ok(()) => None, Err(error) => Some(local(error)?) };
                    if scope.pending_v1() != 0 {
                        return Err(intake("original graph remains pending"));
                    }
                    let report = scope.graph_report_v1(&graph_ticket).map_err(intake)?
                        .ok_or_else(|| intake("original graph has not retired"))?;
                    if report.completion.entries().len() != nodes.len()
                        || report.observations.len() != nodes.len()
                        || !report.errors.is_empty() || !report.versions.is_empty()
                        || !report.version_inputs.is_empty() || report.rejected_observations != 0
                        || report.rejected_releases != 0
                    {
                        return Err(intake("scalar-only graph report differs"));
                    }
                    let mut shards = Vec::with_capacity(nodes.len());
                    let mut failures = Vec::with_capacity(nodes.len());
                    for (index, &node) in nodes.iter().enumerate() {
                        let ticket = scope.graph_generated_ticket_v1(&graph_ticket, node).map_err(intake)?
                            .ok_or_else(|| intake("missing original generated ticket"))?;
                        let failure = match scope.completion_v1(&ticket) {
                            Ok(Some(Ok(()))) => None,
                            Ok(Some(Err(error))) => Some(Failure::Readback(error.clone())),
                            Ok(None) => return Err(intake("original decoder remains pending")),
                            Err(error) => Some(local(error)?),
                        };
                        let (state, checked_values) = match &failure {
                            None => {
                                let result = observers[index].take_scoped_completed_v1(scope, &ticket)
                                    .map_err(intake)?.ok_or_else(|| intake("original typed result contention"))?;
                                case.check_fill(index, result.as_slice()).map_err(intake)?;
                                let count = result.len();
                                drop(result);
                                (State::Succeeded, count)
                            }
                            Some(Failure::RejectedBeforePublication) => (State::RejectedBeforePublication, 0),
                            Some(Failure::DeviceUnavailableBeforeActivation { device_uid })
                                if *device_uid == case.devices[index] => (State::DeviceUnavailableBeforeActivation, 0),
                            Some(Failure::Readback(_)) => (State::ReadbackFailed, 0),
                            _ => return Err(intake("local failure does not name its original shard")),
                        };
                        let entry = report.completion.entries()[index];
                        let expected_status = if failure.is_none() { RuntimeCompletionStatusV1::Succeeded }
                            else { RuntimeCompletionStatusV1::QuiescentWithoutResult };
                        if entry.node() != node
                            || !match entry.state() {
                                CompletionNodeStateV1::Succeeded => failure.is_none(),
                                CompletionNodeStateV1::Failed { origin, .. } => failure.is_some() && origin == node,
                                _ => false,
                            }
                            || report.observations.iter().filter(|&&(id, status)| id == node && status == expected_status).count() != 1
                        {
                            return Err(intake("graph outcome differs from original per-ticket settlement"));
                        }
                        if let Some(failure) = failure { failures.push(failure); }
                        shards.push(Shard { uid: observed[index].0, render_minor: observed[index].1,
                            node: node.get(), elements: case.elements(index), state, checked_values });
                    }
                    if match &driver_failure {
                        Some(failure) => !failures.contains(failure),
                        None => !failures.is_empty(),
                    } {
                        return Err(intake("driver local result differs from exact shard roster"));
                    }
                    Ok((shards, driver_failure))
                },
            ).await.map_err(intake)?;
            let (callback, completion) = settled.into_parts_v1();
            let (shards, first_failure) = callback?;
            if completion.err() != first_failure {
                return Err(intake("original epoch closing result differs"));
            }
            Ok((shards, first_failure))
        },
    ).await)?;
    drop(observers);
    drop(outputs);
    checked(application.revalidate(deadline, budget))?;
    for stream in streams.into_iter().rev() {
        checked(context.destroy_stream(stream))?;
    }
    let usage = result_budget.usage();
    if usage.reserved_peak_bytes != 0
        || usage.retained_members != 0
        || usage.quarantined_members != 0
        || usage.unissued_members != 0
        || usage.poisoned
    {
        return Err(format!(
            "original shard result account not settled: {usage:?}"
        ));
    }
    case.report(&observed, &shards, failure.is_some())
}
