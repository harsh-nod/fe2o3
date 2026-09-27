fn source_constant_scalar_v18(bits: u16, signed: bool) -> ProductionSemanticScalarTypeV2 {
    ProductionSemanticScalarTypeV2::Integer { bits, signed }
}

fn source_constant_node_v18(scalar: ProductionSemanticScalarTypeV2, bits: u64)
    -> NormalizedScalarExpressionV1
{
    NormalizedScalarExpressionV1::Constant { scalar, bits }
}

fn source_constant_binary_v18(operation: ProductionSemanticBinaryOpV2,
    scalar: ProductionSemanticScalarTypeV2, overflow: ProductionOverflowContractV2,
    left: u64, right: u64) -> NormalizedScalarExpressionV1
{
    NormalizedScalarExpressionV1::Binary { operation, scalar, overflow,
        lhs: NormalizedScalarNodeV18::legacy(source_constant_node_v18(scalar, left)),
        rhs: NormalizedScalarNodeV18::legacy(source_constant_node_v18(scalar, right)),
    }
}

fn fold_source_constant_tree_v18(mut tree: NormalizedScalarExpressionV1)
    -> NormalizedScalarExpressionV1
{
    let mut charge = UnsupportedIndexCorrelationBudgetV1 { remaining: 8192 };
    source_scalar_constant_fold_v18(&mut tree, 0, &mut charge).unwrap();
    tree
}

#[test]
fn source_constant_normalization_reuses_integer_width_signedness_and_overflow_contracts() {
    use ProductionSemanticBinaryOpV2 as Op;
    use ProductionOverflowContractV2 as Overflow;
    for bits in [8, 16, 32, 64] {
        let mask = u64::MAX >> (64 - bits);
        for signed in [false, true] {
            let scalar = source_constant_scalar_v18(bits, signed);
            for (operation, left, right, expected) in [
                (Op::Add, 7, 9, 16), (Op::Subtract, 19, 3, 16),
                (Op::Multiply, 4, 4, 16), (Op::BitAnd, 21, 17, 17),
                (Op::BitOr, 16, 3, 19), (Op::BitXor, 21, 5, 16),
            ] {
                for overflow in [Overflow::Wrapping, Overflow::Checked] {
                    let tree = source_constant_binary_v18(operation, scalar, overflow, left, right);
                    let expected = if overflow == Overflow::Checked
                        && matches!(operation, Op::BitAnd | Op::BitOr | Op::BitXor) {
                        tree.clone()
                    } else { source_constant_node_v18(scalar, expected) };
                    assert_eq!(fold_source_constant_tree_v18(tree), expected);
                }
            }
            assert_eq!(fold_source_constant_tree_v18(source_constant_binary_v18(
                Op::Add, scalar, Overflow::Wrapping, mask, 1)), source_constant_node_v18(scalar, 0));
            let maximum = if signed { mask >> 1 } else { mask };
            let checked = source_constant_binary_v18(Op::Add, scalar, Overflow::Checked, maximum, 1);
            assert_eq!(fold_source_constant_tree_v18(checked.clone()), checked);
        }
    }
}

#[test]
fn source_constant_normalization_preserves_signed_division_remainder_and_traps() {
    use ProductionSemanticBinaryOpV2 as Op;
    use ProductionOverflowContractV2 as Overflow;
    for bits in [8, 16, 32, 64] {
        let mask = u64::MAX >> (64 - bits);
        let scalar = source_constant_scalar_v18(bits, true);
        for (op, expected) in [(Op::Divide, mask - 1), (Op::Remainder, mask)] {
            assert_eq!(fold_source_constant_tree_v18(source_constant_binary_v18(
                op, scalar, Overflow::Wrapping, mask - 6, 3)), source_constant_node_v18(scalar, expected));
            for (left, right) in [(17, 0), (1u64 << (bits - 1), mask)] {
                let invalid = source_constant_binary_v18(op, scalar, Overflow::Wrapping, left, right);
                assert_eq!(fold_source_constant_tree_v18(invalid.clone()), invalid);
            }
        }
        let unsigned = source_constant_scalar_v18(bits, false);
        assert_eq!(fold_source_constant_tree_v18(source_constant_binary_v18(
            Op::Divide, unsigned, Overflow::Wrapping, 33, 2)), source_constant_node_v18(unsigned, 16));
        assert_eq!(fold_source_constant_tree_v18(source_constant_binary_v18(
            Op::Remainder, unsigned, Overflow::Wrapping, 33, 2)), source_constant_node_v18(unsigned, 1));
    }
}

