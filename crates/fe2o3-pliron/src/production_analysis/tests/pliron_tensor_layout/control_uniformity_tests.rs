include!("phi_selection_uniformity_tests.rs");
include!("forwarded_phi_loop_tests.rs");

#[test]
fn dynamic_lane_varying_control_may_reconverge_before_tensor_use() {
    let context = &mut setup();
    let (function, _) = function(context, "dynamic_reconverged", 0);
    let entry = function.get_entry_block(context);
    let left = block(context, &function, "left");
    let right = block(context, &function, "right");
    let join = block(context, &function, "join");
    let execution = layout(context, 0, 64, 64);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let cutoff = IndexConstantOp::new(context, 32);
    let choose = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        cutoff.result(context),
        left,
        right,
    );
    let left_join = BranchOp::new(context, join);
    let right_join = BranchOp::new(context, join);
    let matrix = tensor(context, 64);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &invocation);
    append(context, entry, &cutoff);
    append(context, entry, &choose);
    append(context, left, &left_join);
    append(context, right, &right_join);
    append(context, join, &matrix);
    append(context, join, &ret);

    assert!(run_pliron_tensor_layout_check_v1(context, &function).is_clean());
}

#[derive(Clone, Copy)]
enum SplitDependency {
    EntryArgument,
    Invocation,
    Unknown,
}

fn typed_split_report(dependency: SplitDependency) -> crate::PlironTensorLayoutReportV1 {
    let context = &mut setup();
    let (function, arguments) = function(context, "typed_split_control", 1);
    let entry = function.get_entry_block(context);
    let tensor_block = block(context, &function, "tensor");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    append(context, entry, &execution);
    let control = match dependency {
        SplitDependency::EntryArgument => arguments[0],
        SplitDependency::Invocation => {
            let invocation = InvocationIndexOp::new(context, 0, 0);
            let result = invocation.result(context);
            append(context, entry, &invocation);
            result
        }
        SplitDependency::Unknown => {
            let unknown = IndexUnknownOp::new(context);
            let result = unknown.result(context);
            append(context, entry, &unknown);
            result
        }
    };
    let split = AnalysisSplitOp::new_with_control_and_arguments(
        context,
        vec![control],
        vec![],
        vec![],
        tensor_block,
        exit,
    );
    let matrix = tensor(context, 64);
    let tensor_return = ReturnOp::new(context);
    let exit_return = ReturnOp::new(context);
    append(context, entry, &split);
    append(context, tensor_block, &matrix);
    append(context, tensor_block, &tensor_return);
    append(context, exit, &exit_return);
    run_pliron_tensor_layout_check_v1(context, &function)
}

#[test]
fn typed_analysis_split_classifies_only_complete_control_dependencies() {
    assert!(typed_split_report(SplitDependency::EntryArgument).is_clean());
    assert!(matches!(
        typed_split_report(SplitDependency::Invocation).status(),
        KernelCheckStatusV1::Rejected
    ));
    assert!(matches!(
        typed_split_report(SplitDependency::Unknown).status(),
        KernelCheckStatusV1::Incomplete
    ));
}

#[test]
fn dynamic_subgroup_aligned_control_can_select_different_subgroup_traces() {
    let context = &mut setup();
    let (function, _) = function(context, "dynamic_subgroup_paths", 0);
    let entry = function.get_entry_block(context);
    let matrix_block = block(context, &function, "matrix");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let cutoff = IndexConstantOp::new(context, 64);
    let choose = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        cutoff.result(context),
        matrix_block,
        exit,
    );
    let matrix = tensor(context, 64);
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &invocation);
    append(context, entry, &cutoff);
    append(context, entry, &choose);
    append(context, matrix_block, &matrix);
    append(context, matrix_block, &to_exit);
    append(context, exit, &ret);

    assert!(run_pliron_tensor_layout_check_v1(context, &function).is_clean());
}

