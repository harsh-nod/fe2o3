use dialect_gpu::ExecutionLayoutOp;
use dialect_kernel::{
    AnalysisSplitOp, BranchOp, DIALECT_NAME, IndexConstantOp, IndexLessThanBranchOp, IndexType,
    InvocationIndexOp, ReturnOp, TensorConvergenceAttr, TensorLayoutOp, register_dialect,
};
use fe2o3_kernel_analysis::{
    KernelCheckStatusV1, PlironTensorLayoutFindingV1, run_pliron_tensor_layout_check_v1,
};
use fe2o3_kernel_ir::TensorLayoutContractV1;
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
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

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

fn layout(
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
fn a_retained_partial_subgroup_is_rejected() {
    let context = &mut setup();
    let (function, _) = function(context, "partial_subgroup", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, 65, 64, 64);
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
