#![cfg(test)]

use super::*;
use std::cell::Cell;
use std::rc::Rc;

mod lineage;

// An ordinary drop-counted object tests the same private custody container. It
// is not a native DATA owner, device observation, or completion receipt.
struct Original {
    lineage: u64,
    drops: Rc<Cell<usize>>,
}

impl Drop for Original {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn retained_original_survives_refusal_and_moves_exactly_once() {
    let drops = Rc::new(Cell::new(0));
    let mut slot = RetainedDetachedV1::empty();
    assert!(slot.is_disposed_or_unentered());
    slot.capture(|| {
        Ok::<_, ()>(Original {
            lineage: 17,
            drops: drops.clone(),
        })
    })
    .unwrap();
    let address = slot.owner.as_ref().unwrap() as *const Original;
    assert!(slot.is_held());
    assert!(!slot.is_disposed_or_unentered());
    assert!(!slot.matches(|owner| owner.lineage == 18));
    assert!(slot.take_checked(|owner| owner.lineage == 18).is_none());
    assert_eq!(slot.owner.as_ref().unwrap() as *const Original, address);
    assert_eq!(drops.get(), 0);

    let original = slot.take_checked(|owner| owner.lineage == 17).unwrap();
    assert_eq!(original.lineage, 17);
    assert!(Rc::ptr_eq(&original.drops, &drops));
    assert!(slot.is_disposed_or_unentered());
    assert!(slot.take_checked(|_| true).is_none());
    assert_eq!(drops.get(), 0);
    drop(original);
    assert_eq!(drops.get(), 1);
}

#[test]
fn duplicate_capture_never_enters_lower_or_replaces_original() {
    let drops = Rc::new(Cell::new(0));
    let calls = Cell::new(0);
    let mut slot = RetainedDetachedV1::empty();
    slot.capture(|| {
        Ok::<_, ()>(Original {
            lineage: 3,
            drops: drops.clone(),
        })
    })
    .unwrap();
    assert_eq!(
        slot.capture(|| {
            calls.set(calls.get() + 1);
            Ok::<_, ()>(Original {
                lineage: 4,
                drops: drops.clone(),
            })
        }),
        Err(CaptureErrorV1::Occupied)
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(slot.owner.as_ref().unwrap().lineage, 3);
    assert_eq!(drops.get(), 0);
    drop(slot.take_checked(|_| true).unwrap());
    assert_eq!(drops.get(), 1);
    assert_eq!(
        slot.capture(|| {
            calls.set(calls.get() + 1);
            Ok::<_, ()>(Original {
                lineage: 5,
                drops: drops.clone(),
            })
        }),
        Err(CaptureErrorV1::Occupied)
    );
    assert_eq!(calls.get(), 0);
}

#[test]
fn lower_failure_is_not_empty_and_cannot_retry() {
    let mut slot = RetainedDetachedV1::<Original>::empty();
    assert_eq!(
        slot.capture(|| Err("original lower refusal")),
        Err(CaptureErrorV1::Lower("original lower refusal"))
    );
    assert_eq!(slot.phase, DetachedPhaseV1::InLower);
    assert!(!slot.is_disposed_or_unentered());
    assert!(!slot.is_held());
    let calls = Cell::new(0);
    assert_eq!(
        slot.capture(|| {
            calls.set(calls.get() + 1);
            Err("must not enter")
        }),
        Err(CaptureErrorV1::Occupied)
    );
    assert_eq!(calls.get(), 0);
    assert!(slot.take_checked(|_| true).is_none());
}

#[test]
fn lower_unwind_preserves_entered_state_without_retry() {
    let mut slot = RetainedDetachedV1::<Original>::empty();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = slot.capture::<()>(|| panic!("injected original lower unwind"));
    }));
    assert!(result.is_err());
    assert_eq!(slot.phase, DetachedPhaseV1::InLower);
    assert!(!slot.is_disposed_or_unentered());
    let calls = Cell::new(0);
    assert_eq!(
        slot.capture(|| {
            calls.set(calls.get() + 1);
            Err(())
        }),
        Err(CaptureErrorV1::Occupied)
    );
    assert_eq!(calls.get(), 0);
}

#[test]
fn checked_take_unwind_keeps_the_exact_original_rooted() {
    let drops = Rc::new(Cell::new(0));
    let mut slot = RetainedDetachedV1::empty();
    slot.capture(|| {
        Ok::<_, ()>(Original {
            lineage: 29,
            drops: drops.clone(),
        })
    })
    .unwrap();
    let address = slot.owner.as_ref().unwrap() as *const Original;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = slot.take_checked(|_| panic!("injected identity check unwind"));
    }));
    assert!(result.is_err());
    assert!(slot.is_held());
    assert_eq!(slot.owner.as_ref().unwrap() as *const Original, address);
    assert_eq!(drops.get(), 0);
    drop(slot.take_checked(|owner| owner.lineage == 29).unwrap());
    assert_eq!(drops.get(), 1);
}

fn metadata_native(phase: PhaseV1) -> GeneratedNativeAdoptionV1 {
    // Refusal-only fixture: no original native queue, lane or DATA is fabricated.
    GeneratedNativeAdoptionV1 {
        phase,
        lane: 0,
        native_lane: None,
        data: Vec::new(),
        detached: RetainedDetachedV1::empty(),
        returned: ReturnedDataV1::empty(),
        submission: None,
    }
}

