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

fn index_block_arguments(
    context: &mut Context,
    function: &FuncOp,
    name: &str,
    arguments: usize,
) -> (Ptr<BasicBlock>, Vec<Value>) {
    let index: TypeHandle = IndexType::get(context).into();
    let block = BasicBlock::new(
        context,
        Some(name.try_into().unwrap()),
        vec![index; arguments],
    );
    let values = (0..arguments)
        .map(|argument| block.deref(context).get_argument(argument))
        .collect();
    block.insert_at_back(function.get_region(context), context);
    (block, values)
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

fn forwarded_constant_loop(
    context: &mut Context,
    step: u64,
    mutate_before_latch: bool,
    symbolic_bound: bool,
) -> FuncOp {
    let (function, arguments) = make_function(
        context,
        "forwarded_constant_loop",
        usize::from(symbolic_bound),
    );
    let entry = function.get_entry_block(context);
    let (header, induction) = index_block(context, &function, "header");
    let (first, first_induction) = index_block(context, &function, "first");
    let (middle, middle_induction) = index_block(context, &function, "middle");
    let (latch, latch_induction) = index_block(context, &function, "latch");
    let exit = block(context, &function, "exit");
    let start = IndexConstantOp::new(context, 0);
    let bound = (!symbolic_bound).then(|| IndexConstantOp::new(context, 64));
    let step = IndexConstantOp::new(context, step);
    let bound_value = arguments
        .first()
        .copied()
        .or_else(|| bound.as_ref().map(|bound| bound.result(context)))
        .expect("the loop has either a symbolic or constant bound");
    let enter = BranchArgsOp::new(context, vec![start.result(context)], header);
    let condition = IndexLessThanBranchArgsOp::new(
        context,
        induction,
        bound_value,
        vec![induction],
        vec![],
        first,
        exit,
    );
    let early = mutate_before_latch.then(|| {
        IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Add,
            first_induction,
            step.result(context),
        )
    });
    let first_value = early
        .as_ref()
        .map(|operation| operation.result(context))
        .unwrap_or(first_induction);
    let forward_first = BranchArgsOp::new(context, vec![first_value], middle);
    let forward_middle = BranchArgsOp::new(context, vec![middle_induction], latch);
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        latch_induction,
        step.result(context),
    );
    let repeat = BranchArgsOp::new(context, vec![next.result(context)], header);
    let ret = ReturnOp::new(context);
    for operation in [
        start.get_operation(),
        step.get_operation(),
        enter.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    if let Some(bound) = bound {
        bound
            .get_operation()
            .insert_before(context, step.get_operation());
    }
    append(context, header, &condition);
    if let Some(early) = early {
        append(context, first, &early);
    }
    append(context, first, &forward_first);
    append(context, middle, &forward_middle);
    append(context, latch, &next);
    append(context, latch, &repeat);
    append(context, exit, &ret);
    verify_operation(function.get_operation(), context).unwrap();
    function
}

fn multi_carried_symbolic_loop(context: &mut Context) -> FuncOp {
    let (function, function_arguments) = make_function(context, "multi_carried_loop", 1);
    let entry = function.get_entry_block(context);
    let (header, header_arguments) = index_block_arguments(context, &function, "header", 2);
    let (body, body_arguments) = index_block_arguments(context, &function, "body", 2);
    let (latch, latch_arguments) = index_block_arguments(context, &function, "latch", 2);
    let exit = block(context, &function, "exit");
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    let initial_state = IndexConstantOp::new(context, 7);
    let enter = BranchArgsOp::new(
        context,
        vec![zero.result(context), initial_state.result(context)],
        header,
    );
    let condition = IndexLessThanBranchArgsOp::new(
        context,
        header_arguments[0],
        function_arguments[0],
        header_arguments.clone(),
        vec![],
        body,
        exit,
    );
    let next_state = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        body_arguments[1],
        one.result(context),
    );
    let forward = BranchArgsOp::new(
        context,
        vec![body_arguments[0], next_state.result(context)],
        latch,
    );
    let next_induction = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        latch_arguments[0],
        one.result(context),
    );
    let repeat = BranchArgsOp::new(
        context,
        vec![next_induction.result(context), latch_arguments[1]],
        header,
    );
    let ret = ReturnOp::new(context);
    for operation in [
        zero.get_operation(),
        one.get_operation(),
        initial_state.get_operation(),
        enter.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, header, &condition);
    append(context, body, &next_state);
    append(context, body, &forward);
    append(context, latch, &next_induction);
    append(context, latch, &repeat);
    append(context, exit, &ret);
    verify_operation(function.get_operation(), context).unwrap();
    function
}

