use super::super::tests::{SPACE, WORK, fixture, input, observe};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn actual_checked_callback_sees_both_exact_owners_and_transfers_only_owned_lineage() {
    let input = input(&fixture());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(37 + input.storage).unwrap();
    let observed = observe(&input, &mut budget);
    let output_bytes = observed.owner().canonical_bytes().to_vec();
    let (checked, lineage, receipt) = observed
        .try_check_and_finish_with_v18(&mut budget, |relation, budget| {
            assert!(relation.input().belongs_to(&input.owner));
            assert_eq!(relation.output().owner().canonical_bytes(), output_bytes);
            assert!(!relation.grants_authority());
            let storage = size_of::<Vec<usize>>() + 2 * size_of::<usize>();
            budget.reserve_storage(storage).unwrap();
            let mut lineage = Vec::with_capacity(2);
            lineage.extend([
                relation.input().operations().len(),
                relation.output().operations().len(),
            ]);
            budget.release_storage(storage).unwrap();
            Ok::<_, Infallible>((lineage, storage))
        })
        .unwrap();
    assert!(lineage[0] > lineage[1]);
    assert_eq!(budget.storage(), 37 + input.storage);
    budget
        .reserve_storage(checked.storage().retained_storage())
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(checked.owner().canonical_bytes(), output_bytes);
    assert_eq!(checked.bridge().output, *checked.owner().identity());
    assert_eq!(checked.bridge().input, *input.owner.identity());
    assert!(!receipt.grants_authority());
}

#[test]
fn complete_nominal_table_remains_available_after_input_owner_drops() {
    let input = input(&fixture());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(13 + input.storage).unwrap();
    let observed = observe(&input, &mut budget);
    let checked = observed.try_check_and_finish_v18(&mut budget).unwrap();
    let bytes = input.owner.canonical_bytes().to_vec();
    let storage = input.storage;
    drop(input);
    budget.release_storage(storage).unwrap();
    assert_eq!(budget.storage(), 13);
    assert_eq!(checked.input_audit_bytes(), bytes);
    assert_eq!(checked.owner().module().storage_layouts.len(), 2);
    assert!(!checked.grants_authority());
}
