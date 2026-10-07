use super::*;

#[derive(Debug, PartialEq)]
struct Snapshot {
    allocations: usize,
    submissions: usize,
    attempts: usize,
    next_identity: u64,
    backend_submissions: HashSet<u64>,
    usage: Option<crate::RuntimeResourceCreditUsageV1>,
}

fn snapshot(
    context: &RuntimeContextV1<KfdRuntimeBackendV1>,
    plan: &GeneratedShellPlanV1,
) -> Snapshot {
    Snapshot {
        allocations: context.allocations.len(),
        submissions: context.submissions.len(),
        attempts: context.generated_issues.len(),
        next_identity: context.next_identity,
        backend_submissions: context.backend_submissions.clone(),
        usage: context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap(),
    }
}

#[test]
fn unpublished_gate_rejects_descriptive_admission_and_foreign_hold_without_disposal() {
    let mut context = context();
    let (hold, plan, _) = install(&mut context);
    let mut foreign = self::context();
    let (foreign_hold, _, _) = install(&mut foreign);
    let before = snapshot(&context, &plan);
    for _ in 0..2 {
        assert!(!context.gfx942_adoption_unpublished_v1(&hold).unwrap());
        assert!(context.retire_gfx942_unpublished_v1(&hold).is_err());
        assert!(
            context
                .gfx942_adoption_unpublished_v1(&foreign_hold)
                .is_err()
        );
        assert!(context.retire_gfx942_unpublished_v1(&foreign_hold).is_err());
        assert_eq!(snapshot(&context, &plan), before);
        assert_eq!(context.generated_plan_for_hold_v1(&hold).unwrap(), plan);
        assert!(!context.is_terminal());
    }
}

#[test]
fn unpublished_gate_rejects_unreturned_receipt_or_changed_source_binding_without_disposal() {
    for mode in 0..5 {
        let mut context = context();
        let (hold, plan, roster) = install(&mut context);
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        if mode != 0 {
            // Actual Context registration/accounting with a descriptive returned
            // ID. It is deliberately not backed by a native receipt or lane.
            context.install_generated_submission_v1(&hold, 900).unwrap();
            let attempt = context.generated_issues.get_mut(&hold.stream()).unwrap();
            attempt.phase = PhaseV1::Active;
            match mode {
                2 => attempt.submission.as_mut().unwrap().backend_submission = 901,
                3 => attempt.roster.source_identity = std::sync::Arc::new(()),
                4 => attempt.phase = PhaseV1::PhysicallyComplete,
                _ => {}
            }
        }
        let before = snapshot(&context, &plan);
        let phase = context.generated_issues[&hold.stream()].phase;
        let source = std::sync::Arc::clone(
            &context.generated_issues[&hold.stream()]
                .roster
                .source_identity,
        );
        let admitted = context.gfx942_adoption_unpublished_v1(&hold);
        assert!(!matches!(admitted, Ok(true)));
        assert!(context.retire_gfx942_unpublished_v1(&hold).is_err());
        assert_eq!(snapshot(&context, &plan), before);
        let attempt = &context.generated_issues[&hold.stream()];
        assert_eq!(attempt.phase, phase);
        assert!(std::sync::Arc::ptr_eq(
            &attempt.roster.source_identity,
            &source
        ));
        assert!(context.validate_unpublished_hold_v1(&hold).is_ok());
        assert!(!context.is_terminal());
    }
}

#[test]
fn unpublished_gate_preserves_original_journal_corruption_quarantine_without_disposal() {
    for retire in [false, true] {
        for index in 0..3 {
            let mut context = std::mem::ManuallyDrop::new(
                RuntimeContextV1::open_with_version_journal_v1(KfdRuntimeBackendV1::mock(), 6, 3)
                    .unwrap(),
            );
            let device = context.devices()[0].id();
            context
                .configure_allocation_admission_v1(device, 120, 6)
                .unwrap();
            let (hold, plan, roster) = install(&mut context);
            context
                .begin_generated_issue_v1(&hold, plan, &roster)
                .unwrap();
            context.install_generated_submission_v1(&hold, 900).unwrap();
            let attempt = context.generated_issues.get_mut(&hold.stream()).unwrap();
            attempt.phase = PhaseV1::Active;
            let writer = attempt.expected_writer.unwrap();
            let member = plan.members[index].unwrap();
            context
                .allocations
                .get_mut(&member.logical)
                .unwrap()
                .journal = None;
            let mut expected = snapshot(&context, &plan);
            let usage = expected.usage.as_mut().unwrap();
            assert_eq!(usage.retained_records, 3);
            usage.retained_records = 0;
            usage.quarantined_records = 3;

            let result = if retire {
                context.retire_gfx942_unpublished_v1(&hold).map(|()| true)
            } else {
                context.gfx942_adoption_unpublished_v1(&hold)
            };
            assert!(matches!(
                result,
                Err(RuntimeErrorV1::Validation(
                    RuntimeValidationErrorV1::InvalidBackendDescription
                ))
            ));
            assert!(context.is_terminal());
            assert_eq!(snapshot(&context, &plan), expected);
            assert_eq!(
                context
                    .versions
                    .as_ref()
                    .unwrap()
                    .journal_for_test()
                    .lookup_writer(writer)
                    .unwrap(),
                fe2o3_runtime_model::ContextWriterStateV1::Unknown { member_count: 3 }
            );
            let attempt = &context.generated_issues[&hold.stream()];
            assert_eq!(attempt.phase, PhaseV1::Active);
            assert_eq!(attempt.submission.as_ref().unwrap().backend_submission, 900);
            assert_eq!(context.streams[&hold.stream()].generated, Some(plan.key));
            assert_eq!(
                context.unpublished_identity_for_test_v1(hold.stream()),
                Some(Some(hold.identity()))
            );
            for member in plan.members[..plan.count].iter().flatten() {
                assert_eq!(
                    context.allocations[&member.logical].backend_allocation,
                    member.backend
                );
                assert!(context.backend_allocations.contains(&member.backend));
            }
            // The inert fixture intentionally retains the quarantined owners;
            // no terminal cleanup evidence is manufactured for test teardown.
        }
    }
}