#[test]
fn source_constant_normalization_checks_shift_width_and_signed_direction() {
    use ProductionSemanticBinaryOpV2 as Op;
    use ProductionOverflowContractV2 as Overflow;
    for bits in [8, 16, 32, 64] {
        let mask = u64::MAX >> (64 - bits);
        for signed in [false, true] {
            let scalar = source_constant_scalar_v18(bits, signed);
            assert_eq!(fold_source_constant_tree_v18(source_constant_binary_v18(
                Op::ShiftLeft, scalar, Overflow::Wrapping, 1, u64::from(bits - 1))),
                source_constant_node_v18(scalar, 1u64 << (bits - 1)));
            assert_eq!(fold_source_constant_tree_v18(source_constant_binary_v18(
                Op::ShiftRight, scalar, Overflow::Wrapping, mask, 1)),
                source_constant_node_v18(scalar, if signed { mask } else { mask >> 1 }));
            for operation in [Op::ShiftLeft, Op::ShiftRight] {
                for amount in [u64::from(bits), u64::from(bits + 1), mask] {
                    let invalid = source_constant_binary_v18(operation, scalar, Overflow::Wrapping, 1, amount);
                    assert_eq!(fold_source_constant_tree_v18(invalid.clone()), invalid);
                }
            }
        }
    }
}

#[test]
fn source_constant_normalization_does_not_erase_types_symbols_or_float_semantics() {
    use ProductionSemanticBinaryOpV2 as Op;
    use ProductionOverflowContractV2 as Overflow;
    for scalar in [ProductionSemanticScalarTypeV2::Bool,
        ProductionSemanticScalarTypeV2::Float { bits: 32 },
        source_constant_scalar_v18(7, false), source_constant_scalar_v18(128, false)] {
        let tree = source_constant_binary_v18(Op::Add, scalar, Overflow::Wrapping, 1, 1);
        assert_eq!(fold_source_constant_tree_v18(tree.clone()), tree);
    }
    let scalar = source_constant_scalar_v18(8, false);
    for other in [source_constant_node_v18(source_constant_scalar_v18(16, false), 9),
        source_constant_node_v18(scalar, 256),
        NormalizedScalarExpressionV1::Symbol { symbol: 5, scalar }] {
        let tree = NormalizedScalarExpressionV1::Binary { operation: Op::Add, scalar,
            overflow: Overflow::Wrapping,
            lhs: NormalizedScalarNodeV18::legacy(source_constant_node_v18(scalar, 7)),
            rhs: NormalizedScalarNodeV18::legacy(other),
        };
        assert_eq!(fold_source_constant_tree_v18(tree.clone()), tree);
    }
}

#[test]
fn source_constant_normalization_folds_nested_constants_without_replacing_unknown_leaves() {
    let scalar = source_constant_scalar_v18(32, false);
    let sum = source_constant_binary_v18(ProductionSemanticBinaryOpV2::Add, scalar,
        ProductionOverflowContractV2::Checked, 7, 9);
    let mut tree = NormalizedScalarExpressionV1::Binary {
        operation: ProductionSemanticBinaryOpV2::Multiply, scalar,
        overflow: ProductionOverflowContractV2::Checked,
        lhs: NormalizedScalarNodeV18::legacy(sum),
        rhs: NormalizedScalarNodeV18::legacy(NormalizedScalarExpressionV1::Symbol { symbol: 3, scalar }),
    };
    let mut charge = UnsupportedIndexCorrelationBudgetV1 { remaining: 5 };
    source_scalar_constant_fold_v18(&mut tree, 0, &mut charge).unwrap();
    assert_eq!(charge.remaining, 0);
    let NormalizedScalarExpressionV1::Binary { lhs, rhs, .. } = tree else { panic!("unknown leaf was erased"); };
    assert_eq!(*lhs, source_constant_node_v18(scalar, 16));
    assert_eq!(*rhs, NormalizedScalarExpressionV1::Symbol { symbol: 3, scalar });
}

