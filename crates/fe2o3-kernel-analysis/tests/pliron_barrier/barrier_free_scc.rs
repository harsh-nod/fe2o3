use super::*;
use dialect_kernel::{
    AccessKindAttr, BranchArgsOp, IndexBinaryKindAttr, IndexBinaryOp, IndexLessThanBranchArgsOp,
    IndexType, MemorySpaceAttr, RankedAccessOp, RankedViewOp, RankedViewType,
};
use fe2o3_kernel_analysis::{
    KernelCheckStatusV1, PlironBarrierReportV1, run_pliron_progress_check_v1,
};
use pliron::{operation::verify_operation, r#type::TypeHandle, value::Value};

#[derive(Clone, Copy, Debug)]
enum Case {
    Memory,
    VaryingBound,
    TrapPrefix,
    TrapMismatch,
    ReturnExit,
    HiddenBarrier,
    NestedFinite,
    ZeroInnerStep,
    ZeroStep,
    WrappingStep,
    ResetInduction,
    DuplicateEqual,
    DuplicateReset,
    UnsupportedTerminator,
    UnreachableCollectiveCycle,
}

fn index_block(context: &mut Context, function: &FuncOp, name: &str) -> (Ptr<BasicBlock>, Value) {
    let index: TypeHandle = IndexType::get(context).into();
    let block = BasicBlock::new(context, Some(name.try_into().unwrap()), vec![index]);
    let argument = block.deref(context).get_argument(0);
    block.insert_at_back(function.get_region(context), context);
    (block, argument)
}

fn index_pair_block(
    context: &mut Context,
    function: &FuncOp,
    name: &str,
) -> (Ptr<BasicBlock>, [Value; 2]) {
    let index: TypeHandle = IndexType::get(context).into();
    let block = BasicBlock::new(context, Some(name.try_into().unwrap()), vec![index; 2]);
    let arguments = std::array::from_fn(|ordinal| block.deref(context).get_argument(ordinal));
    block.insert_at_back(function.get_region(context), context);
    (block, arguments)
}

fn append_barrier(context: &mut Context, block: Ptr<BasicBlock>) {
    let sync = barrier(context);
    append(context, block, &sync);
}

fn branchy_loop(context: &mut Context, case: Case) -> FuncOp {
    let function = function(context, "barrier_free_scc");
    let entry = function.get_entry_block(context);
    let (header, induction) = index_block(context, &function, "header");
    let (body, body_i) = index_block(context, &function, "body");
    let (left, left_i) = index_block(context, &function, "left");
    let (right, right_i) = index_block(context, &function, "right");
    let (latch, latch_i) = index_block(context, &function, "latch");
    let exit = block(context, &function, "exit");
    let zero = IndexConstantOp::new(context, 0);
    let step = IndexConstantOp::new(
        context,
        match case {
            Case::ZeroStep => 0,
            Case::WrappingStep => 2,
            _ => 1,
        },
    );
    let bound = IndexConstantOp::new(
        context,
        if matches!(case, Case::WrappingStep) {
            u64::MAX
        } else {
            2
        },
    );
    let invocation = InvocationIndexOp::new(context, 0, 4);
    let view_type = RankedViewType::new(context, 32, true, vec![4]).unwrap();
    let memory =
        RankedViewOp::new_in_space(context, view_type, vec![], MemorySpaceAttr::Global).unwrap();
    for operation in [
        zero.get_operation(),
        step.get_operation(),
        bound.get_operation(),
        invocation.get_operation(),
        memory.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }
    if matches!(case, Case::TrapPrefix | Case::TrapMismatch) {
        append_barrier(context, entry);
    }
    let enter = BranchArgsOp::new(context, vec![zero.result(context)], header);
    append(context, entry, &enter);
    let limit = if matches!(case, Case::VaryingBound) {
        invocation.result(context)
    } else {
        bound.result(context)
    };
    let condition = IndexLessThanBranchArgsOp::new(
        context,
        induction,
        limit,
        vec![induction],
        vec![],
        body,
        exit,
    );
    append(context, header, &condition);
    let duplicate = matches!(case, Case::DuplicateEqual | Case::DuplicateReset);
    let second_i = if matches!(case, Case::DuplicateReset) {
        zero.result(context)
    } else {
        body_i
    };
    let split = AnalysisSplitOp::new_with_control_and_arguments(
        context,
        vec![invocation.result(context)],
        vec![body_i],
        vec![second_i],
        left,
        if duplicate { left } else { right },
    );
    append(context, body, &split);
    // The loop is not pure: convergence must compose across bounded memory effects.
    for (arm, kind) in [(left, AccessKindAttr::Read), (right, AccessKindAttr::Write)] {
        if arm == right && duplicate {
            continue;
        }
        let access = RankedAccessOp::new(
            context,
            kind,
            memory.result(context),
            vec![invocation.result(context)],
        )
        .unwrap();
        append(context, arm, &access);
    }
    if matches!(case, Case::HiddenBarrier) {
        append_barrier(context, left);
    }
    let continuation = if matches!(case, Case::NestedFinite | Case::ZeroInnerStep) {
        // One merged entry resets the inner induction on every outer iteration.
        let (inner_entry, outer_i) = index_block(context, &function, "inner_entry");
        let (inner_header, header_i) = index_pair_block(context, &function, "inner_header");
        let (inner_body, body_i) = index_pair_block(context, &function, "inner_body");
        let enter_inner =
            BranchArgsOp::new(context, vec![zero.result(context), outer_i], inner_header);
        append(context, inner_entry, &enter_inner);
        let inner_condition = IndexLessThanBranchArgsOp::new(
            context,
            header_i[0],
            bound.result(context),
            header_i.to_vec(),
            vec![header_i[1]],
            inner_body,
            latch,
        );
        append(context, inner_header, &inner_condition);
        let inner_step = if matches!(case, Case::ZeroInnerStep) {
            zero.result(context)
        } else {
            step.result(context)
        };
        let inner_next =
            IndexBinaryOp::new(context, IndexBinaryKindAttr::Add, body_i[0], inner_step);
        let repeat_inner = BranchArgsOp::new(
            context,
            vec![inner_next.result(context), body_i[1]],
            inner_header,
        );
        append(context, inner_body, &inner_next);
        append(context, inner_body, &repeat_inner);
        inner_entry
    } else {
        latch
    };
    let left_latch = BranchArgsOp::new(
        context,
        vec![left_i],
        if duplicate { right } else { continuation },
    );
    append(context, left, &left_latch);
    match case {
        Case::TrapPrefix | Case::TrapMismatch => {
            if matches!(case, Case::TrapMismatch) {
                append_barrier(context, right);
            }
            let trap = TrapOp::new(context);
            append(context, right, &trap);
        }
        Case::ReturnExit => {
            let ret = ReturnOp::new(context);
            append(context, right, &ret);
        }
        _ => {
            let forwarded = if matches!(case, Case::ResetInduction) {
                zero.result(context)
            } else {
                right_i
            };
            let right_latch = BranchArgsOp::new(context, vec![forwarded], continuation);
            append(context, right, &right_latch);
        }
    }
    let next = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        latch_i,
        step.result(context),
    );
    let repeat = BranchArgsOp::new(context, vec![next.result(context)], header);
    append(context, latch, &next);
    append(context, latch, &repeat);
    append_barrier(context, exit);
    if matches!(case, Case::UnsupportedTerminator) {
        let ret = dialect_gpu::optimization_v1::ReturnOp::new(context, vec![]);
        append(context, exit, &ret);
    } else {
        let ret = ReturnOp::new(context);
        append(context, exit, &ret);
    }
    if matches!(case, Case::UnreachableCollectiveCycle) {
        let unreachable = block(context, &function, "unreachable");
        append_barrier(context, unreachable);
        let repeat = BranchOp::new(context, unreachable);
        append(context, unreachable, &repeat);
    }
    verify_operation(function.get_operation(), context).unwrap();
    function
}

