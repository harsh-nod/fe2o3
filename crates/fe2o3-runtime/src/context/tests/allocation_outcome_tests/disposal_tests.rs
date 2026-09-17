use super::super::async_journal_tests::{MixedArguments, region};
use super::*;
use fe2o3_runtime_model::{
    ContextAllocationStateV1, ContextWriterReferenceV1, ContextWriterStateV1,
};
use writer_tests::assert_diagnostic;

type Context = RuntimeContextV1<AllocationOnlyBackend>;
type Failure = MockMemoryFailure;

struct Fixture {
    context: Context,
    ids: [RuntimeAllocationIdV1; 3],
    records: [AllocationRecordV1; 3],
    states: [ContextAllocationStateV1; 3],
    neighbor: RuntimeAllocationIdV1,
    writer: ContextWriterReferenceV1,
    submission_id: RuntimeSubmissionIdV1,
    submission: Option<RuntimeSubmissionV1<MixedArguments>>,
    callbacks: Arc<AtomicUsize>,
}

impl Fixture {
    fn new(credits: bool, outcome: u8, release_metadata: bool) -> Self {
        let mut context =
            Context::open_with_version_journal_v1(AllocationOnlyBackend::default(), 6, 2).unwrap();
        let device = context.devices()[0].id();
        if credits {
            context
                .configure_allocation_admission_v1(device, 384, 6)
                .unwrap();
        }
        let stream = context.create_stream(device).unwrap();
        let ids = std::array::from_fn(|_| {
            context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
                .unwrap()
        });
        let neighbor = context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 16)
            .unwrap();
        let module = context.load_module(device, b"mixed").unwrap();
        let kernel = context
            .resolve_kernel::<MixedArguments>(module, "mixed")
            .unwrap();
        let args = MixedArguments(vec![
            region(ids[2], RuntimeAccessV1::Write, 0),
            region(neighbor, RuntimeAccessV1::Read, 0),
            region(ids[0], RuntimeAccessV1::ReadWrite, 8),
            region(ids[2], RuntimeAccessV1::Write, 16),
            region(ids[1], RuntimeAccessV1::Write, 0),
        ]);
        let submission_id =
            RuntimeSubmissionIdV1::new(context.context_generation, context.next_identity);
        let callbacks = Arc::new(AtomicUsize::new(0));
        let mut submission = if outcome == 0 {
            let drops = Arc::new(AtomicUsize::new(0));
            let diagnostic = Box::new(Diagnostic {
                drops: drops.clone(),
            });
            let pointer = &*diagnostic as *const Diagnostic;
            context.backend.failure_call = Some(DiagnosticCall::Submit);
            context.backend.call_failure = Failure::Quiescent;
            context.backend.call_diagnostic = Some(diagnostic);
            let result = catch_unwind(AssertUnwindSafe(|| {
                context
                    .launch(stream, &kernel, &args, geometry(), &[])
                    .map(drop)
            }));
            assert_diagnostic(result, Failure::Quiescent, pointer, &drops);
            None
        } else {
            let mut submission = context
                .launch(stream, &kernel, &args, geometry(), &[])
                .unwrap();
            for allocation in ids {
                assert!(matches!(
                    context.release_allocation(allocation),
                    Err(RuntimeErrorV1::Validation(
                        RuntimeValidationErrorV1::ContextReserved
                    ))
                ));
            }
            assert_eq!(context.backend.release_calls, 0);
            let observed = callbacks.clone();
            context
                .on_completion(&submission, move |_| {
                    observed.fetch_add(1, Ordering::SeqCst);
                })
                .unwrap();
            match outcome {
                1 => {
                    context.backend.inner.wait_observation =
                        Some(BackendPollV1::Failed { code: -2 });
                    assert_eq!(
                        context.wait(&mut submission, Duration::ZERO).unwrap(),
                        RuntimePollV1::Failed { code: -2 }
                    );
                }
                2 => {
                    let drops = Arc::new(AtomicUsize::new(0));
                    let diagnostic = Box::new(Diagnostic {
                        drops: drops.clone(),
                    });
                    let pointer = &*diagnostic as *const Diagnostic;
                    context.backend.failure_call = Some(DiagnosticCall::Wait);
                    context.backend.call_failure = Failure::Quiescent;
                    context.backend.call_diagnostic = Some(diagnostic);
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        context.wait(&mut submission, Duration::ZERO).map(drop)
                    }));
                    assert_diagnostic(result, Failure::Quiescent, pointer, &drops);
                }
                3 => context.destroy_stream(stream).unwrap(),
                _ => unreachable!(),
            }
            Some(submission)
        };
        if release_metadata && let Some(submission) = submission.take() {
            context.release_submission(submission).unwrap();
        }
        let records = ids.map(|id| context.allocations[&id]);
        let journal = context.versions.as_ref().unwrap().journal_for_test();
        let states =
            records.map(|record| journal.lookup_allocation(record.journal.unwrap()).unwrap());
        let writer = states[0].pending_writer.unwrap();
        assert!(
            states
                .iter()
                .all(|state| state.pending_writer == Some(writer))
        );
        assert_eq!(writer.key.local, submission_id.local);
        Self {
            context,
            ids,
            records,
            states,
            neighbor,
            writer,
            submission_id,
            submission,
            callbacks,
        }
    }

    fn assert_unknown(&self, disposed: usize) {
        let versions = self.context.versions.as_ref().unwrap();
        assert_eq!(
            versions.journal_for_test().lookup_writer(self.writer),
            Ok(ContextWriterStateV1::Unknown { member_count: 3 })
        );
        for (index, (record, state)) in self.records.iter().zip(&self.states).enumerate() {
            assert_eq!(
                versions
                    .journal_for_test()
                    .lookup_allocation(record.journal.unwrap()),
                Ok(*state)
            );
            let (id, original, member, disposed) =
                versions.submission_disposal_member_for_test_v1(self.submission_id, index);
            assert_eq!(id, self.ids[index]);
            assert_eq!(original, *record);
            assert_eq!(member.allocation, record.journal.unwrap());
            assert_eq!(
                member.device.context_generation,
                record.device.context_generation
            );
            assert_eq!(member.device.local, record.device.local);
            assert_eq!(member.byte_extent, record.byte_len);
            assert_eq!(disposed, !self.context.allocations.contains_key(&id));
        }
        assert_eq!(
            versions.submission_disposal_progress_for_test_v1(self.submission_id),
            Some((disposed, false))
        );
        assert_eq!(self.context.version_journal_writer_records_v1(), Some(1));
    }

    fn assert_charge(&self, records: u64, quarantined: usize) {
        let Some(usage) = self
            .context
            .allocation_admission_usage_v1(self.context.devices()[0].id())
            .unwrap()
        else {
            return;
        };
        assert_eq!(
            usage.used,
            RuntimeResourceVectorV1::ZERO
                .with(K::RequestedAllocationBytes, 64 * records)
                .with(K::AllocationRecords, records)
        );
        assert_eq!(usage.quarantined_records, quarantined);
        assert_eq!(usage.retained_records, records as usize - quarantined);
    }

    fn fail_release(
        &mut self,
        index: usize,
        failure: Failure,
    ) -> (*const Diagnostic, Arc<AtomicUsize>) {
        let drops = Arc::new(AtomicUsize::new(0));
        let diagnostic = Box::new(Diagnostic {
            drops: drops.clone(),
        });
        let pointer = &*diagnostic as *const Diagnostic;
        self.context.backend.release_failure = failure;
        self.context.backend.release_diagnostic = Some(diagnostic);
        self.context.backend.release_failure_handle = Some(self.records[index].backend_allocation);
        (pointer, drops)
    }
}

