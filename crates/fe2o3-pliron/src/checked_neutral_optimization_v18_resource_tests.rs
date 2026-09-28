use super::super::tests::{SPACE, WORK, input, observe};
use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, Module};
use std::panic::AssertUnwindSafe;

fn empty_headers() -> usize {
    type Payload = Box<dyn std::any::Any + Send>;
    type Outcome =
        Result<(Checked, (), KirNeutralOwnedOriginStorageV1), KirCheckedNeutralOptimizationErrorV1>;
    // Same owned/caught bindings that coexist at this boundary. The origin
    // callback is a zero-sized function item; no payload grows with graph size.
    2 * size_of::<Observed<'_>>()
        + 2 * size_of::<Checked>()
        + 8 * size_of::<Outcome>()
        + 2 * size_of::<std::thread::Result<Outcome>>()
        + 4 * size_of::<Payload>()
        + size_of::<AssertUnwindSafe<Payload>>()
        + 2 * size_of::<std::thread::Result<()>>()
        + size_of::<AdoptionProfile<Owner>>()
        + 2 * size_of::<Result<((), usize), Infallible>>()
        + 4 * size_of::<usize>()
        + size_of::<Option<usize>>() // Active callback floor survives unwind.
}

fn unit(
    _: &CheckedCanonicalKirTransitionV18<'_, '_, '_, '_>,
    _: &mut Budget<'_>,
) -> Result<((), usize), Infallible> {
    Ok(((), 0))
}

#[test]
fn empty_adoption_has_independent_exact_and_one_short_work_boundaries() {
    // V18 empty "x" has 41 canonical bytes. Cleanup=5, entry=1, both
    // inventories=2+2, complete table + scalar transition=8, history=41+2.
    const REQUIRED: usize = 61;
    for allowance in [REQUIRED, REQUIRED - 1] {
        let input = input(&Module::new("x"));
        assert_eq!(input.owner.canonical_bytes().len(), 41);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(17 + input.storage).unwrap();
        let observed = observe(&input, &mut budget);
        budget
            .charge_work(WORK - budget.work() - allowance)
            .unwrap();
        let start = budget.work();
        let result = observed.try_check_and_finish_with_v18(&mut budget, unit);
        if allowance == REQUIRED {
            result.unwrap();
            assert_eq!(budget.work(), start + REQUIRED);
        } else {
            assert!(matches!(
                result,
                Err(KirCheckedNeutralOptimizationErrorV1::Resource(
                    Resource::Work(_)
                ))
            ));
            assert_eq!(budget.work(), start + REQUIRED - 2);
        }
        assert_eq!(budget.storage(), 17 + input.storage);
    }
}

#[test]
fn independent_empty_coexistence_is_paid_before_the_source_callback() {
    let input = input(&Module::new("x"));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(29 + input.storage).unwrap();
    let observed = observe(&input, &mut budget);
    let incoming = budget.storage();
    let expected = incoming
        + empty_headers()
        + 2 * size_of::<CanonicalKirInventoryV18<'_>>()
        + size_of::<CheckedCanonicalKirTransitionV18<'_, '_, '_, '_>>()
        + size_of::<usize>(); // Captured expected-floor value in this callback.
    observed
        .try_check_and_finish_with_v18(&mut budget, move |_, budget| {
            assert_eq!(budget.storage(), expected);
            Ok::<_, Infallible>(((), 0))
        })
        .unwrap();
    assert_eq!(budget.storage(), 29 + input.storage);
}

#[test]
fn independent_header_storage_denial_preserves_unrelated_caller_reservations() {
    for allowance in [empty_headers() - 1, empty_headers()] {
        let input = input(&Module::new("x"));
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(31 + input.storage).unwrap();
        let observed = observe(&input, &mut budget);
        let consumed = observed.storage().retained_storage();
        budget
            .reserve_storage(SPACE - budget.storage() - allowance)
            .unwrap();
        let floor = budget.storage() - consumed;
        let before = budget.work();
        let result = observed.try_check_and_finish_with_v18(&mut budget, unit);
        assert!(result.is_err());
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            budget.work() - before,
            if allowance < empty_headers() { 5 } else { 7 }
        );
    }
}