fn analyze(case: Case, expected_progress: KernelCheckStatusV1) -> PlironBarrierReportV1 {
    let context = &mut setup();
    let function = branchy_loop(context, case);
    let progress = run_pliron_progress_check_v1(context, &function);
    assert_eq!(
        progress.status(),
        expected_progress,
        "{case:?}: {progress:?}"
    );
    if matches!(case, Case::NestedFinite) {
        let loops = progress
            .certificates()
            .iter()
            .map(|certificate| {
                (
                    certificate.header(),
                    certificate.body(),
                    certificate.exit(),
                    certificate.step(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(loops, [(1, 2, 6, 1), (8, 9, 5, 1)], "{progress:?}");
    }
    let bounds = fe2o3_kernel_analysis::run_pliron_ranked_bounds_check_v1(context, &function);
    assert!(
        bounds.is_clean(),
        "{case:?}: bounds prerequisite: {bounds:?}"
    );
    let report = run_pliron_barrier_convergence_check_v1(context, &function);
    assert!(!report.grants_compiler_refinement_authority());
    assert!(!report.grants_artifact_or_launch_authority());
    report
}

#[test]
fn finite_barrier_free_scc_preserves_common_barrier_and_trap_prefix() {
    for case in [
        Case::Memory,
        Case::VaryingBound,
        Case::TrapPrefix,
        Case::DuplicateEqual,
        Case::NestedFinite,
    ] {
        let report = analyze(case, KernelCheckStatusV1::Clean);
        assert_eq!(
            report.status(),
            KernelCheckStatusV1::Clean,
            "{case:?}: {report:?}"
        );
        assert!(report.findings().is_empty());
    }
}

#[test]
fn finite_barrier_free_scc_does_not_erase_divergent_exits() {
    for case in [Case::ReturnExit, Case::TrapMismatch] {
        let report = analyze(case, KernelCheckStatusV1::Clean);
        assert_eq!(
            report.status(),
            KernelCheckStatusV1::Rejected,
            "{case:?}: {report:?}"
        );
        assert!(
            matches!(
                report.findings(),
                [PlironBarrierFindingV1::DivergentBarrierPaths { first_trace, second_trace }]
                    if first_trace != second_trace
            ),
            "{case:?}: {report:?}"
        );
        assert!(
            report.findings()[0]
                .to_string()
                .contains("FE2O3-BARRIER-001")
        );
    }
}

#[test]
fn cyclic_convergence_fails_closed_without_exact_progress_and_event_contracts() {
    for (case, progress, detail) in [
        (
            Case::HiddenBarrier,
            KernelCheckStatusV1::Clean,
            "contains a barrier",
        ),
        (
            Case::ZeroInnerStep,
            KernelCheckStatusV1::Incomplete,
            "termination was not proved",
        ),
        (
            Case::ZeroStep,
            KernelCheckStatusV1::Rejected,
            "termination was not proved",
        ),
        (
            Case::WrappingStep,
            KernelCheckStatusV1::Incomplete,
            "termination was not proved",
        ),
        (
            Case::ResetInduction,
            KernelCheckStatusV1::Incomplete,
            "termination was not proved",
        ),
        (
            Case::DuplicateReset,
            KernelCheckStatusV1::Incomplete,
            "termination was not proved",
        ),
    ] {
        let report = analyze(case, progress);
        assert_eq!(
            report.status(),
            KernelCheckStatusV1::Incomplete,
            "{case:?}: {report:?}"
        );
        assert!(
            matches!(
                report.findings(),
                [PlironBarrierFindingV1::AnalysisIncomplete { detail: actual }]
                    if actual.contains(detail)
            ),
            "{case:?}: {report:?}"
        );
        assert!(
            report.findings()[0]
                .to_string()
                .contains("FE2O3-BARRIER-002")
        );
    }
}

#[test]
fn cyclic_fallback_does_not_bypass_ranked_bounds_prerequisites() {
    use fe2o3_kernel_analysis::{RankedBoundsFindingV1, run_pliron_ranked_bounds_check_v1};

    for case in [
        Case::UnsupportedTerminator,
        Case::UnreachableCollectiveCycle,
    ] {
        let context = &mut setup();
        let function = branchy_loop(context, case);
        let bounds = run_pliron_ranked_bounds_check_v1(context, &function);
        match case {
            Case::UnsupportedTerminator => assert!(
                matches!(
                    bounds.findings(),
                    [RankedBoundsFindingV1::UnsupportedTerminator { block: 6, operation }]
                        if operation == "gpu.return"
                ),
                "{bounds:?}"
            ),
            Case::UnreachableCollectiveCycle => assert!(
                matches!(
                    bounds.findings(),
                    [RankedBoundsFindingV1::UnreachableBlock { block: 7 }]
                ),
                "{bounds:?}"
            ),
            _ => unreachable!(),
        }
        let report = run_pliron_barrier_convergence_check_v1(context, &function);
        assert_eq!(
            report.findings(),
            &[PlironBarrierFindingV1::BoundsPrerequisiteRejected]
        );
        assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    }
}
