use super::*;
use crate::RuntimeValidationErrorV1;
use crate::async_engine::PreparedGraphAdmissionV1;

#[test]
fn static_graph_rejects_host_staging_before_preparation_or_reservation() {
    let mut h = Harness::new();
    let stream = h.streams[0];
    let identity = h.context.completion_stream_identity_v1(stream).unwrap();
    let device = h.context.devices()[0].id();
    let allocation = h
        .context
        .allocate(device, crate::RuntimeMemoryKindV1::HostVisible, 32, 8)
        .unwrap();
    let mut request = RuntimeGraphRequestV1::new(
        CompletionGraphV1::new(
            identity.context(),
            vec![identity],
            vec![
                CompletionNodeV1::future(id(1), FutureIdentityV1::new(identity, [1; 32]), None),
                CompletionNodeV1::future(
                    id(2),
                    FutureIdentityV1::new(identity, [2; 32]),
                    Some(id(1)),
                ),
            ],
        )
        .unwrap(),
        vec![(identity, stream)],
    )
    .unwrap();
    request
        .bind_host_staging_v1(
            id(2),
            id(1),
            RuntimeMemoryRegionV1 {
                allocation,
                access: RuntimeAccessV1::Write,
                byte_offset: 0,
                byte_len: 32,
            },
        )
        .unwrap();
    let mut original = Some(request);
    let result = PreparedGraphAdmissionV1::prepare(
        &mut h.context,
        &mut original,
        |_, _, _| -> Result<(), RuntimeGraphErrorV1<MockError>> {
            panic!("static refusal precedes callback")
        },
    );
    assert!(matches!(
        result,
        Err(RuntimeGraphErrorV1::Invalid(
            RuntimeGraphValidationErrorV1::HostStagingRequiresScope
        ))
    ));
    assert!(original.is_some());
    assert_eq!(h.issue_count(), 0);
    h.context.release_allocation(allocation).unwrap();
}

fn prepared(
    h: &mut Harness,
    request: RuntimeGraphRequestV1<MockBackend>,
) -> PreparedGraphAdmissionV1<MockBackend> {
    let mut request = Some(request);
    let admission = PreparedGraphAdmissionV1::prepare(
        &mut h.context,
        &mut request,
        |_, _, _| -> Result<(), RuntimeGraphErrorV1<MockError>> {
            panic!("all ordinary operations were bound")
        },
    );
    assert!(request.is_none());
    admission.unwrap_or_else(|error| panic!("preparation failed: {error:?}"))
}

#[test]
fn shared_graph_preflight_refusal_returns_original_unreserved_request() {
    let mut h = Harness::new();
    let mut request = Some(h.request(false));
    let expected_stream = h.streams[0];
    let mut calls = 0;
    let admission =
        PreparedGraphAdmissionV1::prepare(&mut h.context, &mut request, |context, node, stream| {
            calls += 1;
            assert_eq!(node, id(1));
            assert_eq!(stream, expected_stream);
            assert!(context.completion_stream_identity_v1(stream).is_ok());
            Err::<(), _>(RuntimeGraphErrorV1::Invalid(
                RuntimeGraphValidationErrorV1::MissingOperation,
            ))
        });
    assert!(matches!(
        admission,
        Err(RuntimeGraphErrorV1::Invalid(
            RuntimeGraphValidationErrorV1::MissingOperation
        ))
    ));
    assert_eq!(calls, 1);
    assert_eq!(h.issue_count(), 0);
    let mut request = request.expect("exact input retained on refusal");
    request
        .bind_launch(id(1), h.kernel.clone(), &EmptyArgs, geometry())
        .unwrap();
    let future = h.submit(request);
    h.succeed();
    assert!(result(future).is_ok());
}

