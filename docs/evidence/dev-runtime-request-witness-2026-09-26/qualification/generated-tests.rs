use super::*;
use crate::RuntimeAllocationDeviceAdmissionV1 as Entry;

fn composed_fixture() -> (Fixture, Entry) {
    let root = Entry::qualification_root_v1();
    let entry = Entry::qualification_entry_v1(&root, 7);
    let mut fixture = Fixture::new_with_journal(None, Some(8));
    fixture
        .context
        .backend
        .qualification_install_binding_v1(entry.clone());
    fixture.context.allocation_admission = ContextAllocationAdmissionV1::from_profile(
        fixture.context.devices(),
        RuntimeAllocationAdmissionProfileV1::Required(vec![entry.clone()]),
    )
    .unwrap();
    (fixture, entry)
}

#[test]
fn qualification_generated_install_retire_has_exact_request_custody() {
    let (mut fixture, entry) = composed_fixture();
    fixture.install().unwrap();
    let usage = entry.account().usage_v1();
    assert_eq!(usage.retained_records, fixture.roster.count);
    assert_eq!(
        usage
            .used
            .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
        60
    );
    for _ in 0..3 {
        let plan = fixture
            .context
            .generated_plan_for_hold_v1(&fixture.hold)
            .unwrap();
        assert_eq!(plan.count, fixture.roster.count);
        assert_eq!(entry.account().usage_v1(), usage);
    }
    fixture.retire();
    assert_eq!(
        entry.account().usage_v1().used,
        crate::RuntimeResourceVectorV1::ZERO
    );
    assert!(fixture.context.cleanup().is_complete());
}

#[test]
fn qualification_generated_bind_rejection_refunds_before_metadata_or_source_transfer() {
    let (mut fixture, entry) = composed_fixture();
    let mismatch = Entry::qualification_v1(7, admission(2, 1).1, entry.account().clone());
    fixture
        .context
        .backend
        .qualification_install_binding_v1(mismatch);
    let before = fixture.snapshot();
    assert!(fixture.install().is_err());
    assert_eq!(fixture.snapshot(), before);
    assert!(fixture.storage.control_available());
    assert_eq!(
        entry.account().usage_v1().used,
        crate::RuntimeResourceVectorV1::ZERO
    );
    fixture
        .context
        .release_unpublished_hold_v1(&fixture.hold)
        .unwrap();
    assert!(fixture.context.cleanup().is_complete());
}

#[test]
fn qualification_generated_journal_rejection_after_bind_refunds_unissued_requests() {
    let (mut fixture, entry) = composed_fixture();
    let id = RuntimeAllocationIdV1::new(
        fixture.context.context_generation,
        fixture.context.next_identity,
    );
    let enrollment = fixture
        .context
        .enroll_journal_allocation_v1(id, fixture.device, 20)
        .unwrap();
    fixture.context.commit_journal_allocation_v1(enrollment);
    assert!(fixture.install().is_err());
    assert!(fixture.context.allocations.is_empty());
    assert!(fixture.storage.control_available());
    assert_eq!(
        entry.account().usage_v1().used,
        crate::RuntimeResourceVectorV1::ZERO
    );
}

#[test]
fn qualification_generated_journal_unwind_quarantines_entire_uncommitted_roster() {
    let (mut fixture, entry) = composed_fixture();
    fixture
        .context
        .versions
        .as_mut()
        .unwrap()
        .qualification_corrupt_phase_storage_v1();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fixture.install())).is_err());
    assert!(fixture.context.is_terminal());
    assert!(fixture.context.allocations.is_empty());
    assert!(fixture.storage.control_available());
    let usage = entry.account().usage_v1();
    assert_eq!(usage.quarantined_records, fixture.roster.count);
    assert_eq!(usage.reserved_records, 0);
    assert_eq!(usage.retained_records, 0);
    assert_eq!(
        usage
            .used
            .get(crate::RuntimeResourceKindV1::RequestedAllocationBytes),
        60
    );
    assert!(!fixture.context.cleanup().is_complete());
}

#[test]
fn qualification_generated_pre_adoption_and_retirement_reject_swapped_request_extents() {
    let (mut fixture, entry) = composed_fixture();
    fixture.install().unwrap();
    let plan = fixture
        .context
        .generated_plan_for_hold_v1(&fixture.hold)
        .unwrap();
    let first = plan.members[0].unwrap().logical;
    let second = plan.members[1].unwrap().logical;
    assert_ne!(
        plan.members[0].unwrap().description.byte_len,
        plan.members[1].unwrap().description.byte_len
    );
    let usage = entry.account().usage_v1();
    let before = fixture.snapshot();
    fixture
        .context
        .allocation_admission
        .swap_retained_for_test_v1(first, second);
    assert!(matches!(
        fixture.context.generated_plan_for_hold_v1(&fixture.hold),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidBackendDescription
        ))
    ));
    assert!(matches!(
        fixture.context.retire_generated_shells_v1(&fixture.hold),
        Err(RuntimeErrorV1::Validation(
            RuntimeValidationErrorV1::InvalidBackendDescription
        ))
    ));
    assert_eq!(entry.account().usage_v1(), usage);
    assert_eq!(fixture.snapshot(), before);
    fixture
        .context
        .allocation_admission
        .swap_retained_for_test_v1(first, second);
    fixture.retire();
}
