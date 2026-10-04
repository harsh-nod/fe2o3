use super::*;
use dialect_kernel::{DYNAMIC_EXTENT, IndexBinaryKindAttr, MemorySpaceAttr};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_pliron::{
    ProductionNumericalContractV2, ProductionOverflowContractV2, ProductionRankedBlockV1 as Block,
    ProductionRankedValueIdV1 as Id, ProductionSemanticBinaryOpV2, ProductionSemanticScalarTypeV2,
};

// These are constructor-valid inert recipes exercising the structural checker.
// They do not forge a sealed canonical/source binding or authenticate translation.
// Full public-query tests use the actual prepared-source owner in the backend.
fn local(id: u32) -> Value {
    Value::Local(Id::new(id))
}

const EXTENT: Value = Value::Argument(0);
const WRITE: ProductionGpuWriteSiteV2 = ProductionGpuWriteSiteV2::new(2, 0);

fn write() -> Op {
    Op::ValueAccess {
        kind: AccessKindAttr::Write,
        view: local(2),
        indices: vec![local(1)],
        value: local(3),
    }
}

fn guard(true_block: u32, false_block: u32) -> Term {
    Term::IndexLessThan {
        lhs: local(1),
        rhs: EXTENT,
        true_block,
        false_block,
    }
}

fn blocks() -> Vec<Block> {
    vec![
        Block::new(
            vec![
                Op::IndexUnknown { result: Id::new(0) },
                Op::InvocationIndex {
                    result: Id::new(1),
                    dimension: 0,
                    launch_extent: 0,
                },
                Op::ViewInSpace {
                    result: Id::new(2),
                    element_width: 32,
                    writable: true,
                    shape: vec![DYNAMIC_EXTENT],
                    dynamic_extents: vec![EXTENT],
                    memory_space: MemorySpaceAttr::Global,
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                Op::SemanticConstant {
                    result: Id::new(3),
                    value: 7,
                },
                Op::IndexConstant {
                    result: Id::new(4),
                    value: 0,
                },
                Op::IndexConstant {
                    result: Id::new(5),
                    value: 1,
                },
            ],
            Term::Branch { target: 3 },
        ),
        Block::new(vec![], Term::Return),
        Block::new(vec![write()], Term::Branch { target: 1 }),
        Block::new(vec![], guard(2, 1)),
    ]
}

fn set_terminator(blocks: &mut [Block], block: usize, terminator: Term) {
    blocks[block] = Block::with_index_arguments(
        blocks[block].index_argument_count(),
        blocks[block].operations().to_vec(),
        terminator,
    );
}

fn set_operations(blocks: &mut [Block], block: usize, operations: Vec<Op>) {
    blocks[block] = Block::with_index_arguments(
        blocks[block].index_argument_count(),
        operations,
        blocks[block].terminator().clone(),
    );
}

fn construct(blocks: Vec<Block>) -> ProductionRankedKernelV1 {
    ProductionRankedKernelV1::new("inert_conditional_recipe", 2, blocks)
        .expect("a genuine constructor-valid recipe, not a checker refusal")
}

fn query(kernel: &ProductionRankedKernelV1) -> Result<[u32; 2]> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    let result = check_paths(kernel, local(1), EXTENT, WRITE, &mut budget);
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.peak_storage(), 0);
    result
}

#[test]
fn dynamic_single_write_accepts_non_topological_block_order() {
    let kernel = construct(blocks());
    assert_eq!(query(&kernel), Ok([1, 1]));
    // Ranked block IDs are ordinals by construction; canonical sparse BlockIds
    // remain the responsibility of the reused exact store/source join.
    assert_eq!(kernel.blocks()[0].terminator(), &Term::Branch { target: 3 });
}

