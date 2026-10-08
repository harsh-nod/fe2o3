//! Metadata-only route controls, not native completion or DATA-release evidence.

use super::*;

pub(super) fn single_buffer_shells_with_roster() -> (
    KfdRuntimeBackendV1,
    GeneratedShellPlanV1,
    GeneratedHostRosterV1,
) {
    let mut backend = KfdRuntimeBackendV1::mock();
    let device = backend.description.backend_device;
    let stream = backend.create_stream_v1(device).unwrap();
    let hsaco = crate::synthetic_cov6::preparation_module();
    let mut explicit = vec![0; 16];
    explicit[8..].copy_from_slice(&4u64.to_le_bytes());
    let projection = crate::prepare_gfx942_runtime_dispatch_v1(
        &hsaco,
        "vecadd",
        crate::Gfx942RuntimeDispatchInputsV1::new(
            explicit,
            vec![
                crate::Gfx942RuntimeDispatchBufferV1::new(
                    vec![0; 16],
                    crate::Gfx942RuntimeBufferAccessV1::ReadWrite,
                )
                .unwrap(),
            ],
            vec![fe2o3_kfd::Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4)],
            fe2o3_aql::AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            0,
            4321,
        ),
    )
    .unwrap()
    .into_persistent_projection_v1(&hsaco)
    .unwrap();
    let authority = crate::authorized_execution::tests::source_authority(&projection);
    let roster = GeneratedHostRosterV1::from_projection(&projection).unwrap();
    let (binding, logical) =
        crate::RuntimeContextV1::generated_route_ids_for_test_v1(device, stream, roster.count);
    let plan = backend
        .prepare_generated_shells_v1(binding, &roster, &logical)
        .unwrap();
    assert_eq!(
        plan.profile,
        crate::generated_source::GeneratedProfileV1::Singleton
    );
    assert_eq!(plan.count, 1);
    assert_eq!(roster.count, 1);
    assert_eq!(plan.members[0].unwrap().description.byte_len, 16);
    assert!(plan.members[1..].iter().all(Option::is_none));
    assert!(roster.buffers[1..].iter().all(Option::is_none));
    let bound = backend
        .bind_generated_shell_requests_v1(plan, core::array::from_fn(|_| None))
        .unwrap();
    let mut storage = projection.into_generated_storage_v1();
    let mut source =
        crate::RuntimeGfx942GeneratedSourceMutV1::new(&mut storage, &hsaco, &authority);
    backend.commit_generated_shells_v1(bound, &mut source, &roster);
    (backend, plan, roster)
}

fn fixture() -> (KfdRuntimeBackendV1, GeneratedShellPlanV1, u64, u64) {
    let (backend, plan, roster) = single_buffer_shells_with_roster();
    fixture_from_shells(backend, plan, roster)
}

fn fixture_from_shells(
    mut backend: KfdRuntimeBackendV1,
    plan: GeneratedShellPlanV1,
    roster: GeneratedHostRosterV1,
) -> (KfdRuntimeBackendV1, GeneratedShellPlanV1, u64, u64) {
    let producer = backend.install_generated_receipt_metadata_for_test_v1(&plan, &roster);
    let stream = backend
        .create_stream_v1(plan.binding.backend_device)
        .unwrap();
    let destination = backend.next_id().unwrap();
    let submission = backend.next_id().unwrap();
    let native = backend
        .generated_shells
        .get_mut(&plan.key)
        .unwrap()
        .native
        .as_mut()
        .unwrap();
    native.submission.as_mut().unwrap().receipt = ReceiptV1::Recycled.into();
    native.phase = PhaseV1::Copying;
    native.copy = Some(BackendCopyV1 {
        submission,
        destination,
        root: NativeCopyV1::empty(0, 16).unwrap(),
        shell: None,
        released: None,
        source_released: None,
    });
    backend.submissions.insert(
        submission,
        SubmissionRecordV1 {
            origin: SubmissionOriginV1::GeneratedDataCopy {
                shell: plan.key,
                producer,
                destination,
            },
            stream,
            status: BackendPollV1::Succeeded,
            dependency_depth: 0,
            profile_dispatch_published: false,
        },
    );
    backend.stream_submission_tails.insert(stream, submission);
    (backend, plan, producer, submission)
}

