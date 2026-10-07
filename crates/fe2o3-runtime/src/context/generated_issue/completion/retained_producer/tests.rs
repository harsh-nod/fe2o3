//! Context metadata controls, never a substitute for native detach/currentness.

use super::*;
use crate::context::generated_issue::tests::{context, install_with_graph};

fn original(
    context: &RuntimeContextV1<KfdRuntimeBackendV1>,
    hold: &ContextUnpublishedHoldV1,
) -> RetainedProducerV1 {
    let attempt = &context.generated_issues[&hold.stream()];
    RetainedProducerV1 {
        submission: attempt.id,
        backend_submission: attempt.submission.as_ref().unwrap().backend_submission,
        stream: hold.stream(),
        hold: hold.identity(),
        expected_writer: attempt.expected_writer,
        expected_reader: attempt.expected_reader,
    }
}

fn finish_metadata_fixture(
    context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
    hold: &ContextUnpublishedHoldV1,
) {
    context
        .generated_issues
        .get_mut(&hold.stream())
        .unwrap()
        .phase = PhaseV1::Unknown;
    let settled = super::super::assume_native_settlement_for_context_test_v1(context, hold);
    context
        .settle_completed_gfx942_context_v1(hold, settled)
        .unwrap();
}

#[test]
fn retained_producer_context_keeps_all_original_credits_and_graph_hold() {
    let mut context = context();
    let (hold, plan, roster, graph) = install_with_graph(&mut context, true);
    context
        .begin_generated_issue_v1(&hold, plan, &roster)
        .unwrap();
    context.install_generated_submission_v1(&hold, 900).unwrap();
    context
        .generated_issues
        .get_mut(&hold.stream())
        .unwrap()
        .phase = PhaseV1::Unknown;
    let id = context.generated_issues[&hold.stream()].id;
    let status = context.submissions[&id].status;
    let usage = context
        .allocation_admission_usage_v1(plan.binding.device)
        .unwrap();
    let next = context.next_identity;
    let retained = original(&context, &hold);
    context
        .retain_completed_gfx942_context_v1(&hold, retained)
        .unwrap();
    assert_eq!(
        context.generated_issues[&hold.stream()].phase,
        PhaseV1::RetainedProducer
    );
    assert!(
        context.generated_issues[&hold.stream()]
            .roster
            .matches(&roster)
    );
    assert_eq!(context.submissions[&id].status, status);
    assert_eq!(context.next_identity, next);
    assert_eq!(
        context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap(),
        usage
    );
    assert_eq!(
        (context.allocations.len(), context.backend_allocations.len()),
        (3, 3)
    );
    assert_eq!(
        (context.submissions.len(), context.backend_submissions.len()),
        (1, 1)
    );
    assert_eq!(context.graph_reservation, graph);
    assert_eq!(
        context.unpublished_identity_for_test_v1(hold.stream()),
        Some(Some(hold.identity()))
    );
    context.close_graph_issue_v1(graph.unwrap()).unwrap();
    assert_eq!(
        context.release_graph_v1(graph.unwrap()),
        Err(RuntimeValidationErrorV1::SubmissionPending)
    );
    let duplicate = original(&context, &hold);
    assert!(
        context
            .retain_completed_gfx942_context_v1(&hold, duplicate)
            .is_err()
    );
    assert_eq!(
        context.generated_issues[&hold.stream()].phase,
        PhaseV1::RetainedProducer
    );
    finish_metadata_fixture(&mut context, &hold);
    context.release_graph_v1(graph.unwrap()).unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn retained_producer_context_refuses_wrong_original_identity_without_releasing() {
    for fault in 0..5 {
        let mut context = context();
        let (hold, plan, roster, _) = install_with_graph(&mut context, false);
        let other = context.create_stream(plan.binding.device).unwrap();
        context
            .begin_generated_issue_v1(&hold, plan, &roster)
            .unwrap();
        context.install_generated_submission_v1(&hold, 900).unwrap();
        context
            .generated_issues
            .get_mut(&hold.stream())
            .unwrap()
            .phase = PhaseV1::Unknown;
        let usage = context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap();
        let mut retained = original(&context, &hold);
        match fault {
            0 => retained.backend_submission += 1,
            1 => retained.hold += 1,
            2 => retained.stream = other,
            3 => {
                retained.submission =
                    RuntimeSubmissionIdV1::new(context.context_generation, context.next_identity)
            }
            _ => {
                context
                    .generated_issues
                    .get_mut(&hold.stream())
                    .unwrap()
                    .phase = PhaseV1::PhysicallyComplete
            }
        }
        assert!(
            context
                .retain_completed_gfx942_context_v1(&hold, retained)
                .is_err()
        );
        assert_eq!(
            context
                .allocation_admission_usage_v1(plan.binding.device)
                .unwrap(),
            usage
        );
        assert_eq!(
            (context.allocations.len(), context.submissions.len()),
            (3, 1)
        );
        assert!(context.validate_unpublished_hold_v1(&hold).is_ok());
        finish_metadata_fixture(&mut context, &hold);
        assert!(context.cleanup().is_complete());
    }
}