#[test]
fn shared_graph_preflight_unwind_preserves_original_unreserved_request() {
    let mut h = Harness::new();
    let mut request = Some(h.request(false));
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = PreparedGraphAdmissionV1::prepare(
            &mut h.context,
            &mut request,
            |_, _, _| -> Result<(), RuntimeGraphErrorV1<MockError>> {
                panic!("checked original-carrier preflight")
            },
        );
    }));
    assert!(panic.is_err());
    assert_eq!(h.issue_count(), 0);
    let mut request = request.expect("original request survives callback unwind");
    request
        .bind_launch(id(1), h.kernel.clone(), &EmptyArgs, geometry())
        .unwrap();
    let future = h.submit(request);
    h.succeed();
    assert!(result(future).is_ok());
}

#[test]
fn shared_graph_commit_refusal_returns_request_without_replacing_reservation() {
    let mut h = Harness::new();
    let request = h.bound(false);
    let plan = prepared(&mut h, request);
    let prior = h.context.reserve_graph_v1(1).unwrap();
    let failure = match plan.commit(&mut h.context) {
        Ok(_) => panic!("another reservation must refuse commit"),
        Err(failure) => failure,
    };
    assert!(matches!(
        failure.error,
        RuntimeGraphErrorV1::Context(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert_eq!(h.issue_count(), 0);
    h.context.close_graph_issue_v1(prior).unwrap();
    h.context.release_graph_v1(prior).unwrap();
    let future = h.submit(failure.request);
    h.succeed();
    assert!(result(future).is_ok());
}

#[test]
#[allow(unsafe_code)]
fn shared_graph_nonterminal_finish_preserves_exact_authority_and_reservation() {
    let mut h = Harness::new();
    let request = h.bound(false);
    let (graph, actions) = match prepared(&mut h, request).commit(&mut h.context) {
        Ok(value) => value,
        Err(_) => panic!("idle Context admits"),
    };
    let token = graph.token();
    let failure = match graph.finish(&mut h.context) {
        Ok(_) => panic!("nonterminal authority cannot finish"),
        Err(failure) => failure,
    };
    assert!(matches!(failure.error, RuntimeGraphErrorV1::Busy));
    assert_eq!(failure.graph.token(), token);
    assert!(!h.context.is_terminal());
    assert!(matches!(
        h.context.destroy_stream(h.streams[0]),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    drop(actions);
    let mut graph = failure.graph;
    // SAFETY: every original ordinary action above was inert and is now dropped;
    // the single occurrence never began and has no native token.
    unsafe { graph.cancel_unissued(0) };
    let retired = match graph.finish(&mut h.context) {
        Ok(retired) => retired,
        Err(_) => panic!("cancelled inert action retires"),
    };
    assert_eq!(retired.execution.generation(), token.generation());
    assert_eq!(h.issue_count(), 0);
    h.context.destroy_stream(h.streams[0]).unwrap();
}

#[test]
#[allow(unsafe_code)]
fn shared_graph_terminal_state_cannot_release_live_context_hold() {
    let mut h = Harness::new();
    let request = h.bound(false);
    let (mut graph, actions) = match prepared(&mut h, request).commit(&mut h.context) {
        Ok(value) => value,
        Err(_) => panic!("idle Context admits"),
    };
    let token = graph.token();
    let _hold = h
        .context
        .hold_unpublished_stream_with_access_v1(h.streams[0], Some(token))
        .unwrap();
    drop(actions);
    // SAFETY: the ordinary action was never issued and is now disposed. The
    // separate live hold is deliberately retained to test Context retirement.
    unsafe { graph.cancel_unissued(0) };
    let failure = match graph.finish(&mut h.context) {
        Ok(_) => panic!("a live original hold prevents retirement"),
        Err(failure) => failure,
    };
    assert!(matches!(
        failure.error,
        RuntimeGraphErrorV1::Context(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::SubmissionPending
        ))
    ));
    assert_eq!(failure.graph.token(), token);
    assert!(failure.graph.is_terminal());
    assert!(h.context.is_terminal());
    assert_eq!(h.issue_count(), 0);
}
