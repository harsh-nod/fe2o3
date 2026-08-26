use dialect_kernel::{
    BranchArgsOp, BranchOp, CheckedTiledIndex2DOp, DIALECT_NAME, DimensionOp, IndexBinaryKindAttr,
    IndexBinaryOp, IndexConstantOp, IndexEqualBranchArgsOp, IndexLessThanBranchArgsOp, IndexType,
    InvocationIndexOp, MAX_RANKED_MEMORY_RANK, RankedViewOp, RankedViewType, ReturnOp,
    register_dialect,
};
use fe2o3_kernel_analysis::{
    MAX_SPARSE_INDEX_VALUES_V1, SparseIndexFactV1, SparseIndexFailureV1,
    analyze_pliron_sparse_indices_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
    operation::Operation,
    r#type::TypeHandle,
    value::Value,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    context
}

fn function(context: &mut Context, name: &str) -> FuncOp {
    FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    )
}

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

fn block(context: &mut Context, function: &FuncOp, name: &str) -> Ptr<BasicBlock> {
    let block = BasicBlock::new(context, Some(name.try_into().unwrap()), vec![]);
    block.insert_at_back(function.get_region(context), context);
    block
}

fn index_block(context: &mut Context, function: &FuncOp, name: &str) -> (Ptr<BasicBlock>, Value) {
    let index: TypeHandle = IndexType::get(context).into();
    let block = BasicBlock::new(context, Some(name.try_into().unwrap()), vec![index]);
    let argument = block.deref(context).get_argument(0);
    block.insert_at_back(function.get_region(context), context);
    (block, argument)
}

#[test]
fn sparse_affine_chain_propagates_only_to_its_consumers() {
    let context = &mut setup();
    let function = function(context, "affine_chain");
    let entry = function.get_entry_block(context);
    let invocation = InvocationIndexOp::new(context, 0, 16);
    let stride = IndexConstantOp::new(context, 4);
    let offset = IndexConstantOp::new(context, 3);
    let scaled = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Multiply,
        invocation.result(context),
        stride.result(context),
    );
    let address = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        scaled.result(context),
        offset.result(context),
    );
    let ret = ReturnOp::new(context);
    for operation in [
        invocation.get_operation(),
        stride.get_operation(),
        offset.get_operation(),
        scaled.get_operation(),
        address.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    let SparseIndexFactV1::Affine(fact) = analysis.fact(address.result(context)) else {
        panic!("expected affine fact");
    };
    assert_eq!(fact.constant_term(), 3);
    assert_eq!(fact.coefficients()[0], 4);
    assert_eq!(fact.evaluate(&[7]), Some(31));
    assert_eq!(fact.maximum(analysis.launch_extents()), Some(63));
}

#[test]
fn nonlinear_products_are_unknown_and_checked_overflow_has_a_witness() {
    let context = &mut setup();
    let function = function(context, "nonlinear");
    let entry = function.get_entry_block(context);
    let x = InvocationIndexOp::new(context, 0, 4);
    let y = InvocationIndexOp::new(context, 1, 4);
    let product = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Multiply,
        x.result(context),
        y.result(context),
    );
    let maximum = IndexConstantOp::new(context, u64::MAX);
    let one = IndexConstantOp::new(context, 1);
    let overflow = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        maximum.result(context),
        one.result(context),
    );
    let ret = ReturnOp::new(context);
    for operation in [
        x.get_operation(),
        y.get_operation(),
        product.get_operation(),
        maximum.get_operation(),
        one.get_operation(),
        overflow.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    assert_eq!(
        analysis.fact(product.result(context)),
        SparseIndexFactV1::Unknown
    );
    let overflow = analysis.fact(overflow.result(context));
    let overflow = overflow
        .machine_overflow()
        .expect("checked addition overflow");
    assert_eq!(overflow.operation(), IndexBinaryKindAttr::Add);
    assert_eq!(overflow.operands(), (u64::MAX, 1));
    assert_eq!(overflow.invocation(), &[3, 3]);
}

