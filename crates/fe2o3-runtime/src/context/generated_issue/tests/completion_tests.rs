use super::super::completion::assume_native_settlement_for_context_test_v1 as assumed_settlement;
use super::*;
use crate::{
    RuntimeGfx942GeneratedCompletionCarrierV1, RuntimeGfx942GeneratedCompletionViewV1,
    RuntimeGfx942GeneratedSourceV1, RuntimeGfx942ReadbackErrorV1,
};
use std::{cell::Cell, rc::Rc};

struct OmittingCarrier {
    calls: Rc<Cell<usize>>,
    drops: Rc<Cell<usize>>,
    destinations: Vec<Vec<u8>>,
}

impl Drop for OmittingCarrier {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

impl RuntimeGfx942GeneratedCarrierV1 for OmittingCarrier {
    type CurrentnessError = ();
    type Readback = ();
    fn source(&self) -> RuntimeGfx942GeneratedSourceV1<'_, ()> {
        panic!("inert omission fixture has no Worker or native authority")
    }
    fn prepare_readback(&self) -> Result<(), RuntimeGfx942ReadbackErrorV1> {
        panic!("fixture cannot reserve readback")
    }
    fn install_readback(&mut self, _: ()) {
        panic!("fixture cannot install readback")
    }
}

// SAFETY: adversarial correspondence fixture only, used to reject missing
// callbacks. It never creates a source/view, commits a gate or reaches native
// execution. Its deliberate false success must fail the production guard.
#[allow(unsafe_code)]
unsafe impl RuntimeGfx942GeneratedCompletionCarrierV1 for OmittingCarrier {
    fn completion_domain_v1(
        &self,
    ) -> Result<crate::RuntimeGeneratedResultDomainV1, RuntimeGfx942ReadbackErrorV1> {
        panic!("inert omission fixture has no result gate")
    }

    fn with_completion_view_v1(
        &mut self,
        _: impl for<'a> FnOnce(
            RuntimeGfx942GeneratedCompletionViewV1<'a, ()>,
        ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        self.calls.set(self.calls.get() + 1);
        Ok(())
    }
    fn complete_readback_v1(self) -> Result<(), RuntimeGfx942ReadbackErrorV1> {
        panic!("fixture must never decode")
    }
}

fn carrier(calls: &Rc<Cell<usize>>, drops: &Rc<Cell<usize>>) -> OmittingCarrier {
    OmittingCarrier {
        calls: calls.clone(),
        drops: drops.clone(),
        destinations: vec![vec![0x5a; 16], vec![0xa5; 20], vec![0x3c; 24]],
    }
}

#[test]
fn generated_completion_missing_callback_rejects_without_consuming_original_owner() {
    let calls = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let mut carrier = carrier(&calls, &drops);
    let pointers: Vec<_> = carrier.destinations.iter().map(Vec::as_ptr).collect();
    let result = completion::require_completion_view_v1(&mut carrier, |_| {
        panic!("omitted callback cannot settle completion")
    });
    assert!(matches!(
        result,
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidBackendDescription
        ))
    ));
    assert_eq!(calls.get(), 1);
    assert_eq!(drops.get(), 0);
    assert_eq!(
        carrier
            .destinations
            .iter()
            .map(Vec::as_ptr)
            .collect::<Vec<_>>(),
        pointers
    );
    drop(carrier);
    assert_eq!(drops.get(), 1);
}

fn installed_attempt(
    context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
    backend: u64,
) -> (
    ContextUnpublishedHoldV1,
    GeneratedShellPlanV1,
    GeneratedHostRosterV1,
) {
    let (hold, plan, roster) = install(context);
    context
        .begin_generated_issue_v1(&hold, plan, &roster)
        .unwrap();
    context
        .install_generated_submission_v1(&hold, backend)
        .unwrap();
    context
        .generated_issues
        .get_mut(&hold.stream())
        .unwrap()
        .phase = PhaseV1::PhysicallyComplete;
    (hold, plan, roster)
}

