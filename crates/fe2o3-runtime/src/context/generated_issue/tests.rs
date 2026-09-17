use super::*;
use crate::RuntimeGfx942GeneratedSourceMutV1;
use crate::authorized_execution::tests::{source_authority, source_projection};

fn install(
    context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
) -> (
    ContextUnpublishedHoldV1,
    GeneratedShellPlanV1,
    GeneratedHostRosterV1,
) {
    let (binding, _) = RuntimeContextV1::generated_shell_test_binding_v1(&mut context.backend);
    context
        .backend
        .destroy_stream_v1(binding.backend_stream)
        .unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let hold = context.hold_unpublished_stream_v1(stream).unwrap();
    let (hsaco, projection) = source_projection();
    let authority = source_authority(&projection);
    let roster = GeneratedHostRosterV1::from_projection(&projection).unwrap();
    let mut storage = projection.into_generated_storage_v1();
    let mut source = RuntimeGfx942GeneratedSourceMutV1::new(&mut storage, &hsaco, &authority);
    context
        .install_generated_shells_v1(device, binding.native_device, &hold, &mut source, &roster)
        .unwrap();
    let plan = context.generated_plan_for_hold_v1(&hold).unwrap();
    (hold, plan, roster)
}

fn context() -> RuntimeContextV1<KfdRuntimeBackendV1> {
    let mut context = RuntimeContextV1::open(KfdRuntimeBackendV1::mock()).unwrap();
    context
        .configure_allocation_admission_v1(context.devices()[0].id(), 120, 6)
        .unwrap();
    context
}

#[test]
fn attempt_roots_one_logical_identity_and_full_mutation_roster_before_backend_return() {
    let mut context = context();
    let (hold, plan, roster) = install(&mut context);
    let next = context.next_identity;
    let usage = context
        .allocation_admission_usage_v1(plan.binding.device)
        .unwrap();
    context
        .begin_generated_issue_v1(&hold, plan, &roster)
        .unwrap();
    let attempt = &context.generated_issues[&hold.stream()];
    assert_eq!(
        attempt.id,
        RuntimeSubmissionIdV1::new(context.context_generation, next)
    );
    assert_eq!(attempt.plan, plan);
    assert!(attempt.roster.matches(&roster));
    assert!(attempt.submission.is_none());
    assert_eq!(attempt.phase, PhaseV1::Entering);
    assert!(context.submissions.is_empty());
    assert!(
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .is_err()
    );
    assert_eq!(context.next_identity, next + 1);
    let cleanup = context.cleanup();
    assert!(!cleanup.is_complete());
    assert_eq!(
        context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap(),
        usage
    );
    assert!(
        context.generated_issues[&hold.stream()]
            .submission
            .is_none()
    );
}

#[test]
fn issue_preflight_rejects_changed_roster_hold_and_exhausted_ids_without_attempt() {
    let mut context = context();
    let (hold, plan, roster) = install(&mut context);
    let (other_hold, _, _) = install(&mut context);
    let next = context.next_identity;
    let mut wrong = roster.clone();
    wrong.buffers[0].as_mut().unwrap().bytes += 1;
    assert!(
        context
            .begin_generated_issue_v1(&hold, plan, &wrong)
            .is_err()
    );
    assert!(
        context
            .begin_generated_issue_v1(&other_hold, plan, &roster)
            .is_err()
    );
    assert!(context.generated_issues.is_empty());
    assert_eq!(context.next_identity, next);
    for exhausted in [0, u64::MAX] {
        context.next_identity = exhausted;
        assert!(
            context
                .begin_generated_issue_v1(&hold, plan, &roster)
                .is_err()
        );
        assert_eq!(context.next_identity, exhausted);
        assert!(context.generated_issues.is_empty());
    }
}