#[test]
fn dynamic_lane_varying_early_return_is_rejected() {
    let context = &mut setup();
    let (function, _) = function(context, "dynamic_early_return", 0);
    let entry = function.get_entry_block(context);
    let matrix_block = block(context, &function, "matrix");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let cutoff = IndexConstantOp::new(context, 32);
    let choose = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        cutoff.result(context),
        matrix_block,
        exit,
    );
    let matrix = tensor(context, 64);
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &invocation);
    append(context, entry, &cutoff);
    append(context, entry, &choose);
    append(context, matrix_block, &matrix);
    append(context, matrix_block, &to_exit);
    append(context, exit, &ret);

    assert!(
        run_pliron_tensor_layout_check_v1(context, &function)
            .findings()
            .iter()
            .any(|finding| matches!(
                finding,
                PlironTensorLayoutFindingV1::DivergentSubgroupControl { controller: 0, .. }
            ))
    );
}

#[test]
fn reversed_strict_coordinate_cutoff_is_lane_varying() {
    let context = &mut setup();
    let (function, _) = function(context, "reversed_cutoff", 0);
    let entry = function.get_entry_block(context);
    let matrix_block = block(context, &function, "matrix");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let zero = IndexConstantOp::new(context, 0);
    let choose = IndexLessThanBranchOp::new(
        context,
        zero.result(context),
        invocation.result(context),
        matrix_block,
        exit,
    );
    let matrix = tensor(context, 64);
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &invocation);
    append(context, entry, &zero);
    append(context, entry, &choose);
    append(context, matrix_block, &matrix);
    append(context, matrix_block, &to_exit);
    append(context, exit, &ret);

    assert!(
        run_pliron_tensor_layout_check_v1(context, &function)
            .findings()
            .iter()
            .any(|finding| matches!(
                finding,
                PlironTensorLayoutFindingV1::DivergentSubgroupControl { .. }
            ))
    );
}

#[test]
fn lane_varying_backedge_after_tensor_is_rejected() {
    let context = &mut setup();
    let (function, _) = function(context, "varying_tensor_backedge", 0);
    let entry = function.get_entry_block(context);
    let matrix_block = block(context, &function, "matrix");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let cutoff = IndexConstantOp::new(context, 32);
    let enter = BranchOp::new(context, matrix_block);
    let matrix = tensor(context, 64);
    let repeat = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        cutoff.result(context),
        matrix_block,
        exit,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &invocation);
    append(context, entry, &cutoff);
    append(context, entry, &enter);
    append(context, matrix_block, &matrix);
    append(context, matrix_block, &repeat);
    append(context, exit, &ret);

    assert!(
        run_pliron_tensor_layout_check_v1(context, &function)
            .findings()
            .iter()
            .any(|finding| matches!(
                finding,
                PlironTensorLayoutFindingV1::DivergentSubgroupControl { controller: 1, .. }
            ))
    );
}

#[test]
fn entry_arguments_are_independently_proven_subgroup_uniform() {
    for (name, reverse, arguments) in [
        ("argument_lt_constant", false, 1_usize),
        ("constant_lt_argument", true, 1_usize),
        ("argument_lt_argument", false, 2_usize),
    ] {
        let context = &mut setup();
        let (function, arguments) = function(context, "name", arguments);
        let entry = function.get_entry_block(context);
        let matrix_block = block(context, &function, "matrix");
        let exit = block(context, &function, "exit");
        let execution = layout(context, 0, 64, 64);
        let constant = IndexConstantOp::new(context, 7);
        let rhs = arguments
            .get(1)
            .copied()
            .unwrap_or(constant.result(context));
        let (lhs, rhs) = if reverse {
            (constant.result(context), arguments[0])
        } else {
            (arguments[0], rhs)
        };
        let choose = IndexLessThanBranchOp::new(context, lhs, rhs, matrix_block, exit);
        let matrix = tensor(context, 64);
        let to_exit = BranchOp::new(context, exit);
        let ret = ReturnOp::new(context);
        append(context, entry, &execution);
        append(context, entry, &constant);
        append(context, entry, &choose);
        append(context, matrix_block, &matrix);
        append(context, matrix_block, &to_exit);
        append(context, exit, &ret);

        assert!(
            run_pliron_tensor_layout_check_v1(context, &function).is_clean(),
            "{name}"
        );
    }
}