#[test]
fn source_constant_normalization_has_exact_work_and_depth_boundaries() {
    let scalar = source_constant_scalar_v18(32, false);
    let make = || source_constant_binary_v18(ProductionSemanticBinaryOpV2::Add, scalar,
        ProductionOverflowContractV2::Checked, 7, 9);
    for (limit, accepted) in [(3, true), (2, false)] {
        let mut charge = UnsupportedIndexCorrelationBudgetV1 { remaining: limit };
        let mut tree = make();
        assert_eq!(source_scalar_constant_fold_v18(&mut tree, 0, &mut charge).is_some(), accepted);
        assert_eq!(charge.remaining, 0);
        if accepted { assert_eq!(tree, source_constant_node_v18(scalar, 16)); }
        else { assert_eq!(tree, make()); }
    }
    let mut charge = UnsupportedIndexCorrelationBudgetV1 { remaining: 2 };
    let mut leaf = source_constant_node_v18(scalar, 16);
    assert!(source_scalar_constant_fold_v18(&mut leaf,
        MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2, &mut charge).is_some());
    assert!(source_scalar_constant_fold_v18(&mut leaf,
        MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1, &mut charge).is_none());
    assert_eq!(charge.remaining, 0);
}

#[test]
fn source_constant_normalization_frame_has_an_independent_header_equation() {
    use fe2o3_kernel_ir::scalar_ops_v2 as scalar;
    let frame = size_of::<&mut NormalizedScalarExpressionV1>()
        + size_of::<NormalizedScalarExpressionV1>()
        + 2 * size_of::<Option<&mut NormalizedScalarExpressionV1>>()
        + size_of::<Option<(&mut NormalizedScalarExpressionV1, &mut NormalizedScalarExpressionV1)>>()
        + size_of::<&mut dyn CorrelationChargeV18>() + 2 * size_of::<usize>()
        + size_of::<&mut SourceCorrelationChargeV18<'_, '_, '_, '_>>()
            .max(size_of::<&mut SourceTranslationChargeV18<'_, '_, '_, '_>>())
        + size_of::<Option<usize>>()
        + 3 * size_of::<&mut NormalizedScalarNodeV18>()
        + size_of::<&mut ProductionSemanticBinaryOpV2>()
        + size_of::<&mut ProductionSemanticScalarTypeV2>()
        + size_of::<&mut ProductionOverflowContractV2>()
        + 2 * size_of::<&NormalizedScalarExpressionV1>()
        + 2 * size_of::<&ProductionSemanticScalarTypeV2>() + 2 * size_of::<&u64>()
        + 3 * size_of::<ProductionSemanticScalarTypeV2>()
        + size_of::<ProductionSemanticBinaryOpV2>() + size_of::<ProductionOverflowContractV2>()
        + size_of::<scalar::ScalarType>() + size_of::<scalar::IntWidth>()
        + size_of::<scalar::IntBinary>() + size_of::<scalar::IntMode>()
        + size_of::<scalar::ShiftDirection>() + size_of::<scalar::ShiftPolicy>()
        + size_of::<scalar::Operation>() + size_of::<scalar::FloatCapabilities>()
        + size_of::<Vec<scalar::Diagnostic>>() + size_of::<Result<(), Vec<scalar::Diagnostic>>>()
        + size_of::<Option<scalar::IntOutcome>>() + size_of::<Option<u64>>() + size_of::<Option<()>>()
        + size_of::<Result<u64, std::num::TryFromIntError>>()
        + 3 * size_of::<u64>() + 3 * size_of::<u128>() + size_of::<u16>() + size_of::<bool>();
    let evaluator = size_of::<(scalar::ScalarType, scalar::IntBinary, scalar::IntMode, u128, u128)>()
        + size_of::<(scalar::ScalarType, scalar::ScalarType, scalar::ShiftDirection,
            scalar::ShiftPolicy, u128, u128)>()
        + 4 * size_of::<Option<(scalar::IntWidth, bool)>>()
        + 4 * size_of::<(scalar::IntWidth, bool)>() + 2 * size_of::<(i128, i128)>()
        + 2 * size_of::<(i128, bool)>() + 2 * size_of::<(u128, bool)>()
        + 2 * size_of::<(u128, bool, u128)>()
        + 2 * size_of::<Option<scalar::IntOutcome>>() + 2 * size_of::<scalar::IntOutcome>()
        + 4 * size_of::<scalar::IntWidth>() + 2 * size_of::<scalar::ScalarType>()
        + 20 * size_of::<u128>() + 12 * size_of::<i128>()
        + 8 * size_of::<bool>() + 4 * size_of::<u16>() + 2 * size_of::<u32>();
    assert_eq!(source_scalar_integer_evaluator_headers_v18().unwrap(), evaluator);
    assert_eq!(source_scalar_constant_fold_headers_v18().unwrap(),
        (MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1) * frame + evaluator);
}