#[test]
fn unknown_roster_disposal_joins_all_orders_outcomes_and_metadata_lifetimes() {
    for credits in [false, true] {
        for outcome in 0..4 {
            for release_metadata in [false, true] {
                for order in [
                    [0, 1, 2],
                    [0, 2, 1],
                    [1, 0, 2],
                    [1, 2, 0],
                    [2, 0, 1],
                    [2, 1, 0],
                ] {
                    let mut fixture = Fixture::new(credits, outcome, release_metadata);
                    let next = fixture.context.next_identity;
                    let callbacks = fixture.callbacks.load(Ordering::SeqCst);
                    let status = fixture
                        .submission
                        .as_ref()
                        .map(|submission| fixture.context.query_submission(submission).unwrap());
                    for (count, index) in order.into_iter().enumerate() {
                        fixture
                            .context
                            .release_allocation(fixture.ids[index])
                            .unwrap();
                        assert!(
                            !fixture
                                .context
                                .allocations
                                .contains_key(&fixture.ids[index])
                        );
                        assert!(
                            !fixture
                                .context
                                .backend
                                .inner
                                .memory
                                .contains_key(&fixture.records[index].backend_allocation)
                        );
                        assert!(
                            !fixture
                                .context
                                .backend_allocations
                                .contains(&fixture.records[index].backend_allocation)
                        );
                        let calls = fixture.context.backend.release_calls;
                        assert!(matches!(
                            fixture.context.release_allocation(fixture.ids[index]),
                            Err(RuntimeErrorV1::Validation(
                                RuntimeValidationErrorV1::UnknownAllocation
                            ))
                        ));
                        assert!(matches!(
                            fixture
                                .context
                                .read_allocation(fixture.ids[index], 0, &mut [0; 8]),
                            Err(RuntimeErrorV1::Validation(
                                RuntimeValidationErrorV1::UnknownAllocation
                            ))
                        ));
                        assert_eq!(fixture.context.backend.release_calls, calls);
                        if count < 2 {
                            fixture.assert_unknown(count + 1);
                            fixture.assert_charge(4, 0);
                            assert_eq!(
                                fixture
                                    .context
                                    .version_journal_usage_v1()
                                    .unwrap()
                                    .allocation_records,
                                4
                            );
                        }
                    }
                    assert_eq!(fixture.context.next_identity, next);
                    assert_eq!(
                        fixture.context.backend.release_handles,
                        order.map(|index| fixture.records[index].backend_allocation)
                    );
                    assert_eq!(fixture.context.version_journal_writer_records_v1(), Some(0));
                    assert_eq!(
                        fixture
                            .context
                            .version_journal_usage_v1()
                            .unwrap()
                            .allocation_records,
                        1
                    );
                    assert!(
                        fixture
                            .context
                            .versions
                            .as_ref()
                            .unwrap()
                            .submission_disposal_progress_for_test_v1(fixture.submission_id)
                            .is_none()
                    );
                    fixture.assert_charge(1, 0);
                    if let Some(submission) = &fixture.submission {
                        assert_eq!(
                            fixture.context.query_submission(submission).unwrap(),
                            status.unwrap()
                        );
                        assert!(
                            fixture.context.submissions[&submission.id]
                                .journal_writer
                                .is_none()
                        );
                    }
                    assert_eq!(fixture.callbacks.load(Ordering::SeqCst), callbacks);
                    fixture
                        .context
                        .write_allocation(fixture.neighbor, 0, &[7; 8])
                        .unwrap();
                    assert!(fixture.context.cleanup().is_complete());
                    fixture.assert_charge(0, 0);
                }
            }
        }
    }
}

