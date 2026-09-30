use super::async_journal_tests::region;
use super::*;

struct Fixture {
    context: RuntimeContextV1<MockBackend>,
    source: RuntimeAllocationIdV1,
    stream: RuntimeStreamIdV1,
    submissions: Vec<RuntimeSubmissionV1<RuntimePeerCopyV1>>,
}

impl Fixture {
    fn new(count: usize, journal: bool) -> Self {
        let backend = MockBackend {
            next: 100,
            deferred_copies: true,
            ..MockBackend::default()
        };
        let mut context = if journal {
            RuntimeContextV1::open_with_version_journal_v1(backend, count * 2 + 2, count + 1)
                .unwrap()
        } else {
            RuntimeContextV1::open(backend).unwrap()
        };
        let source_device = context.devices()[0].id();
        let destination_device = context.devices()[1].id();
        let mut submissions = Vec::new();
        let mut first = None;
        for _ in 0..count {
            let stream = context.create_stream(destination_device).unwrap();
            let source = context
                .allocate(source_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap();
            let destination = context
                .allocate(destination_device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap();
            context.write_allocation(source, 0, &[0x51; 64]).unwrap();
            submissions.push(
                context
                    .peer_copy(
                        stream,
                        region(source, RuntimeAccessV1::Read, 0),
                        region(destination, RuntimeAccessV1::Write, 0),
                        &[],
                    )
                    .unwrap(),
            );
            first.get_or_insert((source, stream));
        }
        let (source, stream) = first.unwrap();
        Self {
            context,
            source,
            stream,
            submissions,
        }
    }

    fn admit(&mut self) -> Result<Vec<u64>, RuntimeValidationErrorV1> {
        self.context
            .retained_pair_roster_v1(&self.submissions.iter_mut().collect::<Vec<_>>())
    }
}

#[test]
fn retained_context_roster_uses_existing_scalar_roots_without_backend_effects() {
    for journal in [false, true] {
        for count in [1, 2, 63] {
            let mut f = Fixture::new(count, journal);
            let expected: Vec<_> = f.submissions.iter().map(|s| s.backend_submission).collect();
            assert_eq!(f.admit().unwrap(), expected);
            f.submissions.reverse();
            assert_eq!(
                f.admit().unwrap(),
                expected.into_iter().rev().collect::<Vec<_>>()
            );
            assert!(f.context.backend.batch_calls.is_empty());
            assert!(f.context.native_pair_reservation.is_none());
            assert!(
                f.context
                    .submissions
                    .values()
                    .all(|r| r.status == RuntimeCompletionStatusV1::Pending)
            );
        }
    }
}

#[test]
fn retained_context_roster_rejects_empty_subset_foreign_and_duplicate_handles() {
    let mut f = Fixture::new(2, false);
    assert_eq!(
        f.context.retained_pair_roster_v1(&[]),
        Err(RuntimeValidationErrorV1::InvalidPeerCopyBatch)
    );
    assert_eq!(
        f.context.retained_pair_roster_v1(&[&mut f.submissions[0]]),
        Err(RuntimeValidationErrorV1::InvalidPeerCopyBatch)
    );
    let mut foreign = Fixture::new(1, false);
    let local = Fixture::new(1, false);
    assert_eq!(
        local
            .context
            .retained_pair_roster_v1(&[&mut foreign.submissions[0]]),
        Err(RuntimeValidationErrorV1::UnknownSubmission)
    );
    let backend_id = f.submissions[0].backend_submission;
    f.submissions[1].backend_submission = backend_id;
    f.context
        .submissions
        .get_mut(&f.submissions[1].id)
        .unwrap()
        .backend_submission = backend_id;
    assert_eq!(
        f.admit(),
        Err(RuntimeValidationErrorV1::InvalidPeerCopyBatch)
    );
}

#[test]
fn retained_context_roster_rejects_events_callbacks_dependency_and_nonordinary_state() {
    for fault in 0..6 {
        let mut f = Fixture::new(1, false);
        let id = f.submissions[0].id;
        match fault {
            0 => {
                f.context.record_event(&f.submissions[0]).unwrap();
            }
            1 => {
                f.context.on_completion(&f.submissions[0], |_| {}).unwrap();
            }
            2 => {
                f.context
                    .submissions
                    .get_mut(&id)
                    .unwrap()
                    .dependency_retains = 1;
            }
            3 => {
                f.context
                    .submissions
                    .get_mut(&id)
                    .unwrap()
                    .directed_peer_copy = true;
            }
            4 => {
                f.context.submissions.get_mut(&id).unwrap().producer_launch = true;
            }
            _ => {
                f.context.submissions.get_mut(&id).unwrap().status =
                    RuntimeCompletionStatusV1::Succeeded;
            }
        }
        assert_eq!(
            f.admit(),
            Err(RuntimeValidationErrorV1::InvalidPeerCopyBatch),
            "fault {fault}"
        );
        assert!(f.context.backend.batch_calls.is_empty());
    }
}

#[test]
fn retained_context_persistent_gate_blocks_ordinary_mutation_cleanup_and_graph() {
    let mut f = Fixture::new(1, false);
    f.context.native_pair_reservation = Some(17);
    assert_eq!(f.context.require_retained_pair_v1(17), Ok(()));
    assert_eq!(
        f.context.require_retained_pair_v1(18),
        Err(RuntimeValidationErrorV1::ContextReserved)
    );
    assert!(matches!(
        f.context.write_allocation(f.source, 0, &[1]),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert!(matches!(
        f.context.poll(&mut f.submissions[0]),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert!(matches!(
        f.context.flush_stream(f.stream),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::ContextReserved
        ))
    ));
    assert_eq!(
        f.context.reserve_graph_v1(1),
        Err(RuntimeValidationErrorV1::ContextReserved)
    );
    let report = f.context.cleanup();
    assert!(report.is_native_pair_reserved_v1());
    assert!(!report.is_graph_reserved());
    assert!(!report.is_complete());
    assert!(f.context.backend.cleanup_log.is_empty());
    assert_eq!(f.context.native_pair_reservation, Some(17));
    f.context.terminal = true;
    assert_eq!(
        f.context.require_retained_pair_v1(17),
        Err(RuntimeValidationErrorV1::ContextTerminal)
    );
    assert_eq!(
        f.context.require_live(),
        Err(RuntimeValidationErrorV1::ContextTerminal)
    );
}

#[test]
fn retained_context_shutdown_cannot_return_backend_even_for_empty_reserved_context() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    context.native_pair_reservation = Some(29);
    let failure = context.shutdown().unwrap_err();
    assert!(failure.report.is_native_pair_reserved_v1());
    assert!(!failure.report.is_complete());
    assert_eq!(failure.context.native_pair_reservation, Some(29));
    assert!(failure.context.backend.cleanup_log.is_empty());
}

#[test]
fn retained_context_reservation_establishes_unwind_custody_without_a_journal() {
    let mut context = RuntimeContextV1::open(MockBackend::default()).unwrap();
    assert!(!context.has_unwind_custody_v1());
    context.native_pair_reservation = Some(31);
    assert!(context.has_unwind_custody_v1());
    let failure = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<(), RuntimeBackendFailureV1<MockError>> =
            context.invoke_journal_backend_v1(|_| panic!("retained backend panic"));
    }));
    assert_eq!(
        failure.unwrap_err().downcast_ref::<&str>(),
        Some(&"retained backend panic")
    );
    assert!(context.is_terminal());
    assert_eq!(context.native_pair_reservation, Some(31));
}

#[test]
fn retained_context_real_facade_gates_before_entry_and_restores_before_settlement() {
    let source = include_str!("../native_retained_pair.rs");
    assert!(source.contains("impl RuntimeContextV1<KfdNativeXgmiRuntimeBackendV1>"));
    assert!(
        source
            .find("self.native_pair_reservation = Some(token)")
            .unwrap()
            < source
                .find("backend.begin_retained_peer_batch_v1(")
                .unwrap()
    );
    let finish = source.split("pub fn finish(mut self)").nth(1).unwrap();
    assert!(
        finish
            .find("backend.finish_retained_peer_batch_v1()")
            .unwrap()
            < finish.find("observe_peer_copy_batch_result_v1").unwrap()
    );
    assert!(
        finish.find("observe_peer_copy_batch_result_v1").unwrap()
            < finish.find("native_pair_reservation = None").unwrap()
    );
    assert!(source.contains("self.context.backend.abandon_retained_peer_batch_v1()"));
}
