#[test]
fn u128_product_with_cast_factor_does_not_fabricate_literal_authority() {
    let mut function = checked_product_fixture(false);
    let ty = Type::Scalar(ScalarType::U128);
    function.signature.parameters = vec![ty.clone(); 2];
    let widened = |id, value| {
        Operation::effect_free(
            ValueDef::new(ValueId(id), ty.clone()),
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(value),
                to: ty.clone(),
            },
        )
    };
    block_mut(&mut function, 0).operations = vec![
        typed_constant(10, Constant::U64(u64::MAX)),
        typed_constant(11, Constant::U64(7)),
        typed_constant(12, Constant::U64(32)),
        widened(1, 10),
        widened(2, 11),
        widened(3, 12),
        typed_constant(9, Constant::Bool(false)),
        comparison(4, ComparePredicate::LessThan, 0, 2),
    ];
    block_mut(&mut function, 1).operations[0].results[0].ty = ty;
    assert!(verify_product_fixture(&function));
    // KIR has U128 scalars but no U128 literal. A widened U64 value is not an
    // exact same-typed literal factor and must not acquire transfer authority.
    assert_eq!(refined(&function)[&BlockId(2)].len(), 2);
    with_facts(&function, |facts, _| {
        let mut budget = resource_budget(MAX_RELATIONAL_PROOF_WORK, MAX_RELATIONAL_PROOF_WORK);
        let indexed = IndexedFacts::new(facts, &mut budget).unwrap();
        let range = indexed
            .range(ValueId(5), BlockId(2), &mut budget)
            .unwrap()
            .unwrap();
        assert_eq!((range.min, range.max), (0, u128::MAX));
    });
}

fn product_fixture_with_type(factor: Constant, guard: Constant, bound: Constant) -> Function {
    let mut function = checked_product_fixture(false);
    let ty = factor.ty();
    assert_eq!(guard.ty(), ty);
    assert_eq!(bound.ty(), ty);
    function.signature.parameters = vec![ty.clone(); 2];
    block_mut(&mut function, 0).operations[0] = typed_constant(1, factor);
    block_mut(&mut function, 0).operations[1] = typed_constant(2, guard);
    block_mut(&mut function, 0).operations[2] = typed_constant(3, bound);
    block_mut(&mut function, 1).operations[0].results[0].ty = ty;
    function
}

#[test]
fn checked_product_transfer_respects_zero_and_machine_width_boundaries() {
    let cases = [
        (
            Constant::U32(0),
            Constant::U32(7),
            Constant::U32(32),
            true,
            true,
        ),
        (
            Constant::U8(2),
            Constant::U8(200),
            Constant::U8(32),
            false,
            false,
        ),
        (
            Constant::U8(2),
            Constant::U8(128),
            Constant::U8(255),
            false,
            true,
        ),
    ];
    for (factor, guard, bound, remove_guard, proven) in cases {
        let mut function = product_fixture_with_type(factor, guard, bound);
        if remove_guard {
            block_mut(&mut function, 0).terminator = Some(jump(1));
        }
        assert!(verify_product_fixture(&function));
        let effective = refined(&function);
        if proven {
            assert_eq!(effective[&BlockId(2)], BTreeSet::from([BlockId(3)]));
        } else {
            assert_eq!(effective[&BlockId(2)].len(), 2);
        }
        with_facts(&function, |facts, _| {
            let mut budget = resource_budget(MAX_RELATIONAL_PROOF_WORK, MAX_RELATIONAL_PROOF_WORK);
            let indexed = IndexedFacts::new(facts, &mut budget).unwrap();
            let range = indexed
                .range(ValueId(5), BlockId(2), &mut budget)
                .unwrap()
                .unwrap();
            if remove_guard {
                assert_eq!((range.min, range.max), (0, 0));
            } else if proven {
                assert_eq!((range.min, range.max), (0, 254));
            } else {
                let ty = &function.signature.parameters[0];
                let machine = unsigned_type_range(ty).unwrap();
                assert_eq!((range.min, range.max), (machine.min, machine.max));
            }
        });
    }
}