fn input_guard_recipe(
    global_read: bool,
) -> (
    ProductionRankedKernelV1,
    fe2o3_pliron::ProductionConditionalRankedReadBoundV1,
) {
    use fe2o3_kernel_ir::ConditionalTotalViewAddressDomainV1 as Domain;
    let mut blocks = blocks();
    let mut entry = blocks[0].operations().to_vec();
    entry.push(Op::ViewInSpace {
        result: Id::new(6),
        element_width: 32,
        writable: false,
        shape: vec![DYNAMIC_EXTENT],
        dynamic_extents: vec![Value::Argument(1)],
        memory_space: MemorySpaceAttr::Global,
        allocation_origin: 2,
        noalias_class: 2,
    });
    set_operations(&mut blocks, 0, entry);
    let read = Op::Access {
        kind: AccessKindAttr::Read,
        view: local(6),
        indices: vec![local(1)],
    };
    blocks.push(Block::new(
        vec![],
        Term::IndexLessThan {
            lhs: local(1),
            rhs: Value::Argument(1),
            true_block: 5,
            false_block: 6,
        },
    ));
    blocks.push(Block::new(
        vec![read],
        Term::Branch {
            target: if global_read { 3 } else { 2 },
        },
    ));
    blocks.push(Block::new(vec![], Term::Trap));
    if global_read {
        set_terminator(&mut blocks, 0, Term::Branch { target: 4 });
    } else {
        set_terminator(&mut blocks, 3, guard(4, 1));
    }
    (
        construct(blocks),
        fe2o3_pliron::ProductionConditionalRankedReadBoundV1 {
            block: 5,
            operation: 0,
            view: local(6),
            index: local(1),
            extent: Value::Argument(1),
            domain: if global_read {
                Domain::GlobalLaunch
            } else {
                Domain::GuardedOutput
            },
        },
    )
}

fn input_query(
    kernel: &ProductionRankedKernelV1,
    reads: &[fe2o3_pliron::ProductionConditionalRankedReadBoundV1],
    budget: &mut Budget<'_>,
) -> std::result::Result<[u32; 2], fe2o3_pliron::ProductionRankedRecipeCoverageErrorV1> {
    fe2o3_pliron::check_ranked_recipe_paths_with_input_bounds_v1(
        kernel,
        local(1),
        EXTENT,
        WRITE,
        reads,
        budget,
    )
}

#[test]
fn input_guard_coverage_keeps_output_and_global_domains_distinct() {
    use fe2o3_kernel_ir::ConditionalTotalViewAddressDomainV1 as Domain;
    for global in [false, true] {
        let (kernel, read) = input_guard_recipe(global);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(37).unwrap();
        assert_eq!(input_query(&kernel, &[read], &mut budget), Ok([1, 1]));
        assert_eq!(budget.storage(), 37);
        let mut wrong = read;
        wrong.domain = if global {
            Domain::GuardedOutput
        } else {
            Domain::GlobalLaunch
        };
        assert!(input_query(&kernel, &[wrong], &mut budget).is_err());
        // The false output case is exercised even for an empty output. A read
        // before the output guard therefore retains its whole-launch premise.
        assert!(query(&kernel).is_err());
    }
}

