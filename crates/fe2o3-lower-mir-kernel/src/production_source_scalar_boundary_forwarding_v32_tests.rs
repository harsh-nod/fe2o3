use super::*;
use fe2o3_kernel_ir::{BasicBlock, BlockId, Module, Signature, Terminator, ValueDef};

fn scalar_block(id: u32, parameter: Option<u32>) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    if let Some(value) = parameter {
        block
            .parameters
            .push(ValueDef::new(ValueId(value), Type::Scalar(ScalarType::U32)));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}

fn edge(target: u32, value: u32) -> Option<Terminator> {
    Some(Terminator::Branch {
        target: BlockId(target),
        arguments: vec![ValueId(value)],
    })
}

fn choice(left: u32, left_value: u32, right: u32, right_value: u32) -> Option<Terminator> {
    Some(Terminator::ConditionalBranch {
        condition: ValueId(99),
        then_target: BlockId(left),
        then_arguments: vec![ValueId(left_value)],
        else_target: BlockId(right),
        else_arguments: vec![ValueId(right_value)],
    })
}

fn graph(unknown_cycle: bool, parallel_conflict: bool) -> Module {
    let mut blocks = vec![
        scalar_block(0, None),
        scalar_block(1, Some(10)),
        scalar_block(2, Some(11)),
        scalar_block(3, Some(12)),
        scalar_block(4, Some(13)),
        scalar_block(5, Some(14)),
    ];
    blocks[0].terminator = choice(1, 98, 2, 97);
    blocks[1].terminator = edge(3, 10);
    blocks[2].terminator = edge(3, 11);
    blocks[3].terminator = if parallel_conflict {
        choice(4, 12, 4, 98)
    } else {
        edge(4, 12)
    };
    blocks[4].terminator = edge(5, 13);
    blocks[5].terminator = choice(4, 14, 4, 14);
    if unknown_cycle {
        let mut block = scalar_block(6, Some(15));
        block.terminator = choice(6, 15, 4, 15);
        blocks.push(block);
    }
    let mut module = Module::new("checked-phi-forwarding");
    for name in ["other-owner", "subject"] {
        module.functions.push(Function::internal_helper(
            name,
            Signature::new(
                vec![
                    Type::BOOL,
                    Type::Scalar(ScalarType::U32),
                    Type::Scalar(ScalarType::U32),
                ],
                vec![],
            ),
            vec![ValueId(99), ValueId(98), ValueId(97)],
            blocks.clone(),
        ));
    }
    module
}

fn with_inventory(
    module: &Module,
    consume: impl FnOnce(
        &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        &mut ArgumentBudgetV1<'_>,
    ),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(20_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 32 << 20);
    budget.reserve_storage(19).unwrap();
    let (owner, owner_credit) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
        module, ProductionSemanticKirLimitsV1::default().storage_layout_limits(), &mut budget,
    ).unwrap();
    budget
        .reserve_storage(owner_credit.retained_storage())
        .unwrap();
    let (inventory, inventory_credit) =
        fe2o3_kernel_analysis::CanonicalKirInventoryV18::derive_v18(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_credit.retained_storage())
        .unwrap();
    let floor = budget.storage();
    consume(&inventory, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    budget
        .release_storage(inventory_credit.retained_storage())
        .unwrap();
    drop(owner);
    budget
        .release_storage(owner_credit.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 19);
}

