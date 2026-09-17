use super::*;
use crate::Gfx942RuntimeBufferAccessV1::{self as Access, ReadOnly, ReadWrite, WriteOnly};
use fe2o3_runtime_model::{ContextAllocationStateV1, ContextWriterStateV1};

mod reader_tests;

fn context_with_journal(writers: usize) -> RuntimeContextV1<KfdRuntimeBackendV1> {
    let mut context =
        RuntimeContextV1::open_with_version_journal_v1(KfdRuntimeBackendV1::mock(), 6, writers)
            .unwrap();
    context
        .configure_allocation_admission_v1(context.devices()[0].id(), 120, 6)
        .unwrap();
    context
}

fn states(
    context: &RuntimeContextV1<KfdRuntimeBackendV1>,
    plan: &GeneratedShellPlanV1,
) -> Vec<ContextAllocationStateV1> {
    let journal = context.versions.as_ref().unwrap().journal_for_test();
    plan.members[..plan.count]
        .iter()
        .flatten()
        .map(|member| {
            journal
                .lookup_allocation(context.allocations[&member.logical].journal.unwrap())
                .unwrap()
        })
        .collect()
}

fn access_rosters() -> [[Access; 3]; 3] {
    [
        [ReadOnly, ReadWrite, WriteOnly],
        [ReadOnly; 3],
        [ReadWrite; 3],
    ]
}

