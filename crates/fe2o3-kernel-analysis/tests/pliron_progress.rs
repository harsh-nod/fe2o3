use dialect_kernel::{
    BranchArgsOp, BranchOp, DIALECT_NAME, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp,
    IndexLessThanBranchArgsOp, IndexType, IndexUnsignedCastOp, InvocationIndexOp, ReturnOp,
    register_dialect,
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

#[derive(Clone, Copy)]
enum MultiBlockCase {
    Canonical,
    MutatedForwarding,
    ExternalIntermediateEntry,
    MultipleHeaderEntries,
    InvocationLatchUpdate,
}

#[derive(Clone, Copy)]
enum RangeCase {
    None,
    Bound(&'static [u64]),
    Mismatched(u64),
    NonEntry(u64),
}

fn multi_block_loop(
    context: &mut Context,
    static_bound: Option<u64>,
    step_value: u64,
    case: MultiBlockCase,
    range_case: RangeCase,
) -> FuncOp {
    let (function, arguments) = make_function(
        context,
        "multi_block_loop",
        usize::from(static_bound.is_none()),
    );
    let entry = function.get_entry_block(context);
    let (header, induction) = index_block(context, &function, "header");
    let (first, first_induction) = index_block(context, &function, "first");
    let (second, second_induction) = index_block(context, &function, "second");
    let (latch, latch_induction) = index_block(context, &function, "latch");
    let exit = block(context, &function, "exit");
    let start = IndexConstantOp::new(context, 0);
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    let step = IndexConstantOp::new(context, step_value);
    let bound_constant = static_bound.map(|bound| IndexConstantOp::new(context, bound));
    let mut bound = bound_constant.as_ref().map_or_else(
        || arguments.first().copied().expect("symbolic bound"),
        |bound| bound.result(context),
    );
    let invocation = matches!(case, MultiBlockCase::InvocationLatchUpdate)
        .then(|| InvocationIndexOp::new(context, 0, 8));

    for operation in [
        start.get_operation(),
        zero.get_operation(),
        one.get_operation(),
        step.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    if let Some(bound) = &bound_constant {
        append(context, entry, bound);
    }
    if let Some(invocation) = &invocation {
        append(context, entry, invocation);
    }
    match range_case {
        RangeCase::None | RangeCase::NonEntry(_) => {}
        RangeCase::Bound(widths) => {
            for width in widths {
                let cast = IndexUnsignedCastOp::new(context, bound, *width);
                append(context, entry, &cast);
                bound = cast.result(context);
            }
        }
        RangeCase::Mismatched(width) => {
            let cast = IndexUnsignedCastOp::new(context, start.result(context), width);
            append(context, entry, &cast);
        }
    }
    if matches!(case, MultiBlockCase::MultipleHeaderEntries) {
        let first_entry = block(context, &function, "first_entry");
        let second_entry = block(context, &function, "second_entry");
        let split = IndexLessThanBranchArgsOp::new(
            context,
            zero.result(context),
            one.result(context),
            vec![],
            vec![],
            first_entry,
            second_entry,
        );
        let first_enter = BranchArgsOp::new(context, vec![start.result(context)], header);
        let second_enter = BranchArgsOp::new(context, vec![start.result(context)], header);
        append(context, entry, &split);
        append(context, first_entry, &first_enter);
        append(context, second_entry, &second_enter);
    } else if matches!(case, MultiBlockCase::ExternalIntermediateEntry) {
        let enter = IndexLessThanBranchArgsOp::new(
            context,
            zero.result(context),
            one.result(context),
            vec![start.result(context)],
            vec![start.result(context)],
            header,
            second,
        );
        append(context, entry, &enter);
    } else {
        let enter = BranchArgsOp::new(context, vec![start.result(context)], header);
        append(context, entry, &enter);
    }

    let condition = IndexLessThanBranchArgsOp::new(
        context,
        induction,
        bound,
        vec![induction],
        vec![],
        first,
        exit,
    );
    append(context, header, &condition);

    if matches!(case, MultiBlockCase::MutatedForwarding) {
        let changed = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Add,
            first_induction,
            one.result(context),
        );
        let forward = BranchArgsOp::new(context, vec![changed.result(context)], second);
        append(context, first, &changed);
        append(context, first, &forward);
    } else {
        if let RangeCase::NonEntry(width) = range_case {
            let cast = IndexUnsignedCastOp::new(context, bound, width);
            append(context, first, &cast);
        }
        let forward = BranchArgsOp::new(context, vec![first_induction], second);
        append(context, first, &forward);
    }
    let forward = BranchArgsOp::new(context, vec![second_induction], latch);
    append(context, second, &forward);

    let latch_base = invocation
        .as_ref()
        .map_or(latch_induction, |invocation| invocation.result(context));
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        latch_base,
        step.result(context),
    );
    let repeat = BranchArgsOp::new(context, vec![next.result(context)], header);
    let ret = ReturnOp::new(context);
    append(context, latch, &next);
    append(context, latch, &repeat);
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
fn canonical_multiblock_recurrence_forwards_one_induction_value() {
    let context = &mut setup();
    let function = multi_block_loop(
        context,
        Some(64),
        16,
        MultiBlockCase::Canonical,
        RangeCase::None,
    );
    let report = run_pliron_progress_check_v1(context, &function);
    assert!(report.is_clean(), "{report:?}");
    assert_eq!(report.certificates().len(), 1);
    assert_eq!(report.certificates()[0].step(), 16);
}

#[test]
fn multiblock_symbolic_bound_is_supported_only_for_a_unit_step() {
    let context = &mut setup();
    let function = multi_block_loop(context, None, 1, MultiBlockCase::Canonical, RangeCase::None);
    assert!(run_pliron_progress_check_v1(context, &function).is_clean());

    let function = multi_block_loop(
        context,
        None,
        16,
        MultiBlockCase::Canonical,
        RangeCase::None,
    );
    let report = run_pliron_progress_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [PlironProgressFindingV1::ProgressIncomplete { reason, .. }]
            if reason.contains("symbolic bound") && reason.contains("no-wrap")
    ));
}