#[test]
fn input_guard_coverage_rejects_wrong_read_coordinates_and_complete_roster_omissions() {
    let (kernel, read) = input_guard_recipe(false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    for change in 0..6 {
        let mut wrong = read;
        match change {
            0 => wrong.view = local(2),
            1 => wrong.index = local(4),
            2 => wrong.extent = EXTENT,
            3 => wrong.block = 2,
            4 => wrong.operation = 1,
            _ => wrong.extent = local(5),
        }
        assert!(input_query(&kernel, &[wrong], &mut budget).is_err());
    }
    assert!(input_query(&kernel, &[], &mut budget).is_err());
    assert!(input_query(&kernel, &[read, read], &mut budget).is_err());
    // This fixture's executed load is unused by the store value and is still
    // part of the mandatory read roster.
    assert_eq!(input_query(&kernel, &[read], &mut budget), Ok([1, 1]));
}

#[test]
fn input_guard_coverage_does_not_hide_inverted_shifted_or_missing_trap_guards() {
    for change in 0..4 {
        let (kernel, read) = input_guard_recipe(false);
        let mut blocks = kernel.blocks().to_vec();
        if change == 2 {
            let mut entry = blocks[0].operations().to_vec();
            entry.push(Op::IndexBinary {
                result: Id::new(7),
                kind: IndexBinaryKindAttr::Add,
                lhs: local(1),
                rhs: local(5),
            });
            set_operations(&mut blocks, 0, entry);
        }
        let terminator = match change {
            0 => Term::IndexLessThan {
                lhs: local(1),
                rhs: Value::Argument(1),
                true_block: 6,
                false_block: 5,
            },
            1 | 2 => Term::IndexLessThan {
                lhs: if change == 1 { local(4) } else { local(7) },
                rhs: Value::Argument(1),
                true_block: 5,
                false_block: 6,
            },
            _ => Term::Branch { target: 6 },
        };
        set_terminator(&mut blocks, 4, terminator);
        let kernel = construct(blocks);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        assert!(input_query(&kernel, &[read], &mut budget).is_err());
    }
}

#[test]
fn input_guard_coverage_preserves_the_original_account_and_exact_limits() {
    let (kernel, read) = input_guard_recipe(true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(37).unwrap();
    budget.charge_work(11).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    assert!(input_query(&kernel, &[read], &mut budget).is_ok());
    let used = budget.work();
    let peak = budget.peak_storage();
    assert_eq!(budget.storage(), 37);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert!(input_query(&kernel, &[read], &mut budget).is_ok());
    assert_eq!(budget.work(), 11 + 2 * (used - 11));
    assert_eq!(budget.peak_storage(), peak);
    assert_eq!(budget.storage(), 37);
    assert!(budget.work_ledger_identity_v1() == ledger);
    for (work_limit, storage_limit, accepted) in [
        (used, peak, true),
        (used - 1, peak, false),
        (used, peak - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(37).unwrap();
        budget.charge_work(11).unwrap();
        assert_eq!(input_query(&kernel, &[read], &mut budget).is_ok(), accepted);
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn different_normal_exits_and_access_without_value_remain_structural() {
    let mut blocks = blocks();
    set_operations(
        &mut blocks,
        2,
        vec![Op::Access {
            kind: AccessKindAttr::Write,
            view: local(2),
            indices: vec![local(1)],
        }],
    );
    set_terminator(&mut blocks, 2, Term::Return);
    assert_eq!(query(&construct(blocks)), Ok([1, 2]));
}

#[test]
fn selected_write_value_is_not_reference_value_equivalence() {
    let mut blocks = blocks();
    let mut entry = blocks[0].operations().to_vec();
    entry[3] = Op::SemanticConstant {
        result: Id::new(3),
        value: u64::MAX,
    };
    set_operations(&mut blocks, 0, entry);
    assert_eq!(query(&construct(blocks)), Ok([1, 1]));
}

#[test]
fn constant_equal_branches_use_both_exact_truth_values() {
    for (rhs, true_block, false_block) in [(local(4), 3, 4), (local(5), 4, 3)] {
        let mut blocks = blocks();
        set_terminator(
            &mut blocks,
            0,
            Term::IndexEqual {
                lhs: local(4),
                rhs,
                true_block,
                false_block,
            },
        );
        blocks.push(Block::new(vec![], Term::Trap));
        assert_eq!(query(&construct(blocks)), Ok([1, 1]));
    }
}

#[test]
fn constant_less_than_is_unsigned_including_maximum_literal() {
    for (lhs, rhs, true_block, false_block) in
        [(local(4), local(5), 3, 4), (local(5), local(4), 4, 3)]
    {
        let mut blocks = blocks();
        let mut entry = blocks[0].operations().to_vec();
        entry[5] = Op::IndexConstant {
            result: Id::new(5),
            value: u64::MAX,
        };
        set_operations(&mut blocks, 0, entry);
        set_terminator(
            &mut blocks,
            0,
            Term::IndexLessThan {
                lhs,
                rhs,
                true_block,
                false_block,
            },
        );
        blocks.push(Block::new(vec![], Term::Trap));
        assert_eq!(query(&construct(blocks)), Ok([1, 1]));
    }
}

#[test]
fn constant_edge_to_trap_is_not_silently_discarded() {
    let mut blocks = blocks();
    set_terminator(
        &mut blocks,
        0,
        Term::IndexEqual {
            lhs: local(4),
            rhs: local(5),
            true_block: 3,
            false_block: 4,
        },
    );
    blocks.push(Block::new(vec![], Term::Trap));
    assert_eq!(
        query(&construct(blocks)),
        Err(Error::AbnormalExit {
            block: 4,
            predicate: false,
        })
    );
}

#[test]
fn repeated_exact_guard_uses_one_consistent_predicate_value() {
    let mut blocks = blocks();
    set_terminator(&mut blocks, 0, guard(3, 1));
    assert_eq!(query(&construct(blocks)), Ok([1, 1]));
}

#[test]
fn bypass_and_reversed_polarity_refuse_coverage() {
    let mut bypass = blocks();
    set_terminator(&mut bypass, 0, Term::Branch { target: 1 });
    assert_eq!(
        query(&construct(bypass)),
        Err(Error::WriteCount {
            block: 1,
            predicate: true,
        })
    );
    let mut reverse = blocks();
    set_terminator(&mut reverse, 3, guard(1, 2));
    assert_eq!(
        query(&construct(reverse)),
        Err(Error::WriteCount {
            block: 2,
            predicate: false,
        })
    );
}

#[test]
fn shared_successors_do_not_hide_bypass_or_tail_write() {
    for target in [1, 2] {
        let mut blocks = blocks();
        set_terminator(&mut blocks, 3, guard(target, target));
        assert_eq!(
            query(&construct(blocks)),
            Err(Error::WriteCount {
                block: target,
                predicate: target == 1,
            })
        );
    }
}

#[test]
fn unknown_equal_and_less_than_are_refused_even_with_shared_targets() {
    for equal in [false, true] {
        let mut blocks = blocks();
        let term = if equal {
            Term::IndexEqual {
                lhs: local(0),
                rhs: local(4),
                true_block: 3,
                false_block: 3,
            }
        } else {
            Term::IndexLessThan {
                lhs: Value::Argument(1),
                rhs: local(5),
                true_block: 3,
                false_block: 3,
            }
        };
        set_terminator(&mut blocks, 0, term);
        assert_eq!(
            query(&construct(blocks)),
            Err(Error::UnresolvedCondition { block: 0 })
        );
    }
}

#[test]
fn different_extent_or_swapped_operands_are_not_the_selected_guard() {
    for (lhs, rhs) in [(local(1), Value::Argument(1)), (EXTENT, local(1))] {
        let mut blocks = blocks();
        set_terminator(
            &mut blocks,
            3,
            Term::IndexLessThan {
                lhs,
                rhs,
                true_block: 2,
                false_block: 1,
            },
        );
        assert_eq!(
            query(&construct(blocks)),
            Err(Error::UnresolvedCondition { block: 3 })
        );
    }
}

#[test]
fn feasible_cycles_before_and_after_write_are_refused() {
    let mut blocks_before = blocks();
    set_terminator(&mut blocks_before, 1, Term::Branch { target: 1 });
    assert_eq!(
        query(&construct(blocks_before)),
        Err(Error::Cycle { predicate: false })
    );
    let mut blocks_after = blocks();
    set_terminator(&mut blocks_after, 2, Term::Branch { target: 2 });
    assert_eq!(
        query(&construct(blocks_after)),
        Err(Error::WriteCount {
            block: 2,
            predicate: true,
        })
    );
}

#[test]
fn exact_infeasible_cycle_does_not_turn_into_a_feasible_path() {
    let mut blocks = blocks();
    set_terminator(
        &mut blocks,
        0,
        Term::IndexEqual {
            lhs: local(4),
            rhs: local(4),
            true_block: 3,
            false_block: 4,
        },
    );
    blocks.push(Block::new(vec![], Term::Branch { target: 4 }));
    assert_eq!(query(&construct(blocks)), Ok([1, 1]));
}

#[test]
fn false_case_trap_and_true_case_trap_after_write_are_abnormal() {
    for (block, predicate) in [(1, false), (2, true)] {
        let mut blocks = blocks();
        set_terminator(&mut blocks, block, Term::Trap);
        assert_eq!(
            query(&construct(blocks)),
            Err(Error::AbnormalExit {
                block: block as u32,
                predicate,
            })
        );
    }
}

#[test]
fn second_write_and_dead_write_effects_are_refused() {
    for (block, kind) in [(2, AccessKindAttr::Write), (4, AccessKindAttr::Write)] {
        let mut blocks = blocks();
        if block == 4 {
            blocks.push(Block::new(vec![], Term::Return));
        }
        let mut operations = blocks[block].operations().to_vec();
        let operation = operations.len() as u32;
        operations.push(Op::Access {
            kind,
            view: local(2),
            indices: vec![local(1)],
        });
        set_operations(&mut blocks, block, operations);
        assert_eq!(
            query(&construct(blocks)),
            Err(Error::UnsupportedOperation {
                block: block as u32,
                operation,
            })
        );
    }
}

#[test]
fn input_read_does_not_count_as_an_output_write_or_prove_its_bounds() {
    let mut blocks = blocks();
    let mut operations = blocks[2].operations().to_vec();
    operations.push(Op::Access {
        kind: AccessKindAttr::Read,
        view: local(2),
        indices: vec![local(1)],
    });
    set_operations(&mut blocks, 2, operations);
    assert_eq!(query(&construct(blocks)), Ok([1, 1]));
}

#[test]
fn index_arithmetic_including_overflow_and_division_is_not_whitelisted() {
    for kind in [
        IndexBinaryKindAttr::Add,
        IndexBinaryKindAttr::Multiply,
        IndexBinaryKindAttr::Divide,
        IndexBinaryKindAttr::Remainder,
    ] {
        let mut blocks = blocks();
        let mut entry = blocks[0].operations().to_vec();
        entry.push(Op::IndexBinary {
            result: Id::new(6),
            kind,
            lhs: local(0),
            rhs: local(4),
        });
        set_operations(&mut blocks, 0, entry);
        assert_eq!(
            query(&construct(blocks)),
            Err(Error::UnsupportedOperation {
                block: 0,
                operation: 6,
            })
        );
    }
}

#[test]
fn total_ieee_expression_does_not_change_write_coverage() {
    let scalar = ProductionSemanticScalarTypeV2::Float { bits: 32 };
    let expression = Expression::Binary {
        operation: ProductionSemanticBinaryOpV2::Add,
        scalar,
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(Expression::Symbol { symbol: 1, scalar }),
        rhs: Box::new(Expression::Constant { scalar, bits: 0 }),
    };
    let mut blocks = blocks();
    blocks.push(Block::new(
        vec![Op::SemanticExpression {
            result: Id::new(6),
            expression,
            numerical_contract: ProductionNumericalContractV2::exact_for(scalar),
        }],
        Term::Return,
    ));
    assert_eq!(query(&construct(blocks)), Ok([1, 1]));
}

#[test]
fn literal_and_symbol_expressions_are_not_arithmetic_proofs() {
    let mut blocks = blocks();
    let scalar = ProductionSemanticScalarTypeV2::Float { bits: 32 };
    let mut entry = blocks[0].operations().to_vec();
    for (result, expression) in [
        (
            6,
            Expression::Constant {
                scalar,
                bits: u64::from(f32::NAN.to_bits()),
            },
        ),
        (7, Expression::Symbol { symbol: 8, scalar }),
    ] {
        entry.push(Op::SemanticExpression {
            result: Id::new(result),
            expression,
            numerical_contract: ProductionNumericalContractV2::exact_for(scalar),
        });
    }
    set_operations(&mut blocks, 0, entry);
    assert_eq!(query(&construct(blocks)), Ok([1, 1]));
}

#[test]
fn analysis_split_and_live_block_arguments_are_outside_the_fragment() {
    let mut split = blocks();
    set_terminator(
        &mut split,
        0,
        Term::AnalysisSplit {
            control_dependencies: vec![local(0)],
            first_block: 3,
            second_block: 1,
        },
    );
    assert_eq!(
        query(&construct(split)),
        Err(Error::UnsupportedTerminator { block: 0 })
    );
    let mut arguments = blocks();
    set_terminator(
        &mut arguments,
        0,
        Term::BranchArgs {
            arguments: vec![local(4)],
            target: 4,
        },
    );
    arguments.push(Block::with_index_arguments(
        1,
        vec![],
        Term::Branch { target: 3 },
    ));
    assert_eq!(
        query(&construct(arguments)),
        Err(Error::UnsupportedTerminator { block: 0 })
    );
}

#[test]
fn dead_unresolved_condition_is_still_outside_the_supported_fragment() {
    let mut blocks = blocks();
    blocks.push(Block::new(
        vec![],
        Term::IndexEqual {
            lhs: Value::Argument(1),
            rhs: local(4),
            true_block: 1,
            false_block: 1,
        },
    ));
    assert_eq!(
        query(&construct(blocks)),
        Err(Error::UnresolvedCondition { block: 4 })
    );
}

#[test]
fn constructor_rejects_malformed_definitions_edges_and_index_stored_as_value() {
    let mut malformed = Vec::new();
    let mut duplicate = blocks();
    let mut entry = duplicate[0].operations().to_vec();
    entry.push(Op::IndexConstant {
        result: Id::new(5),
        value: 2,
    });
    set_operations(&mut duplicate, 0, entry);
    malformed.push(duplicate);
    let mut missing_edge = blocks();
    set_terminator(&mut missing_edge, 0, Term::Branch { target: u32::MAX });
    malformed.push(missing_edge);
    let mut semantic_index = blocks();
    set_operations(
        &mut semantic_index,
        2,
        vec![Op::ValueAccess {
            kind: AccessKindAttr::Write,
            view: local(2),
            indices: vec![local(1)],
            value: EXTENT,
        }],
    );
    malformed.push(semantic_index);
    let mut malformed_float = blocks();
    let mut entry = malformed_float[0].operations().to_vec();
    entry.push(Op::SemanticExpression {
        result: Id::new(6),
        expression: Expression::Constant {
            scalar: ProductionSemanticScalarTypeV2::Float { bits: 16 },
            bits: 0,
        },
        numerical_contract: ProductionNumericalContractV2::exact_for(
            ProductionSemanticScalarTypeV2::Float { bits: 16 },
        ),
    });
    set_operations(&mut malformed_float, 0, entry);
    malformed.push(malformed_float);
    for blocks in malformed {
        assert!(ProductionRankedKernelV1::new("not_a_checker_input", 2, blocks).is_err());
    }
}

#[test]
fn shared_work_boundary_is_exact_and_no_scratch_or_retained_storage_is_added() {
    let kernel = construct(blocks());
    let prefix = 17;
    let floor = 43;
    let measure = |limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, floor);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(prefix).unwrap();
        let result = check_paths(&kernel, local(1), EXTENT, WRITE, &mut budget);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        (result, budget.work())
    };
    let (result, exact_work) = measure(usize::MAX);
    assert_eq!(result, Ok([1, 1]));
    assert!(exact_work > prefix);
    assert_eq!(measure(exact_work), (Ok([1, 1]), exact_work));
    let (short, accepted_work) = measure(exact_work - 1);
    assert!(matches!(
        short,
        Err(Error::Resource(ResourceError::Work(_)))
    ));
    assert!(accepted_work >= prefix && accepted_work < exact_work);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, floor - 1);
    assert!(matches!(
        budget.reserve_storage(floor),
        Err(ResourceError::Storage(_))
    ));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn literal_lookup_exhaustion_after_structural_scan_preserves_storage() {
    let mut blocks = blocks();
    set_terminator(
        &mut blocks,
        0,
        Term::IndexEqual {
            lhs: local(4),
            rhs: local(5),
            true_block: 1,
            false_block: 3,
        },
    );
    let kernel = construct(blocks);
    let prefix = 17;
    let floor = 43;
    let measure = |limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, floor);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(prefix).unwrap();
        let result = check_paths(&kernel, local(1), EXTENT, WRITE, &mut budget);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        (result, budget.work())
    };

    // Only the entry condition scans literals: one lookup/block charge plus
    // two units per operation through the fifth and sixth entry operations.
    let lhs_lookup = 1 + 1 + 2 * 5;
    let rhs_lookup = 1 + 1 + 2 * 6;
    let structural_scan = kernel
        .blocks()
        .iter()
        .map(|block| 4 + 4 * block.operations().len() + 6)
        .sum::<usize>()
        + lhs_lookup
        + rhs_lookup;
    // Finish the structural scan, enter the false-case walk, and resolve its
    // first literal. The second lookup visits two operations before exhaustion.
    let before_rhs = prefix + structural_scan + 4 + 6 + lhs_lookup;
    let short_limit = before_rhs + 1 + 1 + 2 * 2;
    let (result, accepted_work) = measure(short_limit);
    let Err(Error::Resource(ResourceError::Work(error))) = result else {
        panic!("expected literal-scan work refusal, got {result:?}");
    };
    assert_eq!(error.limit(), short_limit);
    assert_eq!(error.actual(), short_limit + 2);
    assert_eq!(accepted_work, short_limit);

    let (result, exact_work) = measure(usize::MAX);
    assert_eq!(result, Ok([1, 1]));
    assert!(exact_work > short_limit);
    assert_eq!(measure(exact_work), (Ok([1, 1]), exact_work));
}

#[test]
fn constant_lookup_and_late_refusals_preserve_the_callers_storage() {
    let mut blocks = blocks();
    set_terminator(
        &mut blocks,
        0,
        Term::IndexEqual {
            lhs: local(4),
            rhs: local(5),
            true_block: 4,
            false_block: 3,
        },
    );
    blocks.push(Block::new(vec![], Term::Trap));
    set_terminator(&mut blocks, 2, Term::Trap);
    let kernel = construct(blocks);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 31);
    budget.reserve_storage(31).unwrap();
    assert_eq!(
        check_paths(&kernel, local(1), EXTENT, WRITE, &mut budget),
        Err(Error::AbnormalExit {
            block: 2,
            predicate: true
        })
    );
    assert_eq!(budget.storage(), 31);
    assert_eq!(budget.peak_storage(), 31);
    assert!(budget.work() > 0);
}