#[test]
fn unsigned_cast_preserves_its_source_uniformity() {
    let context = &mut setup();
    let (function, arguments) = function(context, "unsigned_cast_uniformity", 1);
    let entry = function.get_entry_block(context);
    let matrix_block = block(context, &function, "matrix");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let cast = IndexUnsignedCastOp::new(context, arguments[0], 32);
    let expected = IndexConstantOp::new(context, 0);
    let choose = IndexEqualBranchOp::new(
        context,
        cast.result(context),
        expected.result(context),
        matrix_block,
        exit,
    );
    let matrix = tensor(context, 64);
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &cast);
    append(context, entry, &expected);
    append(context, entry, &choose);
    append(context, matrix_block, &matrix);
    append(context, matrix_block, &to_exit);
    append(context, exit, &ret);

    verify_operation(function.get_operation(), context).unwrap();
    assert!(run_pliron_tensor_layout_check_v1(context, &function).is_clean());
}

#[test]
fn unsigned_cast_does_not_make_a_varying_source_uniform() {
    let context = &mut setup();
    let (function, _) = function(context, "unsigned_cast_varying", 0);
    let entry = function.get_entry_block(context);
    let matrix_block = block(context, &function, "matrix");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let cast = IndexUnsignedCastOp::new(context, invocation.result(context), 32);
    let expected = IndexConstantOp::new(context, 0);
    let choose = IndexEqualBranchOp::new(
        context,
        cast.result(context),
        expected.result(context),
        matrix_block,
        exit,
    );
    let matrix = tensor(context, 64);
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &invocation);
    append(context, entry, &cast);
    append(context, entry, &expected);
    append(context, entry, &choose);
    append(context, matrix_block, &matrix);
    append(context, matrix_block, &to_exit);
    append(context, exit, &ret);

    verify_operation(function.get_operation(), context).unwrap();
    assert!(
        run_pliron_tensor_layout_check_v1(context, &function)
            .findings()
            .iter()
            .any(|finding| matches!(
                finding,
                PlironTensorLayoutFindingV1::DivergentSubgroupControl { .. }
            ))
    );
}

#[test]
fn deterministic_join_of_uniform_dependencies_proves_uniform_control_without_authority() {
    let context = &mut setup();
    let (function, arguments) = function(context, "uniform_deterministic_control", 2);
    let entry = function.get_entry_block(context);
    let matrix_block = block(context, &function, "matrix");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let expected = IndexConstantOp::new(context, 0);
    let summary = DeterministicJoinOp::new(context, arguments);
    let choose = IndexEqualBranchOp::new(
        context,
        summary.result(context),
        expected.result(context),
        matrix_block,
        exit,
    );
    let matrix = tensor(context, 64);
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &expected);
    append(context, entry, &summary);
    append(context, entry, &choose);
    append(context, matrix_block, &matrix);
    append(context, matrix_block, &to_exit);
    append(context, exit, &ret);

    verify_operation(function.get_operation(), context).unwrap();
    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(report.is_clean());
    assert!(!report.grants_compiler_refinement_authority());
    assert!(!report.grants_artifact_or_launch_authority());
}

#[test]
fn deterministic_join_of_lane_varying_dependency_is_rejected() {
    let context = &mut setup();
    let (function, _) = function(context, "varying_deterministic_control", 0);
    let entry = function.get_entry_block(context);
    let matrix_block = block(context, &function, "matrix");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let lane = InvocationIndexOp::new(context, 0, 0);
    let expected = IndexConstantOp::new(context, 0);
    let summary = DeterministicJoinOp::new(context, vec![lane.result(context)]);
    let choose = IndexEqualBranchOp::new(
        context,
        summary.result(context),
        expected.result(context),
        matrix_block,
        exit,
    );
    let matrix = tensor(context, 64);
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &lane);
    append(context, entry, &expected);
    append(context, entry, &summary);
    append(context, entry, &choose);
    append(context, matrix_block, &matrix);
    append(context, matrix_block, &to_exit);
    append(context, exit, &ret);

    verify_operation(function.get_operation(), context).unwrap();
    assert!(
        run_pliron_tensor_layout_check_v1(context, &function)
            .findings()
            .iter()
            .any(|finding| matches!(
                finding,
                PlironTensorLayoutFindingV1::DivergentSubgroupControl { controller: 0, .. }
            ))
    );
}

