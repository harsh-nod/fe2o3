#![cfg(test)]

use super::*;
use crate::async_engine::{AdmittedGraphV1, PreparedGraphAdmissionV1, RetiredGraphV1};
use crate::completion::{
    CompletionGraphV1, CompletionNodeIdV1, CompletionNodeV1, EventIdentityV1, FutureIdentityV1,
};
use crate::{
    RuntimeGraphErrorV1, RuntimeGraphRequestV1, RuntimeGraphValidationErrorV1,
    RuntimeGraphVersionSourceV1,
};

mod refusals;

type Request = RuntimeGraphRequestV1<MockBackend>;

fn node(n: u32) -> CompletionNodeIdV1 {
    CompletionNodeIdV1::new(n).unwrap()
}

fn allocate(f: &mut Fixture, device: usize, value: u8) -> RuntimeAllocationIdV1 {
    let allocation = f
        .context
        .allocate(
            f.context.devices()[device].id(),
            RuntimeMemoryKindV1::HostVisible,
            96,
            16,
        )
        .unwrap();
    f.context
        .write_allocation(allocation, 0, &[value; 96])
        .unwrap();
    allocation
}

fn fanout(f: &mut Fixture) -> (Request, [RuntimeAllocationIdV1; 2]) {
    let seed = allocate(f, 0, 0x79);
    let sinks = [allocate(f, 1, 0xe1), allocate(f, 2, 0xe2)];
    let group = f
        .context
        .create_graph_group_v1(&f.streams, RuntimeGraphDeviceCoverageV1::AllAdmitted)
        .unwrap();
    let streams = f.streams.map(|s| group.stream_identity(s).unwrap());
    let event = EventIdentityV1::new(group.context_identity(), [0x51; 32]);
    let future = |n, stream, predecessor: Option<u32>| {
        CompletionNodeV1::future(
            node(n),
            FutureIdentityV1::new(stream, [n as u8; 32]),
            predecessor.map(node),
        )
    };
    let graph = CompletionGraphV1::new(
        group.context_identity(),
        streams.to_vec(),
        vec![
            future(1, streams[0], None),
            CompletionNodeV1::record_event(node(2), streams[0], event, Some(node(1))),
            CompletionNodeV1::wait_event(node(3), streams[1], event, node(2), None),
            future(4, streams[1], Some(3)),
            future(5, streams[1], Some(4)),
            CompletionNodeV1::wait_event(node(6), streams[2], event, node(2), None),
            future(7, streams[2], Some(6)),
            future(8, streams[2], Some(7)),
        ],
    )
    .unwrap();
    let mut request = Request::new_group_v1(graph, group).unwrap();
    request
        .bind_copy(
            node(1),
            region(seed, RuntimeAccessV1::Read),
            region(f.allocations[0], RuntimeAccessV1::Write),
        )
        .unwrap();
    for (copy, consume, index) in [(4, 5, 1), (7, 8, 2)] {
        request
            .bind_tracked_replica_copy_v1(
                node(copy),
                region(f.allocations[0], RuntimeAccessV1::Read),
                region(f.allocations[index], RuntimeAccessV1::Write),
            )
            .unwrap();
        request
            .expect_input_version(
                node(copy),
                region(f.allocations[0], RuntimeAccessV1::Read),
                RuntimeGraphVersionSourceV1::ProducedBy(node(1)),
            )
            .unwrap();
        request
            .bind_copy(
                node(consume),
                region(f.allocations[index], RuntimeAccessV1::Read),
                region(sinks[index - 1], RuntimeAccessV1::Write),
            )
            .unwrap();
        request
            .expect_input_version(
                node(consume),
                region(f.allocations[index], RuntimeAccessV1::Read),
                RuntimeGraphVersionSourceV1::ProducedBy(node(copy)),
            )
            .unwrap();
    }
    (request, sinks)
}

fn prepare(
    context: &mut Context,
    request: &mut Option<Request>,
) -> Result<PreparedGraphAdmissionV1<MockBackend>, RuntimeGraphErrorV1<MockError>> {
    PreparedGraphAdmissionV1::prepare(context, request, |_, _, _| {
        Err(RuntimeGraphErrorV1::Invalid(
            RuntimeGraphValidationErrorV1::MissingOperation,
        ))
    })
}

