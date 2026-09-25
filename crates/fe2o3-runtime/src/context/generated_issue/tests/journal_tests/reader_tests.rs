use super::*;
use fe2o3_runtime_model::{
    ContextAllocationReadV1, ContextReadLeaseReferenceV1, ContextReadQuiescenceEvidenceV1,
    ContextWriterKeyV1, ContextWriterKindV1,
};

pub(super) fn add_foreign_reader(
    context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
    plan: &GeneratedShellPlanV1,
    ordinal: usize,
) -> ContextReadLeaseReferenceV1 {
    let member = plan.members[ordinal].unwrap();
    let allocation = context.allocations[&member.logical].journal.unwrap();
    let state = states(context, plan)[ordinal];
    let consumer = ContextWriterKeyV1 {
        context_generation: context.context_generation,
        local: context.next_id().unwrap(),
        kind: ContextWriterKindV1::Submission,
    };
    let mut output = [None];
    // Inert model premise: generated shell IDs do not escape the public API.
    context
        .versions
        .as_mut()
        .unwrap()
        .read_leases_for_test_v1()
        .acquire_reads(
            consumer,
            &[ContextAllocationReadV1 {
                allocation,
                device: state.device,
                byte_extent: state.byte_extent,
                byte_offset: 0,
                byte_len: state.byte_extent,
                attempt_epoch: state.attempt_epoch,
                content_lineage: state.content_lineage,
            }],
            &mut output,
        )
        .unwrap();
    output[0].unwrap()
}

fn references(
    context: &RuntimeContextV1<KfdRuntimeBackendV1>,
    id: RuntimeSubmissionIdV1,
) -> Vec<ContextReadLeaseReferenceV1> {
    context
        .versions
        .as_ref()
        .unwrap()
        .submission_reader_identity_for_test_v1(id)
        .1
        .to_vec()
}

#[test]
fn generated_inputs_bind_exact_domain_records_and_whole_extents() {
    for accesses in access_rosters() {
        let mut context = context_with_journal(3);
        let (hold, plan, roster, _) = install_with_access(&mut context, false, accesses);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        let attempt = &context.generated_issues[&hold.stream()];
        let id = attempt.id;
        let marker = attempt.expected_reader;
        let readers = accesses
            .iter()
            .filter(|&&access| access == ReadOnly)
            .count();
        assert_eq!(context.version_journal_read_records_v1(), Some(readers));
        assert_eq!(marker.is_some(), readers != 0);
        if readers != 0 {
            let versions = context.versions.as_ref().unwrap();
            let (domain, refs) = versions.submission_reader_identity_for_test_v1(id);
            assert_eq!(domain, generated_writer_domain_v1(&plan));
            assert_eq!(marker.unwrap().count, readers);
            assert_eq!(marker.unwrap().first, refs[0]);
            let sources = versions.submission_reader_sources_for_test_v1(id);
            assert_eq!(sources.len(), readers);
            for (source, reference) in sources.iter().zip(refs) {
                let member = plan.members[..plan.count]
                    .iter()
                    .flatten()
                    .find(|member| member.logical == source.region.allocation)
                    .unwrap();
                assert_eq!(source.record, context.allocations[&member.logical]);
                assert_eq!(source.record.backend_allocation, member.backend);
                assert_eq!(source.region.byte_offset, 0);
                assert_eq!(source.region.byte_len, member.description.byte_len);
                assert_eq!(reference.consumer.context_generation, id.context_generation);
                assert_eq!(reference.consumer.local, id.local);
                assert_eq!(reference.consumer.kind, ContextWriterKindV1::Submission);
            }
        }
        context.install_generated_submission_v1(&hold, 900).unwrap();
        assert_eq!(context.submissions[&id].journal_read, marker);
        assert!(context.generated_issue_token_v1(&hold, &plan).is_ok());
        assert_eq!(
            context.generated_issue_exclusive_readers_v1(&plan),
            Ok(true)
        );
        assert_eq!(context.generated_shells_unread_v1(&plan), Ok(readers == 0));
    }
}