fn terminal(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    function: u32,
    value: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> usize {
    inventory
        .definition_index_for_value(
            fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(function),
            ValueId(value),
            budget,
        )
        .unwrap()
        .unwrap()
}

fn inspect(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    terminals: &[(usize, usize)],
    expected: &[(u32, usize)],
    budget: &mut ArgumentBudgetV1<'_>,
) {
    let floor = budget.storage();
    budget
        .reserve_storage(source_boundary_forwarding_headers_v32().unwrap())
        .unwrap();
    let values = source_boundary_forwarding_v32(
        inventory,
        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(1),
        terminals,
        budget,
    )
    .unwrap();
    assert_eq!(
        values
            .iter()
            .map(|row| (row.value.0, row.original))
            .collect::<Vec<_>>(),
        expected
    );
    for row in &values {
        assert!(
            inventory.functions()[1]
                .definitions
                .contains(&row.definition)
        );
        assert_eq!(
            inventory.definitions()[row.definition].value,
            Some(row.value)
        );
    }
    drop(values);
    budget.release_storage(budget.storage() - floor).unwrap();
}

#[test]
fn checked_phi_forwarding_uses_all_edges_and_grounded_cycles() {
    with_inventory(&graph(false, false), |inventory, budget| {
        let phi = terminal(inventory, 1, 12, budget);
        inspect(inventory, &[(phi, 41)], &[(13, 41), (14, 41)], budget);
        let left = terminal(inventory, 1, 10, budget);
        let right = terminal(inventory, 1, 11, budget);
        inspect(inventory, &[(left, 17), (right, 18)], &[], budget);
        // A downstream checked phi is an opaque terminal, not the join of its
        // different incoming source boundary names.
        inspect(
            inventory,
            &[(left, 17), (right, 18), (phi, 41)],
            &[(13, 41), (14, 41)],
            budget,
        );
        inspect(inventory, &[], &[], budget);
    });
}

#[test]
fn checked_phi_forwarding_refuses_conflicting_parallel_edges_and_ungrounded_inputs() {
    for module in [graph(false, true), graph(true, false)] {
        with_inventory(&module, |inventory, budget| {
            let phi = terminal(inventory, 1, 12, budget);
            inspect(inventory, &[(phi, 41)], &[], budget);
        });
    }
}

#[test]
fn checked_phi_forwarding_rejects_foreign_duplicate_and_nonparameter_terminals() {
    with_inventory(&graph(false, false), |inventory, budget| {
        let local = terminal(inventory, 1, 12, budget);
        let foreign = terminal(inventory, 0, 12, budget);
        let argument = terminal(inventory, 1, 98, budget);
        for terminals in [
            vec![(foreign, 41)],
            vec![(local, 41), (local, 41)],
            vec![(argument, 41)],
        ] {
            let floor = budget.storage();
            budget
                .reserve_storage(source_boundary_forwarding_headers_v32().unwrap())
                .unwrap();
            let result = source_boundary_forwarding_v32(
                inventory,
                fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(1),
                &terminals,
                budget,
            );
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(_))
            ));
            drop(result);
            budget.release_storage(budget.storage() - floor).unwrap();
        }
        inspect(inventory, &[(local, 41)], &[(13, 41), (14, 41)], budget);
    });
}

#[test]
fn genuine_source_phi_forwarding_indexes_original_and_optimized_values_once() {
    with_policy11(
        private_entry_phi_owner_v20,
        |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    let original_leaves = leaves.original.leaves;
                    let function = original.inventory.functions()[0].function;
                    let row = original_leaves
                        .boundary_find_v31([0, 0, 3, 2], budget)?
                        .unwrap();
                    assert_ne!(row.value, ValueId(7));
                    assert_eq!(
                        original_leaves.actual_value(function, ValueId(7), budget)?,
                        Some(NormalizedScalarExpressionV1::Symbol {
                            symbol: row.symbol,
                            scalar: row.scalar
                        })
                    );
                    let storage = budget.storage();
                    let start = budget.work();
                    original_leaves.actual_value(function, ValueId(7), budget)?;
                    let query_work = budget.work() - start;
                    let start = budget.work();
                    for _ in 0..64 {
                        original_leaves.actual_value(function, ValueId(7), budget)?;
                    }
                    assert_eq!(budget.work() - start, 64 * query_work);
                    assert_eq!(budget.storage(), storage);
                    with_check(leaves, budget, |check, budget| {
                        for value in &leaves.boundaries.values {
                            let original = &check.leaves.boundaries.rows[value.original];
                            check.normalize(
                                &ProductionSemanticExpressionV2::Symbol {
                                    symbol: original.symbol,
                                    scalar: original.scalar,
                                },
                                value.value,
                                budget,
                            )?;
                        }
                        Ok(())
                    })
                },
            )
        },
    )
    .unwrap();
}

fn probe(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    terminal: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(23).unwrap();
    let result = (|| {
        budget.reserve_storage(source_boundary_forwarding_headers_v32()?)?;
        let values = source_boundary_forwarding_v32(
            inventory,
            fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(1),
            &[(terminal, 41)],
            &mut budget,
        )?;
        assert_eq!(values.len(), 2);
        drop(values);
        Ok(())
    })();
    let measured = (budget.work(), budget.peak_storage());
    budget.release_storage(budget.storage() - 23).unwrap();
    assert_eq!(budget.storage(), 23);
    (result, measured.0, measured.1)
}

#[test]
fn checked_phi_forwarding_exact_and_one_short_resources_restore_floor() {
    with_inventory(&graph(false, false), |inventory, budget| {
        let phi = terminal(inventory, 1, 12, budget);
        let (result, work, storage) = probe(inventory, phi, usize::MAX, usize::MAX);
        result.unwrap();
        probe(inventory, phi, work, storage).0.unwrap();
        assert!(matches!(
            probe(inventory, phi, work - 1, storage).0,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Work { .. }
            ))
        ));
        assert!(matches!(
            probe(inventory, phi, work, storage - 1).0,
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                ArgumentResourceV1::Storage { .. }
            ))
        ));
    });
}
