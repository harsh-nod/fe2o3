//! Genuine checked native owners; only spare audit capacity is perturbed.
use super::super::optimize_native_neutral_kernel_ir_integer_continuation_v1;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    LogicalStorageLimitsV1 as Limits, Module, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
fn with_native(
    body: impl FnOnce(&Owner, &mut CheckedNeutralKernelIrOwnerIntegerContinuationV1, &mut Budget<'_>),
) {
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 256 * 1024 * 1024);
    let (input, storage) = Owner::from_module_ref_with_verification_budget_v12(
        &Module::new("retained-native"),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let observed =
        optimize_native_neutral_kernel_ir_integer_continuation_v1(&input, &mut budget).unwrap();
    let observed_storage = observed.storage().retained_storage();
    budget.reserve_storage(observed_storage).unwrap();
    let mut owner = observed.try_check_and_finish_v1(&mut budget).unwrap();
    // Consuming adoption already restores the floor before the observed owner.
    let retained = owner.storage().retained_storage();
    budget.reserve_storage(retained).unwrap();
    body(&input, &mut owner, &mut budget);
    drop(owner);
    budget.release_storage(retained).unwrap();
    drop(input);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}
fn observe(owner: &CheckedNeutralKernelIrOwnerIntegerContinuationV1) -> (usize, usize) {
    let mut counter = Counter::new(Limits {
        max_bytes: None,
        max_items: 100_000,
    });
    owner.charge_retained_heap_v11(&mut counter).unwrap();
    (counter.bytes(), counter.items())
}

#[test]
fn genuine_native_audit_copy_uses_actual_capacity_and_no_borrowed_input() {
    with_native(|input, owner, budget| {
        assert_eq!(
            owner.native_input_audit_bytes(),
            input.canonical().canonical_bytes()
        );
        let original = owner.native_input_audit_bytes().to_vec();
        let retained = owner.storage().retained_storage();
        let before = observe(owner);
        let capacity = owner.parts.input_history.capacity();
        owner.parts.input_history.reserve_exact(capacity + 23);
        let after_capacity = owner.parts.input_history.capacity();
        assert!(after_capacity > capacity);
        let budget_before = (budget.work(), budget.storage(), budget.peak_storage());
        let after = observe(owner);
        assert_eq!(after, (before.0 + after_capacity - capacity, before.1));
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            budget_before
        );
        assert_eq!(owner.native_input_audit_bytes(), original);
        assert_eq!(owner.storage().retained_storage(), retained);
    });
}

#[test]
fn genuine_native_complete_fields_are_composed_without_headers_or_receipt_values() {
    with_native(|_, owner, _| {
        let parts = &owner.parts;
        let mut children = Counter::new(Limits {
            max_bytes: None,
            max_items: 100_000,
        });
        parts.owner.charge_retained_heap_v11(&mut children).unwrap();
        parts
            .report
            .charge_retained_heap_storage_v1(&mut children)
            .unwrap();
        parts
            .bridge
            .charge_retained_heap_storage_v1(&mut children)
            .unwrap();
        parts
            .map
            .charge_retained_heap_storage_v1(&mut children)
            .unwrap();
        parts
            .occurrences
            .charge_retained_heap_storage_v1(&mut children)
            .unwrap();
        let expected = (
            children.bytes() + parts.input_history.capacity(),
            children.items() + 1,
        );
        assert_eq!(observe(owner), expected);
        let mut exact = Counter::new(Limits {
            max_bytes: Some(expected.0),
            max_items: expected.1,
        });
        owner.charge_retained_heap_v11(&mut exact).unwrap();
        for (limits, error) in [
            (
                Limits {
                    max_bytes: Some(expected.0 - 1),
                    max_items: expected.1,
                },
                Error::ByteLimit,
            ),
            (
                Limits {
                    max_bytes: Some(expected.0),
                    max_items: expected.1 - 1,
                },
                Error::ItemLimit,
            ),
            (
                Limits {
                    max_bytes: None,
                    max_items: 0,
                },
                Error::ItemLimit,
            ),
        ] {
            let mut counter = Counter::new(limits);
            assert_eq!(owner.charge_retained_heap_v11(&mut counter), Err(error));
        }
    });
}