#[test]
fn generated_reader_capacity_rejects_whole_batch_before_writer_begin() {
    for (first_access, second_access, admitted) in [
        ([ReadOnly; 3], [ReadOnly, ReadWrite, WriteOnly], false),
        ([ReadOnly; 3], [ReadWrite; 3], true),
        ([ReadOnly, ReadWrite, WriteOnly], [ReadOnly; 3], false),
    ] {
        let mut context = context_with_journal(3);
        let (first, first_plan, first_roster, _) =
            install_with_access(&mut context, false, first_access);
        context
            .begin_generated_issue_v1(&first, first_plan, &first_roster)
            .unwrap();
        let first_id = context.generated_issues[&first.stream()].id;
        let original = references(&context, first_id);
        let (second, plan, roster, _) = install_with_access(&mut context, false, second_access);
        let before = states(&context, &plan);
        let next = context.next_identity;
        let writers = context.version_journal_writer_records_v1();
        let credits = context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap();
        let result = context.begin_generated_issue_v1(&second, plan, &roster);
        if admitted {
            result.unwrap();
            assert_eq!(context.next_identity, next + 1);
            assert_eq!(context.version_journal_writer_records_v1(), Some(1));
        } else {
            assert!(matches!(
                result,
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::Capacity
                ))
            ));
            assert_eq!(context.next_identity, next);
            assert_eq!(states(&context, &plan), before);
            assert_eq!(context.version_journal_writer_records_v1(), writers);
            assert!(!context.generated_issues.contains_key(&second.stream()));
        }
        assert_eq!(references(&context, first_id), original);
        assert_eq!(
            context.version_journal_read_records_v1(),
            Some(original.len())
        );
        assert_eq!(
            context
                .allocation_admission_usage_v1(plan.binding.device)
                .unwrap(),
            credits
        );
        assert!(context.submissions.is_empty());
        assert!(!context.is_terminal());
    }
}

