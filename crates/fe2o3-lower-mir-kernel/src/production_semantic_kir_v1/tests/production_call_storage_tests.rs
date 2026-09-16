use super::*;

#[test]
fn call_function_index_looks_up_sparse_blocks_without_repeated_body_scans() {
    let count = 4096_u32;
    let blocks = (0..count)
        .map(|index| BasicBlock::new(BlockId(u32::MAX - index)))
        .collect();
    let target = Function::internal_helper(
        "sparse_call_index",
        Signature::new(vec![], vec![]),
        vec![],
        blocks,
    );
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    budget.reserve_storage(FLOOR).unwrap();
    let index = CallFunctionIndexV1::new(&target, &mut budget).unwrap();
    let before = budget.work();
    let retained = budget.storage() - FLOOR;
    for ordinal in 0..count {
        let id = BlockId(u32::MAX - ordinal);
        assert_eq!(index.block(id, &mut budget).unwrap().id, id);
    }
    assert_eq!(budget.work() - before, 40 * count as usize);
    assert_eq!(
        retained,
        count as usize * std::mem::size_of::<&BasicBlock>()
    );
    drop(index);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

const FLOOR: usize = 23;

fn actual_rows() -> (ProductionSemanticSsaOwnerV1, SemanticKirCorrespondenceV1) {
    let source = argument_owner(false, true, true);
    let (_, rows) =
        lower_argument_owner(&source, ProductionSemanticKirLimitsV1::default()).unwrap();
    (source, rows)
}

fn resource(error: ProductionSemanticKirErrorV1, storage: bool) {
    assert!(
        matches!(error,
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(_)) if storage)
            || matches!(error,
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_)) if !storage)
    );
}

#[test]
fn call_anchor_construction_obeys_exact_work_and_bytes_before_allocation() {
    let (source, _) = actual_rows();
    let size = CallReturnBufferV1::bytes(1).unwrap();
    for (work_limit, storage_limit, failure) in [
        (2, FLOOR + 2 * size, None),
        (1, FLOOR + 2 * size, Some(false)),
        (2, FLOOR + 2 * size - 1, Some(true)),
    ] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        match (
            CallReturnBufferV1::for_function(&source.source_semantic().functions()[1], &mut budget),
            failure,
        ) {
            (Ok(buffer), None) => {
                assert_eq!(buffer.requested, 2);
                assert!(buffer.rows.is_empty());
                assert_eq!(budget.storage(), FLOOR + 2 * size);
                drop(buffer);
                budget.release_storage(2 * size).unwrap();
            }
            (Err(error), Some(storage)) => resource(error, storage),
            _ => panic!("unexpected constructor outcome"),
        }
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), if work_limit == 1 { 0 } else { 2 });
    }
}

