use dialect_kernel::{
    BranchArgsOp, BranchOp, DIALECT_NAME, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp,
    IndexLessThanBranchArgsOp, IndexType, ReturnOp, register_dialect,
};
use fe2o3_kernel_analysis::{
    KernelCheckStatusV1, PlironProgressFindingV1, run_pliron_progress_check_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
    operation::verify_operation,
    r#type::TypeHandle,
    value::Value,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    context
}

fn make_function(context: &mut Context, name: &str, arguments: usize) -> (FuncOp, Vec<Value>) {
    let index: TypeHandle = IndexType::get(context).into();
    let function = FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, vec![index; arguments], vec![]),
    );
    let values = (0..arguments)
        .map(|ordinal| {
            function
                .get_entry_block(context)
                .deref(context)
                .get_argument(ordinal)
        })
        .collect();
    (function, values)
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

fn constant_loop(context: &mut Context, start: u64, bound: u64, step: u64) -> FuncOp {
    let (function, _) = make_function(context, "constant_loop", 0);
    let entry = function.get_entry_block(context);
    let (header, induction) = index_block(context, &function, "header");
    let (body, body_induction) = index_block(context, &function, "body");
    let exit = block(context, &function, "exit");
    let start = IndexConstantOp::new(context, start);
    let bound = IndexConstantOp::new(context, bound);
    let step = IndexConstantOp::new(context, step);
    let enter = BranchArgsOp::new(context, vec![start.result(context)], header);
    let condition = IndexLessThanBranchArgsOp::new(
        context,
        induction,
        bound.result(context),
        vec![induction],
        vec![],
        body,
        exit,
    );
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        body_induction,
        step.result(context),
    );
    let repeat = BranchArgsOp::new(context, vec![next.result(context)], header);
    let ret = ReturnOp::new(context);
    for operation in [
        start.get_operation(),
        bound.get_operation(),
        step.get_operation(),
        enter.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, header, &condition);
    append(context, body, &next);
    append(context, body, &repeat);
    append(context, exit, &ret);
    verify_operation(function.get_operation(), context).unwrap();
    function
}

#[test]
fn canonical_unit_induction_proves_machine_finite_progress() {
    let context = &mut setup();
    let function = constant_loop(context, 0, 8, 1);
    let report = run_pliron_progress_check_v1(context, &function);
    assert!(report.is_clean());
    assert_eq!(report.certificates().len(), 1);
    assert_eq!(report.certificates()[0].step(), 1);
}

#[test]
fn static_positive_nonunit_step_is_proved_only_when_its_update_cannot_overflow() {
    let context = &mut setup();
    let function = constant_loop(context, 0, 64, 16);
    let report = run_pliron_progress_check_v1(context, &function);
    assert!(report.is_clean());
    assert_eq!(report.certificates()[0].step(), 16);

    let function = constant_loop(context, 0, u64::MAX, 16);
    let report = run_pliron_progress_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [PlironProgressFindingV1::ProgressIncomplete { reason, .. }]
            if reason.contains("overflow")
    ));
}

#[test]
fn external_body_predecessor_invalidates_the_canonical_recurrence() {
    let context = &mut setup();
    let (function, _) = make_function(context, "external_body_predecessor", 0);
    let entry = function.get_entry_block(context);
    let (header, induction) = index_block(context, &function, "header");
    let (body, body_induction) = index_block(context, &function, "body");
    let exit = block(context, &function, "exit");
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    let bound = IndexConstantOp::new(context, 8);
    let hostile = IndexConstantOp::new(context, u64::MAX);
    let enter = IndexLessThanBranchArgsOp::new(
        context,
        zero.result(context),
        one.result(context),
        vec![hostile.result(context)],
        vec![zero.result(context)],
        body,
        header,
    );
    let condition = IndexLessThanBranchArgsOp::new(
        context,
        induction,
        bound.result(context),
        vec![induction],
        vec![],
        body,
        exit,
    );
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        body_induction,
        one.result(context),
    );
    let repeat = BranchArgsOp::new(context, vec![next.result(context)], header);
    let ret = ReturnOp::new(context);
    for operation in [
        zero.get_operation(),
        one.get_operation(),
        bound.get_operation(),
        hostile.get_operation(),
        enter.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, header, &condition);
    append(context, body, &next);
    append(context, body, &repeat);
    append(context, exit, &ret);
    verify_operation(function.get_operation(), context).unwrap();
    let report = run_pliron_progress_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [PlironProgressFindingV1::ProgressIncomplete { reason, .. }]
            if reason.contains("body has a predecessor")
    ));
}

#[test]
fn feasible_zero_step_has_a_live_counterexample() {
    let context = &mut setup();
    let function = constant_loop(context, 0, 8, 0);
    let report = run_pliron_progress_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(matches!(
        report.findings(),
        [PlironProgressFindingV1::NonTerminatingCycle { counterexample, .. }]
            if counterexample.contains("i = 0") && counterexample.contains("bound = 8")
    ));
}

#[test]
fn infeasible_zero_step_true_edge_is_not_a_false_rejection() {
    let context = &mut setup();
    let function = constant_loop(context, 8, 8, 0);
    let report = run_pliron_progress_check_v1(context, &function);
    assert!(report.is_clean());
    assert!(report.certificates().is_empty());
}

#[test]
fn symbolic_zero_step_fails_closed_without_inventing_a_witness() {
    let context = &mut setup();
    let (function, arguments) = make_function(context, "symbolic_zero_step", 1);
    let bound = arguments[0];
    let entry = function.get_entry_block(context);
    let (header, induction) = index_block(context, &function, "header");
    let (body, body_induction) = index_block(context, &function, "body");
    let exit = block(context, &function, "exit");
    let start = IndexConstantOp::new(context, 0);
    let zero = IndexConstantOp::new(context, 0);
    let enter = BranchArgsOp::new(context, vec![start.result(context)], header);
    let condition = IndexLessThanBranchArgsOp::new(
        context,
        induction,
        bound,
        vec![induction],
        vec![],
        body,
        exit,
    );
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        body_induction,
        zero.result(context),
    );
    let repeat = BranchArgsOp::new(context, vec![next.result(context)], header);
    let ret = ReturnOp::new(context);
    for operation in [
        start.get_operation(),
        zero.get_operation(),
        enter.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, header, &condition);
    append(context, body, &next);
    append(context, body, &repeat);
    append(context, exit, &ret);
    verify_operation(function.get_operation(), context).unwrap();
    let report = run_pliron_progress_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [PlironProgressFindingV1::ProgressIncomplete { .. }]
    ));
}

#[test]
fn entry_self_cycle_is_rejected_but_unreachable_cycle_is_ignored() {
    let context = &mut setup();
    let (function, _) = make_function(context, "self_cycle", 0);
    let entry = function.get_entry_block(context);
    let repeat = BranchOp::new(context, entry);
    append(context, entry, &repeat);
    let report = run_pliron_progress_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);

    let (function, _) = make_function(context, "unreachable_cycle", 0);
    let entry = function.get_entry_block(context);
    let cycle = block(context, &function, "cycle");
    let ret = ReturnOp::new(context);
    let repeat = BranchOp::new(context, cycle);
    append(context, entry, &ret);
    append(context, cycle, &repeat);
    assert!(run_pliron_progress_check_v1(context, &function).is_clean());
}