#[test]
fn generated_busy_input_rejects_before_identity_and_any_reader_acquisition() {
    for ordinal in 0..3 {
        let mut context = context_with_journal(3);
        let (hold, plan, roster, _) = install_with_access(&mut context, false, [ReadOnly; 3]);
        let _writer = context.retain_test_writer_v1(
            &[plan.members[ordinal].unwrap().logical],
            ContextWriterKindV1::Synchronous,
        );
        let before = states(&context, &plan);
        let next = context.next_identity;
        assert!(matches!(
            context.begin_generated_issue_v1(&hold, plan, &roster),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        assert_eq!(context.next_identity, next);
        assert_eq!(states(&context, &plan), before);
        assert_eq!(context.version_journal_read_records_v1(), Some(0));
        assert_eq!(context.version_journal_writer_records_v1(), Some(1));
        assert!(context.generated_issues.is_empty());
        assert!(!context.is_terminal());
    }
}

#[test]
fn generated_read_only_domain_survives_missing_backend_handle_and_ordinary_release() {
    for installed in [false, true] {
        let mut context = context_with_journal(3);
        let (hold, plan, roster, _) = install_with_access(&mut context, false, [ReadOnly; 3]);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        let id = context.generated_issues[&hold.stream()].id;
        let original = references(&context, id);
        if installed {
            context.install_generated_submission_v1(&hold, 900).unwrap();
        }
        let before = states(&context, &plan);
        assert!(context.release_submission_inputs_v1(id).is_err());
        assert_eq!(references(&context, id), original);
        assert_eq!(states(&context, &plan), before);
        assert_eq!(context.version_journal_read_records_v1(), Some(3));
        assert_eq!(context.version_journal_writer_records_v1(), Some(0));
        assert_eq!(context.is_terminal(), !installed);
        assert!(!context.cleanup().is_complete());
        core::mem::forget(context);
    }
}

#[test]
fn generated_reader_release_validates_before_selected_root_effect() {
    use crate::context::versions::completion_faults::{
        CompletionJournalFailureV1 as Failure, CompletionJournalPointV1 as Point,
        CompletionJournalStageV1 as Stage,
    };

    for installed in [false, true] {
        for fault in 0..4 {
            let mut context = context_with_journal(3);
            let (hold, plan, roster, _) = install_with_access(&mut context, false, [ReadOnly; 3]);
            context
                .begin_generated_issue_v1(&hold, plan, &roster)
                .unwrap();
            let id = context.generated_issues[&hold.stream()].id;
            if installed {
                context.install_generated_submission_v1(&hold, 900).unwrap();
            }
            let domain = generated_writer_domain_v1(&plan);
            let before = states(&context, &plan);
            let versions = context.versions.as_mut().unwrap();
            if fault < 3 {
                versions.corrupt_submission_read_reference_for_test_v1(id, fault);
            } else {
                versions.set_submission_reader_domain_for_test_v1(
                    id,
                    SubmissionWriterDomainV1::Ordinary,
                );
            }
            versions.inject_completion_fault_for_test_v1(
                id,
                Stage::Stable,
                Point::BeforeEffect,
                Failure::Error,
            );
            let retained = references(&context, id);
            assert_eq!(
                context.release_generated_submission_readers_v1(id, domain),
                Err(RuntimeValidationErrorV1::InvalidBackendDescription),
            );
            assert!(
                context
                    .versions
                    .as_ref()
                    .unwrap()
                    .completion_fault_pending_for_test_v1()
            );
            assert_eq!(references(&context, id), retained);
            assert_eq!(states(&context, &plan), before);
            assert_eq!(context.version_journal_read_records_v1(), Some(3));
            assert!(context.is_terminal());
            core::mem::forget(context);
        }
    }
}

#[test]
fn generated_readers_survive_all_ordinary_observations() {
    for accesses in [[ReadOnly; 3], [ReadOnly, ReadWrite, WriteOnly]] {
        let mut context = context_with_journal(3);
        let (hold, plan, roster, _) = install_with_access(&mut context, false, accesses);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        context.install_generated_submission_v1(&hold, 900).unwrap();
        let id = context.generated_issues[&hold.stream()].id;
        let original = references(&context, id);
        let before = states(&context, &plan);
        let record = context.submissions[&id];
        let mut token = context
            .generated_issues
            .get_mut(&hold.stream())
            .unwrap()
            .submission
            .take()
            .unwrap();
        for _ in 0..2 {
            for result in [
                context.poll(&mut token),
                context.wait(&mut token, Duration::ZERO),
                context.drain(&mut token, Instant::now() + Duration::from_secs(1)),
            ] {
                assert!(matches!(
                    result,
                    Err(RuntimeErrorV1::Validation(
                        RuntimeValidationErrorV1::ContextReserved
                    ))
                ));
            }
            assert!(matches!(
                context.cancel(&mut token),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::ContextReserved
                ))
            ));
            assert!(context.record_event(&token).is_err());
            assert!(context.release_submission_ref(&token, None).is_err());
            for status in [
                RuntimeCompletionStatusV1::Pending,
                RuntimeCompletionStatusV1::Succeeded,
                RuntimeCompletionStatusV1::QuiescentWithoutResult,
            ] {
                assert_eq!(
                    context.transition_submission_status(id, status),
                    Err(RuntimeValidationErrorV1::ContextReserved)
                );
            }
            assert_eq!(references(&context, id), original);
            assert_eq!(states(&context, &plan), before);
            completion_tests::assert_submission_retained(&context, id, record);
        }
        context
            .generated_issues
            .get_mut(&hold.stream())
            .unwrap()
            .submission = Some(token);
        assert!(!context.is_terminal());
    }
}