#[test]
fn generated_graph_context_settlement_releases_all_exact_metadata_before_reservation() {
    let mut context = context();
    let (hold, plan, roster, access) = install_with_graph(&mut context, true);
    let token = access.unwrap();
    context
        .begin_generated_issue_v1(&hold, plan, &roster)
        .unwrap();
    context.install_generated_submission_v1(&hold, 900).unwrap();
    context.close_graph_issue_v1(token).unwrap();
    assert_eq!(
        context.release_graph_v1(token),
        Err(RuntimeValidationErrorV1::SubmissionPending)
    );
    assert_eq!(context.allocations.len(), 3);
    // This is the shared Context bookkeeping tail, not a native settlement claim.
    context
        .generated_issues
        .get_mut(&hold.stream())
        .unwrap()
        .phase = PhaseV1::Unknown;
    let settled = assumed_settlement(&context, &hold);
    context
        .settle_completed_gfx942_context_v1(&hold, settled)
        .unwrap();
    assert!(context.allocations.is_empty());
    assert!(context.backend_allocations.is_empty());
    assert!(context.generated_issues.is_empty());
    assert!(context.submissions.is_empty());
    assert!(context.backend_submissions.is_empty());
    assert_eq!(
        context.unpublished_identity_for_test_v1(hold.stream()),
        Some(None)
    );
    assert!(context.streams[&hold.stream()].generated.is_none());
    context.release_graph_v1(token).unwrap();
    assert!(context.graph_reservation.is_none());
}

