use super::*;

fn source_call(
    owner: &ProductionPreRankedKirOwnerV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
) -> &SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) = owner.semantic_ssa().source_semantic().functions()
        [caller.index() as usize]
        .blocks()[block.index() as usize]
        .terminator()
        .kind()
    else {
        panic!("genuine source direct call")
    };
    call
}

fn with_inventory(
    owner: &ProductionPreRankedKirOwnerV1,
    next: impl FnOnce(&CanonicalKirInventoryV1<'_>, usize),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    let retained = owner.unit_local_source_storage_floor_v1().unwrap();
    budget.reserve_storage(FLOOR + retained).unwrap();
    let (inventory, receipt) =
        CanonicalKirInventoryV1::derive(owner.executable(), &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    next(&inventory, floor);
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    budget.release_storage(receipt.retained_storage()).unwrap();
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn ranked_unit_call_query_keeps_both_roots_and_repeated_calls_distinct() {
    let owner = unit_owner(UnitCase::ReadThenWrite, &[2, 2]);
    assert_eq!(owner.empty_effect_helpers().iter().count(), 0);
    with_inventory(&owner, |inventory, floor| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        owner
            .with_checked_unit_local_source_v1(inventory, &mut budget, |view, budget| {
                for row in &owner.helper_memory.unit_source.calls {
                    let call = source_call(&owner, row.caller.function, row.source_block);
                    let proof = view
                        .bounds_neutral_call_v1(
                            row.caller.root,
                            row.caller.function,
                            row.source_block,
                            call,
                            budget,
                        )?
                        .unwrap();
                    assert_eq!(proof.root(), row.caller.root);
                    assert_eq!(proof.caller(), row.caller.function);
                    assert_eq!(proof.callee(), SemanticFunctionIdV1::from_index(2));
                    assert_eq!(proof.source_block(), row.source_block);
                    assert_eq!(proof.native_call(), row.call);
                    assert!(std::ptr::eq(proof.source_call(), call));
                    let operation = &owner.executable().module().functions[row.caller.physical]
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks[row.call.block.block as usize]
                        .operations[row.call.operation as usize];
                    assert!(std::ptr::eq(proof.operation(), operation));
                }
                Ok(())
            })
            .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn ranked_unit_call_query_rejects_foreign_cloned_and_cross_root_source_calls() {
    let owner = unit_owner(UnitCase::Initializer, &[1, 1]);
    let other = unit_owner(UnitCase::Initializer, &[1, 1]);
    let first = owner.helper_memory.unit_source.calls[0];
    let second = owner.helper_memory.unit_source.calls[1];
    let call = source_call(&owner, first.caller.function, first.source_block);
    let cloned = call.clone();
    with_inventory(&owner, |inventory, floor| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        owner
            .with_checked_unit_local_source_v1(inventory, &mut budget, |view, budget| {
                for foreign in [
                    &cloned,
                    source_call(&other, first.caller.function, first.source_block),
                ] {
                    assert!(matches!(
                        view.bounds_neutral_call_v1(
                            first.caller.root,
                            first.caller.function,
                            first.source_block,
                            foreign,
                            budget,
                        ),
                        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                    ));
                }
                assert!(matches!(
                    view.bounds_neutral_call_v1(
                        second.caller.root,
                        second.caller.function,
                        second.source_block,
                        call,
                        budget,
                    ),
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ));
                assert!(
                    view.bounds_neutral_call_v1(
                        second.caller.root,
                        first.caller.function,
                        first.source_block,
                        call,
                        budget,
                    )?
                    .is_none()
                );
                assert!(
                    view.bounds_neutral_call_v1(
                        first.caller.root,
                        first.caller.function,
                        SemanticBlockIdV1::from_index(99),
                        call,
                        budget,
                    )?
                    .is_none()
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(budget.storage(), floor);
    });
    with_inventory(&other, |inventory, floor| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget
            .reserve_storage(floor + owner.unit_local_source_storage_floor_v1().unwrap())
            .unwrap();
        let before = budget.storage();
        assert!(matches!(
            owner.with_checked_unit_local_source_v1(
                inventory,
                &mut budget,
                |_, _| -> Result<(), ProductionSemanticKirErrorV1> { panic!("foreign inventory") }
            ),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert_eq!(budget.storage(), before);
    });
}

#[test]
fn ranked_unit_call_query_does_not_relabel_a_genuine_raw_empty_helper() {
    let owner = array_owner_at_body(ArrayCase::ValueRead { local_index: false }, true).unwrap();
    assert_eq!(
        owner.helper_source_policy_v1(),
        ProductionHelperSourcePolicyV1::RawEmpty
    );
    assert!(owner.empty_effect_helpers().iter().count() > 0);
    let root = SemanticFunctionIdV1::from_index(0);
    let block = SemanticBlockIdV1::from_index(0);
    let call = source_call(&owner, root, block);
    with_inventory(&owner, |inventory, floor| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        owner
            .with_checked_unit_local_source_v1(inventory, &mut budget, |view, budget| {
                assert!(
                    view.bounds_neutral_call_v1(root, root, block, call, budget)?
                        .is_none()
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn ranked_call_sort_preserves_rows_and_rejects_duplicate_full_source_sites() {
    let owner = unit_owner(UnitCase::Initializer, &[2, 2]);
    let original = &owner.helper_memory.unit_source.calls;
    assert!(
        original
            .windows(2)
            .all(|pair| pair[0].source_site_key() < pair[1].source_site_key())
    );
    // Isolated private-row mutation exercises sorting and duplicate validation,
    // not an alternative way to manufacture an admitted owner.
    let mut rows = original.clone();
    rows.reverse();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    let floor = FLOOR + rows.capacity() * std::mem::size_of::<UnitLocalCallRowV1>();
    budget.reserve_storage(floor).unwrap();
    sort_unit_local_calls_v1(&mut rows, &mut budget).unwrap();
    for (actual, expected) in rows.iter().zip(original) {
        assert_eq!(actual.source_site_key(), expected.source_site_key());
        assert_eq!(actual.call, expected.call);
        assert_eq!(actual.ignored_values, expected.ignored_values);
        assert_eq!(actual.callee_association, expected.callee_association);
    }
    rows[1] = rows[0];
    assert!(matches!(
        sort_unit_local_calls_v1(&mut rows, &mut budget),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
}

#[test]
fn ranked_unit_call_query_has_exact_work_and_header_storage_boundaries() {
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    let row = owner.helper_memory.unit_source.calls[0];
    let call = source_call(&owner, row.caller.function, row.source_block);
    let association = owner.helper_memory.unit_source.associations[row.callee_association];
    let name_bytes = owner.executable().module().functions[association.key.physical]
        .id
        .as_str()
        .len();
    // Existing scope17 + guard5 + one (iteration1/key3) probe + joins48 + both names.
    let exact = 74 + 2 * name_bytes;
    with_inventory(&owner, |inventory, floor| {
        let header = std::mem::size_of::<ProductionUnitLocalSourceV1<'_>>();
        for limit in [exact - 1, exact] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, floor + header);
            budget.reserve_storage(floor).unwrap();
            let result =
                owner.with_checked_unit_local_source_v1(inventory, &mut budget, |view, budget| {
                    assert!(
                        view.bounds_neutral_call_v1(
                            row.caller.root,
                            row.caller.function,
                            row.source_block,
                            call,
                            budget
                        )?
                        .is_some()
                    );
                    Ok(())
                });
            if limit == exact {
                result.unwrap();
                assert_eq!(budget.work(), exact);
            } else {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error))) if error.actual() == exact && error.limit() == limit)
                );
                assert_eq!(budget.work(), 74);
            }
            assert_eq!(
                (budget.storage(), budget.peak_storage()),
                (floor, floor + header)
            );
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, floor + header - 1);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            owner.with_checked_unit_local_source_v1(
                inventory,
                &mut budget,
                |_, _| -> Result<(), ProductionSemanticKirErrorV1> {
                    panic!("short source header")
                }
            ),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(_)
                )
            )
        ));
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (17, floor, floor)
        );
    });
}

#[test]
fn ranked_unit_call_query_rejects_replaced_work_and_released_callback_floor() {
    let owner = unit_owner(UnitCase::Initializer, &[1]);
    let row = owner.helper_memory.unit_source.calls[0];
    let call = source_call(&owner, row.caller.function, row.source_block);
    with_inventory(&owner, |inventory, floor| {
        let header = std::mem::size_of::<ProductionUnitLocalSourceV1<'_>>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut other_work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        let mut other = ArgumentBudgetV1::new(&mut other_work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        other.reserve_storage(floor + header).unwrap();
        owner
            .with_checked_unit_local_source_v1(inventory, &mut budget, |view, budget| {
                let original_work = budget.work();
                std::mem::swap(budget, &mut other);
                let result = view.bounds_neutral_call_v1(
                    row.caller.root,
                    row.caller.function,
                    row.source_block,
                    call,
                    budget,
                );
                std::mem::swap(budget, &mut other);
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                ));
                assert_eq!(budget.work(), original_work);
                assert_eq!((other.work(), other.storage()), (5, floor + header));
                budget.release_storage(1)?;
                let result = view.bounds_neutral_call_v1(
                    row.caller.root,
                    row.caller.function,
                    row.source_block,
                    call,
                    budget,
                );
                budget.reserve_storage(1)?;
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                ));
                assert!(
                    view.bounds_neutral_call_v1(
                        row.caller.root,
                        row.caller.function,
                        row.source_block,
                        call,
                        budget
                    )?
                    .is_some()
                );
                Ok(())
            })
            .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn ranked_source_floor_counts_preexisting_capture_once_and_not_transferred_capture_twice() {
    for preexisting in [false, true] {
        let (mut ssa, launch) = unit_source(UnitCase::Initializer, &[1]);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let external = if preexisting {
            let receipt = ssa
                .try_capture_occurrences_with_budget_v1(&mut budget)
                .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            receipt.retained_storage()
        } else {
            0
        };
        let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), FLOOR + external);
        assert_eq!(
            owner.unit_local_source_storage_floor_v1().unwrap(),
            owner.retained_analysis_storage_v1() + external
        );
        assert!(
            owner
                .semantic_ssa()
                .occurrence_storage()
                .unwrap()
                .retained_storage()
                > 0
        );
    }
}
