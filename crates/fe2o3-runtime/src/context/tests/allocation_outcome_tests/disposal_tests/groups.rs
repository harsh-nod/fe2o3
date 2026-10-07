use super::*;
use fe2o3_runtime_model::ContextWriterDisposalEvidenceV1;

struct Multi {
    f: Fixture,
    extra: RuntimeAllocationIdV1,
    extra_record: AllocationRecordV1,
    submission: Option<RuntimeSubmissionV1<MixedArguments>>,
    writer: ContextWriterReferenceV1,
    id: RuntimeSubmissionIdV1,
}

impl Multi {
    fn new(metadata: bool, pending: bool) -> Self {
        let mut f = Fixture::new(true, 1, !metadata);
        let device = f.context.devices()[0].id();
        let extra = f
            .context
            .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
            .unwrap();
        let stream = f.context.create_stream(device).unwrap();
        let module = f.context.load_module(device, b"second").unwrap();
        let kernel = f
            .context
            .resolve_kernel::<MixedArguments>(module, "second")
            .unwrap();
        let mut submission = f
            .context
            .launch(
                stream,
                &kernel,
                &MixedArguments(vec![region(extra, RuntimeAccessV1::Write, 0)]),
                geometry(),
                &[],
            )
            .unwrap();
        let id = submission.id;
        if !pending {
            f.context.wait(&mut submission, Duration::ZERO).unwrap();
        }
        let submission = if metadata || pending {
            Some(submission)
        } else {
            f.context.release_submission(submission).unwrap();
            None
        };
        let extra_record = f.context.allocations[&extra];
        let writer = f
            .context
            .versions
            .as_ref()
            .unwrap()
            .journal_for_test()
            .lookup_allocation(extra_record.journal.unwrap())
            .unwrap()
            .pending_writer
            .unwrap();
        Self {
            f,
            extra,
            extra_record,
            submission,
            writer,
            id,
        }
    }

    fn freeze(&mut self) -> ContextWriterReferenceV1 {
        self.f
            .context
            .freeze_disposal_group_v1(&[self.writer, self.f.writer])
            .unwrap()
    }

    fn ids(&self) -> [RuntimeAllocationIdV1; 4] {
        [self.f.ids[0], self.f.ids[1], self.f.ids[2], self.extra]
    }
}

#[test]
fn multi_root_release_permutations_preserve_per_writer_progress_and_unique_credits() {
    for metadata in [false, true] {
        for original in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            for position in 0..4 {
                let mut f = Multi::new(metadata, false);
                let key = f.freeze();
                assert_eq!(key, f.f.writer);
                let ids = f.ids();
                let mut order = original.to_vec();
                order.insert(position, 3);
                let mut first_count = 0;
                let mut second_count = 0;
                for (step, index) in order.into_iter().enumerate() {
                    f.f.context.release_allocation(ids[index]).unwrap();
                    if index == 3 {
                        second_count += 1;
                    } else {
                        first_count += 1;
                    }
                    assert!(f.f.context.release_allocation(ids[index]).is_err());
                    assert_eq!(f.f.context.backend.release_calls, step + 1);
                    let versions = f.f.context.versions.as_ref().unwrap();
                    if step < 3 {
                        assert_eq!(
                            versions.disposal_group_progress_for_test_v1(key),
                            Some((step + 1, false, 0))
                        );
                        assert_eq!(
                            versions.submission_disposal_progress_for_test_v1(f.f.submission_id),
                            Some((first_count, false))
                        );
                        assert_eq!(
                            versions.submission_disposal_progress_for_test_v1(f.id),
                            Some((second_count, false))
                        );
                        assert_eq!(f.f.context.version_journal_writer_records_v1(), Some(2));
                        f.f.assert_charge(5, 0);
                    } else {
                        assert_eq!(versions.disposal_group_counts_for_test_v1(), (0, 0));
                        assert_eq!(f.f.context.version_journal_writer_records_v1(), Some(0));
                        f.f.assert_charge(1, 0);
                    }
                }
                assert!(f.f.context.cleanup().is_complete());
            }
        }
    }
}

#[test]
fn pending_co_owner_refuses_freeze_without_mutation_then_can_complete() {
    let mut f = Multi::new(true, true);
    assert_eq!(
        f.f.context
            .freeze_disposal_group_v1(&[f.f.writer, f.writer]),
        Err(RuntimeValidationErrorV1::ContextReserved)
    );
    assert!(!f.f.context.is_terminal());
    assert_eq!(
        f.f.context
            .versions
            .as_ref()
            .unwrap()
            .disposal_group_counts_for_test_v1(),
        (0, 0)
    );
    assert_eq!(f.f.context.backend.release_calls, 0);
    f.f.context
        .wait(f.submission.as_mut().unwrap(), Duration::ZERO)
        .unwrap();
    let _ = f.freeze();
    assert!(f.f.context.cleanup().is_complete());
}