#[test]
fn dynamic_launch_does_not_hide_an_axis_independent_overflow() {
    let context = &mut setup();
    let function = function(context, "dynamic_constant_overflow");
    let entry = function.get_entry_block(context);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let static_invocation = InvocationIndexOp::new(context, 1, 3);
    let maximum = IndexConstantOp::new(context, u64::MAX);
    let one = IndexConstantOp::new(context, 1);
    let constant_overflow = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        maximum.result(context),
        one.result(context),
    );
    let runtime_dependent = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        maximum.result(context),
        invocation.result(context),
    );
    let static_dependent = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Multiply,
        maximum.result(context),
        static_invocation.result(context),
    );
    let ret = ReturnOp::new(context);
    for operation in [
        invocation.get_operation(),
        static_invocation.get_operation(),
        maximum.get_operation(),
        one.get_operation(),
        constant_overflow.get_operation(),
        runtime_dependent.get_operation(),
        static_dependent.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }

    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    let constant_overflow_fact = analysis.fact(constant_overflow.result(context));
    let overflow = constant_overflow_fact
        .machine_overflow()
        .expect("constant overflow is independent of the dynamic launch axis");
    assert_eq!(overflow.operands(), (u64::MAX, 1));
    assert_eq!(overflow.invocation(), &[0, 2]);
    assert!(
        analysis
            .fact(runtime_dependent.result(context))
            .machine_overflow()
            .is_none(),
        "the unbounded invocation axis cannot supply a concrete witness"
    );
    let static_dependent_fact = analysis.fact(static_dependent.result(context));
    let overflow = static_dependent_fact
        .machine_overflow()
        .expect("the expression is independent of the unbounded axis");
    assert_eq!(overflow.operation(), IndexBinaryKindAttr::Multiply);
    assert_eq!(overflow.operands(), (u64::MAX, 2));
    assert_eq!(overflow.invocation(), &[0, 2]);
}

#[test]
fn intermediate_overflow_is_not_erased_by_a_later_zero_scale() {
    let context = &mut setup();
    let function = function(context, "intermediate_overflow");
    let entry = function.get_entry_block(context);
    let maximum = IndexConstantOp::new(context, u64::MAX);
    let one = IndexConstantOp::new(context, 1);
    let zero = IndexConstantOp::new(context, 0);
    let overflow = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        maximum.result(context),
        one.result(context),
    );
    let collapsed = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Multiply,
        overflow.result(context),
        zero.result(context),
    );
    let ret = ReturnOp::new(context);
    for operation in [
        maximum.get_operation(),
        one.get_operation(),
        zero.get_operation(),
        overflow.get_operation(),
        collapsed.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }

    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    let collapsed = analysis.fact(collapsed.result(context));
    let retained = collapsed
        .machine_overflow()
        .expect("eager checked evaluation retains the intermediate overflow");
    assert_eq!(retained.operation(), IndexBinaryKindAttr::Add);
    assert_eq!(retained.operands(), (u64::MAX, 1));
}

#[test]
fn nonzero_remainder_is_exact_and_zero_remainder_is_unknown() {
    let context = &mut setup();
    let function = function(context, "remainder");
    let entry = function.get_entry_block(context);
    let x = InvocationIndexOp::new(context, 0, 64);
    let eight = IndexConstantOp::new(context, 8);
    let zero = IndexConstantOp::new(context, 0);
    let wrapped = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Remainder,
        x.result(context),
        eight.result(context),
    );
    let invalid = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Remainder,
        x.result(context),
        zero.result(context),
    );
    let ret = ReturnOp::new(context);
    for operation in [
        x.get_operation(),
        eight.get_operation(),
        zero.get_operation(),
        wrapped.get_operation(),
        invalid.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    assert_eq!(
        analysis.fact(wrapped.result(context)).evaluate(&[19]),
        Some(3)
    );
    assert_eq!(
        analysis.fact(wrapped.result(context)).maximum(&[64]),
        Some(7)
    );
    assert_eq!(
        analysis.fact(invalid.result(context)),
        SparseIndexFactV1::Unknown
    );
}

