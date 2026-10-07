use super::super::completion::assume_native_settlement_for_context_test_v1 as assumed_settlement;
use super::*;
use crate::authorized_execution::tests::source_authority_for_device;

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;

fn context() -> Context {
    let mut context = RuntimeContextV1::open_with_version_journal_v1(
        KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1(),
        6,
        2,
    )
    .unwrap();
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    for device in devices {
        context
            .configure_allocation_admission_v1(device, 60, 3)
            .unwrap();
    }
    context
}

fn install(
    context: &mut Context,
    uid: u64,
) -> (
    ContextUnpublishedHoldV1,
    GeneratedShellPlanV1,
    GeneratedHostRosterV1,
) {
    use crate::Gfx942RuntimeBufferAccessV1::{ReadOnly, ReadWrite, WriteOnly};
    let device = context
        .devices()
        .iter()
        .find(|device| device.backend_device == uid)
        .unwrap()
        .id();
    let stream = context.create_stream(device).unwrap();
    let hold = context.hold_unpublished_stream_v1(stream).unwrap();
    let (hsaco, projection) = source_projection_with_access(
        fe2o3_aql::AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
        [ReadOnly, ReadWrite, WriteOnly],
    );
    let authority = source_authority_for_device(&projection, uid);
    let roster = GeneratedHostRosterV1::from_projection(&projection).unwrap();
    let mut storage = projection.into_generated_storage_v1();
    let mut source = RuntimeGfx942GeneratedSourceMutV1::new(&mut storage, &hsaco, &authority);
    let (descriptive, _) = RuntimeContextV1::generated_route_ids_for_test_v1(uid, 1, 3);
    context
        .install_generated_shells_v1(
            device,
            descriptive.native_device,
            &hold,
            &mut source,
            &roster,
        )
        .unwrap();
    let plan = context.generated_plan_for_hold_v1(&hold).unwrap();
    (hold, plan, roster)
}

fn begin(
    context: &mut Context,
    uid: u64,
    returned: u64,
) -> (ContextUnpublishedHoldV1, GeneratedShellPlanV1) {
    let (hold, plan, roster) = install(context, uid);
    context
        .begin_generated_issue_v1(&hold, plan, &roster)
        .unwrap();
    // Synthetic returned IDs test Context accounting, never native issue or completion.
    context
        .install_generated_submission_v1(&hold, returned)
        .unwrap();
    context
        .generated_issues
        .get_mut(&hold.stream())
        .unwrap()
        .phase = PhaseV1::Active;
    (hold, plan)
}

fn settle(context: &mut Context, hold: &ContextUnpublishedHoldV1) {
    context
        .generated_issues
        .get_mut(&hold.stream())
        .unwrap()
        .phase = PhaseV1::Unknown;
    let settled = assumed_settlement(context, hold);
    context
        .settle_completed_gfx942_context_v1(hold, settled)
        .unwrap();
}

