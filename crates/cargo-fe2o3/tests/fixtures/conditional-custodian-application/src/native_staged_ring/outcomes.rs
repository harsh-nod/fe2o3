//! Join an actually retired graph to original decoder gates and exact versions.
use super::*;
use fe2o3_host::{GeneratedRuntimeChargedResultV1, NativeConditionalFillIntakeErrorV1 as Intake};
use fe2o3_runtime::completion::{CompletionNodeIdV1, CompletionNodeStateV1 as Node};
use fe2o3_runtime::*;
use native_staged_ring_case::{Compute, Row, Transfer};

pub(super) fn intake(error: impl std::fmt::Debug) -> Intake {
    Intake::Rejected(format!("native staged ring: {error:?}"))
}

pub(super) fn local(
    error: RuntimeGfx942ScopeErrorV1,
) -> std::result::Result<RuntimeGfx942SettledFailureV1, Intake> {
    match error {
        RuntimeGfx942ScopeErrorV1::RejectedBeforePublication => {
            Ok(RuntimeGfx942SettledFailureV1::RejectedBeforePublication)
        }
        RuntimeGfx942ScopeErrorV1::DeviceUnavailableBeforeActivation { device_uid } => {
            Ok(RuntimeGfx942SettledFailureV1::DeviceUnavailableBeforeActivation { device_uid })
        }
        RuntimeGfx942ScopeErrorV1::Readback(error) => {
            Ok(RuntimeGfx942SettledFailureV1::Readback(error))
        }
        other => Err(intake(other)),
    }
}

fn transfer(
    state: Node,
    node: CompletionNodeIdV1,
    upstream: &[CompletionNodeIdV1],
) -> std::result::Result<Transfer, Intake> {
    match state {
        Node::Succeeded => Ok(Transfer::Succeeded),
        Node::Failed { origin, .. } if origin == node => Ok(Transfer::SettledFailure),
        Node::DependencyFailed { origin, .. } if upstream.contains(&origin) => {
            Ok(Transfer::DependencyFailed)
        }
        other => Err(intake(("not an exact closed branch state", other))),
    }
}

fn version(
    report: &RuntimeGraphReportV1<KfdRuntimeBackendErrorV1>,
    allocation: RuntimeAllocationIdV1,
    producer: CompletionNodeIdV1,
    bytes: usize,
    state: Transfer,
) -> std::result::Result<bool, Intake> {
    let records: Vec<_> = report
        .versions
        .iter()
        .filter(|row| row.version.allocation() == allocation)
        .collect();
    if records.len() != 2 {
        return Err(intake(
            "one initial and one actual produced version required",
        ));
    }
    let initial = records
        .iter()
        .find(|row| row.version.producer().is_none())
        .ok_or_else(|| intake("missing initial version"))?;
    let output = records
        .iter()
        .find(|row| row.version.producer() == Some(producer))
        .ok_or_else(|| intake("missing exact producer version"))?;
    let expected = match state {
        Transfer::Succeeded => RuntimeGraphVersionStateV1::Committed,
        Transfer::DependencyFailed => RuntimeGraphVersionStateV1::NotProduced,
        Transfer::SettledFailure => RuntimeGraphVersionStateV1::Failed,
    };
    if records.iter().any(|row| {
        row.version.execution() != report.execution
            || row.version.byte_offset() != 0
            || row.version.byte_len() != bytes as u64
    }) || initial.predecessor.is_some()
        || initial.state != RuntimeGraphVersionStateV1::AvailableAtAdmission
        || output.predecessor != Some(initial.version)
        || output.state != expected
        || output.current_at_terminal != (state == Transfer::Succeeded)
        || initial.current_at_terminal != (state == Transfer::DependencyFailed)
    {
        return Err(intake("exact retired version lineage differs"));
    }
    Ok(state == Transfer::Succeeded)
}