#[test]
fn contradictory_original_product_bounds_do_not_manufacture_a_new_range() {
    let mut function = checked_product_fixture(false);
    block_mut(&mut function, 0).operations[2] = typed_constant(3, Constant::U32(200));
    block_mut(&mut function, 1).terminator = Some(branch(6, 8, 4));
    function.body.as_mut().unwrap().blocks.push(block(
        4,
        vec![
            typed_constant(10, Constant::U32(100)),
            comparison(11, ComparePredicate::GreaterThanOrEqual, 5, 10),
        ],
        branch(11, 2, 3),
    ));
    assert!(verify_product_fixture(&function));
    with_facts(&function, |facts, _| {
        let mut budget = resource_budget(MAX_RELATIONAL_PROOF_WORK, MAX_RELATIONAL_PROOF_WORK);
        let indexed = IndexedFacts::new(facts, &mut budget).unwrap();
        let range = indexed
            .range(ValueId(5), BlockId(2), &mut budget)
            .unwrap()
            .unwrap();
        // The direct bound is [100, MAX], incompatible with inferred [0, 12].
        // Keep the original conservative range instead of inventing an empty one.
        assert_eq!((range.min, range.max), (100, u128::from(u32::MAX)));
    });
    assert_eq!(refined(&function)[&BlockId(2)].len(), 2);
}

#[test]
fn borrowed_dominator_membership_has_exact_construction_and_search_budgets() {
    let set = BTreeSet::from([2_u32, 4, 6, 8]);
    assert!(BorrowedIndex::from_set(&set, &mut resource_budget(7, 4)).is_err());
    assert!(BorrowedIndex::from_set(&set, &mut resource_budget(8, 3)).is_err());
    let mut exact = resource_budget(8, 4);
    let index = BorrowedIndex::from_set(&set, &mut exact).unwrap();
    assert_eq!(exact.receipt, Receipt { work: 8, rows: 4 });
    for (key, expected, comparisons) in [
        (6, Some(&()), 1),
        (2, Some(&()), 3),
        (1, None, 3),
        (9, None, 2),
    ] {
        let mut exact = resource_budget(comparisons, 0);
        assert_eq!(index.get(&key, &mut exact), Ok(expected));
        assert_eq!(
            exact.receipt,
            Receipt {
                work: comparisons,
                rows: 0
            }
        );
        assert_eq!(
            index.get(&key, &mut resource_budget(comparisons - 1, 0)),
            Err(())
        );
    }
    let empty = BTreeSet::<u32>::new();
    let mut zero = resource_budget(0, 0);
    let empty_index = BorrowedIndex::from_set(&empty, &mut zero).unwrap();
    assert_eq!(empty_index.get(&1, &mut zero), Ok(None));
    assert_eq!(zero.receipt, Receipt::default());
}

#[test]
fn dominator_membership_and_both_value_indexes_share_the_cumulative_envelope() {
    let types = BTreeMap::from([(1_u32, 1_u32), (2, 2), (3, 3)]);
    let definitions = BTreeMap::from([(4_u32, 4_u32), (5, 5)]);
    let dominators = BTreeSet::from([6_u32, 7]);
    let mut short = resource_budget(14, 6);
    let _types = BorrowedIndex::new(&types, &mut short).unwrap();
    let _definitions = BorrowedIndex::new(&definitions, &mut short).unwrap();
    assert!(BorrowedIndex::from_set(&dominators, &mut short).is_err());
    assert_eq!(short.receipt, Receipt { work: 10, rows: 5 });
    let mut exact = resource_budget(14, 7);
    let _types = BorrowedIndex::new(&types, &mut exact).unwrap();
    let _definitions = BorrowedIndex::new(&definitions, &mut exact).unwrap();
    let _dominators = BorrowedIndex::from_set(&dominators, &mut exact).unwrap();
    assert_eq!(exact.receipt, Receipt { work: 14, rows: 7 });
    // Each subsequent query reserves its own membership index; the budget
    // cannot silently reuse an earlier query's allocation allowance.
    assert!(BorrowedIndex::from_set(&dominators, &mut exact).is_err());
    assert_eq!(exact.receipt, Receipt { work: 14, rows: 7 });
}

fn resource_budget(work_limit: usize, row_limit: usize) -> Budget {
    Budget {
        receipt: Receipt::default(),
        work_limit,
        row_limit,
    }
}