fn script_released_metadata(backend: &mut KfdRuntimeBackendV1, plan: &GeneratedShellPlanV1) {
    let native = backend
        .generated_shells
        .get_mut(&plan.key)
        .unwrap()
        .native
        .as_mut()
        .unwrap();
    let lineage = ProducerLineageV1::for_release_index_metadata_test(
        plan,
        native.submission.as_ref().unwrap(),
    )
    .unwrap();
    // No lower owner, native call or physical receipt exists in this fixture.
    native.copy.as_mut().unwrap().source_released = Some(ReleasedSourceV1 { lineage });
}

#[test]
fn ordinary_release_ignores_large_unrelated_generated_roster() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let plans: Vec<_> = (0..1024)
        .map(|_| backend.park_generated_adopted_metadata_for_test_v1())
        .collect();
    let stream = backend
        .create_stream_v1(backend.description.backend_device)
        .unwrap();
    let submission = backend.next_id().unwrap();
    backend.submissions.insert(
        submission,
        SubmissionRecordV1 {
            origin: SubmissionOriginV1::Ordinary,
            stream,
            status: BackendPollV1::Succeeded,
            dependency_depth: 0,
            profile_dispatch_published: false,
        },
    );
    backend.stream_submission_tails.insert(stream, submission);
    assert_eq!(backend.generated_shells.len(), 1024);
    assert!(
        !backend
            .generated_copy_release_blocked_v1(submission)
            .unwrap()
    );
    backend.release_submission_v1(submission).unwrap();
    assert!(!backend.submissions.contains_key(&submission));
    assert!(!backend.stream_submission_tails.contains_key(&stream));
    assert_eq!(backend.generated_shells.len(), 1024);
    assert!(!backend.terminal);
    for plan in plans {
        let native = backend.generated_shells[&plan.key].native.as_ref().unwrap();
        assert_eq!(native.phase, PhaseV1::Adopted);
        assert!(native.copy.is_none());
        backend.unpark_generated_adopted_metadata_for_test_v1(plan);
    }
    backend.destroy_stream_v1(stream).unwrap();
}

#[test]
fn exact_scripted_release_removes_only_copy_index_at_existing_tail() {
    let (mut backend, plan, producer, submission) = fixture();
    let stream = backend.submissions[&submission].stream;
    script_released_metadata(&mut backend, &plan);
    assert!(
        !backend
            .generated_copy_release_blocked_v1(submission)
            .unwrap()
    );
    backend.release_submission_v1(submission).unwrap();
    assert!(!backend.submissions.contains_key(&submission));
    assert!(!backend.stream_submission_tails.contains_key(&stream));
    assert_eq!(
        backend.generated_submissions.get(&producer),
        Some(&plan.key)
    );
    let native = backend.generated_shells[&plan.key].native.as_ref().unwrap();
    assert_eq!(native.phase, PhaseV1::Copying);
    assert!(native.copy.as_ref().unwrap().source_released.is_some());
    assert!(matches!(backend.release_submission_v1(submission),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::UnknownHandle));
    assert!(!backend.terminal);
    // Scripted metadata is never promoted to actual common/native cleanup.
    core::mem::forget(backend);
}

#[test]
fn missing_copy_index_refuses_without_discarding_original_root() {
    let (mut backend, plan, producer, submission) = fixture();
    let record = backend.submissions.remove(&submission).unwrap();
    assert!(matches!(backend.release_submission_v1(submission),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::UnknownHandle));
    assert_eq!(
        backend.stream_submission_tails.get(&record.stream),
        Some(&submission)
    );
    assert_eq!(
        backend.generated_submissions.get(&producer),
        Some(&plan.key)
    );
    let native = backend.generated_shells[&plan.key].native.as_ref().unwrap();
    assert_eq!(native.copy.as_ref().unwrap().submission, submission);
    assert!(native.copy.as_ref().unwrap().source_released.is_none());
    assert!(backend.submissions.insert(submission, record).is_none());
    core::mem::forget(backend);
}

#[derive(Clone, Copy, Debug)]
enum Mismatch {
    ShellKey,
    ProducerKey,
    DestinationKey,
    MissingProducer,
    ProducerRoute,
    StaleCopy,
    OriginalProducer,
    OriginalProfile,
    OriginalDataCount,
    OriginalSource,
    OriginalReceipt,
    CopyDestination,
    CopyStream,
    ShellPlan,
    NativePhase,
}

