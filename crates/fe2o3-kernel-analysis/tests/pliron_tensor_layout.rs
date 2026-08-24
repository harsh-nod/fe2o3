use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
use dialect_kernel::{
    AnalysisSplitOp, BranchArgsOp, BranchOp, CheckedTiledIndex2DOp, DIALECT_NAME,
    DeterministicJoinOp, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp,
    IndexEqualBranchArgsOp, IndexEqualBranchOp, IndexLessThanBranchArgsOp, IndexLessThanBranchOp,
    IndexType, InvocationIndexOp, ReturnOp, TensorConvergenceAttr, TensorLayoutOp,
    register_dialect,
};
use fe2o3_kernel_analysis::{
    KernelCheckStatusV1, MAX_PLIRON_TENSOR_UNIFORMITY_VALUES_V1, PlironTensorLayoutFindingV1,
    run_pliron_tensor_layout_check_v1,
};
use fe2o3_kernel_ir::TensorLayoutContractV1;
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
    operation::{Operation, verify_operation},
    r#type::TypeHandle,
    value::Value,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    context
}

fn function(context: &mut Context, name: &str, arguments: usize) -> (FuncOp, Vec<Value>) {
    let index: TypeHandle = IndexType::get(context).into();
    let function = FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, vec![index; arguments], vec![]),
    );
    let arguments = function
        .get_entry_block(context)
        .deref(context)
        .arguments()
        .collect();
    (function, arguments)
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

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

fn layout(
    context: &mut Context,
    global_x: u64,
    workgroup_x: u64,
    subgroup: u64,
) -> ExecutionLayoutOp {
    ExecutionLayoutOp::new_with_domain(
        context,
        7,
        [global_x, 1, 1],
        [workgroup_x, 1, 1],
        subgroup,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    )
}

fn potentially_partial_layout(
    context: &mut Context,
    global_x: u64,
    workgroup_x: u64,
    subgroup: u64,
) -> ExecutionLayoutOp {
    ExecutionLayoutOp::new(context, 7, [global_x, 1, 1], [workgroup_x, 1, 1], subgroup)
}

fn tensor(context: &mut Context, active_lanes: u32) -> TensorLayoutOp {
    TensorLayoutOp::new(
        context,
        &TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
        TensorConvergenceAttr::UniformSubgroup,
        active_lanes,
    )
}

#[test]
fn exact_traces_are_compared_only_within_authenticated_subgroups() {
    let context = &mut setup();
    let (function, _) = function(context, "subgroup_scoped", 0);
    let entry = function.get_entry_block(context);
    let first_subgroup = block(context, &function, "first_subgroup");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 128, 128, 64);
    let invocation = InvocationIndexOp::new(context, 0, 128);
    let cutoff = IndexConstantOp::new(context, 64);
    let choose = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        cutoff.result(context),
        first_subgroup,
        exit,
    );
    let matrix = tensor(context, 64);
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &invocation);
    append(context, entry, &cutoff);
    append(context, entry, &choose);
    append(context, first_subgroup, &matrix);
    append(context, first_subgroup, &to_exit);
    append(context, exit, &ret);

    assert!(run_pliron_tensor_layout_check_v1(context, &function).is_clean());
}

#[test]
fn exact_traces_may_differ_across_authenticated_workgroups() {
    let context = &mut setup();
    let (function, _) = function(context, "workgroup_scoped", 0);
    let entry = function.get_entry_block(context);
    let first_workgroup = block(context, &function, "first_workgroup");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 128, 64, 64);
    let invocation = InvocationIndexOp::new(context, 0, 128);
    let cutoff = IndexConstantOp::new(context, 64);
    let choose = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        cutoff.result(context),
        first_workgroup,
        exit,
    );
    let matrix = tensor(context, 64);
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &invocation);
    append(context, entry, &cutoff);
    append(context, entry, &choose);
    append(context, first_workgroup, &matrix);
    append(context, first_workgroup, &to_exit);
    append(context, exit, &ret);

    assert!(run_pliron_tensor_layout_check_v1(context, &function).is_clean());
}

#[test]
fn a_retained_partial_subgroup_is_rejected() {
    let context = &mut setup();
    let (function, _) = function(context, "partial_subgroup", 0);
    let entry = function.get_entry_block(context);
    let execution = potentially_partial_layout(context, 65, 64, 64);
    let matrix = tensor(context, 64);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &matrix);
    append(context, entry, &ret);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(matches!(report.status(), KernelCheckStatusV1::Rejected));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::PartialSubgroupParticipation {
            grid: 7,
            workgroup: 1,
            subgroup: 0,
            expected: 64,
            actual: 1,
        }
    )));
}

#[test]
fn cyclic_symbolic_fallback_never_accepts_a_partial_subgroup() {
    let context = &mut setup();
    let (function, _) = function(context, "partial_subgroup_cycle", 0);
    let entry = function.get_entry_block(context);
    let body = block(context, &function, "body");
    let execution = potentially_partial_layout(context, 65, 64, 64);
    let enter = BranchOp::new(context, body);
    let matrix = tensor(context, 64);
    let repeat = BranchOp::new(context, body);
    append(context, entry, &execution);
    append(context, entry, &enter);
    append(context, body, &matrix);
    append(context, body, &repeat);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(matches!(report.status(), KernelCheckStatusV1::Incomplete));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail }
            if detail.contains("partial workgroup")
    )));
}