#[test]
fn unknown_roster_release_failure_preserves_partial_receipts_and_boxed_diagnostics() {
    for credits in [false, true] {
        for failure in [
            Failure::Rejected,
            Failure::Quiescent,
            Failure::Terminal,
            Failure::Panic,
        ] {
            for prefix in 0..3 {
                let mut fixture = Fixture::new(credits, 1, true);
                let order = [2, 0, 1];
                for index in &order[..prefix] {
                    fixture
                        .context
                        .release_allocation(fixture.ids[*index])
                        .unwrap();
                }
                let index = order[prefix];
                let memory = fixture.context.backend.inner.memory.clone();
                let (pointer, drops) = fixture.fail_release(index, failure);
                let result = catch_unwind(AssertUnwindSafe(|| {
                    fixture.context.release_allocation(fixture.ids[index])
                }));
                assert_diagnostic(result, failure, pointer, &drops);
                let terminal = matches!(failure, Failure::Terminal | Failure::Panic);
                assert_eq!(fixture.context.is_terminal(), terminal);
                assert_eq!(fixture.context.backend.inner.memory, memory);
                fixture.assert_unknown(prefix);
                fixture.assert_charge(4, if terminal { 3 } else { 0 });
                if terminal {
                    let calls = fixture.context.backend.release_calls;
                    assert!(matches!(
                        fixture.context.release_allocation(fixture.ids[index]),
                        Err(RuntimeErrorV1::Validation(
                            RuntimeValidationErrorV1::ContextTerminal
                        ))
                    ));
                    let report = fixture.context.cleanup();
                    assert!(!report.is_complete());
                    assert_eq!(report.writer_journal_records_v1(), 1);
                    assert_eq!(report.allocation_journal_records_v1(), 4);
                    assert_eq!(fixture.context.backend.release_calls, calls);
                } else {
                    for index in &order[prefix..] {
                        fixture
                            .context
                            .release_allocation(fixture.ids[*index])
                            .unwrap();
                    }
                    fixture.assert_charge(1, 0);
                    assert_eq!(fixture.context.backend.release_calls, 4);
                    assert!(fixture.context.cleanup().is_complete());
                }
            }
        }
    }
}

