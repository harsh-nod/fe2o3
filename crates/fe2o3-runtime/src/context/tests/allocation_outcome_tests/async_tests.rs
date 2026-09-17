use super::*;
use fe2o3_runtime_model::{ContextAllocationStateV1, ContextWriterStateV1};
use writer_tests::assert_diagnostic;

type Context = RuntimeContextV1<AllocationOnlyBackend>;
type Failure = MockMemoryFailure;

struct Fixture {
    context: Context,
    stream: RuntimeStreamIdV1,
    allocations: [RuntimeAllocationIdV1; 2],
    module: RuntimeModuleIdV1,
    kernel: TypedRuntimeKernelV1<AddArguments>,
}

impl Fixture {
    fn new(credits: bool) -> Self {
        let mut context =
            Context::open_with_version_journal_v1(AllocationOnlyBackend::default(), 3, 2).unwrap();
        let device = context.devices()[0].id();
        if credits {
            context
                .configure_allocation_admission_v1(device, 192, 3)
                .unwrap();
        }
        let stream = context.create_stream(device).unwrap();
        let allocations = [
            context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap(),
            context
                .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 16)
                .unwrap(),
        ];
        let module = context.load_module(device, b"object").unwrap();
        let kernel = context
            .resolve_kernel::<AddArguments>(module, "add")
            .unwrap();
        Self {
            context,
            stream,
            allocations,
            module,
            kernel,
        }
    }

    fn launch(&mut self, index: usize) -> RuntimeSubmissionV1<AddArguments> {
        self.context
            .launch(
                self.stream,
                &self.kernel,
                &AddArguments {
                    allocation: self.allocations[index],
                    scalar: 1,
                },
                geometry(),
                &[],
            )
            .unwrap()
    }

    fn state(&self, index: usize) -> ContextAllocationStateV1 {
        self.context
            .versions
            .as_ref()
            .unwrap()
            .journal_for_test()
            .lookup_allocation(
                self.context.allocations[&self.allocations[index]]
                    .journal
                    .unwrap(),
            )
            .unwrap()
    }

    fn assert_writer(&self, index: usize, unknown: bool) {
        let writer = self.state(index).pending_writer.unwrap();
        let state = self
            .context
            .versions
            .as_ref()
            .unwrap()
            .journal_for_test()
            .lookup_writer(writer)
            .unwrap();
        assert_eq!(
            state,
            if unknown {
                ContextWriterStateV1::Unknown { member_count: 1 }
            } else {
                ContextWriterStateV1::Pending { member_count: 1 }
            }
        );
        assert_eq!(self.state(index).content_lineage, 0);
    }

    fn failure(
        &mut self,
        call: DiagnosticCall,
        failure: Failure,
    ) -> (*const Diagnostic, Arc<AtomicUsize>) {
        let drops = Arc::new(AtomicUsize::new(0));
        let diagnostic = Box::new(Diagnostic {
            drops: drops.clone(),
        });
        let pointer = &*diagnostic as *const Diagnostic;
        self.context.backend.failure_call = Some(call);
        self.context.backend.call_failure = failure;
        self.context.backend.call_diagnostic = Some(diagnostic);
        (pointer, drops)
    }
}

#[test]
fn actual_launch_preserves_boxed_submit_errors_and_unwind_identity_without_handles() {
    for credits in [false, true] {
        for failure in [
            Failure::Rejected,
            Failure::Quiescent,
            Failure::Terminal,
            Failure::Panic,
        ] {
            let mut fixture = Fixture::new(credits);
            let (pointer, drops) = fixture.failure(DiagnosticCall::Submit, failure);
            let next = fixture.context.next_identity;
            let result = catch_unwind(AssertUnwindSafe(|| {
                fixture
                    .context
                    .launch(
                        fixture.stream,
                        &fixture.kernel,
                        &AddArguments {
                            allocation: fixture.allocations[0],
                            scalar: 1,
                        },
                        geometry(),
                        &[],
                    )
                    .map(drop)
            }));
            assert_diagnostic(result, failure, pointer, &drops);
            assert_eq!(fixture.context.next_identity, next + 1);
            assert!(fixture.context.submissions.is_empty());
            assert_eq!(fixture.state(0).attempt_epoch, 1);
            assert_eq!(fixture.state(1).attempt_epoch, 0);
            if failure == Failure::Rejected {
                assert!(fixture.state(0).pending_writer.is_none());
                assert_eq!(fixture.state(0).content_lineage, 0);
                assert!(fixture.context.cleanup().is_complete());
            } else {
                fixture.assert_writer(0, true);
                assert_eq!(
                    fixture.context.is_terminal(),
                    matches!(failure, Failure::Terminal | Failure::Panic)
                );
                assert_eq!(
                    fixture.context.cleanup().is_complete(),
                    failure == Failure::Quiescent
                );
            }
        }
    }
}

