use super::*;
use dialect_kernel::{
    BranchArgsOp, BranchOp, DIALECT_NAME, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp,
    IndexLessThanBranchArgsOp, ReturnOp, register_dialect,
};
use pliron::{builtin::ops::ModuleOp, dialect::DialectName};

mod native;
mod order;

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    dialect_proof::register_dialect(&mut context).unwrap();
    context
}

fn function(context: &mut Context, inputs: Vec<TypeHandle>) -> FuncOp {
    let signature = FunctionType::get(context, inputs, vec![]);
    FuncOp::new(context, "scoped_ssa".try_into().unwrap(), signature)
}

fn block(context: &mut Context, function: FuncOp, inputs: Vec<TypeHandle>) -> Ptr<BasicBlock> {
    let block = BasicBlock::new(context, None, inputs);
    block.insert_at_back(function.get_region(context), context);
    block
}

fn append(context: &Context, block: Ptr<BasicBlock>, operation: impl Op) {
    operation.get_operation().insert_at_back(block, context);
}

fn binary(context: &mut Context, block: Ptr<BasicBlock>, lhs: Value, rhs: Value) -> IndexBinaryOp {
    let binary = IndexBinaryOp::new(context, IndexBinaryKindAttr::Add, lhs, rhs);
    append(context, block, binary);
    binary
}

fn ret(context: &mut Context, block: Ptr<BasicBlock>) {
    let ret = ReturnOp::new(context);
    append(context, block, ret);
}

fn branch(context: &mut Context, from: Ptr<BasicBlock>, to: Ptr<BasicBlock>) {
    let branch = BranchOp::new(context, to);
    append(context, from, branch);
}

fn conditional(
    context: &mut Context,
    from: Ptr<BasicBlock>,
    value: Value,
    first: Ptr<BasicBlock>,
    second: Ptr<BasicBlock>,
) -> IndexLessThanBranchArgsOp {
    let branch =
        IndexLessThanBranchArgsOp::new(context, value, value, vec![], vec![], first, second);
    append(context, from, branch);
    branch
}