#[test]
fn call_anchor_merge_accounts_for_old_new_and_consumed_source_storage() {
    let (_, rows) = actual_rows();
    let size = CallReturnBufferV1::bytes(1).unwrap();
    for (work_limit, storage_limit, row_limit, failure) in [
        (3, FLOOR + 4 * size, 2, None),
        (2, FLOOR + 4 * size, 2, Some(false)),
        (3, FLOOR + 4 * size - 1, 2, Some(true)),
        (3, FLOOR + 4 * size, 1, None),
    ] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR + 2 * size).unwrap();
        let mut destination = CallReturnBufferV1::from_box(Box::new([rows.call_returns[0]]));
        let source = CallReturnBufferV1::from_box(Box::new([rows.call_returns[1]]));
        let result = destination.append(source, row_limit, &mut budget);
        if row_limit == 1 {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::Blocks,
                    actual: 2,
                    limit: 1
                })
            ));
        } else if let Some(storage) = failure {
            resource(result.unwrap_err(), storage);
        } else {
            result.unwrap();
            assert_eq!(budget.peak_storage(), FLOOR + 4 * size);
        }
        let retained = if row_limit == 2 && failure.is_none() {
            2
        } else {
            1
        };
        assert_eq!(destination.requested, retained);
        assert_eq!(destination.rows.len(), retained);
        assert_eq!(budget.storage(), FLOOR + retained * size);
        drop(destination);
        budget.release_storage(retained * size).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn call_anchor_compaction_releases_unused_capacity_and_failure_payloads() {
    let (source, rows) = actual_rows();
    let size = CallReturnBufferV1::bytes(1).unwrap();
    for (work_limit, storage_limit, failure) in [
        (3, FLOOR + 3 * size, None),
        (2, FLOOR + 3 * size, Some(false)),
        (3, FLOOR + 3 * size - 1, Some(true)),
    ] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let mut buffer =
            CallReturnBufferV1::for_function(&source.source_semantic().functions()[1], &mut budget)
                .unwrap();
        buffer.rows.push(rows.call_returns[0]);
        match (buffer.into_box(&mut budget), failure) {
            (Ok(retained), None) => {
                assert_eq!(retained.len(), 1);
                assert_eq!(budget.storage(), FLOOR + size);
                assert_eq!(budget.peak_storage(), FLOOR + 3 * size);
                drop(retained);
                budget.release_storage(size).unwrap();
            }
            (Err(error), Some(storage)) => resource(error, storage),
            _ => panic!("unexpected compaction outcome"),
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn call_anchor_shared_owner_sort_caches_keys_with_bounded_scratch() {
    let (_, rows) = actual_rows();
    assert_eq!(rows.call_returns.len(), 6);
    let functions = rows
        .lowered_functions
        .iter()
        .enumerate()
        .map(|(ordinal, row)| ((row.correspondence_owner, row.semantic_function), ordinal))
        .collect();
    let bytes = CallReturnBufferV1::bytes(6).unwrap();
    let scratch = 6 * std::mem::size_of::<(u64, SemanticKirCallReturnV1)>();
    for limit in [1_752, 1_751] {
        let mut buffer = CallReturnBufferV1::from_box(rows.call_returns.clone());
        buffer.rows.reverse();
        let before = buffer.rows.clone();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + bytes + scratch);
        budget.reserve_storage(FLOOR + bytes).unwrap();
        let result = buffer.order(&functions, &mut budget);
        if limit == 1_752 {
            result.unwrap();
            assert_eq!(buffer.rows.as_slice(), rows.call_returns.as_ref());
            assert_eq!(budget.work(), limit);
        } else {
            resource(result.unwrap_err(), false);
            assert_eq!(buffer.rows, before);
            assert_eq!(budget.work(), 0);
        }
        assert_eq!(budget.storage(), FLOOR + bytes);
        assert_eq!(
            budget.peak_storage(),
            FLOOR + bytes + if limit == 1_752 { scratch } else { 0 }
        );
    }
    assert!(matches!(
        CallReturnBufferV1::bytes(usize::MAX),
        Err(ArgumentResourceV1::Arithmetic)
    ));
}

#[test]
fn whole_call_anchor_construction_restores_a_nonzero_floor_on_denial() {
    let (source, _) = actual_rows();
    let roster = argument_launch_roster(&source);
    let roots = materialization_launch_roots_v1(&source, &roster).unwrap();
    let run = |budget: &mut ArgumentBudgetV1<'_>| {
        lower_module_with_call_budget_v1(
            &source,
            ProductionSemanticKirLimitsV1::default(),
            Some(&roots),
            None,
            budget,
        )
    };
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let (_, rows) = run(&mut budget).unwrap();
    let required = (budget.work(), budget.peak_storage());
    assert_eq!(
        budget.storage(),
        FLOOR + CallReturnBufferV1::bytes(rows.call_returns.len()).unwrap()
    );
    for (work_limit, storage_limit, failure) in [
        (required.0, required.1, None),
        (required.0 - 1, required.1, Some(false)),
        (required.0, required.1 - 1, Some(true)),
    ] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        match (run(&mut budget), failure) {
            (Ok((_, rows)), None) => {
                let bytes = CallReturnBufferV1::bytes(rows.call_returns.len()).unwrap();
                drop(rows);
                budget.release_storage(bytes).unwrap();
            }
            (Err(error), Some(storage)) => resource(error, storage),
            _ => panic!("unexpected whole-construction outcome"),
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}
