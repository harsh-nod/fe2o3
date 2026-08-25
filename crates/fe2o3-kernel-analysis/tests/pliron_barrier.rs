use dialect_gpu::{
    AddressSpaceAttr, BarrierOp, ExecutionDomainAttr, ExecutionLayoutOp, HierarchyAttr,
    MemoryOrderAttr, MemoryScopeAttr,
};
use dialect_kernel::{
    BranchOp, DIALECT_NAME, IndexConstantOp, IndexLessThanBranchOp, InvocationIndexOp, ReturnOp,
    TrapOp, register_dialect,
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
    function_with_layout(context, name, 4, 4)
}

fn function_with_layout(
    context: &mut Context,
    name: &str,
    workgroup_size: u64,
    subgroup_size: u64,
) -> FuncOp {
    function_with_domain(context, name, workgroup_size, workgroup_size, subgroup_size)
}

fn function_with_domain(
    context: &mut Context,
    name: &str,
    global_extent: u64,
    workgroup_size: u64,
    subgroup_size: u64,
) -> FuncOp {
    let function = FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    );
    let layout = ExecutionLayoutOp::new(
        context,
        7,
        [global_extent, 1, 1],
        [workgroup_size, 1, 1],
        subgroup_size,
    );
    append(context, function.get_entry_block(context), &layout);
    function
}

fn function_with_full_physical_workgroups(
    context: &mut Context,
    name: &str,
    global_extent: u64,
    workgroup_size: u64,
    subgroup_size: u64,
) -> FuncOp {
    let function = FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    );
    let layout = ExecutionLayoutOp::new_with_domain(
        context,
        7,
        [global_extent, 1, 1],
        [workgroup_size, 1, 1],
        subgroup_size,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    );
    append(context, function.get_entry_block(context), &layout);
    function
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
    let function = function_with_domain(context, "uniform_barrier", 64, 4, 4);
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
    assert!(diagnostic.contains("invocation [0, 0, 0]"));
    assert!(diagnostic.contains("invocation [2, 0, 0]"));
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
fn terminal_trap_may_end_before_a_reconverged_barrier() {
    let context = &mut setup();
    let function = function_with_full_physical_workgroups(context, "guarded_barrier", 0, 4, 4);
    let entry = function.get_entry_block(context);
    let access = block(context, &function, "access");
    let trap = block(context, &function, "trap");
    let join = block(context, &function, "join");
    let invocation = InvocationIndexOp::new(context, 0, 0);
    let one = IndexConstantOp::new(context, 1);
    let guard = IndexLessThanBranchOp::new(
        context,
        invocation.result(context),
        one.result(context),
        access,
        trap,
    );
    let to_join = BranchOp::new(context, join);
    let abort = TrapOp::new(context);
    let sync = barrier(context);
    let ret = ReturnOp::new(context);
    append(context, entry, &invocation);
    append(context, entry, &one);
    append(context, entry, &guard);
    append(context, access, &to_join);
    append(context, trap, &abort);
    append(context, join, &sync);
    append(context, join, &ret);

    assert!(run_pliron_barrier_convergence_check_v1(context, &function).is_clean());
}