#[test]
fn observation_failure_matrix_preserves_diagnostics_and_exact_neighbor_credits() {
    for call in [
        DiagnosticCall::Poll,
        DiagnosticCall::Wait,
        DiagnosticCall::Cancel,
        DiagnosticCall::Drain,
        DiagnosticCall::Flush,
    ] {
        for failure in [
            Failure::Rejected,
            Failure::Quiescent,
            Failure::Terminal,
            Failure::Panic,
        ] {
            let mut fixture = Fixture::new(true);
            let mut submission = fixture.launch(0);
            let _neighbor = fixture.launch(1);
            let (pointer, drops) = fixture.failure(call, failure);
            let result = catch_unwind(AssertUnwindSafe(|| match call {
                DiagnosticCall::Poll => fixture.context.poll(&mut submission).map(drop),
                DiagnosticCall::Wait => fixture
                    .context
                    .wait(&mut submission, Duration::ZERO)
                    .map(drop),
                DiagnosticCall::Cancel => fixture.context.cancel(&mut submission).map(drop),
                DiagnosticCall::Drain => fixture
                    .context
                    .drain(&mut submission, Instant::now() + Duration::from_secs(60))
                    .map(drop),
                DiagnosticCall::Flush => fixture.context.flush_stream(fixture.stream),
                _ => unreachable!(),
            }));
            assert_diagnostic(result, failure, pointer, &drops);
            let terminal = matches!(failure, Failure::Terminal | Failure::Panic);
            assert_eq!(fixture.context.is_terminal(), terminal);
            fixture.assert_writer(
                0,
                terminal || (failure == Failure::Quiescent && call != DiagnosticCall::Flush),
            );
            fixture.assert_writer(1, terminal);
            let usage = fixture
                .context
                .allocation_admission_usage_v1(fixture.context.devices()[0].id())
                .unwrap()
                .unwrap();
            assert_eq!(
                usage.used,
                RuntimeResourceVectorV1::ZERO
                    .with(K::RequestedAllocationBytes, 128)
                    .with(K::AllocationRecords, 2)
            );
            assert_eq!(usage.quarantined_records, if terminal { 2 } else { 0 });
            assert_eq!(fixture.context.cleanup().is_complete(), !terminal);
        }
    }
}

#[test]
fn effectful_neighbor_calls_quarantine_pending_writers_on_terminal_or_panic() {
    for call in [
        DiagnosticCall::CreateStream,
        DiagnosticCall::DestroyStream,
        DiagnosticCall::RecordEvent,
        DiagnosticCall::ReleaseEvent,
        DiagnosticCall::ReleaseSubmission,
        DiagnosticCall::LoadModule,
        DiagnosticCall::UnloadModule,
        DiagnosticCall::ResolveKernel,
        DiagnosticCall::Read,
    ] {
        for failure in [Failure::Terminal, Failure::Panic] {
            let mut fixture = Fixture::new(true);
            let mut completed = fixture.launch(1);
            fixture
                .context
                .wait(&mut completed, Duration::ZERO)
                .unwrap();
            let submission = fixture.launch(0);
            let event = if call == DiagnosticCall::ReleaseEvent {
                Some(fixture.context.record_event(&submission).unwrap())
            } else {
                None
            };
            let (pointer, drops) = fixture.failure(call, failure);
            let device = fixture.context.devices()[0].id();
            let result = catch_unwind(AssertUnwindSafe(|| match call {
                DiagnosticCall::CreateStream => fixture.context.create_stream(device).map(drop),
                DiagnosticCall::DestroyStream => fixture.context.destroy_stream(fixture.stream),
                DiagnosticCall::RecordEvent => fixture.context.record_event(&submission).map(drop),
                DiagnosticCall::ReleaseEvent => fixture.context.release_event(event.unwrap()),
                DiagnosticCall::ReleaseSubmission => {
                    fixture.context.release_submission_ref(&completed, None)
                }
                DiagnosticCall::LoadModule => {
                    fixture.context.load_module(device, b"other").map(drop)
                }
                DiagnosticCall::UnloadModule => fixture.context.unload_module(fixture.module),
                DiagnosticCall::ResolveKernel => fixture
                    .context
                    .resolve_kernel::<AddArguments>(fixture.module, "other")
                    .map(drop),
                DiagnosticCall::Read => {
                    fixture
                        .context
                        .read_allocation(fixture.allocations[1], 0, &mut [0; 8])
                }
                _ => unreachable!(),
            }));
            assert_diagnostic(result, failure, pointer, &drops);
            assert!(fixture.context.is_terminal());
            fixture.assert_writer(0, true);
            assert!(fixture.state(1).pending_writer.is_none());
            assert_eq!(fixture.state(1).content_lineage, 1);
            let usage = fixture
                .context
                .allocation_admission_usage_v1(device)
                .unwrap()
                .unwrap();
            assert_eq!((usage.retained_records, usage.quarantined_records), (1, 1));
            assert!(!fixture.context.cleanup().is_complete());
        }
    }
}