#[test]
fn forged_full_workgroups_conflicting_with_static_extent_fail_closed() {
    let context = &mut setup();
    let (function, _) = function(context, "forged_full_workgroups", 0);
    let entry = function.get_entry_block(context);
    let execution = ExecutionLayoutOp::new_with_domain(
        context,
        7,
        [65, 1, 1],
        [64, 1, 1],
        64,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    );
    let matrix = tensor(context, 64);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &matrix);
    append(context, entry, &ret);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(matches!(report.status(), KernelCheckStatusV1::Incomplete));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail }
            if detail.contains("gpu.execution_layout is malformed")
    )));
}

#[test]
fn cyclic_symbolic_fallback_never_accepts_an_unknown_global_extent() {
    let context = &mut setup();
    let (function, _) = function(context, "dynamic_subgroup_cycle", 0);
    let entry = function.get_entry_block(context);
    let body = block(context, &function, "body");
    let execution = potentially_partial_layout(context, 0, 64, 64);
    let enter = BranchOp::new(context, body);
    let matrix = tensor(context, 64);
    let repeat = BranchOp::new(context, body);
    append(context, entry, &execution);
    append(context, entry, &enter);
    append(context, body, &matrix);
    append(context, body, &repeat);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(matches!(report.status(), KernelCheckStatusV1::Incomplete));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail }
            if detail.contains("full subgroup participation")
    )));
}

#[test]
fn dense_exact_traces_charge_every_scanned_operation() {
    let context = &mut setup();
    let (function, _) = function(context, "dense_trace_budget", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, 2048, 64, 64);
    append(context, entry, &execution);
    for value in 0..512 {
        let constant = IndexConstantOp::new(context, value);
        append(context, entry, &constant);
    }
    let matrix = tensor(context, 64);
    let ret = ReturnOp::new(context);
    append(context, entry, &matrix);
    append(context, entry, &ret);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(matches!(report.status(), KernelCheckStatusV1::Incomplete));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail }
            if detail.contains("resource limit")
    )));
}

fn overflowing_affine_control_report(
    equality: bool,
) -> fe2o3_kernel_analysis::PlironTensorLayoutReportV1 {
    let context = &mut setup();
    let (function, _) = function(context, "overflowing_affine_control", 0);
    let entry = function.get_entry_block(context);
    let matrix_block = block(context, &function, "matrix");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let maximum = IndexConstantOp::new(context, u64::MAX);
    let shifted = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        invocation.result(context),
        maximum.result(context),
    );
    append(context, entry, &execution);
    append(context, entry, &invocation);
    append(context, entry, &maximum);
    append(context, entry, &shifted);
    if equality {
        let choose = IndexEqualBranchOp::new(
            context,
            invocation.result(context),
            shifted.result(context),
            matrix_block,
            exit,
        );
        append(context, entry, &choose);
    } else {
        let choose = IndexLessThanBranchOp::new(
            context,
            invocation.result(context),
            shifted.result(context),
            matrix_block,
            exit,
        );
        append(context, entry, &choose);
    }
    let matrix = tensor(context, 64);
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, matrix_block, &matrix);
    append(context, matrix_block, &to_exit);
    append(context, exit, &ret);
    run_pliron_tensor_layout_check_v1(context, &function)
}

#[test]
fn overflowing_affine_order_comparison_cannot_prove_uniform_control() {
    let report = overflowing_affine_control_report(false);
    assert!(matches!(report.status(), KernelCheckStatusV1::Incomplete));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail }
            if detail.contains("unresolved branch")
    )));
}

#[test]
fn overflowing_affine_equality_cannot_prove_uniform_control() {
    let report = overflowing_affine_control_report(true);
    assert!(matches!(report.status(), KernelCheckStatusV1::Incomplete));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail }
            if detail.contains("unresolved branch")
    )));
}

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
fn lane_derived_loop_induction_is_rejected() {
    let context = &mut setup();
    let (function, arguments) = function(context, "varying_induction", 1);
    let bound = arguments[0];
    let entry = function.get_entry_block(context);
    let (header, induction) = index_block(context, &function, "header");
    let (body, body_induction) = index_block(context, &function, "body");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let step = IndexConstantOp::new(context, 16);
    let enter = BranchArgsOp::new(context, vec![invocation.result(context)], header);
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
    append(context, entry, &invocation);
    append(context, entry, &step);
    append(context, entry, &enter);
    append(context, header, &condition);
    append(context, body, &matrix);
    append(context, body, &next);
    append(context, body, &repeat);
    append(context, exit, &ret);

    verify_operation(function.get_operation(), context).unwrap();
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
fn omitted_edge_operands_for_block_arguments_fail_incomplete() {
    let context = &mut setup();
    let (function, _) = function(context, "missing_edge_operand", 0);
    let entry = function.get_entry_block(context);
    let (header, _) = index_block(context, &function, "header");
    let execution = layout(context, 0, 64, 64);
    let enter = BranchOp::new(context, header);
    let matrix = tensor(context, 64);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &enter);
    append(context, header, &matrix);
    append(context, header, &ret);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(matches!(report.status(), KernelCheckStatusV1::Incomplete));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail }
            if detail.contains("predecessor without typed edge operands")
    )));
}