#[test]
fn deterministic_join_of_unknown_dependency_fails_incomplete() {
    let context = &mut setup();
    let (function, _) = function(context, "unknown_deterministic_control", 0);
    let entry = function.get_entry_block(context);
    let matrix_block = block(context, &function, "matrix");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let zero = IndexConstantOp::new(context, 0);
    let sixteen = IndexConstantOp::new(context, 16);
    let unknown = CheckedTiledIndex2DOp::new(
        context,
        zero.result(context),
        zero.result(context),
        sixteen.result(context),
        sixteen.result(context),
        sixteen.result(context),
        [64, 16, 16, 4],
    );
    let summary = DeterministicJoinOp::new(context, vec![unknown.result(context)]);
    let choose = IndexEqualBranchOp::new(
        context,
        summary.result(context),
        zero.result(context),
        matrix_block,
        exit,
    );
    let matrix = tensor(context, 64);
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &zero);
    append(context, entry, &sixteen);
    append(context, entry, &unknown);
    append(context, entry, &summary);
    append(context, entry, &choose);
    append(context, matrix_block, &matrix);
    append(context, matrix_block, &to_exit);
    append(context, exit, &ret);

    verify_operation(function.get_operation(), context).unwrap();
    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(matches!(report.status(), KernelCheckStatusV1::Incomplete));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail }
            if detail.contains("control-dependent on unresolved branch")
    )));
}

#[test]
fn empty_deterministic_join_fails_incomplete() {
    let context = &mut setup();
    let (function, _) = function(context, "empty_deterministic_join", 0);
    let entry = function.get_entry_block(context);
    let matrix_block = block(context, &function, "matrix");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let zero = IndexConstantOp::new(context, 0);
    let summary = DeterministicJoinOp::new(context, vec![]);
    let choose = IndexEqualBranchOp::new(
        context,
        summary.result(context),
        zero.result(context),
        matrix_block,
        exit,
    );
    let matrix = tensor(context, 64);
    let ret_matrix = ReturnOp::new(context);
    let ret_exit = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &zero);
    append(context, entry, &summary);
    append(context, entry, &choose);
    append(context, matrix_block, &matrix);
    append(context, matrix_block, &ret_matrix);
    append(context, exit, &ret_exit);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(matches!(report.status(), KernelCheckStatusV1::Incomplete));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail }
            if detail.contains("deterministic join has no explicit dependencies")
    )));
}

#[test]
fn malformed_equality_edge_operands_fail_incomplete() {
    let context = &mut setup();
    let (function, _) = function(context, "malformed_equality_edge", 0);
    let entry = function.get_entry_block(context);
    let (matrix_block, carried) = index_block(context, &function, "matrix");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let zero = IndexConstantOp::new(context, 0);
    let summary = DeterministicJoinOp::new(context, vec![zero.result(context)]);
    let choose = IndexEqualBranchArgsOp::new(
        context,
        summary.result(context),
        zero.result(context),
        vec![zero.result(context)],
        vec![],
        matrix_block,
        exit,
    );
    Operation::pop_operand(choose.get_operation(), context);
    let matrix = tensor(context, 64);
    let consume = IndexBinaryOp::new(context, IndexBinaryKindAttr::Add, carried, carried);
    let ret_matrix = ReturnOp::new(context);
    let ret_exit = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &zero);
    append(context, entry, &summary);
    append(context, entry, &choose);
    append(context, matrix_block, &matrix);
    append(context, matrix_block, &consume);
    append(context, matrix_block, &ret_matrix);
    append(context, exit, &ret_exit);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(matches!(report.status(), KernelCheckStatusV1::Incomplete));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail }
            if detail.contains("malformed operand count")
    )));
}