#[test]
fn cleanup_resumes_partial_rosters_without_releasing_successful_members_twice() {
    for credits in [false, true] {
        for failure in [
            Failure::Rejected,
            Failure::Quiescent,
            Failure::Terminal,
            Failure::Panic,
        ] {
            for failed in [0, 1] {
                let mut fixture = Fixture::new(credits, 0, true);
                fixture.context.release_allocation(fixture.ids[2]).unwrap();
                let (pointer, drops) = fixture.fail_release(failed, failure);
                let result = catch_unwind(AssertUnwindSafe(|| {
                    let mut report = fixture.context.cleanup();
                    assert!(!report.is_complete());
                    assert_eq!(report.failures.len(), 1);
                    let error = report.failures.pop().unwrap();
                    assert_eq!(
                        error.resource,
                        RuntimeCleanupResourceV1::Allocation(fixture.ids[failed])
                    );
                    Err(match error.failure {
                        RuntimeBackendFailureV1::Rejected(error) => {
                            RuntimeErrorV1::BackendRejected(error)
                        }
                        RuntimeBackendFailureV1::Quiescent(error) => {
                            RuntimeErrorV1::BackendQuiescent(error)
                        }
                        RuntimeBackendFailureV1::Terminal(error) => {
                            RuntimeErrorV1::BackendTerminal(error)
                        }
                    })
                }));
                assert_diagnostic(result, failure, pointer, &drops);
                let terminal = matches!(failure, Failure::Terminal | Failure::Panic);
                fixture.assert_unknown(if terminal { 1 + failed } else { 2 });
                fixture.assert_charge(if terminal { 4 } else { 3 }, if terminal { 3 } else { 0 });
                let calls = fixture.context.backend.release_calls;
                if terminal {
                    assert!(!fixture.context.cleanup().is_complete());
                    assert_eq!(fixture.context.backend.release_calls, calls);
                } else {
                    assert!(!fixture.context.allocations.contains_key(&fixture.neighbor));
                    assert!(fixture.context.cleanup().is_complete());
                    assert_eq!(fixture.context.backend.release_calls, calls + 1);
                    fixture.assert_charge(0, 0);
                }
                assert_eq!(
                    fixture
                        .context
                        .backend
                        .release_handles
                        .iter()
                        .filter(|&&handle| handle == fixture.records[2].backend_allocation)
                        .count(),
                    1
                );
            }
        }
    }
}

