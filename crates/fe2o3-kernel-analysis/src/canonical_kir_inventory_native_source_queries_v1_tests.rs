use super::*;

#[test]
fn native_source_queries_use_exact_function_and_sparse_block_coordinates() {
    let (owner, floor) = admit(&mixed_module());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    budget.reserve_storage(floor).unwrap();
    let (inventory, storage) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let shared = inventory
        .function_for_name("shared", &mut budget)
        .unwrap()
        .unwrap();
    let root_a = inventory
        .function_for_name("root_a", &mut budget)
        .unwrap()
        .unwrap();
    let root_b = inventory
        .function_for_name("root_b", &mut budget)
        .unwrap()
        .unwrap();
    assert_ne!(root_a.coordinate, root_b.coordinate);
    for id in [BlockId(4_000_000_000), BlockId(77), BlockId(5)] {
        let block = inventory
            .block_for_id(shared.coordinate, id, &mut budget)
            .unwrap()
            .unwrap();
        assert_eq!(block.block.id, id);
        assert_eq!(block.coordinate.function, shared.coordinate);
    }
    let a = inventory
        .block_for_id(root_a.coordinate, BlockId(91), &mut budget)
        .unwrap()
        .unwrap();
    let b = inventory
        .block_for_id(root_b.coordinate, BlockId(91), &mut budget)
        .unwrap()
        .unwrap();
    assert!(!std::ptr::eq(a.block, b.block));
    assert!(
        inventory
            .function_for_name("shared_extra", &mut budget)
            .unwrap()
            .is_none()
    );
    assert!(
        inventory
            .block_for_id(root_a.coordinate, BlockId(77), &mut budget)
            .unwrap()
            .is_none()
    );
    assert!(
        inventory
            .block_for_id(FunctionCoordinate(u32::MAX), BlockId(91), &mut budget)
            .unwrap()
            .is_none()
    );
    assert_eq!(budget.storage(), floor + storage.retained_storage());
}

#[test]
fn native_source_index_queries_fail_closed_at_the_shared_work_limit() {
    let (owner, _) = admit(&mixed_module());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let (inventory, _) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    let mut empty_work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut empty = Budget::new(&mut empty_work, 0);
    assert!(inventory.function_for_name("shared", &mut empty).is_err());
    assert!(
        inventory
            .block_for_id(FunctionCoordinate(0), BlockId(91), &mut empty)
            .is_err()
    );
    assert_eq!(empty.storage(), 0);
}