#[test]
fn source_constant_normalization_fixed_scratch_restores_floors_and_refuses_one_short() {
    fn run(limit: usize) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1024);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let entered = std::cell::Cell::new(false);
        let result = with_scoped_source_cleanup_v29(&mut budget, MODULE_FLOOR, |cleanup, budget| {
            let floor = budget.storage();
            source_scalar_normalization_scratch_v18(cleanup, budget,
                source_scalar_constant_fold_headers_v18()?, |budget| {
                    entered.set(true);
                    assert!(budget.storage() >= floor + source_scalar_constant_fold_headers_v18()?);
                    Ok(())
                })
        });
        (result, budget.storage(), budget.peak_storage(), entered.get())
    }
    let (result, retained, peak, entered) = run(MODULE_LIMIT);
    result.unwrap();
    assert!(entered);
    assert_eq!(retained, MODULE_FLOOR);
    let (result, retained, exact, entered) = run(peak);
    result.unwrap();
    assert!(entered);
    assert_eq!((retained, exact), (MODULE_FLOOR, peak));
    let (result, retained, _, entered) = run(peak - 1);
    assert!(!entered);
    assert!(matches!(result, Err(ProductionSourceOwnedViewErrorV18::Resource(
        ArgumentResourceV1::Storage(_)))));
    assert_eq!(retained, MODULE_FLOOR);
}

#[test]
fn source_constant_entry_endpoints_have_exact_and_one_short_whole_transactions() {
    fn run(work_limit: usize, storage_limit: usize)
        -> (SourceOwnedResultV18<()>, usize, usize, bool, bool)
    {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(typed_entry_rhs_owner_v18, &mut budget);
        let result = with_production_optimized_consumer_v18(prepared, &mut budget,
            |original, optimized, budget| {
                original.with_optimized_source_scalar_leaves_v18(optimized, 0, budget,
                    |leaves, budget| leaves.with_checked_entry_writes_v18(budget,
                        |request, budget| check_fixture_entry_rhs_v18(leaves, request, budget),
                        |checked, budget| {
                            checked.check_for(original, optimized, 0, budget)?;
                            assert_eq!(checked.rows.len(), 2);
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        }))
            });
        assert_eq!(budget.storage(), MODULE_FLOOR);
        (result, budget.work(), budget.peak_storage(),
            budget.failed_work().is_some(), budget.failed_storage().is_some())
    }
    let (result, work, peak, denied_work, denied_storage) =
        run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert!(!denied_work && !denied_storage);
    let (result, exact_work, exact_peak, denied_work, denied_storage) = run(work, peak);
    result.unwrap();
    assert_eq!((exact_work, exact_peak, denied_work, denied_storage), (work, peak, false, false));
    for (work_limit, storage_limit, expected) in [(work - 1, peak, (true, false)),
        (work, peak - 1, (false, true))] {
        let (result, _, _, denied_work, denied_storage) = run(work_limit, storage_limit);
        assert!(result.is_err(), "one-short complete typed entry endpoint transaction");
        assert_eq!((denied_work, denied_storage), expected);
    }
}