#[test]
fn generated_read_only_no_handle_failure_quarantines_inputs_without_writer() {
    for panic in [false, true] {
        let mut context = context_with_journal(3);
        let (hold, plan, roster, _) = install_with_access(&mut context, false, [ReadOnly; 3]);
        let (_, neighbor_plan, _, _) = install_with_access(&mut context, false, [ReadWrite; 3]);
        let neighbor = states(&context, &neighbor_plan);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        let id = context.generated_issues[&hold.stream()].id;
        let original = references(&context, id);
        let before = states(&context, &plan);
        let diagnostic = std::sync::Arc::new(());
        let result = catch_unwind(AssertUnwindSafe(|| {
            if panic {
                std::panic::panic_any(diagnostic.clone());
            }
            Err(RuntimeValidationErrorV1::Capacity.into())
        }));
        let result = catch_unwind(AssertUnwindSafe(|| {
            context.finish_generated_issue_v1(&hold, result)
        }));
        if panic {
            assert!(std::sync::Arc::ptr_eq(
                result.unwrap_err().downcast_ref().unwrap(),
                &diagnostic
            ));
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::Capacity
                ))
            ));
        }
        assert!(context.is_terminal());
        assert_eq!(
            context.generated_issues[&hold.stream()].phase,
            PhaseV1::Unknown
        );
        assert_eq!(references(&context, id), original);
        assert_eq!(states(&context, &plan), before);
        assert_eq!(states(&context, &neighbor_plan), neighbor);
        assert_eq!(context.version_journal_writer_records_v1(), Some(0));
        let usage = context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap()
            .unwrap();
        assert_eq!(usage.quarantined_records, 3);
        assert_eq!(usage.retained_records, 3);
        assert_eq!(
            usage.used,
            crate::RuntimeResourceVectorV1::ZERO
                .with(crate::RuntimeResourceKindV1::RequestedAllocationBytes, 120)
                .with(crate::RuntimeResourceKindV1::AllocationRecords, 6)
        );
        assert!(context.retire_gfx942_issued_v1(&hold).is_err());
        assert_eq!(context.version_journal_read_records_v1(), Some(3));
        core::mem::forget(context);
    }
}

#[test]
fn generated_assumed_completion_or_stop_releases_only_original_read_batch() {
    for stop in [false, true] {
        for accesses in [[ReadOnly; 3], [ReadOnly, ReadWrite, WriteOnly]] {
            let mut context = context_with_journal(6);
            let (hold, plan, roster, _) = install_with_access(&mut context, false, accesses);
            context
                .begin_generated_issue_v1(&hold, plan, &roster)
                .unwrap();
            context.install_generated_submission_v1(&hold, 900).unwrap();
            let id = context.generated_issues[&hold.stream()].id;
            let (neighbor, neighbor_plan, neighbor_roster, _) =
                install_with_access(&mut context, false, [ReadOnly; 3]);
            context
                .begin_generated_issue_v1(&neighbor, neighbor_plan, &neighbor_roster)
                .unwrap();
            context
                .install_generated_submission_v1(&neighbor, 901)
                .unwrap();
            let neighbor_id = context.generated_issues[&neighbor.stream()].id;
            let neighbor_refs = references(&context, neighbor_id);
            let neighbor_states = states(&context, &neighbor_plan);
            let observed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let notifications = observed.clone();
            let token = context
                .generated_issues
                .get_mut(&hold.stream())
                .unwrap()
                .submission
                .take()
                .unwrap();
            context
                .on_completion(&token, move |status| {
                    notifications.lock().unwrap().push(status)
                })
                .unwrap();
            context
                .generated_issues
                .get_mut(&hold.stream())
                .unwrap()
                .submission = Some(token);
            // These Context tails assume native disposal/currentness, not native execution.
            if stop {
                context
                    .settle_stopped_gfx942_context_v1(&hold, id, 900)
                    .unwrap();
                assert_eq!(
                    *observed.lock().unwrap(),
                    [RuntimeCompletionStatusV1::QuiescentWithoutResult]
                );
                context.release_unpublished_hold_v1(&hold).unwrap();
            } else {
                context
                    .generated_issues
                    .get_mut(&hold.stream())
                    .unwrap()
                    .phase = PhaseV1::Unknown;
                let wrong =
                    completion::assume_native_settlement_for_context_test_v1(&context, &neighbor);
                assert!(
                    context
                        .settle_completed_gfx942_context_v1(&hold, wrong)
                        .is_err()
                );
                assert!(observed.lock().unwrap().is_empty());
                let receipt =
                    completion::assume_native_settlement_for_context_test_v1(&context, &hold);
                context
                    .settle_completed_gfx942_context_v1(&hold, receipt)
                    .unwrap();
                assert_eq!(
                    *observed.lock().unwrap(),
                    [RuntimeCompletionStatusV1::Succeeded]
                );
            }
            assert_eq!(context.version_journal_read_records_v1(), Some(3));
            assert_eq!(references(&context, neighbor_id), neighbor_refs);
            assert_eq!(states(&context, &neighbor_plan), neighbor_states);
            assert_eq!(context.allocations.len(), 3);
            assert_eq!(context.backend_allocations.len(), 3);
            assert_eq!(context.version_journal_writer_records_v1(), Some(0));
            assert!(!context.generated_issues.contains_key(&hold.stream()));
            assert!(!context.submissions.contains_key(&id));
            let usage = context
                .allocation_admission_usage_v1(plan.binding.device)
                .unwrap()
                .unwrap();
            assert_eq!(usage.retained_records, 3);
            assert_eq!(usage.quarantined_records, 0);
            assert_eq!(
                usage.used,
                crate::RuntimeResourceVectorV1::ZERO
                    .with(crate::RuntimeResourceKindV1::RequestedAllocationBytes, 60)
                    .with(crate::RuntimeResourceKindV1::AllocationRecords, 3)
            );
            assert!(!context.is_terminal());
        }
    }
}

