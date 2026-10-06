use super::*;

#[test]
fn mixed_private_spill_scope_requires_both_actual_pointer_endpoints_to_be_global() {
    let pointer = |space| {
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            space,
            fe2o3_kernel_ir::AccessMode::ReadWrite,
        )
    };
    let types = [
        pointer(AddressSpace::Global),
        pointer(AddressSpace::Private),
        pointer(AddressSpace::Generic),
        pointer(AddressSpace::Workgroup),
        pointer(AddressSpace::Constant),
        Type::Scalar(ScalarType::U32),
    ];
    for (before_index, before) in types.iter().enumerate() {
        for (after_index, after) in types.iter().enumerate() {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(4);
            let mut budget = ArgumentBudgetV1::new(&mut work, 17);
            budget.reserve_storage(17).unwrap();
            let result = source_private_spill_global_pair_v26(before, after, &mut budget);
            match (before_index == 0, after_index == 0) {
                (true, true) => assert!(matches!(result, Ok(true))),
                (false, false) => assert!(matches!(result, Ok(false))),
                _ => assert!(matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "private spill scope changed Global pointer classification"
                    ))
                )),
            }
            assert_eq!(budget.work(), 4);
            assert_eq!(budget.storage(), 17);
        }
    }
}

#[test]
fn mixed_private_spill_scope_work_refusal_does_not_classify_or_change_storage() {
    let pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        fe2o3_kernel_ir::AccessMode::ReadOnly,
    );
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(3);
    let mut budget = ArgumentBudgetV1::new(&mut work, 17);
    budget.reserve_storage(17).unwrap();
    assert!(matches!(
        source_private_spill_global_pair_v26(&pointer, &pointer, &mut budget),
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Work(_)
        ))
    ));
    assert!(budget.failed_work().is_some());
    assert_eq!(budget.storage(), 17);
}