#[test]
fn keyed_copy_release_retains_originals_on_route_owner_and_receipt_mismatch() {
    for mismatch in [
        Mismatch::ShellKey,
        Mismatch::ProducerKey,
        Mismatch::DestinationKey,
        Mismatch::MissingProducer,
        Mismatch::ProducerRoute,
        Mismatch::StaleCopy,
        Mismatch::OriginalProducer,
        Mismatch::OriginalProfile,
        Mismatch::OriginalDataCount,
        Mismatch::OriginalSource,
        Mismatch::OriginalReceipt,
        Mismatch::CopyDestination,
        Mismatch::CopyStream,
        Mismatch::ShellPlan,
        Mismatch::NativePhase,
    ] {
        let (mut backend, plan, producer, submission) =
            if matches!(mismatch, Mismatch::OriginalDataCount) {
                let (backend, plan, roster) = super::super::super::tests::shells_with_roster();
                assert_eq!(
                    plan.profile,
                    crate::generated_source::GeneratedProfileV1::Singleton
                );
                assert_eq!(plan.count, 3);
                assert_eq!(roster.count, 3);
                for (index, bytes) in [16, 20, 24].into_iter().enumerate() {
                    assert_eq!(plan.members[index].unwrap().description.byte_len, bytes);
                }
                fixture_from_shells(backend, plan, roster)
            } else {
                fixture()
            };
        script_released_metadata(&mut backend, &plan);
        let missing = backend.next_handle + 100;
        match mismatch {
            Mismatch::ShellKey | Mismatch::ProducerKey | Mismatch::DestinationKey => {
                let SubmissionOriginV1::GeneratedDataCopy {
                    shell,
                    producer,
                    destination,
                } = &mut backend.submissions.get_mut(&submission).unwrap().origin
                else {
                    panic!()
                };
                match mismatch {
                    Mismatch::ShellKey => *shell = missing,
                    Mismatch::ProducerKey => *producer = missing,
                    Mismatch::DestinationKey => *destination = missing,
                    _ => unreachable!(),
                }
            }
            Mismatch::MissingProducer => {
                backend.generated_submissions.remove(&producer);
            }
            Mismatch::OriginalDataCount => {}
            Mismatch::ProducerRoute => {
                backend.generated_submissions.insert(producer, missing);
            }
            Mismatch::CopyStream => {
                backend.submissions.get_mut(&submission).unwrap().stream =
                    plan.binding.backend_stream;
            }
            Mismatch::ShellPlan => {
                backend
                    .generated_shells
                    .get_mut(&plan.key)
                    .unwrap()
                    .plan
                    .key = missing;
            }
            Mismatch::OriginalSource => {
                let (other, _, other_roster) = super::super::super::tests::shells_with_roster();
                backend
                    .generated_shells
                    .get_mut(&plan.key)
                    .unwrap()
                    .native
                    .as_mut()
                    .unwrap()
                    .submission
                    .as_mut()
                    .unwrap()
                    .roster
                    .source_identity = other_roster.source_identity;
                drop(other);
            }
            _ => {
                let native = backend
                    .generated_shells
                    .get_mut(&plan.key)
                    .unwrap()
                    .native
                    .as_mut()
                    .unwrap();
                match mismatch {
                    Mismatch::StaleCopy => native.copy.as_mut().unwrap().submission = missing,
                    Mismatch::OriginalProducer => native.submission.as_mut().unwrap().id = missing,
                    Mismatch::OriginalProfile => {
                        native.submission.as_mut().unwrap().receipt =
                            typed_receipt::NativeReceiptV1::Cohort3(ReceiptV1::Recycled);
                    }
                    Mismatch::OriginalReceipt => {
                        native.submission.as_mut().unwrap().receipt = ReceiptV1::Ready.into();
                    }
                    Mismatch::CopyDestination => {
                        native.copy.as_mut().unwrap().destination = missing
                    }
                    Mismatch::NativePhase => native.phase = PhaseV1::CopyRetired,
                    _ => unreachable!(),
                }
            }
        }
        assert!(
            matches!(
                backend.release_submission_v1(submission),
                Err(RuntimeBackendFailureV1::Terminal(_))
            ),
            "{mismatch:?}"
        );
        assert!(backend.terminal, "{mismatch:?}");
        assert!(
            backend.submissions.contains_key(&submission),
            "{mismatch:?}"
        );
        let native = backend.generated_shells[&plan.key].native.as_ref().unwrap();
        assert!(
            native.copy.as_ref().unwrap().source_released.is_some(),
            "{mismatch:?}"
        );
        assert!(native.submission.is_some(), "{mismatch:?}");
        if matches!(mismatch, Mismatch::OriginalDataCount) {
            assert_eq!(
                backend.generated_submissions.get(&producer),
                Some(&plan.key)
            );
            assert_eq!(native.submission.as_ref().unwrap().id, producer);
            assert_eq!(native.submission.as_ref().unwrap().roster.count, 3);
            assert_eq!(native.copy.as_ref().unwrap().submission, submission);
            assert_eq!(
                backend
                    .stream_submission_tails
                    .get(&backend.submissions[&submission].stream),
                Some(&submission)
            );
        }
        core::mem::forget(backend);
    }
}

