use crate::{
    KernelCheckStatusV1, MAX_PLIRON_TENSOR_UNIFORMITY_VALUES_V1,
    MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1, PlironTensorLayoutDataflowIssueV1,
    PlironTensorLayoutFindingV1, run_pliron_tensor_layout_check_v1,
};
use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
use dialect_kernel::{
    AnalysisSplitOp, BranchArgsOp, BranchOp, CheckedTiledIndex2DOp, DIALECT_NAME,
    DeterministicJoinOp, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp,
    IndexEqualBranchArgsOp, IndexEqualBranchOp, IndexLessThanBranchArgsOp, IndexLessThanBranchOp,
    IndexType, IndexUnknownOp, IndexUnsignedCastOp, InvocationIndexOp, ReturnOp,
    TensorConvergenceAttr, TensorDataflowRootsV1, TensorLayoutOp, register_dialect,
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

fn index_block_n(
    context: &mut Context,
    function: &FuncOp,
    name: &str,
    count: usize,
) -> (Ptr<BasicBlock>, Vec<Value>) {
    let index: TypeHandle = IndexType::get(context).into();
    let block = BasicBlock::new(context, Some(name.try_into().unwrap()), vec![index; count]);
    let arguments = block.deref(context).arguments().collect();
    block.insert_at_back(function.get_region(context), context);
    (block, arguments)
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

fn root(identity: u64) -> [u64; 4] {
    [identity, 0, 0, 0]
}

fn bound_tensor(
    context: &mut Context,
    contract: &TensorLayoutContractV1,
    lhs: u64,
    rhs: u64,
    accumulator: u64,
    result: u64,
) -> TensorLayoutOp {
    TensorLayoutOp::new_with_dataflow_roots(
        context,
        contract,
        TensorConvergenceAttr::UniformSubgroup,
        64,
        TensorDataflowRootsV1 {
            lhs: root(lhs),
            rhs: root(rhs),
            accumulator: root(accumulator),
            result: root(result),
        },
    )
}

fn dataflow_issue(
    finding: &PlironTensorLayoutFindingV1,
) -> Option<&PlironTensorLayoutDataflowIssueV1> {
    match finding {
        PlironTensorLayoutFindingV1::Dataflow(issue) => Some(issue.as_ref()),
        _ => None,
    }
}

#[test]
fn tensor_layout_dataflow_accepts_a_compatible_accumulator_chain() {
    let context = &mut setup();
    let (function, _) = function(context, "compatible_tensor_chain", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, 64, 64, 64);
    let contract = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
    let producer = bound_tensor(context, &contract, 1, 2, 3, 10);
    let consumer = bound_tensor(context, &contract, 4, 5, 10, 11);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &producer);
    append(context, entry, &consumer);
    append(context, entry, &ret);

    assert!(run_pliron_tensor_layout_check_v1(context, &function).is_clean());
}

#[test]
fn tensor_layout_dataflow_rejects_reinterpreting_an_accumulator_as_an_operand() {
    let context = &mut setup();
    let (function, _) = function(context, "incompatible_tensor_composition", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, 64, 64, 64);
    let contract = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
    let producer = bound_tensor(context, &contract, 1, 2, 3, 10);
    let consumer = bound_tensor(context, &contract, 10, 4, 5, 11);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &producer);
    append(context, entry, &consumer);
    append(context, entry, &ret);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(report.findings().iter().any(|finding| matches!(
        dataflow_issue(finding),
        Some(PlironTensorLayoutDataflowIssueV1::ConsumerMismatch {
            operand: fe2o3_kernel_ir::TensorOperandRoleV1::A,
            ..
        })
    )));
    let message = report
        .findings()
        .iter()
        .find(|finding| matches!(finding, PlironTensorLayoutFindingV1::Dataflow(_)))
        .unwrap()
        .to_string();
    assert!(message.contains("FE2O3-TENSOR-LAYOUT-005"));
    assert!(message.contains("insert a checked conversion/repack"));
    assert!(message.contains("profile Gfx942MfmaBf16F32M16N16K16Wave64"));
}