#[test]
fn failed_native_release_retries_only_unreleased_group_receipts() {
    for failure in [
        Failure::Rejected,
        Failure::Quiescent,
        Failure::Terminal,
        Failure::Panic,
    ] {
        let mut f = Multi::new(false, false);
        let key = f.freeze();
        f.f.context.release_allocation(f.extra).unwrap();
        let (pointer, drops) = f.f.fail_release(1, failure);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            f.f.context.release_allocation(f.f.ids[1])
        }));
        assert_diagnostic(outcome, failure, pointer, &drops);
        assert_eq!(
            f.f.context
                .versions
                .as_ref()
                .unwrap()
                .disposal_group_progress_for_test_v1(key),
            Some((1, false, 0))
        );
        if matches!(failure, Failure::Rejected | Failure::Quiescent) {
            f.f.context.backend.release_failure = Failure::None;
            for id in f.f.ids {
                f.f.context.release_allocation(id).unwrap();
            }
            f.f.assert_charge(1, 0);
            assert_eq!(
                f.f.context
                    .backend
                    .release_handles
                    .iter()
                    .filter(|&&handle| handle == f.extra_record.backend_allocation)
                    .count(),
                1
            );
            assert!(f.f.context.cleanup().is_complete());
        } else {
            assert!(f.f.context.is_terminal());
            let calls = f.f.context.backend.release_calls;
            assert!(!f.f.context.cleanup().is_complete());
            assert_eq!(f.f.context.backend.release_calls, calls);
            assert_eq!(f.f.context.version_journal_writer_records_v1(), Some(2));
        }
    }
}

#[test]
fn corrupt_group_index_refuses_backend_release() {
    let mut f = Multi::new(false, false);
    let _ = f.freeze();
    f.f.context
        .versions
        .as_mut()
        .unwrap()
        .corrupt_disposal_group_index_for_test_v1(f.extra);
    assert!(f.f.context.release_allocation(f.extra).is_err());
    assert!(f.f.context.is_terminal());
    assert_eq!(f.f.context.backend.release_calls, 0);
    assert!(!f.f.context.cleanup().is_complete());
}

#[test]
fn shared_group_finalization_preserves_reused_native_handle_and_fresh_credit() {
    let mut f = Multi::new(false, false);
    let _ = f.freeze();
    f.f.context.release_allocation(f.extra).unwrap();
    f.f.context.backend.inner.handle_override = Some((
        MockHandleKind::Allocation,
        f.extra_record.backend_allocation,
    ));
    let device = f.f.context.devices()[0].id();
    let fresh =
        f.f.context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 16)
            .unwrap();
    let record = f.f.context.allocations[&fresh];
    f.f.assert_charge(6, 0);
    for id in f.f.ids {
        f.f.context.release_allocation(id).unwrap();
    }
    f.f.assert_charge(2, 0);
    assert_eq!(f.f.context.allocations[&fresh], record);
    assert_ne!(record.journal, f.extra_record.journal);
    assert!(
        f.f.context
            .backend_allocations
            .contains(&record.backend_allocation)
    );
    assert!(
        f.f.context
            .backend
            .inner
            .memory
            .contains_key(&record.backend_allocation)
    );
    assert_eq!(
        f.f.context
            .versions
            .as_ref()
            .unwrap()
            .disposal_group_counts_for_test_v1(),
        (0, 0)
    );
    assert!(f.f.context.cleanup().is_complete());
}

#[test]
fn shared_group_credit_prefix_failure_keeps_both_roots_and_all_native_receipts() {
    for prefix in 0..4 {
        let mut f = Multi::new(false, false);
        let key = f.freeze();
        let ids = f.ids();
        for &id in &ids[..3] {
            f.f.context.release_allocation(id).unwrap();
        }
        f.f.context
            .allocation_admission
            .reject_disposal_for_test_v1(ids[prefix]);
        assert!(catch_unwind(AssertUnwindSafe(|| f.f.context.release_allocation(ids[3]))).is_err());
        assert!(f.f.context.is_terminal());
        assert_eq!(
            f.f.context
                .versions
                .as_ref()
                .unwrap()
                .disposal_group_progress_for_test_v1(key),
            Some((4, true, prefix))
        );
        assert_eq!(f.f.context.version_journal_writer_records_v1(), Some(2));
        f.f.assert_charge(5 - prefix as u64, 5 - prefix);
        assert!(!f.f.context.cleanup().is_complete());
        assert_eq!(f.f.context.backend.release_calls, 4);
    }
}

#[test]
fn missing_model_writer_prefix_remains_visible_in_context_custody() {
    let mut f = Multi::new(false, false);
    let key = f.freeze();
    f.f.context.release_allocation(f.extra).unwrap();
    let member =
        f.f.context
            .versions
            .as_ref()
            .unwrap()
            .submission_disposal_member_for_test_v1(f.id, 0)
            .2;
    // Model-prefix fault injection only, not an independent native receipt.
    f.f.context
        .versions
        .as_mut()
        .unwrap()
        .read_leases_for_test_v1()
        .dispose_unknown(
            f.writer,
            &ContextWriterDisposalEvidenceV1 {
                writer: f.writer,
                allocations: &[member],
            },
        )
        .unwrap();
    assert_eq!(f.f.context.version_journal_writer_records_v1(), Some(2));
    f.f.context.release_allocation(f.f.ids[0]).unwrap();
    f.f.context.release_allocation(f.f.ids[1]).unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| f
            .f
            .context
            .release_allocation(f.f.ids[2])))
        .is_err()
    );
    assert!(f.f.context.is_terminal());
    assert_eq!(
        f.f.context
            .versions
            .as_ref()
            .unwrap()
            .disposal_group_progress_for_test_v1(key),
        Some((4, false, 0))
    );
    assert_eq!(f.f.context.version_journal_writer_records_v1(), Some(2));
    assert!(!f.f.context.cleanup().is_complete());
    assert_eq!(f.f.context.backend.release_calls, 4);
}
