use super::*;
use crate::{
    CanonicalKernelIrWorkBudgetV1, Constant, FunctionBody, FunctionRole, Operation, OperationKind,
    Signature, Terminator, ValueDef,
};

const FLOOR: usize = 11;
type BlockRow = VerificationNumericIndexRowV1<&'static BasicBlock>;
type DefinitionRow = VerificationNumericIndexRowV1<VerificationDefinitionV1<'static>>;

fn ordered_function(blocks: usize) -> Function {
    Function {
        id: "source_index".into(),
        signature: Signature::new(vec![Type::INDEX], vec![]),
        role: FunctionRole::InternalHelper,
        body: Some(FunctionBody {
            parameters: vec![ValueId(0)],
            blocks: (0..blocks)
                .map(|index| BasicBlock {
                    id: BlockId(index as u32),
                    parameters: vec![ValueDef {
                        id: ValueId((2 * index + 1) as u32),
                        ty: Type::INDEX,
                    }],
                    operations: vec![
                        Operation {
                            results: vec![ValueDef {
                                id: ValueId((2 * index + 2) as u32),
                                ty: Type::INDEX,
                            }],
                            kind: OperationKind::Constant(Constant::Index(index as u64)),
                        },
                        Operation {
                            results: vec![],
                            kind: OperationKind::Constant(Constant::Index(0)),
                        },
                    ],
                    // All but the first block are unreachable. An index must
                    // still retain their original definitions and sites.
                    terminator: Some(Terminator::Return { values: vec![] }),
                })
                .collect(),
        }),
        required_capabilities: Default::default(),
    }
}

fn bytes<T>(count: usize) -> usize {
    size_of::<Vec<T>>() + count * size_of::<T>()
}