#[test]
fn borrowed_index_construction_has_literal_work_and_row_boundaries() {
    let map = BTreeMap::from([(2_u32, 20_u32), (4, 40), (6, 60), (8, 80)]);
    let mut short_work = resource_budget(7, 4);
    assert!(BorrowedIndex::new(&map, &mut short_work).is_err());
    let mut short_rows = resource_budget(8, 3);
    assert!(BorrowedIndex::new(&map, &mut short_rows).is_err());
    assert_eq!(short_rows.receipt, Receipt::default());
    let mut exact = resource_budget(8, 4);
    let index = BorrowedIndex::new(&map, &mut exact).unwrap();
    assert_eq!(exact.receipt, Receipt { work: 8, rows: 4 });
    assert_eq!(index.rows.len(), 4);
    let empty = BTreeMap::<u32, u32>::new();
    let mut zero = resource_budget(0, 0);
    let empty_index = BorrowedIndex::new(&empty, &mut zero).unwrap();
    assert_eq!(empty_index.get(&1, &mut zero), Ok(None));
    assert_eq!(zero.receipt, Receipt::default());
}

#[test]
fn borrowed_index_search_charges_each_present_and_absent_key_comparison() {
    let map = BTreeMap::from([(2_u32, 20_u32), (4, 40), (6, 60), (8, 80)]);
    let index = BorrowedIndex::new(&map, &mut resource_budget(8, 4)).unwrap();
    for (key, expected, comparisons) in [
        (6, Some(&60), 1),
        (4, Some(&40), 2),
        (2, Some(&20), 3),
        (1, None, 3),
        (5, None, 2),
        (9, None, 2),
    ] {
        let mut exact = resource_budget(comparisons, 0);
        assert_eq!(index.get(&key, &mut exact), Ok(expected));
        assert_eq!(
            exact.receipt,
            Receipt {
                work: comparisons,
                rows: 0
            }
        );
        let mut short = resource_budget(comparisons - 1, 0);
        assert_eq!(index.get(&key, &mut short), Err(()));
        assert_eq!(short.receipt.work, comparisons - 1);
    }
}

#[test]
fn borrowed_indexes_share_one_cumulative_storage_budget() {
    let first = BTreeMap::from([(1_u32, 1_u32), (2, 2), (3, 3)]);
    let second = BTreeMap::from([(4_u32, 4_u32), (5, 5)]);
    let mut short = resource_budget(10, 4);
    let _first_index = BorrowedIndex::new(&first, &mut short).unwrap();
    assert!(BorrowedIndex::new(&second, &mut short).is_err());
    assert_eq!(short.receipt, Receipt { work: 6, rows: 3 });
    let mut exact = resource_budget(10, 5);
    let _first_index = BorrowedIndex::new(&first, &mut exact).unwrap();
    let _second_index = BorrowedIndex::new(&second, &mut exact).unwrap();
    assert_eq!(exact.receipt, Receipt { work: 10, rows: 5 });
    with_facts(&constant_fixture(true), |facts, effective| {
        let original = effective.clone();
        assert_eq!(
            refine_with_limits(
                facts,
                effective,
                MAX_RELATIONAL_PROOF_WORK,
                facts.types.len()
            ),
            Err(())
        );
        assert_eq!(*effective, original);
    });
}

#[test]
fn indexed_refinement_exhaustion_keeps_all_earlier_proposals_unapplied() {
    with_facts(&constant_fixture(true), |facts, effective| {
        let original = effective.clone();
        let receipt = refine_with_limits(
            facts,
            effective,
            MAX_RELATIONAL_PROOF_WORK,
            MAX_RELATIONAL_PROOF_WORK,
        )
        .unwrap();
        assert_eq!(effective[&BlockId(0)], BTreeSet::from([BlockId(1)]));
        assert_eq!(effective[&BlockId(1)], BTreeSet::from([BlockId(3)]));
        let expected = effective.clone();
        *effective = original.clone();
        assert_eq!(
            refine_with_limits(facts, effective, receipt.work - 1, receipt.rows),
            Err(())
        );
        assert_eq!(*effective, original);
        assert_eq!(
            refine_with_limits(facts, effective, receipt.work, receipt.rows - 1),
            Err(())
        );
        assert_eq!(*effective, original);
        assert_eq!(
            refine_with_limits(facts, effective, receipt.work, receipt.rows),
            Ok(receipt)
        );
        assert_eq!(*effective, expected);
    });
}

fn typed_constant(id: u32, value: Constant) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), value.ty()),
        OperationKind::Constant(value),
    )
}