#[test]
fn generated_graph_invalid_access_precedes_native_and_carrier_callbacks() {
    for missing in [false, true] {
        let mut context = context();
        let (hold, _, roster, access) = install_with_graph(&mut context, true);
        let calls = Rc::new(Cell::new(0));
        let drops = Rc::new(Cell::new(0));
        let mut prepared = context.bound_preparation_for_test_v1(carrier(&calls, &drops));
        let mut foreign = RuntimeContextV1::open(KfdRuntimeBackendV1::mock()).unwrap();
        let foreign_token = foreign.reserve_graph_v1(1).unwrap();
        context.graph_reservation = if missing { None } else { Some(foreign_token) };
        let next = context.next_identity;
        assert!(matches!(
            context.progress_gfx942_issue_v1(&mut prepared, &roster, &hold),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        assert!(matches!(
            context.complete_gfx942_issue_v1(&mut prepared, &roster, &hold),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        assert_eq!(calls.get(), 0);
        assert_eq!(drops.get(), 0);
        assert!(!context.is_terminal());
        assert_eq!(context.next_identity, next);
        assert!(context.generated_issues.is_empty());
        assert_eq!(context.allocations.len(), 3);
        context.graph_reservation = access;
        context.retire_generated_shells_v1(&hold).unwrap();
        context.release_unpublished_hold_v1(&hold).unwrap();
        context.close_graph_issue_v1(access.unwrap()).unwrap();
        context.release_graph_v1(access.unwrap()).unwrap();
        foreign.close_graph_issue_v1(foreign_token).unwrap();
        foreign.release_graph_v1(foreign_token).unwrap();
    }
}

#[test]
fn generated_graph_release_rejects_unpaired_backend_indexes_and_callbacks() {
    for member in 0..3 {
        let mut context = context();
        let token = context.reserve_graph_v1(1).unwrap();
        context.close_graph_issue_v1(token).unwrap();
        match member {
            0 => {
                context.backend_submissions.insert(900);
            }
            1 => {
                context.backend_events.insert(901);
            }
            _ => context.completion_callback_count = 1,
        }
        assert_eq!(
            context.release_graph_v1(token),
            Err(RuntimeValidationErrorV1::SubmissionPending)
        );
        assert_eq!(context.graph_reservation, Some(token));
        context.backend_submissions.clear();
        context.backend_events.clear();
        context.completion_callback_count = 0;
        context.release_graph_v1(token).unwrap();
    }
}

#[test]
fn generated_graph_shared_drain_snapshot_excludes_only_registry_owned_submission() {
    let mut context = context();
    let (hold, _, _) = installed_attempt(&mut context, 900);
    let generated = context.generated_issues[&hold.stream()].id;
    let ordinary =
        RuntimeSubmissionIdV1::new(context.context_generation, context.next_id().unwrap());
    let mut record = context.submissions[&generated];
    record.backend_submission = 901;
    context.submissions.insert(ordinary, record);
    context.backend_submissions.insert(901);
    let (pending, _) = context.snapshot_async_drain_v1().unwrap();
    assert_eq!(pending.into_iter().collect::<Vec<_>>(), [ordinary]);
    assert_eq!(context.async_drain_counts_v1().pending, 2);
    assert_eq!(context.generated_issues[&hold.stream()].id, generated);
    // Deliberately constructed bookkeeping, not two real native submissions.
    core::mem::forget(context);
}

pub(super) fn assert_submission_retained(
    context: &RuntimeContextV1<KfdRuntimeBackendV1>,
    id: RuntimeSubmissionIdV1,
    before: SubmissionRecordV1,
) {
    let after = context.submissions[&id];
    assert_eq!(after.backend_submission, before.backend_submission);
    assert_eq!(after.stream, before.stream);
    assert_eq!(after.device, before.device);
    assert_eq!(after.quiescent, before.quiescent);
    assert_eq!(after.status, before.status);
    assert_eq!(after.journal_writer, before.journal_writer);
    assert_eq!(after.journal_read, before.journal_read);
}

#[test]
fn generated_completion_without_retained_device_preserves_all_pre_lending_custody() {
    let mut context = context();
    let (hold, plan, roster) = installed_attempt(&mut context, 900);
    let calls = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let mut prepared = context.bound_preparation_for_test_v1(carrier(&calls, &drops));
    let snapshot = |carrier: &OmittingCarrier| {
        carrier
            .destinations
            .iter()
            .map(|bytes| (bytes.clone(), bytes.as_ptr(), bytes.len(), bytes.capacity()))
            .collect::<Vec<_>>()
    };
    let destinations = snapshot(prepared.value());
    let before = context
        .allocation_admission_usage_v1(plan.binding.device)
        .unwrap();
    let id = context.generated_issues[&hold.stream()].id;
    let record = context.submissions[&id];
    assert_eq!(before.unwrap().retained_records, 3);
    assert_eq!(
        before.unwrap().used,
        crate::RuntimeResourceVectorV1::ZERO
            .with(crate::RuntimeResourceKindV1::RequestedAllocationBytes, 60)
            .with(crate::RuntimeResourceKindV1::AllocationRecords, 3)
    );
    let next = context.next_identity;
    assert!(
        context
            .complete_gfx942_issue_v1(&mut prepared, &roster, &hold)
            .is_err()
    );
    assert!(context.is_terminal());
    assert_eq!(
        calls.get(),
        0,
        "no checked device means no completion lending"
    );
    assert_eq!(drops.get(), 0);
    assert_eq!(context.next_identity, next);
    let attempt = &context.generated_issues[&hold.stream()];
    assert_eq!(attempt.phase, PhaseV1::Unknown);
    assert_eq!(attempt.id, id);
    assert_eq!(attempt.plan, plan);
    assert!(attempt.roster.matches(&roster));
    assert_eq!(attempt.submission.as_ref().unwrap().backend_submission, 900);
    assert_submission_retained(&context, id, record);
    assert_eq!(context.backend_submissions, [900].into_iter().collect());
    assert_eq!(context.allocations.len(), 3);
    for member in plan.members[..plan.count].iter().flatten() {
        assert_eq!(
            context.allocations[&member.logical].backend_allocation,
            member.backend
        );
        assert!(context.backend_allocations.contains(&member.backend));
    }
    assert_eq!(context.streams[&hold.stream()].generated, Some(plan.key));
    assert_eq!(
        context.unpublished_identity_for_test_v1(hold.stream()),
        Some(Some(hold.identity()))
    );
    let mut sealed_usage = before.unwrap();
    sealed_usage.retained_records = 0;
    sealed_usage.quarantined_records = 3;
    assert_eq!(
        context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap(),
        Some(sealed_usage)
    );
    assert_eq!(snapshot(prepared.value()), destinations);
    assert!(
        context
            .complete_gfx942_issue_v1(&mut prepared, &roster, &hold)
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(drops.get(), 0);
    assert_submission_retained(&context, id, record);
    assert_eq!(snapshot(prepared.value()), destinations);
    assert_eq!(
        context
            .allocation_admission_usage_v1(plan.binding.device)
            .unwrap(),
        Some(sealed_usage)
    );
    assert_eq!(
        context.unpublished_identity_for_test_v1(hold.stream()),
        Some(Some(hold.identity()))
    );
    // No native owner exists in this fixture. Preserve the terminal Context
    // exactly as production requires, then dispose only the inert test carrier.
    core::mem::forget(context);
    drop(prepared);
    assert_eq!(drops.get(), 1);
}

#[test]
fn generated_completion_context_tail_releases_only_exact_invocation() {
    let mut context = context();
    let (first, first_plan, first_roster) = installed_attempt(&mut context, 900);
    let (second, second_plan, second_roster) = installed_attempt(&mut context, 901);
    let first_id = context.generated_issues[&first.stream()].id;
    let second_id = context.generated_issues[&second.stream()].id;
    let first_record = context.submissions[&first_id];
    let second_record = context.submissions[&second_id];
    let before = context
        .allocation_admission_usage_v1(first_plan.binding.device)
        .unwrap();
    let notifications = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let observed = notifications.clone();
    let token = context
        .generated_issues
        .get_mut(&first.stream())
        .unwrap()
        .submission
        .take()
        .unwrap();
    context
        .on_completion(&token, move |status| observed.lock().unwrap().push(status))
        .unwrap();
    context
        .generated_issues
        .get_mut(&first.stream())
        .unwrap()
        .submission = Some(token);
    assert_eq!(context.completion_callback_count, 1);

    let unsettled = assumed_settlement(&context, &first);
    assert!(
        context
            .settle_completed_gfx942_context_v1(&first, unsettled)
            .is_err()
    );
    // This exercises actual post-native bookkeeping, not native completion.
    context
        .generated_issues
        .get_mut(&first.stream())
        .unwrap()
        .phase = PhaseV1::Unknown;
    let wrong = assumed_settlement(&context, &second);
    assert!(
        context
            .settle_completed_gfx942_context_v1(&first, wrong)
            .is_err()
    );
    assert_submission_retained(&context, first_id, first_record);
    assert_submission_retained(&context, second_id, second_record);
    assert_eq!(
        context.backend_submissions,
        [900, 901].into_iter().collect()
    );
    assert_eq!(context.allocations.len(), 6);
    assert_eq!(context.backend_allocations.len(), 6);
    for (hold, plan, roster) in [
        (&first, first_plan, &first_roster),
        (&second, second_plan, &second_roster),
    ] {
        let attempt = &context.generated_issues[&hold.stream()];
        assert_eq!(attempt.plan, plan);
        assert!(attempt.roster.matches(roster));
        for member in plan.members[..plan.count].iter().flatten() {
            let record = context.allocations[&member.logical];
            assert_eq!(record.backend_allocation, member.backend);
            assert_eq!(record.device, plan.binding.device);
            assert_eq!(record.kind, member.description.kind);
            assert_eq!(record.byte_len, member.description.byte_len);
            assert!(context.backend_allocations.contains(&member.backend));
        }
        assert_eq!(context.streams[&hold.stream()].generated, Some(plan.key));
        assert_eq!(
            context.unpublished_identity_for_test_v1(hold.stream()),
            Some(Some(hold.identity()))
        );
    }
    assert_eq!(
        context
            .allocation_admission_usage_v1(first_plan.binding.device)
            .unwrap(),
        before
    );
    assert!(notifications.lock().unwrap().is_empty());
    let settled = assumed_settlement(&context, &first);
    context
        .settle_completed_gfx942_context_v1(&first, settled)
        .unwrap();
    assert_eq!(
        *notifications.lock().unwrap(),
        [RuntimeCompletionStatusV1::Succeeded]
    );
    assert_eq!(context.completion_callback_count, 0);
    assert!(!context.completion_callbacks.contains_key(&first_id));
    assert!(!context.is_terminal());
    assert!(!context.submissions.contains_key(&first_id));
    assert_submission_retained(&context, second_id, second_record);
    assert_eq!(context.backend_submissions, [901].into_iter().collect());
    assert!(!context.generated_issues.contains_key(&first.stream()));
    assert_eq!(context.generated_issues[&second.stream()].plan, second_plan);
    assert!(
        context.generated_issues[&second.stream()]
            .roster
            .matches(&second_roster)
    );
    assert_eq!(
        context.generated_issues[&second.stream()].phase,
        PhaseV1::PhysicallyComplete
    );
    assert_eq!(
        context.generated_issues[&second.stream()]
            .submission
            .as_ref()
            .unwrap()
            .backend_submission,
        901
    );
    assert_eq!(context.allocations.len(), 3);
    assert_eq!(context.backend_allocations.len(), 3);
    for member in first_plan.members[..first_plan.count].iter().flatten() {
        assert!(!context.allocations.contains_key(&member.logical));
        assert!(!context.backend_allocations.contains(&member.backend));
    }
    for member in second_plan.members[..second_plan.count].iter().flatten() {
        assert_eq!(
            context.allocations[&member.logical].backend_allocation,
            member.backend
        );
        assert!(context.backend_allocations.contains(&member.backend));
    }
    let usage = context
        .allocation_admission_usage_v1(first_plan.binding.device)
        .unwrap()
        .unwrap();
    assert_eq!(usage.retained_records, 3);
    assert_eq!(
        usage.used,
        crate::RuntimeResourceVectorV1::ZERO
            .with(crate::RuntimeResourceKindV1::RequestedAllocationBytes, 60)
            .with(crate::RuntimeResourceKindV1::AllocationRecords, 3)
    );
    assert_eq!(context.streams[&first.stream()].generated, None);
    assert_eq!(
        context.unpublished_identity_for_test_v1(first.stream()),
        Some(None)
    );
    assert_eq!(
        context.streams[&second.stream()].generated,
        Some(second_plan.key)
    );
    assert_eq!(
        context.unpublished_identity_for_test_v1(second.stream()),
        Some(Some(second.identity()))
    );
    let other = assumed_settlement(&context, &second);
    assert!(
        context
            .settle_completed_gfx942_context_v1(&first, other)
            .is_err()
    );
    context
        .generated_issues
        .get_mut(&second.stream())
        .unwrap()
        .phase = PhaseV1::Unknown;
    let settled = assumed_settlement(&context, &second);
    context
        .settle_completed_gfx942_context_v1(&second, settled)
        .unwrap();
    assert_eq!(
        *notifications.lock().unwrap(),
        [RuntimeCompletionStatusV1::Succeeded]
    );
    assert!(context.submissions.is_empty());
    assert!(context.backend_submissions.is_empty());
    assert!(context.generated_issues.is_empty());
    assert!(context.allocations.is_empty());
    assert!(context.backend_allocations.is_empty());
    let usage = context
        .allocation_admission_usage_v1(first_plan.binding.device)
        .unwrap()
        .unwrap();
    assert_eq!(usage.retained_records, 0);
    assert_eq!(usage.used, crate::RuntimeResourceVectorV1::ZERO);
    context.destroy_stream(first.stream()).unwrap();
    context.destroy_stream(second.stream()).unwrap();
}