#[test]
fn detached_phase_retains_submission_and_blocks_ordinary_issue_or_disposal() {
    let (mut backend, plan, roster) = super::super::tests::shells_with_roster();
    let mut native = metadata_native(PhaseV1::Detached);
    native.submission = Some(issue::GeneratedSubmissionV1 {
        id: 100,
        roster,
        receipt: ReceiptV1::Recycled.into(),
    });
    backend.generated_shells.get_mut(&plan.key).unwrap().native = Some(native);
    backend.generated_submissions.insert(100, plan.key);
    assert!(backend.has_live_generated_native_v1());
    assert!(backend.generated_submission_owner_matches_v1(100, &plan));
    assert!(matches!(
        backend.generated_submission_plan_v1(100),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert!(matches!(
        backend.release_submission_v1(100),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert!(matches!(
        backend.retain_generated_completed_data_v1(&plan, 100),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert!(!backend.validate_generated_shell_disposal_v1(&plan));
    assert_eq!(backend.generated_submissions.get(&100), Some(&plan.key));
    let native = backend.generated_shells[&plan.key].native.as_ref().unwrap();
    assert_eq!(native.phase, PhaseV1::Detached);
    assert_eq!(
        native.submission.as_ref().unwrap().receipt.retirement(),
        Some(RetirementV1::Recycled)
    );
    assert!(!native.is_retired());
    assert!(!backend.terminal);
    assert!(backend.queue.is_none());
    // Dispose only the fixture's handle-free metadata, not a native owner.
    backend.generated_shells.get_mut(&plan.key).unwrap().native = None;
    backend.generated_submissions.clear();
    backend.dispose_generated_shells_v1(&plan);
}

#[test]
fn fabricated_receipt_or_wrong_submission_never_enters_native_detach() {
    for receipt in [
        NativeReceiptV1::from(ReceiptV1::Ready),
        NativeReceiptV1::from(ReceiptV1::Recycled),
        NativeReceiptV1::Cohort3(ReceiptV1::Recycled),
    ] {
        let (mut backend, plan, roster) = super::super::tests::shells_with_roster();
        let profile = receipt.profile();
        let retirement = receipt.retirement();
        let ready = receipt.issue_ready();
        let mut native = metadata_native(PhaseV1::Adopted);
        native.submission = Some(issue::GeneratedSubmissionV1 {
            id: 100,
            roster,
            receipt,
        });
        backend.generated_shells.get_mut(&plan.key).unwrap().native = Some(native);
        backend.generated_submissions.insert(100, plan.key);
        assert_eq!(
            backend.generated_submission_owner_matches_v1(100, &plan),
            profile == plan.profile
        );
        for id in [0, 99, 100, 101] {
            assert!(matches!(
                backend.retain_generated_completed_data_v1(&plan, id),
                Err(RuntimeBackendFailureV1::Rejected(_))
            ));
            let native = backend.generated_shells[&plan.key].native.as_ref().unwrap();
            assert_eq!(native.phase, PhaseV1::Adopted);
            assert_eq!(native.detached.phase, DetachedPhaseV1::Empty);
            assert_eq!(native.submission.as_ref().unwrap().id, 100);
            let receipt = &native.submission.as_ref().unwrap().receipt;
            assert_eq!(receipt.profile(), profile);
            assert_eq!(receipt.retirement(), retirement);
            assert_eq!(receipt.issue_ready(), ready);
            assert_eq!(backend.generated_submissions.get(&100), Some(&plan.key));
            assert!(!backend.terminal);
            assert!(backend.queue.is_none());
        }
        backend.generated_shells.get_mut(&plan.key).unwrap().native = None;
        backend.generated_submissions.clear();
        backend.dispose_generated_shells_v1(&plan);
    }
}

#[test]
fn entered_detach_cannot_be_hidden_by_retired_metadata() {
    let mut native = metadata_native(PhaseV1::Retired);
    native.returned.install(Vec::new());
    assert!(native.is_retired());
    native.detached.phase = DetachedPhaseV1::InLower;
    assert!(!native.is_retired());
    assert_eq!(native.disposed_count(), None);
}

#[test]
fn enclosing_native_error_keeps_detached_metadata_terminal() {
    let (mut backend, plan) = super::super::tests::shells();
    backend.generated_shells.get_mut(&plan.key).unwrap().native =
        Some(metadata_native(PhaseV1::Detached));
    let error = KfdRuntimeBackendV1::rejected(
        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
        "injected closing detach identity refusal",
    );
    assert!(matches!(
        backend.finish_generated_native_call_v1(Ok(Err(error))),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(backend.terminal);
    assert_eq!(
        backend.generated_shells[&plan.key]
            .native
            .as_ref()
            .unwrap()
            .phase,
        PhaseV1::Detached
    );
    assert!(backend.has_live_generated_native_v1());
    assert!(!backend.validate_generated_shell_disposal_v1(&plan));
    assert!(matches!(
        backend.retain_generated_completed_data_v1(&plan, 100),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    // Match the established terminal fixture policy: do not clear quarantine
    // merely to permit Drop. This fixture owns no native lease or DATA.
    core::mem::forget(backend);
}

#[test]
fn native_detached_slot_is_inline_and_bounded() {
    type Owner = fe2o3_kfd::Gfx942DetachedFixedDispatchV1;
    assert!(
        core::mem::size_of::<RetainedDetachedV1<Owner>>()
            <= core::mem::size_of::<Option<Owner>>()
                + core::mem::size_of::<Option<ProducerLineageV1>>()
                + 2 * core::mem::size_of::<usize>()
    );
    assert!(core::mem::size_of::<RetainedDetachedV1<Owner>>() <= 16 * 1024);
}