fn checked_product_fixture(reversed: bool) -> Function {
    let (lhs, rhs) = if reversed { (1, 0) } else { (0, 1) };
    Function::definition(
        "checked_product",
        Signature::new(vec![Type::Scalar(ScalarType::U32); 2], vec![]),
        vec![ValueId(0), ValueId(8)],
        vec![
            block(
                0,
                vec![
                    typed_constant(1, Constant::U32(2)),
                    typed_constant(2, Constant::U32(7)),
                    typed_constant(3, Constant::U32(32)),
                    typed_constant(9, Constant::Bool(false)),
                    comparison(4, ComparePredicate::LessThan, 0, 2),
                ],
                branch(4, 1, 3),
            ),
            block(
                1,
                vec![Operation::checked_binary(
                    ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U32)),
                    ValueDef::new(ValueId(6), Type::BOOL),
                    CheckedBinaryOperator::Multiply,
                    ValueId(lhs),
                    ValueId(rhs),
                )],
                branch(6, 8, 2),
            ),
            block(
                2,
                vec![comparison(7, ComparePredicate::LessThan, 5, 3)],
                branch(7, 3, 8),
            ),
            returned(3),
            block(8, vec![], Terminator::Unreachable),
        ],
    )
}

fn verify_product_fixture(function: &Function) -> bool {
    let mut module = Module::new("checked_product_fixture");
    module.functions.push(function.clone());
    fe2o3_kernel_ir::verify_module(&module).is_ok()
}

#[test]
fn checked_product_guard_proves_exact_observed_comparison_in_both_operand_orders() {
    for reversed in [false, true] {
        let function = checked_product_fixture(reversed);
        assert!(verify_product_fixture(&function));
        let effective = refined(&function);
        assert_eq!(effective[&BlockId(2)], BTreeSet::from([BlockId(3)]));
        assert_eq!(effective[&BlockId(0)].len(), 2);
        assert_eq!(
            analyze_function(&function).value(ValueId(5)),
            Variation::Varying
        );
        with_facts(&function, |facts, _| {
            let mut budget = Budget {
                receipt: Receipt::default(),
                work_limit: MAX_RELATIONAL_PROOF_WORK,
                row_limit: MAX_RELATIONAL_PROOF_WORK,
            };
            let indexed = IndexedFacts::new(facts, &mut budget).unwrap();
            let range = indexed
                .range(ValueId(5), BlockId(2), &mut budget)
                .unwrap()
                .unwrap();
            assert_eq!((range.min, range.max), (0, 12));
            for task in 0_u32..7 {
                assert!(u128::from(task * 2) >= range.min);
                assert!(u128::from(task * 2) <= range.max);
                assert!(task * 2 < 32);
            }
        });
    }
}

#[test]
fn checked_product_transfer_rejects_unsupported_or_unbounded_producers() {
    for mutation in 0..9 {
        let mut function = checked_product_fixture(false);
        let expected_well_typed = !matches!(mutation, 0..=2);
        match mutation {
            0 => {
                block_mut(&mut function, 1).operations[0].results[1].ty =
                    Type::Scalar(ScalarType::U32)
            }
            1 => {
                block_mut(&mut function, 1).operations[0].results[0].ty =
                    Type::Scalar(ScalarType::U64)
            }
            2 => block_mut(&mut function, 0).operations[0] = typed_constant(1, Constant::U64(2)),
            3 => {
                block_mut(&mut function, 2).operations[0] =
                    comparison(7, ComparePredicate::Equal, 6, 9)
            }
            4 => {
                block_mut(&mut function, 1).operations[0].kind = OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Multiply),
                    lhs: ValueId(0),
                    rhs: ValueId(8),
                };
            }
            5 => {
                block_mut(&mut function, 0).operations[1] =
                    typed_constant(2, Constant::U32(u32::MAX))
            }
            6 => block_mut(&mut function, 0).terminator = Some(jump(1)),
            7 => {
                block_mut(&mut function, 1).operations[0] = Operation::effect_free(
                    ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U32)),
                    OperationKind::Binary {
                        op: BinaryOp::Multiply,
                        lhs: ValueId(0),
                        rhs: ValueId(1),
                    },
                );
                block_mut(&mut function, 1).terminator = Some(jump(2));
            }
            _ => {
                block_mut(&mut function, 2).operations.insert(
                    0,
                    Operation::checked_binary(
                        ValueDef::new(ValueId(10), Type::Scalar(ScalarType::U32)),
                        ValueDef::new(ValueId(11), Type::BOOL),
                        CheckedBinaryOperator::Multiply,
                        ValueId(5),
                        ValueId(1),
                    ),
                );
                block_mut(&mut function, 2).operations[1] =
                    comparison(7, ComparePredicate::LessThan, 10, 3);
            }
        }
        assert_eq!(
            verify_product_fixture(&function),
            expected_well_typed,
            "mutation {mutation}"
        );
        assert_eq!(
            refined(&function)[&BlockId(2)].len(),
            2,
            "mutation {mutation}"
        );
    }
}