pub(super) fn collect<'scope, P: RuntimeGfx942GeneratedCompletionCarrierV1>(
    scope: &RuntimeGfx942GeneratedScopeV1<'scope, '_, Backend, P>,
    ticket: &RuntimeGfx942ScopedGraphTicketV1<'scope>,
    plan: &plan::Plan,
    case: &Case,
    observers: &mut [GeneratedRuntimeChargedResultV1<u32>],
    staged: &[bool],
    refused: &[bool],
) -> std::result::Result<(Vec<Row>, Vec<RuntimeGfx942SettledFailureV1>), Intake> {
    let report = scope
        .graph_report_v1(ticket)
        .map_err(intake)?
        .ok_or_else(|| intake("graph not retired"))?;
    let n = plan.devices.len();
    if scope.pending_v1() != 0
        || report.completion.entries().len() != n * 5
        || report.versions.len() != n * 4
        || report.version_inputs.len() != n
        || report.rejected_releases != 0
        || report.rejected_observations != 0
    {
        return Err(intake("retired graph roster/counters differ"));
    }
    for &(node, _) in &report.observations {
        if report
            .observations
            .iter()
            .filter(|&&(id, _)| id == node)
            .count()
            != 1
            || !plan
                .nodes
                .iter()
                .any(|ids| [ids[0], ids[1], ids[4]].contains(&node))
        {
            return Err(intake("duplicate or foreign operation observation"));
        }
    }
    for (node, error) in &report.errors {
        if !plan.nodes.iter().any(|ids| ids[4] == *node)
            || !matches!(
                error,
                RuntimeErrorV1::Validation(_)
                    | RuntimeErrorV1::BackendRejected(_)
                    | RuntimeErrorV1::BackendQuiescent(_)
            )
            || !matches!(scope.graph_node_state_v1(ticket, *node).map_err(intake)?, Node::Failed { origin, .. } if origin == *node)
        {
            return Err(intake(
                "unclassified graph error is not local copy settlement",
            ));
        }
    }
    let mut rows = Vec::with_capacity(n);
    let mut failures = Vec::with_capacity(n);
    for (index, ids) in plan.nodes.iter().enumerate() {
        for (offset, &node) in ids.iter().enumerate() {
            if report.completion.entries()[index * 5 + offset].node() != node {
                return Err(intake("exact ordered node roster differs"));
            }
        }
        let generated = scope
            .graph_generated_ticket_v1(ticket, ids[0])
            .map_err(intake)?
            .ok_or_else(|| intake("original generated ticket missing"))?;
        let failure = match scope.completion_v1(&generated) {
            Ok(Some(Ok(()))) => None,
            Ok(Some(Err(error))) => Some(RuntimeGfx942SettledFailureV1::Readback(error.clone())),
            Ok(None) => return Err(intake("decoder still pending")),
            Err(error) => Some(local(error)?),
        };
        let compute = match &failure {
            None => Compute::Succeeded,
            Some(RuntimeGfx942SettledFailureV1::RejectedBeforePublication) => {
                Compute::RejectedBeforePublication
            }
            Some(RuntimeGfx942SettledFailureV1::DeviceUnavailableBeforeActivation {
                device_uid,
            }) if *device_uid == case.devices[index] => Compute::DeviceUnavailableBeforeActivation,
            Some(RuntimeGfx942SettledFailureV1::Readback(_)) => Compute::ReadbackFailed,
            _ => return Err(intake("failure does not name original shard")),
        };
        let computed = failure.is_none();
        let compute_state = scope.graph_node_state_v1(ticket, ids[0]).map_err(intake)?;
        if !match compute_state {
            Node::Succeeded => computed,
            Node::Failed { origin, .. } => !computed && origin == ids[0],
            _ => false,
        } || !report.observations.contains(&(
            ids[0],
            if computed {
                RuntimeCompletionStatusV1::Succeeded
            } else {
                RuntimeCompletionStatusV1::QuiescentWithoutResult
            },
        )) {
            return Err(intake("original compute settlement differs from graph"));
        }
        let checked_values = if computed {
            let output = observers[index]
                .take_scoped_completed_v1(scope, &generated)
                .map_err(intake)?
                .ok_or_else(|| intake("typed output contention"))?;
            case.check_fill(index, output.as_slice()).map_err(intake)?;
            let len = output.len();
            drop(output);
            len
        } else {
            0
        };
        let staging = transfer(
            scope.graph_node_state_v1(ticket, ids[1]).map_err(intake)?,
            ids[1],
            &ids[..1],
        )?;
        let copy = transfer(
            scope.graph_node_state_v1(ticket, ids[4]).map_err(intake)?,
            ids[4],
            &ids[..2],
        )?;
        if staged[index] != (staging == Transfer::Succeeded)
            || refused[index] != (staging == Transfer::SettledFailure)
        {
            return Err(intake("staging observation differs from actual invocation"));
        }
        for node in &ids[2..4] {
            let expected = scope.graph_node_state_v1(ticket, *node).map_err(intake)?;
            if transfer(expected, *node, &ids[..2])?
                != if staging == Transfer::Succeeded {
                    Transfer::Succeeded
                } else {
                    Transfer::DependencyFailed
                }
            {
                return Err(intake("event edge differs from actual branch"));
            }
        }
        for (node, state) in [(ids[1], staging), (ids[4], copy)] {
            let observations: Vec<_> = report
                .observations
                .iter()
                .filter(|&&(id, _)| id == node)
                .map(|&(_, status)| status)
                .collect();
            match state {
                Transfer::Succeeded if observations == [RuntimeCompletionStatusV1::Succeeded] => {}
                Transfer::DependencyFailed if observations.is_empty() => {}
                Transfer::SettledFailure
                    if observations.is_empty()
                        && report.errors.iter().any(|(id, _)| *id == node) => {}
                Transfer::SettledFailure
                    if observations.len() == 1
                        && matches!(
                            observations[0],
                            RuntimeCompletionStatusV1::Failed(_)
                                | RuntimeCompletionStatusV1::QuiescentWithoutResult
                        ) => {}
                _ => {
                    return Err(intake(
                        "copy/staging observation is not exact terminal settlement",
                    ));
                }
            }
        }
        let bytes = case.elements(index) * 4;
        let staged_version_committed =
            version(report, plan.sources[index], ids[1], bytes, staging)?;
        let copy_version_committed =
            version(report, plan.destinations[index], ids[4], bytes, copy)?;
        let inputs: Vec<_> = report
            .version_inputs
            .iter()
            .filter(|input| input.consumer == ids[4])
            .collect();
        if inputs.len() != 1
            || inputs[0].version.execution() != report.execution
            || inputs[0].version.producer() != Some(ids[1])
            || inputs[0].version.allocation() != plan.sources[index]
            || inputs[0].version.byte_offset() != 0
            || inputs[0].version.byte_len() != bytes as u64
            || inputs[0].available_at_issue != (copy != Transfer::DependencyFailed)
        {
            return Err(intake("copy did not retain exact staged producer version"));
        }
        if let Some(failure) = failure {
            failures.push(failure);
        }
        rows.push(Row {
            source: plan.observed[index],
            destination: plan.observed[(index + 1) % n],
            nodes: ids.map(|node| node.get()),
            compute,
            staging,
            copy,
            checked_values,
            checked_copy_bytes: 0,
            staged_version_committed,
            copy_input_available: inputs[0].available_at_issue,
            copy_version_committed,
        });
    }
    Ok((rows, failures))
}