#[test]
fn different_tensor_instruction_profiles_are_diagnosed_at_the_composition_site() {
    let context = &mut setup();
    let (function, _) = function(context, "different_tensor_profiles", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, 64, 64, 64);
    let producer_contract = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
    let mut consumer_contract = producer_contract;
    consumer_contract.profile = fe2o3_kernel_ir::TensorInstructionProfileV1::IncompatibleWave32;
    consumer_contract.subgroup_width = 32;
    let producer = bound_tensor(context, &producer_contract, 1, 2, 3, 10);
    let consumer = bound_tensor(context, &consumer_contract, 4, 5, 10, 11);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &producer);
    append(context, entry, &consumer);
    append(context, entry, &ret);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    let message = report
        .findings()
        .iter()
        .find_map(|finding| match dataflow_issue(finding) {
            Some(PlironTensorLayoutDataflowIssueV1::ConsumerMismatch {
                consumer_profile: fe2o3_kernel_ir::TensorInstructionProfileV1::IncompatibleWave32,
                operand: fe2o3_kernel_ir::TensorOperandRoleV1::Accumulator,
                ..
            }) => Some(finding.to_string()),
            _ => None,
        })
        .expect("cross-profile composition finding");
    assert!(message.contains("select a consumer instruction"));
    assert!(message.contains("Gfx942MfmaBf16F32M16N16K16Wave64"));
}

#[test]
fn an_unproduced_root_does_not_fabricate_a_layout_fact() {
    let context = &mut setup();
    let (function, _) = function(context, "checked_conversion_boundary", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, 64, 64, 64);
    let contract = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
    let producer = bound_tensor(context, &contract, 1, 2, 3, 10);
    // The standalone pass has no producer fact for root 12 and therefore does
    // not invent one. Production accepts such a root only when authenticated
    // source projection retained an external checked load or initializer.
    let consumer = bound_tensor(context, &contract, 12, 4, 5, 11);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &producer);
    append(context, entry, &consumer);
    append(context, entry, &ret);

    assert!(run_pliron_tensor_layout_check_v1(context, &function).is_clean());
}