fn constructor_frame() -> usize {
    type Owner = ByteFunctionStateV2<'static, 'static>;
    type Rows = (Vec<BlockRow>, Vec<DefinitionRow>);
    size_of::<Owner>()
        + size_of::<std::thread::Result<Result<Rows, Error>>>()
        + size_of::<Result<Option<Owner>, Error>>()
        + size_of::<(&Function, &FunctionBody, &mut Budget<'static>)>()
}

fn radix_frame<T>() -> usize {
    size_of::<std::thread::Result<Result<(), Error>>>()
        + size_of::<(&mut [T], &mut Budget<'static>)>()
}

fn ordered_work(blocks: usize) -> usize {
    let definitions = 1 + 2 * blocks;
    let ordered_scan = |count: usize| if count < 2 { 0 } else { 2 * count - 1 };
    // Entry, two allocations, exhaustion; block fill and two census passes;
    // flattened headers, exact definitions; then two ordered radix proofs.
    4 + 4 * blocks
        + 2 * (2 * blocks)
        + definitions
        + ordered_scan(blocks)
        + ordered_scan(definitions)
}

fn ordered_storage(function: &Function, blocks: usize) -> (usize, usize) {
    let block_rows = bytes::<BlockRow>(blocks);
    let definition_rows = bytes::<DefinitionRow>(1 + 2 * blocks);
    // Only the compiler-defined iterator type is measured here. Source counts,
    // phase liveness, capacities, and all work are derived from the fixture.
    let input = function_definition_rows_v2(function, function.body.as_ref().unwrap());
    let input_frame = size_of_val(&input);
    let phases = [
        block_rows + radix_frame::<BlockRow>(),
        block_rows + input_frame + definition_rows,
        block_rows + definition_rows + radix_frame::<DefinitionRow>(),
    ];
    (
        size_of::<ByteFunctionStateV2<'static, 'static>>() + block_rows + definition_rows,
        constructor_frame() + phases.into_iter().max().unwrap(),
    )
}

#[test]
fn byte_source_index_independent_exact_work_peak_and_retained_vectors() {
    for blocks in [0, 1, 2, 17, 1_024] {
        let function = ordered_function(blocks);
        let expected_work = ordered_work(blocks);
        let (retained, peak) = ordered_storage(&function, blocks);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(expected_work);
        let mut budget = Budget::new(&mut work, FLOOR + peak);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = ByteFunctionStateV2::build(&function, &mut budget)
            .unwrap()
            .unwrap();
        assert_eq!(owner.blocks.len(), blocks);
        assert_eq!(owner.definitions.len(), 1 + 2 * blocks);
        assert_eq!(owner.blocks.capacity(), blocks);
        assert_eq!(owner.definitions.capacity(), 1 + 2 * blocks);
        assert_eq!(budget.work(), expected_work, "blocks={blocks}");
        assert_eq!(budget.storage(), FLOOR + retained, "blocks={blocks}");
        assert_eq!(budget.peak_storage(), FLOOR + peak, "blocks={blocks}");
        owner.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn byte_source_index_one_short_construction_restores_floor_and_keeps_denial() {
    for blocks in [0, 1, 2, 17] {
        let function = ordered_function(blocks);
        let total = ordered_work(blocks);
        let (_, peak) = ordered_storage(&function, blocks);
        for work_short in [false, true] {
            let work_limit = total - usize::from(work_short);
            let storage_limit = FLOOR + peak - usize::from(!work_short);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            let error = ByteFunctionStateV2::build(&function, &mut budget)
                .err()
                .unwrap();
            if work_short {
                assert!(
                    matches!(error, Error::Work(error) if error.actual() == total && error.limit() == work_limit)
                );
            } else {
                assert!(
                    matches!(error, Error::Storage(error) if error.actual() == FLOOR + peak && error.limit() == storage_limit)
                );
            }
            assert_eq!(budget.storage(), FLOOR);
            let accepted = budget.work();
            assert_eq!(
                ByteFunctionStateV2::build(&function, &mut budget)
                    .err()
                    .unwrap(),
                error
            );
            assert_eq!((budget.work(), budget.storage()), (accepted, FLOOR));
        }
    }
}

#[test]
fn byte_source_index_preserves_borrowed_types_all_sites_and_empty_suffixes() {
    let mut function = ordered_function(3);
    function.signature.parameters.push(Type::U64);
    let body = function.body.as_mut().unwrap();
    // Preserve the legacy zipped-parameter convention for malformed arity.
    body.blocks[1].operations[0].results.push(ValueDef {
        id: ValueId(90),
        ty: Type::U64,
    });
    body.blocks[2].operations.extend((0..7).map(|_| Operation {
        results: vec![],
        kind: OperationKind::Constant(Constant::Index(0)),
    }));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let owner = ByteFunctionStateV2::build(&function, &mut budget)
        .unwrap()
        .unwrap();
    let body = function.body.as_ref().unwrap();
    let argument = owner
        .definition(&function, ValueId(0), &mut budget)
        .unwrap()
        .unwrap();
    assert_eq!(
        argument.site,
        VerificationDefinitionSiteV1::FunctionParameter
    );
    assert!(std::ptr::eq(argument.ty, &function.signature.parameters[0]));
    for (ordinal, block) in body.blocks.iter().enumerate() {
        assert!(std::ptr::eq(
            owner
                .block(&function, block.id, &mut budget)
                .unwrap()
                .unwrap(),
            block
        ));
        let parameter = owner
            .definition(&function, block.parameters[0].id, &mut budget)
            .unwrap()
            .unwrap();
        assert_eq!(
            parameter.site,
            VerificationDefinitionSiteV1::BlockParameter(block.id)
        );
        assert!(std::ptr::eq(parameter.ty, &block.parameters[0].ty));
        for result in &block.operations[0].results {
            let found = owner
                .definition(&function, result.id, &mut budget)
                .unwrap()
                .unwrap();
            assert_eq!(
                found.site,
                VerificationDefinitionSiteV1::Operation(BlockId(ordinal as u32), 0)
            );
            assert!(std::ptr::eq(found.ty, &result.ty));
        }
    }
    assert_eq!(owner.definitions.len(), 8);
    assert!(
        owner
            .definition(&function, ValueId(91), &mut budget)
            .unwrap()
            .is_none()
    );
    owner.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn byte_source_index_sparse_duplicates_match_legacy_last_definition_and_block() {
    let mut function = ordered_function(3);
    let body = function.body.as_mut().unwrap();
    body.parameters[0] = ValueId(u32::MAX);
    body.blocks[0].id = BlockId(u32::MAX);
    body.blocks[1].id = BlockId(7);
    body.blocks[2].id = BlockId(u32::MAX);
    body.blocks[2].parameters[0].id = ValueId(2);
    let mut legacy_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut legacy_budget = Budget::new(&mut legacy_work, usize::MAX);
    let legacy = VerificationFunctionStateV1::build(&function, &mut legacy_budget)
        .unwrap()
        .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let owner = ByteFunctionStateV2::build(&function, &mut budget)
        .unwrap()
        .unwrap();
    for id in [0, 1, 2, 3, 4, 5, 6, 7, u32::MAX - 1, u32::MAX] {
        let expected = legacy.definition(ValueId(id), &mut legacy_budget).unwrap();
        let actual = owner
            .definition(&function, ValueId(id), &mut budget)
            .unwrap();
        assert_eq!(actual.map(|row| row.site), expected.map(|row| row.site));
        if let (Some(actual), Some(expected)) = (actual, expected) {
            assert!(std::ptr::eq(actual.ty, expected.ty));
        }
        let expected = legacy.block(BlockId(id), &mut legacy_budget).unwrap();
        let actual = owner.block(&function, BlockId(id), &mut budget).unwrap();
        assert_eq!(
            actual.map(|row| row as *const _),
            expected.map(|row| row as *const _)
        );
    }
    assert!(std::ptr::eq(
        owner
            .block(&function, BlockId(u32::MAX), &mut budget)
            .unwrap()
            .unwrap(),
        &function.body.as_ref().unwrap().blocks[2]
    ));
    assert_eq!(
        owner
            .definition(&function, ValueId(2), &mut budget)
            .unwrap()
            .unwrap()
            .site,
        VerificationDefinitionSiteV1::BlockParameter(BlockId(u32::MAX))
    );
    owner.release(&mut budget).unwrap();
    legacy.release(&mut legacy_budget).unwrap();
    assert_eq!((budget.storage(), legacy_budget.storage()), (0, 0));
}

#[test]
fn byte_source_index_sparse_radix_has_independent_exact_overlap_and_one_short_peak() {
    let mut function = ordered_function(2);
    let body = function.body.as_mut().unwrap();
    body.blocks.swap(0, 1);
    body.parameters[0] = ValueId(u32::MAX);
    // Both input rosters invert at their first comparison. Radix prepays a
    // scratch copy and four passes, each with 3*N row visits + 512 buckets.
    let radix_work = |count| 3 + count + 4 * (3 * count + 512);
    let exact_work = 4 + 4 * 2 + 2 * 4 + 5 + radix_work(2) + radix_work(5);
    let block_rows = bytes::<BlockRow>(2);
    let definition_rows = bytes::<DefinitionRow>(5);
    let input = function_definition_rows_v2(&function, function.body.as_ref().unwrap());
    let expected_peak = constructor_frame()
        + [
            2 * block_rows + radix_frame::<BlockRow>() + size_of::<[usize; 256]>(),
            block_rows + size_of_val(&input) + definition_rows,
            block_rows
                + 2 * definition_rows
                + radix_frame::<DefinitionRow>()
                + size_of::<[usize; 256]>(),
        ]
        .into_iter()
        .max()
        .unwrap();
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work);
        let mut budget = Budget::new(&mut work, FLOOR + expected_peak - usize::from(short));
        budget.reserve_storage(FLOOR).unwrap();
        let result = ByteFunctionStateV2::build(&function, &mut budget);
        if short {
            assert!(
                matches!(result, Err(Error::Storage(error)) if error.actual() == FLOOR + expected_peak)
            );
        } else {
            let owner = result.unwrap().unwrap();
            assert_eq!(budget.peak_storage(), FLOOR + expected_peak);
            owner.release(&mut budget).unwrap();
        }
        assert_eq!(budget.work(), exact_work);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn byte_source_index_query_one_short_is_sticky_and_release_keeps_unrelated_bytes() {
    let function = ordered_function(1);
    // Three sorted definitions; querying the largest uses entry + one probe
    // at index1, one at index2, then final equality: exactly four.
    let construction = ordered_work(1);
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(construction + 4 - usize::from(short));
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = ByteFunctionStateV2::build(&function, &mut budget)
            .unwrap()
            .unwrap();
        budget.reserve_storage(19).unwrap();
        let result = owner.definition(&function, ValueId(2), &mut budget);
        if short {
            let error = result.err().unwrap();
            assert!(matches!(error, Error::Work(error) if error.actual() == construction + 4));
            let prior = budget.work();
            assert_eq!(
                owner
                    .block(&function, BlockId(0), &mut budget)
                    .err()
                    .unwrap(),
                error
            );
            assert_eq!(budget.work(), prior);
        } else {
            assert_eq!(
                result.unwrap().unwrap().site,
                VerificationDefinitionSiteV1::Operation(BlockId(0), 0)
            );
            assert_eq!(budget.work(), construction + 4);
        }
        owner.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR + 19);
    }
}

#[test]
fn byte_source_index_declarations_preserve_entry_work_without_storage_or_owner() {
    let mut function = ordered_function(1);
    function.body = None;
    for limit in [0, 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        let result = ByteFunctionStateV2::build(&function, &mut budget);
        if limit == 0 {
            assert!(matches!(result, Err(Error::Work(error)) if error.actual() == 1));
        } else {
            assert!(result.unwrap().is_none());
            assert_eq!(budget.work(), 1);
        }
        assert_eq!((budget.storage(), budget.peak_storage()), (FLOOR, FLOOR));
    }
}

#[test]
fn byte_source_index_rejects_foreign_source_and_ledger_without_query_work() {
    let function = ordered_function(2);
    let foreign_source = function.clone();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let owner = ByteFunctionStateV2::build(&function, &mut budget)
        .unwrap()
        .unwrap();
    let previous = budget.work();
    assert_eq!(
        owner
            .definition(&foreign_source, ValueId(0), &mut budget)
            .err()
            .unwrap(),
        Error::Accounting
    );
    assert_eq!(budget.work(), previous);
    let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut foreign = Budget::new(&mut foreign_work, usize::MAX);
    assert!(foreign.charge_work(1).is_err());
    assert_eq!(
        owner
            .block(&function, BlockId(0), &mut foreign)
            .err()
            .unwrap(),
        Error::Accounting
    );
    assert_eq!(foreign.work(), 0);
    owner.release(&mut budget).unwrap();
}

#[test]
fn byte_source_index_original_denial_precedes_undercut_but_not_foreign_identity() {
    let function = ordered_function(1);
    let (_, peak) = ordered_storage(&function, 1);
    for storage_denial in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(ordered_work(1));
        let mut budget = Budget::new(&mut work, FLOOR + peak);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = ByteFunctionStateV2::build(&function, &mut budget)
            .unwrap()
            .unwrap();
        let charged = budget.storage();
        let denial = if storage_denial {
            budget.reserve_storage(FLOOR + peak).unwrap_err()
        } else {
            budget.charge_work(1).unwrap_err()
        };
        budget.rollback_storage(FLOOR).unwrap();
        let error = owner
            .block(&function, BlockId(0), &mut budget)
            .err()
            .unwrap();
        assert_eq!(error, denial);
        budget.reserve_storage(charged - FLOOR).unwrap();
        owner.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn byte_source_index_clean_undercut_and_moved_budget_slot_refuse_without_spending() {
    let function = ordered_function(1);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Box::new(Budget::new(&mut work, usize::MAX));
    budget.reserve_storage(FLOOR).unwrap();
    let owner = ByteFunctionStateV2::build(&function, &mut budget)
        .unwrap()
        .unwrap();
    let charged = budget.storage();
    let accepted = budget.work();
    budget.rollback_storage(FLOOR).unwrap();
    assert_eq!(
        owner
            .definition(&function, ValueId(0), &mut budget)
            .err()
            .unwrap(),
        Error::Accounting
    );
    assert_eq!(budget.work(), accepted);
    budget.reserve_storage(charged - FLOOR).unwrap();
    // Move the exact same ledger out of its original heap slot; identity alone
    // is insufficient when the scope's original Budget address is replaced.
    let mut moved = *budget;
    assert_eq!(
        owner
            .block(&function, BlockId(0), &mut moved)
            .err()
            .unwrap(),
        Error::Accounting
    );
    assert_eq!(moved.work(), accepted);
    drop(owner);
    moved.rollback_storage(FLOOR).unwrap();
}

#[test]
fn byte_source_index_two_live_owners_keep_separate_complete_retained_floors() {
    let first = ordered_function(1);
    let second = ordered_function(17);
    let (first_retained, first_peak) = ordered_storage(&first, 1);
    let (second_retained, second_peak) = ordered_storage(&second, 17);
    let peak = first_peak.max(first_retained + second_peak);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(ordered_work(1) + ordered_work(17));
    let mut budget = Budget::new(&mut work, FLOOR + peak);
    budget.reserve_storage(FLOOR).unwrap();
    let first_owner = ByteFunctionStateV2::build(&first, &mut budget)
        .unwrap()
        .unwrap();
    let second_owner = ByteFunctionStateV2::build(&second, &mut budget)
        .unwrap()
        .unwrap();
    assert_eq!(budget.storage(), FLOOR + first_retained + second_retained);
    assert_eq!(budget.peak_storage(), FLOOR + peak);
    second_owner.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR + first_retained);
    first_owner.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn byte_source_index_all_empty_results_and_missing_parameter_types_have_zero_definitions() {
    let mut function = ordered_function(3);
    function.signature.parameters.clear();
    for block in &mut function.body.as_mut().unwrap().blocks {
        block.parameters.clear();
        for operation in &mut block.operations {
            operation.results.clear();
        }
    }
    // Full B/O census and terminal exhaustion remain charged even though no
    // source definition is yielded; the three ordered blocks cost five.
    let expected_work = 4 + 4 * 3 + 2 * 6 + 5;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(expected_work + 1);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let owner = ByteFunctionStateV2::build(&function, &mut budget)
        .unwrap()
        .unwrap();
    assert!(owner.definitions.is_empty());
    assert_eq!(budget.work(), expected_work);
    assert!(
        owner
            .definition(&function, ValueId(0), &mut budget)
            .unwrap()
            .is_none()
    );
    assert_eq!(budget.work(), expected_work + 1);
    owner.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn byte_source_index_accounting_checks_are_stateless_after_source_and_floor_restoration() {
    let function = ordered_function(1);
    let foreign_source = function.clone();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = ByteFunctionStateV2::build(&function, &mut budget)
        .unwrap()
        .unwrap();
    let charged = budget.storage();
    let accepted = budget.work();
    assert_eq!(
        owner
            .definition(&foreign_source, ValueId(2), &mut budget)
            .err()
            .unwrap(),
        Error::Accounting
    );
    assert_eq!(budget.work(), accepted);
    assert_eq!(
        owner
            .definition(&function, ValueId(2), &mut budget)
            .unwrap()
            .unwrap()
            .site,
        VerificationDefinitionSiteV1::Operation(BlockId(0), 0)
    );
    assert_eq!(budget.work(), accepted + 4);
    budget.rollback_storage(FLOOR).unwrap();
    assert_eq!(
        owner
            .block(&function, BlockId(0), &mut budget)
            .err()
            .unwrap(),
        Error::Accounting
    );
    assert_eq!(budget.work(), accepted + 4);
    budget.reserve_storage(charged - FLOOR).unwrap();
    assert!(std::ptr::eq(
        owner
            .block(&function, BlockId(0), &mut budget)
            .unwrap()
            .unwrap(),
        &function.body.as_ref().unwrap().blocks[0]
    ));
    assert_eq!(budget.work(), accepted + 7);
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
    owner.release(&mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