fn admit(
    context: &mut Context,
    request: Request,
) -> (
    AdmittedGraphV1,
    Vec<Option<Box<PreparedContextGraphActionV1>>>,
) {
    match prepare(context, &mut Some(request))
        .unwrap()
        .commit(context)
    {
        Ok(owners) => owners,
        Err(failure) => panic!("graph refusal: {:?}", failure.error),
    }
}

// SAFETY: every successful operation below follows its actual Context poll and
// release; failures are definite no-owner rejections, joins are host-only.
#[allow(unsafe_code)]
fn settle(
    context: &mut Context,
    mut graph: AdmittedGraphV1,
    mut actions: Vec<Option<Box<PreparedContextGraphActionV1>>>,
    refuse_node: Option<u32>,
    retry_release: bool,
) -> RetiredGraphV1 {
    let mut retried = false;
    while let Some(index) = graph.pop_ready_notification() {
        assert!(graph.begin(index));
        let Some(action) = actions[index].take() else {
            unsafe { graph.succeed_join(index) };
            continue;
        };
        if refuse_node.is_some_and(|id| graph.id(index) == node(id)) {
            context.backend.copy_failure = MockMemoryFailure::Rejected;
        }
        let result = context.submit_graph_action_v1(graph.token(), *action);
        context.backend.copy_failure = MockMemoryFailure::None;
        let mut submission = match result {
            Ok(submission) => submission,
            Err(error) => {
                assert!(refuse_node.is_some_and(|id| graph.id(index) == node(id)));
                assert!(matches!(error, RuntimeErrorV1::BackendRejected(_)));
                assert!(!context.is_terminal());
                unsafe { graph.fail(index, 1) };
                continue;
            }
        };
        for _ in 0..4 {
            if context
                .poll_with_graph_access_v1(&mut submission.original, Some(graph.token()))
                .unwrap()
                == RuntimePollV1::Succeeded
            {
                break;
            }
        }
        assert_eq!(
            context.query_submission(&submission.original).unwrap(),
            RuntimeCompletionStatusV1::Succeeded
        );
        if submission.replica.is_some() && retry_release && !retried {
            let usage = context.replica_registry_usage_v1().unwrap();
            context.backend.release_submission_failure = MockMemoryFailure::Rejected;
            assert!(matches!(
                context.release_graph_submission_v1(graph.token(), &submission),
                Err(RuntimeErrorV1::BackendRejected(_))
            ));
            assert_eq!(context.replica_registry_usage_v1().unwrap(), usage);
            assert!(context.submissions.contains_key(&submission.original.id));
            retried = true;
        }
        context
            .release_graph_submission_v1(graph.token(), &submission)
            .unwrap();
        assert!(unsafe { graph.succeed_operation(index) });
    }
    assert!(graph.is_terminal());
    drop(actions);
    match graph.finish(context) {
        Ok(report) => report,
        Err(failure) => panic!("graph retirement: {:?}", failure.error),
    }
}

#[test]
fn graph_replica_fanout_binds_actual_produced_version_and_release_before_consumers() {
    let mut f = Fixture::new(2);
    let (request, sinks) = fanout(&mut f);
    let original_account = f.account.usage();
    let (graph, actions) = admit(&mut f.context, request);
    assert_eq!(f.context.backend.copy_call_count, 0);
    assert_eq!(f.context.replica_registry_usage_v1().unwrap().pending, 0);
    let report = settle(&mut f.context, graph, actions, None, true);
    assert_eq!(
        report
            .version_inputs
            .iter()
            .filter(|input| input.version.producer().is_some())
            .count(),
        4
    );
    assert_eq!(f.context.replica_registry_usage_v1().unwrap().settled, 2);
    assert_eq!(f.account.usage(), original_account);
    let mut references = Vec::new();
    for index in 1..=2 {
        references.push(
            f.context
                .find_current_replica_v1(f.allocations[0], f.allocations[index])
                .unwrap()
                .unwrap(),
        );
        assert_eq!(f.read(index), [0x79; 96]);
        let mut bytes = [0; 96];
        f.context
            .read_allocation(sinks[index - 1], 0, &mut bytes)
            .unwrap();
        assert_eq!(bytes, [0x79; 96]);
    }
    f.context
        .write_allocation(f.allocations[0], 0, &[0x79; 96])
        .unwrap();
    for reference in references {
        assert!(f.context.validate_replica_v1(reference).is_err());
    }
    f.finish();
}

