//! Same-owner inert decode is independent and consumes the existing ledger.
use super::*;
use crate::{
    CanonicalKernelIrWorkBudgetV1 as Work, gfx942_physical_entry_fixture_v20_tests as fixture,
};
type Budget<'a> = CanonicalKernelIrVerificationResourceBudgetV1<'a>;
fn owner() -> VerifiedCanonicalKernelIrModuleV20 {
    let mut work = Work::new(16_000_000);
    VerifiedCanonicalKernelIrModuleV20::from_module_ref_with_verification_budget_v20(
        &fixture::module(true),
        &mut Budget::new(&mut work, 16_000_000),
    )
    .unwrap()
    .0
}
#[test]
fn inert_view_is_not_mutable_owner_custody() {
    let owner = owner();
    let original = owner.canonical_bytes().to_vec();
    let mut work = Work::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    budget.reserve_storage(73).unwrap();
    budget.charge_work(29).unwrap();
    let (mut view, receipt) = owner
        .decoded_inert_view_with_verification_budget_v20(&mut budget)
        .unwrap();
    assert_eq!(&view, owner.module());
    assert_ne!(view.functions.as_ptr(), owner.module().functions.as_ptr());
    assert!(receipt.retained_storage() > std::mem::size_of::<Module>());
    assert_eq!(budget.storage(), 73);
    assert!(budget.work() > 29);
    view.functions.clear();
    assert!(!owner.module().functions.is_empty());
    assert_eq!(owner.canonical_bytes(), original);
}
#[test]
fn exact_and_one_short_reader_resources_restore_nonzero_floor() {
    let owner = owner();
    let run = |wl, sl| {
        let mut work = Work::new(wl);
        let mut b = Budget::new(&mut work, sl);
        b.reserve_storage(73).unwrap();
        b.charge_work(29).unwrap();
        let ok = owner
            .decoded_inert_view_with_verification_budget_v20(&mut b)
            .is_ok();
        assert_eq!(b.storage(), 73);
        let accepted = b.work();
        let peak = b.peak_storage();
        let failed_storage = b.failed_storage();
        (ok, accepted, peak, work.failed_work(), failed_storage)
    };
    let (ok, work, peak, _, _) = run(16_000_000, 16_000_000);
    assert!(ok);
    assert!(run(work, peak).0);
    let short_work = run(work - 1, peak);
    assert!(!short_work.0);
    assert!(short_work.3.is_some());
    let short_storage = run(work, peak - 1);
    assert!(!short_storage.0);
    assert!(short_storage.4.is_some());
}
#[test]
fn inert_reader_refuses_another_revision_in_private_corruption_control() {
    let mut owner = owner();
    owner.canonical.canonical_bytes[8..10].copy_from_slice(&21u16.to_le_bytes());
    let mut work = Work::new(16_000_000);
    let mut b = Budget::new(&mut work, 16_000_000);
    b.reserve_storage(73).unwrap();
    assert!(
        owner
            .decoded_inert_view_with_verification_budget_v20(&mut b)
            .is_err()
    );
    assert_eq!(b.storage(), 73);
}