#[test]
fn final_unknown_disposal_preserves_reused_backend_handle_and_new_allocation_credits() {
    let mut fixture = Fixture::new(true, 1, false);
    let old = fixture.records[0];
    fixture.context.release_allocation(fixture.ids[0]).unwrap();
    fixture.context.backend.inner.handle_override =
        Some((MockHandleKind::Allocation, old.backend_allocation));
    let device = fixture.context.devices()[0].id();
    let fresh = fixture
        .context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 16)
        .unwrap();
    let fresh_record = fixture.context.allocations[&fresh];
    fixture
        .context
        .write_allocation(fresh, 0, &[0x5a; 8])
        .unwrap();
    fixture.assert_charge(5, 0);
    assert_ne!(fresh, fixture.ids[0]);
    assert_ne!(fresh_record.journal, old.journal);
    fixture.context.release_allocation(fixture.ids[2]).unwrap();
    fixture.context.release_allocation(fixture.ids[1]).unwrap();
    fixture.assert_charge(2, 0);
    assert_eq!(fixture.context.allocations[&fresh], fresh_record);
    assert!(
        fixture
            .context
            .backend_allocations
            .contains(&old.backend_allocation)
    );
    assert_eq!(
        fixture
            .context
            .versions
            .as_ref()
            .unwrap()
            .journal_for_test()
            .lookup_allocation(fresh_record.journal.unwrap())
            .unwrap()
            .content_lineage,
        1
    );
    let mut bytes = [0; 8];
    fixture
        .context
        .read_allocation(fresh, 0, &mut bytes)
        .unwrap();
    assert_eq!(bytes, [0x5a; 8]);
    assert!(fixture.context.cleanup().is_complete());
}

#[test]
fn unknown_disposal_rejects_missing_or_substituted_custody_before_backend_entry() {
    for mutation in 0..5 {
        let mut fixture = Fixture::new(true, 1, false);
        match mutation {
            0 => fixture
                .context
                .versions
                .as_mut()
                .unwrap()
                .remove_submission_writer_root_for_test_v1(fixture.submission_id),
            1 => {
                fixture
                    .context
                    .submissions
                    .get_mut(&fixture.submission_id)
                    .unwrap()
                    .journal_writer = None
            }
            2 => {
                fixture
                    .context
                    .allocations
                    .get_mut(&fixture.ids[2])
                    .unwrap()
                    .backend_allocation += 1000
            }
            3 => {
                fixture
                    .context
                    .backend_allocations
                    .remove(&fixture.records[2].backend_allocation);
            }
            4 => fixture
                .context
                .allocation_admission
                .quarantine(fixture.ids[2]),
            _ => unreachable!(),
        }
        let memory = fixture.context.backend.inner.memory.clone();
        assert!(matches!(
            fixture.context.release_allocation(fixture.ids[0]),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::InvalidBackendDescription
            ))
        ));
        assert!(fixture.context.is_terminal());
        assert_eq!(fixture.context.backend.release_calls, 0);
        assert_eq!(fixture.context.backend.inner.memory, memory);
        assert!(!fixture.context.cleanup().is_complete());
    }
}

#[test]
fn final_unknown_disposal_bookkeeping_failure_retains_all_success_receipts() {
    let mut fixture = Fixture::new(true, 1, false);
    fixture.context.release_allocation(fixture.ids[2]).unwrap();
    fixture.context.release_allocation(fixture.ids[0]).unwrap();
    fixture
        .context
        .versions
        .as_mut()
        .unwrap()
        .clear_disposal_phase_for_test_v1(fixture.records[2].journal.unwrap());
    let result = catch_unwind(AssertUnwindSafe(|| {
        fixture.context.release_allocation(fixture.ids[1])
    }));
    assert!(result.is_err());
    assert!(fixture.context.is_terminal());
    fixture.assert_unknown(3);
    fixture.assert_charge(4, 3);
    assert!(
        fixture
            .ids
            .iter()
            .all(|id| !fixture.context.allocations.contains_key(id))
    );
    assert!(fixture.records.iter().all(|record| {
        !fixture
            .context
            .backend
            .inner
            .memory
            .contains_key(&record.backend_allocation)
    }));
    let report = fixture.context.cleanup();
    assert!(!report.is_complete());
    assert_eq!(report.writer_journal_records_v1(), 1);
    assert_eq!(report.allocation_journal_records_v1(), 4);
    assert_eq!(fixture.context.backend.release_calls, 3);
}

