use super::tests::{LIMITS, SPACE, WORK, fixture, input, observe};
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, OperationKind as Op, ScalarType,
    StorageLayoutKindV1 as Layout, StorageLayoutV1, StorageOperationV1 as Storage, ValueId,
};

#[test]
fn fresh_structural_owner_cannot_hide_changed_storage_payload_or_effect_order() {
    for mutation in 0..3 {
        let mut module = fixture();
        module.storage_layouts.push(StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: Layout::Scalar(ScalarType::U32),
        });
        let input = input(&module);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(31 + input.storage).unwrap();
        let mut observed = observe(&input, &mut budget);
        let mut changed = observed.owner().module().clone();
        let operations = &mut changed.functions[0].body.as_mut().unwrap().blocks[0].operations;
        match mutation {
            0 => {
                let op = operations
                    .iter_mut()
                    .find(|op| matches!(op.kind, Op::Storage(Storage::WriteValue { .. })))
                    .unwrap();
                let Op::Storage(Storage::WriteValue { value, .. }) = &mut op.kind else {
                    unreachable!()
                };
                *value = ValueId(0);
            }
            1 => {
                let reads = operations
                    .iter()
                    .enumerate()
                    .filter_map(|(i, op)| {
                        matches!(op.kind, Op::Storage(Storage::ReadValue { .. })).then_some(i)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(reads.len(), 2);
                operations.swap(reads[0], reads[1]);
            }
            2 => changed.storage_layouts[2].kind = Layout::Scalar(ScalarType::I32),
            _ => unreachable!(),
        }
        let (foreign, foreign_storage) =
            Owner::from_module_ref_with_verification_budget_v18(&changed, LIMITS, &mut budget)
                .unwrap();
        budget
            .reserve_storage(foreign_storage.retained_storage())
            .unwrap();
        observed.parts.owner = foreign;
        let called = std::cell::Cell::new(false);
        let result = observed.try_check_and_finish_with_v18(&mut budget, |_, _| {
            called.set(true);
            Ok::<_, ()>(((), 0))
        });
        assert!(
            matches!(
                result,
                Err(KirCheckedNeutralOptimizationErrorV1::Transition(_))
            ),
            "mutation {mutation}"
        );
        assert!(!called.get());
        assert_eq!(
            budget.storage(),
            31 + input.storage + foreign_storage.retained_storage()
        );
    }
}

#[test]
fn same_typed_wrong_original_input_is_not_adopted() {
    let original = input(&fixture());
    let mut foreign = fixture();
    foreign.functions[0].body.as_mut().unwrap().blocks[0].operations[4] =
        super::tests::constant(3, 8);
    let foreign = input(&foreign);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget
        .reserve_storage(19 + original.storage + foreign.storage)
        .unwrap();
    let mut observed = observe(&original, &mut budget);
    observed.parts.input = &foreign.owner;
    assert!(matches!(
        observed.try_check_and_finish_v18(&mut budget),
        Err(KirCheckedNeutralOptimizationErrorV1::Transition(_))
    ));
    assert_eq!(budget.storage(), 19 + original.storage + foreign.storage);
}

#[test]
fn observed_output_cannot_reset_the_original_work_ledger_at_adoption() {
    let input = input(&fixture());
    let mut original_work = Work::new(WORK);
    let mut original_budget = Budget::new(&mut original_work, SPACE);
    original_budget.reserve_storage(input.storage).unwrap();
    let observed = observe(&input, &mut original_budget);
    let original_floor = original_budget.storage();
    let mut other_work = Work::new(WORK);
    let mut other_budget = Budget::new(&mut other_work, SPACE);
    other_budget.reserve_storage(original_floor).unwrap();
    assert!(matches!(
        observed.try_check_and_finish_v18(&mut other_budget),
        Err(KirCheckedNeutralOptimizationErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!(other_budget.storage(), original_floor);
    assert_eq!(other_budget.work(), 0);
    assert_eq!(original_budget.storage(), original_floor);
}