#[test]
fn generated_multi_issue_completion_rejects_synthetic_device_before_lending() {
    use super::completion_tests::{OmittingCarrier, assert_submission_retained, carrier};
    use std::{cell::Cell, rc::Rc};
    for uid in [7, 8, 9] {
        let mut context = context();
        let (hold, plan) = begin(&mut context, uid, 900);
        let roster = context.generated_issues[&hold.stream()].roster.clone();
        let calls = Rc::new(Cell::new(0));
        let drops = Rc::new(Cell::new(0));
        let mut prepared = context
            .bound_multi_preparation_for_test_v1(plan.binding.device, carrier(&calls, &drops));
        let snapshot = |carrier: &OmittingCarrier| {
            carrier
                .destinations
                .iter()
                .map(|bytes| (bytes.clone(), bytes.as_ptr(), bytes.len(), bytes.capacity()))
                .collect::<Vec<_>>()
        };
        let before = snapshot(prepared.value());
        let id = context.generated_issues[&hold.stream()].id;
        let record = context.submissions[&id];
        let next = context.next_identity;
        for _ in 0..2 {
            assert!(
                context
                    .complete_gfx942_issue_v1(&mut prepared, &roster, &hold)
                    .is_err()
            );
            assert!(context.is_terminal());
            assert_eq!(calls.get(), 0);
            assert_eq!(drops.get(), 0);
            assert_eq!(snapshot(prepared.value()), before);
            assert_eq!(context.next_identity, next);
            assert_submission_retained(&context, id, record);
            let attempt = &context.generated_issues[&hold.stream()];
            assert_eq!(attempt.phase, PhaseV1::Unknown);
            assert_eq!(attempt.plan, plan);
            assert!(attempt.roster.matches(&roster));
            assert_eq!(context.allocations.len(), 3);
            assert_eq!(context.backend_submissions, [900].into_iter().collect());
            assert_eq!(
                context.unpublished_identity_for_test_v1(hold.stream()),
                Some(Some(hold.identity()))
            );
            let usage = context
                .allocation_admission_usage_v1(plan.binding.device)
                .unwrap()
                .unwrap();
            assert_eq!(usage.retained_records, 0);
            assert_eq!(usage.quarantined_records, 3);
        }
        core::mem::forget(context);
        drop(prepared);
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn generated_multi_issue_context_readers_writers_and_completion_tails_are_independent() {
    let mut context = context();
    let (a, pa) = begin(&mut context, 7, 900);
    let (b, pb) = begin(&mut context, 8, 901);
    assert_eq!(context.version_journal_writer_records_v1(), Some(2));
    assert_eq!(context.version_journal_read_records_v1(), Some(2));
    assert_eq!(
        context
            .version_journal_usage_v1()
            .unwrap()
            .allocation_records,
        6
    );
    for (hold, plan) in [(&a, &pa), (&b, &pb)] {
        let attempt = &context.generated_issues[&hold.stream()];
        assert!(attempt.expected_writer.is_some());
        assert_eq!(attempt.expected_reader.unwrap().count, 1);
        context.generated_issue_token_v1(hold, plan).unwrap();
    }
    let sibling = context.generated_issues[&a.stream()].id;
    let credit = context
        .allocation_admission_usage_v1(pa.binding.device)
        .unwrap();
    settle(&mut context, &b);
    assert_eq!(context.generated_issues[&a.stream()].id, sibling);
    context.generated_issue_token_v1(&a, &pa).unwrap();
    assert_eq!(
        context
            .allocation_admission_usage_v1(pa.binding.device)
            .unwrap(),
        credit
    );
    assert_eq!(
        context
            .allocation_admission_usage_v1(pb.binding.device)
            .unwrap()
            .unwrap()
            .used,
        crate::RuntimeResourceVectorV1::ZERO
    );
    assert_eq!(context.version_journal_writer_records_v1(), Some(1));
    assert_eq!(context.version_journal_read_records_v1(), Some(1));
    assert_eq!(
        context
            .version_journal_usage_v1()
            .unwrap()
            .allocation_records,
        3
    );
    settle(&mut context, &a);
    assert!(context.allocations.is_empty() && context.submissions.is_empty());
    assert!(context.generated_issues.is_empty() && context.backend_submissions.is_empty());
    assert_eq!(context.version_journal_writer_records_v1(), Some(0));
    assert_eq!(context.version_journal_read_records_v1(), Some(0));
    assert!(context.cleanup().is_complete());
    context.shutdown_owned_backend_v1().unwrap();
}

#[test]
fn generated_multi_issue_context_cross_child_settlement_cannot_consume_sibling() {
    let mut context = context();
    let (a, pa) = begin(&mut context, 7, 900);
    let (b, pb) = begin(&mut context, 8, 901);
    context.generated_issues.get_mut(&a.stream()).unwrap().phase = PhaseV1::Unknown;
    let wrong = assumed_settlement(&context, &b);
    assert!(
        context
            .settle_completed_gfx942_context_v1(&a, wrong)
            .is_err()
    );
    assert!(!context.is_terminal());
    assert_eq!(context.generated_issues.len(), 2);
    assert_eq!(context.allocations.len(), 6);
    context.generated_issue_token_v1(&a, &pa).unwrap();
    context.generated_issue_token_v1(&b, &pb).unwrap();
    settle(&mut context, &a);
    settle(&mut context, &b);
    assert!(context.cleanup().is_complete());
    context.shutdown_owned_backend_v1().unwrap();
}

#[test]
fn generated_multi_issue_context_duplicate_begin_does_not_retain_extra_writers_or_ids() {
    let mut context = context();
    let (hold, plan, roster) = install(&mut context, 8);
    context
        .begin_generated_issue_v1(&hold, plan, &roster)
        .unwrap();
    let next = context.next_identity;
    assert!(
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .is_err()
    );
    assert_eq!(context.next_identity, next);
    assert_eq!(context.version_journal_writer_records_v1(), Some(1));
    assert_eq!(context.version_journal_read_records_v1(), Some(1));
    context.install_generated_submission_v1(&hold, 900).unwrap();
    settle(&mut context, &hold);
    assert!(context.cleanup().is_complete());
    context.shutdown_owned_backend_v1().unwrap();
}

#[test]
fn generated_multi_issue_context_failed_identity_and_unwind_retain_both_accounts() {
    for panic in [false, true] {
        let mut context = context();
        let (a, pa) = begin(&mut context, 7, 900);
        let (b, pb, roster) = install(&mut context, 8);
        let scope = context.generated_adoption_scope_for_hold_v1(&b).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            context.begin_generated_issue_v1(&b, pb, &roster)?;
            if panic {
                std::panic::panic_any(73u32);
            }
            context.install_generated_submission_v1(&b, 900)
        }));
        let result = catch_unwind(AssertUnwindSafe(|| {
            context.finish_generated_issue_scoped_v1(&b, scope, result)
        }));
        if panic {
            assert_eq!(result.unwrap_err().downcast_ref::<u32>(), Some(&73));
        } else {
            assert!(result.unwrap().is_err());
        }
        assert!(context.is_terminal());
        assert_eq!(context.generated_issues.len(), 2);
        assert_eq!(
            context.generated_issues[&b.stream()].phase,
            PhaseV1::Unknown
        );
        assert!(context.generated_issues[&a.stream()].submission.is_some());
        for plan in [pa, pb] {
            let usage = context
                .allocation_admission_usage_v1(plan.binding.device)
                .unwrap()
                .unwrap();
            assert_eq!(usage.retained_records, 0);
            assert_eq!(usage.quarantined_records, 3);
        }
        assert_eq!(context.allocations.len(), 6);
        assert!(!context.cleanup().is_complete());
        core::mem::forget(context);
    }
}