#[test]
fn literal_scan_preserves_first_refusal_and_exact_work_before_dead_admission() {
    let unresolved = Error::UnresolvedCondition { block: 0 };
    let unsupported = Error::UnsupportedOperation {
        block: 4,
        operation: 0,
    };
    for bad in [
        write(),
        Op::Access {
            kind: AccessKindAttr::Write,
            view: local(2),
            indices: vec![local(1)],
        },
        Op::IndexBinary {
            result: Id::new(6),
            kind: IndexBinaryKindAttr::Divide,
            lhs: local(0),
            rhs: local(4),
        },
    ] {
        for (lhs, rhs, exact, last_charge, expected) in [
            (local(0), local(4), 85, 2, unresolved),
            (local(4), local(0), 85, 2, unresolved),
            (Value::Argument(1), local(0), 74, 2, unresolved),
            (local(4), local(5), 119, 4, unsupported),
        ] {
            for equal in [false, true] {
                let mut b = blocks();
                b.push(Block::new(vec![bad.clone()], Term::Return));
                let term = if equal {
                    Term::IndexEqual {
                        lhs,
                        rhs,
                        true_block: 3,
                        false_block: 3,
                    }
                } else {
                    Term::IndexLessThan {
                        lhs,
                        rhs,
                        true_block: 3,
                        false_block: 3,
                    }
                };
                set_terminator(&mut b, 0, term);
                let kernel = construct(b);
                for limit in [exact - 1, exact] {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                    {
                        let mut budget = Budget::new(&mut work, 43);
                        budget.reserve_storage(43).unwrap();
                        budget.charge_work(17).unwrap();
                        let result = check_paths(&kernel, local(1), EXTENT, WRITE, &mut budget);
                        if limit == exact {
                            assert_eq!(result, Err(expected));
                        } else {
                            let Err(Error::Resource(ResourceError::Work(e))) = result else {
                                panic!("{result:?}");
                            };
                            assert_eq!((e.limit(), e.actual()), (limit, exact));
                        }
                        assert_eq!(
                            budget.work(),
                            if limit == exact {
                                exact
                            } else {
                                exact - last_charge
                            }
                        );
                        assert_eq!(
                            (
                                budget.storage(),
                                budget.peak_storage(),
                                budget.failed_storage()
                            ),
                            (43, 43, None)
                        );
                    }
                    assert_eq!(work.failed_work(), (limit < exact).then_some(exact));
                }
            }
        }
    }
}