#[test]
fn cleanup_failure_preserves_boxed_error_and_retained_unknown_writers() {
    for call in [
        DiagnosticCall::DestroyStream,
        DiagnosticCall::ReleaseEvent,
        DiagnosticCall::ReleaseSubmission,
        DiagnosticCall::UnloadModule,
    ] {
        for failure in [Failure::Terminal, Failure::Panic] {
            let mut fixture = Fixture::new(true);
            let submission = fixture.launch(0);
            fixture.context.record_event(&submission).unwrap();
            let (pointer, drops) = fixture.failure(call, failure);
            let result = catch_unwind(AssertUnwindSafe(|| {
                let mut report = fixture.context.cleanup();
                assert!(!report.is_complete());
                assert!(report.is_terminal());
                assert_eq!(report.failures.len(), 1);
                let RuntimeBackendFailureV1::Terminal(error) =
                    report.failures.pop().unwrap().failure
                else {
                    panic!("wrong cleanup classification");
                };
                Err(RuntimeErrorV1::BackendTerminal(error))
            }));
            assert_diagnostic(result, failure, pointer, &drops);
            fixture.assert_writer(0, true);
            let usage = fixture
                .context
                .allocation_admission_usage_v1(fixture.context.devices()[0].id())
                .unwrap()
                .unwrap();
            assert_eq!((usage.retained_records, usage.quarantined_records), (1, 1));
            assert_eq!(fixture.context.version_journal_writer_records_v1(), Some(1));
        }
    }
}

#[test]
fn allocation_and_host_write_panics_quarantine_other_active_writers() {
    for operation in 0..3 {
        let mut fixture = Fixture::new(true);
        let _submission = fixture.launch(0);
        let drops = Arc::new(AtomicUsize::new(0));
        let diagnostic = Box::new(Diagnostic {
            drops: drops.clone(),
        });
        let pointer = &*diagnostic as *const Diagnostic;
        match operation {
            0 => {
                fixture.context.backend.diagnostic = Some(diagnostic);
                fixture.context.backend.panic_before_outcome = true;
            }
            1 => {
                fixture.context.backend.release_diagnostic = Some(diagnostic);
                fixture.context.backend.release_failure = Failure::Panic;
            }
            _ => {
                fixture.context.backend.write_diagnostic = Some(diagnostic);
                fixture.context.backend.write_failure = Failure::Panic;
            }
        }
        let device = fixture.context.devices()[0].id();
        let result = catch_unwind(AssertUnwindSafe(|| match operation {
            0 => fixture
                .context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .map(drop),
            1 => fixture.context.release_allocation(fixture.allocations[1]),
            _ => fixture
                .context
                .write_allocation(fixture.allocations[1], 0, &[1; 8]),
        }));
        assert_diagnostic(result, Failure::Panic, pointer, &drops);
        assert!(fixture.context.is_terminal());
        fixture.assert_writer(0, true);
        let usage = fixture
            .context
            .allocation_admission_usage_v1(device)
            .unwrap()
            .unwrap();
        assert_eq!(usage.quarantined_records, 2);
        assert!(!fixture.context.cleanup().is_complete());
    }
}

#[test]
fn secondary_settlement_failure_cannot_replace_quiescent_backend_diagnostic() {
    let mut fixture = Fixture::new(true);
    let mut submission = fixture.launch(0);
    fixture
        .context
        .submissions
        .get_mut(&submission.id)
        .unwrap()
        .journal_writer
        .as_mut()
        .unwrap()
        .key
        .local += 1;
    let (pointer, drops) = fixture.failure(DiagnosticCall::Wait, Failure::Quiescent);
    let result = catch_unwind(AssertUnwindSafe(|| {
        fixture
            .context
            .wait(&mut submission, Duration::ZERO)
            .map(drop)
    }));
    assert_diagnostic(result, Failure::Quiescent, pointer, &drops);
    assert!(fixture.context.is_terminal());
    assert_eq!(
        fixture.context.query_submission(&submission).unwrap(),
        RuntimeCompletionStatusV1::Pending
    );
    fixture.assert_writer(0, true);
    assert!(!fixture.context.cleanup().is_complete());
}