#[test]
fn checked_tiled_fact_preserves_a_dynamic_component_value() {
    let context = &mut setup();
    let function = function(context, "dynamic_tiled_component");
    let entry = function.get_entry_block(context);
    let invocation = InvocationIndexOp::new(context, 0, 64);
    let four = IndexConstantOp::new(context, 4);
    let sixteen = IndexConstantOp::new(context, 16);
    let component = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Remainder,
        invocation.result(context),
        four.result(context),
    );
    let tiled = CheckedTiledIndex2DOp::new(
        context,
        invocation.result(context),
        component.result(context),
        sixteen.result(context),
        sixteen.result(context),
        sixteen.result(context),
        [64, 16, 16, 4],
    );
    let ret = ReturnOp::new(context);
    for operation in [
        invocation.get_operation(),
        four.get_operation(),
        sixteen.get_operation(),
        component.get_operation(),
        tiled.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }

    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    let fact = analysis
        .fact(tiled.result(context))
        .checked_tiled_2d()
        .cloned()
        .expect("checked tiled fact");
    assert_eq!(fact.component(), component.result(context));
    assert_eq!(fact.runtime_layout(), [sixteen.result(context); 3]);
    assert_eq!(fact.geometry(), [64, 16, 16, 4]);
}

#[test]
fn static_ranked_dimension_propagates_as_a_constant() {
    let context = &mut setup();
    let function = function(context, "static_dimension");
    let entry = function.get_entry_block(context);
    let ty = RankedViewType::new(context, 32, false, vec![17]).unwrap();
    let view = RankedViewOp::new(context, ty, vec![]).unwrap();
    let dimension = DimensionOp::new(context, view.result(context), 0).unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &dimension);
    append(context, entry, &ret);
    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    assert_eq!(
        analysis.fact(dimension.result(context)).constant_value(),
        Some(17)
    );
}

#[test]
fn inconsistent_launch_contracts_fail_before_any_consumer_pass() {
    let context = &mut setup();
    let function = function(context, "inconsistent_launch");
    let entry = function.get_entry_block(context);
    let first = InvocationIndexOp::new(context, 0, 32);
    let second = InvocationIndexOp::new(context, 0, 64);
    let ret = ReturnOp::new(context);
    append(context, entry, &first);
    append(context, entry, &second);
    append(context, entry, &ret);
    assert_eq!(
        analyze_pliron_sparse_indices_v1(context, &function).unwrap_err(),
        SparseIndexFailureV1::InconsistentLaunchExtent {
            dimension: 0,
            first: 32,
            second: 64,
        }
    );
}

#[test]
fn dynamic_and_static_launch_contracts_cannot_share_one_dimension() {
    for (first, second) in [(0, 8), (8, 0)] {
        let context = &mut setup();
        let function = function(context, "mixed_launch_contract");
        let entry = function.get_entry_block(context);
        let first = InvocationIndexOp::new(context, 0, first);
        let second = InvocationIndexOp::new(context, 0, second);
        let ret = ReturnOp::new(context);
        append(context, entry, &first);
        append(context, entry, &second);
        append(context, entry, &ret);
        assert!(matches!(
            analyze_pliron_sparse_indices_v1(context, &function),
            Err(SparseIndexFailureV1::InconsistentLaunchExtent { dimension: 0, .. })
        ));
    }
}

#[test]
fn branch_arguments_propagate_sparse_facts() {
    let context = &mut setup();
    let function = function(context, "branch_argument");
    let entry = function.get_entry_block(context);
    let (join, argument) = index_block(context, &function, "join");
    let seven = IndexConstantOp::new(context, 7);
    let branch = BranchArgsOp::new(context, vec![seven.result(context)], join);
    let ret = ReturnOp::new(context);
    append(context, entry, &seven);
    append(context, entry, &branch);
    append(context, join, &ret);

    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    assert_eq!(analysis.fact(argument).constant_value(), Some(7));
}