#[test]
fn graph_replica_definite_child_refusal_does_not_poison_independent_child() {
    let mut f = Fixture::new(2);
    let (request, sinks) = fanout(&mut f);
    let (graph, actions) = admit(&mut f.context, request);
    let _report = settle(&mut f.context, graph, actions, Some(4), false);
    assert!(!f.context.is_terminal());
    assert_eq!(f.context.replica_registry_usage_v1().unwrap().settled, 1);
    assert!(
        f.context
            .find_current_replica_v1(f.allocations[0], f.allocations[1])
            .unwrap()
            .is_none()
    );
    assert!(
        f.context
            .find_current_replica_v1(f.allocations[0], f.allocations[2])
            .unwrap()
            .is_some()
    );
    let mut bytes = [0; 96];
    f.context.read_allocation(sinks[0], 0, &mut bytes).unwrap();
    assert_eq!(bytes, [0xe1; 96]);
    f.context.read_allocation(sinks[1], 0, &mut bytes).unwrap();
    assert_eq!(bytes, [0x79; 96]);
    f.finish();
}

#[test]
fn graph_replica_failed_producer_never_issues_or_certifies_descendants() {
    let mut f = Fixture::new(2);
    let (request, sinks) = fanout(&mut f);
    let (graph, actions) = admit(&mut f.context, request);
    let _report = settle(&mut f.context, graph, actions, Some(1), false);
    assert!(!f.context.is_terminal());
    let usage = f.context.replica_registry_usage_v1().unwrap();
    assert_eq!((usage.pending, usage.settled), (0, 0));
    for (index, sink) in sinks.into_iter().enumerate() {
        assert_eq!(f.read(index + 1), [0x32 + index as u8; 96]);
        let mut bytes = [0; 96];
        f.context.read_allocation(sink, 0, &mut bytes).unwrap();
        assert_eq!(bytes, [0xe1 + index as u8; 96]);
        assert!(
            f.context
                .find_current_replica_v1(f.allocations[0], f.allocations[index + 1])
                .unwrap()
                .is_none()
        );
    }
    f.finish();
}

// SAFETY: only event joins are marked successful; the tracked operation remains
// in flight and its original graph/table/Context custody must fail stop.
#[allow(unsafe_code)]
pub(super) fn start_unknown(f: &mut Fixture, panic: bool) {
    let mut request = refusals::simple(f, false);
    request
        .bind_tracked_replica_copy_v1(
            node(3),
            region(f.allocations[0], RuntimeAccessV1::Read),
            region(f.allocations[1], RuntimeAccessV1::Write),
        )
        .unwrap();
    let (mut graph, mut actions) = admit(&mut f.context, request);
    loop {
        let index = graph.pop_ready_notification().unwrap();
        assert!(graph.begin(index));
        if let Some(action) = actions[index].take() {
            f.context.backend.copy_failure = if panic {
                MockMemoryFailure::Panic
            } else {
                MockMemoryFailure::Terminal
            };
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                f.context
                    .submit_graph_action_v1(graph.token(), *action)
                    .is_err()
            }));
            assert!(matches!(outcome, Ok(true)) || outcome.is_err());
            assert!(f.context.is_terminal());
            assert!(!graph.is_terminal());
            assert_eq!(f.context.graph_reservation, Some(graph.token()));
            assert_eq!(f.context.replica_registry_usage_v1().unwrap().pending, 1);
            return;
        }
        unsafe { graph.succeed_join(index) };
    }
}