#[test]
fn stale_submission_cannot_select_or_release_current_copy() {
    let (mut backend, plan, _, submission) = fixture();
    let stale = backend.next_id().unwrap();
    assert!(matches!(backend.release_submission_v1(stale),
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::UnknownHandle));
    assert!(backend.submissions.contains_key(&submission));
    assert_eq!(
        backend.generated_shells[&plan.key]
            .native
            .as_ref()
            .unwrap()
            .copy
            .as_ref()
            .unwrap()
            .submission,
        submission
    );
    core::mem::forget(backend);
}

fn admission_source_is_bound(source: &str) -> bool {
    let Some((admission, _)) =
        source.split_once("    pub(crate) fn advance_retained_generated_copy_v1(")
    else {
        return false;
    };
    let anchors = [
        ".try_reserve(1)",
        "let custody = self.reserve_allocation_custody_v1",
        "let submission = self.next_id()?",
        "self.submissions.contains_key(&submission)",
        "native.copy = Some(BackendCopyV1",
        "copy.root.install(",
        "self.submissions.entry(submission)",
        "origin: SubmissionOriginV1::GeneratedDataCopy {",
        "shell: plan.key,",
        "producer,",
        "destination,",
        "self.stream_submission_tails.entry(stream)",
    ];
    let mut remaining = admission;
    for anchor in anchors {
        let Some((_, tail)) = remaining.split_once(anchor) else {
            return false;
        };
        remaining = tail;
    }
    admission
        .matches("origin: SubmissionOriginV1::GeneratedDataCopy {")
        .count()
        == 1
        && admission.matches("Entry::Vacant(entry)").count() == 2
        && !admission.contains("origin: SubmissionOriginV1::Ordinary")
}

#[test]
fn generated_copy_constructor_and_keyed_release_source_correspondence() {
    let source = include_str!("backend.rs");
    assert!(admission_source_is_bound(source));
    for (original, replacement) in [
        (
            "origin: SubmissionOriginV1::GeneratedDataCopy {",
            "origin: SubmissionOriginV1::Ordinary {",
        ),
        ("shell: plan.key,", "shell: producer,"),
        (
            "self.submissions.contains_key(&submission)",
            "self.submissions.contains_key(&producer)",
        ),
        ("Entry::Vacant(entry)", "Entry::Occupied(entry)"),
    ] {
        assert!(!admission_source_is_bound(&source.replacen(
            original,
            replacement,
            1
        )));
    }
    let release = source
        .split_once("    pub(in crate::kfd_backend) fn generated_copy_release_blocked_v1(")
        .unwrap()
        .1
        .split_once("\n#[cfg(test)]")
        .unwrap()
        .0;
    let release: String = release.split_whitespace().collect();
    for forbidden in [
        ".values(",
        ".iter(",
        ".iter_mut(",
        ".into_iter(",
        "validate_generated_shell_records_v1",
    ] {
        assert!(!release.contains(forbidden), "{forbidden}");
    }
    for required in [
        "self.submissions.get(&submission)",
        "self.generated_submissions.get(&producer)",
        "self.generated_shells.get(&shell)",
        "copy.submission != submission",
        "copy.destination != destination",
        "physically_released_count()",
        "lineage.matches(plan, original)",
        "self.poison_terminal_v1()",
    ] {
        let required: String = required.split_whitespace().collect();
        assert!(release.contains(&required), "{required}");
    }
}