#[test]
fn unresolved_divergent_paths_are_rejected_without_inventing_a_trace() {
    let context = &mut setup();
    let function =
        function_with_full_physical_workgroups(context, "unresolved_divergent_barrier", 0, 4, 4);
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
fn dynamic_uniform_barrier_without_participant_provenance_fails_closed() {
    let context = &mut setup();
    let function = function_with_domain(context, "dynamic_partial_barrier", 0, 4, 4);
    let entry = function.get_entry_block(context);
    let sync = barrier(context);
    let ret = ReturnOp::new(context);
    append(context, entry, &sync);
    append(context, entry, &ret);

    let report = run_pliron_barrier_convergence_check_v1(context, &function);
    assert!(!report.is_clean());
    assert!(report.findings().iter().any(|finding| matches!(
        finding,
        PlironBarrierFindingV1::AnalysisIncomplete { detail }
            if detail.contains("full physical workgroups")
    )));
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
    assert!(
        report.findings()[0]
            .to_string()
            .contains("progress-dependent spin synchronization is unsupported")
    );

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

#[test]
fn subgroup_collective_protocol_is_checked_per_wave() {
    for (cutoff_value, scope, clean) in [
        (64, HierarchyAttr::Subgroup, true),
        (32, HierarchyAttr::Subgroup, false),
        (64, HierarchyAttr::Workgroup, false),
    ] {
        let context = &mut setup();
        let function = function_with_layout(context, "scoped_collective", 128, 64);
        let entry = function.get_entry_block(context);
        let collective = block(context, &function, "collective");
        let exit = block(context, &function, "exit");
        let invocation = InvocationIndexOp::new(context, 0, 128);
        let cutoff = IndexConstantOp::new(context, cutoff_value);
        let choose = IndexLessThanBranchOp::new(
            context,
            invocation.result(context),
            cutoff.result(context),
            collective,
            exit,
        );
        let sync = BarrierOp::new(
            context,
            scope,
            if scope == HierarchyAttr::Subgroup {
                MemoryScopeAttr::Subgroup
            } else {
                MemoryScopeAttr::Workgroup
            },
            AddressSpaceAttr::Workgroup,
            MemoryOrderAttr::AcquireRelease,
        );
        let leave = BranchOp::new(context, exit);
        let ret = ReturnOp::new(context);
        append(context, entry, &invocation);
        append(context, entry, &cutoff);
        append(context, entry, &choose);
        append(context, collective, &sync);
        append(context, collective, &leave);
        append(context, exit, &ret);
        assert_eq!(
            run_pliron_barrier_convergence_check_v1(context, &function).is_clean(),
            clean,
            "unexpected convergence result for cutoff={cutoff_value} scope={scope:?}",
        );
    }
}

#[test]
fn ordinary_grid_barrier_is_reported_as_unsupported() {
    let context = &mut setup();
    let function = function_with_domain(context, "unsupported_grid_barrier", 128, 4, 4);
    let entry = function.get_entry_block(context);
    let invocation = InvocationIndexOp::new(context, 0, 128);
    let sync = BarrierOp::new(
        context,
        HierarchyAttr::Grid,
        MemoryScopeAttr::Device,
        AddressSpaceAttr::Global,
        MemoryOrderAttr::AcquireRelease,
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &invocation);
    append(context, entry, &sync);
    append(context, entry, &ret);
    let report = run_pliron_barrier_convergence_check_v1(context, &function);
    assert!(matches!(
        report.findings(),
        [PlironBarrierFindingV1::AnalysisIncomplete { detail }]
            if detail.contains("ordinary grid-wide barriers are unsupported")
    ));
}

#[test]
fn workgroup_barrier_requires_a_full_physical_participant_set() {
    for (extent, clean) in [(65, false), (128, true)] {
        let context = &mut setup();
        let function = function_with_domain(context, "full_workgroups", extent, 64, 64);
        let entry = function.get_entry_block(context);
        let invocation = InvocationIndexOp::new(context, 0, extent);
        let sync = barrier(context);
        let ret = ReturnOp::new(context);
        append(context, entry, &invocation);
        append(context, entry, &sync);
        append(context, entry, &ret);
        let report = run_pliron_barrier_convergence_check_v1(context, &function);
        assert_eq!(
            report.is_clean(),
            clean,
            "unexpected result for extent {extent}"
        );
        if !clean {
            assert!(matches!(
                report.findings(),
                [PlironBarrierFindingV1::AnalysisIncomplete { detail }]
                    if detail.contains("global extent 65")
                        && detail.contains("workgroup extent 64")
            ));
        }
    }
}

#[test]
fn partial_workgroups_are_rejected_per_axis_not_by_linear_volume() {
    for (global_x, clean) in [(65, false), (128, true)] {
        let context = &mut setup();
        let function = FuncOp::new(
            context,
            "two_dimensional_workgroups".try_into().unwrap(),
            FunctionType::get(context, vec![], vec![]),
        );
        let entry = function.get_entry_block(context);
        let layout = ExecutionLayoutOp::new(context, 8, [global_x, 64, 1], [64, 1, 1], 64);
        let x = InvocationIndexOp::new(context, 0, global_x);
        let y = InvocationIndexOp::new(context, 1, 64);
        let sync = barrier(context);
        let ret = ReturnOp::new(context);
        append(context, entry, &layout);
        append(context, entry, &x);
        append(context, entry, &y);
        append(context, entry, &sync);
        append(context, entry, &ret);

        let report = run_pliron_barrier_convergence_check_v1(context, &function);
        assert_eq!(
            report.is_clean(),
            clean,
            "unexpected result for x={global_x}"
        );
        if !clean {
            assert!(matches!(
                report.findings(),
                [PlironBarrierFindingV1::AnalysisIncomplete { detail }]
                    if detail.contains("axis 0")
                        && detail.contains("global extent 65")
                        && detail.contains("workgroup extent 64")
            ));
        }
    }
}