#[test]
fn malformed_conditional_edge_segments_fail_incomplete_without_panicking() {
    let context = &mut setup();
    let (function, _) = function(context, "malformed_conditional_edge", 0);
    let entry = function.get_entry_block(context);
    let (header, induction) = index_block(context, &function, "header");
    let (body, _) = index_block(context, &function, "body");
    let exit = block(context, &function, "exit");
    let execution = layout(context, 0, 64, 64);
    let start = IndexConstantOp::new(context, 0);
    let bound = IndexConstantOp::new(context, 16);
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
    Operation::pop_operand(condition.get_operation(), context);
    let matrix = tensor(context, 64);
    let body_return = ReturnOp::new(context);
    let exit_return = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &start);
    append(context, entry, &bound);
    append(context, entry, &enter);
    append(context, header, &condition);
    append(context, body, &matrix);
    append(context, body, &body_return);
    append(context, exit, &exit_return);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(matches!(report.status(), KernelCheckStatusV1::Incomplete));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail }
            if detail.contains("malformed operand count")
    )));
}

#[test]
fn uniformity_value_resource_limit_fails_closed() {
    let context = &mut setup();
    let (function, _) = function(context, "uniformity_resource_limit", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, 0, 64, 64);
    let matrix = tensor(context, 64);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &matrix);
    append(context, entry, &ret);

    let index: TypeHandle = IndexType::get(context).into();
    let oversized = BasicBlock::new(
        context,
        Some("oversized".try_into().unwrap()),
        vec![index; MAX_PLIRON_TENSOR_UNIFORMITY_VALUES_V1 + 1],
    );
    oversized.insert_at_back(function.get_region(context), context);
    let dead_return = ReturnOp::new(context);
    append(context, oversized, &dead_return);

    assert!(
        run_pliron_tensor_layout_check_v1(context, &function)
            .findings()
            .contains(&PlironTensorLayoutFindingV1::ResourceLimitExceeded)
    );
}

#[test]
fn execution_subgroup_width_must_match_the_tensor_contract() {
    let context = &mut setup();
    let (function, _) = function(context, "wrong_subgroup", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, 64, 64, 32);
    let matrix = tensor(context, 32);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &matrix);
    append(context, entry, &ret);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(matches!(report.status(), KernelCheckStatusV1::Rejected));
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::ExecutionLayoutMismatch {
            declared: 32,
            required: 64,
            ..
        }
    )));
}

#[test]
fn active_lanes_must_match_the_authenticated_execution_layout() {
    let context = &mut setup();
    let (function, _) = function(context, "wrong_active_lanes", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, 64, 64, 64);
    let matrix = tensor(context, 32);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &matrix);
    append(context, entry, &ret);

    assert!(
        run_pliron_tensor_layout_check_v1(context, &function)
            .findings()
            .iter()
            .any(|finding| matches!(
                finding,
                PlironTensorLayoutFindingV1::ActiveLaneMismatch {
                    expected: 64,
                    actual: 32,
                    ..
                }
            ))
    );
}

#[test]
fn raw_zero_fill_declaration_never_grants_compiler_or_launch_authority() {
    let context = &mut setup();
    let (function, _) = function(context, "raw_zero_fill", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, 64, 64, 64);
    let matrix = TensorLayoutOp::new(
        context,
        &TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
            .with_zero_filled_predicate_inputs(),
        TensorConvergenceAttr::UniformSubgroup,
        64,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &matrix);
    append(context, entry, &ret);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(report.is_clean());
    assert!(!report.grants_compiler_refinement_authority());
    assert!(!report.grants_artifact_or_launch_authority());
}

#[test]
fn large_reconverged_cfg_has_linear_per_tensor_analysis() {
    const STAGES: usize = 512;
    let context = &mut setup();
    let (function, _) = function(context, "large_reconverged", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, 0, 64, 64);
    append(context, entry, &execution);
    let mut controller = entry;
    for stage in 0..STAGES {
        let left = block(context, &function, &format!("left_{stage}"));
        let right = block(context, &function, &format!("right_{stage}"));
        let join = block(context, &function, &format!("join_{stage}"));
        let split = AnalysisSplitOp::new(context, left, right);
        let left_join = BranchOp::new(context, join);
        let right_join = BranchOp::new(context, join);
        append(context, controller, &split);
        append(context, left, &left_join);
        append(context, right, &right_join);
        controller = join;
    }
    let matrix = tensor(context, 64);
    let ret = ReturnOp::new(context);
    append(context, controller, &matrix);
    append(context, controller, &ret);

    assert!(run_pliron_tensor_layout_check_v1(context, &function).is_clean());
}
