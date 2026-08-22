use dialect_gpu::{AddressSpaceAttr, BarrierOp, HierarchyAttr, MemoryOrderAttr, MemoryScopeAttr};
use dialect_kernel::{
    BranchOp, DIALECT_NAME, IndexConstantOp, IndexLessThanBranchOp, InvocationIndexOp, ReturnOp,
    register_dialect,
};
use fe2o3_kernel_analysis::{
    KernelCheckPassKindV1, PlironBarrierFindingV1,
    require_pliron_barrier_convergence_before_lowering_v1, run_pliron_barrier_convergence_check_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    context
}

fn function(context: &mut Context, name: &str) -> FuncOp {
    FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    )
}

fn block(context: &mut Context, function: &FuncOp, name: &str) -> Ptr<BasicBlock> {
    let block = BasicBlock::new(context, Some(name.try_into().unwrap()), vec![]);
    block.insert_at_back(function.get_region(context), context);
    block
}

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

fn barrier(context: &mut Context) -> BarrierOp {
    BarrierOp::new(
        context,
        HierarchyAttr::Workgroup,
        MemoryScopeAttr::Workgroup,
        AddressSpaceAttr::Workgroup,
        MemoryOrderAttr::AcquireRelease,
    )
}

#[test]
fn unconditional_barrier_is_convergent_for_a_static_launch() {
    let context = &mut setup();
    let function = function(context, "uniform_barrier");
    let entry = function.get_entry_block(context);
    let invocation = InvocationIndexOp::new(context, 0, 64);
    let sync = barrier(context);
    let ret = ReturnOp::new(context);
    append(context, entry, &invocation);
    append(context, entry, &sync);
    append(context, entry, &ret);
    let report = run_pliron_barrier_convergence_check_v1(context, &function);
    assert_eq!(report.pass(), KernelCheckPassKindV1::BarrierConvergence);
    assert!(report.is_clean());
}

#[test]
fn invocation_varying_branch_reports_exact_divergent_witnesses() {
    let context = &mut setup();
    let function = function(context, "divergent_barrier");
    let entry = function.get_entry_block(context);
    let sync_block = block(context, &function, "sync");
    let exit = block(context, &function, "exit");
    let invocation = InvocationIndexOp::new(context, 0, 4);
    let two = IndexConstantOp::new(context, 2);
    let branch = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        two.result(context),
        sync_block,
        exit,
    );
    let sync = barrier(context);
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &invocation);
    append(context, entry, &two);
    append(context, entry, &branch);
    append(context, sync_block, &sync);
    append(context, sync_block, &to_exit);
    append(context, exit, &ret);

    let error =
        require_pliron_barrier_convergence_before_lowering_v1(context, &function).unwrap_err();
    assert!(matches!(
        error.report().findings(),
        [PlironBarrierFindingV1::DivergentBarrierTrace { .. }]
    ));
    let diagnostic = error.to_string();
    assert!(diagnostic.contains("error[FE2O3-BARRIER-001]"));
    assert!(diagnostic.contains("invocation [0]"));
    assert!(diagnostic.contains("invocation [2]"));
    assert!(diagnostic.contains("move the barrier out of invocation-varying control flow"));
}

#[test]
fn unresolved_branch_that_reconverges_before_barrier_is_clean() {
    let context = &mut setup();
    let function = function(context, "reconverged_barrier");
    let entry = function.get_entry_block(context);
    let left = block(context, &function, "left");
    let right = block(context, &function, "right");
    let join = block(context, &function, "join");
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let four = IndexConstantOp::new(context, 4);
    let choose = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        four.result(context),
        left,
        right,
    );
    let left_join = BranchOp::new(context, join);
    let right_join = BranchOp::new(context, join);
    let sync = barrier(context);
    let ret = ReturnOp::new(context);
    append(context, entry, &invocation);
    append(context, entry, &four);
    append(context, entry, &choose);
    append(context, left, &left_join);
    append(context, right, &right_join);
    append(context, join, &sync);
    append(context, join, &ret);
    assert!(run_pliron_barrier_convergence_check_v1(context, &function).is_clean());
}

#[test]
fn unresolved_divergent_paths_are_rejected_without_inventing_a_trace() {
    let context = &mut setup();
    let function = function(context, "unresolved_divergent_barrier");
    let entry = function.get_entry_block(context);
    let sync_block = block(context, &function, "sync");
    let exit = block(context, &function, "exit");
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let one = IndexConstantOp::new(context, 1);
    let choose = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        one.result(context),
        sync_block,
        exit,
    );
    let sync = barrier(context);
    let to_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &invocation);
    append(context, entry, &one);
    append(context, entry, &choose);
    append(context, sync_block, &sync);
    append(context, sync_block, &to_exit);
    append(context, exit, &ret);

    let error =
        require_pliron_barrier_convergence_before_lowering_v1(context, &function).unwrap_err();
    assert!(matches!(
        error.report().findings(),
        [PlironBarrierFindingV1::DivergentBarrierPaths { .. }]
    ));
    assert!(
        error
            .to_string()
            .contains("divergent collective barrier paths")
    );
}

#[test]
fn equal_barrier_count_at_different_program_points_is_rejected() {
    let context = &mut setup();
    let function = function(context, "different_barriers");
    let entry = function.get_entry_block(context);
    let left = block(context, &function, "left");
    let right = block(context, &function, "right");
    let exit = block(context, &function, "exit");
    let invocation = InvocationIndexOp::new(context, 0, 2);
    let one = IndexConstantOp::new(context, 1);
    let choose = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        one.result(context),
        left,
        right,
    );
    let left_barrier = barrier(context);
    let right_barrier = barrier(context);
    let left_exit = BranchOp::new(context, exit);
    let right_exit = BranchOp::new(context, exit);
    let ret = ReturnOp::new(context);
    append(context, entry, &invocation);
    append(context, entry, &one);
    append(context, entry, &choose);
    append(context, left, &left_barrier);
    append(context, left, &left_exit);
    append(context, right, &right_barrier);
    append(context, right, &right_exit);
    append(context, exit, &ret);
    assert!(!run_pliron_barrier_convergence_check_v1(context, &function).is_clean());
}

#[test]
fn cyclic_control_flow_fails_closed_but_dynamic_unconditional_flow_is_proved() {
    let context = &mut setup();
    let cyclic = function(context, "cyclic");
    let entry = cyclic.get_entry_block(context);
    let invocation = InvocationIndexOp::new(context, 0, 4);
    let sync = barrier(context);
    let backedge = BranchOp::new(context, entry);
    append(context, entry, &invocation);
    append(context, entry, &sync);
    append(context, entry, &backedge);
    let report = run_pliron_barrier_convergence_check_v1(context, &cyclic);
    assert!(matches!(
        report.findings(),
        [PlironBarrierFindingV1::AnalysisIncomplete { .. }]
    ));

    let dynamic = function(context, "dynamic");
    let entry = dynamic.get_entry_block(context);
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let sync = barrier(context);
    let ret = ReturnOp::new(context);
    append(context, entry, &invocation);
    append(context, entry, &sync);
    append(context, entry, &ret);
    let report = run_pliron_barrier_convergence_check_v1(context, &dynamic);
    assert!(report.is_clean());
}
