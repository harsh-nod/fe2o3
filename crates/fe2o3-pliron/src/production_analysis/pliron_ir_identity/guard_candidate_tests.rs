fn guard_candidate_function_v1(
    context: &mut Context,
    legacy: usize,
    argument_carrying: usize,
    unrelated: usize,
) -> FuncOp {
    use dialect_kernel::{BranchOp, IndexEqualBranchOp, IndexLessThanBranchOp};
    register_dialect(context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(context).unwrap();
    dialect_proof::register_dialect(context).unwrap();
    let index = IndexType::get(context).into();
    let signature = FunctionType::get(context, vec![index, index], vec![]);
    let function = FuncOp::new(
        context,
        "guard_candidate_census".try_into().unwrap(),
        signature,
    );
    let entry = function.get_entry_block(context);
    let lhs = entry.deref(context).get_argument(0);
    let rhs = entry.deref(context).get_argument(1);
    let count = legacy + argument_carrying + unrelated;
    let mut blocks = vec![entry];
    for _ in 0..count {
        let block = BasicBlock::new(context, None, vec![]);
        block.insert_at_back(function.get_region(context), context);
        blocks.push(block);
    }
    let exit = blocks[count];
    for ordinal in 0..count {
        let next = blocks[ordinal + 1];
        let operation = if ordinal < legacy {
            IndexLessThanBranchOp::new(context, lhs, rhs, next, exit).get_operation()
        } else if ordinal < legacy + argument_carrying {
            IndexLessThanBranchArgsOp::new(context, lhs, rhs, vec![], vec![], next, exit)
                .get_operation()
        } else if ordinal % 2 == 0 {
            IndexEqualBranchOp::new(context, lhs, rhs, next, exit).get_operation()
        } else {
            BranchOp::new(context, next).get_operation()
        };
        operation.insert_at_back(blocks[ordinal], context);
    }
    ReturnOp::new(context)
        .get_operation()
        .insert_at_back(exit, context);
    function
}

#[test]
fn live_identity_census_counts_both_memory_bounds_guard_families_independently() {
    for (legacy, argument_carrying) in [(0, 0), (1, 0), (0, 1), (1, 1), (3, 2)] {
        let context = &mut Context::new();
        let function = guard_candidate_function_v1(context, legacy, argument_carrying, 4);
        let Ok(capture) = LivePlironStructuralIdentityProviderV1::new(context, &function)
            .capture_with_resource_limits_v1(
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            )
        else {
            panic!("real guard candidate identity capture failed")
        };
        assert_eq!(
            capture.input_census.memory_bounds_guard_candidates,
            legacy + argument_carrying
        );
        // The existing progress census still names only argument-carrying guards.
        assert_eq!(
            capture.input_census.index_lt_branch_candidates,
            argument_carrying
        );
        assert_eq!(capture.input_census.blocks, legacy + argument_carrying + 5);
        assert_eq!(
            capture.input_census.operations,
            legacy + argument_carrying + 5
        );
        assert_eq!(capture.snapshot.input_census, capture.input_census);
    }
}

#[test]
fn unrelated_branch_growth_does_not_invent_bounds_guard_candidates() {
    let mut counts = vec![];
    for unrelated in [2, 20, 128] {
        let context = &mut Context::new();
        let function = guard_candidate_function_v1(context, 1, 1, unrelated);
        let Ok(capture) = LivePlironStructuralIdentityProviderV1::new(context, &function)
            .capture_with_resource_limits_v1(
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            )
        else {
            panic!("real unrelated branch identity capture failed")
        };
        assert_eq!(capture.input_census.memory_bounds_guard_candidates, 2);
        assert_eq!(capture.input_census.index_lt_branch_candidates, 1);
        counts.push(capture.input_census.blocks);
    }
    assert_eq!(counts, [5, 23, 131]);
}