#[test]
fn equal_typed_conditional_edges_retain_the_shared_fact() {
    let context = &mut setup();
    let function = function(context, "equal_conditional_edges");
    let entry = function.get_entry_block(context);
    let (join, argument) = index_block(context, &function, "join");
    let seven = IndexConstantOp::new(context, 7);
    let branch = IndexLessThanBranchArgsOp::new(
        context,
        seven.result(context),
        seven.result(context),
        vec![seven.result(context)],
        vec![seven.result(context)],
        join,
        join,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &seven);
    append(context, entry, &branch);
    append(context, join, &ret);

    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    assert_eq!(analysis.fact(argument).constant_value(), Some(7));
}

#[test]
fn conflicting_typed_conditional_edges_become_unknown() {
    let context = &mut setup();
    let function = function(context, "conflicting_conditional_edges");
    let entry = function.get_entry_block(context);
    let (join, argument) = index_block(context, &function, "join");
    let seven = IndexConstantOp::new(context, 7);
    let nine = IndexConstantOp::new(context, 9);
    let branch = IndexEqualBranchArgsOp::new(
        context,
        seven.result(context),
        nine.result(context),
        vec![seven.result(context)],
        vec![nine.result(context)],
        join,
        join,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &seven);
    append(context, entry, &nine);
    append(context, entry, &branch);
    append(context, join, &ret);

    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    assert_eq!(analysis.fact(argument), SparseIndexFactV1::Unknown);
}

#[test]
fn an_unknown_merge_input_is_absorbing() {
    let context = &mut setup();
    let function = function(context, "unknown_merge_input");
    let entry = function.get_entry_block(context);
    let (join, argument) = index_block(context, &function, "join");
    let seven = IndexConstantOp::new(context, 7);
    let unknown = InvocationIndexOp::new(context, MAX_RANKED_MEMORY_RANK as u32, 16);
    let branch = IndexEqualBranchArgsOp::new(
        context,
        seven.result(context),
        seven.result(context),
        vec![seven.result(context)],
        vec![unknown.result(context)],
        join,
        join,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &seven);
    append(context, entry, &unknown);
    append(context, entry, &branch);
    append(context, join, &ret);

    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    assert_eq!(analysis.fact(argument), SparseIndexFactV1::Unknown);
}

#[test]
fn unreachable_predecessors_do_not_poison_reachable_merges() {
    let context = &mut setup();
    let function = function(context, "unreachable_predecessor");
    let entry = function.get_entry_block(context);
    let dead = block(context, &function, "dead");
    let (join, argument) = index_block(context, &function, "join");
    let seven = IndexConstantOp::new(context, 7);
    let enter = BranchArgsOp::new(context, vec![seven.result(context)], join);
    let nine = IndexConstantOp::new(context, 9);
    let dead_edge = BranchArgsOp::new(context, vec![nine.result(context)], join);
    let ret = ReturnOp::new(context);
    append(context, entry, &seven);
    append(context, entry, &enter);
    append(context, dead, &nine);
    append(context, dead, &dead_edge);
    append(context, join, &ret);

    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    assert_eq!(analysis.fact(argument).constant_value(), Some(7));
}

#[test]
fn untyped_edges_to_block_arguments_fail_closed() {
    let context = &mut setup();
    let function = function(context, "untyped_edge");
    let entry = function.get_entry_block(context);
    let (join, _) = index_block(context, &function, "join");
    let branch = BranchOp::new(context, join);
    let ret = ReturnOp::new(context);
    append(context, entry, &branch);
    append(context, join, &ret);

    assert_eq!(
        analyze_pliron_sparse_indices_v1(context, &function).unwrap_err(),
        SparseIndexFailureV1::MalformedControlFlow {
            detail: "a block argument has a predecessor without typed edge operands",
        }
    );
}