fn advancing_carried_bound_loop(context: &mut Context) -> FuncOp {
    let (function, _) = make_function(context, "advancing_carried_bound", 0);
    let entry = function.get_entry_block(context);
    let (header, header_arguments) = index_block_arguments(context, &function, "header", 2);
    let (body, body_arguments) = index_block_arguments(context, &function, "body", 2);
    let exit = block(context, &function, "exit");
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    let enter = BranchArgsOp::new(
        context,
        vec![zero.result(context), one.result(context)],
        header,
    );
    let condition = IndexLessThanBranchArgsOp::new(
        context,
        header_arguments[0],
        header_arguments[1],
        header_arguments.clone(),
        vec![],
        body,
        exit,
    );
    let next_induction = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        body_arguments[0],
        one.result(context),
    );
    let next_bound = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        body_arguments[1],
        one.result(context),
    );
    let repeat = BranchArgsOp::new(
        context,
        vec![next_induction.result(context), next_bound.result(context)],
        header,
    );
    let ret = ReturnOp::new(context);
    for operation in [
        zero.get_operation(),
        one.get_operation(),
        enter.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    append(context, header, &condition);
    append(context, body, &next_induction);
    append(context, body, &next_bound);
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
fn canonical_multi_block_ring_proves_unchanged_induction_transport() {
    let context = &mut setup();
    let function = forwarded_constant_loop(context, 1, false, true);
    let report = run_pliron_progress_check_v1(context, &function);
    assert!(report.is_clean());
    assert_eq!(report.certificates().len(), 1);
    assert_eq!(report.certificates()[0].step(), 1);
}

#[test]
fn multi_block_ring_tracks_induction_among_other_evolving_carried_values() {
    let context = &mut setup();
    let function = multi_carried_symbolic_loop(context);
    let report = run_pliron_progress_check_v1(context, &function);
    assert!(report.is_clean());
    assert_eq!(report.certificates().len(), 1);
    assert_eq!(report.certificates()[0].step(), 1);
}

#[test]
fn advancing_loop_carried_bound_cannot_prove_progress() {
    let context = &mut setup();
    let function = advancing_carried_bound_loop(context);
    let report = run_pliron_progress_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(report.certificates().is_empty());
    assert!(matches!(
        report.findings(),
        [PlironProgressFindingV1::ProgressIncomplete { reason, .. }]
            if reason.contains("bound depends") && reason.contains("inside the cycle")
    ));
}

#[test]
fn canonical_multi_block_ring_preserves_static_nonunit_no_wrap_checks() {
    let context = &mut setup();
    let function = forwarded_constant_loop(context, 16, false, false);
    let report = run_pliron_progress_check_v1(context, &function);
    assert!(report.is_clean());
    assert_eq!(report.certificates()[0].step(), 16);
}

#[test]
fn multi_block_symbolic_nonunit_step_requires_a_narrow_range_receipt() {
    let context = &mut setup();
    let function = forwarded_constant_loop(context, 16, false, true);
    let report = run_pliron_progress_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [PlironProgressFindingV1::ProgressIncomplete { reason, .. }]
            if reason.contains("symbolic bound") && reason.contains("no-wrap")
    ));
}

#[test]
fn multi_block_zero_step_reports_a_live_nontermination_witness() {
    let context = &mut setup();
    let function = forwarded_constant_loop(context, 0, false, false);
    let report = run_pliron_progress_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(matches!(
        report.findings(),
        [PlironProgressFindingV1::NonTerminatingCycle { counterexample, .. }]
            if counterexample.contains("i = 0") && counterexample.contains("bound = 64")
    ));
}

#[test]
fn multi_block_ring_rejects_an_update_before_the_unique_latch() {
    let context = &mut setup();
    let function = forwarded_constant_loop(context, 1, true, true);
    let report = run_pliron_progress_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [PlironProgressFindingV1::ProgressIncomplete { reason, .. }]
            if reason.contains("non-latch loop edge")
    ));
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