#[test]
fn recipe_query_preserves_exact_work_and_retained_floor_boundaries() {
    let kernel = construct(blocks());
    for (work_limit, storage_limit) in [(155, 43), (154, 43), (155, 42)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        {
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.charge_work(17).unwrap();
            if storage_limit == 42 {
                let Err(ResourceError::Storage(e)) = budget.reserve_storage(43) else {
                    panic!("floor admitted");
                };
                assert_eq!((e.limit(), e.actual()), (42, 43));
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_storage()
                    ),
                    (17, 0, 0, Some(43))
                );
                continue;
            }
            budget.reserve_storage(43).unwrap();
            let result = check_paths(&kernel, local(1), EXTENT, WRITE, &mut budget);
            if work_limit == 155 {
                assert_eq!(result, Ok([1, 1]));
            } else {
                let Err(Error::Resource(ResourceError::Work(e))) = result else {
                    panic!("{result:?}");
                };
                assert_eq!((e.limit(), e.actual()), (154, 155));
            }
            assert_eq!(budget.work(), if work_limit == 155 { 155 } else { 149 });
            assert_eq!(
                (
                    budget.storage(),
                    budget.peak_storage(),
                    budget.failed_storage()
                ),
                (43, 43, None)
            );
        }
        assert_eq!(work.failed_work(), (work_limit < 155).then_some(155));
    }
}