#[test]
fn malformed_typed_edge_segments_fail_without_panicking() {
    let context = &mut setup();
    let function = function(context, "malformed_typed_edge");
    let entry = function.get_entry_block(context);
    let (join, _) = index_block(context, &function, "join");
    let zero = IndexConstantOp::new(context, 0);
    let branch = IndexEqualBranchArgsOp::new(
        context,
        zero.result(context),
        zero.result(context),
        vec![zero.result(context)],
        vec![zero.result(context)],
        join,
        join,
    );
    Operation::pop_operand(branch.get_operation(), context);
    let ret = ReturnOp::new(context);
    append(context, entry, &zero);
    append(context, entry, &branch);
    append(context, join, &ret);

    assert_eq!(
        analyze_pliron_sparse_indices_v1(context, &function).unwrap_err(),
        SparseIndexFailureV1::MalformedControlFlow {
            detail: "typed conditional edge has a malformed operand count",
        }
    );
}

#[test]
fn unreachable_unseeded_cycles_remain_unknown() {
    let context = &mut setup();
    let function = function(context, "unseeded_cycle");
    let entry = function.get_entry_block(context);
    let (cycle, argument) = index_block(context, &function, "cycle");
    let ret = ReturnOp::new(context);
    let repeat = BranchArgsOp::new(context, vec![argument], cycle);
    append(context, entry, &ret);
    append(context, cycle, &repeat);

    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    assert_eq!(analysis.fact(argument), SparseIndexFactV1::Unknown);
}

#[test]
fn a_constant_seeded_self_cycle_reaches_a_fixed_point() {
    let context = &mut setup();
    let function = function(context, "constant_cycle");
    let entry = function.get_entry_block(context);
    let (header, argument) = index_block(context, &function, "header");
    let five = IndexConstantOp::new(context, 5);
    let enter = BranchArgsOp::new(context, vec![five.result(context)], header);
    let repeat = BranchArgsOp::new(context, vec![argument], header);
    append(context, entry, &five);
    append(context, entry, &enter);
    append(context, header, &repeat);

    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    assert_eq!(analysis.fact(argument).constant_value(), Some(5));
}

#[test]
fn loop_recurrences_converge_conservatively_to_unknown() {
    let context = &mut setup();
    let function = function(context, "loop_recurrence");
    let entry = function.get_entry_block(context);
    let (header, argument) = index_block(context, &function, "header");
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    let enter = BranchArgsOp::new(context, vec![zero.result(context)], header);
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        argument,
        one.result(context),
    );
    let repeat = BranchArgsOp::new(context, vec![next.result(context)], header);
    append(context, entry, &zero);
    append(context, entry, &one);
    append(context, entry, &enter);
    append(context, header, &next);
    append(context, header, &repeat);

    let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
    assert_eq!(analysis.fact(argument), SparseIndexFactV1::Unknown);
    assert_eq!(
        analysis.fact(next.result(context)),
        SparseIndexFactV1::Unknown
    );
}

#[test]
fn block_arguments_count_toward_the_sparse_value_budget() {
    let context = &mut setup();
    let function = function(context, "oversized_block");
    let entry = function.get_entry_block(context);
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);
    let index: TypeHandle = IndexType::get(context).into();
    let oversized = BasicBlock::new(
        context,
        Some("oversized".try_into().unwrap()),
        vec![index; MAX_SPARSE_INDEX_VALUES_V1 + 1],
    );
    oversized.insert_at_back(function.get_region(context), context);

    assert_eq!(
        analyze_pliron_sparse_indices_v1(context, &function).unwrap_err(),
        SparseIndexFailureV1::ResourceLimit {
            resource: "SSA value",
            limit: MAX_SPARSE_INDEX_VALUES_V1,
            actual: MAX_SPARSE_INDEX_VALUES_V1 + 1,
        }
    );
}