#[test]
fn unresolved_cyclic_control_fails_incomplete() {
    let context = &mut setup();
    let (function, _) = function(context, "unresolved_cycle", 0);
    let entry = function.get_entry_block(context);
    let loop_block = block(context, &function, "loop");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let choose = AnalysisSplitOp::new(context, loop_block, exit);
    let matrix = tensor(context, 64);
    let backedge = BranchOp::new(context, entry);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &choose);
    append(context, loop_block, &matrix);
    append(context, loop_block, &backedge);
    append(context, exit, &ret);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(matches!(report.status(), KernelCheckStatusV1::Incomplete));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail }
            if detail.contains("control-dependent on unresolved branch")
    )));
}

#[test]
fn parameter_derived_loop_induction_is_proven_subgroup_uniform() {
    let context = &mut setup();
    let (function, arguments) = function(context, "uniform_induction", 1);
    let bound = arguments[0];
    let entry = function.get_entry_block(context);
    let (header, induction) = index_block(context, &function, "header");
    let (body, body_induction) = index_block(context, &function, "body");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let start = IndexConstantOp::new(context, 0);
    let step = IndexConstantOp::new(context, 16);
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
    let matrix = tensor(context, 64);
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        body_induction,
        step.result(context),
    );
    let repeat = BranchArgsOp::new(context, vec![next.result(context)], header);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &start);
    append(context, entry, &step);
    append(context, entry, &enter);
    append(context, header, &condition);
    append(context, body, &matrix);
    append(context, body, &next);
    append(context, body, &repeat);
    append(context, exit, &ret);

    verify_operation(function.get_operation(), context).unwrap();
    assert!(run_pliron_tensor_layout_check_v1(context, &function).is_clean());
}

#[test]
fn nested_uniform_inductions_preserve_every_live_outer_and_inner_value() {
    let context = &mut setup();
    let (function, arguments) = function(context, "nested_uniform_inductions", 1);
    let outer_bound = arguments[0];
    let entry = function.get_entry_block(context);
    let (outer_header, outer_args) = index_block_n(context, &function, "outer_header", 1);
    let (outer_setup, setup_args) = index_block_n(context, &function, "outer_setup", 1);
    let (inner_header, inner_args) = index_block_n(context, &function, "inner_header", 3);
    let (inner_body, body_args) = index_block_n(context, &function, "inner_body", 3);
    let (outer_latch, latch_args) = index_block_n(context, &function, "outer_latch", 1);
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let zero = IndexConstantOp::new(context, 0);
    let one = IndexConstantOp::new(context, 1);
    let enter_outer = BranchArgsOp::new(context, vec![zero.result(context)], outer_header);
    let outer_condition = IndexLessThanBranchArgsOp::new(
        context,
        outer_args[0],
        outer_bound,
        vec![outer_args[0]],
        vec![],
        outer_setup,
        exit,
    );
    let inner_bound = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        setup_args[0],
        one.result(context),
    );
    let enter_inner = BranchArgsOp::new(
        context,
        vec![
            setup_args[0],
            zero.result(context),
            inner_bound.result(context),
        ],
        inner_header,
    );
    let inner_condition = IndexLessThanBranchArgsOp::new(
        context,
        inner_args[1],
        inner_args[2],
        inner_args.clone(),
        vec![inner_args[0]],
        inner_body,
        outer_latch,
    );
    let matrix = tensor(context, 64);
    let next_inner = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        body_args[1],
        one.result(context),
    );
    let repeat_inner = BranchArgsOp::new(
        context,
        vec![body_args[0], next_inner.result(context), body_args[2]],
        inner_header,
    );
    let next_outer = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        latch_args[0],
        one.result(context),
    );
    let repeat_outer = BranchArgsOp::new(context, vec![next_outer.result(context)], outer_header);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &zero);
    append(context, entry, &one);
    append(context, entry, &enter_outer);
    append(context, outer_header, &outer_condition);
    append(context, outer_setup, &inner_bound);
    append(context, outer_setup, &enter_inner);
    append(context, inner_header, &inner_condition);
    append(context, inner_body, &matrix);
    append(context, inner_body, &next_inner);
    append(context, inner_body, &repeat_inner);
    append(context, outer_latch, &next_outer);
    append(context, outer_latch, &repeat_outer);
    append(context, exit, &ret);

    verify_operation(function.get_operation(), context).unwrap();
    assert!(run_pliron_tensor_layout_check_v1(context, &function).is_clean());
}