#[test]
fn generated_shell_readers_block_whole_batch_retirement_before_any_shell_disposal() {
    use fe2o3_runtime_model::{
        ContextAllocationReadV1, ContextReadQuiescenceEvidenceV1, ContextWriterKeyV1,
        ContextWriterKindV1,
    };
    for index in 0..3 {
        let mut context = context_with_journal(3);
        let (hold, plan, _, _) = install_with_access(&mut context, false, [ReadOnly; 3]);
        let before = states(&context, &plan);
        let member = plan.members[index].unwrap();
        let reference = context.allocations[&member.logical].journal.unwrap();
        let state = before[index];
        let consumer = ContextWriterKeyV1 {
            context_generation: context.context_generation,
            local: 900,
            kind: ContextWriterKindV1::Submission,
        };
        let mut output = [None];
        // Inert model premise only: private shell IDs do not escape the public API.
        context
            .versions
            .as_mut()
            .unwrap()
            .read_leases_for_test_v1()
            .acquire_reads(
                consumer,
                &[ContextAllocationReadV1 {
                    allocation: reference,
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
        let credits = context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap();
        assert_eq!(context.generated_shells_unread_v1(&plan), Ok(false));
        assert!(matches!(
            context.retire_generated_shells_v1(&hold),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        assert_eq!(states(&context, &plan), before);
        assert_eq!(
            context
                .allocation_admission_usage_v1(plan.binding.device)
                .unwrap(),
            credits
        );
        assert_eq!(context.generated_plan_for_hold_v1(&hold).unwrap(), plan);
        assert_eq!(context.version_journal_read_records_v1(), Some(1));
        assert!(!context.is_terminal());
        context
            .versions
            .as_mut()
            .unwrap()
            .read_leases_for_test_v1()
            .release_reads(
                consumer,
                &[output[0].unwrap()],
                &ContextReadQuiescenceEvidenceV1 { consumer },
            )
            .unwrap();
        assert_eq!(context.generated_shells_unread_v1(&plan), Ok(true));
        context.retire_generated_shells_v1(&hold).unwrap();
        assert!(context.allocations.is_empty());
        context.release_unpublished_hold_v1(&hold).unwrap();
        assert!(context.cleanup().is_complete());
    }
}

#[test]
fn generated_journal_begin_binds_only_original_writable_members() {
    for accesses in access_rosters() {
        let mut context = context_with_journal(3);
        let (hold, plan, roster, _) = install_with_access(&mut context, false, accesses);
        let before = states(&context, &plan);
        let credits = context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap();
        let next = context.next_identity;
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        let attempt = &context.generated_issues[&hold.stream()];
        assert_eq!(attempt.id.local, next);
        assert_eq!(attempt.plan, plan);
        assert!(attempt.roster.matches(&roster));
        let writer = attempt.expected_writer;
        let count = accesses
            .iter()
            .filter(|&&access| access != ReadOnly)
            .count();
        assert_eq!(writer.is_some(), count != 0);
        assert_eq!(
            context.version_journal_writer_records_v1(),
            Some(usize::from(count != 0))
        );
        let after = states(&context, &plan);
        for (index, access) in accesses.iter().enumerate() {
            assert_eq!(
                after[index].attempt_epoch,
                before[index].attempt_epoch + u64::from(*access != ReadOnly)
            );
            assert_eq!(
                after[index].pending_writer,
                if *access == ReadOnly { None } else { writer }
            );
            if *access == ReadOnly {
                assert_eq!(after[index], before[index]);
            }
        }
        if let Some(writer) = writer {
            assert_eq!(
                context
                    .versions
                    .as_ref()
                    .unwrap()
                    .journal_for_test()
                    .lookup_writer(writer)
                    .unwrap(),
                ContextWriterStateV1::Pending {
                    member_count: count
                }
            );
        }
        assert_eq!(
            context
                .allocation_admission_usage_v1(plan.binding.device)
                .unwrap(),
            credits
        );
        context.install_generated_submission_v1(&hold, 900).unwrap();
        assert_eq!(
            context.submissions[&context.generated_issues[&hold.stream()].id].journal_writer,
            writer
        );
        assert!(context.generated_issue_token_v1(&hold, &plan).is_ok());
    }
}

#[test]
fn generated_journal_capacity_rejects_before_identity_or_begin() {
    let mut context = context_with_journal(1);
    let (first, plan, roster) = install(&mut context);
    context
        .begin_generated_issue_v1(&first, plan, &roster)
        .unwrap();
    let (second, plan, roster) = install(&mut context);
    let next = context.next_identity;
    let before = states(&context, &plan);
    let credits = context
        .allocation_admission_usage_v1(plan.binding.device)
        .unwrap();
    assert!(matches!(
        context.begin_generated_issue_v1(&second, plan, &roster),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(context.next_identity, next);
    assert_eq!(states(&context, &plan), before);
    assert_eq!(
        context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap(),
        credits
    );
    assert!(!context.generated_issues.contains_key(&second.stream()));
    assert!(!context.is_terminal());
}

#[test]
fn generated_journal_generic_observation_and_no_effect_cannot_settle() {
    for journal in [false, true] {
        let mut context = if journal {
            context_with_journal(2)
        } else {
            context()
        };
        let (hold, plan, roster) = install(&mut context);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        context.install_generated_submission_v1(&hold, 900).unwrap();
        let id = context.generated_issues[&hold.stream()].id;
        let record = context.submissions[&id];
        let before = journal.then(|| states(&context, &plan));
        for status in [
            RuntimeCompletionStatusV1::Pending,
            RuntimeCompletionStatusV1::Succeeded,
            RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::BackendCode(-1)),
            RuntimeCompletionStatusV1::QuiescentWithoutResult,
        ] {
            assert_eq!(
                context.transition_submission_status(id, status),
                Err(RuntimeValidationErrorV1::ContextReserved)
            );
            completion_tests::assert_submission_retained(&context, id, record);
        }
        for outcome in [
            SubmissionWriterOutcomeV1::Success,
            SubmissionWriterOutcomeV1::NoEffect,
            SubmissionWriterOutcomeV1::Unknown,
        ] {
            assert_eq!(
                context.settle_submission_writer_v1(id, outcome),
                Err(RuntimeValidationErrorV1::ContextReserved)
            );
        }
        assert!(matches!(
            context.poll_async_drain_v1(id),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        let mut token = context
            .generated_issues
            .get_mut(&hold.stream())
            .unwrap()
            .submission
            .take()
            .unwrap();
        for result in [
            context.poll(&mut token),
            context.wait(&mut token, Duration::ZERO),
            context.drain(&mut token, Instant::now() + Duration::from_secs(60)),
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
        assert!(matches!(
            context.record_event(&token),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        assert!(matches!(
            context.release_submission_ref(&token, None),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        assert!(matches!(
            context.synchronize_stream(hold.stream(), Duration::ZERO),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        context
            .generated_issues
            .get_mut(&hold.stream())
            .unwrap()
            .submission = Some(token);
        assert_eq!(journal.then(|| states(&context, &plan)), before);
        completion_tests::assert_submission_retained(&context, id, record);
        assert!(!context.is_terminal());
    }
}

#[test]
fn generated_journal_assumed_c4_settlement_is_exact_and_neighbor_preserving() {
    for accesses in access_rosters() {
        let mut context = context_with_journal(3);
        let (hold, plan, roster, _) = install_with_access(&mut context, false, accesses);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        context.install_generated_submission_v1(&hold, 900).unwrap();
        let id = context.generated_issues[&hold.stream()].id;
        let (neighbor, neighbor_plan, neighbor_roster) = install(&mut context);
        context
            .begin_generated_issue_v1(&neighbor, neighbor_plan, &neighbor_roster)
            .unwrap();
        context
            .install_generated_submission_v1(&neighbor, 901)
            .unwrap();
        let neighbor_states = states(&context, &neighbor_plan);
        context
            .generated_issues
            .get_mut(&hold.stream())
            .unwrap()
            .phase = PhaseV1::Unknown;
        let wrong = completion::assume_native_settlement_for_context_test_v1(&context, &neighbor);
        assert!(
            context
                .settle_completed_gfx942_context_v1(&hold, wrong)
                .is_err()
        );
        for (member, access) in states(&context, &plan).iter().zip(accesses) {
            assert_eq!(member.pending_writer.is_some(), access != ReadOnly);
        }
        // This is the production bookkeeping tail with a test-only assumed
        // receipt. It does not exercise native protected completion/readback.
        let receipt = completion::assume_native_settlement_for_context_test_v1(&context, &hold);
        context
            .settle_completed_gfx942_context_v1(&hold, receipt)
            .unwrap();
        assert!(!context.submissions.contains_key(&id));
        assert_eq!(context.version_journal_writer_records_v1(), Some(1));
        assert_eq!(
            context
                .version_journal_usage_v1()
                .unwrap()
                .allocation_records,
            3
        );
        assert_eq!(states(&context, &neighbor_plan), neighbor_states);
        assert_eq!(context.allocations.len(), 3);
        for member in plan.members[..plan.count].iter().flatten() {
            assert!(!context.allocations.contains_key(&member.logical));
            assert!(!context.backend_allocations.contains(&member.backend));
        }
        assert_eq!(
            context
                .allocation_admission_usage_v1(plan.binding.device)
                .unwrap()
                .unwrap()
                .retained_records,
            3
        );
        assert_eq!(
            context.unpublished_identity_for_test_v1(hold.stream()),
            Some(None)
        );
        assert!(!context.is_terminal());
    }
}

#[test]
fn generated_journal_stop_batch_disposes_unknown_and_read_only_members() {
    for accesses in access_rosters() {
        let mut context = context_with_journal(3);
        let (hold, plan, roster, _) = install_with_access(&mut context, false, accesses);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        context.install_generated_submission_v1(&hold, 900).unwrap();
        let id = context.generated_issues[&hold.stream()].id;
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
        // Inert shell metadata and a logical submission only: native disposal
        // is an explicit assumption of this metadata-tail test.
        context
            .settle_stopped_gfx942_context_v1(&hold, id, 900)
            .unwrap();
        assert_eq!(
            *observed.lock().unwrap(),
            [RuntimeCompletionStatusV1::QuiescentWithoutResult]
        );
        assert_eq!(context.version_journal_writer_records_v1(), Some(0));
        assert_eq!(
            context
                .version_journal_usage_v1()
                .unwrap()
                .allocation_records,
            0
        );
        assert!(context.allocations.is_empty());
        assert!(context.backend_allocations.is_empty());
        assert!(context.submissions.is_empty());
        assert!(context.generated_issues.is_empty());
        let credits = context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap()
            .unwrap();
        assert_eq!(credits.used, crate::RuntimeResourceVectorV1::ZERO);
        assert_eq!(credits.retained_records, 0);
        assert!(!context.is_terminal());
        context.release_unpublished_hold_v1(&hold).unwrap();
    }
}

#[test]
fn generated_journal_no_handle_failures_retain_unknown_writer_and_original_members() {
    for panic in [false, true] {
        let mut context = context_with_journal(1);
        let (hold, plan, roster) = install(&mut context);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        let writer = context.generated_issues[&hold.stream()]
            .expected_writer
            .unwrap();
        let before = states(&context, &plan);
        let credits = context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap();
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
            assert!(result.unwrap().is_err());
        }
        assert!(context.is_terminal());
        assert_eq!(
            context
                .versions
                .as_ref()
                .unwrap()
                .journal_for_test()
                .lookup_writer(writer)
                .unwrap(),
            ContextWriterStateV1::Unknown { member_count: 3 }
        );
        assert_eq!(states(&context, &plan), before);
        assert_eq!(context.version_journal_writer_records_v1(), Some(1));
        let after = context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap()
            .unwrap();
        assert_eq!(after.used, credits.unwrap().used);
        assert_eq!(after.retained_records, 0);
        assert_eq!(after.quarantined_records, 3);
        assert!(context.retire_gfx942_issued_v1(&hold).is_err());
        assert_eq!(context.allocations.len(), 3);
        core::mem::forget(context);
    }
}

#[test]
fn generated_journal_post_model_credit_fault_retains_finalizing_writer() {
    let mut context = context_with_journal(1);
    let (hold, plan, roster) = install(&mut context);
    context
        .begin_generated_issue_v1(&hold, plan, &roster)
        .unwrap();
    context.install_generated_submission_v1(&hold, 900).unwrap();
    let id = context.generated_issues[&hold.stream()].id;
    context
        .allocation_admission
        .reject_disposal_for_test_v1(plan.members[0].unwrap().logical);
    let result = catch_unwind(AssertUnwindSafe(|| {
        context.settle_stopped_gfx942_context_v1(&hold, id, 900)
    }));
    assert!(result.is_err());
    assert!(context.is_terminal());
    assert_eq!(
        context
            .version_journal_usage_v1()
            .unwrap()
            .allocation_records,
        0
    );
    assert_eq!(context.version_journal_writer_records_v1(), Some(1));
    assert_eq!(
        context
            .versions
            .as_ref()
            .unwrap()
            .submission_disposal_progress_for_test_v1(id),
        Some((3, true))
    );
    assert!(context.allocations.is_empty());
    assert!(context.backend_allocations.is_empty());
    assert_eq!(
        context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap()
            .unwrap()
            .quarantined_records,
        3
    );
    assert!(context.retire_gfx942_issued_v1(&hold).is_err());
    assert!(!context.cleanup().is_complete());
    core::mem::forget(context);
}

#[test]
fn generated_journal_success_advances_lineage_once_before_shell_retirement() {
    let mut context = context_with_journal(1);
    let accesses = [ReadOnly, ReadWrite, WriteOnly];
    let (hold, plan, roster, _) = install_with_access(&mut context, false, accesses);
    context
        .begin_generated_issue_v1(&hold, plan, &roster)
        .unwrap();
    context.install_generated_submission_v1(&hold, 900).unwrap();
    let id = context.generated_issues[&hold.stream()].id;
    let before = states(&context, &plan);
    // Isolate the shared settlement bookkeeping under an assumed successful
    // protected completion; no native receipt is produced by this test.
    context
        .settle_generated_custody_v1(hold.stream(), SubmissionWriterOutcomeV1::Success)
        .unwrap();
    let after = states(&context, &plan);
    for index in 0..3 {
        assert_eq!(after[index].attempt_epoch, before[index].attempt_epoch);
        assert_eq!(
            after[index].content_lineage,
            if accesses[index] == ReadOnly {
                before[index].content_lineage
            } else {
                before[index].attempt_epoch
            }
        );
        assert_eq!(after[index].pending_writer, None);
    }
    assert_eq!(after[0], before[0]);
    assert_eq!(context.version_journal_writer_records_v1(), Some(0));
    assert!(context.submissions[&id].journal_writer.is_none());
    assert!(
        context.generated_issues[&hold.stream()]
            .expected_writer
            .is_none()
    );
    assert!(
        context
            .settle_generated_custody_v1(hold.stream(), SubmissionWriterOutcomeV1::Success)
            .is_err()
    );
    assert_eq!(states(&context, &plan), after);
    assert!(context.is_terminal());
    core::mem::forget(context);
}

#[test]
fn generated_journal_changed_marker_or_missing_root_cannot_commit_completion() {
    for fault in 0..4 {
        let mut context = context_with_journal(2);
        let (hold, plan, roster) = install(&mut context);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        context.install_generated_submission_v1(&hold, 900).unwrap();
        let id = context.generated_issues[&hold.stream()].id;
        let (neighbor, neighbor_plan, neighbor_roster) = install(&mut context);
        context
            .begin_generated_issue_v1(&neighbor, neighbor_plan, &neighbor_roster)
            .unwrap();
        context
            .install_generated_submission_v1(&neighbor, 901)
            .unwrap();
        context
            .generated_issues
            .get_mut(&hold.stream())
            .unwrap()
            .phase = PhaseV1::Unknown;
        let receipt = completion::assume_native_settlement_for_context_test_v1(&context, &hold);
        let before = states(&context, &plan);
        let neighbor_before = states(&context, &neighbor_plan);
        match fault {
            0 => {
                context
                    .generated_issues
                    .get_mut(&hold.stream())
                    .unwrap()
                    .expected_writer = None
            }
            1 => context.submissions.get_mut(&id).unwrap().journal_writer = None,
            2 => context
                .versions
                .as_mut()
                .unwrap()
                .remove_submission_writer_root_for_test_v1(id),
            _ => {
                context
                    .generated_issues
                    .get_mut(&hold.stream())
                    .unwrap()
                    .roster
                    .buffers[0]
                    .as_mut()
                    .unwrap()
                    .access = ReadOnly
            }
        }
        assert!(
            context
                .settle_completed_gfx942_context_v1(&hold, receipt)
                .is_err()
        );
        assert_eq!(states(&context, &plan), before);
        assert_eq!(states(&context, &neighbor_plan), neighbor_before);
        assert_eq!(context.allocations.len(), 6);
        assert_eq!(context.version_journal_writer_records_v1(), Some(2));
        assert_eq!(
            context
                .allocation_admission_usage_v1(plan.binding.device)
                .unwrap()
                .unwrap()
                .retained_records,
            6
        );
        assert!(context.backend.validate_generated_shell_disposal_v1(&plan));
        core::mem::forget(context);
    }
}

#[test]
fn generated_journal_stop_preflights_read_only_members_before_batch_disposal() {
    let mut context = context_with_journal(2);
    let (hold, plan, roster, _) =
        install_with_access(&mut context, false, [ReadOnly, ReadWrite, WriteOnly]);
    context
        .begin_generated_issue_v1(&hold, plan, &roster)
        .unwrap();
    context.install_generated_submission_v1(&hold, 900).unwrap();
    let id = context.generated_issues[&hold.stream()].id;
    let extra = reader_tests::add_foreign_reader(&mut context, &plan, 0);
    let before = states(&context, &plan);
    assert!(
        context
            .settle_stopped_gfx942_context_v1(&hold, id, 900)
            .is_err()
    );
    assert!(!context.is_terminal());
    assert_eq!(states(&context, &plan), before);
    assert!(context.backend.validate_generated_shell_disposal_v1(&plan));
    assert_eq!(context.allocations.len(), 3);
    assert_eq!(context.backend_allocations.len(), 3);
    assert_eq!(context.version_journal_writer_records_v1(), Some(1));
    assert_eq!(context.version_journal_read_records_v1(), Some(2));
    assert!(
        context
            .versions
            .as_mut()
            .unwrap()
            .read_leases_for_test_v1()
            .lookup_read(extra)
            .is_ok()
    );
    core::mem::forget(context);
}