#[test]
fn checked_product_guard_cannot_come_from_a_future_loop_visit() {
    let mut function = checked_product_fixture(false);
    block_mut(&mut function, 0).terminator = Some(jump(1));
    block_mut(&mut function, 2).terminator = Some(branch(7, 4, 8));
    function
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(block(4, vec![], branch(4, 1, 3)));
    assert!(verify_product_fixture(&function));
    assert_eq!(refined(&function)[&BlockId(2)].len(), 2);
}

#[test]
fn checked_product_transfer_does_not_accept_signed_arithmetic() {
    let mut function = checked_product_fixture(false);
    function.signature.parameters = vec![Type::Scalar(ScalarType::I32); 2];
    for (index, id, value) in [(0, 1, 2), (1, 2, 7), (2, 3, 32)] {
        block_mut(&mut function, 0).operations[index] = typed_constant(id, Constant::I32(value));
    }
    block_mut(&mut function, 1).operations[0].results[0].ty = Type::Scalar(ScalarType::I32);
    assert!(verify_product_fixture(&function));
    assert_eq!(refined(&function)[&BlockId(2)].len(), 2);
}

#[test]
fn checked_product_refinement_is_transactional_at_exact_budget_boundaries() {
    let function = checked_product_fixture(false);
    with_facts(&function, |facts, effective| {
        let original = effective.clone();
        let receipt = refine_with_limits(
            facts,
            effective,
            MAX_RELATIONAL_PROOF_WORK,
            MAX_RELATIONAL_PROOF_WORK,
        )
        .unwrap();
        assert!(receipt.work > 0);
        assert!(receipt.rows > 0);
        assert_eq!(effective[&BlockId(2)], BTreeSet::from([BlockId(3)]));
        let expected = effective.clone();
        *effective = original.clone();
        assert_eq!(
            refine_with_limits(facts, effective, receipt.work - 1, receipt.rows),
            Err(())
        );
        assert_eq!(*effective, original);
        assert_eq!(
            refine_with_limits(facts, effective, receipt.work, receipt.rows - 1),
            Err(())
        );
        assert_eq!(*effective, original);
        assert_eq!(
            refine_with_limits(facts, effective, receipt.work, receipt.rows),
            Ok(receipt)
        );
        assert_eq!(*effective, expected);
    });
}

#[test]
fn already_singleton_queries_skip_expensive_proofs_without_partial_mutation() {
    let function = constant_fixture(true);
    let full_receipt = with_facts(&function, |facts, effective| {
        refine_with_limits(
            facts,
            effective,
            MAX_RELATIONAL_PROOF_WORK,
            MAX_RELATIONAL_PROOF_WORK,
        )
        .unwrap()
    });
    with_facts(&function, |facts, effective| {
        effective.insert(BlockId(0), BTreeSet::from([BlockId(1)]));
        let original = effective.clone();
        let receipt = refine_with_limits(
            facts,
            effective,
            MAX_RELATIONAL_PROOF_WORK,
            MAX_RELATIONAL_PROOF_WORK,
        )
        .unwrap();
        assert!(receipt.work < full_receipt.work);
        assert_eq!(effective[&BlockId(1)], BTreeSet::from([BlockId(3)]));
        let expected = effective.clone();
        *effective = original.clone();
        assert_eq!(
            refine_with_limits(facts, effective, receipt.work - 1, receipt.rows),
            Err(())
        );
        assert_eq!(*effective, original);
        assert_eq!(
            refine_with_limits(facts, effective, receipt.work, receipt.rows),
            Ok(receipt)
        );
        assert_eq!(*effective, expected);
    });
}