#[test]
fn tensor_layout_dataflow_rejects_incompatible_producer_join() {
    let context = &mut setup();
    let (function, _) = function(context, "incompatible_tensor_join", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, 64, 64, 64);
    let canonical = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
    let mut incompatible = canonical;
    incompatible.accumulator.fragment_elements = 3;
    let first = bound_tensor(context, &canonical, 1, 2, 3, 10);
    let second = bound_tensor(context, &incompatible, 4, 5, 6, 10);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &first);
    append(context, entry, &second);
    append(context, entry, &ret);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert!(report.findings().iter().any(|finding| matches!(
        dataflow_issue(finding),
        Some(PlironTensorLayoutDataflowIssueV1::MergeConflict { .. })
    )));
    assert!(
        report
            .findings()
            .iter()
            .map(ToString::to_string)
            .any(|message| message.contains("same fragment layout"))
    );
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
fn an_exact_workgroup_partial_final_subgroup_cannot_upgrade_full_wave_tensor_proof() {
    let context = &mut setup();
    let (function, _) = function(context, "exact_workgroup_partial_subgroup", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, 65, 65, 64);
    let matrix = tensor(context, 64);
    let ret = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &matrix);
    append(context, entry, &ret);

    let report = run_pliron_tensor_layout_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironTensorLayoutFindingV1::PartialSubgroupParticipation {
            grid: 7,
            workgroup: 0,
            subgroup: 1,
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
fn cyclic_symbolic_fallback_never_accepts_an_exact_workgroup_partial_subgroup() {
    let context = &mut setup();
    let (function, _) = function(context, "exact_workgroup_partial_subgroup_cycle", 0);
    let entry = function.get_entry_block(context);
    let body = block(context, &function, "body");
    let execution = layout(context, 65, 65, 64);
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
            if detail.contains("partial subgroup")
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

fn overflowing_affine_control_report(equality: bool) -> crate::PlironTensorLayoutReportV1 {
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

include!("pliron_tensor_layout/control_uniformity_tests.rs");
fn quotient_control_report(
    divisor: Option<u64>,
    dimension: u32,
    global_extents: [u64; 3],
    workgroup_extents: [u64; 3],
    launch_extent: u64,
) -> crate::PlironTensorLayoutReportV1 {
    let context = &mut setup();
    let argument_count = usize::from(divisor.is_none());
    let (function, arguments) = function(context, "quotient_control", argument_count);
    let entry = function.get_entry_block(context);
    let matrix_block = block(context, &function, "matrix");
    let exit = block(context, &function, "exit");
    let execution = ExecutionLayoutOp::new_with_domain(
        context,
        7,
        global_extents,
        workgroup_extents,
        64,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    );
    let invocation = InvocationIndexOp::new(context, dimension, launch_extent);
    let constant = divisor.map(|value| IndexConstantOp::new(context, value));
    let divisor = constant
        .as_ref()
        .map_or_else(|| arguments[0], |constant| constant.result(context));
    let quotient = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Divide,
        invocation.result(context),
        divisor,
    );
    let one = IndexConstantOp::new(context, 1);
    let choose = IndexLessThanBranchOp::new(
        context,
        quotient.result(context),
        one.result(context),
        matrix_block,
        exit,
    );
    let matrix = tensor(context, 64);
    let matrix_return = ReturnOp::new(context);
    let exit_return = ReturnOp::new(context);
    append(context, entry, &execution);
    append(context, entry, &invocation);
    if let Some(constant) = &constant {
        append(context, entry, constant);
    }
    append(context, entry, &quotient);
    append(context, entry, &one);
    append(context, entry, &choose);
    append(context, matrix_block, &matrix);
    append(context, matrix_block, &matrix_return);
    append(context, exit, &exit_return);
    run_pliron_tensor_layout_check_v1(context, &function)
}

#[test]
fn positive_subgroup_multiple_quotients_are_uniform() {
    for divisor in [64, 128] {
        assert!(
            quotient_control_report(Some(divisor), 0, [128, 1, 1], [128, 1, 1], 128).is_clean()
        );
    }
}

#[test]
fn invalid_or_unproven_subgroup_quotients_never_mint_uniformity() {
    let reports = [
        quotient_control_report(Some(0), 0, [128, 1, 1], [128, 1, 1], 128),
        quotient_control_report(Some(96), 0, [128, 1, 1], [128, 1, 1], 128),
        quotient_control_report(None, 0, [128, 1, 1], [128, 1, 1], 128),
        quotient_control_report(Some(1), 1, [32, 4, 1], [32, 4, 1], 4),
        quotient_control_report(Some(64), 0, [32, 2, 1], [32, 2, 1], 32),
        quotient_control_report(Some(64), 0, [96, 1, 1], [96, 1, 1], 128),
    ];
    for report in reports {
        assert!(!report.is_clean(), "an unproved quotient was accepted");
    }
}

#[test]
fn quotient_remainder_and_lane_offset_do_not_inherit_uniformity() {
    for operation in [IndexBinaryKindAttr::Add, IndexBinaryKindAttr::Remainder] {
        let context = &mut setup();
        let (function, _) = function(context, "quotient_then_varying", 0);
        let entry = function.get_entry_block(context);
        let matrix_block = block(context, &function, "matrix");
        let exit = block(context, &function, "exit");
        let execution = layout(context, 128, 128, 64);
        let invocation = InvocationIndexOp::new(context, 0, 128);
        let divisor = IndexConstantOp::new(context, 64);
        let quotient = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Divide,
            invocation.result(context),
            divisor.result(context),
        );
        let varying = IndexBinaryOp::new(
            context,
            operation,
            quotient.result(context),
            invocation.result(context),
        );
        let cutoff = IndexConstantOp::new(context, 32);
        let choose = IndexLessThanBranchOp::new(
            context,
            varying.result(context),
            cutoff.result(context),
            matrix_block,
            exit,
        );
        let matrix = tensor(context, 64);
        let matrix_return = ReturnOp::new(context);
        let exit_return = ReturnOp::new(context);
        append(context, entry, &execution);
        append(context, entry, &invocation);
        append(context, entry, &divisor);
        append(context, entry, &quotient);
        append(context, entry, &varying);
        append(context, entry, &cutoff);
        append(context, entry, &choose);
        append(context, matrix_block, &matrix);
        append(context, matrix_block, &matrix_return);
        append(context, exit, &exit_return);
        assert!(!run_pliron_tensor_layout_check_v1(context, &function).is_clean());
    }
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
fn repeated_high_fan_in_dependencies_are_charged_before_collection() {
    let context = &mut setup();
    let (function, _) = function(context, "uniformity_fan_in_limit", 0);
    let entry = function.get_entry_block(context);
    let execution = layout(context, 0, 64, 64);
    let zero = IndexConstantOp::new(context, 0);
    append(context, entry, &execution);
    append(context, entry, &zero);
    let dependencies = vec![zero.result(context); 64];
    for _ in 0..=(MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1 / dependencies.len()) {
        let join = DeterministicJoinOp::new(context, dependencies.clone());
        append(context, entry, &join);
    }
    let matrix = tensor(context, 64);
    let ret = ReturnOp::new(context);
    append(context, entry, &matrix);
    append(context, entry, &ret);

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