#[test]
fn generated_marker_root_reference_and_domain_corruption_cannot_release_batch() {
    for fault in 0..10 {
        let mut context = context_with_journal(3);
        let (hold, plan, roster, _) = install_with_access(&mut context, false, [ReadOnly; 3]);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        context.install_generated_submission_v1(&hold, 900).unwrap();
        let id = context.generated_issues[&hold.stream()].id;
        context
            .generated_issues
            .get_mut(&hold.stream())
            .unwrap()
            .phase = PhaseV1::Unknown;
        let receipt = completion::assume_native_settlement_for_context_test_v1(&context, &hold);
        let before = states(&context, &plan);
        let credits = context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap();
        match fault {
            0 => {
                context
                    .generated_issues
                    .get_mut(&hold.stream())
                    .unwrap()
                    .expected_reader = None
            }
            1 => context.submissions.get_mut(&id).unwrap().journal_read = None,
            2 => context
                .versions
                .as_mut()
                .unwrap()
                .remove_submission_readers_for_test_v1(id),
            3..=5 => context
                .versions
                .as_mut()
                .unwrap()
                .corrupt_submission_read_reference_for_test_v1(id, fault - 3),
            _ => {
                let domain = match fault {
                    6 => SubmissionWriterDomainV1::Ordinary,
                    7 => SubmissionWriterDomainV1::Generated {
                        stream: hold.stream(),
                        hold: hold.identity() + 1,
                        shell_key: plan.key,
                    },
                    8 => SubmissionWriterDomainV1::Generated {
                        stream: hold.stream(),
                        hold: hold.identity(),
                        shell_key: plan.key + 1,
                    },
                    _ => SubmissionWriterDomainV1::Generated {
                        stream: context.create_stream(plan.binding.device).unwrap(),
                        hold: hold.identity(),
                        shell_key: plan.key,
                    },
                };
                context
                    .versions
                    .as_mut()
                    .unwrap()
                    .set_submission_reader_domain_for_test_v1(id, domain);
            }
        }
        assert!(
            context
                .settle_completed_gfx942_context_v1(&hold, receipt)
                .is_err()
        );
        assert_eq!(context.version_journal_read_records_v1(), Some(3));
        assert_eq!(states(&context, &plan), before);
        assert_eq!(
            context
                .allocation_admission_usage_v1(plan.binding.device)
                .unwrap(),
            credits
        );
        assert_eq!(
            context.submissions[&id].status,
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(context.allocations.len(), 3);
        assert!(context.backend.validate_generated_shell_disposal_v1(&plan));
        core::mem::forget(context);
    }
}

#[test]
fn generated_foreign_reader_blocks_retirement_preflight_without_releasing_own_inputs() {
    for ordinal in 0..3 {
        let mut context = context_with_journal(4);
        let (hold, plan, roster, _) = install_with_access(&mut context, false, [ReadOnly; 3]);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        context.install_generated_submission_v1(&hold, 900).unwrap();
        let id = context.generated_issues[&hold.stream()].id;
        let original = references(&context, id);
        let foreign = add_foreign_reader(&mut context, &plan, ordinal);
        let before = states(&context, &plan);
        let credits = context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap();
        assert_eq!(
            context.generated_issue_exclusive_readers_v1(&plan),
            Ok(false)
        );
        assert!(matches!(
            context.settle_stopped_gfx942_context_v1(&hold, id, 900),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        assert_eq!(context.version_journal_read_records_v1(), Some(4));
        assert_eq!(references(&context, id), original);
        assert_eq!(states(&context, &plan), before);
        assert_eq!(
            context
                .allocation_admission_usage_v1(plan.binding.device)
                .unwrap(),
            credits
        );
        assert!(context.backend.validate_generated_shell_disposal_v1(&plan));
        context
            .versions
            .as_mut()
            .unwrap()
            .read_leases_for_test_v1()
            .release_reads(
                foreign.consumer,
                &[foreign],
                &ContextReadQuiescenceEvidenceV1 {
                    consumer: foreign.consumer,
                },
            )
            .unwrap();
        assert_eq!(
            context.generated_issue_exclusive_readers_v1(&plan),
            Ok(true)
        );
        assert_eq!(references(&context, id), original);
        assert_eq!(context.version_journal_read_records_v1(), Some(3));
        assert_eq!(context.generated_shells_unread_v1(&plan), Ok(false));
        assert!(matches!(
            context.retire_generated_shells_v1(&hold),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        assert_eq!(context.version_journal_read_records_v1(), Some(3));
        assert!(!context.is_terminal());
    }
}

#[test]
fn generated_settlement_receipt_cannot_release_a_substituted_live_reader_batch() {
    let mut context = context_with_journal(3);
    let (hold, plan, roster, _) = install_with_access(&mut context, false, [ReadOnly; 3]);
    context
        .begin_generated_issue_v1(&hold, plan, &roster)
        .unwrap();
    context.install_generated_submission_v1(&hold, 900).unwrap();
    let id = context.generated_issues[&hold.stream()].id;
    context
        .generated_issues
        .get_mut(&hold.stream())
        .unwrap()
        .phase = PhaseV1::Unknown;
    let old_receipt = completion::assume_native_settlement_for_context_test_v1(&context, &hold);
    let old_refs = references(&context, id);
    let sources = context
        .versions
        .as_ref()
        .unwrap()
        .submission_reader_sources_for_test_v1(id)
        .to_vec();
    let domain = generated_writer_domain_v1(&plan);
    // Private ABA injection: replace the model batch, not a supported generated retry.
    context
        .release_generated_submission_readers_v1(id, domain)
        .unwrap();
    let prepared = context.prepare_submission_readers_v1(&sources).unwrap();
    let replacement = context
        .begin_submission_readers_v1(id, prepared, domain)
        .unwrap();
    context.submissions.get_mut(&id).unwrap().journal_read = replacement;
    context
        .generated_issues
        .get_mut(&hold.stream())
        .unwrap()
        .expected_reader = replacement;
    let new_refs = references(&context, id);
    assert_ne!(old_refs, new_refs);
    assert!(context.generated_issue_token_v1(&hold, &plan).is_ok());
    assert!(
        context
            .settle_completed_gfx942_context_v1(&hold, old_receipt)
            .is_err()
    );
    assert_eq!(references(&context, id), new_refs);
    assert_eq!(context.version_journal_read_records_v1(), Some(3));
    assert_eq!(
        context.submissions[&id].status,
        RuntimeCompletionStatusV1::Pending
    );
    assert_eq!(context.allocations.len(), 3);
    assert!(!context.is_terminal());
}

#[test]
fn generated_post_release_disposal_panic_does_not_resurrect_readers() {
    let mut context = context_with_journal(3);
    let (hold, plan, roster, _) = install_with_access(&mut context, false, [ReadOnly; 3]);
    context
        .begin_generated_issue_v1(&hold, plan, &roster)
        .unwrap();
    context.install_generated_submission_v1(&hold, 900).unwrap();
    let id = context.generated_issues[&hold.stream()].id;
    context
        .allocation_admission
        .reject_disposal_for_test_v1(plan.members[0].unwrap().logical);
    // Native quiescence/disposal is assumed; the injected fault is Context credit commit.
    assert!(
        catch_unwind(AssertUnwindSafe(
            || context.settle_stopped_gfx942_context_v1(&hold, id, 900)
        ))
        .is_err()
    );
    assert!(context.is_terminal());
    assert_eq!(context.version_journal_read_records_v1(), Some(0));
    assert!(
        context.generated_issues[&hold.stream()]
            .expected_reader
            .is_none()
    );
    assert_eq!(context.allocations.len(), 3);
    assert_eq!(context.backend_allocations.len(), 3);
    assert!(!context.cleanup().is_complete());
    core::mem::forget(context);
}

#[test]
fn generated_read_only_default_profile_keeps_no_journal_markers() {
    for stop in [false, true] {
        let mut context = context();
        let (hold, plan, roster, _) = install_with_access(&mut context, false, [ReadOnly; 3]);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        context.install_generated_submission_v1(&hold, 900).unwrap();
        let id = context.generated_issues[&hold.stream()].id;
        assert!(
            context.generated_issues[&hold.stream()]
                .expected_reader
                .is_none()
        );
        assert!(context.submissions[&id].journal_read.is_none());
        assert_eq!(context.version_journal_read_records_v1(), None);
        assert_eq!(
            context.generated_issue_exclusive_readers_v1(&plan),
            Ok(true)
        );
        // No native claim: exercise the same owner-only Context settlement tail.
        if stop {
            context
                .settle_stopped_gfx942_context_v1(&hold, id, 900)
                .unwrap();
            context.release_unpublished_hold_v1(&hold).unwrap();
        } else {
            context
                .generated_issues
                .get_mut(&hold.stream())
                .unwrap()
                .phase = PhaseV1::Unknown;
            let receipt = completion::assume_native_settlement_for_context_test_v1(&context, &hold);
            context
                .settle_completed_gfx942_context_v1(&hold, receipt)
                .unwrap();
        }
        assert!(context.allocations.is_empty());
        assert!(context.submissions.is_empty());
        assert!(context.generated_issues.is_empty());
        assert!(!context.is_terminal());
    }
}

#[test]
fn generated_graph_readers_release_before_exact_reservation_retirement() {
    let mut context = context_with_journal(3);
    let (hold, plan, roster, access) = install_with_access(&mut context, true, [ReadOnly; 3]);
    let access = access.unwrap();
    context
        .begin_generated_issue_v1(&hold, plan, &roster)
        .unwrap();
    context.install_generated_submission_v1(&hold, 900).unwrap();
    context.close_graph_issue_v1(access).unwrap();
    assert_eq!(
        context.release_graph_v1(access),
        Err(RuntimeValidationErrorV1::SubmissionPending)
    );
    assert_eq!(context.version_journal_read_records_v1(), Some(3));
    context
        .generated_issues
        .get_mut(&hold.stream())
        .unwrap()
        .phase = PhaseV1::Unknown;
    let receipt = completion::assume_native_settlement_for_context_test_v1(&context, &hold);
    context
        .settle_completed_gfx942_context_v1(&hold, receipt)
        .unwrap();
    assert_eq!(context.version_journal_read_records_v1(), Some(0));
    assert!(context.allocations.is_empty());
    assert!(context.generated_issues.is_empty());
    context.release_graph_v1(access).unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn generated_no_effect_rejects_before_releasing_any_input_custody() {
    for accesses in [[ReadOnly; 3], [ReadOnly, ReadWrite, WriteOnly]] {
        let mut context = context_with_journal(3);
        let (hold, plan, roster, _) = install_with_access(&mut context, false, accesses);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        context.install_generated_submission_v1(&hold, 900).unwrap();
        let id = context.generated_issues[&hold.stream()].id;
        let original = references(&context, id);
        let before = states(&context, &plan);
        let marker = context.generated_issues[&hold.stream()].expected_reader;
        let credits = context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap();
        assert_eq!(
            context.settle_generated_custody_v1(hold.stream(), SubmissionWriterOutcomeV1::NoEffect),
            Err(RuntimeValidationErrorV1::InvalidBackendDescription)
        );
        assert_eq!(references(&context, id), original);
        assert_eq!(states(&context, &plan), before);
        assert_eq!(
            context.generated_issues[&hold.stream()].expected_reader,
            marker
        );
        assert_eq!(context.submissions[&id].journal_read, marker);
        assert_eq!(
            context.submissions[&id].status,
            RuntimeCompletionStatusV1::Pending
        );
        assert_eq!(
            context
                .allocation_admission_usage_v1(plan.binding.device)
                .unwrap(),
            credits
        );
        assert!(context.generated_issue_token_v1(&hold, &plan).is_ok());
        assert!(!context.is_terminal());
    }
}