#[test]
fn attempt_failure_before_backend_identity_is_sticky_unknown_with_credits_retained() {
    for panic in [false, true] {
        let mut context = context();
        let (hold, plan, roster) = install(&mut context);
        let usage = context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            context.begin_generated_issue_v1(&hold, plan, &roster)?;
            if panic {
                panic!("backend preparation unwind before handle");
            }
            Err(RuntimeValidationErrorV1::Capacity.into())
        }));
        let result = catch_unwind(AssertUnwindSafe(|| {
            context.finish_generated_issue_v1(&hold, result)
        }));
        assert!(if panic {
            result.is_err()
        } else {
            result.unwrap().is_err()
        });
        assert!(context.is_terminal());
        let next = context.next_identity;
        let attempt = &context.generated_issues[&hold.stream()];
        assert_eq!(attempt.phase, PhaseV1::Unknown);
        assert!(attempt.submission.is_none());
        assert!(context.retire_gfx942_issued_v1(&hold).is_err());
        assert_eq!(context.next_identity, next);
        assert_eq!(
            context
                .allocation_admission_usage_v1(plan.binding.device)
                .unwrap(),
            usage
        );
        assert!(!context.cleanup().is_complete());
        core::mem::forget(context);
    }
}

#[test]
fn invalid_returned_backend_identity_is_rooted_before_protocol_terminalization() {
    for backend in [0, 900] {
        let mut context = context();
        let (hold, plan, roster) = install(&mut context);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        if backend != 0 {
            context.backend_submissions.insert(backend);
        }
        let result = context.install_generated_submission_v1(&hold, backend);
        assert!(result.is_err());
        assert!(
            context
                .finish_generated_issue_v1(&hold, Ok(result))
                .is_err()
        );
        let attempt = &context.generated_issues[&hold.stream()];
        assert_eq!(
            attempt.submission.as_ref().unwrap().backend_submission,
            backend
        );
        assert_eq!(attempt.submission.as_ref().unwrap().id, attempt.id);
        assert_eq!(attempt.phase, PhaseV1::Unknown);
        assert!(context.submissions.is_empty());
        assert!(!context.cleanup().is_complete());
        core::mem::forget(context);
    }
}

#[test]
fn a_different_live_token_cannot_authorize_generated_progress_or_retirement() {
    let mut context = context();
    let (first, first_plan, first_roster) = install(&mut context);
    let (second, second_plan, second_roster) = install(&mut context);
    for (hold, plan, roster, backend) in [
        (&first, first_plan, &first_roster, 900),
        (&second, second_plan, &second_roster, 901),
    ] {
        context
            .begin_generated_issue_v1(hold, plan, roster)
            .unwrap();
        context
            .install_generated_submission_v1(hold, backend)
            .unwrap();
        context
            .generated_issues
            .get_mut(&hold.stream())
            .unwrap()
            .phase = PhaseV1::Active;
        assert!(context.generated_issue_token_v1(hold, &plan).is_ok());
    }
    let mut other = context.generated_issues.remove(&second.stream()).unwrap();
    core::mem::swap(
        &mut context
            .generated_issues
            .get_mut(&first.stream())
            .unwrap()
            .submission,
        &mut other.submission,
    );
    context.generated_issues.insert(second.stream(), other);
    assert!(
        context
            .live_submission_record(
                context.generated_issues[&first.stream()]
                    .submission
                    .as_ref()
                    .unwrap()
            )
            .is_ok()
    );
    assert!(
        context
            .generated_issue_token_v1(&first, &first_plan)
            .is_err()
    );
    let before = context
        .allocation_admission_usage_v1(first_plan.binding.device)
        .unwrap();
    assert!(context.retire_gfx942_issued_v1(&first).is_err());
    assert_eq!(context.submissions.len(), 2);
    assert_eq!(context.allocations.len(), 6);
    assert_eq!(
        context
            .allocation_admission_usage_v1(first_plan.binding.device)
            .unwrap(),
        before
    );
    assert_eq!(
        context.generated_issues[&first.stream()].phase,
        PhaseV1::Unknown
    );
    core::mem::forget(context);
}