fn observe(context: &Context, function: &FuncOp) -> Result<(), Failure> {
    TRACE.set(Trace::default());
    let scan = def_use_closure_v1::native_census(context, function);
    let order = def_use_closure_v1::check(
        context,
        function,
        &scan,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .unwrap();
    verify(context, function, order)
}

fn captures(context: &Context, function: &FuncOp) -> bool {
    LivePlironStructuralIdentityProviderV1::new(context, function)
        .capture_with_resource_limits_v1(
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .is_ok()
}

fn compare(context: &Context, function: &FuncOp, expected: bool) {
    let generic = catch_unwind(AssertUnwindSafe(|| {
        verify_operation(function.get_operation(), context)
    }));
    let generic_ok = matches!(generic, Ok(Ok(())));
    assert_eq!(generic_ok, expected, "generic: {generic:?}");
    let scoped = observe(context, function);
    assert_eq!(scoped.is_ok(), expected, "scoped: {scoped:?}");
}

#[test]
fn actual_provider_uses_one_tree_and_visits_every_operand() {
    let context = &mut setup();
    let index = IndexType::get(context).into();
    let function = function(context, vec![index, index]);
    let mut blocks = vec![function.get_entry_block(context)];
    for _ in 1..32 {
        blocks.push(block(context, function, vec![index, index]));
    }
    for (ordinal, block) in blocks.iter().copied().enumerate() {
        let lhs = block.deref(context).get_argument(0);
        let rhs = block.deref(context).get_argument(1);
        for _ in 0..16 {
            binary(context, block, lhs, rhs);
        }
        if let Some(next) = blocks.get(ordinal + 1) {
            let branch = BranchArgsOp::new(context, vec![lhs, rhs], *next);
            append(context, block, branch);
        } else {
            ret(context, block);
        }
    }
    TRACE.set(Trace::default());
    let mut provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
    let captured = provider
        .capture_with_resource_limits_v1(
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .ok()
        .unwrap();
    assert_eq!(captured.input_census.operands, 1086);
    assert_eq!(
        TRACE.get(),
        Trace {
            structural_verifications: 1,
            tree_requests: 1,
            operand_visits: 1086,
            cross_block_queries: 0,
            order_queries: 0,
        }
    );
}

#[test]
fn same_block_forward_and_self_uses_reject() {
    for self_use in [false, true] {
        let context = &mut setup();
        let index = IndexType::get(context).into();
        let function = function(context, vec![index]);
        let entry = function.get_entry_block(context);
        let argument = entry.deref(context).get_argument(0);
        let first = binary(context, entry, argument, argument);
        let second = binary(context, entry, argument, argument);
        ret(context, entry);
        let definition = if self_use { first } else { second };
        Operation::replace_operand(
            first.get_operation(),
            context,
            1,
            definition.result(context),
        );
        compare(context, &function, false);
        assert!(matches!(
            observe(context, &function),
            Err(Failure::Dominance {
                block: 0,
                operation: 0,
                operand: 1
            })
        ));
    }
}

#[test]
fn diamond_arm_cannot_define_join_operand() {
    let context = &mut setup();
    let index = IndexType::get(context).into();
    let function = function(context, vec![index]);
    let entry = function.get_entry_block(context);
    let left = block(context, function, vec![]);
    let right = block(context, function, vec![]);
    let join = block(context, function, vec![]);
    let argument = entry.deref(context).get_argument(0);
    conditional(context, entry, argument, left, right);
    let definition = binary(context, left, argument, argument);
    branch(context, left, join);
    branch(context, right, join);
    binary(context, join, definition.result(context), argument);
    ret(context, join);
    compare(context, &function, false);
}

#[test]
fn fresh_capture_recomputes_dominance_after_edge_mutation() {
    let context = &mut setup();
    let index = IndexType::get(context).into();
    let function = function(context, vec![index]);
    let entry = function.get_entry_block(context);
    let defining = block(context, function, vec![]);
    let join = block(context, function, vec![]);
    let argument = entry.deref(context).get_argument(0);
    let split = conditional(context, entry, argument, defining, defining);
    let definition = binary(context, defining, argument, argument);
    branch(context, defining, join);
    binary(context, join, definition.result(context), argument);
    ret(context, join);
    compare(context, &function, true);
    let mut provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
    let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
    assert!(provider.capture_with_resource_limits_v1(limits).is_ok());
    Operation::replace_successor(split.get_operation(), context, 1, join);
    TRACE.set(Trace::default());
    assert!(provider.capture_with_resource_limits_v1(limits).is_err());
    assert_eq!(TRACE.get().tree_requests, 1);
    compare(context, &function, false);
    Operation::replace_successor(split.get_operation(), context, 1, defining);
    TRACE.set(Trace::default());
    assert!(provider.capture_with_resource_limits_v1(limits).is_ok());
    assert_eq!(TRACE.get().tree_requests, 1);
}

#[test]
fn unreachable_same_block_arguments_and_ordered_results_remain_valid() {
    let context = &mut setup();
    let function = function(context, vec![]);
    let entry = function.get_entry_block(context);
    let index = IndexType::get(context).into();
    let unreachable = block(context, function, vec![index]);
    ret(context, entry);
    let value = unreachable.deref(context).get_argument(0);
    let first = binary(context, unreachable, value, value);
    binary(context, unreachable, first.result(context), value);
    ret(context, unreachable);
    compare(context, &function, true);
    assert_eq!(TRACE.get().cross_block_queries, 0);
    assert!(captures(context, &function));
}

#[test]
fn cross_unreachable_use_rejects_without_panicking() {
    let context = &mut setup();
    let index = IndexType::get(context).into();
    let function = function(context, vec![index]);
    let entry = function.get_entry_block(context);
    let unreachable = block(context, function, vec![]);
    let argument = entry.deref(context).get_argument(0);
    ret(context, entry);
    binary(context, unreachable, argument, argument);
    ret(context, unreachable);
    assert!(matches!(
        observe(context, &function),
        Err(Failure::Dominance {
            block: 1,
            operation: 0,
            operand: 0
        })
    ));
    assert_eq!(TRACE.get().tree_requests, 1);
    assert_eq!(TRACE.get().cross_block_queries, 0);
}

#[test]
fn all_three_block_single_successor_graphs_match_generic_verification() {
    // 64 graphs cover cycles, disconnected components, self loops and entry
    // backedges; each of nine block-argument definition/use pairs is checked.
    for edges in 0..64 {
        for definition in 0..3 {
            for user in 0..3 {
                let context = &mut setup();
                let index = IndexType::get(context).into();
                let function = function(context, vec![index]);
                let blocks = [
                    function.get_entry_block(context),
                    block(context, function, vec![index]),
                    block(context, function, vec![index]),
                ];
                let value = blocks[definition].deref(context).get_argument(0);
                for (ordinal, block) in blocks.iter().copied().enumerate() {
                    let local = block.deref(context).get_argument(0);
                    if ordinal == user {
                        binary(context, block, value, local);
                    }
                    let target = (edges >> (2 * ordinal)) & 3;
                    if target == 3 {
                        ret(context, block);
                    } else {
                        let branch = BranchArgsOp::new(context, vec![local], blocks[target]);
                        append(context, block, branch);
                    }
                }
                let generic = catch_unwind(AssertUnwindSafe(|| {
                    verify_operation(function.get_operation(), context)
                }));
                let scoped = observe(context, &function);
                assert_eq!(
                    scoped.is_ok(),
                    matches!(generic, Ok(Ok(()))),
                    "edges={edges} definition={definition} user={user}: {scoped:?}"
                );
            }
        }
    }
}

#[test]
fn irreducible_cycle_uses_real_dominance_not_block_order() {
    let context = &mut setup();
    let index = IndexType::get(context).into();
    let function = function(context, vec![index]);
    let entry = function.get_entry_block(context);
    let left = block(context, function, vec![]);
    let right = block(context, function, vec![]);
    let exit = block(context, function, vec![]);
    let argument = entry.deref(context).get_argument(0);
    conditional(context, entry, argument, left, right);
    let definition = binary(context, left, argument, argument);
    conditional(context, left, argument, right, exit);
    let user = binary(context, right, argument, argument);
    conditional(context, right, argument, left, exit);
    ret(context, exit);
    compare(context, &function, true);
    Operation::replace_operand(user.get_operation(), context, 0, definition.result(context));
    compare(context, &function, false);
}

#[test]
fn raw_structural_and_interface_errors_precede_tree_construction() {
    for malformed in 0..3 {
        let context = &mut setup();
        let function = function(context, vec![]);
        let entry = function.get_entry_block(context);
        match malformed {
            0 => {}
            1 => {
                ret(context, entry);
                let constant = IndexConstantOp::new(context, 1);
                append(context, entry, constant);
            }
            _ => {
                let index = IndexType::get(context).into();
                let target = block(context, function, vec![index]);
                branch(context, entry, target);
                ret(context, target);
            }
        }
        compare(context, &function, false);
        assert!(matches!(
            observe(context, &function),
            Err(Failure::Structural(_))
        ));
        assert_eq!(TRACE.get().tree_requests, 0);
    }
}

#[test]
fn unsupported_root_nested_region_and_foreign_operands_never_enter_scoped_verifier() {
    for malformed in 0..3 {
        let context = &mut setup();
        let function = if malformed == 0 {
            let module = ModuleOp::new(context, "module".try_into().unwrap());
            FuncOp::from_operation(module.get_operation())
        } else {
            let function = function(context, vec![]);
            let entry = function.get_entry_block(context);
            let constant = IndexConstantOp::new(context, 1);
            append(context, entry, constant);
            let user = binary(
                context,
                entry,
                constant.result(context),
                constant.result(context),
            );
            if malformed == 1 {
                Operation::add_region(user.get_operation(), context);
            } else {
                let foreign = IndexConstantOp::new(context, 2);
                Operation::replace_operand(
                    user.get_operation(),
                    context,
                    0,
                    foreign.result(context),
                );
            }
            ret(context, entry);
            function
        };
        TRACE.set(Trace::default());
        assert!(!captures(context, &function));
        assert_eq!(TRACE.get(), Trace::default());
    }
}

#[test]
fn cumulative_capture_admission_still_precedes_structural_verification() {
    let context = &mut setup();
    let function = function(context, vec![]);
    ret(context, function.get_entry_block(context));
    let mut provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
    let captured = provider
        .capture_with_resource_limits_v1(
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .ok()
        .unwrap();
    let work = captured.resource_upper_bound.work_upper_bound();
    let peak = captured.resource_upper_bound.peak_storage_upper_bound();
    drop(captured);
    for limits in [
        ProductionAnalysisResourceLimitsV1::new(work - 1, peak),
        ProductionAnalysisResourceLimitsV1::new(work, peak - 1),
    ] {
        TRACE.set(Trace::default());
        assert!(matches!(
            provider.capture_with_resource_limits_v1(limits),
            Err(IdentityCaptureFailureV1::ResourceLimit(_))
        ));
        assert_eq!(TRACE.get(), Trace::default());
    }
    TRACE.set(Trace::default());
    assert!(
        provider
            .capture_with_resource_limits_v1(ProductionAnalysisResourceLimitsV1::new(work, peak))
            .is_ok()
    );
    assert_eq!(TRACE.get().tree_requests, 1);
}