#[test]
fn compiler_derived_unsigned_widths_prove_symbolic_nonunit_no_wrap() {
    for widths in [&[8_u64][..], &[16_u64][..], &[32_u64][..]] {
        let context = &mut setup();
        let function = multi_block_loop(
            context,
            None,
            16,
            MultiBlockCase::Canonical,
            RangeCase::Bound(widths),
        );
        let report = run_pliron_progress_check_v1(context, &function);
        assert!(report.is_clean(), "widths={widths:?}: {report:?}");
        assert_eq!(report.certificates()[0].step(), 16);
    }
}

#[test]
fn u64_range_does_not_hide_a_nonunit_update_overflow() {
    let context = &mut setup();
    let function = multi_block_loop(
        context,
        None,
        16,
        MultiBlockCase::Canonical,
        RangeCase::Bound(&[64]),
    );
    let report = run_pliron_progress_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [PlironProgressFindingV1::ProgressIncomplete { reason, .. }]
            if reason.contains("overflow")
    ));
}

#[test]
fn unrelated_and_unused_unsigned_casts_do_not_discharge_the_bound() {
    for (range_case, expected) in [
        (RangeCase::Mismatched(32), "symbolic bound"),
        (RangeCase::NonEntry(32), "symbolic bound"),
    ] {
        let context = &mut setup();
        let function = multi_block_loop(context, None, 16, MultiBlockCase::Canonical, range_case);
        let report = run_pliron_progress_check_v1(context, &function);
        assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
        assert!(matches!(
            report.findings(),
            [PlironProgressFindingV1::ProgressIncomplete { reason, .. }]
                if reason.contains(expected)
        ));
    }
}

#[test]
fn unsigned_cast_width_is_verifier_closed_and_non_authoritative() {
    let context = &mut setup();
    let (function, arguments) = make_function(context, "malformed_range", 1);
    let cast = IndexUnsignedCastOp::new(context, arguments[0], 24);
    assert!(verify_operation(cast.get_operation(), context).is_err());
    assert!(!cast.grants_compiler_refinement_authority());
    assert!(!cast.grants_artifact_or_launch_authority());
    let ret = ReturnOp::new(context);
    append(context, function.get_entry_block(context), &ret);
}

#[test]
fn multiblock_static_bound_must_cover_the_final_update_without_wrap() {
    let context = &mut setup();
    let function = multi_block_loop(
        context,
        Some(u64::MAX),
        16,
        MultiBlockCase::Canonical,
        RangeCase::None,
    );
    let report = run_pliron_progress_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [PlironProgressFindingV1::ProgressIncomplete { reason, .. }]
            if reason.contains("overflow")
    ));
}

#[test]
fn multiblock_recurrence_rejects_mutation_and_alternate_entry() {
    for case in [
        MultiBlockCase::MutatedForwarding,
        MultiBlockCase::ExternalIntermediateEntry,
        MultiBlockCase::InvocationLatchUpdate,
    ] {
        let context = &mut setup();
        let function = multi_block_loop(context, Some(64), 1, case, RangeCase::None);
        let report = run_pliron_progress_check_v1(context, &function);
        assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    }
}

#[test]
fn multiblock_two_external_header_entries_are_not_single_entry() {
    let context = &mut setup();
    let function = multi_block_loop(
        context,
        Some(64),
        1,
        MultiBlockCase::MultipleHeaderEntries,
        RangeCase::None,
    );
    let report = run_pliron_progress_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [PlironProgressFindingV1::ProgressIncomplete { reason, .. }]
            if reason.contains("exactly one external entry")
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