#[test]
fn post_model_credit_failure_keeps_finalizing_root_visible_and_terminal() {
    for failed_index in 0..3 {
        let mut fixture = Fixture::new(true, 1, false);
        fixture.context.release_allocation(fixture.ids[2]).unwrap();
        fixture.context.release_allocation(fixture.ids[0]).unwrap();
        fixture
            .context
            .allocation_admission
            .reject_disposal_for_test_v1(fixture.ids[failed_index]);
        let result = catch_unwind(AssertUnwindSafe(|| {
            fixture.context.release_allocation(fixture.ids[1])
        }));
        assert!(result.is_err());
        assert!(fixture.context.is_terminal());
        let versions = fixture.context.versions.as_ref().unwrap();
        assert!(
            versions
                .journal_for_test()
                .lookup_writer(fixture.writer)
                .is_err()
        );
        assert_eq!(
            versions.submission_disposal_progress_for_test_v1(fixture.submission_id),
            Some((3, true))
        );
        assert_eq!(fixture.context.version_journal_writer_records_v1(), Some(1));
        assert_eq!(
            fixture
                .context
                .version_journal_usage_v1()
                .unwrap()
                .allocation_records,
            1
        );
        fixture.assert_charge(4 - failed_index as u64, 3 - failed_index);
        assert!(
            fixture.context.submissions[&fixture.submission_id]
                .journal_writer
                .is_some()
        );
        let report = fixture.context.cleanup();
        assert!(!report.is_complete());
        assert_eq!(report.writer_journal_records_v1(), 1);
        assert_eq!(fixture.context.backend.release_calls, 3);
    }
}

#[test]
fn unknown_disposal_keeps_independent_device_credit_accounts_separate() {
    let mut fixture = Fixture::new(true, 1, true);
    let device = fixture.context.devices()[1].id();
    fixture
        .context
        .configure_allocation_admission_v1(device, 64, 1)
        .unwrap();
    let allocation = fixture
        .context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let stream = fixture.context.create_stream(device).unwrap();
    let module = fixture
        .context
        .load_module(device, b"other-device")
        .unwrap();
    let kernel = fixture
        .context
        .resolve_kernel::<MixedArguments>(module, "other")
        .unwrap();
    let mut submission = fixture
        .context
        .launch(
            stream,
            &kernel,
            &MixedArguments(vec![region(allocation, RuntimeAccessV1::Write, 0)]),
            geometry(),
            &[],
        )
        .unwrap();
    fixture.context.backend.inner.wait_observation = Some(BackendPollV1::Failed { code: -7 });
    assert_eq!(
        fixture
            .context
            .wait(&mut submission, Duration::ZERO)
            .unwrap(),
        RuntimePollV1::Failed { code: -7 }
    );
    fixture.context.release_allocation(fixture.ids[2]).unwrap();
    fixture.context.release_allocation(allocation).unwrap();
    fixture.assert_unknown(1);
    fixture.assert_charge(4, 0);
    assert_eq!(
        fixture
            .context
            .allocation_admission_usage_v1(device)
            .unwrap()
            .unwrap()
            .used,
        RuntimeResourceVectorV1::ZERO
    );
    fixture.context.release_allocation(fixture.ids[0]).unwrap();
    fixture.context.release_allocation(fixture.ids[1]).unwrap();
    fixture.assert_charge(1, 0);
    assert!(fixture.context.cleanup().is_complete());
}
